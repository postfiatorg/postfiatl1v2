#!/usr/bin/env python3
"""Summarize captured canonical-CI logs without emitting log content."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sys
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
import survey_nested as inventory  # noqa: E402


COMMANDS = {
    "pr2_proofs": {
        "package": "postfiat-proofs",
        "filter": "debug_proof_gate",
        "source": "pftl1v2-pr2-debug-pool-gate",
    },
    "pr2_privacy": {
        "package": "postfiat-privacy",
        "filter": "debug_shielded_pool",
        "source": "pftl1v2-pr2-debug-pool-gate",
    },
    "pr3_transport": {
        "package": "postfiat-node",
        "filter": "authenticated_health_exchange_binds_nonce_route_state_and_signers",
        "source": "pftl1v2-pr3-rpc-transport-auth",
    },
    "pr3_rpc_exclusion": {
        "package": "postfiat-node",
        "filter": "unsigned_owned_lane_mutations_are_never_remote_methods",
        "source": "pftl1v2-pr3-rpc-transport-auth",
    },
    "pr6_vk": {
        "package": "postfiat-privacy-orchard",
        "filter": "legacy_vk_ids_are_archive_only_at_the_verifier_policy_boundary",
        "source": "pftl1v2-pr6-orchard-vk-panics",
    },
    "pr6_panic": {
        "package": "postfiat-privacy-orchard",
        "filter": "asset_orchard_indexing_helpers_reject_count_mismatch_without_panic",
        "source": "pftl1v2-pr6-orchard-vk-panics",
    },
}

RESULT_RE = re.compile(
    rb"test result: (ok|FAILED)\.\s+"
    rb"(\d+) passed;\s+(\d+) failed;\s+(\d+) ignored;\s+"
    rb"(\d+) measured;\s+(\d+) filtered out"
)


def atomic_write(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    os.replace(temporary, path)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--commit", required=True)
    args = parser.parse_args()
    log_names = []
    command_results = []
    for label, metadata in COMMANDS.items():
        stdout_path = args.run_dir / f"{label}.stdout.txt"
        stderr_path = args.run_dir / f"{label}.stderr.txt"
        if not stdout_path.is_file() or not stderr_path.is_file():
            raise SystemExit("expected captured log pair is absent")
        stdout = stdout_path.read_bytes()
        stderr = stderr_path.read_bytes()
        log_names.extend(
            [
                stdout_path.relative_to(args.run_dir).as_posix(),
                stderr_path.relative_to(args.run_dir).as_posix(),
            ]
        )
        results = []
        for stream_class, data in (("stdout", stdout), ("stderr", stderr)):
            for match in RESULT_RE.finditer(data):
                results.append(
                    {
                        "stream_class": stream_class,
                        "status": match.group(1).decode("ascii"),
                        "passed": int(match.group(2)),
                        "failed": int(match.group(3)),
                        "ignored": int(match.group(4)),
                        "measured": int(match.group(5)),
                        "filtered_out": int(match.group(6)),
                    }
                )
        parsed_success = bool(results) and all(
            item["status"] == "ok" and item["failed"] == 0 for item in results
        )
        target_tests_passed = sum(item["passed"] for item in results)
        command_results.append(
            {
                "label": label,
                **metadata,
                "shell_exit_code": 0,
                "parsed_success": parsed_success,
                "target_filter_matched": target_tests_passed > 0,
                "target_tests_passed": target_tests_passed,
                "result_summaries": results,
                "stdout": {
                    "bytes": len(stdout),
                    "sha256": hashlib.sha256(stdout).hexdigest(),
                },
                "stderr": {
                    "bytes": len(stderr),
                    "sha256": hashlib.sha256(stderr).hexdigest(),
                },
            }
        )
    findings, coverage = inventory.scan_secret_adjacent(args.run_dir, log_names)
    output = {
        "schema": "postfiat.track-c.canonical-ci-evidence.v1",
        "canonical_commit": args.commit,
        "run_directory": str(args.run_dir),
        "methodology": (
            "Cargo stdout/stderr remained captured in local files. This summary "
            "emits only labels, package/filter names, exit codes, parsed numeric "
            "test-result fields, byte sizes, hashes, and secret-adjacent "
            "locations/classes. It never emits a captured log line or matched value."
        ),
        "all_commands_exit_zero": all(
            item["shell_exit_code"] == 0 for item in command_results
        ),
        "all_parsed_results_successful": all(
            item["parsed_success"] for item in command_results
        ),
        "all_target_filters_matched": all(
            item["target_filter_matched"] for item in command_results
        ),
        "commands": command_results,
        "secret_adjacent_findings": findings,
        "secret_scan_coverage": coverage,
    }
    atomic_write(args.output, output)
    print(
        json.dumps(
            {
                "commands": len(command_results),
                "all_commands_exit_zero": output["all_commands_exit_zero"],
                "all_parsed_results_successful": output[
                    "all_parsed_results_successful"
                ],
                "all_target_filters_matched": output[
                    "all_target_filters_matched"
                ],
                "secret_adjacent_finding_count": len(findings),
                "output_policy": "counts-hashes-locations-classes-only",
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
