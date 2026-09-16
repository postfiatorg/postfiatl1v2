//! Canonical governance record encodings, shared by state commitment formats.
use super::*;

pub(super) fn append_governance_amendment(
    bytes: &mut Vec<u8>,
    prefix: &str,
    amendment: &GovernanceAmendment,
) {
    append_canonical_str(
        bytes,
        &format!("{prefix}.amendment_id"),
        &amendment.amendment_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.chain_id"), &amendment.chain_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.genesis_hash"),
        &amendment.genesis_hash,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.protocol_version"),
        amendment.protocol_version,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.instance_id"),
        &amendment.instance_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.proposal_id"),
        &amendment.proposal_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.certificate_id"),
        &amendment.certificate_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.proposer"), &amendment.proposer);
    append_string_list(bytes, &format!("{prefix}.validator"), &amendment.validators);
    append_canonical_usize(bytes, &format!("{prefix}.quorum"), amendment.quorum);
    append_canonical_str(bytes, &format!("{prefix}.kind"), &amendment.kind);
    append_canonical_u32(bytes, &format!("{prefix}.value"), amendment.value);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.activation_height"),
        amendment.activation_height,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.veto_until_height"),
        amendment.veto_until_height,
    );
    append_canonical_bool(bytes, &format!("{prefix}.paused"), amendment.paused);
    append_string_list(bytes, &format!("{prefix}.support"), &amendment.support);
    append_canonical_usize(
        bytes,
        &format!("{prefix}.vote_count"),
        amendment.votes.len(),
    );
    for vote in &amendment.votes {
        append_governance_vote(bytes, &format!("{prefix}.vote"), vote);
    }
}

pub(super) fn append_governance_vote(bytes: &mut Vec<u8>, prefix: &str, vote: &GovernanceVote) {
    append_canonical_str(bytes, &format!("{prefix}.vote_id"), &vote.vote_id);
    append_canonical_str(bytes, &format!("{prefix}.validator"), &vote.validator);
    append_canonical_bool(bytes, &format!("{prefix}.accept"), vote.accept);
}

pub(super) fn append_governance_activation_record(
    bytes: &mut Vec<u8>,
    prefix: &str,
    record: &GovernanceAmendmentActivationRecord,
) {
    append_canonical_str(bytes, &format!("{prefix}.schema"), &record.schema);
    append_canonical_str(
        bytes,
        &format!("{prefix}.activation_record_id"),
        &record.activation_record_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.amendment_id"),
        &record.amendment_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.chain_id"), &record.chain_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.genesis_hash"),
        &record.genesis_hash,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.protocol_version"),
        record.protocol_version,
    );
    append_canonical_str(bytes, &format!("{prefix}.batch_id"), &record.batch_id);
    append_canonical_str(bytes, &format!("{prefix}.kind"), &record.kind);
    append_canonical_u32(bytes, &format!("{prefix}.value"), record.value);
    append_canonical_u32(
        bytes,
        &format!("{prefix}.previous_value"),
        record.previous_value,
    );
    append_canonical_u32(bytes, &format!("{prefix}.new_value"), record.new_value);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.activation_height"),
        record.activation_height,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.veto_until_height"),
        record.veto_until_height,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.activated_height"),
        record.activated_height,
    );
}

pub(super) fn append_governance_supersession_record(
    bytes: &mut Vec<u8>,
    prefix: &str,
    record: &GovernanceAmendmentSupersessionRecord,
) {
    append_canonical_str(bytes, &format!("{prefix}.schema"), &record.schema);
    append_canonical_str(
        bytes,
        &format!("{prefix}.supersession_record_id"),
        &record.supersession_record_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.superseded_amendment_id"),
        &record.superseded_amendment_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.superseding_amendment_id"),
        &record.superseding_amendment_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.chain_id"), &record.chain_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.genesis_hash"),
        &record.genesis_hash,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.protocol_version"),
        record.protocol_version,
    );
    append_canonical_str(bytes, &format!("{prefix}.batch_id"), &record.batch_id);
    append_canonical_str(bytes, &format!("{prefix}.kind"), &record.kind);
    append_canonical_u32(
        bytes,
        &format!("{prefix}.previous_value"),
        record.previous_value,
    );
    append_canonical_u32(bytes, &format!("{prefix}.new_value"), record.new_value);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.supersession_height"),
        record.supersession_height,
    );
}

