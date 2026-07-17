use postfiat_crypto_provider::{bytes_to_hex, hex_to_bytes};
use serde::{Deserialize, Serialize};

pub const ORCHARD_PROOF_SYSTEM_ID: &str = "postfiat.privacy.orchard-halo2.v2";
pub const ORCHARD_ACTION_CIRCUIT_ID: &str = "orchard.action.v2";
pub const ORCHARD_ANCHOR_BYTES: usize = 32;
pub const ORCHARD_NULLIFIER_BYTES: usize = 32;
pub const ORCHARD_COMMITMENT_BYTES: usize = 32;
pub const ORCHARD_RANDOMIZED_VERIFICATION_KEY_BYTES: usize = 32;
pub const ORCHARD_VALUE_COMMITMENT_BYTES: usize = 32;
pub const ORCHARD_REDPALLAS_SIGNATURE_BYTES: usize = 64;
pub const ORCHARD_EPK_BYTES: usize = 32;
pub const ORCHARD_ENC_CIPHERTEXT_BYTES: usize = 580;
pub const ORCHARD_OUT_CIPHERTEXT_BYTES: usize = 80;
pub const ORCHARD_COMPACT_CIPHERTEXT_BYTES: usize = 52;
pub const ORCHARD_PROOF_MAX_BYTES: usize = 1_048_576;
pub const ORCHARD_CIPHERTEXT_MAX_BYTES: usize = 4096;
pub const ORCHARD_EXTERNAL_BINDING_HASH_BYTES: usize = 48;
pub const SHIELDED_SWAP_ACTION_SCHEMA: &str = "postfiat-shielded-swap-action-v1";
pub const SHIELDED_SWAP_PROOF_SYSTEM_ID: &str = "postfiat.privacy.asset-swap.v1";
pub const SHIELDED_SWAP_CIRCUIT_ID: &str = "shielded_swap.asset_conservation.v1";
pub const SHIELDED_SWAP_COMMITMENT_BYTES: usize = 48;
pub const SHIELDED_SWAP_LEGACY_TRANSCRIPT_HASH_BYTES: usize = 48;
pub const SHIELDED_SWAP_LEG_COUNT: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchardTypeError {
    code: &'static str,
    message: String,
}

impl OrchardTypeError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }
}

impl std::fmt::Display for OrchardTypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for OrchardTypeError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct OrchardProofSystemId(String);

