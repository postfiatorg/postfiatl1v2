"""Offline verifier and read-only browser for storage-scaling evidence packets.

The verifier never opens a socket. The optional browser serves only the report
produced by a successful offline verification and clearly separates recorded
live state, deployed lineage, and repository state.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import sys
from dataclasses import dataclass
from datetime import datetime
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path, PurePosixPath
from typing import Any, Mapping, Sequence

PACKET_SCHEMA = "postfiat-storage-scaling-evidence-packet-v1"
REPORT_SCHEMA = "postfiat-storage-scaling-verification-v1"
MANIFEST_FILE = "storage-scaling-packet.json"
CHECKSUM_FILE = "SHA256SUMS.txt"
HEIGHTS = [50, 5000]
CONTROLLED_CHAIN_ID = "postfiat-wan-devnet-2"
CONTROLLED_GENESIS_HASH = (
    "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad255"
    "21aff3ed334da07e150a7233a3e90a9"
)
MATERIAL_STAGE_PATHS = {
    "proposal_ms": ("round_timings", "proposal_ms"),
    "verification_ms": ("round_timings", "verification_ms"),
    "vote_requests_ms": ("round_timings", "vote_requests_ms"),
    "local_vote_ms": ("round_timings", "local_vote_ms"),
    "certificate_ms": ("round_timings", "certificate_ms"),
    "local_apply_ms": ("round_timings", "local_apply_ms"),
    "certified_sends_ms": ("round_timings", "certified_sends_ms"),
    "post_apply_status_ms": ("round_timings", "post_apply_status_ms"),
    "local_commit_publish_ms": ("round_timings", "local_commit_publish_ms"),
    "write_commit_ms": ("round_timings", "local_apply_breakdown", "write_commit_ms"),
}
MODEL_RELATIVE_MATERIALITY = 0.10
MODEL_RESIDUAL_SIGMAS = 2.0
PERFORMANCE_LANES = ("selected-indexed", "legacy-jsonl")
PERFORMANCE_LANE_HEIGHTS = {
    "selected-indexed": [50, 5000],
    "legacy-jsonl": [50],
}
PERFORMANCE_STORAGE_BEHAVIORS = {
    "legacy-jsonl": (
        "authenticated JSONL with full-prefix append verification and full "
        "ordered-history proposal work"
    ),
    "selected-indexed": (
        "transactional redb finality path with the fixed-size accumulator"
    ),
}
PERFORMANCE_BACKEND_MODES = {
    "legacy-jsonl": "legacy-jsonl",
    "selected-indexed": "transactional",
}
TRANSACTIONAL_COUNTER_FIELDS = (
    "read_transactions",
    "write_transactions",
    "committed_write_transactions",
    "records_read",
    "records_written",
    "bytes_read",
    "bytes_written",
    "page_reads",
    "page_writes",
    "full_history_scans",
    "full_history_records_read",
    "full_history_bytes_read",
    "durable_commit_micros",
)
LEGACY_COUNTER_FIELDS = (
    "jsonl_append_calls",
    "checkpoint_bytes_read",
    "crash_suffix_bytes_read",
    "crash_suffix_records_verified",
    "legacy_prefix_bytes_read",
    "legacy_prefix_records_verified",
    "ordered_history_bytes_read",
    "ordered_history_records_read",
    "ordered_index_bitmap_bytes_read",
    "ordered_index_bitmap_bytes_written",
    "ordered_index_slots_read",
    "ordered_index_slots_written",
)
SIGNED_TRANSFER_CORPUS_SCHEMA = "postfiat-tx-latency-signed-transfer-corpus-v1"
RESOURCE_SAMPLE_SCHEMA = "postfiat-storage-resource-samples-v1"
RESOURCE_SAMPLE_TARGET_INTERVAL_MS = 100
PREPARED_INPUT_MANIFEST_SCHEMA = "postfiat-storage-prepared-input-manifest-v1"
PREPARED_BUILD_COUNTER_FIELDS = (
    "committed_write_transactions",
    "page_reads",
    "page_writes",
    "full_history_scans",
    "full_history_records_read",
    "full_history_bytes_read",
)
PREPARED_BUILD_ZERO_COUNTER_FIELDS = (
    "full_history_scans",
    "full_history_records_read",
    "full_history_bytes_read",
)
VOTE_LOCK_WORK_GATE_SCHEMA = "postfiat-storage-vote-lock-work-gate-v1"
VOTE_LOCK_MAX_FILES_EXAMINED = 3
VOTE_LOCK_MAX_BYTES_DECODED = 4_096
VOTE_LOCK_REASON_MIGRATION_REPEATED = "VOTE_LOCK_MIGRATION_REPEATED"
VOTE_LOCK_REASON_MIGRATION_LATE = (
    "VOTE_LOCK_MIGRATION_AFTER_FIRST_FINALIZED_ROUND"
)
VOTE_LOCK_REASON_FILES_EXCEEDED = "VOTE_LOCK_FILES_EXAMINED_EXCEEDED"
VOTE_LOCK_REASON_BYTES_EXCEEDED = "VOTE_LOCK_BYTES_DECODED_EXCEEDED"
PERFORMANCE_QUALIFICATION_TIMEOUT_MS = 900_000
MAX_PROPOSAL_PAGE_READS_PER_ROUND = 64
MAX_APPLY_PAGE_READS_PER_VALIDATOR_ROUND = 64
MAX_APPLY_PAGE_WRITES_PER_VALIDATOR_ROUND = 32
PERFORMANCE_RESOURCE_FIELDS = (
    "cpu_ticks",
    "peak_rss_kib",
    "disk_growth_bytes",
    "bytes_read",
    "bytes_written",
    "sample_count",
    "duration_ms",
    "observed_pid_count",
    "foreground_process_count",
    "foreground_min_sample_count",
    "host_cpu_ticks",
    "host_total_memory_kib",
    "host_min_available_memory_kib",
    "network_received_bytes",
    "network_transmitted_bytes",
)
MAX_PACKET_FILES = 4096
MAX_FILE_BYTES = 512 * 1024 * 1024
HEX40 = re.compile(r"[0-9a-f]{40}")
HEX64 = re.compile(r"[0-9a-f]{64}")
HEX96 = re.compile(r"[0-9a-f]{96}")
SENSITIVE = re.compile(
    r"private[-_ ]?key(?![A-Za-z0-9_])|secret|password|mnemonic|spending[-_ ]?key|"
    r"full[-_ ]?viewing[-_ ]?key|master[-_ ]?seed|rseed|ssh[-_ ]?cred",
    re.IGNORECASE,
)
LOCAL_PATH = re.compile(r"(?:/home/|/Users/|[A-Za-z]:\\\\Users\\\\)")
NONLOCAL_IPV4 = re.compile(
    r"(?<![0-9.])(?!127\.0\.0\.1\b)(?:[0-9]{1,3}\.){3}[0-9]{1,3}(?![0-9.])"
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
ARTIFACT_SCHEMAS = {
    "source": "postfiat-storage-source-identity-v1",
    "replay": "postfiat-storage-scaling-replay-v1",
    "performance": "postfiat-storage-scaling-time-budgeted-six-validator-campaign-v4",
    "tamper": "postfiat-storage-scaling-tamper-matrix-v1",
    "migration": "postfiat-storage-scaling-six-clone-migration-v1",
    "redaction": "postfiat-storage-scaling-redaction-v1",
}
REQUIRED_TAMPER_REASONS = {
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
REQUIRED_TAMPER_CASES = set(REQUIRED_TAMPER_REASONS)


class StorageScalingVerificationError(ValueError):
    """A packet failed a closed verification gate."""


@dataclass(frozen=True)
class VerifiedPacket:
    packet_dir: Path
    manifest: Mapping[str, Any]
    report: Mapping[str, Any]


def _fail(message: str) -> None:
    raise StorageScalingVerificationError(message)


def _object(value: Any, label: str) -> Mapping[str, Any]:
    if not isinstance(value, dict):
        _fail(f"{label} must be an object")
    return value


def _list(value: Any, label: str) -> list[Any]:
    if not isinstance(value, list):
        _fail(f"{label} must be an array")
    return value


def _bool(value: Any, label: str) -> bool:
    if value is not True:
        _fail(f"{label} must be true")
    return True


def _bounded_read(path: Path) -> bytes:
    if path.is_symlink() or not path.is_file():
        _fail(f"packet entry is not a regular file: {path.name}")
    size = path.stat().st_size
    if size > MAX_FILE_BYTES:
        _fail(f"packet entry exceeds the byte limit: {path.name}")
    return path.read_bytes()


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _safe_relative(name: str) -> Path:
    pure = PurePosixPath(name)
    if pure.is_absolute() or not pure.parts or any(part in {"", ".", ".."} for part in pure.parts):
        _fail(f"unsafe checksum path: {name!r}")
    return Path(*pure.parts)


def _verify_checksums(packet_dir: Path) -> tuple[dict[str, str], str]:
    checksum_path = packet_dir / CHECKSUM_FILE
    raw = _bounded_read(checksum_path).decode("utf-8")
    entries: dict[str, str] = {}
    for line in raw.splitlines():
        digest, separator, name = line.partition("  ")
        if not separator or HEX64.fullmatch(digest) is None:
            _fail(f"malformed checksum line: {line!r}")
        relative = _safe_relative(name).as_posix()
        if relative == CHECKSUM_FILE or relative in entries:
            _fail(f"invalid or duplicate checksum entry: {relative}")
        entries[relative] = digest
    if not entries or len(entries) > MAX_PACKET_FILES:
        _fail("checksum entry count is outside the closed bound")
    actual_files = {
        path.relative_to(packet_dir).as_posix()
        for path in packet_dir.rglob("*")
        if path.is_file() and path.name != CHECKSUM_FILE
    }
    if actual_files != set(entries):
        _fail("checksum manifest does not exactly cover packet files")
    for name, expected in entries.items():
        path = packet_dir / _safe_relative(name)
        _bounded_read(path)
        observed = _sha256(path)
        if observed != expected:
            _fail(f"checksum mismatch for {name}")
    return entries, hashlib.sha256(raw.encode("utf-8")).hexdigest()


def _load_manifest(packet_dir: Path) -> Mapping[str, Any]:
    try:
        decoded = json.loads(_bounded_read(packet_dir / MANIFEST_FILE))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        _fail(f"manifest is not canonical JSON: {error}")
    manifest = _object(decoded, "manifest")
    if manifest.get("schema") != PACKET_SCHEMA:
        _fail("unsupported packet schema")
    if manifest.get("status") != "PASS":
        _fail("packet status is not PASS")
    captured_at = manifest.get("captured_at")
    if not isinstance(captured_at, str) or not captured_at.endswith("Z"):
        _fail("packet capture time is missing or not UTC")
    return manifest


def _load_json_file(path: Path, label: str) -> Mapping[str, Any]:
    try:
        decoded = json.loads(_bounded_read(path))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        _fail(f"{label} is not valid JSON: {error}")
    return _object(decoded, label)


def _bound_json(
    packet_dir: Path,
    checksums: Mapping[str, str],
    reference: Any,
    label: str,
) -> Mapping[str, Any]:
    entry = _object(reference, f"{label} reference")
    name = _safe_relative(str(entry.get("path", ""))).as_posix()
    expected = str(entry.get("sha256", ""))
    if HEX64.fullmatch(expected) is None:
        _fail(f"{label} reference digest is invalid")
    if checksums.get(name) != expected:
        _fail(f"{label} reference is not bound by the packet checksums")
    path = packet_dir / _safe_relative(name)
    if _sha256(path) != expected:
        _fail(f"{label} reference digest does not match its file")
    return _load_json_file(path, label)


def _verify_artifacts(
    packet_dir: Path,
    checksums: Mapping[str, str],
    manifest: Mapping[str, Any],
) -> dict[str, Mapping[str, Any]]:
    references = _object(manifest.get("artifacts"), "artifacts")
    if set(references) != set(ARTIFACT_SCHEMAS):
        _fail("artifact set does not exactly match the closed evidence set")
    reports: dict[str, Mapping[str, Any]] = {}
    paths: set[str] = set()
    for label, schema in ARTIFACT_SCHEMAS.items():
        reference = _object(references.get(label), f"{label} artifact reference")
        name = _safe_relative(str(reference.get("path", ""))).as_posix()
        if name in paths or name in {MANIFEST_FILE, CHECKSUM_FILE}:
            _fail(f"{label} artifact path is duplicated or reserved")
        paths.add(name)
        report = _bound_json(packet_dir, checksums, reference, f"{label} artifact")
        if report.get("schema") != schema:
            _fail(f"{label} artifact schema is unsupported")
        reports[label] = report
    return reports


def _verify_source(
    packet_dir: Path,
    manifest: Mapping[str, Any],
    report: Mapping[str, Any],
) -> None:
    source = _object(manifest.get("source"), "source")
    for key in ("git_revision", "assembly_revision", "spec_sha3_384", "binaries"):
        if report.get(key) != source.get(key):
            _fail(f"source artifact disagrees with manifest field {key}")
    if report.get("clean_checkout") is not True:
        _fail("source artifact was not produced from a clean checkout")
    if report.get("build_profile") != "release":
        _fail("source artifact does not identify a release build")
    if HEX40.fullmatch(str(source.get("git_revision", ""))) is None:
        _fail("source git revision is not a full lowercase object ID")
    if HEX40.fullmatch(str(source.get("assembly_revision", ""))) is None:
        _fail("packet assembly revision is not a full lowercase object ID")
    if HEX96.fullmatch(str(source.get("spec_sha3_384", ""))) is None:
        _fail("source specification digest is invalid")
    binaries = _list(source.get("binaries"), "source binaries")
    expected_binary_paths = {
        "bin/postfiat-node",
        "bin/postfiat-storage-corpus-batches",
        "bin/postfiat-node-rollback",
        "bin/postfiat-node-incompatible",
    }
    observed_binaries: dict[str, str] = {}
    for index, value in enumerate(binaries):
        binary = _object(value, f"source binary {index}")
        name = str(binary.get("path", ""))
        expected = str(binary.get("sha256", ""))
        if name in observed_binaries or HEX64.fullmatch(expected) is None:
            _fail(f"binary {index} digest is invalid")
        path = packet_dir / _safe_relative(name)
        if _sha256(path) != expected:
            _fail(f"binary identity mismatch for {name}")
        observed_binaries[name] = expected
    if set(observed_binaries) != expected_binary_paths:
        _fail("source binary roles do not match the required three-binary set")
    if len(set(observed_binaries.values())) != len(expected_binary_paths):
        _fail("source binary roles are not distinct")


def _verify_binary_build(
    report: Mapping[str, Any],
    label: str,
    source_revision: str,
) -> None:
    build = _object(report.get("node_binary_build"), f"{label} binary build")
    if build.get("git_revision") != source_revision[:8]:
        _fail(f"{label} embedded binary revision disagrees with the packet source")
    if build.get("profile") != "release":
        _fail(f"{label} binary was not built with the release profile")


def _verify_state_distinction(manifest: Mapping[str, Any]) -> None:
    states = _object(manifest.get("state_distinction"), "state distinction")
    for label in ("live", "deployed", "repository"):
        state = _object(states.get(label), f"state distinction {label}")
        if not isinstance(state.get("exact_identifier"), str) or not state["exact_identifier"]:
            _fail(f"{label} exact identifier is missing")
        if not isinstance(state.get("observed_at"), str) or not state["observed_at"]:
            _fail(f"{label} observation time is missing")
        if not isinstance(state.get("freshness"), str) or not state["freshness"]:
            _fail(f"{label} freshness label is missing")
    live = _object(states["live"], "live state")
    if live.get("live_probe") is not False:
        _fail("packet verification must not claim to be a live fleet probe")


def _verify_replay(
    packet_dir: Path,
    checksums: Mapping[str, str],
    replay: Mapping[str, Any],
    source_revision: str,
    current_binary_digest: str,
) -> None:
    if replay.get("source_revision") != source_revision:
        _fail("replay source revision disagrees with the packet")
    if replay.get("node_binary_sha256") != current_binary_digest:
        _fail("replay binary identity disagrees with the packet")
    _verify_binary_build(replay, "replay", source_revision)
    if replay.get("quarantine_archive_blocks") != 915:
        _fail("quarantine archive block count is not 915")
    if replay.get("authenticated_history_height") != 924:
        _fail("authenticated history does not reach height 924")
    for key in (
        "exact_pre_activation_replay",
        "full_replay_passed",
        "logical_rebuild_identical",
        "canonical_export_identical",
    ):
        _bool(replay.get(key), f"replay {key}")
    for key in ("tip_hash", "state_root", "ordered_history_accumulator"):
        if HEX96.fullmatch(str(replay.get(key, ""))) is None:
            _fail(f"replay {key} is invalid")

    expected = {915: "quarantine_archive", 924: "authenticated_history"}
    receipts = _list(replay.get("receipts"), "replay receipts")
    seen: set[int] = set()
    receipt_identities: dict[int, tuple[str, str, str]] = {}
    for reference in receipts:
        receipt = _bound_json(packet_dir, checksums, reference, "replay receipt")
        if receipt.get("schema") != "postfiat-storage-replay-receipt-v1":
            _fail("replay receipt schema is unsupported")
        if receipt.get("source_revision") != source_revision:
            _fail("replay receipt source revision disagrees with the packet")
        if receipt.get("node_binary_sha256") != current_binary_digest:
            _fail("replay receipt binary identity disagrees with the packet")
        _verify_binary_build(receipt, "replay receipt", source_revision)
        height = receipt.get("source_height")
        if not isinstance(height, int) or expected.get(height) != receipt.get("source_kind"):
            _fail("replay receipt does not identify a required source")
        if height in seen or receipt.get("block_count") != height:
            _fail("replay receipt height is duplicated or incomplete")
        seen.add(height)
        receipt_identities[height] = (
            str(receipt.get("tip_hash", "")),
            str(receipt.get("state_root", "")),
            str(receipt.get("ordered_history_accumulator", "")),
        )
        if receipt.get("chain_id") != CONTROLLED_CHAIN_ID:
            _fail(f"replay receipt at height {height} used the wrong chain")
        if receipt.get("genesis_hash") != CONTROLLED_GENESIS_HASH:
            _fail(f"replay receipt at height {height} used the wrong genesis")
        if receipt.get("commitment_mode") != "legacy_below_storage_activation":
            _fail(f"replay receipt at height {height} used the wrong commitment mode")
        for key in (
            "exact_replay",
            "full_replay_passed",
            "logical_rebuild_identical",
            "canonical_export_identical",
        ):
            _bool(receipt.get(key), f"replay receipt {height} {key}")
        for key in ("tip_hash", "state_root", "ordered_history_accumulator"):
            if HEX96.fullmatch(str(receipt.get(key, ""))) is None:
                _fail(f"replay receipt {height} {key} is invalid")
        canonical_export = _object(
            receipt.get("canonical_export_receipt"),
            f"replay receipt {height} canonical export",
        )
        if (
            canonical_export.get("schema")
            != "postfiat-transactional-canonical-export-receipt-v1"
            or canonical_export.get("finalized_height") != height
            or type(canonical_export.get("record_count")) is not int
            or canonical_export.get("record_count", 0) <= 0
            or HEX96.fullmatch(
                str(canonical_export.get("records_sha3_384", ""))
            )
            is None
            or HEX64.fullmatch(str(receipt.get("canonical_export_sha256", "")))
            is None
        ):
            _fail(f"replay receipt {height} canonical export is invalid")
    if seen != set(expected):
        _fail("replay receipts do not cover exact heights 915 and 924")
    if receipt_identities[924] != (
        replay.get("tip_hash"),
        replay.get("state_root"),
        replay.get("ordered_history_accumulator"),
    ):
        _fail("replay summary disagrees with the exact height-924 receipt")


def _metric_p95(row: Mapping[str, Any], metric: str) -> float:
    aggregate = _object(row.get("aggregate"), f"performance aggregate at {row.get('height')}")
    value = _object(aggregate.get(metric), f"{metric} aggregate").get("p95")
    if not isinstance(value, (int, float)) or value <= 0:
        _fail(f"{metric} p95 is invalid at height {row.get('height')}")
    return float(value)


def _percentile(values: list[float], fraction: float) -> float:
    if not values:
        _fail("cannot calculate a percentile over an empty sample")
    ordered = sorted(values)
    return ordered[max(0, math.ceil(len(ordered) * fraction) - 1)]


def _nested_stage_value(
    iteration: Mapping[str, Any], stage: str, path: tuple[str, ...]
) -> float:
    current: Any = iteration
    for component in path:
        if not isinstance(current, dict) or component not in current:
            _fail(f"performance iteration omitted material stage {stage}")
        current = current[component]
    if (
        not isinstance(current, (int, float))
        or not math.isfinite(float(current))
        or float(current) < 0
    ):
        _fail(f"performance material stage {stage} is invalid")
    return float(current)


def _distribution_summary(values: list[float]) -> dict[str, float | int]:
    if not values:
        _fail("cannot summarize an empty performance distribution")
    mean = sum(values) / len(values)
    return {
        "count": len(values),
        "min": min(values),
        "p50": _percentile(values, 0.50),
        "p95": _percentile(values, 0.95),
        "p99": _percentile(values, 0.99),
        "max": max(values),
        "mean": mean,
        "population_stddev": math.sqrt(
            sum((value - mean) ** 2 for value in values) / len(values)
        ),
    }


def _ordinary_least_squares(points: list[tuple[float, float]]) -> dict[str, Any]:
    if len(points) < 2:
        _fail("height relationship model has too few observations")
    mean_x = sum(point[0] for point in points) / len(points)
    mean_y = sum(point[1] for point in points) / len(points)
    denominator = sum((x - mean_x) ** 2 for x, _ in points)
    if denominator <= 0:
        _fail("height relationship model has no distinct x values")
    slope = sum((x - mean_x) * (y - mean_y) for x, y in points) / denominator
    intercept = mean_y - slope * mean_x
    predictions = [intercept + slope * x for x, _ in points]
    residuals = [y - prediction for (_, y), prediction in zip(points, predictions)]
    residual_rmse = math.sqrt(
        sum(residual * residual for residual in residuals) / len(residuals)
    )
    total_variation = sum((y - mean_y) ** 2 for _, y in points)
    residual_variation = sum(residual * residual for residual in residuals)
    r_squared = (
        1.0 - residual_variation / total_variation
        if total_variation > 0
        else 0.0
    )
    return {
        "slope": slope,
        "intercept_ms": intercept,
        "predictions_ms": predictions,
        "residuals_ms": residuals,
        "residual_rmse_ms": residual_rmse,
        "r_squared": max(0.0, min(1.0, r_squared)),
    }


def _constant_fit(values: list[float]) -> dict[str, Any]:
    mean = sum(values) / len(values)
    predictions = [mean for _ in values]
    residuals = [value - mean for value in values]
    return {
        "intercept_ms": mean,
        "predictions_ms": predictions,
        "residuals_ms": residuals,
        "residual_rmse_ms": math.sqrt(
            sum(residual * residual for residual in residuals) / len(residuals)
        ),
    }


def _verify_numeric_structure(observed: Any, expected: Any, label: str) -> None:
    if isinstance(expected, bool) or expected is None or isinstance(expected, str):
        if observed != expected:
            _fail(f"{label} differs")
        return
    if isinstance(expected, (int, float)):
        if (
            not isinstance(observed, (int, float))
            or isinstance(observed, bool)
            or not math.isclose(
                float(observed), float(expected), rel_tol=1e-12, abs_tol=1e-9
            )
        ):
            _fail(f"{label} differs")
        return
    if isinstance(expected, list):
        if not isinstance(observed, list) or len(observed) != len(expected):
            _fail(f"{label} differs")
        for index, (observed_value, expected_value) in enumerate(
            zip(observed, expected)
        ):
            _verify_numeric_structure(
                observed_value, expected_value, f"{label}[{index}]"
            )
        return
    if isinstance(expected, dict):
        if not isinstance(observed, dict) or set(observed) != set(expected):
            _fail(f"{label} differs")
        for key, expected_value in expected.items():
            _verify_numeric_structure(
                observed[key], expected_value, f"{label}.{key}"
            )
        return
    if observed != expected:
        _fail(f"{label} differs")


def _verify_height_relationship_models(
    performance: Mapping[str, Any],
    observations_by_stage: Mapping[str, list[dict[str, Any]]],
    height_50_p95: Mapping[str, list[float]],
    *,
    reject_material_positive: bool = True,
) -> None:
    envelope = _object(
        performance.get("height_relationship_model"),
        "height relationship model",
    )
    if envelope.get("schema") != "postfiat-storage-height-cost-model-v2":
        _fail("height relationship model schema is unsupported")
    if envelope.get("sample_kind") != "per_window_p95":
        _fail("height relationship model sample kind is unsupported")
    if envelope.get("relative_materiality") != MODEL_RELATIVE_MATERIALITY:
        _fail("height relationship model materiality threshold changed")
    if envelope.get("residual_sigmas") != MODEL_RESIDUAL_SIGMAS:
        _fail("height relationship model residual threshold changed")
    recorded = _object(envelope.get("stages"), "height relationship stages")
    if set(recorded) != set(MATERIAL_STAGE_PATHS):
        _fail("height relationship model does not cover the closed stage set")

    for stage in MATERIAL_STAGE_PATHS:
        observations = observations_by_stage[stage]
        points = [
            (float(observation["height"]), float(observation["p95_ms"]))
            for observation in observations
        ]
        values = [point[1] for point in points]
        linear = _ordinary_least_squares(points)
        logarithmic = _ordinary_least_squares(
            [(math.log(height), value) for height, value in points]
        )
        constant = _constant_fit(values)
        baseline = _percentile(height_50_p95[stage], 0.50)
        predicted_delta = linear["slope"] * (HEIGHTS[-1] - HEIGHTS[0])
        within_height_ranges = []
        for height in HEIGHTS:
            same_height = [
                value for observed_height, value in points if observed_height == height
            ]
            within_height_ranges.append(max(same_height) - min(same_height))
        same_height_variance_allowance = max(within_height_ranges)
        material_threshold = max(
            baseline * MODEL_RELATIVE_MATERIALITY,
            linear["residual_rmse_ms"] * MODEL_RESIDUAL_SIGMAS,
            same_height_variance_allowance,
        )
        material_positive = (
            linear["slope"] > 0 and predicted_delta > material_threshold
        )
        logarithmic_slope = logarithmic.pop("slope")
        linear_slope = linear.pop("slope")
        fits = {
            "constant": constant,
            "logarithmic": {
                **logarithmic,
                "slope_ms_per_log_height": logarithmic_slope,
            },
            "linear": {
                **linear,
                "slope_ms_per_height": linear_slope,
            },
        }
        linear_fit = fits["linear"]
        expected: dict[str, Any] = {
            "slope_ms_per_height": linear_fit["slope_ms_per_height"],
            "intercept_ms": linear_fit["intercept_ms"],
            "predictions_ms": linear_fit["predictions_ms"],
            "residuals_ms": linear_fit["residuals_ms"],
            "residual_rmse_ms": linear_fit["residual_rmse_ms"],
            "r_squared": linear_fit["r_squared"],
            "observations": observations,
            "fits": fits,
            "preferred_fit_by_rmse": min(
                fits,
                key=lambda name: float(fits[name]["residual_rmse_ms"]),
            ),
            "sample_kind": "per_window_p95",
            "sample_count": len(points),
            "height_50_window_p95_median_ms": baseline,
            "max_same_height_window_range_ms": same_height_variance_allowance,
            "predicted_delta_50_to_5000_ms": predicted_delta,
            "material_threshold_ms": material_threshold,
            "relative_materiality": MODEL_RELATIVE_MATERIALITY,
            "residual_sigmas": MODEL_RESIDUAL_SIGMAS,
            "material_positive_linear_relationship": material_positive,
        }
        observed = _object(recorded.get(stage), f"height relationship stage {stage}")
        _verify_numeric_structure(
            observed,
            expected,
            f"height relationship stage {stage}",
        )
        if material_positive and reject_material_positive:
            _fail(f"material stage {stage} retains a positive height relationship")


def _verify_performance_fleet(
    value: Any,
    label: str,
    expected_height: int,
) -> tuple[str, str]:
    fleet = _list(value, label)
    if len(fleet) != 6:
        _fail(f"{label} does not contain six validators")
    node_ids: set[str] = set()
    identities: set[tuple[int, str, str]] = set()
    for row_value in fleet:
        row = _object(row_value, label)
        node_id = str(row.get("node_id", ""))
        height = row.get("height")
        tip = str(row.get("tip", ""))
        state_root = str(row.get("state_root", ""))
        if (
            node_id not in {f"validator-{index}" for index in range(6)}
            or node_id in node_ids
            or height != expected_height
            or HEX96.fullmatch(tip) is None
            or HEX96.fullmatch(state_root) is None
        ):
            _fail(f"{label} contains an invalid validator identity")
        node_ids.add(node_id)
        identities.add((height, tip, state_root))
    if len(identities) != 1:
        _fail(f"{label} did not converge")
    _, tip, state_root = next(iter(identities))
    return tip, state_root


def _recompute_vote_lock_work(
    raw: Mapping[str, Any],
    lane_name: str,
) -> dict[str, Any]:
    iterations = _list(raw.get("iterations"), "performance vote-lock iterations")
    validators: dict[str, dict[str, Any]] = {}
    for round_number, iteration_value in enumerate(iterations, start=1):
        iteration = _object(iteration_value, "performance vote-lock iteration")
        round_timings = _object(
            iteration.get("round_timings"),
            "performance vote-lock round timings",
        )
        targets = _list(
            round_timings.get("vote_request_targets"),
            "performance vote-lock targets",
        )
        if len(targets) != 5:
            _fail(f"performance lane {lane_name} vote-lock target count differs")
        observed_this_round: set[str] = set()
        for target_value in targets:
            target = _object(target_value, "performance vote-lock target")
            node_id = str(target.get("target", ""))
            if (
                node_id not in {f"validator-{index}" for index in range(6)}
                or node_id in observed_this_round
                or target.get("result") != "ok"
            ):
                _fail(
                    f"performance lane {lane_name} vote-lock target identity differs"
                )
            observed_this_round.add(node_id)
            request = _object(
                target.get("vote_request_breakdown"),
                "performance vote-lock request",
            )
            remote = _object(
                request.get("remote_handling"),
                "performance remote vote-lock handling",
            )
            timing = _object(
                remote.get("block_vote_breakdown"),
                "performance block-vote timing",
            )
            files_examined = timing.get("vote_lock_files_examined", 0)
            bytes_decoded = timing.get("vote_lock_bytes_decoded", 0)
            migration_performed = timing.get(
                "vote_lock_migration_performed",
                False,
            )
            if (
                not isinstance(files_examined, int)
                or isinstance(files_examined, bool)
                or files_examined < 0
                or not isinstance(bytes_decoded, int)
                or isinstance(bytes_decoded, bool)
                or bytes_decoded < 0
                or not isinstance(migration_performed, bool)
            ):
                _fail(f"performance lane {lane_name} vote-lock telemetry is invalid")
            summary = validators.setdefault(
                node_id,
                {
                    "votes_observed": 0,
                    "migration_rounds": [],
                    "max_files_examined": 0,
                    "max_bytes_decoded": 0,
                },
            )
            summary["votes_observed"] += 1
            summary["max_files_examined"] = max(
                int(summary["max_files_examined"]),
                files_examined,
            )
            summary["max_bytes_decoded"] = max(
                int(summary["max_bytes_decoded"]),
                bytes_decoded,
            )
            if migration_performed:
                summary["migration_rounds"].append(round_number)
                if len(summary["migration_rounds"]) > 1:
                    _fail(
                        f"{VOTE_LOCK_REASON_MIGRATION_REPEATED}: "
                        f"performance lane {lane_name}"
                    )
                if round_number != 1:
                    _fail(
                        f"{VOTE_LOCK_REASON_MIGRATION_LATE}: "
                        f"performance lane {lane_name}"
                    )
            else:
                if files_examined > VOTE_LOCK_MAX_FILES_EXAMINED:
                    _fail(
                        f"{VOTE_LOCK_REASON_FILES_EXCEEDED}: "
                        f"performance lane {lane_name}"
                    )
                if bytes_decoded > VOTE_LOCK_MAX_BYTES_DECODED:
                    _fail(
                        f"{VOTE_LOCK_REASON_BYTES_EXCEEDED}: "
                        f"performance lane {lane_name}"
                    )

    if (
        len(iterations) != 50
        or sorted(validators) != [f"validator-{index}" for index in range(6)]
    ):
        _fail(f"performance lane {lane_name} vote-lock coverage is incomplete")
    public_validators = {
        node_id: {
            "passed": True,
            "votes_observed": int(summary["votes_observed"]),
            "migration_rounds": list(summary["migration_rounds"]),
            "max_files_examined": int(summary["max_files_examined"]),
            "max_bytes_decoded": int(summary["max_bytes_decoded"]),
            "reason_codes": [],
            "violations": [],
        }
        for node_id, summary in sorted(validators.items())
    }
    return {
        "schema": VOTE_LOCK_WORK_GATE_SCHEMA,
        "passed": True,
        "reason_codes": [],
        "limits": {
            "migration_max_per_validator_per_window_restore": 1,
            "migration_allowed_finalized_round": 1,
            "non_migration_max_files_examined": VOTE_LOCK_MAX_FILES_EXAMINED,
            "non_migration_max_bytes_decoded": VOTE_LOCK_MAX_BYTES_DECODED,
            "legacy_absent_fields_default_to_zero_false": True,
        },
        "rounds_observed": len(iterations),
        "validators_observed": len(public_validators),
        "validators": public_validators,
    }


def _verify_vote_lock_work(
    packet_dir: Path,
    checksums: Mapping[str, str],
    window: Mapping[str, Any],
    raw: Mapping[str, Any],
    lane_name: str,
) -> None:
    expected = _recompute_vote_lock_work(raw, lane_name)
    receipt = _bound_json(
        packet_dir,
        checksums,
        {
            "path": window.get("vote_lock_work_receipt"),
            "sha256": window.get("vote_lock_work_receipt_sha256"),
        },
        f"performance lane {lane_name} vote-lock work receipt",
    )
    if (
        window.get("vote_lock_work_gate_pass") is not True
        or window.get("vote_lock_work_gate_reason_codes") != []
        or window.get("vote_lock_work") != expected
        or receipt != expected
    ):
        _fail(f"performance lane {lane_name} vote-lock gate summary differs")


def _verify_resource_samples(
    packet_dir: Path,
    checksums: Mapping[str, str],
    window: Mapping[str, Any],
    lane_name: str,
    expected_rounds: int,
) -> dict[str, int | float]:
    report = _bound_json(
        packet_dir,
        checksums,
        {
            "path": window.get("resource_samples"),
            "sha256": window.get("resource_samples_sha256"),
        },
        f"performance lane {lane_name} resource samples",
    )
    if (
        report.get("schema") != RESOURCE_SAMPLE_SCHEMA
        or report.get("sample_target_interval_ms")
        != RESOURCE_SAMPLE_TARGET_INTERVAL_MS
    ):
        _fail(f"performance lane {lane_name} resource sample schema is invalid")
    samples = _list(report.get("samples"), "performance resource samples")
    if len(samples) < 2:
        _fail(f"performance lane {lane_name} resource samples are incomplete")

    normalized_samples: list[Mapping[str, Any]] = []
    per_pid: dict[str, list[Mapping[str, Any]]] = {}
    previous_offset = -1
    for sample_value in samples:
        sample = _object(sample_value, "performance resource sample")
        if set(sample) != {
            "monotonic_offset_ns",
            "host_cpu_ticks",
            "host_memory",
            "network",
            "node_disk_bytes",
            "processes",
        }:
            _fail(f"performance lane {lane_name} resource sample fields differ")
        offset = sample.get("monotonic_offset_ns")
        host_cpu = sample.get("host_cpu_ticks")
        if (
            not isinstance(offset, int)
            or isinstance(offset, bool)
            or offset <= previous_offset
            or not isinstance(host_cpu, int)
            or isinstance(host_cpu, bool)
            or host_cpu < 0
        ):
            _fail(f"performance lane {lane_name} resource sample timing is invalid")
        previous_offset = offset
        host_memory = _object(sample.get("host_memory"), "resource host memory")
        network = _object(sample.get("network"), "resource network")
        if set(host_memory) != {"total_kib", "available_kib"} or set(network) != {
            "received",
            "transmitted",
        }:
            _fail(f"performance lane {lane_name} resource host fields differ")
        for value in (*host_memory.values(), *network.values()):
            if (
                not isinstance(value, int)
                or isinstance(value, bool)
                or value < 0
            ):
                _fail(f"performance lane {lane_name} resource host value is invalid")
        node_disk = sample.get("node_disk_bytes")
        if node_disk is not None and (
            not isinstance(node_disk, int)
            or isinstance(node_disk, bool)
            or node_disk < 0
        ):
            _fail(f"performance lane {lane_name} resource disk value is invalid")
        processes = _object(sample.get("processes"), "resource processes")
        for pid, metrics_value in processes.items():
            if not isinstance(pid, str) or not pid.isdigit() or int(pid) <= 0:
                _fail(f"performance lane {lane_name} resource process id is invalid")
            metrics = _object(metrics_value, "resource process metrics")
            if set(metrics) != {"cpu_ticks", "rss_kib", "read_bytes", "write_bytes"}:
                _fail(f"performance lane {lane_name} resource process fields differ")
            if any(
                not isinstance(value, int)
                or isinstance(value, bool)
                or value < 0
                for value in metrics.values()
            ):
                _fail(f"performance lane {lane_name} resource process value is invalid")
            per_pid.setdefault(pid, []).append(metrics)
        normalized_samples.append(sample)

    if normalized_samples[0]["monotonic_offset_ns"] != 0:
        _fail(f"performance lane {lane_name} resource samples lack a zero origin")
    first = normalized_samples[0]
    last = normalized_samples[-1]
    if not isinstance(first["node_disk_bytes"], int) or not isinstance(
        last["node_disk_bytes"], int
    ):
        _fail(f"performance lane {lane_name} resource disk endpoints are missing")

    foreground = _list(
        report.get("foreground_processes"),
        "performance foreground processes",
    )
    if len(foreground) != expected_rounds:
        _fail(f"performance lane {lane_name} foreground process count differs")
    expected_counts: dict[str, int] = {}
    for process_value in foreground:
        process = _object(process_value, "performance foreground process")
        if set(process) != {"pid", "started_offset_ns", "ended_offset_ns"}:
            _fail(f"performance lane {lane_name} foreground process fields differ")
        pid = process.get("pid")
        started = process.get("started_offset_ns")
        ended = process.get("ended_offset_ns")
        if (
            not isinstance(pid, int)
            or isinstance(pid, bool)
            or pid <= 0
            or not isinstance(started, int)
            or isinstance(started, bool)
            or not isinstance(ended, int)
            or isinstance(ended, bool)
            or started < 0
            or ended <= started
            or ended > last["monotonic_offset_ns"]
            or str(pid) in expected_counts
        ):
            _fail(f"performance lane {lane_name} foreground process is invalid")
        expected_counts[str(pid)] = sum(
            1
            for sample in normalized_samples
            if started <= sample["monotonic_offset_ns"] <= ended
            and str(pid) in sample["processes"]
        )
    recorded_counts = _object(
        report.get("foreground_sample_counts"),
        "performance foreground sample counts",
    )
    if dict(recorded_counts) != expected_counts or min(expected_counts.values()) < 2:
        _fail(f"performance lane {lane_name} foreground sampling is incomplete")

    process_cpu_ticks = 0
    process_read_bytes = 0
    process_write_bytes = 0
    for values in per_pid.values():
        process_cpu_ticks += max(row["cpu_ticks"] for row in values) - min(
            row["cpu_ticks"] for row in values
        )
        process_read_bytes += max(row["read_bytes"] for row in values) - min(
            row["read_bytes"] for row in values
        )
        process_write_bytes += max(row["write_bytes"] for row in values) - min(
            row["write_bytes"] for row in values
        )
    return {
        "cpu_ticks": process_cpu_ticks,
        "peak_rss_kib": max(
            sum(metrics["rss_kib"] for metrics in sample["processes"].values())
            for sample in normalized_samples
        ),
        "disk_growth_bytes": max(
            0, last["node_disk_bytes"] - first["node_disk_bytes"]
        ),
        "bytes_read": process_read_bytes,
        "bytes_written": process_write_bytes,
        "sample_count": len(normalized_samples),
        "duration_ms": (
            last["monotonic_offset_ns"] - first["monotonic_offset_ns"]
        )
        / 1_000_000,
        "observed_pid_count": len(per_pid),
        "foreground_process_count": len(foreground),
        "foreground_min_sample_count": min(expected_counts.values()),
        "host_cpu_ticks": last["host_cpu_ticks"] - first["host_cpu_ticks"],
        "host_total_memory_kib": first["host_memory"]["total_kib"],
        "host_min_available_memory_kib": min(
            sample["host_memory"]["available_kib"]
            for sample in normalized_samples
        ),
        "network_received_bytes": (
            last["network"]["received"] - first["network"]["received"]
        ),
        "network_transmitted_bytes": (
            last["network"]["transmitted"] - first["network"]["transmitted"]
        ),
    }


def _canonical_signed_transfer_identity(
    transfer_value: Any,
    label: str,
) -> tuple[str, str, int]:
    transfer = _object(transfer_value, label)
    unsigned = _object(transfer.get("unsigned"), f"{label} unsigned transfer")
    unsigned_fields = (
        "chain_id",
        "genesis_hash",
        "protocol_version",
        "address_namespace",
        "transaction_kind",
        "signature_algorithm_id",
        "from",
        "to",
        "amount",
        "fee",
        "sequence",
    )
    if set(unsigned) != set(unsigned_fields) or set(transfer) != {
        "unsigned",
        "algorithm_id",
        "public_key_hex",
        "signature_hex",
    }:
        _fail(f"{label} has a non-canonical field set")
    canonical_unsigned = {field: unsigned[field] for field in unsigned_fields}
    canonical_transfer = {
        "unsigned": canonical_unsigned,
        "algorithm_id": transfer["algorithm_id"],
        "public_key_hex": transfer["public_key_hex"],
        "signature_hex": transfer["signature_hex"],
    }
    for field in (
        "chain_id",
        "genesis_hash",
        "address_namespace",
        "transaction_kind",
        "signature_algorithm_id",
        "from",
        "to",
    ):
        if not isinstance(unsigned[field], str) or not unsigned[field]:
            _fail(f"{label} unsigned field {field} is invalid")
    for field in ("protocol_version", "amount", "fee", "sequence"):
        if (
            not isinstance(unsigned[field], int)
            or isinstance(unsigned[field], bool)
            or unsigned[field] < 0
        ):
            _fail(f"{label} unsigned field {field} is invalid")
    for field in ("algorithm_id", "public_key_hex", "signature_hex"):
        if not isinstance(transfer[field], str) or not transfer[field]:
            _fail(f"{label} field {field} is invalid")
    canonical_json = json.dumps(
        canonical_transfer,
        ensure_ascii=False,
        separators=(",", ":"),
    ).encode("utf-8")
    signed_sha256 = hashlib.sha256(canonical_json).hexdigest()
    signing_bytes = (
        "postfiat.transfer.v1\n"
        f"chain_id={unsigned['chain_id']}\n"
        f"genesis_hash={unsigned['genesis_hash']}\n"
        f"protocol_version={unsigned['protocol_version']}\n"
        f"address_namespace={unsigned['address_namespace']}\n"
        f"transaction_kind={unsigned['transaction_kind']}\n"
        f"signature_algorithm_id={unsigned['signature_algorithm_id']}\n"
        f"from={unsigned['from']}\n"
        f"to={unsigned['to']}\n"
        f"amount={unsigned['amount']}\n"
        f"fee={unsigned['fee']}\n"
        f"sequence={unsigned['sequence']}\n"
        f"algorithm={transfer['algorithm_id']}\n"
        f"public_key={transfer['public_key_hex']}\n"
        f"signature={transfer['signature_hex']}\n"
    ).encode("utf-8")
    tx_id = hashlib.sha3_384(b"postfiat.tx_id.v1\x00" + signing_bytes).hexdigest()
    return tx_id, signed_sha256, int(unsigned["sequence"])


def _verify_signed_transfer_corpus(
    packet_dir: Path,
    checksums: Mapping[str, str],
    path: Any,
    digest: Any,
    *,
    wallet_address: str,
    recipient_address: str,
    transfer_count: int,
    label: str,
) -> tuple[tuple[tuple[str, str], ...], int, int]:
    corpus = _bound_json(
        packet_dir,
        checksums,
        {"path": path, "sha256": digest},
        label,
    )
    transfers = _list(corpus.get("transfers"), f"{label} transfers")
    if corpus.get("schema") != SIGNED_TRANSFER_CORPUS_SCHEMA:
        _fail(f"{label} schema is unsupported")
    if len(transfers) != transfer_count:
        _fail(f"{label} transfer count differs")
    identities: list[tuple[str, str]] = []
    sequences: list[int] = []
    for index, transfer in enumerate(transfers):
        tx_id, signed_sha256, sequence = _canonical_signed_transfer_identity(
            transfer,
            f"{label} entry {index}",
        )
        unsigned = _object(
            _object(transfer, f"{label} entry {index}").get("unsigned"),
            f"{label} entry {index} unsigned transfer",
        )
        if (
            unsigned.get("from") != wallet_address
            or unsigned.get("to") != recipient_address
            or unsigned.get("amount") != 10
        ):
            _fail(f"{label} entry {index} workload binding differs")
        identities.append((tx_id, signed_sha256))
        sequences.append(sequence)
    if len(set(identities)) != len(identities):
        _fail(f"{label} contains duplicate signed transactions")
    if sequences != list(range(sequences[0], sequences[0] + transfer_count)):
        _fail(f"{label} sequences are not contiguous")
    return tuple(identities), sequences[0], sequences[-1]


def _recompute_raw_storage_work(
    raw: Mapping[str, Any], lane_name: str
) -> dict[str, Any]:
    transactional_totals = {field: 0 for field in TRANSACTIONAL_COUNTER_FIELDS}
    legacy_totals = {field: 0 for field in LEGACY_COUNTER_FIELDS}
    full_history_records_read = 0
    full_history_bytes_read = 0
    selected = lane_name == "selected-indexed"

    def add_work(value: Any, label: str, *, apply_stage: bool) -> None:
        nonlocal full_history_records_read, full_history_bytes_read
        work = _object(value, f"{label} storage work")
        legacy = _object(work.get("legacy"), f"{label} legacy storage work")
        if set(legacy) != set(LEGACY_COUNTER_FIELDS):
            _fail(f"{label} legacy storage counter set differs")
        for field in LEGACY_COUNTER_FIELDS:
            counter = legacy.get(field)
            if not isinstance(counter, int) or isinstance(counter, bool) or counter < 0:
                _fail(f"{label} legacy storage counter {field} is invalid")
            legacy_totals[field] += counter

        transactional_value = work.get("transactional")
        if selected:
            transactional = _object(
                transactional_value, f"{label} transactional storage work"
            )
            if set(transactional) != set(TRANSACTIONAL_COUNTER_FIELDS):
                _fail(f"{label} transactional storage counter set differs")
            for field in TRANSACTIONAL_COUNTER_FIELDS:
                counter = transactional.get(field)
                if (
                    not isinstance(counter, int)
                    or isinstance(counter, bool)
                    or counter < 0
                ):
                    _fail(f"{label} transactional storage counter {field} is invalid")
                transactional_totals[field] += counter
            max_reads = (
                MAX_APPLY_PAGE_READS_PER_VALIDATOR_ROUND
                if apply_stage
                else MAX_PROPOSAL_PAGE_READS_PER_ROUND
            )
            max_writes = MAX_APPLY_PAGE_WRITES_PER_VALIDATOR_ROUND if apply_stage else 0
            if (
                transactional["page_reads"] > max_reads
                or transactional["page_writes"] > max_writes
            ):
                _fail(f"{label} transactional page work exceeds the per-stage bound")
            if apply_stage:
                if transactional["committed_write_transactions"] != 1:
                    _fail(f"{label} did not commit exactly one storage transaction")
            elif any(
                transactional[field] != 0
                for field in (
                    "write_transactions",
                    "committed_write_transactions",
                    "records_written",
                    "bytes_written",
                    "page_writes",
                    "durable_commit_micros",
                )
            ):
                _fail(f"{label} reported a write outside finalized apply")
            if any(legacy[field] != 0 for field in LEGACY_COUNTER_FIELDS):
                _fail(f"{label} selected storage unexpectedly used legacy work")
        elif transactional_value is not None:
            _fail(f"{label} comparison storage unexpectedly used transactional work")

        expected_records = (
            (transactional_value or {}).get("full_history_records_read", 0)
            + legacy["crash_suffix_records_verified"]
            + legacy["legacy_prefix_records_verified"]
            + legacy["ordered_history_records_read"]
        )
        expected_bytes = (
            (transactional_value or {}).get("full_history_bytes_read", 0)
            + legacy["checkpoint_bytes_read"]
            + legacy["crash_suffix_bytes_read"]
            + legacy["legacy_prefix_bytes_read"]
            + legacy["ordered_history_bytes_read"]
            + legacy["ordered_index_bitmap_bytes_read"]
        )
        if (
            work.get("full_history_records_read") != expected_records
            or work.get("full_history_bytes_read") != expected_bytes
        ):
            _fail(f"{label} full-history storage summary differs")
        full_history_records_read += expected_records
        full_history_bytes_read += expected_bytes

    iterations = _list(raw.get("iterations"), "performance raw storage iterations")
    for iteration_index, iteration_value in enumerate(iterations, start=1):
        iteration = _object(iteration_value, "performance raw storage iteration")
        timings = _object(
            iteration.get("round_timings"),
            f"performance iteration {iteration_index} round timings",
        )
        proposal = _object(
            timings.get("proposal_breakdown"),
            f"performance iteration {iteration_index} proposal",
        )
        add_work(
            proposal.get("storage_work"),
            f"performance iteration {iteration_index} proposal",
            apply_stage=False,
        )

        vote_targets = _list(
            timings.get("vote_request_targets"),
            f"performance iteration {iteration_index} validator reconstructions",
        )
        if len(vote_targets) != 5:
            _fail(f"performance iteration {iteration_index} lacks five reconstructions")
        for target_value in vote_targets:
            target = _object(target_value, "performance validator reconstruction")
            if target.get("result") != "ok":
                _fail("performance validator reconstruction failed")
            request = _object(
                target.get("vote_request_breakdown"),
                "performance validator reconstruction request",
            )
            remote = _object(
                request.get("remote_handling"),
                "performance remote validator handling",
            )
            block_vote = _object(
                remote.get("block_vote_breakdown"),
                "performance remote validator block vote",
            )
            add_work(
                block_vote.get("storage_work"),
                "performance validator reconstruction",
                apply_stage=False,
            )

        local_apply = _object(
            timings.get("local_apply_breakdown"),
            f"performance iteration {iteration_index} local apply",
        )
        add_work(
            local_apply.get("storage_work"),
            f"performance iteration {iteration_index} local apply",
            apply_stage=True,
        )
        send_targets = _list(
            timings.get("certified_send_targets"),
            f"performance iteration {iteration_index} certified applies",
        )
        if len(send_targets) != 5:
            _fail(f"performance iteration {iteration_index} lacks five certified applies")
        for target_value in send_targets:
            target = _object(target_value, "performance certified apply")
            if target.get("result") != "ok":
                _fail("performance certified apply failed")
            add_work(
                target.get("storage_work"),
                "performance certified apply",
                apply_stage=True,
            )

    return {
        "transactional": transactional_totals,
        "legacy": legacy_totals,
        "full_history_records_read": full_history_records_read,
        "full_history_bytes_read": full_history_bytes_read,
        "fsync_count": (
            transactional_totals["committed_write_transactions"]
            if selected
            else legacy_totals["jsonl_append_calls"]
        ),
    }


def _verify_performance_lane(
    packet_dir: Path,
    checksums: Mapping[str, str],
    lane: Mapping[str, Any],
    lane_name: str,
    expected_binary_digest: str,
    expected_source_revision: str,
    snapshot_bindings: Mapping[int, Mapping[str, Any]],
) -> tuple[list[Mapping[str, Any]], dict[int, list[Mapping[str, Any]]]]:
    if lane.get("lane") != lane_name:
        _fail(f"performance lane {lane_name} identifies the wrong mode")
    source_revision = str(lane.get("source_revision", ""))
    if (
        HEX40.fullmatch(source_revision) is None
        or source_revision != expected_source_revision
    ):
        _fail(f"performance lane {lane_name} source revision differs")
    if lane.get("storage_behavior") != PERFORMANCE_STORAGE_BEHAVIORS[lane_name]:
        _fail(f"performance lane {lane_name} storage behavior is unbound")
    if lane.get("node_binary_sha256") != expected_binary_digest:
        _fail(f"performance lane {lane_name} binary identity is unbound")
    _verify_binary_build(lane, f"performance lane {lane_name}", source_revision)
    if lane.get("storage_backend_mode") != PERFORMANCE_BACKEND_MODES[lane_name]:
        _fail(f"performance lane {lane_name} backend selector is unbound")
    if lane.get("chain_id") != "postfiat-storage-scaling-local-v1":
        _fail(f"performance lane {lane_name} used the wrong local chain")
    if lane.get("storage_activation_height") != 1:
        _fail(f"performance lane {lane_name} used the wrong activation boundary")
    for key in ("height_1_snapshot_sha256", "topology_sha256"):
        if HEX64.fullmatch(str(lane.get(key, ""))) is None:
            _fail(f"performance lane {lane_name} {key} is invalid")
    environment = _object(
        lane.get("environment"),
        f"performance lane {lane_name} environment",
    )
    affinity = environment.get("cpu_affinity")
    if (
        set(environment)
        != {"cpu_affinity", "filesystem_device", "filesystem_block_size_bytes"}
        or not isinstance(affinity, list)
        or not affinity
        or any(
            not isinstance(cpu, int) or isinstance(cpu, bool) or cpu < 0
            for cpu in affinity
        )
        or len(set(affinity)) != len(affinity)
        or not isinstance(environment.get("filesystem_device"), int)
        or isinstance(environment.get("filesystem_device"), bool)
        or environment["filesystem_device"] < 0
        or not isinstance(environment.get("filesystem_block_size_bytes"), int)
        or isinstance(environment.get("filesystem_block_size_bytes"), bool)
        or environment["filesystem_block_size_bytes"] <= 0
    ):
        _fail(f"performance lane {lane_name} environment is unbound")
    for key in ("wallet_address", "recipient_address"):
        if not isinstance(lane.get(key), str) or not lane[key]:
            _fail(f"performance lane {lane_name} {key} is missing")
    validator_public_identities = _list(
        lane.get("validator_public_identities"),
        f"performance lane {lane_name} validator key identities",
    )
    if len(validator_public_identities) != 6:
        _fail(f"performance lane {lane_name} validator key count is invalid")
    expected_validator_ids = [f"validator-{index}" for index in range(6)]
    observed_validator_ids: list[str] = []
    for value in validator_public_identities:
        identity = _object(value, f"performance lane {lane_name} validator key")
        observed_validator_ids.append(str(identity.get("node_id", "")))
        if (
            identity.get("algorithm_id") != "ML-DSA-65"
            or HEX64.fullmatch(str(identity.get("public_key_sha256", ""))) is None
        ):
            _fail(f"performance lane {lane_name} validator key identity is invalid")
    if observed_validator_ids != expected_validator_ids:
        _fail(f"performance lane {lane_name} validator key order is invalid")

    rows = _list(lane.get("rows"), f"performance lane {lane_name} rows")
    expected_heights = PERFORMANCE_LANE_HEIGHTS[lane_name]
    if [
        row.get("height") if isinstance(row, dict) else None for row in rows
    ] != expected_heights:
        _fail(f"performance lane {lane_name} heights differ")
    stage_observations: dict[str, list[dict[str, Any]]] = {
        stage: [] for stage in MATERIAL_STAGE_PATHS
    }
    height_50_stage_p95: dict[str, list[float]] = {
        stage: [] for stage in MATERIAL_STAGE_PATHS
    }
    verified_windows: dict[int, list[Mapping[str, Any]]] = {}
    for row_value in rows:
        row = _object(row_value, f"performance lane {lane_name} row")
        height = int(row["height"])
        snapshot_binding = snapshot_bindings.get(height)
        if snapshot_binding is None:
            _fail(f"performance lane {lane_name} height {height} lacks a shared input")
        windows = _list(
            row.get("windows"),
            f"performance lane {lane_name} height {height} windows",
        )
        if len(windows) != 5:
            _fail(f"performance lane {lane_name} height {height} lacks five windows")
        samples: dict[str, list[float]] = {
            "consensus_round_ms": [],
            "wallet_to_finality_ms": [],
        }
        row_source_snapshots: set[str] = set()
        row_initial_identities: set[tuple[str, str]] = set()
        row_verified_windows: list[Mapping[str, Any]] = []
        for window_index, window_value in enumerate(windows, start=1):
            window = _object(window_value, f"performance lane {lane_name} window")
            if (
                window.get("label") != f"height-{height}-window-{window_index}"
                or window.get("storage_lane") != lane_name
                or window.get("starting_height") != height
                or window.get("rounds") != 50
                or window.get("validators_converged") != 6
            ):
                _fail(f"performance lane {lane_name} window cardinality is invalid")
            _bool(
                window.get("literal_receipts_exact"),
                f"performance lane {lane_name} literal receipts",
            )
            _bool(
                window.get("backend_work_gate_pass"),
                f"performance lane {lane_name} backend work gate",
            )
            if HEX64.fullmatch(
                str(window.get("signed_transfer_corpus_sha256", ""))
            ) is None:
                _fail(
                    f"performance lane {lane_name} window signed corpus is invalid"
                )
            if (
                window.get("source_snapshot_sha256")
                != snapshot_binding["snapshot_sha256"]
                or window.get("signed_transfer_corpus")
                != snapshot_binding["corpus_path"]
                or window.get("signed_transfer_corpus_sha256")
                != snapshot_binding["corpus_sha256"]
            ):
                _fail(
                    f"performance lane {lane_name} height {height} did not use "
                    "the shared frozen input and signed corpus"
                )
            if lane_name == "selected-indexed":
                if (
                    window.get("node_preparation_mode")
                    != "byte-verified-prepared-fleet-clone"
                    or window.get("prepared_fleet_sha256")
                    != snapshot_binding["prepared_fleet_sha256"]
                    or window.get("result_snapshot_sha256") is not None
                    or HEX64.fullmatch(
                        str(window.get("result_prepared_fleet_sha256", ""))
                    )
                    is None
                ):
                    _fail(
                        f"performance lane {lane_name} height {height} did not use "
                        "the frozen prepared fleet"
                    )
            elif (
                window.get("node_preparation_mode")
                != "authenticated-portable-snapshot-import"
                or window.get("prepared_fleet_sha256") is not None
                or window.get("result_prepared_fleet_sha256") is not None
                or HEX64.fullmatch(
                    str(window.get("source_snapshot_sha256", ""))
                )
                is None
                or HEX64.fullmatch(
                    str(window.get("result_snapshot_sha256", ""))
                )
                is None
            ):
                _fail(
                    f"performance lane {lane_name} height {height} did not use "
                    "the authenticated portable snapshot import"
                )
            row_source_snapshots.add(str(window["source_snapshot_sha256"]))
            initial_tip, initial_root = _verify_performance_fleet(
                window.get("initial_fleet"),
                f"performance lane {lane_name} initial fleet",
                height,
            )
            final_tip, final_root = _verify_performance_fleet(
                window.get("final_fleet"),
                f"performance lane {lane_name} final fleet",
                height + 50,
            )
            row_initial_identities.add((initial_tip, initial_root))
            if (
                window.get("final_height") != height + 50
                or window.get("final_tip") != final_tip
                or window.get("final_state_root") != final_root
                or not initial_tip
                or not initial_root
            ):
                _fail(f"performance lane {lane_name} final identity is inconsistent")

            storage = _object(
                window.get("storage"), f"performance lane {lane_name} storage"
            )
            resources = _object(
                window.get("resources"), f"performance lane {lane_name} resources"
            )
            for key in PERFORMANCE_RESOURCE_FIELDS:
                if (
                    not isinstance(resources.get(key), (int, float))
                    or isinstance(resources.get(key), bool)
                    or resources[key] < 0
                ):
                    _fail(f"performance lane {lane_name} resource {key} is missing")
            expected_resources = _verify_resource_samples(
                packet_dir,
                checksums,
                window,
                lane_name,
                expected_rounds=50,
            )
            for key, expected_value in expected_resources.items():
                _verify_numeric_structure(
                    resources.get(key),
                    expected_value,
                    f"performance lane {lane_name} sampled resource {key}",
                )
            transactional = _object(
                storage.get("transactional"),
                f"performance lane {lane_name} transactional counters",
            )
            legacy = _object(
                storage.get("legacy"),
                f"performance lane {lane_name} legacy counters",
            )
            for counter_set, fields, counter_label in (
                (transactional, TRANSACTIONAL_COUNTER_FIELDS, "transactional"),
                (legacy, LEGACY_COUNTER_FIELDS, "legacy"),
            ):
                if set(counter_set) != set(fields):
                    _fail(
                        f"performance lane {lane_name} {counter_label} counter set differs"
                    )
                for key in fields:
                    value = counter_set.get(key)
                    if (
                        not isinstance(value, int)
                        or isinstance(value, bool)
                        or value < 0
                    ):
                        _fail(
                            f"performance lane {lane_name} {counter_label} "
                            f"counter {key} is invalid"
                        )
            for key in (
                "full_history_records_read",
                "full_history_bytes_read",
                "fsync_count",
            ):
                value = storage.get(key)
                if not isinstance(value, int) or isinstance(value, bool) or value < 0:
                    _fail(f"performance lane {lane_name} storage counter {key} is invalid")
            zero_full_history = (
                storage["full_history_records_read"] == 0
                and storage["full_history_bytes_read"] == 0
            )
            if window.get("zero_full_history_reads") is not zero_full_history:
                _fail(f"performance lane {lane_name} full-history summary differs")
            expected_bounded_index = (
                transactional["page_reads"]
                <= 50
                * (
                    MAX_PROPOSAL_PAGE_READS_PER_ROUND
                    + 5 * MAX_PROPOSAL_PAGE_READS_PER_ROUND
                    + 6 * MAX_APPLY_PAGE_READS_PER_VALIDATOR_ROUND
                )
                if lane_name == "selected-indexed"
                else False
            )
            if window.get("bounded_index_pages") is not expected_bounded_index:
                _fail(f"performance lane {lane_name} bounded-index summary differs")
            expected_constant_accumulator = (
                transactional["page_writes"]
                <= 50 * 6 * MAX_APPLY_PAGE_WRITES_PER_VALIDATOR_ROUND
                if lane_name == "selected-indexed"
                else False
            )
            if (
                window.get("constant_accumulator_work")
                is not expected_constant_accumulator
            ):
                _fail(f"performance lane {lane_name} accumulator summary differs")
            if any(transactional[key] != 0 for key in TRANSACTIONAL_COUNTER_FIELDS):
                if lane_name != "selected-indexed":
                    _fail(
                        f"performance lane {lane_name} unexpectedly used transactional storage"
                    )
            if lane_name == "selected-indexed":
                if transactional["committed_write_transactions"] != 300:
                    _fail("selected performance window did not commit once per validator")
                if (
                    storage["fsync_count"] != 300
                    or storage["full_history_records_read"] != 0
                    or storage["full_history_bytes_read"] != 0
                    or transactional["full_history_scans"] != 0
                ):
                    _fail("selected performance work gate differs")
            elif (
                legacy["legacy_prefix_records_verified"] <= 0
                or legacy["legacy_prefix_bytes_read"] <= 0
                or legacy["ordered_history_records_read"] <= 0
                or legacy["ordered_history_bytes_read"] <= 0
                or legacy["ordered_index_slots_read"] != 0
                or legacy["ordered_index_slots_written"] != 0
            ):
                _fail("legacy performance work gate differs")
            expected_resource_counters = {
                "page_reads": transactional["page_reads"],
                "page_writes": transactional["page_writes"],
                "fsync_count": storage["fsync_count"],
                "fsync_micros": transactional["durable_commit_micros"],
            }
            for key, expected_value in expected_resource_counters.items():
                if resources.get(key) != expected_value:
                    _fail(f"performance lane {lane_name} resource {key} differs")

            raw = _bound_json(
                packet_dir,
                checksums,
                {
                    "path": window.get("normalized_report"),
                    "sha256": window.get("normalized_report_sha256"),
                },
                f"performance lane {lane_name} window report",
            )
            if (
                raw.get("schema") != "postfiat-real-transaction-latency-benchmark-v1"
                or raw.get("status") != "passed"
            ):
                _fail(f"performance lane {lane_name} normalized window did not pass")
            config = _object(raw.get("config"), "performance raw configuration")
            expected_config = {
                "mode": "wallet-to-finality",
                "build_mode": "release",
                "transport": "local-loopback-persistent-validator-services",
                "validators": 6,
                "rounds": 50,
                "vote_policy": "full",
                "timeout_ms": PERFORMANCE_QUALIFICATION_TIMEOUT_MS,
                "amount": 10,
                "wallet_address": lane["wallet_address"],
                "recipient": lane["recipient_address"],
                "input_source": "signed-transfer-corpus",
                "signed_transfer_corpus": "$SIGNED_TRANSFER_CORPUS",
                "signed_transfer_corpus_sha256": snapshot_binding["corpus_sha256"],
                "signed_transfer_corpus_offset": 0,
            }
            for key, expected_value in expected_config.items():
                if config.get(key) != expected_value:
                    _fail(
                        f"performance lane {lane_name} raw configuration {key} differs"
                    )
            if lane_name == "selected-indexed":
                if (
                    config.get("resident_transactional_store") is not True
                    or config.get("expected_start_height") != height
                ):
                    _fail("selected performance raw storage mode is invalid")
            elif (
                config.get("resident_transactional_store") is not False
                or config.get("expected_start_height") is not None
            ):
                _fail(f"comparison performance lane {lane_name} used selected mode")
            checks = _object(raw.get("checks"), "performance raw checks")
            for key in (
                "all_receipts_accepted",
                "all_rounds_ok",
                "all_transactions_final",
                "all_vote_policies_match",
                "converged",
                "final_height_matches_rounds",
                "iteration_count_matches_rounds",
                "no_duplicate_receipts",
                "state_verified_after_run",
                "exact_input_binding",
            ):
                _bool(checks.get(key), f"performance raw check {key}")
            raw_final = _object(raw.get("final_state"), "performance raw final state")
            if (
                raw_final.get("height") != height + 50
                or raw_final.get("block_tip_hash") != final_tip
                or raw_final.get("state_root") != final_root
                or raw_final.get("state_verification_count") != 6
            ):
                _fail(f"performance lane {lane_name} raw final state differs")
            iterations = _list(raw.get("iterations"), "performance iterations")
            if len(iterations) != 50:
                _fail(f"performance lane {lane_name} window lacks 50 iterations")
            expected_transaction_identities = snapshot_binding[
                "transaction_identities"
            ]
            observed_transaction_identities: list[tuple[str, str]] = []
            stage_samples = {stage: [] for stage in MATERIAL_STAGE_PATHS}
            for iteration_index, iteration_value in enumerate(iterations, start=1):
                iteration = _object(iteration_value, "performance iteration")
                for key in (
                    "round_ok",
                    "receipt_accepted",
                    "finality_confirmed",
                    "all_sends_verified",
                    "all_vote_requests_verified",
                ):
                    _bool(iteration.get(key), f"performance iteration {key}")
                if (
                    iteration.get("iteration") != iteration_index
                    or iteration.get("block_height") != height + iteration_index
                    or iteration.get("quorum") != 5
                    or HEX96.fullmatch(str(iteration.get("block_hash", ""))) is None
                    or HEX96.fullmatch(str(iteration.get("certificate_id", ""))) is None
                    or iteration.get("input_source") != "signed-transfer-corpus"
                    or iteration.get("signed_transfer_corpus_index")
                    != iteration_index - 1
                    or HEX96.fullmatch(str(iteration.get("tx_id", ""))) is None
                    or HEX64.fullmatch(
                        str(iteration.get("signed_transfer_sha256", ""))
                    )
                    is None
                ):
                    _fail(f"performance lane {lane_name} iteration identity is invalid")
                transaction_identity = (
                    str(iteration["tx_id"]),
                    str(iteration["signed_transfer_sha256"]),
                )
                if (
                    transaction_identity
                    != expected_transaction_identities[iteration_index - 1]
                ):
                    _fail(
                        f"performance lane {lane_name} iteration did not consume "
                        "the bound signed corpus"
                    )
                observed_transaction_identities.append(transaction_identity)
                for metric in samples:
                    metric_value = iteration.get(metric)
                    if not isinstance(metric_value, (int, float)) or metric_value <= 0:
                        _fail(f"performance iteration {metric} is invalid")
                    samples[metric].append(float(metric_value))
                for stage, path in MATERIAL_STAGE_PATHS.items():
                    stage_samples[stage].append(
                        _nested_stage_value(iteration, stage, path)
                    )
            _verify_vote_lock_work(
                packet_dir,
                checksums,
                window,
                raw,
                lane_name,
            )
            raw_storage = _recompute_raw_storage_work(raw, lane_name)
            for field in TRANSACTIONAL_COUNTER_FIELDS:
                if field in ("full_history_records_read", "full_history_bytes_read"):
                    continue
                if storage.get(field) != transactional[field]:
                    _fail(
                        f"performance lane {lane_name} flattened transactional "
                        f"counter {field} differs"
                    )
            for counter_group in ("transactional", "legacy"):
                if raw_storage[counter_group] != storage[counter_group]:
                    _fail(
                        f"performance lane {lane_name} {counter_group} storage "
                        "summary differs from raw stage telemetry"
                    )
            for field in (
                "full_history_records_read",
                "full_history_bytes_read",
                "fsync_count",
            ):
                if raw_storage[field] != storage[field]:
                    _fail(
                        f"performance lane {lane_name} storage {field} differs "
                        "from raw stage telemetry"
                    )
            for stage, values in stage_samples.items():
                window_p95 = _percentile(values, 0.95)
                stage_observations[stage].append(
                    {
                        "height": height,
                        "window": str(window["label"]),
                        "p95_ms": window_p95,
                    }
                )
                if height == HEIGHTS[0]:
                    height_50_stage_p95[stage].append(window_p95)
            row_verified_windows.append(
                {
                    "initial_tip": initial_tip,
                    "initial_state_root": initial_root,
                    "final_state_root": final_root,
                    "transaction_identities": tuple(
                        observed_transaction_identities
                    ),
                }
            )

        if len(row_source_snapshots) != 1 or len(row_initial_identities) != 1:
            _fail(
                f"performance lane {lane_name} height {height} windows did not share one snapshot"
            )
        verified_windows[height] = row_verified_windows
        aggregate = _object(row.get("aggregate"), "performance aggregate")
        for metric, values in samples.items():
            observed = _object(aggregate.get(metric), f"performance {metric} aggregate")
            _verify_numeric_structure(
                observed,
                _distribution_summary(values),
                f"performance lane {lane_name} aggregate {metric}",
            )
        resource_variance = _object(
            row.get("resource_variance"),
            f"performance lane {lane_name} resource variance",
        )
        if set(resource_variance) != set(PERFORMANCE_RESOURCE_FIELDS):
            _fail(f"performance lane {lane_name} resource variance fields differ")
        for field in PERFORMANCE_RESOURCE_FIELDS:
            values = [
                float(_object(window, "performance window")["resources"][field])
                for window in windows
            ]
            _verify_numeric_structure(
                resource_variance[field],
                _distribution_summary(values),
                f"performance lane {lane_name} resource variance {field}",
            )

    envelope = _object(
        lane.get("height_relationship_model"), "height relationship model"
    )
    if lane_name == "selected-indexed":
        _verify_height_relationship_models(
            lane,
            stage_observations,
            height_50_stage_p95,
            reject_material_positive=True,
        )
        stages = _object(envelope.get("stages"), "height relationship stages")
        expected_no_positive = all(
            _object(stages.get(stage), f"height relationship stage {stage}").get(
                "material_positive_linear_relationship"
            )
            is False
            for stage in MATERIAL_STAGE_PATHS
        )
    else:
        if (
            envelope.get("schema") != "postfiat-storage-height-cost-model-v2"
            or envelope.get("sample_kind") != "per_window_p95"
            or envelope.get("relative_materiality") != MODEL_RELATIVE_MATERIALITY
            or envelope.get("residual_sigmas") != MODEL_RESIDUAL_SIGMAS
            or envelope.get("stages") != {}
        ):
            _fail("legacy performance lane unexpectedly fit a height model")
        expected_no_positive = None
    if lane.get("no_positive_linear_height_relationship") is not expected_no_positive:
        _fail(f"performance lane {lane_name} height relationship summary differs")
    if lane.get("comparison_windows_pass") is not True:
        _fail(f"performance lane {lane_name} comparison windows did not pass")
    expected_selected_gate = True if lane_name == "selected-indexed" else None
    if lane.get("selected_storage_gates_pass") is not expected_selected_gate:
        _fail(f"performance lane {lane_name} selected gate summary is invalid")
    return rows, verified_windows


def _verify_prepared_input_build(
    packet_dir: Path,
    checksums: Mapping[str, str],
    performance: Mapping[str, Any],
    source_revision: str,
    binary_digests: Mapping[str, str],
    snapshot_bindings: Mapping[int, Mapping[str, Any]],
    verified_windows: Mapping[str, Mapping[int, Sequence[Mapping[str, Any]]]],
) -> None:
    manifest = _bound_json(
        packet_dir,
        checksums,
        {
            "path": performance.get("prepared_input_manifest"),
            "sha256": performance.get("prepared_input_manifest_sha256"),
        },
        "performance prepared-input manifest",
    )
    if manifest.get("schema") != PREPARED_INPUT_MANIFEST_SCHEMA:
        _fail("performance prepared-input manifest schema is unsupported")
    expected_build = {
        "candidate": manifest.get("candidate"),
        "batch_builder": manifest.get("batch_builder"),
        "runner": manifest.get("runner"),
        "build": manifest.get("build"),
    }
    prepared_by = manifest.get("prepared_by")
    if prepared_by is not None:
        expected_build["prepared_by"] = prepared_by
    if performance.get("prepared_input_build") != expected_build:
        _fail("performance prepared-input build identity differs from the manifest")
    candidate = _object(manifest.get("candidate"), "prepared-input candidate")
    node_build = _object(
        candidate.get("node_binary_build"), "prepared-input node build"
    )
    if (
        candidate.get("source_revision") != source_revision
        or candidate.get("node_binary_sha256")
        != binary_digests["bin/postfiat-node"]
        or node_build.get("git_revision") != source_revision[:8]
        or node_build.get("profile") != "release"
    ):
        _fail("performance prepared-input candidate binding differs")
    batch_builder = _object(
        manifest.get("batch_builder"), "prepared-input batch builder"
    )
    builder_build = _object(
        batch_builder.get("build"), "prepared-input batch builder build"
    )
    runner = _object(manifest.get("runner"), "prepared-input runner")
    runner_revision = str(runner.get("source_revision", ""))
    if (
        batch_builder.get("binary_sha256")
        != binary_digests["bin/postfiat-storage-corpus-batches"]
        or HEX40.fullmatch(runner_revision) is None
        or builder_build.get("git_revision") != runner_revision[:8]
        or builder_build.get("profile") != "release"
        or runner.get("vote_lock_work_gate_schema")
        != VOTE_LOCK_WORK_GATE_SCHEMA
        or HEX96.fullmatch(str(runner.get("spec_sha3_384", ""))) is None
        or any(
            HEX64.fullmatch(str(runner.get(field, ""))) is None
            for field in (
                "paired_runner_sha256",
                "selected_runner_sha256",
                "shared_runner_sha256",
            )
        )
    ):
        _fail("performance prepared-input helper or runner binding differs")
    build_batch_builder = batch_builder
    build_builder_build = builder_build
    if prepared_by is not None:
        prepared_by_object = _object(prepared_by, "prepared-input prepared-by")
        prepared_candidate = _object(
            prepared_by_object.get("candidate"),
            "prepared-input prepared-by candidate",
        )
        prepared_node_build = _object(
            prepared_candidate.get("node_binary_build"),
            "prepared-input prepared-by node build",
        )
        prepared_runner = _object(
            prepared_by_object.get("runner"),
            "prepared-input prepared-by runner",
        )
        prepared_runner_revision = str(prepared_runner.get("source_revision", ""))
        build_batch_builder = _object(
            prepared_by_object.get("batch_builder"),
            "prepared-input prepared-by batch builder",
        )
        build_builder_build = _object(
            build_batch_builder.get("build"),
            "prepared-input prepared-by batch builder build",
        )
        prepared_source_revision = str(prepared_candidate.get("source_revision", ""))
        if (
            set(prepared_by_object)
            != {"source_manifest_sha256", "candidate", "batch_builder", "runner"}
            or HEX64.fullmatch(
                str(prepared_by_object.get("source_manifest_sha256", ""))
            )
            is None
            or HEX40.fullmatch(prepared_source_revision) is None
            or HEX64.fullmatch(
                str(prepared_candidate.get("node_binary_sha256", ""))
            )
            is None
            or prepared_node_build.get("git_revision")
            != prepared_source_revision[:8]
            or prepared_node_build.get("profile") != "release"
            or HEX40.fullmatch(prepared_runner_revision) is None
            or HEX96.fullmatch(
                str(prepared_runner.get("spec_sha3_384", ""))
            )
            is None
            or any(
                HEX64.fullmatch(str(prepared_runner.get(field, ""))) is None
                for field in (
                    "paired_runner_sha256",
                    "selected_runner_sha256",
                    "shared_runner_sha256",
                )
            )
            or HEX64.fullmatch(
                str(build_batch_builder.get("binary_sha256", ""))
            )
            is None
            or build_builder_build.get("git_revision")
            != prepared_runner_revision[:8]
            or build_builder_build.get("profile") != "release"
        ):
            _fail("performance prepared-input prepared-by binding differs")
    public = _object(manifest.get("public_inputs"), "prepared-input public inputs")
    private_bundle = _object(
        manifest.get("private_bundle"), "prepared-input private bundle reference"
    )
    topology_reference = _object(
        manifest.get("topology"), "prepared-input topology reference"
    )
    height_one_reference = _object(
        manifest.get("height_1_snapshot"),
        "prepared-input height-1 snapshot reference",
    )
    for label, reference in (
        ("private bundle", private_bundle),
        ("topology", topology_reference),
        ("height-1 snapshot", height_one_reference),
    ):
        path = reference.get("path")
        if (
            not isinstance(path, str)
            or not path
            or Path(path).is_absolute()
            or HEX64.fullmatch(str(reference.get("sha256", ""))) is None
        ):
            _fail(f"prepared-input {label} reference is invalid")
    selected_lane = _object(
        _object(performance.get("lanes"), "performance lanes").get(
            "selected-indexed"
        ),
        "selected performance lane",
    )
    if (
        public.get("topology_sha256") != selected_lane.get("topology_sha256")
        or public.get("height_1_snapshot_sha256")
        != selected_lane.get("height_1_snapshot_sha256")
        or public.get("validator_public_identities")
        != selected_lane.get("validator_public_identities")
        or topology_reference.get("sha256") != public.get("topology_sha256")
        or height_one_reference.get("sha256")
        != public.get("height_1_snapshot_sha256")
    ):
        _fail("performance prepared-input public inputs differ from measurement")

    imported = _object(
        performance.get("prepared_input_import"), "prepared-input import receipt"
    )
    imported_fleets = _list(
        imported.get("prepared_fleets"), "prepared-input imported fleets"
    )
    manifest_materials = _list(
        manifest.get("materials"), "prepared-input materials"
    )
    if (
        imported.get("private_bundle_source_sha256")
        != private_bundle.get("sha256")
        or imported.get("private_bundle_destination_sha256")
        != private_bundle.get("sha256")
        or imported.get("height_1_snapshot_destination_sha256")
        != height_one_reference.get("sha256")
        or len(imported_fleets) != len(manifest_materials)
    ):
        _fail("performance prepared-input copy receipt differs")
    for material_value, fleet_value in zip(manifest_materials, imported_fleets):
        material = _object(material_value, "prepared-input imported material")
        fleet = _object(fleet_value, "prepared-input fleet copy receipt")
        fleet_reference = _object(
            material.get("prepared_fleet"), "prepared-input fleet reference"
        )
        if (
            fleet.get("height") != material.get("height")
            or fleet.get("source_sha256") != fleet_reference.get("sha256")
            or fleet.get("destination_sha256") != fleet_reference.get("sha256")
        ):
            _fail("performance prepared-input source/destination fleet differs")
    imported_advances = _list(
        imported.get("advances"), "prepared-input imported advances"
    )
    advances = _list(manifest.get("advances"), "prepared-input advances")
    if not advances or len(imported_advances) != len(advances):
        _fail("performance prepared-input build receipts are incomplete")
    expected_start = 1
    aggregate = {field: 0 for field in PREPARED_BUILD_COUNTER_FIELDS}
    last_validators: list[Mapping[str, Any]] = []
    last_tip = ""
    last_root = ""
    for index, (advance_value, imported_value) in enumerate(
        zip(advances, imported_advances),
        start=1,
    ):
        advance = _object(advance_value, f"prepared-input advance {index}")
        imported_advance = _object(
            imported_value, f"prepared-input imported advance {index}"
        )
        start = advance.get("starting_height")
        final = advance.get("final_height")
        rounds = advance.get("rounds")
        if (
            type(start) is not int
            or type(final) is not int
            or type(rounds) is not int
            or start != expected_start
            or final <= start
            or rounds != final - start
            or imported_advance.get("unit_id") != advance.get("unit_id")
            or HEX64.fullmatch(
                str(advance.get("result_prepared_fleet_sha256", ""))
            )
            is None
        ):
            _fail("performance prepared-input advances are not contiguous from height 1")
        counters = _object(
            advance.get("counters"), f"prepared-input advance {index} counters"
        )
        if set(counters) != set(PREPARED_BUILD_COUNTER_FIELDS):
            _fail("performance prepared-input build counter set differs")
        for field in PREPARED_BUILD_COUNTER_FIELDS:
            value = counters.get(field)
            if type(value) is not int or value < 0:
                _fail(f"performance prepared-input counter {field} is invalid")
            aggregate[field] += value
        if (
            counters["committed_write_transactions"] != rounds * 6
            or any(
                counters[field] != 0
                for field in PREPARED_BUILD_ZERO_COUNTER_FIELDS
            )
        ):
            _fail("performance prepared-input build performed invalid storage work")
        manifest_receipt = _object(
            advance.get("receipt"), f"prepared-input advance {index} receipt binding"
        )
        manifest_report = _object(
            advance.get("report"), f"prepared-input advance {index} report binding"
        )
        if (
            imported_advance.get("source_receipt_sha256")
            != manifest_receipt.get("sha256")
            or imported_advance.get("source_report_sha256")
            != manifest_report.get("sha256")
        ):
            _fail("performance prepared-input build artifact digest differs")
        receipt = _bound_json(
            packet_dir,
            checksums,
            {
                "path": imported_advance.get("receipt"),
                "sha256": imported_advance.get("receipt_sha256"),
            },
            f"prepared-input advance {index} receipt",
        )
        raw_report = _bound_json(
            packet_dir,
            checksums,
            {
                "path": imported_advance.get("report"),
                "sha256": imported_advance.get("report_sha256"),
            },
            f"prepared-input advance {index} report",
        )
        if (
            raw_report.get("schema")
            != "postfiat-storage-scaling-persistent-advance-report-v1"
            or raw_report.get("status") != "passed"
        ):
            _fail(f"performance prepared-input advance {index} report did not pass")
        storage = _object(
            receipt.get("storage"), f"prepared-input advance {index} storage"
        )
        transactional = _object(
            storage.get("transactional"),
            f"prepared-input advance {index} transactional storage",
        )
        observed_counters: dict[str, int] = {}
        for field in PREPARED_BUILD_COUNTER_FIELDS:
            value = storage.get(field)
            if type(value) is not int or value < 0 or transactional.get(field) != value:
                _fail(
                    f"performance prepared-input receipt counter {field} differs"
                )
            observed_counters[field] = value
        last_validators_value = _list(
            receipt.get("final_fleet"),
            f"prepared-input advance {index} final fleet",
        )
        last_tip, last_root = _verify_performance_fleet(
            last_validators_value,
            f"prepared-input advance {index} final fleet",
            final,
        )
        last_validators = [
            _object(value, f"prepared-input advance {index} validator")
            for value in last_validators_value
        ]
        if (
            receipt.get("starting_height") != start
            or receipt.get("final_height") != final
            or receipt.get("rounds") != rounds
            or receipt.get("validators_converged") != 6
            or receipt.get("literal_receipts_exact") is not True
            or receipt.get("backend_work_gate_pass") is not True
            or receipt.get("zero_full_history_reads") is not True
            or observed_counters != counters
            or receipt.get("final_tip") != advance.get("final_tip")
            or receipt.get("final_state_root") != advance.get("final_state_root")
            or last_tip != advance.get("final_tip")
            or last_root != advance.get("final_state_root")
            or receipt.get("result_prepared_fleet_sha256")
            != advance.get("result_prepared_fleet_sha256")
            or receipt.get("batch_builder_binary_sha256")
            != build_batch_builder.get("binary_sha256")
            or receipt.get("batch_builder_build") != build_builder_build
        ):
            _fail(f"performance prepared-input advance {index} receipt differs")
        expected_start = final

    build = _object(manifest.get("build"), "prepared-input final build")
    final_validators = _list(
        build.get("final_validators"), "prepared-input build-final validators"
    )
    final_tip, final_root = _verify_performance_fleet(
        final_validators,
        "prepared-input build-final validators",
        expected_start,
    )
    build_elapsed = build.get("elapsed_seconds")
    if (
        expected_start != 5000
        or not isinstance(build_elapsed, (int, float))
        or isinstance(build_elapsed, bool)
        or not math.isfinite(float(build_elapsed))
        or float(build_elapsed) < 0
        or build.get("counters") != aggregate
        or build.get("final_height") != expected_start
        or build.get("final_tip") != last_tip
        or build.get("final_state_root") != last_root
        or build.get("final_tip") != final_tip
        or build.get("final_state_root") != final_root
        or final_validators != last_validators
        or build.get("final_prepared_fleet_sha256")
        != advances[-1].get("result_prepared_fleet_sha256")
    ):
        _fail("performance prepared-input final build identity differs")

    materials = manifest_materials
    performance_materials = _list(
        performance.get("materials_by_height"), "performance materials"
    )
    if [
        value.get("height") if isinstance(value, dict) else None for value in materials
    ] != HEIGHTS:
        _fail("performance prepared-input materials are incomplete")
    for manifest_value, performance_value in zip(materials, performance_materials):
        material = _object(manifest_value, "prepared-input material")
        measured = _object(performance_value, "performance material")
        height = int(material["height"])
        fleet_reference = _object(
            material.get("prepared_fleet"),
            f"prepared-input height {height} fleet",
        )
        corpus_reference = _object(
            material.get("signed_transfer_corpus"),
            f"prepared-input height {height} corpus",
        )
        expected_snapshot_sha256 = None
        if height == 50:
            expected_snapshot_sha256 = _object(
                material.get("snapshot"), "prepared-input height-50 snapshot"
            ).get("sha256")
        elif material.get("snapshot") is not None:
            _fail("performance prepared-input top material retained a snapshot")
        if (
            measured.get("height") != height
            or measured.get("prepared_fleet_sha256")
            != fleet_reference.get("sha256")
            or measured.get("snapshot_sha256") != expected_snapshot_sha256
            or measured.get("signed_transfer_corpus_sha256")
            != corpus_reference.get("sha256")
            or measured.get("transfer_count") != material.get("transfer_count")
            or measured.get("first_sequence") != material.get("first_sequence")
            or measured.get("last_sequence") != material.get("last_sequence")
            or snapshot_bindings[height]["prepared_fleet_sha256"]
            != fleet_reference.get("sha256")
            or any(
                measured.get(field) != material.get(field)
                for field in (
                    "corpus_source_mode",
                    "corpus_source_prepared_fleet_sha256",
                    "corpus_scratch_before_sha256",
                    "corpus_scratch_after_sha256",
                    "corpus_scratch_mutated",
                    "corpus_scratch_discarded",
                    "corpus_scratch_restored_sha256",
                )
            )
        ):
            _fail(f"performance prepared-input height {height} material differs")
    top_digest = build.get("final_prepared_fleet_sha256")
    selected_rows = _list(selected_lane.get("rows"), "selected performance rows")
    top_windows = _list(selected_rows[-1].get("windows"), "selected top windows")
    if any(window.get("prepared_fleet_sha256") != top_digest for window in top_windows):
        _fail("performance top-height window used a different build-final fleet")
    if any(
        window.get("initial_tip") != final_tip
        or window.get("initial_state_root") != final_root
        for window in verified_windows["selected-indexed"][5000]
    ):
        _fail("performance top-height window initial identity differs from build end")


def _verify_performance(
    packet_dir: Path,
    checksums: Mapping[str, str],
    performance: Mapping[str, Any],
    source_revision: str,
    binary_digests: Mapping[str, str],
) -> dict[str, float]:
    input_mode = performance.get("input_mode")
    if input_mode not in {None, "prepared-input-manifest"}:
        _fail("performance input mode is unsupported")
    if performance.get("status") != "PASS":
        _fail("performance campaign did not pass")
    if performance.get("campaign_mode") != "release-qualification":
        _fail("performance campaign is not a release qualification")
    if performance.get("evidence_eligible") is not True:
        _fail("performance campaign is not evidence eligible")
    if performance.get("source_worktree_clean") is not True:
        _fail("performance campaign source worktree was not clean")
    if (
        performance.get("offline") is not True
        or performance.get("network_contacted") is not False
        or performance.get("devnet_queried_or_mutated") is not False
    ):
        _fail("performance campaign execution mode is not offline-only")
    captured_at = performance.get("captured_at")
    if not isinstance(captured_at, str) or not captured_at.endswith("Z"):
        _fail("performance campaign capture time is missing or not UTC")
    try:
        datetime.fromisoformat(captured_at[:-1] + "+00:00")
    except ValueError:
        _fail("performance campaign capture time is invalid")
    if performance.get("source_revision") != source_revision:
        _fail("performance source revision differs from the packet source")
    if HEX40.fullmatch(str(performance.get("runner_source_revision", ""))) is None:
        _fail("performance runner checkout revision is invalid")
    current_binary_digest = binary_digests["bin/postfiat-node"]
    if performance.get("node_binary_sha256") != current_binary_digest:
        _fail("performance binary identity differs from the packet source")
    _verify_binary_build(performance, "performance", source_revision)
    if performance.get("validator_count") != 6:
        _fail("performance topology is not six validators")
    if performance.get("qualification_profile") != "time-budgeted-redb-v4":
        _fail("performance qualification profile differs")
    if (
        performance.get("batch_builder_binary_sha256")
        != binary_digests["bin/postfiat-storage-corpus-batches"]
        or performance.get("batch_builder_binary")
        != "postfiat-storage-corpus-batches"
    ):
        _fail("performance batch builder identity differs from the packet source")
    builder_build = _object(
        performance.get("batch_builder_build"), "performance batch builder build"
    )
    if builder_build.get("profile") != "release" or (
        input_mode is None
        and builder_build.get("git_revision")
        != str(performance["runner_source_revision"])[:8]
    ):
        _fail("performance batch builder build differs from the runner source")
    if performance.get("windows_per_height") != 5 or performance.get("rounds_per_window") != 50:
        _fail("performance window cardinality differs from the specification")
    if performance.get("lane_order") != list(PERFORMANCE_LANES):
        _fail("performance lane order differs from the time-budgeted profile")
    if performance.get("lane_height_matrix") != [
        {"lane": "selected-indexed", "height": 50},
        {"lane": "selected-indexed", "height": 5000},
        {"lane": "legacy-jsonl", "height": 50},
    ]:
        _fail("performance lane/height matrix differs")
    if performance.get("timeout_ms") != PERFORMANCE_QUALIFICATION_TIMEOUT_MS:
        _fail("performance timeout policy differs from the closed envelope")
    maximum = performance.get("max_wall_seconds")
    elapsed = performance.get("elapsed_wall_seconds")
    if (
        maximum != 4 * 60 * 60
        or not isinstance(elapsed, (int, float))
        or isinstance(elapsed, bool)
        or not math.isfinite(float(elapsed))
        or float(elapsed) < 0
        or float(elapsed) > maximum
    ):
        _fail("performance wall-clock budget differs or was exceeded")
    lanes = _object(performance.get("lanes"), "performance lanes")
    if set(lanes) != set(PERFORMANCE_LANES):
        _fail("performance report does not contain exactly two lanes")
    selected_lane = _object(lanes["selected-indexed"], "selected performance lane")
    wallet_address = str(selected_lane.get("wallet_address", ""))
    recipient_address = str(selected_lane.get("recipient_address", ""))
    if not wallet_address or not recipient_address:
        _fail("performance workload accounts are missing")

    snapshot_entries = _list(
        performance.get("materials_by_height"),
        "performance shared height materials and corpora",
    )
    if [
        entry.get("height") if isinstance(entry, dict) else None
        for entry in snapshot_entries
    ] != HEIGHTS:
        _fail("performance shared snapshot/corpus heights differ")
    snapshot_bindings: dict[int, Mapping[str, Any]] = {}
    for entry_value in snapshot_entries:
        entry = _object(entry_value, "performance shared snapshot/corpus entry")
        height = int(entry["height"])
        snapshot_value = entry.get("snapshot")
        snapshot_sha256 = entry.get("snapshot_sha256")
        if height == HEIGHTS[0]:
            snapshot_path = str(snapshot_value or "")
            valid_snapshot = (
                HEX64.fullmatch(str(snapshot_sha256 or "")) is not None
                and _safe_relative(snapshot_path).as_posix() == snapshot_path
            )
            expected_corpus_mode = "authenticated-portable-snapshot-import"
            expected_corpus_fleet = None
            valid_scratch = all(
                entry.get(field) is None
                for field in (
                    "corpus_scratch_before_sha256",
                    "corpus_scratch_after_sha256",
                    "corpus_scratch_mutated",
                    "corpus_scratch_discarded",
                    "corpus_scratch_restored_sha256",
                )
            )
        else:
            valid_snapshot = snapshot_value is None and snapshot_sha256 is None
            expected_corpus_mode = "disposable-canonical-prepared-fleet-clone"
            expected_corpus_fleet = entry.get("prepared_fleet_sha256")
            scratch_before = entry.get("corpus_scratch_before_sha256")
            scratch_after = entry.get("corpus_scratch_after_sha256")
            valid_scratch = (
                HEX64.fullmatch(str(scratch_before or "")) is not None
                and HEX64.fullmatch(str(scratch_after or "")) is not None
                and scratch_before == entry.get("prepared_fleet_sha256")
                and entry.get("corpus_scratch_restored_sha256")
                == entry.get("prepared_fleet_sha256")
                and entry.get("corpus_scratch_mutated")
                is (scratch_before != scratch_after)
                and entry.get("corpus_scratch_discarded") is True
            )
        if (
            not valid_snapshot
            or not valid_scratch
            or HEX64.fullmatch(str(entry.get("prepared_fleet_sha256", ""))) is None
            or entry.get("corpus_source_mode") != expected_corpus_mode
            or entry.get("corpus_source_prepared_fleet_sha256")
            != expected_corpus_fleet
            or entry.get("transfer_count") != 50
        ):
            _fail(f"performance height {height} snapshot/corpus binding is invalid")
        transaction_identities, first_sequence, last_sequence = (
            _verify_signed_transfer_corpus(
                packet_dir,
                checksums,
                entry.get("signed_transfer_corpus"),
                entry.get("signed_transfer_corpus_sha256"),
                wallet_address=wallet_address,
                recipient_address=recipient_address,
                transfer_count=50,
                label=f"performance height {height} signed transfer corpus",
            )
        )
        if (
            entry.get("first_sequence") != first_sequence
            or entry.get("last_sequence") != last_sequence
        ):
            _fail(f"performance height {height} corpus sequence binding differs")
        snapshot_bindings[height] = {
            "snapshot_sha256": snapshot_sha256,
            "prepared_fleet_sha256": entry["prepared_fleet_sha256"],
            "corpus_path": entry["signed_transfer_corpus"],
            "corpus_sha256": entry["signed_transfer_corpus_sha256"],
            "transaction_identities": transaction_identities,
        }

    verified_rows: dict[str, list[Mapping[str, Any]]] = {}
    verified_windows: dict[str, dict[int, list[Mapping[str, Any]]]] = {}
    lane_sources: set[str] = set()
    lane_binaries: set[str] = set()
    for lane_name in PERFORMANCE_LANES:
        lane = _object(lanes.get(lane_name), f"performance lane {lane_name}")
        rows, windows = _verify_performance_lane(
            packet_dir,
            checksums,
            lane,
            lane_name,
            current_binary_digest,
            source_revision,
            snapshot_bindings,
        )
        verified_rows[lane_name] = rows
        verified_windows[lane_name] = windows
        lane_sources.add(str(lane.get("source_revision")))
        lane_binaries.add(str(lane.get("node_binary_sha256")))
    if lane_sources != {source_revision} or lane_binaries != {current_binary_digest}:
        _fail("performance lanes did not use one source revision and binary")
    if input_mode == "prepared-input-manifest":
        _verify_prepared_input_build(
            packet_dir,
            checksums,
            performance,
            source_revision,
            binary_digests,
            snapshot_bindings,
            verified_windows,
        )
    for height in [50]:
        for window_index in range(5):
            compared = [
                verified_windows[lane_name][height][window_index]
                for lane_name in PERFORMANCE_LANES
            ]
            if (
                len(
                    {
                        (window["initial_tip"], window["initial_state_root"])
                        for window in compared
                    }
                )
                != 1
                or len({window["final_state_root"] for window in compared}) != 1
                or len(
                    {
                        window["transaction_identities"]
                        for window in compared
                    }
                )
                != 1
            ):
                _fail(
                    f"performance height {height} window {window_index + 1} "
                    "is not an exact cross-backend comparison"
                )
    if performance.get("rows") != selected_lane.get("rows"):
        _fail("top-level performance rows differ from the selected lane")

    pairing = _object(performance.get("pairing"), "performance pairing")
    required_pairing_checks = (
        "same_host",
        "same_source_revision",
        "same_binary",
        "same_chain_id",
        "same_validator_count",
        "same_validator_keys",
        "same_topology_file",
        "same_authenticated_snapshot_at_shared_height",
        "same_signed_transactions_at_shared_height",
        "same_wallet_and_recipient_accounts",
        "same_window_cardinality_at_shared_height",
        "same_full_vote_policy",
        "same_timeout_policy",
        "same_host_allocation",
        "same_storage_medium",
        "same_final_state_for_identical_inputs",
    )
    if set(pairing) != {
        *required_pairing_checks,
        "shared_comparison_height",
        "changed_input_at_shared_height",
    }:
        _fail("performance pairing field set differs from the profile")
    for key in required_pairing_checks:
        _bool(pairing.get(key), f"performance pairing {key}")
    if pairing.get("shared_comparison_height") != 50:
        _fail("performance shared comparison height differs")
    if (
        pairing.get("changed_input_at_shared_height")
        != "authenticated node-local storage backend mode only"
    ):
        _fail("performance comparison changed more than the backend selector")
    accounts = {
        (lane.get("wallet_address"), lane.get("recipient_address"))
        for lane in lanes.values()
        if isinstance(lane, dict)
    }
    if len(accounts) != 1:
        _fail("performance lanes did not use the same derived accounts")
    validator_key_sets = {
        json.dumps(lane.get("validator_public_identities"), sort_keys=True)
        for lane in lanes.values()
        if isinstance(lane, dict)
    }
    if len(validator_key_sets) != 1:
        _fail("performance lanes did not use the same validator keys")
    topology_and_seed_snapshots = {
        (lane.get("topology_sha256"), lane.get("height_1_snapshot_sha256"))
        for lane in lanes.values()
        if isinstance(lane, dict)
    }
    if len(topology_and_seed_snapshots) != 1:
        _fail("performance lanes did not use the same topology and seed snapshot")
    host = _object(performance.get("host"), "performance host identity")
    if (
        not isinstance(host.get("cpu_affinity"), list)
        or not host["cpu_affinity"]
        or not all(isinstance(cpu, int) and cpu >= 0 for cpu in host["cpu_affinity"])
        or not isinstance(host.get("campaign_root_device"), int)
        or not isinstance(host.get("filesystem_block_size_bytes"), int)
        or host["filesystem_block_size_bytes"] <= 0
    ):
        _fail("performance host allocation or storage medium is unbound")
    lane_environments = [
        _object(lane.get("environment"), "performance lane environment")
        for lane in lanes.values()
        if isinstance(lane, dict)
    ]
    lane_affinities = {
        tuple(environment["cpu_affinity"]) for environment in lane_environments
    }
    lane_storage_media = {
        (
            environment["filesystem_device"],
            environment["filesystem_block_size_bytes"],
        )
        for environment in lane_environments
    }
    if (
        len(lane_affinities) != 1
        or len(lane_storage_media) != 1
        or tuple(host["cpu_affinity"]) != next(iter(lane_affinities))
        or (
            host["campaign_root_device"],
            host["filesystem_block_size_bytes"],
        )
        != next(iter(lane_storage_media))
    ):
        _fail("performance lanes did not share one host allocation and storage medium")

    legacy_rows = verified_rows["legacy-jsonl"]
    selected_rows = verified_rows["selected-indexed"]
    baseline = _object(
        performance.get("legacy_height_50_baseline"), "legacy height-50 baseline"
    )
    ratios: dict[str, float] = {}
    for metric in ("consensus_round_ms", "wallet_to_finality_ms"):
        legacy_50 = _metric_p95(legacy_rows[0], metric)
        recorded_legacy = baseline.get(metric)
        if not isinstance(recorded_legacy, (int, float)) or not math.isclose(
            float(recorded_legacy), legacy_50, rel_tol=1e-12, abs_tol=1e-9
        ):
            _fail(f"legacy baseline {metric} does not derive from the raw lane")
        selected_50 = _metric_p95(selected_rows[0], metric)
        selected_5000 = _metric_p95(selected_rows[-1], metric)
        ratios[f"{metric}_height50_vs_legacy"] = selected_50 / legacy_50
        ratios[f"{metric}_height5000_vs_height50"] = selected_5000 / selected_50
    recorded_ratios = _object(performance.get("ratios"), "performance ratios")
    if set(recorded_ratios) != set(ratios):
        _fail("performance ratio set differs from the required gates")
    for key, expected in ratios.items():
        recorded = recorded_ratios.get(key)
        if not isinstance(recorded, (int, float)) or not math.isclose(
            float(recorded), expected, rel_tol=1e-12, abs_tol=1e-9
        ):
            _fail(f"performance ratio {key} differs from raw lanes")
        if expected > 1.10:
            _fail(f"performance ratio exceeds 110% for {key}")
    _bool(performance.get("comparison_windows_pass"), "comparison window gate")
    _bool(performance.get("window_gates_pass"), "selected window gate")
    _bool(
        performance.get("vote_lock_work_gates_pass"),
        "vote-lock work gate",
    )
    _bool(
        performance.get("no_positive_linear_height_relationship"),
        "height relationship gate",
    )
    if performance.get("height_relationship_model") != selected_lane.get(
        "height_relationship_model"
    ):
        _fail("top-level height relationship model differs from selected lane")
    return ratios


def _verify_original_e3_manifest(
    manifest: Mapping[str, Any],
    source_revision: str,
    label: str,
) -> None:
    binding = _object(manifest.get("live_binding"), f"{label} live binding")
    sources = _list(manifest.get("source_files"), f"{label} source files")
    rebound = _object(manifest.get("rebound_from"), f"{label} rebound provenance")
    if (
        manifest.get("schema")
        != "postfiat-cobalt-adversarial-e3-campaign-manifest-v1"
        or manifest.get("campaign_id") != "cobalt-e3-adversarial-recovery-v1"
        or manifest.get("source_revision") != source_revision
        or manifest.get("history_entry_count") != 4
        or tuple(_list(manifest.get("tamper_cases"), f"{label} tamper cases"))
        != ORIGINAL_E3_TAMPER_CASES
        or tuple(
            _list(manifest.get("forged_catch_up_cases"), f"{label} forged cases")
        )
        != ORIGINAL_E3_FORGED_CASES
        or binding.get("validators") != [f"validator-{index}" for index in range(6)]
        or binding.get("quorum") != 5
        or rebound.get("path")
        != "benchmarks/cobalt-adversarial-verification/e3/campaign-manifest.json"
        or rebound.get("sha256") != ORIGINAL_E3_MANIFEST_SHA256
        or rebound.get("policy")
        != "same frozen cases and live binding, current source hashes"
    ):
        _fail(f"{label} does not preserve the frozen E3 campaign boundary")
    observed_paths: list[str] = []
    for raw_source in sources:
        source = _object(raw_source, f"{label} source file")
        observed_paths.append(str(source.get("path", "")))
        if HEX64.fullmatch(str(source.get("sha256", ""))) is None:
            _fail(f"{label} source file hash is invalid")
    if tuple(observed_paths) != ORIGINAL_E3_SOURCE_PATHS:
        _fail(f"{label} source file set differs from the frozen boundary")


def _verify_original_e3_campaign(
    packet_dir: Path,
    checksums: Mapping[str, str],
    reference: Any,
    manifest: Mapping[str, Any],
    manifest_sha256: str,
    source_revision: str,
    label: str,
) -> None:
    _verify_original_e3_manifest(manifest, source_revision, f"{label} manifest")
    campaign = _bound_json(packet_dir, checksums, reference, label)
    if campaign.get("schema") != "postfiat-cobalt-adversarial-e3-campaign-v1":
        _fail(f"{label} schema is unsupported")
    summary = _object(campaign.get("summary"), f"{label} summary")
    expected_summary = {
        "manifest_sha256": manifest_sha256,
        "source_revision": source_revision,
        "validator_count": 6,
        "tamper_case_count": 24,
        "forged_catch_up_case_count": 18,
        "recovery_case_count": 6,
        "rejected_case_count": 42,
        "durable_mutation_count": 0,
        "signed_evidence_count": 18,
        "byte_identical_recovery_count": 6,
        "manual_repair_action_count": 0,
        "summary_only": False,
        "pass": True,
    }
    for field, expected in expected_summary.items():
        if summary.get(field) != expected:
            _fail(f"{label} summary field {field} disagrees with the frozen campaign")
    if summary.get("signed_evidence_verified") is not True:
        _fail(f"{label} signed evidence was not verified")

    cases = _list(campaign.get("cases"), f"{label} cases")
    recoveries = _list(campaign.get("recoveries"), f"{label} recoveries")
    if len(cases) != 42 or len(recoveries) != 6:
        _fail(f"{label} did not include all 48 frozen cases")
    for raw_case in cases:
        case = _object(raw_case, f"{label} case")
        if (
            case.get("ok") is not True
            or case.get("detected_before_rejoin") is not True
            or case.get("durable_state_mutated") is not False
            or case.get("state_hash_before") != case.get("state_hash_after")
            or case.get("journal_sha256_before") != case.get("journal_sha256_after")
        ):
            _fail(f"{label} contains an unsafe rejected case")
    for raw_recovery in recoveries:
        recovery = _object(raw_recovery, f"{label} recovery")
        if (
            recovery.get("ok") is not True
            or recovery.get("byte_identical") is not True
            or recovery.get("restart_succeeded") is not True
            or recovery.get("no_manual_repair") is not True
            or recovery.get("honest_history_sha256")
            != recovery.get("restored_history_sha256")
        ):
            _fail(f"{label} contains an unsafe recovery case")


def _verify_compatible_rollback(
    report: Mapping[str, Any],
    source_revision: str,
    current_binary_digest: str,
    rollback_binary_digest: str,
) -> None:
    if (
        report.get("schema") != "postfiat-storage-compatible-rollback-v1"
        or report.get("status") != "PASS"
        or report.get("evidence_eligible") is not True
        or report.get("source_revision") != source_revision
        or report.get("chain_id") != "postfiat-storage-scaling-local-v1"
        or report.get("validator_count") != 6
        or report.get("storage_activation_height") != 1
        or report.get("consensus_activation_height") != 2
        or report.get("network_contacted") is not False
        or report.get("devnet_queried_or_mutated") is not False
    ):
        _fail("compatible rollback report identity or execution mode is invalid")
    for key in (
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
    ):
        _bool(report.get(key), f"compatible rollback {key}")

    current = _object(report.get("current_binary"), "current rollback binary")
    rollback = _object(report.get("rollback_binary"), "older rollback binary")
    if (
        current.get("source_revision") != source_revision
        or current.get("git_revision") != source_revision[:8]
        or current.get("profile") != "release"
        or current.get("sha256") != current_binary_digest
    ):
        _fail("current rollback binary is not bound to packet source")
    rollback_revision = str(rollback.get("source_revision", ""))
    if (
        HEX40.fullmatch(rollback_revision) is None
        or rollback_revision == source_revision
        or rollback.get("git_revision") != rollback_revision[:8]
        or rollback.get("profile") != "release"
        or rollback.get("sha256") != rollback_binary_digest
        or rollback.get("sha256") == current.get("sha256")
    ):
        _fail("older rollback binary identity is invalid or unbound")

    identities = _object(report.get("identities"), "compatible rollback identities")
    expected_heights = {
        "current_post_activation": 2,
        "rollback_resume_input": 2,
        "rollback_finalized": 3,
        "forward_resume_input": 3,
        "forward_finalized": 4,
    }
    parsed: dict[str, Mapping[str, Any]] = {}
    for label, height in expected_heights.items():
        identity = _object(identities.get(label), f"compatible rollback {label}")
        if identity.get("height") != height:
            _fail(f"compatible rollback {label} height is invalid")
        for key in ("tip", "state_root"):
            if HEX96.fullmatch(str(identity.get(key, ""))) is None:
                _fail(f"compatible rollback {label} {key} is invalid")
        parsed[label] = identity
    if (
        parsed["rollback_resume_input"] != parsed["current_post_activation"]
        or parsed["forward_resume_input"] != parsed["rollback_finalized"]
    ):
        _fail("compatible rollback did not resume the exact certified tips")


def _verify_tamper(
    packet_dir: Path,
    checksums: Mapping[str, str],
    matrix: Mapping[str, Any],
    source_revision: str,
    current_binary_digest: str,
    rollback_binary_digest: str,
) -> int:
    if matrix.get("status") != "PASS":
        _fail("tamper matrix did not pass")
    if matrix.get("coverage_complete") is not True:
        _fail("tamper matrix coverage is incomplete")
    if matrix.get("uncovered_requirements") != []:
        _fail("tamper matrix retains uncovered requirements")
    if matrix.get("source_revision") != source_revision:
        _fail("tamper matrix source revision disagrees with the packet")
    if matrix.get("offline") is not True or matrix.get("network_contacted") is not False:
        _fail("tamper matrix is not offline evidence")
    cases = _list(matrix.get("cases"), "tamper cases")
    observed: set[str] = set()
    observed_tests: set[tuple[str, str]] = set()
    for value in cases:
        case = _object(value, "tamper case")
        name = str(case.get("name", ""))
        if not name or name in observed:
            _fail("tamper case name is empty or duplicated")
        observed.add(name)
        _bool(case.get("passed"), f"tamper case {name}")
        _bool(case.get("no_partial_mutation"), f"tamper case {name} mutation gate")
        if case.get("reason_code") != REQUIRED_TAMPER_REASONS.get(name):
            _fail(f"tamper case {name} reason code is not the frozen classification")
        if case.get("terminal_state") not in {
            "rejected_voting_blocked",
            "recovered_old_tip",
            "recovered_new_tip",
        }:
            _fail(f"tamper case {name} terminal state is invalid")
        receipt = _bound_json(
            packet_dir,
            checksums,
            case.get("receipt"),
            f"tamper receipt {name}",
        )
        if receipt.get("schema") != "postfiat-storage-tamper-receipt-v1":
            _fail(f"tamper case {name} receipt schema is unsupported")
        if receipt.get("source_revision") != source_revision:
            _fail(f"tamper receipt {name} source revision disagrees with the packet")
        if receipt.get("offline") is not True or receipt.get("network_contacted") is not False:
            _fail(f"tamper receipt {name} is not offline evidence")
        for key in (
            "name",
            "passed",
            "reason_code",
            "no_partial_mutation",
            "terminal_state",
        ):
            if receipt.get(key) != case.get(key):
                _fail(f"tamper receipt {name} disagrees on {key}")
        tests = _list(receipt.get("test_receipts"), f"tamper receipt {name} tests")
        if not tests:
            _fail(f"tamper receipt {name} has no executable test evidence")
        case_tests: set[tuple[str, str]] = set()
        for raw_test in tests:
            test = _object(raw_test, f"tamper receipt {name} test")
            package = str(test.get("package", ""))
            test_filter = str(test.get("test_filter", ""))
            if not package or not test_filter or test.get("result") != "passed":
                _fail(f"tamper receipt {name} contains an invalid test result")
            if test_filter == "__full_campaign__":
                if package != "postfiat-cobalt-e3-harness":
                    _fail(f"tamper receipt {name} E3 package identity is invalid")
                if type(test.get("executed_case_count")) is not int or test.get(
                    "executed_case_count"
                ) != 48:
                    _fail(f"tamper receipt {name} did not run the frozen E3 campaign")
                if HEX64.fullmatch(
                    str(test.get("verify_command_sha256", ""))
                ) is None:
                    _fail(f"tamper receipt {name} E3 campaign proof is incomplete")
                manifest_reference = _object(
                    test.get("manifest"),
                    f"tamper receipt {name} E3 manifest reference",
                )
                manifest = _bound_json(
                    packet_dir,
                    checksums,
                    manifest_reference,
                    f"tamper receipt {name} E3 manifest",
                )
                _verify_original_e3_campaign(
                    packet_dir,
                    checksums,
                    test.get("report"),
                    manifest,
                    str(manifest_reference.get("sha256", "")),
                    source_revision,
                    f"tamper receipt {name} E3 report",
                )
            elif (
                package == "postfiat-storage-rollback-rehearsal"
                and test_filter == "compatible_post_activation_software_rollback"
            ):
                if test.get("executed_test_count") != 1:
                    _fail("compatible rollback receipt has an invalid execution count")
                rollback_report = _bound_json(
                    packet_dir,
                    checksums,
                    test.get("report"),
                    "compatible rollback report",
                )
                _verify_compatible_rollback(
                    rollback_report,
                    source_revision,
                    current_binary_digest,
                    rollback_binary_digest,
                )
            elif type(test.get("executed_test_count")) is not int or test.get(
                "executed_test_count"
            ) < 1:
                _fail(f"tamper receipt {name} test filter executed zero tests")
            if HEX64.fullmatch(str(test.get("command_sha256", ""))) is None:
                _fail(f"tamper receipt {name} test command is not checksum-bound")
            for optional_hash in ("verify_command_sha256", "report_sha256"):
                if optional_hash in test and HEX64.fullmatch(str(test[optional_hash])) is None:
                    _fail(f"tamper receipt {name} {optional_hash} is invalid")
            test_identity = (package, test_filter)
            case_tests.add(test_identity)
            observed_tests.add(test_identity)
        if case.get("terminal_state") == "rejected_voting_blocked" and (
            "postfiat-node",
            "ambiguous_active_transactional_state_blocks_vote_without_mutation",
        ) not in case_tests:
            _fail(f"tamper case {name} lacks active-storage vote-block evidence")
        if name == "compatible_post_activation_software_rollback" and (
            "postfiat-storage-rollback-rehearsal",
            "compatible_post_activation_software_rollback",
        ) not in case_tests:
            _fail("compatible rollback case lacks a two-binary rehearsal")
    if observed != REQUIRED_TAMPER_CASES:
        _fail(
            "tamper matrix case set differs from the frozen set: "
            f"missing={sorted(REQUIRED_TAMPER_CASES - observed)} "
            f"unexpected={sorted(observed - REQUIRED_TAMPER_CASES)}"
        )
    required_evidence_tests = {
        ("postfiat-cobalt-e3-harness", "__full_campaign__"),
        (
            "postfiat-storage-rollback-rehearsal",
            "compatible_post_activation_software_rollback",
        ),
    }
    if not required_evidence_tests.issubset(observed_tests):
        _fail("tamper matrix omitted a required full-campaign evidence source")
    if matrix.get("unique_test_count") != len(observed_tests):
        _fail("tamper matrix unique test count is inconsistent")
    return len(cases)


def _verify_migration_rebuild(
    rebuild: Mapping[str, Any],
    label: str,
    expected_height: int,
) -> tuple[str, str, str]:
    for key in (
        "rebuild_passed",
        "verify_only_passed",
        "generation_pointer_published",
    ):
        _bool(rebuild.get(key), f"{label} {key}")
    required = rebuild.get("required_disk_bytes")
    available = rebuild.get("available_disk_bytes")
    if (
        type(required) is not int
        or required < 0
        or type(available) is not int
        or available < required
    ):
        _fail(f"{label} disk-capacity evidence is invalid")
    for key, pattern in (
        ("packet_root", HEX96),
        ("current_state_root", HEX96),
        ("node_state_root", HEX96),
        ("manifest_sha256", HEX64),
        ("manifest_file_sha3_384", HEX96),
    ):
        if pattern.fullmatch(str(rebuild.get(key, ""))) is None:
            _fail(f"{label} {key} is invalid")

    logical = _object(rebuild.get("logical_store_report"), f"{label} logical store")
    if (
        logical.get("schema") != "postfiat-storage-logical-integrity-v1"
        or logical.get("backend") != "redb"
        or logical.get("storage_format") != "postfiat-redb-v1"
        or logical.get("finalized_height") != expected_height
        or logical.get("block_count") != expected_height
        or logical.get("archive_count") != expected_height
        or logical.get("ordered_batch_count") != expected_height
        or type(logical.get("receipt_count")) is not int
        or logical.get("receipt_count", -1) < 0
        or type(logical.get("history_index_count")) is not int
        or logical.get("history_index_count", -1) < 0
        or HEX96.fullmatch(str(logical.get("accumulator", ""))) is None
    ):
        _fail(f"{label} logical store evidence is invalid")

    canonical = _object(
        rebuild.get("canonical_export_receipt"),
        f"{label} canonical export",
    )
    if (
        canonical.get("schema")
        != "postfiat-transactional-canonical-export-receipt-v1"
        or canonical.get("finalized_height") != expected_height
        or type(canonical.get("record_count")) is not int
        or canonical.get("record_count", 0) <= 0
        or HEX96.fullmatch(str(canonical.get("records_sha3_384", ""))) is None
    ):
        _fail(f"{label} canonical export evidence is invalid")
    return (
        str(rebuild["packet_root"]),
        str(rebuild["current_state_root"]),
        str(rebuild["node_state_root"]),
    )


def _verify_migration(
    migration: Mapping[str, Any],
    replay: Mapping[str, Any],
    source_revision: str,
    current_binary_digest: str,
    incompatible_binary_digest: str,
) -> None:
    if migration.get("status") != "PASS" or migration.get("evidence_eligible") is not True:
        _fail("migration rehearsal is not an evidence-eligible PASS")
    _bool(migration.get("source_worktree_clean"), "migration source worktree clean")
    captured_at = migration.get("captured_at")
    if not isinstance(captured_at, str) or not captured_at.endswith("Z"):
        _fail("migration capture time is missing or not UTC")
    try:
        datetime.fromisoformat(captured_at[:-1] + "+00:00")
    except ValueError:
        _fail("migration capture time is invalid")
    if migration.get("source_revision") != source_revision:
        _fail("migration source revision disagrees with the packet")
    current_binary = str(migration.get("node_binary_sha256", ""))
    if current_binary != current_binary_digest:
        _fail("migration binary identity disagrees with the packet")
    _verify_binary_build(migration, "migration", source_revision)
    source_height = migration.get("source_height")
    if source_height != 924:
        _fail("migration rehearsal did not start from exact height 924")
    if migration.get("chain_id") != CONTROLLED_CHAIN_ID:
        _fail("migration rehearsal did not use the controlled chain domain")
    if migration.get("genesis_hash") != CONTROLLED_GENESIS_HASH:
        _fail("migration rehearsal did not use the controlled genesis")
    _bool(
        migration.get("exact_existing_chain_rehearsal"),
        "migration exact existing-chain rehearsal",
    )
    if migration.get("clone_count") != 6:
        _fail("migration rehearsal did not use six clones")
    for key in (
        "offline_rebuild",
        "second_logical_scan",
        "generation_pointer_published",
        "pre_activation_restart",
        "activation",
        "pre_activation_cancellation",
        "catch_up",
        "pre_activation_rollback",
        "post_activation_forward_recovery",
        "all_six_converged",
        "mixed_version_refused",
        "backup_verified",
        "disk_capacity_verified",
        "stop_conditions_verified",
        "consensus_v2_unchanged",
        "cobalt_authority_unchanged",
        "literal_receipts_exact",
        "zero_post_activation_full_history_scans",
        "loopback_transport_used",
    ):
        _bool(migration.get(key), f"migration {key}")
    for key in ("external_network_contacted", "devnet_queried_or_mutated"):
        if migration.get(key) is not False:
            _fail(f"migration {key} must be false")
    stop_receipt = _object(
        migration.get("stop_condition_receipt"),
        "migration source stop receipt",
    )
    if (
        set(stop_receipt)
        != {
            "schema",
            "source_directory_count",
            "processes_examined",
            "unreadable_process_count",
            "matching_process_count",
        }
        or stop_receipt.get("schema")
        != "postfiat-storage-source-stop-receipt-v1"
        or stop_receipt.get("source_directory_count") != 6
        or type(stop_receipt.get("processes_examined")) is not int
        or stop_receipt.get("processes_examined", 0) <= 0
        or stop_receipt.get("unreadable_process_count") != 0
        or stop_receipt.get("matching_process_count") != 0
    ):
        _fail("migration source stop receipt is invalid")

    identities = _object(migration.get("identities"), "migration identities")
    identity_names = (
        "source_tip",
        "source_state_root",
        "packet_root",
        "activation_id",
        "cancelled_activation_id",
        "cancellation_id",
        "activation_tip",
        "activation_state_root",
        "final_tip",
        "final_state_root",
    )
    if set(identities) != set(identity_names):
        _fail("migration identity set is incomplete or unexpected")
    for key in identity_names:
        if HEX96.fullmatch(str(identities.get(key, ""))) is None:
            _fail(f"migration identity {key} is invalid")
    if (
        identities["source_tip"] != replay.get("tip_hash")
        or identities["source_state_root"] != replay.get("state_root")
    ):
        _fail("migration source identity disagrees with exact height-924 replay")

    incompatible = _object(
        migration.get("incompatible_binary"),
        "migration incompatible binary",
    )
    incompatible_revision = str(incompatible.get("source_revision", ""))
    incompatible_digest = str(incompatible.get("sha256", ""))
    if (
        HEX40.fullmatch(incompatible_revision) is None
        or incompatible_revision == source_revision
        or incompatible.get("git_revision") != incompatible_revision[:8]
        or incompatible.get("profile") != "release"
        or incompatible_digest != incompatible_binary_digest
        or incompatible_digest == current_binary
    ):
        _fail("migration incompatible binary identity is invalid or unbound")
    mixed = _object(migration.get("mixed_version_probe"), "migration mixed-version probe")
    if (
        type(mixed.get("exit_code")) is not int
        or mixed.get("exit_code", 0) == 0
        or mixed.get("reason_code") != "storage_unsupported_schema"
        or mixed.get("reason_detail")
        != "transactional migration verification binding is invalid"
        or HEX64.fullmatch(str(mixed.get("failure_output_sha256", ""))) is None
        or mixed.get("artifact_absent") is not True
        or mixed.get("binary_sha256") != incompatible_digest
        or mixed.get("source_revision") != incompatible_revision
        or mixed.get("verifier_boundary") != "v1 binary refused v2 migration generation"
    ):
        _fail("migration mixed-version refusal evidence is invalid")

    cobalt = _object(migration.get("cobalt_boundary"), "migration Cobalt boundary")
    before = _object(cobalt.get("before"), "migration Cobalt boundary before")
    after = _object(cobalt.get("after"), "migration Cobalt boundary after")
    cobalt_keys = {
        "validator_registry_semantic_sha256",
        "cobalt_governance_semantic_sha256",
    }
    if set(before) != cobalt_keys or after != before:
        _fail("migration changed or incompletely recorded the Cobalt authority boundary")
    if any(HEX64.fullmatch(str(before[key])) is None for key in cobalt_keys):
        _fail("migration Cobalt boundary digest is invalid")

    restart_groups = _object(migration.get("restart_receipts"), "migration restarts")
    expected_restart_groups = {
        "pre_activation",
        "scheduled_staggered",
        "post_activation_forward",
    }
    if set(restart_groups) != expected_restart_groups:
        _fail("migration restart group set is incomplete or unexpected")
    validator_ids = {f"validator-{index}" for index in range(6)}
    for group in sorted(expected_restart_groups):
        receipts = _list(restart_groups[group], f"migration restart group {group}")
        observed_ids: set[str] = set()
        for value in receipts:
            receipt = _object(value, f"migration restart receipt {group}")
            validator_id = str(receipt.get("validator_id", ""))
            if (
                validator_id not in validator_ids
                or validator_id in observed_ids
                or receipt.get("stopped_cleanly") is not True
                or receipt.get("reopened_and_ready") is not True
            ):
                _fail(f"migration restart receipt {group} is invalid")
            observed_ids.add(validator_id)
        if observed_ids != validator_ids:
            _fail(f"migration restart group {group} does not cover six validators")

    phase_contract = (
        ("legacy-finality", "transparent", ("accepted",)),
        (
            "cancelled-activation-scheduled",
            "governance",
            ("storage_commitment_activation_scheduled",),
        ),
        (
            "pre-activation-cancellation",
            "governance",
            ("storage_commitment_activation_cancelled",),
        ),
        ("post-cancellation-legacy-finality", "transparent", ("accepted",)),
        (
            "final-activation-scheduled",
            "governance",
            ("storage_commitment_activation_scheduled",),
        ),
        ("pre-activation-one", "transparent", ("accepted",)),
        ("pre-activation-two", "transparent", ("accepted",)),
        ("activation-finality", "transparent", ("accepted",)),
        ("post-activation-finality", "transparent", ("accepted",)),
        ("post-activation-forward-recovery", "transparent", ("accepted",)),
    )
    phases = _list(migration.get("phases"), "migration phases")
    if len(phases) != len(phase_contract):
        _fail("migration phase count is not the required ten-phase sequence")
    certificate_ids: set[str] = set()
    batch_digests: set[str] = set()
    catch_up_validator = ""
    phase_identities: dict[str, Mapping[str, Any]] = {}
    for offset, (expected_label, batch_kind, receipt_codes) in enumerate(
        phase_contract,
        start=1,
    ):
        phase = _object(phases[offset - 1], f"migration phase {expected_label}")
        expected_height = source_height + offset
        if (
            phase.get("label") != expected_label
            or phase.get("height") != expected_height
            or phase.get("batch_kind") != batch_kind
            or phase.get("receipt_codes") != list(receipt_codes)
            or phase.get("applied_validator_count") != 6
            or phase.get("certificate_validator_count") != 6
            or phase.get("certificate_quorum") != 5
        ):
            _fail(f"migration phase {expected_label} has invalid fixed fields")
        for key in (
            "receipt_accepted",
            "consensus_v2_commit",
            "transport_round_ok",
            "all_vote_requests_verified",
        ):
            _bool(phase.get(key), f"migration phase {expected_label} {key}")
        certificate_id = str(phase.get("certificate_id", ""))
        certificate_digest = str(phase.get("certificate_sha256", ""))
        batch_digest = str(phase.get("batch_sha256", ""))
        if (
            HEX96.fullmatch(certificate_id) is None
            or certificate_id in certificate_ids
            or HEX64.fullmatch(certificate_digest) is None
            or HEX64.fullmatch(batch_digest) is None
            or batch_digest in batch_digests
        ):
            _fail(f"migration phase {expected_label} artifact identity is invalid or reused")
        certificate_ids.add(certificate_id)
        batch_digests.add(batch_digest)
        identity = _object(phase.get("identity"), f"migration phase {expected_label} identity")
        if (
            identity.get("height") != expected_height
            or HEX96.fullmatch(str(identity.get("tip", ""))) is None
            or HEX96.fullmatch(str(identity.get("state_root", ""))) is None
        ):
            _fail(f"migration phase {expected_label} finalized identity is invalid")
        phase_identities[expected_label] = identity

        degraded = expected_label == "activation-finality"
        if (
            phase.get("initial_applied_validator_count") != (5 if degraded else 6)
            or phase.get("certificate_vote_count") != (5 if degraded else 6)
            or phase.get("all_certified_sends_verified") is not (False if degraded else True)
        ):
            _fail(f"migration phase {expected_label} participation policy is invalid")
        failures = _list(
            phase.get("failed_peer_targets"),
            f"migration phase {expected_label} failed peers",
        )
        if degraded:
            catch_up_validator = str(phase.get("catch_up_validator", ""))
            if (
                catch_up_validator not in validator_ids - {"validator-0"}
                or failures != [catch_up_validator]
                or phase.get("catch_up_receipt_accepted") is not True
                or type(phase.get("catch_up_receipt_count")) is not int
                or phase.get("catch_up_receipt_count", 0) <= 0
                or phase.get("catch_up_receipt_codes") != ["accepted"]
            ):
                _fail("migration activation catch-up evidence is invalid")
        elif failures:
            _fail(f"migration phase {expected_label} unexpectedly recorded peer failures")

    activation_identity = phase_identities["activation-finality"]
    final_identity = phase_identities["post-activation-forward-recovery"]
    if (
        identities["activation_tip"] != activation_identity["tip"]
        or identities["activation_state_root"] != activation_identity["state_root"]
        or identities["final_tip"] != final_identity["tip"]
        or identities["final_state_root"] != final_identity["state_root"]
    ):
        _fail("migration top-level identities disagree with phase finality")

    clones = _list(migration.get("clones"), "migration clones")
    if len(clones) != 6:
        _fail("migration clone report count is not six")
    stage_contract = {
        "initial_migration": source_height,
        "post_restart_refreeze": source_height + 1,
        "final_activation_refreeze": source_height + 4,
    }
    source_tree_digests: set[str] = set()
    stage_packet_roots = {stage: set() for stage in stage_contract}
    stage_current_roots = {stage: set() for stage in stage_contract}
    stage_node_roots = {stage: set() for stage in stage_contract}
    observed_clone_ids: set[str] = set()
    for value in clones:
        clone = _object(value, "migration clone")
        validator_id = str(clone.get("validator_id", ""))
        source_digest = str(clone.get("source_tree_sha256", ""))
        if (
            validator_id not in validator_ids
            or validator_id in observed_clone_ids
            or HEX64.fullmatch(source_digest) is None
            or clone.get("backup_tree_sha256") != source_digest
            or clone.get("backup_reverified_sha256") != source_digest
        ):
            _fail("migration clone source or immutable backup binding is invalid")
        observed_clone_ids.add(validator_id)
        source_tree_digests.add(source_digest)
        for stage, expected_height in stage_contract.items():
            rebuild = _object(clone.get(stage), f"migration clone {validator_id} {stage}")
            packet_root, current_root, node_root = _verify_migration_rebuild(
                rebuild,
                f"migration clone {validator_id} {stage}",
                expected_height,
            )
            stage_packet_roots[stage].add(packet_root)
            stage_current_roots[stage].add(current_root)
            stage_node_roots[stage].add(node_root)
        clone_final = _object(clone.get("final_identity"), "migration clone final identity")
        if clone_final != final_identity:
            _fail(f"migration clone {validator_id} did not converge on the final identity")
    if observed_clone_ids != validator_ids or len(source_tree_digests) != 6:
        _fail("migration clone identities or immutable source trees are not distinct")
    for stage in stage_contract:
        if (
            len(stage_packet_roots[stage]) != 1
            or len(stage_current_roots[stage]) != 1
            or len(stage_node_roots[stage]) != 6
        ):
            _fail(f"migration clone roots do not satisfy shared/local policy at {stage}")
    if stage_packet_roots["final_activation_refreeze"] != {identities["packet_root"]}:
        _fail("migration activation packet root disagrees with the six final refreezes")


def _verify_redaction(packet_dir: Path, redaction: Mapping[str, Any]) -> None:
    _bool(redaction.get("passed"), "redaction gate")
    allow = set(_list(redaction.get("allowed_nonlocal_ip_files", []), "redaction allowlist"))
    for path in packet_dir.rglob("*"):
        if not path.is_file() or path.name == CHECKSUM_FILE or path.stat().st_size > 16 * 1024 * 1024:
            continue
        relative = path.relative_to(packet_dir).as_posix()
        text = path.read_text(encoding="utf-8", errors="ignore")
        if SENSITIVE.search(text):
            _fail(f"sensitive material marker found in {relative}")
        if LOCAL_PATH.search(text):
            _fail(f"host-local path found in {relative}")
        if relative not in allow and NONLOCAL_IPV4.search(text):
            _fail(f"non-loopback IPv4 address found in {relative}")


def verify_packet(packet: str | Path) -> VerifiedPacket:
    packet_dir = Path(packet).expanduser().resolve()
    if not packet_dir.is_dir() or packet_dir.is_symlink():
        _fail("packet path is not a regular directory")
    checksums, checksum_root = _verify_checksums(packet_dir)
    manifest = _load_manifest(packet_dir)
    artifacts = _verify_artifacts(packet_dir, checksums, manifest)
    _verify_source(packet_dir, manifest, artifacts["source"])
    _verify_state_distinction(manifest)
    source = _object(manifest.get("source"), "source")
    source_revision = str(source["git_revision"])
    binaries = {
        str(binary.get("path")): str(binary.get("sha256"))
        for binary in _list(source.get("binaries"), "source binaries")
        if isinstance(binary, dict)
    }
    current_binary_digest = binaries["bin/postfiat-node"]
    rollback_binary_digest = binaries["bin/postfiat-node-rollback"]
    incompatible_binary_digest = binaries["bin/postfiat-node-incompatible"]
    _verify_replay(
        packet_dir,
        checksums,
        artifacts["replay"],
        source_revision,
        current_binary_digest,
    )
    ratios = _verify_performance(
        packet_dir,
        checksums,
        artifacts["performance"],
        source_revision,
        binaries,
    )
    tamper_case_count = _verify_tamper(
        packet_dir,
        checksums,
        artifacts["tamper"],
        source_revision,
        current_binary_digest,
        rollback_binary_digest,
    )
    _verify_migration(
        artifacts["migration"],
        artifacts["replay"],
        source_revision,
        current_binary_digest,
        incompatible_binary_digest,
    )
    _verify_redaction(packet_dir, artifacts["redaction"])
    report: dict[str, Any] = {
        "schema": REPORT_SCHEMA,
        "verified": True,
        "offline": True,
        "live_probe_performed": False,
        "packet_dir": str(packet_dir),
        "captured_at": manifest["captured_at"],
        "checksum_manifest_sha256": checksum_root,
        "checked_file_count": len(checksums),
        "tamper_case_count": tamper_case_count,
        "performance_ratios": ratios,
        "state_distinction": manifest["state_distinction"],
        "conclusion": "storage scaling completion gates passed for this recorded packet",
    }
    return VerifiedPacket(packet_dir=packet_dir, manifest=manifest, report=report)


_HTML = """<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>PostFiat Storage Scaling Evidence</title><style>
:root{color-scheme:dark;font-family:ui-monospace,SFMono-Regular,monospace;background:#09110f;color:#e7f5ec}
body{max-width:1100px;margin:auto;padding:2rem}header{border-bottom:1px solid #355347;margin-bottom:2rem}
.badge{display:inline-block;padding:.35rem .6rem;border:1px solid #66d19e;color:#66d19e}main{display:grid;gap:1rem}
section{background:#101d19;border:1px solid #29483c;padding:1rem}h2{color:#9ee6be;font-size:1rem;text-transform:uppercase}
dl{display:grid;grid-template-columns:minmax(10rem,1fr) 2fr;gap:.5rem}dt{color:#8ca69a}dd{margin:0;overflow-wrap:anywhere}
.warning{color:#f4c86b}footer{margin-top:2rem;color:#8ca69a}</style></head>
<body><header><p class="badge">Verified packet · read only</p><h1>Storage scaling evidence</h1>
<p class="warning">Packet verification is not a live fleet probe.</p></header><main id="states"></main>
<footer>Recorded evidence only. No mutation routes are served.</footer><script>
fetch('/api/packet').then(r=>r.json()).then(data=>{const root=document.getElementById('states');
for(const key of ['live','deployed','repository']){const item=data.state_distinction[key];const s=document.createElement('section');
const h=document.createElement('h2');h.textContent=key;const d=document.createElement('dl');
for(const [name,value] of Object.entries(item)){const dt=document.createElement('dt');dt.textContent=name;const dd=document.createElement('dd');dd.textContent=String(value);d.append(dt,dd)}
s.append(h,d);root.append(s)}}).catch(error=>{document.getElementById('states').textContent='Verification report unavailable: '+error});
</script></body></html>"""


def serve_verified_packet(verified: VerifiedPacket, bind: str, port: int) -> None:
    payload = json.dumps(verified.report, sort_keys=True).encode("utf-8")

    class Handler(BaseHTTPRequestHandler):
        def _serve(self, include_body: bool) -> None:
            if self.path in {"/", "/index.html"}:
                body, content_type = _HTML.encode("utf-8"), "text/html; charset=utf-8"
            elif self.path == "/api/packet":
                body, content_type = payload, "application/json"
            else:
                self.send_error(HTTPStatus.NOT_FOUND)
                return
            self.send_response(HTTPStatus.OK)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.send_header(
                "Content-Security-Policy",
                "default-src 'self'; style-src 'unsafe-inline'; script-src 'unsafe-inline'",
            )
            self.end_headers()
            if include_body:
                self.wfile.write(body)

        def do_GET(self) -> None:  # noqa: N802
            self._serve(True)

        def do_HEAD(self) -> None:  # noqa: N802
            self._serve(False)

        def do_POST(self) -> None:  # noqa: N802
            self.send_error(HTTPStatus.METHOD_NOT_ALLOWED)

        def log_message(self, message: str, *args: object) -> None:
            print(f"storage-scaling-browser: {message % args}", file=sys.stderr)

    if bind not in {"127.0.0.1", "::1", "localhost"}:
        _fail("browser bind must be loopback")
    server = ThreadingHTTPServer((bind, port), Handler)
    print(f"storage_scaling_browser=http://{bind}:{server.server_port}/")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    verify = commands.add_parser("verify", help="verify a packet without network access")
    verify.add_argument("packet")
    verify.add_argument("--output")
    serve = commands.add_parser("serve", help="serve a successfully verified packet read-only")
    serve.add_argument("packet")
    serve.add_argument("--bind", default="127.0.0.1")
    serve.add_argument("--port", type=int, default=0)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        verified = verify_packet(args.packet)
        if args.command == "verify":
            encoded = json.dumps(verified.report, indent=2, sort_keys=True) + "\n"
            if args.output:
                output = Path(args.output).expanduser()
                output.parent.mkdir(parents=True, exist_ok=True)
                output.write_text(encoded, encoding="utf-8")
            print(encoded, end="")
            return 0
        serve_verified_packet(verified, args.bind, args.port)
        return 0
    except (OSError, StorageScalingVerificationError) as error:
        print(f"storage-scaling-verification-failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
