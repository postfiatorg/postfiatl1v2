#!/usr/bin/env python3
"""Produce a fail-closed, redacted acceptance report for the controlled pNOK demo."""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
from typing import Any
import urllib.request


PFUSDC_ASSET_ID = (
    "02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c2"
    "33f6830bd5221fe2717fb6a1a7005d7b"
)
PNOK_ASSET_ID = (
    "4b8dd69b1f1ee425ae84d17f2fc2fe3630d904c4c356b0ff2df0e00c20109e63"
    "46d765ca4c0407ddb08edcb6525d5cc1"
)
WNOK = "0xadc2E8B500B2605739E9f40d0951bD40F7135e3F"
SOURCE_RPC = "http://127.0.0.1:19545"
BASE_ATOMS = 20_000_000
QUOTE_ATOMS = 210
TARGET_RUNS = 10
FORBIDDEN_PUBLIC_KEYS = {
    "private_inputs",
    "wallet_address",
    "liquidity_wallet_address",
    "facility_key_file",
    "key_file",
    "master_seed",
    "seed",
    "note_opening",
    "note_commitment",
    "input_commitments",
    "output_commitments",
    "spending_key",
    "secret_key",
}


def load_json(path: Path) -> Any:
    return json.loads(path.read_text())


def command(argv: list[str], *, cwd: Path | None = None) -> str:
    result = subprocess.run(argv, cwd=cwd, text=True, capture_output=True)
    if result.returncode != 0:
        raise RuntimeError(f"command failed ({result.returncode}): {' '.join(argv)}: {result.stderr[:500]}")
    return result.stdout.strip()


def http_json(url: str) -> Any:
    with urllib.request.urlopen(url, timeout=10) as response:
        return json.load(response)


def atomic_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    fd, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        os.fchmod(fd, 0o600)
        with os.fdopen(fd, "w") as stream:
            json.dump(value, stream, indent=2, sort_keys=True)
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def walk_keys(value: Any, path: str = "$") -> list[str]:
    violations: list[str] = []
    if isinstance(value, dict):
        for key, child in value.items():
            if key.lower() in FORBIDDEN_PUBLIC_KEYS:
                violations.append(f"{path}.{key}")
            violations.extend(walk_keys(child, f"{path}.{key}"))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            violations.extend(walk_keys(child, f"{path}[{index}]"))
    return violations


def accepted_job(status: dict[str, Any], direction: str) -> bool:
    return (
        status.get("ok") is True
        and status.get("schema") == "postfiat-pnok-private-fix-wallet-job-status-v1"
        and status.get("direction") == direction
        and status.get("status") == "accepted"
        and status.get("execution_stage") == "complete"
        and status.get("execution_privacy") == "private on PFTL"
        and status.get("source_boundary") == "controlled sandbox checkpoint"
        and status.get("base_atoms") == str(BASE_ATOMS)
        and status.get("quote_atoms") == str(QUOTE_ATOMS)
        and status.get("fee_atoms") == "0"
        and status.get("price_impact_bps") == 0
        and status.get("supply_unchanged") is True
        and status.get("nullifier_occurrence_counts") == [1, 1]
        and status.get("output_occurrence_counts") == [1, 1]
        and (direction != "acquire" or status.get("replay_rejected_without_effect") is True)
    )


def exact_note(notes: list[Any], owner: str, asset_id: str, amount_atoms: int) -> bool:
    matches = [
        note
        for note in notes
        if isinstance(note, dict)
        and note.get("wallet_address") == owner
        and note.get("asset_id") == asset_id
        and int(note.get("amount_atoms", -1)) == amount_atoms
        and note.get("state") == "spendable"
    ]
    return len(matches) == 1


