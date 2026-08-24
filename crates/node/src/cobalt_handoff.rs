//! Versioned, consensus-ordered Cobalt governance-authority handoff.
//!
//! The handoff is deliberately narrower than block consensus: it can authorize
//! validator-registry and trust-graph evolution only. Consensus v2 remains the
//! sole block-ordering and finality protocol.

use super::*;

pub const COBALT_AUTHORITY_TRANSITION_SIGNATURE_CONTEXT_V1: &[u8] =
    b"postfiat-l1-v2/cobalt-authority-transition/v1";
pub const COBALT_VALIDATOR_UPDATE_SIGNATURE_CONTEXT_V1: &[u8] =
    b"postfiat-l1-v2/cobalt-validator-update/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GovernanceAuthorityBatchKind {
    Foundation,
    AuthorityTransition,
    CobaltValidatorTrustUpdate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CobaltAuthorityProgress {
    transition_id: String,
    parent_lock_hash: String,
    trust_graph_root: String,
    amendment_sequence: u64,
}

pub fn cobalt_authority_transition_id(
    transition: &postfiat_types::CobaltGovernanceAuthorityTransitionV1,
) -> io::Result<String> {
    let encoded = serde_json::to_vec(&(
        (
            postfiat_types::COBALT_AUTHORITY_TRANSITION_SCHEMA_V1,
            transition.chain_id.as_str(),
            transition.genesis_hash.as_str(),
            transition.from_authority_mode,
            transition.to_authority_mode,
            transition.transition_kind.as_str(),
            transition.previous_transition_id.as_deref(),
        ),
        (
            transition.old_registry_root.as_str(),
            transition.cobalt_lock_hash.as_str(),
            transition.trust_graph_root.as_str(),
            transition.cobalt_registry_root.as_str(),
            transition.amendment_sequence,
            transition.activation_height,
            transition.cobalt_protocol_version,
        ),
        (
            transition.authority_scope.as_str(),
            transition.validators.as_slice(),
            transition.approval_quorum,
        ),
    ))
    .map_err(invalid_data)?;
    Ok(hash_hex(
        "postfiat.cobalt.governance-authority-transition.v1",
        &encoded,
    ))
}

pub fn cobalt_authority_transition_approval_signing_bytes(
    transition: &postfiat_types::CobaltGovernanceAuthorityTransitionV1,
    approval: &postfiat_types::SignedCobaltAuthorityTransitionApprovalV1,
) -> io::Result<Vec<u8>> {
    serde_json::to_vec(&(
        (
            postfiat_types::SIGNED_COBALT_AUTHORITY_TRANSITION_APPROVAL_SCHEMA_V1,
            transition.transition_id.as_str(),
            transition.chain_id.as_str(),
            transition.genesis_hash.as_str(),
            transition.from_authority_mode,
            transition.to_authority_mode,
            transition.transition_kind.as_str(),
        ),
        (
            transition.previous_transition_id.as_deref(),
            transition.old_registry_root.as_str(),
            transition.cobalt_lock_hash.as_str(),
            transition.trust_graph_root.as_str(),
            transition.cobalt_registry_root.as_str(),
            transition.amendment_sequence,
            transition.activation_height,
        ),
        (
            transition.cobalt_protocol_version,
            transition.authority_scope.as_str(),
            transition.validators.as_slice(),
            transition.approval_quorum,
        ),
        (
            approval.validator.as_str(),
            approval.old_registry_root.as_str(),
            approval.proposal_slot,
            approval.expires_at_height,
            approval.algorithm_id.as_str(),
        ),
    ))
    .map_err(invalid_data)
}

pub fn cobalt_validator_update_authorization_signing_bytes(
    update: &ValidatorRegistryUpdateRecord,
    authorization: &postfiat_types::SignedCobaltValidatorUpdateAuthorizationV1,
) -> io::Result<Vec<u8>> {
    let mut unsigned_update = update.clone();
    unsigned_update.signed_authorizations.clear();
    unsigned_update.cobalt_authorizations.clear();
    serde_json::to_vec(&(
        postfiat_types::SIGNED_COBALT_VALIDATOR_UPDATE_AUTHORIZATION_SCHEMA_V1,
        &unsigned_update,
        authorization.validator.as_str(),
        authorization.authority_transition_id.as_str(),
        authorization.parent_cobalt_lock_hash.as_str(),
        authorization.amendment_sequence,
        authorization.proposal_slot,
        authorization.expires_at_height,
        authorization.algorithm_id.as_str(),
    ))
    .map_err(invalid_data)
}

pub(super) fn verify_governance_authority_batch(
    genesis: &Genesis,
    governance: &GovernanceState,
    registry: &ValidatorRegistry,
    batch: &GovernanceActionBatch,
    proposal_slot: u64,
) -> io::Result<GovernanceAuthorityBatchKind> {
    if !batch.cobalt_authority_transitions.is_empty() {
        if batch.cobalt_authority_transitions.len() != 1
            || governance_batch_action_count(batch) != 1
        {
            return Err(permission(
                "Cobalt authority transition must be the only action in its batch",
            ));
        }
        verify_cobalt_authority_transition(
            genesis,
            governance,
            registry,
            &batch.cobalt_authority_transitions[0],
            proposal_slot,
        )?;
        return Ok(GovernanceAuthorityBatchKind::AuthorityTransition);
    }

    match governance.authority_mode {
        postfiat_types::GOVERNANCE_AUTHORITY_MODE_FOUNDATION => {
            if batch.validator_registry_updates.iter().any(|update| {
                !update.cobalt_authorizations.is_empty()
                    || update.cobalt_decision_certificate.is_some()
            }) {
                return Err(permission(
                    "Cobalt validator authorization is inactive under Foundation authority",
                ));
            }
            if batch
                .amendments
                .iter()
                .any(|amendment| amendment.kind == GOVERNANCE_KIND_AUTHORITY_MODE)
            {
                return Err(permission(
                    "authority_mode cannot be changed by a label-only governance amendment",
                ));
            }
            Ok(GovernanceAuthorityBatchKind::Foundation)
        }
        postfiat_types::GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED => {
            if batch.validator_registry_updates.len() != 1
                || governance_batch_action_count(batch) != 1
            {
                return Err(permission(
                    "Cobalt authority accepts exactly one validator trust update per governance batch",
                ));
            }
            verify_cobalt_validator_trust_update(
                genesis,
                governance,
                registry,
                &batch.validator_registry_updates[0],
                proposal_slot,
            )?;
            Ok(GovernanceAuthorityBatchKind::CobaltValidatorTrustUpdate)
        }
        _ => Err(permission("unsupported governance authority mode")),
    }
}

