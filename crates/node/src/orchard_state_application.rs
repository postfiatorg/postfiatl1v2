fn apply_verified_orchard_action_to_shielded_state(
    genesis: &Genesis,
    shielded: &mut ShieldedState,
    action: &OrchardShieldedAction,
    verified: &VerifiedOrchardBundle,
) -> io::Result<Receipt> {
    let exit_policy = match orchard_fee_burn_amount_for_apply(genesis, shielded, action, verified)?
    {
        Ok(policy) => policy,
        Err(receipt) => return Ok(receipt),
    };
    apply_verified_orchard_action_to_shielded_state_with_exit_policy(
        genesis,
        shielded,
        action,
        verified,
        exit_policy,
    )
}

fn apply_verified_orchard_action_to_shielded_state_with_exit_policy(
    genesis: &Genesis,
    shielded: &mut ShieldedState,
    action: &OrchardShieldedAction,
    verified: &VerifiedOrchardBundle,
    exit_policy: OrchardValueExitPolicy,
) -> io::Result<Receipt> {
    apply_verified_orchard_action_to_shielded_state_with_exit_policy_and_deposit_credit(
        genesis,
        shielded,
        action,
        verified,
        exit_policy,
        0,
    )
}

fn apply_verified_orchard_action_to_shielded_state_with_exit_policy_and_deposit_credit(
    genesis: &Genesis,
    shielded: &mut ShieldedState,
    action: &OrchardShieldedAction,
    verified: &VerifiedOrchardBundle,
    exit_policy: OrchardValueExitPolicy,
    deposit_budget_credit: u64,
) -> io::Result<Receipt> {
    if action.pool_id.trim().is_empty() {
        return Ok(Receipt::rejected(
            orchard_action_receipt_id(genesis, action, verified, "empty_pool_id")?,
            "empty_pool_id",
            "Orchard action pool id is empty",
        ));
    }

    if has_duplicate_strings(
        verified
            .nullifiers
            .iter()
            .map(|nullifier| nullifier.as_hex()),
    ) {
        return Ok(Receipt::rejected(
            orchard_action_receipt_id(genesis, action, verified, "duplicate_nullifier")?,
            "duplicate_nullifier",
            "Orchard action contains duplicate nullifiers",
        ));
    }
    if has_duplicate_strings(
        verified
            .output_commitments
            .iter()
            .map(|commitment| commitment.as_hex()),
    ) {
        return Ok(Receipt::rejected(
            orchard_action_receipt_id(genesis, action, verified, "duplicate_output_commitment")?,
            "duplicate_output_commitment",
            "Orchard action contains duplicate output commitments",
        ));
    }
    let action_anchor = verified.anchor.as_hex().to_string();
    if let Some(pool) = shielded.orchard.as_ref() {
        if pool.pool_id != action.pool_id {
            return Ok(Receipt::rejected(
                orchard_action_receipt_id(genesis, action, verified, "pool_id_mismatch")?,
                "pool_id_mismatch",
                format!(
                    "Orchard pool state is for `{}`, action is for `{}`",
                    pool.pool_id, action.pool_id
                ),
            ));
        }

        if let Some(nullifier) = verified
            .nullifiers
            .iter()
            .map(|nullifier| nullifier.as_hex())
            .find(|nullifier| pool.is_nullified(nullifier))
        {
            return Ok(Receipt::rejected(
                orchard_action_receipt_id(genesis, action, verified, "duplicate_nullifier")?,
                "duplicate_nullifier",
                format!("Orchard nullifier `{nullifier}` already exists"),
            ));
        }
        if let Some(commitment) = verified
            .output_commitments
            .iter()
            .map(|commitment| commitment.as_hex())
            .find(|commitment| {
                pool.output_commitments
                    .iter()
                    .any(|existing| existing == commitment)
            })
        {
            return Ok(Receipt::rejected(
                orchard_action_receipt_id(
                    genesis,
                    action,
                    verified,
                    "duplicate_output_commitment",
                )?,
                "duplicate_output_commitment",
                format!("Orchard output commitment `{commitment}` already exists"),
            ));
        }

        if !orchard_anchor_is_retained_for_apply(pool, &action_anchor)? {
            return Ok(Receipt::rejected(
                orchard_action_receipt_id(genesis, action, verified, "unretained_orchard_anchor")?,
                "unretained_orchard_anchor",
                format!("Orchard anchor `{action_anchor}` is not retained by pool state"),
            ));
        }
    } else if action_anchor != orchard_empty_root_hex() {
        return Ok(Receipt::rejected(
            orchard_action_receipt_id(genesis, action, verified, "unretained_orchard_anchor")?,
            "unretained_orchard_anchor",
            format!("Orchard anchor `{action_anchor}` is not the empty Orchard root"),
        ));
    }

    let turnstile_deposit_amount =
        orchard_turnstile_deposit_amount(verified.value_balance).map_err(invalid_data)?;
    if turnstile_deposit_amount > 0 {
        let budget_total = orchard_turnstile_budget_total(shielded, &action.pool_id)?
            .checked_add(deposit_budget_credit)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Orchard turnstile budget total overflow",
                )
            })?;
        let consumed = shielded
            .orchard
            .as_ref()
            .map(|pool| pool.turnstile_deposit_total)
            .unwrap_or(0);
        if consumed > budget_total {
            return Ok(Receipt::rejected(
                orchard_action_receipt_id(
                    genesis,
                    action,
                    verified,
                    "turnstile_accounting_invalid",
                )?,
                "turnstile_accounting_invalid",
                format!(
                    "Orchard pool consumed {consumed} turnstile units but only {budget_total} units are recorded"
                ),
            ));
        }
        let available = budget_total - consumed;
        if turnstile_deposit_amount > available {
            return Ok(Receipt::rejected(
                orchard_action_receipt_id(
                    genesis,
                    action,
                    verified,
                    "turnstile_insufficient_deposit",
                )?,
                "turnstile_insufficient_deposit",
                format!(
                    "Orchard action requires {turnstile_deposit_amount} turnstile units but only {available} are available"
                ),
            ));
        }
    }

    let pool = shielded
        .orchard
        .get_or_insert_with(|| OrchardPoolState::empty(action.pool_id.clone()));
    ensure_orchard_root_history_for_apply(pool)?;
    for nullifier in &verified.nullifiers {
        pool.nullifiers.push(nullifier.as_hex().to_string());
    }
    for commitment in &verified.output_commitments {
        pool.output_commitments
            .push(commitment.as_hex().to_string());
    }
    for output in &verified.encrypted_outputs {
        pool.encrypted_outputs
            .push(orchard_output_record_from_verified(output));
    }
    if !pool
        .accepted_anchors
        .iter()
        .any(|existing| existing == &action_anchor)
    {
        pool.accepted_anchors.push(action_anchor);
    }
    if turnstile_deposit_amount > 0 {
        pool.turnstile_deposit_total = pool
            .turnstile_deposit_total
            .checked_add(turnstile_deposit_amount)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Orchard turnstile deposit total overflow",
                )
            })?;
        pool.value_balance_total = pool
            .value_balance_total
            .checked_add(verified.value_balance)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Orchard value balance total overflow",
                )
            })?;
    }
    if exit_policy.fee_burned > 0 {
        pool.fee_burn_total = pool
            .fee_burn_total
            .checked_add(exit_policy.fee_burned)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Orchard fee burn total overflow",
                )
            })?;
    }
    if exit_policy.withdrawn > 0 {
        pool.withdraw_total = pool
            .withdraw_total
            .checked_add(exit_policy.withdrawn)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Orchard withdraw total overflow",
                )
            })?;
    }
    if verified.value_balance != 0 && turnstile_deposit_amount == 0 {
        pool.value_balance_total = pool
            .value_balance_total
            .checked_add(verified.value_balance)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Orchard value balance total overflow",
                )
            })?;
    }
    append_orchard_current_root(pool)?;

    let mut receipt = Receipt::accepted(
        orchard_action_receipt_id(genesis, action, verified, "accepted")?,
        "Orchard shielded action verified and public pool state updated",
    );
    if exit_policy.fee_burned > 0
        || exit_policy.minimum_fee > 0
        || exit_policy.state_expansion_fee > 0
    {
        receipt = receipt.with_fee_policy_and_state_expansion(
            action.fee,
            exit_policy.fee_burned,
            exit_policy.minimum_fee,
            ACCOUNT_RESERVE,
            exit_policy.state_expansion_fee,
        );
    }
    Ok(receipt)
}

fn orchard_empty_root_hex() -> String {
    orchard_empty_anchor().as_hex().to_string()
}

