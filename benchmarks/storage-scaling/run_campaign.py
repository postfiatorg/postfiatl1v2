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
RESOURCE_SAMPLE_SCHEMA = "postfiat-storage-resource-samples-v1"
RESOURCE_SAMPLE_TARGET_INTERVAL_MS = 100
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
    rows: list[dict[str, Any]], root: Path
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
            if not isinstance(iterations, list) or len(iterations) != ROUNDS_PER_WINDOW:
                raise RuntimeError(
                    f"height {height} performance window has the wrong round count"
                )
            for stage, path in MATERIAL_STAGE_PATHS.items():
                values = [
                    nested_positive_float(iteration, path)
                    for iteration in iterations
                    if isinstance(iteration, dict)
                ]
                if len(values) != ROUNDS_PER_WINDOW:
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
                if height == HEIGHTS[0]:
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


def add_transactional_counters(
    totals: dict[str, int], counters: dict[str, Any]
) -> None:
    for field in TRANSACTIONAL_COUNTER_FIELDS:
        value = counters.get(field)
        if value is None:
            raise RuntimeError(f"storage telemetry omitted {field}")
        parsed = int(value)
        if parsed < 0:
            raise RuntimeError(f"storage telemetry was negative: {field}")
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
    report: dict[str, Any],
) -> tuple[dict[str, int], dict[str, dict[str, Any]]]:
    iterations = report.get("iterations")
    if not isinstance(iterations, list):
        raise RuntimeError("benchmark report omitted iterations")
    totals = {field: 0 for field in TRANSACTIONAL_COUNTER_FIELDS}
    remote_latest: dict[str, dict[str, Any]] = {}

    for iteration in iterations:
        if not isinstance(iteration, dict):
            raise RuntimeError("benchmark iteration is not an object")
        round_timings = iteration.get("round_timings")
        if not isinstance(round_timings, dict):
            raise RuntimeError("benchmark iteration omitted round timings")

        proposal = round_timings.get("proposal_breakdown")
        if not isinstance(proposal, dict):
            raise RuntimeError("benchmark iteration omitted proposal telemetry")
        proposal_work = proposal.get("transactional_work")
        if not isinstance(proposal_work, dict):
            raise RuntimeError("proposal telemetry omitted transactional work")
        require_bounded_pages(
            proposal_work,
            stage="proposal",
            max_reads=MAX_PROPOSAL_PAGE_READS_PER_ROUND,
            max_writes=0,
        )
        add_transactional_counters(totals, proposal_work)

        local_apply = round_timings.get("local_apply_breakdown")
        if not isinstance(local_apply, dict):
            raise RuntimeError("benchmark iteration omitted local apply telemetry")
        storage_work = local_apply.get("storage_work")
        if not isinstance(storage_work, dict):
            raise RuntimeError("local apply telemetry omitted storage work")
        if (
            int(storage_work.get("full_history_records_read", -1)) != 0
            or int(storage_work.get("full_history_bytes_read", -1)) != 0
        ):
            raise RuntimeError("local apply performed full-history work")
        transactional = storage_work.get("transactional")
        if not isinstance(transactional, dict):
            raise RuntimeError("local apply telemetry omitted transactional work")
        require_bounded_pages(
            transactional,
            stage="local apply",
            max_reads=MAX_APPLY_PAGE_READS_PER_VALIDATOR_ROUND,
            max_writes=MAX_APPLY_PAGE_WRITES_PER_VALIDATOR_ROUND,
        )
        add_transactional_counters(totals, transactional)

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
            if storage.get("transactional_active") is not True:
                raise RuntimeError(
                    f"certified-send target {node_id} was not transactionally active"
                )
            transactional_work = target.get("transactional_work")
            if not isinstance(transactional_work, dict):
                raise RuntimeError(
                    f"certified-send target {node_id} omitted exact apply telemetry"
                )
            require_bounded_pages(
                transactional_work,
                stage=f"certified apply {node_id}",
                max_reads=MAX_APPLY_PAGE_READS_PER_VALIDATOR_ROUND,
                max_writes=MAX_APPLY_PAGE_WRITES_PER_VALIDATOR_ROUND,
            )
            add_transactional_counters(totals, transactional_work)
            remote_latest[node_id] = {
                "storage": storage,
                "last_apply_transactional_work": transactional_work,
            }

    totals["fsync_count"] = totals["committed_write_transactions"]
    return totals, remote_latest


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


