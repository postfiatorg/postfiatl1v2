#!/usr/bin/env python3
"""Run one ce22 asset transaction from the elected validator proposer.

This helper is copied to the elected validator and invoked inside a private
mount namespace where ``DATA_DIR/certified-send-outbox`` is overlaid with a
per-operation empty directory.  That prevents the round wrapper from resuming
unrelated durable-send jobs while preserving the validator's real consensus
state and key.  The new round uses synchronous certified sends.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--node-bin", type=Path, required=True)
    parser.add_argument("--data-dir", type=Path, required=True)
    parser.add_argument("--topology", type=Path, required=True)
    parser.add_argument("--key-file", type=Path, required=True)
    parser.add_argument("--signed-file", type=Path, required=True)
    parser.add_argument("--artifact-dir", type=Path, required=True)
    parser.add_argument("--height", type=int, required=True)
    parser.add_argument("--view", type=int, default=0)
    parser.add_argument("--timeout-ms", type=int, default=30_000)
    parser.add_argument("--send-retries", type=int, default=16)
    parser.add_argument("--retry-backoff-ms", type=int, default=250)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    signed = json.loads(args.signed_file.read_text())
    signed_json = json.dumps(signed, separators=(",", ":"))
    command = [
        str(args.node_bin),
        "transport-peer-certified-mempool-round",
        "--unsafe-devnet-json-storage",
        "--data-dir",
        str(args.data_dir),
        "--topology",
        str(args.topology),
        "--key-file",
        str(args.key_file),
        "--proposal-key-file",
        str(args.key_file),
        "--signed-asset-transaction-json",
        signed_json,
        "--artifact-dir",
        str(args.artifact_dir),
        "--height",
        str(args.height),
        "--view",
        str(args.view),
        "--require-local-proposer",
        "--quorum-early-full-propagation",
        "--local-apply-before-certified-send",
        "--timeout-ms",
        str(args.timeout_ms),
        "--send-retries",
        str(args.send_retries),
        "--retry-backoff-ms",
        str(args.retry_backoff_ms),
    ]
    completed = subprocess.run(command, text=True, capture_output=True)
    if completed.returncode != 0:
        raise RuntimeError(
            "certified round command failed "
            f"(exit {completed.returncode}): {completed.stderr.strip()}"
        )
    report = json.loads(completed.stdout)
    if report.get("round_ok") is not True:
        raise RuntimeError("certified round did not report round_ok=true")
    report_file = args.artifact_dir / "round-report.json"
    report_file.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    report_file.chmod(0o600)
    print(
        json.dumps(
            {
                "schema": report.get("schema"),
                "node_id": report.get("node_id"),
                "submitted_tx_id": report.get("submitted_tx_id"),
                "round_ok": report.get("round_ok"),
                "block_height": report["round"]["certification"]["block_height"],
                "certificate_id": report["round"]["certification"]["certificate_id"],
                "vote_count": report["round"]["certification"]["vote_count"],
                "all_sends_verified": report["round"]["all_sends_verified"],
                "local_apply_verified": report["round"]["local_apply_verified"],
            },
            indent=2,
            sort_keys=True,
        )
    )


if __name__ == "__main__":
    main()