fn orchard_empty_root_record() -> OrchardRootRecord {
    OrchardRootRecord {
        root: orchard_empty_root_hex(),
        output_count: 0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OrchardValueExitPolicy {
    fee_burned: u64,
    withdrawn: u64,
    minimum_fee: u64,
    state_expansion_fee: u64,
}

fn orchard_fee_burn_amount_for_apply(
    genesis: &Genesis,
    shielded: &ShieldedState,
    action: &OrchardShieldedAction,
    verified: &VerifiedOrchardBundle,
) -> io::Result<Result<OrchardValueExitPolicy, Receipt>> {
    if action.fee == 0 {
        if verified.value_balance > 0 {
            return Ok(Err(Receipt::rejected(
                orchard_action_receipt_id(
                    genesis,
                    action,
                    verified,
                    "orchard_withdraw_unsupported",
                )?,
                "orchard_withdraw_unsupported",
                "positive Orchard value balance requires fee or withdraw accounting",
            )));
        }
        return Ok(Ok(OrchardValueExitPolicy {
            fee_burned: 0,
            withdrawn: 0,
            minimum_fee: 0,
            state_expansion_fee: 0,
        }));
    }
    if verified.value_balance <= 0 {
        return Ok(Err(Receipt::rejected(
            orchard_action_receipt_id(genesis, action, verified, "orchard_fee_mismatch")?,
            "orchard_fee_mismatch",
            "nonzero Orchard fee requires a matching positive value balance",
        )));
    }
    let positive_value_balance = u64::try_from(verified.value_balance).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "positive Orchard value balance does not fit u64",
        )
    })?;
    if positive_value_balance != action.fee {
        return Ok(Err(Receipt::rejected(
            orchard_action_receipt_id(genesis, action, verified, "orchard_fee_mismatch")?,
            "orchard_fee_mismatch",
            format!(
                "Orchard fee {} does not match positive value balance {}",
                action.fee, positive_value_balance
            ),
        )));
    }
    let minimum_fee = orchard_minimum_fee_for_action(action, verified);
    if action.fee < minimum_fee {
        return Ok(Err(Receipt::rejected(
            orchard_action_receipt_id(genesis, action, verified, "orchard_fee_too_low")?,
            "orchard_fee_too_low",
            format!("minimum Orchard fee is {minimum_fee}"),
        )
        .with_fee_policy(0, 0, minimum_fee, 0)));
    }
    let issued_value = shielded
        .orchard
        .as_ref()
        .map(orchard_pool_issued_value)
        .transpose()?
        .unwrap_or(0);
    if positive_value_balance > issued_value {
        return Ok(Err(Receipt::rejected(
            orchard_action_receipt_id(genesis, action, verified, "orchard_fee_exceeds_pool")?,
            "orchard_fee_exceeds_pool",
            format!(
                "Orchard fee {positive_value_balance} exceeds issued pool value {issued_value}"
            ),
        )));
    }
    Ok(Ok(OrchardValueExitPolicy {
        fee_burned: positive_value_balance,
        withdrawn: 0,
        minimum_fee,
        state_expansion_fee: 0,
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OrchardWithdrawApplyPlan {
    exit_policy: OrchardValueExitPolicy,
    recipient_after_credit: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OrchardDepositApplyPlan {
    turnstile_event: TurnstileEvent,
    funding_transfer_id: String,
    funding_transfer_fee: u64,
    total_burn: u64,
    minimum_fee: u64,
}

fn orchard_deposit_amount_for_apply(
    genesis: &Genesis,
    action: &OrchardShieldedAction,
    verified: &VerifiedOrchardBundle,
    payload: &OrchardDepositActionPayload,
) -> io::Result<Result<OrchardDepositApplyPlan, Receipt>> {
    if let Err(error) = validate_orchard_deposit_payload(payload) {
        return Ok(Err(Receipt::rejected(
            orchard_action_receipt_id(genesis, action, verified, "orchard_deposit_bad_payload")?,
            "orchard_deposit_bad_payload",
            error.to_string(),
        )));
    }
    if action.fee != 0 {
        return Ok(Err(Receipt::rejected(
            orchard_action_receipt_id(genesis, action, verified, "orchard_deposit_fee_mismatch")?,
            "orchard_deposit_fee_mismatch",
            "Orchard deposit action fee must be zero",
        )));
    }
    if payload.funding_transfer.unsigned.to != FEE_COLLECTOR_ADDRESS {
        return Ok(Err(Receipt::rejected(
            orchard_action_receipt_id(
                genesis,
                action,
                verified,
                "orchard_deposit_bad_funding_target",
            )?,
            "orchard_deposit_bad_funding_target",
            "Orchard deposit funding transfer must target the transparent burn sink",
        )));
    }
    let expected_funding_amount = payload.amount.checked_add(payload.fee).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Orchard deposit amount plus fee overflowed",
        )
    })?;
    if payload.funding_transfer.unsigned.amount != expected_funding_amount {
        return Ok(Err(Receipt::rejected(
            orchard_action_receipt_id(
                genesis,
                action,
                verified,
                "orchard_deposit_funding_amount_mismatch",
            )?,
            "orchard_deposit_funding_amount_mismatch",
            format!(
                "Orchard deposit funding amount {} does not match amount {} plus fee {}",
                payload.funding_transfer.unsigned.amount, payload.amount, payload.fee
            ),
        )));
    }
    let funding_transfer_id = transfer_tx_id(&payload.funding_transfer);
    let expected_binding =
        orchard_deposit_external_binding_hash(OrchardDepositExternalBindingInput {
            genesis,
            pool_id: &action.pool_id,
            funding_transfer_id: &funding_transfer_id,
            from_address: &payload.funding_transfer.unsigned.from,
            amount: payload.amount,
            fee: payload.fee,
            policy_id: &payload.policy_id,
            disclosure_hash: &payload.disclosure_hash,
        })?;
    if action.external_binding_hash.as_deref() != Some(expected_binding.as_str()) {
        return Ok(Err(Receipt::rejected(
            orchard_action_receipt_id(
                genesis,
                action,
                verified,
                "orchard_deposit_external_binding_mismatch",
            )?,
            "orchard_deposit_external_binding_mismatch",
            "Orchard deposit action is not bound to the transparent funding envelope",
        )));
    }
    let deposit_value = orchard_turnstile_deposit_amount(verified.value_balance)?;
    if deposit_value != payload.amount {
        return Ok(Err(Receipt::rejected(
            orchard_action_receipt_id(genesis, action, verified, "orchard_deposit_value_mismatch")?,
            "orchard_deposit_value_mismatch",
            format!(
                "Orchard deposit value {deposit_value} does not match payload amount {}",
                payload.amount
            ),
        )));
    }
    let minimum_fee = orchard_minimum_resource_fee_for_action(action);
    if payload.fee < minimum_fee {
        return Ok(Err(Receipt::rejected(
            orchard_action_receipt_id(genesis, action, verified, "orchard_deposit_fee_too_low")?,
            "orchard_deposit_fee_too_low",
            format!("minimum Orchard deposit fee is {minimum_fee}"),
        )
        .with_fee_policy(
            0,
            0,
            minimum_fee.saturating_add(payload.funding_transfer.unsigned.fee),
            0,
        )));
    }
    let mut event = TurnstileEvent {
        event_id: String::new(),
        kind: TURNSTILE_KIND_ORCHARD_DEPOSIT.to_string(),
        owner: payload.funding_transfer.unsigned.from.clone(),
        asset_id: DEFAULT_SHIELDED_ASSET_ID.to_string(),
        amount: payload.amount,
        note_id: funding_transfer_id.clone(),
        source_pool: TRANSPARENT_BOOTSTRAP_POOL_ID.to_string(),
        target_pool: action.pool_id.clone(),
    };
    event.event_id = orchard_deposit_turnstile_event_id(&event)?;
    let total_burn = payload
        .amount
        .checked_add(payload.fee)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "deposit burn overflow"))?;
    Ok(Ok(OrchardDepositApplyPlan {
        turnstile_event: event,
        funding_transfer_id,
        funding_transfer_fee: payload.funding_transfer.unsigned.fee,
        total_burn,
        minimum_fee,
    }))
}

fn orchard_withdraw_amount_for_apply(
    genesis: &Genesis,
    ledger: &LedgerState,
    shielded: &ShieldedState,
    action: &OrchardShieldedAction,
    verified: &VerifiedOrchardBundle,
    payload: &OrchardWithdrawActionPayload,
) -> io::Result<Result<OrchardWithdrawApplyPlan, Receipt>> {
    if let Err(error) = validate_orchard_withdraw_payload(payload) {
        return Ok(Err(Receipt::rejected(
            orchard_action_receipt_id(genesis, action, verified, "orchard_withdraw_bad_payload")?,
            "orchard_withdraw_bad_payload",
            error.to_string(),
        )));
    }
    if payload.fee != action.fee {
        return Ok(Err(Receipt::rejected(
            orchard_action_receipt_id(genesis, action, verified, "orchard_withdraw_fee_mismatch")?,
            "orchard_withdraw_fee_mismatch",
            format!(
                "Orchard withdraw payload fee {} does not match action fee {}",
                payload.fee, action.fee
            ),
        )));
    }
    let expected_binding = orchard_withdraw_external_binding_hash(
        genesis,
        &action.pool_id,
        &payload.to,
        payload.amount,
        payload.fee,
        &payload.policy_id,
        &payload.disclosure_hash,
    )?;
    if action.external_binding_hash.as_deref() != Some(expected_binding.as_str()) {
        return Ok(Err(Receipt::rejected(
            orchard_action_receipt_id(
                genesis,
                action,
                verified,
                "orchard_withdraw_external_binding_mismatch",
            )?,
            "orchard_withdraw_external_binding_mismatch",
            "Orchard withdraw action is not bound to the transparent withdrawal envelope",
        )));
    }
    if verified.value_balance <= 0 {
        return Ok(Err(Receipt::rejected(
            orchard_action_receipt_id(
                genesis,
                action,
                verified,
                "orchard_withdraw_value_mismatch",
            )?,
            "orchard_withdraw_value_mismatch",
            "Orchard withdraw requires positive value balance",
        )));
    }
    let positive_value_balance = u64::try_from(verified.value_balance).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "positive Orchard withdraw value balance does not fit u64",
        )
    })?;
    let expected_exit = payload.amount.checked_add(payload.fee).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Orchard withdraw amount plus fee overflowed",
        )
    })?;
    if positive_value_balance != expected_exit {
        return Ok(Err(Receipt::rejected(
            orchard_action_receipt_id(genesis, action, verified, "orchard_withdraw_value_mismatch")?,
            "orchard_withdraw_value_mismatch",
            format!(
                "Orchard withdraw value balance {positive_value_balance} does not match amount {} plus fee {}",
                payload.amount, payload.fee
            ),
        )));
    }
    let state_expansion_fee = orchard_withdraw_state_expansion_fee(ledger, &payload.to);
    let minimum_fee =
        orchard_minimum_fee_for_action(action, verified).saturating_add(state_expansion_fee);
    if payload.fee < minimum_fee {
        return Ok(Err(Receipt::rejected(
            orchard_action_receipt_id(genesis, action, verified, "orchard_withdraw_fee_too_low")?,
            "orchard_withdraw_fee_too_low",
            format!("minimum Orchard withdraw fee is {minimum_fee}"),
        )
        .with_fee_policy_and_state_expansion(
            0,
            0,
            minimum_fee,
            ACCOUNT_RESERVE,
            state_expansion_fee,
        )));
    }
    let issued_value = shielded
        .orchard
        .as_ref()
        .map(orchard_pool_issued_value)
        .transpose()?
        .unwrap_or(0);
    if positive_value_balance > issued_value {
        return Ok(Err(Receipt::rejected(
            orchard_action_receipt_id(genesis, action, verified, "orchard_withdraw_exceeds_pool")?,
            "orchard_withdraw_exceeds_pool",
            format!(
                "Orchard withdraw exit {positive_value_balance} exceeds issued pool value {issued_value}"
            ),
        )));
    }
    let recipient_base = ledger
        .account(&payload.to)
        .map(|account| account.balance)
        .unwrap_or_default();
    let Some(recipient_after_credit) = recipient_base.checked_add(payload.amount) else {
        return Ok(Err(Receipt::rejected(
            orchard_action_receipt_id(genesis, action, verified, "balance_overflow")?,
            "balance_overflow",
            "Orchard withdraw recipient balance would overflow",
        )
        .with_fee_policy_and_state_expansion(
            0,
            0,
            minimum_fee,
            ACCOUNT_RESERVE,
            state_expansion_fee,
        )));
    };
    if recipient_after_credit < ACCOUNT_RESERVE {
        return Ok(Err(Receipt::rejected(
            orchard_action_receipt_id(genesis, action, verified, "below_account_reserve")?,
            "below_account_reserve",
            format!(
                "recipient `{}` final balance {recipient_after_credit} is below reserve {ACCOUNT_RESERVE}",
                payload.to
            ),
        )
        .with_fee_policy_and_state_expansion(
            0,
            0,
            minimum_fee,
            ACCOUNT_RESERVE,
            state_expansion_fee,
        )));
    }

    Ok(Ok(OrchardWithdrawApplyPlan {
        exit_policy: OrchardValueExitPolicy {
            fee_burned: payload.fee,
            withdrawn: payload.amount,
            minimum_fee,
            state_expansion_fee,
        },
        recipient_after_credit,
    }))
}

fn orchard_withdraw_state_expansion_fee(ledger: &LedgerState, to: &str) -> u64 {
    if ledger.account(to).is_some() {
        0
    } else {
        TRANSFER_ACCOUNT_CREATION_FEE
    }
}