pub fn verify_cobalt_scoped_governance_batch(
    genesis: &Genesis,
    governance: &GovernanceState,
    registry: &ValidatorRegistry,
    batch: &GovernanceActionBatch,
    proposal_slot: u64,
) -> io::Result<()> {
    verify_governance_authority_batch(genesis, governance, registry, batch, proposal_slot)
        .map(|_| ())
}

pub fn cobalt_governance_state_commitment(governance: &GovernanceState) -> Vec<u8> {
    let mut commitment = Vec::new();
    append_governance_state(&mut commitment, governance);
    commitment
}

pub fn verify_cobalt_authority_transition(
    genesis: &Genesis,
    governance: &GovernanceState,
    registry: &ValidatorRegistry,
    transition: &postfiat_types::CobaltGovernanceAuthorityTransitionV1,
    proposal_slot: u64,
) -> io::Result<()> {
    if transition.schema != postfiat_types::COBALT_AUTHORITY_TRANSITION_SCHEMA_V1
        || transition.chain_id != genesis.chain_id
        || transition.genesis_hash != genesis_hash(genesis)
        || transition.authority_scope != postfiat_types::COBALT_AUTHORITY_SCOPE_VALIDATOR_TRUST_V1
    {
        return Err(permission("Cobalt authority transition domain mismatch"));
    }
    if transition.activation_height == 0 || transition.activation_height != proposal_slot {
        return Err(permission(
            "Cobalt authority transition must be ordered at its exact activation height",
        ));
    }
    if transition.amendment_sequence == 0 || transition.cobalt_protocol_version == 0 {
        return Err(permission(
            "Cobalt authority transition sequence and protocol version must be nonzero",
        ));
    }
    validate_digest("Cobalt transition id", &transition.transition_id)?;
    validate_digest("Cobalt old registry root", &transition.old_registry_root)?;
    validate_digest("Cobalt lock hash", &transition.cobalt_lock_hash)?;
    validate_digest("Cobalt trust graph root", &transition.trust_graph_root)?;
    validate_digest("Cobalt registry root", &transition.cobalt_registry_root)?;
    if let Some(previous_transition_id) = &transition.previous_transition_id {
        validate_digest("Cobalt previous transition id", previous_transition_id)?;
    }

    let validators = active_validator_ids(governance)?;
    let registry_root = validator_registry_root(registry, &validators)?;
    let expected_quorum =
        bft_quorum_threshold(validators.len()).map_err(|error| invalid(error.to_string()))?;
    if transition.validators != validators
        || transition.old_registry_root != registry_root
        || transition.cobalt_registry_root != registry_root
        || transition.approval_quorum != expected_quorum
    {
        return Err(permission(
            "Cobalt authority transition does not bind the active validator registry",
        ));
    }

    verify_transition_direction(governance, transition)?;
    let expected_id = cobalt_authority_transition_id(transition)?;
    if transition.transition_id != expected_id {
        return Err(permission("Cobalt authority transition id mismatch"));
    }

    if transition.approvals.len() < expected_quorum || transition.approvals.len() > validators.len()
    {
        return Err(permission(
            "Cobalt authority transition approval set is outside quorum bounds",
        ));
    }
    let mut previous_validator: Option<&str> = None;
    for approval in &transition.approvals {
        if previous_validator.is_some_and(|previous| previous >= approval.validator.as_str()) {
            return Err(permission(
                "Cobalt authority transition approvals must be sorted unique",
            ));
        }
        previous_validator = Some(approval.validator.as_str());
        if approval.schema != postfiat_types::SIGNED_COBALT_AUTHORITY_TRANSITION_APPROVAL_SCHEMA_V1
            || !validators.contains(&approval.validator)
            || approval.old_registry_root != registry_root
            || approval.proposal_slot != proposal_slot
            || approval.expires_at_height < proposal_slot
            || approval.algorithm_id != ML_DSA_65_ALGORITHM
        {
            return Err(permission(
                "Cobalt authority transition approval binding mismatch",
            ));
        }
        verify_registry_signature(
            registry,
            &approval.validator,
            &approval.algorithm_id,
            &cobalt_authority_transition_approval_signing_bytes(transition, approval)?,
            &approval.signature_hex,
            COBALT_AUTHORITY_TRANSITION_SIGNATURE_CONTEXT_V1,
            "Cobalt authority transition approval",
        )?;
    }
    Ok(())
}

