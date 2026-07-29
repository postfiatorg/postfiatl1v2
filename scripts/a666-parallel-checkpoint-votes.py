#!/usr/bin/env python3
"""Collect six independent Ethereum checkpoint votes concurrently."""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import json
from pathlib import Path
import re
import shlex
import subprocess
import tempfile
import time
from typing import Any


EXPECTED_VALIDATORS = 6


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--hosts-file", type=Path, required=True)
    parser.add_argument("--checkpoint-file", type=Path, required=True)
    parser.add_argument("--proof-dir", type=Path, required=True)
    parser.add_argument("--workflow-id", required=True)
    parser.add_argument("--remote-suffix", required=True)
    parser.add_argument("--remote-node", required=True)
    parser.add_argument("--ethereum-rpc", required=True)
    parser.add_argument("--validator2-remote-root", required=True)
    return parser.parse_args()


def run(command: list[str], *, capture: bool = False) -> subprocess.CompletedProcess[str]:
    return subprocess.run(command, check=True, text=True, capture_output=capture)


def ssh_options(control_dir: Path) -> list[str]:
    return [
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=15",
        "-o",
        "ControlMaster=auto",
        "-o",
        "ControlPersist=60",
        "-o",
        f"ControlPath={control_dir}/%C",
    ]


def collect_vote(
    *,
    validator: str,
    host: str,
    checkpoint_file: Path,
    proof_dir: Path,
    workflow_id: str,
    remote_suffix: str,
    remote_node: str,
    ethereum_rpc: str,
    validator2_host: str,
    validator2_remote_root: str,
    control_dir: Path,
) -> dict[str, Any]:
    started_unix_ms = int(time.time() * 1000)
    started = time.monotonic()
    options = ssh_options(control_dir)
    remote_run = f"/var/lib/postfiat/{validator}/{workflow_id}-{remote_suffix}"
    remote_checkpoint = f"{remote_run}/checkpoint.json"
    remote_vote = f"{remote_run}/{validator}.vote.json"
    local_vote = proof_dir / f"{validator}.vote.json"
    local_report = proof_dir / f"{validator}.vote-report.json"

    run(
        [
            "ssh",
            *options,
            f"root@{host}",
            f"install -d -m 700 {shlex.quote(remote_run)}",
        ]
    )
    run(
        [
            "scp",
            "-q",
            *options,
            str(checkpoint_file),
            f"root@{host}:{remote_checkpoint}",
        ]
    )
    command = " ".join(
        shlex.quote(value)
        for value in [
            remote_node,
            "ethereum-checkpoint-vote-sign",
            "--data-dir",
            f"/var/lib/postfiat/{validator}",
            "--checkpoint-file",
            remote_checkpoint,
            "--ethereum-rpc",
            ethereum_rpc,
            "--validator",
            validator,
            "--validator-key-file",
            f"/var/lib/postfiat/{validator}/validator_keys.json",
            "--vote-file",
            remote_vote,
        ]
    )
    completed = run(["ssh", *options, f"root@{host}", command], capture=True)
    local_report.write_text(completed.stdout)
    local_report.chmod(0o644)
    run(["scp", "-q", *options, f"root@{host}:{remote_vote}", str(local_vote)])

    validator2_vote = f"{validator2_remote_root}/{validator}.vote.json"
    run(
        [
            "scp",
            "-q",
            *options,
            str(local_vote),
            f"root@{validator2_host}:{validator2_vote}",
        ]
    )
    completed_unix_ms = int(time.time() * 1000)
    return {
        "validator": validator,
        "host": host,
        "started_at_unix_ms": started_unix_ms,
        "completed_at_unix_ms": completed_unix_ms,
        "elapsed_ms": (time.monotonic() - started) * 1000.0,
        "local_vote_file": str(local_vote),
        "remote_vote_file": validator2_vote,
    }


def main() -> None:
    args = parse_args()
    safe_component = re.compile(r"[a-z0-9][a-z0-9-]{0,63}")
    if not safe_component.fullmatch(args.workflow_id):
        raise RuntimeError("workflow-id must match [a-z0-9][a-z0-9-]{0,63}")
    if not safe_component.fullmatch(args.remote_suffix):
        raise RuntimeError("remote-suffix must match [a-z0-9][a-z0-9-]{0,63}")
    hosts = json.loads(args.hosts_file.read_text())
    expected = {f"validator-{index}" for index in range(EXPECTED_VALIDATORS)}
    if not isinstance(hosts, dict) or set(hosts) != expected:
        raise RuntimeError("hosts file must map validator-0 through validator-5")
    if not args.checkpoint_file.is_file():
        raise RuntimeError(f"checkpoint file not found: {args.checkpoint_file}")
    args.proof_dir.mkdir(parents=True, exist_ok=True)

    started_unix_ms = int(time.time() * 1000)
    started = time.monotonic()
    rows: list[dict[str, Any]] = []
    with tempfile.TemporaryDirectory(prefix="a666-ssh-control-") as control_dir_text:
        control_dir = Path(control_dir_text)
        with ThreadPoolExecutor(max_workers=EXPECTED_VALIDATORS) as executor:
            futures = {
                executor.submit(
                    collect_vote,
                    validator=validator,
                    host=hosts[validator],
                    checkpoint_file=args.checkpoint_file,
                    proof_dir=args.proof_dir,
                    workflow_id=args.workflow_id,
                    remote_suffix=args.remote_suffix,
                    remote_node=args.remote_node,
                    ethereum_rpc=args.ethereum_rpc,
                    validator2_host=hosts["validator-2"],
                    validator2_remote_root=args.validator2_remote_root,
                    control_dir=control_dir,
                ): validator
                for validator in sorted(expected)
            }
            for future in as_completed(futures):
                rows.append(future.result())

    rows.sort(key=lambda row: row["validator"])
    remote_vote_files = [row["remote_vote_file"] for row in rows]
    result = {
        "schema": "postfiat-a666-parallel-checkpoint-votes-v1",
        "strategy": "six independent validator signatures collected concurrently",
        "validator_count": len(rows),
        "started_at_unix_ms": started_unix_ms,
        "completed_at_unix_ms": int(time.time() * 1000),
        "elapsed_ms": (time.monotonic() - started) * 1000.0,
        "remote_vote_files": remote_vote_files,
        "remote_vote_files_csv": ",".join(remote_vote_files),
        "validators": rows,
    }
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