fn apply_verified_orchard_withdraw_action_to_state(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    shielded: &mut ShieldedState,
    action: &OrchardShieldedAction,
    verified: &VerifiedOrchardBundle,
    payload: &OrchardWithdrawActionPayload,
) -> io::Result<Receipt> {
    let plan = match orchard_withdraw_amount_for_apply(
        genesis, ledger, shielded, action, verified, payload,
    )? {
        Ok(plan) => plan,
        Err(receipt) => return Ok(receipt),
    };
    let receipt = apply_verified_orchard_action_to_shielded_state_with_exit_policy(
        genesis,
        shielded,
        action,
        verified,
        plan.exit_policy,
    )?;
    if receipt.accepted {
        let recipient = ledger.ensure_account(&payload.to);
        recipient.balance = plan.recipient_after_credit;
    }
    Ok(receipt)
}

fn apply_verified_orchard_deposit_action_to_state(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    shielded: &mut ShieldedState,
    action: &OrchardShieldedAction,
    verified: &VerifiedOrchardBundle,
    payload: &OrchardDepositActionPayload,
) -> io::Result<Receipt> {
    let plan = match orchard_deposit_amount_for_apply(genesis, action, verified, payload)? {
        Ok(plan) => plan,
        Err(receipt) => return Ok(receipt),
    };
    if shielded
        .turnstile_events
        .iter()
        .any(|event| event.event_id == plan.turnstile_event.event_id)
    {
        return Ok(Receipt::rejected(
            orchard_action_receipt_id(genesis, action, verified, "duplicate_turnstile_deposit")?,
            "duplicate_turnstile_deposit",
            format!(
                "Orchard deposit funding transfer `{}` is already recorded",
                plan.funding_transfer_id
            ),
        ));
    }

    let mut trial_ledger = ledger.clone();
    let funding_receipt = execute_transfer(genesis, &mut trial_ledger, &payload.funding_transfer);
    if !funding_receipt.accepted {
        return Ok(Receipt::rejected(
            orchard_action_receipt_id(
                genesis,
                action,
                verified,
                "orchard_deposit_funding_rejected",
            )?,
            "orchard_deposit_funding_rejected",
            format!(
                "Orchard deposit funding transfer rejected with {}: {}",
                funding_receipt.code, funding_receipt.message
            ),
        )
        .with_fee_policy(
            0,
            0,
            plan.minimum_fee.saturating_add(plan.funding_transfer_fee),
            0,
        ));
    }
    let Some(sink) = trial_ledger.account_mut(FEE_COLLECTOR_ADDRESS) else {
        return Ok(Receipt::rejected(
            orchard_action_receipt_id(genesis, action, verified, "orchard_deposit_sink_missing")?,
            "orchard_deposit_sink_missing",
            "Orchard deposit funding sink missing after accepted funding transfer",
        ));
    };
    if sink.balance < plan.total_burn {
        return Ok(Receipt::rejected(
            orchard_action_receipt_id(genesis, action, verified, "orchard_deposit_sink_underflow")?,
            "orchard_deposit_sink_underflow",
            "Orchard deposit funding sink balance is below burn amount",
        ));
    }
    sink.balance -= plan.total_burn;

    let mut trial_shielded = shielded.clone();
    let mut receipt =
        apply_verified_orchard_action_to_shielded_state_with_exit_policy_and_deposit_credit(
            genesis,
            &mut trial_shielded,
            action,
            verified,
            OrchardValueExitPolicy {
                fee_burned: 0,
                withdrawn: 0,
                minimum_fee: 0,
                state_expansion_fee: 0,
            },
            payload.amount,
        )?;
    if !receipt.accepted {
        return Ok(receipt);
    }
    trial_shielded.turnstile_events.push(plan.turnstile_event);
    verify_shielded_state(&trial_shielded)?;

    receipt = receipt.with_fee_policy(
        payload.fee.saturating_add(plan.funding_transfer_fee),
        payload.fee.saturating_add(plan.funding_transfer_fee),
        plan.minimum_fee.saturating_add(plan.funding_transfer_fee),
        0,
    );
    *ledger = trial_ledger;
    *shielded = trial_shielded;
    Ok(receipt)
}

fn apply_verified_shielded_swap_action_to_state(
    genesis: &Genesis,
    shielded: &mut ShieldedState,
    action: &ShieldedSwapAction,
    verified: &VerifiedShieldedSwap,
) -> io::Result<Receipt> {
    if action.pool_id.trim().is_empty() {
        return Ok(Receipt::rejected(
            shielded_swap_receipt_id(genesis, action, verified, "empty_pool_id")?,
            "empty_pool_id",
            "shielded swap pool id is empty",
        ));
    }
    if verified.fee != 0 {
        return Ok(Receipt::rejected(
            shielded_swap_receipt_id(genesis, action, verified, "unsupported_shielded_swap_fee")?,
            "unsupported_shielded_swap_fee",
            "shielded swap v1 requires fee 0 until fee burn accounting is specified",
        ));
    }
    if has_duplicate_strings(
        verified
            .nullifiers
            .iter()
            .map(|nullifier| nullifier.as_hex()),
    ) {
        return Ok(Receipt::rejected(
            shielded_swap_receipt_id(genesis, action, verified, "duplicate_nullifier")?,
            "duplicate_nullifier",
            "shielded swap contains duplicate nullifiers",
        ));
    }
    if has_duplicate_strings(
        verified
            .output_commitments
            .iter()
            .map(|commitment| commitment.as_hex()),
    ) {
        return Ok(Receipt::rejected(
            shielded_swap_receipt_id(genesis, action, verified, "duplicate_output_commitment")?,
            "duplicate_output_commitment",
            "shielded swap contains duplicate output commitments",
        ));
    }

    let action_anchor = verified.anchor.as_hex().to_string();
    if let Some(pool) = shielded.orchard.as_ref() {
        if pool.pool_id != action.pool_id {
            return Ok(Receipt::rejected(
                shielded_swap_receipt_id(genesis, action, verified, "pool_id_mismatch")?,
                "pool_id_mismatch",
                format!(
                    "Orchard pool state is for `{}`, shielded swap is for `{}`",
                    pool.pool_id, action.pool_id
                ),
            ));
        }
        if let Some(nullifier) = verified
            .nullifiers
            .iter()
            .map(|nullifier| nullifier.as_hex())
            .find(|nullifier| pool.is_nullified(nullifier))
        {
            return Ok(Receipt::rejected(
                shielded_swap_receipt_id(genesis, action, verified, "duplicate_nullifier")?,
                "duplicate_nullifier",
                format!("shielded swap nullifier `{nullifier}` already exists"),
            ));
        }
        if let Some(commitment) = verified
            .output_commitments
            .iter()
            .map(|commitment| commitment.as_hex())
            .find(|commitment| {
                pool.output_commitments
                    .iter()
                    .any(|existing| existing == commitment)
            })
        {
            return Ok(Receipt::rejected(
                shielded_swap_receipt_id(genesis, action, verified, "duplicate_output_commitment")?,
                "duplicate_output_commitment",
                format!("shielded swap output commitment `{commitment}` already exists"),
            ));
        }
        if !orchard_anchor_is_retained_for_apply(pool, &action_anchor)? {
            return Ok(Receipt::rejected(
                shielded_swap_receipt_id(genesis, action, verified, "unretained_orchard_anchor")?,
                "unretained_orchard_anchor",
                format!("shielded swap anchor `{action_anchor}` is not retained by pool state"),
            ));
        }
    } else if action_anchor != orchard_empty_root_hex() {
        return Ok(Receipt::rejected(
            shielded_swap_receipt_id(genesis, action, verified, "unretained_orchard_anchor")?,
            "unretained_orchard_anchor",
            format!("shielded swap anchor `{action_anchor}` is not the empty Orchard root"),
        ));
    }

    let pool = shielded
        .orchard
        .get_or_insert_with(|| OrchardPoolState::empty(action.pool_id.clone()));
    ensure_orchard_root_history_for_apply(pool)?;
    for nullifier in &verified.nullifiers {
        pool.nullifiers.push(nullifier.as_hex().to_string());
    }
    for commitment in &verified.output_commitments {
        pool.output_commitments
            .push(commitment.as_hex().to_string());
    }
    for output in &verified.encrypted_outputs {
        pool.encrypted_outputs
            .push(orchard_output_record_from_verified(output));
    }
    for ((output_commitment, asset_commitment), value_commitment) in verified
        .output_commitments
        .iter()
        .zip(verified.output_asset_commitments.iter())
        .zip(verified.output_value_commitments.iter())
    {
        pool.asset_commitment_records
            .push(OrchardAssetCommitmentRecord {
                output_commitment: output_commitment.as_hex().to_string(),
                asset_commitment: asset_commitment.as_hex().to_string(),
                value_commitment: value_commitment.as_hex().to_string(),
            });
    }
    if !pool
        .accepted_anchors
        .iter()
        .any(|existing| existing == &action_anchor)
    {
        pool.accepted_anchors.push(action_anchor);
    }
    append_orchard_current_root(pool)?;

    verify_shielded_state(shielded)?;
    Ok(Receipt::accepted(
        shielded_swap_receipt_id(genesis, action, verified, "accepted")?,
        "shielded swap verified and public pool state updated",
    ))
}

