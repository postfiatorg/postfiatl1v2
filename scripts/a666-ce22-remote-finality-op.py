#!/usr/bin/env python3
"""Submit one additive a666 operation through a clean ce22 proposer round."""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import shlex
import subprocess
import sys
import time
from typing import Any


EXPECTED_VALIDATORS = 6
REMOTE_BINARY = (
    "/opt/postfiat/releases/open-source-consensus-viewlock-b04c595e/postfiat-node"
)
REMOTE_TOPOLOGY = (
    "/etc/postfiat/releases/open-source-consensus-viewlock-b04c595e/topology.json"
)


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


def load_proposer_hosts(path: Path) -> dict[str, str]:
    value = json.loads(path.read_text())
    expected = {f"validator-{index}" for index in range(EXPECTED_VALIDATORS)}
    if not isinstance(value, dict) or set(value) != expected:
        raise RuntimeError("proposer hosts file must map exactly validator-0 through validator-5")
    if any(
        not isinstance(host, str)
        or not host
        or any(character.isspace() for character in host)
        or "@" in host
        for host in value.values()
    ):
        raise RuntimeError("proposer hosts file contains an invalid SSH host")
    return value


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--ops-file", type=Path, required=True)
    parser.add_argument("--artifact-dir", type=Path, required=True)
    parser.add_argument("--node-bin", type=Path, required=True)
    parser.add_argument("--remote-runner", type=Path, required=True)
    parser.add_argument("--proposer-hosts-file", type=Path, required=True)
    parser.add_argument("--remote-binary", default=REMOTE_BINARY)
    parser.add_argument("--remote-topology", default=REMOTE_TOPOLOGY)
    parser.add_argument(
        "--ports",
        default="28650,28651,28652,28653,28654,28655",
    )
    parser.add_argument("--timeout-seconds", type=float, default=45.0)
    parser.add_argument("--preflight-seconds", type=float, default=45.0)
    parser.add_argument("--postflight-seconds", type=float, default=45.0)
    parser.add_argument("--resident-manifest", type=Path)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    proposer_hosts = load_proposer_hosts(args.proposer_hosts_file)
    rpc = load_rpc_helpers(Path(__file__).resolve().parent)
    ports = [int(value) for value in args.ports.split(",")]
    if len(ports) != EXPECTED_VALIDATORS:
        raise RuntimeError("exactly six RPC endpoints are required")
    if args.artifact_dir.exists():
        raise RuntimeError(f"artifact directory already exists: {args.artifact_dir}")
    args.artifact_dir.mkdir(parents=True, mode=0o700)

    payload = json.loads(args.ops_file.read_text())
    operations = payload.get("operations")
    if not isinstance(operations, list) or len(operations) != 1:
        raise RuntimeError("ops file must contain exactly one operation")
    item = operations[0]
    label = item["label"]
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]{0,63}", label):
        raise RuntimeError("operation label is not a safe remote path component")
    source = item["source"]
    key_file = Path(item["key_file"])
    operation = item["operation"]
    if not key_file.is_file():
        raise RuntimeError(f"declared signing key does not exist: {key_file}")

    pre = rpc.wait_for_fleet_status(
        ports,
        args.timeout_seconds,
        args.preflight_seconds,
    )
    parent = pre[0]
    preflight = {
        "schema": "postfiat-a666-ce22-remote-finality-preflight-v1",
        "label": label,
        "validator_count": len(pre),
        "height": parent["block_height"],
        "block_tip_hash": parent["block_tip_hash"],
        "state_root": parent["state_root"],
        "mempool_pending": 0,
        "nodes": [
            {
                "node_id": row["node_id"],
                "port": row["port"],
                "height": row["block_height"],
                "state_root": row["state_root"],
            }
            for row in pre
        ],
    }
    rpc.write_json(args.artifact_dir / "preflight-fleet.json", preflight, 0o644)

    quote_request = rpc.request(
        f"{label}-quote",
        "asset_fee_quote",
        {
            "source": source,
            "operation_json": json.dumps(operation, separators=(",", ":")),
        },
    )
    quote_response = rpc.rpc_call(ports[0], quote_request, args.timeout_seconds)
    rpc.write_json(args.artifact_dir / "quote.request.json", quote_request, 0o644)
    rpc.write_json(args.artifact_dir / "quote.response.json", quote_response, 0o644)
    if quote_response.get("ok") is not True:
        raise RuntimeError(f"asset fee quote failed: {quote_response.get('error')}")
    quote_file = args.artifact_dir / "quote.result.json"
    rpc.write_json(quote_file, quote_response["result"], 0o644)
    signed_raw = run(
        [
            str(args.node_bin),
            "wallet-sign-asset-transaction",
            "--key-file",
            str(key_file),
            "--quote-file",
            str(quote_file),
        ],
        capture=True,
    ).stdout
    signed = json.loads(signed_raw)
    signed_file = args.artifact_dir / "signed.json"
    rpc.write_json(signed_file, signed)

    next_height = int(parent["block_height"]) + 1
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
    proposer_report = json.loads(proposer_probe.stdout)
    proposer = proposer_report["proposer"]
    if proposer not in proposer_hosts:
        raise RuntimeError(f"unknown elected proposer: {proposer}")
    host = proposer_hosts[proposer]
    index = proposer.rsplit("-", 1)[1]
    data_dir = f"/var/lib/postfiat/validator-{index}"
    key_path = f"{data_dir}/validator_keys.json"
    remote_label = f"a666-{next_height}-{label}"
    remote_signed = f"{data_dir}/{remote_label}.signed.json"
    remote_artifacts = f"{data_dir}/a666-finality-artifacts/{remote_label}"
    isolated_outbox = f"{data_dir}/a666-isolated-outboxes/{remote_label}"
    remote_runner = "/usr/local/sbin/a666-remote-sync-round.py"

    if args.resident_manifest is not None:
        resident_manifest = json.loads(args.resident_manifest.read_text())
        if resident_manifest.get("schema") != "postfiat.a666.resident_round_manifest.v1":
            raise RuntimeError("resident round manifest schema mismatch")
        entries = [
            entry
            for entry in resident_manifest.get("entries", [])
            if entry.get("height") == next_height
            and entry.get("batch_kind") == "transparent"
        ]
        if len(entries) != 1:
            raise RuntimeError(
                f"resident manifest does not select one transparent worker at {next_height}"
            )
        entry = entries[0]
        if entry.get("proposer") != proposer or entry.get("host") != host:
            raise RuntimeError("resident worker does not match the elected proposer")
        build_root = f"{entry['remote']['root']}/build-{label}"
        remote_signed = f"{build_root}/signed.json"
        remote_batch = f"{build_root}/batch.json"
        run(
            [
                "scp",
                "-q",
                str(signed_file),
                f"root@{host}:/tmp/{remote_label}.signed.json",
            ]
        )
        build = "set -euo pipefail; " + "; ".join(
            [
                f"test ! -e {shlex.quote(build_root)}",
                (
                    "install -d -o postfiat -g postfiat -m 700 "
                    f"{shlex.quote(build_root)}"
                ),
                (
                    "install -o postfiat -g postfiat -m 600 "
                    f"{shlex.quote('/tmp/' + remote_label + '.signed.json')} "
                    f"{shlex.quote(remote_signed)}"
                ),
                (
                    "runuser -u postfiat -- "
                    f"{shlex.quote(args.remote_binary)} signed-asset-batch "
                    f"--data-dir {shlex.quote(data_dir)} "
                    f"--batch-file {shlex.quote(remote_batch)} "
                    "--signed-asset-transaction-files "
                    f"{shlex.quote(remote_signed)} >/dev/null"
                ),
            ]
        )
        run(["ssh", "-o", "BatchMode=yes", f"root@{host}", build])
        local_batch = args.artifact_dir / "resident-input-batch.json"
        run(["scp", "-q", f"root@{host}:{remote_batch}", str(local_batch)])
        resident_artifacts = args.artifact_dir / "resident-finality"
        run(
            [
                sys.executable,
                str(Path(__file__).resolve().parent / "a666-resident-rounds.py"),
                "submit",
                "--manifest",
                str(args.resident_manifest),
                "--batch-file",
                str(local_batch),
                "--batch-kind",
                "transparent",
                "--label",
                label,
                "--artifact-dir",
                str(resident_artifacts),
                "--ports",
                args.ports,
                "--timeout-seconds",
                str(args.timeout_seconds),
                "--preflight-seconds",
                str(args.preflight_seconds),
                "--postflight-seconds",
                str(args.postflight_seconds),
            ]
        )
        resident_summary = json.loads(
            (resident_artifacts / "summary.json").read_text()
        )
        shutil.copytree(
            resident_artifacts / "consensus",
            args.artifact_dir / "consensus",
        )
        round_summary = {
            "schema": "postfiat-transport-peer-certified-mempool-round-v1",
            "node_id": proposer,
            "submitted_tx_id": None,
            "round_ok": resident_summary["round_ok"],
            "block_height": resident_summary["end_height"],
            "certificate_id": json.loads(
                (args.artifact_dir / "consensus/round-report.json").read_text()
            )["certification"]["certificate_id"],
            "vote_count": resident_summary["vote_count"],
            "all_sends_verified": resident_summary["all_sends_verified"],
            "local_apply_verified": resident_summary["local_apply_verified"],
            "execution_mode": "prewarmed_resident_worker",
        }
        rpc.write_json(args.artifact_dir / "remote-round-summary.json", round_summary, 0o644)
        summary = {
            "schema": "postfiat-a666-ce22-remote-finality-operation-v1",
            "execution_mode": "prewarmed_resident_worker",
            "label": label,
            "source": source,
            "transaction_kind": signed["unsigned"]["transaction_kind"],
            "tx_id": None,
            "accepted": resident_summary["accepted"],
            "confirmed": resident_summary["confirmed"],
            "round_ok": resident_summary["round_ok"],
            "proposer": proposer,
            "validator_count": EXPECTED_VALIDATORS,
            "vote_count": resident_summary["vote_count"],
            "start_height": resident_summary["start_height"],
            "end_height": resident_summary["end_height"],
            "start_state_root": resident_summary["start_state_root"],
            "end_state_root": resident_summary["end_state_root"],
            "end_mempool_pending": 0,
            "all_sends_verified": resident_summary["all_sends_verified"],
            "trust_class": "CONTROLLED",
        }
        rpc.write_json(args.artifact_dir / "summary.json", summary, 0o644)
        print(json.dumps(summary, indent=2, sort_keys=True))
        return

    run(
        [
            "scp",
            "-q",
            str(args.remote_runner),
            f"root@{host}:/tmp/{remote_label}.runner.py",
        ]
    )
    run(["scp", "-q", str(signed_file), f"root@{host}:/tmp/{remote_label}.signed.json"])
    prepare = "set -euo pipefail; " + "; ".join(
        [
            f"test ! -e {shlex.quote(remote_artifacts)}",
            f"test ! -e {shlex.quote(isolated_outbox)}",
            f"install -d -o postfiat -g postfiat -m 700 {shlex.quote(isolated_outbox)}",
            (
                "install -d -o postfiat -g postfiat -m 700 "
                f"{shlex.quote(data_dir + '/a666-finality-artifacts')}"
            ),
            (
                "install -o postfiat -g postfiat -m 600 "
                f"{shlex.quote('/tmp/' + remote_label + '.signed.json')} "
                f"{shlex.quote(remote_signed)}"
            ),
            (
                "install -o root -g root -m 755 "
                f"{shlex.quote('/tmp/' + remote_label + '.runner.py')} "
                f"{shlex.quote(remote_runner)}"
            ),
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
        "--signed-file",
        remote_signed,
        "--artifact-dir",
        remote_artifacts,
        "--height",
        str(next_height),
        "--view",
        "0",
    ]
    namespace_command = (
        "set -euo pipefail; "
        f"mount --bind {shlex.quote(isolated_outbox)} "
        f"{shlex.quote(data_dir + '/certified-send-outbox')}; "
        "exec runuser -u postfiat -- "
        + " ".join(shlex.quote(value) for value in runner_args)
    )
    remote_namespace_command = (
        "unshare --mount --propagation private -- /bin/bash -c "
        + shlex.quote(namespace_command)
    )
    run(
        [
            "ssh",
            "-o",
            "BatchMode=yes",
            f"root@{host}",
            remote_namespace_command,
        ],
    )

    consensus_dir = args.artifact_dir / "consensus"
    consensus_dir.mkdir(mode=0o700)
    run(
        [
            "rsync",
            "-a",
            f"root@{host}:{remote_artifacts}/",
            f"{consensus_dir}/",
        ]
    )
    full_report = json.loads((consensus_dir / "round-report.json").read_text())
    if (
        full_report.get("round_ok") is not True
        or full_report["round"].get("all_sends_verified") is not True
        or full_report["round"].get("local_apply_verified") is not True
    ):
        raise RuntimeError("copied consensus report did not prove full propagation")
    round_summary = {
        "schema": full_report.get("schema"),
        "node_id": full_report.get("node_id"),
        "submitted_tx_id": full_report.get("submitted_tx_id"),
        "round_ok": full_report.get("round_ok"),
        "block_height": full_report["round"]["certification"]["block_height"],
        "certificate_id": full_report["round"]["certification"]["certificate_id"],
        "vote_count": full_report["round"]["certification"]["vote_count"],
        "all_sends_verified": full_report["round"]["all_sends_verified"],
        "local_apply_verified": full_report["round"]["local_apply_verified"],
    }
    rpc.write_json(args.artifact_dir / "remote-round-summary.json", round_summary, 0o644)

    deadline = time.monotonic() + args.postflight_seconds
    post = None
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            candidate = rpc.fleet_status(ports, args.timeout_seconds)
            if candidate[0]["block_height"] == next_height:
                post = candidate
                break
        except Exception as error:
            last_error = error
        time.sleep(0.25)
    if post is None:
        raise RuntimeError(f"fleet did not converge after finality: {last_error}")

    summary = {
        "schema": "postfiat-a666-ce22-remote-finality-operation-v1",
        "label": label,
        "source": source,
        "transaction_kind": signed["unsigned"]["transaction_kind"],
        "tx_id": full_report["submitted_tx_id"],
        "accepted": True,
        "confirmed": True,
        "round_ok": True,
        "proposer": proposer,
        "validator_count": len(post),
        "vote_count": full_report["round"]["certification"]["vote_count"],
        "start_height": parent["block_height"],
        "end_height": post[0]["block_height"],
        "start_state_root": parent["state_root"],
        "end_state_root": post[0]["state_root"],
        "end_block_tip_hash": post[0]["block_tip_hash"],
        "end_mempool_pending": 0,
        "all_sends_verified": True,
        "trust_class": "CONTROLLED",
    }
    rpc.write_json(args.artifact_dir / "summary.json", summary, 0o644)
    print(json.dumps(summary, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
