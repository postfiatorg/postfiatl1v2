use super::*;
use postfiat_types::solana_exit;

pub(super) fn registers_descriptor(batch: &GovernanceActionBatch) -> bool {
    batch
        .vault_bridge_route_profile_activations
        .iter()
        .any(|a| {
            a.amendment.kind.ends_with(solana_exit::PROFILE_HASH)
                && a.profile == solana_exit::descriptor()
        })
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
        || (registers_descriptor(batch)
            && solana_exit::release_height(&genesis.chain_id, &genesis_hash(genesis))
                .is_some_and(|h| h <= height))
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
    let validators = active_validator_ids(governance).map_err(|e| e.to_string())?;
    if amendment.validators != validators
        || amendment.quorum != validators.len()
        || amendment.signed_authorizations.len() != validators.len()
    {
        return Err("Solana exit decision requires every active validator signature".into());
    }
    if amendment.kind == solana_exit::OPEN {
        if solana_exit::open_height(governance).is_some()
            || height >= record.profile.expires_at_height
        {
            return Err("Solana exit burns already opened or descriptor expired".into());
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
    if amendment.kind == solana_exit::OPEN {
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
