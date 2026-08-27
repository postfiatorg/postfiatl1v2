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
ARTIFACT_SCHEMAS = {
    "source": "postfiat-storage-source-identity-v1",
    "replay": "postfiat-storage-scaling-replay-v1",
    "performance": "postfiat-storage-scaling-six-validator-campaign-v1",
    "tamper": "postfiat-storage-scaling-tamper-matrix-v1",
    "migration": "postfiat-storage-scaling-six-clone-migration-v1",
    "redaction": "postfiat-storage-scaling-redaction-v1",
}
REQUIRED_TAMPER_CASES = {
    "history_truncated",
    "history_padded",
    "history_reordered",
    "history_duplicated",
    "history_omitted",
    "history_modified",
    "wrong_domain",
    "stale_generation",
    "missing_table_or_snapshot",
    "index_without_history",
    "history_without_index",
    "conflicting_ordered_indexes",
    "incorrect_count_or_accumulator",
    "forged_receipt_archive_or_state",
    "disk_or_write_failure",
    "process_kill_before_commit",
    "process_kill_during_commit",
    "process_kill_after_commit",
    "migration_activation_cancellation_restart",
    "catch_up_and_rollback",
}


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


def _verify_tamper(
    packet_dir: Path,
    checksums: Mapping[str, str],
    matrix: Mapping[str, Any],
    source_revision: str,
) -> int:
    if matrix.get("source_revision") != source_revision:
        _fail("tamper matrix source revision disagrees with the packet")
    cases = _list(matrix.get("cases"), "tamper cases")
    observed: set[str] = set()
    for value in cases:
        case = _object(value, "tamper case")
        name = str(case.get("name", ""))
        if not name or name in observed:
            _fail("tamper case name is empty or duplicated")
        observed.add(name)
        _bool(case.get("passed"), f"tamper case {name}")
        _bool(case.get("no_partial_mutation"), f"tamper case {name} mutation gate")
        if not isinstance(case.get("reason_code"), str) or not case["reason_code"]:
            _fail(f"tamper case {name} has no stable reason code")
        if case.get("terminal_state") not in {"rejected_voting_blocked", "recovered_old_tip", "recovered_new_tip"}:
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
        for key in (
            "name",
            "passed",
            "reason_code",
            "no_partial_mutation",
            "terminal_state",
        ):
            if receipt.get(key) != case.get(key):
                _fail(f"tamper receipt {name} disagrees on {key}")
    missing = REQUIRED_TAMPER_CASES - observed
    if missing:
        _fail(f"tamper matrix is missing cases: {sorted(missing)}")
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