fn apply_verified_asset_orchard_swap_action_to_state(
    genesis: &Genesis,
    shielded: &mut ShieldedState,
    action: &AssetOrchardSwapAction,
    verified: &VerifiedAssetOrchardSwap,
) -> io::Result<Receipt> {
    if action.pool_id.trim().is_empty() {
        return Ok(Receipt::rejected(
            asset_orchard_swap_receipt_id(genesis, action, verified, "empty_pool_id")?,
            "empty_pool_id",
            "asset-orchard swap pool id is empty",
        ));
    }
    if verified.fee != 0 {
        return Ok(Receipt::rejected(
            asset_orchard_swap_receipt_id(
                genesis,
                action,
                verified,
                "unsupported_asset_orchard_fee",
            )?,
            "unsupported_asset_orchard_fee",
            "asset-orchard swap v1 requires fee 0",
        ));
    }
    if has_duplicate_strings(verified.nullifiers.iter().map(|nullifier| nullifier.as_hex())) {
        return Ok(Receipt::rejected(
            asset_orchard_swap_receipt_id(genesis, action, verified, "duplicate_nullifier")?,
            "duplicate_nullifier",
            "asset-orchard swap contains duplicate nullifiers",
        ));
    }
    if has_duplicate_strings(
        verified
            .output_commitments
            .iter()
            .map(|commitment| commitment.as_hex()),
    ) {
        return Ok(Receipt::rejected(
            asset_orchard_swap_receipt_id(
                genesis,
                action,
                verified,
                "duplicate_output_commitment",
            )?,
            "duplicate_output_commitment",
            "asset-orchard swap contains duplicate output commitments",
        ));
    }
    if let Err((code, message)) = validate_asset_orchard_swap_accounting(action, verified) {
        return Ok(Receipt::rejected(
            asset_orchard_swap_receipt_id(genesis, action, verified, code)?,
            code,
            message,
        ));
    }

    let action_anchor = verified.anchor.as_hex().to_string();
    if let Some(pool) = shielded.orchard.as_ref() {
        if pool.pool_id != action.pool_id {
            return Ok(Receipt::rejected(
                asset_orchard_swap_receipt_id(genesis, action, verified, "pool_id_mismatch")?,
                "pool_id_mismatch",
                format!(
                    "Orchard pool state is for `{}`, asset-orchard swap is for `{}`",
                    pool.pool_id, action.pool_id
                ),
            ));
        }
        if let Some(nullifier) = verified
            .nullifiers
            .iter()
            .map(|nullifier| nullifier.as_hex())
            .find(|nullifier| pool.is_nullified(nullifier))
        {
            return Ok(Receipt::rejected(
                asset_orchard_swap_receipt_id(genesis, action, verified, "duplicate_nullifier")?,
                "duplicate_nullifier",
                format!("asset-orchard swap nullifier `{nullifier}` already exists"),
            ));
        }
        if let Some(commitment) = verified
            .output_commitments
            .iter()
            .map(|commitment| commitment.as_hex())
            .find(|commitment| {
                pool.output_commitments
                    .iter()
                    .any(|existing| existing == commitment)
            })
        {
            return Ok(Receipt::rejected(
                asset_orchard_swap_receipt_id(
                    genesis,
                    action,
                    verified,
                    "duplicate_output_commitment",
                )?,
                "duplicate_output_commitment",
                format!("asset-orchard output commitment `{commitment}` already exists"),
            ));
        }
        if !orchard_anchor_is_retained_for_apply(pool, &action_anchor)? {
            return Ok(Receipt::rejected(
                asset_orchard_swap_receipt_id(
                    genesis,
                    action,
                    verified,
                    "unretained_orchard_anchor",
                )?,
                "unretained_orchard_anchor",
                format!("asset-orchard anchor `{action_anchor}` is not retained by pool state"),
            ));
        }
    } else if action_anchor != orchard_empty_root_hex() {
        return Ok(Receipt::rejected(
            asset_orchard_swap_receipt_id(genesis, action, verified, "unretained_orchard_anchor")?,
            "unretained_orchard_anchor",
            format!("asset-orchard anchor `{action_anchor}` is not the empty Orchard root"),
        ));
    }

    let shielded_before_apply = shielded.clone();
    let apply_result = (|| -> io::Result<()> {
        let pool = shielded
            .orchard
            .get_or_insert_with(|| OrchardPoolState::empty(action.pool_id.clone()));
        ensure_orchard_root_history_for_apply(pool)?;
        for nullifier in &verified.nullifiers {
            pool.nullifiers.push(nullifier.as_hex().to_string());
        }
        for commitment in &verified.output_commitments {
            pool.output_commitments
                .push(commitment.as_hex().to_string());
        }
        for (output_commitment, encrypted_output) in verified
            .output_commitments
            .iter()
            .zip(verified.encrypted_outputs.iter())
        {
            pool.asset_orchard_outputs
                .push(AssetOrchardEncryptedOutputRecord {
                    output_commitment: output_commitment.as_hex().to_string(),
                    encrypted_output: encrypted_output.as_hex().to_string(),
                });
        }
        if !pool
            .accepted_anchors
            .iter()
            .any(|existing| existing == &action_anchor)
        {
            pool.accepted_anchors.push(action_anchor);
        }
        append_orchard_current_root(pool)?;
        Ok(())
    })();
    if let Err(error) = apply_result {
        *shielded = shielded_before_apply;
        return Err(error);
    }
    if let Err(error) = verify_shielded_state(shielded) {
        *shielded = shielded_before_apply;
        return Err(error);
    }
    Ok(Receipt::accepted(
        asset_orchard_swap_receipt_id(genesis, action, verified, "accepted")?,
        "asset-orchard swap verified and public pool state updated",
    ))
}

fn validate_asset_orchard_swap_accounting(
    action: &AssetOrchardSwapAction,
    verified: &VerifiedAssetOrchardSwap,
) -> Result<(), (&'static str, String)> {
    if action.accounting_inputs != verified.accounting_inputs
        || action.accounting_outputs != verified.accounting_outputs
    {
        return Err((
            "asset_orchard_accounting_mismatch",
            "asset-orchard swap verified accounting records do not match action accounting records"
                .to_string(),
        ));
    }
    validate_asset_orchard_accounting_record_set(
        "input",
        &verified.accounting_inputs,
        ASSET_ORCHARD_LEG_COUNT,
    )?;
    validate_asset_orchard_accounting_record_set(
        "output",
        &verified.accounting_outputs,
        verified.output_commitments.len(),
    )?;
    for (index, (record, commitment)) in verified
        .accounting_outputs
        .iter()
        .zip(verified.output_commitments.iter())
        .enumerate()
    {
        if record.output_commitment != commitment.as_hex() {
            return Err((
                "asset_orchard_accounting_output_commitment_mismatch",
                format!(
                    "asset-orchard accounting output {index} commitment does not match verified output commitment"
                ),
            ));
        }
    }
    let input_sum =
        asset_orchard_accounting_commitment_sum(&verified.accounting_inputs).map_err(
            |error| {
                (
                    error.code(),
                    format!("asset-orchard input accounting commitment sum failed: {error}"),
                )
            },
        )?;
    let output_sum =
        asset_orchard_accounting_commitment_sum(&verified.accounting_outputs).map_err(
            |error| {
                (
                    error.code(),
                    format!("asset-orchard output accounting commitment sum failed: {error}"),
                )
            },
        )?;
    if input_sum != output_sum {
        return Err((
            "asset_orchard_accounting_not_conserved",
            "asset-orchard swap aggregate accounting commitment sum is not conserved".to_string(),
        ));
    }
    Ok(())
}

fn validate_asset_orchard_accounting_record_set(
    label: &'static str,
    records: &[AssetOrchardSwapAccountingRecord],
    expected_len: usize,
) -> Result<(), (&'static str, String)> {
    if records.len() != expected_len {
        return Err((
            "asset_orchard_accounting_bad_record_count",
            format!(
                "asset-orchard {label} accounting record count {} does not match expected {expected_len}",
                records.len()
            ),
        ));
    }
    if has_duplicate_strings(records.iter().map(|record| record.output_commitment.as_str())) {
        return Err((
            "duplicate_asset_orchard_accounting_commitment",
            format!("asset-orchard {label} accounting records contain duplicate commitments"),
        ));
    }
    for record in records {
        validate_asset_orchard_accounting_record(label, record)?;
    }
    Ok(())
}

fn validate_asset_orchard_accounting_record(
    label: &'static str,
    record: &AssetOrchardSwapAccountingRecord,
) -> Result<(), (&'static str, String)> {
    validate_lower_hex_field(
        "AssetOrchard accounting output commitment",
        &record.output_commitment,
        ORCHARD_COMMITMENT_BYTES * 2,
    )
    .map_err(|error| {
        (
            "asset_orchard_accounting_bad_commitment",
            format!("{label} accounting commitment is invalid: {error}"),
        )
    })?;
    record.value_commitment.to_affine().map_err(|error| {
        (
            "invalid_asset_orchard_accounting_value_commitment",
            format!("asset-orchard {label} accounting value commitment is invalid: {error}"),
        )
    })?;
    Ok(())
}

fn validate_asset_orchard_accounting_asset_id(
    asset_id: &str,
) -> Result<(), (&'static str, String)> {
    if asset_id.trim().is_empty()
        || asset_id != asset_id.trim()
        || asset_id.chars().any(char::is_control)
        || asset_id.len() > ASSET_ORCHARD_MAX_ASSET_ID_BYTES
    {
        return Err((
            "asset_orchard_accounting_bad_asset_id",
            format!("asset-orchard accounting asset id `{asset_id}` is not canonical"),
        ));
    }
    Ok(())
}

fn credit_asset_orchard_balance(
    pool: &mut OrchardPoolState,
    asset_id: &str,
    amount: u64,
) -> Result<(), (&'static str, String)> {
    validate_asset_orchard_accounting_asset_id(asset_id)?;
    if amount == 0 {
        return Err((
            "zero_asset_orchard_accounting_value",
            "asset-orchard ingress amount must be nonzero".to_string(),
        ));
    }
    let index = match pool
        .asset_orchard_balances
        .iter()
        .position(|balance| balance.asset_id == asset_id)
    {
        Some(index) => index,
        None => {
            pool.asset_orchard_balances
                .push(AssetOrchardAssetBalance {
                    asset_id: asset_id.to_string(),
                    ingress_total: 0,
                    egress_total: 0,
                    live_total: 0,
                });
            pool.asset_orchard_balances.len() - 1
        }
    };
    let balance = &mut pool.asset_orchard_balances[index];
    balance.ingress_total = balance.ingress_total.checked_add(amount).ok_or_else(|| {
        (
            "asset_orchard_accounting_overflow",
            format!("asset-orchard ingress total overflow for asset `{asset_id}`"),
        )
    })?;
    balance.live_total = balance.live_total.checked_add(amount).ok_or_else(|| {
        (
            "asset_orchard_accounting_overflow",
            format!("asset-orchard live total overflow for asset `{asset_id}`"),
        )
    })?;
    sort_asset_orchard_balances(pool);
    Ok(())
}

fn debit_asset_orchard_balance(
    pool: &mut OrchardPoolState,
    asset_id: &str,
    amount: u64,
) -> Result<(), (&'static str, String)> {
    validate_asset_orchard_accounting_asset_id(asset_id)?;
    if amount == 0 {
        return Err((
            "zero_asset_orchard_accounting_value",
            "asset-orchard egress amount must be nonzero".to_string(),
        ));
    }
    let Some(balance) = pool
        .asset_orchard_balances
        .iter_mut()
        .find(|balance| balance.asset_id == asset_id)
    else {
        return Err((
            "asset_orchard_accounting_underflow",
            format!("asset-orchard egress has no live balance for asset `{asset_id}`"),
        ));
    };
    if balance.live_total < amount {
        return Err((
            "asset_orchard_accounting_underflow",
            format!(
                "asset-orchard egress amount {amount} exceeds live balance {} for asset `{asset_id}`",
                balance.live_total
            ),
        ));
    }
    balance.egress_total = balance.egress_total.checked_add(amount).ok_or_else(|| {
        (
            "asset_orchard_accounting_overflow",
            format!("asset-orchard egress total overflow for asset `{asset_id}`"),
        )
    })?;
    balance.live_total -= amount;
    sort_asset_orchard_balances(pool);
    Ok(())
}

fn sort_asset_orchard_balances(pool: &mut OrchardPoolState) {
    pool.asset_orchard_balances
        .sort_by(|left, right| left.asset_id.cmp(&right.asset_id));
}

struct AssetOrchardIngressStatePayload<'a> {
    burn_transaction: &'a SignedAssetTransaction,
    pool_id: &'a str,
    asset_id: &'a str,
    amount: u64,
    output_commitment: &'a str,
    encrypted_output: &'a str,
    receipt_domain: &'static str,
}

fn asset_orchard_ingress_v1_state_payload(
    payload: &AssetOrchardIngressActionPayload,
) -> AssetOrchardIngressStatePayload<'_> {
    AssetOrchardIngressStatePayload {
        burn_transaction: &payload.burn_transaction,
        pool_id: &payload.pool_id,
        asset_id: &payload.asset_id,
        amount: payload.amount,
        output_commitment: &payload.output_commitment,
        encrypted_output: &payload.encrypted_output,
        receipt_domain: "postfiat.privacy.asset_orchard_ingress_receipt.v1",
    }
}