pub fn verify_cobalt_validator_trust_update(
    genesis: &Genesis,
    governance: &GovernanceState,
    registry: &ValidatorRegistry,
    update: &ValidatorRegistryUpdateRecord,
    proposal_slot: u64,
) -> io::Result<()> {
    let domain = cobalt_domain(genesis);
    verify_cobalt_validator_registry_update(&domain, update)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if !update.signed_authorizations.is_empty() {
        return Err(permission(
            "old-rule signed authorizations are forbidden after Cobalt activation",
        ));
    }
    if update.activation_height != proposal_slot {
        return Err(permission(
            "Cobalt validator trust update must be ordered at its exact activation height",
        ));
    }

    let validators = active_validator_ids(governance)?;
    let registry_root = validator_registry_root(registry, &validators)?;
    let quorum =
        bft_quorum_threshold(validators.len()).map_err(|error| invalid(error.to_string()))?;
    if update.validators != validators
        || validator_registry_update_previous_validators(update) != validators
        || update.previous_registry_root != registry_root
        || update.quorum != quorum
    {
        return Err(permission(
            "Cobalt validator trust update does not bind the current active registry",
        ));
    }

    let progress = current_cobalt_authority_progress(governance)?;
    let decision_certificate = update.cobalt_decision_certificate.as_ref().ok_or_else(|| {
        permission("Cobalt validator update requires a protocol decision certificate")
    })?;
    let expected_payload_hash =
        crate::cobalt_authority_certificate::cobalt_validator_update_payload_hash(update)?;
    let expected_round = proposal_slot
        .checked_sub(1)
        .ok_or_else(|| permission("Cobalt validator update proposal slot has no protocol round"))?;
    let previous_ratification = governance
        .validator_registry_updates
        .iter()
        .rev()
        .find(|prior| !prior.cobalt_authorizations.is_empty())
        .map(|prior| {
            prior
                .cobalt_decision_certificate
                .as_ref()
                .ok_or_else(|| {
                    invalid("stored Cobalt validator update has no decision certificate")
                })
                .and_then(crate::cobalt_authority_certificate::cobalt_decision_ratification)
        })
        .transpose()?;
    crate::cobalt_authority_certificate::verify_cobalt_validator_update_decision_certificate(
        decision_certificate,
        &domain,
        registry,
        &validators,
        &registry_root,
        &progress.trust_graph_root,
        &expected_payload_hash,
        expected_round,
        proposal_slot,
        previous_ratification.as_ref(),
    )?;

    let expected_sequence = progress
        .amendment_sequence
        .checked_add(1)
        .ok_or_else(|| invalid("Cobalt amendment sequence overflow"))?;
    if update.previous_trust_graph_root.as_deref() != Some(&progress.trust_graph_root)
        || update.new_trust_graph_root.is_none()
        || update.trust_graph_transition_id.is_none()
    {
        return Err(permission(
            "Cobalt validator update does not extend the active trust graph",
        ));
    }

    if update.cobalt_authorizations.len() != update.support.len()
        || update.cobalt_authorizations.len() != update.votes.len()
        || update.cobalt_authorizations.len() < quorum
    {
        return Err(permission(
            "Cobalt validator update requires one signed authorization for every support vote",
        ));
    }
    let mut previous_validator: Option<&str> = None;
    for ((authorization, support), vote) in update
        .cobalt_authorizations
        .iter()
        .zip(&update.support)
        .zip(&update.votes)
    {
        if previous_validator.is_some_and(|previous| previous >= authorization.validator.as_str()) {
            return Err(permission(
                "Cobalt validator update authorizations must be sorted unique",
            ));
        }
        previous_validator = Some(authorization.validator.as_str());
        if authorization.schema
            != postfiat_types::SIGNED_COBALT_VALIDATOR_UPDATE_AUTHORIZATION_SCHEMA_V1
            || authorization.validator != *support
            || authorization.validator != vote.validator
            || !vote.accept
            || !validators.contains(&authorization.validator)
            || authorization.authority_transition_id != progress.transition_id
            || authorization.parent_cobalt_lock_hash != progress.parent_lock_hash
            || authorization.amendment_sequence != expected_sequence
            || authorization.proposal_slot != proposal_slot
            || authorization.expires_at_height < proposal_slot
            || authorization.algorithm_id != ML_DSA_65_ALGORITHM
        {
            return Err(permission(
                "Cobalt validator update authorization binding mismatch",
            ));
        }
        verify_registry_signature(
            registry,
            &authorization.validator,
            &authorization.algorithm_id,
            &cobalt_validator_update_authorization_signing_bytes(update, authorization)?,
            &authorization.signature_hex,
            COBALT_VALIDATOR_UPDATE_SIGNATURE_CONTEXT_V1,
            "Cobalt validator update authorization",
        )?;
    }
    Ok(())
}

pub fn verify_cobalt_authority_history(
    genesis: &Genesis,
    governance: &GovernanceState,
) -> io::Result<()> {
    if governance.cobalt_authority_transitions.is_empty() {
        if governance.authority_mode == postfiat_types::GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED {
            return Err(invalid(
                "Cobalt authority mode has no versioned handoff record",
            ));
        }
        if governance.validator_registry_updates.iter().any(|update| {
            !update.cobalt_authorizations.is_empty() || update.cobalt_decision_certificate.is_some()
        }) {
            return Err(invalid(
                "Cobalt-authorized validator update predates the authority handoff",
            ));
        }
        return Ok(());
    }

    if governance.validator_registry_updates.iter().any(|update| {
        !update.cobalt_authorizations.is_empty() && update.cobalt_decision_certificate.is_none()
    }) {
        return Err(invalid(
            "stored Cobalt validator update has no protocol decision certificate",
        ));
    }

    let mut previous: Option<&postfiat_types::CobaltGovernanceAuthorityTransitionV1> = None;
    let mut progress_by_transition = BTreeMap::<String, CobaltAuthorityProgress>::new();
    let mut ordered_sequences = Vec::<(u64, u64)>::new();
    for transition in &governance.cobalt_authority_transitions {
        if transition.schema != postfiat_types::COBALT_AUTHORITY_TRANSITION_SCHEMA_V1
            || transition.chain_id != genesis.chain_id
            || transition.genesis_hash != genesis_hash(genesis)
            || transition.authority_scope
                != postfiat_types::COBALT_AUTHORITY_SCOPE_VALIDATOR_TRUST_V1
            || transition.transition_id != cobalt_authority_transition_id(transition)?
        {
            return Err(invalid("stored Cobalt authority transition is invalid"));
        }
        validate_transition_modes(transition).map_err(invalid)?;
        validate_digest("stored Cobalt transition id", &transition.transition_id)?;
        validate_digest("stored Cobalt lock", &transition.cobalt_lock_hash)?;
        validate_digest(
            "stored Cobalt trust graph root",
            &transition.trust_graph_root,
        )?;
        validate_digest(
            "stored Cobalt registry root",
            &transition.cobalt_registry_root,
        )?;
        if transition.amendment_sequence == 0
            || transition.activation_height == 0
            || transition.cobalt_protocol_version == 0
        {
            return Err(invalid(
                "stored Cobalt transition has a zero version boundary",
            ));
        }
        match previous {
            None => {
                if transition.previous_transition_id.is_some()
                    || transition.from_authority_mode
                        != postfiat_types::GOVERNANCE_AUTHORITY_MODE_FOUNDATION
                {
                    return Err(invalid(
                        "stored first Cobalt transition is not an activation",
                    ));
                }
            }
            Some(prior) => {
                if transition.previous_transition_id.as_deref()
                    != Some(prior.transition_id.as_str())
                    || transition.from_authority_mode != prior.to_authority_mode
                    || transition.activation_height <= prior.activation_height
                    || transition.cobalt_protocol_version <= prior.cobalt_protocol_version
                {
                    return Err(invalid(
                        "stored Cobalt authority transition history is not forward-moving",
                    ));
                }
            }
        }
        if progress_by_transition
            .insert(
                transition.transition_id.clone(),
                CobaltAuthorityProgress {
                    transition_id: transition.transition_id.clone(),
                    parent_lock_hash: transition.cobalt_lock_hash.clone(),
                    trust_graph_root: transition.trust_graph_root.clone(),
                    amendment_sequence: transition.amendment_sequence,
                },
            )
            .is_some()
        {
            return Err(invalid("duplicate stored Cobalt authority transition"));
        }
        ordered_sequences.push((transition.activation_height, transition.amendment_sequence));
        previous = Some(transition);
    }

    for update in &governance.validator_registry_updates {
        let Some(first) = update.cobalt_authorizations.first() else {
            continue;
        };
        if !update.signed_authorizations.is_empty() {
            return Err(invalid(
                "stored validator update mixes Foundation and Cobalt authorization",
            ));
        }
        let progress = progress_by_transition
            .get_mut(&first.authority_transition_id)
            .ok_or_else(|| invalid("stored Cobalt update references an unknown handoff"))?;
        let expected_sequence = progress
            .amendment_sequence
            .checked_add(1)
            .ok_or_else(|| invalid("stored Cobalt sequence overflow"))?;
        if first.parent_cobalt_lock_hash != progress.parent_lock_hash
            || first.amendment_sequence != expected_sequence
            || first.proposal_slot != update.activation_height
            || update.previous_trust_graph_root.as_deref() != Some(&progress.trust_graph_root)
            || update.new_trust_graph_root.is_none()
            || update.cobalt_authorizations.iter().any(|authorization| {
                authorization.schema
                    != postfiat_types::SIGNED_COBALT_VALIDATOR_UPDATE_AUTHORIZATION_SCHEMA_V1
                    || authorization.authority_transition_id != progress.transition_id
                    || authorization.parent_cobalt_lock_hash != progress.parent_lock_hash
                    || authorization.amendment_sequence != expected_sequence
                    || authorization.proposal_slot != update.activation_height
            })
        {
            return Err(invalid(
                "stored Cobalt validator update does not extend its handoff lock",
            ));
        }
        progress.parent_lock_hash = update.update_id.clone();
        progress.trust_graph_root = update
            .new_trust_graph_root
            .clone()
            .ok_or_else(|| invalid("stored Cobalt update has no new trust graph root"))?;
        progress.amendment_sequence = expected_sequence;
        ordered_sequences.push((update.activation_height, expected_sequence));
    }

    ordered_sequences.sort_unstable();
    for pair in ordered_sequences.windows(2) {
        if pair[0].0 >= pair[1].0 || pair[0].1 >= pair[1].1 {
            return Err(invalid(
                "stored Cobalt authority events are not strictly forward-moving",
            ));
        }
    }

    for (index, transition) in governance
        .cobalt_authority_transitions
        .iter()
        .enumerate()
        .skip(1)
    {
        let prior = &governance.cobalt_authority_transitions[index - 1];
        if transition.from_authority_mode
            == postfiat_types::GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED
        {
            let progress = progress_by_transition
                .get(&prior.transition_id)
                .ok_or_else(|| invalid("stored Cobalt rollback parent is missing"))?;
            if transition.cobalt_lock_hash != progress.parent_lock_hash
                || transition.trust_graph_root != progress.trust_graph_root
            {
                return Err(invalid(
                    "stored Cobalt rollback does not bind the prior authority lock",
                ));
            }
        }
    }

    if governance.authority_mode
        != governance
            .cobalt_authority_transitions
            .last()
            .expect("nonempty transition history checked")
            .to_authority_mode
    {
        return Err(invalid(
            "governance authority mode does not match its transition history",
        ));
    }
    Ok(())
}

