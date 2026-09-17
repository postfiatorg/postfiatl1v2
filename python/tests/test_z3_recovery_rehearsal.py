"""G5 process-loss/replay rehearsal; all terminal responses are synthetic."""
from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path
from unittest.mock import Mock

import pytest

from postfiat_rpc import z3_composition as compose
from postfiat_rpc import z3_cycle as z3
from test_z3_failure_rehearsal import (
    configured, demo, driver_inputs, offline_only, retain_attempt, stage_checkpoint,
)
from test_z3_cycle import save

CONFIRMED_STEPS = ("arc-deposit", "subscribe", "burn", "arc-release")


def prepare_request(root, config, steps, name, monkeypatch):
    """Build unsigned primary requests with the real driver, stub bridge data."""
    step = next(step for step in steps if step.name == name)
    work = Path(config["paths"]["work_dir"])
    work.mkdir(exist_ok=True)
    if name == "subscribe":
        args, _, _, _ = driver_inputs(root / "driver")
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
        z3.write_new(retained_terminal, terminal)
        json.dump(terminal, kwargs["stdout"])
        raise KeyboardInterrupt("synthetic process loss after confirmed response")

    with pytest.raises(KeyboardInterrupt):
        compose.confirm_one(config, gate, steps, name, tmp_path, interrupt_after_confirmation)
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
    if step.request:
        assert Path(step.request).read_bytes() == request_bytes

    root = tmp_path / "recovery"
    root.mkdir()
    save(root, "marker.json", marker)
    save(root, "terminal.json", terminal)
    manifest = retain_attempt(
        root, config, gate, name, "environmental_interruption", submission_started=True,
        diagnostic="process interrupted after synthetic confirmation; exact replay refused",
        evidence={"marker": "marker.json", "terminal": "terminal.json"})
    verdict = z3.verify_manifest(manifest)
    assert verdict["verdict"] == "FAIL"
    assert verdict["status"] == "unclean"
    assert verdict["counts_as_cycle"] and verdict["pause_campaign"]
    assert verdict["consecutive_after_correction"] == 0


@pytest.mark.parametrize("name,submitted", [("preflight", False), ("ingress-capture", True)])
def test_environmental_interruption_depends_on_prior_submission(tmp_path, name, submitted):
    config, steps = configured(tmp_path)
    gate = stage_checkpoint(config, steps, name)
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
    assert any(item["step"] == "arc-deposit" for item in gate["completed"]) == submitted
    root = tmp_path / "environment"
    root.mkdir()
    save(root, "marker.json", marker)
    manifest = retain_attempt(
        root, config, gate, name, "environmental_interruption", submission_started=submitted,
        diagnostic="interrupted during preparation", evidence={"marker": "marker.json"},
        prior_consecutive=4)
    verdict = z3.verify_manifest(manifest)
    assert verdict["verdict"] == ("FAIL" if submitted else "NOT_A_CYCLE")
    assert verdict["counts_as_cycle"] == submitted
    assert verdict["reset_required_after_correction"] == submitted
    assert verdict["consecutive_after_correction"] == (0 if submitted else 4)
    assert verdict["pause_campaign"] == submitted
    retry = Mock(side_effect=AssertionError("preparation retried without reconciliation"))
    with pytest.raises(z3.CycleError, match="replay refused"):
        compose.confirm_one(config, gate, steps, name, tmp_path, retry)
    retry.assert_not_called()


@pytest.mark.parametrize("command", ["issue", "redeem"])
def test_driver_restart_never_replaces_request_identity(tmp_path, monkeypatch, command):
    args, _, _, _ = driver_inputs(tmp_path / "driver")
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


def test_changed_request_cannot_bypass_retained_attempt(tmp_path, monkeypatch):
    config, steps = configured(tmp_path)
    step, gate = prepare_request(tmp_path, config, steps, "subscribe", monkeypatch)
    calls = []
    def interrupt(argv, **kwargs):
        calls.append(argv)
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
