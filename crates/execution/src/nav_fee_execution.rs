use postfiat_types::{
    nav_fee_attach_schedule, nav_fee_cancel_pending, nav_fee_claim_units,
    nav_fee_propose_schedule_update, LedgerState, NavFeeClaimOperation,
    NavFeeScheduleAttachOperation, NavFeeScheduleCancelOperation, NavFeeScheduleSetOperation,
};

use crate::{
    apply_prepared_issued_asset_credit, asset_gate, issued_asset_credit_recipient_line_index_gated,
    nav_profile_for_asset, prepare_issued_asset_credit_gated, AssetExecutionCompatibility,
    AssetGateOutcome, AssetMovementOpKind,
};

pub(crate) fn apply_nav_fee_schedule_attach(
    ledger: &mut LedgerState,
    operation: &NavFeeScheduleAttachOperation,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    if !compatibility.nav_fee_v1_active(block_height) {
        return Err((
            "nav_fee_not_active",
            format!("nav_fee_schedule_attach is not active at block height {block_height}"),
        ));
    }
    let nav_asset = ledger
        .nav_asset(&operation.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_nav_asset",
                format!("nav asset `{}` does not exist", operation.asset_id),
            )
        })?;
    if nav_asset.issuer != operation.issuer {
        return Err((
            "nav_issuer_mismatch",
            "nav_fee_schedule_attach issuer does not match nav asset issuer".to_string(),
        ));
    }
    let max_epoch_gap_blocks = nav_profile_for_asset(ledger, &nav_asset)
        .map(|profile| profile.max_epoch_gap_blocks)
        .filter(|gap| *gap > 0);
    let fee_state = nav_fee_attach_schedule(
        nav_asset.fee_state.as_ref(),
        &operation.issuer,
        &operation.schedule,
        block_height,
        nav_asset.nav_per_unit,
        max_epoch_gap_blocks,
    )?;
    let nav_mut = ledger.nav_asset_mut(&operation.asset_id).ok_or_else(|| {
        (
            "missing_nav_asset",
            "nav asset disappeared during fee schedule attach".to_string(),
        )
    })?;
    nav_mut.fee_state = Some(fee_state);
    Ok(())
}

pub(crate) fn apply_nav_fee_schedule_set(
    ledger: &mut LedgerState,
    operation: &NavFeeScheduleSetOperation,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    if !compatibility.nav_fee_v1_active(block_height) {
        return Err((
            "nav_fee_not_active",
            format!("nav_fee_schedule_set is not active at block height {block_height}"),
        ));
    }
    let nav_asset = ledger
        .nav_asset(&operation.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_nav_asset",
                format!("nav asset `{}` does not exist", operation.asset_id),
            )
        })?;
    if nav_asset.issuer != operation.issuer {
        return Err((
            "nav_issuer_mismatch",
            "nav_fee_schedule_set issuer does not match nav asset issuer".to_string(),
        ));
    }
    let max_epoch_gap_blocks = nav_profile_for_asset(ledger, &nav_asset)
        .map(|profile| profile.max_epoch_gap_blocks)
        .filter(|gap| *gap > 0);
    let next_state = nav_fee_propose_schedule_update(
        nav_asset.fee_state.as_ref(),
        &operation.issuer,
        &operation.schedule,
        block_height,
        max_epoch_gap_blocks,
        false,
    )?;
    let nav_mut = ledger.nav_asset_mut(&operation.asset_id).ok_or_else(|| {
        (
            "missing_nav_asset",
            "nav asset disappeared during fee schedule set".to_string(),
        )
    })?;
    nav_mut.fee_state = Some(next_state);
    Ok(())
}

pub(crate) fn apply_nav_fee_schedule_cancel(
    ledger: &mut LedgerState,
    operation: &NavFeeScheduleCancelOperation,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    if !compatibility.nav_fee_v1_active(block_height) {
        return Err((
            "nav_fee_not_active",
            format!("nav_fee_schedule_cancel is not active at block height {block_height}"),
        ));
    }
    let nav_asset = ledger
        .nav_asset(&operation.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_nav_asset",
                format!("nav asset `{}` does not exist", operation.asset_id),
            )
        })?;
    if nav_asset.issuer != operation.issuer {
        return Err((
            "nav_issuer_mismatch",
            "nav_fee_schedule_cancel issuer does not match nav asset issuer".to_string(),
        ));
    }
    let next_state = nav_fee_cancel_pending(nav_asset.fee_state.as_ref())?;
    let nav_mut = ledger.nav_asset_mut(&operation.asset_id).ok_or_else(|| {
        (
            "missing_nav_asset",
            "nav asset disappeared during fee schedule cancel".to_string(),
        )
    })?;
    nav_mut.fee_state = Some(next_state);
    Ok(())
}

pub(crate) fn apply_nav_fee_claim(
    ledger: &mut LedgerState,
    operation: &NavFeeClaimOperation,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    if !compatibility.nav_fee_v1_active(block_height) {
        return Err((
            "nav_fee_not_active",
            format!("nav_fee_claim is not active at block height {block_height}"),
        ));
    }
    let nav_asset = ledger
        .nav_asset(&operation.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_nav_asset",
                format!("nav asset `{}` does not exist", operation.asset_id),
            )
        })?;
    let asset = ledger
        .asset_definition(&operation.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_asset",
                format!("asset `{}` does not exist", operation.asset_id),
            )
        })?;
    let next_state = nav_fee_claim_units(
        nav_asset.fee_state.as_ref(),
        &operation.recipient,
        operation.amount,
    )?;

    // Claiming units out of the unclaimed bucket runs through `asset_gate(...)`
    // (whereas epoch accrual into the bucket bypasses `asset_gate` so finalization never deadlocks).
    match asset_gate(
        ledger,
        &asset,
        Some(&nav_asset.issuer),
        Some(&operation.recipient),
        AssetMovementOpKind::NavFeeClaim,
        block_height,
    ) {
        AssetGateOutcome::Allow => {}
        AssetGateOutcome::AllowReturnOnly => {
            return Err((
                "asset_gate_recipient_ineligible",
                "nav_fee_claim requires eligible fee recipient".to_string(),
            ));
        }
        AssetGateOutcome::RouteToHold { reason_code } => {
            return Err((
                reason_code,
                "nav_fee_claim rejected by asset_gate".to_string(),
            ));
        }
        AssetGateOutcome::Reject { code, message } => return Err((code, message)),
    }

    let to_index = issued_asset_credit_recipient_line_index_gated(
        ledger,
        &asset,
        &operation.recipient,
        operation.amount,
        "nav_fee_claim",
    )?;
    if compatibility.credential_did_v1_active(block_height)
        && ledger.trustlines[to_index].reserve_paid == 0
        && ledger.trustlines[to_index].holder_limit.is_none()
    {
        ledger.trustlines[to_index].implicit = true;
    }
    let (recipient_after, required_limit) = prepare_issued_asset_credit_gated(
        ledger,
        &asset,
        &operation.recipient,
        to_index,
        operation.amount,
        "nav_fee_claim",
    )?;
    apply_prepared_issued_asset_credit(ledger, to_index, recipient_after, required_limit);

    let nav_mut = ledger.nav_asset_mut(&operation.asset_id).ok_or_else(|| {
        (
            "missing_nav_asset",
            "nav asset disappeared during fee claim".to_string(),
        )
    })?;
    nav_mut.fee_state = Some(next_state);
    Ok(())
}