fn asset_orchard_ingress_v2_state_payload(
    payload: &AssetOrchardIngressV2ActionPayload,
) -> AssetOrchardIngressStatePayload<'_> {
    AssetOrchardIngressStatePayload {
        burn_transaction: &payload.burn_transaction,
        pool_id: &payload.pool_id,
        asset_id: &payload.asset_id,
        amount: payload.amount,
        output_commitment: &payload.output_commitment,
        encrypted_output: &payload.encrypted_output,
        receipt_domain: "postfiat.privacy.asset_orchard_ingress_receipt.v2",
    }
}

fn apply_asset_orchard_ingress_action_to_state(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    shielded: &mut ShieldedState,
    payload: &AssetOrchardIngressStatePayload<'_>,
    block_height: u64,
    asset_execution_compatibility: AssetExecutionCompatibility,
) -> io::Result<Receipt> {
    let timing_enabled = std::env::var_os("POSTFIAT_ORCHARD_TIMING_STDERR").is_some();
    let timing_total = std::time::Instant::now();
    let mut timing_stage = timing_total;
    macro_rules! log_ingress_timing {
        ($label:literal) => {
            if timing_enabled {
                let now = std::time::Instant::now();
                eprintln!(
                    "asset_orchard_ingress_timing label={} stage_ms={:.3} total_ms={:.3}",
                    $label,
                    node_timing_elapsed_ms(timing_stage),
                    node_timing_elapsed_ms(timing_total)
                );
                timing_stage = now;
            }
        };
    }
    log_ingress_timing!("validated_payload");
    if let Some(pool) = shielded.orchard.as_ref() {
        if pool.pool_id != payload.pool_id {
            return Ok(Receipt::rejected(
                asset_orchard_ingress_receipt_id(genesis, payload, "pool_id_mismatch")?,
                "pool_id_mismatch",
                format!(
                    "Orchard pool state is for `{}`, AssetOrchard ingress is for `{}`",
                    pool.pool_id, payload.pool_id
                ),
            ));
        }
        if pool
            .output_commitments
            .iter()
            .any(|existing| existing == payload.output_commitment)
        {
            return Ok(Receipt::rejected(
                asset_orchard_ingress_receipt_id(
                    genesis,
                    payload,
                    "duplicate_output_commitment",
                )?,
                "duplicate_output_commitment",
                format!(
                    "AssetOrchard ingress output commitment `{}` already exists",
                    payload.output_commitment
                ),
            ));
        }
    }
    log_ingress_timing!("duplicate_check");

    let mut trial_ledger = ledger.clone();
    log_ingress_timing!("clone_ledger");
    let burn_receipt = execute_asset_transaction_with_replay_compatibility(
        genesis,
        &mut trial_ledger,
        &payload.burn_transaction,
        block_height,
        asset_execution_compatibility,
    );
    log_ingress_timing!("execute_burn");
    if !burn_receipt.accepted {
        return Ok(Receipt::rejected(
            asset_orchard_ingress_receipt_id(genesis, payload, "burn_rejected")?,
            "asset_orchard_ingress_burn_rejected",
            format!(
                "AssetOrchard ingress burn rejected with {}: {}",
                burn_receipt.code, burn_receipt.message
            ),
        )
        .with_fee_policy(
            0,
            0,
            burn_receipt.minimum_fee,
            burn_receipt.state_expansion_fee,
        ));
    }

    let mut trial_shielded = shielded.clone();
    log_ingress_timing!("clone_shielded");
    {
        let pool = trial_shielded
            .orchard
            .get_or_insert_with(|| OrchardPoolState::empty(payload.pool_id.to_string()));
        ensure_orchard_root_history_for_apply(pool)?;
        log_ingress_timing!("ensure_root_history");
        pool.output_commitments
            .push(payload.output_commitment.to_string());
        pool.asset_orchard_outputs
            .push(AssetOrchardEncryptedOutputRecord {
                output_commitment: payload.output_commitment.to_string(),
                encrypted_output: payload.encrypted_output.to_string(),
            });
        if let Err((code, message)) =
            credit_asset_orchard_balance(pool, &payload.asset_id, payload.amount)
        {
            return Ok(Receipt::rejected(
                asset_orchard_ingress_receipt_id(genesis, payload, code)?,
                code,
                message,
            ));
        }
        append_orchard_current_root(pool)?;
        log_ingress_timing!("append_current_root");
    }
    verify_shielded_state(&trial_shielded)?;
    log_ingress_timing!("verify_shielded_state");
    *ledger = trial_ledger;
    *shielded = trial_shielded;
    log_ingress_timing!("commit_state");
    let _ = timing_stage;
    Ok(Receipt::accepted(
        asset_orchard_ingress_receipt_id(genesis, payload, "accepted")?,
        "AssetOrchard ingress burned transparent issued asset and appended typed note commitment",
    )
    .with_fee_policy(
        burn_receipt.fee_charged,
        burn_receipt.fee_burned,
        burn_receipt.minimum_fee,
        burn_receipt.state_expansion_fee,
    ))
}

fn apply_asset_orchard_egress_action_to_state(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    shielded: &mut ShieldedState,
    payload: &AssetOrchardEgressActionPayload,
) -> io::Result<Receipt> {
    if let Err(error) = validate_asset_orchard_egress_payload_for_genesis(genesis, payload) {
        return Ok(Receipt::rejected(
            asset_orchard_egress_receipt_id(genesis, payload, "bad_payload")?,
            "asset_orchard_egress_bad_payload",
            error.to_string(),
        ));
    }
    let Some(pool) = shielded.orchard.as_ref() else {
        return Ok(Receipt::rejected(
            asset_orchard_egress_receipt_id(genesis, payload, "empty_pool")?,
            "asset_orchard_egress_empty_pool",
            "AssetOrchard egress requires an existing Orchard pool".to_string(),
        ));
    };
    if pool.pool_id != payload.pool_id {
        return Ok(Receipt::rejected(
            asset_orchard_egress_receipt_id(genesis, payload, "pool_id_mismatch")?,
            "pool_id_mismatch",
            format!(
                "Orchard pool state is for `{}`, AssetOrchard egress is for `{}`",
                pool.pool_id, payload.pool_id
            ),
        ));
    }
    if !pool
        .asset_orchard_outputs
        .iter()
        .any(|output| output.output_commitment == payload.output_commitment)
    {
        return Ok(Receipt::rejected(
            asset_orchard_egress_receipt_id(genesis, payload, "missing_output_commitment")?,
            "missing_output_commitment",
            format!(
                "AssetOrchard egress output commitment `{}` is not a typed pool output",
                payload.output_commitment
            ),
        ));
    }
    if pool.is_nullified(&payload.nullifier) {
        return Ok(Receipt::rejected(
            asset_orchard_egress_receipt_id(genesis, payload, "duplicate_nullifier")?,
            "duplicate_nullifier",
            format!(
                "AssetOrchard egress nullifier `{}` already exists",
                payload.nullifier
            ),
        ));
    }

    let mut trial_ledger = ledger.clone();
    if let Err((code, message)) = credit_issued_asset_from_shielded_pool(
        &mut trial_ledger,
        &payload.to,
        &payload.asset_id,
        payload.amount,
    ) {
        return Ok(Receipt::rejected(
            asset_orchard_egress_receipt_id(genesis, payload, code)?,
            code,
            message,
        ));
    }
    trial_ledger
        .validate_asset_state(&genesis.chain_id)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    trial_ledger
        .validate_nav_state(&genesis.chain_id)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    let mut trial_shielded = shielded.clone();
    {
        let pool = trial_shielded
            .orchard
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing Orchard pool"))?;
        if let Err((code, message)) =
            debit_asset_orchard_balance(pool, &payload.asset_id, payload.amount)
        {
            return Ok(Receipt::rejected(
                asset_orchard_egress_receipt_id(genesis, payload, code)?,
                code,
                message,
            ));
        }
        pool.nullifiers.push(payload.nullifier.clone());
    }
    verify_shielded_state(&trial_shielded)?;
    *ledger = trial_ledger;
    *shielded = trial_shielded;
    Ok(Receipt::accepted(
        asset_orchard_egress_receipt_id(genesis, payload, "accepted")?,
        "AssetOrchard disclosed egress nullified typed note and credited public issued asset balance",
    ))
}

