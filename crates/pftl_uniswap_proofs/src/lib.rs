use postfiat_crypto_provider::{bytes_to_hex, hex_to_bytes};
use postfiat_pfusdc_proofs::verify_pftl_finality_segment_v1;
use postfiat_types::{
    pftl_uniswap_consensus_receipt_computed_hash, pftl_uniswap_route_state_hash,
    verify_pftl_uniswap_consensus_receipt_merkle_proof, BlockRecord, PfUsdcEgressFinalityStepV1,
    PftlUniswapConsensusReceipt, PftlUniswapConsensusRouteState, PftlUniswapMintPacketV2,
    PftlUniswapReceiptMerkleProofV1, ValidatorRegistryEntry,
    PFTL_UNISWAP_A666_ISSUE_MULTIPLIER_BPS, PFTL_UNISWAP_A666_REDEEM_MULTIPLIER_BPS,
    PFTL_UNISWAP_EXPORT_STATUS_SOURCE_DEBITED, PFTL_UNISWAP_EXTERNAL_PACKET_SCHEMA_V2,
    PFTL_UNISWAP_ROUTE_SCHEMA_V2, PFTL_UNISWAP_TRUST_CLASS_BFT_CHECKPOINT,
    PFTL_UNISWAP_TRUST_CLASS_TRUSTLESS_FINALITY,
};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};

pub const PFTL_UNISWAP_RECEIPT_PROOF_WITNESS_SCHEMA_V1: &str =
    "postfiat-pftl-uniswap-receipt-proof-witness-v1";
pub const PFTL_UNISWAP_CHECKPOINT_PROOF_WITNESS_SCHEMA_V1: &str =
    "postfiat-pftl-uniswap-checkpoint-proof-witness-v1";
pub const PFTL_UNISWAP_RECEIPT_PROOF_PROGRAM_VERSION_V1: u32 = 1;
pub const MAX_FINALITY_ANCESTRY_STEPS: usize = 64;
pub const MAX_COMMITTEE_MEMBERS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "proof_kind", content = "witness", rename_all = "snake_case")]
pub enum PftlUniswapProofInputV1 {
    Receipt(Box<PftlUniswapReceiptProofWitnessV1>),
    Checkpoint(Box<PftlUniswapCheckpointProofWitnessV1>),
}

