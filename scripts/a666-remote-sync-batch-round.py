#!/usr/bin/env python3
"""Run one already-built batch through the elected validator proposer."""

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
    parser.add_argument("--batch-kind", choices=("transparent", "governance", "shielded", "bridge"), required=True)
    parser.add_argument("--batch-file", type=Path, required=True)
    parser.add_argument("--artifact-dir", type=Path, required=True)
    parser.add_argument("--height", type=int, required=True)
    parser.add_argument("--view", type=int, default=0)
    parser.add_argument("--timeout-ms", type=int, default=30_000)
    parser.add_argument("--send-retries", type=int, default=16)
    parser.add_argument("--retry-backoff-ms", type=int, default=250)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    command = [
        str(args.node_bin),
        "transport-peer-certified-batch-round",
        "--unsafe-devnet-json-storage",
        "--data-dir",
        str(args.data_dir),
        "--topology",
        str(args.topology),
        "--key-file",
        str(args.key_file),
        "--proposal-key-file",
        str(args.key_file),
        "--batch-kind",
        args.batch_kind,
        "--batch-file",
        str(args.batch_file),
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
            "certified batch round failed "
            f"(exit {completed.returncode}): {completed.stderr.strip()}"
        )
    report = json.loads(completed.stdout)
    report_file = args.artifact_dir / "round-report.json"
    report_file.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    report_file.chmod(0o600)
    certification = report.get("certification") or {}
    print(
        json.dumps(
            {
                "schema": report.get("schema"),
                "from": report.get("from"),
                "round_ok": report.get("round_ok"),
                "block_height": certification.get("block_height"),
                "certificate_id": certification.get("certificate_id"),
                "vote_count": certification.get("vote_count"),
                "all_sends_verified": report.get("all_sends_verified"),
                "local_apply_verified": report.get("local_apply_verified"),
            },
            indent=2,
            sort_keys=True,
        )
    )
    if report.get("round_ok") is not True:
        raise RuntimeError("certified batch round did not report round_ok=true")


if __name__ == "__main__":
    main()
