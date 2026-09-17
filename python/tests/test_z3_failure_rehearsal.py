"""G5 offline rehearsal: real Python guards, synthetic chain responses only.

The Arc boundary belongs to the wrapper, not the primary reserve driver.
No synthetic proof or receipt here is evidence of consensus execution.
"""
from __future__ import annotations

from argparse import Namespace
import copy
import json
from pathlib import Path
import socket
import subprocess
from unittest.mock import Mock

import pytest

from postfiat_rpc import z3_composition as compose
from postfiat_rpc import z3_cycle as z3
from test_z3_composition import checkpoint, fixtures, inputs, signers
from test_z3_cycle import build, digest, packet_fixture, save

SCENARIOS = json.loads(
    (Path(__file__).parent / "fixtures/z3/failure_rehearsal.json").read_text()
)["scenarios"]
demo = fixtures.demo


@pytest.fixture(autouse=True)
def offline_only(monkeypatch):
    """Tripwires: subprocess/network/signing paths cannot escape the rehearsal."""
    def forbidden(*args, **kwargs):
        pytest.fail("offline rehearsal attempted external execution or signer access")
    monkeypatch.setattr(socket, "socket", forbidden)
    monkeypatch.setattr(socket, "create_connection", forbidden)
    monkeypatch.setattr(subprocess, "run", forbidden)
    monkeypatch.setattr(subprocess, "Popen", forbidden)
    original_open = Path.open

    def public_open(path, *args, **kwargs):
        if str(path).startswith("/not-opened/") or path.name == "holder-placeholder.json":
            forbidden()
        return original_open(path, *args, **kwargs)
    monkeypatch.setattr(Path, "open", public_open)
    # Only the checkout/binary availability probe is substituted. The real
    # 39-command compiler, checkpoint guards and durable markers still run.
    monkeypatch.setattr(compose, "ensure_available", lambda *args: None)


def configured(root):
    config = inputs(root)
    config["manifest"]["accounts"]["owner"] = fixtures.SUBSCRIBER
    values = {
        "INPUTS": str(root / "inputs.json"), "CHECKPOINT": str(root / "checkpoint.json"),
        "DEPOSIT_TX": "0x" + "1" * 64, "INGRESS_EXPIRES_HEIGHT": 200,
        "ARC_CONFIRMATION_DEPTH": 10, "ISSUE_HEIGHT": 101, "ROUTE_HEIGHT": 102,
        "REDEEM_HEIGHT": 103, "REDEEM_OUTPUT_ATOMS": 899,
        "WITHDRAWAL_ID": "2" * 64, "PRIOR_CHECKPOINT": "3" * 64,
        "EGRESS_PUBLIC_VALUES_HEX": "0x01", "EGRESS_PROOF_HEX": "0x02",
        "SETTLEMENT_OPERATION": str(root / "settlement.json"),
    }
    steps = compose.compile_steps(config, signers(), values)
    assert len(steps) == 39
    return config, steps


def stage_checkpoint(config, steps, name):
    index = next(i for i, step in enumerate(steps) if step.name == name)
    gate = copy.deepcopy(checkpoint(config, steps, index))
    egress = name in {"egress-witness", "egress-proof", "arc-release",
                      "release-replay-check", "prepare-egress-settlement", "egress-settle"}
    for row in gate["validators"]:
        row["queues"] = {"mempool": 0, "reservations": int(name == "subscribe"),
                         "egress": int(egress)}
    if name == "entitlement-release":
        gate["entitlement_atoms"] = config["parameters"]["mint_amount_atoms"]
    for previous in gate["completed"]:
        if previous["step"] == "release-replay-check":
            previous["rejected"] = True
    return gate


def retain_attempt(root, config, gate, step, reason, *, submission_started,
                   diagnostic, evidence=None, prior_consecutive=4, details=None):
    """Seal observed rejection separately from a never-completed cycle packet."""
    root.mkdir(exist_ok=True)
    attempt = {"step": step, "reason": reason, "submission_started": submission_started,
               "prior_consecutive": prior_consecutive}
    save(root, "observation.json", {**attempt, "diagnostic": diagnostic, "synthetic": True,
                                    "details": details or {}})
    save(root, "checkpoint.json", gate)
    output = root / "attempt.json"
    z3.build_attempt_manifest(config["manifest"], root, output, attempt,
                             {"observation": "observation.json", "checkpoint": "checkpoint.json",
                              **(evidence or {})})
    return output