pub(super) fn append_governance_rollback_record(
    bytes: &mut Vec<u8>,
    prefix: &str,
    record: &GovernanceAmendmentRollbackRecord,
) {
    append_canonical_str(bytes, &format!("{prefix}.schema"), &record.schema);
    append_canonical_str(
        bytes,
        &format!("{prefix}.rollback_record_id"),
        &record.rollback_record_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.rolled_back_amendment_id"),
        &record.rolled_back_amendment_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.restored_amendment_id"),
        &record.restored_amendment_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.rollback_amendment_id"),
        &record.rollback_amendment_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.chain_id"), &record.chain_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.genesis_hash"),
        &record.genesis_hash,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.protocol_version"),
        record.protocol_version,
    );
    append_canonical_str(bytes, &format!("{prefix}.batch_id"), &record.batch_id);
    append_canonical_str(bytes, &format!("{prefix}.kind"), &record.kind);
    append_canonical_u32(
        bytes,
        &format!("{prefix}.previous_value"),
        record.previous_value,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.restored_value"),
        record.restored_value,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.rollback_height"),
        record.rollback_height,
    );
}

pub(super) fn append_governance_agent_dry_run_record(
    bytes: &mut Vec<u8>,
    prefix: &str,
    record: &GovernanceAgentDryRunRecord,
) {
    append_canonical_str(bytes, &format!("{prefix}.schema"), &record.schema);
    append_canonical_str(bytes, &format!("{prefix}.record_id"), &record.record_id);
    append_canonical_str(bytes, &format!("{prefix}.dry_run_id"), &record.dry_run_id);
    append_canonical_str(bytes, &format!("{prefix}.chain_id"), &record.chain_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.genesis_hash"),
        &record.genesis_hash,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.protocol_version"),
        record.protocol_version,
    );
    append_canonical_str(bytes, &format!("{prefix}.batch_id"), &record.batch_id);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.recorded_height"),
        record.recorded_height,
    );
    append_canonical_str(bytes, &format!("{prefix}.action_mode"), &record.action_mode);
    append_canonical_str(
        bytes,
        &format!("{prefix}.previous_dry_run_id"),
        &record.previous_dry_run_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.bundle_hash"), &record.bundle_hash);
    append_canonical_str(
        bytes,
        &format!("{prefix}.architecture_statement_hash"),
        &record.architecture_statement_hash,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.objective_statement_hash"),
        &record.objective_statement_hash,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.ruleset_hash"),
        &record.ruleset_hash,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.compiled_policy_hash"),
        &record.compiled_policy_hash,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.replay_bundle_root"),
        &record.replay_bundle_root,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.replay_bundle_uri"),
        &record.replay_bundle_uri,
    );
    append_canonical_str(bytes, &format!("{prefix}.report_root"), &record.report_root);
    append_canonical_str(bytes, &format!("{prefix}.report_uri"), &record.report_uri);
    append_canonical_str(
        bytes,
        &format!("{prefix}.validator_registry_root_before"),
        &record.validator_registry_root_before,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.validator_registry_root_after"),
        &record.validator_registry_root_after,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.registry_mutation_count"),
        record.registry_mutation_count,
    );
}

