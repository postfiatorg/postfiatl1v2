//! Account and issue settlement for the disposable L1 proof profile.
use super::*;
use postfiat_execution::{
    check_pq_recipient, prepare_pq_account_debit, prepare_pq_line_balance, prepare_pq_native_credit,
};
use postfiat_types::pq_payment::{Chain, Mode, PqPayment, PqPool};
use postfiat_types::{
    AssetDefinition, AssetOrchardEgressPolicyRegistry, VAULT_BRIDGE_BUCKET_STATUS_ACTIVE,
};

// Only cryptographic verification can be reused. The ordered-state tests run
// on every use, including a retry on the same thread at another height.
type Verified = (
    String,
    [u8; 48],
    u32,
    String,
    postfiat_privacy_binius::PaymentStatement,
);
static VERIFIED: std::sync::OnceLock<std::sync::Mutex<std::collections::VecDeque<Verified>>> =
    std::sync::OnceLock::new();

fn verified_cache() -> &'static std::sync::Mutex<std::collections::VecDeque<Verified>> {
    VERIFIED.get_or_init(|| std::sync::Mutex::new(std::collections::VecDeque::new()))
}

// A local scheduling limit applies to all callers, including peer proposals
// and archive replay. Waiting for a slot has no effect on the proof verdict.
static ACTIVE_VERIFIERS: std::sync::Mutex<usize> = std::sync::Mutex::new(0);
static SLOT_READY: std::sync::Condvar = std::sync::Condvar::new();
struct VerificationSlot;
impl VerificationSlot {
    fn acquire() -> Self {
        let active = ACTIVE_VERIFIERS.lock().unwrap_or_else(|e| e.into_inner());
        let mut active = SLOT_READY
            .wait_while(active, |n| *n >= postfiat_types::PQ_REHEARSAL_PROOFS)
            .unwrap_or_else(|e| e.into_inner());
        *active += 1;
        Self
    }
}
impl Drop for VerificationSlot {
    fn drop(&mut self) {
        *ACTIVE_VERIFIERS.lock().unwrap_or_else(|e| e.into_inner()) -= 1;
        SLOT_READY.notify_one();
    }
}

pub(super) fn verify_statement(
    payment: &PqPayment,
    chain: Chain,
) -> Result<postfiat_privacy_binius::PaymentStatement, postfiat_privacy_binius::VerifyError> {
    let digest = payment.digest();
    {
        let entries = verified_cache().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = entries.iter().find(|e| {
            e.0 == chain.id && e.1 == chain.genesis && e.2 == chain.protocol && e.3 == digest
        }) {
            return Ok(entry.4.clone());
        }
    }
    let _slot = VerificationSlot::acquire();
    #[cfg(test)]
    PROOF_VERIFICATIONS.with(|n| n.set(n.get() + 1));
    let statement = postfiat_privacy_binius::verify_l1(payment, chain)?;
    {
        let mut entries = verified_cache().lock().unwrap_or_else(|e| e.into_inner());
        if !entries.iter().any(|e| {
            e.0 == chain.id && e.1 == chain.genesis && e.2 == chain.protocol && e.3 == digest
        }) {
            if entries.len() >= postfiat_types::PQ_REHEARSAL_PROOFS * 4 {
                entries.pop_front();
            }
            entries.push_back((
                chain.id.to_owned(),
                chain.genesis,
                chain.protocol,
                digest,
                statement.clone(),
            ));
        }
    }
    Ok(statement)
}

#[cfg(test)]
pub(super) fn clear_verified() {
    verified_cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
}

