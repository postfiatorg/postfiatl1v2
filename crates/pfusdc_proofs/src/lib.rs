use std::collections::BTreeSet;

use postfiat_consensus_cobalt::{verify_validator_registry_update, CobaltDomain};
use postfiat_crypto_provider::{
    hash_hex, hex_to_bytes, ml_dsa_65_verify_with_context, ML_DSA_65_ALGORITHM,
};
use postfiat_ordering_fast::{
    consensus_v2_commit_from_precommit_qc, consensus_v2_domain, verify_consensus_v2_commit,
    ConsensusV2QcGraph, ConsensusV2Validator, ConsensusV2ValidatorSet,
};
use postfiat_types::{
    pfusdc_egress_proof_nullifier_v1, vault_bridge_withdrawal_packet_evm_digest,
    vault_bridge_withdrawal_packet_hash, verify_bridge_exit_merkle_proof_v1, ConsensusV2BlockRef,
    GovernanceActionBatch, PfUsdcCheckpointProofWitnessV1, PfUsdcCheckpointPublicValuesV1,
    PfUsdcEgressFinalityStepV1, PfUsdcEgressProofWitnessV1, PfUsdcEgressPublicValuesV1,
    SignedGovernanceAuthorizationV2, ValidatorRegistryEntry, ValidatorRegistryUpdateRecord,
    BRIDGE_EXIT_ACCEPTED_RECEIPT_CODE, CONSENSUS_V2_COMMIT_SCHEMA,
    PFUSDC_CHECKPOINT_PROOF_WITNESS_SCHEMA_V1, PFUSDC_CHECKPOINT_PUBLIC_VALUES_SCHEMA_V1,
    PFUSDC_EGRESS_PROOF_WITNESS_SCHEMA_V1, PFUSDC_EGRESS_PUBLIC_VALUES_SCHEMA_V1,
    SIGNED_GOVERNANCE_AUTHORIZATION_SCHEMA_V2,
};

pub const PFUSDC_EGRESS_PROOF_PROGRAM_VERSION_V1: u32 = 1;
const GOVERNANCE_AUTHORIZATION_SIGNATURE_CONTEXT_V2: &[u8] =
    b"postfiat-l1-v2/governance-authorization/v2";

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
    let segment = verify_finality_segment(
        &witness.chain_id,
        &witness.genesis_hash,
        witness.protocol_version,
        &witness.prior_checkpoint_block_id,
        &witness.finality_ancestry,
        &witness.block,
        witness.committee_epoch,
        &witness.committee,
    )?;
    let committed_block = segment.committed_block;
    let validator_set = segment.validator_set;

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
        // When a transition is proved this carries the starting committee
        // root. The EVM verifier binds it to its stored trust anchor; the SP1
        // proof binds every transition from that root to `committee_root`.
        committee_transition_commitment: segment.transition_start_root,
        finalized_block_height: committed_block.height,
        finalized_block_view: header.view,
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

pub fn verify_checkpoint_witness_v1(
    witness: &PfUsdcCheckpointProofWitnessV1,
) -> Result<PfUsdcCheckpointPublicValuesV1, String> {
    witness.validate_bounds()?;
    if witness.schema != PFUSDC_CHECKPOINT_PROOF_WITNESS_SCHEMA_V1 {
        return Err("wrong pfUSDC checkpoint witness schema".to_string());
    }
    let segment = verify_finality_segment(
        &witness.chain_id,
        &witness.genesis_hash,
        witness.protocol_version,
        &witness.prior_checkpoint_block_id,
        &witness.finality_ancestry,
        &witness.block,
        witness.committee_epoch,
        &witness.committee,
    )?;
    let committed = segment.committed_block;
    let mut values = PfUsdcCheckpointPublicValuesV1 {
        schema: PFUSDC_CHECKPOINT_PUBLIC_VALUES_SCHEMA_V1.to_string(),
        proof_program_version: PFUSDC_EGRESS_PROOF_PROGRAM_VERSION_V1,
        pftl_chain_id: witness.chain_id.clone(),
        pftl_genesis_hash: witness.genesis_hash.clone(),
        pftl_protocol_version: witness.protocol_version,
        prior_checkpoint_block_id: witness.prior_checkpoint_block_id.clone(),
        resulting_checkpoint_block_id: committed.block_id.clone(),
        committee_epoch: witness.committee_epoch,
        committee_root: segment.validator_set.committee_root,
        committee_transition_commitment: segment.transition_start_root,
        finalized_block_height: committed.height,
        finalized_block_view: witness.block.header.view,
        finalized_block_id: committed.block_id,
        finalized_parent_block_id: committed.parent_block_id,
        finalized_state_root: committed.state_root,
        public_values_commitment: String::new(),
    };
    values.seal()?;
    values.validate()?;
    Ok(values)
}