pub(super) fn append_vault_bridge_route_profile_record(
    bytes: &mut Vec<u8>,
    prefix: &str,
    record: &postfiat_types::VaultBridgeRouteProfileRecordV1,
) {
    append_canonical_str(bytes, &format!("{prefix}.schema"), &record.schema);
    append_canonical_str(
        bytes,
        &format!("{prefix}.profile_hash"),
        &record.profile_hash,
    );
    let profile = &record.profile;
    append_canonical_str(bytes, &format!("{prefix}.profile.schema"), &profile.schema);
    append_canonical_str(
        bytes,
        &format!("{prefix}.profile.route_id"),
        &profile.route_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.profile.asset_id"),
        &profile.asset_id,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.profile.source_chain_id"),
        profile.source_chain_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.profile.vault_address"),
        &profile.vault_address,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.profile.vault_runtime_code_hash"),
        &profile.vault_runtime_code_hash,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.profile.token_address"),
        &profile.token_address,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.profile.token_runtime_code_hash"),
        &profile.token_runtime_code_hash,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.profile.route_epoch"),
        profile.route_epoch,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.profile.verifier_kind"),
        &profile.verifier_kind,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.profile.evidence_tier"),
        &profile.evidence_tier,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.profile.verifier_policy_hash"),
        &profile.verifier_policy_hash,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.profile.verifier_program_vkey"),
        &profile.verifier_program_vkey,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.profile.verifier_proof_encoding"),
        &profile.verifier_proof_encoding,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.profile.max_proof_bytes"),
        profile.max_proof_bytes,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.profile.max_public_values_bytes"),
        profile.max_public_values_bytes,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.profile.max_snapshot_age_blocks"),
        profile.max_snapshot_age_blocks,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.profile.challenge_window_blocks"),
        profile.challenge_window_blocks,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.profile.max_epoch_gap_blocks"),
        profile.max_epoch_gap_blocks,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.profile.settle_deadline_blocks"),
        profile.settle_deadline_blocks,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.profile.min_challenge_bond"),
        profile.min_challenge_bond,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.profile.min_attestations"),
        profile.min_attestations,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.profile.minimum_confirmations"),
        profile.minimum_confirmations,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.profile.activation_height"),
        profile.activation_height,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.profile.expires_at_height"),
        profile.expires_at_height,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.governance_amendment_id"),
        &record.governance_amendment_id,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.authorized_height"),
        record.authorized_height,
    );
}

pub(super) fn append_storage_commitment_activation_record(
    bytes: &mut Vec<u8>,
    prefix: &str,
    record: &postfiat_types::StorageCommitmentActivationRecordV1,
) {
    append_canonical_str(bytes, &format!("{prefix}.schema"), &record.schema);
    append_canonical_str(bytes, &format!("{prefix}.feature_id"), &record.feature_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.activation_id"),
        &record.activation_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.authorization_amendment_id"),
        &record.authorization_amendment_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.chain_id"), &record.chain_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.genesis_hash"),
        &record.genesis_hash,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.protocol_version"),
        record.protocol_version,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.scheduling_block_height"),
        record.scheduling_block_height,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.activation_height"),
        record.activation_height,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.legacy_commitment_version"),
        &record.legacy_commitment_version,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.new_commitment_version"),
        &record.new_commitment_version,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.pre_activation_finalized_height"),
        record.pre_activation_finalized_height,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.pre_activation_block_hash"),
        &record.pre_activation_block_hash,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.pre_activation_state_root"),
        &record.pre_activation_state_root,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.pre_activation_ordered_count"),
        record.pre_activation_ordered_count,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.pre_activation_ordered_accumulator"),
        &record.pre_activation_ordered_accumulator,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.migration_packet_root"),
        &record.migration_packet_root,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.required_verifier_version"),
        &record.required_verifier_version,
    );
}

pub(super) fn append_storage_commitment_cancellation_record(
    bytes: &mut Vec<u8>,
    prefix: &str,
    record: &postfiat_types::StorageCommitmentCancellationRecordV1,
) {
    append_canonical_str(bytes, &format!("{prefix}.schema"), &record.schema);
    append_canonical_str(
        bytes,
        &format!("{prefix}.cancellation_id"),
        &record.cancellation_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.activation_id"),
        &record.activation_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.authorization_amendment_id"),
        &record.authorization_amendment_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.chain_id"), &record.chain_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.genesis_hash"),
        &record.genesis_hash,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.protocol_version"),
        record.protocol_version,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.cancellation_height"),
        record.cancellation_height,
    );
    append_canonical_str(bytes, &format!("{prefix}.reason"), &record.reason);
}