/// A proof-only finality segment. It deliberately contains no export receipt,
/// route state, or mint packet, so checkpoint liveness never depends on user
/// traffic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapCheckpointProofWitnessV1 {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub prior_checkpoint_block_id: String,
    pub finality_ancestry: Vec<PfUsdcEgressFinalityStepV1>,
    pub block: BlockRecord,
    pub committee_epoch: u64,
    pub committee: Vec<ValidatorRegistryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapReceiptProofWitnessV1 {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub prior_checkpoint_block_id: String,
    pub finality_ancestry: Vec<PfUsdcEgressFinalityStepV1>,
    pub block: BlockRecord,
    pub committee_epoch: u64,
    pub committee: Vec<ValidatorRegistryEntry>,
    pub receipt: PftlUniswapConsensusReceipt,
    pub receipt_merkle_proof: PftlUniswapReceiptMerkleProofV1,
    pub route_state_after: PftlUniswapConsensusRouteState,
    pub mint_packet: PftlUniswapMintPacketV2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PftlUniswapReceiptPublicValuesV1 {
    pub proof_program_version: u32,
    pub pftl_chain_id_hash: [u8; 32],
    pub pftl_genesis_hash_commitment: [u8; 32],
    pub pftl_protocol_version: u32,
    pub committee_root_commitment: [u8; 32],
    pub committee_transition_commitment: [u8; 32],
    pub finalized_block_commitment: [u8; 32],
    pub finalized_state_root_commitment: [u8; 32],
    pub route_epoch: u64,
    pub policy_hash_commitment: [u8; 32],
    pub route_id_commitment: [u8; 32],
    pub route_trust_class: [u8; 32],
    pub route_config_digest_commitment: [u8; 32],
    pub native_nav_asset_id_commitment: [u8; 32],
    pub settlement_asset_id_commitment: [u8; 32],
    pub pricing_nav_epoch: u64,
    pub pricing_reserve_packet_hash_commitment: [u8; 32],
    pub source_wallet_commitment: [u8; 32],
    pub source_receipt_root_commitment: [u8; 32],
    pub source_receipt_hash_commitment: [u8; 32],
    pub accepted_receipt_code: [u8; 32],
    pub packet_digest: [u8; 32],
    pub destination_chain_id: u64,
    pub controller: [u8; 20],
    pub wrapped_token: [u8; 20],
    pub recipient: [u8; 20],
    pub mint_amount_atoms: u64,
    pub settlement_value_atoms: u64,
    pub packet_nonce: [u8; 32],
    pub deadline: u64,
    pub source_height: u64,
    pub prior_checkpoint_commitment: [u8; 32],
    pub resulting_checkpoint_commitment: [u8; 32],
    pub finalized_height: u64,
    pub proof_nullifier: [u8; 32],
}

impl PftlUniswapReceiptPublicValuesV1 {
    /// Solidity ABI encoding for the all-static `ReceiptPublicValues` tuple.
    pub fn abi_encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(35 * 32);
        push_u256_u64(&mut out, u64::from(self.proof_program_version));
        out.extend_from_slice(&self.pftl_chain_id_hash);
        out.extend_from_slice(&self.pftl_genesis_hash_commitment);
        push_u256_u64(&mut out, u64::from(self.pftl_protocol_version));
        out.extend_from_slice(&self.committee_root_commitment);
        out.extend_from_slice(&self.committee_transition_commitment);
        out.extend_from_slice(&self.finalized_block_commitment);
        out.extend_from_slice(&self.finalized_state_root_commitment);
        push_u256_u64(&mut out, self.route_epoch);
        out.extend_from_slice(&self.policy_hash_commitment);
        out.extend_from_slice(&self.route_id_commitment);
        out.extend_from_slice(&self.route_trust_class);
        out.extend_from_slice(&self.route_config_digest_commitment);
        out.extend_from_slice(&self.native_nav_asset_id_commitment);
        out.extend_from_slice(&self.settlement_asset_id_commitment);
        push_u256_u64(&mut out, self.pricing_nav_epoch);
        out.extend_from_slice(&self.pricing_reserve_packet_hash_commitment);
        out.extend_from_slice(&self.source_wallet_commitment);
        out.extend_from_slice(&self.source_receipt_root_commitment);
        out.extend_from_slice(&self.source_receipt_hash_commitment);
        out.extend_from_slice(&self.accepted_receipt_code);
        out.extend_from_slice(&self.packet_digest);
        push_u256_u64(&mut out, self.destination_chain_id);
        push_address(&mut out, &self.controller);
        push_address(&mut out, &self.wrapped_token);
        push_address(&mut out, &self.recipient);
        push_u256_u64(&mut out, self.mint_amount_atoms);
        push_u256_u64(&mut out, self.settlement_value_atoms);
        out.extend_from_slice(&self.packet_nonce);
        push_u256_u64(&mut out, self.deadline);
        push_u256_u64(&mut out, self.source_height);
        out.extend_from_slice(&self.prior_checkpoint_commitment);
        out.extend_from_slice(&self.resulting_checkpoint_commitment);
        push_u256_u64(&mut out, self.finalized_height);
        out.extend_from_slice(&self.proof_nullifier);
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PftlUniswapCheckpointPublicValuesV1 {
    pub proof_program_version: u32,
    pub pftl_chain_id_hash: [u8; 32],
    pub pftl_genesis_hash_commitment: [u8; 32],
    pub pftl_protocol_version: u32,
    pub prior_checkpoint_commitment: [u8; 32],
    pub resulting_checkpoint_commitment: [u8; 32],
    pub finalized_height: u64,
    pub proof_nullifier: [u8; 32],
}

impl PftlUniswapCheckpointPublicValuesV1 {
    /// Solidity ABI encoding for the all-static `CheckpointPublicValues` tuple.
    pub fn abi_encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 * 32);
        push_u256_u64(&mut out, u64::from(self.proof_program_version));
        out.extend_from_slice(&self.pftl_chain_id_hash);
        out.extend_from_slice(&self.pftl_genesis_hash_commitment);
        push_u256_u64(&mut out, u64::from(self.pftl_protocol_version));
        out.extend_from_slice(&self.prior_checkpoint_commitment);
        out.extend_from_slice(&self.resulting_checkpoint_commitment);
        push_u256_u64(&mut out, self.finalized_height);
        out.extend_from_slice(&self.proof_nullifier);
        out
    }
}