def run_rounds(
    *,
    node_bin: Path,
    root: Path,
    seed: Path,
    topology: Path,
    source_snapshot: Path,
    wallet_key: Path,
    wallet_address: str,
    recipient: str,
    label: str,
    rounds: int,
    storage_lane: str = SELECTED_STORAGE_LANE,
) -> tuple[dict[str, Any], Path]:
    if storage_lane not in HISTORICAL_STORAGE_LANES | {SELECTED_STORAGE_LANE}:
        raise ValueError(f"unsupported storage lane: {storage_lane}")
    selected_transactional = storage_lane == SELECTED_STORAGE_LANE
    logs = root / "logs"
    nodes = root / "nodes"
    SHARED.prepare_nodes(node_bin, nodes, source_snapshot, seed, logs, label)
    before = full_fleet_status(node_bin, nodes)
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

    counters: dict[str, Any]
    remote_storage: dict[str, dict[str, Any]]
    if selected_transactional:
        counters, remote_storage = storage_work_from_report(report)
        if (
            counters["full_history_scans"] != 0
            or counters["full_history_records_read"] != 0
            or counters["full_history_bytes_read"] != 0
        ):
            raise RuntimeError(f"{label} performed full-history work: {counters}")
        if counters["committed_write_transactions"] != rounds * VALIDATORS:
            raise RuntimeError(
                f"{label} durable commit count is not one per height and validator: {counters}"
            )
    else:
        counters = {
            "telemetry_available": False,
            "reason": "historical release report predates transactional page counters",
        }
        remote_storage = {}

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
    normalized_path = root / "normalized" / f"{label}.report.json"
    write_json(normalized_path, normalized)

    result_snapshot = root / "snapshots" / f"{label}.snapshot"
    export_snapshot(
        node_bin,
        nodes / "validator-0",
        result_snapshot,
        logs,
        label,
    )
    result = {
        "label": label,
        "storage_lane": storage_lane,
        "source_snapshot_sha256": directory_digest(source_snapshot),
        "starting_height": initial_height,
        "rounds": rounds,
        "validators_converged": VALIDATORS,
        "literal_receipts_exact": True,
        "zero_full_history_reads": True if selected_transactional else None,
        "bounded_index_pages": (
            counters["page_reads"]
            <= rounds
            * (
                MAX_PROPOSAL_PAGE_READS_PER_ROUND
                + VALIDATORS * MAX_APPLY_PAGE_READS_PER_VALIDATOR_ROUND
            )
            if selected_transactional
            else None
        ),
        "constant_accumulator_work": (
            counters["page_writes"]
            <= rounds * VALIDATORS * MAX_APPLY_PAGE_WRITES_PER_VALIDATOR_ROUND
            if selected_transactional
            else None
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
            "proposal/apply deltas plus in-process certified-send acknowledgements"
            if selected_transactional
            else "external process and filesystem resource sampler only"
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
        "result_snapshot_sha256": directory_digest(result_snapshot),
    }
    write_json(root / "receipts" / f"{label}.json", result)
    print(
        f"storage-scaling-window={label} start={initial_height} "
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
            advance, current_snapshot = run_rounds(
                node_bin=node_bin,
                root=root,
                seed=seed,
                topology=topology,
                source_snapshot=current_snapshot,
                wallet_key=wallet_key,
                wallet_address=wallet_address,
                recipient=recipient,
                label=f"advance-{current_height}-to-{target_height}",
                rounds=advance_rounds,
            )
            current_height = int(advance["final_height"])
        if current_height != target_height:
            raise RuntimeError("campaign snapshot height drifted")

        base_snapshot = current_snapshot
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
