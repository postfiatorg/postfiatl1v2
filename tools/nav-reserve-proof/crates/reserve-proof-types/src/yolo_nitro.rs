//! Nitro COSE/X.509 verification with deterministic, caller-committed time.
//! x509-parser is used only for parsing; RustCrypto verifies every signature.

use crate::yolo_cbor::{decode_nitro_cbor, decode_strict_cbor, NITRO_CBOR_LIMITS};
use crate::yolo_collection::{canonical_bytes, validate_digest};
use crate::yolo_witness::{decode_hex, NitroProofPolicyV1};
use p384::ecdsa::{signature::Verifier, Signature, VerifyingKey};
use p384::pkcs8::DecodePublicKey;
use rsa::{traits::PublicKeyParts, RsaPublicKey};
use serde_cbor::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use x509_parser::{certificate::X509Certificate, parse_x509_certificate};

pub const NITRO_USER_DATA_SCHEMA_V2: &str = "postfiat.yolo.nitro_user_data.v2";
const ECDSA_SHA384_OID: &str = concat!("1.2.840", ".10045.4.3.3");
const ALLOWED_CRITICAL_EXTENSION_OIDS: &[&str] = &[
    concat!("2.5.29", ".19"),
    concat!("2.5.29", ".15"),
    concat!("2.5.29", ".14"),
    concat!("2.5.29", ".35"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedNitroClaims {
    pub pcrs: BTreeMap<u8, Vec<u8>>,
    pub statement_key: String,
    pub recipient_spki: Vec<u8>,
    pub timestamp_ms: u64,
}

fn bytes<'a>(value: &'a Value, maximum: usize, name: &str) -> Result<&'a [u8], String> {
    match value {
        Value::Bytes(value) if !value.is_empty() && value.len() <= maximum => Ok(value),
        _ => Err(format!("Nitro {name} byte length or type is invalid")),
    }
}

fn map<'a>(value: &'a Value, name: &str) -> Result<&'a BTreeMap<Value, Value>, String> {
    match value {
        Value::Map(value) => Ok(value),
        _ => Err(format!("Nitro {name} must be a map")),
    }
}

fn field<'a>(map: &'a BTreeMap<Value, Value>, name: &str) -> Result<&'a Value, String> {
    map.get(&Value::Text(name.into()))
        .ok_or_else(|| format!("Nitro missing field {name}"))
}

fn parse_certificate(input: &[u8]) -> Result<X509Certificate<'_>, String> {
    if input.is_empty() || input.len() > 1024 {
        return Err("Nitro certificate length is invalid".into());
    }
    let (remaining, cert) =
        parse_x509_certificate(input).map_err(|_| "Nitro certificate is not DER")?;
    if !remaining.is_empty() {
        return Err("Nitro certificate has trailing DER".into());
    }
    if cert.signature_algorithm != cert.tbs_certificate.signature
        || cert.signature_algorithm.algorithm.to_id_string() != ECDSA_SHA384_OID
        || cert.signature_algorithm.parameters.is_some()
        || cert.signature_value.unused_bits != 0
    {
        return Err("Nitro certificate signature algorithm must be ECDSA SHA-384".into());
    }
    let mut extensions = BTreeSet::new();
    for extension in cert.extensions() {
        let oid = extension.oid.to_id_string();
        if !extensions.insert(oid.clone()) {
            return Err("Nitro certificate has duplicate extensions".into());
        }
        // Other critical extensions would impose constraints this verifier does
        // not implement. Unknown noncritical extensions carry no authority.
        if extension.critical && !ALLOWED_CRITICAL_EXTENSION_OIDS.contains(&oid.as_str()) {
            return Err("Nitro certificate has an unsupported critical extension".into());
        }
        if matches!(
            extension.parsed_extension(),
            x509_parser::extensions::ParsedExtension::ParseError { .. }
        ) {
            return Err("Nitro certificate extension is malformed".into());
        }
    }
    Ok(cert)
}

