"""Offline ordering, hard-stop and negative packet tests."""
from __future__ import annotations

import copy
from datetime import datetime, timedelta, timezone
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
from types import SimpleNamespace

import pytest

from postfiat_rpc import z3_composition as compose
from postfiat_rpc import z3_cycle as z3
from test_z3_cycle import build, metadata, packet_fixture, save

spec = importlib.util.spec_from_file_location(
    "z3_driver_fixtures", compose.REPO / "scripts/test-a666-pfusdc-reserve-demo.py")
assert spec and spec.loader
fixtures = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixtures)


def inputs(root: Path) -> dict:
    meta = metadata()
    meta["accounts"].update(route_operator="pffcb93d9f87a843a8aa34e1adf241f5d58143e81b",
                            reserve_operator="pfd0c86d9084915e1fefd22eab891806397d5a5937",
                            bridge_settler="pf" + "2" * 40)
    meta["proof_keys"]["nav"] = fixtures.IDENTITIES["nav_program_vkey"]
    meta["proof_keys"]["ingress"] = "0x0050a8b0daed2fa75f44d4102b42204c34668a03d311cb727cc6ca3f8df5cf16"
    meta["proof_keys"]["egress"] = "0x0036cbe7d36bbfe1118a3c544eeba74f3791d2a19bf5ec59b972f72d36416852"
    meta["policy_hashes"]["nav_valuation"] = fixtures.IDENTITIES["nav_valuation_policy_hash"]
    names = ("node_bin", "prover_bin", "egress_elf", "data_dir", "work_dir", "packet_dir",
             "remote_runner", "proposer_hosts_file", "remote_binary", "remote_topology",
             "opening_nav_manifest", "fresh_packet_operation")
    return {"manifest": meta, "reserve_identities": copy.deepcopy(fixtures.IDENTITIES),
            "paths": {key: str(root / key) for key in names},
            "parameters": {"cap_atoms": 1000000, "mint_amount_atoms": 1000,
                           "window_start": "2026-09-17T00:00:00Z",
                           "window_end": "2026-09-24T00:00:00Z",
                           "reservation_ttl_blocks": 128, "latency_bound_seconds": 45,
                           "deposit_nonce": "0x" + "1" * 64,
                           "route_binding": "0xd9e0cd409c5d1e118d65c78ee059adcbba937616353e9675e350d52ee8d498b2",
                           "reservation_recipient": "0x" + "2" * 40, "egress_release": "arc-v2"}}


def signers() -> dict:
    return {key: "/not-opened/" + key for key in
            ("keystore", "password_file", "holder", "proposer", "finalizer", "issuer", "reserve", "settler")}


def checkpoint(config, steps, index=0):
    final = {"height": 101, "block_id": "b" * 64, "state_root": "c" * 64}
    now = datetime.now(timezone.utc)
    record = {"captured_utc": (now - timedelta(seconds=1)).isoformat().replace("+00:00", "Z"),
              "expires_utc": (now + timedelta(seconds=120)).isoformat().replace("+00:00", "Z"),
              "inputs_sha256": compose.metadata_hash(config), "ready_for": steps[index].name,
              "identities": config["manifest"]["identities"], "accounts": config["manifest"]["accounts"],
              "finalized": final, "proof_valid_from": 100, "proof_valid_until": 200,
              "entitlement_atoms": 0, "required_capacity_atoms": 1005, "available_capacity_atoms": 1005,
              "arc_usdc_balance_atoms": 1005, "allowance_atoms": 1005,
              "arc_required_gas_wei": 1, "arc_wallet_wei": 10**20,
              "completed": []}
    record.update({key: True for key in ("contract_keys_match", "source_enabled", "nonce_unused",
                                       "balances_confirmed", "signing_recoverable", "quote_current",
                                       "allowance_sufficient")})
    record["validators"] = [
        {"validator_id": f"validator-{i}", **final, "route_state_hash": "d" * 64,
         "nav_state_hash": "e" * 64, "asset_state_hash": "f" * 64,
         "queues": {"mempool": 0, "reservations": 0, "egress": 0}} for i in range(6)]
    root = Path(config["paths"]["packet_dir"])
    root.mkdir(exist_ok=True)
    for previous in steps[:index]:
        name = previous.name + ".json"
        save(root, name, {"synthetic": True})
        record["completed"].append({
            "step": previous.name, "command_sha256": previous.command_sha256,
            "artifacts": [{"path": name, "sha256": hashlib.sha256((root / name).read_bytes()).hexdigest()}],
            "receipt": {"accepted": True, "finalized": True, "tx_id": previous.command_sha256}})
    return record


