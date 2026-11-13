//! B23 account-value statement -> Nitro verification -> existing reserve ABI.
//! No broker response, credential or private key is needed by this proof.
use crate::yolo_collection::{canonical_bytes, domain_sha256};
use crate::yolo_nitro::verify_nitro_document;
use crate::yolo_witness::{NitroProofPolicyV1, StatementRuleV1};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use ed25519_dalek::{Signature, VerifyingKey};
use postfiat_types::{
    NavReservePublicValuesV1, NavReserveTrustCountsV1, NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha3::{Digest, Sha3_384};

pub const MAX_WITNESS: usize = 64 * 1024;
const STATEMENT: &str = "postfiat.b23.nitro_nav_reading.v1";
const MANIFEST: &str = "postfiat.b23.nitro_nav_manifest.v1";
const SOURCE_POLICY: &str = "postfiat.b23.nav_source_policy.v1";
const VALUATION_POLICY: &str = "postfiat.b23.nav_valuation_policy.v1";
const FIELD: &str = "securitiesAccount.currentBalances.liquidationValue";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Schedule {
    pub opening_epoch: u64,
    pub opening_unix_ms: u64,
    pub first_daily_epoch: u64,
    pub first_daily_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub pftl_genesis_hash: String,
    pub nav_asset_id: String,
    pub proof_profile_id: String,
    pub valuation_policy_hash: String,
    pub source_manifest_hash: String,
    pub valuation_unit_id: String,
    pub account_commitment_sha256: String,
    pub cutoff_unix_ms: u64,
    pub observation_epoch: u64,
    pub observation_height: u64,
    pub challenge_sha256: String,
    pub operator_public_key_der_base64: String,
    pub schedule: Schedule,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reading {
    pub account_commitment_sha256: String,
    pub cutoff_unix_ms: u64,
    pub started_unix_ms: u64,
    pub observed_unix_ms: u64,
    pub liquidation_value_atoms: u64,
    pub source_field: String,
    pub account_list_response_sha256: String,
    pub account_response_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Body {
    pub schema: String,
    pub manifest_sha256: String,
    pub reading: Reading,
    pub public_values_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub schema: String,
    pub body: Body,
    pub binding_digest: String,
    pub signature: String,
    pub document_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub schema: String,
    pub measured_policy: Value,
    pub valuation_policy: Value,
    pub root_der_hex: String,
    pub nitro_policy: NitroProofPolicyV1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Witness {
    pub policy: Policy,
    pub manifest: Manifest,
    pub envelope: Envelope,
}

fn require(condition: bool, error: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(error.into())
    }
}

fn fixed(value: &str, bytes: usize) -> Result<Vec<u8>, String> {
    require(
        value.len() == bytes * 2
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "invalid fixed digest",
    )?;
    hex::decode(value).map_err(|_| "invalid hex".into())
}

pub fn commitment48<T: Serialize>(domain: &str, value: &T) -> Result<String, String> {
    let mut hash = Sha3_384::new();
    hash.update(domain.as_bytes());
    hash.update([0]);
    hash.update(canonical_bytes(value)?);
    Ok(hex::encode(hash.finalize()))
}

pub fn decode(encoded: &[u8]) -> Result<Witness, String> {
    require(
        !encoded.is_empty() && encoded.len() <= MAX_WITNESS,
        "NAV witness bound",
    )?;
    let witness: Witness =
        serde_json::from_slice(encoded).map_err(|_| "invalid NAV witness JSON")?;
    require(
        canonical_bytes(&witness)? == encoded,
        "NAV witness must use canonical JSON",
    )?;
    Ok(witness)
}

pub fn execute(witness: &Witness) -> Result<NavReservePublicValuesV1, String> {
    let m = &witness.manifest;
    let p = &witness.policy;
    let e = &witness.envelope;
    let r = &e.body.reading;
    require(
        m.schema == MANIFEST
            && p.schema == SOURCE_POLICY
            && e.body.schema == STATEMENT
            && e.schema == "postfiat.b23.attested_nav_reading.v1",
        "NAV schema mismatch",
    )?;
    require(
        m.observation_epoch > 0 && m.observation_height > 0 && r.liquidation_value_atoms > 0,
        "zero NAV epoch/height/value",
    )?;
    for value in [
        &m.account_commitment_sha256,
        &m.challenge_sha256,
        &r.account_list_response_sha256,
        &r.account_response_sha256,
    ] {
        require(fixed(value, 32)? != vec![0; 32], "zero NAV commitment")?;
    }
    let s = &m.schedule;
    require(
        s.opening_epoch > 0
            && s.opening_unix_ms > 0
            && s.opening_epoch.checked_add(1) == Some(s.first_daily_epoch)
            && s.first_daily_unix_ms % 86_400_000 == 65_700_000,
        "invalid NAV schedule",
    )?;
    let gap = s
        .first_daily_unix_ms
        .checked_sub(s.opening_unix_ms)
        .ok_or("opening after first daily fix")?;
    require(
        gap > 65_000 && gap < 86_400_000,
        "opening outside approved day",
    )?;
    let cutoff = if m.observation_epoch == s.opening_epoch {
        s.opening_unix_ms
    } else {
        m.observation_epoch
            .checked_sub(s.first_daily_epoch)
            .and_then(|n| n.checked_mul(86_400_000))
            .and_then(|n| n.checked_add(s.first_daily_unix_ms))
            .ok_or("invalid scheduled epoch")?
    };
    let end = cutoff.checked_add(65_000).ok_or("cutoff overflow")?;
    require(
        m.cutoff_unix_ms == cutoff
            && r.cutoff_unix_ms == cutoff
            && r.started_unix_ms >= cutoff
            && r.observed_unix_ms >= r.started_unix_ms
            && r.observed_unix_ms <= end - 5000,
        "stale or wrong scheduled reading",
    )?;
    require(
        r.account_commitment_sha256 == m.account_commitment_sha256 && r.source_field == FIELD,
        "wrong account or source field",
    )?;

    let mut measured = serde_json::to_value(m).map_err(|_| "manifest serialization")?;
    for key in [
        "observation_epoch",
        "observation_height",
        "proof_profile_id",
        "cutoff_unix_ms",
        "challenge_sha256",
        "source_manifest_hash",
        "valuation_policy_hash",
    ] {
        measured
            .as_object_mut()
            .ok_or("manifest object")?
            .remove(key);
    }
    require(
        measured == p.measured_policy,
        "manifest differs from measured account policy",
    )?;
    let valuation = json!({"schema":VALUATION_POLICY,"source_field":FIELD,"scale":1_000_000,
        "additional_adjustments":[],"account_commitment_sha256":m.account_commitment_sha256,
        "schedule":s,"maximum_read_window_ms":60_000,"evidence_class":"attested"});
    require(valuation == p.valuation_policy, "wrong valuation policy")?;
    require(
        domain_sha256(VALUATION_POLICY, &valuation)? == m.valuation_policy_hash,
        "valuation policy hash mismatch",
    )?;
    require(
        commitment48(SOURCE_POLICY, p)? == m.source_manifest_hash,
        "source policy hash mismatch",
    )?;
    require(
        domain_sha256(MANIFEST, m)? == e.body.manifest_sha256,
        "manifest digest mismatch",
    )?;
    let binding = domain_sha256(STATEMENT, &e.body)?;
    require(binding == e.binding_digest, "statement digest mismatch")?;
    require(
        p.nitro_policy.statement_rule == StatementRuleV1::NavReadingV1
            && p.nitro_policy.max_age_ms == 65_000
            && p.nitro_policy.max_future_skew_ms == 0,
        "wrong NAV Nitro policy",
    )?;
    require(
        p.nitro_policy.approved_pcr_sets.len() == 1
            && p.nitro_policy.approved_pcr_sets[0]
                .iter()
                .map(|p| p.index)
                .collect::<Vec<_>>()
                == vec![0, 1, 2],
        "NAV requires exact PCR0/1/2",
    )?;
    let root = hex::decode(&p.root_der_hex).map_err(|_| "root DER encoding")?;
    let document = BASE64
        .decode(&e.document_base64)
        .map_err(|_| "Nitro document encoding")?;
    let claims = verify_nitro_document(&document, &root, &p.nitro_policy, end, &binding)?;
    require(
        claims.timestamp_ms >= r.observed_unix_ms && claims.timestamp_ms <= end,
        "Nitro time is outside reading window",
    )?;
    let key: [u8; 32] = fixed(&claims.statement_key, 32)?
        .try_into()
        .map_err(|_| "statement key size")?;
    let signature = BASE64
        .decode(&e.signature)
        .map_err(|_| "signature encoding")?;
    VerifyingKey::from_bytes(&key)
        .map_err(|_| "invalid signing key")?
        .verify_strict(
            &fixed(&binding, 32)?,
            &Signature::from_slice(&signature).map_err(|_| "signature length")?,
        )
        .map_err(|_| "NAV signature invalid")?;

    let source = commitment48(&format!("{STATEMENT}.source"), r)?;
    let trust = json!({"source":source,"account":m.account_commitment_sha256,"class":"attested",
        "basis":FIELD,"additional_adjustments":[]});
    let counts = NavReserveTrustCountsV1 {
        cryptographic: 0,
        attested: 1,
        controlled: 0,
    };
    let values = NavReservePublicValuesV1 {
        schema: NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1.into(),
        pftl_genesis_hash: m.pftl_genesis_hash.clone(),
        nav_asset_id: m.nav_asset_id.clone(),
        proof_profile_id: m.proof_profile_id.clone(),
        valuation_policy_hash: m.valuation_policy_hash.clone(),
        source_manifest_hash: m.source_manifest_hash.clone(),
        valuation_unit_id: m.valuation_unit_id.clone(),
        valuation_scale: 1_000_000,
        observation_epoch: m.observation_epoch,
        observation_not_before: m.observation_height,
        observation_not_after: m.observation_height,
        source_observation_root: source,
        gross_assets: r.liquidation_value_atoms,
        total_liabilities: 0,
        verified_net_assets: r.liquidation_value_atoms,
        cryptographically_verified_value: 0,
        attested_value: r.liquidation_value_atoms,
        controlled_value: 0,
        source_count: 1,
        quantity_trust_counts: counts,
        valuation_trust_counts: counts,
        quantity_trust_root: commitment48(&format!("{STATEMENT}.quantity"), &trust)?,
        valuation_trust_root: commitment48(&format!("{STATEMENT}.valuation"), &trust)?,
        source_disclosure_root: commitment48(&format!("{STATEMENT}.disclosure"), &trust)?,
    };
    require(
        hex::encode(values.encode()?) == e.body.public_values_hex,
        "attested reserve ABI mismatch",
    )?;
    Ok(values)
}