#[cfg(test)]
thread_local! {
    static PROOF_VERIFICATIONS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Uncached proof verifications on this thread.
#[cfg(test)]
pub(super) fn proof_verifications() -> u64 {
    PROOF_VERIFICATIONS.with(std::cell::Cell::get)
}

fn enabled_issue<'a>(
    ledger: &'a LedgerState,
    genesis: &Genesis,
    governance: &GovernanceState,
    policies: &AssetOrchardEgressPolicyRegistry,
    height: u64,
    asset_id: &str,
    reserve: u64,
) -> Result<&'a AssetDefinition, &'static str> {
    if asset_id != postfiat_types::PQ_REHEARSAL_ISSUE {
        return Err("pq_issue_disabled");
    }
    let asset = ledger
        .asset_definition(asset_id)
        .ok_or("pq_issue_disabled")?;
    let family = ledger
        .asset_definition(&asset.asset_family_id)
        .ok_or("pq_issue_family")?;
    if !family.asset_family_id.is_empty()
        || asset.source_series_id != asset_id
        || asset.issuer != family.issuer
        || asset.precision != family.precision
        || asset.requires_authorization != family.requires_authorization
        || asset.freeze_enabled != family.freeze_enabled
        || asset.clawback_enabled != family.clawback_enabled
    {
        return Err("pq_issue_family");
    }
    if policies.record_at(asset_id, height).is_some()
        || policies.record_at(&family.asset_id, height).is_some()
        || policies.activated_at(height)
    {
        return Err("pq_unsupported_disclosure");
    }
    if ledger
        .nav_asset(&family.asset_id)
        .is_none_or(|nav| nav.halted || nav.issuer != asset.issuer)
    {
        return Err("pq_issue_halted");
    }
    let (policy_hash, _) = governance
        .active_vault_bridge_route_policy_hash(&family.asset_id, height)
        .map_err(|_| "pq_route_inactive")?;
    let route = governance
        .authorized_vault_bridge_route_profile(&family.asset_id, &policy_hash)
        .map_err(|_| "pq_route_inactive")?;
    if route.authorized_height > height
        || crate::execution_actions::validate_vault_bridge_route_profile_against_ledger(
            ledger,
            &route.profile,
            &policy_hash,
        )
        .is_err()
    {
        return Err("pq_route_inactive");
    }
    let bucket = ledger
        .vault_bridge_bucket_states
        .iter()
        .find(|b| b.bucket_id == asset.source_bucket_id)
        .ok_or("pq_issue_backing")?;
    if bucket.policy_hash != policy_hash
        || bucket.status != VAULT_BRIDGE_BUCKET_STATUS_ACTIVE
        || bucket.validate().is_err()
        || bucket.outstanding_vault_bridge_atoms < reserve
        || route
            .profile
            .bucket_source_series_id(&genesis.chain_id, bucket)
            .as_deref()
            != Ok(asset_id)
    {
        return Err("pq_issue_backing");
    }
    Ok(asset)
}

/// The activation check precedes this lookup at both block admission and settlement.
pub(super) fn fee_terms(
    governance: &GovernanceState,
    terms: &postfiat_types::pq_payment::Terms,
    inner_fee: u64,
    height: u64,
) -> Result<(u64, u64), &'static str> {
    let pft_minimum = MIN_TRANSFER_FEE.max(postfiat_types::PQ_REHEARSAL_MIN_FEE);
    if terms.asset == [0; 48] {
        return Ok((inner_fee, pft_minimum));
    }
    let id = bytes_to_hex(&terms.asset);
    if terms.mode == Mode::Shield {
        if id != postfiat_types::PQ_REHEARSAL_ISSUE
            && id != postfiat_types::PQ_ARCHIVE_FIXTURE_ISSUE
        {
            return Err("pq_issue_disabled");
        }
        return Ok((terms.public_fee, pft_minimum));
    }
    let required = governance.pq_issued_fee(&id, height).ok_or("pq_fee_unconfigured")?;
    if inner_fee != required {
        return Err("pq_fee_mismatch");
    }
    Ok((inner_fee, required))
}

/// Only the ratified governance batch calls this transfer. All recipient and
/// backing checks precede both writes, just as they do for a public exit.
pub(super) fn withdraw_fees(
    genesis: &Genesis,
    governance: &GovernanceState,
    ledger: &mut LedgerState,
    recipient: &str,
    amount: u64,
    height: u64,
) -> Result<(), &'static str> {
    let digest = genesis_hash(genesis);
    if !postfiat_types::pq_rehearsal_domain(&genesis.chain_id, &digest)
        || !governance.pq_rehearsal_active(
            &genesis.chain_id,
            &digest,
            genesis.protocol_version,
            height,
        )
    {
        return Err("pq_settlement_inactive");
    }
    let id = postfiat_types::PQ_REHEARSAL_ISSUE;
    let held = ledger
        .collected_private_fees
        .get(id)
        .copied()
        .ok_or("pq_fee_balance")?;
    let remaining = held.checked_sub(amount).ok_or("pq_fee_balance")?;
    let compatibility =
        crate::execution_actions::asset_execution_compatibility_for_genesis_and_governance(
            genesis, governance,
        );
    if !compatibility.pfusdc_source_series_active(height) {
        return Err("pq_issue_disabled");
    }
    let asset = enabled_issue(
        ledger,
        genesis,
        governance,
        &governance.asset_orchard_egress_policy_registry(),
        height,
        id,
        held,
    )?;
    check_pq_recipient(ledger, recipient, height, compatibility)?;
    let (index, balance) = prepare_pq_line_balance(ledger, asset, recipient, amount, true)?;
    ledger.trustlines[index].balance = balance;
    if remaining == 0 {
        ledger.collected_private_fees.remove(id);
    } else {
        ledger
            .collected_private_fees
            .insert(id.to_owned(), remaining);
    }
    Ok(())
}