struct VerifiedFinalitySegment {
    committed_block: ConsensusV2BlockRef,
    validator_set: ConsensusV2ValidatorSet,
    transition_start_root: String,
}

#[allow(clippy::too_many_arguments)]
fn verify_finality_segment(
    chain_id: &str,
    genesis_hash: &str,
    protocol_version: u32,
    prior_checkpoint_block_id: &str,
    ancestry: &[PfUsdcEgressFinalityStepV1],
    target_block: &postfiat_types::BlockRecord,
    target_committee_epoch: u64,
    target_committee: &[ValidatorRegistryEntry],
) -> Result<VerifiedFinalitySegment, String> {
    let mut cursor = prior_checkpoint_block_id.to_string();
    let mut expected_committee: Option<(
        u64,
        Vec<ValidatorRegistryEntry>,
        ConsensusV2ValidatorSet,
    )> = None;
    let mut transition_start_root = String::new();
    for step in ancestry {
        let step_committee =
            if let Some((expected_epoch, expected_records, expected_set)) = &expected_committee {
                if step.committee_epoch != *expected_epoch {
                    return Err("finality ancestry changes committee without proof".to_string());
                }
                if step.committee == *expected_records {
                    expected_set.clone()
                } else {
                    let candidate = consensus_committee(&step.committee)?;
                    if candidate.committee_root != expected_set.committee_root {
                        return Err("finality ancestry changes committee without proof".to_string());
                    }
                    candidate
                }
            } else {
                let candidate = consensus_committee(&step.committee)?;
                expected_committee = Some((
                    step.committee_epoch,
                    step.committee.clone(),
                    candidate.clone(),
                ));
                candidate
            };
        let committed = verify_finalized_ancestry_block(
            chain_id,
            genesis_hash,
            protocol_version,
            &step.block,
            step.committee_epoch,
            &step_committee,
        )?;
        if committed.parent_block_id != cursor {
            return Err("finality ancestry is not contiguous".to_string());
        }
        cursor = committed.block_id;
        if !step.next_committee.is_empty() {
            let next = consensus_committee(&step.next_committee)?;
            verify_committee_transition(
                chain_id,
                genesis_hash,
                protocol_version,
                step,
                &step_committee,
                &next,
            )?;
            if transition_start_root.is_empty() {
                transition_start_root = step_committee.committee_root.clone();
            }
            expected_committee =
                Some((step.next_committee_epoch, step.next_committee.clone(), next));
        }
    }
    let validator_set = if let Some((expected_epoch, expected_records, expected_set)) =
        &expected_committee
    {
        if target_committee_epoch != *expected_epoch {
            return Err("target block committee does not follow proved ancestry".to_string());
        }
        if target_committee == expected_records.as_slice() {
            expected_set.clone()
        } else {
            let candidate = consensus_committee(target_committee)?;
            if candidate.committee_root != expected_set.committee_root {
                return Err("target block committee does not follow proved ancestry".to_string());
            }
            candidate
        }
    } else {
        consensus_committee(target_committee)?
    };
    let committed_block = verify_finalized_block(
        chain_id,
        genesis_hash,
        protocol_version,
        target_block,
        target_committee_epoch,
        &validator_set,
    )?;
    if committed_block.parent_block_id != cursor {
        return Err("finality segment does not start at the prior checkpoint".to_string());
    }
    Ok(VerifiedFinalitySegment {
        committed_block,
        validator_set,
        transition_start_root,
    })
}

