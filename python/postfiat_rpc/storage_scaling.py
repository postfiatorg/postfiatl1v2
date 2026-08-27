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
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path, PurePosixPath
from typing import Any, Mapping, Sequence

PACKET_SCHEMA = "postfiat-storage-scaling-evidence-packet-v1"
REPORT_SCHEMA = "postfiat-storage-scaling-verification-v1"
MANIFEST_FILE = "storage-scaling-packet.json"
CHECKSUM_FILE = "SHA256SUMS.txt"
HEIGHTS = [50, 100, 500, 1000, 5000]
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
MAX_PACKET_FILES = 4096
MAX_FILE_BYTES = 512 * 1024 * 1024
HEX40 = re.compile(r"[0-9a-f]{40}")
HEX64 = re.compile(r"[0-9a-f]{64}")
HEX96 = re.compile(r"[0-9a-f]{96}")
SENSITIVE = re.compile(
    r"private[-_ ]?key|secret|password|mnemonic|spending[-_ ]?key|"
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
    "performance": "postfiat-storage-scaling-six-validator-campaign-v1",
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
    for key in ("git_revision", "spec_sha3_384", "binaries"):
        if report.get(key) != source.get(key):
            _fail(f"source artifact disagrees with manifest field {key}")
    if report.get("clean_checkout") is not True:
        _fail("source artifact was not produced from a clean checkout")
    if report.get("build_profile") != "release":
        _fail("source artifact does not identify a release build")
    if HEX40.fullmatch(str(source.get("git_revision", ""))) is None:
        _fail("source git revision is not a full lowercase object ID")
    if HEX96.fullmatch(str(source.get("spec_sha3_384", ""))) is None:
        _fail("source specification digest is invalid")
    binaries = _list(source.get("binaries"), "source binaries")
    if not binaries:
        _fail("source binaries are missing")
    for index, value in enumerate(binaries):
        binary = _object(value, f"source binary {index}")
        name = str(binary.get("path", ""))
        expected = str(binary.get("sha256", ""))
        if HEX64.fullmatch(expected) is None:
            _fail(f"binary {index} digest is invalid")
        path = packet_dir / _safe_relative(name)
        if _sha256(path) != expected:
            _fail(f"binary identity mismatch for {name}")


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
    binary_digests: set[str],
) -> None:
    if replay.get("source_revision") != source_revision:
        _fail("replay source revision disagrees with the packet")
    if replay.get("node_binary_sha256") not in binary_digests:
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
    for reference in receipts:
        receipt = _bound_json(packet_dir, checksums, reference, "replay receipt")
        if receipt.get("schema") != "postfiat-storage-replay-receipt-v1":
            _fail("replay receipt schema is unsupported")
        if receipt.get("source_revision") != source_revision:
            _fail("replay receipt source revision disagrees with the packet")
        if receipt.get("node_binary_sha256") not in binary_digests:
            _fail("replay receipt binary identity disagrees with the packet")
        _verify_binary_build(receipt, "replay receipt", source_revision)
        height = receipt.get("source_height")
        if not isinstance(height, int) or expected.get(height) != receipt.get("source_kind"):
            _fail("replay receipt does not identify a required source")
        if height in seen or receipt.get("block_count") != height:
            _fail("replay receipt height is duplicated or incomplete")
        seen.add(height)
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


def _ordinary_least_squares(points: list[tuple[float, float]]) -> dict[str, float]:
    if len(points) < 2:
        _fail("height relationship model has too few observations")
    mean_x = sum(point[0] for point in points) / len(points)
    mean_y = sum(point[1] for point in points) / len(points)
    denominator = sum((x - mean_x) ** 2 for x, _ in points)
    if denominator <= 0:
        _fail("height relationship model has no distinct heights")
    slope = sum((x - mean_x) * (y - mean_y) for x, y in points) / denominator
    intercept = mean_y - slope * mean_x
    residuals = [y - (intercept + slope * x) for x, y in points]
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
        "slope_ms_per_height": slope,
        "intercept_ms": intercept,
        "residual_rmse_ms": residual_rmse,
        "r_squared": max(0.0, min(1.0, r_squared)),
    }


