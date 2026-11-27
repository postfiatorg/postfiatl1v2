#!/usr/bin/env python3
"""Deterministic generator for sdk/navcoin/vectors/nav_fee_vectors_v1.json.

Produces 120 exact-atom cross-language test vectors covering:
  - universal_nav (16 vectors)
  - schedule_validation (24 vectors)
  - single_epoch_accrual (40 vectors)
  - pending_notice_and_lifecycle (20 vectors)
  - multi_epoch_compounding (20 vectors)

Every generated accrual vector is also checked against strict economic and
integer-rounding invariants (dust stays with holders, universal post-fee NAV,
monotonic HWM, zero hidden carry-forward debt).
"""

from __future__ import annotations

import json
import pathlib
import sys
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "python"))

from postfiat_rpc.navcoin_sdk import (  # noqa: E402
    NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS,
    PROTOCOL_BLOCKS_PER_30_DAYS,
    PROTOCOL_BLOCKS_PER_7_DAYS,
    PROTOCOL_BLOCKS_PER_DAY,
    PROTOCOL_BLOCKS_PER_YEAR,
    U64_MAX,
    compute_nav_per_unit,
    evaluate_vector,
)

ISSUER = "pf1111111111111111111111111111111111111111"
RECIPIENT = "pf3333333333333333333333333333333333333333"
RECIPIENT_2 = "pf4444444444444444444444444444444444444444"
ATTESTOR = "pf5555555555555555555555555555555555555555"
OTHER_ACCOUNT = "pf6666666666666666666666666666666666666666"
ASSET_ID = "99bb" * 24


def make_schedule(
    *,
    annual: int = 100,
    hwm: int = 1000,
    issue: int = 50,
    redeem: int = 50,
    fix: int = 25,
    recipient: str = RECIPIENT,
    notice: int = NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS,
    cap: int = PROTOCOL_BLOCKS_PER_DAY,
    locked: bool = False,
    disclosure_ref: str | None = None,
    attached_at_height: int = 100_000,
) -> dict[str, Any]:
    out: dict[str, Any] = {
        "annual_rate_bps": annual,
        "hwm_rate_bps": hwm,
        "issue_spread_bps": issue,
        "redeem_spread_bps": redeem,
        "fix_fee_cap_bps": fix,
        "recipient": recipient,
        "increase_notice_blocks": str(notice),
        "accrual_cap_blocks": str(cap),
        "rates_locked": locked,
        "attached_at_height": str(attached_at_height),
    }
    if disclosure_ref is not None:
        out["disclosure_ref"] = disclosure_ref
    return out


def make_fee_state(
    *,
    active: dict[str, Any] | None = None,
    pending: dict[str, Any] | None = None,
    hwm: int = 100_000_000,
    last_height: int = 100_000,
    unclaimed: int = 0,
    total_accrued: int = 0,
    total_claimed: int = 0,
) -> dict[str, Any]:
    return {
        "active": active or make_schedule(),
        "pending": pending,
        "high_water_mark_nav_per_unit": str(hwm),
        "last_accrual_height": str(last_height),
        "unclaimed_fee_units": str(unclaimed),
        "total_accrued_fee_units": str(total_accrued),
        "total_claimed_fee_units": str(total_claimed),
    }


def assert_accrual_invariants(acc: dict[str, Any], precision: int) -> None:
    v = int(acc["verified_net_assets"])
    s_before = int(acc["supply_before"])
    s_after = int(acc["supply_after"])
    minted = int(acc["minted_fee_units"])
    nav_after = int(acc["nav_after"])
    hwm_before = int(acc["hwm_before"])
    hwm_after = int(acc["hwm_after"])
    f_m = int(acc["time_fee_value"])
    f_p = int(acc["hwm_fee_value"])
    f_tot = int(acc["total_fee_value"])
    u = 10**precision

    assert f_tot == f_m + f_p, f"total_fee_value mismatch: {acc}"
    assert s_after == s_before + minted, f"supply conservation mismatch: {acc}"
    assert hwm_after >= hwm_before, f"HWM decreased: {acc}"
    if s_after > 0:
        assert nav_after == (v * u) // s_after, f"universal post-fee NAV mismatch: {acc}"
        # Dust stays with holders: minted units valued at post-fee NAV <= total_fee_value
        assert minted * nav_after <= f_tot * u, f"fee dilution exceeded total_fee_value: {acc}"
    else:
        assert nav_after == 0


