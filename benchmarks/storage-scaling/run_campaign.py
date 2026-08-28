#!/usr/bin/env python3
"""Run the selected-store development smoke and shared lane helpers.

Release qualification is owned by run_paired_campaign.py. This module remains
an executable one-round selected-store smoke and provides the common local
six-validator mechanics used by the paired runner. No external network or
devnet endpoint is contacted.
"""

from __future__ import annotations

import argparse
import hashlib
import hmac
import importlib.util
import json
import math
import shutil
import subprocess
import threading
import time
from pathlib import Path
from typing import Any

VALIDATORS = 6
CHAIN_ID = "postfiat-storage-scaling-local-v1"
# Height 1 funds the benchmark wallet under legacy Consensus v1. Consensus v2
# activates for the first measured/advance round at height 2, while the storage
# commitment is active from genesis height 1.
CONSENSUS_ACTIVATION_HEIGHT = 2
STORAGE_ACTIVATION_HEIGHT = 1
HEIGHTS = [50, 100, 500, 1000, 5000]
WINDOWS_PER_HEIGHT = 5
ROUNDS_PER_WINDOW = 50
SELECTED_STORAGE_LANE = "selected-indexed"
HISTORICAL_STORAGE_LANES = {"legacy-jsonl", "bounded-jsonl"}
STORAGE_BACKEND_MODES = {
    "legacy-jsonl": "legacy-jsonl",
    "bounded-jsonl": "bounded-jsonl",
    "selected-indexed": "transactional",
}
STORAGE_BACKEND_IDENTITIES = {
    "legacy-jsonl": ("filesystem-full-prefix", False),
    "bounded-jsonl": ("filesystem-fixed-slot-index", False),
    "selected-indexed": ("redb", True),
}
RESOURCE_SAMPLE_SCHEMA = "postfiat-storage-resource-samples-v1"
RESOURCE_SAMPLE_TARGET_INTERVAL_MS = 100
PERSISTENT_ADVANCE_REPORT_SCHEMA = (
    "postfiat-storage-scaling-persistent-advance-report-v1"
)
BATCH_BUILD_REPORT_SCHEMA = "postfiat-storage-corpus-batch-build-report-v1"
QUALIFICATION_TIMEOUT_MS = 900_000
MAX_PROPOSAL_PAGE_READS_PER_ROUND = 64
MAX_APPLY_PAGE_READS_PER_VALIDATOR_ROUND = 64
MAX_APPLY_PAGE_WRITES_PER_VALIDATOR_ROUND = 32
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
REPO = Path(__file__).resolve().parents[2]
SHARED_RUNNER = (
    REPO
    / "benchmarks"
    / "cobalt-activate-or-retire"
    / "run_consensus_v2_cobalt_integration.py"
)


def load_shared_runner() -> Any:
    spec = importlib.util.spec_from_file_location(
        "postfiat_storage_scaling_shared_runner", SHARED_RUNNER
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load shared runner: {SHARED_RUNNER}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


SHARED = load_shared_runner()


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def directory_digest(root: Path) -> str:
    if root.is_symlink() or not root.is_dir():
        raise ValueError(f"snapshot root is not a regular directory: {root}")
    hasher = hashlib.sha256()
    files = sorted(path for path in root.rglob("*") if path.is_file())
    for path in files:
        if path.is_symlink():
            raise ValueError(f"snapshot contains a symlink: {path}")
        relative = path.relative_to(root).as_posix().encode("utf-8")
        hasher.update(len(relative).to_bytes(8, "big"))
        hasher.update(relative)
        file_digest = bytes.fromhex(digest(path))
        hasher.update(len(file_digest).to_bytes(8, "big"))
        hasher.update(file_digest)
    return hasher.hexdigest()


def validate_prepared_fleet(root: Path) -> None:
    if root.is_symlink() or not root.is_dir():
        raise ValueError(f"prepared fleet is not a regular directory: {root}")
    expected = {f"validator-{index}" for index in range(VALIDATORS)}
    observed = {path.name for path in root.iterdir()}
    if observed != expected:
        raise ValueError(
            "prepared fleet validator set differs: "
            f"expected={sorted(expected)} observed={sorted(observed)}"
        )
    for validator in sorted(root.iterdir()):
        if validator.is_symlink() or not validator.is_dir():
            raise ValueError(f"prepared validator is not a directory: {validator}")
        for path in validator.rglob("*"):
            if path.is_symlink():
                raise ValueError(f"prepared fleet contains a symlink: {path}")
            if not path.is_dir() and not path.is_file():
                raise ValueError(f"prepared fleet contains a special file: {path}")


def validate_regular_tree(root: Path, label: str) -> None:
    if root.is_symlink() or not root.is_dir():
        raise ValueError(f"{label} is not a regular directory: {root}")
    for path in root.rglob("*"):
        if path.is_symlink():
            raise ValueError(f"{label} contains a symlink: {path}")
        if not path.is_dir() and not path.is_file():
            raise ValueError(f"{label} contains a special file: {path}")


def tree_inventory(root: Path, label: str) -> dict[Path, tuple[str, int, int]]:
    validate_regular_tree(root, label)
    inventory: dict[Path, tuple[str, int, int]] = {}
    for path in root.rglob("*"):
        relative = path.relative_to(root)
        if path.is_dir():
            inventory[relative] = ("directory", 0, 0)
        else:
            metadata = path.stat()
            inventory[relative] = (
                "file",
                metadata.st_size,
                metadata.st_mtime_ns,
            )
    return inventory


def remove_tree_entry(path: Path) -> None:
    if path.is_symlink():
        raise ValueError(f"refusing to remove a symlink from clone workspace: {path}")
    if path.is_dir():
        shutil.rmtree(path)
    elif path.exists():
        path.unlink()


def incrementally_reset_tree(source: Path, destination: Path) -> None:
    source_inventory = tree_inventory(source, "clone source")
    destination_inventory = tree_inventory(destination, "clone destination")

    for relative, (kind, _, _) in sorted(
        destination_inventory.items(),
        key=lambda item: len(item[0].parts),
        reverse=True,
    ):
        source_entry = source_inventory.get(relative)
        if source_entry is None or source_entry[0] != kind:
            remove_tree_entry(destination / relative)

    for relative, (kind, size, modified_ns) in sorted(
        source_inventory.items(),
        key=lambda item: (len(item[0].parts), item[0].as_posix()),
    ):
        source_path = source / relative
        destination_path = destination / relative
        if kind == "directory":
            destination_path.mkdir(parents=True, exist_ok=True)
            continue
        if destination_path.is_file() and not destination_path.is_symlink():
            metadata = destination_path.stat()
            if metadata.st_size == size and metadata.st_mtime_ns == modified_ns:
                continue
        elif destination_path.exists() or destination_path.is_symlink():
            remove_tree_entry(destination_path)
        destination_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source_path, destination_path)


def clone_regular_tree(
    source: Path,
    destination: Path,
    expected_sha256: str,
    *,
    label: str,
) -> str:
    validate_regular_tree(source, f"{label} source")
    resolved_source = source.resolve()
    resolved_destination = destination.resolve(strict=False)
    if (
        resolved_source == resolved_destination
        or resolved_source.is_relative_to(resolved_destination)
        or resolved_destination.is_relative_to(resolved_source)
    ):
        raise ValueError(f"{label} source and destination overlap")
    if destination.is_symlink():
        raise ValueError(f"{label} destination must not be a symlink")

    used_incremental_reset = destination.exists()
    if used_incremental_reset:
        if not destination.is_dir():
            raise ValueError(f"{label} destination is not a directory")
        incrementally_reset_tree(source, destination)
    else:
        shutil.copytree(source, destination, copy_function=shutil.copy2)

    validate_regular_tree(destination, f"{label} destination")
    observed_sha256 = directory_digest(destination)
    if observed_sha256 != expected_sha256 and used_incremental_reset:
        shutil.rmtree(destination)
        shutil.copytree(source, destination, copy_function=shutil.copy2)
        validate_regular_tree(destination, f"{label} destination")
        observed_sha256 = directory_digest(destination)
    if observed_sha256 != expected_sha256:
        raise RuntimeError(f"{label} clone does not match its expected digest")
    return observed_sha256


def clone_prepared_fleet(
    source: Path,
    destination: Path,
    expected_sha256: str,
) -> str:
    validate_prepared_fleet(source)
    observed_sha256 = clone_regular_tree(
        source,
        destination,
        expected_sha256,
        label="prepared fleet",
    )
    validate_prepared_fleet(destination)
    return observed_sha256


def rebase_prepared_generation_pointers(nodes: Path) -> None:
    validate_prepared_fleet(nodes)
    mac_domain = b"postfiat.storage.state-file.v1:state file"
    for index in range(VALIDATORS):
        validator = nodes / f"validator-{index}"
        pointer_path = validator / "transactional_generation.json"
        key_path = validator / ".integrity.key"
        if (
            pointer_path.is_symlink()
            or not pointer_path.is_file()
            or key_path.is_symlink()
            or not key_path.is_file()
            or key_path.stat().st_nlink != 1
            or key_path.stat().st_mode & 0o077 != 0
        ):
            raise ValueError("prepared generation pointer binding is incomplete")
        key = key_path.read_bytes()
        if len(key) != 48:
            raise ValueError("prepared generation integrity key is invalid")
        raw = pointer_path.read_bytes().rstrip(b"\r\n")
        try:
            body, trailer = raw.rsplit(b"\n", 1)
        except ValueError as error:
            raise ValueError("prepared generation pointer is not authenticated") from error
        if not trailer.startswith(b"pftmac1:") or not hmac.compare_digest(
            trailer.removeprefix(b"pftmac1:").decode("ascii"),
            hmac.new(key, mac_domain + b"\x00" + body, hashlib.sha3_384).hexdigest(),
        ):
            raise ValueError("prepared generation pointer authentication failed")
        decoded = json.loads(body)
        if not isinstance(decoded, dict):
            raise ValueError("prepared generation pointer is not an object")
        database_directory = Path(str(decoded.get("database_directory", "")))
        database_file = str(decoded.get("database_file", ""))
        destination_directory = validator / database_directory.name
        if (
            set(decoded)
            != {
                "schema",
                "generation",
                "database_directory",
                "database_file",
                "migration_packet_root",
            }
            or decoded.get("schema")
            != "postfiat-transactional-generation-pointer-v1"
            or not database_directory.is_absolute()
            or not database_directory.name
            or not database_file
            or not (destination_directory / database_file).is_file()
            or len(str(decoded.get("migration_packet_root", ""))) != 96
        ):
            raise ValueError("prepared generation pointer is malformed")
        rebased = {
            "schema": decoded["schema"],
            "generation": decoded["generation"],
            "database_directory": str(destination_directory.resolve()),
            "database_file": database_file,
            "migration_packet_root": decoded["migration_packet_root"],
        }
        rebased_body = json.dumps(rebased, indent=2).encode("utf-8")
        rebased_mac = hmac.new(
            key,
            mac_domain + b"\x00" + rebased_body,
            hashlib.sha3_384,
        ).hexdigest()
        mode = pointer_path.stat().st_mode & 0o777
        temporary = pointer_path.with_name(f".{pointer_path.name}.rebase")
        if temporary.exists() or temporary.is_symlink():
            raise ValueError("prepared generation pointer rebase file already exists")
        temporary.write_bytes(
            rebased_body + b"\npftmac1:" + rebased_mac.encode("ascii") + b"\n"
        )
        temporary.chmod(mode)
        temporary.replace(pointer_path)


def write_json(path: Path, value: Any, mode: int = 0o644) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    path.chmod(mode)


def read_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_bytes())
    if not isinstance(value, dict):
        raise ValueError(f"expected a JSON object: {path}")
    return value


def run_git_revision() -> str:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=REPO,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    )
    revision = completed.stdout.strip()
    if len(revision) != 40:
        raise ValueError("git revision is not a full object ID")
    return revision


def git_is_clean() -> bool:
    completed = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=REPO,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    )
    return completed.stdout == ""