fn verify_finalized_ancestry_block(
    chain_id: &str,
    genesis_hash: &str,
    protocol_version: u32,
    block: &postfiat_types::BlockRecord,
    committee_epoch: u64,
    validator_set: &ConsensusV2ValidatorSet,
) -> Result<ConsensusV2BlockRef, String> {
    let header = &block.header;
    let commit = header
        .consensus_v2_commit
        .as_ref()
        .ok_or_else(|| "egress finality block has no consensus-v2 commit".to_string())?;
    if commit.schema != CONSENSUS_V2_COMMIT_SCHEMA {
        return Err("unsupported ancestry consensus-v2 commit schema".to_string());
    }
    let domain = consensus_v2_domain(
        chain_id.to_string(),
        genesis_hash.to_string(),
        protocol_version,
        committee_epoch,
        validator_set,
    );
    // A non-nil precommit QC is consensus-v2's commit authority. Historical
    // ancestry needs that authority plus exact header linkage; proposal and
    // prepare evidence remain mandatory for the terminal target block.
    let committed =
        consensus_v2_commit_from_precommit_qc(&domain, validator_set, &commit.precommit_qc)
            .map_err(|error| format!("invalid consensus-v2 ancestry finality: {error}"))?;
    if commit.precommit_qc.round.height != header.height
        || commit.precommit_qc.round.view != header.view
        || committed.height != header.height
        || committed.parent_block_id != header.parent_hash
        || committed.state_root != header.state_root
        || committed.bridge_exit_root != header.bridge_exit_root
        || committed.block_id != header.block_hash
    {
        return Err("ancestry precommit QC does not exactly bind block header".to_string());
    }
    Ok(committed)
}

fn consensus_committee(
    records: &[ValidatorRegistryEntry],
) -> Result<ConsensusV2ValidatorSet, String> {
    let mut validator_ids = BTreeSet::new();
    let validators = records
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
    ConsensusV2ValidatorSet::try_new(validators)
        .map_err(|error| format!("invalid egress witness committee: {error}"))
}

fn verify_finalized_block(
    chain_id: &str,
    genesis_hash: &str,
    protocol_version: u32,
    block: &postfiat_types::BlockRecord,
    committee_epoch: u64,
    validator_set: &ConsensusV2ValidatorSet,
) -> Result<ConsensusV2BlockRef, String> {
    let header = &block.header;
    let commit = header
        .consensus_v2_commit
        .as_ref()
        .ok_or_else(|| "egress finality block has no consensus-v2 commit".to_string())?;
    let domain = consensus_v2_domain(
        chain_id.to_string(),
        genesis_hash.to_string(),
        protocol_version,
        committee_epoch,
        validator_set,
    );
    let committed = verify_consensus_v2_commit(
        &domain,
        validator_set,
        commit,
        &ConsensusV2QcGraph::default(),
    )
    .map_err(|error| format!("invalid consensus-v2 finality: {error}"))?;
    if committed.height != header.height
        || committed.parent_block_id != header.parent_hash
        || committed.state_root != header.state_root
        || committed.bridge_exit_root != header.bridge_exit_root
        || committed.block_id != header.block_hash
        || committed.block_id != commit.proposal.block.block_id
        || commit.proposal.round.height != header.height
        || commit.proposal.round.view != header.view
    {
        return Err("consensus-v2 commit does not exactly bind block header".to_string());
    }
    Ok(committed)
}