fn verify_certificate_signature(
    child: &X509Certificate<'_>,
    issuer: &X509Certificate<'_>,
) -> Result<(), String> {
    if child.issuer() != issuer.subject() {
        return Err("Nitro certificate issuer mismatch".into());
    }
    let key = VerifyingKey::from_public_key_der(issuer.public_key().raw)
        .map_err(|_| "Nitro issuer key must be P-384 SPKI")?;
    let signature = Signature::from_der(&child.signature_value.data)
        .map_err(|_| "Nitro certificate signature is malformed")?;
    key.verify(child.tbs_certificate.as_ref(), &signature)
        .map_err(|_| "Nitro certificate signature is invalid".into())
}

fn verify_chain<'a>(
    leaf: &'a [u8],
    bundle: &'a [Value],
    root: &[u8],
    time_ms: u64,
) -> Result<X509Certificate<'a>, String> {
    if bundle.is_empty() || bundle.len() > 16 || bytes(&bundle[0], 1024, "root")? != root {
        return Err("Nitro CA bundle must begin with the pinned root".into());
    }
    let mut chain_bytes = vec![leaf];
    for value in bundle.iter().rev() {
        chain_bytes.push(bytes(value, 1024, "certificate")?);
    }
    let mut unique = BTreeSet::new();
    let mut chain = Vec::new();
    for encoded in chain_bytes {
        if !unique.insert(encoded) {
            return Err("Nitro certificate chain contains a duplicate".into());
        }
        chain.push(parse_certificate(encoded)?);
    }
    let time = i128::from(time_ms);
    for (index, cert) in chain.iter().enumerate() {
        if i128::from(cert.validity().not_before.timestamp()) * 1000 > time
            || time > i128::from(cert.validity().not_after.timestamp()) * 1000
        {
            return Err("Nitro certificate is outside its validity interval".into());
        }
        let constraints = cert
            .basic_constraints()
            .map_err(|_| "Nitro basic constraints are invalid")?
            .ok_or("Nitro basic constraints are required")?;
        let usage = cert
            .key_usage()
            .map_err(|_| "Nitro key usage is invalid")?
            .ok_or("Nitro key usage is required")?;
        if index == 0 {
            if constraints.value.ca
                || constraints.value.path_len_constraint.is_some()
                || !usage.value.digital_signature()
            {
                return Err("Nitro leaf certificate constraints are invalid".into());
            }
        } else if !constraints.value.ca
            || !usage.value.key_cert_sign()
            || constraints
                .value
                .path_len_constraint
                .is_some_and(|limit| index - 1 > limit as usize)
        {
            return Err("Nitro CA authority or path length is invalid".into());
        }
    }
    for pair in chain.windows(2) {
        verify_certificate_signature(&pair[0], &pair[1])?;
    }
    let root = chain.last().ok_or("Nitro certificate chain is empty")?;
    verify_certificate_signature(root, root)?;
    Ok(chain.remove(0))
}

pub fn verify_nitro_document(
    document: &[u8],
    root_der: &[u8],
    policy: &NitroProofPolicyV1,
    verification_time_ms: u64,
    binding_digest: &str,
) -> Result<VerifiedNitroClaims, String> {
    NitroVerifier::new(root_der, policy, verification_time_ms)?.verify(document, binding_digest)
}

/// Cache only certificate chains already verified inside this computation.
/// Root, policy and verification time are immutable for the cache's lifetime.
/// Every document still receives its own COSE and statement-binding checks.
pub struct NitroVerifier<'a> {
    root_der: &'a [u8],
    policy: &'a NitroProofPolicyV1,
    verification_time_ms: u64,
    chains: BTreeMap<[u8; 32], VerifyingKey>,
}

impl<'a> NitroVerifier<'a> {
    pub fn new(
        root_der: &'a [u8],
        policy: &'a NitroProofPolicyV1,
        verification_time_ms: u64,
    ) -> Result<Self, String> {
        policy.validate()?;
        if root_der.len() > 1024
            || hex::encode(Sha256::digest(root_der)) != policy.root_certificate_sha256
        {
            return Err("Nitro root does not match the committed policy".into());
        }
        Ok(Self {
            root_der,
            policy,
            verification_time_ms,
            chains: BTreeMap::new(),
        })
    }