fn apply_asset_orchard_private_egress_action_to_state(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    shielded: &mut ShieldedState,
    payload: &AssetOrchardPrivateEgressActionPayload,
    archived_pre_repin: bool,
    archive_replay: bool,
) -> io::Result<Receipt> {
    let total_start = std::time::Instant::now();
    let mut timing = AssetOrchardPrivateEgressStateApplyTimingReport::default();

    macro_rules! finish_receipt {
        ($receipt:expr) => {{
            let receipt = $receipt;
            timing.total_ms = node_timing_elapsed_ms(total_start);
            timing.accepted = receipt.accepted;
            timing.receipt_code = receipt.code.clone();
            timing.result = if receipt.accepted {
                "accepted".to_string()
            } else {
                format!("rejected:{}", receipt.code)
            };
            record_asset_orchard_private_egress_state_apply_timing(timing);
            return Ok(receipt);
        }};
    }

    let stage_start = std::time::Instant::now();
    let action = match asset_orchard_private_egress_action_from_payload(payload) {
        Ok(action) => action,
        Err(error) => {
            timing.payload_decode_ms = node_timing_elapsed_ms(stage_start);
            finish_receipt!(Receipt::rejected(
                asset_orchard_private_egress_receipt_id(genesis, payload, "bad_payload")?,
                "asset_orchard_private_egress_bad_payload",
                error.to_string(),
            ));
        }
    };
    timing.payload_decode_ms = node_timing_elapsed_ms(stage_start);

    let (verified_anchor, verified_nullifier) = if archived_pre_repin {
        // The caller admits this only for an immutable chain/height/batch tuple
        // whose original certificate is verified by archive replay.  New and
        // non-allowlisted actions always execute the current proof verifier.
        (action.anchor.clone(), action.nullifier.clone())
    } else {
        let stage_start = std::time::Instant::now();
        let domain = orchard_authorizing_domain(genesis, &payload.pool_id)?;
        timing.domain_ms = node_timing_elapsed_ms(stage_start);

        reset_asset_orchard_private_egress_timings();
        let stage_start = std::time::Instant::now();
        let verified_result = if archive_replay {
            verify_serialized_asset_orchard_private_egress_action_for_archive_replay(
                &action,
                &domain,
                &payload.to,
                &payload.asset_id,
                &payload.policy_id,
                &payload.disclosure_hash,
            )
        } else {
            verify_serialized_asset_orchard_private_egress_action(
                &action,
                &domain,
                &payload.to,
                &payload.asset_id,
                &payload.policy_id,
                &payload.disclosure_hash,
            )
        };
        let verified = match verified_result {
            Ok(verified) => {
                timing.verifier_ms = node_timing_elapsed_ms(stage_start);
                timing.verifier_breakdown = take_asset_orchard_private_egress_timings();
                verified
            }
            Err(error) => {
                timing.verifier_ms = node_timing_elapsed_ms(stage_start);
                timing.verifier_breakdown = take_asset_orchard_private_egress_timings();
                finish_receipt!(Receipt::rejected(
                    asset_orchard_private_egress_receipt_id(genesis, payload, "bad_payload")?,
                    "asset_orchard_private_egress_bad_payload",
                    error.to_string(),
                ));
            }
        };
        (verified.anchor, verified.nullifier)
    };

    let stage_start = std::time::Instant::now();
    let Some(pool) = shielded.orchard.as_ref() else {
        timing.pool_lookup_ms = node_timing_elapsed_ms(stage_start);
        finish_receipt!(Receipt::rejected(
            asset_orchard_private_egress_receipt_id(genesis, payload, "empty_pool")?,
            "asset_orchard_private_egress_empty_pool",
            "AssetOrchard private egress requires an existing Orchard pool".to_string(),
        ));
    };
    if pool.pool_id != payload.pool_id {
        timing.pool_lookup_ms = node_timing_elapsed_ms(stage_start);
        finish_receipt!(Receipt::rejected(
            asset_orchard_private_egress_receipt_id(genesis, payload, "pool_id_mismatch")?,
            "pool_id_mismatch",
            format!(
                "Orchard pool state is for `{}`, AssetOrchard private egress is for `{}`",
                pool.pool_id, payload.pool_id
            ),
        ));
    }
    timing.pool_lookup_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    if !orchard_anchor_is_retained_for_apply(pool, verified_anchor.as_hex())? {
        timing.retained_anchor_check_ms = node_timing_elapsed_ms(stage_start);
        finish_receipt!(Receipt::rejected(
            asset_orchard_private_egress_receipt_id(
                genesis,
                payload,
                "unretained_orchard_anchor",
            )?,
            "unretained_orchard_anchor",
            format!(
                "AssetOrchard private egress anchor `{}` is not retained by pool state",
                verified_anchor.as_hex()
            ),
        ));
    }
    timing.retained_anchor_check_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    if pool.is_nullified(verified_nullifier.as_hex()) {
        timing.nullifier_check_ms = node_timing_elapsed_ms(stage_start);
        finish_receipt!(Receipt::rejected(
            asset_orchard_private_egress_receipt_id(genesis, payload, "duplicate_nullifier")?,
            "duplicate_nullifier",
            format!(
                "AssetOrchard private egress nullifier `{}` already exists",
                verified_nullifier.as_hex()
            ),
        ));
    }
    timing.nullifier_check_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let mut trial_ledger = ledger.clone();
    timing.trial_ledger_clone_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    if let Err((code, message)) = credit_issued_asset_from_shielded_pool(
        &mut trial_ledger,
        &payload.to,
        &payload.asset_id,
        payload.amount,
    ) {
        timing.trial_ledger_credit_ms = node_timing_elapsed_ms(stage_start);
        finish_receipt!(Receipt::rejected(
            asset_orchard_private_egress_receipt_id(genesis, payload, code)?,
            code,
            message,
        ));
    }
    timing.trial_ledger_credit_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    trial_ledger
        .validate_asset_state(&genesis.chain_id)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    timing.trial_ledger_validate_asset_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    trial_ledger
        .validate_nav_state(&genesis.chain_id)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    timing.trial_ledger_validate_nav_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let mut trial_shielded = shielded.clone();
    timing.trial_shielded_clone_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    {
        let pool = trial_shielded
            .orchard
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing Orchard pool"))?;
        if let Err((code, message)) =
            debit_asset_orchard_balance(pool, &payload.asset_id, payload.amount)
        {
            timing.trial_shielded_nullifier_push_ms = node_timing_elapsed_ms(stage_start);
            finish_receipt!(Receipt::rejected(
                asset_orchard_private_egress_receipt_id(genesis, payload, code)?,
                code,
                message,
            ));
        }
        pool.nullifiers
            .push(verified_nullifier.as_hex().to_string());
    }
    timing.trial_shielded_nullifier_push_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    verify_shielded_state(&trial_shielded)?;
    timing.verify_shielded_state_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    *ledger = trial_ledger;
    *shielded = trial_shielded;
    timing.commit_state_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let receipt = Receipt::accepted(
        asset_orchard_private_egress_receipt_id(genesis, payload, "accepted")?,
        "AssetOrchard private egress nullified typed note and credited public issued asset balance without disclosing note opening",
    );
    timing.receipt_ms = node_timing_elapsed_ms(stage_start);
    finish_receipt!(receipt);
}

fn orchard_minimum_fee_for_action(
    action: &OrchardShieldedAction,
    verified: &VerifiedOrchardBundle,
) -> u64 {
    if verified.value_balance <= 0 {
        return 0;
    }
    orchard_minimum_resource_fee_for_action(action)
}

fn orchard_minimum_resource_fee_for_action(action: &OrchardShieldedAction) -> u64 {
    let quanta = orchard_action_weight_bytes(action).div_ceil(ORCHARD_FEE_BURN_BYTE_QUANTUM);
    let byte_fee = u64::try_from(quanta)
        .unwrap_or(u64::MAX)
        .saturating_mul(ORCHARD_FEE_BURN_FEE_PER_QUANTUM);
    ORCHARD_FEE_BURN_MIN_FEE.max(byte_fee)
}

fn orchard_action_weight_bytes(action: &OrchardShieldedAction) -> usize {
    let mut bytes = action
        .pool_id
        .len()
        .saturating_add(action.proof_system_id.as_str().len())
        .saturating_add(action.circuit_id.as_str().len())
        .saturating_add(1)
        .saturating_add(ORCHARD_ANCHOR_BYTES)
        .saturating_add(std::mem::size_of::<i64>())
        .saturating_add(std::mem::size_of::<u64>())
        .saturating_add(
            action
                .external_binding_hash
                .as_ref()
                .map(|hash| hash.len() / 2)
                .unwrap_or(0),
        )
        .saturating_add(action.proof.byte_len())
        .saturating_add(ORCHARD_REDPALLAS_SIGNATURE_BYTES);
    for nullifier in &action.nullifiers {
        bytes = bytes.saturating_add(nullifier.as_hex().len() / 2);
    }
    for key in &action.randomized_verification_keys {
        bytes = bytes.saturating_add(key.as_hex().len() / 2);
    }
    for commitment in &action.value_commitments {
        bytes = bytes.saturating_add(commitment.as_hex().len() / 2);
    }
    for commitment in &action.output_commitments {
        bytes = bytes.saturating_add(commitment.as_hex().len() / 2);
    }
    for output in &action.encrypted_outputs {
        bytes = bytes
            .saturating_add(output.cmx.as_hex().len() / 2)
            .saturating_add(output.epk.byte_len())
            .saturating_add(output.enc_ciphertext.byte_len())
            .saturating_add(output.out_ciphertext.byte_len())
            .saturating_add(
                output
                    .compact_ciphertext
                    .as_ref()
                    .map(|ciphertext| ciphertext.byte_len())
                    .unwrap_or(0),
            );
    }
    for signature in &action.spend_authorization_signatures {
        bytes = bytes.saturating_add(signature.as_hex().len() / 2);
    }
    bytes
}

fn orchard_anchor_is_retained_for_apply(pool: &OrchardPoolState, anchor: &str) -> io::Result<bool> {
    if pool.root_history.iter().any(|record| record.root == anchor) {
        return Ok(true);
    }
    if pool.root_history.is_empty() {
        if pool.output_commitments.is_empty() {
            return Ok(anchor == orchard_empty_root_hex());
        }
        return Ok(anchor == orchard_pool_current_root(pool)?);
    }
    Ok(false)
}

fn orchard_turnstile_deposit_amount(value_balance: i64) -> io::Result<u64> {
    if value_balance >= 0 {
        return Ok(0);
    }
    let positive = value_balance.checked_neg().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Orchard value balance cannot be converted to a deposit amount",
        )
    })?;
    u64::try_from(positive).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Orchard deposit amount does not fit u64",
        )
    })
}

fn orchard_turnstile_migration_total(shielded: &ShieldedState, pool_id: &str) -> io::Result<u64> {
    let mut total = 0_u64;
    for event in &shielded.turnstile_events {
        if event.kind == TURNSTILE_KIND_POOL_MIGRATION && event.target_pool == pool_id {
            total = total.checked_add(event.amount).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Orchard turnstile migration total overflow",
                )
            })?;
        }
    }
    Ok(total)
}

fn orchard_turnstile_budget_total(shielded: &ShieldedState, pool_id: &str) -> io::Result<u64> {
    let mut total = orchard_turnstile_migration_total(shielded, pool_id)?;
    for event in &shielded.turnstile_events {
        if event.target_pool != pool_id {
            continue;
        }
        if event.kind == TURNSTILE_KIND_ORCHARD_DEPOSIT {
            total = total.checked_add(event.amount).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Orchard turnstile budget total overflow",
                )
            })?;
        }
    }
    Ok(total)
}

fn orchard_pool_issued_value(pool: &OrchardPoolState) -> io::Result<u64> {
    if pool.value_balance_total > 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Orchard pool has positive value balance total",
        ));
    }
    orchard_turnstile_deposit_amount(pool.value_balance_total)
}

fn ensure_orchard_root_history_for_apply(pool: &mut OrchardPoolState) -> io::Result<()> {
    if pool.root_history.is_empty() {
        let record = if pool.output_commitments.is_empty() {
            orchard_empty_root_record()
        } else {
            let cache = update_orchard_frontier_cache_for_current_outputs(pool)?;
            OrchardRootRecord {
                root: cache.root,
                output_count: cache.output_count,
            }
        };
        pool.root_history.push(record);
    }
    Ok(())
}

fn append_orchard_current_root(pool: &mut OrchardPoolState) -> io::Result<()> {
    let cache = update_orchard_frontier_cache_for_current_outputs(pool)?;
    let latest_root = cache.root;
    let output_count = cache.output_count;
    if pool.root_history.last().map(|record| record.root.as_str()) != Some(latest_root.as_str()) {
        pool.root_history.push(OrchardRootRecord {
            root: latest_root,
            output_count,
        });
    }
    Ok(())
}

fn orchard_pool_current_root(pool: &OrchardPoolState) -> io::Result<String> {
    if let Some(snapshot) = trusted_orchard_frontier_snapshot(pool) {
        let start = usize::try_from(snapshot.output_count).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Orchard frontier cache output_count does not fit this platform",
            )
        })?;
        let suffix = orchard_commitments_from_hexes(&pool.output_commitments[start..])?;
        if let Ok(snapshot) =
            orchard_frontier_snapshot_append_commitments(Some(&snapshot), &suffix)
        {
            return Ok(snapshot.root);
        }
    }
    let commitments = orchard_pool_commitments(pool)?;
    orchard_anchor_from_commitments(&commitments)
        .map(|anchor| anchor.as_hex().to_string())
        .map_err(invalid_data)
}