fn verify_committee_transition(
    chain_id: &str,
    genesis_hash: &str,
    protocol_version: u32,
    step: &PfUsdcEgressFinalityStepV1,
    old_set: &ConsensusV2ValidatorSet,
    new_set: &ConsensusV2ValidatorSet,
) -> Result<(), String> {
    let expected_next_epoch = step
        .committee_epoch
        .checked_add(1)
        .ok_or_else(|| "committee epoch overflow".to_string())?;
    if step.block.header.batch_kind != "governance"
        || step.next_committee_epoch != expected_next_epoch
    {
        return Err("committee transition is not an epoch-advancing governance block".to_string());
    }
    let batch: GovernanceActionBatch = serde_json::from_str(&step.governance_payload_json)
        .map_err(|error| format!("invalid committee transition governance payload: {error}"))?;
    if batch.batch_id != step.block.header.batch_id
        || batch.validator_registry_updates.len() != 1
        || !batch.amendments.is_empty()
        || !batch.governance_agent_dry_runs.is_empty()
        || !batch.fastswap_bootstraps.is_empty()
        || !batch.fastpay_recovery_bootstraps.is_empty()
        || !batch.vault_bridge_route_profile_activations.is_empty()
    {
        return Err(
            "committee transition block must contain exactly one registry update".to_string(),
        );
    }
    let encoded_payload = serde_json::to_vec(&(
        chain_id,
        genesis_hash,
        protocol_version,
        "governance",
        batch.batch_id.as_str(),
        step.governance_payload_json.as_str(),
    ))
    .map_err(|error| format!("encode committee transition payload hash: {error}"))?;
    let payload_hash = hash_hex("postfiat.batch_archive_payload.v1", &encoded_payload);
    let committed_payload_hash = step
        .block
        .header
        .consensus_v2_commit
        .as_ref()
        .ok_or_else(|| "committee transition block commit is absent".to_string())?
        .precommit_qc
        .block
        .as_ref()
        .ok_or_else(|| "committee transition precommit QC is nil".to_string())?
        .payload_hash
        .as_str();
    if payload_hash != committed_payload_hash {
        return Err("committee transition payload is not bound by finality".to_string());
    }
    let update = &batch.validator_registry_updates[0];
    let domain = CobaltDomain {
        chain_id: chain_id.to_string(),
        genesis_hash: genesis_hash.to_string(),
        protocol_version,
    };
    verify_validator_registry_update(&domain, update)
        .map_err(|error| format!("invalid committee registry update: {error}"))?;
    let old_ids = old_set.validator_ids();
    let new_ids = new_set.validator_ids();
    let previous_ids = if update.previous_validators.is_empty() {
        update.validators.as_slice()
    } else {
        update.previous_validators.as_slice()
    };
    let resulting_ids = if update.new_validators.is_empty() {
        update.validators.as_slice()
    } else {
        update.new_validators.as_slice()
    };
    if update.validators != old_ids
        || previous_ids != old_ids
        || resulting_ids != new_ids
        || update.quorum < old_set.quorum
        || update.activation_height != step.block.header.height
        || registry_root(&step.committee, &old_ids)? != update.previous_registry_root
        || registry_root(&step.next_committee, &new_ids)? != update.new_registry_root
    {
        return Err("committee registry update does not bind old/new committees".to_string());
    }
    verify_registry_update_authorizations(
        update,
        &step.committee,
        old_set,
        step.committee_epoch,
        step.block.header.height,
    )
}