    fn leaf_key(&mut self, leaf: &[u8], bundle: &[Value]) -> Result<VerifyingKey, String> {
        if leaf.is_empty() || leaf.len() > 1024 || bundle.is_empty() || bundle.len() > 16 {
            return Err("Nitro certificate chain exceeds its bounds".into());
        }
        let mut hash = Sha256::new();
        hash.update(b"postfiat.yolo.nitro_certificate_chain.v1\0");
        hash.update((leaf.len() as u32).to_be_bytes());
        hash.update(leaf);
        hash.update((bundle.len() as u32).to_be_bytes());
        for cert in bundle {
            let cert = bytes(cert, 1024, "certificate")?;
            hash.update((cert.len() as u32).to_be_bytes());
            hash.update(cert);
        }
        let digest: [u8; 32] = hash.finalize().into();
        if let Some(key) = self.chains.get(&digest) {
            return Ok(key.clone());
        }
        if self.chains.len() >= crate::yolo_witness::MAX_STATEMENTS {
            return Err("Nitro distinct certificate chain count exceeds its bound".into());
        }
        let leaf = verify_chain(leaf, bundle, self.root_der, self.verification_time_ms)?;
        let key = VerifyingKey::from_public_key_der(leaf.public_key().raw)
            .map_err(|_| "Nitro COSE signer must use P-384")?;
        self.chains.insert(digest, key.clone());
        Ok(key)
    }

