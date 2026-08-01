#!/usr/bin/env python3
"""Register a durable successor pNOK demo FIX for repeatable private swaps."""

from __future__ import annotations

import argparse
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
from typing import Any


PFUSDC_ASSET_ID = (
    "02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c2"
    "33f6830bd5221fe2717fb6a1a7005d7b"
)
PNOK_ASSET_ID = (
    "4b8dd69b1f1ee425ae84d17f2fc2fe3630d904c4c356b0ff2df0e00c20109e63"
    "46d765ca4c0407ddb08edcb6525d5cc1"
)
OPERATOR = "pfb8f250e27a4ff89bf81da0f4a4bdd63f8ba45c12"
BASE_ATOMS = 20_000_000
QUOTE_ATOMS = 210
RATIO_NUMERATOR = 21
RATIO_DENOMINATOR = 2_000_000
SOURCE_LABEL = "pnok_demo_fix"
POLICY_HASH_DOMAIN = "postfiat.pnok_demo_fix.governance_policy_hash.v1"
PACKET_HASH_DOMAIN = "postfiat.fx_fix.packet_hash.v1"


def load_demo_module(script_dir: Path) -> Any:
    path = script_dir / "pnok-private-fix-demo.py"
    spec = importlib.util.spec_from_file_location("pnok_private_fix_demo_support", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import pNOK demo support from {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def packet_hash(packet: dict[str, Any], demo: Any) -> str:
    previous_fix_hash = packet.get("previous_fix_hash") or "none"
    preimage = (
        f"version={packet['version']}\n"
        f"schema={packet['schema']}\n"
        f"operator={packet['operator']}\n"
        f"base_asset_id={packet['base_asset_id']}\n"
        f"quote_asset_id={packet['quote_asset_id']}\n"
        f"epoch={packet['epoch']}\n"
        f"ratio_numerator={packet['ratio_numerator']}\n"
        f"ratio_denominator={packet['ratio_denominator']}\n"
        f"band_bps={packet['band_bps']}\n"
        f"fee_bps={packet['fee_bps']}\n"
        f"valid_from_height={packet['valid_from_height']}\n"
        f"expires_at_height={packet['expires_at_height']}\n"
        f"minimum_base_atoms={packet['minimum_base_atoms']}\n"
        f"capacity_base_atoms={packet['capacity_base_atoms']}\n"
        f"capacity_quote_atoms={packet['capacity_quote_atoms']}\n"
        f"max_fills={packet['max_fills']}\n"
        f"source_label={packet['source_label']}\n"
        f"source_observation_commitment={packet['source_observation_commitment']}\n"
        f"governance_policy_hash={packet['governance_policy_hash']}\n"
        f"previous_fix_hash={previous_fix_hash}\n"
    )
    return demo.domain_hash_384(PACKET_HASH_DOMAIN, preimage.encode())


def fleet_state(demo: Any, args: argparse.Namespace) -> tuple[Any, list[int], int]:
    rpc = demo.load_rpc_helpers(Path(__file__).resolve().parent)
    ports = [int(value) for value in args.ports.split(",")]
    if len(ports) != 6 or len(set(ports)) != 6:
        raise RuntimeError("exactly six distinct PFTL RPC ports are required")
    rows = rpc.wait_for_fleet_status(
        ports, args.rpc_timeout_seconds, args.convergence_timeout_seconds
    )
    heights = {int(row["block_height"]) for row in rows}
    roots = {row["state_root"] for row in rows}
    if len(heights) != 1 or len(roots) != 1:
        raise RuntimeError("validator fleet is not at one finalized height and root")
    return rpc, ports, heights.pop()


def predecessor(demo: Any, rpc: Any, ports: list[int], timeout: float) -> dict[str, Any]:
    listing = demo.fleet_rpc_identical(
        rpc,
        ports,
        "fx_fix_list",
        {
            "base_asset_id": PFUSDC_ASSET_ID,
            "quote_asset_id": PNOK_ASSET_ID,
            "active_only": False,
            "limit": 128,
        },
        timeout,
    )
    rows = [
        row
        for row in listing.get("fixes", [])
        if row.get("state", {}).get("packet", {}).get("operator") == OPERATOR
    ]
    if not rows:
        raise RuntimeError("no predecessor pNOK FIX exists for the configured operator and pair")
    rows.sort(key=lambda row: int(row["state"]["packet"]["epoch"]))
    latest = rows[-1]["state"]["packet"]
    expected_epoch = list(range(1, int(latest["epoch"]) + 1))
    observed_epoch = [int(row["state"]["packet"]["epoch"]) for row in rows]
    if observed_epoch != expected_epoch:
        raise RuntimeError("pNOK FIX epoch lineage is not contiguous")
    return latest


def build_packet(
    demo: Any,
    prior: dict[str, Any],
    current_height: int,
    max_fills: int,
    validity_blocks: int,
) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    if not 1 <= max_fills <= 1_024:
        raise RuntimeError("max_fills must be in 1..=1024")
    if validity_blocks < 100:
        raise RuntimeError("validity_blocks must be at least 100")
    policy = {
        "schema": "postfiat.pnok_demo_fix.governance_policy.v1",
        "trust_class": "CONTROLLED_DEMO",
        "private_execution_on_pftl": True,
        "private_end_to_end": False,
        "operator": OPERATOR,
        "base_asset_id": PFUSDC_ASSET_ID,
        "quote_asset_id": PNOK_ASSET_ID,
        "band_bps": 0,
        "fee_bps": 0,
        "minimum_base_atoms": BASE_ATOMS,
        # These capacities are the exact maximum for each fill. Consensus also
        # bounds aggregate execution through max_fills and private nullifiers.
        "capacity_base_atoms": BASE_ATOMS,
        "capacity_quote_atoms": QUOTE_ATOMS,
        "max_fills": max_fills,
    }
    policy_hash = demo.domain_hash_384(
        POLICY_HASH_DOMAIN, demo.canonical_json(policy).encode()
    )
    packet = {
        "version": 1,
        "schema": "postfiat.fx_fix.packet.v1",
        "operator": OPERATOR,
        "base_asset_id": PFUSDC_ASSET_ID,
        "quote_asset_id": PNOK_ASSET_ID,
        "epoch": int(prior["epoch"]) + 1,
        "ratio_numerator": RATIO_NUMERATOR,
        "ratio_denominator": RATIO_DENOMINATOR,
        "band_bps": 0,
        "fee_bps": 0,
        "valid_from_height": current_height + 1,
        "expires_at_height": current_height + validity_blocks,
        "minimum_base_atoms": BASE_ATOMS,
        "capacity_base_atoms": BASE_ATOMS,
        "capacity_quote_atoms": QUOTE_ATOMS,
        "max_fills": max_fills,
        "source_label": SOURCE_LABEL,
        "source_observation_commitment": prior["source_observation_commitment"],
        "governance_policy_hash": policy_hash,
        "previous_fix_hash": prior["packet_hash"],
    }
    packet["packet_hash"] = packet_hash(packet, demo)
    commitments = {
        "schema": "postfiat.pnok_demo_fix.commitments.v1",
        "governance_policy": {
            "domain": POLICY_HASH_DOMAIN,
            "encoding": "canonical-json-sorted-compact-utf8",
            "hash": policy_hash,
        },
        "packet": {
            "domain": PACKET_HASH_DOMAIN,
            "encoding": "canonical-field-lines-utf8",
            "hash": packet["packet_hash"],
        },
    }
    return packet, policy, commitments


def verify_registered(
    demo: Any,
    rpc: Any,
    ports: list[int],
    packet: dict[str, Any],
    timeout: float,
) -> dict[str, Any] | None:
    report = demo.fleet_rpc_identical(
        rpc,
        ports,
        "fx_fix_info",
        {"fix_packet_hash": packet["packet_hash"]},
        timeout,
    )
    if report.get("found") is not True:
        return None
    observed = report.get("fix", {}).get("state", {}).get("packet")
    if observed != packet:
        raise RuntimeError("registered successor FIX differs from the immutable packet")
    return report


def validate_packet(packet: dict[str, Any], demo: Any, max_fills: int) -> None:
    expected = {
        "version": 1,
        "schema": "postfiat.fx_fix.packet.v1",
        "operator": OPERATOR,
        "base_asset_id": PFUSDC_ASSET_ID,
        "quote_asset_id": PNOK_ASSET_ID,
        "ratio_numerator": RATIO_NUMERATOR,
        "ratio_denominator": RATIO_DENOMINATOR,
        "band_bps": 0,
        "fee_bps": 0,
        "minimum_base_atoms": BASE_ATOMS,
        "capacity_base_atoms": BASE_ATOMS,
        "capacity_quote_atoms": QUOTE_ATOMS,
        "max_fills": max_fills,
        "source_label": SOURCE_LABEL,
    }
    for field, value in expected.items():
        if packet.get(field) != value:
            raise RuntimeError(f"successor FIX {field} does not match immutable configuration")
    if int(packet.get("epoch", 0)) <= 1 or not packet.get("previous_fix_hash"):
        raise RuntimeError("successor FIX does not bind a prior epoch")
    if packet.get("packet_hash") != packet_hash(packet, demo):
        raise RuntimeError("successor FIX packet hash is not canonical")


def parse_args() -> argparse.Namespace:
    repo = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--max-fills", type=int, default=19)
    parser.add_argument("--validity-blocks", type=int, default=2_000)
    parser.add_argument(
        "--operator-key-file",
        type=Path,
        default=repo / "deployments/pnok-controlled-demo-20260801/private/facility.wallet-key.json",
    )
    parser.add_argument("--ports", default="28650,28651,28652,28653,28654,28655")
    parser.add_argument("--rpc-timeout-seconds", type=float, default=45.0)
    parser.add_argument("--convergence-timeout-seconds", type=float, default=60.0)
    parser.add_argument("--node-bin", type=Path, default=repo / "target/release/postfiat-node")
    parser.add_argument(
        "--remote-finality-script",
        type=Path,
        default=repo / "scripts/a666-ce22-remote-finality-op.py",
    )
    parser.add_argument(
        "--remote-runner",
        type=Path,
        default=repo / "scripts/a666-remote-sync-round.py",
    )
    parser.add_argument(
        "--proposer-hosts-file",
        type=Path,
        default=repo / "docs/evidence/a666-joe-mainnet-e2e-20260728/proposer-hosts.json",
    )
    parser.add_argument(
        "--remote-binary",
        default="/opt/postfiat/releases/pnok-private-fix-2246d25/postfiat-node",
    )
    parser.add_argument(
        "--remote-topology",
        default="/etc/postfiat/releases/pnok-private-fix-2246d25/topology.json",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if not args.operator_key_file.is_file() or not args.node_bin.is_file():
        raise RuntimeError("operator key or local node binary is missing")
    demo = load_demo_module(Path(__file__).resolve().parent)
    root = args.output_dir.resolve()
    private = root / "private"
    public = root / "public"
    private.mkdir(parents=True, exist_ok=True, mode=0o700)
    public.mkdir(parents=True, exist_ok=True, mode=0o700)
    rpc, ports, height = fleet_state(demo, args)

    packet_file = public / "fix-packet.json"
    if packet_file.exists():
        packet = demo.load_json(packet_file)
    else:
        prior = predecessor(demo, rpc, ports, args.rpc_timeout_seconds)
        packet, policy, commitments = build_packet(
            demo, prior, height, args.max_fills, args.validity_blocks
        )
        demo.atomic_write_json(packet_file, packet, 0o644)
        demo.atomic_write_json(public / "governance-policy.json", policy, 0o644)
        demo.atomic_write_json(public / "commitments.json", commitments, 0o644)
    validate_packet(packet, demo, args.max_fills)

    report = verify_registered(demo, rpc, ports, packet, args.rpc_timeout_seconds)
    if report is None:
        operations = {
            "schema": "postfiat-certified-asset-ops-request-v1",
            "operations": [
                {
                    "label": f"pnok-demo-fix-epoch-{packet['epoch']}",
                    "source": OPERATOR,
                    "key_file": str(args.operator_key_file.resolve()),
                    "operation": {
                        "operation": "fx_fix_register_v1",
                        "operator": OPERATOR,
                        "packet": packet,
                    },
                }
            ],
        }
        ops_file = private / "register-fix.ops.json"
        demo.atomic_write_json(ops_file, operations, 0o600)
        attempts = sorted(private.glob("register-finality-attempt-*"))
        if len(attempts) >= 5:
            raise RuntimeError("successor FIX registration exhausted five durable attempts")
        attempt = len(attempts) + 1
        artifact = private / f"register-finality-attempt-{attempt}"
        command = [
            sys.executable,
            str(args.remote_finality_script),
            "--ops-file",
            str(ops_file),
            "--artifact-dir",
            str(artifact),
            "--node-bin",
            str(args.node_bin),
            "--remote-runner",
            str(args.remote_runner),
            "--proposer-hosts-file",
            str(args.proposer_hosts_file),
            "--remote-binary",
            args.remote_binary,
            "--remote-topology",
            args.remote_topology,
            "--ports",
            args.ports,
            "--timeout-seconds",
            str(args.rpc_timeout_seconds),
            "--preflight-seconds",
            str(args.convergence_timeout_seconds),
            "--postflight-seconds",
            str(args.convergence_timeout_seconds),
        ]
        result = subprocess.run(command, text=True, capture_output=True)
        demo.atomic_write_json(
            private / f"register-attempt-{attempt}.process.json",
            {
                "argv": command,
                "returncode": result.returncode,
                "stdout": result.stdout,
                "stderr": result.stderr,
            },
            0o600,
        )
        rpc, ports, _ = fleet_state(demo, args)
        report = verify_registered(demo, rpc, ports, packet, args.rpc_timeout_seconds)
        if report is None:
            raise RuntimeError(f"successor FIX did not finalize; runner exit={result.returncode}")

    status = {
        "schema": "postfiat-pnok-demo-fix-successor-status-v1",
        "stage": "complete",
        "packet_hash": packet["packet_hash"],
        "epoch": packet["epoch"],
        "status": report["fix"]["status"],
        "remaining_fill_slots": report["fix"]["remaining_fill_slots"],
        "max_fills": packet["max_fills"],
        "valid_from_height": packet["valid_from_height"],
        "expires_at_height": packet["expires_at_height"],
        "trust_class": "CONTROLLED",
    }
    demo.atomic_write_json(public / "status.json", status, 0o644)
    print(demo.canonical_json(status))


if __name__ == "__main__":
    main()