def build_vectors() -> list[dict[str, Any]]:
    vectors: list[dict[str, Any]] = []

    def add(vid: str, kind: str, description: str, inp: dict[str, Any]) -> dict[str, Any]:
        vec = {"id": vid, "kind": kind, "description": description, "input": inp}
        expected = evaluate_vector(vec)
        vec["expected"] = expected
        if kind == "single_epoch_accrual":
            assert_accrual_invariants(expected["accrual"], inp["precision"])
        elif kind == "multi_epoch_compounding":
            for acc in expected["accruals"]:
                assert_accrual_invariants(acc, inp["precision"])
        vectors.append(vec)
        return expected

    # =========================================================================
    # 1. universal_nav (16 vectors)
    # =========================================================================
    add(
        "UNAV-001",
        "universal_nav",
        "Exact divisibility at precision 8 (Train B worked example pre-fee)",
        {
            "verified_net_assets": "1100000000000",
            "circulating_supply": "1000000000000",
            "precision": 8,
            "claimed_nav_per_unit": "110000000",
        },
    )
    add(
        "UNAV-002",
        "universal_nav",
        "Non-divisible floor truncation at precision 0 (10 / 3 = 3 rem 1)",
        {
            "verified_net_assets": "10",
            "circulating_supply": "3",
            "precision": 0,
            "claimed_nav_per_unit": "3",
        },
    )
    add(
        "UNAV-003",
        "universal_nav",
        "Non-divisible floor truncation at precision 6 (10 * 1e6 / 3 = 3333333 rem 1)",
        {
            "verified_net_assets": "10",
            "circulating_supply": "3",
            "precision": 6,
            "claimed_nav_per_unit": "3333333",
        },
    )
    add(
        "UNAV-004",
        "universal_nav",
        "Reject 1-atom understatement (closes hidden-fee gap on zero-fee NAVCoins)",
        {
            "verified_net_assets": "10",
            "circulating_supply": "3",
            "precision": 6,
            "claimed_nav_per_unit": "3333332",
        },
    )
    add(
        "UNAV-005",
        "universal_nav",
        "Reject 1-atom overstatement (ceiling instead of floor)",
        {
            "verified_net_assets": "10",
            "circulating_supply": "3",
            "precision": 6,
            "claimed_nav_per_unit": "3333334",
        },
    )
    add(
        "UNAV-006",
        "universal_nav",
        "Zero circulating supply returns 0 nav_per_unit without division by zero",
        {
            "verified_net_assets": "500000000",
            "circulating_supply": "0",
            "precision": 6,
            "claimed_nav_per_unit": "0",
        },
    )
    add(
        "UNAV-007",
        "universal_nav",
        "Zero net assets with positive supply yields 0 nav_per_unit",
        {
            "verified_net_assets": "0",
            "circulating_supply": "1000000",
            "precision": 6,
            "claimed_nav_per_unit": "0",
        },
    )
    add(
        "UNAV-008",
        "universal_nav",
        "Tiny net assets truncating to 0 nav_per_unit (dust stays in remainder)",
        {
            "verified_net_assets": "1",
            "circulating_supply": "1000000000000",
            "precision": 6,
            "claimed_nav_per_unit": "0",
        },
    )
    add(
        "UNAV-009",
        "universal_nav",
        "Fresh live_ledger_supply check passes when supply matches exactly",
        {
            "verified_net_assets": "982300000",
            "circulating_supply": "1000",
            "precision": 0,
            "claimed_nav_per_unit": "982300",
            "live_ledger_supply": "1000",
        },
    )
    add(
        "UNAV-010",
        "universal_nav",
        "Stale supply snapshot rejected when circulating_supply != live_ledger_supply",
        {
            "verified_net_assets": "982300000",
            "circulating_supply": "1000",
            "precision": 0,
            "claimed_nav_per_unit": "982300",
            "live_ledger_supply": "1001",
        },
    )
    add(
        "UNAV-011",
        "universal_nav",
        "Precision 2 exact floor check",
        {
            "verified_net_assets": "999999999",
            "circulating_supply": "7",
            "precision": 2,
            "claimed_nav_per_unit": str((999_999_999 * 100) // 7),
        },
    )
    add(
        "UNAV-012",
        "universal_nav",
        "Precision 12 exact floor check",
        {
            "verified_net_assets": "123456789012345",
            "circulating_supply": "9876543210987",
            "precision": 12,
            "claimed_nav_per_unit": str((123_456_789_012_345 * 10**12) // 9_876_543_210_987),
        },
    )
    add(
        "UNAV-013",
        "universal_nav",
        "u256 intermediate at u64::MAX and precision 18 (overflows u128 and float64)",
        {
            "verified_net_assets": str(U64_MAX),
            "circulating_supply": str(U64_MAX),
            "precision": 18,
            "claimed_nav_per_unit": str(10**18),
        },
    )
    add(
        "UNAV-014",
        "universal_nav",
        "IEEE-754 float64 precision trap above 2^53 (exact integer floor required)",
        {
            "verified_net_assets": "9007199254740993",
            "circulating_supply": "9007199254740992",
            "precision": 18,
            "claimed_nav_per_unit": str((9_007_199_254_740_993 * 10**18) // 9_007_199_254_740_992),
        },
    )
    add(
        "UNAV-015",
        "universal_nav",
        "Float64 trap rejected if off by 1 atom near 2^60",
        {
            "verified_net_assets": "1152921504606846975",
            "circulating_supply": "1152921504606846973",
            "precision": 12,
            "claimed_nav_per_unit": str(
                ((1_152_921_504_606_846_975 * 10**12) // 1_152_921_504_606_846_973) - 1
            ),
        },
    )
    add(
        "UNAV-016",
        "universal_nav",
        "Post-fee worked example universal NAV verification (V=1.1e12, S'=1009265979203, prec=8)",
        {
            "verified_net_assets": "1100000000000",
            "circulating_supply": "1009265979203",
            "precision": 8,
            "claimed_nav_per_unit": "108990100",
        },
    )

    # =========================================================================
    # 2. schedule_validation (24 vectors)
    # =========================================================================
    add(
        "SVAL-001",
        "schedule_validation",
        "Valid zero-rate schedule attaches cleanly at block height 50000",
        {
            "subkind": "attach",
            "issuer": ISSUER,
            "block_height": "50000",
            "current_nav_per_unit": "1000000",
            "schedule": make_schedule(annual=0, hwm=0, issue=0, redeem=0, fix=0),
        },
    )
    add(
        "SVAL-002",
        "schedule_validation",
        "Valid exact max protocol caps (500 annual, 5000 hwm, 500/500 spread, 100 fix, 604800 notice)",
        {
            "subkind": "attach",
            "issuer": ISSUER,
            "block_height": "120000",
            "current_nav_per_unit": "0",
            "schedule": make_schedule(
                annual=500, hwm=5000, issue=500, redeem=500, fix=100, notice=604_800
            ),
        },
    )
    add(
        "SVAL-003",
        "schedule_validation",
        "Valid longer notice (30d) and exact 128-byte disclosure_ref",
        {
            "subkind": "attach",
            "issuer": ISSUER,
            "block_height": "120000",
            "current_nav_per_unit": "100000000",
            "schedule": make_schedule(
                notice=PROTOCOL_BLOCKS_PER_30_DAYS,
                disclosure_ref="ipfs://" + ("a" * 121),
            ),
        },
    )
    add(
        "SVAL-004",
        "schedule_validation",
        "Reject annual_rate_bps = 501 (> 5% cap)",
        {
            "subkind": "attach",
            "issuer": ISSUER,
            "block_height": "1000",
            "schedule": make_schedule(annual=501),
        },
    )
    add(
        "SVAL-005",
        "schedule_validation",
        "Reject hwm_rate_bps = 5001 (> 50% cap)",
        {
            "subkind": "attach",
            "issuer": ISSUER,
            "block_height": "1000",
            "schedule": make_schedule(hwm=5001),
        },
    )
    add(
        "SVAL-006",
        "schedule_validation",
        "Reject issue_spread_bps = 501 (> 5% cap)",
        {
            "subkind": "attach",
            "issuer": ISSUER,
            "block_height": "1000",
            "schedule": make_schedule(issue=501),
        },
    )
    add(
        "SVAL-007",
        "schedule_validation",
        "Reject redeem_spread_bps = 501 (> 5% cap)",
        {
            "subkind": "attach",
            "issuer": ISSUER,
            "block_height": "1000",
            "schedule": make_schedule(redeem=501),
        },
    )
    add(
        "SVAL-008",
        "schedule_validation",
        "Reject fix_fee_cap_bps = 101 (> 1% cap)",
        {
            "subkind": "attach",
            "issuer": ISSUER,
            "block_height": "1000",
            "schedule": make_schedule(fix=101),
        },
    )
    add(
        "SVAL-009",
        "schedule_validation",
        "Reject increase_notice_blocks = 604799 (1 block below 7d minimum)",
        {
            "subkind": "attach",
            "issuer": ISSUER,
            "block_height": "1000",
            "schedule": make_schedule(notice=604_799),
        },
    )
    add(
        "SVAL-010",
        "schedule_validation",
        "Reject increase_notice_blocks = 0",
        {
            "subkind": "attach",
            "issuer": ISSUER,
            "block_height": "1000",
            "schedule": make_schedule(notice=0),
        },
    )
    add(
        "SVAL-011",
        "schedule_validation",
        "Reject recipient equal to issuer account",
        {
            "subkind": "attach",
            "issuer": ISSUER,
            "block_height": "1000",
            "schedule": make_schedule(recipient=ISSUER),
        },
    )
    add(
        "SVAL-012",
        "schedule_validation",
        "Reject malformed recipient address",
        {
            "subkind": "attach",
            "issuer": ISSUER,
            "block_height": "1000",
            "schedule": make_schedule(recipient="not-a-pf-address"),
        },
    )
    add(
        "SVAL-013",
        "schedule_validation",
        "Reject accrual_cap_blocks = 0",
        {
            "subkind": "attach",
            "issuer": ISSUER,
            "block_height": "1000",
            "schedule": make_schedule(cap=0),
        },
    )
    add(
        "SVAL-014",
        "schedule_validation",
        "Reject accrual_cap_blocks exceeding profile max_epoch_gap_blocks",
        {
            "subkind": "attach",
            "issuer": ISSUER,
            "block_height": "1000",
            "max_epoch_gap_blocks": "86400",
            "schedule": make_schedule(cap=86_401),
        },
    )
    add(
        "SVAL-015",
        "schedule_validation",
        "Reject disclosure_ref exceeding 128 UTF-8 bytes (multi-byte UTF-8 boundary)",
        {
            "subkind": "attach",
            "issuer": ISSUER,
            "block_height": "1000",
            "schedule": make_schedule(disclosure_ref="é" * 65),
        },
    )
    add(
        "SVAL-016",
        "schedule_validation",
        "Reject duplicate attach when fee_state already exists",
        {
            "subkind": "attach",
            "issuer": ISSUER,
            "block_height": "2000",
            "existing_fee_state": make_fee_state(),
            "schedule": make_schedule(),
        },
    )
    add(
        "SVAL-017",
        "schedule_validation",
        "Unscheduled NAVCoin spread hike with < 7d notice rejected (closes instant-spread bypass)",
        {
            "subkind": "unscheduled_spread",
            "current_issue_spread_bps": 50,
            "current_redeem_spread_bps": 50,
            "new_issue_spread_bps": 100,
            "new_redeem_spread_bps": 50,
            "proposed_at_height": "100000",
            "effective_at_height": "704799",
        },
    )
    add(
        "SVAL-018",
        "schedule_validation",
        "Unscheduled NAVCoin spread hike with exact 7d notice accepted",
        {
            "subkind": "unscheduled_spread",
            "current_issue_spread_bps": 50,
            "current_redeem_spread_bps": 50,
            "new_issue_spread_bps": 100,
            "new_redeem_spread_bps": 150,
            "proposed_at_height": "100000",
            "effective_at_height": "704800",
        },
    )
    add(
        "SVAL-019",
        "schedule_validation",
        "Unscheduled NAVCoin pure spread cut accepted immediately (0 block wait)",
        {
            "subkind": "unscheduled_spread",
            "current_issue_spread_bps": 100,
            "current_redeem_spread_bps": 100,
            "new_issue_spread_bps": 25,
            "new_redeem_spread_bps": 25,
            "proposed_at_height": "100000",
            "effective_at_height": "100000",
        },
    )
    add(
        "SVAL-020",
        "schedule_validation",
        "FX fix fee at 100 bps passes on unscheduled NAVCoin; 101 bps rejected in SVAL-021",
        {
            "subkind": "fx_fix_cap",
            "fee_bps": 100,
            "fee_state": None,
        },
    )
    add(
        "SVAL-021",
        "schedule_validation",
        "FX fix fee at 101 bps rejected by 1% universal protocol cap",
        {
            "subkind": "fx_fix_cap",
            "fee_bps": 101,
            "fee_state": None,
        },
    )
    add(
        "SVAL-022",
        "schedule_validation",
        "FX fix fee exceeding asset schedule fix_fee_cap_bps (26 > 25) rejected",
        {
            "subkind": "fx_fix_cap",
            "fee_bps": 26,
            "fee_state": make_fee_state(active=make_schedule(fix=25)),
        },
    )
    add(
        "SVAL-023",
        "schedule_validation",
        "Fee-enabled NAVCoin with attested_value > 0 rejects issuer self-attestation",
        {
            "subkind": "non_issuer_attestor",
            "issuer": ISSUER,
            "fee_state": make_fee_state(),
            "attested_value": "500000000",
            "attestations": [{"attestor": ISSUER, "pass": True}],
        },
    )
    add(
        "SVAL-024",
        "schedule_validation",
        "Fee-enabled NAVCoin with attested_value > 0 passes with non-issuer passing attestor",
        {
            "subkind": "non_issuer_attestor",
            "issuer": ISSUER,
            "fee_state": make_fee_state(),
            "attested_value": "500000000",
            "attestations": [
                {"attestor": ISSUER, "pass": True},
                {"attestor": ATTESTOR, "pass": True},
            ],
        },
    )

    # =========================================================================
    # 3. single_epoch_accrual (40 vectors)
    # =========================================================================
    # ACC-001: Exact Train B Contract Section 3.6 worked example!
    res_001 = add(
        "ACC-001",
        "single_epoch_accrual",
        "Frozen Train B worked example (V=1.1e12, S=1e12, H=1e8, m=100, p=1000, dh=315360)",
        {
            "asset_id": ASSET_ID,
            "epoch": 12,
            "block_height": "200000",
            "verified_net_assets": "1100000000000",
            "supply_before": "1000000000000",
            "precision": 8,
            "fee_state": make_fee_state(
                active=make_schedule(annual=100, hwm=1000, cap=1_000_000),
                hwm=100_000_000,
                last_height=0,  # wait: 200000 - last_height must equal 315360 -> use block_height=515360, last=200000? Let's match exact delta_blocks!
            ),
        },
    )
    # Fix ACC-001 so block_height=200000? In Train B line 804 block_height=200000, delta_blocks=315360 wasn't derived from h_last=0 unless h_now=515360; let's set block_height=515360 and last_height=200000 so delta_blocks is EXACTLY 315360!
    vectors.pop()
    res_001 = add(
        "ACC-001",
        "single_epoch_accrual",
        "Frozen Train B worked example (V=1.1e12, S=1e12, H=1e8, m=100, p=1000, dh=315360)",
        {
            "asset_id": ASSET_ID,
            "epoch": 12,
            "block_height": "515360",
            "verified_net_assets": "1100000000000",
            "supply_before": "1000000000000",
            "precision": 8,
            "fee_state": make_fee_state(
                active=make_schedule(annual=100, hwm=1000, cap=1_000_000),
                hwm=100_000_000,
                last_height=200_000,
            ),
        },
    )
    assert res_001["accrual"]["time_fee_value"] == "110000000"
    assert res_001["accrual"]["nav_after_time_fee"] == "109989000"
    assert res_001["accrual"]["hwm_fee_value"] == "9989000000"
    assert res_001["accrual"]["total_fee_value"] == "10099000000"
    assert res_001["accrual"]["minted_fee_units"] == "9265979203"
    assert res_001["accrual"]["supply_after"] == "1009265979203"
    assert res_001["accrual"]["nav_after"] == "108990100"
    assert res_001["accrual"]["hwm_after"] == "108990100"

    add(
        "ACC-002",
        "single_epoch_accrual",
        "Pillar 7 worked example at precision 6 (V=1.1e12, S=1e12, H=1e6, m=100, p=1000, dh=315360)",
        {
            "asset_id": ASSET_ID,
            "epoch": 1,
            "block_height": "415360",
            "verified_net_assets": "1100000000000",
            "supply_before": "1000000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=100, hwm=1000, cap=1_000_000),
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-003",
        "single_epoch_accrual",
        "Zero-fee schedule (m=0, p=0) records zero_rates and still advances HWM",
        {
            "asset_id": ASSET_ID,
            "epoch": 1,
            "block_height": "186400",
            "verified_net_assets": "1200000000000",
            "supply_before": "1000000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=0, hwm=0),
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-004",
        "single_epoch_accrual",
        "Zero supply fail-safe (S=0) records zero_supply and advances last_accrual_height",
        {
            "asset_id": ASSET_ID,
            "epoch": 1,
            "block_height": "186400",
            "verified_net_assets": "500000000",
            "supply_before": "0",
            "precision": 6,
            "fee_state": make_fee_state(hwm=0, last_height=100_000),
        },
    )
    add(
        "ACC-005",
        "single_epoch_accrual",
        "Zero net assets fail-safe (V=0, S>0) records zero_net_assets",
        {
            "asset_id": ASSET_ID,
            "epoch": 2,
            "block_height": "186400",
            "verified_net_assets": "0",
            "supply_before": "1000000",
            "precision": 6,
            "fee_state": make_fee_state(hwm=1_000_000, last_height=100_000),
        },
    )
    add(
        "ACC-006",
        "single_epoch_accrual",
        "Zero delta blocks with annual-only schedule records zero_delta_blocks",
        {
            "asset_id": ASSET_ID,
            "epoch": 2,
            "block_height": "100000",
            "verified_net_assets": "1000000000",
            "supply_before": "1000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=500, hwm=0),
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-007",
        "single_epoch_accrual",
        "Zero delta blocks with HWM gain above H still accrues HWM fee",
        {
            "asset_id": ASSET_ID,
            "epoch": 2,
            "block_height": "100000",
            "verified_net_assets": "1200000000",
            "supply_before": "1000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=100, hwm=2000),
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-008",
        "single_epoch_accrual",
        "Block height behind last_accrual_height saturates delta_blocks to 0",
        {
            "asset_id": ASSET_ID,
            "epoch": 2,
            "block_height": "99999",
            "verified_net_assets": "1000000000",
            "supply_before": "1000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=250, hwm=0),
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-009",
        "single_epoch_accrual",
        "First S > 0 initialization (hwm_before=0 initializes to nav_before, zero retroactive HWM tax)",
        {
            "asset_id": ASSET_ID,
            "epoch": 1,
            "block_height": "186400",
            "verified_net_assets": "2500000000",
            "supply_before": "1000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=0, hwm=5000),
                hwm=0,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-010",
        "single_epoch_accrual",
        "Max caps (5%/yr + 50% HWM) over 1 day with 10% gain",
        {
            "asset_id": ASSET_ID,
            "epoch": 3,
            "block_height": str(100_000 + PROTOCOL_BLOCKS_PER_DAY),
            "verified_net_assets": "1100000000000",
            "supply_before": "1000000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=500, hwm=5000, cap=PROTOCOL_BLOCKS_PER_DAY),
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-011",
        "single_epoch_accrual",
        "Max caps (5%/yr + 50% HWM) over 7 days with 25% gain",
        {
            "asset_id": ASSET_ID,
            "epoch": 3,
            "block_height": str(100_000 + PROTOCOL_BLOCKS_PER_7_DAYS),
            "verified_net_assets": "1250000000000",
            "supply_before": "1000000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=500, hwm=5000, cap=PROTOCOL_BLOCKS_PER_7_DAYS),
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-012",
        "single_epoch_accrual",
        "Max caps (5%/yr + 50% HWM) over 30 days with 50% gain",
        {
            "asset_id": ASSET_ID,
            "epoch": 4,
            "block_height": str(100_000 + PROTOCOL_BLOCKS_PER_30_DAYS),
            "verified_net_assets": "1500000000000",
            "supply_before": "1000000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=500, hwm=5000, cap=PROTOCOL_BLOCKS_PER_30_DAYS),
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-013",
        "single_epoch_accrual",
        "Max caps (5%/yr + 50% HWM) over full year (31,536,000 blocks) with 2x gain",
        {
            "asset_id": ASSET_ID,
            "epoch": 5,
            "block_height": str(100_000 + PROTOCOL_BLOCKS_PER_YEAR),
            "verified_net_assets": "2000000000000",
            "supply_before": "1000000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=500, hwm=5000, cap=PROTOCOL_BLOCKS_PER_YEAR),
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-014",
        "single_epoch_accrual",
        "Annual-only max cap (5%/yr, p=0) over full year on flat NAV",
        {
            "asset_id": ASSET_ID,
            "epoch": 1,
            "block_height": str(100_000 + PROTOCOL_BLOCKS_PER_YEAR),
            "verified_net_assets": "1000000000000",
            "supply_before": "1000000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=500, hwm=0, cap=PROTOCOL_BLOCKS_PER_YEAR),
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-015",
        "single_epoch_accrual",
        "HWM-only max cap (m=0, p=5000) with 20% gain",
        {
            "asset_id": ASSET_ID,
            "epoch": 1,
            "block_height": "186400",
            "verified_net_assets": "1200000000000",
            "supply_before": "1000000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=0, hwm=5000),
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-016",
        "single_epoch_accrual",
        "HWM drawdown (nav_before < hwm_before): time fee accrues, hwm_fee = 0, HWM unchanged",
        {
            "asset_id": ASSET_ID,
            "epoch": 2,
            "block_height": str(100_000 + PROTOCOL_BLOCKS_PER_DAY),
            "verified_net_assets": "850000000000",
            "supply_before": "1000000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=200, hwm=2000),
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-017",
        "single_epoch_accrual",
        "HWM exact tie after time fee (nav_after_time_fee == hwm_before -> hwm_fee = 0)",
        {
            "asset_id": ASSET_ID,
            "epoch": 2,
            "block_height": str(100_000 + PROTOCOL_BLOCKS_PER_YEAR),
            "verified_net_assets": "1000000000000",
            "supply_before": "1000000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=500, hwm=2000, cap=PROTOCOL_BLOCKS_PER_YEAR),
                hwm=950_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-018",
        "single_epoch_accrual",
        "Step-1-before-Step-2 order: nav_before > H, but time fee brings nav_after_time_fee < H -> hwm_fee = 0",
        {
            "asset_id": ASSET_ID,
            "epoch": 2,
            "block_height": str(100_000 + PROTOCOL_BLOCKS_PER_YEAR),
            "verified_net_assets": "1020000000000",
            "supply_before": "1000000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=500, hwm=5000, cap=PROTOCOL_BLOCKS_PER_YEAR),
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-019",
        "single_epoch_accrual",
        "Deadman halt capping: elapsed 10M blocks capped to accrual_cap_blocks = 86400",
        {
            "asset_id": ASSET_ID,
            "epoch": 6,
            "block_height": "10100000",
            "verified_net_assets": "1000000000000",
            "supply_before": "1000000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=500, hwm=0, cap=PROTOCOL_BLOCKS_PER_DAY),
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-020",
        "single_epoch_accrual",
        "Floor truncation dust: 1 block elapsed with tiny V rounds time_fee_value to 0",
        {
            "asset_id": ASSET_ID,
            "epoch": 1,
            "block_height": "100001",
            "verified_net_assets": "100000",
            "supply_before": "100000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=100, hwm=0),
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-021",
        "single_epoch_accrual",
        "Floor truncation dust: non-zero F_m=1 but delta_s_raw floors to 0 (S < V - F)",
        {
            "asset_id": ASSET_ID,
            "epoch": 1,
            "block_height": str(100_000 + PROTOCOL_BLOCKS_PER_YEAR),
            "verified_net_assets": "1000",
            "supply_before": "10",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=10, hwm=0, cap=PROTOCOL_BLOCKS_PER_YEAR),
                hwm=100_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-022",
        "single_epoch_accrual",
        "Partial max_supply clipping (reason_code = clamped_max_supply, clipped > 0, no carry-forward)",
        {
            "asset_id": ASSET_ID,
            "epoch": 12,
            "block_height": "515360",
            "verified_net_assets": "1100000000000",
            "supply_before": "1000000000000",
            "precision": 8,
            "max_supply": "1005000000000",
            "fee_state": make_fee_state(
                active=make_schedule(annual=100, hwm=1000, cap=1_000_000),
                hwm=100_000_000,
                last_height=200_000,
            ),
        },
    )
    add(
        "ACC-023",
        "single_epoch_accrual",
        "Full max_supply clipping (S == max_supply -> minted = 0, all raw units clipped)",
        {
            "asset_id": ASSET_ID,
            "epoch": 12,
            "block_height": "515360",
            "verified_net_assets": "1100000000000",
            "supply_before": "1000000000000",
            "precision": 8,
            "max_supply": "1000000000000",
            "fee_state": make_fee_state(
                active=make_schedule(annual=100, hwm=1000, cap=1_000_000),
                hwm=100_000_000,
                last_height=200_000,
            ),
        },
    )
    add(
        "ACC-024",
        "single_epoch_accrual",
        "Exact max_supply boundary (S + delta_s_raw == max_supply -> NOT clamped, reason_code = accrued)",
        {
            "asset_id": ASSET_ID,
            "epoch": 12,
            "block_height": "515360",
            "verified_net_assets": "1100000000000",
            "supply_before": "1000000000000",
            "precision": 8,
            "max_supply": "1009265979203",
            "fee_state": make_fee_state(
                active=make_schedule(annual=100, hwm=1000, cap=1_000_000),
                hwm=100_000_000,
                last_height=200_000,
            ),
        },
    )
    add(
        "ACC-025",
        "single_epoch_accrual",
        "clamped_full_value fail-safe when extreme elapsed * annual_rate >= V (25 years elapsed)",
        {
            "asset_id": ASSET_ID,
            "epoch": 1,
            "block_height": str(100_000 + 25 * PROTOCOL_BLOCKS_PER_YEAR),
            "verified_net_assets": "1000000000",
            "supply_before": "1000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=500, hwm=0, cap=30 * PROTOCOL_BLOCKS_PER_YEAR),
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-026",
        "single_epoch_accrual",
        "u256 intermediate overflow guard: V=1e18, S=1e18, precision=18 (V*U=1e36, g*S*p > 2^128)",
        {
            "asset_id": ASSET_ID,
            "epoch": 1,
            "block_height": str(100_000 + PROTOCOL_BLOCKS_PER_DAY),
            "verified_net_assets": "1500000000000000000",
            "supply_before": "1000000000000000000",
            "precision": 18,
            "fee_state": make_fee_state(
                active=make_schedule(annual=500, hwm=5000, cap=PROTOCOL_BLOCKS_PER_DAY),
                hwm=10**18,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-027",
        "single_epoch_accrual",
        "Near u64::MAX supply clamping when S + delta_s_raw > u64::MAX",
        {
            "asset_id": ASSET_ID,
            "epoch": 1,
            "block_height": str(100_000 + PROTOCOL_BLOCKS_PER_YEAR),
            "verified_net_assets": str(U64_MAX),
            "supply_before": str(U64_MAX - 100),
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=500, hwm=0, cap=PROTOCOL_BLOCKS_PER_YEAR),
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-028",
        "single_epoch_accrual",
        "Precision 0 integer-only NAV accrual with non-divisible V/S",
        {
            "asset_id": ASSET_ID,
            "epoch": 1,
            "block_height": str(100_000 + PROTOCOL_BLOCKS_PER_30_DAYS),
            "verified_net_assets": "999999999",
            "supply_before": "7777",
            "precision": 0,
            "fee_state": make_fee_state(
                active=make_schedule(annual=300, hwm=2500, cap=PROTOCOL_BLOCKS_PER_30_DAYS),
                hwm=100_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-029",
        "single_epoch_accrual",
        "Precision 2 cent-precision NAV accrual",
        {
            "asset_id": ASSET_ID,
            "epoch": 2,
            "block_height": str(100_000 + PROTOCOL_BLOCKS_PER_7_DAYS),
            "verified_net_assets": "1333333333",
            "supply_before": "10000007",
            "precision": 2,
            "fee_state": make_fee_state(
                active=make_schedule(annual=150, hwm=1500, cap=PROTOCOL_BLOCKS_PER_7_DAYS),
                hwm=12_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-030",
        "single_epoch_accrual",
        "Precision 12 high-precision prime-ratio NAV accrual",
        {
            "asset_id": ASSET_ID,
            "epoch": 3,
            "block_height": str(100_000 + 123_457),
            "verified_net_assets": "999999937000000",
            "supply_before": "791900000003",
            "precision": 12,
            "fee_state": make_fee_state(
                active=make_schedule(annual=333, hwm=1777, cap=PROTOCOL_BLOCKS_PER_7_DAYS),
                hwm=1_000_000_000_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-031",
        "single_epoch_accrual",
        "Mixed-rate transition (pending hike activates at h == effective): uses min(old, new) = old rates",
        {
            "asset_id": ASSET_ID,
            "epoch": 10,
            "block_height": "704800",
            "verified_net_assets": "1200000000000",
            "supply_before": "1000000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=100, hwm=1000, cap=PROTOCOL_BLOCKS_PER_7_DAYS),
                pending={
                    "schedule": make_schedule(annual=400, hwm=4000, cap=PROTOCOL_BLOCKS_PER_30_DAYS),
                    "proposed_at_height": "100000",
                    "effective_at_height": "704800",
                    "is_increase": True,
                },
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-032",
        "single_epoch_accrual",
        "Mixed-rate transition (pending fee cut activates): uses min(old, new) = new lower rates",
        {
            "asset_id": ASSET_ID,
            "epoch": 10,
            "block_height": "186400",
            "verified_net_assets": "1200000000000",
            "supply_before": "1000000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=400, hwm=4000, cap=PROTOCOL_BLOCKS_PER_7_DAYS),
                pending={
                    "schedule": make_schedule(annual=50, hwm=500, cap=PROTOCOL_BLOCKS_PER_DAY),
                    "proposed_at_height": "150000",
                    "effective_at_height": "150000",
                    "is_increase": False,
                },
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-033",
        "single_epoch_accrual",
        "Mixed-rate transition (annual up, HWM down): applies min(old.m, new.m) and min(old.p, new.p)",
        {
            "asset_id": ASSET_ID,
            "epoch": 10,
            "block_height": "704800",
            "verified_net_assets": "1200000000000",
            "supply_before": "1000000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=100, hwm=3000, cap=PROTOCOL_BLOCKS_PER_7_DAYS),
                pending={
                    "schedule": make_schedule(annual=400, hwm=500, cap=PROTOCOL_BLOCKS_PER_7_DAYS),
                    "proposed_at_height": "100000",
                    "effective_at_height": "704800",
                    "is_increase": True,
                },
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-034",
        "single_epoch_accrual",
        "Pending hike NOT yet effective (h == effective - 1): stays pending, applies active rates",
        {
            "asset_id": ASSET_ID,
            "epoch": 9,
            "block_height": "704799",
            "verified_net_assets": "1200000000000",
            "supply_before": "1000000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=100, hwm=1000, cap=PROTOCOL_BLOCKS_PER_7_DAYS),
                pending={
                    "schedule": make_schedule(annual=400, hwm=4000, cap=PROTOCOL_BLOCKS_PER_7_DAYS),
                    "proposed_at_height": "100000",
                    "effective_at_height": "704800",
                    "is_increase": True,
                },
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-035",
        "single_epoch_accrual",
        "Pending cut to zero rates (m=0, p=0) activates cleanly with reason_code = zero_rates",
        {
            "asset_id": ASSET_ID,
            "epoch": 11,
            "block_height": "200000",
            "verified_net_assets": "1300000000000",
            "supply_before": "1000000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=250, hwm=2500),
                pending={
                    "schedule": make_schedule(annual=0, hwm=0),
                    "proposed_at_height": "150000",
                    "effective_at_height": "150000",
                    "is_increase": False,
                },
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-036",
        "single_epoch_accrual",
        "Prior unclaimed balance accumulates monotonically across accrual",
        {
            "asset_id": ASSET_ID,
            "epoch": 5,
            "block_height": str(100_000 + PROTOCOL_BLOCKS_PER_DAY),
            "verified_net_assets": "1150000000000",
            "supply_before": "1000500000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=200, hwm=1500),
                hwm=1_050_000,
                last_height=100_000,
                unclaimed=500_000_000,
                total_accrued=800_000_000,
                total_claimed=300_000_000,
            ),
        },
    )
    add(
        "ACC-037",
        "single_epoch_accrual",
        "Smallest positive annual rate (1 bps) over 1 day on 1B micro-USD",
        {
            "asset_id": ASSET_ID,
            "epoch": 1,
            "block_height": str(100_000 + PROTOCOL_BLOCKS_PER_DAY),
            "verified_net_assets": "1000000000000000",
            "supply_before": "1000000000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=1, hwm=0),
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-038",
        "single_epoch_accrual",
        "Smallest positive HWM rate (1 bps) on 1-atom gain above HWM",
        {
            "asset_id": ASSET_ID,
            "epoch": 1,
            "block_height": "100010",
            "verified_net_assets": "1000001000000000",
            "supply_before": "1000000000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=0, hwm=1),
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-039",
        "single_epoch_accrual",
        "Massive 100x NAV rally with 50% HWM cap tests dilution math F*S/(V-F) accuracy",
        {
            "asset_id": ASSET_ID,
            "epoch": 1,
            "block_height": str(100_000 + PROTOCOL_BLOCKS_PER_DAY),
            "verified_net_assets": "100000000000000",
            "supply_before": "1000000000000",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=500, hwm=5000),
                hwm=1_000_000,
                last_height=100_000,
            ),
        },
    )
    add(
        "ACC-040",
        "single_epoch_accrual",
        "Zero-supply epoch still promotes pending schedule at effective_at_height",
        {
            "asset_id": ASSET_ID,
            "epoch": 2,
            "block_height": "704800",
            "verified_net_assets": "0",
            "supply_before": "0",
            "precision": 6,
            "fee_state": make_fee_state(
                active=make_schedule(annual=100, hwm=1000),
                pending={
                    "schedule": make_schedule(annual=200, hwm=2000),
                    "proposed_at_height": "100000",
                    "effective_at_height": "704800",
                    "is_increase": True,
                },
                hwm=0,
                last_height=100_000,
            ),
        },
    )

    # =========================================================================
    # 4. pending_notice_and_lifecycle (20 vectors)
    # =========================================================================
    add(
        "LIFE-001",
        "pending_notice_and_lifecycle",
        "Increasing annual_rate_bps marks is_increase=true and waits >= 7d notice",
        {
            "action": "propose",
            "issuer": ISSUER,
            "block_height": "100000",
            "fee_state": make_fee_state(active=make_schedule(annual=100, notice=604_800)),
            "schedule": make_schedule(annual=101, notice=604_800),
        },
    )
    add(
        "LIFE-002",
        "pending_notice_and_lifecycle",
        "Increasing hwm_rate_bps marks is_increase=true and waits active notice",
        {
            "action": "propose",
            "issuer": ISSUER,
            "block_height": "100000",
            "fee_state": make_fee_state(active=make_schedule(hwm=1000, notice=1_000_000)),
            "schedule": make_schedule(hwm=1500, notice=1_000_000),
        },
    )
    add(
        "LIFE-003",
        "pending_notice_and_lifecycle",
        "Increasing issue_spread_bps marks is_increase=true",
        {
            "action": "propose",
            "issuer": ISSUER,
            "block_height": "100000",
            "fee_state": make_fee_state(active=make_schedule(issue=50)),
            "schedule": make_schedule(issue=51),
        },
    )
    add(
        "LIFE-004",
        "pending_notice_and_lifecycle",
        "Increasing redeem_spread_bps marks is_increase=true",
        {
            "action": "propose",
            "issuer": ISSUER,
            "block_height": "100000",
            "fee_state": make_fee_state(active=make_schedule(redeem=50)),
            "schedule": make_schedule(redeem=75),
        },
    )
    add(
        "LIFE-005",
        "pending_notice_and_lifecycle",
        "Increasing fix_fee_cap_bps marks is_increase=true",
        {
            "action": "propose",
            "issuer": ISSUER,
            "block_height": "100000",
            "fee_state": make_fee_state(active=make_schedule(fix=25)),
            "schedule": make_schedule(fix=50),
        },
    )
    add(
        "LIFE-006",
        "pending_notice_and_lifecycle",
        "Increasing accrual_cap_blocks marks is_increase=true",
        {
            "action": "propose",
            "issuer": ISSUER,
            "block_height": "100000",
            "fee_state": make_fee_state(active=make_schedule(cap=86_400)),
            "schedule": make_schedule(cap=172_800),
        },
    )
    add(
        "LIFE-007",
        "pending_notice_and_lifecycle",
        "Decreasing increase_notice_blocks counts as an increase and waits CURRENT notice window",
        {
            "action": "propose",
            "issuer": ISSUER,
            "block_height": "100000",
            "fee_state": make_fee_state(active=make_schedule(notice=1_209_600)),
            "schedule": make_schedule(notice=604_800),
        },
    )
    add(
        "LIFE-008",
        "pending_notice_and_lifecycle",
        "Pure fee cut sets is_increase=false and effective_at_height=block_height (next finalization)",
        {
            "action": "propose",
            "issuer": ISSUER,
            "block_height": "200000",
            "fee_state": make_fee_state(active=make_schedule(annual=200, hwm=2000)),
            "schedule": make_schedule(annual=100, hwm=1000),
        },
    )
    add(
        "LIFE-009",
        "pending_notice_and_lifecycle",
        "Lengthening increase_notice_blocks alone is NOT an increase (effective immediately)",
        {
            "action": "propose",
            "issuer": ISSUER,
            "block_height": "200000",
            "fee_state": make_fee_state(active=make_schedule(notice=604_800)),
            "schedule": make_schedule(notice=2_592_000),
        },
    )
    add(
        "LIFE-010",
        "pending_notice_and_lifecycle",
        "Cancelling an active pending hike succeeds and clears pending",
        {
            "action": "cancel",
            "fee_state": make_fee_state(
                pending={
                    "schedule": make_schedule(annual=300),
                    "proposed_at_height": "100000",
                    "effective_at_height": "704800",
                    "is_increase": True,
                }
            ),
        },
    )
    add(
        "LIFE-011",
        "pending_notice_and_lifecycle",
        "Cancelling when no pending schedule exists fails with nav_fee_no_pending_schedule",
        {
            "action": "cancel",
            "fee_state": make_fee_state(pending=None),
        },
    )
    add(
        "LIFE-012",
        "pending_notice_and_lifecycle",
        "Reducing redeem exit capacity during pending hike rejected",
        {
            "action": "exit_capacity",
            "fee_state": make_fee_state(
                pending={
                    "schedule": make_schedule(annual=300),
                    "proposed_at_height": "100000",
                    "effective_at_height": "704800",
                    "is_increase": True,
                }
            ),
            "current_redeem_capacity_atoms": "1000000",
            "new_redeem_capacity_atoms": "999999",
        },
    )
    add(
        "LIFE-013",
        "pending_notice_and_lifecycle",
        "Maintaining or raising redeem exit capacity during pending hike is allowed",
        {
            "action": "exit_capacity",
            "fee_state": make_fee_state(
                pending={
                    "schedule": make_schedule(annual=300),
                    "proposed_at_height": "100000",
                    "effective_at_height": "704800",
                    "is_increase": True,
                }
            ),
            "current_redeem_capacity_atoms": "1000000",
            "new_redeem_capacity_atoms": "1500000",
        },
    )
    add(
        "LIFE-014",
        "pending_notice_and_lifecycle",
        "Reducing redeem exit capacity during pending fee CUT (is_increase=false) is allowed",
        {
            "action": "exit_capacity",
            "fee_state": make_fee_state(
                pending={
                    "schedule": make_schedule(annual=50),
                    "proposed_at_height": "100000",
                    "effective_at_height": "100000",
                    "is_increase": False,
                }
            ),
            "current_redeem_capacity_atoms": "1000000",
            "new_redeem_capacity_atoms": "500000",
        },
    )
    add(
        "LIFE-015",
        "pending_notice_and_lifecycle",
        "rates_locked=true permanently rejects any rate increase",
        {
            "action": "propose",
            "issuer": ISSUER,
            "block_height": "200000",
            "fee_state": make_fee_state(active=make_schedule(annual=100, locked=True)),
            "schedule": make_schedule(annual=101, locked=True),
        },
    )
    add(
        "LIFE-016",
        "pending_notice_and_lifecycle",
        "rates_locked=true permanently rejects unlocking (true -> false)",
        {
            "action": "propose",
            "issuer": ISSUER,
            "block_height": "200000",
            "fee_state": make_fee_state(active=make_schedule(annual=100, locked=True)),
            "schedule": make_schedule(annual=100, locked=False),
        },
    )
    add(
        "LIFE-017",
        "pending_notice_and_lifecycle",
        "rates_locked=true allows unilateral recipient rotation with 7-day notice (key recovery)",
        {
            "action": "propose",
            "issuer": ISSUER,
            "block_height": "200000",
            "fee_state": make_fee_state(active=make_schedule(annual=100, locked=True)),
            "schedule": make_schedule(annual=100, recipient=RECIPIENT_2, locked=True),
            "recipient_cosigned": False,
        },
    )
    add(
        "LIFE-018",
        "pending_notice_and_lifecycle",
        "rates_locked=true allows co-signed recipient rotation immediately",
        {
            "action": "propose",
            "issuer": ISSUER,
            "block_height": "200000",
            "fee_state": make_fee_state(active=make_schedule(annual=100, locked=True)),
            "schedule": make_schedule(annual=100, recipient=RECIPIENT_2, locked=True),
            "recipient_cosigned": True,
        },
    )
    add(
        "LIFE-019",
        "pending_notice_and_lifecycle",
        "Valid partial claim reduces unclaimed_fee_units and increments total_claimed_fee_units",
        {
            "action": "claim",
            "caller": RECIPIENT,
            "amount": "4000",
            "fee_state": make_fee_state(unclaimed=10_000, total_accrued=10_000, total_claimed=0),
        },
    )
    add(
        "LIFE-020",
        "pending_notice_and_lifecycle",
        "Over-claim rejected with nav_fee_insufficient_unclaimed",
        {
            "action": "claim",
            "caller": RECIPIENT,
            "amount": "10001",
            "fee_state": make_fee_state(unclaimed=10_000, total_accrued=10_000, total_claimed=0),
        },
    )

    # =========================================================================
    # 5. multi_epoch_compounding (20 vectors)
    # =========================================================================
    add(
        "COMP-001",
        "multi_epoch_compounding",
        "5-epoch steady growth compounding (annual 100 bps + HWM 1000 bps)",
        {
            "asset_id": ASSET_ID,
            "issuer": ISSUER,
            "precision": 6,
            "initial_supply": "1000000000000",
            "initial_fee_state": make_fee_state(
                active=make_schedule(annual=100, hwm=1000, cap=PROTOCOL_BLOCKS_PER_30_DAYS),
                hwm=1_000_000,
                last_height=100_000,
            ),
            "epochs": [
                {"epoch": 1, "block_height": str(100_000 + PROTOCOL_BLOCKS_PER_7_DAYS), "verified_net_assets": "1050000000000"},
                {"epoch": 2, "block_height": str(100_000 + 2 * PROTOCOL_BLOCKS_PER_7_DAYS), "verified_net_assets": "1100000000000"},
                {"epoch": 3, "block_height": str(100_000 + 3 * PROTOCOL_BLOCKS_PER_7_DAYS), "verified_net_assets": "1160000000000"},
                {"epoch": 4, "block_height": str(100_000 + 4 * PROTOCOL_BLOCKS_PER_7_DAYS), "verified_net_assets": "1220000000000"},
                {"epoch": 5, "block_height": str(100_000 + 5 * PROTOCOL_BLOCKS_PER_7_DAYS), "verified_net_assets": "1300000000000"},
            ],
        },
    )
    add(
        "COMP-002",
        "multi_epoch_compounding",
        "6-epoch bull -> drawdown -> partial recovery -> full HWM breakout cycle",
        {
            "asset_id": ASSET_ID,
            "issuer": ISSUER,
            "precision": 6,
            "initial_supply": "1000000000000",
            "initial_fee_state": make_fee_state(
                active=make_schedule(annual=100, hwm=2000, cap=PROTOCOL_BLOCKS_PER_30_DAYS),
                hwm=1_000_000,
                last_height=100_000,
            ),
            "epochs": [
                {"epoch": 1, "block_height": "186400", "verified_net_assets": "1200000000000"},  # bull
                {"epoch": 2, "block_height": "272800", "verified_net_assets": "900000000000"},   # drawdown
                {"epoch": 3, "block_height": "359200", "verified_net_assets": "1000000000000"},  # partial recovery
                {"epoch": 4, "block_height": "445600", "verified_net_assets": "1150000000000"},  # still below HWM
                {"epoch": 5, "block_height": "532000", "verified_net_assets": "1350000000000"},  # breakout above HWM
                {"epoch": 6, "block_height": "618400", "verified_net_assets": "1400000000000"},  # continued gain
            ],
        },
    )
    add(
        "COMP-003",
        "multi_epoch_compounding",
        "Attach on live asset at NAV=2.50 (zero retroactive HWM tax; accrues only on gains above 2.50)",
        {
            "asset_id": ASSET_ID,
            "issuer": ISSUER,
            "precision": 6,
            "initial_supply": "1000000000000",
            "initial_fee_state": make_fee_state(
                active=make_schedule(annual=200, hwm=2000, attached_at_height=500_000),
                hwm=2_500_000,
                last_height=500_000,
            ),
            "epochs": [
                {"epoch": 50, "block_height": "586400", "verified_net_assets": "2500000000000"},
                {"epoch": 51, "block_height": "672800", "verified_net_assets": "2600000000000"},
                {"epoch": 52, "block_height": "759200", "verified_net_assets": "2400000000000"},
                {"epoch": 53, "block_height": "845600", "verified_net_assets": "2800000000000"},
            ],
        },
    )
    add(
        "COMP-004",
        "multi_epoch_compounding",
        "5-epoch hike notice lifecycle: propose -> pre-effective -> mixed-rate activation -> full new rate -> claim",
        {
            "asset_id": ASSET_ID,
            "issuer": ISSUER,
            "precision": 6,
            "initial_supply": "1000000000000",
            "initial_fee_state": make_fee_state(
                active=make_schedule(annual=100, hwm=1000, cap=PROTOCOL_BLOCKS_PER_7_DAYS),
                hwm=1_000_000,
                last_height=100_000,
            ),
            "epochs": [
                {
                    "epoch": 1,
                    "block_height": "186400",
                    "verified_net_assets": "1050000000000",
                    "propose_update": {
                        "block_height": "150000",
                        "schedule": make_schedule(annual=300, hwm=2500, cap=PROTOCOL_BLOCKS_PER_7_DAYS),
                    },
                },
                {"epoch": 2, "block_height": "500000", "verified_net_assets": "1100000000000"},  # still pending (< 754800)
                {"epoch": 3, "block_height": "760000", "verified_net_assets": "1150000000000"},  # activates using min(old,new)
                {"epoch": 4, "block_height": "846400", "verified_net_assets": "1250000000000"},  # full new 300/2500 rates
                {
                    "epoch": 5,
                    "block_height": "932800",
                    "verified_net_assets": "1300000000000",
                    "claim": {"caller": RECIPIENT, "amount": "1000000"},
                },
            ],
        },
    )
    add(
        "COMP-005",
        "multi_epoch_compounding",
        "Pending hike proposed then cancelled before effective_at_height never activates",
        {
            "asset_id": ASSET_ID,
            "issuer": ISSUER,
            "precision": 6,
            "initial_supply": "1000000000000",
            "initial_fee_state": make_fee_state(
                active=make_schedule(annual=100, hwm=1000, cap=PROTOCOL_BLOCKS_PER_7_DAYS),
                hwm=1_000_000,
                last_height=100_000,
            ),
            "epochs": [
                {
                    "epoch": 1,
                    "block_height": "200000",
                    "verified_net_assets": "1050000000000",
                    "propose_update": {
                        "block_height": "150000",
                        "schedule": make_schedule(annual=500, hwm=5000, cap=PROTOCOL_BLOCKS_PER_7_DAYS),
                    },
                },
                {
                    "epoch": 2,
                    "block_height": "400000",
                    "verified_net_assets": "1100000000000",
                    "cancel_pending": True,
                    "cancel_block_height": "350000",
                },
                {"epoch": 3, "block_height": "800000", "verified_net_assets": "1200000000000"},
                {"epoch": 4, "block_height": "900000", "verified_net_assets": "1250000000000"},
            ],
        },
    )
    add(
        "COMP-006",
        "multi_epoch_compounding",
        "Pure fee cut takes effect immediately on next finalization",
        {
            "asset_id": ASSET_ID,
            "issuer": ISSUER,
            "precision": 6,
            "initial_supply": "1000000000000",
            "initial_fee_state": make_fee_state(
                active=make_schedule(annual=400, hwm=4000, cap=PROTOCOL_BLOCKS_PER_7_DAYS),
                hwm=1_000_000,
                last_height=100_000,
            ),
            "epochs": [
                {"epoch": 1, "block_height": "186400", "verified_net_assets": "1100000000000"},
                {
                    "epoch": 2,
                    "block_height": "272800",
                    "verified_net_assets": "1200000000000",
                    "propose_update": {
                        "block_height": "200000",
                        "schedule": make_schedule(annual=50, hwm=500, cap=PROTOCOL_BLOCKS_PER_7_DAYS),
                    },
                },
                {"epoch": 3, "block_height": "359200", "verified_net_assets": "1300000000000"},
                {"epoch": 4, "block_height": "445600", "verified_net_assets": "1400000000000"},
            ],
        },
    )
    add(
        "COMP-007",
        "multi_epoch_compounding",
        "Deadman halt gap (10M blocks capped at 86400) followed by normal daily epochs",
        {
            "asset_id": ASSET_ID,
            "issuer": ISSUER,
            "precision": 6,
            "initial_supply": "1000000000000",
            "initial_fee_state": make_fee_state(
                active=make_schedule(annual=500, hwm=1000, cap=PROTOCOL_BLOCKS_PER_DAY),
                hwm=1_000_000,
                last_height=100_000,
            ),
            "epochs": [
                {"epoch": 1, "block_height": "186400", "verified_net_assets": "1020000000000"},
                {"epoch": 2, "block_height": "10186400", "verified_net_assets": "1050000000000"},
                {"epoch": 3, "block_height": "10272800", "verified_net_assets": "1080000000000"},
                {"epoch": 4, "block_height": "10359200", "verified_net_assets": "1100000000000"},
            ],
        },
    )
    add(
        "COMP-008",
        "multi_epoch_compounding",
        "max_supply clipped on epoch 2 verifies zero carry-forward debt after external redemption frees supply",
        {
            "asset_id": ASSET_ID,
            "issuer": ISSUER,
            "precision": 6,
            "initial_supply": "1000000000",
            "max_supply": "1050000000",
            "initial_fee_state": make_fee_state(
                active=make_schedule(annual=500, hwm=5000, cap=PROTOCOL_BLOCKS_PER_30_DAYS),
                hwm=1_000_000,
                last_height=100_000,
            ),
            "epochs": [
                {"epoch": 1, "block_height": "186400", "verified_net_assets": "1050000000"},
                {"epoch": 2, "block_height": "272800", "verified_net_assets": "1400000000"},  # clips at 1.05B
                {
                    "epoch": 3,
                    "block_height": "359200",
                    "external_redeem_units": "200000000",
                    "verified_net_assets": "1100000000",
                },  # no carry-forward debt from epoch 2!
                {"epoch": 4, "block_height": "445600", "verified_net_assets": "1200000000"},
            ],
        },
    )
    add(
        "COMP-009",
        "multi_epoch_compounding",
        "Zero-supply start (S=0 epoch 1 -> first mint S>0 epoch 2 initializes HWM -> gain -> drawdown)",
        {
            "asset_id": ASSET_ID,
            "issuer": ISSUER,
            "precision": 6,
            "initial_supply": "0",
            "initial_fee_state": make_fee_state(
                active=make_schedule(annual=100, hwm=2000),
                hwm=0,
                last_height=100_000,
            ),
            "epochs": [
                {"epoch": 1, "block_height": "186400", "verified_net_assets": "0"},
                {
                    "epoch": 2,
                    "block_height": "272800",
                    "external_mint_units": "1000000000",
                    "verified_net_assets": "1500000000",
                },  # initializes HWM = 1.500000
                {"epoch": 3, "block_height": "359200", "verified_net_assets": "1800000000"},
                {"epoch": 4, "block_height": "445600", "verified_net_assets": "1600000000"},
            ],
        },
    )
    add(
        "COMP-010",
        "multi_epoch_compounding",
        "Max-cap (5% annual + 50% HWM) quarterly compounding over 1 full year with claims",
        {
            "asset_id": ASSET_ID,
            "issuer": ISSUER,
            "precision": 8,
            "initial_supply": "1000000000000",
            "initial_fee_state": make_fee_state(
                active=make_schedule(annual=500, hwm=5000, cap=PROTOCOL_BLOCKS_PER_YEAR),
                hwm=100_000_000,
                last_height=100_000,
            ),
            "epochs": [
                {
                    "epoch": 1,
                    "block_height": str(100_000 + PROTOCOL_BLOCKS_PER_YEAR // 4),
                    "verified_net_assets": "1150000000000",
                    "claim": {"caller": RECIPIENT, "amount": "5000000000"},
                },
                {
                    "epoch": 2,
                    "block_height": str(100_000 + PROTOCOL_BLOCKS_PER_YEAR // 2),
                    "verified_net_assets": "1300000000000",
                },
                {
                    "epoch": 3,
                    "block_height": str(100_000 + (3 * PROTOCOL_BLOCKS_PER_YEAR) // 4),
                    "verified_net_assets": "1250000000000",
                },
                {
                    "epoch": 4,
                    "block_height": str(100_000 + PROTOCOL_BLOCKS_PER_YEAR),
                    "verified_net_assets": "1600000000000",
                    "claim": {"caller": RECIPIENT, "amount": "10000000000"},
                },
            ],
        },
    )
    add(
        "COMP-011",
        "multi_epoch_compounding",
        "Precision 18 high-scale (1e18 atoms) 4-epoch compounding (tests u256/BigInt parity)",
        {
            "asset_id": ASSET_ID,
            "issuer": ISSUER,
            "precision": 18,
            "initial_supply": "1000000000000000000",
            "initial_fee_state": make_fee_state(
                active=make_schedule(annual=250, hwm=2000, cap=PROTOCOL_BLOCKS_PER_30_DAYS),
                hwm=10**18,
                last_height=100_000,
            ),
            "epochs": [
                {"epoch": 1, "block_height": "186400", "verified_net_assets": "1100000000000000000"},
                {"epoch": 2, "block_height": "272800", "verified_net_assets": "1250000000000000000"},
                {"epoch": 3, "block_height": "359200", "verified_net_assets": "1200000000000000000"},
                {"epoch": 4, "block_height": "445600", "verified_net_assets": "1400000000000000000"},
            ],
        },
    )
    add(
        "COMP-012",
        "multi_epoch_compounding",
        "Precision 0 integer-only multi-epoch compounding",
        {
            "asset_id": ASSET_ID,
            "issuer": ISSUER,
            "precision": 0,
            "initial_supply": "10000",
            "initial_fee_state": make_fee_state(
                active=make_schedule(annual=400, hwm=2500, cap=PROTOCOL_BLOCKS_PER_30_DAYS),
                hwm=10_000,
                last_height=100_000,
            ),
            "epochs": [
                {"epoch": 1, "block_height": str(100_000 + PROTOCOL_BLOCKS_PER_30_DAYS), "verified_net_assets": "120000000"},
                {"epoch": 2, "block_height": str(100_000 + 2 * PROTOCOL_BLOCKS_PER_30_DAYS), "verified_net_assets": "140000000"},
                {"epoch": 3, "block_height": str(100_000 + 3 * PROTOCOL_BLOCKS_PER_30_DAYS), "verified_net_assets": "130000000"},
                {"epoch": 4, "block_height": str(100_000 + 4 * PROTOCOL_BLOCKS_PER_30_DAYS), "verified_net_assets": "165000000"},
            ],
        },
    )
    add(
        "COMP-013",
        "multi_epoch_compounding",
        "Zero-fee schedule (m=0,p=0) tracks HWM before later hike so prior gains are never taxed",
        {
            "asset_id": ASSET_ID,
            "issuer": ISSUER,
            "precision": 6,
            "initial_supply": "1000000000000",
            "initial_fee_state": make_fee_state(
                active=make_schedule(annual=0, hwm=0, cap=PROTOCOL_BLOCKS_PER_7_DAYS),
                hwm=1_000_000,
                last_height=100_000,
            ),
            "epochs": [
                {
                    "epoch": 1,
                    "block_height": "200000",
                    "verified_net_assets": "1500000000000",
                    "propose_update": {
                        "block_height": "200000",
                        "schedule": make_schedule(annual=200, hwm=2000, cap=PROTOCOL_BLOCKS_PER_7_DAYS),
                    },
                },  # HWM rises to 1.50 with 0 fee
                {"epoch": 2, "block_height": "804800", "verified_net_assets": "1600000000000"},  # activates using min(0, new)=0, HWM=1.60
                {"epoch": 3, "block_height": "891200", "verified_net_assets": "1700000000000"},  # taxes only gain above 1.60!
            ],
        },
    )
    add(
        "COMP-014",
        "multi_epoch_compounding",
        "Mixed change (annual up, HWM down): waits 7d, transition uses min of each rate, then full new rates",
        {
            "asset_id": ASSET_ID,
            "issuer": ISSUER,
            "precision": 6,
            "initial_supply": "1000000000000",
            "initial_fee_state": make_fee_state(
                active=make_schedule(annual=100, hwm=3000, cap=PROTOCOL_BLOCKS_PER_7_DAYS),
                hwm=1_000_000,
                last_height=100_000,
            ),
            "epochs": [
                {
                    "epoch": 1,
                    "block_height": "186400",
                    "verified_net_assets": "1100000000000",
                    "propose_update": {
                        "block_height": "150000",
                        "schedule": make_schedule(annual=400, hwm=500, cap=PROTOCOL_BLOCKS_PER_7_DAYS),
                    },
                },
                {"epoch": 2, "block_height": "500000", "verified_net_assets": "1200000000000"},
                {"epoch": 3, "block_height": "754800", "verified_net_assets": "1350000000000"},  # applied: m=100, p=500
                {"epoch": 4, "block_height": "841200", "verified_net_assets": "1500000000000"},  # applied: m=400, p=500
            ],
        },
    )
    add(
        "COMP-015",
        "multi_epoch_compounding",
        "Recipient rotation under rates_locked=true followed by claim from new recipient",
        {
            "asset_id": ASSET_ID,
            "issuer": ISSUER,
            "precision": 6,
            "initial_supply": "1000000000000",
            "initial_fee_state": make_fee_state(
                active=make_schedule(annual=200, hwm=1000, locked=True, cap=PROTOCOL_BLOCKS_PER_7_DAYS),
                hwm=1_000_000,
                last_height=100_000,
            ),
            "epochs": [
                {
                    "epoch": 1,
                    "block_height": "186400",
                    "verified_net_assets": "1100000000000",
                    "propose_update": {
                        "block_height": "186400",
                        "schedule": make_schedule(
                            annual=200, hwm=1000, recipient=RECIPIENT_2, locked=True, cap=PROTOCOL_BLOCKS_PER_7_DAYS
                        ),
                        "recipient_cosigned": False,
                    },
                },
                {"epoch": 2, "block_height": "791200", "verified_net_assets": "1200000000000"},
                {
                    "epoch": 3,
                    "block_height": "877600",
                    "verified_net_assets": "1250000000000",
                    "claim": {"caller": RECIPIENT_2, "amount": "5000000000"},
                },
            ],
        },
    )
    add(
        "COMP-016",
        "multi_epoch_compounding",
        "5-epoch choppy oscillation around HWM tests exact step-1/step-2 gating on every tick",
        {
            "asset_id": ASSET_ID,
            "issuer": ISSUER,
            "precision": 6,
            "initial_supply": "1000000000000",
            "initial_fee_state": make_fee_state(
                active=make_schedule(annual=150, hwm=1500),
                hwm=1_000_000,
                last_height=100_000,
            ),
            "epochs": [
                {"epoch": 1, "block_height": "186400", "verified_net_assets": "1010000000000"},
                {"epoch": 2, "block_height": "272800", "verified_net_assets": "1005000000000"},
                {"epoch": 3, "block_height": "359200", "verified_net_assets": "1015000000000"},
                {"epoch": 4, "block_height": "445600", "verified_net_assets": "1008000000000"},
                {"epoch": 5, "block_height": "532000", "verified_net_assets": "1025000000000"},
            ],
        },
    )
    add(
        "COMP-017",
        "multi_epoch_compounding",
        "External primary mints & redemptions between epochs preserve exact NAV & HWM accounting",
        {
            "asset_id": ASSET_ID,
            "issuer": ISSUER,
            "precision": 6,
            "initial_supply": "1000000000000",
            "initial_fee_state": make_fee_state(
                active=make_schedule(annual=100, hwm=1000),
                hwm=1_000_000,
                last_height=100_000,
            ),
            "epochs": [
                {"epoch": 1, "block_height": "186400", "verified_net_assets": "1100000000000"},
                {
                    "epoch": 2,
                    "block_height": "272800",
                    "external_mint_units": "500000000000",
                    "verified_net_assets": "1700000000000",
                },
                {
                    "epoch": 3,
                    "block_height": "359200",
                    "external_redeem_units": "300000000000",
                    "verified_net_assets": "1500000000000",
                },
                {"epoch": 4, "block_height": "445600", "verified_net_assets": "1650000000000"},
            ],
        },
    )
    add(
        "COMP-018",
        "multi_epoch_compounding",
        "Zero net-assets dip (V=0 on epoch 2) and full recovery on epoch 3 without HWM corruption",
        {
            "asset_id": ASSET_ID,
            "issuer": ISSUER,
            "precision": 6,
            "initial_supply": "1000000000000",
            "initial_fee_state": make_fee_state(
                active=make_schedule(annual=200, hwm=1000),
                hwm=1_000_000,
                last_height=100_000,
            ),
            "epochs": [
                {"epoch": 1, "block_height": "186400", "verified_net_assets": "1100000000000"},
                {"epoch": 2, "block_height": "272800", "verified_net_assets": "0"},
                {"epoch": 3, "block_height": "359200", "verified_net_assets": "1300000000000"},
            ],
        },
    )
    add(
        "COMP-019",
        "multi_epoch_compounding",
        "Rapid 1-block epochs test dust truncation across 4 consecutive blocks",
        {
            "asset_id": ASSET_ID,
            "issuer": ISSUER,
            "precision": 6,
            "initial_supply": "100000000000",
            "initial_fee_state": make_fee_state(
                active=make_schedule(annual=500, hwm=1000),
                hwm=1_000_000,
                last_height=100_000,
            ),
            "epochs": [
                {"epoch": 1, "block_height": "100001", "verified_net_assets": "100000000000"},
                {"epoch": 2, "block_height": "100002", "verified_net_assets": "100000100000"},
                {"epoch": 3, "block_height": "100003", "verified_net_assets": "100000200000"},
                {"epoch": 4, "block_height": "100004", "verified_net_assets": "100000500000"},
            ],
        },
    )
    add(
        "COMP-020",
        "multi_epoch_compounding",
        "Full accrual & multi-claim conservation (claiming from unclaimed bucket never alters circulating_supply)",
        {
            "asset_id": ASSET_ID,
            "issuer": ISSUER,
            "precision": 8,
            "initial_supply": "1000000000000",
            "initial_fee_state": make_fee_state(
                active=make_schedule(annual=100, hwm=1000, cap=1_000_000),
                hwm=100_000_000,
                last_height=200_000,
            ),
            "epochs": [
                {
                    "epoch": 12,
                    "block_height": "515360",
                    "verified_net_assets": "1100000000000",
                    "claim": {"caller": RECIPIENT, "amount": "5000000000"},
                },
                {
                    "epoch": 13,
                    "block_height": "830720",
                    "verified_net_assets": "1200000000000",
                    "claim": {"caller": RECIPIENT, "amount": "4265979203"},
                },
                {
                    "epoch": 14,
                    "block_height": "1146080",
                    "verified_net_assets": "1250000000000",
                },
            ],
        },
    )

    return vectors


def main() -> int:
    vectors = build_vectors()
    counts: dict[str, int] = {}
    for v in vectors:
        counts[v["kind"]] = counts.get(v["kind"], 0) + 1
    assert len(vectors) == 120, f"expected 120 vectors, got {len(vectors)}"

    bundle = {
        "schema": "postfiat-nav-fee-vectors-v1",
        "contract_ref": "l1-upgrade-train-b-contract.md#section-3",
        "protocol_constants": {
            "blocks_per_day": str(PROTOCOL_BLOCKS_PER_DAY),
            "blocks_per_7_days": str(PROTOCOL_BLOCKS_PER_7_DAYS),
            "blocks_per_30_days": str(PROTOCOL_BLOCKS_PER_30_DAYS),
            "blocks_per_year": str(PROTOCOL_BLOCKS_PER_YEAR),
            "bps_denominator": "10000",
            "max_annual_rate_bps": 500,
            "max_hwm_rate_bps": 5000,
            "max_spread_bps": 500,
            "max_fix_fee_cap_bps": 100,
            "min_increase_notice_blocks": str(NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS),
        },
        "vector_count": len(vectors),
        "category_counts": counts,
        "vectors": vectors,
    }

    out_path = ROOT / "sdk" / "navcoin" / "vectors" / "nav_fee_vectors_v1.json"
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(bundle, indent=2, sort_keys=False) + "\n", encoding="utf-8")
    print(f"wrote {len(vectors)} vectors to {out_path} ({counts})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
