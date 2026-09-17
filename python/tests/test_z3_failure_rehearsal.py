"""G5 offline rehearsal: real Python guards, synthetic chain responses only.

The Arc boundary belongs to the wrapper, not the primary reserve driver.
No synthetic proof or receipt here is evidence of consensus execution.
"""
from __future__ import annotations

from argparse import Namespace
import copy
import hashlib
import json
from pathlib import Path
import socket
import subprocess
from unittest.mock import Mock

import pytest

from postfiat_rpc import z3_composition as compose
from postfiat_rpc import z3_cycle as z3
from test_z3_composition import checkpoint, fixtures, inputs, signers
from test_z3_cycle import build, metadata, packet_fixture, save

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
                   diagnostic, evidence=None, prior_consecutive=4):
    """Seal observed rejection separately from a never-completed cycle packet."""
    root.mkdir(exist_ok=True)
    attempt = {"step": step, "reason": reason, "submission_started": submission_started,
               "prior_consecutive": prior_consecutive}
    save(root, "observation.json", {**attempt, "diagnostic": diagnostic, "synthetic": True})
    save(root, "checkpoint.json", gate)
    output = root / "attempt.json"
    z3.build_attempt_manifest(config["manifest"], root, output, attempt,
                             {"observation": "observation.json", "checkpoint": "checkpoint.json",
                              **(evidence or {})})
    return output


def driver_inputs(root):
    """A tiny source-preserving issue/redeem fixture, with no operation build."""
    root.mkdir(exist_ok=True)
    signer = root / "holder-placeholder.json"
    # ensure_key only stats this empty placeholder; the tripwire forbids reads.
    signer.touch()
    route = fixtures.route(min_order_atoms=1, native_spendable_balances=[
        {"wallet": fixtures.SUBSCRIBER, "amount_atoms": 1000}])
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


def exercise_driver(root, case, monkeypatch):
    args, route, nav, issue = driver_inputs(root)
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
    gate = stage_checkpoint(config, steps, case["step"])
    # Prove the unmodified synthetic checkpoint reaches the intended gate.
    index = next(i for i, step in enumerate(steps) if step.name == case["step"])
    compose.check_checkpoint(config, gate, steps, index)
    if case["field"] in ("route_id", "settlement_source_asset_id"):
        gate["identities"][case["field"]] = case["value"]
    else:
        gate[case["field"]] = case["value"]
    runner = Mock(side_effect=AssertionError("submission/build runner reached"))
    with pytest.raises(z3.CycleError, match=case["guard"]) as caught:
        compose.confirm_one(config, gate, steps, case["step"], tmp_path, runner)
    runner.assert_not_called()
    assert not Path(config["paths"]["work_dir"]).exists()
    exercise_driver(tmp_path / "driver", case, monkeypatch)

    # A duplicate is a fresh unsafe attempt, not the mandatory read-only
    # release-replay-check of a clean cycle. Never disguise it as interruption.
    prior_submission = case["id"] == "duplicate_deposit" or any(
        step.kind in ("arc", "pftl") for step in steps[:index])
    attempt = retain_attempt(tmp_path / "attempt", config, gate, case["step"], case["reason"],
                             submission_started=prior_submission, diagnostic=str(caught.value))
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
