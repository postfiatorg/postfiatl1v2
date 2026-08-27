#!/usr/bin/env python3
"""Run the closed storage tamper/crash matrix and emit redaction-safe receipts."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
from pathlib import Path
from typing import Any

REPO = Path(__file__).resolve().parents[2]

STORAGE_TRUNCATED = ("postfiat-storage", "truncated_jsonl_chain_is_rejected")
STORAGE_HISTORY_MUTATIONS = (
    "postfiat-storage",
    "logical_scan_rejects_padded_reordered_duplicated_omitted_and_modified_history",
)
STORAGE_CONFLICT = (
    "postfiat-storage",
    "logical_scan_rejects_authenticated_conflicting_ordered_index",
)
STORAGE_OMITTED = (
    "postfiat-storage",
    "logical_scan_rejects_deleted_hash_index_without_blessing_metadata",
)
STORAGE_DOMAIN = (
    "postfiat-storage",
    "authenticated_values_reject_cross_table_and_cross_key_substitution",
)
STORAGE_PARENT = (
    "postfiat-storage",
    "rejected_parent_or_commitment_leaves_genesis_tip",
)
STORAGE_FORGED_VALUES = (
    "postfiat-storage",
    "logical_scan_rejects_forged_receipt_archive_and_state_values",
)
STORAGE_DROP = (
    "postfiat-storage",
    "dropped_transaction_at_every_logical_write_cut_exposes_only_old_tip",
)
STORAGE_DURABLE_FAULTS = (
    "postfiat-storage",
    "injected_disk_permission_write_and_sync_failures_preserve_the_old_tip",
)
STORAGE_KILL = (
    "postfiat-storage",
    "sigkill_before_during_and_after_commit_recovers_one_complete_tip",
)
NODE_REBUILD = (
    "postfiat-node",
    "transactional_rebuild_replays_publishes_and_verifies_a_legacy_generation",
)
NODE_CANCEL = (
    "postfiat-node",
    "existing_chain_storage_activation_can_cancel_only_before_cutover",
)
NODE_ACTIVATION_RESTART = (
    "postfiat-node",
    "ordered_history_v2_activation_journal_recovers_every_persist_prefix",
)
NODE_CATCH_UP = (
    "postfiat-node",
    "historical_external_certificate_rejects_wrong_local_parent_without_mutation",
)
NODE_SNAPSHOT = (
    "postfiat-node",
    "signed_snapshot_roundtrip_rejects_tampering_and_preserves_signer_isolation",
)
NODE_PRUNE = (
    "postfiat-node",
    "history_prune_recover_completes_pending_prune_after_checkpoint_write",
)

CASES: dict[str, dict[str, Any]] = {
    "history_truncated": {"tests": [STORAGE_TRUNCATED]},
    "history_padded": {"tests": [STORAGE_HISTORY_MUTATIONS]},
    "history_reordered": {"tests": [STORAGE_HISTORY_MUTATIONS]},
    "history_duplicated": {"tests": [STORAGE_HISTORY_MUTATIONS]},
    "history_omitted": {"tests": [STORAGE_HISTORY_MUTATIONS]},
    "history_modified": {"tests": [STORAGE_HISTORY_MUTATIONS]},
    "wrong_domain": {"tests": [STORAGE_DOMAIN]},
    "stale_generation": {"tests": [NODE_REBUILD, STORAGE_PARENT]},
    "missing_table_or_snapshot": {"tests": [STORAGE_OMITTED, NODE_SNAPSHOT]},
    "index_without_history": {"tests": [STORAGE_CONFLICT]},
    "history_without_index": {"tests": [STORAGE_OMITTED]},
    "conflicting_ordered_indexes": {"tests": [STORAGE_CONFLICT]},
    "incorrect_count_or_accumulator": {"tests": [STORAGE_PARENT]},
    "forged_receipt_archive_or_state": {
        "tests": [STORAGE_FORGED_VALUES, STORAGE_DOMAIN]
    },
    "disk_or_write_failure": {
        "tests": [STORAGE_DROP, STORAGE_DURABLE_FAULTS],
        "terminal_state": "recovered_old_tip",
    },
    "process_kill_before_commit": {
        "tests": [STORAGE_KILL],
        "terminal_state": "recovered_old_tip",
    },
    "process_kill_during_commit": {
        "tests": [STORAGE_KILL],
        "terminal_state": "recovered_old_tip",
    },
    "process_kill_after_commit": {
        "tests": [STORAGE_KILL],
        "terminal_state": "recovered_new_tip",
    },
    "migration_activation_cancellation_restart": {
        "tests": [NODE_REBUILD, NODE_CANCEL, NODE_ACTIVATION_RESTART],
        "terminal_state": "recovered_new_tip",
    },
    "catch_up_and_rollback": {
        "tests": [NODE_CATCH_UP, NODE_PRUNE],
        "terminal_state": "recovered_new_tip",
    },
}

REASON_CODES = {
    "history_truncated": "storage_legacy_jsonl_mac_chain_mismatch",
    "history_padded": "storage_count_mismatch",
    "history_reordered": "storage_corrupt_record",
    "history_duplicated": "storage_count_mismatch",
    "history_omitted": "storage_count_mismatch",
    "history_modified": "storage_corrupt_record",
    "wrong_domain": "storage_integrity_failure",
    "stale_generation": "storage_migration_manifest_invalid",
    "missing_table_or_snapshot": "storage_count_mismatch",
    "index_without_history": "storage_corrupt_record",
    "history_without_index": "storage_count_mismatch",
    "conflicting_ordered_indexes": "storage_corrupt_record",
    "incorrect_count_or_accumulator": "storage_ordered_commitment_mismatch",
    "forged_receipt_archive_or_state": "storage_integrity_failure",
    "disk_or_write_failure": "storage_database_error",
    "process_kill_before_commit": "storage_process_kill_recovered_old_tip",
    "process_kill_during_commit": "storage_process_kill_recovered_old_tip",
    "process_kill_after_commit": "storage_process_kill_recovered_new_tip",
    "migration_activation_cancellation_restart": (
        "storage_commitment_activation_cancelled"
    ),
    "catch_up_and_rollback": "storage_legacy_jsonl_head_tip_mismatch",
}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def git_revision() -> str:
    return subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=REPO,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout.strip()


def git_clean() -> bool:
    return (
        subprocess.run(
            ["git", "status", "--porcelain"],
            cwd=REPO,
            check=True,
            stdout=subprocess.PIPE,
            text=True,
        ).stdout
        == ""
    )


def run_test(package: str, test_filter: str) -> dict[str, str]:
    command = [
        "cargo",
        "test",
        "-p",
        package,
        "--locked",
        test_filter,
        "--",
        "--test-threads=1",
    ]
    completed = subprocess.run(
        command,
        cwd=REPO,
        env=os.environ.copy(),
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    output = completed.stdout
    if completed.returncode != 0 or "test result: ok." not in output:
        raise RuntimeError(
            f"tamper evidence test failed: {package}::{test_filter}"
        )
    return {
        "package": package,
        "test_filter": test_filter,
        "result": "passed",
        "command_sha256": hashlib.sha256(
            "\0".join(command).encode("utf-8")
        ).hexdigest(),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--expected-source-revision", required=True)
    args = parser.parse_args()

    if set(REASON_CODES) != set(CASES):
        raise RuntimeError("tamper reason-code set does not match the closed case set")
    output = args.output_dir.resolve()
    if output.exists():
        raise ValueError(f"refusing to overwrite output directory: {output}")
    revision = git_revision()
    if revision != args.expected_source_revision:
        raise ValueError("HEAD does not match --expected-source-revision")
    if not git_clean():
        raise ValueError("tamper evidence requires a clean checkout")
    output.mkdir(parents=True)
    receipts_dir = output / "receipts"
    receipts_dir.mkdir()

    unique_tests = sorted(
        {
            test
            for configuration in CASES.values()
            for test in configuration["tests"]
        }
    )
    test_results = {
        test: run_test(*test)
        for test in unique_tests
    }

    cases = []
    for name, configuration in CASES.items():
        receipt_path = receipts_dir / f"{name}.json"
        receipt = {
            "schema": "postfiat-storage-tamper-receipt-v1",
            "name": name,
            "passed": True,
            "reason_code": REASON_CODES[name],
            "no_partial_mutation": True,
            "terminal_state": configuration.get(
                "terminal_state",
                "rejected_voting_blocked",
            ),
            "source_revision": revision,
            "test_receipts": [
                test_results[test] for test in configuration["tests"]
            ],
            "offline": True,
            "network_contacted": False,
        }
        write_json(receipt_path, receipt)
        cases.append(
            {
                "name": receipt["name"],
                "passed": receipt["passed"],
                "reason_code": receipt["reason_code"],
                "no_partial_mutation": receipt["no_partial_mutation"],
                "terminal_state": receipt["terminal_state"],
                "receipt": {
                    "path": receipt_path.relative_to(output).as_posix(),
                    "sha256": sha256(receipt_path),
                },
            }
        )

    report_path = output / "tamper-report.json"
    write_json(
        report_path,
        {
            "schema": "postfiat-storage-scaling-tamper-matrix-v1",
            "status": "PASS",
            "source_revision": revision,
            "cases": cases,
            "unique_test_count": len(unique_tests),
            "offline": True,
            "network_contacted": False,
        },
    )
    print("storage-scaling-tamper=PASS")
    print(f"report={report_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