def _verify_height_relationship_models(
    performance: Mapping[str, Any],
    points: Mapping[str, list[tuple[float, float]]],
    height_50_p95: Mapping[str, list[float]],
) -> None:
    envelope = _object(
        performance.get("height_relationship_model"),
        "height relationship model",
    )
    if envelope.get("schema") != "postfiat-storage-height-relationship-model-v1":
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
        observed = _object(recorded.get(stage), f"height relationship stage {stage}")
        model = _ordinary_least_squares(points[stage])
        baseline = _percentile(height_50_p95[stage], 0.50)
        predicted_delta = model["slope_ms_per_height"] * (HEIGHTS[-1] - HEIGHTS[0])
        material_threshold = max(
            baseline * MODEL_RELATIVE_MATERIALITY,
            model["residual_rmse_ms"] * MODEL_RESIDUAL_SIGMAS,
        )
        material_positive = (
            model["slope_ms_per_height"] > 0
            and predicted_delta > material_threshold
        )
        expected: dict[str, Any] = {
            **model,
            "sample_kind": "per_window_p95",
            "sample_count": len(points[stage]),
            "height_50_window_p95_median_ms": baseline,
            "predicted_delta_50_to_5000_ms": predicted_delta,
            "material_threshold_ms": material_threshold,
            "relative_materiality": MODEL_RELATIVE_MATERIALITY,
            "residual_sigmas": MODEL_RESIDUAL_SIGMAS,
            "material_positive_linear_relationship": material_positive,
        }
        for key, value in expected.items():
            recorded_value = observed.get(key)
            if isinstance(value, float):
                if (
                    not isinstance(recorded_value, (int, float))
                    or not math.isclose(
                        float(recorded_value),
                        value,
                        rel_tol=1e-12,
                        abs_tol=1e-9,
                    )
                ):
                    _fail(f"height relationship stage {stage} disagrees on {key}")
            elif recorded_value != value:
                _fail(f"height relationship stage {stage} disagrees on {key}")
        if material_positive:
            _fail(f"material stage {stage} retains a positive height relationship")


