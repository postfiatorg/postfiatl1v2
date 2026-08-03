#!/usr/bin/env python3
"""Advance the exact six-validator A666 fleet with value-carrying PFT transfers.

The public A666 reserve proof binds an observation window ending at a specific
PFTL height. Consensus must reach that height before the packet can be
submitted. This tool alternates small native transfers between two funded
operator accounts, one certified block per transfer, and fails closed around
every round. It never manufactures empty blocks or edits validator state.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
from typing import Any


EXPECTED_VALIDATORS = 6
DEFAULT_REMOTE_BINARY = (
    "/opt/postfiat/releases/pnok-private-fix-2246d25/postfiat-node"
)
DEFAULT_REMOTE_TOPOLOGY = (
    "/etc/postfiat/releases/pnok-private-fix-2246d25/topology.json"
)


def load_rpc_helpers(script_dir: Path) -> Any:
    path = script_dir / "a666-ce22-finality-op.py"
    spec = importlib.util.spec_from_file_location("a666_rpc_helpers", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import RPC helpers from {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def load_key_address(path: Path) -> str:
    if not path.is_file():
        raise RuntimeError(f"signing key file does not exist: {path}")
    value = json.loads(path.read_text())
    address = value.get("address")
    if not isinstance(address, str) or not address.startswith("pf"):
        raise RuntimeError(f"signing key file has no valid public address: {path}")
    return address


def proposer_hosts_from_fleet(path: Path) -> dict[str, str]:
    hosts: dict[str, str] = {}
    for raw_line in path.read_text().splitlines():
        fields = raw_line.split()
        if len(fields) >= 2 and fields[0].startswith("validator-"):
            hosts[fields[0]] = fields[1]
    expected = {f"validator-{index}" for index in range(EXPECTED_VALIDATORS)}
    if set(hosts) != expected:
        raise RuntimeError("fleet file must define exactly validator-0 through validator-5")
    return hosts


def write_json(path: Path, value: Any, mode: int = 0o600) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    os.chmod(path, mode)


def build_round_plan(
    current_height: int,
    target_height: int,
    amount: int,
    address_a: str,
    address_b: str,
) -> list[dict[str, Any]]:
    rounds = max(0, target_height - current_height)
    plan = []
    for offset in range(rounds):
        height = current_height + offset + 1
        source = address_a if offset % 2 == 0 else address_b
        recipient = address_b if offset % 2 == 0 else address_a
        plan.append(
            {
                "height": height,
                "source": source,
                "recipient": recipient,
                "amount": amount,
                "label": f"proof-height-pad-{height}",
            }
        )
    return plan


def parse_args() -> argparse.Namespace:
    script_dir = Path(__file__).resolve().parent
    parser = argparse.ArgumentParser()
    parser.add_argument("--target-height", type=int, default=784)
    parser.add_argument("--artifact-dir", type=Path, required=True)
    parser.add_argument("--node-bin", type=Path, required=True)
    parser.add_argument("--key-a", type=Path, required=True)
    parser.add_argument("--key-b", type=Path, required=True)
    parser.add_argument("--amount", type=int, default=10)
    parser.add_argument("--max-rounds", type=int, default=64)
    parser.add_argument(
        "--fleet-file",
        type=Path,
        default=Path("/home/postfiat/repos/wan-vultr-all-fleet.txt"),
    )
    parser.add_argument("--proposer-hosts-file", type=Path)
    parser.add_argument(
        "--remote-finality-op",
        type=Path,
        default=script_dir / "a666-ce22-remote-finality-op.py",
    )
    parser.add_argument(
        "--remote-runner",
        type=Path,
        default=script_dir / "a666-remote-sync-round.py",
    )
    parser.add_argument("--remote-binary", default=DEFAULT_REMOTE_BINARY)
    parser.add_argument("--remote-topology", default=DEFAULT_REMOTE_TOPOLOGY)
    parser.add_argument(
        "--ports",
        default="39650,39651,39652,39653,39654,39655",
    )
    parser.add_argument("--timeout-seconds", type=float, default=45.0)
    parser.add_argument("--preflight-seconds", type=float, default=45.0)
    parser.add_argument("--postflight-seconds", type=float, default=45.0)
    parser.add_argument(
        "--execute",
        action="store_true",
        help="perform the transfers; without this flag only a preflight plan is written",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.target_height < 1:
        raise RuntimeError("target height must be positive")
    if args.amount < 1:
        raise RuntimeError("transfer amount must be positive")
    if args.max_rounds < 1:
        raise RuntimeError("max rounds must be positive")
    if args.artifact_dir.exists():
        raise RuntimeError(f"artifact directory already exists: {args.artifact_dir}")
    if not args.node_bin.is_file() or not os.access(args.node_bin, os.X_OK):
        raise RuntimeError(f"node binary is not executable: {args.node_bin}")
    if not args.remote_finality_op.is_file() or not args.remote_runner.is_file():
        raise RuntimeError("remote finality scripts are missing")

    address_a = load_key_address(args.key_a)
    address_b = load_key_address(args.key_b)
    if address_a == address_b:
        raise RuntimeError("padding transfer accounts must be distinct")

    ports = [int(value) for value in args.ports.split(",")]
    if len(ports) != EXPECTED_VALIDATORS or len(set(ports)) != EXPECTED_VALIDATORS:
        raise RuntimeError("exactly six distinct RPC endpoints are required")
    rpc = load_rpc_helpers(Path(__file__).resolve().parent)
    pre = rpc.wait_for_fleet_status(
        ports,
        args.timeout_seconds,
        args.preflight_seconds,
    )
    current_height = int(pre[0]["block_height"])
    rounds = max(0, args.target_height - current_height)
    if rounds > args.max_rounds:
        raise RuntimeError(
            f"required {rounds} rounds exceeds fail-closed maximum {args.max_rounds}"
        )

    args.artifact_dir.mkdir(parents=True, mode=0o700)
    if args.proposer_hosts_file is None:
        hosts = proposer_hosts_from_fleet(args.fleet_file)
        proposer_hosts_file = args.artifact_dir / "proposer-hosts.private.json"
        write_json(proposer_hosts_file, hosts)
    else:
        proposer_hosts_file = args.proposer_hosts_file

    plan_rounds = build_round_plan(
        current_height,
        args.target_height,
        args.amount,
        address_a,
        address_b,
    )
    plan = {
        "schema": "postfiat-a666-proof-height-padding-plan-v1",
        "execute": args.execute,
        "start_height": current_height,
        "target_height": args.target_height,
        "required_rounds": rounds,
        "validator_count": EXPECTED_VALIDATORS,
        "start_tip_hash": pre[0]["block_tip_hash"],
        "start_state_root": pre[0]["state_root"],
        "mempool_pending": 0,
        "rounds": plan_rounds,
    }
    write_json(args.artifact_dir / "plan.json", plan, 0o644)

    if not args.execute or rounds == 0:
        print(json.dumps(plan, indent=2, sort_keys=True))
        return

    summaries = []
    for offset, item in enumerate(plan_rounds):
        key_file = args.key_a if offset % 2 == 0 else args.key_b
        round_root = args.artifact_dir / f"height-{item['height']}"
        spec_file = args.artifact_dir / f"height-{item['height']}.transfer.private.json"
        write_json(
            spec_file,
            {
                "label": item["label"],
                "from": item["source"],
                "to": item["recipient"],
                "amount": item["amount"],
                "key_file": str(key_file),
            },
        )
        command = [
            sys.executable,
            str(args.remote_finality_op),
            "--transfer-spec-file",
            str(spec_file),
            "--artifact-dir",
            str(round_root),
            "--node-bin",
            str(args.node_bin),
            "--remote-runner",
            str(args.remote_runner),
            "--proposer-hosts-file",
            str(proposer_hosts_file),
            "--remote-binary",
            args.remote_binary,
            "--remote-topology",
            args.remote_topology,
            "--ports",
            args.ports,
            "--timeout-seconds",
            str(args.timeout_seconds),
            "--preflight-seconds",
            str(args.preflight_seconds),
            "--postflight-seconds",
            str(args.postflight_seconds),
        ]
        subprocess.run(command, check=True, text=True, capture_output=True)
        summary = json.loads((round_root / "summary.json").read_text())
        if (
            summary.get("round_ok") is not True
            or summary.get("end_height") != item["height"]
            or summary.get("end_mempool_pending") != 0
            or summary.get("transaction_kind") != "transfer"
        ):
            raise RuntimeError(f"height {item['height']} did not close exactly")
        summaries.append(summary)

    post = rpc.wait_for_fleet_status(
        ports,
        args.timeout_seconds,
        args.postflight_seconds,
    )
    if int(post[0]["block_height"]) != args.target_height:
        raise RuntimeError("fleet final height does not equal requested target")
    report = {
        "schema": "postfiat-a666-proof-height-padding-report-v1",
        "status": "pass",
        "start_height": current_height,
        "target_height": args.target_height,
        "completed_rounds": len(summaries),
        "validator_count": EXPECTED_VALIDATORS,
        "final_tip_hash": post[0]["block_tip_hash"],
        "final_state_root": post[0]["state_root"],
        "mempool_pending": 0,
        "round_summaries": summaries,
    }
    write_json(args.artifact_dir / "report.json", report, 0o644)
    print(json.dumps(report, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
