#!/usr/bin/env python3
"""Submit one prebuilt batch through a clean elected-proposer ce22 round."""

from __future__ import annotations

import argparse
import importlib.util
import json
from pathlib import Path
import re
import shlex
import subprocess
import time
from typing import Any


EXPECTED_VALIDATORS = 6


def load_rpc_helpers(script_dir: Path) -> Any:
    path = script_dir / "a666-ce22-finality-op.py"
    spec = importlib.util.spec_from_file_location("a666_rpc_helpers", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import RPC helpers from {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def run(command: list[str], *, capture: bool = False) -> subprocess.CompletedProcess[str]:
    return subprocess.run(command, check=True, text=True, capture_output=capture)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--batch-file", type=Path, required=True)
    parser.add_argument("--batch-kind", choices=("transparent", "governance", "shielded", "bridge"), required=True)
    parser.add_argument("--label", required=True)
    parser.add_argument("--artifact-dir", type=Path, required=True)
    parser.add_argument("--remote-runner", type=Path, required=True)
    parser.add_argument("--proposer-hosts-file", type=Path, required=True)
    parser.add_argument("--remote-binary", required=True)
    parser.add_argument("--remote-topology", required=True)
    parser.add_argument("--ports", default="28650,28651,28652,28653,28654,28655")
    parser.add_argument("--timeout-seconds", type=float, default=45.0)
    parser.add_argument("--postflight-seconds", type=float, default=45.0)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]{0,63}", args.label):
        raise RuntimeError("label is not a safe remote path component")
    proposer_hosts = json.loads(args.proposer_hosts_file.read_text())
    expected = {f"validator-{index}" for index in range(EXPECTED_VALIDATORS)}
    if not isinstance(proposer_hosts, dict) or set(proposer_hosts) != expected:
        raise RuntimeError("proposer hosts file must map validator-0 through validator-5")
    ports = [int(value) for value in args.ports.split(",")]
    if len(ports) != EXPECTED_VALIDATORS:
        raise RuntimeError("exactly six RPC endpoints are required")
    if args.artifact_dir.exists():
        raise RuntimeError(f"artifact directory already exists: {args.artifact_dir}")
    args.artifact_dir.mkdir(parents=True, mode=0o700)

    rpc = load_rpc_helpers(Path(__file__).resolve().parent)
    pre = rpc.fleet_status(ports, args.timeout_seconds)
    parent = pre[0]
    next_height = int(parent["block_height"]) + 1
    rpc.write_json(
        args.artifact_dir / "preflight-fleet.json",
        {
            "schema": "postfiat-a666-ce22-remote-finality-batch-preflight-v1",
            "label": args.label,
            "batch_kind": args.batch_kind,
            "height": parent["block_height"],
            "state_root": parent["state_root"],
            "nodes": pre,
        },
        0o644,
    )

    proposer_probe = run(
        [
            "ssh",
            "-o",
            "BatchMode=yes",
            f"root@{proposer_hosts['validator-0']}",
            args.remote_binary,
            "block-proposer",
            "--unsafe-devnet-json-storage",
            "--data-dir",
            "/var/lib/postfiat/validator-0",
            "--height",
            str(next_height),
            "--view",
            "0",
        ],
        capture=True,
    )
    proposer = json.loads(proposer_probe.stdout)["proposer"]
    if proposer not in proposer_hosts:
        raise RuntimeError(f"unknown elected proposer: {proposer}")
    host = proposer_hosts[proposer]
    index = proposer.rsplit("-", 1)[1]
    data_dir = f"/var/lib/postfiat/validator-{index}"
    key_path = f"{data_dir}/validator_keys.json"
    remote_label = f"a666-{next_height}-{args.label}"
    remote_batch = f"{data_dir}/{remote_label}.batch.json"
    remote_artifacts = f"{data_dir}/a666-finality-artifacts/{remote_label}"
    isolated_outbox = f"{data_dir}/a666-isolated-outboxes/{remote_label}"
    remote_runner = "/usr/local/sbin/a666-remote-sync-batch-round.py"

    run(["scp", "-q", str(args.remote_runner), f"root@{host}:/tmp/{remote_label}.runner.py"])
    run(["scp", "-q", str(args.batch_file), f"root@{host}:/tmp/{remote_label}.batch.json"])
    prepare = "set -euo pipefail; " + "; ".join(
        [
            f"test ! -e {shlex.quote(remote_artifacts)}",
            f"test ! -e {shlex.quote(isolated_outbox)}",
            f"install -d -o postfiat -g postfiat -m 700 {shlex.quote(isolated_outbox)}",
            f"install -d -o postfiat -g postfiat -m 700 {shlex.quote(data_dir + '/a666-finality-artifacts')}",
            f"install -o postfiat -g postfiat -m 600 {shlex.quote('/tmp/' + remote_label + '.batch.json')} {shlex.quote(remote_batch)}",
            f"install -o root -g root -m 755 {shlex.quote('/tmp/' + remote_label + '.runner.py')} {shlex.quote(remote_runner)}",
        ]
    )
    run(["ssh", "-o", "BatchMode=yes", f"root@{host}", prepare])

    runner_args = [
        remote_runner,
        "--node-bin",
        args.remote_binary,
        "--data-dir",
        data_dir,
        "--topology",
        args.remote_topology,
        "--key-file",
        key_path,
        "--batch-kind",
        args.batch_kind,
        "--batch-file",
        remote_batch,
        "--artifact-dir",
        remote_artifacts,
        "--height",
        str(next_height),
        "--view",
        "0",
    ]
    namespace_command = (
        "set -euo pipefail; "
        f"mount --bind {shlex.quote(isolated_outbox)} {shlex.quote(data_dir + '/certified-send-outbox')}; "
        "exec runuser -u postfiat -- "
        + " ".join(shlex.quote(value) for value in runner_args)
    )
    run(
        [
            "ssh",
            "-o",
            "BatchMode=yes",
            f"root@{host}",
            "unshare --mount --propagation private -- /bin/bash -c "
            + shlex.quote(namespace_command),
        ]
    )

    consensus_dir = args.artifact_dir / "consensus"
    consensus_dir.mkdir(mode=0o700)
    run(["rsync", "-a", f"root@{host}:{remote_artifacts}/", f"{consensus_dir}/"])
    report = json.loads((consensus_dir / "round-report.json").read_text())
    if (
        report.get("round_ok") is not True
        or report.get("all_sends_verified") is not True
        or report.get("local_apply_verified") is not True
    ):
        raise RuntimeError("copied consensus report did not prove full propagation")

    deadline = time.monotonic() + args.postflight_seconds
    post = None
    while time.monotonic() < deadline:
        candidate = rpc.fleet_status(ports, args.timeout_seconds)
        if candidate[0]["block_height"] == next_height:
            post = candidate
            break
        time.sleep(0.25)
    if post is None:
        raise RuntimeError("fleet did not converge after finality")
    summary = {
        "schema": "postfiat-a666-ce22-remote-finality-batch-v1",
        "label": args.label,
        "batch_kind": args.batch_kind,
        "accepted": True,
        "confirmed": True,
        "round_ok": True,
        "proposer": proposer,
        "vote_count": report["certification"]["vote_count"],
        "start_height": parent["block_height"],
        "end_height": post[0]["block_height"],
        "start_state_root": parent["state_root"],
        "end_state_root": post[0]["state_root"],
        "all_sends_verified": True,
        "local_apply_verified": True,
    }
    rpc.write_json(args.artifact_dir / "summary.json", summary, 0o644)
    print(json.dumps(summary, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