pub(super) fn apply_cobalt_authority_transition(
    governance: &mut GovernanceState,
    transition: &postfiat_types::CobaltGovernanceAuthorityTransitionV1,
    block_height: u64,
) -> Result<(), String> {
    if transition.activation_height != block_height {
        return Err("Cobalt authority transition activation height mismatch".to_string());
    }
    if governance
        .cobalt_authority_transitions
        .iter()
        .any(|existing| existing.transition_id == transition.transition_id)
    {
        return Err("Cobalt authority transition replay rejected".to_string());
    }
    if governance.authority_mode != transition.from_authority_mode {
        return Err("Cobalt authority transition source mode mismatch".to_string());
    }
    let previous = governance.cobalt_authority_transitions.last();
    if transition.previous_transition_id.as_deref()
        != previous.map(|existing| existing.transition_id.as_str())
    {
        return Err("Cobalt authority transition parent mismatch".to_string());
    }
    if let Some(previous) = previous {
        let minimum_sequence = highest_cobalt_amendment_sequence(governance)
            .checked_add(1)
            .ok_or_else(|| "Cobalt amendment sequence overflow".to_string())?;
        if transition.activation_height <= previous.activation_height
            || transition.cobalt_protocol_version <= previous.cobalt_protocol_version
            || transition.amendment_sequence != minimum_sequence
        {
            return Err("Cobalt authority transition is not forward-moving".to_string());
        }
    }
    validate_transition_modes(transition)?;
    governance
        .cobalt_authority_transitions
        .push(transition.clone());
    governance.authority_mode = transition.to_authority_mode;
    Ok(())
}

