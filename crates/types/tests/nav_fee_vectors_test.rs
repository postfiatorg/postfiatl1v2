use postfiat_types::{
    nav_fee_accrue_epoch, nav_fee_attach_schedule, nav_fee_cancel_pending,
    nav_fee_check_exit_capacity_update, nav_fee_check_fx_fix_fee_cap,
    nav_fee_check_non_issuer_attestation, nav_fee_check_unscheduled_spread_update,
    nav_fee_claim_units, nav_fee_propose_schedule_update, nav_fee_verify_nav_per_unit,
    NavFeeScheduleInputV1, NavFeeScheduleV1, NavFeeStateV1, NavReserveAttestation,
    PendingNavFeeScheduleV1, PROTOCOL_BLOCKS_PER_YEAR,
};
use serde_json::Value;

fn parse_u64(v: &Value, field: &str) -> u64 {
    match &v[field] {
        Value::String(s) => s
            .parse::<u64>()
            .unwrap_or_else(|err| panic!("invalid u64 string for `{field}`={s}: {err}")),
        Value::Number(n) => n
            .as_u64()
            .unwrap_or_else(|| panic!("invalid u64 number for `{field}`={n}")),
        other => panic!("missing/non-u64 field `{field}`: {other}"),
    }
}

fn parse_opt_u64(v: &Value, field: &str) -> Option<u64> {
    let val = v.get(field)?;
    if val.is_null() {
        return None;
    }
    Some(parse_u64(v, field))
}

fn parse_u128(v: &Value, field: &str) -> u128 {
    match &v[field] {
        Value::String(s) => s
            .parse::<u128>()
            .unwrap_or_else(|err| panic!("invalid u128 string for `{field}`={s}: {err}")),
        Value::Number(n) => n
            .as_u64()
            .map(u128::from)
            .unwrap_or_else(|| panic!("invalid u128 number for `{field}`={n}")),
        other => panic!("missing/non-u128 field `{field}`: {other}"),
    }
}

fn parse_u32(v: &Value, field: &str) -> u32 {
    u32::try_from(parse_u64(v, field)).unwrap_or_else(|_| panic!("u32 overflow for `{field}`"))
}

fn parse_u8(v: &Value, field: &str) -> u8 {
    u8::try_from(parse_u64(v, field)).unwrap_or_else(|_| panic!("u8 overflow for `{field}`"))
}

fn parse_schedule_input(v: &Value) -> NavFeeScheduleInputV1 {
    NavFeeScheduleInputV1 {
        annual_rate_bps: parse_u32(v, "annual_rate_bps"),
        hwm_rate_bps: parse_u32(v, "hwm_rate_bps"),
        issue_spread_bps: parse_u32(v, "issue_spread_bps"),
        redeem_spread_bps: parse_u32(v, "redeem_spread_bps"),
        fix_fee_cap_bps: parse_u32(v, "fix_fee_cap_bps"),
        recipient: v["recipient"].as_str().expect("recipient str").to_string(),
        increase_notice_blocks: parse_u64(v, "increase_notice_blocks"),
        accrual_cap_blocks: parse_u64(v, "accrual_cap_blocks"),
        rates_locked: v
            .get("rates_locked")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        disclosure_ref: v
            .get("disclosure_ref")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    }
}

fn parse_schedule(v: &Value) -> NavFeeScheduleV1 {
    let input = parse_schedule_input(v);
    NavFeeScheduleV1 {
        annual_rate_bps: input.annual_rate_bps,
        hwm_rate_bps: input.hwm_rate_bps,
        issue_spread_bps: input.issue_spread_bps,
        redeem_spread_bps: input.redeem_spread_bps,
        fix_fee_cap_bps: input.fix_fee_cap_bps,
        recipient: input.recipient,
        increase_notice_blocks: input.increase_notice_blocks,
        accrual_cap_blocks: input.accrual_cap_blocks,
        rates_locked: input.rates_locked,
        disclosure_ref: input.disclosure_ref,
        attached_at_height: parse_opt_u64(v, "attached_at_height").unwrap_or(0),
    }
}