pub(super) fn append_validator_registry_update_record(
    bytes: &mut Vec<u8>,
    prefix: &str,
    record: &ValidatorRegistryUpdateRecord,
) {
    append_canonical_str(bytes, &format!("{prefix}.schema"), &record.schema);
    append_canonical_str(bytes, &format!("{prefix}.update_id"), &record.update_id);
    append_canonical_str(bytes, &format!("{prefix}.chain_id"), &record.chain_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.genesis_hash"),
        &record.genesis_hash,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.protocol_version"),
        record.protocol_version,
    );
    append_canonical_str(bytes, &format!("{prefix}.instance_id"), &record.instance_id);
    append_canonical_str(bytes, &format!("{prefix}.proposal_id"), &record.proposal_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.certificate_id"),
        &record.certificate_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.proposer"), &record.proposer);
    append_string_list(bytes, &format!("{prefix}.validator"), &record.validators);
    append_canonical_usize(bytes, &format!("{prefix}.quorum"), record.quorum);
    append_string_list(bytes, &format!("{prefix}.support"), &record.support);
    append_canonical_usize(bytes, &format!("{prefix}.vote_count"), record.votes.len());
    for vote in &record.votes {
        append_governance_vote(bytes, &format!("{prefix}.vote"), vote);
    }
    if !record.cobalt_authorizations.is_empty() {
        append_canonical_usize(
            bytes,
            &format!("{prefix}.cobalt_authorization_count"),
            record.cobalt_authorizations.len(),
        );
        for authorization in &record.cobalt_authorizations {
            append_signed_cobalt_validator_update_authorization(
                bytes,
                &format!("{prefix}.cobalt_authorization"),
                authorization,
            );
        }
    }
    append_canonical_u64(
        bytes,
        &format!("{prefix}.activation_height"),
        record.activation_height,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.previous_registry_root"),
        &record.previous_registry_root,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.new_registry_root"),
        &record.new_registry_root,
    );
    append_option_str(
        bytes,
        &format!("{prefix}.previous_trust_graph_root"),
        &record.previous_trust_graph_root,
    );
    append_option_str(
        bytes,
        &format!("{prefix}.new_trust_graph_root"),
        &record.new_trust_graph_root,
    );
    append_option_str(
        bytes,
        &format!("{prefix}.trust_graph_transition_id"),
        &record.trust_graph_transition_id,
    );
    append_string_list(
        bytes,
        &format!("{prefix}.previous_validator"),
        &record.previous_validators,
    );
    append_string_list(
        bytes,
        &format!("{prefix}.new_validator"),
        &record.new_validators,
    );
    append_canonical_str(bytes, &format!("{prefix}.operation"), &record.operation);
    append_canonical_str(
        bytes,
        &format!("{prefix}.subject_node_id"),
        &record.subject_node_id,
    );
    append_option_validator_registry_entry(
        bytes,
        &format!("{prefix}.previous_record"),
        record.previous_record.as_ref(),
    );
    append_option_validator_registry_entry(
        bytes,
        &format!("{prefix}.new_record"),
        record.new_record.as_ref(),
    );
}

pub(super) fn append_signed_cobalt_validator_update_authorization(
    bytes: &mut Vec<u8>,
    prefix: &str,
    authorization: &postfiat_types::SignedCobaltValidatorUpdateAuthorizationV1,
) {
    append_canonical_str(bytes, &format!("{prefix}.schema"), &authorization.schema);
    append_canonical_str(
        bytes,
        &format!("{prefix}.validator"),
        &authorization.validator,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.authority_transition_id"),
        &authorization.authority_transition_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.parent_cobalt_lock_hash"),
        &authorization.parent_cobalt_lock_hash,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.amendment_sequence"),
        authorization.amendment_sequence,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.proposal_slot"),
        authorization.proposal_slot,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.expires_at_height"),
        authorization.expires_at_height,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.algorithm_id"),
        &authorization.algorithm_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.signature_hex"),
        &authorization.signature_hex,
    );
}

pub(super) fn append_cobalt_authority_transition(
    bytes: &mut Vec<u8>,
    prefix: &str,
    transition: &postfiat_types::CobaltGovernanceAuthorityTransitionV1,
) {
    append_canonical_str(bytes, &format!("{prefix}.schema"), &transition.schema);
    append_canonical_str(
        bytes,
        &format!("{prefix}.transition_id"),
        &transition.transition_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.chain_id"), &transition.chain_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.genesis_hash"),
        &transition.genesis_hash,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.from_authority_mode"),
        transition.from_authority_mode,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.to_authority_mode"),
        transition.to_authority_mode,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.transition_kind"),
        &transition.transition_kind,
    );
    append_option_str(
        bytes,
        &format!("{prefix}.previous_transition_id"),
        &transition.previous_transition_id,
    );
    for (field, value) in [
        ("old_registry_root", &transition.old_registry_root),
        ("cobalt_lock_hash", &transition.cobalt_lock_hash),
        ("trust_graph_root", &transition.trust_graph_root),
        ("cobalt_registry_root", &transition.cobalt_registry_root),
    ] {
        append_canonical_str(bytes, &format!("{prefix}.{field}"), value);
    }
    append_canonical_u64(
        bytes,
        &format!("{prefix}.amendment_sequence"),
        transition.amendment_sequence,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.activation_height"),
        transition.activation_height,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.cobalt_protocol_version"),
        transition.cobalt_protocol_version,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.authority_scope"),
        &transition.authority_scope,
    );
    append_string_list(
        bytes,
        &format!("{prefix}.validator"),
        &transition.validators,
    );
    append_canonical_usize(
        bytes,
        &format!("{prefix}.approval_quorum"),
        transition.approval_quorum,
    );
    append_canonical_usize(
        bytes,
        &format!("{prefix}.approval_count"),
        transition.approvals.len(),
    );
    for approval in &transition.approvals {
        append_canonical_str(
            bytes,
            &format!("{prefix}.approval.schema"),
            &approval.schema,
        );
        append_canonical_str(
            bytes,
            &format!("{prefix}.approval.validator"),
            &approval.validator,
        );
        append_canonical_str(
            bytes,
            &format!("{prefix}.approval.old_registry_root"),
            &approval.old_registry_root,
        );
        append_canonical_u64(
            bytes,
            &format!("{prefix}.approval.proposal_slot"),
            approval.proposal_slot,
        );
        append_canonical_u64(
            bytes,
            &format!("{prefix}.approval.expires_at_height"),
            approval.expires_at_height,
        );
        append_canonical_str(
            bytes,
            &format!("{prefix}.approval.algorithm_id"),
            &approval.algorithm_id,
        );
        append_canonical_str(
            bytes,
            &format!("{prefix}.approval.signature_hex"),
            &approval.signature_hex,
        );
    }
}

pub(super) fn append_option_validator_registry_entry(
    bytes: &mut Vec<u8>,
    prefix: &str,
    entry: Option<&ValidatorRegistryEntry>,
) {
    append_canonical_bool(bytes, &format!("{prefix}.present"), entry.is_some());
    if let Some(entry) = entry {
        append_validator_registry_entry(bytes, prefix, entry);
    }
}

pub(super) fn append_validator_registry_entry(
    bytes: &mut Vec<u8>,
    prefix: &str,
    entry: &ValidatorRegistryEntry,
) {
    append_canonical_str(bytes, &format!("{prefix}.node_id"), &entry.node_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.algorithm_id"),
        &entry.algorithm_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.public_key_hex"),
        &entry.public_key_hex,
    );
    append_canonical_bool(bytes, &format!("{prefix}.active"), entry.active);
}