fn update_orchard_frontier_cache_for_current_outputs(
    pool: &mut OrchardPoolState,
) -> io::Result<OrchardFrontierCache> {
    if let Some(snapshot) = trusted_orchard_frontier_snapshot(pool) {
        let start = usize::try_from(snapshot.output_count).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Orchard frontier cache output_count does not fit this platform",
            )
        })?;
        let suffix = orchard_commitments_from_hexes(&pool.output_commitments[start..])?;
        if let Ok(snapshot) =
            orchard_frontier_snapshot_append_commitments(Some(&snapshot), &suffix)
        {
            let cache = orchard_frontier_cache_from_snapshot(snapshot);
            pool.frontier_cache = Some(cache.clone());
            return Ok(cache);
        }
    }

    let commitments = orchard_pool_commitments(pool)?;
    let snapshot = orchard_frontier_snapshot_from_commitments(&commitments).map_err(invalid_data)?;
    let cache = orchard_frontier_cache_from_snapshot(snapshot);
    pool.frontier_cache = if cache.output_count == 0 {
        None
    } else {
        Some(cache.clone())
    };
    Ok(cache)
}

fn trusted_orchard_frontier_snapshot(pool: &OrchardPoolState) -> Option<OrchardFrontierSnapshot> {
    let cache = pool.frontier_cache.as_ref()?;
    if !orchard_frontier_cache_matches_history(pool, cache) {
        return None;
    }
    Some(OrchardFrontierSnapshot {
        output_count: cache.output_count,
        root: cache.root.clone(),
        latest_leaf: cache.latest_leaf.clone(),
        ommers: cache.ommers.clone(),
    })
}

fn orchard_frontier_cache_matches_history(
    pool: &OrchardPoolState,
    cache: &OrchardFrontierCache,
) -> bool {
    let Ok(output_len) = u64::try_from(pool.output_commitments.len()) else {
        return false;
    };
    if cache.output_count > output_len {
        return false;
    }
    if cache.output_count == 0 {
        return cache.root == orchard_empty_root_hex()
            && cache.latest_leaf.is_none()
            && cache.ommers.is_empty();
    }
    pool.root_history
        .iter()
        .rev()
        .find(|record| record.output_count == cache.output_count)
        .is_some_and(|record| record.root == cache.root)
}

fn orchard_frontier_cache_from_snapshot(
    snapshot: OrchardFrontierSnapshot,
) -> OrchardFrontierCache {
    OrchardFrontierCache {
        output_count: snapshot.output_count,
        root: snapshot.root,
        latest_leaf: snapshot.latest_leaf,
        ommers: snapshot.ommers,
    }
}

fn orchard_pool_commitments(pool: &OrchardPoolState) -> io::Result<Vec<OrchardOutputCommitment>> {
    orchard_commitments_from_hexes(&pool.output_commitments)
}

fn orchard_commitments_from_hexes(
    commitments: &[String],
) -> io::Result<Vec<OrchardOutputCommitment>> {
    commitments
        .iter()
        .map(|commitment| OrchardOutputCommitment::parse_hex(commitment).map_err(invalid_data))
        .collect()
}

fn orchard_output_record_from_verified(
    output: &EncryptedShieldedOutput,
) -> OrchardEncryptedOutputRecord {
    OrchardEncryptedOutputRecord {
        cmx: output.cmx.as_hex().to_string(),
        epk: output.epk.as_hex().to_string(),
        enc_ciphertext: output.enc_ciphertext.as_hex().to_string(),
        out_ciphertext: output.out_ciphertext.as_hex().to_string(),
        compact_ciphertext: output
            .compact_ciphertext
            .as_ref()
            .map(|ciphertext| ciphertext.as_hex().to_string()),
    }
}

fn orchard_encrypted_output_from_record(
    output: &OrchardEncryptedOutputRecord,
) -> io::Result<EncryptedShieldedOutput> {
    Ok(EncryptedShieldedOutput {
        cmx: OrchardOutputCommitment::parse_hex(output.cmx.clone()).map_err(invalid_data)?,
        epk: BoundedHexBlob::parse_hex("epk", output.epk.clone(), ORCHARD_EPK_BYTES)
            .map_err(invalid_data)?,
        enc_ciphertext: BoundedHexBlob::parse_hex(
            "enc_ciphertext",
            output.enc_ciphertext.clone(),
            ORCHARD_ENC_CIPHERTEXT_BYTES,
        )
        .map_err(invalid_data)?,
        out_ciphertext: BoundedHexBlob::parse_hex(
            "out_ciphertext",
            output.out_ciphertext.clone(),
            ORCHARD_OUT_CIPHERTEXT_BYTES,
        )
        .map_err(invalid_data)?,
        compact_ciphertext: output
            .compact_ciphertext
            .as_ref()
            .map(|ciphertext| {
                BoundedHexBlob::parse_hex(
                    "compact_ciphertext",
                    ciphertext.clone(),
                    ORCHARD_COMPACT_CIPHERTEXT_BYTES,
                )
                .map_err(invalid_data)
            })
            .transpose()?,
    })
}

fn orchard_wallet_decrypted_output(
    pool: &OrchardPoolState,
    commitments: &[OrchardOutputCommitment],
    output: OrchardDecryptedOutput,
) -> io::Result<OrchardWalletDecryptedOutput> {
    let spent = pool.is_nullified(&output.nullifier);
    let witness = orchard_merkle_witness_from_commitments(commitments, output.output_index)
        .map_err(invalid_data)?;
    Ok(OrchardWalletDecryptedOutput {
        output_index: output.output_index,
        merkle_position: witness.position,
        commitment: output.commitment,
        nullifier: output.nullifier,
        rho: output.rho,
        rseed: output.rseed,
        value: output.value,
        spent,
        witness_anchor: witness.anchor,
        witness_output_count: witness.output_count,
        witness_auth_path: witness.auth_path,
        address_raw_hex: output.address_raw_hex,
        memo_hex: output.memo_hex,
    })
}

fn orchard_spending_key_bytes(spending_key_hex: &str) -> io::Result<[u8; 32]> {
    let bytes = hex_to_bytes(spending_key_hex).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Orchard spending key hex is invalid: {error}"),
        )
    })?;
    bytes.try_into().map_err(|bytes: Vec<u8>| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Orchard spending key hex must decode to 32 bytes, got {}",
                bytes.len()
            ),
        )
    })
}

fn orchard_spend_action_spending_key(options: &OrchardSpendActionOptions) -> io::Result<[u8; 32]> {
    let provided = options.spending_key_hex.is_some() as u8 + options.key_file.is_some() as u8;
    if provided != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "provide exactly one Orchard spend authority: --spending-key-hex or --key-file",
        ));
    }
    if let Some(spending_key_hex) = &options.spending_key_hex {
        return orchard_spending_key_bytes(spending_key_hex);
    }
    let Some(key_file_path) = options.key_file.as_ref() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "provide exactly one Orchard spend authority: --spending-key-hex or --key-file",
        ));
    };
    let key_file = read_orchard_wallet_key_file(key_file_path)?;
    orchard_spending_key_bytes(&key_file.spending_key_hex)
}

fn orchard_withdraw_action_spending_key(
    options: &OrchardWithdrawActionOptions,
) -> io::Result<[u8; 32]> {
    let provided = options.spending_key_hex.is_some() as u8 + options.key_file.is_some() as u8;
    if provided != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "provide exactly one Orchard spend authority: --spending-key-hex or --key-file",
        ));
    }
    if let Some(spending_key_hex) = &options.spending_key_hex {
        return orchard_spending_key_bytes(spending_key_hex);
    }
    let Some(key_file_path) = options.key_file.as_ref() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "provide exactly one Orchard spend authority: --spending-key-hex or --key-file",
        ));
    };
    let key_file = read_orchard_wallet_key_file(key_file_path)?;
    orchard_spending_key_bytes(&key_file.spending_key_hex)
}

enum OrchardScanKey {
    Spending([u8; 32]),
    FullViewing([u8; 96]),
}

impl OrchardScanKey {
    fn address_raw_hex(&self) -> io::Result<String> {
        match self {
            Self::Spending(spending_key) => {
                orchard_default_address_from_spending_key(*spending_key).map_err(invalid_data)
            }
            Self::FullViewing(full_viewing_key) => {
                orchard_default_address_from_full_viewing_key(*full_viewing_key)
                    .map_err(invalid_data)
            }
        }
    }

    fn scan(
        &self,
        nullifiers: &[OrchardNullifier],
        outputs: &[EncryptedShieldedOutput],
    ) -> io::Result<Vec<OrchardDecryptedOutput>> {
        match self {
            Self::Spending(spending_key) => {
                orchard_scan_encrypted_outputs_with_spending_key(*spending_key, nullifiers, outputs)
                    .map_err(invalid_data)
            }
            Self::FullViewing(full_viewing_key) => {
                orchard_scan_encrypted_outputs_with_full_viewing_key(
                    *full_viewing_key,
                    nullifiers,
                    outputs,
                )
                .map_err(invalid_data)
            }
        }
    }
}

fn orchard_scan_key(options: &OrchardWalletScanOptions) -> io::Result<OrchardScanKey> {
    let provided = options.spending_key_hex.is_some() as u8
        + options.key_file.is_some() as u8
        + options.view_key_file.is_some() as u8;
    if provided != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "provide exactly one Orchard scan key: --spending-key-hex, --key-file, or --view-key-file",
        ));
    }
    if let Some(spending_key_hex) = &options.spending_key_hex {
        return orchard_spending_key_bytes(spending_key_hex).map(OrchardScanKey::Spending);
    }
    if let Some(key_file) = &options.key_file {
        let key_file = read_orchard_wallet_key_file(key_file)?;
        return orchard_spending_key_bytes(&key_file.spending_key_hex)
            .map(OrchardScanKey::Spending);
    }
    let Some(view_key_file) = options.view_key_file.as_ref() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "provide exactly one Orchard scan key: --spending-key-hex, --key-file, or --view-key-file",
        ));
    };
    let view_key = read_orchard_view_key_file(view_key_file)?;
    orchard_full_viewing_key_bytes(&view_key.full_viewing_key_hex).map(OrchardScanKey::FullViewing)
}

fn orchard_latest_retained_anchor(shielded: &ShieldedState, pool_id: &str) -> io::Result<String> {
    let Some(pool) = shielded.orchard.as_ref() else {
        return Ok(orchard_empty_root_hex());
    };
    if pool.pool_id != pool_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Orchard pool id `{}` does not match expected pool `{pool_id}`",
                pool.pool_id
            ),
        ));
    }
    verify_orchard_pool_state(pool)?;
    if let Some(latest) = pool.root_history.last() {
        return Ok(latest.root.clone());
    }
    orchard_pool_current_root(pool)
}

fn orchard_output_recipient_address(options: &OrchardOutputActionOptions) -> io::Result<String> {
    orchard_recipient_address_from_sources(
        options.recipient_address_raw_hex.as_ref(),
        options.recipient_key_file.as_ref(),
        options.recipient_view_key_file.as_ref(),
        "--recipient-address-raw-hex, --recipient-key-file, or --recipient-view-key-file",
    )
}

fn orchard_deposit_recipient_address(options: &OrchardDepositActionOptions) -> io::Result<String> {
    orchard_recipient_address_from_sources(
        options.recipient_address_raw_hex.as_ref(),
        options.recipient_key_file.as_ref(),
        options.recipient_view_key_file.as_ref(),
        "--recipient-address-raw-hex, --recipient-key-file, or --recipient-view-key-file",
    )
}