pub fn verify_pftl_uniswap_checkpoint_witness_v1(
    witness: &PftlUniswapCheckpointProofWitnessV1,
) -> Result<PftlUniswapCheckpointPublicValuesV1, String> {
    validate_checkpoint_witness_bounds(witness)?;
    let finalized = verify_pftl_finality_segment_v1(
        &witness.chain_id,
        &witness.genesis_hash,
        witness.protocol_version,
        &witness.prior_checkpoint_block_id,
        &witness.finality_ancestry,
        &witness.block,
        witness.committee_epoch,
        &witness.committee,
    )?;
    let prior_checkpoint_commitment =
        keccak_hex48("prior_checkpoint", &witness.prior_checkpoint_block_id)?;
    let resulting_checkpoint_bytes =
        hex48("resulting_checkpoint", &finalized.committed_block.block_id)?;
    let resulting_checkpoint_commitment: [u8; 32] =
        Keccak256::digest(resulting_checkpoint_bytes).into();
    let mut nullifier_preimage = Vec::with_capacity(32 + 32 + 8);
    nullifier_preimage.extend_from_slice(&prior_checkpoint_commitment);
    nullifier_preimage.extend_from_slice(&resulting_checkpoint_commitment);
    nullifier_preimage.extend_from_slice(&finalized.committed_block.height.to_be_bytes());
    Ok(PftlUniswapCheckpointPublicValuesV1 {
        proof_program_version: PFTL_UNISWAP_RECEIPT_PROOF_PROGRAM_VERSION_V1,
        pftl_chain_id_hash: Keccak256::digest(witness.chain_id.as_bytes()).into(),
        pftl_genesis_hash_commitment: keccak_hex48("genesis_hash", &witness.genesis_hash)?,
        pftl_protocol_version: witness.protocol_version,
        prior_checkpoint_commitment,
        resulting_checkpoint_commitment,
        finalized_height: finalized.committed_block.height,
        proof_nullifier: keccak_domain(
            b"postfiat.pftl_uniswap.checkpoint_proof_nullifier.v1",
            &nullifier_preimage,
        ),
    })
}