fn verify_transition_direction(
    governance: &GovernanceState,
    transition: &postfiat_types::CobaltGovernanceAuthorityTransitionV1,
) -> io::Result<()> {
    if transition.from_authority_mode != governance.authority_mode {
        return Err(permission(
            "Cobalt authority transition source mode mismatch",
        ));
    }
    validate_transition_modes(transition).map_err(permission)?;
    let previous = governance.cobalt_authority_transitions.last();
    if transition.previous_transition_id.as_deref()
        != previous.map(|existing| existing.transition_id.as_str())
    {
        return Err(permission("Cobalt authority transition parent mismatch"));
    }
    match previous {
        None => {
            if transition.from_authority_mode
                != postfiat_types::GOVERNANCE_AUTHORITY_MODE_FOUNDATION
                || transition.to_authority_mode
                    != postfiat_types::GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED
            {
                return Err(permission(
                    "first Cobalt authority transition must activate from Foundation",
                ));
            }
        }
        Some(previous) => {
            let expected_sequence = highest_cobalt_amendment_sequence(governance)
                .checked_add(1)
                .ok_or_else(|| invalid("Cobalt amendment sequence overflow"))?;
            if transition.activation_height <= previous.activation_height
                || transition.cobalt_protocol_version <= previous.cobalt_protocol_version
                || transition.amendment_sequence != expected_sequence
            {
                return Err(permission(
                    "Cobalt authority transition must move height, sequence, and protocol version forward",
                ));
            }
            if transition.from_authority_mode
                == postfiat_types::GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED
            {
                let progress = current_cobalt_authority_progress(governance)?;
                if transition.cobalt_lock_hash != progress.parent_lock_hash
                    || transition.trust_graph_root != progress.trust_graph_root
                {
                    return Err(permission(
                        "Cobalt rollback must bind the exact current lock and trust graph",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_transition_modes(
    transition: &postfiat_types::CobaltGovernanceAuthorityTransitionV1,
) -> Result<(), String> {
    match (
        transition.from_authority_mode,
        transition.to_authority_mode,
        transition.transition_kind.as_str(),
    ) {
        (
            postfiat_types::GOVERNANCE_AUTHORITY_MODE_FOUNDATION,
            postfiat_types::GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED,
            postfiat_types::COBALT_AUTHORITY_TRANSITION_ACTIVATE,
        )
        | (
            postfiat_types::GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED,
            postfiat_types::GOVERNANCE_AUTHORITY_MODE_FOUNDATION,
            postfiat_types::COBALT_AUTHORITY_TRANSITION_ROLLBACK,
        ) => Ok(()),
        _ => Err("Cobalt authority transition mode and kind mismatch".to_string()),
    }
}

fn current_cobalt_authority_progress(
    governance: &GovernanceState,
) -> io::Result<CobaltAuthorityProgress> {
    let transition = governance
        .cobalt_authority_transitions
        .last()
        .ok_or_else(|| invalid("Cobalt authority mode has no transition record"))?;
    let mut progress = CobaltAuthorityProgress {
        transition_id: transition.transition_id.clone(),
        parent_lock_hash: transition.cobalt_lock_hash.clone(),
        trust_graph_root: transition.trust_graph_root.clone(),
        amendment_sequence: transition.amendment_sequence,
    };
    for update in &governance.validator_registry_updates {
        let Some(first) = update.cobalt_authorizations.first() else {
            continue;
        };
        if first.authority_transition_id != progress.transition_id {
            continue;
        }
        let expected_sequence = progress
            .amendment_sequence
            .checked_add(1)
            .ok_or_else(|| invalid("Cobalt amendment sequence overflow"))?;
        if first.amendment_sequence != expected_sequence
            || first.parent_cobalt_lock_hash != progress.parent_lock_hash
            || update.cobalt_authorizations.iter().any(|authorization| {
                authorization.authority_transition_id != progress.transition_id
                    || authorization.parent_cobalt_lock_hash != progress.parent_lock_hash
                    || authorization.amendment_sequence != expected_sequence
            })
        {
            return Err(invalid(
                "stored Cobalt validator update does not extend the authority lock",
            ));
        }
        progress.parent_lock_hash = update.update_id.clone();
        progress.trust_graph_root = update
            .new_trust_graph_root
            .clone()
            .ok_or_else(|| invalid("stored Cobalt update has no new trust graph root"))?;
        progress.amendment_sequence = expected_sequence;
    }
    Ok(progress)
}

fn highest_cobalt_amendment_sequence(governance: &GovernanceState) -> u64 {
    governance
        .cobalt_authority_transitions
        .iter()
        .map(|transition| transition.amendment_sequence)
        .chain(
            governance
                .validator_registry_updates
                .iter()
                .flat_map(|update| update.cobalt_authorizations.first())
                .map(|authorization| authorization.amendment_sequence),
        )
        .max()
        .unwrap_or(0)
}

fn verify_registry_signature(
    registry: &ValidatorRegistry,
    validator: &str,
    algorithm_id: &str,
    message: &[u8],
    signature_hex: &str,
    context: &[u8],
    label: &str,
) -> io::Result<()> {
    let record = validator_registry_record(registry, validator)?;
    if record.algorithm_id != algorithm_id {
        return Err(permission(format!("{label} key algorithm mismatch")));
    }
    let public_key = decode_ml_dsa_65_public_key_hex(
        &format!("{label} validator public key"),
        &record.public_key_hex,
    )?;
    let signature = decode_ml_dsa_65_signature_hex(&format!("{label} signature"), signature_hex)?;
    if !ml_dsa_65_verify_with_context(&public_key, message, &signature, context) {
        return Err(permission(format!("{label} signature verification failed")));
    }
    Ok(())
}

fn governance_batch_action_count(batch: &GovernanceActionBatch) -> usize {
    batch.amendments.len()
        + batch.validator_registry_updates.len()
        + batch.cobalt_authority_transitions.len()
        + batch.governance_agent_dry_runs.len()
        + batch.fastswap_bootstraps.len()
        + batch.fastpay_recovery_bootstraps.len()
        + batch.vault_bridge_route_profile_activations.len()
}

fn validate_digest(label: &str, value: &str) -> io::Result<()> {
    validate_lower_hex_len(label, value, 96).map_err(invalid)
}

fn permission(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message.into())
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cobalt_shadow::{
        assemble_protocol_transcript, build_registry_binding_manifest, CobaltShadowIdentity,
        CobaltShadowLimits, CobaltShadowRegistryBinding, CobaltShadowService,
    };
    use postfiat_consensus_cobalt::{
        trust_graph_transition_id, TrustGraphTransition, VALIDATOR_REGISTRY_OP_ROTATE_KEY,
    };
    use postfiat_types::{
        CobaltGovernanceAuthorityTransitionV1, CobaltValidatorUpdateDecisionCertificateV1,
        SignedCobaltAuthorityTransitionApprovalV1, SignedCobaltValidatorUpdateAuthorizationV1,
        ValidatorRegistryEntry, COBALT_VALIDATOR_UPDATE_DECISION_CERTIFICATE_SCHEMA_V1,
    };

    struct Fixture {
        genesis: Genesis,
        governance: GovernanceState,
        registry: ValidatorRegistry,
        keys: Vec<MlDsa65KeyPair>,
        validators: Vec<String>,
        registry_root: String,
        cobalt_root: PathBuf,
        cobalt_binding: CobaltShadowRegistryBinding,
    }

    fn fixture() -> Fixture {
        let genesis = Genesis::new_with_validator_count("postfiat-cobalt-handoff-test", 4);
        let validators = (0..4)
            .map(|index| format!("validator-{index}"))
            .collect::<Vec<_>>();
        let keys = (0..4)
            .map(|index| ml_dsa_65_keygen_from_seed(&[index as u8 + 41; 32]))
            .collect::<Vec<_>>();
        let registry = ValidatorRegistry {
            validators: validators
                .iter()
                .zip(&keys)
                .map(|(node_id, key)| ValidatorRegistryRecord {
                    node_id: node_id.clone(),
                    algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                    public_key_hex: bytes_to_hex(&key.public_key),
                })
                .collect(),
        };
        let governance = GovernanceState::new(4);
        let registry_root = validator_registry_root(&registry, &validators).expect("registry root");
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let cobalt_root = std::env::temp_dir().join(format!(
            "postfiat-cobalt-authority-certificate-{}-{nonce}",
            std::process::id()
        ));
        let mut services = validators
            .iter()
            .map(|validator| {
                CobaltShadowService::initialize(
                    cobalt_root.join(validator),
                    CobaltShadowIdentity {
                        node_id: validator.clone(),
                        chain_id: genesis.chain_id.clone(),
                        genesis_hash: genesis_hash(&genesis),
                        protocol_version: genesis.protocol_version,
                    },
                    CobaltShadowLimits::default(),
                )
                .expect("initialize Cobalt signer")
            })
            .collect::<Vec<_>>();
        let validator_bindings = services
            .iter()
            .zip(validators.iter().zip(&keys))
            .map(|(service, (validator, key))| {
                let key_path = cobalt_root.join(format!("{validator}.validator-keys.json"));
                crate::write_validator_key_file(
                    &key_path,
                    &ValidatorKeyFile {
                        validators: vec![ValidatorKeyRecord {
                            node_id: validator.clone(),
                            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                            public_key_hex: bytes_to_hex(&key.public_key),
                            private_key_hex: bytes_to_hex(&key.private_key),
                        }],
                    },
                )
                .expect("write validator key");
                service
                    .create_validator_binding(registry_root.clone(), &key_path)
                    .expect("create Cobalt key binding")
            })
            .collect::<Vec<_>>();
        let cobalt_binding = build_registry_binding_manifest(
            registry_root.clone(),
            registry.clone(),
            validator_bindings,
            bft_quorum_threshold(validators.len()).expect("quorum"),
            10,
        )
        .expect("build Cobalt registry binding");
        for service in &mut services {
            service
                .bind_registry_manifest(&cobalt_binding)
                .expect("bind Cobalt registry");
        }
        Fixture {
            genesis,
            governance,
            registry,
            keys,
            validators,
            registry_root,
            cobalt_root,
            cobalt_binding,
        }
    }

    fn signed_transition(
        fixture: &Fixture,
        governance: &GovernanceState,
        height: u64,
        protocol_version: u32,
        lock_hash: String,
        graph_root: String,
    ) -> CobaltGovernanceAuthorityTransitionV1 {
        let from = governance.authority_mode;
        let (to, kind) = if from == postfiat_types::GOVERNANCE_AUTHORITY_MODE_FOUNDATION {
            (
                postfiat_types::GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED,
                postfiat_types::COBALT_AUTHORITY_TRANSITION_ACTIVATE,
            )
        } else {
            (
                postfiat_types::GOVERNANCE_AUTHORITY_MODE_FOUNDATION,
                postfiat_types::COBALT_AUTHORITY_TRANSITION_ROLLBACK,
            )
        };
        let sequence = if governance.cobalt_authority_transitions.is_empty() {
            7
        } else {
            highest_cobalt_amendment_sequence(governance) + 1
        };
        let mut transition = CobaltGovernanceAuthorityTransitionV1 {
            schema: postfiat_types::COBALT_AUTHORITY_TRANSITION_SCHEMA_V1.to_string(),
            transition_id: String::new(),
            chain_id: fixture.genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&fixture.genesis),
            from_authority_mode: from,
            to_authority_mode: to,
            transition_kind: kind.to_string(),
            previous_transition_id: governance
                .cobalt_authority_transitions
                .last()
                .map(|previous| previous.transition_id.clone()),
            old_registry_root: fixture.registry_root.clone(),
            cobalt_lock_hash: lock_hash,
            trust_graph_root: graph_root,
            cobalt_registry_root: fixture.registry_root.clone(),
            amendment_sequence: sequence,
            activation_height: height,
            cobalt_protocol_version: protocol_version,
            authority_scope: postfiat_types::COBALT_AUTHORITY_SCOPE_VALIDATOR_TRUST_V1.to_string(),
            validators: fixture.validators.clone(),
            approval_quorum: bft_quorum_threshold(fixture.validators.len()).expect("quorum"),
            approvals: Vec::new(),
        };
        transition.transition_id = cobalt_authority_transition_id(&transition).expect("id");
        transition.approvals = fixture
            .validators
            .iter()
            .zip(&fixture.keys)
            .take(transition.approval_quorum)
            .map(|(validator, key)| {
                let mut approval = SignedCobaltAuthorityTransitionApprovalV1 {
                    schema: postfiat_types::SIGNED_COBALT_AUTHORITY_TRANSITION_APPROVAL_SCHEMA_V1
                        .to_string(),
                    validator: validator.clone(),
                    old_registry_root: fixture.registry_root.clone(),
                    proposal_slot: height,
                    expires_at_height: height + 10,
                    algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                    signature_hex: String::new(),
                };
                approval.signature_hex = bytes_to_hex(
                    &ml_dsa_65_sign_with_context(
                        &key.private_key,
                        &cobalt_authority_transition_approval_signing_bytes(&transition, &approval)
                            .expect("signing bytes"),
                        COBALT_AUTHORITY_TRANSITION_SIGNATURE_CONTEXT_V1,
                    )
                    .expect("signature"),
                );
                approval
            })
            .collect();
        transition
    }

    fn activate(fixture: &Fixture) -> GovernanceState {
        let mut governance = fixture.governance.clone();
        let transition = signed_transition(
            fixture,
            &governance,
            10,
            1,
            "11".repeat(48),
            fixture.cobalt_binding.trust_graph.trust_graph_root.clone(),
        );
        verify_cobalt_authority_transition(
            &fixture.genesis,
            &governance,
            &fixture.registry,
            &transition,
            10,
        )
        .expect("transition verifies");
        apply_cobalt_authority_transition(&mut governance, &transition, 10)
            .expect("transition applies");
        governance
    }

    fn signed_rotate_update(
        fixture: &Fixture,
        governance: &GovernanceState,
        height: u64,
    ) -> ValidatorRegistryUpdateRecord {
        let progress = current_cobalt_authority_progress(governance).expect("progress");
        let replacement = ml_dsa_65_keygen_from_seed(&[99; 32]);
        let mut new_registry = fixture.registry.clone();
        new_registry.validators[0].public_key_hex = bytes_to_hex(&replacement.public_key);
        let new_root =
            validator_registry_root(&new_registry, &fixture.validators).expect("new root");
        let new_graph_root = "33".repeat(48);
        let transition = TrustGraphTransition {
            previous_registry_root: fixture.registry_root.clone(),
            new_registry_root: new_root.clone(),
            previous_trust_graph_root: progress.trust_graph_root.clone(),
            new_trust_graph_root: new_graph_root.clone(),
            activation_height: height,
            transition_id: String::new(),
        };
        let transition_id =
            trust_graph_transition_id(&cobalt_domain(&fixture.genesis), &transition)
                .expect("trust transition id");
        let request = ValidatorRegistryUpdateRequest {
            activation_height: height,
            previous_registry_root: fixture.registry_root.clone(),
            new_registry_root: new_root,
            previous_trust_graph_root: Some(progress.trust_graph_root.clone()),
            new_trust_graph_root: Some(new_graph_root),
            trust_graph_transition_id: Some(transition_id),
            previous_validators: fixture.validators.clone(),
            new_validators: fixture.validators.clone(),
            operation: VALIDATOR_REGISTRY_OP_ROTATE_KEY.to_string(),
            subject_node_id: fixture.validators[0].clone(),
            previous_record: Some(ValidatorRegistryEntry {
                node_id: fixture.validators[0].clone(),
                algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                public_key_hex: fixture.registry.validators[0].public_key_hex.clone(),
                active: true,
            }),
            new_record: Some(ValidatorRegistryEntry {
                node_id: fixture.validators[0].clone(),
                algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                public_key_hex: bytes_to_hex(&replacement.public_key),
                active: true,
            }),
        };
        let quorum = bft_quorum_threshold(fixture.validators.len()).expect("quorum");
        let mut update = certify_validator_registry_update(
            &cobalt_domain(&fixture.genesis),
            &EssentialSubsetConfig {
                validators: fixture.validators.clone(),
                quorum,
            },
            request,
            fixture.validators[..quorum].to_vec(),
        )
        .expect("update");
        let payload_hash =
            crate::cobalt_authority_certificate::cobalt_validator_update_payload_hash(&update)
                .expect("Cobalt update payload hash");
        let mut services = fixture
            .validators
            .iter()
            .map(|validator| {
                CobaltShadowService::open(fixture.cobalt_root.join(validator))
                    .expect("open Cobalt signer")
            })
            .collect::<Vec<_>>();
        let round = height.checked_sub(1).expect("Cobalt protocol round");
        let proposal = services[0]
            .create_protocol_proposal(&fixture.cobalt_binding, round, payload_hash)
            .expect("create Cobalt proposal");
        let contributions = services
            .iter_mut()
            .map(|service| {
                service
                    .create_protocol_contribution(&fixture.cobalt_binding, &proposal)
                    .expect("create Cobalt contribution")
            })
            .collect::<Vec<_>>();
        let transcript =
            assemble_protocol_transcript(&fixture.cobalt_binding, proposal, contributions)
                .expect("assemble Cobalt protocol transcript");
        update.cobalt_decision_certificate = Some(CobaltValidatorUpdateDecisionCertificateV1 {
            schema: COBALT_VALIDATOR_UPDATE_DECISION_CERTIFICATE_SCHEMA_V1.to_string(),
            registry_binding: serde_json::to_value(&fixture.cobalt_binding)
                .expect("serialize Cobalt registry binding"),
            protocol_transcript: serde_json::to_value(transcript)
                .expect("serialize Cobalt protocol transcript"),
        });
        let sequence = progress.amendment_sequence + 1;
        update.cobalt_authorizations = fixture
            .validators
            .iter()
            .zip(&fixture.keys)
            .take(quorum)
            .map(|(validator, key)| {
                let mut authorization = SignedCobaltValidatorUpdateAuthorizationV1 {
                    schema: postfiat_types::SIGNED_COBALT_VALIDATOR_UPDATE_AUTHORIZATION_SCHEMA_V1
                        .to_string(),
                    validator: validator.clone(),
                    authority_transition_id: progress.transition_id.clone(),
                    parent_cobalt_lock_hash: progress.parent_lock_hash.clone(),
                    amendment_sequence: sequence,
                    proposal_slot: height,
                    expires_at_height: height + 10,
                    algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                    signature_hex: String::new(),
                };
                authorization.signature_hex = bytes_to_hex(
                    &ml_dsa_65_sign_with_context(
                        &key.private_key,
                        &cobalt_validator_update_authorization_signing_bytes(
                            &update,
                            &authorization,
                        )
                        .expect("signing bytes"),
                        COBALT_VALIDATOR_UPDATE_SIGNATURE_CONTEXT_V1,
                    )
                    .expect("signature"),
                );
                authorization
            })
            .collect();
        update
    }

    #[test]
    fn handoff_requires_distinct_mldsa65_quorum_approvals() {
        let fixture = fixture();
        let transition = signed_transition(
            &fixture,
            &fixture.governance,
            10,
            1,
            "11".repeat(48),
            "22".repeat(48),
        );
        verify_cobalt_authority_transition(
            &fixture.genesis,
            &fixture.governance,
            &fixture.registry,
            &transition,
            10,
        )
        .expect("valid approval quorum");

        let mut missing = transition.clone();
        missing.approvals.pop();
        assert!(verify_cobalt_authority_transition(
            &fixture.genesis,
            &fixture.governance,
            &fixture.registry,
            &missing,
            10,
        )
        .expect_err("missing signature rejected")
        .to_string()
        .contains("quorum"));

        let mut duplicate = transition;
        duplicate.approvals[1] = duplicate.approvals[0].clone();
        assert!(verify_cobalt_authority_transition(
            &fixture.genesis,
            &fixture.governance,
            &fixture.registry,
            &duplicate,
            10,
        )
        .expect_err("duplicate signature rejected")
        .to_string()
        .contains("sorted unique"));
    }

    #[test]
    fn authority_modes_are_exclusive_and_cobalt_scope_is_validator_trust_only() {
        let fixture = fixture();
        let governance = activate(&fixture);
        let mut update = signed_rotate_update(&fixture, &governance, 11);
        verify_cobalt_validator_trust_update(
            &fixture.genesis,
            &governance,
            &fixture.registry,
            &update,
            11,
        )
        .expect("Cobalt-authorized validator trust update");

        update
            .signed_authorizations
            .push(SignedGovernanceAuthorizationV2 {
                schema: SIGNED_GOVERNANCE_AUTHORIZATION_SCHEMA_V2.to_string(),
                validator: fixture.validators[0].clone(),
                vote_id: update.votes[0].vote_id.clone(),
                old_registry_root: fixture.registry_root.clone(),
                committee_epoch: 0,
                proposal_slot: 11,
                expires_at_height: 20,
                algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                signature_hex: "00".repeat(ML_DSA_65_SIGNATURE_BYTES),
            });
        assert!(verify_cobalt_validator_trust_update(
            &fixture.genesis,
            &governance,
            &fixture.registry,
            &update,
            11,
        )
        .expect_err("mixed old/new authority rejected")
        .to_string()
        .contains("old-rule"));

        let amendment = ratify_governance_amendment_with_lifecycle(
            &cobalt_domain(&fixture.genesis),
            &EssentialSubsetConfig {
                validators: fixture.validators.clone(),
                quorum: 3,
            },
            GOVERNANCE_KIND_CRYPTO_POLICY,
            2,
            fixture.validators[..3].to_vec(),
            GovernanceAmendmentLifecycle::immediate(),
        )
        .expect("amendment fixture");
        let batch = GovernanceActionBatch::new("unused", vec![amendment]);
        assert!(verify_governance_authority_batch(
            &fixture.genesis,
            &governance,
            &fixture.registry,
            &batch,
            11,
        )
        .expect_err("non-validator Cobalt action rejected")
        .to_string()
        .contains("validator trust"));
    }

    #[test]
    fn cobalt_authority_rejects_quorum_only_and_tampered_protocol_decisions() {
        let fixture = fixture();
        let governance = activate(&fixture);
        let update = signed_rotate_update(&fixture, &governance, 11);

        let mut quorum_only = update.clone();
        quorum_only.cobalt_decision_certificate = None;
        assert!(verify_cobalt_validator_trust_update(
            &fixture.genesis,
            &governance,
            &fixture.registry,
            &quorum_only,
            11,
        )
        .expect_err("quorum signatures without Cobalt decision must be rejected")
        .to_string()
        .contains("protocol decision certificate"));

        let mut tampered = update;
        tampered
            .cobalt_decision_certificate
            .as_mut()
            .expect("decision certificate")
            .protocol_transcript["payload_hash"] = serde_json::Value::String("ff".repeat(48));
        assert!(verify_cobalt_validator_trust_update(
            &fixture.genesis,
            &governance,
            &fixture.registry,
            &tampered,
            11,
        )
        .expect_err("tampered Cobalt decision must be rejected")
        .to_string()
        .contains("not bound"));

        let mut wrong_domain = signed_rotate_update(&fixture, &governance, 12);
        let certificate = wrong_domain
            .cobalt_decision_certificate
            .as_mut()
            .expect("decision certificate");
        certificate.registry_binding["trust_graph"]["chain_id"] =
            serde_json::Value::String("another-chain".to_string());
        certificate.protocol_transcript["trust_graph"]["chain_id"] =
            serde_json::Value::String("another-chain".to_string());
        assert!(verify_cobalt_validator_trust_update(
            &fixture.genesis,
            &governance,
            &fixture.registry,
            &wrong_domain,
            12,
        )
        .expect_err("cross-chain Cobalt decision must be rejected")
        .to_string()
        .contains("live chain"));
    }

    #[test]
    fn transition_replay_and_non_forward_rollback_are_rejected() {
        let fixture = fixture();
        let mut governance = fixture.governance.clone();
        let transition = signed_transition(
            &fixture,
            &governance,
            10,
            1,
            "11".repeat(48),
            "22".repeat(48),
        );
        apply_cobalt_authority_transition(&mut governance, &transition, 10).expect("activate");
        assert!(
            apply_cobalt_authority_transition(&mut governance, &transition, 10)
                .expect_err("replay rejected")
                .contains("replay")
        );

        let mut rollback = signed_transition(
            &fixture,
            &governance,
            11,
            2,
            transition.cobalt_lock_hash.clone(),
            transition.trust_graph_root.clone(),
        );
        rollback.amendment_sequence = transition.amendment_sequence;
        rollback.transition_id = cobalt_authority_transition_id(&rollback).expect("rollback id");
        assert!(verify_cobalt_authority_transition(
            &fixture.genesis,
            &governance,
            &fixture.registry,
            &rollback,
            11,
        )
        .expect_err("non-forward rollback rejected")
        .to_string()
        .contains("move"));

        let rollback = signed_transition(
            &fixture,
            &governance,
            11,
            2,
            transition.cobalt_lock_hash,
            transition.trust_graph_root,
        );
        verify_cobalt_authority_transition(
            &fixture.genesis,
            &governance,
            &fixture.registry,
            &rollback,
            11,
        )
        .expect("forward rollback verifies");
        apply_cobalt_authority_transition(&mut governance, &rollback, 11)
            .expect("forward rollback applies");
        verify_cobalt_authority_history(&fixture.genesis, &governance)
            .expect("rollback history verifies");
        assert_eq!(
            governance.authority_mode,
            postfiat_types::GOVERNANCE_AUTHORITY_MODE_FOUNDATION
        );
        assert_eq!(governance.cobalt_authority_transitions.len(), 2);
    }

    #[test]
    fn cobalt_update_replay_parent_is_rejected_after_first_apply() {
        let fixture = fixture();
        let mut governance = activate(&fixture);
        let update = signed_rotate_update(&fixture, &governance, 11);
        verify_cobalt_validator_trust_update(
            &fixture.genesis,
            &governance,
            &fixture.registry,
            &update,
            11,
        )
        .expect("first update verifies");
        governance.validator_registry_updates.push(update.clone());
        let replay_error = verify_cobalt_validator_trust_update(
            &fixture.genesis,
            &governance,
            &fixture.registry,
            &update,
            11,
        )
        .expect_err("replayed update rejected");
        assert!(
            replay_error.to_string().contains("active trust graph")
                || replay_error.to_string().contains("binding mismatch")
                || replay_error.to_string().contains("trust graph")
        );
    }

    #[test]
    fn handoff_uses_the_consensus_ordered_governance_batch_and_committed_state_path() {
        let fixture = fixture();
        let mut governance = fixture.governance.clone();
        let transition = signed_transition(
            &fixture,
            &governance,
            10,
            1,
            "11".repeat(48),
            "22".repeat(48),
        );
        let batch = build_governance_action_batch_with_cobalt_authority_transition(
            &fixture.genesis,
            transition.clone(),
        )
        .expect("batch");
        verify_governance_action_batch_id(&fixture.genesis, &batch).expect("batch id");
        verify_live_signed_governance_batch(
            &fixture.genesis,
            &governance,
            &fixture.registry,
            &batch,
            10,
        )
        .expect("live authorization");

        let mut before = Vec::new();
        append_governance_state(&mut before, &governance);
        let receipts = execute_governance_batch(&mut governance, None, &batch, 10);
        assert_eq!(receipts.len(), 1);
        assert!(receipts[0].accepted);
        assert_eq!(receipts[0].tx_id, transition.transition_id);
        assert_eq!(
            governance.authority_mode,
            postfiat_types::GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED
        );
        let mut after = Vec::new();
        append_governance_state(&mut after, &governance);
        assert_ne!(
            before, after,
            "handoff must enter the replicated state root"
        );

        let persisted = serde_json::to_vec(&governance).expect("serialize governance");
        let mut restored: GovernanceState =
            serde_json::from_slice(&persisted).expect("restore governance");
        verify_cobalt_authority_history(&fixture.genesis, &restored)
            .expect("restored authority history");
        let mut restored_commitment = Vec::new();
        append_governance_state(&mut restored_commitment, &restored);
        assert_eq!(after, restored_commitment);
        let replay = execute_governance_batch(&mut restored, None, &batch, 10);
        assert_eq!(replay.len(), 1);
        assert!(!replay[0].accepted);
        assert_eq!(replay[0].code, "cobalt_authority_transition_rejected");
    }
}