def test_dry_run_success_order_and_no_execution(tmp_path, capsys, monkeypatch):
    monkeypatch.setattr(subprocess, "run", lambda *a, **k: pytest.fail("dry-run executed a command"))
    config = inputs(tmp_path)
    result = compose.dry_run(config, signers())
    commands = capsys.readouterr().out
    names = [step.name for step in compose.compile_steps(config, signers())]
    ordered = ["preflight", "arc-deposit", "ingress-claim", "reserve", "subscribe",
               "entitlement-release", "nav-finalize", "route-advance", "redeem",
               "burn", "arc-release", "egress-settle", "final-convergence"]
    assert [names.index(name) for name in ordered] == sorted(names.index(name) for name in ordered)
    assert "arc-ingress-capture" in commands and "withdrawWithProof(bytes,bytes)" in commands
    assert "--settlement" not in next(step.argv for step in compose.compile_steps(config, signers())
                                    if step.name == "prepare-subscription")
    assert result["submissions"] == 0
    skeleton = Path(result["skeleton"])
    assert "not-opened" not in skeleton.read_text()
    assert z3.verify_manifest(skeleton)["verdict"] == "FAIL"
    with pytest.raises(z3.CycleError, match="overwrite"):
        compose.dry_run(config, signers())


@pytest.mark.parametrize("case", ["stale_proof", "wrong_route", "wrong_asset", "duplicate",
                                 "replay", "active_entitlement", "insufficient_capacity", "partial_artifact"])
def test_required_negative_packets(tmp_path, case):
    layout = packet_fixture(tmp_path)
    section, role = {
        "stale_proof": ("ingress", "proof_report"),
        "wrong_route": ("redemption", "record"),
        "wrong_asset": ("ingress", "record"),
        "duplicate": ("ingress", "claim_receipt"),
        "replay": ("egress", "replay"),
        "active_entitlement": ("entitlement_release", "record"),
        "insufficient_capacity": ("redemption", "quote"),
        "partial_artifact": ("egress", "record"),
    }[case]
    path = layout[section][role]
    data = json.loads((tmp_path / path).read_text())
    if case == "stale_proof":
        data["valid_until_height"] = 1
    elif case == "wrong_route":
        data["identities"]["route_id"] = "wrong-route"
    elif case == "wrong_asset":
        data["identities"]["settlement_source_asset_id"] = "f" * 96
    elif case == "duplicate":
        original = json.loads((tmp_path / layout["ingress"]["propose_receipt"]).read_text())
        data["tx_id"] = original["tx_id"]
    elif case == "replay":
        data["rejected"] = False
    elif case == "active_entitlement":
        data["state"]["entitlement_atoms"] = 1
    elif case == "insufficient_capacity":
        data.update(nav_amount_atoms=2000, base_value_atoms=2000,
                    settlement_output_atoms=1999, redemption_spread_atoms=1)
    else:
        del data["receipts"]["burn_receipt"]
    save(tmp_path, path, data)
    verdict = z3.verify_manifest(build(tmp_path, layout))
    assert verdict["verdict"] == "FAIL", verdict


@pytest.mark.parametrize("field,value", [
    ("proof_valid_until", 1), ("nonce_unused", False), ("entitlement_atoms", 1),
    ("available_capacity_atoms", 1004), ("allowance_sufficient", False),
])
def test_submission_preflight_negatives(tmp_path, field, value):
    config = inputs(tmp_path)
    steps = compose.compile_steps(config, signers())
    gate = checkpoint(config, steps, 1)
    gate[field] = value
    with pytest.raises(z3.CycleError):
        compose.check_checkpoint(config, gate, steps, 1)