def percentile(values: list[float], fraction: float) -> float:
    if not values:
        raise ValueError("cannot calculate a percentile over an empty sample")
    ordered = sorted(values)
    index = max(0, math.ceil(len(ordered) * fraction) - 1)
    return ordered[index]


def latency_stats(values: list[float]) -> dict[str, float | int]:
    if not values:
        raise ValueError("cannot summarize an empty latency sample")
    return {
        "count": len(values),
        "min_ms": min(values),
        "p50_ms": percentile(values, 0.50),
        "p95_ms": percentile(values, 0.95),
        "p99_ms": percentile(values, 0.99),
        "max_ms": max(values),
        "mean_ms": sum(values) / len(values),
    }


def nested_positive_float(value: dict[str, Any], path: tuple[str, ...]) -> float:
    current: Any = value
    for component in path:
        if not isinstance(current, dict) or component not in current:
            raise RuntimeError(
                f"performance iteration omitted material stage {'.'.join(path)}"
            )
        current = current[component]
    if not isinstance(current, (int, float)) or not math.isfinite(float(current)):
        raise RuntimeError(
            f"performance material stage {'.'.join(path)} is not finite"
        )
    parsed = float(current)
    if parsed < 0:
        raise RuntimeError(
            f"performance material stage {'.'.join(path)} is negative"
        )
    return parsed


def distribution_summary(values: list[float]) -> dict[str, float | int]:
    if not values:
        raise ValueError("cannot summarize an empty distribution")
    mean = sum(values) / len(values)
    return {
        "count": len(values),
        "min": min(values),
        "p50": percentile(values, 0.50),
        "p95": percentile(values, 0.95),
        "p99": percentile(values, 0.99),
        "max": max(values),
        "mean": mean,
        "population_stddev": math.sqrt(
            sum((value - mean) ** 2 for value in values) / len(values)
        ),
    }


def ordinary_least_squares(points: list[tuple[float, float]]) -> dict[str, Any]:
    if len(points) < 2:
        raise ValueError("linear model requires at least two observations")
    mean_x = sum(point[0] for point in points) / len(points)
    mean_y = sum(point[1] for point in points) / len(points)
    denominator = sum((x - mean_x) ** 2 for x, _ in points)
    if denominator <= 0:
        raise ValueError("linear model requires distinct x values")
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


def constant_fit(values: list[float]) -> dict[str, Any]:
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