def synthetic_ledger(root, stage):
    """Accounting plus identities, including recoverable nonterminal objects.

    Amounts come from the existing complete-packet fixture; the intermediate
    source reservation follows the G2 reserve/escrow contract. This is a test
    environment, not a substitute consensus implementation.
    """
    layout = packet_fixture(root)
    if stage in ("after_burn", "after_release"):
        state = z3.read_json(root / layout["egress"][stage])
    else:
        section = "ingress" if stage == "reserved" else stage
        state = z3.read_json(root / layout[section]["record"])["state"]
    if stage == "reserved":
        state.update(pfusdc_wallet=0, source_escrow=1005, reservations=1, pending_orders=1)
    past_deposit = stage != "preflight"
    subscribed = stage in ("subscription", "entitlement_release", "nav_route_epoch",
                           "redemption", "after_burn", "after_release", "egress", "final_convergence")
    burned = stage in ("after_burn", "after_release", "egress", "final_convergence")
    released = stage in ("after_release", "egress", "final_convergence")
    reservation = "12" * 48
    withdrawal = digest("withdrawal")
    ledger = {
        "accounting": state,
        "deposits": {digest("deposit-id"): {
            "amount_atoms": 1005, "credited": state["uncredited_deposits"] == 0,
        }} if past_deposit else {},
        "reservations": {reservation: {"escrow_atoms": 1005, "mint_atoms": 1000}}
                        if stage == "reserved" else {},
        "entitlements": {reservation: {"remaining_atoms": 1000}}
                        if state["entitlement_count"] else {},
        "withdrawals": {withdrawal: {
            "amount_atoms": 899, "nullifier": digest("nullifier"),
            "state": "released_unsettled" if released else "burned_pending_release",
        }} if state["pending_egress"] else {},
        "used_deposit_nonces": ["0x" + "1" * 64] if past_deposit else [],
        "used_subscription_nonces": ["12" * 32] if subscribed else [],
        "used_burn_requests": [digest("egressburn_receipt")] if burned else [],
        "used_release_nullifiers": [digest("nullifier")] if released else [],
    }
    assert_ledger_invariants(ledger)
    return ledger


def assert_ledger_invariants(ledger):
    state = ledger["accounting"]
    z3.validate_state(state)
    # Baseline holdings belong to other accounts; none of these failures may
    # spend them or double-count escrow, principal or spread as fresh supply.
    assert state["series_supply"] == 20000 + sum(
        state[field] for field in ("pfusdc_wallet", "source_principal", "source_spread", "source_escrow"))
    assert state["family_supply"] == state["series_supply"]
    assert state["native_supply"] == 10000 + state["native_wallet"]
    assert state["settlement_reserve"] == state["source_principal"]
    assert state["non_nav_spread"] == state["source_spread"]
    assert state["reservations"] == len(ledger["reservations"])
    assert state["pending_orders"] == len(ledger["reservations"])
    assert state["source_escrow"] == sum(row["escrow_atoms"] for row in ledger["reservations"].values())
    assert state["entitlement_count"] == len(ledger["entitlements"])
    assert state["entitlement_atoms"] == sum(row["remaining_atoms"] for row in ledger["entitlements"].values())
    assert state["pending_egress"] == sum(row["amount_atoms"] for row in ledger["withdrawals"].values())
    assert state["released_unsettled"] == sum(
        row["amount_atoms"] for row in ledger["withdrawals"].values()
        if row["state"] == "released_unsettled")
    assert state["uncredited_deposits"] == sum(
        row["amount_atoms"] for row in ledger["deposits"].values() if not row["credited"])
    for field in ("used_deposit_nonces", "used_subscription_nonces",
                  "used_burn_requests", "used_release_nullifiers"):
        assert len(ledger[field]) == len(set(ledger[field]))