pub fn verify_pftl_uniswap_receipt_witness_v1(
    witness: &PftlUniswapReceiptProofWitnessV1,
) -> Result<PftlUniswapReceiptPublicValuesV1, String> {
    validate_witness_bounds(witness)?;
    let finalized = verify_pftl_finality_segment_v1(
        &witness.chain_id,
        &witness.genesis_hash,
        witness.protocol_version,
        &witness.prior_checkpoint_block_id,
        &witness.finality_ancestry,
        &witness.block,
        witness.committee_epoch,
        &witness.committee,
    )?;
    let header = &witness.block.header;
    let receipt_root = header
        .pftl_uniswap_receipt_root
        .as_ref()
        .ok_or_else(|| "finalized block has no PFTL-Uniswap receipt root".to_string())?;
    if finalized.committed_block.pftl_uniswap_receipt_root.as_ref() != Some(receipt_root) {
        return Err("finality does not bind the PFTL-Uniswap receipt root".to_string());
    }

    witness.receipt.validate()?;
    if witness.receipt.receipt_hash
        != pftl_uniswap_consensus_receipt_computed_hash(&witness.receipt)
    {
        return Err("receipt hash does not match its canonical fields".to_string());
    }
    verify_pftl_uniswap_consensus_receipt_merkle_proof(
        receipt_root,
        &witness.receipt.receipt_hash,
        &witness.receipt_merkle_proof,
    )?;
    if witness.receipt.transition != "export_debit"
        || witness.receipt.block_height > header.height
        || witness.receipt.packet_hash.as_deref()
            != Some(witness.mint_packet.source_packet_hash.as_str())
        || witness.receipt.amount_atoms != Some(witness.mint_packet.mint_amount_atoms)
    {
        return Err("receipt is not the exact finalized export debit".to_string());
    }

    let route = &witness.route_state_after;
    route.validate()?;
    if witness.receipt.route_id != route.route_id
        || witness.receipt.state_after_hash != pftl_uniswap_route_state_hash(route)
    {
        return Err("export receipt does not bind the supplied route state".to_string());
    }
    let v2 = route
        .v2
        .as_ref()
        .ok_or_else(|| "export route is not schema v2".to_string())?;
    let policy = &v2.primary_market_policy;
    if v2.route_schema_version != PFTL_UNISWAP_ROUTE_SCHEMA_V2
        || v2.outbound_verification_class != PFTL_UNISWAP_TRUST_CLASS_TRUSTLESS_FINALITY
        || v2.return_verification_class != PFTL_UNISWAP_TRUST_CLASS_BFT_CHECKPOINT
        || policy.issue_multiplier_bps != PFTL_UNISWAP_A666_ISSUE_MULTIPLIER_BPS
        || policy.redeem_multiplier_bps != PFTL_UNISWAP_A666_REDEEM_MULTIPLIER_BPS
    {
        return Err("route does not satisfy a666 v2 trust and pricing policy".to_string());
    }
    let export = route
        .export_packets
        .get(&witness.mint_packet.source_packet_hash)
        .ok_or_else(|| "route state does not contain the export packet".to_string())?;
    let expected_digest = witness.mint_packet.evm_digest()?;
    let policy_commitment = keccak_hex48("policy_hash", &policy.policy_hash)?;
    if export.status != PFTL_UNISWAP_EXPORT_STATUS_SOURCE_DEBITED
        || export.ethereum_packet_schema_version != Some(PFTL_UNISWAP_EXTERNAL_PACKET_SCHEMA_V2)
        || export.route_epoch != Some(v2.route_epoch)
        || export.policy_hash.as_ref() != Some(&policy.policy_hash)
        || export.ethereum_packet_digest.as_deref() != Some(expected_digest.as_str())
        || export.reservation_id.as_deref() != Some(witness.mint_packet.reservation_id.as_str())
        || export.nonce != witness.mint_packet.nonce
        || export.ethereum_recipient != witness.mint_packet.ethereum_recipient
        || export.amount_atoms != witness.mint_packet.mint_amount_atoms
        || export.settlement_value_atoms != Some(witness.mint_packet.settlement_value_atoms)
        || export.source_height != witness.mint_packet_source_height()
        || export.destination_deadline_seconds != witness.mint_packet.deadline_seconds
        || witness.mint_packet.route_config_digest != route.route_config_digest
        || witness.mint_packet.settlement_asset_id != route.settlement_asset_id
        || witness.mint_packet.native_nav_asset_id != route.native_nav_asset_id
        || witness.mint_packet.pricing_reserve_packet_hash != policy.pricing_reserve_packet_hash
        || witness.mint_packet.policy_hash_commitment != bytes_to_hex(&policy_commitment)
        || witness.mint_packet.route_epoch != v2.route_epoch
        || witness.mint_packet.pricing_nav_epoch != policy.pricing_nav_epoch
        || witness.mint_packet.destination_chain_id != route.ethereum_chain_id
        || witness.mint_packet.destination_controller != route.handoff_controller
        || witness.mint_packet.wrapped_token != route.wrapped_navcoin_token
        || witness.mint_packet.source_receipt_hash != witness.receipt.receipt_hash
        || witness.mint_packet.source_receipt_root != *receipt_root
    {
        return Err("mint packet does not exactly match the finalized route export".to_string());
    }
    let packet_digest = hex32("packet_digest", &expected_digest)?;
    let receipt_hash_bytes = hex48("source_receipt_hash", &witness.receipt.receipt_hash)?;
    let checkpoint_bytes = hex48("resulting_checkpoint", &finalized.committed_block.block_id)?;
    let mut nullifier_preimage = Vec::with_capacity(48 + 48 + 32);
    nullifier_preimage.extend_from_slice(&receipt_hash_bytes);
    nullifier_preimage.extend_from_slice(&checkpoint_bytes);
    nullifier_preimage.extend_from_slice(&packet_digest);

    Ok(PftlUniswapReceiptPublicValuesV1 {
        proof_program_version: PFTL_UNISWAP_RECEIPT_PROOF_PROGRAM_VERSION_V1,
        pftl_chain_id_hash: Keccak256::digest(witness.chain_id.as_bytes()).into(),
        pftl_genesis_hash_commitment: keccak_hex48("genesis_hash", &witness.genesis_hash)?,
        pftl_protocol_version: witness.protocol_version,
        committee_root_commitment: keccak_hex48("committee_root", &finalized.committee_root)?,
        committee_transition_commitment: optional_hex48_commitment(
            "committee_transition",
            &finalized.transition_start_root,
        )?,
        finalized_block_commitment: Keccak256::digest(checkpoint_bytes).into(),
        finalized_state_root_commitment: keccak_hex48(
            "finalized_state_root",
            &finalized.committed_block.state_root,
        )?,
        route_epoch: v2.route_epoch,
        policy_hash_commitment: policy_commitment,
        route_id_commitment: Keccak256::digest(route.route_id.as_bytes()).into(),
        route_trust_class: Keccak256::digest(
            PFTL_UNISWAP_TRUST_CLASS_TRUSTLESS_FINALITY.as_bytes(),
        )
        .into(),
        route_config_digest_commitment: keccak_hex48(
            "route_config_digest",
            &route.route_config_digest,
        )?,
        native_nav_asset_id_commitment: keccak_hex48(
            "native_nav_asset_id",
            &route.native_nav_asset_id,
        )?,
        settlement_asset_id_commitment: keccak_hex48(
            "settlement_asset_id",
            &route.settlement_asset_id,
        )?,
        pricing_nav_epoch: policy.pricing_nav_epoch,
        pricing_reserve_packet_hash_commitment: keccak_hex48(
            "pricing_reserve_packet_hash",
            &policy.pricing_reserve_packet_hash,
        )?,
        source_wallet_commitment: Keccak256::digest(export.source_wallet.as_bytes()).into(),
        source_receipt_root_commitment: keccak_hex48("receipt_root", receipt_root)?,
        source_receipt_hash_commitment: keccak_hex48(
            "receipt_hash",
            &witness.receipt.receipt_hash,
        )?,
        accepted_receipt_code: Keccak256::digest(b"export_debit").into(),
        packet_digest,
        destination_chain_id: route.ethereum_chain_id,
        controller: evm_address("controller", &route.handoff_controller)?,
        wrapped_token: evm_address("wrapped_token", &route.wrapped_navcoin_token)?,
        recipient: evm_address("recipient", &export.ethereum_recipient)?,
        mint_amount_atoms: export.amount_atoms,
        settlement_value_atoms: export
            .settlement_value_atoms
            .expect("v2 export settlement value checked above"),
        packet_nonce: hex32("packet_nonce", &export.nonce)?,
        deadline: export.destination_deadline_seconds,
        source_height: export.source_height,
        prior_checkpoint_commitment: keccak_hex48(
            "prior_checkpoint",
            &witness.prior_checkpoint_block_id,
        )?,
        resulting_checkpoint_commitment: Keccak256::digest(checkpoint_bytes).into(),
        finalized_height: finalized.committed_block.height,
        proof_nullifier: keccak_domain(
            b"postfiat.pftl_uniswap.receipt_proof_nullifier.v1",
            &nullifier_preimage,
        ),
    })
}

