use super::*;
use postfiat_types::solana_exit;

pub(super) fn is_registration_amendment(kind: &str) -> bool {
    kind.strip_suffix(solana_exit::PROFILE_HASH).is_some_and(|prefix|
        prefix == postfiat_types::vault_bridge_route_amendment_prefix(solana_exit::FAMILY))
}

pub(super) fn registers_descriptor(batch: &GovernanceActionBatch) -> bool {
    batch
        .vault_bridge_route_profile_activations
        .iter()
        .any(|a| {
            is_registration_amendment(&a.amendment.kind)
                && a.profile == solana_exit::descriptor()
        })
}

pub(super) fn require_operator_release(
    chain: &str,
    genesis_hash: &str,
    height: u64,
) -> io::Result<()> {
    if solana_exit::release_height(chain, genesis_hash).is_none_or(|release| height < release) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Solana exit release height has not been reached",
        ));
    }
    Ok(())
}

/// Historical pre-release registration attempts must reproduce their original
/// rejection receipt. Every successful registration and every later decision
/// must cross the current validator signature boundary, including in replay.
pub(super) fn needs_signatures(
    genesis: &Genesis,
    batch: &GovernanceActionBatch,
    height: u64,
) -> bool {
    batch
        .amendments
        .iter()
        .any(|a| solana_exit::is_decision(&a.kind))
        || ((!batch.validator_registry_updates.is_empty()
            || batch
                .amendments
                .iter()
                .any(|a| a.kind == GOVERNANCE_KIND_VALIDATOR_SET)
            || registers_descriptor(batch))
            && solana_exit::release_height(&genesis.chain_id, &genesis_hash(genesis))
                .is_some_and(|h| h <= height))
}

pub(super) fn require_unanimity(
    amendment: &GovernanceAmendment,
    validators: &[String],
) -> Result<(), String> {
    if amendment.validators != validators
        || amendment.quorum != validators.len()
        || amendment.signed_authorizations.len() != validators.len()
    {
        return Err("Solana exit governance requires every active validator signature".into());
    }
    Ok(())
}

/// These are ordinary signed governance amendments. No relayer signature or
/// bare issuer settlement is an attestation of finalized Solana execution.
pub(super) fn validate_decision(
    genesis: &Genesis,
    governance: &GovernanceState,
    amendment: &GovernanceAmendment,
    height: u64,
) -> Result<(), String> {
    let (record, registration) = solana_exit::registered(governance)
        .ok_or("Solana exit decision requires the released descriptor")?;
    if registration.chain_id != genesis.chain_id
        || registration.genesis_hash != genesis_hash(genesis)
        || amendment.chain_id != registration.chain_id
        || amendment.genesis_hash != registration.genesis_hash
        || record.authorized_height >= height
        || amendment.activation_height != height
        || amendment.value != 1
        || amendment.paused
    {
        return Err(
            "Solana exit decision requires a later explicit governance block at its signed height"
                .into(),
        );
    }
    require_unanimity(
        amendment,
        &active_validator_ids(governance).map_err(|e| e.to_string())?,
    )?;
    if amendment.kind == solana_exit::OPEN {
        if solana_exit::open_height(governance).is_some()
            || height >= record.profile.expires_at_height
        {
            return Err("Solana exit burns already opened or descriptor expired".into());
        }
        verify_open_history(governance)?;
    } else if amendment.kind == solana_exit::CLOSE {
        if solana_exit::open_height(governance).is_none() {
            return Err("Solana exit burns are already closed".into());
        }
    } else {
        solana_exit::payout_fields(&amendment.kind)?;
    }
    Ok(())
}

pub(super) fn apply_decision(
    genesis: Option<&Genesis>,
    governance: &GovernanceState,
    ledger: Option<&mut LedgerState>,
    amendment: &GovernanceAmendment,
    height: u64,
) -> Result<(), String> {
    let genesis = genesis.ok_or("Solana exit decision requires genesis")?;
    validate_decision(genesis, governance, amendment, height)?;
    if matches!(
        amendment.kind.as_str(),
        solana_exit::OPEN | solana_exit::CLOSE
    ) {
        return Ok(());
    }
    let payout = solana_exit::payout_fields(&amendment.kind)?;
    if governance
        .amendments
        .iter()
        .filter(|a| a.kind.starts_with(solana_exit::PAID))
        .any(|a| {
            solana_exit::payout_fields(&a.kind).is_ok_and(|used| {
                used.signature_hex == payout.signature_hex && used.index == payout.index
            })
        })
    {
        return Err("Solana payout in this transaction already settled".into());
    }
    postfiat_execution::settle_governed_solana_payout(
        ledger.ok_or("Solana settlement requires canonical ledger")?,
        &genesis.chain_id,
        payout.packet_hash,
        &amendment.amendment_id,
        height,
    )
}