def load_demo_module(script_dir: Path) -> Any:
    source = script_dir / "pnok-private-fix-demo.py"
    spec = importlib.util.spec_from_file_location("pnok_acceptance_demo_support", source)
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot import the pNOK private FIX driver")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def parse_args() -> argparse.Namespace:
    repo = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, default=repo)
    parser.add_argument("--source-repo", type=Path, default=repo.parent / "cbdc-tokenization-sandbox")
    parser.add_argument(
        "--campaign-report",
        type=Path,
        default=repo / "deployments/pnok-private-fix-20260801/browser-qualification-10x/report.json",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=repo / "deployments/pnok-private-fix-20260801/acceptance/public/report.json",
    )
    parser.add_argument("--ports", default="28650,28651,28652,28653,28654,28655")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    repo = args.repo.resolve()
    source_repo = args.source_repo.resolve()
    campaign_path = args.campaign_report.resolve()
    checks: dict[str, bool] = {}

    launch = load_json(
        repo / "deployments/pnok-controlled-demo-20260801/source-besu/evidence/launch-inputs.public.json"
    )
    upstream = command(["git", "rev-parse", "origin/development"], cwd=source_repo)
    merge_base = command(["git", "merge-base", "HEAD", "origin/development"], cwd=source_repo)
    checks["upstream_wnok_and_besu_revision_pinned"] = (
        upstream == launch.get("upstream_revision") == merge_base
        and launch.get("image") == "hyperledger/besu:26.7.0"
        and str(launch.get("image_digest", "")).startswith("sha256:")
        and command(["cast", "chain-id", "--rpc-url", SOURCE_RPC]) == "2018"
    )

    bridge_status = load_json(
        repo
        / "deployments/pnok-private-fix-20260801/source-live-route-v2/deposit-500/verification/vault-bridge-status.json"
    )
    relay = load_json(
        repo / "deployments/pnok-private-fix-20260801/source-live-route-v2/deposit-500/relay.report.json"
    )
    plan = relay["relay_bundle"]["plan"]
    vault = "0x" + plan["evidence"]["vault_address"].removeprefix("0x")
    route_binding = "0x" + plan["evidence"]["route_binding"].removeprefix("0x")
    minter_role = command(["cast", "keccak", "MINTER_ROLE"])
    burner_role = command(["cast", "keccak", "BURNER_ROLE"])
    transfer_role = command(["cast", "keccak", "TRANSFER_FROM_ROLE"])
    role = lambda value: command(
        ["cast", "call", "--rpc-url", SOURCE_RPC, WNOK, "hasRole(bytes32,address)(bool)", value, vault]
    )
    allowlisted = command(
        ["cast", "call", "--rpc-url", SOURCE_RPC, WNOK, "allowlistQuery(address)(bool)", vault]
    )
    vault_balance = int(
        command(["cast", "call", "--rpc-url", SOURCE_RPC, WNOK, "balanceOf(address)(uint256)", vault])
    )
    verifier = command(
        ["cast", "call", "--rpc-url", SOURCE_RPC, vault, "releaseVerifier()(address)"]
    )
    checks["vault_is_allowlisted_and_least_privilege"] = (
        allowlisted == "true"
        and role(minter_role) == "false"
        and role(burner_role) == "false"
        and role(transfer_role) == "true"
        and command(["cast", "call", "--rpc-url", SOURCE_RPC, vault, "wnok()(address)"]).lower()
        == WNOK.lower()
        and command(["cast", "call", "--rpc-url", SOURCE_RPC, vault, "routeBinding()(bytes32)"]).lower()
        == route_binding.lower()
        and command(["cast", "call", "--rpc-url", SOURCE_RPC, vault, "paused()(bool)"]) == "false"
    )
    checks["controlled_redemption_path_is_bound_and_forbids_live_value"] = (
        command(["cast", "call", "--rpc-url", SOURCE_RPC, verifier, "boundVault()(address)"]).lower()
        == vault.lower()
        and command(
            ["cast", "call", "--rpc-url", SOURCE_RPC, verifier, "routeBinding()(bytes32)"]
        ).lower()
        == route_binding.lower()
        and command(
            ["cast", "call", "--rpc-url", SOURCE_RPC, verifier, "LIVE_VALUE_ENABLED()(bool)"]
        )
        == "false"
        and command(
            ["cast", "call", "--rpc-url", SOURCE_RPC, verifier, "ROUTE_TRUST_CLASS()(bytes32)"]
        ).lower()
        == command(["cast", "keccak", "CONTROLLED"]).lower()
    )

    bucket = bridge_status["buckets"][0]
    receipt = bridge_status["receipts"][0]
    allocation = bridge_status["allocations"][0]
    checks["five_hundred_wnok_created_five_hundred_pnok_once"] = (
        plan["evidence"]["amount_atoms"] == 500
        and vault_balance == 500
        and bridge_status.get("issued_supply_atoms") == 500
        and bridge_status.get("receipt_count") == 1
        and bridge_status.get("bridge_deposit_count") == 1
        and receipt.get("amount_atoms") == 500
        and receipt.get("status") == "counted"
    )
    checks["pnok_supply_exactly_matches_bridge_accounting"] = (
        bridge_status.get("asset_id") == PNOK_ASSET_ID
        and bridge_status.get("issued_supply_atoms") == 500
        and bridge_status.get("counted_value_atoms") == 500
        and bucket.get("gross_receipt_atoms") == 500
        and bucket.get("outstanding_vault_bridge_atoms") == 500
        and allocation.get("amount_atoms") == 500
        and allocation.get("remaining_atoms") == 500
        and bridge_status.get("redemption_count") == 0
    )

    campaign = load_json(campaign_path)
    runs = campaign.get("runs", [])
    acquisition_jobs = [run.get("acquire", {}) for run in runs]
    reset_jobs = [run.get("reset", {}) for run in runs[1:]]
    all_jobs = acquisition_jobs + reset_jobs
    checks["ten_consecutive_browser_demos_without_manual_repair"] = (
        campaign.get("ok") is True
        and campaign.get("completed_runs") == TARGET_RUNS
        and campaign.get("consecutive_without_manual_state_repair") is True
        and campaign.get("each_acquisition_browser_initiated") is True
        and campaign.get("refresh_recovery_exercised_each_run") is True
        and len(runs) == TARGET_RUNS
        and [run.get("run_index") for run in runs] == list(range(1, TARGET_RUNS + 1))
        and all(accepted_job(status, "acquire") for status in acquisition_jobs)
        and all(accepted_job(status, "restore") for status in reset_jobs)
        and len({status.get("job_id") for status in all_jobs}) == 19
        and campaign.get("browser_errors") == []
    )
    checks["private_swap_supply_nullifier_output_and_replay_invariants"] = all(
        status.get("supply_unchanged") is True
        and status.get("nullifier_occurrence_counts") == [1, 1]
        and status.get("output_occurrence_counts") == [1, 1]
        and status.get("replay_rejected_without_effect") is True
        for status in acquisition_jobs
    )
    checks["wallet_ux_uses_exact_privacy_and_trust_labels"] = all(
        status.get("execution_privacy") == "private on PFTL"
        and status.get("source_boundary") == "controlled sandbox checkpoint"
        for status in acquisition_jobs
    )

    epoch_three = load_json(
        repo / "deployments/pnok-private-fix-20260801/repeat-fix-epoch-3/public/fix-packet.json"
    )
    packet_hash = epoch_three["packet_hash"]
    checks["finalized_fix_is_exact_zero_fee_and_bounded"] = (
        epoch_three.get("epoch") == 3
        and epoch_three.get("base_asset_id") == PFUSDC_ASSET_ID
        and epoch_three.get("quote_asset_id") == PNOK_ASSET_ID
        and epoch_three.get("ratio_numerator") == 21
        and epoch_three.get("ratio_denominator") == 2_000_000
        and epoch_three.get("band_bps") == 0
        and epoch_three.get("fee_bps") == 0
        and epoch_three.get("minimum_base_atoms") == BASE_ATOMS
        and epoch_three.get("capacity_base_atoms") == BASE_ATOMS * 19
        and epoch_three.get("capacity_quote_atoms") == QUOTE_ATOMS * 19
        and epoch_three.get("max_fills") == 19
        and all(status.get("fix_packet_hash") == packet_hash for status in all_jobs)
    )

    demo = load_demo_module(repo / "scripts")
    rpc = demo.load_rpc_helpers(repo / "scripts")
    ports = [int(value) for value in args.ports.split(",")]
    rows = rpc.wait_for_fleet_status(ports, 45.0, 60.0)
    checks["six_validator_fleet_converged"] = (
        len(rows) == 6
        and len({row["block_height"] for row in rows}) == 1
        and len({row["state_root"] for row in rows}) == 1
    )
    fix_info = demo.fleet_rpc_identical(
        rpc, ports, "fx_fix_info", {"fix_packet_hash": packet_hash}, 45.0
    )["fix"]
    checks["fix_capacity_exhausted_exactly_after_nineteen_private_fills"] = (
        fix_info.get("status") == "filled"
        and fix_info.get("remaining_fill_slots") == 0
        and fix_info.get("state", {}).get("fill_count") == 19
        and fix_info.get("committed_base_atoms") == BASE_ATOMS * 19
        and fix_info.get("committed_quote_atoms") == QUOTE_ATOMS * 19
    )

    service_config = load_json(
        repo / "deployments/pnok-private-fix-20260801/wallet-service-config.json"
    )
    note_response = http_json("http://127.0.0.1:18799/asset-orchard/notes")
    notes = note_response.get("notes", [])
    checks["bob_controls_exact_final_pnok_output"] = exact_note(
        notes, service_config["demo_wallet"], PNOK_ASSET_ID, QUOTE_ATOMS
    )
    checks["facility_controls_exact_final_pfusdc_output"] = exact_note(
        notes, service_config["fix_operator"], PFUSDC_ASSET_ID, BASE_ATOMS
    )
    final_readiness = campaign.get("final_readiness", {})
    checks["final_inventory_matches_tenth_acquisition"] = (
        final_readiness.get("acquire_inventory_ready") is False
        and final_readiness.get("restore_inventory_ready") is True
    )

    public_files = [
        repo / "deployments/pnok-private-fix-20260801/browser-run-01/report.json",
        campaign_path,
        repo / "deployments/pnok-private-fix-20260801/repeat-fix-epoch-3/public/fix-packet.json",
        repo / "deployments/pnok-private-fix-20260801/repeat-fix-epoch-3/public/status.json",
    ]
    privacy_violations = {
        str(path.relative_to(repo)): walk_keys(load_json(path))
        for path in public_files
        if walk_keys(load_json(path))
    }
    tracked_private = command(["git", "ls-files"], cwd=repo).splitlines()
    checks["public_evidence_excludes_private_custody_material"] = (
        not privacy_violations
        and not any(
            path.startswith("deployments/pnok-")
            and any(token in path.lower() for token in ("master-seed", "wallet-key", "orchard-key", "/private/"))
            for path in tracked_private
        )
    )

    checks["controlled_demo_is_not_mislabeled_tier4"] = (
        launch.get("route_trust_class") == "CONTROLLED"
        and launch.get("live_value_enabled") is False
        and all(status.get("source_boundary") == "controlled sandbox checkpoint" for status in all_jobs)
    )

    report = {
        "schema": "postfiat-pnok-first-demo-acceptance-report-v1",
        "ok": all(checks.values()),
        "scope": "controlled pNOK/private-FX demo; not Tier 4 and not production",
        "upstream_revision": upstream,
        "source_bridge_commit": command(["git", "rev-parse", "HEAD"], cwd=source_repo),
        "postfiat_commit": command(["git", "rev-parse", "HEAD"], cwd=repo),
        "pnok_asset_id": PNOK_ASSET_ID,
        "fix_packet_hash": packet_hash,
        "browser_runs": len(runs),
        "private_inverse_resets": len(reset_jobs),
        "checks": checks,
        "failed_checks": sorted(name for name, passed in checks.items() if not passed),
        "generated_at_unix_ms": int(time.time() * 1000),
    }
    atomic_json(args.output.resolve(), report)
    print(json.dumps(report, indent=2, sort_keys=True))
    if not report["ok"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