impl OrchardProofSystemId {
    pub fn production_v2() -> Self {
        Self(ORCHARD_PROOF_SYSTEM_ID.to_string())
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, OrchardTypeError> {
        let value = value.into();
        if value != ORCHARD_PROOF_SYSTEM_ID {
            return Err(OrchardTypeError::new(
                "unsupported_proof_system",
                format!("unsupported Orchard proof system `{value}`"),
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for OrchardProofSystemId {
    type Error = OrchardTypeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<OrchardProofSystemId> for String {
    fn from(value: OrchardProofSystemId) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct OrchardCircuitId(String);

impl OrchardCircuitId {
    pub fn action_v2() -> Self {
        Self(ORCHARD_ACTION_CIRCUIT_ID.to_string())
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, OrchardTypeError> {
        let value = value.into();
        if value != ORCHARD_ACTION_CIRCUIT_ID {
            return Err(OrchardTypeError::new(
                "unsupported_circuit",
                format!("unsupported Orchard circuit `{value}`"),
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for OrchardCircuitId {
    type Error = OrchardTypeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<OrchardCircuitId> for String {
    fn from(value: OrchardCircuitId) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct OrchardAnchor(String);

impl OrchardAnchor {
    pub fn parse_hex(value: impl Into<String>) -> Result<Self, OrchardTypeError> {
        let value = parse_fixed_lower_hex("anchor", value.into(), ORCHARD_ANCHOR_BYTES)?;
        parse_orchard_anchor(&value)?;
        Ok(Self(value))
    }

    pub fn from_bytes(bytes: &[u8; ORCHARD_ANCHOR_BYTES]) -> Result<Self, OrchardTypeError> {
        Self::parse_hex(bytes_to_hex(bytes))
    }

    pub fn from_orchard(anchor: orchard::Anchor) -> Self {
        Self(bytes_to_hex(&anchor.to_bytes()))
    }

    pub fn to_orchard(&self) -> Result<orchard::Anchor, OrchardTypeError> {
        parse_orchard_anchor(&self.0)
    }

    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for OrchardAnchor {
    type Error = OrchardTypeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse_hex(value)
    }
}

impl From<OrchardAnchor> for String {
    fn from(value: OrchardAnchor) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct OrchardNullifier(String);

impl OrchardNullifier {
    pub fn parse_hex(value: impl Into<String>) -> Result<Self, OrchardTypeError> {
        let value = parse_fixed_lower_hex("nullifier", value.into(), ORCHARD_NULLIFIER_BYTES)?;
        parse_orchard_nullifier(&value)?;
        Ok(Self(value))
    }

    pub fn from_bytes(bytes: &[u8; ORCHARD_NULLIFIER_BYTES]) -> Result<Self, OrchardTypeError> {
        Self::parse_hex(bytes_to_hex(bytes))
    }

    pub fn from_orchard(nullifier: orchard::note::Nullifier) -> Self {
        Self(bytes_to_hex(&nullifier.to_bytes()))
    }

    pub fn to_orchard(&self) -> Result<orchard::note::Nullifier, OrchardTypeError> {
        parse_orchard_nullifier(&self.0)
    }

    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for OrchardNullifier {
    type Error = OrchardTypeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse_hex(value)
    }
}

impl From<OrchardNullifier> for String {
    fn from(value: OrchardNullifier) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct OrchardOutputCommitment(String);

impl OrchardOutputCommitment {
    pub fn parse_hex(value: impl Into<String>) -> Result<Self, OrchardTypeError> {
        let value =
            parse_fixed_lower_hex("output_commitment", value.into(), ORCHARD_COMMITMENT_BYTES)?;
        parse_orchard_output_commitment(&value)?;
        Ok(Self(value))
    }

    pub fn from_bytes(bytes: &[u8; ORCHARD_COMMITMENT_BYTES]) -> Result<Self, OrchardTypeError> {
        Self::parse_hex(bytes_to_hex(bytes))
    }

    pub fn from_orchard(commitment: orchard::note::ExtractedNoteCommitment) -> Self {
        Self(bytes_to_hex(&commitment.to_bytes()))
    }

    pub fn to_orchard(&self) -> Result<orchard::note::ExtractedNoteCommitment, OrchardTypeError> {
        parse_orchard_output_commitment(&self.0)
    }

    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for OrchardOutputCommitment {
    type Error = OrchardTypeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse_hex(value)
    }
}

impl From<OrchardOutputCommitment> for String {
    fn from(value: OrchardOutputCommitment) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct OrchardFlags(u8);

impl OrchardFlags {
    pub fn enabled() -> Self {
        Self(orchard::bundle::Flags::ENABLED.to_byte())
    }

    pub fn parse(value: u8) -> Result<Self, OrchardTypeError> {
        parse_orchard_flags(value)?;
        Ok(Self(value))
    }

    pub fn from_orchard(flags: orchard::bundle::Flags) -> Self {
        Self(flags.to_byte())
    }

    pub fn to_orchard(&self) -> Result<orchard::bundle::Flags, OrchardTypeError> {
        parse_orchard_flags(self.0)
    }

    pub fn as_byte(&self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for OrchardFlags {
    type Error = OrchardTypeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<OrchardFlags> for u8 {
    fn from(value: OrchardFlags) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct OrchardRandomizedVerificationKey(String);

impl OrchardRandomizedVerificationKey {
    pub fn parse_hex(value: impl Into<String>) -> Result<Self, OrchardTypeError> {
        let value = parse_fixed_lower_hex(
            "randomized_verification_key",
            value.into(),
            ORCHARD_RANDOMIZED_VERIFICATION_KEY_BYTES,
        )?;
        parse_orchard_randomized_verification_key(&value)?;
        Ok(Self(value))
    }

    pub fn from_orchard(
        key: &orchard::primitives::redpallas::VerificationKey<
            orchard::primitives::redpallas::SpendAuth,
        >,
    ) -> Self {
        let bytes: [u8; ORCHARD_RANDOMIZED_VERIFICATION_KEY_BYTES] = key.into();
        Self(bytes_to_hex(&bytes))
    }

    pub fn to_orchard(
        &self,
    ) -> Result<
        orchard::primitives::redpallas::VerificationKey<orchard::primitives::redpallas::SpendAuth>,
        OrchardTypeError,
    > {
        parse_orchard_randomized_verification_key(&self.0)
    }

    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for OrchardRandomizedVerificationKey {
    type Error = OrchardTypeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse_hex(value)
    }
}

impl From<OrchardRandomizedVerificationKey> for String {
    fn from(value: OrchardRandomizedVerificationKey) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct OrchardValueCommitment(String);

impl OrchardValueCommitment {
    pub fn parse_hex(value: impl Into<String>) -> Result<Self, OrchardTypeError> {
        let value = parse_fixed_lower_hex(
            "value_commitment",
            value.into(),
            ORCHARD_VALUE_COMMITMENT_BYTES,
        )?;
        parse_orchard_value_commitment(&value)?;
        Ok(Self(value))
    }

    pub fn from_orchard(commitment: &orchard::value::ValueCommitment) -> Self {
        Self(bytes_to_hex(&commitment.to_bytes()))
    }

    pub fn to_orchard(&self) -> Result<orchard::value::ValueCommitment, OrchardTypeError> {
        parse_orchard_value_commitment(&self.0)
    }

    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for OrchardValueCommitment {
    type Error = OrchardTypeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse_hex(value)
    }
}

impl From<OrchardValueCommitment> for String {
    fn from(value: OrchardValueCommitment) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct OrchardSpendAuthSignature(String);

impl OrchardSpendAuthSignature {
    pub fn parse_hex(value: impl Into<String>) -> Result<Self, OrchardTypeError> {
        Ok(Self(parse_fixed_lower_hex(
            "spend_authorization_signature",
            value.into(),
            ORCHARD_REDPALLAS_SIGNATURE_BYTES,
        )?))
    }

    pub fn from_orchard(
        signature: &orchard::primitives::redpallas::Signature<
            orchard::primitives::redpallas::SpendAuth,
        >,
    ) -> Self {
        let bytes: [u8; ORCHARD_REDPALLAS_SIGNATURE_BYTES] = signature.into();
        Self(bytes_to_hex(&bytes))
    }

    pub fn to_orchard(
        &self,
    ) -> Result<
        orchard::primitives::redpallas::Signature<orchard::primitives::redpallas::SpendAuth>,
        OrchardTypeError,
    > {
        Ok(orchard::primitives::redpallas::Signature::from(
            fixed_hex_array::<ORCHARD_REDPALLAS_SIGNATURE_BYTES>(
                "spend_authorization_signature",
                &self.0,
            )?,
        ))
    }

    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for OrchardSpendAuthSignature {
    type Error = OrchardTypeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse_hex(value)
    }
}

impl From<OrchardSpendAuthSignature> for String {
    fn from(value: OrchardSpendAuthSignature) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct OrchardBindingSignature(String);

impl OrchardBindingSignature {
    pub fn parse_hex(value: impl Into<String>) -> Result<Self, OrchardTypeError> {
        Ok(Self(parse_fixed_lower_hex(
            "binding_signature",
            value.into(),
            ORCHARD_REDPALLAS_SIGNATURE_BYTES,
        )?))
    }

    pub fn from_orchard(
        signature: &orchard::primitives::redpallas::Signature<
            orchard::primitives::redpallas::Binding,
        >,
    ) -> Self {
        let bytes: [u8; ORCHARD_REDPALLAS_SIGNATURE_BYTES] = signature.into();
        Self(bytes_to_hex(&bytes))
    }

    pub fn to_orchard(
        &self,
    ) -> Result<
        orchard::primitives::redpallas::Signature<orchard::primitives::redpallas::Binding>,
        OrchardTypeError,
    > {
        Ok(orchard::primitives::redpallas::Signature::from(
            fixed_hex_array::<ORCHARD_REDPALLAS_SIGNATURE_BYTES>("binding_signature", &self.0)?,
        ))
    }

    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for OrchardBindingSignature {
    type Error = OrchardTypeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse_hex(value)
    }
}

impl From<OrchardBindingSignature> for String {
    fn from(value: OrchardBindingSignature) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct BoundedHexBlob {
    hex: String,
}

impl BoundedHexBlob {
    pub fn parse_hex(
        label: &'static str,
        value: impl Into<String>,
        max_bytes: usize,
    ) -> Result<Self, OrchardTypeError> {
        let value = value.into();
        parse_lower_hex(label, &value, 1, max_bytes)?;
        Ok(Self { hex: value })
    }

    pub fn from_bytes(bytes: &[u8], max_bytes: usize) -> Result<Self, OrchardTypeError> {
        if bytes.is_empty() {
            return Err(OrchardTypeError::new(
                "empty_blob",
                "blob must not be empty",
            ));
        }
        if bytes.len() > max_bytes {
            return Err(OrchardTypeError::new(
                "oversized_blob",
                format!("blob has {} bytes, max {max_bytes}", bytes.len()),
            ));
        }
        Ok(Self {
            hex: bytes_to_hex(bytes),
        })
    }

    pub fn as_hex(&self) -> &str {
        &self.hex
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, OrchardTypeError> {
        hex_to_bytes(&self.hex)
            .map_err(|error| OrchardTypeError::new("invalid_hex", error.to_string()))
    }

    pub fn to_fixed_bytes<const N: usize>(
        &self,
        label: &'static str,
    ) -> Result<[u8; N], OrchardTypeError> {
        fixed_hex_array(label, &self.hex)
    }

    pub fn byte_len(&self) -> usize {
        self.hex.len() / 2
    }
}

impl TryFrom<String> for BoundedHexBlob {
    type Error = OrchardTypeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse_hex("blob", value, ORCHARD_CIPHERTEXT_MAX_BYTES)
    }
}

impl From<BoundedHexBlob> for String {
    fn from(value: BoundedHexBlob) -> Self {
        value.hex
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct OrchardProofBytes(BoundedHexBlob);

impl OrchardProofBytes {
    pub fn parse_hex(value: impl Into<String>) -> Result<Self, OrchardTypeError> {
        BoundedHexBlob::parse_hex("proof", value, ORCHARD_PROOF_MAX_BYTES).map(Self)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, OrchardTypeError> {
        BoundedHexBlob::from_bytes(bytes, ORCHARD_PROOF_MAX_BYTES).map(Self)
    }

    pub fn as_hex(&self) -> &str {
        self.0.as_hex()
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, OrchardTypeError> {
        self.0.to_bytes()
    }

    pub fn byte_len(&self) -> usize {
        self.0.byte_len()
    }
}

impl TryFrom<String> for OrchardProofBytes {
    type Error = OrchardTypeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse_hex(value)
    }
}

impl From<OrchardProofBytes> for String {
    fn from(value: OrchardProofBytes) -> Self {
        value.0.hex
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedShieldedOutput {
    pub cmx: OrchardOutputCommitment,
    pub epk: BoundedHexBlob,
    pub enc_ciphertext: BoundedHexBlob,
    pub out_ciphertext: BoundedHexBlob,
    pub compact_ciphertext: Option<BoundedHexBlob>,
}

impl EncryptedShieldedOutput {
    pub fn from_bytes(
        cmx: OrchardOutputCommitment,
        epk: &[u8; ORCHARD_EPK_BYTES],
        enc_ciphertext: &[u8; ORCHARD_ENC_CIPHERTEXT_BYTES],
        out_ciphertext: &[u8; ORCHARD_OUT_CIPHERTEXT_BYTES],
        compact_ciphertext: Option<&[u8; ORCHARD_COMPACT_CIPHERTEXT_BYTES]>,
    ) -> Result<Self, OrchardTypeError> {
        Ok(Self {
            cmx,
            epk: BoundedHexBlob::from_bytes(epk, ORCHARD_EPK_BYTES)?,
            enc_ciphertext: BoundedHexBlob::from_bytes(
                enc_ciphertext,
                ORCHARD_ENC_CIPHERTEXT_BYTES,
            )?,
            out_ciphertext: BoundedHexBlob::from_bytes(
                out_ciphertext,
                ORCHARD_OUT_CIPHERTEXT_BYTES,
            )?,
            compact_ciphertext: compact_ciphertext
                .map(|ciphertext| {
                    BoundedHexBlob::from_bytes(ciphertext, ORCHARD_COMPACT_CIPHERTEXT_BYTES)
                })
                .transpose()?,
        })
    }

    pub fn validate(&self) -> Result<(), OrchardTypeError> {
        validate_blob_exact("epk", &self.epk, ORCHARD_EPK_BYTES)?;
        validate_blob_exact(
            "enc_ciphertext",
            &self.enc_ciphertext,
            ORCHARD_ENC_CIPHERTEXT_BYTES,
        )?;
        validate_blob_exact(
            "out_ciphertext",
            &self.out_ciphertext,
            ORCHARD_OUT_CIPHERTEXT_BYTES,
        )?;
        if let Some(compact) = &self.compact_ciphertext {
            validate_blob_exact(
                "compact_ciphertext",
                compact,
                ORCHARD_COMPACT_CIPHERTEXT_BYTES,
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ShieldedSwapCommitment(String);

impl ShieldedSwapCommitment {
    pub fn parse_hex(value: impl Into<String>) -> Result<Self, OrchardTypeError> {
        Ok(Self(parse_fixed_lower_hex(
            "shielded_swap_commitment",
            value.into(),
            SHIELDED_SWAP_COMMITMENT_BYTES,
        )?))
    }

    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ShieldedSwapCommitment {
    type Error = OrchardTypeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse_hex(value)
    }
}

impl From<ShieldedSwapCommitment> for String {
    fn from(value: ShieldedSwapCommitment) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShieldedSwapAction {
    pub schema: String,
    pub pool_id: String,
    pub proof_system_id: String,
    pub circuit_id: String,
    pub anchor: OrchardAnchor,
    pub nullifiers: Vec<OrchardNullifier>,
    pub input_asset_commitments: Vec<ShieldedSwapCommitment>,
    pub input_value_commitments: Vec<ShieldedSwapCommitment>,
    pub input_authorization_commitments: Vec<ShieldedSwapCommitment>,
    pub output_commitments: Vec<OrchardOutputCommitment>,
    pub output_asset_commitments: Vec<ShieldedSwapCommitment>,
    pub output_value_commitments: Vec<ShieldedSwapCommitment>,
    pub encrypted_outputs: Vec<EncryptedShieldedOutput>,
    pub swap_binding_hash: ShieldedSwapCommitment,
    pub fee: u64,
    pub proof: BoundedHexBlob,
}

impl ShieldedSwapAction {
    pub fn validate(&self) -> Result<(), OrchardTypeError> {
        if self.schema != SHIELDED_SWAP_ACTION_SCHEMA {
            return Err(OrchardTypeError::new(
                "unsupported_shielded_swap_schema",
                format!("unsupported shielded swap schema `{}`", self.schema),
            ));
        }
        if self.proof_system_id != SHIELDED_SWAP_PROOF_SYSTEM_ID {
            return Err(OrchardTypeError::new(
                "unsupported_shielded_swap_proof_system",
                format!(
                    "unsupported shielded swap proof system `{}`",
                    self.proof_system_id
                ),
            ));
        }
        if self.circuit_id != SHIELDED_SWAP_CIRCUIT_ID {
            return Err(OrchardTypeError::new(
                "unsupported_shielded_swap_circuit",
                format!("unsupported shielded swap circuit `{}`", self.circuit_id),
            ));
        }
        validate_plain_identifier("pool_id", &self.pool_id)?;
        validate_action_count("nullifiers", self.nullifiers.len(), SHIELDED_SWAP_LEG_COUNT)?;
        validate_action_count(
            "input_asset_commitments",
            self.input_asset_commitments.len(),
            SHIELDED_SWAP_LEG_COUNT,
        )?;
        validate_action_count(
            "input_value_commitments",
            self.input_value_commitments.len(),
            SHIELDED_SWAP_LEG_COUNT,
        )?;
        validate_action_count(
            "input_authorization_commitments",
            self.input_authorization_commitments.len(),
            SHIELDED_SWAP_LEG_COUNT,
        )?;
        validate_action_count(
            "output_commitments",
            self.output_commitments.len(),
            SHIELDED_SWAP_LEG_COUNT,
        )?;
        validate_action_count(
            "output_asset_commitments",
            self.output_asset_commitments.len(),
            SHIELDED_SWAP_LEG_COUNT,
        )?;
        validate_action_count(
            "output_value_commitments",
            self.output_value_commitments.len(),
            SHIELDED_SWAP_LEG_COUNT,
        )?;
        validate_action_count(
            "encrypted_outputs",
            self.encrypted_outputs.len(),
            SHIELDED_SWAP_LEG_COUNT,
        )?;
        if self.proof.byte_len() != SHIELDED_SWAP_LEGACY_TRANSCRIPT_HASH_BYTES {
            return Err(OrchardTypeError::new(
                "invalid_shielded_swap_transcript_hash_length",
                format!(
                    "legacy shielded swap transcript hash has {} bytes, expected {SHIELDED_SWAP_LEGACY_TRANSCRIPT_HASH_BYTES}",
                    self.proof.byte_len()
                ),
            ));
        }
        if has_duplicate_hex(self.nullifiers.iter().map(OrchardNullifier::as_hex)) {
            return Err(OrchardTypeError::new(
                "duplicate_nullifier",
                "shielded swap contains duplicate nullifiers",
            ));
        }
        if has_duplicate_hex(
            self.output_commitments
                .iter()
                .map(OrchardOutputCommitment::as_hex),
        ) {
            return Err(OrchardTypeError::new(
                "duplicate_output_commitment",
                "shielded swap contains duplicate output commitments",
            ));
        }
        for (index, output) in self.encrypted_outputs.iter().enumerate() {
            if output.cmx != self.output_commitments[index] {
                return Err(OrchardTypeError::new(
                    "output_commitment_mismatch",
                    format!("encrypted output {index} cmx does not match output_commitments"),
                ));
            }
            output.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardShieldedAction {
    pub pool_id: String,
    pub proof_system_id: OrchardProofSystemId,
    pub circuit_id: OrchardCircuitId,
    pub flags: OrchardFlags,
    pub anchor: OrchardAnchor,
    pub nullifiers: Vec<OrchardNullifier>,
    pub randomized_verification_keys: Vec<OrchardRandomizedVerificationKey>,
    pub value_commitments: Vec<OrchardValueCommitment>,
    pub output_commitments: Vec<OrchardOutputCommitment>,
    pub encrypted_outputs: Vec<EncryptedShieldedOutput>,
    pub value_balance: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_binding_hash: Option<String>,
    pub fee: u64,
    pub proof: OrchardProofBytes,
    pub spend_authorization_signatures: Vec<OrchardSpendAuthSignature>,
    pub binding_signature: OrchardBindingSignature,
}

impl OrchardShieldedAction {
    pub fn validate(&self) -> Result<(), OrchardTypeError> {
        validate_plain_identifier("pool_id", &self.pool_id)?;
        self.flags.to_orchard()?;
        if self.nullifiers.is_empty() {
            return Err(OrchardTypeError::new(
                "missing_nullifier",
                "Orchard action must contain at least one nullifier",
            ));
        }
        if let Some(hash) = &self.external_binding_hash {
            parse_lower_hex(
                "external_binding_hash",
                hash,
                ORCHARD_EXTERNAL_BINDING_HASH_BYTES,
                ORCHARD_EXTERNAL_BINDING_HASH_BYTES,
            )?;
        }
        let action_count = self.nullifiers.len();
        validate_action_count(
            "randomized_verification_keys",
            self.randomized_verification_keys.len(),
            action_count,
        )?;
        validate_action_count(
            "value_commitments",
            self.value_commitments.len(),
            action_count,
        )?;
        validate_action_count(
            "output_commitments",
            self.output_commitments.len(),
            action_count,
        )?;
        validate_action_count(
            "encrypted_outputs",
            self.encrypted_outputs.len(),
            action_count,
        )?;
        validate_action_count(
            "spend_authorization_signatures",
            self.spend_authorization_signatures.len(),
            action_count,
        )?;
        for (index, output) in self.encrypted_outputs.iter().enumerate() {
            if output.cmx != self.output_commitments[index] {
                return Err(OrchardTypeError::new(
                    "output_commitment_mismatch",
                    format!("encrypted output {index} cmx does not match output_commitments"),
                ));
            }
            output.validate()?;
        }
        Ok(())
    }
}

fn parse_fixed_lower_hex(
    label: &'static str,
    value: String,
    expected_bytes: usize,
) -> Result<String, OrchardTypeError> {
    parse_lower_hex(label, &value, expected_bytes, expected_bytes)?;
    Ok(value)
}

fn parse_orchard_anchor(value: &str) -> Result<orchard::Anchor, OrchardTypeError> {
    let bytes = fixed_hex_array::<ORCHARD_ANCHOR_BYTES>("anchor", value)?;
    Option::<orchard::Anchor>::from(orchard::Anchor::from_bytes(bytes)).ok_or_else(|| {
        OrchardTypeError::new(
            "invalid_orchard_anchor",
            "anchor is not a canonical Orchard anchor",
        )
    })
}

fn parse_orchard_nullifier(value: &str) -> Result<orchard::note::Nullifier, OrchardTypeError> {
    let bytes = fixed_hex_array::<ORCHARD_NULLIFIER_BYTES>("nullifier", value)?;
    Option::<orchard::note::Nullifier>::from(orchard::note::Nullifier::from_bytes(&bytes))
        .ok_or_else(|| {
            OrchardTypeError::new(
                "invalid_orchard_nullifier",
                "nullifier is not a canonical Orchard nullifier",
            )
        })
}

fn parse_orchard_output_commitment(
    value: &str,
) -> Result<orchard::note::ExtractedNoteCommitment, OrchardTypeError> {
    let bytes = fixed_hex_array::<ORCHARD_COMMITMENT_BYTES>("output_commitment", value)?;
    Option::<orchard::note::ExtractedNoteCommitment>::from(
        orchard::note::ExtractedNoteCommitment::from_bytes(&bytes),
    )
    .ok_or_else(|| {
        OrchardTypeError::new(
            "invalid_orchard_output_commitment",
            "output commitment is not a canonical Orchard commitment",
        )
    })
}

fn parse_orchard_flags(value: u8) -> Result<orchard::bundle::Flags, OrchardTypeError> {
    orchard::bundle::Flags::from_byte(value).ok_or_else(|| {
        OrchardTypeError::new(
            "invalid_orchard_flags",
            format!("Orchard flags byte {value:#04x} has reserved bits set"),
        )
    })
}

fn parse_orchard_randomized_verification_key(
    value: &str,
) -> Result<
    orchard::primitives::redpallas::VerificationKey<orchard::primitives::redpallas::SpendAuth>,
    OrchardTypeError,
> {
    let bytes = fixed_hex_array::<ORCHARD_RANDOMIZED_VERIFICATION_KEY_BYTES>(
        "randomized_verification_key",
        value,
    )?;
    orchard::primitives::redpallas::VerificationKey::try_from(bytes).map_err(|_| {
        OrchardTypeError::new(
            "invalid_orchard_randomized_verification_key",
            "randomized verification key is not a canonical Orchard spend-auth key",
        )
    })
}

fn parse_orchard_value_commitment(
    value: &str,
) -> Result<orchard::value::ValueCommitment, OrchardTypeError> {
    let bytes = fixed_hex_array::<ORCHARD_VALUE_COMMITMENT_BYTES>("value_commitment", value)?;
    Option::<orchard::value::ValueCommitment>::from(orchard::value::ValueCommitment::from_bytes(
        &bytes,
    ))
    .ok_or_else(|| {
        OrchardTypeError::new(
            "invalid_orchard_value_commitment",
            "value commitment is not a canonical Orchard value commitment",
        )
    })
}

fn fixed_hex_array<const N: usize>(
    label: &'static str,
    value: &str,
) -> Result<[u8; N], OrchardTypeError> {
    let bytes = hex_to_bytes(value).map_err(|error| {
        OrchardTypeError::new("invalid_hex", format!("{label} has invalid hex: {error}"))
    })?;
    bytes.try_into().map_err(|bytes: Vec<u8>| {
        OrchardTypeError::new(
            "invalid_hex_length",
            format!("{label} decoded to {} bytes, expected {N}", bytes.len()),
        )
    })
}

fn parse_lower_hex(
    label: &'static str,
    value: &str,
    min_bytes: usize,
    max_bytes: usize,
) -> Result<(), OrchardTypeError> {
    if value.is_empty() {
        return Err(OrchardTypeError::new(
            "empty_hex",
            format!("{label} hex must not be empty"),
        ));
    }
    if value.trim() != value {
        return Err(OrchardTypeError::new(
            "noncanonical_hex",
            format!("{label} hex has leading or trailing whitespace"),
        ));
    }
    if !value.len().is_multiple_of(2) {
        return Err(OrchardTypeError::new(
            "noncanonical_hex",
            format!("{label} hex has odd length"),
        ));
    }
    if value.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return Err(OrchardTypeError::new(
            "noncanonical_hex",
            format!("{label} hex must be lowercase"),
        ));
    }
    let byte_len = value.len() / 2;
    if byte_len < min_bytes {
        return Err(OrchardTypeError::new(
            "undersized_hex",
            format!("{label} has {byte_len} bytes, min {min_bytes}"),
        ));
    }
    if byte_len > max_bytes {
        return Err(OrchardTypeError::new(
            "oversized_hex",
            format!("{label} has {byte_len} bytes, max {max_bytes}"),
        ));
    }
    hex_to_bytes(value).map_err(|error| {
        OrchardTypeError::new("invalid_hex", format!("{label} has invalid hex: {error}"))
    })?;
    Ok(())
}

fn validate_plain_identifier(label: &'static str, value: &str) -> Result<(), OrchardTypeError> {
    if value.is_empty() {
        return Err(OrchardTypeError::new(
            "empty_identifier",
            format!("{label} must not be empty"),
        ));
    }
    if value.trim() != value || value.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(OrchardTypeError::new(
            "invalid_identifier",
            format!("{label} contains invalid whitespace or control characters"),
        ));
    }
    Ok(())
}

fn validate_blob_exact(
    label: &'static str,
    blob: &BoundedHexBlob,
    expected_bytes: usize,
) -> Result<(), OrchardTypeError> {
    if blob.byte_len() != expected_bytes {
        return Err(OrchardTypeError::new(
            "invalid_blob_length",
            format!(
                "{label} has {} bytes, expected {expected_bytes}",
                blob.byte_len()
            ),
        ));
    }
    Ok(())
}

fn validate_action_count(
    label: &'static str,
    actual: usize,
    expected: usize,
) -> Result<(), OrchardTypeError> {
    if actual != expected {
        return Err(OrchardTypeError::new(
            "action_count_mismatch",
            format!("{label} count {actual} does not match action count {expected}"),
        ));
    }
    Ok(())
}

fn has_duplicate_hex<'a>(values: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = Vec::<&'a str>::new();
    for value in values {
        if seen.iter().any(|existing| *existing == value) {
            return true;
        }
        seen.push(value);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchard::{
        builder::{Builder, BundleType},
        keys::{FullViewingKey, Scope, SpendingKey},
        value::NoteValue,
        Anchor,
    };
    use rand::{rngs::StdRng, SeedableRng};

    fn bytes(byte: u8, len: usize) -> Vec<u8> {
        vec![byte; len]
    }

    struct ValidPublicParts {
        flags: OrchardFlags,
        nullifier: OrchardNullifier,
        randomized_verification_key: OrchardRandomizedVerificationKey,
        value_commitment: OrchardValueCommitment,
        output_commitment: OrchardOutputCommitment,
        encrypted_output: EncryptedShieldedOutput,
    }

    fn valid_public_parts() -> ValidPublicParts {
        let spending_key = SpendingKey::from_bytes([7u8; 32]).unwrap();
        let recipient = FullViewingKey::from(&spending_key).address_at(0u32, Scope::External);
        let mut builder = Builder::new(BundleType::DEFAULT, Anchor::from_bytes([0u8; 32]).unwrap());
        builder
            .add_output(None, recipient, NoteValue::from_raw(10), [0u8; 512])
            .expect("add output");
        let (bundle, _) = builder
            .build::<i64>(StdRng::from_seed([11u8; 32]))
            .expect("build bundle")
            .expect("bundle should exist");
        let action = bundle.actions().iter().next().expect("action");
        let output_commitment = OrchardOutputCommitment::from_orchard(*action.cmx());
        let encrypted_note = action.encrypted_note();
        let encrypted_output = EncryptedShieldedOutput::from_bytes(
            output_commitment.clone(),
            &encrypted_note.epk_bytes,
            &encrypted_note.enc_ciphertext,
            &encrypted_note.out_ciphertext,
            None,
        )
        .expect("encrypted output");

        ValidPublicParts {
            flags: OrchardFlags::from_orchard(*bundle.flags()),
            nullifier: OrchardNullifier::from_orchard(*action.nullifier()),
            randomized_verification_key: OrchardRandomizedVerificationKey::from_orchard(
                action.rk(),
            ),
            value_commitment: OrchardValueCommitment::from_orchard(action.cv_net()),
            output_commitment,
            encrypted_output,
        }
    }

    #[test]
    fn fixed_hex_wrappers_accept_expected_lengths() {
        let anchor = OrchardAnchor::from_bytes(&[1u8; ORCHARD_ANCHOR_BYTES]).expect("anchor");
        let nullifier =
            OrchardNullifier::from_bytes(&[2u8; ORCHARD_NULLIFIER_BYTES]).expect("nullifier");
        let commitment = OrchardOutputCommitment::from_bytes(&[3u8; ORCHARD_COMMITMENT_BYTES])
            .expect("commitment");

        assert_eq!(anchor.as_hex().len(), ORCHARD_ANCHOR_BYTES * 2);
        assert_eq!(nullifier.as_hex().len(), ORCHARD_NULLIFIER_BYTES * 2);
        assert_eq!(commitment.as_hex().len(), ORCHARD_COMMITMENT_BYTES * 2);
        assert!(OrchardAnchor::parse_hex(anchor.as_hex()).is_ok());
        assert!(OrchardNullifier::parse_hex(nullifier.as_hex()).is_ok());
        assert!(OrchardOutputCommitment::parse_hex(commitment.as_hex()).is_ok());
    }

    #[test]
    fn fixed_hex_wrappers_round_trip_upstream_orchard_types() {
        let anchor = OrchardAnchor::from_orchard(orchard::Anchor::empty_tree());
        let upstream_anchor = anchor.to_orchard().expect("anchor");
        assert_eq!(
            upstream_anchor.to_bytes(),
            orchard::Anchor::empty_tree().to_bytes()
        );

        let nullifier =
            OrchardNullifier::from_bytes(&[2u8; ORCHARD_NULLIFIER_BYTES]).expect("nullifier");
        let upstream_nullifier = nullifier.to_orchard().expect("nullifier");
        assert_eq!(
            OrchardNullifier::from_orchard(upstream_nullifier).as_hex(),
            nullifier.as_hex()
        );

        let commitment = OrchardOutputCommitment::from_bytes(&[3u8; ORCHARD_COMMITMENT_BYTES])
            .expect("commitment");
        let upstream_commitment = commitment.to_orchard().expect("commitment");
        assert_eq!(
            OrchardOutputCommitment::from_orchard(upstream_commitment).as_hex(),
            commitment.as_hex()
        );

        let parts = valid_public_parts();
        let upstream_key = parts.randomized_verification_key.to_orchard().expect("rk");
        assert_eq!(
            OrchardRandomizedVerificationKey::from_orchard(&upstream_key).as_hex(),
            parts.randomized_verification_key.as_hex()
        );
        let upstream_value_commitment = parts.value_commitment.to_orchard().expect("cv");
        assert_eq!(
            OrchardValueCommitment::from_orchard(&upstream_value_commitment).as_hex(),
            parts.value_commitment.as_hex()
        );
    }

    #[test]
    fn fixed_hex_wrappers_reject_noncanonical_orchard_field_encodings() {
        assert_eq!(
            OrchardAnchor::parse_hex(bytes_to_hex(&[0xff; ORCHARD_ANCHOR_BYTES]))
                .expect_err("noncanonical anchor")
                .code(),
            "invalid_orchard_anchor"
        );
        assert_eq!(
            OrchardNullifier::parse_hex(bytes_to_hex(&[0xff; ORCHARD_NULLIFIER_BYTES]))
                .expect_err("noncanonical nullifier")
                .code(),
            "invalid_orchard_nullifier"
        );
        assert_eq!(
            OrchardOutputCommitment::parse_hex(bytes_to_hex(&[0xff; ORCHARD_COMMITMENT_BYTES]))
                .expect_err("noncanonical commitment")
                .code(),
            "invalid_orchard_output_commitment"
        );
    }

    #[test]
    fn fixed_hex_wrappers_reject_wrong_lengths_and_uppercase() {
        assert_eq!(
            OrchardAnchor::parse_hex("aa")
                .expect_err("short anchor")
                .code(),
            "undersized_hex"
        );
        assert_eq!(
            OrchardNullifier::parse_hex("AB".repeat(ORCHARD_NULLIFIER_BYTES))
                .expect_err("uppercase nullifier")
                .code(),
            "noncanonical_hex"
        );
        assert_eq!(
            OrchardOutputCommitment::parse_hex("0".repeat(ORCHARD_COMMITMENT_BYTES * 2 + 2))
                .expect_err("long commitment")
                .code(),
            "oversized_hex"
        );
    }

    #[test]
    fn proof_bytes_are_bounded() {
        let proof = OrchardProofBytes::from_bytes(&bytes(7, 128)).expect("proof");
        assert_eq!(proof.byte_len(), 128);

        assert_eq!(
            OrchardProofBytes::from_bytes(&[])
                .expect_err("empty proof")
                .code(),
            "empty_blob"
        );
        assert_eq!(
            OrchardProofBytes::from_bytes(&bytes(0, ORCHARD_PROOF_MAX_BYTES + 1))
                .expect_err("oversized proof")
                .code(),
            "oversized_blob"
        );
    }

    #[test]
    fn legacy_v1_profile_ids_are_rejected() {
        assert_eq!(
            OrchardProofSystemId::parse("postfiat.privacy.orchard-halo2.v1")
                .expect_err("legacy proof system id")
                .code(),
            "unsupported_proof_system"
        );
        assert_eq!(
            OrchardCircuitId::parse("orchard.action.v1")
                .expect_err("legacy circuit id")
                .code(),
            "unsupported_circuit"
        );
    }

    #[test]
    fn action_validation_rejects_mismatched_outputs() {
        let parts = valid_public_parts();
        let action = OrchardShieldedAction {
            pool_id: "orchard-v1".to_string(),
            proof_system_id: OrchardProofSystemId::production_v2(),
            circuit_id: OrchardCircuitId::action_v2(),
            flags: parts.flags,
            anchor: OrchardAnchor::from_bytes(&[1u8; ORCHARD_ANCHOR_BYTES]).expect("anchor"),
            nullifiers: vec![parts.nullifier],
            randomized_verification_keys: vec![parts.randomized_verification_key],
            value_commitments: vec![parts.value_commitment],
            output_commitments: vec![parts.output_commitment],
            encrypted_outputs: Vec::new(),
            value_balance: 0,
            external_binding_hash: None,
            fee: 1,
            proof: OrchardProofBytes::from_bytes(&bytes(4, 128)).expect("proof"),
            spend_authorization_signatures: vec![OrchardSpendAuthSignature::parse_hex(
                bytes_to_hex(&bytes(5, ORCHARD_REDPALLAS_SIGNATURE_BYTES)),
            )
            .expect("spend signature")],
            binding_signature: OrchardBindingSignature::parse_hex(bytes_to_hex(&bytes(
                6,
                ORCHARD_REDPALLAS_SIGNATURE_BYTES,
            )))
            .expect("binding signature"),
        };

        assert_eq!(
            action.validate().expect_err("mismatched outputs").code(),
            "action_count_mismatch"
        );
    }

    #[test]
    fn action_validation_accepts_bounded_shape() {
        let parts = valid_public_parts();
        let action = OrchardShieldedAction {
            pool_id: "orchard-v1".to_string(),
            proof_system_id: OrchardProofSystemId::production_v2(),
            circuit_id: OrchardCircuitId::action_v2(),
            flags: parts.flags,
            anchor: OrchardAnchor::from_bytes(&[1u8; ORCHARD_ANCHOR_BYTES]).expect("anchor"),
            nullifiers: vec![parts.nullifier],
            randomized_verification_keys: vec![parts.randomized_verification_key],
            value_commitments: vec![parts.value_commitment],
            output_commitments: vec![parts.output_commitment],
            encrypted_outputs: vec![parts.encrypted_output],
            value_balance: 0,
            external_binding_hash: Some("11".repeat(ORCHARD_EXTERNAL_BINDING_HASH_BYTES)),
            fee: 1,
            proof: OrchardProofBytes::from_bytes(&bytes(8, 128)).expect("proof"),
            spend_authorization_signatures: vec![OrchardSpendAuthSignature::parse_hex(
                bytes_to_hex(&bytes(9, ORCHARD_REDPALLAS_SIGNATURE_BYTES)),
            )
            .expect("spend signature")],
            binding_signature: OrchardBindingSignature::parse_hex(bytes_to_hex(&bytes(
                10,
                ORCHARD_REDPALLAS_SIGNATURE_BYTES,
            )))
            .expect("binding signature"),
        };

        action.validate().expect("valid action");

        let mut invalid_binding = action;
        invalid_binding.external_binding_hash =
            Some("aa".repeat(ORCHARD_EXTERNAL_BINDING_HASH_BYTES - 1));
        assert_eq!(
            invalid_binding
                .validate()
                .expect_err("short external binding hash must fail")
                .code(),
            "undersized_hex"
        );
    }

    #[test]
    fn output_validation_rejects_wrong_ciphertext_lengths() {
        let commitment = OrchardOutputCommitment::from_bytes(&[3u8; ORCHARD_COMMITMENT_BYTES])
            .expect("commitment");
        let output = EncryptedShieldedOutput {
            cmx: commitment,
            epk: BoundedHexBlob::from_bytes(&bytes(4, ORCHARD_EPK_BYTES), ORCHARD_EPK_BYTES)
                .expect("epk"),
            enc_ciphertext: BoundedHexBlob::from_bytes(
                &bytes(5, ORCHARD_ENC_CIPHERTEXT_BYTES - 1),
                ORCHARD_CIPHERTEXT_MAX_BYTES,
            )
            .expect("enc ciphertext"),
            out_ciphertext: BoundedHexBlob::from_bytes(
                &bytes(6, ORCHARD_OUT_CIPHERTEXT_BYTES),
                ORCHARD_OUT_CIPHERTEXT_BYTES,
            )
            .expect("out ciphertext"),
            compact_ciphertext: None,
        };

        assert_eq!(
            output
                .validate()
                .expect_err("short ciphertext should fail")
                .code(),
            "invalid_blob_length"
        );
    }

    #[test]
    fn blobs_serialize_as_hex_strings() {
        let proof = OrchardProofBytes::from_bytes(&bytes(8, 4)).expect("proof");
        let encoded = serde_json::to_string(&proof).expect("encode proof");
        assert_eq!(encoded, "\"08080808\"");

        let blob =
            BoundedHexBlob::from_bytes(&bytes(9, 3), ORCHARD_CIPHERTEXT_MAX_BYTES).expect("blob");
        let encoded = serde_json::to_string(&blob).expect("encode blob");
        assert_eq!(encoded, "\"090909\"");
    }
}