    pub fn verify(
        &mut self,
        document: &[u8],
        binding_digest: &str,
    ) -> Result<VerifiedNitroClaims, String> {
        let policy = self.policy;
        let verification_time_ms = self.verification_time_ms;
        validate_digest("attestation binding", binding_digest)?;
        let outer = decode_nitro_cbor(document)?;
        let outer = match outer {
            Value::Tag(18, value) => *value,
            value => value,
        };
        let Value::Array(parts) = outer else {
            return Err("Nitro COSE must be an array".into());
        };
        if parts.len() != 4 {
            return Err("Nitro COSE must have four fields".into());
        }
        let protected = bytes(&parts[0], 64, "protected header")?;
        let protected_value = decode_strict_cbor(protected, NITRO_CBOR_LIMITS)?;
        let header = map(&protected_value, "protected header")?;
        if header.len() != 1
            || header.get(&Value::Integer(1)) != Some(&Value::Integer(-35))
            || !map(&parts[1], "unprotected header")?.is_empty()
        {
            return Err("Nitro COSE headers must contain only protected ES384".into());
        }
        let payload_bytes = bytes(&parts[2], 16 * 1024, "payload")?;
        let signature = bytes(&parts[3], 96, "signature")?;
        let payload_value = decode_nitro_cbor(payload_bytes)?;
        let payload = map(&payload_value, "payload")?;
        let required = [
            "module_id",
            "digest",
            "timestamp",
            "pcrs",
            "certificate",
            "cabundle",
            "public_key",
            "user_data",
            "nonce",
        ];
        if payload.len() != required.len() {
            return Err("Nitro payload field count mismatch".into());
        }
        for name in required {
            field(payload, name)?;
        }
        if !matches!(field(payload, "module_id")?, Value::Text(value) if !value.is_empty() && value.len() <= 256)
            || field(payload, "digest")? != &Value::Text("SHA384".into())
        {
            return Err("Nitro module identity or PCR digest algorithm is invalid".into());
        }
        let timestamp_ms = match field(payload, "timestamp")? {
            Value::Integer(value) => {
                u64::try_from(*value).map_err(|_| "Nitro timestamp is invalid")?
            }
            _ => return Err("Nitro timestamp must be an integer".into()),
        };
        if timestamp_ms == 0
            || u128::from(timestamp_ms)
                > u128::from(verification_time_ms) + u128::from(policy.max_future_skew_ms)
            || u128::from(verification_time_ms)
                > u128::from(timestamp_ms) + u128::from(policy.max_age_ms)
        {
            return Err("Nitro attestation timestamp is stale or in the future".into());
        }
        let encoded_pcrs = map(field(payload, "pcrs")?, "PCRs")?;
        if encoded_pcrs.is_empty() || encoded_pcrs.len() > 32 {
            return Err("Nitro PCR count is invalid".into());
        }
        let mut pcrs = BTreeMap::new();
        for (index, value) in encoded_pcrs {
            let index = match index {
                Value::Integer(index) if (0..32).contains(index) => *index as u8,
                _ => return Err("Nitro PCR index is invalid".into()),
            };
            let digest = bytes(value, 48, "PCR")?;
            if digest.len() != 48 {
                return Err("Nitro SHA-384 PCR must have 48 bytes".into());
            }
            pcrs.insert(index, digest.to_vec());
        }
        if !policy.approved_pcr_sets.iter().any(|set| {
            set.iter().all(|pcr| {
                pcrs.get(&pcr.index)
                    .is_some_and(|actual| hex::encode(actual) == pcr.sha384)
            })
        }) {
            return Err("Nitro PCR measurements are not approved non-debug measurements".into());
        }
        let leaf = bytes(field(payload, "certificate")?, 1024, "leaf certificate")?;
        let Value::Array(bundle) = field(payload, "cabundle")? else {
            return Err("Nitro cabundle must be an array".into());
        };
        let signing_key = self.leaf_key(leaf, bundle)?;
        let signature = Signature::from_slice(signature)
            .map_err(|_| "Nitro COSE signature must have 96 bytes")?;
        let message = serde_cbor::to_vec(&Value::Array(vec![
            Value::Text("Signature1".into()),
            Value::Bytes(protected.to_vec()),
            Value::Bytes(vec![]),
            Value::Bytes(payload_bytes.to_vec()),
        ]))
        .map_err(|_| "Nitro COSE signing structure encoding failed")?;
        signing_key
            .verify(&message, &signature)
            .map_err(|_| "Nitro COSE signature is invalid")?;
        let recipient_spki = bytes(field(payload, "public_key")?, 1024, "recipient SPKI")?;
        let recipient = RsaPublicKey::from_public_key_der(recipient_spki)
            .map_err(|_| "Nitro recipient SPKI is invalid")?;
        if recipient.n().bits() != 2048 {
            return Err("Nitro recipient must be RSA-2048".into());
        }
        let user_bytes = bytes(field(payload, "user_data")?, 1024, "user data")?;
        let user: serde_json::Value =
            serde_json::from_slice(user_bytes).map_err(|_| "Nitro user data must be JSON")?;
        if canonical_bytes(&user)? != user_bytes {
            return Err("Nitro user data must be canonical JSON".into());
        }
        let user = user
            .as_object()
            .ok_or("Nitro user data must be an object")?;
        if user.len() != 3
            || user.get("schema").and_then(|value| value.as_str())
                != Some(NITRO_USER_DATA_SCHEMA_V2)
            || user
                .get("statementBindingDigest")
                .and_then(|value| value.as_str())
                != Some(binding_digest)
        {
            return Err("Nitro user data schema or statement binding mismatch".into());
        }
        let statement_key = user
            .get("statementKey")
            .and_then(|value| value.as_str())
            .ok_or("Nitro statement key is missing")?;
        validate_digest("Nitro statement key", statement_key)?;
        if bytes(field(payload, "nonce")?, 32, "nonce")?
            != decode_hex("binding digest", binding_digest, 32)?
        {
            return Err("Nitro nonce does not match the statement binding".into());
        }
        Ok(VerifiedNitroClaims {
            pcrs,
            statement_key: statement_key.into(),
            recipient_spki: recipient_spki.to_vec(),
            timestamp_ms,
        })
    }
}
