"""G5 process-loss/replay rehearsal; all terminal responses are synthetic."""
from __future__ import annotations

import copy
import hashlib
import json
import subprocess
import sys
from pathlib import Path
from unittest.mock import Mock

import pytest

from postfiat_rpc import z3_composition as compose
from postfiat_rpc import z3_cycle as z3
from test_z3_failure_rehearsal import (
    assert_ledger_invariants, assert_ledger_unchanged, configured, demo, driver_inputs,
    offline_only, retain_attempt, stage_checkpoint, synthetic_ledger,
)
from test_z3_cycle import save

CONFIRMED_STEPS = ("arc-deposit", "subscribe", "burn", "arc-release")
RECOVERY_STATES = {
    "arc-deposit": ("preflight", "deposit"),
    "subscribe": ("reserved", "subscription"),
    "burn": ("redemption", "after_burn"),
    "arc-release": ("after_burn", "after_release"),
}
CONFIRMED_DELTAS = {
    "arc-deposit": {"source_vault": 1005, "uncredited_deposits": 1005,
                    "arc_wallet_wei": -1005 * 10**12 - 17},
    "subscribe": {"native_supply": 1000, "native_wallet": 1000,
                  "settlement_reserve": 1000, "source_principal": 1000,
                  "source_spread": 5, "non_nav_spread": 5, "source_escrow": -1005,
                  "reservations": -1, "pending_orders": -1,
                  "entitlement_atoms": 1000, "entitlement_count": 1},
    "burn": {"family_supply": -899, "series_supply": -899, "pfusdc_wallet": -899,
             "redeemed_total": 899, "pending_egress": 899},
    "arc-release": {"source_vault": -899, "released_unsettled": 899,
                    "arc_wallet_wei": 899 * 10**12 - 23},
}