pub(super) fn apply(
    genesis: &Genesis,
    genesis_digest: &str,
    ledger: &mut LedgerState,
    shielded: &mut ShieldedState,
    payment: &PqPayment,
    height: u64,
    compatibility: AssetExecutionCompatibility,
    governance: &GovernanceState,
    policies: &AssetOrchardEgressPolicyRegistry,
) -> Result<Receipt, &'static str> {
    if !postfiat_types::pq_rehearsal_domain(&genesis.chain_id, genesis_digest)
        || !governance.pq_rehearsal_active(
            &genesis.chain_id,
            genesis_digest,
            genesis.protocol_version,
            height,
        )
    {
        return Err("pq_settlement_inactive");
    }
    let (terms, _, inner) = payment.parts().map_err(|_| "pq_encoding")?;
    let chain = Chain {
        id: &genesis.chain_id,
        genesis: hex_to_bytes(genesis_digest)
            .map_err(|_| "pq_genesis")?
            .try_into()
            .map_err(|_| "pq_genesis")?,
        protocol: genesis.protocol_version,
    };
    let empty = PqPool::default();
    let pool = shielded.pq_pool_v1.as_ref().unwrap_or(&empty);
    let (next, receipt) = pq_pool_state::prepare_pair(pool, payment, chain, height)?;
    let pair = next.pending.back().ok_or("pq_missing_pair")?;
    let native = terms.asset == [0; 48];
    let (fee, base_minimum) = fee_terms(governance, &terms, pair.fee, height)?;
    let issued_spend = !native && terms.mode != Mode::Shield;
    let mut collector = None;
    let payer = terms
        .account
        .as_ref()
        .map(|auth| prepare_pq_account_debit(ledger, auth, if native { pair.mint } else { fee }))
        .transpose()?;
    let recipient = if terms.mode == Mode::Exit {
        let address = format!("pf{}", bytes_to_hex(&inner[84..104]));
        check_pq_recipient(ledger, &address, height, compatibility)?;
        Some(address)
    } else {
        None
    };
    let mut line = None;
    let mut credit = None;
    let mut creation_fee = 0;
    if native {
        if let Some(address) = &recipient {
            let (account, expansion) = prepare_pq_native_credit(ledger, address, pair.exit)?;
            credit = Some(account);
            creation_fee = expansion;
        }
    } else {
        if !compatibility.pfusdc_source_series_active(height) {
            return Err("pq_issue_disabled");
        }
        let held = ledger
            .collected_private_fees
            .get(&pair.asset)
            .copied()
            .unwrap_or(0);
        let next_held = held.checked_add(pair.fee).ok_or("pq_fee_balance")?;
        let previous_reserve = pool
            .head
            .assets
            .get(&pair.asset)
            .map_or(0, |asset| asset.reserve);
        let before = previous_reserve
            .checked_add(held)
            .ok_or("pq_issue_backing")?;
        let after = next.head.assets[&pair.asset]
            .reserve
            .checked_add(next_held)
            .ok_or("pq_issue_backing")?;
        let asset = enabled_issue(
            ledger,
            genesis,
            governance,
            policies,
            height,
            &pair.asset,
            before.max(after),
        )?;
        if issued_spend {
            collector = Some((pair.asset.clone(), next_held));
        }
        line = match terms.mode {
            Mode::Shield => Some(prepare_pq_line_balance(
                ledger,
                asset,
                &payer.as_ref().ok_or("pq_missing_source")?.address,
                pair.mint,
                false,
            )?),
            Mode::Exit => Some(prepare_pq_line_balance(
                ledger,
                asset,
                recipient.as_deref().ok_or("pq_missing_recipient")?,
                pair.exit,
                true,
            )?),
            Mode::Internal => None,
        };
    }
    let minimum = base_minimum
        .checked_add(creation_fee)
        .ok_or("pq_minimum_fee")?;
    if fee < minimum {
        return Err("pq_minimum_fee");
    }
    // These receipt fields feed chain-wide PFT totals. The pair records the asset fee.
    let (pft_fee, pft_minimum) = if issued_spend { (0, 0) } else { (fee, minimum) };
    let receipt = receipt.with_fee_policy_and_state_expansion(
        pft_fee,
        pft_fee,
        pft_minimum,
        ACCOUNT_RESERVE,
        creation_fee,
    );

    for account in [payer, credit].into_iter().flatten() {
        let slot = ledger.ensure_account(&account.address);
        *slot = account;
    }
    if let Some((index, balance)) = line {
        ledger.trustlines[index].balance = balance;
    }
    if let Some((asset, balance)) = collector {
        ledger.collected_private_fees.insert(asset, balance);
    }
    shielded.pq_pool_v1 = Some(next);
    Ok(receipt)
}