def _verify_performance(
    packet_dir: Path,
    checksums: Mapping[str, str],
    performance: Mapping[str, Any],
    source_revision: str,
    binary_digests: set[str],
) -> dict[str, float]:
    if performance.get("status") != "PASS":
        _fail("performance campaign did not pass")
    if performance.get("campaign_mode") != "release-qualification":
        _fail("performance campaign is not a release qualification")
    if performance.get("evidence_eligible") is not True:
        _fail("performance campaign is not evidence eligible")
    if performance.get("source_revision") != source_revision:
        _fail("performance source revision differs from the packet source")
    if performance.get("node_binary_sha256") not in binary_digests:
        _fail("performance binary identity differs from the packet source")
    _verify_binary_build(performance, "performance", source_revision)
    if performance.get("validator_count") != 6:
        _fail("performance topology is not six validators")
    if performance.get("windows_per_height") != 5 or performance.get("rounds_per_window") != 50:
        _fail("performance window cardinality differs from the specification")
    rows = _list(performance.get("rows"), "performance rows")
    if [row.get("height") if isinstance(row, dict) else None for row in rows] != HEIGHTS:
        _fail("performance heights differ from the required sequence")
    baseline = _object(performance.get("legacy_height_50_baseline"), "legacy height-50 baseline")
    ratios: dict[str, float] = {}
    stage_points: dict[str, list[tuple[float, float]]] = {
        stage: [] for stage in MATERIAL_STAGE_PATHS
    }
    height_50_stage_p95: dict[str, list[float]] = {
        stage: [] for stage in MATERIAL_STAGE_PATHS
    }
    for row_value in rows:
        row = _object(row_value, "performance row")
        windows = _list(row.get("windows"), f"height {row.get('height')} windows")
        if len(windows) != 5:
            _fail(f"height {row.get('height')} does not contain five windows")
        samples = {
            "consensus_round_ms": [],
            "wallet_to_finality_ms": [],
        }
        for window in windows:
            value = _object(window, "performance window")
            if value.get("rounds") != 50 or value.get("validators_converged") != 6:
                _fail("performance window lacks 50 converged six-validator rounds")
            for key in (
                "literal_receipts_exact",
                "zero_full_history_reads",
                "bounded_index_pages",
                "constant_accumulator_work",
            ):
                _bool(value.get(key), f"performance window {key}")
            storage = _object(value.get("storage"), "performance storage counters")
            if storage.get("committed_write_transactions") != 300:
                _fail("performance window did not commit once per validator and round")
            if storage.get("fsync_count") != 300:
                _fail("performance window fsync count differs from durable commits")
            for key in (
                "full_history_scans",
                "full_history_records_read",
                "full_history_bytes_read",
            ):
                if storage.get(key) != 0:
                    _fail(f"performance storage counter {key} is not zero")
            resources = _object(value.get("resources"), "performance resources")
            for key in (
                "cpu_ticks",
                "peak_rss_kib",
                "disk_growth_bytes",
                "bytes_read",
                "bytes_written",
                "page_reads",
                "page_writes",
                "fsync_count",
                "fsync_micros",
            ):
                if not isinstance(resources.get(key), (int, float)) or resources[key] < 0:
                    _fail(f"performance resource {key} is missing")

            raw = _bound_json(
                packet_dir,
                checksums,
                {
                    "path": value.get("normalized_report"),
                    "sha256": value.get("normalized_report_sha256"),
                },
                "performance window report",
            )
            if (
                raw.get("schema") != "postfiat-real-transaction-latency-benchmark-v1"
                or raw.get("status") != "passed"
            ):
                _fail("normalized performance window did not pass")
            iterations = _list(raw.get("iterations"), "performance iterations")
            if len(iterations) != 50:
                _fail("normalized performance window does not contain 50 iterations")
            stage_samples = {stage: [] for stage in MATERIAL_STAGE_PATHS}
            for iteration_value in iterations:
                iteration = _object(iteration_value, "performance iteration")
                for key in ("round_ok", "receipt_accepted", "finality_confirmed"):
                    _bool(iteration.get(key), f"performance iteration {key}")
                for metric in samples:
                    metric_value = iteration.get(metric)
                    if not isinstance(metric_value, (int, float)) or metric_value <= 0:
                        _fail(f"performance iteration {metric} is invalid")
                    samples[metric].append(float(metric_value))
                for stage, path in MATERIAL_STAGE_PATHS.items():
                    stage_samples[stage].append(
                        _nested_stage_value(iteration, stage, path)
                    )
            height = float(row["height"])
            for stage, values in stage_samples.items():
                window_p95 = _percentile(values, 0.95)
                stage_points[stage].append((height, window_p95))
                if row["height"] == HEIGHTS[0]:
                    height_50_stage_p95[stage].append(window_p95)

        for metric, values in samples.items():
            ordered = sorted(values)
            observed_p95 = ordered[max(0, math.ceil(len(ordered) * 0.95) - 1)]
            if abs(observed_p95 - _metric_p95(row, metric)) > 1e-9:
                _fail(f"performance aggregate {metric} does not match raw iterations")
    for metric in ("consensus_round_ms", "wallet_to_finality_ms"):
        selected_50 = _metric_p95(rows[0], metric)
        selected_5000 = _metric_p95(rows[-1], metric)
        legacy_50 = baseline.get(metric)
        if not isinstance(legacy_50, (int, float)) or legacy_50 <= 0:
            _fail(f"legacy baseline {metric} is invalid")
        baseline_ratio = selected_50 / float(legacy_50)
        scaling_ratio = selected_5000 / selected_50
        ratios[f"{metric}_height50_vs_legacy"] = baseline_ratio
        ratios[f"{metric}_height5000_vs_height50"] = scaling_ratio
        if baseline_ratio > 1.10 or scaling_ratio > 1.10:
            _fail(f"performance ratio exceeds 110% for {metric}")
    _verify_height_relationship_models(
        performance,
        stage_points,
        height_50_stage_p95,
    )
    _bool(performance.get("no_positive_linear_height_relationship"), "height relationship gate")
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
    binary_digests: set[str],
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
        or current.get("sha256") not in binary_digests
    ):
        _fail("current rollback binary is not bound to packet source")
    rollback_revision = str(rollback.get("source_revision", ""))
    if (
        HEX40.fullmatch(rollback_revision) is None
        or rollback_revision == source_revision
        or rollback.get("git_revision") != rollback_revision[:8]
        or rollback.get("profile") != "release"
        or rollback.get("sha256") not in binary_digests
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
    binary_digests: set[str],
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
                    binary_digests,
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


def _verify_migration(
    migration: Mapping[str, Any],
    source_revision: str,
    binary_digests: set[str],
) -> None:
    if migration.get("source_revision") != source_revision:
        _fail("migration source revision disagrees with the packet")
    if migration.get("node_binary_sha256") not in binary_digests:
        _fail("migration binary identity disagrees with the packet")
    _verify_binary_build(migration, "migration", source_revision)
    if migration.get("source_height") != 924:
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
    ):
        _bool(migration.get(key), f"migration {key}")
    identities = _object(migration.get("identities"), "migration identities")
    for key in ("source_tip", "source_state_root", "packet_root", "activation_id"):
        if HEX96.fullmatch(str(identities.get(key, ""))) is None:
            _fail(f"migration identity {key} is invalid")


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
    binary_digests = {
        str(binary.get("sha256"))
        for binary in _list(source.get("binaries"), "source binaries")
        if isinstance(binary, dict)
    }
    _verify_replay(
        packet_dir,
        checksums,
        artifacts["replay"],
        source_revision,
        binary_digests,
    )
    ratios = _verify_performance(
        packet_dir,
        checksums,
        artifacts["performance"],
        source_revision,
        binary_digests,
    )
    tamper_case_count = _verify_tamper(
        packet_dir,
        checksums,
        artifacts["tamper"],
        source_revision,
        binary_digests,
    )
    _verify_migration(
        artifacts["migration"],
        source_revision,
        binary_digests,
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
