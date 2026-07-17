use std::collections::BTreeSet;

use postfiat_crypto_provider::ML_DSA_65_ALGORITHM;
use postfiat_ordering_fast::{
    consensus_v2_domain, verify_consensus_v2_commit, ConsensusV2QcGraph, ConsensusV2Validator,
    ConsensusV2ValidatorSet,
};
use postfiat_types::{
    pfusdc_egress_proof_nullifier_v1, vault_bridge_withdrawal_packet_evm_digest,
    vault_bridge_withdrawal_packet_hash, verify_bridge_exit_merkle_proof_v1,
    PfUsdcEgressProofWitnessV1, PfUsdcEgressPublicValuesV1, BRIDGE_EXIT_ACCEPTED_RECEIPT_CODE,
    PFUSDC_EGRESS_PROOF_WITNESS_SCHEMA_V1, PFUSDC_EGRESS_PUBLIC_VALUES_SCHEMA_V1,
};

pub const PFUSDC_EGRESS_PROOF_PROGRAM_VERSION_V1: u32 = 1;

pub fn verify_egress_witness_v1(
    witness: &PfUsdcEgressProofWitnessV1,
) -> Result<PfUsdcEgressPublicValuesV1, String> {
    witness.validate_bounds()?;
    if witness.schema != PFUSDC_EGRESS_PROOF_WITNESS_SCHEMA_V1 {
        return Err("wrong pfUSDC egress witness schema".to_string());
    }
    let header = &witness.block.header;
    if header.height < witness.bridge_exit_root_activation_height {
        return Err("withdrawal block predates bridge-exit-root activation".to_string());
    }
    let bridge_exit_root = header
        .bridge_exit_root
        .as_ref()
        .ok_or_else(|| "withdrawal block has no bridge-exit root".to_string())?;
    let commit = header
        .consensus_v2_commit
        .as_ref()
        .ok_or_else(|| "withdrawal block has no consensus-v2 commit".to_string())?;

    let mut validator_ids = BTreeSet::new();
    let validators = witness
        .committee
        .iter()
        .map(|record| {
            if !record.active
                || record.algorithm_id != ML_DSA_65_ALGORITHM
                || !validator_ids.insert(record.node_id.as_str())
            {
                return Err(
                    "egress witness committee is inactive, non-ML-DSA, or duplicate".to_string(),
                );
            }
            Ok(ConsensusV2Validator {
                validator_id: record.node_id.clone(),
                public_key_hex: record.public_key_hex.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let validator_set = ConsensusV2ValidatorSet::try_new(validators)
        .map_err(|error| format!("invalid egress witness committee: {error}"))?;
    let domain = consensus_v2_domain(
        witness.chain_id.clone(),
        witness.genesis_hash.clone(),
        witness.protocol_version,
        witness.committee_epoch,
        &validator_set,
    );
    let committed_block = verify_consensus_v2_commit(
        &domain,
        &validator_set,
        commit,
        &ConsensusV2QcGraph::default(),
    )
    .map_err(|error| format!("invalid consensus-v2 finality: {error}"))?;
    if committed_block.height != header.height
        || committed_block.parent_block_id != header.parent_hash
        || committed_block.state_root != header.state_root
        || committed_block.bridge_exit_root.as_ref() != Some(bridge_exit_root)
        || committed_block.block_id != commit.proposal.block.block_id
        || commit.proposal.round.height != header.height
        || commit.proposal.round.view != header.view
    {
        return Err(
            "consensus-v2 commit does not exactly bind withdrawal block header".to_string(),
        );
    }
    if witness.prior_checkpoint_block_id != committed_block.parent_block_id {
        return Err("egress v1 proof must start at the finalized parent checkpoint".to_string());
    }

    if !witness.receipt.accepted
        || witness.receipt.code != BRIDGE_EXIT_ACCEPTED_RECEIPT_CODE
        || witness.receipt.tx_id != witness.withdrawal_packet.burn_tx_id
        || !witness
            .block
            .receipt_ids
            .iter()
            .any(|receipt_id| receipt_id == &witness.receipt.tx_id)
    {
        return Err(
            "egress witness does not contain the literal accepted burn receipt".to_string(),
        );
    }
    verify_bridge_exit_merkle_proof_v1(bridge_exit_root, &witness.merkle_proof)?;
    let leaf = &witness.merkle_proof.leaf;
    let packet = &witness.withdrawal_packet;
    if leaf.accepted_receipt_id != witness.receipt.tx_id
        || leaf.accepted_receipt_code != BRIDGE_EXIT_ACCEPTED_RECEIPT_CODE
        || leaf.asset_id != packet.vault_bridge_asset_id
        || leaf.burn_tx_id != packet.burn_tx_id
        || leaf.withdrawal_id != packet.withdrawal_id
        || leaf.source_bucket_id != packet.source_bucket_id
        || leaf.amount_atoms != packet.amount_atoms
        || leaf.recipient != packet.recipient
        || leaf.destination_hash != packet.destination_hash
        || leaf.evidence_root != packet.evidence_root
        || leaf.finalized_height != packet.finalized_height
    {
        return Err("bridge-exit leaf does not exactly match withdrawal packet".to_string());
    }
    let packet_hash = vault_bridge_withdrawal_packet_hash(packet)?;
    let packet_digest = vault_bridge_withdrawal_packet_evm_digest(packet)?;
    if packet_hash != witness.withdrawal_packet_hash
        || packet_digest != witness.withdrawal_packet_evm_digest
        || leaf.withdrawal_packet_hash != packet_hash
        || leaf.withdrawal_packet_evm_digest != packet_digest
    {
        return Err("withdrawal packet hash/digest mismatch".to_string());
    }

    witness.route_profile.validate()?;
    let route = &witness.route_profile.profile;
    if witness.route_profile.profile_hash != route.profile_hash()?
        || route.asset_id != packet.vault_bridge_asset_id
        || route.source_chain_id != packet.source_chain_id
        || route.vault_address != packet.vault_address
        || route.token_address != packet.token_address
        || route.activation_height > header.height
        || route.expires_at_height <= header.height
    {
        return Err("withdrawal packet does not match its active route profile".to_string());
    }
    let vault_code_hash = route
        .vault_runtime_code_hash
        .strip_prefix("0x")
        .unwrap_or(&route.vault_runtime_code_hash)
        .to_string();
    let token_code_hash = route
        .token_runtime_code_hash
        .strip_prefix("0x")
        .unwrap_or(&route.token_runtime_code_hash)
        .to_string();
    let exit_leaf_commitment = leaf.commitment()?;
    let proof_nullifier = pfusdc_egress_proof_nullifier_v1(
        u64::from(route.route_epoch),
        &packet.burn_tx_id,
        &packet.withdrawal_id,
        &committed_block.block_id,
    )?;
    let mut public_values = PfUsdcEgressPublicValuesV1 {
        schema: PFUSDC_EGRESS_PUBLIC_VALUES_SCHEMA_V1.to_string(),
        proof_program_version: PFUSDC_EGRESS_PROOF_PROGRAM_VERSION_V1,
        pftl_chain_id: witness.chain_id.clone(),
        pftl_genesis_hash: witness.genesis_hash.clone(),
        pftl_protocol_version: witness.protocol_version,
        route_profile_hash: witness.route_profile.profile_hash.clone(),
        route_epoch: u64::from(route.route_epoch),
        prior_checkpoint_block_id: witness.prior_checkpoint_block_id.clone(),
        resulting_checkpoint_block_id: committed_block.block_id.clone(),
        committee_epoch: witness.committee_epoch,
        committee_root: validator_set.committee_root.clone(),
        committee_transition_commitment: String::new(),
        finalized_block_height: committed_block.height,
        finalized_block_view: commit.proposal.round.view,
        finalized_block_id: committed_block.block_id.clone(),
        finalized_parent_block_id: committed_block.parent_block_id.clone(),
        finalized_state_root: committed_block.state_root.clone(),
        bridge_exit_root: bridge_exit_root.clone(),
        exit_leaf_index: witness.merkle_proof.leaf_index,
        exit_leaf_commitment,
        accepted_receipt_id: witness.receipt.tx_id.clone(),
        accepted_receipt_code: witness.receipt.code.clone(),
        asset_id: packet.vault_bridge_asset_id.clone(),
        burn_tx_id: packet.burn_tx_id.clone(),
        withdrawal_id: packet.withdrawal_id.clone(),
        source_bucket_id: packet.source_bucket_id.clone(),
        amount_atoms: packet.amount_atoms,
        recipient: packet.recipient.clone(),
        destination_hash: packet.destination_hash.clone(),
        evidence_root: packet.evidence_root.clone(),
        withdrawal_finalized_height: packet.finalized_height,
        arbitrum_chain_id: packet.source_chain_id,
        vault_address: packet.vault_address.clone(),
        vault_runtime_code_hash: vault_code_hash,
        token_address: packet.token_address.clone(),
        token_runtime_code_hash: token_code_hash,
        withdrawal_packet_digest: packet_digest,
        withdrawal_packet_hash: packet_hash,
        proof_nullifier,
        public_values_commitment: String::new(),
    };
    public_values.seal()?;
    public_values.validate()?;
    Ok(public_values)
}
