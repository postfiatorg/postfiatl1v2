//! Hyperliquid reserve verification from a HyperEVM reader receipt included
//! under a governed, quorum-certified block header.

use alloy_primitives::{eip191_hash_message, keccak256, Address, Bytes, Signature, B256, U256};
use alloy_rlp::encode_fixed_size;
use alloy_trie::{nybbles::Nibbles, proof::verify_proof};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_384};

use crate::bft_checkpoint::BftSourceCheckpointCertificateV1;

pub const HL_RECEIPT_EVENT_SIGNATURE: &[u8] = b"HyperCoreSnapshot(bytes32,uint64,bytes)";
pub const HYPERLIQUID_RECEIPT_ADAPTER_KIND_V1: &str = "hyperliquid-hyperevm-receipt-v1";
pub const MAX_HYPERLIQUID_HEADER_BYTES: usize = 8 * 1024;
pub const MAX_HYPERLIQUID_RECEIPT_BYTES: usize = 512 * 1024;
pub const MAX_HYPERLIQUID_RECEIPT_PROOF_NODES: usize = 64;
pub const MAX_HYPERLIQUID_RECEIPT_PROOF_NODE_BYTES: usize = 64 * 1024;
pub const MAX_HYPERLIQUID_RECEIPT_PROOF_BYTES: usize = 1024 * 1024;
pub const MAX_HYPERLIQUID_PERP_ROWS: usize = 64;
pub const MAX_HYPERLIQUID_SPOT_ROWS: usize = 64;
pub const MAX_HYPERLIQUID_ALLOWED_SPOT_TOKENS: usize = 64;
pub const MAX_HYPERLIQUID_RECEIPT_LOGS: usize = 128;
pub const MAX_HYPERLIQUID_LOG_TOPICS: usize = 8;