impl PftlUniswapReceiptProofWitnessV1 {
    fn mint_packet_source_height(&self) -> u64 {
        self.receipt.block_height
    }
}

fn validate_witness_bounds(witness: &PftlUniswapReceiptProofWitnessV1) -> Result<(), String> {
    if witness.schema != PFTL_UNISWAP_RECEIPT_PROOF_WITNESS_SCHEMA_V1 {
        return Err("wrong PFTL-Uniswap receipt witness schema".to_string());
    }
    if witness.chain_id.is_empty()
        || witness.genesis_hash.is_empty()
        || witness.protocol_version == 0
        || witness.prior_checkpoint_block_id.is_empty()
        || witness.committee_epoch == 0
        || witness.committee.is_empty()
        || witness.committee.len() > MAX_COMMITTEE_MEMBERS
        || witness.finality_ancestry.len() > MAX_FINALITY_ANCESTRY_STEPS
    {
        return Err("PFTL-Uniswap receipt witness bounds are invalid".to_string());
    }
    validate_finality_ancestry_bounds(&witness.finality_ancestry)?;
    Ok(())
}

fn validate_checkpoint_witness_bounds(
    witness: &PftlUniswapCheckpointProofWitnessV1,
) -> Result<(), String> {
    if witness.schema != PFTL_UNISWAP_CHECKPOINT_PROOF_WITNESS_SCHEMA_V1 {
        return Err("wrong PFTL-Uniswap checkpoint witness schema".to_string());
    }
    hex48("genesis_hash", &witness.genesis_hash)?;
    hex48(
        "prior_checkpoint_block_id",
        &witness.prior_checkpoint_block_id,
    )?;
    if witness.chain_id.is_empty()
        || witness.protocol_version == 0
        || witness.committee_epoch == 0
        || witness.committee.is_empty()
        || witness.committee.len() > MAX_COMMITTEE_MEMBERS
        || witness.finality_ancestry.len() > MAX_FINALITY_ANCESTRY_STEPS
    {
        return Err("PFTL-Uniswap checkpoint witness bounds are invalid".to_string());
    }
    validate_finality_ancestry_bounds(&witness.finality_ancestry)
}