fn orchard_spend_recipient_address(options: &OrchardSpendActionOptions) -> io::Result<String> {
    orchard_recipient_address_from_sources(
        options.recipient_address_raw_hex.as_ref(),
        options.recipient_key_file.as_ref(),
        options.recipient_view_key_file.as_ref(),
        "--recipient-address-raw-hex, --recipient-key-file, or --recipient-view-key-file",
    )
}

fn orchard_spend_change_address(
    options: &OrchardSpendActionOptions,
    spending_key: [u8; 32],
) -> io::Result<String> {
    let provided = orchard_change_address_source_count(options);
    if provided == 0 {
        return orchard_default_address_from_spending_key(spending_key).map_err(invalid_data);
    }
    if provided != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "provide at most one Orchard change recipient: --change-recipient-address-raw-hex, --change-recipient-key-file, or --change-recipient-view-key-file",
        ));
    }
    orchard_recipient_address_from_sources(
        options.change_address_raw_hex.as_ref(),
        options.change_key_file.as_ref(),
        options.change_view_key_file.as_ref(),
        "--change-recipient-address-raw-hex, --change-recipient-key-file, or --change-recipient-view-key-file",
    )
}

fn orchard_withdraw_change_address(
    options: &OrchardWithdrawActionOptions,
    spending_key: [u8; 32],
) -> io::Result<String> {
    let provided = orchard_withdraw_change_address_source_count(options);
    if provided == 0 {
        return orchard_default_address_from_spending_key(spending_key).map_err(invalid_data);
    }
    if provided != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "provide at most one Orchard change recipient: --change-recipient-address-raw-hex, --change-recipient-key-file, or --change-recipient-view-key-file",
        ));
    }
    orchard_recipient_address_from_sources(
        options.change_address_raw_hex.as_ref(),
        options.change_key_file.as_ref(),
        options.change_view_key_file.as_ref(),
        "--change-recipient-address-raw-hex, --change-recipient-key-file, or --change-recipient-view-key-file",
    )
}

fn ensure_orchard_change_address_not_requested(
    options: &OrchardSpendActionOptions,
) -> io::Result<()> {
    if orchard_change_address_source_count(options) == 0 {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "Orchard change recipient options require --amount below input minus fee",
    ))
}

fn ensure_orchard_withdraw_change_address_not_requested(
    options: &OrchardWithdrawActionOptions,
) -> io::Result<()> {
    if orchard_withdraw_change_address_source_count(options) == 0 {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "Orchard change recipient options require --amount plus --fee below input value",
    ))
}

fn orchard_change_address_source_count(options: &OrchardSpendActionOptions) -> u8 {
    options.change_address_raw_hex.is_some() as u8
        + options.change_key_file.is_some() as u8
        + options.change_view_key_file.is_some() as u8
}

fn orchard_withdraw_change_address_source_count(options: &OrchardWithdrawActionOptions) -> u8 {
    options.change_address_raw_hex.is_some() as u8
        + options.change_key_file.is_some() as u8
        + options.change_view_key_file.is_some() as u8
}

fn orchard_recipient_address_from_sources(
    recipient_address_raw_hex: Option<&String>,
    recipient_key_file: Option<&PathBuf>,
    recipient_view_key_file: Option<&PathBuf>,
    source_help: &str,
) -> io::Result<String> {
    let provided = recipient_address_raw_hex.is_some() as u8
        + recipient_key_file.is_some() as u8
        + recipient_view_key_file.is_some() as u8;
    if provided != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("provide exactly one Orchard recipient: {source_help}"),
        ));
    }
    if let Some(address) = recipient_address_raw_hex {
        validate_hex_string(
            "Orchard recipient raw address",
            address,
            Some(ORCHARD_RAW_ADDRESS_BYTES * 2),
        )?;
        return Ok(address.clone());
    }
    if let Some(key_file) = recipient_key_file {
        let key_file = read_orchard_wallet_key_file(key_file)?;
        return Ok(key_file.address_raw_hex);
    }
    let Some(view_key_file) = recipient_view_key_file else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("provide exactly one Orchard recipient: {source_help}"),
        ));
    };
    let view_key = read_orchard_view_key_file(view_key_file)?;
    Ok(view_key.address_raw_hex)
}

fn orchard_memo_bytes(memo_hex: Option<&str>) -> io::Result<[u8; ORCHARD_MEMO_BYTES]> {
    let Some(memo_hex) = memo_hex else {
        return Ok([0u8; ORCHARD_MEMO_BYTES]);
    };
    validate_hex_string("Orchard memo", memo_hex, Some(ORCHARD_MEMO_BYTES * 2))?;
    let bytes = hex_to_bytes(memo_hex).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Orchard memo hex is invalid: {error}"),
        )
    })?;
    bytes.try_into().map_err(|bytes: Vec<u8>| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Orchard memo must decode to {} bytes, got {}",
                ORCHARD_MEMO_BYTES,
                bytes.len()
            ),
        )
    })
}

fn derive_orchard_spending_key(
    master_seed_hex: &str,
    account_index: u32,
) -> io::Result<Zeroizing<[u8; 32]>> {
    let master_seed = Zeroizing::new(wallet_master_seed_bytes(master_seed_hex)?);
    orchard_spending_key_from_zip32_seed(&*master_seed, account_index).map_err(invalid_data)
}

fn orchard_full_viewing_key_bytes(full_viewing_key_hex: &str) -> io::Result<[u8; 96]> {
    let bytes = hex_to_bytes(full_viewing_key_hex).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Orchard full viewing key hex is invalid: {error}"),
        )
    })?;
    bytes.try_into().map_err(|bytes: Vec<u8>| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Orchard full viewing key hex must decode to 96 bytes, got {}",
                bytes.len()
            ),
        )
    })
}

fn orchard_wallet_key_report(
    path: &Path,
    key_file: &OrchardWalletKeyFile,
) -> OrchardWalletKeyReport {
    OrchardWalletKeyReport {
        schema: ORCHARD_WALLET_KEY_REPORT_SCHEMA.to_string(),
        key_file: path.display().to_string(),
        account_index: key_file.account_index,
        address_raw_hex: key_file.address_raw_hex.clone(),
    }
}

fn orchard_view_key_report(path: &Path, view_key: &OrchardViewKeyFile) -> OrchardViewKeyReport {
    OrchardViewKeyReport {
        schema: ORCHARD_VIEW_KEY_REPORT_SCHEMA.to_string(),
        view_key_file: path.display().to_string(),
        account_index: view_key.account_index,
        address_raw_hex: view_key.address_raw_hex.clone(),
        spend_authority_exported: false,
    }
}

fn orchard_action_receipt_id(
    genesis: &Genesis,
    action: &OrchardShieldedAction,
    verified: &VerifiedOrchardBundle,
    code: &str,
) -> io::Result<String> {
    direct_receipt_id(
        genesis,
        "postfiat.privacy.orchard_action_receipt.v1",
        &(
            action.pool_id.as_str(),
            verified.anchor.as_hex(),
            verified.value_balance,
            action.fee,
            verified
                .nullifiers
                .iter()
                .map(|nullifier| nullifier.as_hex())
                .collect::<Vec<_>>(),
            verified
                .output_commitments
                .iter()
                .map(|commitment| commitment.as_hex())
                .collect::<Vec<_>>(),
            code,
        ),
    )
}

fn shielded_swap_receipt_id(
    genesis: &Genesis,
    action: &ShieldedSwapAction,
    verified: &VerifiedShieldedSwap,
    code: &str,
) -> io::Result<String> {
    direct_receipt_id(
        genesis,
        "postfiat.privacy.shielded_swap_receipt.v1",
        &(
            action.pool_id.as_str(),
            verified.anchor.as_hex(),
            verified
                .nullifiers
                .iter()
                .map(|nullifier| nullifier.as_hex())
                .collect::<Vec<_>>(),
            verified
                .output_commitments
                .iter()
                .map(|commitment| commitment.as_hex())
                .collect::<Vec<_>>(),
            verified
                .output_asset_commitments
                .iter()
                .map(|commitment| commitment.as_hex())
                .collect::<Vec<_>>(),
            verified
                .output_value_commitments
                .iter()
                .map(|commitment| commitment.as_hex())
                .collect::<Vec<_>>(),
            verified.swap_binding_hash.as_hex(),
            verified.fee,
            code,
        ),
    )
}

fn asset_orchard_swap_receipt_id(
    genesis: &Genesis,
    action: &AssetOrchardSwapAction,
    verified: &VerifiedAssetOrchardSwap,
    code: &str,
) -> io::Result<String> {
    direct_receipt_id(
        genesis,
        "postfiat.privacy.asset_orchard_swap_receipt.v1",
        &(
            action.pool_id.as_str(),
            verified.pool_domain.as_hex(),
            verified.anchor.as_hex(),
            verified
                .nullifiers
                .iter()
                .map(|nullifier| nullifier.as_hex())
                .collect::<Vec<_>>(),
            verified
                .randomized_verification_keys
                .iter()
                .map(|rk| rk.as_hex())
                .collect::<Vec<_>>(),
            verified
                .output_commitments
                .iter()
                .map(|commitment| commitment.as_hex())
                .collect::<Vec<_>>(),
            verified.swap_binding_hash.as_hex(),
            verified.fee,
            code,
        ),
    )
}

fn asset_orchard_ingress_receipt_id(
    genesis: &Genesis,
    payload: &AssetOrchardIngressStatePayload<'_>,
    code: &str,
) -> io::Result<String> {
    direct_receipt_id(
        genesis,
        payload.receipt_domain,
        &(
            asset_transaction_tx_id(&payload.burn_transaction),
            payload.pool_id,
            payload.asset_id,
            payload.amount,
            payload.output_commitment,
            code,
        ),
    )
}

fn asset_orchard_egress_receipt_id(
    genesis: &Genesis,
    payload: &AssetOrchardEgressActionPayload,
    code: &str,
) -> io::Result<String> {
    direct_receipt_id(
        genesis,
        "postfiat.privacy.asset_orchard_egress_receipt.v1",
        &(
            payload.pool_id.as_str(),
            payload.to.as_str(),
            payload.asset_id.as_str(),
            payload.amount,
            payload.output_commitment.as_str(),
            payload.nullifier.as_str(),
            payload.randomized_verification_key.as_str(),
            code,
        ),
    )
}

fn asset_orchard_private_egress_receipt_id(
    genesis: &Genesis,
    payload: &AssetOrchardPrivateEgressActionPayload,
    code: &str,
) -> io::Result<String> {
    direct_receipt_id(
        genesis,
        "postfiat.privacy.asset_orchard_private_egress_receipt.v1",
        &(
            payload.pool_id.as_str(),
            payload.to.as_str(),
            payload.asset_id.as_str(),
            payload.amount,
            payload.fee,
            payload.anchor.as_str(),
            payload.nullifier.as_str(),
            payload.randomized_verification_key.as_str(),
            payload.exit_binding_hash.as_str(),
            payload.policy_id.as_str(),
            payload.disclosure_hash.as_str(),
            code,
        ),
    )
}
