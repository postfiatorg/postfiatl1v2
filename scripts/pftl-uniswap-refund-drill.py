#!/usr/bin/env python3
"""Exercise the PFTL-Uniswap refund path against the node transition CLI."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import shutil
import subprocess
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
GATE3_EXECUTOR = ROOT / "scripts" / "pftl-uniswap-gate3-fork-execute.py"
DEFAULT_OUT_DIR = ROOT / "docs" / "evidence" / "pftl-uniswap-wallet-e2e-2026-07-02" / "orchardmanager-0247-refund-01"


def env_str(name: str, default: str) -> str:
    return os.environ.get(name, default)


def env_int(name: str, default: int) -> int:
    value = os.environ.get(name)
    return default if value is None else int(value, 0)


def load_gate3_module():
    spec = importlib.util.spec_from_file_location("pftl_uniswap_gate3_executor", GATE3_EXECUTOR)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load Gate 3 executor from {GATE3_EXECUTOR}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


gate3 = load_gate3_module()

ROUTE_ID = env_str("PFTL_UNISWAP_REFUND_ROUTE_ID", "pftl-a651-usdc-wallet-e2e-20260702-refund-v1")
PACKET_HASH = env_str("PFTL_UNISWAP_REFUND_PACKET_HASH", "c1" * 48)
EXPORT_NONCE = env_str("PFTL_UNISWAP_REFUND_EXPORT_NONCE", "c2" * 32)
SOURCE_HEIGHT = env_int("PFTL_UNISWAP_REFUND_SOURCE_HEIGHT", 10)
REFUND_NOT_BEFORE_HEIGHT = env_int("PFTL_UNISWAP_REFUND_NOT_BEFORE_HEIGHT", 20)
REFUND_CURRENT_HEIGHT = env_int("PFTL_UNISWAP_REFUND_CURRENT_HEIGHT", REFUND_NOT_BEFORE_HEIGHT)
REFUND_AMOUNT_ATOMS = env_int("PFTL_UNISWAP_REFUND_AMOUNT_ATOMS", 10)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out-dir", default=str(DEFAULT_OUT_DIR), help="evidence output directory")
    parser.add_argument("--replace", action="store_true", help="replace an existing evidence directory")
    return parser.parse_args()


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=False) + "\n", encoding="utf-8")


def node_json(cmd: list[str], output_file: Path) -> dict[str, Any]:
    return gate3.node_json(cmd, output_file)


def run_node_expect_failure(cmd: list[str], output_file: Path) -> dict[str, Any]:
    result = subprocess.run(cmd, cwd=ROOT, text=True, capture_output=True, check=False)
    report = {
        "schema": "postfiat-command-failure-evidence-v1",
        "cmd": cmd,
        "returncode": result.returncode,
        "stdout": result.stdout,
        "stderr": result.stderr,
    }
    write_json(output_file, report)
    if result.returncode == 0:
        raise RuntimeError(f"command unexpectedly succeeded: {' '.join(cmd)}")
    return report


def non_consumption_proof_hash(route_id: str, packet_hash: str, refund_not_before_height: int) -> str:
    preimage = (
        f"route_id={route_id}\n"
        f"packet_hash={packet_hash}\n"
        f"refund_not_before_height={refund_not_before_height}\n"
    )
    hasher = hashlib.sha3_384()
    hasher.update(b"postfiat.pftl_uniswap.non_consumption_commitment.v1")
    hasher.update(b"\0")
    hasher.update(preimage.encode("utf-8"))
    return hasher.hexdigest()


def main() -> int:
    args = parse_args()
    out_dir = Path(args.out_dir).resolve()
    if out_dir.exists():
        if not args.replace:
            raise RuntimeError(f"evidence directory already exists: {out_dir}; pass --replace")
        shutil.rmtree(out_dir)
    inputs_dir = out_dir / "inputs"
    reports_dir = out_dir / "reports"
    sidecar_dir = out_dir / "sidecar"
    inputs_dir.mkdir(parents=True)
    reports_dir.mkdir(parents=True)
    sidecar_dir.mkdir(parents=True)

    gate3.run(
        ["cargo", "build", "-p", "postfiat-node"],
        stdout_file=reports_dir / "00-cargo-build-postfiat-node.txt",
    )

    route_config = {
        "schema": "postfiat-pftl-uniswap-route-config-v1",
        "route_id": ROUTE_ID,
        "route_family": "primary_pftl_mint",
        "native_nav_asset_id": gate3.NATIVE_NAV_ASSET_ID,
        "settlement_asset_id": gate3.SETTLEMENT_ASSET_ID,
        "wrapped_navcoin_token": "0xd969897adeb947a22e9621db2db186e6ea11140f",
        "handoff_controller": "0xdd3f8638b7faeff0fa61d0596917cf788436d495",
        "settlement_adapter": "0x9812525c76ca09f30410c0402abc612abc3bcadc",
        "verifier_mode": gate3.VERIFIER_MODE,
        "route_trust_class": gate3.TRUST_CLASS,
        "uniswap_pool_id_or_path": "0x5c7ea7b5e0091029297604a5908e13ee671b937917c96bc62e940796a269443d",
        "router": "0x839cbf32175fc81712dc2c59617c0cd2b5d421cf",
        "failure_behavior": "refund_unconsumed_pftl_packet",
        "route_supply_cap_atoms": gate3.ROUTE_SUPPLY_CAP_ATOMS,
        "packet_notional_cap_atoms": gate3.PACKET_NOTIONAL_CAP_ATOMS,
        "seed_nav_epoch": gate3.LATEST_FINALIZED_NAV_EPOCH,
        "seed_usdc_atoms": gate3.SEED_USDC_ATOMS,
        "seed_wrapped_navcoin_atoms": REFUND_AMOUNT_ATOMS,
        "lp_recipient": "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
        "lp_custody_policy": "controlled_refund_drill_no_lp",
    }
    route_config_file = inputs_dir / "route-config.json"
    write_json(route_config_file, route_config)

    route_init = node_json(
        [
            str(gate3.NODE_BIN),
            "navcoin-bridge-route-init",
            "--data-dir",
            str(sidecar_dir),
            "--config-file",
            str(route_config_file),
            "--ethereum-chain-id",
            "1",
            "--latest-finalized-nav-epoch",
            str(gate3.LATEST_FINALIZED_NAV_EPOCH),
            "--return-finality-blocks",
            str(gate3.RETURN_FINALITY_BLOCKS),
            "--replace",
        ],
        reports_dir / "01-route-init.json",
    )

    primary_request = {
        "route_id": ROUTE_ID,
        "source_wallet": gate3.SOURCE_WALLET,
        "settlement_asset_id": gate3.SETTLEMENT_ASSET_ID,
        "subscription_nonce": "c3" * 32,
        "quote": {
            "settlement_value_atoms": REFUND_AMOUNT_ATOMS * gate3.NAV_PRICE_SETTLEMENT_ATOMS_PER_NAV_ATOM,
            "nav_price_settlement_atoms_per_nav_atom": gate3.NAV_PRICE_SETTLEMENT_ATOMS_PER_NAV_ATOM,
            "pricing_nav_epoch": gate3.LATEST_FINALIZED_NAV_EPOCH,
            "pricing_reserve_packet_hash": gate3.PRICING_RESERVE_PACKET_HASH,
        },
    }
    primary_file = inputs_dir / "primary-subscription.json"
    write_json(primary_file, primary_request)
    primary_report = node_json(
        [
            str(gate3.NODE_BIN),
            "navcoin-bridge-primary-subscribe",
            "--data-dir",
            str(sidecar_dir),
            "--request-file",
            str(primary_file),
        ],
        reports_dir / "02-primary-subscription.json",
    )

    export_request = {
        "route_id": ROUTE_ID,
        "packet_hash": PACKET_HASH,
        "nonce": EXPORT_NONCE,
        "source_wallet": gate3.SOURCE_WALLET,
        "ethereum_recipient": "0x70997970c51812dc3a010c7d01b50e0d17dc79c8",
        "amount_atoms": REFUND_AMOUNT_ATOMS,
        "source_height": SOURCE_HEIGHT,
        "destination_deadline_seconds": gate3.DESTINATION_DEADLINE_SECONDS,
        "refund_not_before_height": REFUND_NOT_BEFORE_HEIGHT,
    }
    export_file = inputs_dir / "export-refund-target.json"
    write_json(export_file, export_request)
    export_report = node_json(
        [
            str(gate3.NODE_BIN),
            "navcoin-bridge-export-debit",
            "--data-dir",
            str(sidecar_dir),
            "--request-file",
            str(export_file),
        ],
        reports_dir / "03-export-refund-target.json",
    )
    supply_after_export = node_json(
        [str(gate3.NODE_BIN), "navcoin-bridge-supply-status", "--data-dir", str(sidecar_dir), "--route-id", ROUTE_ID],
        reports_dir / "04-supply-after-export.json",
    )

    proof_hash = non_consumption_proof_hash(ROUTE_ID, PACKET_HASH, REFUND_NOT_BEFORE_HEIGHT)
    proof_report = {
        "schema": "postfiat-pftl-uniswap-non-consumption-proof-hash-evidence-v1",
        "route_id": ROUTE_ID,
        "packet_hash": PACKET_HASH,
        "refund_not_before_height": REFUND_NOT_BEFORE_HEIGHT,
        "domain": "postfiat.pftl_uniswap.non_consumption_commitment.v1",
        "preimage": (
            f"route_id={ROUTE_ID}\n"
            f"packet_hash={PACKET_HASH}\n"
            f"refund_not_before_height={REFUND_NOT_BEFORE_HEIGHT}\n"
        ),
        "non_consumption_proof_hash": proof_hash,
    }
    write_json(reports_dir / "05-non-consumption-proof-hash.json", proof_report)

    early_refund_request = {
        "packet_hash": PACKET_HASH,
        "current_height": REFUND_NOT_BEFORE_HEIGHT - 1,
        "non_consumption_proof_hash": proof_hash,
    }
    early_refund_file = inputs_dir / "refund-too-early.json"
    write_json(early_refund_file, early_refund_request)
    early_refund_failure = run_node_expect_failure(
        [
            str(gate3.NODE_BIN),
            "navcoin-bridge-refund-source",
            "--data-dir",
            str(sidecar_dir),
            "--route-id",
            ROUTE_ID,
            "--request-file",
            str(early_refund_file),
        ],
        reports_dir / "06-refund-too-early-rejected.json",
    )

    refund_request = {
        "packet_hash": PACKET_HASH,
        "current_height": REFUND_CURRENT_HEIGHT,
        "non_consumption_proof_hash": proof_hash,
    }
    refund_file = inputs_dir / "refund-source.json"
    write_json(refund_file, refund_request)
    refund_report = node_json(
        [
            str(gate3.NODE_BIN),
            "navcoin-bridge-refund-source",
            "--data-dir",
            str(sidecar_dir),
            "--route-id",
            ROUTE_ID,
            "--request-file",
            str(refund_file),
        ],
        reports_dir / "07-refund-source.json",
    )
    supply_after_refund = node_json(
        [str(gate3.NODE_BIN), "navcoin-bridge-supply-status", "--data-dir", str(sidecar_dir), "--route-id", ROUTE_ID],
        reports_dir / "08-supply-after-refund.json",
    )
    replay_after_refund = node_json(
        [str(gate3.NODE_BIN), "navcoin-bridge-receipt-replay", "--data-dir", str(sidecar_dir), "--route-id", ROUTE_ID],
        reports_dir / "09-receipt-replay-after-refund.json",
    )
    consume_after_refund_failure = run_node_expect_failure(
        [
            str(gate3.NODE_BIN),
            "navcoin-bridge-destination-consume",
            "--data-dir",
            str(sidecar_dir),
            "--route-id",
            ROUTE_ID,
            "--packet-hash",
            PACKET_HASH,
        ],
        reports_dir / "10-consume-after-refund-rejected.json",
    )

    checks = {
        "early_refund_rejected": "refund_before_window" in early_refund_failure["stderr"],
        "refund_status_source_refunded": refund_report["result"]["status"] == "SourceRefunded",
        "refund_receipt_commits_proof": refund_report["receipt"]["non_consumption_proof_hash"] == proof_hash,
        "spendable_restored": supply_after_refund["pftl_spendable_supply_atoms"] == REFUND_AMOUNT_ATOMS,
        "outstanding_claims_zero": supply_after_refund["outstanding_bridge_claims_atoms"] == 0,
        "ethereum_spendable_zero": supply_after_refund["ethereum_spendable_supply_atoms"] == 0,
        "invariant_holds": supply_after_refund["invariant_holds"] is True,
        "replay_verified": replay_after_refund["status"] == "verified",
        "consume_after_refund_rejected": "export_packet_not_settleable" in consume_after_refund_failure["stderr"],
    }
    failed = [name for name, ok in checks.items() if not ok]
    if failed:
        raise RuntimeError(f"refund drill failed checks: {failed}")

    summary = {
        "schema": "postfiat-pftl-uniswap-refund-drill-summary-v1",
        "route_id": ROUTE_ID,
        "route_config_digest": route_init["route_config_digest"],
        "packet_hash": PACKET_HASH,
        "export_receipt_hash": export_report["receipt_hash"],
        "refund_receipt_hash": refund_report["receipt_hash"],
        "non_consumption_proof_hash": proof_hash,
        "primary_report": primary_report,
        "supply_after_export": supply_after_export,
        "supply_after_refund": supply_after_refund,
        "receipt_replay": replay_after_refund,
        "checks": checks,
    }
    write_json(reports_dir / "11-summary.json", summary)
    print(json.dumps(summary, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