fn validate_finality_ancestry_bounds(
    finality_ancestry: &[PfUsdcEgressFinalityStepV1],
) -> Result<(), String> {
    for step in finality_ancestry {
        if step.committee_epoch == 0
            || step.committee.is_empty()
            || step.committee.len() > MAX_COMMITTEE_MEMBERS
            || step.next_committee.len() > MAX_COMMITTEE_MEMBERS
            || step.governance_payload_json.len() > 1_048_576
            || (step.next_committee.is_empty() != (step.next_committee_epoch == 0))
            || (step.governance_payload_json.is_empty() && !step.next_committee.is_empty())
        {
            return Err("PFTL-Uniswap finality ancestry bounds are invalid".to_string());
        }
    }
    Ok(())
}

fn keccak_hex48(field: &str, value: &str) -> Result<[u8; 32], String> {
    Ok(Keccak256::digest(hex48(field, value)?).into())
}

fn optional_hex48_commitment(field: &str, value: &str) -> Result<[u8; 32], String> {
    if value.is_empty() {
        Ok(Keccak256::digest([]).into())
    } else {
        keccak_hex48(field, value)
    }
}

fn hex48(field: &str, value: &str) -> Result<[u8; 48], String> {
    let bytes = hex_to_bytes(value).map_err(|error| format!("{field}: {error}"))?;
    bytes
        .try_into()
        .map_err(|_| format!("{field} must encode exactly 48 bytes"))
}

fn hex32(field: &str, value: &str) -> Result<[u8; 32], String> {
    let bytes = hex_to_bytes(value).map_err(|error| format!("{field}: {error}"))?;
    bytes
        .try_into()
        .map_err(|_| format!("{field} must encode exactly 32 bytes"))
}

fn evm_address(field: &str, value: &str) -> Result<[u8; 20], String> {
    let raw = value
        .strip_prefix("0x")
        .ok_or_else(|| format!("{field} must be 0x-prefixed"))?;
    let bytes = hex_to_bytes(raw).map_err(|error| format!("{field}: {error}"))?;
    bytes
        .try_into()
        .map_err(|_| format!("{field} must encode exactly 20 bytes"))
}

fn keccak_domain(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(domain);
    hasher.update([0_u8]);
    hasher.update(bytes);
    hasher.finalize().into()
}

fn push_u256_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&[0_u8; 24]);
    out.extend_from_slice(&value.to_be_bytes());
}

fn push_address(out: &mut Vec<u8>, value: &[u8; 20]) {
    out.extend_from_slice(&[0_u8; 12]);
    out.extend_from_slice(value);
}

#[cfg(test)]
mod tests;
