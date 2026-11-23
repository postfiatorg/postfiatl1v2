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
thread_local! {
    static VERIFIED: std::cell::RefCell<std::collections::VecDeque<Verified>> = const { std::cell::RefCell::new(std::collections::VecDeque::new()) };
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
    VERIFIED.with_borrow_mut(|entries| {
        if let Some(entry) = entries.iter().find(|e| {
            e.0 == chain.id && e.1 == chain.genesis && e.2 == chain.protocol && e.3 == digest
        }) {
            return Ok(entry.4.clone());
        }
        let _slot = VerificationSlot::acquire();
        #[cfg(test)]
        PROOF_VERIFICATIONS.with(|n| n.set(n.get() + 1));
        let statement = postfiat_privacy_binius::verify_l1(payment, chain)?;
        if entries.len() == postfiat_types::PQ_REHEARSAL_PROOFS {
            entries.pop_front();
        }
        entries.push_back((
            chain.id.to_owned(),
            chain.genesis,
            chain.protocol,
            digest,
            statement.clone(),
        ));
        Ok(statement)
    })
}

#[cfg(test)]
pub(super) fn clear_verified() {
    VERIFIED.with_borrow_mut(|entries| entries.clear());
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
    if genesis_digest != pq_payment_fixture::GENESIS_HASH
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
    let fee = if native { pair.fee } else { terms.public_fee };
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
        let asset = enabled_issue(
            ledger,
            genesis,
            governance,
            policies,
            height,
            &pair.asset,
            next.head.assets[&pair.asset].reserve.max(
                pool.head
                    .assets
                    .get(&pair.asset)
                    .map_or(0, |asset| asset.reserve),
            ),
        )?;
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
    let minimum = MIN_TRANSFER_FEE + creation_fee;
    if fee < minimum {
        return Err("pq_minimum_fee");
    }
    let receipt = receipt.with_fee_policy_and_state_expansion(
        fee,
        fee,
        minimum,
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
    shielded.pq_pool_v1 = Some(next);
    Ok(receipt)
}