def route_from_ledger(ledger):
    state = ledger["accounting"]
    return fixtures.route(
        min_order_atoms=1,
        authorized_valid_supply_atoms=state["native_supply"],
        pftl_spendable_supply_atoms=state["native_wallet"],
        outstanding_bridge_claims_atoms=10000,
        settlement_reserve_atoms=state["settlement_reserve"],
        non_nav_spread_atoms=state["non_nav_spread"],
        active_reservation_count=state["reservations"],
        active_reservation_atoms=sum(row["mint_atoms"] for row in ledger["reservations"].values()),
        export_entitlement_count=state["entitlement_count"],
        export_entitlement_atoms=state["entitlement_atoms"],
        native_spendable_balances=[{"wallet": fixtures.SUBSCRIBER,
                                    "amount_atoms": state["native_wallet"]}],
        source_settlement_custody=[fixtures.custody(
            principal_atoms=state["source_principal"], spread_atoms=state["source_spread"],
            reservation_escrows={key: row["escrow_atoms"] for key, row in ledger["reservations"].items()})],
    )


def assert_ledger_unchanged(before, after):
    assert_ledger_invariants(before)
    assert_ledger_invariants(after)
    z3.transition(before["accounting"], after["accounting"], {}, "rejected/replayed attempt")
    assert after == before, "failure changed an economic value or retained request identity"
    before_route, after_route = route_from_ledger(before), route_from_ledger(after)
    assert demo.economic_route_snapshot(before_route) == demo.economic_route_snapshot(after_route)
    demo.verify_source_custody_delta(before_route, after_route, fixtures.IDENTITIES, 0, 0)
    for asset, field in ((fixtures.SOURCE, "pfusdc_wallet"), (fixtures.NATIVE_ASSET, "native_wallet")):
        balances = [demo.account_balance(fixtures.balance(asset, state["accounting"][field]),
                                        asset, fixtures.SUBSCRIBER, fixtures.IDENTITIES["pftl_chain_id"])
                    for state in (before, after)]
        assert balances[0] == balances[1]


def duplicate_identity(case, ledger):
    field = {
        "duplicate_deposit": "used_deposit_nonces",
        "duplicate_subscription_nonce": "used_subscription_nonces",
        "duplicate_burn": "used_burn_requests",
        "duplicate_arc_release": "used_release_nullifiers",
    }.get(case["id"])
    if not field:
        return None
    # The exact identity in the synthetic retained response is reused, not a
    # newly generated nonce or merely another transaction of the same kind.
    return {"registry": field, "identity": ledger[field][0]}


def driver_inputs(root, ledger=None):
    """A tiny source-preserving issue/redeem fixture, with no operation build."""
    root.mkdir(exist_ok=True)
    signer = root / "holder-placeholder.json"
    # ensure_key only stats this empty placeholder; the tripwire forbids reads.
    signer.touch()
    route = fixtures.route(min_order_atoms=1, native_spendable_balances=[
        {"wallet": fixtures.SUBSCRIBER, "amount_atoms": 1000}])
    if ledger is not None:
        route = route_from_ledger(ledger)
    nav = fixtures.nav(nav_per_unit=10**8)
    issue = {
        "schema": "postfiat.a666.pfusdc_reserve_demo_issue.v1",
        "identities": fixtures.IDENTITIES, "route_id": fixtures.IDENTITIES["route_id"],
        "subscriber": fixtures.SUBSCRIBER, "pricing_nav_epoch": 1,
        "pricing_reserve_packet_hash": "10" * 48, "route_epoch": 2,
        "mint_amount_atoms": 1000, "base_value_atoms": 1000,
    }
    for name, data in (("identities.json", fixtures.IDENTITIES), ("route.json", route),
                       ("nav.json", nav), ("issue.json", issue)):
        save(root, name, data)
    args = Namespace(identities=root / "identities.json", holder_key_file=signer,
                     route_status=root / "route.json", nav_manifest=root / "nav.json",
                     issue_manifest=root / "issue.json", output_dir=root / "output",
                     current_height=101, reservation_ttl_blocks=128, expiry_ttl_blocks=128,
                     mint_amount_atoms=1000, nav_amount_atoms=900,
                     subscriber=fixtures.SUBSCRIBER, owner=fixtures.SUBSCRIBER,
                     ethereum_recipient=fixtures.RECIPIENT)
    return args, route, nav, issue