def test_one_confirmation_runs_exactly_one_and_duplicate_stops(tmp_path, monkeypatch):
    config = inputs(tmp_path)
    steps = compose.compile_steps(config, signers())
    gate = checkpoint(config, steps, 1)
    calls = []
    monkeypatch.setattr(compose, "ensure_available", lambda *args: None)
    def run(argv, **kwargs):
        calls.append(argv)
        return SimpleNamespace(returncode=0)
    result = compose.confirm_one(config, gate, steps, "arc-deposit", tmp_path, run)
    assert result["executed_step"] == "arc-deposit"
    assert calls == [steps[1].argv]
    with pytest.raises(z3.CycleError, match="overwrite"):
        compose.confirm_one(config, gate, steps, "arc-deposit", tmp_path, run)
    assert len(calls) == 1


def test_absent_arc_command_on_main_fails_closed(tmp_path):
    config = inputs(tmp_path)
    steps = compose.compile_steps(config, signers())
    with pytest.raises(z3.CycleError, match="Arc commands absent"):
        compose.confirm_one(config, checkpoint(config, steps, 1), steps, "arc-deposit", tmp_path,
                            lambda *a, **k: pytest.fail("must not execute"))


def test_missing_predecessor_artifact_stops(tmp_path):
    config = inputs(tmp_path)
    steps = compose.compile_steps(config, signers())
    gate = checkpoint(config, steps, 1)
    gate["completed"][0]["artifacts"] = []
    with pytest.raises(z3.CycleError, match="partial artifact"):
        compose.check_checkpoint(config, gate, steps, 1)


def test_failed_command_retains_attempt_and_stops(tmp_path, monkeypatch):
    config = inputs(tmp_path)
    steps = compose.compile_steps(config, signers())
    gate = checkpoint(config, steps, 1)
    monkeypatch.setattr(compose, "ensure_available", lambda *args: None)
    with pytest.raises(z3.CycleError, match="step failed"):
        compose.confirm_one(config, gate, steps, "arc-deposit", tmp_path,
                            lambda *a, **k: SimpleNamespace(returncode=1))
    assert (Path(config["paths"]["work_dir"]) / "attempts/arc-deposit.json").is_file()


def test_short_pair_address_rejected(tmp_path):
    config = inputs(tmp_path)
    config["manifest"]["identities"]["source_vault_address"] = "0x160307"
    with pytest.raises(z3.CycleError, match="full address"):
        compose.compile_steps(config, signers())


def test_route_epoch_builder_uses_explicit_profile_and_remaining_capacity(tmp_path):
    # Dummy signer-path placeholder: the builder only checks existence.
    signer = tmp_path / "operator-placeholder.json"
    signer.touch()
    route = fixtures.route(paused=True, issue_capacity_remaining_atoms=10**9,
                           redeem_capacity_remaining_atoms=2 * 10**9, max_nav_age_blocks=256)
    nav = fixtures.nav(epoch=3)
    nav["prior_epoch"] = route["pricing_nav_epoch"]
    for name, value in (("route.json", route), ("nav.json", nav), ("identities.json", fixtures.IDENTITIES)):
        save(tmp_path, name, value)
    output = tmp_path / "built"
    result = subprocess.run([
        sys.executable, str(compose.REPO / "scripts/a666-build-route-epoch-advance.py"),
        "--route-status", str(tmp_path / "route.json"), "--nav-manifest", str(tmp_path / "nav.json"),
        "--identities", str(tmp_path / "identities.json"), "--operator", fixtures.SUBSCRIBER,
        "--issuer-key-file", str(signer), "--valid-from-height", "10", "--output-dir", str(output)
    ], capture_output=True, text=True)
    assert result.returncode == 0, result.stderr
    manifest = json.loads((output / "route-epoch-advance-manifest.json").read_text())
    assert manifest["next_policy"]["issue_capacity_atoms"] == 10**9
    assert manifest["next_policy"]["redeem_capacity_atoms"] == 2 * 10**9