def prepare_request(root, config, steps, name, monkeypatch):
    """Build unsigned primary requests with the real driver, stub bridge data."""
    step = next(step for step in steps if step.name == name)
    work = Path(config["paths"]["work_dir"])
    work.mkdir(exist_ok=True)
    if name == "subscribe":
        issue_state = synthetic_ledger(root / "issue-driver-state", "ingress")
        args, _, _, _ = driver_inputs(root / "driver", issue_state)
        args.output_dir = work / "issue"
        monkeypatch.setattr(demo.secrets, "token_hex", lambda width: "12" * width)
        issue = demo.cmd_build_issue(args)
        assert issue["subscription_nonce"] == "12" * 32
    elif name == "burn":
        ids, accounts = config["manifest"]["identities"], config["manifest"]["accounts"]
        body = {
            "operation": "vault_bridge_burn_to_redeem", "owner": accounts["owner"],
            "issuer": accounts["route_operator"], "asset_id": ids["settlement_asset_id"],
            "bucket_id": ids["source_bucket_id"], "amount_atoms": 899, "epoch": 8,
            "reserve_packet_hash": "34" * 48,
            "destination_ref": "evm-erc20:5042002:" + accounts["arc_wallet"].lower(),
        }
        # This wraps synthetic public operation data; it does not sign or read
        # the path supplied as the signer argument.
        compose.write_request(Path(step.request), body, accounts["owner"], "/not-opened/holder")
    elif name == "arc-release":
        proof = work / "egress-proof"
        proof.mkdir()
        (proof / "public-values.bin").write_bytes(b"\x01")
        (proof / "proof-calldata.bin").write_bytes(b"\x02")
    gate = stage_checkpoint(config, steps, name)
    if step.request:
        raw = Path(step.request).read_bytes()
        operation = json.loads(raw)["operations"][0]["operation"]
        gate["next_operation_sha256"] = hashlib.sha256(
            json.dumps(operation, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    return step, gate


@pytest.mark.parametrize("name", CONFIRMED_STEPS)
def test_confirmed_interruption_exact_identity_rejected_as_replay(tmp_path, monkeypatch, name):
    config, steps = configured(tmp_path)
    step, gate = prepare_request(tmp_path, config, steps, name, monkeypatch)
    prior_stage, confirmed_stage = RECOVERY_STATES[name]
    ledger = synthetic_ledger(tmp_path / "prior-state", prior_stage)
    before = copy.deepcopy(ledger)
    confirmed = synthetic_ledger(tmp_path / "confirmed-state", confirmed_stage)
    work = Path(config["paths"]["work_dir"])
    calls = []
    terminal = {
        "synthetic": True, "step": name, "accepted": True, "finalized": True,
        "tx_id": hashlib.sha256(name.encode()).hexdigest(),
        "command_sha256": step.command_sha256,
        "request_sha256": hashlib.sha256(Path(step.request).read_bytes()).hexdigest()
                          if step.request else None,
    }
    retained_terminal = tmp_path / "terminal.json"

    def interrupt_after_confirmation(argv, **kwargs):
        calls.append(argv)
        assert argv == step.argv
        # The test environment confirms once, then the wrapper process loses
        # control before returning. Exit status is never confirmation evidence.
        ledger.clear()
        ledger.update(copy.deepcopy(confirmed))
        z3.write_new(retained_terminal, terminal)
        json.dump(terminal, kwargs["stdout"])
        raise KeyboardInterrupt("synthetic process loss after confirmed response")

    with pytest.raises(KeyboardInterrupt):
        compose.confirm_one(config, gate, steps, name, tmp_path, interrupt_after_confirmation)
    assert_ledger_invariants(ledger)
    z3.transition(before["accounting"], ledger["accounting"], CONFIRMED_DELTAS[name],
                  "one confirmed transition")
    after_confirmation = copy.deepcopy(ledger)
    marker_path = work / "attempts" / (name + ".json")
    marker_bytes = marker_path.read_bytes()
    stdout_bytes = (work / (name + ".stdout.json")).read_bytes()
    request_bytes = Path(step.request).read_bytes() if step.request else None
    marker = z3.read_json(marker_path)
    assert marker["state"] == "attempted"  # never promoted by exit code
    assert marker["command_sha256"] == terminal["command_sha256"]
    assert marker["request_sha256"] == terminal["request_sha256"]
    assert marker["inputs_sha256"] == compose.metadata_hash(config)
    assert marker["kind"] in ("arc", "pftl")

    # Reconstruct config/commands as a new invocation; only retained files carry
    # the replay fence. No runner memory or newly generated nonce is consulted.
    restarted_config, restarted_steps = configured(tmp_path)
    replay_runner = Mock(side_effect=AssertionError("recovery resubmitted"))
    with pytest.raises(z3.CycleError, match="replay refused"):
        compose.confirm_one(restarted_config, copy.deepcopy(gate), restarted_steps,
                            name, tmp_path, replay_runner)
    replay_runner.assert_not_called()
    assert calls == [step.argv]
    assert marker_path.read_bytes() == marker_bytes
    assert (work / (name + ".stdout.json")).read_bytes() == stdout_bytes
    assert z3.read_json(retained_terminal) == terminal
    assert_ledger_unchanged(after_confirmation, ledger)
    if step.request:
        assert Path(step.request).read_bytes() == request_bytes

    root = tmp_path / "recovery"
    root.mkdir()
    save(root, "marker.json", marker)
    save(root, "terminal.json", terminal)
    save(root, "before.json", before)
    save(root, "confirmed.json", after_confirmation)
    save(root, "after-replay.json", ledger)
    manifest = retain_attempt(
        root, config, gate, name, "environmental_interruption", submission_started=True,
        diagnostic="process interrupted after synthetic confirmation; exact replay refused",
        evidence={"marker": "marker.json", "terminal": "terminal.json",
                  "before": "before.json", "confirmed": "confirmed.json", "after": "after-replay.json"})
    verdict = z3.verify_manifest(manifest)
    assert verdict["verdict"] == "FAIL"
    assert verdict["status"] == "unclean"
    assert verdict["counts_as_cycle"] and verdict["pause_campaign"]
    assert verdict["consecutive_after_correction"] == 0


@pytest.mark.parametrize("name,submitted", [("preflight", False), ("ingress-capture", True)])
def test_environmental_interruption_depends_on_prior_submission(tmp_path, name, submitted):
    config, steps = configured(tmp_path)
    gate = stage_checkpoint(config, steps, name)
    ledger = synthetic_ledger(tmp_path / "state-fixture", "deposit" if submitted else "preflight")
    before = copy.deepcopy(ledger)
    calls = []

    def interruption(argv, **kwargs):
        calls.append(argv)
        raise KeyboardInterrupt("synthetic environmental interruption during preparation")

    with pytest.raises(KeyboardInterrupt):
        compose.confirm_one(config, gate, steps, name, tmp_path, interruption)
    marker_path = Path(config["paths"]["work_dir"]) / "attempts" / (name + ".json")
    marker = z3.read_json(marker_path)
    assert marker["kind"] == "prepare"
    assert len(calls) == 1
    assert_ledger_unchanged(before, ledger)
    assert any(item["step"] == "arc-deposit" for item in gate["completed"]) == submitted
    root = tmp_path / "environment"
    root.mkdir()
    save(root, "marker.json", marker)
    save(root, "before.json", before)
    save(root, "after.json", ledger)
    manifest = retain_attempt(
        root, config, gate, name, "environmental_interruption", submission_started=submitted,
        diagnostic="interrupted during preparation",
        evidence={"marker": "marker.json", "before": "before.json", "after": "after.json"},
        prior_consecutive=4)
    verdict = z3.verify_manifest(manifest)
    assert verdict["verdict"] == ("FAIL" if submitted else "NOT_A_CYCLE")
    assert verdict["counts_as_cycle"] == submitted
    assert verdict["reset_required_after_correction"] == submitted
    assert verdict["consecutive_after_correction"] == (0 if submitted else 4)
    assert verdict["pause_campaign"] == submitted
    assert_ledger_unchanged(before, ledger)
    retry = Mock(side_effect=AssertionError("preparation retried without reconciliation"))
    with pytest.raises(z3.CycleError, match="replay refused"):
        compose.confirm_one(config, gate, steps, name, tmp_path, retry)
    retry.assert_not_called()


@pytest.mark.parametrize("reason", ["unsafe_preflight", "environmental_interruption"])
def test_incomplete_attempt_cli_never_exits_as_clean(tmp_path, monkeypatch, capsys, reason):
    config, steps = configured(tmp_path)
    manifest = retain_attempt(
        tmp_path / "attempt", config, stage_checkpoint(config, steps, "preflight"),
        "preflight", reason, submission_started=False, diagnostic="synthetic pre-submission stop")
    monkeypatch.setattr(sys, "argv", ["z3-cycle", "verify", str(manifest)])
    assert z3.main() == 1
    verdict = json.loads(capsys.readouterr().out)
    assert verdict["verdict"] == ("FAIL" if reason == "unsafe_preflight" else "NOT_A_CYCLE")


@pytest.mark.parametrize("command", ["issue", "redeem"])
def test_driver_restart_never_replaces_request_identity(tmp_path, monkeypatch, command):
    ledger = synthetic_ledger(tmp_path / "driver-state", "ingress" if command == "issue" else "nav_route_epoch")
    before_state = copy.deepcopy(ledger)
    args, _, _, _ = driver_inputs(tmp_path / "driver", ledger)
    monkeypatch.setattr(demo.secrets, "token_hex", lambda width: "12" * width)
    builder = demo.cmd_build_issue if command == "issue" else demo.cmd_build_redeem
    original = builder(args)
    before = {path.name: path.read_bytes() for path in args.output_dir.iterdir()}
    nonce = Mock(side_effect=AssertionError("restart generated a replacement nonce"))
    request = Mock(side_effect=AssertionError("restart built a replacement request"))
    monkeypatch.setattr(demo.secrets, "token_hex", nonce)
    monkeypatch.setattr(demo, "operation_request", request)
    with pytest.raises(demo.DemoError, match="refusing to overwrite"):
        builder(copy.deepcopy(args))
    nonce.assert_not_called()
    request.assert_not_called()
    assert {path.name: path.read_bytes() for path in args.output_dir.iterdir()} == before
    nonce_field = "subscription_nonce" if command == "issue" else "redemption_nonce"
    manifest_name = "issue-manifest.json" if command == "issue" else "redeem-manifest.json"
    assert json.loads(before[manifest_name])[nonce_field] == original[nonce_field]
    assert_ledger_unchanged(before_state, ledger)


@pytest.mark.parametrize("name", ["arc-deposit", "subscribe"])
def test_publication_evidence_blocks_false_environmental_exception(tmp_path, name):
    config, steps = configured(tmp_path)
    gate = stage_checkpoint(config, steps, name)
    root = tmp_path / "false-exception"
    root.mkdir()
    save(root, "marker.json", {"step": name, "kind": "arc" if name == "arc-deposit" else "pftl",
                               "state": "attempted"})
    manifest = retain_attempt(
        root, config, gate, name, "environmental_interruption", submission_started=False,
        diagnostic="untrusted projection incorrectly claims no submission",
        evidence={"marker": "marker.json"})
    verdict = z3.verify_manifest(manifest)
    assert verdict["verdict"] == "FAIL"
    assert "prevents environmental exemption" in verdict["error"]


@pytest.mark.parametrize("name", ["arc-deposit", "ingress-proof"])
def test_declared_command_timeout_pauses_without_retry(tmp_path, name):
    config, steps = configured(tmp_path)
    gate = stage_checkpoint(config, steps, name)
    ledger = synthetic_ledger(tmp_path / "state-fixture", "preflight" if name == "arc-deposit" else "deposit")
    before = copy.deepcopy(ledger)
    calls = []
    def timeout(argv, **kwargs):
        calls.append(argv)
        assert kwargs["timeout"] == config["parameters"]["latency_bound_seconds"]
        raise subprocess.TimeoutExpired(argv, kwargs["timeout"])
    with pytest.raises(subprocess.TimeoutExpired):
        compose.confirm_one(config, gate, steps, name, tmp_path, timeout)
    marker_path = Path(config["paths"]["work_dir"]) / "attempts" / (name + ".json")
    root = tmp_path / "timeout"
    root.mkdir()
    save(root, "marker.json", z3.read_json(marker_path))
    save(root, "before.json", before)
    save(root, "after.json", ledger)
    # Even without a terminal response, publication is possible after a value
    # step starts, or when the preceding deposit is already confirmed.
    manifest = retain_attempt(
        root, config, gate, name, "timeout", submission_started=True,
        diagnostic="declared command bound exceeded; publication requires reconciliation",
        evidence={"marker": "marker.json", "before": "before.json", "after": "after.json"})
    verdict = z3.verify_manifest(manifest)
    assert verdict["verdict"] == "FAIL" and verdict["pause_campaign"]
    assert verdict["reset_required_after_correction"]
    retry = Mock(side_effect=AssertionError("timeout retried"))
    with pytest.raises(z3.CycleError, match="replay refused"):
        compose.confirm_one(config, gate, steps, name, tmp_path, retry)
    retry.assert_not_called()
    assert len(calls) == 1
    assert_ledger_unchanged(before, ledger)


def test_changed_request_cannot_bypass_retained_attempt(tmp_path, monkeypatch):
    config, steps = configured(tmp_path)
    step, gate = prepare_request(tmp_path, config, steps, "subscribe", monkeypatch)
    ledger = synthetic_ledger(tmp_path / "prior-state", "reserved")
    confirmed = synthetic_ledger(tmp_path / "confirmed-state", "subscription")
    before = copy.deepcopy(ledger)
    calls = []
    def interrupt(argv, **kwargs):
        calls.append(argv)
        ledger.clear()
        ledger.update(copy.deepcopy(confirmed))
        raise KeyboardInterrupt("uncertain publication")
    with pytest.raises(KeyboardInterrupt):
        compose.confirm_one(config, gate, steps, step.name, tmp_path, interrupt)
    marker_path = Path(config["paths"]["work_dir"]) / "attempts/subscribe.json"
    marker = marker_path.read_bytes()
    request = json.loads(Path(step.request).read_bytes())
    request["operations"][0]["operation"]["subscription_nonce"] = "56" * 32
    Path(step.request).write_text(json.dumps(request))
    operation = request["operations"][0]["operation"]
    gate["next_operation_sha256"] = hashlib.sha256(
        json.dumps(operation, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    retry = Mock(side_effect=AssertionError("changed identity resubmitted"))
    with pytest.raises(z3.CycleError, match="replay refused"):
        compose.confirm_one(config, gate, steps, step.name, tmp_path, retry)
    retry.assert_not_called()
    assert len(calls) == 1
    assert marker_path.read_bytes() == marker
    z3.transition(before["accounting"], ledger["accounting"], CONFIRMED_DELTAS["subscribe"],
                  "original uncertain submission only")
    assert_ledger_unchanged(confirmed, ledger)