fn registry_root(
    records: &[ValidatorRegistryEntry],
    validator_ids: &[String],
) -> Result<String, String> {
    let mut by_id = std::collections::BTreeMap::new();
    for record in records {
        if by_id.insert(record.node_id.as_str(), record).is_some() {
            return Err("duplicate validator in registry root".to_string());
        }
    }
    let tuples = validator_ids
        .iter()
        .map(|validator_id| {
            let record = by_id
                .get(validator_id.as_str())
                .ok_or_else(|| "validator missing from registry root".to_string())?;
            Ok((
                record.node_id.as_str(),
                record.algorithm_id.as_str(),
                record.public_key_hex.as_str(),
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let encoded = serde_json::to_vec(&tuples)
        .map_err(|error| format!("encode validator registry root: {error}"))?;
    Ok(hash_hex("postfiat.validator_registry.root.v1", &encoded))
}

fn verify_registry_update_authorizations(
    update: &ValidatorRegistryUpdateRecord,
    old_committee: &[ValidatorRegistryEntry],
    old_set: &ConsensusV2ValidatorSet,
    expected_committee_epoch: u64,
    proposal_height: u64,
) -> Result<(), String> {
    if update.signed_authorizations.len() != update.votes.len()
        || update.signed_authorizations.len() != update.support.len()
        || update.signed_authorizations.len() < old_set.quorum
    {
        return Err("committee transition authorization set is below BFT quorum".to_string());
    }
    let keys = old_committee
        .iter()
        .map(|record| (record.node_id.as_str(), record))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut prior_validator: Option<&str> = None;
    for ((authorization, vote), support_validator) in update
        .signed_authorizations
        .iter()
        .zip(&update.votes)
        .zip(&update.support)
    {
        if prior_validator.is_some_and(|prior| prior >= authorization.validator.as_str()) {
            return Err("committee transition authorizations are not sorted unique".to_string());
        }
        prior_validator = Some(authorization.validator.as_str());
        if authorization.schema != SIGNED_GOVERNANCE_AUTHORIZATION_SCHEMA_V2
            || authorization.validator != *support_validator
            || authorization.validator != vote.validator
            || authorization.vote_id != vote.vote_id
            || !vote.accept
            || authorization.old_registry_root != update.previous_registry_root
            || authorization.committee_epoch != expected_committee_epoch
            || authorization.proposal_slot != proposal_height
            || authorization.expires_at_height < proposal_height
            || authorization.algorithm_id != ML_DSA_65_ALGORITHM
        {
            return Err("committee transition authorization binding mismatch".to_string());
        }
        let record = keys
            .get(authorization.validator.as_str())
            .ok_or_else(|| "committee transition signer is not in old committee".to_string())?;
        let public_key = hex_to_bytes(&record.public_key_hex)
            .map_err(|error| format!("decode committee transition public key: {error}"))?;
        let signature = hex_to_bytes(&authorization.signature_hex)
            .map_err(|error| format!("decode committee transition signature: {error}"))?;
        let message = registry_update_authorization_signing_bytes(update, authorization)?;
        if !ml_dsa_65_verify_with_context(
            &public_key,
            &message,
            &signature,
            GOVERNANCE_AUTHORIZATION_SIGNATURE_CONTEXT_V2,
        ) {
            return Err("committee transition authorization signature is invalid".to_string());
        }
    }
    Ok(())
}

fn registry_update_authorization_signing_bytes(
    update: &ValidatorRegistryUpdateRecord,
    authorization: &SignedGovernanceAuthorizationV2,
) -> Result<Vec<u8>, String> {
    serde_json::to_vec(&(
        (
            SIGNED_GOVERNANCE_AUTHORIZATION_SCHEMA_V2,
            "validator_registry_update",
            update.chain_id.as_str(),
            update.genesis_hash.as_str(),
            update.protocol_version,
            update.instance_id.as_str(),
            update.proposal_id.as_str(),
            update.proposer.as_str(),
            update.validators.as_slice(),
            update.quorum,
            update.activation_height,
            update.previous_registry_root.as_str(),
            update.new_registry_root.as_str(),
            update.operation.as_str(),
            update.subject_node_id.as_str(),
        ),
        (
            update.previous_trust_graph_root.as_deref(),
            update.new_trust_graph_root.as_deref(),
            update.trust_graph_transition_id.as_deref(),
            update.previous_validators.as_slice(),
            update.new_validators.as_slice(),
            update.previous_record.as_ref(),
            update.new_record.as_ref(),
        ),
        (
            authorization.old_registry_root.as_str(),
            authorization.committee_epoch,
            authorization.proposal_slot,
            authorization.expires_at_height,
            authorization.validator.as_str(),
            authorization.vote_id.as_str(),
            true,
            authorization.algorithm_id.as_str(),
        ),
    ))
    .map_err(|error| format!("encode committee transition authorization: {error}"))
}

#[cfg(test)]
mod tests;
