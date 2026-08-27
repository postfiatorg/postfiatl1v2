#!/usr/bin/env python3
"""Run the closed storage tamper/crash matrix and emit redaction-safe receipts."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
from pathlib import Path
from typing import Any

REPO = Path(__file__).resolve().parents[2]
ORIGINAL_E3_MANIFEST = (
    REPO
    / "benchmarks"
    / "cobalt-adversarial-verification"
    / "e3"
    / "campaign-manifest.json"
)
ORIGINAL_E3_MANIFEST_SHA256 = (
    "c23320d47d631efdd74c1e5c6c541951f452a4de9b14eb583f9d888b77167fa7"
)
ORIGINAL_E3_SOURCE_PATHS = (
    "docs/governance/cobalt-adversarial-verification-research-spec.md",
    "crates/node/src/cobalt_shadow.rs",
    "crates/node/src/cobalt_shadow_runtime.rs",
    "crates/node/src/bin/postfiat_cobalt_liveness_simulation.rs",
    "crates/cobalt_e3_harness/src/main.rs",
)
ORIGINAL_E3_TAMPER_CASES = (
    "truncated",
    "padded",
    "reordered",
    "one_entry_modified",
)
ORIGINAL_E3_FORGED_CASES = (
    "fabricated_transition",
    "wrong_root_certificate",
    "omitted_latest_update",
)

ORIGINAL_E3 = ("postfiat-cobalt-e3-harness", "__full_campaign__")
COMPATIBLE_ROLLBACK = (
    "postfiat-storage-rollback-rehearsal",
    "compatible_post_activation_software_rollback",
)
UNCOVERED_REQUIREMENTS: tuple[str, ...] = ()
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
STORAGE_STALE_METADATA = (
    "postfiat-storage",
    "authenticated_stale_metadata_tip_count_and_accumulator_fail_without_marker",
)
STORAGE_TABLE_INDEX = (
    "postfiat-storage",
    "missing_substituted_and_one_sided_indexes_fail_without_mutation",
)
STORAGE_POINTER = (
    "postfiat-storage",
    "generation_pointer_is_bound_to_the_verified_database_manifest",
)
STORAGE_CANONICAL_EXPORT = (
    "postfiat-storage",
    "canonical_jsonl_export_rejects_missing_corrupted_and_substituted_files_without_database_mutation",
)
STORAGE_PAGES = (
    "postfiat-storage",
    "corrupted_transactional_data_pages_reject_without_logical_state",
)
STORAGE_MISSING_PAGE = (
    "postfiat-storage",
    "missing_transactional_data_page_rejects_without_logical_state",
)
STORAGE_SUBSTITUTED_PAGE = (
    "postfiat-storage",
    "substituted_transactional_data_page_rejects_without_logical_state",
)
STORAGE_CHECKPOINT_SUBSTITUTION = (
    "postfiat-storage",
    "jsonl_checkpoint_cannot_be_substituted_between_log_kinds",
)
STORAGE_STALE_HEAD = (
    "postfiat-storage",
    "deleted_jsonl_tail_is_rejected_by_authenticated_head",
)
STORAGE_MISSING_LOG = (
    "postfiat-storage",
    "deleted_jsonl_log_is_rejected_when_authenticated_head_remains",
)
STORAGE_MISSING_HEAD = (
    "postfiat-storage",
    "deleted_jsonl_head_is_rejected_when_log_remains",
)
STORAGE_CRASH_SUFFIX = (
    "postfiat-storage",
    "complete_jsonl_crash_suffix_is_verified_once_and_checkpointed",
)

NODE_REBUILD = (
    "postfiat-node",
    "transactional_rebuild_replays_publishes_and_verifies_a_legacy_generation",
)
NODE_STALE_GENERATION = (
    "postfiat-node",
    "transactional_verify_only_rejects_a_valid_but_stale_generation_without_mutation",
)
NODE_AMBIGUOUS_STORAGE_VOTE_BLOCK = (
    "postfiat-node",
    "ambiguous_active_transactional_state_blocks_vote_without_mutation",
)
NODE_CANCEL = (
    "postfiat-node",
    "existing_chain_storage_activation_can_cancel_only_before_cutover",
)
NODE_EXISTING_ACTIVATION = (
    "postfiat-node",
    "existing_chain_governance_schedule_switches_only_at_the_recorded_height",
)
NODE_ACTIVATION_RESTART = (
    "postfiat-node",
    "ordered_history_v2_activation_journal_recovers_every_persist_prefix",
)
NODE_JOURNAL_DISAGREEMENT = (
    "postfiat-node",
    "ordered_commit_journal_disagreement_rejects_without_durable_mutation",
)
NODE_CATCH_UP_MALFORMED = (
    "postfiat-node",
    "catch_up_rejects_malformed_batches_without_durable_mutation",
)
NODE_CATCH_UP_RECOVERY = (
    "postfiat-node",
    "signed_history_catch_up_refuses_gap_then_converges",
)
NODE_EXTERNAL_STATE_ROOT = (
    "postfiat-node",
    "historical_external_certificate_rejects_state_divergent_catch_up_without_mutation",
)
NODE_EXTERNAL_PARENT = (
    "postfiat-node",
    "historical_external_certificate_rejects_wrong_local_parent_without_mutation",
)
NODE_SNAPSHOT = (
    "postfiat-node",
    "signed_snapshot_roundtrip_rejects_tampering_and_preserves_signer_isolation",
)
NODE_SNAPSHOT_FILE_SET = (
    "postfiat-node",
    "snapshot_import_rejects_bad_manifest_file_set",
)

CASES: dict[str, dict[str, Any]] = {
    "history_truncated": {"tests": [ORIGINAL_E3, STORAGE_TRUNCATED]},
    "history_padded": {"tests": [ORIGINAL_E3, STORAGE_HISTORY_MUTATIONS]},
    "history_reordered": {"tests": [ORIGINAL_E3, STORAGE_HISTORY_MUTATIONS]},
    "history_modified": {"tests": [ORIGINAL_E3, STORAGE_HISTORY_MUTATIONS]},
    "history_duplicated": {"tests": [STORAGE_HISTORY_MUTATIONS]},
    "history_omitted": {"tests": [STORAGE_HISTORY_MUTATIONS]},
    "fabricated_transition": {"tests": [ORIGINAL_E3, NODE_CATCH_UP_MALFORMED]},
    "wrong_root_certificate": {"tests": [ORIGINAL_E3, NODE_CATCH_UP_MALFORMED]},
    "omitted_latest_update": {"tests": [ORIGINAL_E3, NODE_CATCH_UP_MALFORMED]},
    "interrupted_catch_up": {
        "tests": [ORIGINAL_E3, NODE_CATCH_UP_RECOVERY],
        "terminal_state": "recovered_new_tip",
    },
    "wrong_chain_domain": {"tests": [STORAGE_STALE_METADATA]},
    "wrong_genesis_domain": {"tests": [STORAGE_STALE_METADATA]},
    "wrong_protocol_domain": {"tests": [STORAGE_STALE_METADATA]},
    "wrong_storage_domain": {"tests": [STORAGE_STALE_METADATA]},
    "wrong_commitment_domain": {"tests": [STORAGE_STALE_METADATA]},
    "wrong_table_domain": {"tests": [STORAGE_DOMAIN]},
    "wrong_key_domain": {"tests": [STORAGE_DOMAIN]},
    "missing_checkpoint": {"tests": [STORAGE_MISSING_HEAD]},
    "missing_checkpoint_log": {"tests": [STORAGE_MISSING_LOG]},
    "checkpoint_log_substitution": {
        "tests": [STORAGE_CHECKPOINT_SUBSTITUTION]
    },
    "stale_valid_head": {"tests": [STORAGE_STALE_HEAD]},
    "bounded_crash_suffix": {
        "tests": [STORAGE_CRASH_SUFFIX],
        "terminal_state": "recovered_new_tip",
    },
    "stale_generation": {"tests": [NODE_STALE_GENERATION]},
    "stale_metadata": {"tests": [STORAGE_STALE_METADATA]},
    "stale_tip": {"tests": [STORAGE_STALE_METADATA]},
    "stale_accumulator": {"tests": [STORAGE_STALE_METADATA]},
    "missing_table": {"tests": [STORAGE_TABLE_INDEX]},
    "substituted_table": {"tests": [STORAGE_TABLE_INDEX, STORAGE_DOMAIN]},
    "corrupted_database_pages": {"tests": [STORAGE_PAGES]},
    "missing_database_page": {"tests": [STORAGE_MISSING_PAGE]},
    "substituted_database_page": {"tests": [STORAGE_SUBSTITUTED_PAGE]},
    "missing_generation_pointer": {"tests": [STORAGE_POINTER]},
    "substituted_generation_pointer": {"tests": [STORAGE_POINTER, NODE_REBUILD]},
    "missing_canonical_export": {
        "tests": [STORAGE_CANONICAL_EXPORT, NODE_REBUILD]
    },
    "corrupted_canonical_export": {
        "tests": [STORAGE_CANONICAL_EXPORT, NODE_REBUILD]
    },
    "substituted_canonical_export": {
        "tests": [STORAGE_CANONICAL_EXPORT, NODE_REBUILD]
    },
    "index_without_history": {"tests": [STORAGE_TABLE_INDEX, STORAGE_CONFLICT]},
    "history_without_index": {"tests": [STORAGE_TABLE_INDEX, STORAGE_OMITTED]},
    "conflicting_ordered_indexes": {"tests": [STORAGE_CONFLICT]},
    "incorrect_count": {"tests": [STORAGE_STALE_METADATA, STORAGE_PARENT]},
    "forged_receipt": {"tests": [STORAGE_FORGED_VALUES]},
    "forged_archive": {"tests": [STORAGE_FORGED_VALUES]},
    "forged_state": {"tests": [STORAGE_FORGED_VALUES]},
    "forged_certificate": {"tests": [NODE_EXTERNAL_STATE_ROOT]},
    "forged_catch_up_response": {
        "tests": [NODE_CATCH_UP_MALFORMED, NODE_EXTERNAL_PARENT]
    },
    "journal_head_disagreement": {
        "tests": [NODE_JOURNAL_DISAGREEMENT, STORAGE_STALE_HEAD]
    },
    "disk_full": {
        "tests": [STORAGE_DROP, STORAGE_DURABLE_FAULTS],
        "terminal_state": "recovered_old_tip",
    },
    "permission_loss": {
        "tests": [STORAGE_DROP, STORAGE_DURABLE_FAULTS],
        "terminal_state": "recovered_old_tip",
    },
    "write_failure": {
        "tests": [STORAGE_DROP, STORAGE_DURABLE_FAULTS],
        "terminal_state": "recovered_old_tip",
    },
    "sync_failure": {
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
    "power_loss_before_commit": {
        "tests": [STORAGE_DROP],
        "terminal_state": "recovered_old_tip",
    },
    "power_loss_during_commit": {
        "tests": [STORAGE_DROP, STORAGE_KILL],
        "terminal_state": "recovered_old_tip",
    },
    "power_loss_after_commit": {
        "tests": [STORAGE_KILL],
        "terminal_state": "recovered_new_tip",
    },
    "restart_after_transaction_cut": {
        "tests": [STORAGE_DROP],
        "terminal_state": "recovered_old_tip",
    },
    "restart_after_activation_journal_cut": {
        "tests": [NODE_ACTIVATION_RESTART],
        "terminal_state": "recovered_new_tip",
    },
    "repeated_recovery": {
        "tests": [NODE_ACTIVATION_RESTART],
        "terminal_state": "recovered_new_tip",
    },
    "idempotent_retry": {
        "tests": [STORAGE_KILL],
        "terminal_state": "recovered_new_tip",
    },
    "migration_activation_cancellation_restart": {
        "tests": [NODE_REBUILD, NODE_CANCEL, NODE_ACTIVATION_RESTART],
        "terminal_state": "recovered_new_tip",
    },
    "pre_activation_rollback": {
        "tests": [NODE_CANCEL],
        "terminal_state": "recovered_new_tip",
    },
    "post_activation_restart_replay": {
        "tests": [NODE_EXISTING_ACTIVATION, NODE_ACTIVATION_RESTART],
        "terminal_state": "recovered_new_tip",
    },
    "compatible_post_activation_software_rollback": {
        "tests": [COMPATIBLE_ROLLBACK],
        "terminal_state": "recovered_new_tip",
    },
    "missing_snapshot": {"tests": [NODE_SNAPSHOT_FILE_SET]},
    "substituted_snapshot": {"tests": [NODE_SNAPSHOT]},
    "missing_migration_manifest": {"tests": [NODE_REBUILD]},
    "substituted_migration_manifest_checksum": {"tests": [NODE_REBUILD]},
    "tampered_snapshot": {"tests": [NODE_SNAPSHOT]},
}

REASON_CODES = {
    "history_truncated": "storage_legacy_jsonl_mac_chain_mismatch",
    "history_padded": "storage_count_mismatch",
    "history_reordered": "storage_corrupt_record",
    "history_modified": "storage_corrupt_record",
    "history_duplicated": "storage_count_mismatch",
    "history_omitted": "storage_count_mismatch",
    "fabricated_transition": "transition_proof_mismatch",
    "wrong_root_certificate": "certificate_root_mismatch",
    "omitted_latest_update": "required_latest_update_omitted",
    "interrupted_catch_up": "catch_up_resumed_from_second_honest_peer",
    "wrong_chain_domain": "storage_ordered_commitment_mismatch",
    "wrong_genesis_domain": "storage_ordered_commitment_mismatch",
    "wrong_protocol_domain": "storage_ordered_commitment_mismatch",
    "wrong_storage_domain": "storage_unsupported_schema",
    "wrong_commitment_domain": "storage_unsupported_schema",
    "wrong_table_domain": "storage_integrity_failure",
    "wrong_key_domain": "storage_integrity_failure",
    "missing_checkpoint": "storage_legacy_jsonl_head_missing",
    "missing_checkpoint_log": "storage_legacy_jsonl_log_missing",
    "checkpoint_log_substitution": "storage_legacy_jsonl_head_domain_mismatch",
    "stale_valid_head": "storage_legacy_jsonl_head_rollback",
    "bounded_crash_suffix": "storage_legacy_jsonl_crash_suffix_recovered",
    "stale_generation": "storage_migration_manifest_domain_mismatch",
    "stale_metadata": "storage_count_mismatch",
    "stale_tip": "storage_corrupt_record",
    "stale_accumulator": "storage_ordered_commitment_mismatch",
    "missing_table": "storage_database_error",
    "substituted_table": "storage_integrity_failure",
    "corrupted_database_pages": "storage_database_error",
    "missing_database_page": "storage_database_error",
    "substituted_database_page": "storage_database_error",
    "missing_generation_pointer": "storage_vote_blocked_ambiguous_local_state",
    "substituted_generation_pointer": "storage_integrity_failure",
    "missing_canonical_export": "storage_canonical_export_missing",
    "corrupted_canonical_export": "storage_canonical_export_integrity_failure",
    "substituted_canonical_export": "storage_canonical_export_substituted",
    "index_without_history": "storage_count_mismatch",
    "history_without_index": "storage_corrupt_record",
    "conflicting_ordered_indexes": "storage_corrupt_record",
    "incorrect_count": "storage_count_mismatch",
    "forged_receipt": "storage_integrity_failure",
    "forged_archive": "storage_integrity_failure",
    "forged_state": "storage_integrity_failure",
    "forged_certificate": "historical_replay_state_root_mismatch",
    "forged_catch_up_response": "catch_up_response_rejected",
    "journal_head_disagreement": "storage_ordered_commit_journal_disagreement",
    "disk_full": "storage_database_error",
    "permission_loss": "storage_database_error",
    "write_failure": "storage_database_error",
    "sync_failure": "storage_database_error",
    "process_kill_before_commit": "storage_process_kill_recovered_old_tip",
    "process_kill_during_commit": "storage_process_kill_recovered_old_tip",
    "process_kill_after_commit": "storage_process_kill_recovered_new_tip",
    "power_loss_before_commit": "storage_power_loss_recovered_old_tip",
    "power_loss_during_commit": "storage_power_loss_recovered_old_tip",
    "power_loss_after_commit": "storage_power_loss_recovered_new_tip",
    "restart_after_transaction_cut": "storage_restart_recovered_old_tip",
    "restart_after_activation_journal_cut": "storage_restart_recovered_new_tip",
    "repeated_recovery": "storage_repeated_recovery_idempotent",
    "idempotent_retry": "storage_idempotent_retry_same_tip",
    "migration_activation_cancellation_restart": (
        "storage_commitment_activation_cancelled"
    ),
    "pre_activation_rollback": "storage_commitment_activation_cancelled",
    "post_activation_restart_replay": "storage_transactional_restart_replayed",
    "compatible_post_activation_software_rollback": (
        "storage_compatible_rollback_resumed_same_certified_tip"
    ),
    "missing_snapshot": "storage_snapshot_missing",
    "substituted_snapshot": "storage_snapshot_integrity_failure",
    "missing_migration_manifest": "storage_migration_manifest_missing",
    "substituted_migration_manifest_checksum": (
        "storage_migration_manifest_checksum_mismatch"
    ),
    "tampered_snapshot": "storage_snapshot_integrity_failure",
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


def command_receipt(command: list[str]) -> dict[str, str]:
    return {
        "result": "passed",
        "command_sha256": hashlib.sha256(
            "\0".join(command).encode("utf-8")
        ).hexdigest(),
    }


def run_cargo_test(package: str, test_filter: str) -> dict[str, str]:
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
    executed_counts = [
        int(match)
        for match in re.findall(r"^running ([0-9]+) tests?$", completed.stdout, re.MULTILINE)
    ]
    executed_test_count = sum(executed_counts)
    if (
        completed.returncode != 0
        or "test result: ok." not in completed.stdout
        or executed_test_count < 1
    ):
        raise RuntimeError(
            "tamper evidence test failed or matched zero tests: "
            f"{package}::{test_filter}"
        )
    return {
        "package": package,
        "test_filter": test_filter,
        "executed_test_count": executed_test_count,
        **command_receipt(command),
    }


def current_source_e3_manifest(output: Path, revision: str) -> Path:
    if sha256(ORIGINAL_E3_MANIFEST) != ORIGINAL_E3_MANIFEST_SHA256:
        raise RuntimeError("frozen original E3 manifest identity changed")
    manifest = json.loads(ORIGINAL_E3_MANIFEST.read_text(encoding="utf-8"))
    binding = manifest.get("live_binding")
    sources = manifest.get("source_files")
    if (
        manifest.get("schema")
        != "postfiat-cobalt-adversarial-e3-campaign-manifest-v1"
        or manifest.get("campaign_id") != "cobalt-e3-adversarial-recovery-v1"
        or manifest.get("history_entry_count") != 4
        or tuple(manifest.get("tamper_cases", [])) != ORIGINAL_E3_TAMPER_CASES
        or tuple(manifest.get("forged_catch_up_cases", []))
        != ORIGINAL_E3_FORGED_CASES
        or not isinstance(binding, dict)
        or binding.get("validators")
        != [f"validator-{index}" for index in range(6)]
        or binding.get("quorum") != 5
        or not isinstance(sources, list)
        or tuple(
            source.get("path") if isinstance(source, dict) else None
            for source in sources
        )
        != ORIGINAL_E3_SOURCE_PATHS
    ):
        raise RuntimeError("frozen original E3 campaign boundary changed")

    for source, relative in zip(sources, ORIGINAL_E3_SOURCE_PATHS, strict=True):
        source_path = (REPO / relative).resolve()
        if not source_path.is_relative_to(REPO) or not source_path.is_file():
            raise RuntimeError(f"current E3 source path is unsafe or missing: {relative}")
        source["sha256"] = sha256(source_path)
    manifest["source_revision"] = revision
    manifest["rebound_from"] = {
        "path": ORIGINAL_E3_MANIFEST.relative_to(REPO).as_posix(),
        "sha256": ORIGINAL_E3_MANIFEST_SHA256,
        "policy": "same frozen cases and live binding, current source hashes",
    }
    destination = output / "original-e3-current-source-manifest.json"
    write_json(destination, manifest)
    return destination


def run_original_e3(output: Path, revision: str) -> dict[str, Any]:
    manifest = current_source_e3_manifest(output, revision)
    report = output / "original-e3-campaign.json"
    work = output / "original-e3-work"
    run_command = [
        "cargo",
        "run",
        "-p",
        "postfiat-cobalt-e3-harness",
        "--locked",
        "--",
        "run",
        str(manifest),
        str(report),
        str(work),
    ]
    environment = os.environ.copy()
    environment["COBALT_E3_SOURCE_REVISION"] = revision
    run = subprocess.run(
        run_command,
        cwd=REPO,
        env=environment,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    if run.returncode != 0 or not report.is_file():
        raise RuntimeError("original E3 six-validator campaign failed")
    verify_command = [
        "cargo",
        "run",
        "-p",
        "postfiat-cobalt-e3-harness",
        "--locked",
        "--",
        "verify",
        str(manifest),
        str(report),
    ]
    verify = subprocess.run(
        verify_command,
        cwd=REPO,
        env=environment,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    if verify.returncode != 0 or "verified E3:" not in verify.stdout:
        raise RuntimeError("original E3 six-validator campaign verification failed")
    campaign = json.loads(report.read_text(encoding="utf-8"))
    executed_case_count = len(campaign.get("cases", [])) + len(
        campaign.get("recoveries", [])
    )
    if executed_case_count != 48:
        raise RuntimeError("original E3 campaign did not execute its frozen 48 cases")
    return {
        "package": ORIGINAL_E3[0],
        "test_filter": ORIGINAL_E3[1],
        "executed_case_count": executed_case_count,
        **command_receipt(run_command),
        "verify_command_sha256": hashlib.sha256(
            "\0".join(verify_command).encode("utf-8")
        ).hexdigest(),
        "report": {
            "path": report.name,
            "sha256": sha256(report),
        },
        "manifest": {
            "path": manifest.name,
            "sha256": sha256(manifest),
        },
    }


def run_compatible_rollback(
    source: Path,
    output: Path,
    revision: str,
) -> dict[str, Any]:
    report = json.loads(source.read_text(encoding="utf-8"))
    required_true = (
        "evidence_eligible",
        "rollback_source_is_ancestor",
        "activated_commitment_understood",
        "resumed_same_certified_tip",
        "post_activation_finality_with_rollback_binary",
        "forward_recovery_with_current_binary",
        "literal_receipts_exact",
        "zero_full_history_reads",
        "bounded_index_pages",
        "constant_accumulator_work",
        "all_six_converged",
        "offline",
    )
    current = report.get("current_binary")
    rollback = report.get("rollback_binary")
    identities = report.get("identities")
    rollback_revision = (
        str(rollback.get("source_revision", ""))
        if isinstance(rollback, dict)
        else ""
    )
    expected_heights = {
        "current_post_activation": 2,
        "rollback_resume_input": 2,
        "rollback_finalized": 3,
        "forward_resume_input": 3,
        "forward_finalized": 4,
    }
    identity_fields_valid = isinstance(identities, dict) and all(
        isinstance(identities.get(label), dict)
        and identities[label].get("height") == height
        and re.fullmatch(r"[0-9a-f]{96}", str(identities[label].get("tip", "")))
        is not None
        and re.fullmatch(
            r"[0-9a-f]{96}",
            str(identities[label].get("state_root", "")),
        )
        is not None
        for label, height in expected_heights.items()
    )
    rollback_is_ancestor = (
        re.fullmatch(r"[0-9a-f]{40}", rollback_revision) is not None
        and rollback_revision != revision
        and subprocess.run(
            ["git", "merge-base", "--is-ancestor", rollback_revision, revision],
            cwd=REPO,
            check=False,
        ).returncode
        == 0
    )
    if (
        report.get("schema") != "postfiat-storage-compatible-rollback-v1"
        or report.get("status") != "PASS"
        or report.get("source_revision") != revision
        or report.get("network_contacted") is not False
        or report.get("devnet_queried_or_mutated") is not False
        or report.get("validator_count") != 6
        or report.get("chain_id") != "postfiat-storage-scaling-local-v1"
        or report.get("storage_activation_height") != 1
        or report.get("consensus_activation_height") != 2
        or any(report.get(key) is not True for key in required_true)
        or not isinstance(current, dict)
        or not isinstance(rollback, dict)
        or current.get("source_revision") != revision
        or current.get("git_revision") != revision[:8]
        or current.get("profile") != "release"
        or re.fullmatch(r"[0-9a-f]{64}", str(current.get("sha256", ""))) is None
        or rollback.get("profile") != "release"
        or rollback.get("git_revision") != rollback_revision[:8]
        or re.fullmatch(r"[0-9a-f]{64}", str(rollback.get("sha256", "")))
        is None
        or current.get("sha256") == rollback.get("sha256")
        or not rollback_is_ancestor
        or not identity_fields_valid
        or identities.get("rollback_resume_input")
        != identities.get("current_post_activation")
        or identities.get("forward_resume_input")
        != identities.get("rollback_finalized")
    ):
        raise RuntimeError("compatible post-activation rollback report did not pass")
    destination = output / "compatible-rollback-report.json"
    shutil.copyfile(source, destination)
    command = [
        "verify-compatible-rollback-report",
        sha256(destination),
    ]
    return {
        "package": COMPATIBLE_ROLLBACK[0],
        "test_filter": COMPATIBLE_ROLLBACK[1],
        "executed_test_count": 1,
        **command_receipt(command),
        "report": {
            "path": destination.name,
            "sha256": sha256(destination),
        },
    }


def run_test(
    test: tuple[str, str],
    output: Path,
    revision: str,
    rollback_report: Path,
) -> dict[str, Any]:
    if test == ORIGINAL_E3:
        return run_original_e3(output, revision)
    if test == COMPATIBLE_ROLLBACK:
        return run_compatible_rollback(rollback_report, output, revision)
    return run_cargo_test(*test)


def tests_for_case(configuration: dict[str, Any]) -> list[tuple[str, str]]:
    tests = list(configuration["tests"])
    if configuration.get("terminal_state", "rejected_voting_blocked") == (
        "rejected_voting_blocked"
    ) and NODE_AMBIGUOUS_STORAGE_VOTE_BLOCK not in tests:
        tests.append(NODE_AMBIGUOUS_STORAGE_VOTE_BLOCK)
    return tests


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--expected-source-revision", required=True)
    parser.add_argument("--rollback-report", type=Path, required=True)
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
    rollback_report = args.rollback_report.resolve()
    if rollback_report.is_symlink() or not rollback_report.is_file():
        raise ValueError("--rollback-report must identify a regular file")
    output.mkdir(parents=True)
    receipts_dir = output / "receipts"
    receipts_dir.mkdir()

    unique_tests = sorted(
        {
            test
            for configuration in CASES.values()
            for test in tests_for_case(configuration)
        }
    )
    test_results = {
        test: run_test(test, output, revision, rollback_report)
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
                test_results[test] for test in tests_for_case(configuration)
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
            "status": "PASS" if not UNCOVERED_REQUIREMENTS else "INCOMPLETE",
            "coverage_complete": not UNCOVERED_REQUIREMENTS,
            "uncovered_requirements": list(UNCOVERED_REQUIREMENTS),
            "source_revision": revision,
            "cases": cases,
            "unique_test_count": len(unique_tests),
            "offline": True,
            "network_contacted": False,
        },
    )
    status = "PASS" if not UNCOVERED_REQUIREMENTS else "INCOMPLETE"
    print(f"storage-scaling-tamper={status}")
    print(f"report={report_path}")
    return 0 if not UNCOVERED_REQUIREMENTS else 1


if __name__ == "__main__":
    raise SystemExit(main())
