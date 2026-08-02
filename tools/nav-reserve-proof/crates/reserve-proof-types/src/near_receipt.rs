//! Staked-NEAR quantity verification from a reader callback receipt included
//! beneath a governed, quorum-certified NEAR head.

use alloy_primitives::{keccak256, B256};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{de::IgnoredAny, ser::SerializeMap, Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use sha3::Sha3_384;
use thiserror::Error;

use crate::bft_checkpoint::{BftSourceCheckpointCertificateV1, BftSourceCheckpointV1};

pub type NearHash = [u8; 32];

pub const NEAR_RECEIPT_QUANTITY_ADAPTER_KIND_V1: &str = "near-stake-receipt-bft-checkpoint-v1";
pub const NEAR_CHECKPOINT_KIND_V1: &str = "near-head-v1";
pub const MAX_NEAR_LOGS: usize = 64;
pub const MAX_NEAR_LOG_BYTES: usize = 16 * 1024;
pub const MAX_NEAR_RECEIPT_IDS: usize = 64;
pub const MAX_NEAR_MERKLE_PATH: usize = 128;
pub const MAX_NEAR_APPROVALS: usize = 256;
pub const MAX_NEAR_VALIDATOR_PROPOSALS: usize = 256;
pub const MAX_NEAR_CHUNK_ITEMS: usize = 256;
pub const MAX_NEAR_CHUNK_ENDORSEMENT_BYTES: usize = 16 * 1024;
pub const MAX_NEAR_PAYLOAD_BYTES: usize = 64 * 1024;
pub const MAX_NEAR_ACCOUNT_ID_BYTES: usize = 64;
pub const MAX_NEAR_NAMED_ACCOUNT_ID_BYTES: usize = 256;

const POLICY_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_near_receipt_policy.v1";
const OWNER_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_near_owner_commitment.v1";
const OWNER_AUTHORIZATION_DOMAIN: &[u8] = b"postfiat.reserve_near_owner_authorization.v1";
const SOURCE_STATE_DOMAIN: &[u8] = b"postfiat.reserve_near_source_state.v1";
const EVIDENCE_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_near_receipt_evidence.v1";
const METADATA_DOMAIN: &[u8] = b"postfiat.reserve_near_receipt_metadata.v1";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NearReceiptPolicyV1 {
    pub source_domain: String,
    pub reader_account_id: String,
    pub reader_code_hash: String,
    pub pool_id: String,
    pub pool_code_hash: String,
    pub snapshot_standard: String,
    pub snapshot_version: String,
    pub snapshot_event: String,
    pub owner_public_key: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NearReceiptQuantityProofV1 {
    pub policy: NearReceiptPolicyV1,
    pub checkpoint_certificate: BftSourceCheckpointCertificateV1,
    pub account_id: String,
    pub ownership_signature: Vec<u8>,
    pub commitment: String,
    pub salt: Vec<u8>,
    pub payload: Vec<u8>,
    pub proof: NearLightClientProof,
    pub head: NearHeadBlock,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NearReceiptQuantityVerificationV1 {
    pub proven_block_hash: String,
    pub outcome_root: String,
    pub head_block_hash: String,
    pub head_block_merkle_root: String,
    pub commitment: String,
    pub staked_yocto: u128,
    pub unstaked_yocto: u128,
    pub total_yocto: u128,
    pub evidence_commitment: String,
    pub metadata_hash: B256,
}

pub struct NearReceiptVerifyContextV1<'a> {
    pub pftl_genesis_hash: &'a str,
    pub nav_asset_id: &'a str,
    pub proof_profile_id: &'a str,
    pub valuation_policy_hash: &'a str,
    pub source_manifest_hash: &'a str,
    pub source_id: &'a str,
    pub source_domain: &'a str,
    pub asset_or_position_id: &'a str,
    pub reserve_owner_commitment: &'a str,
    pub quantity_verifier_commitment: &'a str,
    pub observed_at_pftl_height: u64,
    pub expected_evidence_commitment: &'a str,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NearLightClientProof {
    pub block_header_lite: NearBlockHeaderLite,
    pub outcome_proof: NearOutcomeProof,
    pub outcome_root_proof: Vec<NearMerklePathItem>,
    pub block_proof: Vec<NearMerklePathItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NearBlockHeaderLite {
    pub inner_lite: NearInnerLite,
    pub inner_rest_hash: String,
    pub prev_block_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NearInnerLite {
    pub height: u64,
    pub epoch_id: String,
    pub next_epoch_id: String,
    pub prev_state_root: String,
    pub outcome_root: String,
    pub timestamp: u64,
    pub timestamp_nanosec: String,
    pub next_bp_hash: String,
    pub block_merkle_root: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NearOutcomeProof {
    pub block_hash: String,
    pub id: String,
    pub outcome: NearExecutionOutcome,
    pub proof: Vec<NearMerklePathItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NearExecutionOutcome {
    pub logs: Vec<String>,
    pub receipt_ids: Vec<String>,
    pub gas_burnt: u64,
    pub tokens_burnt: String,
    pub executor_id: String,
    pub status: NearExecutionStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NearExecutionStatus {
    Unknown,
    Failure,
    SuccessValue(Vec<u8>),
    SuccessReceiptId(String),
}

#[derive(Serialize, Deserialize)]
enum NearExecutionStatusBinary {
    Unknown,
    Failure,
    SuccessValue(Vec<u8>),
    SuccessReceiptId(String),
}

impl Serialize for NearExecutionStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if !serializer.is_human_readable() {
            let binary = match self {
                Self::Unknown => NearExecutionStatusBinary::Unknown,
                Self::Failure => NearExecutionStatusBinary::Failure,
                Self::SuccessValue(value) => NearExecutionStatusBinary::SuccessValue(value.clone()),
                Self::SuccessReceiptId(value) => {
                    NearExecutionStatusBinary::SuccessReceiptId(value.clone())
                }
            };
            return binary.serialize(serializer);
        }

        match self {
            Self::Unknown => serializer.serialize_str("Unknown"),
            Self::Failure => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("Failure", &())?;
                map.end()
            }
            Self::SuccessValue(value) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("SuccessValue", &BASE64.encode(value))?;
                map.end()
            }
            Self::SuccessReceiptId(value) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("SuccessReceiptId", value)?;
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for NearExecutionStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            return Ok(
                match NearExecutionStatusBinary::deserialize(deserializer)? {
                    NearExecutionStatusBinary::Unknown => Self::Unknown,
                    NearExecutionStatusBinary::Failure => Self::Failure,
                    NearExecutionStatusBinary::SuccessValue(value) => Self::SuccessValue(value),
                    NearExecutionStatusBinary::SuccessReceiptId(value) => {
                        Self::SuccessReceiptId(value)
                    }
                },
            );
        }

        let status = serde_json::Value::deserialize(deserializer)?;
        if status == "Unknown" {
            return Ok(Self::Unknown);
        }
        let object = status
            .as_object()
            .ok_or_else(|| serde::de::Error::custom("NEAR execution status must be an object"))?;
        if object.contains_key("Unknown") {
            Ok(Self::Unknown)
        } else if object.contains_key("Failure") {
            Ok(Self::Failure)
        } else if let Some(value) = object.get("SuccessValue") {
            let bytes = match value {
                serde_json::Value::String(encoded) => BASE64
                    .decode(encoded)
                    .map_err(|_| serde::de::Error::custom("SuccessValue was not valid base64"))?,
                serde_json::Value::Array(items) => {
                    let mut bytes = Vec::with_capacity(items.len());
                    for item in items {
                        let byte = item
                            .as_u64()
                            .ok_or_else(|| serde::de::Error::custom("SuccessValue byte item"))?;
                        bytes
                            .push(u8::try_from(byte).map_err(|_| {
                                serde::de::Error::custom("SuccessValue byte range")
                            })?);
                    }
                    bytes
                }
                _ => {
                    return Err(serde::de::Error::custom(
                        "SuccessValue must be base64 string or byte array",
                    ));
                }
            };
            Ok(Self::SuccessValue(bytes))
        } else if let Some(value) = object.get("SuccessReceiptId") {
            let receipt_id = value
                .as_str()
                .ok_or_else(|| serde::de::Error::custom("SuccessReceiptId must be string"))?;
            Ok(Self::SuccessReceiptId(receipt_id.to_string()))
        } else {
            Err(serde::de::Error::custom("unknown NEAR execution status"))
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NearIgnoredJson;

impl Serialize for NearIgnoredJson {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_unit()
    }
}

impl<'de> Deserialize<'de> for NearIgnoredJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            IgnoredAny::deserialize(deserializer)?;
        } else {
            <()>::deserialize(deserializer)?;
        }
        Ok(Self)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NearMerklePathItem {
    pub hash: String,
    pub direction: NearMerkleDirection,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum NearMerkleDirection {
    Left,
    Right,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NearHeadBlock {
    pub head_height: u64,
    pub head_hash: String,
    pub head_block_merkle_root: String,
    pub header: NearFullBlockHeader,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NearFullBlockHeader {
    pub approvals: Vec<Option<String>>,
    pub block_body_hash: String,
    pub block_merkle_root: String,
    pub block_ordinal: u64,
    pub challenges_result: Vec<NearIgnoredJson>,
    pub challenges_root: String,
    pub chunk_endorsements: Vec<Vec<u8>>,
    pub chunk_headers_root: String,
    pub chunk_mask: Vec<bool>,
    pub chunk_receipts_root: String,
    pub chunk_tx_root: String,
    pub epoch_id: String,
    pub epoch_sync_data_hash: Option<String>,
    pub gas_price: String,
    pub hash: String,
    pub height: u64,
    pub last_ds_final_block: String,
    pub last_final_block: String,
    pub latest_protocol_version: u32,
    pub next_bp_hash: String,
    pub next_epoch_id: String,
    pub outcome_root: String,
    pub prev_hash: String,
    pub prev_height: u64,
    pub prev_state_root: String,
    pub random_value: String,
    pub shard_split: Option<(u64, String)>,
    pub timestamp: u64,
    pub timestamp_nanosec: String,
    pub total_supply: String,
    pub validator_proposals: Vec<NearValidatorStakeView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NearValidatorStakeView {
    pub account_id: String,
    pub public_key: String,
    pub stake: String,
    pub validator_stake_struct_version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NearApprovalReport {
    pub approval_message: Vec<u8>,
    pub approvals_present: usize,
    pub approvals_valid: usize,
    pub total_stake_yocto: u128,
    pub approved_stake_yocto: u128,
    pub quorum: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NearSnapshotPayload {
    pub account_id: String,
    pub pool_id: String,
    pub staked_yocto: u128,
    pub unstaked_yocto: u128,
    pub block_timestamp: u64,
    pub salt: [u8; 32],
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NearReceiptLegError {
    #[error("base58 hash had bad length or encoding")]
    BadBase58Hash,
    #[error("proven block hash mismatch")]
    BadProvenBlockHash,
    #[error("outcome root proof failed")]
    BadOutcomeRoot,
    #[error("block proof did not fold to the pinned head block merkle root")]
    BadBlockMerkleRoot,
    #[error("head block hash does not equal the certified head hash")]
    BadCertifiedHeadHash,
    #[error("unsupported NEAR head header version")]
    UnsupportedHeadVersion,
    #[error("outcome status was not a callback SuccessValue")]
    BadOutcomeStatus,
    #[error("outcome executor does not match reader account")]
    ReaderMismatch,
    #[error("snapshot event missing")]
    MissingSnapshotEvent,
    #[error("snapshot event malformed or inconsistent")]
    BadSnapshotEvent,
    #[error("payload malformed")]
    BadPayload,
    #[error("payload account mismatch")]
    AccountMismatch,
    #[error("payload pool mismatch")]
    PoolMismatch,
    #[error("bad salt")]
    BadSalt,
    #[error("validator set hash mismatch")]
    BadValidatorSetHash,
    #[error("approval signature failed")]
    BadApprovalSignature,
    #[error("arithmetic overflow")]
    ArithmeticOverflow,
    #[error("proof or policy bounds exceeded")]
    BoundsExceeded,
    #[error("policy binding mismatch")]
    PolicyMismatch,
    #[error("checkpoint binding or certificate mismatch")]
    CheckpointMismatch,
    #[error("reserve owner authorization failed")]
    OwnerAuthorization,
    #[error("evidence commitment mismatch")]
    EvidenceCommitment,
}

impl NearReceiptPolicyV1 {
    pub fn validate(&self) -> Result<(), NearReceiptLegError> {
        validate_identifier(&self.source_domain)?;
        validate_near_account_id(&self.reader_account_id, MAX_NEAR_NAMED_ACCOUNT_ID_BYTES)?;
        validate_near_account_id(&self.pool_id, MAX_NEAR_NAMED_ACCOUNT_ID_BYTES)?;
        decode_hash(&self.reader_code_hash)?;
        decode_hash(&self.pool_code_hash)?;
        validate_text(&self.snapshot_standard, 128)?;
        validate_text(&self.snapshot_version, 64)?;
        validate_text(&self.snapshot_event, 128)?;
        if self.owner_public_key.len() != 32 {
            return Err(NearReceiptLegError::PolicyMismatch);
        }
        VerifyingKey::from_bytes(
            self.owner_public_key
                .as_slice()
                .try_into()
                .map_err(|_| NearReceiptLegError::PolicyMismatch)?,
        )
        .map_err(|_| NearReceiptLegError::PolicyMismatch)?;
        Ok(())
    }

    pub fn commitment(&self, committee_root: &str) -> Result<String, NearReceiptLegError> {
        self.validate()?;
        validate_lower_hex(committee_root, 48)?;
        let mut out = Vec::new();
        append_bytes(&mut out, self.source_domain.as_bytes())?;
        append_bytes(&mut out, self.reader_account_id.as_bytes())?;
        out.extend_from_slice(&decode_hash(&self.reader_code_hash)?);
        append_bytes(&mut out, self.pool_id.as_bytes())?;
        out.extend_from_slice(&decode_hash(&self.pool_code_hash)?);
        append_bytes(&mut out, self.snapshot_standard.as_bytes())?;
        append_bytes(&mut out, self.snapshot_version.as_bytes())?;
        append_bytes(&mut out, self.snapshot_event.as_bytes())?;
        append_bytes(&mut out, &self.owner_public_key)?;
        append_hex(&mut out, committee_root, 48)?;
        Ok(hash48(POLICY_COMMITMENT_DOMAIN, &[&out]))
    }

    pub fn reserve_owner_commitment(
        &self,
        account_id: &str,
    ) -> Result<String, NearReceiptLegError> {
        self.validate()?;
        validate_implicit_account(account_id, &self.owner_public_key)?;
        Ok(hash48(
            OWNER_COMMITMENT_DOMAIN,
            &[account_id.as_bytes(), &self.owner_public_key],
        ))
    }

    pub fn source_state_commitment(
        &self,
        head_block_merkle_root: &NearHash,
    ) -> Result<B256, NearReceiptLegError> {
        self.validate()?;
        let mut out = Vec::new();
        append_bytes(&mut out, self.source_domain.as_bytes())?;
        out.extend_from_slice(head_block_merkle_root);
        append_bytes(&mut out, self.reader_account_id.as_bytes())?;
        out.extend_from_slice(&decode_hash(&self.reader_code_hash)?);
        append_bytes(&mut out, self.pool_id.as_bytes())?;
        out.extend_from_slice(&decode_hash(&self.pool_code_hash)?);
        Ok(keccak256(domain_message(SOURCE_STATE_DOMAIN, &out)))
    }
}

pub fn verify_near_receipt_quantity_proof_v1(
    witness: &NearReceiptQuantityProofV1,
    context: &NearReceiptVerifyContextV1<'_>,
) -> Result<NearReceiptQuantityVerificationV1, NearReceiptLegError> {
    validate_proof_bounds(witness)?;
    witness.policy.validate()?;
    witness
        .checkpoint_certificate
        .verify()
        .map_err(|_| NearReceiptLegError::CheckpointMismatch)?;
    let checkpoint = &witness.checkpoint_certificate.checkpoint;
    let committee_root = witness
        .checkpoint_certificate
        .committee
        .root()
        .map_err(|_| NearReceiptLegError::CheckpointMismatch)?;
    let policy_commitment = witness.policy.commitment(&committee_root)?;
    if policy_commitment != context.quantity_verifier_commitment
        || witness.policy.source_domain != context.source_domain
    {
        return Err(NearReceiptLegError::PolicyMismatch);
    }
    if checkpoint.pftl_genesis_hash != context.pftl_genesis_hash
        || checkpoint.checkpoint_kind != NEAR_CHECKPOINT_KIND_V1
        || checkpoint.source_domain != context.source_domain
        || checkpoint.pftl_observation_height != context.observed_at_pftl_height
    {
        return Err(NearReceiptLegError::CheckpointMismatch);
    }
    let owner_commitment = witness
        .policy
        .reserve_owner_commitment(&witness.account_id)?;
    if owner_commitment != context.reserve_owner_commitment {
        return Err(NearReceiptLegError::OwnerAuthorization);
    }

    let proven_block_hash = near_block_hash_from_lite(&witness.proof.block_header_lite)?;
    let expected_proven_block_hash = decode_hash(&witness.proof.outcome_proof.block_hash)?;
    if proven_block_hash != expected_proven_block_hash {
        return Err(NearReceiptLegError::BadProvenBlockHash);
    }

    let outcome_root = near_outcome_root(&witness.proof)?;
    let expected_outcome_root =
        decode_hash(&witness.proof.block_header_lite.inner_lite.outcome_root)?;
    if outcome_root != expected_outcome_root {
        return Err(NearReceiptLegError::BadOutcomeRoot);
    }

    let block_merkle_root = near_block_merkle_root_from_proof(&witness.proof)?;
    let expected_block_merkle_root = decode_hash(&witness.head.header.block_merkle_root)?;
    if block_merkle_root != expected_block_merkle_root {
        return Err(NearReceiptLegError::BadBlockMerkleRoot);
    }

    let head_hash = near_head_block_hash(&witness.head.header)?;
    if B256::from(head_hash) != checkpoint.source_block_hash
        || checkpoint.source_height != witness.head.head_height
        || checkpoint.source_height != witness.head.header.height
        || witness.head.head_hash != to_base58(&head_hash)
        || witness.head.header.hash != witness.head.head_hash
        || witness.head.head_block_merkle_root != witness.head.header.block_merkle_root
    {
        return Err(NearReceiptLegError::BadCertifiedHeadHash);
    }
    let source_state_commitment = witness
        .policy
        .source_state_commitment(&expected_block_merkle_root)?;
    if source_state_commitment != checkpoint.source_state_commitment {
        return Err(NearReceiptLegError::CheckpointMismatch);
    }

    if witness.proof.outcome_proof.outcome.executor_id != witness.policy.reader_account_id {
        return Err(NearReceiptLegError::ReaderMismatch);
    }
    let success_value = near_success_value_payload(&witness.proof.outcome_proof.outcome.status)?;
    if success_value != witness.payload {
        return Err(NearReceiptLegError::BadPayload);
    }

    let commitment = decode_hash(&witness.commitment)?;
    let computed_commitment = sha256(&witness.payload);
    if commitment != computed_commitment {
        return Err(NearReceiptLegError::BadSnapshotEvent);
    }
    let event =
        near_snapshot_event_from_logs(&witness.proof.outcome_proof.outcome.logs, &witness.policy)?;
    if event.commitment != witness.commitment || event.payload != witness.payload {
        return Err(NearReceiptLegError::BadSnapshotEvent);
    }

    let payload = decode_near_snapshot_payload(&witness.payload)?;
    if payload.account_id != witness.account_id {
        return Err(NearReceiptLegError::AccountMismatch);
    }
    if payload.pool_id != witness.policy.pool_id {
        return Err(NearReceiptLegError::PoolMismatch);
    }
    if event.block_timestamp != payload.block_timestamp {
        return Err(NearReceiptLegError::BadSnapshotEvent);
    }
    let salt: [u8; 32] = witness
        .salt
        .as_slice()
        .try_into()
        .map_err(|_| NearReceiptLegError::BadSalt)?;
    if payload.salt != salt {
        return Err(NearReceiptLegError::BadSalt);
    }

    let total_yocto = payload
        .staked_yocto
        .checked_add(payload.unstaked_yocto)
        .ok_or(NearReceiptLegError::ArithmeticOverflow)?;
    verify_owner_authorization(
        witness,
        context,
        &policy_commitment,
        checkpoint,
        &commitment,
    )?;
    let evidence_commitment = near_receipt_evidence_commitment(
        witness,
        &policy_commitment,
        checkpoint,
        &proven_block_hash,
        &outcome_root,
        &head_hash,
        &commitment,
        total_yocto,
    )?;
    if evidence_commitment != context.expected_evidence_commitment {
        return Err(NearReceiptLegError::EvidenceCommitment);
    }
    let metadata_hash = near_receipt_metadata_hash(
        witness,
        &policy_commitment,
        &proven_block_hash,
        &outcome_root,
        &head_hash,
        &commitment,
    )?;

    Ok(NearReceiptQuantityVerificationV1 {
        proven_block_hash: to_base58(&proven_block_hash),
        outcome_root: to_base58(&outcome_root),
        head_block_hash: to_base58(&head_hash),
        head_block_merkle_root: to_base58(&block_merkle_root),
        commitment: to_base58(&commitment),
        staked_yocto: payload.staked_yocto,
        unstaked_yocto: payload.unstaked_yocto,
        total_yocto,
        evidence_commitment,
        metadata_hash,
    })
}

pub fn near_block_hash_from_lite(
    header: &NearBlockHeaderLite,
) -> Result<NearHash, NearReceiptLegError> {
    let inner_lite_hash = sha256(&encode_inner_lite(&header.inner_lite)?);
    let inner_rest_hash = decode_hash(&header.inner_rest_hash)?;
    let prev_block_hash = decode_hash(&header.prev_block_hash)?;
    Ok(combine_hash(
        &combine_hash(&inner_lite_hash, &inner_rest_hash),
        &prev_block_hash,
    ))
}

pub fn near_head_block_hash(header: &NearFullBlockHeader) -> Result<NearHash, NearReceiptLegError> {
    if header.latest_protocol_version < 70 {
        return Err(NearReceiptLegError::UnsupportedHeadVersion);
    }
    let inner_lite = NearInnerLite {
        height: header.height,
        epoch_id: header.epoch_id.clone(),
        next_epoch_id: header.next_epoch_id.clone(),
        prev_state_root: header.prev_state_root.clone(),
        outcome_root: header.outcome_root.clone(),
        timestamp: header.timestamp,
        timestamp_nanosec: header.timestamp_nanosec.clone(),
        next_bp_hash: header.next_bp_hash.clone(),
        block_merkle_root: header.block_merkle_root.clone(),
    };
    let inner_lite_hash = sha256(&encode_inner_lite(&inner_lite)?);
    // NEAR's DynamicResharding feature activates BlockHeaderV6 at protocol
    // version 85. V6 removes the deprecated challenge fields and appends the
    // optional shard split. Using a speculative future threshold here hashes
    // current mainnet v6 headers as v5 and makes every valid receipt fail.
    let inner_rest = if header.latest_protocol_version >= 85 {
        encode_inner_rest_v6(header)?
    } else {
        encode_inner_rest_v5(header)?
    };
    let inner_rest_hash = sha256(&inner_rest);
    let prev_hash = decode_hash(&header.prev_hash)?;
    Ok(combine_hash(
        &combine_hash(&inner_lite_hash, &inner_rest_hash),
        &prev_hash,
    ))
}

pub fn near_outcome_root(proof: &NearLightClientProof) -> Result<NearHash, NearReceiptLegError> {
    let outcome_leaf = near_outcome_leaf_hash(&proof.outcome_proof)?;
    let chunk_outcome_root = fold_merkle_path(outcome_leaf, &proof.outcome_proof.proof)?;
    fold_merkle_path(sha256(&chunk_outcome_root), &proof.outcome_root_proof)
}

pub fn near_block_merkle_root_from_proof(
    proof: &NearLightClientProof,
) -> Result<NearHash, NearReceiptLegError> {
    let proven_block_hash = near_block_hash_from_lite(&proof.block_header_lite)?;
    fold_merkle_path(proven_block_hash, &proof.block_proof)
}

pub fn near_outcome_leaf_hash(proof: &NearOutcomeProof) -> Result<NearHash, NearReceiptLegError> {
    let id = decode_hash(&proof.id)?;
    let partial_hash = sha256(&encode_partial_outcome(&proof.outcome)?);
    let mut hashes = Vec::with_capacity(proof.outcome.logs.len() + 2);
    hashes.push(id);
    hashes.push(partial_hash);
    hashes.extend(proof.outcome.logs.iter().map(|log| sha256(log.as_bytes())));
    Ok(hash_borsh_hash_vec(&hashes))
}

pub fn near_success_value_payload(
    status: &NearExecutionStatus,
) -> Result<Vec<u8>, NearReceiptLegError> {
    match status {
        NearExecutionStatus::SuccessValue(value) => Ok(value.clone()),
        _ => Err(NearReceiptLegError::BadOutcomeStatus),
    }
}

pub fn near_snapshot_event_from_logs(
    logs: &[String],
    policy: &NearReceiptPolicyV1,
) -> Result<NearSnapshotEvent, NearReceiptLegError> {
    if logs.len() > MAX_NEAR_LOGS {
        return Err(NearReceiptLegError::BoundsExceeded);
    }
    for log in logs {
        if log.len() > MAX_NEAR_LOG_BYTES {
            return Err(NearReceiptLegError::BoundsExceeded);
        }
        let Some(json) = log.strip_prefix("EVENT_JSON:") else {
            continue;
        };
        let event: NearNep297Event =
            serde_json::from_str(json).map_err(|_| NearReceiptLegError::BadSnapshotEvent)?;
        if event.standard != policy.snapshot_standard
            || event.version != policy.snapshot_version
            || event.event != policy.snapshot_event
            || event.data.len() != 1
        {
            continue;
        }
        let row = event
            .data
            .into_iter()
            .next()
            .ok_or(NearReceiptLegError::BadSnapshotEvent)?;
        let commitment = decode_hash(&row.commitment)?;
        let payload = BASE64
            .decode(&row.payload)
            .map_err(|_| NearReceiptLegError::BadSnapshotEvent)?;
        if payload.len() > MAX_NEAR_PAYLOAD_BYTES || sha256(&payload) != commitment {
            return Err(NearReceiptLegError::BadSnapshotEvent);
        }
        return Ok(NearSnapshotEvent {
            commitment: row.commitment,
            block_timestamp: row.block_timestamp,
            payload,
        });
    }
    Err(NearReceiptLegError::MissingSnapshotEvent)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NearSnapshotEvent {
    pub commitment: String,
    pub block_timestamp: u64,
    pub payload: Vec<u8>,
}

#[derive(Debug, Deserialize)]
struct NearNep297Event {
    standard: String,
    version: String,
    event: String,
    data: Vec<NearSnapshotEventRow>,
}

#[derive(Debug, Deserialize)]
struct NearSnapshotEventRow {
    commitment: String,
    block_timestamp: u64,
    payload: String,
}

pub fn decode_near_snapshot_payload(
    payload: &[u8],
) -> Result<NearSnapshotPayload, NearReceiptLegError> {
    let mut cursor = NearPayloadCursor::new(payload);
    let account_id = cursor.string()?;
    let pool_id = cursor.string()?;
    let staked_yocto = cursor.u128()?;
    let unstaked_yocto = cursor.u128()?;
    let block_timestamp = cursor.u64()?;
    let salt = cursor.bytes_array::<32>()?;
    cursor.done()?;
    Ok(NearSnapshotPayload {
        account_id,
        pool_id,
        staked_yocto,
        unstaked_yocto,
        block_timestamp,
        salt,
    })
}

pub fn near_receipt_metadata_hash(
    witness: &NearReceiptQuantityProofV1,
    policy_commitment: &str,
    proven_block_hash: &NearHash,
    outcome_root: &NearHash,
    head_hash: &NearHash,
    commitment: &NearHash,
) -> Result<B256, NearReceiptLegError> {
    let mut out = Vec::new();
    append_hex(&mut out, policy_commitment, 48)?;
    append_bytes(&mut out, witness.account_id.as_bytes())?;
    out.extend_from_slice(proven_block_hash);
    out.extend_from_slice(outcome_root);
    out.extend_from_slice(head_hash);
    out.extend_from_slice(commitment);
    Ok(keccak256(domain_message(METADATA_DOMAIN, &out)))
}

pub fn near_owner_authorization_statement_v1(
    witness: &NearReceiptQuantityProofV1,
    context: &NearReceiptVerifyContextV1<'_>,
) -> Result<Vec<u8>, NearReceiptLegError> {
    validate_proof_bounds(witness)?;
    validate_context_bindings(context)?;
    let committee_root = witness
        .checkpoint_certificate
        .committee
        .root()
        .map_err(|_| NearReceiptLegError::CheckpointMismatch)?;
    let policy_commitment = witness.policy.commitment(&committee_root)?;
    let commitment = decode_hash(&witness.commitment)?;
    owner_authorization_statement(
        witness,
        context,
        &policy_commitment,
        &witness.checkpoint_certificate.checkpoint,
        &commitment,
    )
}

pub fn near_receipt_evidence_commitment_v1(
    witness: &NearReceiptQuantityProofV1,
) -> Result<String, NearReceiptLegError> {
    validate_proof_bounds(witness)?;
    let committee_root = witness
        .checkpoint_certificate
        .committee
        .root()
        .map_err(|_| NearReceiptLegError::CheckpointMismatch)?;
    let policy_commitment = witness.policy.commitment(&committee_root)?;
    let proven_block_hash = near_block_hash_from_lite(&witness.proof.block_header_lite)?;
    let outcome_root = near_outcome_root(&witness.proof)?;
    let head_hash = near_head_block_hash(&witness.head.header)?;
    let commitment = decode_hash(&witness.commitment)?;
    let payload = decode_near_snapshot_payload(&witness.payload)?;
    let total_yocto = payload
        .staked_yocto
        .checked_add(payload.unstaked_yocto)
        .ok_or(NearReceiptLegError::ArithmeticOverflow)?;
    near_receipt_evidence_commitment(
        witness,
        &policy_commitment,
        &witness.checkpoint_certificate.checkpoint,
        &proven_block_hash,
        &outcome_root,
        &head_hash,
        &commitment,
        total_yocto,
    )
}

fn verify_owner_authorization(
    witness: &NearReceiptQuantityProofV1,
    context: &NearReceiptVerifyContextV1<'_>,
    policy_commitment: &str,
    checkpoint: &BftSourceCheckpointV1,
    commitment: &NearHash,
) -> Result<(), NearReceiptLegError> {
    if witness.ownership_signature.len() != 64 {
        return Err(NearReceiptLegError::OwnerAuthorization);
    }
    let key_bytes: &[u8; 32] = witness
        .policy
        .owner_public_key
        .as_slice()
        .try_into()
        .map_err(|_| NearReceiptLegError::OwnerAuthorization)?;
    let key =
        VerifyingKey::from_bytes(key_bytes).map_err(|_| NearReceiptLegError::OwnerAuthorization)?;
    let signature = Signature::from_slice(&witness.ownership_signature)
        .map_err(|_| NearReceiptLegError::OwnerAuthorization)?;
    let statement =
        owner_authorization_statement(witness, context, policy_commitment, checkpoint, commitment)?;
    key.verify(&statement, &signature)
        .map_err(|_| NearReceiptLegError::OwnerAuthorization)
}

fn owner_authorization_statement(
    witness: &NearReceiptQuantityProofV1,
    context: &NearReceiptVerifyContextV1<'_>,
    policy_commitment: &str,
    checkpoint: &BftSourceCheckpointV1,
    commitment: &NearHash,
) -> Result<Vec<u8>, NearReceiptLegError> {
    validate_context_bindings(context)?;
    let mut out = Vec::new();
    append_hex(&mut out, context.pftl_genesis_hash, 48)?;
    append_hex(&mut out, context.nav_asset_id, 48)?;
    append_hex(&mut out, context.proof_profile_id, 48)?;
    append_hex(&mut out, context.valuation_policy_hash, 32)?;
    append_hex(&mut out, context.source_manifest_hash, 48)?;
    append_bytes(&mut out, context.source_id.as_bytes())?;
    append_bytes(&mut out, context.source_domain.as_bytes())?;
    append_bytes(&mut out, context.asset_or_position_id.as_bytes())?;
    append_hex(&mut out, context.reserve_owner_commitment, 48)?;
    append_hex(&mut out, context.quantity_verifier_commitment, 48)?;
    out.extend_from_slice(&context.observed_at_pftl_height.to_be_bytes());
    append_hex(&mut out, policy_commitment, 48)?;
    append_bytes(
        &mut out,
        &checkpoint
            .canonical_bytes()
            .map_err(|_| NearReceiptLegError::CheckpointMismatch)?,
    )?;
    append_bytes(&mut out, witness.account_id.as_bytes())?;
    out.extend_from_slice(commitment);
    append_bytes(&mut out, &witness.salt)?;
    Ok(domain_message(OWNER_AUTHORIZATION_DOMAIN, &out))
}

#[allow(clippy::too_many_arguments)]
fn near_receipt_evidence_commitment(
    witness: &NearReceiptQuantityProofV1,
    policy_commitment: &str,
    checkpoint: &BftSourceCheckpointV1,
    proven_block_hash: &NearHash,
    outcome_root: &NearHash,
    head_hash: &NearHash,
    commitment: &NearHash,
    total_yocto: u128,
) -> Result<String, NearReceiptLegError> {
    let mut out = Vec::new();
    append_hex(&mut out, policy_commitment, 48)?;
    append_bytes(
        &mut out,
        &checkpoint
            .canonical_bytes()
            .map_err(|_| NearReceiptLegError::CheckpointMismatch)?,
    )?;
    append_bytes(&mut out, witness.account_id.as_bytes())?;
    append_bytes(&mut out, &witness.ownership_signature)?;
    out.extend_from_slice(proven_block_hash);
    out.extend_from_slice(outcome_root);
    out.extend_from_slice(head_hash);
    out.extend_from_slice(commitment);
    out.extend_from_slice(&total_yocto.to_be_bytes());
    Ok(hash48(EVIDENCE_COMMITMENT_DOMAIN, &[&out]))
}

fn validate_context_bindings(
    context: &NearReceiptVerifyContextV1<'_>,
) -> Result<(), NearReceiptLegError> {
    validate_lower_hex(context.pftl_genesis_hash, 48)?;
    validate_lower_hex(context.nav_asset_id, 48)?;
    validate_lower_hex(context.proof_profile_id, 48)?;
    validate_lower_hex(context.valuation_policy_hash, 32)?;
    validate_lower_hex(context.source_manifest_hash, 48)?;
    validate_lower_hex(context.reserve_owner_commitment, 48)?;
    validate_lower_hex(context.quantity_verifier_commitment, 48)?;
    validate_lower_hex(context.expected_evidence_commitment, 48)?;
    validate_identifier(context.source_id)?;
    validate_identifier(context.source_domain)?;
    validate_text(context.asset_or_position_id, 256)?;
    if context.observed_at_pftl_height == 0 {
        return Err(NearReceiptLegError::PolicyMismatch);
    }
    Ok(())
}

fn validate_proof_bounds(witness: &NearReceiptQuantityProofV1) -> Result<(), NearReceiptLegError> {
    witness.policy.validate()?;
    validate_implicit_account(&witness.account_id, &witness.policy.owner_public_key)?;
    if witness.ownership_signature.len() != 64
        || witness.salt.len() != 32
        || witness.payload.is_empty()
        || witness.payload.len() > MAX_NEAR_PAYLOAD_BYTES
        || witness.proof.outcome_proof.proof.len() > MAX_NEAR_MERKLE_PATH
        || witness.proof.outcome_root_proof.len() > MAX_NEAR_MERKLE_PATH
        || witness.proof.block_proof.len() > MAX_NEAR_MERKLE_PATH
        || witness.proof.outcome_proof.outcome.logs.len() > MAX_NEAR_LOGS
        || witness.proof.outcome_proof.outcome.receipt_ids.len() > MAX_NEAR_RECEIPT_IDS
        || witness.head.header.approvals.len() > MAX_NEAR_APPROVALS
        || witness.head.header.validator_proposals.len() > MAX_NEAR_VALIDATOR_PROPOSALS
        || witness.head.header.chunk_mask.len() > MAX_NEAR_CHUNK_ITEMS
        || witness.head.header.chunk_endorsements.len() > MAX_NEAR_CHUNK_ITEMS
        || witness.head.header.challenges_result.len() > MAX_NEAR_CHUNK_ITEMS
    {
        return Err(NearReceiptLegError::BoundsExceeded);
    }
    if witness
        .proof
        .outcome_proof
        .outcome
        .logs
        .iter()
        .any(|log| log.len() > MAX_NEAR_LOG_BYTES)
        || witness
            .head
            .header
            .chunk_endorsements
            .iter()
            .any(|value| value.len() > MAX_NEAR_CHUNK_ENDORSEMENT_BYTES)
        || witness
            .head
            .header
            .validator_proposals
            .iter()
            .any(|validator| {
                validator.account_id.len() > MAX_NEAR_NAMED_ACCOUNT_ID_BYTES
                    || validator.public_key.len() > 128
                    || validator.stake.len() > 39
                    || validator.validator_stake_struct_version.len() > 16
            })
    {
        return Err(NearReceiptLegError::BoundsExceeded);
    }
    match &witness.proof.outcome_proof.outcome.status {
        NearExecutionStatus::SuccessValue(value) if value.len() > MAX_NEAR_PAYLOAD_BYTES => {
            return Err(NearReceiptLegError::BoundsExceeded)
        }
        NearExecutionStatus::SuccessReceiptId(value) if value.len() > 64 => {
            return Err(NearReceiptLegError::BoundsExceeded)
        }
        _ => {}
    }
    Ok(())
}

pub fn near_next_bps_hash_versioned(
    next_bps: &[NearValidatorStakeView],
) -> Result<NearHash, NearReceiptLegError> {
    let mut out = Vec::new();
    put_u32(
        &mut out,
        u32::try_from(next_bps.len()).map_err(|_| NearReceiptLegError::ArithmeticOverflow)?,
    );
    for validator in next_bps {
        if validator.validator_stake_struct_version != "V1" {
            return Err(NearReceiptLegError::BadValidatorSetHash);
        }
        out.push(0);
        put_string(&mut out, &validator.account_id)?;
        out.extend_from_slice(&encode_public_key(&validator.public_key)?);
        put_u128(&mut out, parse_u128(&validator.stake)?);
    }
    Ok(sha256(&out))
}

pub fn verify_approvals_v1_fixture_bps(
    head: &NearBlockHeaderLite,
    next_block_inner_hash: &str,
    approvals_after_next: &[Option<String>],
    current_bps: &[NearValidatorStakeView],
) -> Result<NearApprovalReport, NearReceiptLegError> {
    if approvals_after_next.len() != current_bps.len() {
        return Err(NearReceiptLegError::BadApprovalSignature);
    }
    let head_hash = near_block_hash_from_lite(head)?;
    let next_block_inner_hash = decode_hash(next_block_inner_hash)?;
    let next_block_hash = combine_hash(&next_block_inner_hash, &head_hash);
    let mut message = Vec::with_capacity(41);
    message.push(0);
    message.extend_from_slice(&next_block_hash);
    put_u64(&mut message, head.inner_lite.height + 2);

    let mut approvals_present = 0usize;
    let mut approvals_valid = 0usize;
    let mut total_stake_yocto = 0u128;
    let mut approved_stake_yocto = 0u128;
    for (approval, validator) in approvals_after_next.iter().zip(current_bps) {
        let stake = parse_u128(&validator.stake)?;
        total_stake_yocto = total_stake_yocto
            .checked_add(stake)
            .ok_or(NearReceiptLegError::ArithmeticOverflow)?;
        let Some(approval) = approval else {
            continue;
        };
        approvals_present += 1;
        let signature = decode_prefixed_base58::<64>(approval, "ed25519:")?;
        let public_key = decode_prefixed_base58::<32>(&validator.public_key, "ed25519:")?;
        let key = VerifyingKey::from_bytes(&public_key)
            .map_err(|_| NearReceiptLegError::BadApprovalSignature)?;
        let signature = Signature::from_bytes(&signature);
        if key.verify(&message, &signature).is_ok() {
            approvals_valid += 1;
            approved_stake_yocto = approved_stake_yocto
                .checked_add(stake)
                .ok_or(NearReceiptLegError::ArithmeticOverflow)?;
        }
    }
    let quorum = approved_stake_yocto
        .checked_mul(3)
        .zip(total_stake_yocto.checked_mul(2))
        .is_some_and(|(approved, total)| approved > total);
    Ok(NearApprovalReport {
        approval_message: message,
        approvals_present,
        approvals_valid,
        total_stake_yocto,
        approved_stake_yocto,
        quorum,
    })
}

fn encode_inner_lite(inner: &NearInnerLite) -> Result<Vec<u8>, NearReceiptLegError> {
    let mut out = Vec::with_capacity(8 + 32 * 7 + 8);
    put_u64(&mut out, inner.height);
    out.extend_from_slice(&decode_hash(&inner.epoch_id)?);
    out.extend_from_slice(&decode_hash(&inner.next_epoch_id)?);
    out.extend_from_slice(&decode_hash(&inner.prev_state_root)?);
    out.extend_from_slice(&decode_hash(&inner.outcome_root)?);
    put_u64(&mut out, parse_u64(&inner.timestamp_nanosec)?);
    out.extend_from_slice(&decode_hash(&inner.next_bp_hash)?);
    out.extend_from_slice(&decode_hash(&inner.block_merkle_root)?);
    Ok(out)
}

fn encode_inner_rest_v6(header: &NearFullBlockHeader) -> Result<Vec<u8>, NearReceiptLegError> {
    let mut out = Vec::new();
    out.extend_from_slice(&decode_hash(&header.block_body_hash)?);
    out.extend_from_slice(&decode_hash(&header.chunk_receipts_root)?);
    out.extend_from_slice(&decode_hash(&header.chunk_headers_root)?);
    out.extend_from_slice(&decode_hash(&header.chunk_tx_root)?);
    out.extend_from_slice(&decode_hash(&header.random_value)?);
    encode_validator_stakes(&mut out, &header.validator_proposals)?;
    encode_bool_vec(&mut out, &header.chunk_mask)?;
    put_u128(&mut out, parse_u128(&header.gas_price)?);
    put_u128(&mut out, parse_u128(&header.total_supply)?);
    out.extend_from_slice(&decode_hash(&header.last_final_block)?);
    out.extend_from_slice(&decode_hash(&header.last_ds_final_block)?);
    put_u64(&mut out, header.block_ordinal);
    put_u64(&mut out, header.prev_height);
    encode_option_hash(&mut out, header.epoch_sync_data_hash.as_deref())?;
    encode_approvals(&mut out, &header.approvals)?;
    put_u32(&mut out, header.latest_protocol_version);
    encode_vec_vec_u8(&mut out, &header.chunk_endorsements)?;
    encode_shard_split(&mut out, header.shard_split.as_ref())?;
    Ok(out)
}

fn encode_inner_rest_v5(header: &NearFullBlockHeader) -> Result<Vec<u8>, NearReceiptLegError> {
    if !header.challenges_result.is_empty() {
        return Err(NearReceiptLegError::UnsupportedHeadVersion);
    }
    let mut out = Vec::new();
    out.extend_from_slice(&decode_hash(&header.block_body_hash)?);
    out.extend_from_slice(&decode_hash(&header.chunk_receipts_root)?);
    out.extend_from_slice(&decode_hash(&header.chunk_headers_root)?);
    out.extend_from_slice(&decode_hash(&header.chunk_tx_root)?);
    out.extend_from_slice(&decode_hash(&header.challenges_root)?);
    out.extend_from_slice(&decode_hash(&header.random_value)?);
    encode_validator_stakes(&mut out, &header.validator_proposals)?;
    encode_bool_vec(&mut out, &header.chunk_mask)?;
    put_u128(&mut out, parse_u128(&header.gas_price)?);
    put_u128(&mut out, parse_u128(&header.total_supply)?);
    put_u32(&mut out, 0);
    out.extend_from_slice(&decode_hash(&header.last_final_block)?);
    out.extend_from_slice(&decode_hash(&header.last_ds_final_block)?);
    put_u64(&mut out, header.block_ordinal);
    put_u64(&mut out, header.prev_height);
    encode_option_hash(&mut out, header.epoch_sync_data_hash.as_deref())?;
    encode_approvals(&mut out, &header.approvals)?;
    put_u32(&mut out, header.latest_protocol_version);
    encode_vec_vec_u8(&mut out, &header.chunk_endorsements)?;
    Ok(out)
}

fn encode_partial_outcome(outcome: &NearExecutionOutcome) -> Result<Vec<u8>, NearReceiptLegError> {
    let mut out = Vec::new();
    put_u32(
        &mut out,
        u32::try_from(outcome.receipt_ids.len())
            .map_err(|_| NearReceiptLegError::ArithmeticOverflow)?,
    );
    for receipt_id in &outcome.receipt_ids {
        out.extend_from_slice(&decode_hash(receipt_id)?);
    }
    put_u64(&mut out, outcome.gas_burnt);
    put_u128(&mut out, parse_u128(&outcome.tokens_burnt)?);
    put_string(&mut out, &outcome.executor_id)?;
    encode_execution_status(&mut out, &outcome.status)?;
    Ok(out)
}

fn encode_execution_status(
    out: &mut Vec<u8>,
    status: &NearExecutionStatus,
) -> Result<(), NearReceiptLegError> {
    match status {
        NearExecutionStatus::Unknown => {
            out.push(0);
            Ok(())
        }
        NearExecutionStatus::Failure => {
            out.push(1);
            Ok(())
        }
        NearExecutionStatus::SuccessValue(bytes) => {
            out.push(2);
            put_bytes(out, bytes)?;
            Ok(())
        }
        NearExecutionStatus::SuccessReceiptId(receipt_id) => {
            out.push(3);
            out.extend_from_slice(&decode_hash(receipt_id)?);
            Ok(())
        }
    }
}

fn fold_merkle_path(
    mut acc: NearHash,
    path: &[NearMerklePathItem],
) -> Result<NearHash, NearReceiptLegError> {
    for item in path {
        let sibling = decode_hash(&item.hash)?;
        acc = match item.direction {
            NearMerkleDirection::Left => combine_hash(&sibling, &acc),
            NearMerkleDirection::Right => combine_hash(&acc, &sibling),
        };
    }
    Ok(acc)
}

fn hash_borsh_hash_vec(hashes: &[NearHash]) -> NearHash {
    let mut out = Vec::with_capacity(4 + hashes.len() * 32);
    put_u32(&mut out, hashes.len() as u32);
    for hash in hashes {
        out.extend_from_slice(hash);
    }
    sha256(&out)
}

fn encode_validator_stakes(
    out: &mut Vec<u8>,
    validators: &[NearValidatorStakeView],
) -> Result<(), NearReceiptLegError> {
    put_u32(
        out,
        u32::try_from(validators.len()).map_err(|_| NearReceiptLegError::ArithmeticOverflow)?,
    );
    for validator in validators {
        if validator.validator_stake_struct_version != "V1" {
            return Err(NearReceiptLegError::BadValidatorSetHash);
        }
        out.push(0);
        put_string(out, &validator.account_id)?;
        out.extend_from_slice(&encode_public_key(&validator.public_key)?);
        put_u128(out, parse_u128(&validator.stake)?);
    }
    Ok(())
}

fn encode_public_key(value: &str) -> Result<Vec<u8>, NearReceiptLegError> {
    let public_key = decode_prefixed_base58::<32>(value, "ed25519:")?;
    let mut out = Vec::with_capacity(33);
    out.push(0);
    out.extend_from_slice(&public_key);
    Ok(out)
}

fn encode_bool_vec(out: &mut Vec<u8>, values: &[bool]) -> Result<(), NearReceiptLegError> {
    put_u32(
        out,
        u32::try_from(values.len()).map_err(|_| NearReceiptLegError::ArithmeticOverflow)?,
    );
    for value in values {
        out.push(u8::from(*value));
    }
    Ok(())
}

fn encode_option_hash(out: &mut Vec<u8>, value: Option<&str>) -> Result<(), NearReceiptLegError> {
    match value {
        Some(value) => {
            out.push(1);
            out.extend_from_slice(&decode_hash(value)?);
        }
        None => out.push(0),
    }
    Ok(())
}

fn encode_approvals(
    out: &mut Vec<u8>,
    approvals: &[Option<String>],
) -> Result<(), NearReceiptLegError> {
    put_u32(
        out,
        u32::try_from(approvals.len()).map_err(|_| NearReceiptLegError::ArithmeticOverflow)?,
    );
    for approval in approvals {
        match approval {
            Some(approval) => {
                out.push(1);
                out.extend_from_slice(&encode_signature(approval)?);
            }
            None => out.push(0),
        }
    }
    Ok(())
}

fn encode_signature(value: &str) -> Result<Vec<u8>, NearReceiptLegError> {
    let signature = decode_prefixed_base58::<64>(value, "ed25519:")?;
    let mut out = Vec::with_capacity(65);
    out.push(0);
    out.extend_from_slice(&signature);
    Ok(out)
}

fn encode_vec_vec_u8(out: &mut Vec<u8>, values: &[Vec<u8>]) -> Result<(), NearReceiptLegError> {
    put_u32(
        out,
        u32::try_from(values.len()).map_err(|_| NearReceiptLegError::ArithmeticOverflow)?,
    );
    for value in values {
        put_bytes(out, value)?;
    }
    Ok(())
}

fn encode_shard_split(
    out: &mut Vec<u8>,
    value: Option<&(u64, String)>,
) -> Result<(), NearReceiptLegError> {
    match value {
        Some((shard_id, account_id)) => {
            out.push(1);
            put_u64(out, *shard_id);
            put_string(out, account_id)?;
        }
        None => out.push(0),
    }
    Ok(())
}

fn decode_hash(value: &str) -> Result<NearHash, NearReceiptLegError> {
    if value.is_empty() || value.len() > 64 {
        return Err(NearReceiptLegError::BadBase58Hash);
    }
    let bytes = bs58::decode(value)
        .into_vec()
        .map_err(|_| NearReceiptLegError::BadBase58Hash)?;
    bytes
        .try_into()
        .map_err(|_| NearReceiptLegError::BadBase58Hash)
}

fn decode_prefixed_base58<const N: usize>(
    value: &str,
    prefix: &str,
) -> Result<[u8; N], NearReceiptLegError> {
    let raw = value
        .strip_prefix(prefix)
        .ok_or(NearReceiptLegError::BadBase58Hash)?;
    if raw.is_empty() || raw.len() > N.saturating_mul(2) {
        return Err(NearReceiptLegError::BadBase58Hash);
    }
    let bytes = bs58::decode(raw)
        .into_vec()
        .map_err(|_| NearReceiptLegError::BadBase58Hash)?;
    bytes
        .try_into()
        .map_err(|_| NearReceiptLegError::BadBase58Hash)
}

fn to_base58(hash: &NearHash) -> String {
    bs58::encode(hash).into_string()
}

fn validate_identifier(value: &str) -> Result<(), NearReceiptLegError> {
    if value.is_empty()
        || value.len() > 256
        || !value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || (index > 0 && matches!(byte, b'.' | b'_' | b':' | b'-'))
        })
    {
        return Err(NearReceiptLegError::PolicyMismatch);
    }
    Ok(())
}

fn validate_text(value: &str, max: usize) -> Result<(), NearReceiptLegError> {
    if value.is_empty()
        || value.len() > max
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && !byte.is_ascii_control())
    {
        return Err(NearReceiptLegError::PolicyMismatch);
    }
    Ok(())
}

fn validate_near_account_id(value: &str, max: usize) -> Result<(), NearReceiptLegError> {
    if value.len() < 2
        || value.len() > max
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
    {
        return Err(NearReceiptLegError::PolicyMismatch);
    }
    Ok(())
}

fn validate_implicit_account(
    account_id: &str,
    owner_public_key: &[u8],
) -> Result<(), NearReceiptLegError> {
    if account_id.len() != MAX_NEAR_ACCOUNT_ID_BYTES
        || !account_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || hex::encode(owner_public_key) != account_id
    {
        return Err(NearReceiptLegError::OwnerAuthorization);
    }
    Ok(())
}

fn validate_lower_hex(value: &str, expected_bytes: usize) -> Result<(), NearReceiptLegError> {
    if value.len() != expected_bytes.saturating_mul(2)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(NearReceiptLegError::PolicyMismatch);
    }
    Ok(())
}

fn append_hex(
    out: &mut Vec<u8>,
    value: &str,
    expected_bytes: usize,
) -> Result<(), NearReceiptLegError> {
    validate_lower_hex(value, expected_bytes)?;
    out.extend_from_slice(&hex::decode(value).map_err(|_| NearReceiptLegError::PolicyMismatch)?);
    Ok(())
}

fn append_bytes(out: &mut Vec<u8>, value: &[u8]) -> Result<(), NearReceiptLegError> {
    let length = u32::try_from(value.len()).map_err(|_| NearReceiptLegError::BoundsExceeded)?;
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(value);
    Ok(())
}

fn domain_message(domain: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(domain.len().saturating_add(payload.len()).saturating_add(8));
    out.extend_from_slice(&(domain.len() as u32).to_be_bytes());
    out.extend_from_slice(domain);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    out
}

fn hash48(domain: &[u8], parts: &[&[u8]]) -> String {
    let mut hasher = Sha3_384::new();
    hasher.update((domain.len() as u32).to_be_bytes());
    hasher.update(domain);
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    hex::encode(hasher.finalize())
}

fn parse_u64(value: &str) -> Result<u64, NearReceiptLegError> {
    value
        .parse::<u64>()
        .map_err(|_| NearReceiptLegError::BadPayload)
}

fn parse_u128(value: &str) -> Result<u128, NearReceiptLegError> {
    value
        .parse::<u128>()
        .map_err(|_| NearReceiptLegError::BadPayload)
}

fn combine_hash(left: &NearHash, right: &NearHash) -> NearHash {
    let mut bytes = [0u8; 64];
    bytes[..32].copy_from_slice(left);
    bytes[32..].copy_from_slice(right);
    sha256(&bytes)
}

fn sha256(bytes: &[u8]) -> NearHash {
    Sha256::digest(bytes).into()
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u128(out: &mut Vec<u8>, value: u128) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_string(out: &mut Vec<u8>, value: &str) -> Result<(), NearReceiptLegError> {
    put_bytes(out, value.as_bytes())
}

fn put_bytes(out: &mut Vec<u8>, value: &[u8]) -> Result<(), NearReceiptLegError> {
    put_u32(
        out,
        u32::try_from(value.len()).map_err(|_| NearReceiptLegError::ArithmeticOverflow)?,
    );
    out.extend_from_slice(value);
    Ok(())
}

struct NearPayloadCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> NearPayloadCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn string(&mut self) -> Result<String, NearReceiptLegError> {
        let len = self.u32()? as usize;
        let bytes = self.take(len)?;
        std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| NearReceiptLegError::BadPayload)
    }

    fn u32(&mut self) -> Result<u32, NearReceiptLegError> {
        Ok(u32::from_le_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| NearReceiptLegError::BadPayload)?,
        ))
    }

    fn u64(&mut self) -> Result<u64, NearReceiptLegError> {
        Ok(u64::from_le_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| NearReceiptLegError::BadPayload)?,
        ))
    }

    fn u128(&mut self) -> Result<u128, NearReceiptLegError> {
        Ok(u128::from_le_bytes(
            self.take(16)?
                .try_into()
                .map_err(|_| NearReceiptLegError::BadPayload)?,
        ))
    }

    fn bytes_array<const N: usize>(&mut self) -> Result<[u8; N], NearReceiptLegError> {
        self.take(N)?
            .try_into()
            .map_err(|_| NearReceiptLegError::BadPayload)
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], NearReceiptLegError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(NearReceiptLegError::ArithmeticOverflow)?;
        if end > self.bytes.len() {
            return Err(NearReceiptLegError::BadPayload);
        }
        let out = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(out)
    }

    fn done(&self) -> Result<(), NearReceiptLegError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(NearReceiptLegError::BadPayload)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bft_checkpoint::{
        BftCheckpointCommitteeV1, BftCheckpointValidatorV1, BftSourceCheckpointCertificateV1,
        BftSourceCheckpointV1, BftSourceCheckpointVoteV1,
        BFT_SOURCE_CHECKPOINT_SIGNATURE_CONTEXT_V1,
    };
    use crate::{
        verify_observation_evidence, EvidenceDimensionV1, FreshnessPolicyV1, LiabilityTreatmentV1,
        ReserveProofContextV1, SourceEvidenceV1, SourceManifestEntryV1, SourceObservationV1,
        TrustClassV1,
    };
    use ed25519_dalek::{Signer, SigningKey};
    use postfiat_crypto_provider::{
        ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context_seed, MlDsa65KeyPair,
    };

    const HISTORICAL_WITNESS: &str = include_str!(
        "../../../../../docs/evidence/a666-pfusdc-reserve-demo-20260730/live-run-01/por-preissue/near-receipt-witness.json"
    );

    #[derive(Clone)]
    struct OwnedContext {
        pftl_genesis_hash: String,
        nav_asset_id: String,
        proof_profile_id: String,
        valuation_policy_hash: String,
        source_manifest_hash: String,
        source_id: String,
        source_domain: String,
        asset_or_position_id: String,
        reserve_owner_commitment: String,
        quantity_verifier_commitment: String,
        observed_at_pftl_height: u64,
        expected_evidence_commitment: String,
    }

    impl OwnedContext {
        fn borrow(&self) -> NearReceiptVerifyContextV1<'_> {
            NearReceiptVerifyContextV1 {
                pftl_genesis_hash: &self.pftl_genesis_hash,
                nav_asset_id: &self.nav_asset_id,
                proof_profile_id: &self.proof_profile_id,
                valuation_policy_hash: &self.valuation_policy_hash,
                source_manifest_hash: &self.source_manifest_hash,
                source_id: &self.source_id,
                source_domain: &self.source_domain,
                asset_or_position_id: &self.asset_or_position_id,
                reserve_owner_commitment: &self.reserve_owner_commitment,
                quantity_verifier_commitment: &self.quantity_verifier_commitment,
                observed_at_pftl_height: self.observed_at_pftl_height,
                expected_evidence_commitment: &self.expected_evidence_commitment,
            }
        }
    }

    fn historical_proof_and_head() -> (NearLightClientProof, NearHeadBlock) {
        let value: serde_json::Value = serde_json::from_str(HISTORICAL_WITNESS).unwrap();
        (
            serde_json::from_value(value["proof"].clone()).unwrap(),
            serde_json::from_value(value["head"].clone()).unwrap(),
        )
    }

    fn committee() -> (BftCheckpointCommitteeV1, Vec<MlDsa65KeyPair>) {
        let keys = (0u8..4)
            .map(|index| ml_dsa_65_keygen_from_seed(&[index + 1; 32]))
            .collect::<Vec<_>>();
        (
            BftCheckpointCommitteeV1 {
                epoch: 7,
                quorum: 3,
                validators: keys
                    .iter()
                    .enumerate()
                    .map(|(index, key)| BftCheckpointValidatorV1 {
                        validator_id: format!("validator-{index}"),
                        public_key: key.public_key.clone(),
                    })
                    .collect(),
            },
            keys,
        )
    }

    fn certify(
        committee: BftCheckpointCommitteeV1,
        keys: &[MlDsa65KeyPair],
        checkpoint: BftSourceCheckpointV1,
    ) -> BftSourceCheckpointCertificateV1 {
        let mut certificate = BftSourceCheckpointCertificateV1 {
            committee,
            checkpoint,
            votes: (0..3)
                .map(|index| BftSourceCheckpointVoteV1 {
                    validator_id: format!("validator-{index}"),
                    signature: Vec::new(),
                })
                .collect(),
        };
        for (index, vote) in certificate.votes.iter_mut().enumerate() {
            let statement = certificate
                .checkpoint
                .vote_signing_statement(&vote.validator_id)
                .unwrap();
            vote.signature = ml_dsa_65_sign_with_context_seed(
                &keys[index].private_key,
                &statement,
                BFT_SOURCE_CHECKPOINT_SIGNATURE_CONTEXT_V1,
                &[0x90 + index as u8; 32],
            )
            .unwrap();
        }
        certificate
    }

    fn encode_payload(
        account_id: &str,
        pool_id: &str,
        staked: u128,
        unstaked: u128,
        timestamp: u64,
        salt: [u8; 32],
    ) -> Vec<u8> {
        let mut out = Vec::new();
        put_bytes(&mut out, account_id.as_bytes()).unwrap();
        put_bytes(&mut out, pool_id.as_bytes()).unwrap();
        put_u128(&mut out, staked);
        put_u128(&mut out, unstaked);
        put_u64(&mut out, timestamp);
        out.extend_from_slice(&salt);
        out
    }

    fn fixture() -> (NearReceiptQuantityProofV1, OwnedContext) {
        let owner_key = SigningKey::from_bytes(&[0x2a; 32]);
        let owner_public_key = owner_key.verifying_key().to_bytes();
        let account_id = hex::encode(owner_public_key);
        let source_domain = "near:mainnet".to_string();
        let pool_id = "public-pool.poolv1.near".to_string();
        let policy = NearReceiptPolicyV1 {
            source_domain: source_domain.clone(),
            reader_account_id: "public-reader.near".to_string(),
            reader_code_hash: to_base58(&[0x31; 32]),
            pool_id: pool_id.clone(),
            pool_code_hash: to_base58(&[0x32; 32]),
            snapshot_standard: "postfiat-nav".to_string(),
            snapshot_version: "1.0.0".to_string(),
            snapshot_event: "NearStakeSnapshot".to_string(),
            owner_public_key: owner_public_key.to_vec(),
        };
        let salt = [0x42; 32];
        let timestamp = 1_785_437_354_895_501_665;
        let payload = encode_payload(
            &account_id,
            &pool_id,
            4_800_000_000_000_000_000_000_000_000,
            52_000_000_000_000_000_000_000_000,
            timestamp,
            salt,
        );
        let commitment = to_base58(&sha256(&payload));
        let (mut proof, mut head) = historical_proof_and_head();
        proof.outcome_proof.outcome.executor_id = policy.reader_account_id.clone();
        proof.outcome_proof.outcome.status = NearExecutionStatus::SuccessValue(payload.clone());
        proof.outcome_proof.outcome.logs = vec![format!(
            "EVENT_JSON:{}",
            serde_json::json!({
                "standard": policy.snapshot_standard,
                "version": policy.snapshot_version,
                "event": policy.snapshot_event,
                "data": [{
                    "commitment": commitment,
                    "block_timestamp": timestamp,
                    "payload": BASE64.encode(&payload)
                }]
            })
        )];
        proof.outcome_proof.outcome.receipt_ids.clear();
        proof.outcome_proof.proof.clear();
        proof.outcome_root_proof.clear();
        let outcome_leaf = near_outcome_leaf_hash(&proof.outcome_proof).unwrap();
        proof.block_header_lite.inner_lite.outcome_root = to_base58(&sha256(&outcome_leaf));
        proof.block_header_lite.inner_lite.timestamp = timestamp;
        proof.block_header_lite.inner_lite.timestamp_nanosec = timestamp.to_string();
        proof.block_header_lite.inner_rest_hash = to_base58(&[0x33; 32]);
        proof.block_header_lite.prev_block_hash = to_base58(&[0x34; 32]);
        let proven_hash = near_block_hash_from_lite(&proof.block_header_lite).unwrap();
        proof.outcome_proof.block_hash = to_base58(&proven_hash);
        proof.block_proof.clear();

        head.header.block_merkle_root = to_base58(&proven_hash);
        head.head_block_merkle_root = head.header.block_merkle_root.clone();
        head.header.outcome_root = to_base58(&[0x35; 32]);
        head.header.prev_hash = to_base58(&[0x36; 32]);
        head.head_height = head.header.height;
        let head_hash = near_head_block_hash(&head.header).unwrap();
        head.header.hash = to_base58(&head_hash);
        head.head_hash = head.header.hash.clone();

        let (committee, keys) = committee();
        let committee_root = committee.root().unwrap();
        let source_state_commitment = policy.source_state_commitment(&proven_hash).unwrap();
        let checkpoint = BftSourceCheckpointV1 {
            pftl_genesis_hash: "11".repeat(48),
            checkpoint_kind: NEAR_CHECKPOINT_KIND_V1.to_string(),
            source_domain: source_domain.clone(),
            source_height: head.header.height,
            source_block_hash: B256::from(head_hash),
            source_state_commitment,
            observed_source_head: head.header.height + 3,
            minimum_depth: 3,
            pftl_observation_height: 200,
            committee_epoch: committee.epoch,
            committee_root: committee_root.clone(),
        };
        let certificate = certify(committee, &keys, checkpoint);
        let reserve_owner_commitment = policy.reserve_owner_commitment(&account_id).unwrap();
        let quantity_verifier_commitment = policy.commitment(&committee_root).unwrap();
        let mut witness = NearReceiptQuantityProofV1 {
            policy,
            checkpoint_certificate: certificate,
            account_id,
            ownership_signature: vec![0; 64],
            commitment,
            salt: salt.to_vec(),
            payload,
            proof,
            head,
        };
        let mut context = OwnedContext {
            pftl_genesis_hash: "11".repeat(48),
            nav_asset_id: "22".repeat(48),
            proof_profile_id: "33".repeat(48),
            valuation_policy_hash: "44".repeat(32),
            source_manifest_hash: "55".repeat(48),
            source_id: "near-stake".to_string(),
            source_domain,
            asset_or_position_id: format!("near:stake:{}", witness.policy.pool_id),
            reserve_owner_commitment,
            quantity_verifier_commitment,
            observed_at_pftl_height: 200,
            expected_evidence_commitment: "00".repeat(48),
        };
        let statement = near_owner_authorization_statement_v1(&witness, &context.borrow()).unwrap();
        witness.ownership_signature = owner_key.sign(&statement).to_bytes().to_vec();
        context.expected_evidence_commitment =
            near_receipt_evidence_commitment_v1(&witness).unwrap();
        (witness, context)
    }

    #[test]
    fn historical_artifact_reconstructs_near_roots_and_head_hash() {
        let (proof, head) = historical_proof_and_head();
        assert_eq!(
            to_base58(&near_block_hash_from_lite(&proof.block_header_lite).unwrap()),
            proof.outcome_proof.block_hash
        );
        assert_eq!(
            to_base58(&near_outcome_root(&proof).unwrap()),
            proof.block_header_lite.inner_lite.outcome_root
        );
        assert_eq!(
            to_base58(&near_block_merkle_root_from_proof(&proof).unwrap()),
            head.head_block_merkle_root
        );
        assert_eq!(
            to_base58(&near_head_block_hash(&head.header).unwrap()),
            head.head_hash
        );
    }

    #[test]
    fn verifies_receipt_checkpoint_owner_policy_and_quantity() {
        let (witness, context) = fixture();
        let verified = verify_near_receipt_quantity_proof_v1(&witness, &context.borrow()).unwrap();
        assert_eq!(verified.total_yocto, 4_852_000_000_000_000_000_000_000_000);
        assert_eq!(
            verified.evidence_commitment,
            context.expected_evidence_commitment
        );
        assert_ne!(verified.metadata_hash, B256::ZERO);
    }

    #[test]
    fn registered_guest_dispatch_executes_near_quantity_verifier() {
        let (witness, context) = fixture();
        let proof_context = ReserveProofContextV1 {
            pftl_genesis_hash: context.pftl_genesis_hash.clone(),
            nav_asset_id: context.nav_asset_id.clone(),
            proof_profile_id: context.proof_profile_id.clone(),
            valuation_policy_hash: context.valuation_policy_hash.clone(),
            source_manifest_hash: context.source_manifest_hash.clone(),
            valuation_unit_id: "66".repeat(48),
            valuation_scale: 100_000_000,
            observation_epoch: 1,
            observation_not_before: 199,
            observation_not_after: 200,
        };
        let entry = SourceManifestEntryV1 {
            source_id: context.source_id.clone(),
            adapter_kind: NEAR_RECEIPT_QUANTITY_ADAPTER_KIND_V1.to_string(),
            source_domain: context.source_domain.clone(),
            asset_or_position_id: context.asset_or_position_id.clone(),
            reserve_owner_commitment: context.reserve_owner_commitment.clone(),
            quantity_verifier_commitment: context.quantity_verifier_commitment.clone(),
            valuation_verifier_commitment: "77".repeat(48),
            quantity_evidence_class: TrustClassV1::Cryptographic,
            valuation_evidence_class: TrustClassV1::Controlled,
            freshness_policy: FreshnessPolicyV1 {
                max_age_blocks: 10,
                max_observation_span_blocks: 10,
            },
            haircut_policy_hash: "88".repeat(48),
            liability_treatment: LiabilityTreatmentV1::Asset,
            adapter_schema_version: 1,
        };
        let observation = SourceObservationV1 {
            source_id: context.source_id,
            observed_at_block: context.observed_at_pftl_height,
            gross_assets: 1,
            total_liabilities: 0,
            quantity_evidence: SourceEvidenceV1::NearReceiptQuantity {
                evidence_commitment: context.expected_evidence_commitment,
                proof: Box::new(witness),
            },
            valuation_evidence: SourceEvidenceV1::Controlled {
                evidence_commitment: "99".repeat(48),
            },
            disclosure_commitment: "aa".repeat(48),
        };
        verify_observation_evidence(
            &proof_context,
            &entry,
            &observation,
            EvidenceDimensionV1::Quantity,
        )
        .unwrap();
    }

    #[test]
    fn rejects_proof_policy_owner_evidence_and_bound_substitution() {
        let (witness, context) = fixture();

        let mut tampered = witness.clone();
        tampered.payload[0] ^= 1;
        assert_eq!(
            verify_near_receipt_quantity_proof_v1(&tampered, &context.borrow()).unwrap_err(),
            NearReceiptLegError::BadPayload
        );

        let mut tampered = witness.clone();
        tampered.ownership_signature[0] ^= 1;
        assert_eq!(
            verify_near_receipt_quantity_proof_v1(&tampered, &context.borrow()).unwrap_err(),
            NearReceiptLegError::OwnerAuthorization
        );

        let mut wrong_context = context.clone();
        wrong_context.expected_evidence_commitment = "99".repeat(48);
        assert_eq!(
            verify_near_receipt_quantity_proof_v1(&witness, &wrong_context.borrow()).unwrap_err(),
            NearReceiptLegError::EvidenceCommitment
        );

        let mut tampered = witness.clone();
        tampered.policy.pool_code_hash = to_base58(&[0x77; 32]);
        assert_eq!(
            verify_near_receipt_quantity_proof_v1(&tampered, &context.borrow()).unwrap_err(),
            NearReceiptLegError::PolicyMismatch
        );

        let mut oversized = witness;
        oversized.proof.outcome_proof.outcome.logs = vec![String::new(); MAX_NEAR_LOGS + 1];
        assert_eq!(
            verify_near_receipt_quantity_proof_v1(&oversized, &context.borrow()).unwrap_err(),
            NearReceiptLegError::BoundsExceeded
        );
    }
}