fn parse_fee_state(v: &Value) -> NavFeeStateV1 {
    let pending = v
        .get("pending")
        .filter(|p| !p.is_null())
        .map(|p| PendingNavFeeScheduleV1 {
            schedule: parse_schedule(&p["schedule"]),
            proposed_at_height: parse_u64(p, "proposed_at_height"),
            effective_at_height: parse_u64(p, "effective_at_height"),
            is_increase: p["is_increase"].as_bool().expect("is_increase bool"),
        });
    NavFeeStateV1 {
        active: parse_schedule(&v["active"]),
        pending,
        high_water_mark_nav_per_unit: parse_u64(v, "high_water_mark_nav_per_unit"),
        last_accrual_height: parse_u64(v, "last_accrual_height"),
        unclaimed_fee_units: parse_u64(v, "unclaimed_fee_units"),
        total_accrued_fee_units: parse_u64(v, "total_accrued_fee_units"),
        total_claimed_fee_units: parse_u64(v, "total_claimed_fee_units"),
    }
}

fn parse_opt_fee_state(v: &Value, field: &str) -> Option<NavFeeStateV1> {
    let val = v.get(field)?;
    if val.is_null() {
        return None;
    }
    Some(parse_fee_state(val))
}

fn assert_accrual_matches(
    vector_id: &str,
    actual: &postfiat_types::NavFeeAccrualV1,
    expected: &Value,
) {
    assert_eq!(
        actual.asset_id,
        expected["asset_id"].as_str().unwrap(),
        "{vector_id}: asset_id"
    );
    assert_eq!(
        actual.epoch,
        parse_u64(expected, "epoch"),
        "{vector_id}: epoch"
    );
    assert_eq!(
        actual.block_height,
        parse_u64(expected, "block_height"),
        "{vector_id}: block_height"
    );
    assert_eq!(
        actual.blocks_per_year,
        parse_u64(expected, "blocks_per_year"),
        "{vector_id}: blocks_per_year"
    );
    assert_eq!(
        actual.delta_blocks,
        parse_u64(expected, "delta_blocks"),
        "{vector_id}: delta_blocks"
    );
    assert_eq!(
        actual.verified_net_assets,
        parse_u64(expected, "verified_net_assets"),
        "{vector_id}: verified_net_assets"
    );
    assert_eq!(
        actual.supply_before,
        parse_u64(expected, "supply_before"),
        "{vector_id}: supply_before"
    );
    assert_eq!(
        actual.nav_before,
        parse_u64(expected, "nav_before"),
        "{vector_id}: nav_before"
    );
    assert_eq!(
        actual.hwm_before,
        parse_u64(expected, "hwm_before"),
        "{vector_id}: hwm_before"
    );
    assert_eq!(
        actual.annual_rate_bps_applied,
        parse_u32(expected, "annual_rate_bps_applied"),
        "{vector_id}: annual_rate_bps_applied"
    );
    assert_eq!(
        actual.hwm_rate_bps_applied,
        parse_u32(expected, "hwm_rate_bps_applied"),
        "{vector_id}: hwm_rate_bps_applied"
    );
    assert_eq!(
        actual.time_fee_value,
        parse_u64(expected, "time_fee_value"),
        "{vector_id}: time_fee_value"
    );
    assert_eq!(
        actual.nav_after_time_fee,
        parse_u64(expected, "nav_after_time_fee"),
        "{vector_id}: nav_after_time_fee"
    );
    assert_eq!(
        actual.hwm_fee_value,
        parse_u64(expected, "hwm_fee_value"),
        "{vector_id}: hwm_fee_value"
    );
    assert_eq!(
        actual.total_fee_value,
        parse_u64(expected, "total_fee_value"),
        "{vector_id}: total_fee_value"
    );
    assert_eq!(
        actual.minted_fee_units,
        parse_u64(expected, "minted_fee_units"),
        "{vector_id}: minted_fee_units"
    );
    assert_eq!(
        actual.clipped_fee_units,
        parse_u64(expected, "clipped_fee_units"),
        "{vector_id}: clipped_fee_units"
    );
    assert_eq!(
        actual.supply_after,
        parse_u64(expected, "supply_after"),
        "{vector_id}: supply_after"
    );
    assert_eq!(
        actual.nav_after,
        parse_u64(expected, "nav_after"),
        "{vector_id}: nav_after"
    );
    assert_eq!(
        actual.hwm_after,
        parse_u64(expected, "hwm_after"),
        "{vector_id}: hwm_after"
    );
    assert_eq!(
        actual.reason_code,
        expected["reason_code"].as_str().unwrap(),
        "{vector_id}: reason_code"
    );
}