def exercise_driver(root, case, monkeypatch, ledger):
    args, route, nav, issue = driver_inputs(root, ledger)
    build_request = Mock(side_effect=AssertionError("request construction reached"))
    nonce = Mock(side_effect=AssertionError("new request identity generated"))
    monkeypatch.setattr(demo, "operation_request", build_request)
    monkeypatch.setattr(demo.secrets, "token_hex", nonce)
    mode = case["driver"]
    if mode == "stale_nav":
        nav["epoch"] = issue["pricing_nav_epoch"]
        nav["reserve_packet_hash"] = issue["pricing_reserve_packet_hash"]
        route["pricing_nav_epoch"] = nav["epoch"]
        route["pricing_reserve_packet_hash"] = nav["reserve_packet_hash"]
        expected, command = "fresh NAV", demo.cmd_build_redeem
    elif mode == "wrong_route":
        route["route_id"] = "wrong-route"
        expected, command = "route_id", demo.cmd_build_issue
    elif mode == "wrong_asset":
        route["source_settlement_custody"][0]["asset_id"] = "f" * 96
        expected, command = "source", demo.cmd_build_issue
    elif mode == "active_entitlement":
        route.update(export_entitlement_count=1, export_entitlement_atoms=1000)
        expected, command = "active order state", demo.cmd_build_redeem
    else:
        # Bridge proof and nonce assertions have no reserve-driver API.
        # The wrapper already blocked dispatch; exercise its read-only identity
        # and custody checks, without pretending it validates Arc consensus.
        demo.validate_route(route, fixtures.IDENTITIES)
        demo.validate_nav_binding(route, nav, fixtures.IDENTITIES)
        demo.verify_source_custody_delta(route, copy.deepcopy(route),
                                        fixtures.IDENTITIES, 0, 0)
        command = None
    save(root, "route.json", route)
    save(root, "nav.json", nav)
    if command:
        with pytest.raises(demo.DemoError, match=expected):
            command(args)
    build_request.assert_not_called()
    nonce.assert_not_called()
    assert not args.output_dir.exists()


@pytest.mark.parametrize("case", SCENARIOS, ids=lambda row: row["id"])
def test_failure_rejected_before_build(tmp_path, monkeypatch, case):
    config, steps = configured(tmp_path)
    ledger = synthetic_ledger(tmp_path / "state-fixture", case["state_section"])
    if case["id"] == "duplicate_subscription_nonce":
        ledger["used_subscription_nonces"].append("12" * 32)
    before = copy.deepcopy(ledger)
    identity = duplicate_identity(case, ledger)
    gate = stage_checkpoint(config, steps, case["step"])
    # Prove the unmodified synthetic checkpoint reaches the intended gate.
    index = next(i for i, step in enumerate(steps) if step.name == case["step"])
    compose.check_checkpoint(config, gate, steps, index)
    if case["field"] in ("route_id", "settlement_source_asset_id"):
        gate["identities"][case["field"]] = case["value"]
    else:
        gate[case["field"]] = case["value"]
    if identity:
        gate["nonce_unused"] = identity["identity"] not in ledger[identity["registry"]]
        assert gate["nonce_unused"] is False
    if case["id"] == "duplicate_burn":
        for row in gate["validators"]:
            row["queues"]["egress"] = len(ledger["withdrawals"])
    def forbidden_submission(*args, **kwargs):
        # A dispatch regression visibly corrupts the simulated ledger before
        # failing; unchanged-state evidence also requires zero runner calls.
        ledger["accounting"]["pfusdc_wallet"] += 1
        raise AssertionError("submission/build runner reached")
    runner = Mock(side_effect=forbidden_submission)
    with pytest.raises(z3.CycleError, match=case["guard"]) as caught:
        compose.confirm_one(config, gate, steps, case["step"], tmp_path, runner)
    runner.assert_not_called()
    assert not Path(config["paths"]["work_dir"]).exists()
    exercise_driver(tmp_path / "driver", case, monkeypatch, ledger)
    assert_ledger_unchanged(before, ledger)

    # A duplicate is a fresh unsafe attempt, not the mandatory read-only
    # release-replay-check of a clean cycle. Never disguise it as interruption.
    prior_submission = case["id"] == "duplicate_deposit" or any(
        step.kind in ("arc", "pftl") for step in steps[:index])
    root = tmp_path / "attempt"
    root.mkdir()
    save(root, "before.json", before)
    save(root, "after.json", ledger)
    attempt = retain_attempt(root, config, gate, case["step"], case["reason"],
                             submission_started=prior_submission, diagnostic=str(caught.value),
                             evidence={"before": "before.json", "after": "after.json"},
                             details={"scenario": case["id"], "request_identity": identity})
    recorded = z3.read_json(attempt)["attempt"]
    assert recorded["status"] == "unclean"
    assert recorded["reason"] == case["reason"]
    verdict = z3.verify_manifest(attempt)
    assert verdict["verdict"] == "FAIL", verdict
    assert verdict["reason"] == case["reason"]
    assert verdict["pause_campaign"] and verdict["reset_required_after_correction"]
    assert verdict["consecutive_after_correction"] == 0