const POLICY_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_hyperliquid_receipt_policy.v1";
const EVIDENCE_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_hyperliquid_receipt_evidence.v1";
const OWNER_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_hyperliquid_owner_commitment.v1";
const OWNER_AUTHORIZATION_DOMAIN: &[u8] = b"postfiat.reserve_hyperliquid_owner_authorization.v1";
const METADATA_DOMAIN: &[u8] = b"postfiat.reserve_hyperliquid_receipt_metadata.v1";
const SOURCE_STATE_COMMITMENT_DOMAIN: &[u8] =
    b"postfiat.reserve_hyperliquid_source_state_commitment.v1";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HyperliquidSpotTokenPolicyV1 {
    pub token: u64,
    pub wei_decimals: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HyperliquidReceiptPolicyV1 {
    pub source_domain: String,
    pub aggregate_position_id: String,
    pub hyperevm_chain_id: u64,
    pub reader_contract: Address,
    pub reader_code_hash: B256,
    pub required_perps: Vec<u32>,
    pub allowed_spot_tokens: Vec<HyperliquidSpotTokenPolicyV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HyperliquidReceiptProofV1 {
    pub policy: HyperliquidReceiptPolicyV1,
    pub checkpoint_certificate: BftSourceCheckpointCertificateV1,
    pub owner: Address,
    pub ownership_signature: Vec<u8>,
    pub block_header_rlp: Vec<u8>,
    pub receipt_index: u64,
    pub receipt_rlp: Vec<u8>,
    pub receipt_proof_nodes: Vec<Vec<u8>>,
    pub salt: B256,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HyperliquidReceiptVerificationV1 {
    pub block_hash: B256,
    pub receipts_root: B256,
    pub block_time_ms: u64,
    pub commitment: B256,
    pub payload: HlReceiptPayload,
    pub spot_locked_usd_e8: u64,
    pub spot_unlocked_usd_e8: u64,
    pub cash_locked_usd_e8: u64,
    pub cash_unlocked_usd_e8: u64,
    pub perp_notional_usd_e8: u64,
    pub gross_assets_usd_e8: u64,
    pub total_liabilities_usd_e8: u64,
    pub metadata_hash: B256,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HlReceiptPayload {
    pub account: Address,
    pub margin_summary: HlReceiptMarginSummary,
    pub withdrawable_usd_e6: u64,
    pub perps: Vec<HlReceiptPerpSnapshot>,
    pub spots: Vec<HlReceiptSpotSnapshot>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HlReceiptMarginSummary {
    pub account_value: i64,
    pub margin_used: u64,
    pub ntl_pos: u64,
    pub raw_usd: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HlReceiptPerpSnapshot {
    pub perp: u32,
    pub szi: i64,
    pub mark_px: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HlReceiptSpotSnapshot {
    pub token: u64,
    pub total: u64,
    pub hold: u64,
    pub wei_decimals: u8,
    pub price_usd_e8: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HlReceiptLegError {
    BadPinnedBlockHash,
    BadBlockHeader,
    BadReceiptProof,
    BadReceipt,
    MissingSnapshotLog,
    DuplicateSnapshotLog,
    WrongReaderContract,
    WrongEventTopic,
    BadCommitment,
    BadAbi,
    AccountMismatch,
    NegativeAccountValue,
    WithdrawableExceedsAccountValue,
    BadSpotToken,
    BadPerpPosition,
    PerpNotionalMismatch,
    BadSpotBalance,
    BadPrice,
    DuplicateRow,
    ArithmeticOverflow,
    BoundsExceeded,
    CheckpointMismatch,
    PolicyMismatch,
    OwnerAuthorization,
    EvidenceCommitment,
}

impl HyperliquidReceiptPolicyV1 {
    pub fn validate(&self) -> Result<(), HlReceiptLegError> {
        if self.hyperevm_chain_id == 0
            || self.reader_contract == Address::ZERO
            || self.reader_code_hash == B256::ZERO
            || self.source_domain != format!("eip155:{}", self.hyperevm_chain_id)
            || self.source_domain.is_empty()
            || self.source_domain.len() > 256
            || !self.source_domain.bytes().enumerate().all(|(index, byte)| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || (index > 0 && matches!(byte, b'.' | b'_' | b':' | b'-'))
            })
            || self.aggregate_position_id.is_empty()
            || self.aggregate_position_id.len() > 256
            || !self
                .aggregate_position_id
                .bytes()
                .enumerate()
                .all(|(index, byte)| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || (index > 0 && matches!(byte, b'.' | b'_' | b':' | b'-'))
                })
            || self.allowed_spot_tokens.is_empty()
            || self.allowed_spot_tokens.len() > MAX_HYPERLIQUID_ALLOWED_SPOT_TOKENS
            || self.required_perps.len() > MAX_HYPERLIQUID_PERP_ROWS
        {
            return Err(HlReceiptLegError::PolicyMismatch);
        }
        let mut previous_perp = None;
        for perp in &self.required_perps {
            if *perp > u32::from(u16::MAX) || previous_perp >= Some(*perp) {
                return Err(HlReceiptLegError::PolicyMismatch);
            }
            previous_perp = Some(*perp);
        }
        let mut previous = None;
        for token in &self.allowed_spot_tokens {
            if !matches!(token.token, 150 | 404)
                || token.wei_decimals != 8
                || previous >= Some(token.token)
            {
                return Err(HlReceiptLegError::PolicyMismatch);
            }
            previous = Some(token.token);
        }
        Ok(())
    }

    pub fn commitment(&self, committee_root: &str) -> Result<String, HlReceiptLegError> {
        self.validate()?;
        validate_lower_hex(committee_root, 48)?;
        let mut bytes = Vec::new();
        append_bytes(&mut bytes, self.source_domain.as_bytes())?;
        append_bytes(&mut bytes, self.aggregate_position_id.as_bytes())?;
        bytes.extend_from_slice(&self.hyperevm_chain_id.to_be_bytes());
        bytes.extend_from_slice(self.reader_contract.as_slice());
        bytes.extend_from_slice(self.reader_code_hash.as_slice());
        append_hex(&mut bytes, committee_root, 48)?;
        append_u32(&mut bytes, self.required_perps.len())?;
        for perp in &self.required_perps {
            bytes.extend_from_slice(&perp.to_be_bytes());
        }
        append_u32(&mut bytes, self.allowed_spot_tokens.len())?;
        for token in &self.allowed_spot_tokens {
            bytes.extend_from_slice(&token.token.to_be_bytes());
            bytes.push(token.wei_decimals);
        }
        Ok(hash48(POLICY_COMMITMENT_DOMAIN, &[&bytes]))
    }

    fn validate_spot_token(&self, token: u64, wei_decimals: u8) -> Result<(), HlReceiptLegError> {
        match self
            .allowed_spot_tokens
            .binary_search_by_key(&token, |entry| entry.token)
        {
            Ok(index) if self.allowed_spot_tokens[index].wei_decimals == wei_decimals => Ok(()),
            _ => Err(HlReceiptLegError::BadSpotToken),
        }
    }
}

impl HyperliquidReceiptProofV1 {
    pub fn commitment(&self) -> Result<String, HlReceiptLegError> {
        validate_proof_bounds(self)?;
        let checkpoint = &self.checkpoint_certificate.checkpoint;
        let committee_root = self
            .checkpoint_certificate
            .committee
            .root()
            .map_err(|_| HlReceiptLegError::CheckpointMismatch)?;
        let policy_commitment = self.policy.commitment(&committee_root)?;
        let mut bytes = Vec::new();
        append_hex(&mut bytes, &policy_commitment, 48)?;
        append_hex(&mut bytes, &checkpoint.pftl_genesis_hash, 48)?;
        append_bytes(&mut bytes, checkpoint.checkpoint_kind.as_bytes())?;
        append_bytes(&mut bytes, checkpoint.source_domain.as_bytes())?;
        bytes.extend_from_slice(&checkpoint.source_height.to_be_bytes());
        bytes.extend_from_slice(&checkpoint.source_timestamp_ms.to_be_bytes());
        bytes.extend_from_slice(checkpoint.source_block_hash.as_slice());
        bytes.extend_from_slice(checkpoint.source_state_commitment.as_slice());
        bytes.extend_from_slice(&checkpoint.observed_source_head.to_be_bytes());
        bytes.extend_from_slice(&checkpoint.minimum_depth.to_be_bytes());
        bytes.extend_from_slice(&checkpoint.pftl_observation_height.to_be_bytes());
        bytes.extend_from_slice(&checkpoint.committee_epoch.to_be_bytes());
        append_hex(&mut bytes, &checkpoint.committee_root, 48)?;
        append_u32(&mut bytes, self.checkpoint_certificate.votes.len())?;
        for vote in &self.checkpoint_certificate.votes {
            append_bytes(&mut bytes, vote.validator_id.as_bytes())?;
            append_bytes(&mut bytes, &vote.signature)?;
        }
        bytes.extend_from_slice(self.owner.as_slice());
        append_bytes(&mut bytes, &self.ownership_signature)?;
        append_bytes(&mut bytes, &self.block_header_rlp)?;
        bytes.extend_from_slice(&self.receipt_index.to_be_bytes());
        append_bytes(&mut bytes, &self.receipt_rlp)?;
        append_u32(&mut bytes, self.receipt_proof_nodes.len())?;
        for node in &self.receipt_proof_nodes {
            append_bytes(&mut bytes, node)?;
        }
        bytes.extend_from_slice(self.salt.as_slice());
        Ok(hash48(EVIDENCE_COMMITMENT_DOMAIN, &[&bytes]))
    }

    fn verify_owner_authorization(
        &self,
        context: &HyperliquidReceiptVerifyContextV1<'_>,
        committee_root: &str,
    ) -> Result<(), HlReceiptLegError> {
        if self.ownership_signature.len() != 65 {
            return Err(HlReceiptLegError::OwnerAuthorization);
        }
        let statement = hyperliquid_owner_authorization_statement(self, context, committee_root)?;
        let signature: [u8; 65] = self
            .ownership_signature
            .as_slice()
            .try_into()
            .map_err(|_| HlReceiptLegError::OwnerAuthorization)?;
        let signature = Signature::from_raw_array(&signature)
            .map_err(|_| HlReceiptLegError::OwnerAuthorization)?;
        let recovered = signature
            .recover_address_from_prehash(&eip191_hash_message(&statement))
            .map_err(|_| HlReceiptLegError::OwnerAuthorization)?;
        if recovered != self.owner {
            return Err(HlReceiptLegError::OwnerAuthorization);
        }
        Ok(())
    }
}

pub fn hyperliquid_owner_commitment(owner: Address) -> String {
    hash48(OWNER_COMMITMENT_DOMAIN, &[owner.as_slice()])
}

pub fn hyperliquid_source_state_commitment_v1(
    receipts_root: B256,
    reader_contract: Address,
    reader_code_hash: B256,
) -> B256 {
    let mut bytes = Vec::with_capacity(SOURCE_STATE_COMMITMENT_DOMAIN.len() + 4 + 32 + 20 + 32);
    bytes.extend_from_slice(&(SOURCE_STATE_COMMITMENT_DOMAIN.len() as u32).to_be_bytes());
    bytes.extend_from_slice(SOURCE_STATE_COMMITMENT_DOMAIN);
    bytes.extend_from_slice(receipts_root.as_slice());
    bytes.extend_from_slice(reader_contract.as_slice());
    bytes.extend_from_slice(reader_code_hash.as_slice());
    keccak256(bytes)
}

pub fn hyperliquid_owner_authorization_statement(
    proof: &HyperliquidReceiptProofV1,
    context: &HyperliquidReceiptVerifyContextV1<'_>,
    committee_root: &str,
) -> Result<Vec<u8>, HlReceiptLegError> {
    if proof
        .checkpoint_certificate
        .committee
        .root()
        .map_err(|_| HlReceiptLegError::CheckpointMismatch)?
        != committee_root
    {
        return Err(HlReceiptLegError::CheckpointMismatch);
    }
    hyperliquid_owner_authorization_statement_for_policy_v1(
        &proof.policy,
        &proof.checkpoint_certificate,
        proof.owner,
        context,
    )
}

pub fn hyperliquid_owner_authorization_statement_for_policy_v1(
    policy: &HyperliquidReceiptPolicyV1,
    checkpoint_certificate: &BftSourceCheckpointCertificateV1,
    owner: Address,
    context: &HyperliquidReceiptVerifyContextV1<'_>,
) -> Result<Vec<u8>, HlReceiptLegError> {
    let committee_root = checkpoint_certificate
        .committee
        .root()
        .map_err(|_| HlReceiptLegError::CheckpointMismatch)?;
    let policy_commitment = policy.commitment(&committee_root)?;
    let mut statement = Vec::new();
    append_hex(&mut statement, context.pftl_genesis_hash, 48)?;
    append_hex(&mut statement, context.nav_asset_id, 48)?;
    append_hex(&mut statement, context.proof_profile_id, 48)?;
    append_hex(&mut statement, context.valuation_policy_hash, 32)?;
    append_hex(&mut statement, context.source_manifest_hash, 48)?;
    append_bytes(&mut statement, context.source_id.as_bytes())?;
    append_bytes(&mut statement, context.source_domain.as_bytes())?;
    append_bytes(&mut statement, context.asset_or_position_id.as_bytes())?;
    statement.extend_from_slice(owner.as_slice());
    statement.extend_from_slice(policy.reader_contract.as_slice());
    append_hex(&mut statement, &policy_commitment, 48)?;
    statement.extend_from_slice(
        checkpoint_certificate
            .checkpoint
            .source_block_hash
            .as_slice(),
    );
    Ok(domain_message(OWNER_AUTHORIZATION_DOMAIN, &statement))
}

fn validate_proof_bounds(proof: &HyperliquidReceiptProofV1) -> Result<(), HlReceiptLegError> {
    if proof.block_header_rlp.is_empty()
        || proof.block_header_rlp.len() > MAX_HYPERLIQUID_HEADER_BYTES
        || proof.receipt_rlp.is_empty()
        || proof.receipt_rlp.len() > MAX_HYPERLIQUID_RECEIPT_BYTES
        || proof.receipt_proof_nodes.is_empty()
        || proof.receipt_proof_nodes.len() > MAX_HYPERLIQUID_RECEIPT_PROOF_NODES
    {
        return Err(HlReceiptLegError::BoundsExceeded);
    }
    let mut total = 0usize;
    for node in &proof.receipt_proof_nodes {
        if node.is_empty() || node.len() > MAX_HYPERLIQUID_RECEIPT_PROOF_NODE_BYTES {
            return Err(HlReceiptLegError::BoundsExceeded);
        }
        total = total
            .checked_add(node.len())
            .ok_or(HlReceiptLegError::BoundsExceeded)?;
    }
    if total > MAX_HYPERLIQUID_RECEIPT_PROOF_BYTES {
        return Err(HlReceiptLegError::BoundsExceeded);
    }
    proof.policy.validate()
}

pub struct HyperliquidReceiptVerifyContextV1<'a> {
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
    pub valuation_verifier_commitment: &'a str,
    pub observed_at_pftl_height: u64,
    pub expected_gross_assets: u64,
    pub expected_total_liabilities: u64,
    pub expected_evidence_commitment: &'a str,
}

pub fn verify_hyperliquid_receipt_proof_v1(
    proof: &HyperliquidReceiptProofV1,
    context: &HyperliquidReceiptVerifyContextV1<'_>,
) -> Result<HyperliquidReceiptVerificationV1, HlReceiptLegError> {
    validate_proof_bounds(proof)?;
    proof
        .checkpoint_certificate
        .verify()
        .map_err(|_| HlReceiptLegError::CheckpointMismatch)?;
    proof.policy.validate()?;
    let checkpoint = &proof.checkpoint_certificate.checkpoint;
    if checkpoint.pftl_genesis_hash != context.pftl_genesis_hash
        || checkpoint.source_domain != context.source_domain
        || checkpoint.pftl_observation_height != context.observed_at_pftl_height
        || checkpoint.source_domain != proof.policy.source_domain
        || checkpoint.checkpoint_kind != "hyperevm-header"
    {
        return Err(HlReceiptLegError::CheckpointMismatch);
    }
    let canonical_domain = format!("eip155:{}", proof.policy.hyperevm_chain_id);
    if proof.policy.source_domain != canonical_domain {
        return Err(HlReceiptLegError::PolicyMismatch);
    }
    if context.asset_or_position_id != proof.policy.aggregate_position_id
        || hyperliquid_owner_commitment(proof.owner) != context.reserve_owner_commitment
    {
        return Err(HlReceiptLegError::PolicyMismatch);
    }
    let committee_root = proof
        .checkpoint_certificate
        .committee
        .root()
        .map_err(|_| HlReceiptLegError::CheckpointMismatch)?;
    let policy_commitment = proof.policy.commitment(&committee_root)?;
    if policy_commitment != context.quantity_verifier_commitment
        || policy_commitment != context.valuation_verifier_commitment
    {
        return Err(HlReceiptLegError::PolicyMismatch);
    }
    if proof.commitment()? != context.expected_evidence_commitment {
        return Err(HlReceiptLegError::EvidenceCommitment);
    }
    proof.verify_owner_authorization(context, &committee_root)?;

    let block_hash = keccak256(&proof.block_header_rlp);
    if block_hash != checkpoint.source_block_hash {
        return Err(HlReceiptLegError::BadPinnedBlockHash);
    }
    let (receipts_root, block_timestamp_seconds) =
        receipts_root_and_timestamp_from_header(&proof.block_header_rlp)?;
    if checkpoint.source_state_commitment
        != hyperliquid_source_state_commitment_v1(
            receipts_root,
            proof.policy.reader_contract,
            proof.policy.reader_code_hash,
        )
    {
        return Err(HlReceiptLegError::CheckpointMismatch);
    }
    verify_receipt_inclusion(
        receipts_root,
        proof.receipt_index,
        &proof.receipt_rlp,
        &proof.receipt_proof_nodes,
    )?;
    let log = find_snapshot_log(&proof.receipt_rlp, proof.policy.reader_contract)?;
    let (block_time_ms, payload_bytes) = decode_event_data(&log.data)?;
    if block_timestamp_seconds
        .checked_mul(1_000)
        .ok_or(HlReceiptLegError::ArithmeticOverflow)?
        != block_time_ms
        || checkpoint.source_timestamp_ms != block_time_ms
    {
        return Err(HlReceiptLegError::BadBlockHeader);
    }
    let mut commitment_preimage = payload_bytes.clone();
    commitment_preimage.extend_from_slice(proof.salt.as_slice());
    let commitment = keccak256(&commitment_preimage);
    if log.topics.get(1).copied() != Some(commitment) {
        return Err(HlReceiptLegError::BadCommitment);
    }
    let payload = decode_snapshot_payload(&payload_bytes)?;
    if payload.account != proof.owner {
        return Err(HlReceiptLegError::AccountMismatch);
    }
    let values = compute_values(&payload, &proof.policy)?;
    if values.gross_assets_usd_e8 != context.expected_gross_assets
        || values.total_liabilities_usd_e8 != context.expected_total_liabilities
    {
        return Err(HlReceiptLegError::EvidenceCommitment);
    }

    Ok(HyperliquidReceiptVerificationV1 {
        block_hash,
        receipts_root,
        block_time_ms,
        commitment,
        payload,
        spot_locked_usd_e8: values.spot_locked_usd_e8,
        spot_unlocked_usd_e8: values.spot_unlocked_usd_e8,
        cash_locked_usd_e8: values.cash_locked_usd_e8,
        cash_unlocked_usd_e8: values.cash_unlocked_usd_e8,
        perp_notional_usd_e8: values.perp_notional_usd_e8,
        gross_assets_usd_e8: values.gross_assets_usd_e8,
        total_liabilities_usd_e8: values.total_liabilities_usd_e8,
        metadata_hash: hyperliquid_receipt_metadata_hash(proof, receipts_root),
    })
}

pub fn hl_receipt_event_topic0() -> B256 {
    keccak256(HL_RECEIPT_EVENT_SIGNATURE)
}

#[derive(Clone, Copy)]
struct ComputedValuesV1 {
    spot_locked_usd_e8: u64,
    spot_unlocked_usd_e8: u64,
    cash_locked_usd_e8: u64,
    cash_unlocked_usd_e8: u64,
    perp_notional_usd_e8: u64,
    gross_assets_usd_e8: u64,
    total_liabilities_usd_e8: u64,
}

fn compute_values(
    payload: &HlReceiptPayload,
    policy: &HyperliquidReceiptPolicyV1,
) -> Result<ComputedValuesV1, HlReceiptLegError> {
    if payload.margin_summary.account_value < 0 {
        return Err(HlReceiptLegError::NegativeAccountValue);
    }
    let account_value = u64::try_from(payload.margin_summary.account_value)
        .map_err(|_| HlReceiptLegError::ArithmeticOverflow)?;
    if account_value < payload.withdrawable_usd_e6 {
        return Err(HlReceiptLegError::WithdrawableExceedsAccountValue);
    }

    if payload.perps.len() != policy.required_perps.len() {
        return Err(HlReceiptLegError::BadPerpPosition);
    }
    let mut perp_notional = 0u128;
    for (perp, required_perp) in payload.perps.iter().zip(&policy.required_perps) {
        if perp.perp != *required_perp {
            return Err(HlReceiptLegError::BadPerpPosition);
        }
        if perp.szi == 0 {
            continue;
        }
        if perp.mark_px == 0 {
            return Err(HlReceiptLegError::BadPrice);
        }
        let szi_abs = perp
            .szi
            .checked_abs()
            .ok_or(HlReceiptLegError::ArithmeticOverflow)? as u128;
        let row = szi_abs
            .checked_mul(u128::from(perp.mark_px))
            .and_then(|value| value.checked_mul(100))
            .ok_or(HlReceiptLegError::ArithmeticOverflow)?;
        perp_notional = perp_notional
            .checked_add(row)
            .ok_or(HlReceiptLegError::ArithmeticOverflow)?;
    }
    let account_perp_notional = u128::from(payload.margin_summary.ntl_pos)
        .checked_mul(100)
        .ok_or(HlReceiptLegError::ArithmeticOverflow)?;
    if perp_notional != account_perp_notional {
        return Err(HlReceiptLegError::PerpNotionalMismatch);
    }

    let mut seen_spots = Vec::with_capacity(payload.spots.len());
    for spot in &payload.spots {
        if seen_spots.contains(&spot.token) {
            return Err(HlReceiptLegError::DuplicateRow);
        }
        seen_spots.push(spot.token);
    }
    if payload.spots.len() != policy.allowed_spot_tokens.len() {
        return Err(HlReceiptLegError::BadSpotToken);
    }
    let mut spot_locked = 0u128;
    let mut spot_unlocked = 0u128;
    for (spot, governed) in payload.spots.iter().zip(&policy.allowed_spot_tokens) {
        if spot.token != governed.token || spot.wei_decimals != governed.wei_decimals {
            return Err(HlReceiptLegError::BadSpotToken);
        }
        policy.validate_spot_token(spot.token, spot.wei_decimals)?;
        if spot.hold > spot.total {
            return Err(HlReceiptLegError::BadSpotBalance);
        }
        if spot.total == 0 && spot.hold == 0 {
            continue;
        }
        if spot.price_usd_e8 == 0 {
            return Err(HlReceiptLegError::BadPrice);
        }
        let unlocked_units = spot
            .total
            .checked_sub(spot.hold)
            .ok_or(HlReceiptLegError::ArithmeticOverflow)?;
        spot_locked = spot_locked
            .checked_add(token_units_to_usd_e8_floor(
                spot.hold,
                spot.price_usd_e8,
                spot.wei_decimals,
            )?)
            .ok_or(HlReceiptLegError::ArithmeticOverflow)?;
        spot_unlocked = spot_unlocked
            .checked_add(token_units_to_usd_e8_floor(
                unlocked_units,
                spot.price_usd_e8,
                spot.wei_decimals,
            )?)
            .ok_or(HlReceiptLegError::ArithmeticOverflow)?;
    }

    let cash_unlocked = u128::from(payload.withdrawable_usd_e6)
        .checked_mul(100)
        .ok_or(HlReceiptLegError::ArithmeticOverflow)?;
    let cash_locked = u128::from(account_value - payload.withdrawable_usd_e6)
        .checked_mul(100)
        .ok_or(HlReceiptLegError::ArithmeticOverflow)?;

    let gross_assets = spot_locked
        .checked_add(spot_unlocked)
        .and_then(|value| value.checked_add(cash_locked))
        .and_then(|value| value.checked_add(cash_unlocked))
        .ok_or(HlReceiptLegError::ArithmeticOverflow)?;
    Ok(ComputedValuesV1 {
        spot_locked_usd_e8: u64::try_from(spot_locked)
            .map_err(|_| HlReceiptLegError::ArithmeticOverflow)?,
        spot_unlocked_usd_e8: u64::try_from(spot_unlocked)
            .map_err(|_| HlReceiptLegError::ArithmeticOverflow)?,
        cash_locked_usd_e8: u64::try_from(cash_locked)
            .map_err(|_| HlReceiptLegError::ArithmeticOverflow)?,
        cash_unlocked_usd_e8: u64::try_from(cash_unlocked)
            .map_err(|_| HlReceiptLegError::ArithmeticOverflow)?,
        perp_notional_usd_e8: u64::try_from(perp_notional)
            .map_err(|_| HlReceiptLegError::ArithmeticOverflow)?,
        gross_assets_usd_e8: u64::try_from(gross_assets)
            .map_err(|_| HlReceiptLegError::ArithmeticOverflow)?,
        total_liabilities_usd_e8: 0,
    })
}

pub fn hyperliquid_receipt_metadata_hash(
    proof: &HyperliquidReceiptProofV1,
    receipts_root: B256,
) -> B256 {
    let mut out = Vec::new();
    out.extend_from_slice(METADATA_DOMAIN);
    out.extend_from_slice(proof.owner.as_slice());
    out.extend_from_slice(proof.policy.reader_contract.as_slice());
    out.extend_from_slice(
        proof
            .checkpoint_certificate
            .checkpoint
            .source_block_hash
            .as_slice(),
    );
    out.extend_from_slice(receipts_root.as_slice());
    out.extend_from_slice(&proof.receipt_index.to_be_bytes());
    out.extend_from_slice(proof.salt.as_slice());
    out.extend_from_slice(&keccak256(&proof.receipt_rlp)[..]);
    keccak256(out)
}

fn token_units_to_usd_e8_floor(
    amount: u64,
    price_usd_e8: u64,
    decimals: u8,
) -> Result<u128, HlReceiptLegError> {
    let unit = 10u128
        .checked_pow(u32::from(decimals))
        .ok_or(HlReceiptLegError::ArithmeticOverflow)?;
    let value = U256::from(amount)
        .checked_mul(U256::from(price_usd_e8))
        .ok_or(HlReceiptLegError::ArithmeticOverflow)?
        / U256::from(unit);
    u256_to_u128(value)
}

fn verify_receipt_inclusion(
    receipts_root: B256,
    receipt_index: u64,
    receipt_rlp: &[u8],
    proof_nodes: &[Vec<u8>],
) -> Result<(), HlReceiptLegError> {
    let index = usize::try_from(receipt_index).map_err(|_| HlReceiptLegError::BadReceiptProof)?;
    let proof = proof_nodes
        .iter()
        .map(|node| Bytes::copy_from_slice(node))
        .collect::<Vec<_>>();
    verify_proof(
        receipts_root,
        receipt_trie_key(index),
        Some(receipt_rlp.to_vec()),
        proof.iter(),
    )
    .map_err(|_| HlReceiptLegError::BadReceiptProof)
}

fn receipt_trie_key(index: usize) -> Nibbles {
    Nibbles::unpack(encode_fixed_size(&index))
}

fn receipts_root_and_timestamp_from_header(
    header_rlp: &[u8],
) -> Result<(B256, u64), HlReceiptLegError> {
    let header = rlp_item(header_rlp, 0).map_err(|_| HlReceiptLegError::BadBlockHeader)?;
    if !header.list || header.total_len != header_rlp.len() {
        return Err(HlReceiptLegError::BadBlockHeader);
    }
    let fields = rlp_list_items(header.payload).map_err(|_| HlReceiptLegError::BadBlockHeader)?;
    if fields.len() < 15 || fields.len() > 21 {
        return Err(HlReceiptLegError::BadBlockHeader);
    }
    let receipts = fields.get(5).ok_or(HlReceiptLegError::BadBlockHeader)?;
    if receipts.list || receipts.payload.len() != 32 {
        return Err(HlReceiptLegError::BadBlockHeader);
    }
    let timestamp = fields.get(11).ok_or(HlReceiptLegError::BadBlockHeader)?;
    if timestamp.list || timestamp.payload.len() > 8 {
        return Err(HlReceiptLegError::BadBlockHeader);
    }
    let mut timestamp_seconds = 0u64;
    for byte in timestamp.payload {
        timestamp_seconds = timestamp_seconds
            .checked_mul(256)
            .and_then(|value| value.checked_add(u64::from(*byte)))
            .ok_or(HlReceiptLegError::BadBlockHeader)?;
    }
    Ok((B256::from_slice(receipts.payload), timestamp_seconds))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReceiptLog {
    address: Address,
    topics: Vec<B256>,
    data: Vec<u8>,
}

fn find_snapshot_log(
    receipt: &[u8],
    reader_contract: Address,
) -> Result<ReceiptLog, HlReceiptLegError> {
    let logs = decode_receipt_logs(receipt)?;
    let topic0 = hl_receipt_event_topic0();
    let mut wrong_contract = false;
    let mut matching_log = None;
    for log in logs {
        if log.topics.first().copied() != Some(topic0) {
            continue;
        }
        if log.address != reader_contract {
            wrong_contract = true;
            continue;
        }
        if log.topics.len() != 2 {
            return Err(HlReceiptLegError::WrongEventTopic);
        }
        if matching_log.replace(log).is_some() {
            return Err(HlReceiptLegError::DuplicateSnapshotLog);
        }
    }
    if let Some(log) = matching_log {
        Ok(log)
    } else if wrong_contract {
        Err(HlReceiptLegError::WrongReaderContract)
    } else {
        Err(HlReceiptLegError::MissingSnapshotLog)
    }
}

fn decode_receipt_logs(receipt: &[u8]) -> Result<Vec<ReceiptLog>, HlReceiptLegError> {
    if receipt.is_empty() {
        return Err(HlReceiptLegError::BadReceipt);
    }
    let body = if receipt[0] <= 0x7f {
        &receipt[1..]
    } else {
        receipt
    };
    let receipt_item = rlp_item(body, 0).map_err(|_| HlReceiptLegError::BadReceipt)?;
    if !receipt_item.list || receipt_item.total_len != body.len() {
        return Err(HlReceiptLegError::BadReceipt);
    }
    let fields = rlp_list_items(receipt_item.payload).map_err(|_| HlReceiptLegError::BadReceipt)?;
    let logs_item = fields.get(3).ok_or(HlReceiptLegError::BadReceipt)?;
    if !logs_item.list {
        return Err(HlReceiptLegError::BadReceipt);
    }
    let log_items = rlp_list_items(logs_item.payload).map_err(|_| HlReceiptLegError::BadReceipt)?;
    if log_items.len() > MAX_HYPERLIQUID_RECEIPT_LOGS {
        return Err(HlReceiptLegError::BoundsExceeded);
    }
    let mut logs = Vec::with_capacity(log_items.len());
    for log_item in log_items {
        if !log_item.list {
            return Err(HlReceiptLegError::BadReceipt);
        }
        let fields = rlp_list_items(log_item.payload).map_err(|_| HlReceiptLegError::BadReceipt)?;
        if fields.len() != 3 {
            return Err(HlReceiptLegError::BadReceipt);
        }
        if fields[0].list || fields[0].payload.len() != 20 {
            return Err(HlReceiptLegError::BadReceipt);
        }
        if !fields[1].list || fields[2].list {
            return Err(HlReceiptLegError::BadReceipt);
        }
        let topic_items =
            rlp_list_items(fields[1].payload).map_err(|_| HlReceiptLegError::BadReceipt)?;
        if topic_items.len() > MAX_HYPERLIQUID_LOG_TOPICS {
            return Err(HlReceiptLegError::BoundsExceeded);
        }
        let mut topics = Vec::with_capacity(topic_items.len());
        for topic in topic_items {
            if topic.list || topic.payload.len() != 32 {
                return Err(HlReceiptLegError::BadReceipt);
            }
            topics.push(B256::from_slice(topic.payload));
        }
        logs.push(ReceiptLog {
            address: Address::from_slice(fields[0].payload),
            topics,
            data: fields[2].payload.to_vec(),
        });
    }
    Ok(logs)
}

fn decode_event_data(data: &[u8]) -> Result<(u64, Vec<u8>), HlReceiptLegError> {
    require_range(data, 0, 96)?;
    let block_time_ms = read_word_u64(data, 0)?;
    let offset = read_word_usize(data, 32)?;
    if offset != 64 {
        return Err(HlReceiptLegError::BadAbi);
    }
    let len = read_word_usize(data, offset)?;
    let payload_start = offset
        .checked_add(32)
        .ok_or(HlReceiptLegError::ArithmeticOverflow)?;
    require_range(data, payload_start, len)?;
    let padded = len
        .div_ceil(32)
        .checked_mul(32)
        .ok_or(HlReceiptLegError::ArithmeticOverflow)?;
    require_range(data, payload_start, padded)?;
    let payload_end = payload_start
        .checked_add(len)
        .ok_or(HlReceiptLegError::ArithmeticOverflow)?;
    Ok((block_time_ms, data[payload_start..payload_end].to_vec()))
}

fn decode_snapshot_payload(payload: &[u8]) -> Result<HlReceiptPayload, HlReceiptLegError> {
    let head_len = 8 * 32;
    require_range(payload, 0, head_len)?;
    let account = read_word_address(payload, 0)?;
    let margin_summary = HlReceiptMarginSummary {
        account_value: read_word_i64(payload, 32)?,
        margin_used: read_word_u64(payload, 64)?,
        ntl_pos: read_word_u64(payload, 96)?,
        raw_usd: read_word_i64(payload, 128)?,
    };
    let withdrawable_usd_e6 = read_word_u64(payload, 160)?;
    let perps_offset = read_word_usize(payload, 192)?;
    let spots_offset = read_word_usize(payload, 224)?;
    if perps_offset < head_len || spots_offset < head_len {
        return Err(HlReceiptLegError::BadAbi);
    }
    let perps = decode_perp_array(payload, perps_offset)?;
    let spots = decode_spot_array(payload, spots_offset)?;
    Ok(HlReceiptPayload {
        account,
        margin_summary,
        withdrawable_usd_e6,
        perps,
        spots,
    })
}

fn decode_perp_array(
    payload: &[u8],
    offset: usize,
) -> Result<Vec<HlReceiptPerpSnapshot>, HlReceiptLegError> {
    require_range(payload, offset, 32)?;
    let len = read_word_usize(payload, offset)?;
    if len > MAX_HYPERLIQUID_PERP_ROWS {
        return Err(HlReceiptLegError::BoundsExceeded);
    }
    let rows_start = offset
        .checked_add(32)
        .ok_or(HlReceiptLegError::ArithmeticOverflow)?;
    let rows_len = len
        .checked_mul(3 * 32)
        .ok_or(HlReceiptLegError::ArithmeticOverflow)?;
    require_range(payload, rows_start, rows_len)?;
    let mut rows = Vec::with_capacity(len);
    for index in 0..len {
        let base = index
            .checked_mul(3 * 32)
            .and_then(|value| rows_start.checked_add(value))
            .ok_or(HlReceiptLegError::ArithmeticOverflow)?;
        rows.push(HlReceiptPerpSnapshot {
            perp: read_word_u32(payload, base)?,
            szi: read_word_i64(payload, base + 32)?,
            mark_px: read_word_u64(payload, base + 64)?,
        });
    }
    Ok(rows)
}

fn decode_spot_array(
    payload: &[u8],
    offset: usize,
) -> Result<Vec<HlReceiptSpotSnapshot>, HlReceiptLegError> {
    require_range(payload, offset, 32)?;
    let len = read_word_usize(payload, offset)?;
    if len > MAX_HYPERLIQUID_SPOT_ROWS {
        return Err(HlReceiptLegError::BoundsExceeded);
    }
    let rows_start = offset
        .checked_add(32)
        .ok_or(HlReceiptLegError::ArithmeticOverflow)?;
    let rows_len = len
        .checked_mul(5 * 32)
        .ok_or(HlReceiptLegError::ArithmeticOverflow)?;
    require_range(payload, rows_start, rows_len)?;
    let mut rows = Vec::with_capacity(len);
    for index in 0..len {
        let base = index
            .checked_mul(5 * 32)
            .and_then(|value| rows_start.checked_add(value))
            .ok_or(HlReceiptLegError::ArithmeticOverflow)?;
        rows.push(HlReceiptSpotSnapshot {
            token: read_word_u64(payload, base)?,
            total: read_word_u64(payload, base + 32)?,
            hold: read_word_u64(payload, base + 64)?,
            wei_decimals: read_word_u8(payload, base + 96)?,
            price_usd_e8: read_word_u64(payload, base + 128)?,
        });
    }
    Ok(rows)
}

#[derive(Clone, Copy)]
struct RlpItem<'a> {
    list: bool,
    payload: &'a [u8],
    total_len: usize,
}

fn rlp_item(input: &[u8], offset: usize) -> Result<RlpItem<'_>, ()> {
    let first = *input.get(offset).ok_or(())?;
    if first <= 0x7f {
        return Ok(RlpItem {
            list: false,
            payload: &input[offset..offset + 1],
            total_len: 1,
        });
    }
    let (list, len_of_len, len, payload_start) = match first {
        0x80..=0xb7 => (false, 0usize, usize::from(first - 0x80), offset + 1),
        0xb8..=0xbf => {
            let len_of_len = usize::from(first - 0xb7);
            let len = read_be_usize(input, offset + 1, len_of_len)?;
            (false, len_of_len, len, offset + 1 + len_of_len)
        }
        0xc0..=0xf7 => (true, 0usize, usize::from(first - 0xc0), offset + 1),
        _ => {
            let len_of_len = usize::from(first - 0xf7);
            let len = read_be_usize(input, offset + 1, len_of_len)?;
            (true, len_of_len, len, offset + 1 + len_of_len)
        }
    };
    let prefix_len = 1 + len_of_len;
    let total_len = prefix_len.checked_add(len).ok_or(())?;
    let end = payload_start.checked_add(len).ok_or(())?;
    if end > input.len() {
        return Err(());
    }
    Ok(RlpItem {
        list,
        payload: &input[payload_start..end],
        total_len,
    })
}

fn rlp_list_items(payload: &[u8]) -> Result<Vec<RlpItem<'_>>, ()> {
    let mut items = Vec::new();
    let mut offset = 0usize;
    while offset < payload.len() {
        if items.len() >= 4_096 {
            return Err(());
        }
        let item = rlp_item(payload, offset)?;
        offset = offset.checked_add(item.total_len).ok_or(())?;
        items.push(item);
    }
    if offset == payload.len() {
        Ok(items)
    } else {
        Err(())
    }
}

fn read_be_usize(input: &[u8], offset: usize, len: usize) -> Result<usize, ()> {
    if len == 0 || len > std::mem::size_of::<usize>() {
        return Err(());
    }
    let end = offset.checked_add(len).ok_or(())?;
    if end > input.len() {
        return Err(());
    }
    let mut value = 0usize;
    for byte in &input[offset..end] {
        value = value
            .checked_mul(256)
            .and_then(|v| v.checked_add(usize::from(*byte)))
            .ok_or(())?;
    }
    Ok(value)
}

fn require_range(bytes: &[u8], offset: usize, len: usize) -> Result<(), HlReceiptLegError> {
    let end = offset
        .checked_add(len)
        .ok_or(HlReceiptLegError::ArithmeticOverflow)?;
    if end > bytes.len() {
        Err(HlReceiptLegError::BadAbi)
    } else {
        Ok(())
    }
}

fn read_word(bytes: &[u8], offset: usize) -> Result<&[u8; 32], HlReceiptLegError> {
    require_range(bytes, offset, 32)?;
    bytes[offset..offset + 32]
        .try_into()
        .map_err(|_| HlReceiptLegError::BadAbi)
}

fn read_word_address(bytes: &[u8], offset: usize) -> Result<Address, HlReceiptLegError> {
    let word = read_word(bytes, offset)?;
    if !word[..12].iter().all(|byte| *byte == 0) {
        return Err(HlReceiptLegError::BadAbi);
    }
    Ok(Address::from_slice(&word[12..32]))
}

fn read_word_u8(bytes: &[u8], offset: usize) -> Result<u8, HlReceiptLegError> {
    let word = read_word(bytes, offset)?;
    if !word[..31].iter().all(|byte| *byte == 0) {
        return Err(HlReceiptLegError::BadAbi);
    }
    Ok(word[31])
}

fn read_word_u32(bytes: &[u8], offset: usize) -> Result<u32, HlReceiptLegError> {
    let word = read_word(bytes, offset)?;
    if !word[..28].iter().all(|byte| *byte == 0) {
        return Err(HlReceiptLegError::BadAbi);
    }
    Ok(u32::from_be_bytes(
        word[28..32]
            .try_into()
            .map_err(|_| HlReceiptLegError::BadAbi)?,
    ))
}

fn read_word_u64(bytes: &[u8], offset: usize) -> Result<u64, HlReceiptLegError> {
    let word = read_word(bytes, offset)?;
    if !word[..24].iter().all(|byte| *byte == 0) {
        return Err(HlReceiptLegError::BadAbi);
    }
    Ok(u64::from_be_bytes(
        word[24..32]
            .try_into()
            .map_err(|_| HlReceiptLegError::BadAbi)?,
    ))
}

fn read_word_usize(bytes: &[u8], offset: usize) -> Result<usize, HlReceiptLegError> {
    let word = read_word(bytes, offset)?;
    if !word[..16].iter().all(|byte| *byte == 0) {
        return Err(HlReceiptLegError::BadAbi);
    }
    let value = u128::from_be_bytes(
        word[16..32]
            .try_into()
            .map_err(|_| HlReceiptLegError::BadAbi)?,
    );
    usize::try_from(value).map_err(|_| HlReceiptLegError::BadAbi)
}

fn read_word_i64(bytes: &[u8], offset: usize) -> Result<i64, HlReceiptLegError> {
    let word = read_word(bytes, offset)?;
    let negative = word[0] == 0xff;
    let expected = if negative { 0xff } else { 0x00 };
    if !word[..24].iter().all(|byte| *byte == expected) {
        return Err(HlReceiptLegError::BadAbi);
    }
    Ok(i64::from_be_bytes(
        word[24..32]
            .try_into()
            .map_err(|_| HlReceiptLegError::BadAbi)?,
    ))
}

fn u256_to_u128(value: U256) -> Result<u128, HlReceiptLegError> {
    if value > U256::from(u128::MAX) {
        Err(HlReceiptLegError::ArithmeticOverflow)
    } else {
        Ok(value.to::<u128>())
    }
}

fn validate_lower_hex(value: &str, expected_bytes: usize) -> Result<(), HlReceiptLegError> {
    if value.len() != expected_bytes.saturating_mul(2)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(HlReceiptLegError::PolicyMismatch);
    }
    Ok(())
}

fn append_hex(
    out: &mut Vec<u8>,
    value: &str,
    expected_bytes: usize,
) -> Result<(), HlReceiptLegError> {
    validate_lower_hex(value, expected_bytes)?;
    out.extend_from_slice(&hex::decode(value).map_err(|_| HlReceiptLegError::PolicyMismatch)?);
    Ok(())
}

fn append_bytes(out: &mut Vec<u8>, value: &[u8]) -> Result<(), HlReceiptLegError> {
    let length = u32::try_from(value.len()).map_err(|_| HlReceiptLegError::BoundsExceeded)?;
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(value);
    Ok(())
}

fn append_u32(out: &mut Vec<u8>, value: usize) -> Result<(), HlReceiptLegError> {
    let value = u32::try_from(value).map_err(|_| HlReceiptLegError::BoundsExceeded)?;
    out.extend_from_slice(&value.to_be_bytes());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bft_checkpoint::{
        BftCheckpointCommitteeV1, BftCheckpointValidatorV1, BftSourceCheckpointCertificateV1,
        BftSourceCheckpointV1, BftSourceCheckpointVoteV1,
    };
    use crate::{
        verify_observation_evidence, EvidenceDimensionV1, FreshnessPolicyV1, LiabilityTreatmentV1,
        ReserveProofContextV1, SourceEvidenceV1, SourceManifestEntryV1, SourceObservationV1,
        TrustClassV1,
    };
    use alloy_trie::{proof::ProofRetainer, HashBuilder};
    use k256::ecdsa::SigningKey;
    use postfiat_crypto_provider::{
        ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context_seed, MlDsa65KeyPair,
    };

    const HISTORICAL_WITNESS: &str = include_str!(
        "../../../../../benchmarks/nav-reserve-proof-historical/hl-receipt-witness.json"
    );

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct HistoricalReceiptWitness {
        block_header_rlp: Vec<u8>,
        owner: Address,
        pinned_block_hash: B256,
        reader_contract: Address,
        receipt_index: u64,
        receipt_proof_nodes: Vec<Vec<u8>>,
        receipt_rlp: Vec<u8>,
        salt: B256,
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

    fn word_u64(value: u64) -> [u8; 32] {
        let mut word = [0u8; 32];
        word[24..].copy_from_slice(&value.to_be_bytes());
        word
    }

    fn word_i64(value: i64) -> [u8; 32] {
        let mut word = if value < 0 { [0xff; 32] } else { [0u8; 32] };
        word[24..].copy_from_slice(&value.to_be_bytes());
        word
    }

    fn word_address(value: Address) -> [u8; 32] {
        let mut word = [0u8; 32];
        word[12..].copy_from_slice(value.as_slice());
        word
    }

    fn snapshot_payload(owner: Address) -> Vec<u8> {
        let perps_offset = 8 * 32;
        let spots_offset = perps_offset + 32 + 3 * 32;
        let mut payload = Vec::new();
        payload.extend_from_slice(&word_address(owner));
        payload.extend_from_slice(&word_i64(10_000_000));
        payload.extend_from_slice(&word_u64(1_000_000));
        payload.extend_from_slice(&word_u64(100_000_000));
        payload.extend_from_slice(&word_i64(9_000_000));
        payload.extend_from_slice(&word_u64(4_000_000));
        payload.extend_from_slice(&word_u64(perps_offset as u64));
        payload.extend_from_slice(&word_u64(spots_offset as u64));
        payload.extend_from_slice(&word_u64(1));
        payload.extend_from_slice(&word_u64(7));
        payload.extend_from_slice(&word_i64(-2));
        payload.extend_from_slice(&word_u64(50_000_000));
        payload.extend_from_slice(&word_u64(1));
        payload.extend_from_slice(&word_u64(404));
        payload.extend_from_slice(&word_u64(100_000_000));
        payload.extend_from_slice(&word_u64(25_000_000));
        payload.extend_from_slice(&word_u64(8));
        payload.extend_from_slice(&word_u64(10_000_000_000));
        payload
    }

    fn rlp_bytes(value: &[u8]) -> Vec<u8> {
        if value.len() == 1 && value[0] < 0x80 {
            return value.to_vec();
        }
        if value.len() < 56 {
            let mut out = vec![0x80 + value.len() as u8];
            out.extend_from_slice(value);
            return out;
        }
        let length = (value.len() as u64).to_be_bytes();
        let first = length.iter().position(|byte| *byte != 0).unwrap_or(7);
        let encoded = &length[first..];
        let mut out = vec![0xb7 + encoded.len() as u8];
        out.extend_from_slice(encoded);
        out.extend_from_slice(value);
        out
    }

    fn rlp_list(items: &[Vec<u8>]) -> Vec<u8> {
        let payload = items.iter().flatten().copied().collect::<Vec<_>>();
        if payload.len() < 56 {
            let mut out = vec![0xc0 + payload.len() as u8];
            out.extend_from_slice(&payload);
            return out;
        }
        let length = (payload.len() as u64).to_be_bytes();
        let first = length.iter().position(|byte| *byte != 0).unwrap_or(7);
        let encoded = &length[first..];
        let mut out = vec![0xf7 + encoded.len() as u8];
        out.extend_from_slice(encoded);
        out.extend_from_slice(&payload);
        out
    }

    fn rlp_u64(value: u64) -> Vec<u8> {
        if value == 0 {
            return rlp_bytes(&[]);
        }
        let bytes = value.to_be_bytes();
        let first = bytes.iter().position(|byte| *byte != 0).unwrap_or(7);
        rlp_bytes(&bytes[first..])
    }

    fn snapshot_receipt(
        reader: Address,
        owner: Address,
        salt: B256,
        timestamp_seconds: u64,
        snapshot_log_count: usize,
    ) -> Vec<u8> {
        let payload = snapshot_payload(owner);
        let mut commitment_preimage = payload.clone();
        commitment_preimage.extend_from_slice(salt.as_slice());
        let commitment = keccak256(commitment_preimage);
        let mut event_data = Vec::new();
        event_data.extend_from_slice(&word_u64(timestamp_seconds * 1_000));
        event_data.extend_from_slice(&word_u64(64));
        event_data.extend_from_slice(&word_u64(payload.len() as u64));
        event_data.extend_from_slice(&payload);
        while !event_data.len().is_multiple_of(32) {
            event_data.push(0);
        }
        let topics = rlp_list(&[
            rlp_bytes(hl_receipt_event_topic0().as_slice()),
            rlp_bytes(commitment.as_slice()),
        ]);
        let log = rlp_list(&[rlp_bytes(reader.as_slice()), topics, rlp_bytes(&event_data)]);
        let logs = rlp_list(&vec![log; snapshot_log_count]);
        let receipt_body = rlp_list(&[rlp_u64(1), rlp_u64(21_000), rlp_bytes(&[0u8; 256]), logs]);
        let mut receipt = vec![2u8];
        receipt.extend_from_slice(&receipt_body);
        receipt
    }

    fn receipt_and_root(
        reader: Address,
        owner: Address,
        salt: B256,
        timestamp_seconds: u64,
    ) -> (Vec<u8>, B256, Vec<Vec<u8>>) {
        let receipt = snapshot_receipt(reader, owner, salt, timestamp_seconds, 1);

        let path = receipt_trie_key(0);
        let mut builder =
            HashBuilder::default().with_proof_retainer(ProofRetainer::from_iter([path]));
        builder.add_leaf(path, &receipt);
        let root = builder.root();
        let nodes = builder
            .take_proof_nodes()
            .into_nodes_sorted()
            .into_iter()
            .map(|(_, node)| node.to_vec())
            .collect();
        (receipt, root, nodes)
    }

    fn header(receipts_root: B256, timestamp_seconds: u64) -> Vec<u8> {
        rlp_list(&[
            rlp_bytes(&[0x11; 32]),
            rlp_bytes(&[0x12; 32]),
            rlp_bytes(&[0x13; 20]),
            rlp_bytes(&[0x14; 32]),
            rlp_bytes(&[0x15; 32]),
            rlp_bytes(receipts_root.as_slice()),
            rlp_bytes(&[0u8; 256]),
            rlp_u64(1),
            rlp_u64(100),
            rlp_u64(30_000_000),
            rlp_u64(21_000),
            rlp_u64(timestamp_seconds),
            rlp_bytes(&[]),
            rlp_bytes(&[0x16; 32]),
            rlp_bytes(&[0u8; 8]),
        ])
    }

    fn fixture() -> (
        HyperliquidReceiptProofV1,
        HyperliquidReceiptVerifyContextV1<'static>,
    ) {
        let ownership_key = SigningKey::from_bytes((&[0x22; 32]).into()).unwrap();
        let owner = Address::from_private_key(&ownership_key);
        let reader = Address::repeat_byte(0x33);
        let reader_code_hash = B256::repeat_byte(0x66);
        let salt = B256::repeat_byte(0x44);
        let timestamp_seconds = 1_781_366_498;
        let (receipt, receipts_root, proof_nodes) =
            receipt_and_root(reader, owner, salt, timestamp_seconds);
        let header = header(receipts_root, timestamp_seconds);
        let block_hash = keccak256(&header);
        let (committee, keys) = committee();
        let committee_root = committee.root().unwrap();
        let checkpoint = BftSourceCheckpointV1 {
            pftl_genesis_hash: "11".repeat(48),
            checkpoint_kind: "hyperevm-header".to_string(),
            source_domain: "eip155:999".to_string(),
            source_height: 1_000,
            source_timestamp_ms: timestamp_seconds * 1_000,
            source_block_hash: block_hash,
            source_state_commitment: hyperliquid_source_state_commitment_v1(
                receipts_root,
                reader,
                reader_code_hash,
            ),
            observed_source_head: 1_012,
            minimum_depth: 12,
            pftl_observation_height: 200,
            committee_epoch: committee.epoch,
            committee_root: committee_root.clone(),
        };
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
                crate::bft_checkpoint::BFT_SOURCE_CHECKPOINT_SIGNATURE_CONTEXT_V1,
                &[0x80 + index as u8; 32],
            )
            .unwrap();
        }

        let policy = HyperliquidReceiptPolicyV1 {
            source_domain: "eip155:999".to_string(),
            aggregate_position_id: "hyperliquid-test-position".to_string(),
            hyperevm_chain_id: 999,
            reader_contract: reader,
            reader_code_hash,
            required_perps: vec![7],
            allowed_spot_tokens: vec![HyperliquidSpotTokenPolicyV1 {
                token: 404,
                wei_decimals: 8,
            }],
        };
        let verifier_commitment = policy.commitment(&committee_root).unwrap();
        let position = policy.aggregate_position_id.clone();
        let position_static: &'static str = Box::leak(position.into_boxed_str());
        let verifier_static: &'static str = Box::leak(verifier_commitment.into_boxed_str());
        let owner_commitment_static: &'static str =
            Box::leak(hyperliquid_owner_commitment(owner).into_boxed_str());
        let mut proof = HyperliquidReceiptProofV1 {
            policy,
            checkpoint_certificate: certificate,
            owner,
            ownership_signature: vec![0; 65],
            block_header_rlp: header,
            receipt_index: 0,
            receipt_rlp: receipt,
            receipt_proof_nodes: proof_nodes,
            salt,
        };
        let placeholder = HyperliquidReceiptVerifyContextV1 {
            pftl_genesis_hash: "111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111",
            nav_asset_id: "222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222",
            proof_profile_id: "333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333",
            valuation_policy_hash: "4444444444444444444444444444444444444444444444444444444444444444",
            source_manifest_hash: "555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555",
            source_id: "hyperliquid",
            source_domain: "eip155:999",
            asset_or_position_id: position_static,
            reserve_owner_commitment: owner_commitment_static,
            quantity_verifier_commitment: verifier_static,
            valuation_verifier_commitment: verifier_static,
            observed_at_pftl_height: 200,
            expected_gross_assets: 11_000_000_000,
            expected_total_liabilities: 0,
            expected_evidence_commitment: "",
        };
        let statement =
            hyperliquid_owner_authorization_statement(&proof, &placeholder, &committee_root)
                .unwrap();
        let digest = eip191_hash_message(&statement);
        let (signature, recovery_id) = ownership_key
            .sign_prehash_recoverable(digest.as_slice())
            .unwrap();
        proof.ownership_signature = Signature::from((signature, recovery_id))
            .as_bytes()
            .to_vec();
        let evidence = proof.commitment().unwrap();
        let evidence_static: &'static str = Box::leak(evidence.into_boxed_str());
        let context = HyperliquidReceiptVerifyContextV1 {
            expected_evidence_commitment: evidence_static,
            ..placeholder
        };
        (proof, context)
    }

    #[test]
    fn verifies_checkpoint_receipt_owner_policy_and_values() {
        let (proof, context) = fixture();
        let verified = verify_hyperliquid_receipt_proof_v1(&proof, &context).unwrap();
        assert_eq!(verified.spot_locked_usd_e8, 2_500_000_000);
        assert_eq!(verified.spot_unlocked_usd_e8, 7_500_000_000);
        assert_eq!(verified.cash_locked_usd_e8, 600_000_000);
        assert_eq!(verified.cash_unlocked_usd_e8, 400_000_000);
        assert_eq!(verified.gross_assets_usd_e8, 11_000_000_000);
        assert_eq!(verified.total_liabilities_usd_e8, 0);
        assert_eq!(verified.perp_notional_usd_e8, 10_000_000_000);
    }

    #[test]
    fn historical_artifact_reconstructs_receipt_and_values() {
        let witness: HistoricalReceiptWitness = serde_json::from_str(HISTORICAL_WITNESS).unwrap();
        assert_eq!(
            keccak256(&witness.block_header_rlp),
            witness.pinned_block_hash
        );
        let (receipts_root, block_timestamp_seconds) =
            receipts_root_and_timestamp_from_header(&witness.block_header_rlp).unwrap();
        verify_receipt_inclusion(
            receipts_root,
            witness.receipt_index,
            &witness.receipt_rlp,
            &witness.receipt_proof_nodes,
        )
        .unwrap();
        let log = find_snapshot_log(&witness.receipt_rlp, witness.reader_contract).unwrap();
        let (block_time_ms, payload_bytes) = decode_event_data(&log.data).unwrap();
        assert_eq!(block_time_ms, block_timestamp_seconds * 1_000);
        let mut commitment_preimage = payload_bytes.clone();
        commitment_preimage.extend_from_slice(witness.salt.as_slice());
        assert_eq!(log.topics[1], keccak256(commitment_preimage));
        let payload = decode_snapshot_payload(&payload_bytes).unwrap();
        assert_eq!(payload.account, witness.owner);
        let policy = HyperliquidReceiptPolicyV1 {
            source_domain: "eip155:999".to_string(),
            aggregate_position_id: "hyperliquid-test-position".to_string(),
            hyperevm_chain_id: 999,
            reader_contract: witness.reader_contract,
            // The historical witness predates policy-pinned reader code. This
            // test reconstructs the receipt payload only; full successor
            // fixtures must supply and verify the actual governed code hash.
            reader_code_hash: B256::repeat_byte(0x42),
            required_perps: payload.perps.iter().map(|row| row.perp).collect(),
            allowed_spot_tokens: payload
                .spots
                .iter()
                .map(|row| HyperliquidSpotTokenPolicyV1 {
                    token: row.token,
                    wei_decimals: row.wei_decimals,
                })
                .collect(),
        };
        let values = compute_values(&payload, &policy).unwrap();
        assert!(values.gross_assets_usd_e8 > 0);
        assert_eq!(
            values.perp_notional_usd_e8,
            payload.margin_summary.ntl_pos * 100
        );
    }

    #[test]
    fn registered_guest_dispatch_executes_hyperliquid_verifier() {
        let (proof, context) = fixture();
        let proof_context = ReserveProofContextV1 {
            pftl_genesis_hash: context.pftl_genesis_hash.to_string(),
            nav_asset_id: context.nav_asset_id.to_string(),
            proof_profile_id: context.proof_profile_id.to_string(),
            valuation_policy_hash: context.valuation_policy_hash.to_string(),
            source_manifest_hash: context.source_manifest_hash.to_string(),
            valuation_unit_id: "66".repeat(48),
            valuation_scale: 100_000_000,
            observation_epoch: 1,
            observation_not_before: context.observed_at_pftl_height - 1,
            observation_not_after: context.observed_at_pftl_height,
        };
        let entry = SourceManifestEntryV1 {
            source_id: context.source_id.to_string(),
            adapter_kind: HYPERLIQUID_RECEIPT_ADAPTER_KIND_V1.to_string(),
            source_domain: context.source_domain.to_string(),
            asset_or_position_id: context.asset_or_position_id.to_string(),
            reserve_owner_commitment: context.reserve_owner_commitment.to_string(),
            quantity_verifier_commitment: context.quantity_verifier_commitment.to_string(),
            valuation_verifier_commitment: context.valuation_verifier_commitment.to_string(),
            quantity_evidence_class: TrustClassV1::Cryptographic,
            valuation_evidence_class: TrustClassV1::Cryptographic,
            freshness_policy: FreshnessPolicyV1 {
                max_age_blocks: 10,
                max_observation_span_blocks: 10,
            },
            haircut_policy_hash: "77".repeat(48),
            liability_treatment: LiabilityTreatmentV1::Asset,
            adapter_schema_version: 1,
        };
        let evidence = SourceEvidenceV1::HyperliquidReceipt {
            evidence_commitment: context.expected_evidence_commitment.to_string(),
            proof: Box::new(proof),
        };
        let observation = SourceObservationV1 {
            source_id: context.source_id.to_string(),
            observed_at_block: context.observed_at_pftl_height,
            gross_assets: context.expected_gross_assets,
            total_liabilities: context.expected_total_liabilities,
            quantity_evidence: evidence.clone(),
            valuation_evidence: evidence,
            disclosure_commitment: "88".repeat(48),
        };
        verify_observation_evidence(
            &proof_context,
            &entry,
            &observation,
            EvidenceDimensionV1::Quantity,
        )
        .unwrap();
        verify_observation_evidence(
            &proof_context,
            &entry,
            &observation,
            EvidenceDimensionV1::Valuation,
        )
        .unwrap();
    }

    #[test]
    fn rejects_receipt_policy_owner_value_and_bound_substitution() {
        let (mut proof, mut context) = fixture();
        *proof.receipt_rlp.last_mut().unwrap() ^= 1;
        context.expected_evidence_commitment =
            Box::leak(proof.commitment().unwrap().into_boxed_str());
        assert_eq!(
            verify_hyperliquid_receipt_proof_v1(&proof, &context),
            Err(HlReceiptLegError::BadReceiptProof)
        );

        let (mut proof, context) = fixture();
        proof.policy.allowed_spot_tokens[0].wei_decimals = 7;
        assert_eq!(
            verify_hyperliquid_receipt_proof_v1(&proof, &context),
            Err(HlReceiptLegError::PolicyMismatch)
        );

        let (mut proof, mut context) = fixture();
        proof.ownership_signature[0] ^= 1;
        context.expected_evidence_commitment =
            Box::leak(proof.commitment().unwrap().into_boxed_str());
        assert_eq!(
            verify_hyperliquid_receipt_proof_v1(&proof, &context),
            Err(HlReceiptLegError::OwnerAuthorization)
        );

        let (proof, mut context) = fixture();
        context.expected_gross_assets += 1;
        assert_eq!(
            verify_hyperliquid_receipt_proof_v1(&proof, &context),
            Err(HlReceiptLegError::EvidenceCommitment)
        );

        let (mut proof, _) = fixture();
        proof.receipt_proof_nodes = vec![vec![0; MAX_HYPERLIQUID_RECEIPT_PROOF_NODE_BYTES + 1]];
        assert_eq!(proof.commitment(), Err(HlReceiptLegError::BoundsExceeded));
    }

    #[test]
    fn rejects_duplicate_rows_negative_equity_bad_holds_and_overflow() {
        let policy = HyperliquidReceiptPolicyV1 {
            source_domain: "eip155:999".to_string(),
            aggregate_position_id: "hyperliquid-test-position".to_string(),
            hyperevm_chain_id: 999,
            reader_contract: Address::repeat_byte(1),
            reader_code_hash: B256::repeat_byte(3),
            required_perps: vec![7],
            allowed_spot_tokens: vec![HyperliquidSpotTokenPolicyV1 {
                token: 404,
                wei_decimals: 8,
            }],
        };
        let mut payload =
            decode_snapshot_payload(&snapshot_payload(Address::repeat_byte(2))).unwrap();
        payload.perps.clear();
        assert_eq!(
            compute_values(&payload, &policy).map(|_| ()),
            Err(HlReceiptLegError::BadPerpPosition)
        );

        let mut payload =
            decode_snapshot_payload(&snapshot_payload(Address::repeat_byte(2))).unwrap();
        payload.perps[0].perp = 8;
        assert_eq!(
            compute_values(&payload, &policy).map(|_| ()),
            Err(HlReceiptLegError::BadPerpPosition)
        );

        let mut omitted_policy = policy.clone();
        omitted_policy.required_perps.clear();
        let mut omitted_payload =
            decode_snapshot_payload(&snapshot_payload(Address::repeat_byte(2))).unwrap();
        omitted_payload.perps.clear();
        assert_eq!(
            compute_values(&omitted_payload, &omitted_policy).map(|_| ()),
            Err(HlReceiptLegError::PerpNotionalMismatch)
        );

        let mut payload =
            decode_snapshot_payload(&snapshot_payload(Address::repeat_byte(2))).unwrap();
        payload.spots.push(payload.spots[0].clone());
        assert_eq!(
            compute_values(&payload, &policy).map(|_| ()),
            Err(HlReceiptLegError::DuplicateRow)
        );

        let mut payload =
            decode_snapshot_payload(&snapshot_payload(Address::repeat_byte(2))).unwrap();
        payload.spots.clear();
        assert_eq!(
            compute_values(&payload, &policy).map(|_| ()),
            Err(HlReceiptLegError::BadSpotToken)
        );

        let mut payload =
            decode_snapshot_payload(&snapshot_payload(Address::repeat_byte(2))).unwrap();
        payload.margin_summary.account_value = -1;
        assert_eq!(
            compute_values(&payload, &policy).map(|_| ()),
            Err(HlReceiptLegError::NegativeAccountValue)
        );

        let mut payload =
            decode_snapshot_payload(&snapshot_payload(Address::repeat_byte(2))).unwrap();
        payload.spots[0].hold = payload.spots[0].total + 1;
        assert_eq!(
            compute_values(&payload, &policy).map(|_| ()),
            Err(HlReceiptLegError::BadSpotBalance)
        );

        let mut payload =
            decode_snapshot_payload(&snapshot_payload(Address::repeat_byte(2))).unwrap();
        payload.spots[0].total = u64::MAX;
        payload.spots[0].price_usd_e8 = u64::MAX;
        assert_eq!(
            compute_values(&payload, &policy).map(|_| ()),
            Err(HlReceiptLegError::ArithmeticOverflow)
        );
    }

    #[test]
    fn policy_and_checkpoint_commitments_bind_complete_reader_configuration() {
        let (proof, _) = fixture();
        let root = B256::repeat_byte(0x11);
        let base = hyperliquid_source_state_commitment_v1(
            root,
            proof.policy.reader_contract,
            proof.policy.reader_code_hash,
        );
        assert_ne!(
            base,
            hyperliquid_source_state_commitment_v1(
                B256::repeat_byte(0x12),
                proof.policy.reader_contract,
                proof.policy.reader_code_hash,
            )
        );
        assert_ne!(
            base,
            hyperliquid_source_state_commitment_v1(
                root,
                Address::repeat_byte(0x13),
                proof.policy.reader_code_hash,
            )
        );
        assert_ne!(
            base,
            hyperliquid_source_state_commitment_v1(
                root,
                proof.policy.reader_contract,
                B256::repeat_byte(0x14),
            )
        );

        let mut bad_policy = proof.policy.clone();
        bad_policy.required_perps = vec![7, 7];
        assert_eq!(
            bad_policy.validate(),
            Err(HlReceiptLegError::PolicyMismatch)
        );
        let mut bad_policy = proof.policy;
        bad_policy.reader_code_hash = B256::ZERO;
        assert_eq!(
            bad_policy.validate(),
            Err(HlReceiptLegError::PolicyMismatch)
        );

        let (proof, _) = fixture();
        let mut bad_policy = proof.policy.clone();
        bad_policy.required_perps = vec![u32::from(u16::MAX) + 1];
        assert_eq!(
            bad_policy.validate(),
            Err(HlReceiptLegError::PolicyMismatch)
        );
        let mut bad_policy = proof.policy;
        bad_policy.source_domain = "eip155:1".to_string();
        assert_eq!(
            bad_policy.validate(),
            Err(HlReceiptLegError::PolicyMismatch)
        );
    }

    #[test]
    fn duplicate_snapshot_events_fail_closed() {
        let reader = Address::repeat_byte(0x31);
        let receipt = snapshot_receipt(
            reader,
            Address::repeat_byte(0x32),
            B256::repeat_byte(0x33),
            1_781_366_498,
            2,
        );
        assert_eq!(
            find_snapshot_log(&receipt, reader),
            Err(HlReceiptLegError::DuplicateSnapshotLog)
        );
    }
}