#[test]
fn nav_fee_all_120_cross_language_vectors_pass() {
    let raw = include_str!("vectors/nav_fee_vectors_v1.json");
    let doc: Value = serde_json::from_str(raw).expect("parse nav_fee_vectors_v1.json");
    assert_eq!(doc["schema"].as_str(), Some("postfiat-nav-fee-vectors-v1"));
    let vectors = doc["vectors"].as_array().expect("vectors array");
    assert_eq!(vectors.len(), 120, "expected exactly 120 NAV fee vectors");

    for vector in vectors {
        let id = vector["id"].as_str().expect("vector id");
        let kind = vector["kind"].as_str().expect("vector kind");
        let input = &vector["input"];
        let expected = &vector["expected"];
        let expect_ok = expected["ok"].as_bool().expect("expected.ok bool");

        match kind {
            "universal_nav" => {
                let res = nav_fee_verify_nav_per_unit(
                    parse_u64(input, "verified_net_assets"),
                    parse_u64(input, "circulating_supply"),
                    parse_u8(input, "precision"),
                    parse_u64(input, "claimed_nav_per_unit"),
                    parse_opt_u64(input, "live_ledger_supply"),
                );
                if expect_ok {
                    let (nav, rem) =
                        res.unwrap_or_else(|(c, m)| panic!("{id}: expected ok, got err {c}: {m}"));
                    assert_eq!(
                        nav,
                        parse_u64(expected, "expected_nav_per_unit"),
                        "{id}: expected_nav_per_unit"
                    );
                    assert_eq!(
                        rem,
                        parse_u128(expected, "floor_remainder_scaled"),
                        "{id}: floor_remainder_scaled"
                    );
                } else {
                    let (code, _) = res.expect_err(&format!("{id}: expected error"));
                    assert_eq!(
                        code,
                        expected["error_code"].as_str().unwrap(),
                        "{id}: error_code"
                    );
                }
            }
            "schedule_validation" => {
                let subkind = input["subkind"].as_str().expect("subkind");
                match subkind {
                    "attach" => {
                        let existing = parse_opt_fee_state(input, "existing_fee_state");
                        let sched_in = parse_schedule_input(&input["schedule"]);
                        let res = nav_fee_attach_schedule(
                            existing.as_ref(),
                            input["issuer"].as_str().unwrap(),
                            &sched_in,
                            parse_u64(input, "block_height"),
                            parse_opt_u64(input, "current_nav_per_unit").unwrap_or(0),
                            parse_opt_u64(input, "max_epoch_gap_blocks"),
                        );
                        if expect_ok {
                            let state = res
                                .unwrap_or_else(|(c, m)| panic!("{id}: expected ok, got {c}: {m}"));
                            if let Some(h) = parse_opt_u64(expected, "attached_at_height") {
                                assert_eq!(
                                    state.active.attached_at_height, h,
                                    "{id}: attached_at_height"
                                );
                            }
                            if let Some(h) = parse_opt_u64(expected, "last_accrual_height") {
                                assert_eq!(
                                    state.last_accrual_height, h,
                                    "{id}: last_accrual_height"
                                );
                            }
                            if let Some(h) = parse_opt_u64(expected, "high_water_mark_nav_per_unit")
                            {
                                assert_eq!(state.high_water_mark_nav_per_unit, h, "{id}: hwm");
                            }
                        } else {
                            let (code, _) = res.expect_err(&format!("{id}: expected error"));
                            assert_eq!(
                                code,
                                expected["error_code"].as_str().unwrap(),
                                "{id}: error_code"
                            );
                        }
                    }
                    "unscheduled_spread" => {
                        let res = nav_fee_check_unscheduled_spread_update(
                            parse_u32(input, "current_issue_spread_bps"),
                            parse_u32(input, "current_redeem_spread_bps"),
                            parse_u32(input, "new_issue_spread_bps"),
                            parse_u32(input, "new_redeem_spread_bps"),
                            parse_u64(input, "proposed_at_height"),
                            parse_u64(input, "effective_at_height"),
                        );
                        if expect_ok {
                            res.unwrap_or_else(|(c, m)| panic!("{id}: expected ok, got {c}: {m}"));
                        } else {
                            let (code, _) = res.expect_err(&format!("{id}: expected error"));
                            assert_eq!(
                                code,
                                expected["error_code"].as_str().unwrap(),
                                "{id}: error_code"
                            );
                        }
                    }
                    "fx_fix_cap" => {
                        let fs = parse_opt_fee_state(input, "fee_state");
                        let res =
                            nav_fee_check_fx_fix_fee_cap(parse_u32(input, "fee_bps"), fs.as_ref());
                        if expect_ok {
                            res.unwrap_or_else(|(c, m)| panic!("{id}: expected ok, got {c}: {m}"));
                        } else {
                            let (code, _) = res.expect_err(&format!("{id}: expected error"));
                            assert_eq!(
                                code,
                                expected["error_code"].as_str().unwrap(),
                                "{id}: error_code"
                            );
                        }
                    }
                    "non_issuer_attestor" => {
                        let fs = parse_opt_fee_state(input, "fee_state");
                        let attestations: Vec<NavReserveAttestation> = input["attestations"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|a| NavReserveAttestation {
                                attestor: a["attestor"].as_str().unwrap().to_string(),
                                pass: a["pass"].as_bool().unwrap(),
                                observation_root: "00".repeat(48),
                                attested_at_height: 0,
                            })
                            .collect();
                        let res = nav_fee_check_non_issuer_attestation(
                            input["issuer"].as_str().unwrap(),
                            fs.as_ref(),
                            parse_u64(input, "attested_value"),
                            &attestations,
                        );
                        if expect_ok {
                            res.unwrap_or_else(|(c, m)| panic!("{id}: expected ok, got {c}: {m}"));
                        } else {
                            let (code, _) = res.expect_err(&format!("{id}: expected error"));
                            assert_eq!(
                                code,
                                expected["error_code"].as_str().unwrap(),
                                "{id}: error_code"
                            );
                        }
                    }
                    other => panic!("{id}: unknown schedule_validation subkind `{other}`"),
                }
            }
            "pending_notice_and_lifecycle" => {
                let action = input["action"].as_str().expect("action");
                let fs = parse_opt_fee_state(input, "fee_state");
                match action {
                    "propose" => {
                        let sched_in = parse_schedule_input(&input["schedule"]);
                        let res = nav_fee_propose_schedule_update(
                            fs.as_ref(),
                            input["issuer"].as_str().unwrap(),
                            &sched_in,
                            parse_u64(input, "block_height"),
                            parse_opt_u64(input, "max_epoch_gap_blocks"),
                            input
                                .get("recipient_cosigned")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                        );
                        if expect_ok {
                            let next = res
                                .unwrap_or_else(|(c, m)| panic!("{id}: expected ok, got {c}: {m}"));
                            let pending = next.pending.as_ref().expect("pending schedule");
                            assert_eq!(
                                pending.is_increase,
                                expected["is_increase"].as_bool().unwrap(),
                                "{id}: is_increase"
                            );
                            assert_eq!(
                                pending.proposed_at_height,
                                parse_u64(expected, "proposed_at_height"),
                                "{id}: proposed_at_height"
                            );
                            assert_eq!(
                                pending.effective_at_height,
                                parse_u64(expected, "effective_at_height"),
                                "{id}: effective_at_height"
                            );
                        } else {
                            let (code, _) = res.expect_err(&format!("{id}: expected error"));
                            assert_eq!(
                                code,
                                expected["error_code"].as_str().unwrap(),
                                "{id}: error_code"
                            );
                        }
                    }
                    "cancel" => {
                        let res = nav_fee_cancel_pending(fs.as_ref());
                        if expect_ok {
                            let next = res
                                .unwrap_or_else(|(c, m)| panic!("{id}: expected ok, got {c}: {m}"));
                            assert!(next.pending.is_none(), "{id}: pending cleared");
                        } else {
                            let (code, _) = res.expect_err(&format!("{id}: expected error"));
                            assert_eq!(
                                code,
                                expected["error_code"].as_str().unwrap(),
                                "{id}: error_code"
                            );
                        }
                    }
                    "exit_capacity" => {
                        let res = nav_fee_check_exit_capacity_update(
                            fs.as_ref(),
                            parse_u64(input, "current_redeem_capacity_atoms"),
                            parse_u64(input, "new_redeem_capacity_atoms"),
                        );
                        if expect_ok {
                            res.unwrap_or_else(|(c, m)| panic!("{id}: expected ok, got {c}: {m}"));
                        } else {
                            let (code, _) = res.expect_err(&format!("{id}: expected error"));
                            assert_eq!(
                                code,
                                expected["error_code"].as_str().unwrap(),
                                "{id}: error_code"
                            );
                        }
                    }
                    "claim" => {
                        let res = nav_fee_claim_units(
                            fs.as_ref(),
                            input["caller"].as_str().unwrap(),
                            parse_u64(input, "amount"),
                        );
                        if expect_ok {
                            let next = res
                                .unwrap_or_else(|(c, m)| panic!("{id}: expected ok, got {c}: {m}"));
                            assert_eq!(
                                next.unclaimed_fee_units,
                                parse_u64(expected, "unclaimed_remaining"),
                                "{id}: unclaimed_remaining"
                            );
                            assert_eq!(
                                next.total_claimed_fee_units,
                                parse_u64(expected, "total_claimed_fee_units"),
                                "{id}: total_claimed_fee_units"
                            );
                        } else {
                            let (code, _) = res.expect_err(&format!("{id}: expected error"));
                            assert_eq!(
                                code,
                                expected["error_code"].as_str().unwrap(),
                                "{id}: error_code"
                            );
                        }
                    }
                    other => panic!("{id}: unknown lifecycle action `{other}`"),
                }
            }
            "single_epoch_accrual" => {
                let fs = parse_fee_state(&input["fee_state"]);
                let res = nav_fee_accrue_epoch(
                    input["asset_id"].as_str().unwrap(),
                    parse_u64(input, "epoch"),
                    parse_u64(input, "block_height"),
                    parse_u64(input, "verified_net_assets"),
                    parse_u64(input, "supply_before"),
                    parse_u8(input, "precision"),
                    &fs,
                    parse_opt_u64(input, "max_supply"),
                    PROTOCOL_BLOCKS_PER_YEAR,
                );
                if expect_ok {
                    let (next_state, accrual) =
                        res.unwrap_or_else(|(c, m)| panic!("{id}: expected ok, got {c}: {m}"));
                    assert_accrual_matches(id, &accrual, &expected["accrual"]);
                    assert_eq!(
                        next_state.last_accrual_height,
                        parse_u64(expected, "last_accrual_height"),
                        "{id}: last_accrual_height"
                    );
                    assert_eq!(
                        next_state.active.annual_rate_bps,
                        parse_u32(expected, "next_active_annual_rate_bps"),
                        "{id}: next_active_annual_rate_bps"
                    );
                    assert_eq!(
                        next_state.active.hwm_rate_bps,
                        parse_u32(expected, "next_active_hwm_rate_bps"),
                        "{id}: next_active_hwm_rate_bps"
                    );
                    assert_eq!(
                        next_state.pending.is_none(),
                        expected["pending_cleared"].as_bool().unwrap(),
                        "{id}: pending_cleared"
                    );
                    assert_eq!(
                        next_state.unclaimed_fee_units,
                        parse_u64(expected, "unclaimed_fee_units"),
                        "{id}: unclaimed_fee_units"
                    );
                    assert_eq!(
                        next_state.total_accrued_fee_units,
                        parse_u64(expected, "total_accrued_fee_units"),
                        "{id}: total_accrued_fee_units"
                    );
                } else {
                    let (code, _) = res.expect_err(&format!("{id}: expected error"));
                    assert_eq!(
                        code,
                        expected["error_code"].as_str().unwrap(),
                        "{id}: error_code"
                    );
                }
            }
            "multi_epoch_compounding" => {
                let asset_id = input["asset_id"].as_str().unwrap();
                let issuer = input["issuer"].as_str().unwrap();
                let precision = parse_u8(input, "precision");
                let max_supply = parse_opt_u64(input, "max_supply");
                let mut supply = parse_u64(input, "initial_supply");
                let mut state = parse_fee_state(&input["initial_fee_state"]);
                let epochs = input["epochs"].as_array().unwrap();
                let expected_accruals = expected["accruals"].as_array().unwrap();
                assert_eq!(epochs.len(), expected_accruals.len(), "{id}: epoch count");

                for (idx, step) in epochs.iter().enumerate() {
                    if let Some(propose) = step.get("propose_update").filter(|v| !v.is_null()) {
                        let sched_in = parse_schedule_input(&propose["schedule"]);
                        let recipient_cosigned = propose
                            .get("recipient_cosigned")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                        state = nav_fee_propose_schedule_update(
                            Some(&state),
                            issuer,
                            &sched_in,
                            parse_u64(propose, "block_height"),
                            None,
                            recipient_cosigned,
                        )
                        .unwrap_or_else(|(c, m)| {
                            panic!("{id}[{idx}]: propose_update failed {c}: {m}")
                        });
                    }
                    if step
                        .get("cancel_pending")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                    {
                        state = nav_fee_cancel_pending(Some(&state)).unwrap_or_else(|(c, m)| {
                            panic!("{id}[{idx}]: cancel_pending failed {c}: {m}")
                        });
                    }
                    if let Some(mint) = parse_opt_u64(step, "external_mint_units") {
                        supply = supply.checked_add(mint).expect("external mint overflow");
                    }
                    if let Some(redeem) = parse_opt_u64(step, "external_redeem_units") {
                        supply = supply
                            .checked_sub(redeem)
                            .expect("external redeem underflow");
                    }
                    let (next_state, accrual) = nav_fee_accrue_epoch(
                        asset_id,
                        parse_u64(step, "epoch"),
                        parse_u64(step, "block_height"),
                        parse_u64(step, "verified_net_assets"),
                        supply,
                        precision,
                        &state,
                        max_supply,
                        PROTOCOL_BLOCKS_PER_YEAR,
                    )
                    .unwrap_or_else(|(c, m)| panic!("{id}[{idx}]: accrue failed {c}: {m}"));
                    assert_accrual_matches(
                        &format!("{id}[{idx}]"),
                        &accrual,
                        &expected_accruals[idx],
                    );
                    state = next_state;
                    supply = accrual.supply_after;
                    if let Some(claim) = step.get("claim").filter(|v| !v.is_null()) {
                        state = nav_fee_claim_units(
                            Some(&state),
                            claim["caller"].as_str().unwrap(),
                            parse_u64(claim, "amount"),
                        )
                        .unwrap_or_else(|(c, m)| panic!("{id}[{idx}]: claim failed {c}: {m}"));
                    }
                }
                assert!(expect_ok, "{id}: multi_epoch expects ok");
                assert_eq!(
                    supply,
                    parse_u64(expected, "final_supply"),
                    "{id}: final_supply"
                );
                assert_eq!(
                    state.high_water_mark_nav_per_unit,
                    parse_u64(expected, "final_hwm"),
                    "{id}: final_hwm"
                );
                assert_eq!(
                    state.active.annual_rate_bps,
                    parse_u32(expected, "final_active_annual_rate_bps"),
                    "{id}: final_active_annual_rate_bps"
                );
                assert_eq!(
                    state.active.hwm_rate_bps,
                    parse_u32(expected, "final_active_hwm_rate_bps"),
                    "{id}: final_active_hwm_rate_bps"
                );
                assert_eq!(
                    state.active.recipient,
                    expected["final_active_recipient"].as_str().unwrap(),
                    "{id}: final_active_recipient"
                );
                assert_eq!(
                    state.pending.is_some(),
                    expected["final_has_pending"].as_bool().unwrap(),
                    "{id}: final_has_pending"
                );
                assert_eq!(
                    state.unclaimed_fee_units,
                    parse_u64(expected, "final_unclaimed_fee_units"),
                    "{id}: final_unclaimed_fee_units"
                );
                assert_eq!(
                    state.total_accrued_fee_units,
                    parse_u64(expected, "final_total_accrued_fee_units"),
                    "{id}: final_total_accrued_fee_units"
                );
                assert_eq!(
                    state.total_claimed_fee_units,
                    parse_u64(expected, "final_total_claimed_fee_units"),
                    "{id}: final_total_claimed_fee_units"
                );
            }
            other => panic!("{id}: unknown vector kind `{other}`"),
        }
    }
}