/// A numbered amendment clears explicit membership. It preserves the parent IDs
/// only if those IDs are already exactly the requested numbered set.
pub(super) fn numbered_parent_count(governance: &GovernanceState) -> Option<u32> {
    let count = governance.active_validator_count;
    let ids = &governance.active_validators;
    (ids.is_empty()
        || (ids.len() == count as usize
            && ids
                .iter()
                .enumerate()
                .all(|(i, id)| *id == format!("validator-{i}"))))
    .then_some(count)
}

/// Ordinary validator-set amendments replace the members with the numbered local set.
/// A real membership change has no registry transition witness for the frozen checker.
fn changes_declared_numbered_set(a: &GovernanceAmendment) -> bool {
    a.kind == GOVERNANCE_KIND_VALIDATOR_SET
        && (a.value as usize != a.validators.len()
            || local_validator_ids(a.value).is_ok_and(|ids| ids != a.validators))
}

/// Block finality epochs start at one. The current Foundation signature verifier
/// counts prior updates from zero, and Cobalt forbids V2 authorizations outright.
/// Check the actual record against the guest's requirements in both paths.
fn checker_accepts_authorization_form(
    governance: &GovernanceState,
    update: &ValidatorRegistryUpdateRecord,
) -> bool {
    let signatures = &update.signed_authorizations;
    let Ok(quorum) = postfiat_ordering_fast::bft_quorum_threshold(update.validators.len()) else {
        return false;
    };
    if update.quorum < quorum
        || signatures.len() < quorum
        || signatures.len() != update.votes.len()
        || signatures.len() != update.support.len()
    {
        return false;
    }
    let finality_epoch = 1 + governance
        .validator_registry_updates
        .iter()
        .filter(|prior| prior.activation_height < update.activation_height)
        .count() as u64;
    signatures
        .iter()
        .all(|a| a.committee_epoch == finality_epoch && a.proposal_slot == update.activation_height)
}

fn verify_open_history(governance: &GovernanceState) -> Result<(), String> {
    let anchor = solana_exit::TRUSTED_CHECKPOINT_HEIGHT;
    for update in &governance.validator_registry_updates {
        // A Foundation update may have been ordered for a different activation height.
        if update.activation_height >= anchor
            || update
                .signed_authorizations
                .iter()
                .any(|a| a.proposal_slot >= anchor)
        {
            if update.activation_height == anchor
                || !checker_accepts_authorization_form(governance, update)
            {
                return Err(format!("Solana exit checker cannot cross registry update at height {} from checkpoint {anchor}", update.activation_height));
            }
        }
    }
    for record in governance
        .amendment_activation_records
        .iter()
        .filter(|r| r.activated_height >= anchor && r.kind == GOVERNANCE_KIND_VALIDATOR_SET)
    {
        if record.previous_value != record.new_value
            || governance
                .amendments
                .iter()
                .find(|a| a.amendment_id == record.amendment_id)
                .is_none_or(changes_declared_numbered_set)
        {
            return Err(format!(
                "Solana exit checker cannot cross validator-set amendment at height {}",
                record.activated_height
            ));
        }
    }
    Ok(())
}

/// Evaluate against the parent, before applying any action in this batch. This
/// is shared by proposal, commit (including catch-up), and full archive replay.
pub(super) fn verify_rotation_interlock(
    genesis: &Genesis,
    governance: &GovernanceState,
    ledger: &LedgerState,
    parent_numbered_count: Option<u32>,
    batch: &GovernanceActionBatch,
    height: u64,
) -> io::Result<()> {
    let updates = &batch.validator_registry_updates;
    let changes_set = batch
        .amendments
        .iter()
        .any(|a| a.kind == GOVERNANCE_KIND_VALIDATOR_SET && Some(a.value) != parent_numbered_count);
    if (updates.is_empty() && !changes_set)
        || solana_exit::release_height(&genesis.chain_id, &genesis_hash(genesis))
            .is_none_or(|h| height < h)
    {
        return Ok(());
    }
    if !changes_set
        && updates.len() == 1
        && crate::cobalt_handoff::governance_batch_action_count(batch) == 1
        && updates[0].activation_height == height
        && checker_accepts_authorization_form(governance, &updates[0])
    {
        return Ok(());
    }
    let reason = if solana_exit::open_height(governance).is_some()
        || batch.amendments.iter().any(|a| a.kind == solana_exit::OPEN)
    {
        "Solana burns are open or opening; close burns in a separate unanimous block"
    } else if ledger.vault_bridge_redemptions.iter().any(|r| {
        r.withdrawal_packet.source_chain_id == postfiat_types::SOLANA_EGRESS_PACKET_CHAIN
            && r.state == postfiat_types::VAULT_BRIDGE_REDEMPTION_STATE_PENDING
    }) {
        "Solana redemptions are pending; pay and settle every queued redemption"
    } else {
        return Ok(());
    };
    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!("Frozen Solana payout checker cannot follow this committee change: {reason}"),
    ))
}