@pytest.mark.parametrize("case", SCENARIOS, ids=lambda row: row["id"])
def test_failure_cannot_pass_cycle_verifier(tmp_path, case):
    """Independent negative receipt/readback audit, not a continued failed run."""
    layout = packet_fixture(tmp_path)
    path = layout[case["packet_section"]][case["packet_role"]]
    data = z3.read_json(tmp_path / path)
    if case["id"] in ("stale_nav", "stale_arc_proof"):
        data["valid_until_height"] = 1
    elif case["id"] in ("wrong_route", "wrong_asset"):
        data["identities"][case["field"]] = case["value"]
    elif case["id"] == "active_entitlement":
        data["state"].update(entitlement_atoms=1000, entitlement_count=1)
    elif case["packet_role"] == "receipt":
        data["status"] = "0x0"
    else:
        data["accepted"] = False
    save(tmp_path, path, data)
    verdict = z3.verify_manifest(build(tmp_path, layout))
    assert verdict["verdict"] == "FAIL", verdict
    assert case["packet_error"] in verdict["error"], verdict


def test_attempt_artifacts_are_bound_and_cannot_be_overwritten(tmp_path):
    config, steps = configured(tmp_path)
    gate = stage_checkpoint(config, steps, "preflight")
    root = tmp_path / "attempt"
    manifest = retain_attempt(root, config, gate, "preflight", "unsafe_preflight",
                              submission_started=False, diagnostic="stale NAV")
    original = manifest.read_bytes()
    # Unsafe preflight is not the environmental interruption exception.
    assert z3.verify_manifest(manifest)["consecutive_after_correction"] == 0
    with pytest.raises(z3.CycleError, match="overwrite"):
        retain_attempt(root, config, gate, "preflight", "unsafe_preflight",
                       submission_started=False, diagnostic="stale NAV")
    assert manifest.read_bytes() == original
    save(root, "observation.json", {"changed": True})
    assert "attempt artifact hash" in z3.verify_manifest(manifest)["error"]


@pytest.mark.parametrize("field", [
    "native_wallet", "family_supply", "settlement_reserve", "reservations",
    "entitlement_atoms", "pending_egress", "source_escrow", "arc_wallet_wei",
])
def test_invariant_audit_detects_economic_mutation(tmp_path, field):
    before = synthetic_ledger(tmp_path / "state-fixture", "after_burn")
    after = copy.deepcopy(before)
    after["accounting"][field] += 1
    with pytest.raises((z3.CycleError, AssertionError)):
        assert_ledger_unchanged(before, after)


def test_invariant_audit_detects_withdrawal_identity_substitution(tmp_path):
    before = synthetic_ledger(tmp_path / "state-fixture", "after_burn")
    after = copy.deepcopy(before)
    withdrawal = after["withdrawals"].pop(digest("withdrawal"))
    after["withdrawals"][digest("different-withdrawal")] = withdrawal
    with pytest.raises(AssertionError, match="retained request identity"):
        assert_ledger_unchanged(before, after)


def test_attempt_outcome_cannot_be_relabelled_clean(tmp_path):
    config, steps = configured(tmp_path)
    manifest = retain_attempt(tmp_path / "attempt", config,
                              stage_checkpoint(config, steps, "preflight"),
                              "preflight", "unsafe_preflight", submission_started=False,
                              diagnostic="inconsistent nonce")
    data = z3.read_json(manifest)
    data["attempt"]["status"] = "clean"
    save(manifest.parent, manifest.name, data)
    assert "attempt outcome/status" in z3.verify_manifest(manifest)["error"]
