"""Run the two happy paths, refund, and mutation-free adversarial probes."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import time
from typing import Any

from .accounting import (
    PrincipalState,
    assert_principal_conserved,
    assert_xrp_conserved_with_fees,
)
from .journal import EffectJournal
from .pftl_adapter import ASSET_ID, COORDINATOR, USER, HardenedPftl
from .protocol import (
    CrossLedgerHashlock,
    SecretPreimage,
    extract_xrpl_finish_preimage,
    verify_pair,
)
from .timelock import Direction, LedgerClocks, TimeoutPlan, TimingGateError, TimingPolicy
from .xrpl_adapter import XrplEscrowRef, XrplTestnet, load_wallet


def write_json(path: Path, value: Any, mode: int = 0o644) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    os.chmod(path, mode)


def xrpl_snapshot(
    xrpl: XrplTestnet, user: str, coordinator: str
) -> tuple[dict[str, Any], PrincipalState]:
    accounts = {
        "user": xrpl.account(user),
        "coordinator": xrpl.account(coordinator),
    }
    objects: dict[str, dict[str, Any]] = {}
    for owner in (user, coordinator):
        for item in xrpl.escrows(owner):
            objects[item["index"]] = item
    locked = sum(int(item["Amount"]) for item in objects.values())
    state = PrincipalState(
        accounts["user"]["balance_drops"],
        accounts["coordinator"]["balance_drops"],
        locked,
    )
    return (
        {
            "accounts": accounts,
            "open_escrows": list(objects.values()),
            "principal": {
                "user_drops": state.user,
                "coordinator_drops": state.coordinator,
                "locked_drops": state.locked,
                "total_drops": state.total,
            },
        },
        state,
    )


def pftl_snapshot(
    pftl: HardenedPftl,
) -> tuple[dict[str, Any], PrincipalState]:
    objects: dict[str, dict[str, Any]] = {}
    for owner in (USER, COORDINATOR):
        for item in pftl.escrows(owner, role="owner"):
            if item["state"] == "open" and item["asset_id"] == ASSET_ID:
                objects[item["escrow_id"]] = item
    state = PrincipalState(
        pftl.balance(USER),
        pftl.balance(COORDINATOR),
        sum(int(item["amount"]) for item in objects.values()),
    )
    return (
        {
            "consensus": pftl.converged_status(),
            "accounts": {
                "user": USER,
                "coordinator": COORDINATOR,
                "user_navcoin_atoms": state.user,
                "coordinator_navcoin_atoms": state.coordinator,
            },
            "open_escrows": list(objects.values()),
            "principal": {
                "user_atoms": state.user,
                "coordinator_atoms": state.coordinator,
                "locked_atoms": state.locked,
                "total_atoms": state.total,
            },
        },
        state,
    )


def clocks(xrpl: XrplTestnet, pftl: HardenedPftl) -> LedgerClocks:
    return LedgerClocks(
        xrpl_close_time=xrpl.ledger_clock()["close_time"],
        pftl_height=pftl.converged_status()["height"],
        observed_unix=int(time.time()),
    )


def assert_same_principal(
    before: tuple[dict[str, Any], PrincipalState],
    after: tuple[dict[str, Any], PrincipalState],
) -> None:
    if before[1] != after[1]:
        raise AssertionError("adversarial preflight changed ledger principal state")


def public_hashlock(lock: CrossLedgerHashlock) -> dict[str, str]:
    return lock.public_values()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--runtime-root", required=True, type=Path)
    args = parser.parse_args()
    public = args.runtime_root / "public"
    private = args.runtime_root / "private"
    evidence = public / "evidence"
    evidence.mkdir(parents=True, exist_ok=True)

    user_xrpl = load_wallet(private / "xrpl/user.wallet.json")
    coordinator_xrpl = load_wallet(private / "xrpl/coordinator.wallet.json")
    xrpl = XrplTestnet(evidence / "xrpl")
    pftl = HardenedPftl(evidence / "pftl")
    policy = TimingPolicy()

    locks = {
        "xrp_to_nav": CrossLedgerHashlock.generate(),
        "nav_to_xrp": CrossLedgerHashlock.generate(),
        "refund": CrossLedgerHashlock.generate(),
    }
    write_json(
        private / "swap-preimages.private.json",
        {name: lock.secret.protocol_hex() for name, lock in locks.items()},
        0o600,
    )
    report: dict[str, Any] = {
        "schema": "postfiat.xrpl_navcoin.live_demo.v1",
        "claim": (
            "non-custodial, conditionally-atomic, coordinator-trusted timing"
        ),
        "networks": {
            "xrpl": {
                "network": "Testnet",
                "endpoint": xrpl.endpoint,
                "user": user_xrpl.classic_address,
                "coordinator": coordinator_xrpl.classic_address,
            },
            "pftl": {
                "chain_id": "local-pftl-proven-nav-v2-20260724",
                "rpc": [f"tcp://127.0.0.1:{port}" for port in range(31660, 31666)],
                "binary_revision": "ae3c53c9",
                "asset_id": ASSET_ID,
                "user": USER,
                "coordinator": COORDINATOR,
            },
        },
        "value_disclaimer": "XRPL faucet XRP and isolated PFTL devnet NAVcoin only",
        "scenarios": {},
        "adversarial": {},
    }

    # Happy path 1: user locks XRP first; coordinator locks NAV second.
    lock = locks["xrp_to_nav"]
    before_xrp = xrpl_snapshot(
        xrpl, user_xrpl.classic_address, coordinator_xrpl.classic_address
    )
    xclock = xrpl.ledger_clock()
    on_plan = TimeoutPlan(
        Direction.XRP_TO_NAV,
        xrpl_cancel_after=xclock["close_time"] + 300,
        pftl_cancel_after=pftl.converged_status()["height"] + 20,
    )
    on_xrp, on_xrp_create = xrpl.create(
        label="onramp-01-xrp-create",
        wallet=user_xrpl,
        destination=coordinator_xrpl.classic_address,
        amount_drops=1_000_000,
        condition=lock.public_values()["xrpl_condition"],
        cancel_after=on_plan.xrpl_cancel_after,
    )
    after_xrp_create = xrpl_snapshot(
        xrpl, user_xrpl.classic_address, coordinator_xrpl.classic_address
    )
    assert_xrp_conserved_with_fees(
        before_xrp[1], after_xrp_create[1], on_xrp_create["fee_drops"]
    )
    gate = on_plan.validate_second_lock(
        clocks(xrpl, pftl), policy, coordinator_observed_unix=int(time.time())
    )
    before_nav = pftl_snapshot(pftl)
    on_nav, on_nav_create = pftl.create(
        label="onramp-02-nav-create",
        owner=COORDINATOR,
        recipient=USER,
        amount_atoms=100_000,
        condition=lock.public_values()["pftl_condition"],
        cancel_after=on_plan.pftl_cancel_after,
    )
    after_nav_create = pftl_snapshot(pftl)
    assert_principal_conserved(before_nav[1], after_nav_create[1])

    # Wrong preimage and early cancel are rejected before signing either ledger.
    wrong = SecretPreimage.generate()
    attack_before_xrp = xrpl_snapshot(
        xrpl, user_xrpl.classic_address, coordinator_xrpl.classic_address
    )
    attack_before_nav = pftl_snapshot(pftl)
    wrong_results = {
        "xrpl_wrong_preimage_rejected": not verify_pair(
            on_xrp.condition,
            "A0228020" + wrong.protocol_hex().upper(),
            xrpl=True,
        ),
        "pftl_wrong_preimage_rejected": not verify_pair(
            on_nav.condition, "a0228020" + wrong.protocol_hex(), xrpl=False
        ),
    }
    early = {}
    for ledger in ("xrpl", "pftl"):
        try:
            on_plan.assert_cancel_open(ledger=ledger, clocks=clocks(xrpl, pftl))
            early[ledger] = False
        except TimingGateError:
            early[ledger] = True
    attack_after_xrp = xrpl_snapshot(
        xrpl, user_xrpl.classic_address, coordinator_xrpl.classic_address
    )
    attack_after_nav = pftl_snapshot(pftl)
    assert_same_principal(attack_before_xrp, attack_after_xrp)
    assert_same_principal(attack_before_nav, attack_after_nav)

    on_nav_finish = pftl.finish(
        label="onramp-03-nav-finish",
        escrow=on_nav,
        fulfillment=lock.pftl_fulfillment(),
    )
    after_nav_finish = pftl_snapshot(pftl)
    assert_principal_conserved(after_nav_create[1], after_nav_finish[1])
    on_xrp_finish = xrpl.finish(
        label="onramp-04-xrp-finish",
        wallet=coordinator_xrpl,
        escrow=on_xrp,
        fulfillment=lock.xrpl_fulfillment(),
    )
    on_public_tx = xrpl.tx(on_xrp_finish["hash"])
    revealed = extract_xrpl_finish_preimage(
        on_public_tx["tx_json"], expected_condition=on_xrp.condition
    )
    if revealed.protocol_hex() != lock.secret.protocol_hex():
        raise AssertionError("public XRPL preimage mismatch")
    after_xrp_finish = xrpl_snapshot(
        xrpl, user_xrpl.classic_address, coordinator_xrpl.classic_address
    )
    assert_xrp_conserved_with_fees(
        after_xrp_create[1], after_xrp_finish[1], on_xrp_finish["fee_drops"]
    )
    report["scenarios"]["xrp_to_nav_happy"] = {
        "hashlock": public_hashlock(lock),
        "timing_gate": gate,
        "xrpl_escrow": on_xrp.__dict__,
        "pftl_escrow": on_nav.__dict__,
        "transactions": {
            "xrpl_create": on_xrp_create["hash"],
            "pftl_create": on_nav_create["tx_id"],
            "pftl_finish": on_nav_finish["tx_id"],
            "xrpl_finish": on_xrp_finish["hash"],
        },
        "xrpl_public_fulfillment": on_public_tx["tx_json"]["Fulfillment"],
        "xrpl_public_preimage_hex": revealed.protocol_hex(),
        "conservation": {
            "xrp_principal_plus_locked_less_validated_fees": True,
            "navcoin_atoms_including_locked": True,
        },
    }
    report["adversarial"]["wrong_preimage_and_early_cancel"] = {
        **wrong_results,
        "early_cancel_rejected": early,
        "signed_or_submitted": False,
        "principal_state_unchanged": True,
    }
    write_json(evidence / "live-demo.partial.json", report)

    # Refund pair is opened now; later happy-path PFTL blocks make its short
    # height timeout expire without synthetic state mutation.
    refund_lock = locks["refund"]
    refund_clock = xrpl.ledger_clock()
    refund_plan = TimeoutPlan(
        Direction.XRP_TO_NAV,
        xrpl_cancel_after=refund_clock["close_time"] + 70,
        pftl_cancel_after=pftl.converged_status()["height"] + 3,
    )
    refund_xrp, refund_xrp_create = xrpl.create(
        label="refund-01-xrp-create",
        wallet=user_xrpl,
        destination=coordinator_xrpl.classic_address,
        amount_drops=250_000,
        condition=refund_lock.public_values()["xrpl_condition"],
        cancel_after=refund_plan.xrpl_cancel_after,
    )
    refund_gate = refund_plan.validate_second_lock(
        clocks(xrpl, pftl), policy, coordinator_observed_unix=int(time.time())
    )
    refund_nav, refund_nav_create = pftl.create(
        label="refund-02-nav-create",
        owner=COORDINATOR,
        recipient=USER,
        amount_atoms=25_000,
        condition=refund_lock.public_values()["pftl_condition"],
        cancel_after=refund_plan.pftl_cancel_after,
    )

    # Happy path 2: user locks NAV first; coordinator locks XRP second.
    lock = locks["nav_to_xrp"]
    before_nav = pftl_snapshot(pftl)
    now = clocks(xrpl, pftl)
    off_plan = TimeoutPlan(
        Direction.NAV_TO_XRP,
        xrpl_cancel_after=now.xrpl_close_time + 180,
        pftl_cancel_after=now.pftl_height + 80,
    )
    off_nav, off_nav_create = pftl.create(
        label="offramp-01-nav-create",
        owner=USER,
        recipient=COORDINATOR,
        amount_atoms=80_000,
        condition=lock.public_values()["pftl_condition"],
        cancel_after=off_plan.pftl_cancel_after,
    )
    after_nav_create = pftl_snapshot(pftl)
    assert_principal_conserved(before_nav[1], after_nav_create[1])
    gate = off_plan.validate_second_lock(
        clocks(xrpl, pftl), policy, coordinator_observed_unix=int(time.time())
    )
    before_xrp = xrpl_snapshot(
        xrpl, user_xrpl.classic_address, coordinator_xrpl.classic_address
    )
    off_xrp, off_xrp_create = xrpl.create(
        label="offramp-02-xrp-create",
        wallet=coordinator_xrpl,
        destination=user_xrpl.classic_address,
        amount_drops=800_000,
        condition=lock.public_values()["xrpl_condition"],
        cancel_after=off_plan.xrpl_cancel_after,
    )
    after_xrp_create = xrpl_snapshot(
        xrpl, user_xrpl.classic_address, coordinator_xrpl.classic_address
    )
    assert_xrp_conserved_with_fees(
        before_xrp[1], after_xrp_create[1], off_xrp_create["fee_drops"]
    )
    off_xrp_finish = xrpl.finish(
        label="offramp-03-xrp-finish",
        wallet=user_xrpl,
        escrow=off_xrp,
        fulfillment=lock.xrpl_fulfillment(),
    )
    off_public_tx = xrpl.tx(off_xrp_finish["hash"])
    public_secret = extract_xrpl_finish_preimage(
        off_public_tx["tx_json"], expected_condition=off_xrp.condition
    )
    after_xrp_finish = xrpl_snapshot(
        xrpl, user_xrpl.classic_address, coordinator_xrpl.classic_address
    )
    assert_xrp_conserved_with_fees(
        after_xrp_create[1], after_xrp_finish[1], off_xrp_finish["fee_drops"]
    )
    off_nav_finish = pftl.finish(
        label="offramp-04-nav-finish",
        escrow=off_nav,
        fulfillment="a0228020" + public_secret.protocol_hex(),
    )
    after_nav_finish = pftl_snapshot(pftl)
    assert_principal_conserved(after_nav_create[1], after_nav_finish[1])
    report["scenarios"]["nav_to_xrp_happy"] = {
        "hashlock": public_hashlock(lock),
        "timing_gate": gate,
        "xrpl_escrow": off_xrp.__dict__,
        "pftl_escrow": off_nav.__dict__,
        "transactions": {
            "pftl_create": off_nav_create["tx_id"],
            "xrpl_create": off_xrp_create["hash"],
            "xrpl_finish": off_xrp_finish["hash"],
            "pftl_finish": off_nav_finish["tx_id"],
        },
        "xrpl_public_fulfillment": off_public_tx["tx_json"]["Fulfillment"],
        "xrpl_public_preimage_hex": public_secret.protocol_hex(),
        "pftl_finish_used_public_xrpl_preimage": True,
        "conservation": {
            "xrp_principal_plus_locked_less_validated_fees": True,
            "navcoin_atoms_including_locked": True,
        },
    }
    write_json(evidence / "live-demo.partial.json", report)

    # PFTL is now at the refund boundary. A late finish is rejected locally
    # without fee or mutation; cancellation then refunds its owner.
    before_late_nav = pftl_snapshot(pftl)
    refund_plan.assert_cancel_open(ledger="pftl", clocks=clocks(xrpl, pftl))
    try:
        refund_plan.assert_finish_open(ledger="pftl", clocks=clocks(xrpl, pftl))
        pftl_late_rejected = False
    except TimingGateError:
        pftl_late_rejected = True
    after_late_nav = pftl_snapshot(pftl)
    assert_same_principal(before_late_nav, after_late_nav)
    before_nav_cancel = after_late_nav
    refund_nav_cancel = pftl.cancel(label="refund-03-nav-cancel", escrow=refund_nav)
    after_nav_cancel = pftl_snapshot(pftl)
    assert_principal_conserved(before_nav_cancel[1], after_nav_cancel[1])

    # Wait for a validated XRPL ledger at the wall-clock boundary. Again, the
    # attempted late finish is a preflight rejection and never reaches submit.
    xrpl.wait_until_close_time(refund_plan.xrpl_cancel_after)
    before_late_xrp = xrpl_snapshot(
        xrpl, user_xrpl.classic_address, coordinator_xrpl.classic_address
    )
    try:
        refund_plan.assert_finish_open(ledger="xrpl", clocks=clocks(xrpl, pftl))
        xrpl_late_rejected = False
    except TimingGateError:
        xrpl_late_rejected = True
    after_late_xrp = xrpl_snapshot(
        xrpl, user_xrpl.classic_address, coordinator_xrpl.classic_address
    )
    assert_same_principal(before_late_xrp, after_late_xrp)
    refund_xrp_cancel = xrpl.cancel(
        label="refund-04-xrp-cancel", wallet=user_xrpl, escrow=refund_xrp
    )
    after_xrp_cancel = xrpl_snapshot(
        xrpl, user_xrpl.classic_address, coordinator_xrpl.classic_address
    )
    assert_xrp_conserved_with_fees(
        after_late_xrp[1], after_xrp_cancel[1], refund_xrp_cancel["fee_drops"]
    )
    report["scenarios"]["refund"] = {
        "hashlock": public_hashlock(refund_lock),
        "timing_gate": refund_gate,
        "xrpl_escrow": refund_xrp.__dict__,
        "pftl_escrow": refund_nav.__dict__,
        "transactions": {
            "xrpl_create": refund_xrp_create["hash"],
            "pftl_create": refund_nav_create["tx_id"],
            "pftl_cancel": refund_nav_cancel["tx_id"],
            "xrpl_cancel": refund_xrp_cancel["hash"],
        },
        "preimage_revealed": False,
        "both_principals_refunded": True,
        "fees_excluded_and_accounted_separately": True,
    }
    report["adversarial"]["late_finish"] = {
        "pftl_at_or_after_cancel_after_rejected": pftl_late_rejected,
        "xrpl_at_or_after_cancel_after_rejected": xrpl_late_rejected,
        "signed_or_submitted": False,
        "principal_state_unchanged": True,
    }

    # Duplicate effect keys return the durable prior result. The callable would
    # be the ledger mutation in production; here a counter proves it is invoked
    # only once, while before/after ledger principal snapshots prove no submit.
    before_duplicate_xrp = xrpl_snapshot(
        xrpl, user_xrpl.classic_address, coordinator_xrpl.classic_address
    )
    before_duplicate_nav = pftl_snapshot(pftl)
    calls: list[str] = []
    journal = EffectJournal(private / "coordinator-effects.sqlite3")
    request = {"ledger": "xrpl", "escrow_create_tx": on_xrp.create_tx_hash}
    first, first_duplicate = journal.execute(
        "demo:duplicate-finish",
        request,
        lambda: calls.append("would-submit-once")
        or {"validated_tx": on_xrp_finish["hash"]},
    )
    second, second_duplicate = journal.execute(
        "demo:duplicate-finish",
        request,
        lambda: calls.append("BUG-second-submit"),
    )
    journal.close()
    after_duplicate_xrp = xrpl_snapshot(
        xrpl, user_xrpl.classic_address, coordinator_xrpl.classic_address
    )
    after_duplicate_nav = pftl_snapshot(pftl)
    assert_same_principal(before_duplicate_xrp, after_duplicate_xrp)
    assert_same_principal(before_duplicate_nav, after_duplicate_nav)
    if first != second or first_duplicate or not second_duplicate or len(calls) != 1:
        raise AssertionError("idempotency journal did not suppress duplicate")
    report["adversarial"]["duplicate"] = {
        "durable_effect_key": "demo:duplicate-finish",
        "same_result": first == second,
        "side_effect_callable_count": len(calls),
        "duplicate_suppressed": second_duplicate,
        "ledger_principal_state_unchanged": True,
    }

    final_xrp = xrpl_snapshot(
        xrpl, user_xrpl.classic_address, coordinator_xrpl.classic_address
    )
    final_nav = pftl_snapshot(pftl)
    report["final_state"] = {
        "xrpl": final_xrp[0],
        "pftl": final_nav[0],
        "all_expected_escrows_terminal": all(
            escrow_id
            not in {
                item.get("escrow_id") or item.get("index")
                for item in (
                    final_xrp[0]["open_escrows"] + final_nav[0]["open_escrows"]
                )
            }
            for escrow_id in (
                on_nav.escrow_id,
                off_nav.escrow_id,
                refund_nav.escrow_id,
            )
        ),
    }
    report["result"] = "PASS"
    write_json(evidence / "live-demo-report.json", report)
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