def height_relationship_models(
    rows: list[dict[str, Any]],
    root: Path,
    *,
    expected_heights: list[int] = HEIGHTS,
    rounds_per_window: int = ROUNDS_PER_WINDOW,
) -> dict[str, dict[str, Any]]:
    stage_observations: dict[str, list[dict[str, Any]]] = {
        stage: [] for stage in MATERIAL_STAGE_PATHS
    }
    height_50_p95: dict[str, list[float]] = {
        stage: [] for stage in MATERIAL_STAGE_PATHS
    }
    for row in rows:
        height = int(row["height"])
        for window in row["windows"]:
            report = read_json(root / window["normalized_report"])
            iterations = report.get("iterations")
            if not isinstance(iterations, list) or len(iterations) != rounds_per_window:
                raise RuntimeError(
                    f"height {height} performance window has the wrong round count"
                )
            for stage, path in MATERIAL_STAGE_PATHS.items():
                values = [
                    nested_positive_float(iteration, path)
                    for iteration in iterations
                    if isinstance(iteration, dict)
                ]
                if len(values) != rounds_per_window:
                    raise RuntimeError(
                        f"height {height} performance window omitted stage {stage}"
                    )
                window_p95 = percentile(values, 0.95)
                stage_observations[stage].append(
                    {
                        "height": height,
                        "window": str(window["label"]),
                        "p95_ms": window_p95,
                    }
                )
                if height == expected_heights[0]:
                    height_50_p95[stage].append(window_p95)

    models: dict[str, dict[str, Any]] = {}
    for stage, observations in stage_observations.items():
        points = [
            (float(observation["height"]), float(observation["p95_ms"]))
            for observation in observations
        ]
        values = [point[1] for point in points]
        linear = ordinary_least_squares(points)
        logarithmic = ordinary_least_squares(
            [(math.log(height), value) for height, value in points]
        )
        constant = constant_fit(values)
        baseline = percentile(height_50_p95[stage], 0.50)
        predicted_delta = linear["slope"] * (
            expected_heights[-1] - expected_heights[0]
        )
        within_height_ranges = []
        for height in expected_heights:
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
        models[stage] = {
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
    return models


def aggregate_round_reports(
    reports: list[dict[str, Any]],
    report_path: Path,
    iterations_path: Path,
) -> dict[str, Any]:
    if not reports:
        raise RuntimeError("round campaign produced no reports")
    iterations: list[dict[str, Any]] = []
    for index, report in enumerate(reports, start=1):
        rows = report.get("iterations")
        if report.get("status") != "passed" or not isinstance(rows, list) or len(rows) != 1:
            raise RuntimeError(f"round {index} did not produce one passing iteration")
        row = rows[0]
        if not isinstance(row, dict):
            raise RuntimeError(f"round {index} iteration is malformed")
        normalized = dict(row)
        normalized["iteration"] = index
        iterations.append(normalized)

    combined = dict(reports[0])
    config = dict(combined.get("config", {}))
    config["rounds"] = len(iterations)
    combined["config"] = config
    combined["iterations"] = iterations
    combined["iterations_file"] = iterations_path.as_posix()
    combined["final_state"] = reports[-1].get("final_state")
    combined["not_measured"] = sorted(
        {
            str(item)
            for report in reports
            for item in report.get("not_measured", [])
        }
    )
    combined["checks"] = {
        key: all(report.get("checks", {}).get(key) is True for report in reports)
        for key in {
            key
            for report in reports
            for key in report.get("checks", {})
        }
    }
    combined["latency"] = {
        metric: latency_stats([float(row[metric]) for row in iterations])
        for metric in (
            "wallet_to_finality_ms",
            "admitted_to_finality_ms",
            "consensus_round_ms",
            "refresh_account_tx_index_ms",
        )
    }
    write_json(report_path, combined)
    iterations_path.parent.mkdir(parents=True, exist_ok=True)
    iterations_path.write_text(
        "".join(json.dumps(row, sort_keys=True) + "\n" for row in iterations),
        encoding="utf-8",
    )
    return combined


def full_fleet_status(node_bin: Path, nodes: Path) -> list[dict[str, Any]]:
    statuses: list[dict[str, Any]] = []
    for index in range(VALIDATORS):
        completed = SHARED.run(
            [
                str(node_bin),
                "status",
                "--data-dir",
                str(nodes / f"validator-{index}"),
            ]
        )
        value = json.loads(completed.stdout)
        if not isinstance(value, dict):
            raise ValueError("node status is not a JSON object")
        statuses.append(value)
    return statuses


def fleet_identity(statuses: list[dict[str, Any]]) -> tuple[int, str, str]:
    identities = {
        (
            int(status["block_height"]),
            str(status["block_tip_hash"]),
            str(status["state_root"]),
        )
        for status in statuses
    }
    if len(statuses) != VALIDATORS or len(identities) != 1:
        raise RuntimeError("six-validator fleet did not converge")
    return next(iter(identities))


TRANSACTIONAL_COUNTER_FIELDS = [
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
]
LEGACY_COUNTER_FIELDS = [
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
]
VOTE_LOCK_WORK_GATE_SCHEMA = "postfiat-storage-vote-lock-work-gate-v1"
VOTE_LOCK_MAX_FILES_EXAMINED = 3
VOTE_LOCK_MAX_BYTES_DECODED = 4_096
VOTE_LOCK_REASON_INVALID_TELEMETRY = "VOTE_LOCK_TELEMETRY_INVALID"
VOTE_LOCK_REASON_MIGRATION_REPEATED = "VOTE_LOCK_MIGRATION_REPEATED"
VOTE_LOCK_REASON_MIGRATION_LATE = (
    "VOTE_LOCK_MIGRATION_AFTER_FIRST_FINALIZED_ROUND"
)
VOTE_LOCK_REASON_FILES_EXCEEDED = "VOTE_LOCK_FILES_EXAMINED_EXCEEDED"
VOTE_LOCK_REASON_BYTES_EXCEEDED = "VOTE_LOCK_BYTES_DECODED_EXCEEDED"


def add_counter_fields(
    totals: dict[str, int], counters: dict[str, Any], fields: list[str], stage: str
) -> None:
    for field in fields:
        value = counters.get(field)
        if value is None:
            raise RuntimeError(f"{stage} storage telemetry omitted {field}")
        parsed = int(value)
        if parsed < 0:
            raise RuntimeError(f"{stage} storage telemetry was negative: {field}")
        totals[field] += parsed


def require_bounded_pages(
    counters: dict[str, Any], *, stage: str, max_reads: int, max_writes: int
) -> None:
    page_reads = int(counters.get("page_reads", -1))
    page_writes = int(counters.get("page_writes", -1))
    if not (0 <= page_reads <= max_reads and 0 <= page_writes <= max_writes):
        raise RuntimeError(
            f"{stage} exceeded its per-round page bound: "
            f"reads={page_reads}/{max_reads}, writes={page_writes}/{max_writes}"
        )


def storage_work_from_report(
    report: dict[str, Any], storage_lane: str
) -> tuple[dict[str, Any], dict[str, dict[str, Any]]]:
    iterations = report.get("iterations")
    if not isinstance(iterations, list):
        raise RuntimeError("benchmark report omitted iterations")
    transactional_totals = {field: 0 for field in TRANSACTIONAL_COUNTER_FIELDS}
    legacy_totals = {field: 0 for field in LEGACY_COUNTER_FIELDS}
    full_history_records_read = 0
    full_history_bytes_read = 0
    remote_latest: dict[str, dict[str, Any]] = {}
    selected_transactional = storage_lane == SELECTED_STORAGE_LANE
    expected_backend, expected_transactional = STORAGE_BACKEND_IDENTITIES[storage_lane]

    def add_storage_work(work: Any, stage: str, apply_stage: bool) -> None:
        nonlocal full_history_records_read, full_history_bytes_read
        if not isinstance(work, dict):
            raise RuntimeError(f"{stage} omitted exact storage work")
        records = int(work.get("full_history_records_read", -1))
        byte_count = int(work.get("full_history_bytes_read", -1))
        if records < 0 or byte_count < 0:
            raise RuntimeError(f"{stage} reported invalid full-history work")
        full_history_records_read += records
        full_history_bytes_read += byte_count
        legacy = work.get("legacy")
        if not isinstance(legacy, dict):
            raise RuntimeError(f"{stage} omitted legacy storage counters")
        add_counter_fields(legacy_totals, legacy, LEGACY_COUNTER_FIELDS, stage)
        transactional = work.get("transactional")
        if selected_transactional:
            if not isinstance(transactional, dict):
                raise RuntimeError(f"{stage} omitted transactional storage counters")
            require_bounded_pages(
                transactional,
                stage=stage,
                max_reads=(
                    MAX_APPLY_PAGE_READS_PER_VALIDATOR_ROUND
                    if apply_stage
                    else MAX_PROPOSAL_PAGE_READS_PER_ROUND
                ),
                max_writes=(
                    MAX_APPLY_PAGE_WRITES_PER_VALIDATOR_ROUND if apply_stage else 0
                ),
            )
            add_counter_fields(
                transactional_totals,
                transactional,
                TRANSACTIONAL_COUNTER_FIELDS,
                stage,
            )
        elif transactional is not None:
            raise RuntimeError(
                f"{stage} unexpectedly reported transactional work in {storage_lane}"
            )

    for iteration in iterations:
        if not isinstance(iteration, dict):
            raise RuntimeError("benchmark iteration is not an object")
        round_timings = iteration.get("round_timings")
        if not isinstance(round_timings, dict):
            raise RuntimeError("benchmark iteration omitted round timings")

        proposal = round_timings.get("proposal_breakdown")
        if not isinstance(proposal, dict):
            raise RuntimeError("benchmark iteration omitted proposal telemetry")
        add_storage_work(proposal.get("storage_work"), "proposal", False)

        vote_targets = round_timings.get("vote_request_targets")
        if not isinstance(vote_targets, list) or len(vote_targets) != VALIDATORS - 1:
            raise RuntimeError("round did not report five validator reconstructions")
        for target in vote_targets:
            if not isinstance(target, dict) or target.get("result") != "ok":
                raise RuntimeError("validator reconstruction target failed")
            node_id = str(target.get("target", ""))
            request = target.get("vote_request_breakdown")
            remote = request.get("remote_handling") if isinstance(request, dict) else None
            block_vote = (
                remote.get("block_vote_breakdown")
                if isinstance(remote, dict)
                else None
            )
            if not isinstance(block_vote, dict):
                raise RuntimeError(
                    f"validator reconstruction {node_id} omitted remote storage telemetry"
                )
            add_storage_work(
                block_vote.get("storage_work"),
                f"validator reconstruction {node_id}",
                False,
            )

        local_apply = round_timings.get("local_apply_breakdown")
        if not isinstance(local_apply, dict):
            raise RuntimeError("benchmark iteration omitted local apply telemetry")
        add_storage_work(local_apply.get("storage_work"), "local apply", True)

        targets = round_timings.get("certified_send_targets")
        if not isinstance(targets, list) or len(targets) != VALIDATORS - 1:
            raise RuntimeError("round did not report five certified-send targets")
        for target in targets:
            if not isinstance(target, dict) or target.get("result") != "ok":
                raise RuntimeError("certified-send target failed")
            node_id = str(target.get("target", ""))
            storage = target.get("storage")
            if not isinstance(storage, dict):
                raise RuntimeError(
                    f"certified-send target {node_id} omitted in-process storage telemetry"
                )
            if (
                storage.get("backend") != expected_backend
                or storage.get("transactional_active") is not expected_transactional
            ):
                raise RuntimeError(
                    f"certified-send target {node_id} reported the wrong backend"
                )
            storage_work = target.get("storage_work")
            add_storage_work(storage_work, f"certified apply {node_id}", True)
            remote_latest[node_id] = {
                "storage": storage,
                "last_apply_storage_work": storage_work,
            }

    totals: dict[str, Any] = dict(transactional_totals)
    totals["transactional"] = transactional_totals
    totals["legacy"] = legacy_totals
    totals["full_history_records_read"] = full_history_records_read
    totals["full_history_bytes_read"] = full_history_bytes_read
    totals["fsync_count"] = (
        transactional_totals["committed_write_transactions"]
        if selected_transactional
        else legacy_totals["jsonl_append_calls"]
    )
    return totals, remote_latest


def vote_lock_work_from_report(report: dict[str, Any]) -> dict[str, Any]:
    iterations = report.get("iterations")
    if not isinstance(iterations, list):
        raise RuntimeError("benchmark report omitted iterations")

    validators: dict[str, dict[str, Any]] = {}

    def validator_summary(node_id: str) -> dict[str, Any]:
        return validators.setdefault(
            node_id,
            {
                "votes_observed": 0,
                "migration_rounds": [],
                "max_files_examined": 0,
                "max_bytes_decoded": 0,
                "reason_codes": set(),
                "violations": [],
            },
        )

    def add_violation(
        summary: dict[str, Any],
        reason_code: str,
        *,
        round_number: int,
        files_examined: int | None,
        bytes_decoded: int | None,
        migration_performed: bool | None,
    ) -> None:
        summary["reason_codes"].add(reason_code)
        summary["violations"].append(
            {
                "reason_code": reason_code,
                "finalized_round": round_number,
                "files_examined": files_examined,
                "bytes_decoded": bytes_decoded,
                "migration_performed": migration_performed,
            }
        )

    for round_number, iteration in enumerate(iterations, start=1):
        if not isinstance(iteration, dict):
            raise RuntimeError("benchmark iteration is not an object")
        round_timings = iteration.get("round_timings")
        if not isinstance(round_timings, dict):
            raise RuntimeError("benchmark iteration omitted round timings")
        vote_targets = round_timings.get("vote_request_targets")
        if not isinstance(vote_targets, list) or len(vote_targets) != VALIDATORS - 1:
            raise RuntimeError("round did not report five validator vote timings")
        observed_this_round: set[str] = set()
        for target in vote_targets:
            if not isinstance(target, dict) or target.get("result") != "ok":
                raise RuntimeError("validator vote target failed")
            node_id = str(target.get("target", ""))
            if not node_id:
                raise RuntimeError("validator vote target omitted its identity")
            summary = validator_summary(node_id)
            if node_id in observed_this_round:
                add_violation(
                    summary,
                    VOTE_LOCK_REASON_INVALID_TELEMETRY,
                    round_number=round_number,
                    files_examined=None,
                    bytes_decoded=None,
                    migration_performed=None,
                )
                continue
            observed_this_round.add(node_id)
            request = target.get("vote_request_breakdown")
            remote = request.get("remote_handling") if isinstance(request, dict) else None
            block_vote = (
                remote.get("block_vote_breakdown")
                if isinstance(remote, dict)
                else None
            )
            if not isinstance(block_vote, dict):
                raise RuntimeError(
                    f"validator vote target {node_id} omitted block-vote telemetry"
                )

            raw_files = block_vote.get("vote_lock_files_examined", 0)
            raw_bytes = block_vote.get("vote_lock_bytes_decoded", 0)
            raw_migration = block_vote.get("vote_lock_migration_performed", False)
            valid_counts = (
                isinstance(raw_files, int)
                and not isinstance(raw_files, bool)
                and raw_files >= 0
                and isinstance(raw_bytes, int)
                and not isinstance(raw_bytes, bool)
                and raw_bytes >= 0
            )
            valid_migration = isinstance(raw_migration, bool)
            files_examined = raw_files if valid_counts else None
            bytes_decoded = raw_bytes if valid_counts else None
            migration_performed = raw_migration if valid_migration else None
            summary["votes_observed"] += 1
            if not valid_counts or not valid_migration:
                add_violation(
                    summary,
                    VOTE_LOCK_REASON_INVALID_TELEMETRY,
                    round_number=round_number,
                    files_examined=files_examined,
                    bytes_decoded=bytes_decoded,
                    migration_performed=migration_performed,
                )
                continue

            summary["max_files_examined"] = max(
                int(summary["max_files_examined"]), files_examined
            )
            summary["max_bytes_decoded"] = max(
                int(summary["max_bytes_decoded"]), bytes_decoded
            )
            if migration_performed:
                summary["migration_rounds"].append(round_number)
                if len(summary["migration_rounds"]) > 1:
                    add_violation(
                        summary,
                        VOTE_LOCK_REASON_MIGRATION_REPEATED,
                        round_number=round_number,
                        files_examined=files_examined,
                        bytes_decoded=bytes_decoded,
                        migration_performed=True,
                    )
                if round_number != 1:
                    add_violation(
                        summary,
                        VOTE_LOCK_REASON_MIGRATION_LATE,
                        round_number=round_number,
                        files_examined=files_examined,
                        bytes_decoded=bytes_decoded,
                        migration_performed=True,
                    )
                continue

            if files_examined > VOTE_LOCK_MAX_FILES_EXAMINED:
                add_violation(
                    summary,
                    VOTE_LOCK_REASON_FILES_EXCEEDED,
                    round_number=round_number,
                    files_examined=files_examined,
                    bytes_decoded=bytes_decoded,
                    migration_performed=False,
                )
            if bytes_decoded > VOTE_LOCK_MAX_BYTES_DECODED:
                add_violation(
                    summary,
                    VOTE_LOCK_REASON_BYTES_EXCEEDED,
                    round_number=round_number,
                    files_examined=files_examined,
                    bytes_decoded=bytes_decoded,
                    migration_performed=False,
                )

    reason_codes: set[str] = set()
    public_validators: dict[str, dict[str, Any]] = {}
    for node_id, summary in sorted(validators.items()):
        validator_reason_codes = sorted(summary["reason_codes"])
        reason_codes.update(validator_reason_codes)
        public_validators[node_id] = {
            "passed": not validator_reason_codes,
            "votes_observed": int(summary["votes_observed"]),
            "migration_rounds": list(summary["migration_rounds"]),
            "max_files_examined": int(summary["max_files_examined"]),
            "max_bytes_decoded": int(summary["max_bytes_decoded"]),
            "reason_codes": validator_reason_codes,
            "violations": list(summary["violations"]),
        }

    return {
        "schema": VOTE_LOCK_WORK_GATE_SCHEMA,
        "passed": not reason_codes,
        "reason_codes": sorted(reason_codes),
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


def normalize_report_paths(value: Any, root: Path) -> Any:
    needle = str(root)
    if isinstance(value, str):
        return value.replace(needle, "$RUN_ROOT")
    if isinstance(value, list):
        return [normalize_report_paths(item, root) for item in value]
    if isinstance(value, dict):
        return {
            key: normalize_report_paths(item, root) for key, item in value.items()
        }
    return value


def export_snapshot(
    node_bin: Path, data_dir: Path, snapshot: Path, logs: Path, label: str
) -> None:
    if snapshot.exists():
        raise ValueError(f"refusing to overwrite snapshot: {snapshot}")
    SHARED.run(
        [
            str(node_bin),
            "snapshot-export",
            "--data-dir",
            str(data_dir),
            "--snapshot-dir",
            str(snapshot),
        ],
        stdout_path=logs / f"{label}.snapshot-export.json",
        stderr_path=logs / f"{label}.snapshot-export.stderr",
    )


def create_signed_transfer_corpus(
    *,
    node_bin: Path,
    source_snapshot: Path | None = None,
    source_data_dir: Path | None = None,
    wallet_key: Path,
    wallet_address: str,
    recipient: str,
    count: int,
    output_file: Path,
    logs: Path,
    label: str,
) -> dict[str, Any]:
    if count <= 0:
        raise ValueError("signed transfer corpus count must be positive")
    if (source_snapshot is None) == (source_data_dir is None):
        raise ValueError(
            "signed transfer corpus requires exactly one source snapshot or data directory"
        )
    if output_file.exists():
        raise ValueError(f"refusing to overwrite corpus: {output_file}")
    corpus_node: Path | None = None
    if source_snapshot is not None:
        corpus_node = output_file.parent / f".{label}.corpus-node"
        if corpus_node.exists():
            raise ValueError(f"refusing to overwrite corpus node: {corpus_node}")
        corpus_source = corpus_node
    else:
        assert source_data_dir is not None
        if source_data_dir.is_symlink() or not source_data_dir.is_dir():
            raise ValueError(
                "signed transfer corpus source data directory is not a regular directory"
            )
        corpus_source = source_data_dir
    try:
        if source_snapshot is not None:
            SHARED.run(
                [
                    str(node_bin),
                    "snapshot-import",
                    "--data-dir",
                    str(corpus_source),
                    "--snapshot-dir",
                    str(source_snapshot),
                    "--node-id",
                    "validator-0",
                ],
                stdout_path=logs / f"{label}.corpus-import.json",
                stderr_path=logs / f"{label}.corpus-import.stderr",
            )
        completed = SHARED.run(
            [
                str(node_bin),
                "tx-latency-corpus-create",
                "--data-dir",
                str(corpus_source),
                "--wallet-key-file",
                str(wallet_key),
                "--wallet-address",
                wallet_address,
                "--recipient",
                recipient,
                "--amount",
                "10",
                "--count",
                str(count),
                "--output",
                str(output_file),
            ],
            stdout_path=logs / f"{label}.corpus-create.json",
            stderr_path=logs / f"{label}.corpus-create.stderr",
        )
    finally:
        if corpus_node is not None and corpus_node.exists():
            shutil.rmtree(corpus_node)
    report = json.loads(completed.stdout)
    if not isinstance(report, dict):
        raise RuntimeError("signed transfer corpus report is not an object")
    if (
        int(report.get("transfer_count", 0)) != count
        or report.get("sha256") != digest(output_file)
    ):
        raise RuntimeError("signed transfer corpus report did not bind its output")
    return report


def validate_batch_build_report(
    report: dict[str, Any],
    *,
    batch_root: Path,
    signed_transfer_corpus: Path,
    rounds: int,
    expected_builder_revision: str,
) -> list[dict[str, Any]]:
    if (
        report.get("schema") != BATCH_BUILD_REPORT_SCHEMA
        or report.get("source_git_revision") != expected_builder_revision[:8]
        or report.get("build_profile") != "release"
        or report.get("corpus_sha256") != digest(signed_transfer_corpus)
        or int(report.get("transfer_count", 0)) != rounds
    ):
        raise RuntimeError("advance batch builder report identity differs")
    batches = report.get("batches")
    if not isinstance(batches, list) or len(batches) != rounds:
        raise RuntimeError("advance batch builder emitted the wrong batch count")
    if batch_root.is_symlink() or not batch_root.is_dir():
        raise RuntimeError("advance batch root is not a regular directory")
    observed_files = {
        path.name
        for path in batch_root.iterdir()
        if path.is_file() and path.name.endswith(".batch.json")
    }
    expected_files: set[str] = set()
    tx_ids: set[str] = set()
    batch_ids: set[str] = set()
    first_sequence: int | None = None
    normalized: list[dict[str, Any]] = []
    for index, raw in enumerate(batches):
        if not isinstance(raw, dict):
            raise RuntimeError("advance batch builder entry is malformed")
        filename = f"round-{index + 1:06}.batch.json"
        sequence = int(raw.get("sequence", -1))
        if first_sequence is None:
            first_sequence = sequence
        if (
            int(raw.get("corpus_index", -1)) != index
            or raw.get("batch_file") != filename
            or sequence < 0
            or sequence != first_sequence + index
        ):
            raise RuntimeError("advance batch builder ordering differs from the corpus")
        tx_id = str(raw.get("tx_id", ""))
        batch_id = str(raw.get("batch_id", ""))
        if (
            len(tx_id) != 96
            or len(batch_id) != 96
            or len(str(raw.get("payload_hash", ""))) != 96
            or len(str(raw.get("signed_transfer_sha256", ""))) != 64
            or len(str(raw.get("batch_sha256", ""))) != 64
            or tx_id in tx_ids
            or batch_id in batch_ids
        ):
            raise RuntimeError("advance batch builder emitted an invalid identity")
        tx_ids.add(tx_id)
        batch_ids.add(batch_id)
        path = batch_root / filename
        if path.is_symlink() or not path.is_file():
            raise RuntimeError(f"advance batch builder omitted {filename}")
        if digest(path) != raw.get("batch_sha256"):
            raise RuntimeError(f"advance batch builder digest changed for {filename}")
        expected_files.add(filename)
        normalized.append(dict(raw))
    if observed_files != expected_files:
        raise RuntimeError("advance batch directory contains an unexpected batch set")
    return normalized


def persistent_advance_iteration(
    round_report: dict[str, Any],
    batch: dict[str, Any],
    *,
    iteration: int,
    block_height: int,
) -> dict[str, Any]:
    certification = round_report.get("certification")
    timings = round_report.get("timings")
    finality_rows = round_report.get("local_hot_finality")
    sends = round_report.get("sends")
    expected_proposer = f"validator-{block_height % VALIDATORS}"
    if (
        round_report.get("schema")
        != "postfiat-transport-peer-certified-batch-round-v1"
        or round_report.get("round_ok") is not True
        or round_report.get("from") != "validator-0"
        or round_report.get("proposal_proposer") != expected_proposer
        or round_report.get("proposal_signed") is not True
        or round_report.get("proposal_signature_signer") != expected_proposer
        or round_report.get("require_local_proposer") is not False
        or round_report.get("require_signed_proposal") is not True
        or round_report.get("allow_peer_failures") is not False
        or round_report.get("local_apply_before_certified_send") is not True
        or round_report.get("certified_sends_deferred") is not False
        or round_report.get("all_vote_requests_verified") is not True
        or round_report.get("all_sends_verified") is not True
        or Path(str(round_report.get("batch_file", ""))).name
        != str(batch.get("batch_file", ""))
        or int(round_report.get("local_receipt_count", -1)) != 1
        or int(round_report.get("local_accepted_count", -1)) != 1
        or int(round_report.get("local_rejected_count", -1)) != 0
        or round_report.get("vote_request_failures") != []
        or round_report.get("send_failures") != []
        or round_report.get("unresolved_vote_targets") != []
        or round_report.get("skipped_certified_send_targets") != []
        or not isinstance(certification, dict)
        or not isinstance(timings, dict)
        or not isinstance(finality_rows, list)
        or len(finality_rows) != 1
        or not isinstance(sends, list)
        or len(sends) != VALIDATORS - 1
    ):
        raise RuntimeError(f"persistent advance round {iteration} failed its exact gate")
    finality = finality_rows[0]
    receipt = finality.get("receipt") if isinstance(finality, dict) else None
    block = finality.get("block") if isinstance(finality, dict) else None
    header = block.get("header") if isinstance(block, dict) else None
    if (
        not isinstance(receipt, dict)
        or receipt.get("accepted") is not True
        or receipt.get("tx_id") != batch["tx_id"]
        or finality.get("tx_id") != batch["tx_id"]
        or finality.get("confirmed") is not True
        or not isinstance(header, dict)
        or int(header.get("height", 0)) != block_height
        or header.get("batch_id") != batch["batch_id"]
        or certification.get("certificate_id") != header.get("certificate_id")
        or int(certification.get("block_height", 0)) != block_height
        or int(certification.get("vote_count", 0)) != VALIDATORS
    ):
        raise RuntimeError(f"persistent advance round {iteration} receipt binding differs")

    states = [round_report.get("local_state")]
    for send in sends:
        ack = send.get("ack") if isinstance(send, dict) else None
        state = ack.get("certified_state") if isinstance(ack, dict) else None
        if (
            not isinstance(send, dict)
            or send.get("verified") is not True
            or not isinstance(ack, dict)
            or ack.get("applied") is not True
            or int(ack.get("receipt_count", -1)) != 1
            or int(ack.get("accepted_count", -1)) != 1
            or int(ack.get("rejected_count", -1)) != 0
            or not isinstance(state, dict)
        ):
            raise RuntimeError(
                f"persistent advance round {iteration} certified send differs"
            )
        states.append(state)
    node_ids = {str(state.get("node_id", "")) for state in states if isinstance(state, dict)}
    identities = {
        (
            int(state.get("block_height", 0)),
            str(state.get("block_tip_hash", "")),
            str(state.get("state_root", "")),
        )
        for state in states
        if isinstance(state, dict)
    }
    if (
        node_ids != {f"validator-{index}" for index in range(VALIDATORS)}
        or len(identities) != 1
        or next(iter(identities))[0] != block_height
    ):
        raise RuntimeError(f"persistent advance round {iteration} did not converge")

    local_apply = timings.get("local_apply_breakdown")
    if not isinstance(local_apply, dict):
        raise RuntimeError(f"persistent advance round {iteration} omitted local apply")
    write_commit_ms = float(local_apply.get("write_commit_ms", 0.0))
    write_breakdown = local_apply.get("write_commit_breakdown")
    refresh_ms = (
        float(write_breakdown.get("refresh_account_tx_index_ms", 0.0))
        if isinstance(write_breakdown, dict)
        else 0.0
    )
    finality_ms = float(timings.get("client_visible_finality_ms", 0.0))
    return {
        "iteration": iteration,
        "source_node": expected_proposer,
        "tx_id": batch["tx_id"],
        "input_source": "signed-transfer-corpus-prebuilt-batch",
        "signed_transfer_corpus_index": int(batch["corpus_index"]),
        "signed_transfer_sha256": batch["signed_transfer_sha256"],
        "block_height": block_height,
        "block_hash": header["block_hash"],
        "certificate_id": header["certificate_id"],
        "vote_policy": "full",
        "validators": VALIDATORS,
        "quorum": 5,
        "vote_count": int(certification["vote_count"]),
        "quote_ms": 0.0,
        "wallet_sign_ms": 0.0,
        "mempool_submit_ms": 0.0,
        "mempool_batch_ms": 0.0,
        "wallet_to_finality_ms": finality_ms,
        "admitted_to_finality_ms": finality_ms,
        "consensus_round_ms": finality_ms,
        "round_function_return_ms": float(timings.get("total_ms", finality_ms)),
        "certified_sends_ms": float(timings.get("certified_sends_ms", 0.0)),
        "local_apply_ms": float(timings.get("local_apply_ms", 0.0)),
        "write_commit_ms": write_commit_ms,
        "refresh_account_tx_index_ms": refresh_ms,
        "receipt_accepted": True,
        "finality_confirmed": True,
        "round_ok": True,
        "all_vote_requests_verified": True,
        "all_sends_verified": True,
        "round_timings": timings,
    }


def run_rounds(
    *,
    node_bin: Path,
    root: Path,
    seed: Path,
    topology: Path,
    source_snapshot: Path | None,
    wallet_key: Path,
    wallet_address: str,
    recipient: str,
    signed_transfer_corpus: Path,
    label: str,
    rounds: int,
    storage_lane: str = SELECTED_STORAGE_LANE,
    prepared_fleet: Path | None = None,
    prepared_fleet_sha256: str | None = None,
    nodes_root: Path | None = None,
    rebase_prepared_pointers: bool = False,
) -> tuple[dict[str, Any], Path | None]:
    if storage_lane not in HISTORICAL_STORAGE_LANES | {SELECTED_STORAGE_LANE}:
        raise ValueError(f"unsupported storage lane: {storage_lane}")
    selected_transactional = storage_lane == SELECTED_STORAGE_LANE
    if signed_transfer_corpus.is_symlink() or not signed_transfer_corpus.is_file():
        raise ValueError("signed transfer corpus must be a regular non-symlink file")
    logs = root / "logs"
    nodes = root / "nodes" if nodes_root is None else nodes_root
    if (prepared_fleet is None) != (prepared_fleet_sha256 is None):
        raise ValueError(
            "prepared fleet path and digest must either both be set or both be absent"
        )
    if prepared_fleet is not None and not selected_transactional:
        raise ValueError("prepared fleets are valid only for selected transactional runs")
    if rebase_prepared_pointers and prepared_fleet is None:
        raise ValueError("prepared pointer rebasing requires a prepared fleet")
    if prepared_fleet is None and source_snapshot is None:
        raise ValueError("snapshot preparation requires a source snapshot")
    if prepared_fleet is None:
        assert source_snapshot is not None
        SHARED.prepare_nodes(node_bin, nodes, source_snapshot, seed, logs, label)
        node_preparation_mode = "authenticated-portable-snapshot-import"
        observed_prepared_fleet_sha256 = None
    else:
        observed_prepared_fleet_sha256 = clone_prepared_fleet(
            prepared_fleet,
            nodes,
            str(prepared_fleet_sha256),
        )
        if rebase_prepared_pointers:
            rebase_prepared_generation_pointers(nodes)
        node_preparation_mode = "byte-verified-prepared-fleet-clone"
    if prepared_fleet is None:
        backend_mode = STORAGE_BACKEND_MODES[storage_lane]
        for index in range(VALIDATORS):
            command = [
                str(node_bin),
                "storage-backend-configure",
                "--data-dir",
                str(nodes / f"validator-{index}"),
                "--mode",
                backend_mode,
                "--offline-confirmed",
            ]
            if not selected_transactional:
                command.append("--unsafe-comparison-mode")
            SHARED.run(
                command,
                stdout_path=logs / f"{label}.validator-{index}.backend.json",
                stderr_path=logs / f"{label}.validator-{index}.backend.stderr",
            )
    before = full_fleet_status(node_bin, nodes)
    expected_backend, expected_transactional = STORAGE_BACKEND_IDENTITIES[storage_lane]
    for status in before:
        storage = status.get("storage")
        if not isinstance(storage, dict):
            raise RuntimeError(f"{storage_lane} status omitted storage identity")
        if (
            storage.get("backend") != expected_backend
            or storage.get("transactional_active") is not expected_transactional
        ):
            raise RuntimeError(
                f"{storage_lane} configured the wrong backend: {storage}"
            )
    initial_height, _, _ = fleet_identity(before)

    services: dict[int, tuple[Any, tuple[Any, Any]]] = {}
    foreground_pids: set[int] = set()
    services_lock = threading.Lock()
    samples: list[dict[str, Any]] = []
    benchmark_processes: list[dict[str, int]] = []
    stop_event = threading.Event()
    sample_thread: threading.Thread | None = None
    round_reports: list[dict[str, Any]] = []

    def start_service(index: int, restart: int = 0) -> None:
        process, process_handles = SHARED.start_validator(
            node_bin,
            nodes,
            topology,
            root,
            logs,
            label,
            index,
            restart=restart,
            timeout_ms=QUALIFICATION_TIMEOUT_MS,
        )
        with services_lock:
            services[index] = (process, process_handles)

    def stop_service(index: int) -> None:
        with services_lock:
            service = services.pop(index)
        process, process_handles = service
        SHARED.stop_validators([process], [process_handles])

    def current_pids() -> list[int]:
        with services_lock:
            service_pids = [
                process.pid
                for process, _ in services.values()
                if process.poll() is None
            ]
            return sorted(set(service_pids) | foreground_pids)

    def run_observed_benchmark(
        command: list[str], stdout_path: Path, stderr_path: Path
    ) -> None:
        stdout_path.parent.mkdir(parents=True, exist_ok=True)
        stderr_path.parent.mkdir(parents=True, exist_ok=True)
        with stdout_path.open("wb") as stdout_handle, stderr_path.open(
            "wb"
        ) as stderr_handle:
            started_monotonic_ns = time.monotonic_ns()
            process = subprocess.Popen(
                command,
                stdout=stdout_handle,
                stderr=stderr_handle,
            )
            with services_lock:
                foreground_pids.add(process.pid)
            try:
                return_code = process.wait()
            except BaseException:
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()
                raise
            finally:
                ended_monotonic_ns = time.monotonic_ns()
                with services_lock:
                    foreground_pids.discard(process.pid)
                benchmark_processes.append(
                    {
                        "pid": process.pid,
                        "started_monotonic_ns": started_monotonic_ns,
                        "ended_monotonic_ns": ended_monotonic_ns,
                    }
                )
        if return_code != 0:
            raise RuntimeError(
                f"{command[0]} {command[1]} failed with {return_code}; "
                f"stdout={stdout_path} stderr={stderr_path}"
            )

    try:
        for index in range(VALIDATORS):
            start_service(index)
        sample_thread = SHARED.start_resource_sampler(
            stop_event, current_pids, nodes, samples
        )
        for round_index in range(1, rounds + 1):
            next_height = initial_height + round_index
            # `leader_for_view` canonicalizes validator IDs and selects
            # ((height % count) + (view % count)) % count. This campaign uses
            # the frozen validator-0..validator-5 set at view zero, so selecting
            # locally avoids opening a redb database already owned by its
            # resident validator service.
            proposer_index = next_height % VALIDATORS
            proposer = f"validator-{proposer_index}"
            if proposer_index not in services:
                raise RuntimeError(
                    f"height {next_height} proposer service is unavailable: {proposer}"
                )

            # redb enforces one process owner for a database. The selected lane
            # pauses only the deterministic proposer service; historical JSONL
            # lanes retain the original six-resident-validator topology.
            if selected_transactional:
                stop_service(proposer_index)
            round_lane = f"{label}/round-{round_index:06}"
            log_label = f"{label}.round-{round_index:06}"
            command = SHARED.benchmark_command(
                node_bin,
                root,
                nodes,
                topology,
                wallet_key,
                wallet_address,
                recipient,
                1,
                round_lane,
                timeout_ms=QUALIFICATION_TIMEOUT_MS,
            )
            command.extend(
                [
                    "--signed-transfer-corpus",
                    str(signed_transfer_corpus),
                    "--signed-transfer-corpus-offset",
                    str(round_index - 1),
                ]
            )
            if selected_transactional:
                command.extend(
                    [
                        "--resident-transactional-store",
                        "--expected-start-height",
                        str(next_height - 1),
                    ]
                )
            run_observed_benchmark(
                command,
                logs / f"{log_label}.stdout.json",
                logs / f"{log_label}.stderr",
            )
            round_reports.append(read_json(root / round_lane / "report.json"))
            if selected_transactional:
                start_service(proposer_index, restart=round_index)
    finally:
        if sample_thread is not None:
            stop_event.set()
            sample_thread.join()
        with services_lock:
            remaining = list(services.values())
            services.clear()
        SHARED.stop_validators(
            [process for process, _ in remaining],
            [process_handles for _, process_handles in remaining],
        )

    after = full_fleet_status(node_bin, nodes)
    final_height, final_tip, final_root = fleet_identity(after)
    if final_height != initial_height + rounds:
        raise RuntimeError(
            f"{label} finalized height {final_height}, expected {initial_height + rounds}"
        )
    report_path = root / label / "report.json"
    report = aggregate_round_reports(
        round_reports,
        report_path,
        root / label / "iterations.jsonl",
    )
    iterations = report.get("iterations")
    if report.get("status") != "passed" or not isinstance(iterations, list):
        raise RuntimeError(f"{label} benchmark report did not pass")
    if len(iterations) != rounds:
        raise RuntimeError(f"{label} report has the wrong iteration count")
    if not all(
        row.get("round_ok") is True
        and row.get("receipt_accepted") is True
        and row.get("finality_confirmed") is True
        for row in iterations
        if isinstance(row, dict)
    ):
        raise RuntimeError(f"{label} contains a failed or non-final iteration")

    corpus_sha256 = digest(signed_transfer_corpus)
    for round_index, round_report in enumerate(round_reports):
        config = round_report.get("config")
        round_iterations = round_report.get("iterations")
        if not isinstance(config, dict) or not isinstance(round_iterations, list):
            raise RuntimeError(f"{label} round {round_index + 1} omitted input binding")
        if (
            config.get("input_source") != "signed-transfer-corpus"
            or config.get("signed_transfer_corpus_sha256") != corpus_sha256
            or int(config.get("signed_transfer_corpus_offset", -1)) != round_index
            or len(round_iterations) != 1
            or not isinstance(round_iterations[0], dict)
            or round_iterations[0].get("input_source") != "signed-transfer-corpus"
            or int(round_iterations[0].get("signed_transfer_corpus_index", -1))
            != round_index
            or not str(round_iterations[0].get("signed_transfer_sha256", ""))
        ):
            raise RuntimeError(
                f"{label} round {round_index + 1} did not bind the exact corpus input"
            )

    counters, remote_storage = storage_work_from_report(report, storage_lane)
    vote_lock_work = vote_lock_work_from_report(report)
    vote_lock_work_path = root / "vote-lock-work" / f"{label}.json"
    write_json(vote_lock_work_path, vote_lock_work)
    if vote_lock_work["passed"] is not True:
        reason_codes = vote_lock_work["reason_codes"]
        reason_code = (
            str(reason_codes[0])
            if isinstance(reason_codes, list) and reason_codes
            else VOTE_LOCK_REASON_INVALID_TELEMETRY
        )
        raise RuntimeError(f"{reason_code}: {label} vote-lock work gate failed")
    legacy_work = counters["legacy"]
    if selected_transactional:
        backend_work_gate_pass = (
            counters["full_history_scans"] == 0
            and counters["full_history_records_read"] == 0
            and counters["full_history_bytes_read"] == 0
            and counters["committed_write_transactions"] == rounds * VALIDATORS
        )
        if not backend_work_gate_pass:
            raise RuntimeError(f"{label} selected-store work gate failed: {counters}")
    elif storage_lane == "bounded-jsonl":
        backend_work_gate_pass = (
            legacy_work["legacy_prefix_records_verified"] == 0
            and legacy_work["legacy_prefix_bytes_read"] == 0
            and legacy_work["ordered_history_records_read"] == 0
            and legacy_work["ordered_history_bytes_read"] == 0
            and legacy_work["ordered_index_bitmap_bytes_read"] > 0
            and legacy_work["ordered_index_bitmap_bytes_written"] > 0
            and legacy_work["ordered_index_slots_written"] > 0
        )
        if not backend_work_gate_pass:
            raise RuntimeError(f"{label} bounded-JSONL work gate failed: {counters}")
    else:
        prefix_rescan_observed = (
            legacy_work["legacy_prefix_records_verified"] > 0
            and legacy_work["legacy_prefix_bytes_read"] > 0
        )
        first_append_only = (
            rounds == 1
            and legacy_work["jsonl_append_calls"] > 0
            and legacy_work["legacy_prefix_records_verified"] == 0
            and legacy_work["legacy_prefix_bytes_read"] == 0
        )
        backend_work_gate_pass = (
            (prefix_rescan_observed or first_append_only)
            and legacy_work["ordered_history_records_read"] > 0
            and legacy_work["ordered_history_bytes_read"] > 0
            and legacy_work["ordered_index_slots_read"] == 0
            and legacy_work["ordered_index_slots_written"] == 0
        )
        if not backend_work_gate_pass:
            raise RuntimeError(f"{label} legacy work gate failed: {counters}")

    resource = SHARED.resource_summary(samples)
    if len(benchmark_processes) != rounds:
        raise RuntimeError(
            f"{label} resource sampler tracked {len(benchmark_processes)} "
            f"foreground processes, expected {rounds}"
        )
    if not samples:
        raise RuntimeError(f"{label} resource sampler emitted no samples")
    sample_origin_ns = int(samples[0]["monotonic_ns"])
    normalized_samples: list[dict[str, Any]] = []
    for sample in samples:
        normalized_sample = dict(sample)
        normalized_sample["monotonic_offset_ns"] = (
            int(normalized_sample.pop("monotonic_ns")) - sample_origin_ns
        )
        normalized_samples.append(normalized_sample)
    normalized_benchmarks = [
        {
            "pid": process["pid"],
            "started_offset_ns": process["started_monotonic_ns"] - sample_origin_ns,
            "ended_offset_ns": process["ended_monotonic_ns"] - sample_origin_ns,
        }
        for process in benchmark_processes
    ]
    if len({process["pid"] for process in normalized_benchmarks}) != rounds:
        raise RuntimeError(f"{label} reused a foreground process identifier")
    foreground_sample_counts = {
        str(process["pid"]): sum(
            1
            for sample in normalized_samples
            if process["started_offset_ns"]
            <= sample["monotonic_offset_ns"]
            <= process["ended_offset_ns"]
            and str(process["pid"]) in sample["processes"]
        )
        for process in normalized_benchmarks
    }
    foreground_min_sample_count = min(foreground_sample_counts.values())
    if foreground_min_sample_count < 2:
        raise RuntimeError(
            f"{label} resource sampler observed a foreground benchmark fewer "
            "than two times"
        )
    resource_samples_path = root / "resource-samples" / f"{label}.json"
    write_json(
        resource_samples_path,
        {
            "schema": RESOURCE_SAMPLE_SCHEMA,
            "sample_target_interval_ms": RESOURCE_SAMPLE_TARGET_INTERVAL_MS,
            "samples": normalized_samples,
            "foreground_processes": normalized_benchmarks,
            "foreground_sample_counts": foreground_sample_counts,
        },
    )
    normalized = normalize_report_paths(report, root)
    normalized_config = normalized.get("config")
    if not isinstance(normalized_config, dict):
        raise RuntimeError(f"{label} normalized report omitted configuration")
    normalized_config["signed_transfer_corpus"] = "$SIGNED_TRANSFER_CORPUS"
    normalized_config["base_dir"] = "$WORKING_FLEET"
    normalized_path = root / "normalized" / f"{label}.report.json"
    write_json(normalized_path, normalized)

    result_snapshot: Path | None = None
    if prepared_fleet is None:
        result_snapshot = root / "snapshots" / f"{label}.snapshot"
        export_snapshot(
            node_bin,
            nodes / "validator-0",
            result_snapshot,
            logs,
            label,
        )
    result_prepared_fleet_sha256 = (
        directory_digest(nodes) if selected_transactional else None
    )
    result = {
        "label": label,
        "storage_lane": storage_lane,
        "source_snapshot_sha256": (
            directory_digest(source_snapshot) if source_snapshot is not None else None
        ),
        "node_preparation_mode": node_preparation_mode,
        "prepared_fleet_sha256": observed_prepared_fleet_sha256,
        "result_prepared_fleet_sha256": result_prepared_fleet_sha256,
        "signed_transfer_corpus": signed_transfer_corpus.as_posix(),
        "signed_transfer_corpus_sha256": corpus_sha256,
        "starting_height": initial_height,
        "rounds": rounds,
        "validators_converged": VALIDATORS,
        "literal_receipts_exact": True,
        "backend_work_gate_pass": backend_work_gate_pass,
        "vote_lock_work_gate_pass": vote_lock_work["passed"],
        "vote_lock_work_gate_reason_codes": vote_lock_work["reason_codes"],
        "vote_lock_work": vote_lock_work,
        "vote_lock_work_receipt": vote_lock_work_path.relative_to(root).as_posix(),
        "vote_lock_work_receipt_sha256": digest(vote_lock_work_path),
        "zero_full_history_reads": (
            counters["full_history_records_read"] == 0
            and counters["full_history_bytes_read"] == 0
        ),
        "bounded_index_pages": (
            counters["page_reads"]
            <= rounds
            * (
                MAX_PROPOSAL_PAGE_READS_PER_ROUND
                + (VALIDATORS - 1) * MAX_PROPOSAL_PAGE_READS_PER_ROUND
                + VALIDATORS * MAX_APPLY_PAGE_READS_PER_VALIDATOR_ROUND
            )
            if selected_transactional
            else legacy_work["ordered_index_bitmap_bytes_read"] > 0
            and legacy_work["ordered_index_bitmap_bytes_written"] > 0
            if storage_lane == "bounded-jsonl"
            else False
        ),
        "constant_accumulator_work": (
            backend_work_gate_pass if storage_lane != "legacy-jsonl" else False
        ),
        "final_height": final_height,
        "final_tip": final_tip,
        "final_state_root": final_root,
        "latency": report["latency"],
        "storage": counters,
        "resources": {
            "cpu_ticks": resource["validator_cpu_ticks"],
            "peak_rss_kib": resource["validator_peak_rss_kib"],
            "disk_growth_bytes": max(0, resource["node_disk_delta_bytes"]),
            "bytes_read": resource["validator_read_bytes"],
            "bytes_written": resource["validator_write_bytes"],
            "page_reads": counters.get("page_reads"),
            "page_writes": counters.get("page_writes"),
            "fsync_count": counters.get("fsync_count"),
            "fsync_micros": counters.get("durable_commit_micros"),
            "sample_count": resource["sample_count"],
            "duration_ms": resource["duration_ms"],
            "observed_pid_count": len(resource["observed_pids"]),
            "foreground_process_count": len(benchmark_processes),
            "foreground_min_sample_count": foreground_min_sample_count,
            "host_cpu_ticks": resource["host_cpu_ticks"],
            "host_total_memory_kib": resource["host_total_memory_kib"],
            "host_min_available_memory_kib": resource[
                "host_min_available_memory_kib"
            ],
            "network_received_bytes": resource["network_received_bytes"],
            "network_transmitted_bytes": resource["network_transmitted_bytes"],
        },
        "storage_telemetry_source": (
            "mode-generic proposer, remote validator reconstruction, local apply, "
            "and in-process certified-apply deltas"
        ),
        "remote_validator_storage_final": remote_storage,
        "initial_fleet": [
            {
                "node_id": status["node_id"],
                "height": status["block_height"],
                "tip": status["block_tip_hash"],
                "state_root": status["state_root"],
            }
            for status in before
        ],
        "final_fleet": [
            {
                "node_id": status["node_id"],
                "height": status["block_height"],
                "tip": status["block_tip_hash"],
                "state_root": status["state_root"],
            }
            for status in after
        ],
        "normalized_report": normalized_path.relative_to(root).as_posix(),
        "normalized_report_sha256": digest(normalized_path),
        "resource_samples": resource_samples_path.relative_to(root).as_posix(),
        "resource_samples_sha256": digest(resource_samples_path),
        "result_snapshot_sha256": (
            directory_digest(result_snapshot) if result_snapshot is not None else None
        ),
    }
    write_json(root / "receipts" / f"{label}.json", result)
    print(
        f"storage-scaling-window={label} start={initial_height} "
        f"end={final_height} rounds={rounds}",
        flush=True,
    )
    return result, result_snapshot


def run_persistent_advance(
    *,
    node_bin: Path,
    batch_builder_bin: Path,
    expected_builder_revision: str,
    root: Path,
    seed: Path,
    topology: Path,
    source_snapshot: Path | None,
    signed_transfer_corpus: Path,
    label: str,
    rounds: int,
    prepared_fleet: Path | None = None,
    prepared_fleet_sha256: str | None = None,
    nodes_root: Path | None = None,
) -> tuple[dict[str, Any], Path | None]:
    if rounds <= 0:
        raise ValueError("persistent advance rounds must be positive")
    if signed_transfer_corpus.is_symlink() or not signed_transfer_corpus.is_file():
        raise ValueError("signed transfer corpus must be a regular non-symlink file")
    if batch_builder_bin.is_symlink() or not batch_builder_bin.is_file():
        raise ValueError("batch builder must be a regular non-symlink file")
    if (prepared_fleet is None) != (prepared_fleet_sha256 is None):
        raise ValueError(
            "prepared fleet path and digest must either both be set or both be absent"
        )
    if prepared_fleet is None and source_snapshot is None:
        raise ValueError("initial persistent advance requires a source snapshot")

    logs = root / "logs"
    nodes = root / "nodes" if nodes_root is None else nodes_root
    if prepared_fleet is None:
        assert source_snapshot is not None
        SHARED.prepare_nodes(node_bin, nodes, source_snapshot, seed, logs, label)
        node_preparation_mode = "authenticated-portable-snapshot-import"
        observed_prepared_fleet_sha256 = None
        for index in range(VALIDATORS):
            SHARED.run(
                [
                    str(node_bin),
                    "storage-backend-configure",
                    "--data-dir",
                    str(nodes / f"validator-{index}"),
                    "--mode",
                    "transactional",
                    "--offline-confirmed",
                ],
                stdout_path=logs / f"{label}.validator-{index}.backend.json",
                stderr_path=logs / f"{label}.validator-{index}.backend.stderr",
            )
    else:
        observed_prepared_fleet_sha256 = clone_prepared_fleet(
            prepared_fleet,
            nodes,
            str(prepared_fleet_sha256),
        )
        node_preparation_mode = "byte-verified-prepared-fleet-clone"

    before = full_fleet_status(node_bin, nodes)
    initial_height, _, _ = fleet_identity(before)
    for status in before:
        storage = status.get("storage")
        if (
            not isinstance(storage, dict)
            or storage.get("backend") != "redb"
            or storage.get("transactional_active") is not True
        ):
            raise RuntimeError("persistent advance prepared the wrong storage backend")

    label_root = root / label
    if label_root.exists():
        raise ValueError(f"refusing to overwrite persistent advance: {label_root}")
    label_root.mkdir(parents=True)
    pending_batches = label_root / "pending-batches"
    processed_batches = label_root / "processed-batches"
    batch_report_path = label_root / "batch-build-report.json"
    batch_stderr_path = logs / f"{label}.batch-build.stderr"
    completed = SHARED.run(
        [
            str(batch_builder_bin),
            "--data-dir",
            str(nodes / "validator-0"),
            "--signed-transfer-corpus",
            str(signed_transfer_corpus),
            "--output-dir",
            str(pending_batches),
        ],
        stdout_path=batch_report_path,
        stderr_path=batch_stderr_path,
    )
    batch_report = json.loads(completed.stdout)
    if not isinstance(batch_report, dict):
        raise RuntimeError("advance batch builder report is not an object")
    batches = validate_batch_build_report(
        batch_report,
        batch_root=pending_batches,
        signed_transfer_corpus=signed_transfer_corpus,
        rounds=rounds,
        expected_builder_revision=expected_builder_revision,
    )
    reference_status = before[0]
    if (
        Path(str(batch_report.get("data_dir", ""))).resolve()
        != (nodes / "validator-0").resolve()
        or Path(str(batch_report.get("output_dir", ""))).resolve()
        != pending_batches.resolve()
        or batch_report.get("chain_id") != reference_status.get("chain_id")
        or batch_report.get("genesis_hash") != reference_status.get("genesis_hash")
        or int(batch_report.get("protocol_version", 0))
        != int(reference_status.get("protocol_version", 0))
    ):
        raise RuntimeError("advance batch builder domain differs from the fleet")

    services: list[Any] = []
    service_handles: list[tuple[Any, Any]] = []
    foreground_pids: set[int] = set()
    samples: list[dict[str, Any]] = []
    stop_event = threading.Event()
    sample_thread: threading.Thread | None = None
    foreground: dict[str, int] | None = None
    loop_report_path = label_root / "loop-report.json"
    loop_stderr_path = logs / f"{label}.loop.stderr"

    def current_pids() -> list[int]:
        service_pids = [process.pid for process in services if process.poll() is None]
        return sorted(set(service_pids) | foreground_pids)

    command = [
        str(node_bin),
        "transport-peer-certified-batch-loop",
        "--data-dir",
        str(nodes / "validator-0"),
        "--topology",
        str(topology),
        "--batch-kind",
        "transparent",
        "--batch-dir",
        str(pending_batches),
        "--key-file",
        str(nodes / "validator-0" / "validator_keys.json"),
        "--proposal-key-file",
        str(nodes / "validator-0" / "validator_keys.json"),
        "--local-apply-before-certified-send",
        "--artifact-root",
        str(label_root / "artifacts"),
        "--processed-dir",
        str(processed_batches),
        "--max-rounds",
        str(rounds),
        "--start-height",
        str(initial_height + 1),
        "--poll-ms",
        "1",
        "--timeout-ms",
        str(QUALIFICATION_TIMEOUT_MS),
        "--send-retries",
        "16",
        "--retry-backoff-ms",
        "100",
    ]
    return_code: int | None = None
    try:
        for index in range(1, VALIDATORS):
            process, handles = SHARED.start_validator(
                node_bin,
                nodes,
                topology,
                root,
                logs,
                label,
                index,
                timeout_ms=QUALIFICATION_TIMEOUT_MS,
            )
            services.append(process)
            service_handles.append(handles)
        sample_thread = SHARED.start_resource_sampler(
            stop_event, current_pids, nodes, samples
        )
        with loop_report_path.open("wb") as stdout_handle, loop_stderr_path.open(
            "wb"
        ) as stderr_handle:
            started_monotonic_ns = time.monotonic_ns()
            process = subprocess.Popen(
                command,
                stdout=stdout_handle,
                stderr=stderr_handle,
            )
            foreground_pids.add(process.pid)
            try:
                return_code = process.wait()
            except BaseException:
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()
                raise
            finally:
                ended_monotonic_ns = time.monotonic_ns()
                foreground_pids.discard(process.pid)
                foreground = {
                    "pid": process.pid,
                    "started_monotonic_ns": started_monotonic_ns,
                    "ended_monotonic_ns": ended_monotonic_ns,
                }
    finally:
        if sample_thread is not None:
            stop_event.set()
            sample_thread.join()
        SHARED.stop_validators(services, service_handles)
    if return_code != 0:
        raise RuntimeError(
            f"persistent advance loop failed with {return_code}; "
            f"stdout={loop_report_path} stderr={loop_stderr_path}"
        )
    if foreground is None:
        raise RuntimeError("persistent advance did not start its foreground process")

    loop_report = read_json(loop_report_path)
    loop_rounds = loop_report.get("rounds")
    if (
        loop_report.get("schema")
        != "postfiat-transport-peer-certified-batch-loop-v1"
        or loop_report.get("node_id") != "validator-0"
        or loop_report.get("loop_ok") is not True
        or int(loop_report.get("processed_round_count", 0)) != rounds
        or int(loop_report.get("max_rounds", 0)) != rounds
        or int(loop_report.get("start_height", 0)) != initial_height + 1
        or loop_report.get("shutdown_reason") != "max_rounds"
        or loop_report.get("require_local_proposer") is not False
        or loop_report.get("require_signed_proposal") is not True
        or loop_report.get("allow_peer_failures") is not False
        or loop_report.get("local_apply_before_certified_send") is not True
        or loop_report.get("defer_certified_sends") is not False
        or not isinstance(loop_rounds, list)
        or len(loop_rounds) != rounds
        or len(loop_report.get("processed_batch_files", [])) != rounds
        or len(loop_report.get("archived_batch_files", [])) != rounds
    ):
        raise RuntimeError("persistent advance loop summary failed its exact gate")
    if any(
        path.is_file() and path.name.endswith(".batch.json")
        for path in pending_batches.iterdir()
    ):
        raise RuntimeError("persistent advance left a processed batch pending")
    batches = validate_batch_build_report(
        batch_report,
        batch_root=processed_batches,
        signed_transfer_corpus=signed_transfer_corpus,
        rounds=rounds,
        expected_builder_revision=expected_builder_revision,
    )
    certificate_files = {
        path.name
        for path in processed_batches.iterdir()
        if path.is_file() and path.name.endswith(".certificate.json")
    }
    expected_certificates = {
        f"round-{index:06}.certificate.json" for index in range(1, rounds + 1)
    }
    if certificate_files != expected_certificates:
        raise RuntimeError("persistent advance archived the wrong certificate set")

    iterations = [
        persistent_advance_iteration(
            round_report,
            batches[index],
            iteration=index + 1,
            block_height=initial_height + index + 1,
        )
        for index, round_report in enumerate(loop_rounds)
        if isinstance(round_report, dict)
    ]
    if len(iterations) != rounds:
        raise RuntimeError("persistent advance loop contains a malformed round")

    after = full_fleet_status(node_bin, nodes)
    final_height, final_tip, final_root = fleet_identity(after)
    if final_height != initial_height + rounds:
        raise RuntimeError(
            f"{label} finalized height {final_height}, expected {initial_height + rounds}"
        )
    corpus_sha256 = digest(signed_transfer_corpus)
    report = {
        "schema": PERSISTENT_ADVANCE_REPORT_SCHEMA,
        "generated_utc": f"unix_seconds:{int(time.time())}",
        "status": "passed",
        "config": {
            "mode": "setup-only-persistent-advance",
            "build_mode": "release",
            "transport": "single-process-peer-certified-batch-loop",
            "base_dir": nodes.as_posix(),
            "topology_file": topology.as_posix(),
            "validators": VALIDATORS,
            "rounds": rounds,
            "vote_policy": "full",
            "timeout_ms": QUALIFICATION_TIMEOUT_MS,
            "input_source": "signed-transfer-corpus-prebuilt-batch",
            "signed_transfer_corpus": signed_transfer_corpus.as_posix(),
            "signed_transfer_corpus_sha256": corpus_sha256,
            "signed_transfer_corpus_offset": 0,
            "resident_transactional_store": True,
            "expected_start_height": initial_height,
            "local_apply_before_certified_send": True,
            "defer_certified_sends": False,
            "performance_evidence": False,
        },
        "checks": {
            "all_receipts_accepted": True,
            "all_rounds_ok": True,
            "all_transactions_final": True,
            "all_vote_policies_match": True,
            "converged": True,
            "exact_input_binding": True,
            "final_height_matches_rounds": True,
            "iteration_count_matches_rounds": True,
            "no_duplicate_receipts": True,
            "state_verified_after_run": True,
        },
        "not_measured": [
            "release performance comparison",
            "wallet signing latency",
            "mempool admission latency",
        ],
        "final_state": {
            "height": final_height,
            "block_tip_hash": final_tip,
            "state_root": final_root,
            "state_verification_count": VALIDATORS,
        },
        "iterations": iterations,
        "latency": {
            metric: latency_stats([float(row[metric]) for row in iterations])
            for metric in (
                "wallet_to_finality_ms",
                "admitted_to_finality_ms",
                "consensus_round_ms",
                "refresh_account_tx_index_ms",
            )
        },
    }
    report_path = label_root / "report.json"
    iterations_path = label_root / "iterations.jsonl"
    write_json(report_path, report)
    iterations_path.write_text(
        "".join(json.dumps(row, sort_keys=True) + "\n" for row in iterations),
        encoding="utf-8",
    )

    counters, remote_storage = storage_work_from_report(
        report, SELECTED_STORAGE_LANE
    )
    backend_work_gate_pass = (
        counters["full_history_scans"] == 0
        and counters["full_history_records_read"] == 0
        and counters["full_history_bytes_read"] == 0
        and counters["committed_write_transactions"] == rounds * VALIDATORS
    )
    if not backend_work_gate_pass:
        raise RuntimeError(f"{label} selected-store work gate failed: {counters}")

    if not samples:
        raise RuntimeError(f"{label} resource sampler emitted no samples")
    resource = SHARED.resource_summary(samples)
    sample_origin_ns = int(samples[0]["monotonic_ns"])
    normalized_samples: list[dict[str, Any]] = []
    for sample in samples:
        normalized_sample = dict(sample)
        normalized_sample["monotonic_offset_ns"] = (
            int(normalized_sample.pop("monotonic_ns")) - sample_origin_ns
        )
        normalized_samples.append(normalized_sample)
    normalized_foreground = {
        "pid": foreground["pid"],
        "started_offset_ns": foreground["started_monotonic_ns"] - sample_origin_ns,
        "ended_offset_ns": foreground["ended_monotonic_ns"] - sample_origin_ns,
    }
    foreground_sample_count = sum(
        1
        for sample in normalized_samples
        if normalized_foreground["started_offset_ns"]
        <= sample["monotonic_offset_ns"]
        <= normalized_foreground["ended_offset_ns"]
        and str(foreground["pid"]) in sample["processes"]
    )
    if foreground_sample_count < 2:
        raise RuntimeError(
            f"{label} resource sampler observed the persistent process fewer than two times"
        )
    resource_samples_path = root / "resource-samples" / f"{label}.json"
    write_json(
        resource_samples_path,
        {
            "schema": RESOURCE_SAMPLE_SCHEMA,
            "sample_target_interval_ms": RESOURCE_SAMPLE_TARGET_INTERVAL_MS,
            "samples": normalized_samples,
            "foreground_processes": [normalized_foreground],
            "foreground_sample_counts": {
                str(foreground["pid"]): foreground_sample_count
            },
        },
    )
    normalized = normalize_report_paths(report, root)
    normalized_config = normalized["config"]
    normalized_config["signed_transfer_corpus"] = "$SIGNED_TRANSFER_CORPUS"
    normalized_config["base_dir"] = "$WORKING_FLEET"
    normalized_path = root / "normalized" / f"{label}.report.json"
    write_json(normalized_path, normalized)

    result_snapshot: Path | None = None
    if prepared_fleet is None:
        result_snapshot = root / "snapshots" / f"{label}.snapshot"
        export_snapshot(
            node_bin,
            nodes / "validator-0",
            result_snapshot,
            logs,
            label,
        )
    result_prepared_fleet_sha256 = directory_digest(nodes)
    result = {
        "label": label,
        "storage_lane": SELECTED_STORAGE_LANE,
        "advance_execution_mode": "persistent-peer-certified-batch-loop",
        "performance_evidence": False,
        "source_snapshot_sha256": (
            directory_digest(source_snapshot) if source_snapshot is not None else None
        ),
        "node_preparation_mode": node_preparation_mode,
        "prepared_fleet_sha256": observed_prepared_fleet_sha256,
        "result_prepared_fleet_sha256": result_prepared_fleet_sha256,
        "signed_transfer_corpus": signed_transfer_corpus.as_posix(),
        "signed_transfer_corpus_sha256": corpus_sha256,
        "starting_height": initial_height,
        "rounds": rounds,
        "validators_converged": VALIDATORS,
        "literal_receipts_exact": True,
        "backend_work_gate_pass": True,
        "zero_full_history_reads": True,
        "bounded_index_pages": (
            counters["page_reads"]
            <= rounds
            * (
                MAX_PROPOSAL_PAGE_READS_PER_ROUND
                + (VALIDATORS - 1) * MAX_PROPOSAL_PAGE_READS_PER_ROUND
                + VALIDATORS * MAX_APPLY_PAGE_READS_PER_VALIDATOR_ROUND
            )
        ),
        "constant_accumulator_work": True,
        "final_height": final_height,
        "final_tip": final_tip,
        "final_state_root": final_root,
        "latency": report["latency"],
        "storage": counters,
        "resources": {
            "cpu_ticks": resource["validator_cpu_ticks"],
            "peak_rss_kib": resource["validator_peak_rss_kib"],
            "disk_growth_bytes": max(0, resource["node_disk_delta_bytes"]),
            "bytes_read": resource["validator_read_bytes"],
            "bytes_written": resource["validator_write_bytes"],
            "page_reads": counters.get("page_reads"),
            "page_writes": counters.get("page_writes"),
            "fsync_count": counters.get("fsync_count"),
            "fsync_micros": counters.get("durable_commit_micros"),
            "sample_count": resource["sample_count"],
            "duration_ms": resource["duration_ms"],
            "observed_pid_count": len(resource["observed_pids"]),
            "foreground_process_count": 1,
            "foreground_min_sample_count": foreground_sample_count,
            "host_cpu_ticks": resource["host_cpu_ticks"],
            "host_total_memory_kib": resource["host_total_memory_kib"],
            "host_min_available_memory_kib": resource[
                "host_min_available_memory_kib"
            ],
            "network_received_bytes": resource["network_received_bytes"],
            "network_transmitted_bytes": resource["network_transmitted_bytes"],
        },
        "storage_telemetry_source": (
            "persistent driver proposal, five remote validator reconstructions, "
            "local apply, and five in-process certified-apply deltas"
        ),
        "remote_validator_storage_final": remote_storage,
        "initial_fleet": [
            {
                "node_id": status["node_id"],
                "height": status["block_height"],
                "tip": status["block_tip_hash"],
                "state_root": status["state_root"],
            }
            for status in before
        ],
        "final_fleet": [
            {
                "node_id": status["node_id"],
                "height": status["block_height"],
                "tip": status["block_tip_hash"],
                "state_root": status["state_root"],
            }
            for status in after
        ],
        "batch_builder_binary_sha256": digest(batch_builder_bin),
        "batch_builder_build": {
            "git_revision": batch_report["source_git_revision"],
            "profile": batch_report["build_profile"],
        },
        "batch_builder_report": batch_report_path.relative_to(root).as_posix(),
        "batch_builder_report_sha256": digest(batch_report_path),
        "batch_loop_report": loop_report_path.relative_to(root).as_posix(),
        "batch_loop_report_sha256": digest(loop_report_path),
        "processed_batches": processed_batches.relative_to(root).as_posix(),
        "processed_batches_sha256": directory_digest(processed_batches),
        "normalized_report": normalized_path.relative_to(root).as_posix(),
        "normalized_report_sha256": digest(normalized_path),
        "resource_samples": resource_samples_path.relative_to(root).as_posix(),
        "resource_samples_sha256": digest(resource_samples_path),
        "result_snapshot_sha256": (
            directory_digest(result_snapshot) if result_snapshot is not None else None
        ),
    }
    if not result["bounded_index_pages"]:
        raise RuntimeError(f"{label} exceeded the cumulative page bound")
    write_json(root / "receipts" / f"{label}.json", result)
    print(
        f"storage-scaling-persistent-advance={label} start={initial_height} "
        f"end={final_height} rounds={rounds}",
        flush=True,
    )
    return result, result_snapshot


def setup_seed(
    node_bin: Path,
    root: Path,
    base_port: int,
    rpc_base_port: int,
    storage_activation_height: int | None = STORAGE_ACTIVATION_HEIGHT,
    validator_key_file: Path | None = None,
) -> tuple[Path, Path, Path, str, str, Path]:
    private = root / "private"
    logs = root / "logs"
    seed = private / "seed"
    private.mkdir(mode=0o700)
    logs.mkdir()
    if validator_key_file is not None:
        if validator_key_file.is_symlink() or not validator_key_file.is_file():
            raise ValueError("shared validator key file must be a regular file")
        seed.mkdir(mode=0o700)
        staged_validator_keys = seed / "validator_keys.json"
        shutil.copyfile(validator_key_file, staged_validator_keys)
        staged_validator_keys.chmod(0o600)

    init_command = [
        str(node_bin),
        "init-consensus-v2",
        "--data-dir",
        str(seed),
        "--chain-id",
        CHAIN_ID,
        "--node-id",
        "validator-0",
        "--validators",
        str(VALIDATORS),
        "--activation-height",
        str(CONSENSUS_ACTIVATION_HEIGHT),
    ]
    if storage_activation_height is not None:
        init_command.extend(
            ["--storage-activation-height", str(storage_activation_height)]
        )
    SHARED.run(
        init_command,
        stdout_path=logs / "init.json",
        stderr_path=logs / "init.stderr",
    )
    SHARED.split_seed_validator_keys(seed)

    master_seed = private / "wallet-master-seed.hex"
    master_seed.write_text("01" * 32 + "\n", encoding="ascii")
    master_seed.chmod(0o600)
    wallet_key = private / "wallet.key.json"
    wallet_backup = private / "wallet.backup.json"
    recipient_key = private / "recipient.key.json"
    recipient_backup = private / "recipient.backup.json"
    wallet_report_path = logs / "wallet-report.json"
    recipient_report_path = logs / "recipient-report.json"
    for index, key_file, backup_file, report_file, label in (
        (0, wallet_key, wallet_backup, wallet_report_path, "wallet"),
        (1, recipient_key, recipient_backup, recipient_report_path, "recipient"),
    ):
        SHARED.run(
            [
                str(node_bin),
                "wallet-keygen",
                "--chain-id",
                CHAIN_ID,
                "--master-seed-hex-file",
                str(master_seed),
                "--account-index",
                str(index),
                "--key-file",
                str(key_file),
                "--backup-file",
                str(backup_file),
            ],
            stdout_path=report_file,
            stderr_path=logs / f"{label}.stderr",
        )
    wallet_address = str(read_json(wallet_report_path)["address"])
    recipient = str(read_json(recipient_report_path)["address"])

    fund_batch = private / "fund.batch.json"
    fund_certificate = private / "fund.certificate.json"
    SHARED.run(
        [
            str(node_bin),
            "batch-transfer",
            "--data-dir",
            str(seed),
            "--to",
            wallet_address,
            "--amount",
            "100000000",
            "--batch-file",
            str(fund_batch),
        ],
        stdout_path=logs / "fund-create.json",
        stderr_path=logs / "fund-create.stderr",
    )
    SHARED.run(
        [
            str(node_bin),
            "certify-batch",
            "--data-dir",
            str(seed),
            "--batch-kind",
            "transparent",
            "--batch-file",
            str(fund_batch),
            "--validator-key-dir",
            str(seed),
            "--proposal-file",
            str(private / "fund.proposal.json"),
            "--vote-dir",
            str(private / "fund.votes"),
            "--certificate-file",
            str(fund_certificate),
            "--height",
            "1",
        ],
        stdout_path=logs / "fund-certify.json",
        stderr_path=logs / "fund-certify.stderr",
    )
    fund_apply = SHARED.run(
        [
            str(node_bin),
            "apply-batch",
            "--data-dir",
            str(seed),
            "--batch-file",
            str(fund_batch),
            "--certificate-file",
            str(fund_certificate),
        ],
        stdout_path=logs / "fund-apply.json",
        stderr_path=logs / "fund-apply.stderr",
    )
    receipts = json.loads(fund_apply.stdout)
    if not isinstance(receipts, list) or not receipts or not all(
        isinstance(receipt, dict) and receipt.get("accepted") is True
        for receipt in receipts
    ):
        raise RuntimeError("benchmark wallet funding receipt was not accepted")

    seed_snapshot = root / "snapshots" / "height-1.snapshot"
    export_snapshot(node_bin, seed, seed_snapshot, logs, "height-1")

    topology = root / "topology.json"
    topology_command = [
        str(node_bin),
        "topology-consensus-v2",
        "--chain-id",
        CHAIN_ID,
        "--validators",
        str(VALIDATORS),
        "--base-port",
        str(base_port),
        "--rpc-base-port",
        str(rpc_base_port),
        "--activation-height",
        str(CONSENSUS_ACTIVATION_HEIGHT),
        "--output",
        str(topology),
    ]
    if storage_activation_height is not None:
        topology_command.extend(
            ["--storage-activation-height", str(storage_activation_height)]
        )
    SHARED.run(
        topology_command,
        stdout_path=logs / "topology.stdout.json",
        stderr_path=logs / "topology.stderr",
    )
    return seed, seed_snapshot, wallet_key, wallet_address, recipient, topology


def require_release_binary_identity(
    node_bin: Path,
    data_dir: Path,
    expected_source_revision: str,
) -> dict[str, str]:
    completed = SHARED.run(
        [
            str(node_bin),
            "status",
            "--data-dir",
            str(data_dir),
        ]
    )
    status = json.loads(completed.stdout)
    if not isinstance(status, dict):
        raise RuntimeError("release binary status is not a JSON object")
    revision = str(status.get("build_git_revision", ""))
    profile = str(status.get("build_profile", ""))
    if revision != expected_source_revision[:8]:
        raise RuntimeError(
            "release binary embedded revision does not match the requested source"
        )
    if profile != "release":
        raise RuntimeError("qualification binary was not built with the release profile")
    return {"git_revision": revision, "profile": profile}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--node-bin", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--expected-source-revision", required=True)
    parser.add_argument(
        "--development-smoke",
        action="store_true",
        help=(
            "run one round at height 2 from a dirty checkout; the output is "
            "never evidence-eligible"
        ),
    )
    args = parser.parse_args()
    if not args.development_smoke:
        raise ValueError(
            "release qualification requires run_paired_campaign.py; "
            "run_campaign.py is the selected-lane development-smoke helper"
        )

    raw_node_bin = args.node_bin.expanduser()
    raw_root = args.output_dir.expanduser()
    if raw_node_bin.is_symlink() or raw_root.is_symlink():
        raise ValueError("binary and output paths must not be symlinks")
    node_bin = raw_node_bin.resolve()
    root = raw_root.resolve()
    if not node_bin.is_file() or node_bin.parent.name != "release":
        raise ValueError("--node-bin must be a regular target/release binary")
    if root.exists():
        raise ValueError(f"refusing to overwrite output directory: {root}")
    if args.expected_source_revision != run_git_revision():
        raise ValueError("HEAD does not match --expected-source-revision")
    heights = [2]
    windows_per_height = 1
    rounds_per_window = 1
    root.mkdir(parents=True)
    (root / "snapshots").mkdir()
    (root / "receipts").mkdir()
    (root / "normalized").mkdir()
    (root / "corpora").mkdir()

    base_port, rpc_base_port = SHARED.find_ports()
    seed, current_snapshot, wallet_key, wallet_address, recipient, topology = (
        setup_seed(node_bin, root, base_port, rpc_base_port)
    )
    binary_build = require_release_binary_identity(
        node_bin,
        seed,
        args.expected_source_revision,
    )

    current_height = 1
    rows: list[dict[str, Any]] = []
    for target_height in heights:
        if current_height < target_height:
            advance_rounds = target_height - current_height
            advance_label = f"advance-{current_height}-to-{target_height}"
            advance_corpus = root / "corpora" / f"{advance_label}.json"
            create_signed_transfer_corpus(
                node_bin=node_bin,
                source_snapshot=current_snapshot,
                wallet_key=wallet_key,
                wallet_address=wallet_address,
                recipient=recipient,
                count=advance_rounds,
                output_file=advance_corpus,
                logs=root / "logs",
                label=advance_label,
            )
            advance, current_snapshot = run_rounds(
                node_bin=node_bin,
                root=root,
                seed=seed,
                topology=topology,
                source_snapshot=current_snapshot,
                wallet_key=wallet_key,
                wallet_address=wallet_address,
                recipient=recipient,
                signed_transfer_corpus=advance_corpus,
                label=advance_label,
                rounds=advance_rounds,
            )
            current_height = int(advance["final_height"])
        if current_height != target_height:
            raise RuntimeError("campaign snapshot height drifted")

        base_snapshot = current_snapshot
        window_corpus = root / "corpora" / f"height-{target_height}.json"
        create_signed_transfer_corpus(
            node_bin=node_bin,
            source_snapshot=base_snapshot,
            wallet_key=wallet_key,
            wallet_address=wallet_address,
            recipient=recipient,
            count=rounds_per_window,
            output_file=window_corpus,
            logs=root / "logs",
            label=f"height-{target_height}",
        )
        windows: list[dict[str, Any]] = []
        first_result_snapshot: Path | None = None
        for window_index in range(windows_per_height):
            result, result_snapshot = run_rounds(
                node_bin=node_bin,
                root=root,
                seed=seed,
                topology=topology,
                source_snapshot=base_snapshot,
                wallet_key=wallet_key,
                wallet_address=wallet_address,
                recipient=recipient,
                signed_transfer_corpus=window_corpus,
                label=f"height-{target_height}-window-{window_index + 1}",
                rounds=rounds_per_window,
            )
            windows.append(result)
            if window_index == 0:
                first_result_snapshot = result_snapshot
        if first_result_snapshot is None:
            raise RuntimeError("height produced no measurement windows")
        current_snapshot = first_result_snapshot
        current_height = target_height + rounds_per_window
        rows.append({"height": target_height, "windows": windows})

    for row in rows:
        for metric in ("consensus_round_ms", "wallet_to_finality_ms"):
            samples = [
                float(iteration[metric])
                for window in row["windows"]
                for iteration in read_json(root / window["normalized_report"])["iterations"]
            ]
            row.setdefault("aggregate", {})[metric] = distribution_summary(samples)

    window_gates_pass = all(
        window["literal_receipts_exact"] is True
        and window["zero_full_history_reads"] is True
        and window["bounded_index_pages"] is True
        and window["constant_accumulator_work"] is True
        and int(window["validators_converged"]) == VALIDATORS
        and int(window["resources"]["foreground_process_count"]) == 1
        and int(window["resources"]["foreground_min_sample_count"]) >= 2
        for row in rows
        for window in row["windows"]
    )
    status = (
        "DEVELOPMENT SMOKE PASS"
        if window_gates_pass
        else "DEVELOPMENT SMOKE BLOCKED"
    )
    report = {
        "schema": "postfiat-storage-scaling-selected-development-smoke-v1",
        "status": status,
        "campaign_mode": "development-smoke",
        "evidence_eligible": False,
        "source_revision": args.expected_source_revision,
        "node_binary_sha256": digest(node_bin),
        "node_binary": node_bin.name,
        "node_binary_build": binary_build,
        "spec_sha3_384": hashlib.sha3_384(
            (REPO / "docs/architecture/storage-scaling-fix-spec.md").read_bytes()
        ).hexdigest(),
        "shared_runner_sha256": digest(SHARED_RUNNER),
        "topology_sha256": digest(topology),
        "validator_count": VALIDATORS,
        "windows_per_height": windows_per_height,
        "rounds_per_window": rounds_per_window,
        "rows": rows,
        "window_gates_pass": window_gates_pass,
        "page_bounds_per_round": {
            "proposal_reads": MAX_PROPOSAL_PAGE_READS_PER_ROUND,
            "vote_reconstruction_reads_per_validator": (
                MAX_PROPOSAL_PAGE_READS_PER_ROUND
            ),
            "apply_reads_per_validator": MAX_APPLY_PAGE_READS_PER_VALIDATOR_ROUND,
            "apply_writes_per_validator": MAX_APPLY_PAGE_WRITES_PER_VALIDATOR_ROUND,
        },
        "claims_not_made": [
            "release qualification",
            "legacy baseline comparison",
            "height scaling relationship",
            "public WAN or devnet performance",
        ],
        "devnet_queried_or_mutated": False,
    }
    write_json(root / "campaign-report.json", report)
    print(f"storage-scaling-campaign={status}", flush=True)
    print(f"report={root / 'campaign-report.json'}", flush=True)
    return 0 if status == "DEVELOPMENT SMOKE PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
