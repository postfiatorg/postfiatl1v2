#!/usr/bin/env python3
"""Run the time-budgeted, one-binary storage qualification locally.

The release profile runs the selected transactional store at heights 50 and
5,000, then the legacy JSON/JSONL baseline at height 50. Every comparison uses
one exact release binary and one authenticated height-50 snapshot/corpus. Long
advances are split into checkpointed units. No external network or controlled
devnet endpoint is contacted.
"""

from __future__ import annotations

import argparse
import copy
import fcntl
import hashlib
import importlib.util
import json
import os
import platform
import re
import shutil
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

REPO = Path(__file__).resolve().parents[2]
BASE_RUNNER = REPO / "benchmarks" / "storage-scaling" / "run_campaign.py"
SPEC = REPO / "docs" / "architecture" / "storage-scaling-fix-spec.md"
LANE_ORDER = ("selected-indexed", "legacy-jsonl")
STORAGE_BEHAVIORS = {
    "legacy-jsonl": (
        "authenticated JSONL with full-prefix append verification and full "
        "ordered-history proposal work"
    ),
    "selected-indexed": (
        "transactional redb finality path with the fixed-size accumulator"
    ),
}
SCHEMA = "postfiat-storage-scaling-time-budgeted-six-validator-campaign-v4"
CHECKPOINT_SCHEMA = "postfiat-storage-scaling-campaign-checkpoint-v4"
QUALIFICATION_PROFILE = "time-budgeted-redb-v4"
RELEASE_MATRIX = (
    ("selected-indexed", 50),
    ("selected-indexed", 5_000),
    ("legacy-jsonl", 50),
)
DEVELOPMENT_MATRIX = (
    ("selected-indexed", 2),
    ("selected-indexed", 3),
    ("legacy-jsonl", 2),
)
ADVANCE_CHUNK_ROUNDS = 1_500
RELEASE_MAX_WALL_SECONDS = 4 * 60 * 60
DEVELOPMENT_MAX_WALL_SECONDS = 15 * 60
SAFE_UNIT = re.compile(r"[^a-zA-Z0-9_.-]+")


def load_base_runner() -> Any:
    spec = importlib.util.spec_from_file_location(
        "postfiat_storage_scaling_selected_runner", BASE_RUNNER
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load selected-store runner: {BASE_RUNNER}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


BASE = load_base_runner()


class DevelopmentStop(RuntimeError):
    """Intentional development-only stop after a durable checkpoint."""


class TimeBudgetExceeded(RuntimeError):
    """The bound campaign wall-clock budget was exhausted."""


def utc_now() -> str:
    return (
        datetime.now(timezone.utc)
        .isoformat(timespec="seconds")
        .replace("+00:00", "Z")
    )


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


def atomic_write_checkpoint(path: Path, value: dict[str, Any]) -> None:
    temporary = path.with_name(f".{path.name}.tmp")
    if temporary.exists():
        temporary.unlink()
    payload = (json.dumps(value, indent=2, sort_keys=True) + "\n").encode("utf-8")
    descriptor = os.open(
        temporary,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
        0o600,
    )
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        directory = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        if temporary.exists():
            temporary.unlink()


def prepare_runner_root(root: Path, *, seed_root: bool = False) -> None:
    root.mkdir(parents=True)
    names = ["snapshots", "receipts", "normalized"]
    if not seed_root:
        names.append("logs")
    for name in names:
        (root / name).mkdir()


def validator_public_identities(path: Path) -> list[dict[str, str]]:
    value = BASE.read_json(path)
    records = value.get("validators")
    if not isinstance(records, list) or len(records) != BASE.VALIDATORS:
        raise ValueError("validator key file does not contain six records")
    identities: list[dict[str, str]] = []
    for record in records:
        if not isinstance(record, dict):
            raise ValueError("validator key record is malformed")
        node_id = str(record.get("node_id", ""))
        algorithm_id = str(record.get("algorithm_id", ""))
        public_key_hex = str(record.get("public_key_hex", ""))
        if (
            node_id not in {f"validator-{index}" for index in range(BASE.VALIDATORS)}
            or algorithm_id != "ML-DSA-65"
            or not public_key_hex
            or len(public_key_hex) % 2 != 0
        ):
            raise ValueError("validator public key identity is invalid")
        try:
            public_key = bytes.fromhex(public_key_hex)
        except ValueError as error:
            raise ValueError("validator public key is not hexadecimal") from error
        identities.append(
            {
                "node_id": node_id,
                "algorithm_id": algorithm_id,
                "public_key_sha256": hashlib.sha256(public_key).hexdigest(),
            }
        )
    identities.sort(key=lambda identity: identity["node_id"])
    if [identity["node_id"] for identity in identities] != [
        f"validator-{index}" for index in range(BASE.VALIDATORS)
    ]:
        raise ValueError("validator public key identities are duplicated")
    return identities


def require_revision(value: str, label: str) -> None:
    if len(value) != 40 or any(byte not in "0123456789abcdef" for byte in value):
        raise ValueError(f"{label} must be a full lowercase Git object ID")


def release_binary(path: Path, label: str) -> Path:
    raw = path.expanduser()
    if raw.is_symlink():
        raise ValueError(f"{label} must not be a symlink")
    resolved = raw.resolve()
    if not resolved.is_file() or resolved.parent.name != "release":
        raise ValueError(f"{label} must be a regular target/release binary")
    return resolved


def aggregate_rows(rows: list[dict[str, Any]], lane_root: Path) -> None:
    resource_fields = (
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
    for row in rows:
        for metric in ("consensus_round_ms", "wallet_to_finality_ms"):
            samples = [
                float(iteration[metric])
                for window in row["windows"]
                for iteration in BASE.read_json(
                    lane_root / window["normalized_report"]
                )["iterations"]
            ]
            row.setdefault("aggregate", {})[metric] = BASE.distribution_summary(samples)
        row["resource_variance"] = {
            field: BASE.distribution_summary(
                [float(window["resources"][field]) for window in row["windows"]]
            )
            for field in resource_fields
        }


def prefix_report_references(
    rows: list[dict[str, Any]], lane_root: Path, campaign_root: Path
) -> None:
    prefix = lane_root.relative_to(campaign_root)
    for row in rows:
        for window in row["windows"]:
            for field in ("normalized_report", "resource_samples"):
                relative = Path(str(window[field]))
                window[field] = (prefix / relative).as_posix()
            corpus = Path(str(window["signed_transfer_corpus"]))
            window["signed_transfer_corpus"] = corpus.relative_to(
                campaign_root
            ).as_posix()


def metric_p95(lane: dict[str, Any], height: int, metric: str) -> float:
    row = next(row for row in lane["rows"] if int(row["height"]) == height)
    return float(row["aggregate"][metric]["p95"])


def host_description(campaign_root: Path) -> dict[str, Any]:
    uname = platform.uname()
    filesystem = os.statvfs(campaign_root)
    rustc = subprocess.run(
        ["rustc", "-Vv"],
        cwd=REPO,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout.strip()
    return {
        "system": uname.system,
        "kernel_release": uname.release,
        "machine": uname.machine,
        "logical_cpu_count": os.cpu_count(),
        "cpu_affinity": sorted(os.sched_getaffinity(0)),
        "page_size_bytes": os.sysconf("SC_PAGE_SIZE"),
        "campaign_root_device": campaign_root.stat().st_dev,
        "filesystem_block_size_bytes": filesystem.f_bsize,
        "clock_ticks_per_second": os.sysconf("SC_CLK_TCK"),
        "python": platform.python_version(),
        "rustc": rustc,
    }


def window_transaction_identities(
    lane_root: Path, window: dict[str, Any]
) -> tuple[tuple[str, str], ...]:
    report = BASE.read_json(lane_root / str(window["normalized_report"]))
    iterations = report.get("iterations")
    if not isinstance(iterations, list):
        raise RuntimeError("normalized benchmark report omitted iterations")
    identities: list[tuple[str, str]] = []
    for iteration in iterations:
        if not isinstance(iteration, dict):
            raise RuntimeError("normalized benchmark iteration is malformed")
        tx_id = str(iteration.get("tx_id", ""))
        signed_digest = str(iteration.get("signed_transfer_sha256", ""))
        if not tx_id or len(signed_digest) != 64:
            raise RuntimeError("normalized benchmark iteration omitted exact input identity")
        identities.append((tx_id, signed_digest))
    return tuple(identities)


def campaign_configuration(development_smoke: bool) -> dict[str, Any]:
    matrix = DEVELOPMENT_MATRIX if development_smoke else RELEASE_MATRIX
    return {
        "qualification_profile": (
            "development-resume-smoke-v1"
            if development_smoke
            else QUALIFICATION_PROFILE
        ),
        "lane_height_matrix": [
            {"lane": lane, "height": height} for lane, height in matrix
        ],
        "windows_per_height": 1 if development_smoke else BASE.WINDOWS_PER_HEIGHT,
        "rounds_per_window": 1 if development_smoke else BASE.ROUNDS_PER_WINDOW,
        "advance_chunk_rounds": 1 if development_smoke else ADVANCE_CHUNK_ROUNDS,
        "node_preparation_mode": "byte-verified-prepared-fleet-clone",
        "advance_execution_mode": "persistent-peer-certified-batch-loop",
        "timeout_ms": BASE.QUALIFICATION_TIMEOUT_MS,
        "max_wall_seconds": (
            DEVELOPMENT_MAX_WALL_SECONDS
            if development_smoke
            else RELEASE_MAX_WALL_SECONDS
        ),
    }


def runner_bindings() -> dict[str, str]:
    return {
        "spec_sha3_384": hashlib.sha3_384(SPEC.read_bytes()).hexdigest(),
        "paired_runner_sha256": sha256(Path(__file__).resolve()),
        "selected_runner_sha256": sha256(BASE_RUNNER),
        "shared_runner_sha256": sha256(BASE.SHARED_RUNNER),
    }


def safe_campaign_path(root: Path, raw: str, label: str) -> Path:
    relative = Path(raw)
    if relative.is_absolute() or not relative.parts or any(
        part in {"", ".", ".."} for part in relative.parts
    ):
        raise ValueError(f"{label} is not a safe campaign-relative path")
    resolved = root.joinpath(*relative.parts).resolve()
    if not resolved.is_relative_to(root):
        raise ValueError(f"{label} escaped the campaign root")
    return resolved


def optional_campaign_path(root: Path, raw: Any, label: str) -> Path | None:
    if raw is None:
        return None
    if not isinstance(raw, str):
        raise ValueError(f"{label} is not a string or null")
    return safe_campaign_path(root, raw, label)


def is_sha256(value: Any) -> bool:
    return (
        isinstance(value, str)
        and re.fullmatch(r"[0-9a-f]{64}", value) is not None
    )


class CampaignState:
    def __init__(
        self,
        root: Path,
        checkpoint: dict[str, Any],
        *,
        stop_after_units: int | None,
    ) -> None:
        self.root = root
        self.path = root / "campaign-checkpoint.json"
        self.value = checkpoint
        self.elapsed_before = float(checkpoint.get("elapsed_wall_seconds", 0.0))
        self.segment_started = time.monotonic()
        self.stop_after_units = stop_after_units
        self.segment_completed_units = 0
        self.current_unit_started_monotonic: float | None = None

    def elapsed(self) -> float:
        return self.elapsed_before + (time.monotonic() - self.segment_started)

    def write(self) -> None:
        self.value["elapsed_wall_seconds"] = self.elapsed()
        self.value["updated_at"] = utc_now()
        atomic_write_checkpoint(self.path, self.value)

    def begin_unit(
        self,
        unit_id: str,
        *,
        runner_root: Path | None = None,
        label: str | None = None,
        owned_corpus: Path | None = None,
        owned_prepared_fleet: Path | None = None,
    ) -> None:
        self.current_unit_started_monotonic = time.monotonic()
        current: dict[str, Any] = {"unit_id": unit_id, "started_at": utc_now()}
        if runner_root is not None:
            current["runner_root"] = runner_root.relative_to(self.root).as_posix()
        if label is not None:
            current["label"] = label
        if owned_corpus is not None:
            current["owned_corpus"] = owned_corpus.relative_to(self.root).as_posix()
        if owned_prepared_fleet is not None:
            current["owned_prepared_fleet"] = owned_prepared_fleet.relative_to(
                self.root
            ).as_posix()
        self.value["current_unit"] = current
        self.write()

    def finish_current_unit_timing(self) -> dict[str, Any]:
        current = self.value.get("current_unit")
        if not isinstance(current, dict):
            raise RuntimeError("campaign has no current unit to finish")
        if self.current_unit_started_monotonic is None:
            raise RuntimeError("current campaign unit omitted its monotonic start")
        timing = {
            "started_at": current["started_at"],
            "finished_at": utc_now(),
            "elapsed_seconds": (
                time.monotonic() - self.current_unit_started_monotonic
            ),
        }
        current.update(timing)
        self.current_unit_started_monotonic = None
        return timing

    def finish_unit(self) -> None:
        current = self.value.get("current_unit")
        if not isinstance(current, dict):
            raise RuntimeError("campaign has no current unit to finish")
        unit_id = str(current.get("unit_id", ""))
        completed = self.value.get("completed_units")
        if not isinstance(completed, dict):
            raise RuntimeError("campaign omitted its completed-unit ledger")
        if not isinstance(completed.get(unit_id), dict):
            completed[unit_id] = {"kind": "material"}
        completed[unit_id].update(
            {"outcome": "completed", **self.finish_current_unit_timing()}
        )
        self.value["current_unit"] = None
        self.value["status"] = "RUNNING"
        self.write()
        self.segment_completed_units += 1
        maximum = int(self.value["configuration"]["max_wall_seconds"])
        if self.elapsed() > maximum:
            raise TimeBudgetExceeded(
                f"campaign exceeded its {maximum}-second wall-clock budget"
            )
        if (
            self.stop_after_units is not None
            and self.segment_completed_units >= self.stop_after_units
        ):
            raise DevelopmentStop("development stop requested after checkpoint")

    def mark_interrupted(self, error: BaseException) -> None:
        status = (
            "TIME_BUDGET_EXCEEDED"
            if isinstance(error, TimeBudgetExceeded)
            else "INTERRUPTED"
            if isinstance(error, (KeyboardInterrupt, DevelopmentStop))
            else "FAILED"
        )
        self.value["status"] = status
        current = self.value.get("current_unit")
        if (
            isinstance(current, dict)
            and self.current_unit_started_monotonic is not None
        ):
            timing = self.finish_current_unit_timing()
            current["error_type"] = type(error).__name__
            current["error_message"] = str(error)
            if status == "FAILED":
                current["outcome"] = "failed"
                self.value.setdefault("failed_units", []).append(
                    {
                        "unit_id": str(current.get("unit_id", "unknown")),
                        "outcome": "failed",
                        **timing,
                        "error_type": type(error).__name__,
                        "error_message": str(error),
                    }
                )
        self.value["last_stop"] = {
            "at": utc_now(),
            "type": type(error).__name__,
            "message": str(error),
        }
        self.write()


def completed_unit_record(
    root: Path,
    runner_root: Path,
    *,
    kind: str,
    result: dict[str, Any],
    result_snapshot: Path | None,
    result_prepared_fleet: Path | None = None,
) -> dict[str, Any]:
    receipt = runner_root / "receipts" / f"{result['label']}.json"
    if BASE.read_json(receipt) != result:
        raise RuntimeError("completed-unit receipt differs from in-memory result")
    record = {
        "kind": kind,
        "runner_root": runner_root.relative_to(root).as_posix(),
        "result": result,
        "result_snapshot": (
            result_snapshot.relative_to(root).as_posix()
            if result_snapshot is not None
            else None
        ),
        "result_snapshot_sha256": (
            BASE.directory_digest(result_snapshot)
            if result_snapshot is not None
            else None
        ),
        "result_prepared_fleet": (
            result_prepared_fleet.relative_to(root).as_posix()
            if result_prepared_fleet is not None
            else None
        ),
        "result_prepared_fleet_sha256": (
            BASE.directory_digest(result_prepared_fleet)
            if result_prepared_fleet is not None
            else None
        ),
        "receipt": receipt.relative_to(root).as_posix(),
        "receipt_sha256": sha256(receipt),
    }
    if record["result_snapshot_sha256"] != result.get("result_snapshot_sha256"):
        raise RuntimeError("completed-unit snapshot binding differs from its result")
    if result_prepared_fleet is not None and record[
        "result_prepared_fleet_sha256"
    ] != result.get("result_prepared_fleet_sha256"):
        raise RuntimeError("completed-unit prepared fleet differs from its result")
    if result_snapshot is None and not is_sha256(
        result.get("result_prepared_fleet_sha256")
    ):
        raise RuntimeError("snapshot-free completed unit omitted its result fleet digest")
    return record


def verify_completed_unit(
    root: Path,
    record: dict[str, Any],
    *,
    expected_builder_revision: str | None = None,
    expected_builder_sha256: str | None = None,
) -> None:
    runner_root = safe_campaign_path(
        root, str(record.get("runner_root", "")), "completed unit runner root"
    )
    result = record.get("result")
    if not isinstance(result, dict):
        raise ValueError("completed unit omitted its result")
    receipt = safe_campaign_path(
        root, str(record.get("receipt", "")), "completed unit receipt"
    )
    if sha256(receipt) != record.get("receipt_sha256"):
        raise ValueError("completed unit receipt digest changed")
    if BASE.read_json(receipt) != result:
        raise ValueError("completed unit receipt content changed")
    snapshot = optional_campaign_path(
        root, record.get("result_snapshot"), "completed unit snapshot"
    )
    snapshot_sha256 = record.get("result_snapshot_sha256")
    if snapshot is None:
        if snapshot_sha256 is not None or result.get("result_snapshot_sha256") is not None:
            raise ValueError("completed unit snapshot nullability changed")
        if not is_sha256(result.get("result_prepared_fleet_sha256")):
            raise ValueError("snapshot-free completed unit omitted its result fleet digest")
    elif (
        BASE.directory_digest(snapshot) != snapshot_sha256
        or snapshot_sha256 != result.get("result_snapshot_sha256")
    ):
        raise ValueError("completed unit snapshot digest changed")
    result_prepared_fleet = optional_campaign_path(
        root,
        record.get("result_prepared_fleet"),
        "completed unit result prepared fleet",
    )
    result_prepared_sha256 = record.get("result_prepared_fleet_sha256")
    if result_prepared_fleet is None:
        if result_prepared_sha256 is not None:
            raise ValueError("completed unit retained fleet nullability changed")
    else:
        BASE.validate_prepared_fleet(result_prepared_fleet)
        if (
            BASE.directory_digest(result_prepared_fleet) != result_prepared_sha256
            or result_prepared_sha256 != result.get("result_prepared_fleet_sha256")
        ):
            raise ValueError("completed unit result prepared fleet changed")
    for field in ("normalized_report", "resource_samples"):
        artifact = (runner_root / str(result.get(field, ""))).resolve()
        expected = result.get(f"{field}_sha256")
        if (
            not artifact.is_relative_to(runner_root)
            or artifact.is_symlink()
            or not artifact.is_file()
            or sha256(artifact) != expected
        ):
            raise ValueError(f"completed unit {field} changed")
    corpus = Path(str(result.get("signed_transfer_corpus", "")))
    if not corpus.is_absolute() or not corpus.resolve().is_relative_to(root):
        raise ValueError("completed unit corpus path is outside the campaign")
    if sha256(corpus) != result.get("signed_transfer_corpus_sha256"):
        raise ValueError("completed unit corpus changed")
    corpus_generation = result.get("corpus_generation")
    if corpus_generation is not None:
        if not isinstance(corpus_generation, dict):
            raise ValueError("completed unit corpus generation is malformed")
        scratch_before = corpus_generation.get("scratch_before_sha256")
        scratch_after = corpus_generation.get("scratch_after_sha256")
        if (
            corpus_generation.get("mode")
            != "disposable-canonical-prepared-fleet-clone"
            or corpus_generation.get("source_prepared_fleet_sha256")
            != result.get("prepared_fleet_sha256")
            or not is_sha256(scratch_before)
            or not is_sha256(scratch_after)
            or scratch_before != result.get("prepared_fleet_sha256")
            or corpus_generation.get("scratch_restored_sha256")
            != result.get("prepared_fleet_sha256")
            or corpus_generation.get("scratch_mutated")
            is not (scratch_before != scratch_after)
            or corpus_generation.get("scratch_discarded") is not True
        ):
            raise ValueError("completed unit corpus generation binding changed")
    if result.get("advance_execution_mode") == "persistent-peer-certified-batch-loop":
        if (
            result.get("performance_evidence") is not False
            or not is_sha256(result.get("batch_builder_binary_sha256"))
            or (
                expected_builder_sha256 is not None
                and result.get("batch_builder_binary_sha256")
                != expected_builder_sha256
            )
        ):
            raise ValueError("persistent advance builder identity changed")

        def bound_runner_path(field: str, *, directory: bool = False) -> Path:
            path = (runner_root / str(result.get(field, ""))).resolve()
            if not path.is_relative_to(runner_root) or path.is_symlink():
                raise ValueError(f"persistent advance {field} escaped its runner")
            if directory and not path.is_dir():
                raise ValueError(f"persistent advance {field} is not a directory")
            if not directory and not path.is_file():
                raise ValueError(f"persistent advance {field} is not a file")
            return path

        batch_report_path = bound_runner_path("batch_builder_report")
        loop_report_path = bound_runner_path("batch_loop_report")
        processed_batches = bound_runner_path("processed_batches", directory=True)
        if (
            sha256(batch_report_path)
            != result.get("batch_builder_report_sha256")
            or sha256(loop_report_path) != result.get("batch_loop_report_sha256")
            or BASE.directory_digest(processed_batches)
            != result.get("processed_batches_sha256")
        ):
            raise ValueError("persistent advance raw artifact changed")
        batch_report = BASE.read_json(batch_report_path)
        builder_revision = str(batch_report.get("source_git_revision", ""))
        builder_build = result.get("batch_builder_build")
        if (
            not isinstance(builder_build, dict)
            or builder_build.get("git_revision") != builder_revision
            or builder_build.get("profile") != batch_report.get("build_profile")
        ):
            raise ValueError("persistent advance builder build changed")
        if expected_builder_revision is not None and builder_revision != expected_builder_revision[:8]:
            raise ValueError("persistent advance builder revision changed")
        batches = BASE.validate_batch_build_report(
            batch_report,
            batch_root=processed_batches,
            signed_transfer_corpus=corpus,
            rounds=int(result.get("rounds", 0)),
            expected_builder_revision=(
                expected_builder_revision
                if expected_builder_revision is not None
                else builder_revision.ljust(40, "0")
            ),
        )
        loop_report = BASE.read_json(loop_report_path)
        loop_rounds = loop_report.get("rounds")
        rounds = int(result.get("rounds", 0))
        start = int(result.get("starting_height", -1))
        if (
            loop_report.get("schema")
            != "postfiat-transport-peer-certified-batch-loop-v1"
            or loop_report.get("loop_ok") is not True
            or int(loop_report.get("processed_round_count", 0)) != rounds
            or not isinstance(loop_rounds, list)
            or len(loop_rounds) != rounds
        ):
            raise ValueError("persistent advance loop report changed")
        verified = [
            BASE.persistent_advance_iteration(
                raw,
                batches[index],
                iteration=index + 1,
                block_height=start + index + 1,
            )
            for index, raw in enumerate(loop_rounds)
            if isinstance(raw, dict)
        ]
        if (
            len(verified) != rounds
            or verified[-1]["block_height"] != result.get("final_height")
        ):
            raise ValueError("persistent advance independently verified rounds changed")


def move_if_present(source: Path, destination_root: Path) -> str | None:
    if not source.exists():
        return None
    destination_root.mkdir(parents=True, exist_ok=True)
    destination = destination_root / source.name
    if destination.exists():
        raise ValueError(f"interrupted-unit destination already exists: {destination}")
    shutil.move(str(source), str(destination))
    return destination.name


def quarantine_current_unit(state: CampaignState) -> None:
    current = state.value.get("current_unit")
    if not isinstance(current, dict):
        return
    if not isinstance(current.get("finished_at"), str):
        finished_at = utc_now()
        started_at = str(current.get("started_at", finished_at))
        try:
            elapsed_seconds = max(
                0.0,
                (
                    datetime.fromisoformat(finished_at.replace("Z", "+00:00"))
                    - datetime.fromisoformat(started_at.replace("Z", "+00:00"))
                ).total_seconds(),
            )
        except ValueError:
            started_at = finished_at
            elapsed_seconds = 0.0
        current.update(
            {
                "started_at": started_at,
                "finished_at": finished_at,
                "elapsed_seconds": elapsed_seconds,
            }
        )
    unit_id = str(current.get("unit_id", "unknown"))
    safe_name = SAFE_UNIT.sub("-", unit_id).strip("-") or "unknown"
    destination = (
        state.root
        / "interrupted"
        / f"{safe_name}-{datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')}"
    )
    moved: list[str] = []
    runner_raw = current.get("runner_root")
    label = current.get("label")
    if isinstance(runner_raw, str) and isinstance(label, str):
        runner = safe_campaign_path(state.root, runner_raw, "current unit runner")
        candidates = [
            runner / label,
            runner / "snapshots" / f"{label}.snapshot",
            runner / "normalized" / f"{label}.report.json",
            runner / "resource-samples" / f"{label}.json",
            runner / "receipts" / f"{label}.json",
            runner / "nodes",
        ]
        for candidate in candidates:
            name = move_if_present(candidate, destination)
            if name is not None:
                moved.append(name)
        logs = runner / "logs"
        if logs.is_dir():
            for candidate in sorted(logs.glob(f"{label}*")):
                name = move_if_present(candidate, destination / "logs")
                if name is not None:
                    moved.append(f"logs/{name}")
    corpus_raw = current.get("owned_corpus")
    if isinstance(corpus_raw, str):
        corpus = safe_campaign_path(state.root, corpus_raw, "current unit corpus")
        name = move_if_present(corpus, destination / "corpora")
        if name is not None:
            moved.append(f"corpora/{name}")
    prepared_fleet_raw = current.get("owned_prepared_fleet")
    if isinstance(prepared_fleet_raw, str):
        prepared_fleet = safe_campaign_path(
            state.root,
            prepared_fleet_raw,
            "current unit prepared fleet",
        )
        name = move_if_present(prepared_fleet, destination / "prepared-fleets")
        if name is not None:
            moved.append(f"prepared-fleets/{name}")
    state.value.setdefault("interrupted_units", []).append(
        {
            "unit_id": unit_id,
            "outcome": "discarded",
            "quarantined_at": utc_now(),
            "moved_entries": moved,
            "started_at": current.get("started_at"),
            "finished_at": current.get("finished_at"),
            "elapsed_seconds": current.get("elapsed_seconds"),
            "error_type": current.get("error_type"),
            "error_message": current.get("error_message"),
        }
    )
    state.value["current_unit"] = None
    state.value["status"] = "RUNNING"
    state.write()


def validate_checkpoint(
    state: CampaignState,
    *,
    expected_source_revision: str,
    node_bin: Path,
    batch_builder_bin: Path,
    configuration: dict[str, Any],
) -> None:
    checkpoint = state.value
    if checkpoint.get("status") not in {"RUNNING", "INTERRUPTED"}:
        raise ValueError("campaign checkpoint is not resumable")
    if checkpoint.get("schema") != CHECKPOINT_SCHEMA:
        raise ValueError("campaign checkpoint schema mismatch")
    if checkpoint.get("campaign_schema") != SCHEMA:
        raise ValueError("campaign report schema changed")
    if checkpoint.get("configuration") != configuration:
        raise ValueError("campaign configuration changed")
    if checkpoint.get("source_revision") != expected_source_revision:
        raise ValueError("campaign source revision changed")
    if checkpoint.get("runner_source_revision") != BASE.run_git_revision():
        raise ValueError("campaign runner checkout revision changed")
    if checkpoint.get("node_binary_sha256") != sha256(node_bin):
        raise ValueError("campaign release binary changed")
    if checkpoint.get("batch_builder_binary_sha256") != sha256(batch_builder_bin):
        raise ValueError("campaign batch builder binary changed")
    if checkpoint.get("runner_bindings") != runner_bindings():
        raise ValueError("campaign runner, shared runner, or specification changed")
    if checkpoint.get("host") != host_description(state.root):
        raise ValueError("campaign host allocation or storage medium changed")
    public = checkpoint.get("public_inputs")
    if not isinstance(public, dict):
        raise ValueError("campaign checkpoint omitted public inputs")
    shared_root = state.root / "shared"
    seed = shared_root / "private" / "seed"
    topology = shared_root / "topology.json"
    height_one = shared_root / "snapshots" / "height-1.snapshot"
    if validator_public_identities(seed / "validator_keys.json") != public.get(
        "validator_public_identities"
    ):
        raise ValueError("campaign validator identities changed")
    if sha256(topology) != public.get("topology_sha256"):
        raise ValueError("campaign topology changed")
    if BASE.directory_digest(height_one) != public.get("height_1_snapshot_sha256"):
        raise ValueError("campaign height-1 snapshot changed")
    if BASE.require_release_binary_identity(
        node_bin, seed, expected_source_revision
    ) != checkpoint.get("node_binary_build"):
        raise ValueError("campaign binary build identity changed")
    current_snapshot = optional_campaign_path(
        state.root,
        checkpoint.get("current_snapshot"),
        "campaign current snapshot",
    )
    current_snapshot_sha256 = checkpoint.get("current_snapshot_sha256")
    if current_snapshot is None:
        if current_snapshot_sha256 is not None:
            raise ValueError("campaign current snapshot nullability changed")
    elif BASE.directory_digest(current_snapshot) != current_snapshot_sha256:
        raise ValueError("campaign current snapshot changed")
    current_height = int(checkpoint.get("current_height", 0))
    if current_height == 1 and current_snapshot is None:
        raise ValueError("height-one campaign omitted its current snapshot")
    current_prepared_raw = checkpoint.get("current_prepared_fleet")
    current_prepared_sha256 = checkpoint.get("current_prepared_fleet_sha256")
    if current_height > 1:
        current_prepared = safe_campaign_path(
            state.root,
            str(current_prepared_raw),
            "campaign current prepared fleet",
        )
        BASE.validate_prepared_fleet(current_prepared)
        if BASE.directory_digest(current_prepared) != current_prepared_sha256:
            raise ValueError("campaign current prepared fleet changed")
    elif current_prepared_raw is not None or current_prepared_sha256 is not None:
        raise ValueError("height-one campaign unexpectedly has a prepared fleet")
    materials = checkpoint.get("height_materials")
    if not isinstance(materials, dict):
        raise ValueError("campaign checkpoint omitted height materials")
    for raw_height, material in materials.items():
        if not isinstance(material, dict) or str(int(raw_height)) != raw_height:
            raise ValueError("campaign height material is malformed")
        height = int(raw_height)
        snapshot = optional_campaign_path(
            state.root, material.get("snapshot"), "height snapshot"
        )
        snapshot_sha256 = material.get("snapshot_sha256")
        legacy_height = any(
            entry.get("lane") == "legacy-jsonl" and entry.get("height") == height
            for entry in configuration["lane_height_matrix"]
        )
        corpus = safe_campaign_path(
            state.root,
            str(material.get("signed_transfer_corpus", "")),
            "height signed corpus",
        )
        if legacy_height:
            if snapshot is None or BASE.directory_digest(snapshot) != snapshot_sha256:
                raise ValueError(f"campaign height {raw_height} snapshot changed")
        elif snapshot is not None or snapshot_sha256 is not None:
            raise ValueError(
                f"campaign height {raw_height} retained an unnecessary snapshot"
            )
        if sha256(corpus) != material.get("signed_transfer_corpus_sha256"):
            raise ValueError(f"campaign height {raw_height} corpus changed")
        prepared_fleet = safe_campaign_path(
            state.root,
            str(material.get("prepared_fleet", "")),
            "height prepared fleet",
        )
        BASE.validate_prepared_fleet(prepared_fleet)
        if BASE.directory_digest(prepared_fleet) != material.get(
            "prepared_fleet_sha256"
        ):
            raise ValueError(f"campaign height {raw_height} prepared fleet changed")
        expected_corpus_mode = (
            "authenticated-portable-snapshot-import"
            if legacy_height
            else "disposable-canonical-prepared-fleet-clone"
        )
        if material.get("corpus_source_mode") != expected_corpus_mode:
            raise ValueError(f"campaign height {raw_height} corpus source changed")
        expected_corpus_fleet = (
            None if legacy_height else material.get("prepared_fleet_sha256")
        )
        if (
            material.get("corpus_source_prepared_fleet_sha256")
            != expected_corpus_fleet
        ):
            raise ValueError(
                f"campaign height {raw_height} corpus fleet binding changed"
            )
        scratch_before = material.get("corpus_scratch_before_sha256")
        scratch_after = material.get("corpus_scratch_after_sha256")
        if legacy_height:
            if any(
                material.get(field) is not None
                for field in (
                    "corpus_scratch_before_sha256",
                    "corpus_scratch_after_sha256",
                    "corpus_scratch_mutated",
                    "corpus_scratch_discarded",
                    "corpus_scratch_restored_sha256",
                )
            ):
                raise ValueError(
                    f"campaign height {raw_height} legacy corpus has scratch metadata"
                )
        elif (
            not is_sha256(scratch_before)
            or not is_sha256(scratch_after)
            or scratch_before != material.get("prepared_fleet_sha256")
            or material.get("corpus_scratch_restored_sha256")
            != material.get("prepared_fleet_sha256")
            or material.get("corpus_scratch_mutated")
            is not (scratch_before != scratch_after)
            or material.get("corpus_scratch_discarded") is not True
        ):
            raise ValueError(
                f"campaign height {raw_height} corpus scratch binding changed"
            )
    completed = checkpoint.get("completed_units")
    if not isinstance(completed, dict):
        raise ValueError("campaign checkpoint omitted completed units")
    for unit_id, record in completed.items():
        if not isinstance(record, dict):
            raise ValueError("campaign completed unit is malformed")
        timing_fields = (
            record.get("started_at"),
            record.get("finished_at"),
            record.get("elapsed_seconds"),
        )
        if any(value is not None for value in timing_fields) and (
            not isinstance(timing_fields[0], str)
            or not isinstance(timing_fields[1], str)
            or not isinstance(timing_fields[2], (int, float))
            or isinstance(timing_fields[2], bool)
            or timing_fields[2] < 0
        ):
            raise ValueError("campaign completed unit timing is malformed")
        if record.get("kind") == "material":
            if (
                not unit_id.startswith("material/height-")
                or unit_id.removeprefix("material/height-")
                not in checkpoint["height_materials"]
                or any(value is None for value in timing_fields)
            ):
                raise ValueError("campaign completed material unit is malformed")
            continue
        verify_completed_unit(
            state.root,
            record,
            expected_builder_revision=str(checkpoint["runner_source_revision"]),
            expected_builder_sha256=str(
                checkpoint["batch_builder_binary_sha256"]
            ),
        )
    if float(checkpoint.get("elapsed_wall_seconds", 0.0)) >= int(
        configuration["max_wall_seconds"]
    ):
        raise TimeBudgetExceeded("campaign checkpoint has exhausted its time budget")


def initialize_campaign(
    root: Path,
    *,
    node_bin: Path,
    batch_builder_bin: Path,
    expected_source_revision: str,
    runner_source_revision: str,
    configuration: dict[str, Any],
) -> dict[str, Any]:
    corpora_root = root / "corpora"
    corpora_root.mkdir()
    (root / "prepared-fleets").mkdir()
    shared_root = root / "shared"
    canonical_root = root / "canonical"
    prepare_runner_root(shared_root, seed_root=True)
    prepare_runner_root(canonical_root)
    lane_roots = {lane: root / "lanes" / lane for lane in LANE_ORDER}
    for lane_root in lane_roots.values():
        prepare_runner_root(lane_root)

    base_port, rpc_base_port = BASE.SHARED.find_ports()
    seed, current_snapshot, wallet_key, wallet_address, recipient, topology = (
        BASE.setup_seed(
            node_bin,
            shared_root,
            base_port,
            rpc_base_port,
            storage_activation_height=BASE.STORAGE_ACTIVATION_HEIGHT,
        )
    )
    binary_build = BASE.require_release_binary_identity(
        node_bin,
        seed,
        expected_source_revision,
    )
    return {
        "schema": CHECKPOINT_SCHEMA,
        "campaign_schema": SCHEMA,
        "status": "RUNNING",
        "started_at": utc_now(),
        "updated_at": utc_now(),
        "elapsed_wall_seconds": 0.0,
        "source_revision": expected_source_revision,
        "runner_source_revision": runner_source_revision,
        "node_binary_sha256": sha256(node_bin),
        "node_binary": node_bin.name,
        "node_binary_build": binary_build,
        "batch_builder_binary_sha256": sha256(batch_builder_bin),
        "batch_builder_binary": batch_builder_bin.name,
        "runner_bindings": runner_bindings(),
        "configuration": configuration,
        "host": host_description(root),
        "public_inputs": {
            "wallet_address": wallet_address,
            "recipient_address": recipient,
            "validator_public_identities": validator_public_identities(
                seed / "validator_keys.json"
            ),
            "topology_sha256": sha256(topology),
            "height_1_snapshot_sha256": BASE.directory_digest(current_snapshot),
        },
        "current_height": 1,
        "current_snapshot": current_snapshot.relative_to(root).as_posix(),
        "current_snapshot_sha256": BASE.directory_digest(current_snapshot),
        "current_prepared_fleet": None,
        "current_prepared_fleet_sha256": None,
        "height_materials": {},
        "completed_units": {},
        "current_unit": None,
        "interrupted_units": [],
        "failed_units": [],
        "last_stop": None,
        "final_report_sha256": None,
        "private_paths": {
            "seed": seed.relative_to(root).as_posix(),
            "wallet_key": wallet_key.relative_to(root).as_posix(),
            "topology": topology.relative_to(root).as_posix(),
        },
    }


def campaign_paths(
    root: Path, checkpoint: dict[str, Any]
) -> tuple[Path, Path, str, str, Path]:
    private_paths = checkpoint["private_paths"]
    seed = safe_campaign_path(root, private_paths["seed"], "seed path")
    wallet_key = safe_campaign_path(root, private_paths["wallet_key"], "wallet path")
    topology = safe_campaign_path(root, private_paths["topology"], "topology path")
    public = checkpoint["public_inputs"]
    return (
        seed,
        wallet_key,
        str(public["wallet_address"]),
        str(public["recipient_address"]),
        topology,
    )


def record_result(
    state: CampaignState,
    *,
    unit_id: str,
    runner_root: Path,
    kind: str,
    result: dict[str, Any],
    result_snapshot: Path | None,
    result_prepared_fleet: Path | None = None,
) -> None:
    state.value["completed_units"][unit_id] = completed_unit_record(
        state.root,
        runner_root,
        kind=kind,
        result=result,
        result_snapshot=result_snapshot,
        result_prepared_fleet=result_prepared_fleet,
    )


def validate_canonical_generation_pointers(
    prepared_fleet: Path,
    working_fleet: Path,
) -> None:
    for index in range(BASE.VALIDATORS):
        pointer_path = (
            prepared_fleet / f"validator-{index}" / "transactional_generation.json"
        )
        if pointer_path.is_symlink() or not pointer_path.is_file():
            raise RuntimeError(
                f"prepared validator-{index} omitted its generation pointer"
            )
        payload = pointer_path.read_text(encoding="utf-8")
        try:
            pointer, end = json.JSONDecoder().raw_decode(payload)
        except json.JSONDecodeError as error:
            raise RuntimeError(
                f"prepared validator-{index} generation pointer is malformed"
            ) from error
        if (
            not isinstance(pointer, dict)
            or pointer.get("schema")
            != "postfiat-transactional-generation-pointer-v1"
            or pointer.get("generation") != "generation-00000001"
            or pointer.get("database_file") != "postfiat-state-v1.redb"
            or re.fullmatch(
                r"\s*pftmac1:[0-9a-f]{96}\s*",
                payload[end:],
            )
            is None
        ):
            raise RuntimeError(
                f"prepared validator-{index} generation pointer is malformed"
            )
        expected_directory = (
            working_fleet
            / f"validator-{index}"
            / "transactional-snapshot-generation-v1"
        ).resolve()
        if Path(str(pointer.get("database_directory", ""))) != expected_directory:
            raise RuntimeError(
                f"prepared validator-{index} generation pointer does not bind "
                "the canonical working fleet"
            )


def create_corpus_from_prepared_fleet(
    *,
    node_bin: Path,
    prepared_fleet: Path,
    prepared_fleet_sha256: str,
    working_fleet: Path,
    wallet_key: Path,
    wallet_address: str,
    recipient: str,
    count: int,
    expected_first_sequence: int,
    output_file: Path,
    logs: Path,
    label: str,
) -> tuple[dict[str, Any], dict[str, Any]]:
    BASE.validate_prepared_fleet(prepared_fleet)
    validate_canonical_generation_pointers(prepared_fleet, working_fleet)
    working_before = BASE.clone_prepared_fleet(
        prepared_fleet,
        working_fleet,
        prepared_fleet_sha256,
    )
    working_after: str | None = None
    try:
        report = BASE.create_signed_transfer_corpus(
            node_bin=node_bin,
            source_data_dir=working_fleet / "validator-0",
            wallet_key=wallet_key,
            wallet_address=wallet_address,
            recipient=recipient,
            count=count,
            output_file=output_file,
            logs=logs,
            label=label,
        )
        working_after = BASE.directory_digest(working_fleet)
        expected_last_sequence = expected_first_sequence + count - 1
        if (
            int(report.get("first_sequence", -1)) != expected_first_sequence
            or int(report.get("last_sequence", -1)) != expected_last_sequence
        ):
            raise RuntimeError(
                "prepared-fleet corpus sequence differs from campaign state"
            )
    finally:
        restored_sha256 = BASE.clone_prepared_fleet(
            prepared_fleet,
            working_fleet,
            prepared_fleet_sha256,
        )
    if working_after is None:
        raise RuntimeError("prepared-fleet corpus omitted its scratch digest")
    if BASE.directory_digest(prepared_fleet) != prepared_fleet_sha256:
        raise RuntimeError("source prepared fleet changed during corpus creation")
    provenance = {
        "mode": "disposable-canonical-prepared-fleet-clone",
        "source_prepared_fleet_sha256": prepared_fleet_sha256,
        "scratch_before_sha256": working_before,
        "scratch_after_sha256": working_after,
        "scratch_mutated": working_after != working_before,
        "scratch_discarded": True,
        "scratch_restored_sha256": restored_sha256,
    }
    return report, provenance


def advance_to(
    state: CampaignState,
    *,
    target_height: int,
    node_bin: Path,
    batch_builder_bin: Path,
    seed: Path,
    wallet_key: Path,
    wallet_address: str,
    recipient: str,
    topology: Path,
) -> None:
    configuration = state.value["configuration"]
    chunk_size = int(configuration["advance_chunk_rounds"])
    canonical_root = state.root / "canonical"
    corpora_root = state.root / "corpora"
    while int(state.value["current_height"]) < target_height:
        current_height = int(state.value["current_height"])
        next_height = min(target_height, current_height + chunk_size)
        label = f"advance-{current_height}-to-{next_height}"
        unit_id = f"canonical/{label}"
        source_snapshot = optional_campaign_path(
            state.root,
            state.value.get("current_snapshot"),
            "current canonical snapshot",
        )
        corpus = corpora_root / f"{label}.json"
        prepared_fleet = (
            state.root / "prepared-fleets" / f"canonical-height-{next_height}"
        )
        current_prepared_raw = state.value.get("current_prepared_fleet")
        current_prepared = (
            safe_campaign_path(
                state.root,
                str(current_prepared_raw),
                "current advance prepared fleet",
            )
            if current_prepared_raw is not None
            else None
        )
        current_prepared_sha256 = state.value.get(
            "current_prepared_fleet_sha256"
        )
        state.begin_unit(
            unit_id,
            runner_root=canonical_root,
            label=label,
            owned_corpus=corpus,
            owned_prepared_fleet=prepared_fleet,
        )
        if current_prepared is None:
            if source_snapshot is None:
                raise RuntimeError("initial advance omitted its portable snapshot")
            BASE.create_signed_transfer_corpus(
                node_bin=node_bin,
                source_snapshot=source_snapshot,
                wallet_key=wallet_key,
                wallet_address=wallet_address,
                recipient=recipient,
                count=next_height - current_height,
                output_file=corpus,
                logs=canonical_root / "logs",
                label=label,
            )
        else:
            if not isinstance(current_prepared_sha256, str):
                raise RuntimeError("advance prepared fleet omitted its digest")
            _report, corpus_provenance = create_corpus_from_prepared_fleet(
                node_bin=node_bin,
                prepared_fleet=current_prepared,
                prepared_fleet_sha256=current_prepared_sha256,
                working_fleet=canonical_root / "nodes",
                wallet_key=wallet_key,
                wallet_address=wallet_address,
                recipient=recipient,
                count=next_height - current_height,
                expected_first_sequence=current_height,
                output_file=corpus,
                logs=canonical_root / "logs",
                label=label,
            )
        result, result_snapshot = BASE.run_persistent_advance(
            node_bin=node_bin,
            batch_builder_bin=batch_builder_bin,
            expected_builder_revision=str(state.value["runner_source_revision"]),
            root=canonical_root,
            seed=seed,
            topology=topology,
            source_snapshot=source_snapshot,
            signed_transfer_corpus=corpus,
            label=label,
            rounds=next_height - current_height,
            prepared_fleet=current_prepared,
            prepared_fleet_sha256=current_prepared_sha256,
            nodes_root=(
                canonical_root / "nodes" if current_prepared is not None else None
            ),
        )
        if (
            int(result["starting_height"]) != current_height
            or int(result["final_height"]) != next_height
            or result.get("node_preparation_mode")
            != (
                "byte-verified-prepared-fleet-clone"
                if current_prepared is not None
                else "authenticated-portable-snapshot-import"
            )
        ):
            raise RuntimeError("canonical advance ended at the wrong height")
        prepared_fleet_sha256 = BASE.directory_digest(canonical_root / "nodes")
        if result.get("result_prepared_fleet_sha256") != prepared_fleet_sha256:
            raise RuntimeError("canonical advance result fleet digest differs")
        if current_prepared is not None:
            result["corpus_generation"] = corpus_provenance
            BASE.write_json(
                canonical_root / "receipts" / f"{label}.json",
                result,
            )
        BASE.clone_prepared_fleet(
            canonical_root / "nodes",
            prepared_fleet,
            prepared_fleet_sha256,
        )
        record_result(
            state,
            unit_id=unit_id,
            runner_root=canonical_root,
            kind="advance",
            result=result,
            result_snapshot=result_snapshot,
            result_prepared_fleet=prepared_fleet,
        )
        state.value["current_height"] = next_height
        state.value["current_snapshot"] = (
            result_snapshot.relative_to(state.root).as_posix()
            if result_snapshot is not None
            else None
        )
        state.value["current_snapshot_sha256"] = (
            BASE.directory_digest(result_snapshot)
            if result_snapshot is not None
            else None
        )
        state.value["current_prepared_fleet"] = prepared_fleet.relative_to(
            state.root
        ).as_posix()
        state.value["current_prepared_fleet_sha256"] = prepared_fleet_sha256
        state.finish_unit()


def freeze_height_material(
    state: CampaignState,
    *,
    height: int,
    node_bin: Path,
    wallet_key: Path,
    wallet_address: str,
    recipient: str,
) -> None:
    key = str(height)
    if key in state.value["height_materials"]:
        return
    if int(state.value["current_height"]) != height:
        raise RuntimeError("cannot freeze material at a different current height")
    source_snapshot = optional_campaign_path(
        state.root, state.value.get("current_snapshot"), "height snapshot"
    )
    prepared_fleet = safe_campaign_path(
        state.root,
        str(state.value.get("current_prepared_fleet")),
        "height prepared fleet",
    )
    BASE.validate_prepared_fleet(prepared_fleet)
    prepared_fleet_sha256 = str(
        state.value.get("current_prepared_fleet_sha256", "")
    )
    if BASE.directory_digest(prepared_fleet) != prepared_fleet_sha256:
        raise RuntimeError("height prepared fleet changed before material freeze")
    corpus = state.root / "corpora" / f"height-{height}.json"
    unit_id = f"material/height-{height}"
    state.begin_unit(
        unit_id,
        runner_root=state.root / "canonical",
        label=f"height-{height}",
        owned_corpus=corpus,
    )
    legacy_height = any(
        entry.get("lane") == "legacy-jsonl" and entry.get("height") == height
        for entry in state.value["configuration"]["lane_height_matrix"]
    )
    if legacy_height:
        if source_snapshot is None:
            raise RuntimeError("legacy comparison height omitted its portable snapshot")
        report = BASE.create_signed_transfer_corpus(
            node_bin=node_bin,
            source_snapshot=source_snapshot,
            wallet_key=wallet_key,
            wallet_address=wallet_address,
            recipient=recipient,
            count=int(state.value["configuration"]["rounds_per_window"]),
            output_file=corpus,
            logs=state.root / "canonical" / "logs",
            label=f"height-{height}",
        )
        corpus_source_mode = "authenticated-portable-snapshot-import"
        corpus_source_prepared_fleet_sha256 = None
    else:
        report, corpus_provenance = create_corpus_from_prepared_fleet(
            node_bin=node_bin,
            prepared_fleet=prepared_fleet,
            prepared_fleet_sha256=prepared_fleet_sha256,
            working_fleet=state.root / "canonical" / "nodes",
            wallet_key=wallet_key,
            wallet_address=wallet_address,
            recipient=recipient,
            count=int(state.value["configuration"]["rounds_per_window"]),
            expected_first_sequence=height,
            output_file=corpus,
            logs=state.root / "canonical" / "logs",
            label=f"height-{height}",
        )
        corpus_source_mode = corpus_provenance["mode"]
        corpus_source_prepared_fleet_sha256 = prepared_fleet_sha256
    state.value["height_materials"][key] = {
        "height": height,
        "snapshot": (
            source_snapshot.relative_to(state.root).as_posix()
            if source_snapshot is not None
            else None
        ),
        "snapshot_sha256": (
            BASE.directory_digest(source_snapshot)
            if source_snapshot is not None
            else None
        ),
        "prepared_fleet": prepared_fleet.relative_to(state.root).as_posix(),
        "prepared_fleet_sha256": prepared_fleet_sha256,
        "corpus_source_mode": corpus_source_mode,
        "corpus_source_prepared_fleet_sha256": (
            corpus_source_prepared_fleet_sha256
        ),
        "corpus_scratch_before_sha256": (
            corpus_provenance["scratch_before_sha256"]
            if not legacy_height
            else None
        ),
        "corpus_scratch_after_sha256": (
            corpus_provenance["scratch_after_sha256"]
            if not legacy_height
            else None
        ),
        "corpus_scratch_mutated": (
            corpus_provenance["scratch_mutated"] if not legacy_height else None
        ),
        "corpus_scratch_discarded": (
            corpus_provenance["scratch_discarded"] if not legacy_height else None
        ),
        "corpus_scratch_restored_sha256": (
            corpus_provenance["scratch_restored_sha256"]
            if not legacy_height
            else None
        ),
        "signed_transfer_corpus": corpus.relative_to(state.root).as_posix(),
        "signed_transfer_corpus_sha256": sha256(corpus),
        "transfer_count": int(report["transfer_count"]),
        "first_sequence": int(report["first_sequence"]),
        "last_sequence": int(report["last_sequence"]),
    }
    state.finish_unit()


def run_windows(
    state: CampaignState,
    *,
    lane: str,
    height: int,
    node_bin: Path,
    seed: Path,
    wallet_key: Path,
    wallet_address: str,
    recipient: str,
    topology: Path,
) -> None:
    material = state.value["height_materials"][str(height)]
    source_snapshot = optional_campaign_path(
        state.root, material.get("snapshot"), "window source snapshot"
    )
    corpus = safe_campaign_path(
        state.root, material["signed_transfer_corpus"], "window signed corpus"
    )
    prepared_fleet = safe_campaign_path(
        state.root,
        material["prepared_fleet"],
        "window prepared fleet",
    )
    use_prepared_fleet = lane == BASE.SELECTED_STORAGE_LANE
    if not use_prepared_fleet and source_snapshot is None:
        raise RuntimeError("legacy window omitted its portable snapshot")
    lane_root = state.root / "lanes" / lane
    windows = int(state.value["configuration"]["windows_per_height"])
    rounds = int(state.value["configuration"]["rounds_per_window"])
    for window_index in range(1, windows + 1):
        label = f"height-{height}-window-{window_index}"
        unit_id = f"{lane}/{label}"
        if unit_id in state.value["completed_units"]:
            continue
        state.begin_unit(unit_id, runner_root=lane_root, label=label)
        result, result_snapshot = BASE.run_rounds(
            node_bin=node_bin,
            root=lane_root,
            seed=seed,
            topology=topology,
            source_snapshot=source_snapshot,
            wallet_key=wallet_key,
            wallet_address=wallet_address,
            recipient=recipient,
            signed_transfer_corpus=corpus,
            label=label,
            rounds=rounds,
            storage_lane=lane,
            prepared_fleet=prepared_fleet if use_prepared_fleet else None,
            prepared_fleet_sha256=(
                material["prepared_fleet_sha256"] if use_prepared_fleet else None
            ),
            nodes_root=(state.root / "canonical" / "nodes") if use_prepared_fleet else None,
        )
        if (
            result["source_snapshot_sha256"] != material.get("snapshot_sha256")
            or result["signed_transfer_corpus_sha256"]
            != material["signed_transfer_corpus_sha256"]
            or int(result["starting_height"]) != height
            or int(result["final_height"]) != height + rounds
            or result.get("node_preparation_mode")
            != (
                "byte-verified-prepared-fleet-clone"
                if use_prepared_fleet
                else "authenticated-portable-snapshot-import"
            )
            or result.get("prepared_fleet_sha256")
            != (material["prepared_fleet_sha256"] if use_prepared_fleet else None)
            or (
                result.get("result_snapshot_sha256") is not None
                if use_prepared_fleet
                else not is_sha256(result.get("result_snapshot_sha256"))
            )
            or (
                not is_sha256(result.get("result_prepared_fleet_sha256"))
                if use_prepared_fleet
                else result.get("result_prepared_fleet_sha256") is not None
            )
        ):
            raise RuntimeError(f"{unit_id} did not preserve the frozen input boundary")
        record_result(
            state,
            unit_id=unit_id,
            runner_root=lane_root,
            kind="window",
            result=result,
            result_snapshot=result_snapshot,
        )
        state.finish_unit()

    if (
        use_prepared_fleet
        and BASE.directory_digest(prepared_fleet)
        != material["prepared_fleet_sha256"]
    ):
        raise RuntimeError(
            f"{lane}/height-{height} frozen prepared fleet changed during windows"
        )


def build_report(
    state: CampaignState,
    *,
    node_bin: Path,
    development_smoke: bool,
) -> dict[str, Any]:
    configuration = state.value["configuration"]
    matrix = DEVELOPMENT_MATRIX if development_smoke else RELEASE_MATRIX
    builder_builds = {
        (
            str(result.get("batch_builder_build", {}).get("git_revision", "")),
            str(result.get("batch_builder_build", {}).get("profile", "")),
        )
        for record in state.value["completed_units"].values()
        if isinstance(record, dict)
        for result in [record.get("result")]
        if isinstance(result, dict)
        and result.get("advance_execution_mode")
        == "persistent-peer-certified-batch-loop"
    }
    if builder_builds != {
        (str(state.value["runner_source_revision"])[:8], "release")
    }:
        raise RuntimeError("persistent advances do not share one bound builder build")
    builder_revision, builder_profile = next(iter(builder_builds))
    lanes: dict[str, dict[str, Any]] = {}
    for lane in LANE_ORDER:
        lane_root = state.root / "lanes" / lane
        heights = [height for matrix_lane, height in matrix if matrix_lane == lane]
        rows: list[dict[str, Any]] = []
        for height in heights:
            windows = [
                copy.deepcopy(
                    state.value["completed_units"][
                        f"{lane}/height-{height}-window-{window_index}"
                    ]["result"]
                )
                for window_index in range(
                    1, int(configuration["windows_per_height"]) + 1
                )
            ]
            rows.append({"height": height, "windows": windows})
        aggregate_rows(rows, lane_root)
        relationship_models = (
            BASE.height_relationship_models(
                rows,
                lane_root,
                expected_heights=heights,
                rounds_per_window=int(configuration["rounds_per_window"]),
            )
            if not development_smoke and lane == BASE.SELECTED_STORAGE_LANE
            else {}
        )
        no_positive = (
            all(
                model["material_positive_linear_relationship"] is False
                for model in relationship_models.values()
            )
            if relationship_models
            else None
        )
        comparison_windows_pass = all(
            window["literal_receipts_exact"] is True
            and window["backend_work_gate_pass"] is True
            and int(window["validators_converged"]) == BASE.VALIDATORS
            and int(window["resources"]["foreground_process_count"])
            == int(configuration["rounds_per_window"])
            and int(window["resources"]["foreground_min_sample_count"]) >= 2
            for row in rows
            for window in row["windows"]
        )
        selected_gates = (
            all(
                window["zero_full_history_reads"] is True
                and window["bounded_index_pages"] is True
                and window["constant_accumulator_work"] is True
                for row in rows
                for window in row["windows"]
            )
            if lane == BASE.SELECTED_STORAGE_LANE
            else None
        )
        prefix_report_references(rows, lane_root, state.root)
        public = state.value["public_inputs"]
        lanes[lane] = {
            "lane": lane,
            "storage_behavior": STORAGE_BEHAVIORS[lane],
            "source_revision": state.value["source_revision"],
            "node_binary_sha256": sha256(node_bin),
            "node_binary": node_bin.name,
            "node_binary_build": state.value["node_binary_build"],
            "storage_backend_mode": BASE.STORAGE_BACKEND_MODES[lane],
            "storage_activation_height": BASE.STORAGE_ACTIVATION_HEIGHT,
            "chain_id": BASE.CHAIN_ID,
            "wallet_address": public["wallet_address"],
            "recipient_address": public["recipient_address"],
            "validator_public_identities": public["validator_public_identities"],
            "topology_sha256": public["topology_sha256"],
            "environment": {
                "cpu_affinity": sorted(os.sched_getaffinity(0)),
                "filesystem_device": lane_root.stat().st_dev,
                "filesystem_block_size_bytes": os.statvfs(lane_root).f_bsize,
            },
            "height_1_snapshot_sha256": public["height_1_snapshot_sha256"],
            "rows": rows,
            "comparison_windows_pass": comparison_windows_pass,
            "selected_storage_gates_pass": selected_gates,
            "height_relationship_model": {
                "schema": "postfiat-storage-height-cost-model-v2",
                "sample_kind": "per_window_p95",
                "relative_materiality": BASE.MODEL_RELATIVE_MATERIALITY,
                "residual_sigmas": BASE.MODEL_RESIDUAL_SIGMAS,
                "stages": relationship_models,
            },
            "no_positive_linear_height_relationship": no_positive,
        }

    selected = lanes[BASE.SELECTED_STORAGE_LANE]
    legacy = lanes["legacy-jsonl"]
    baseline: dict[str, float] = {}
    ratios: dict[str, float] = {}
    if not development_smoke:
        for metric in ("consensus_round_ms", "wallet_to_finality_ms"):
            baseline[metric] = metric_p95(legacy, 50, metric)
            selected_50 = metric_p95(selected, 50, metric)
            selected_5000 = metric_p95(selected, 5_000, metric)
            ratios[f"{metric}_height50_vs_legacy"] = (
                selected_50 / baseline[metric]
            )
            ratios[f"{metric}_height5000_vs_height50"] = (
                selected_5000 / selected_50
            )

    shared_height = 2 if development_smoke else 50
    exact_pairing = True
    for window_index in range(int(configuration["windows_per_height"])):
        compared = [
            lanes[lane]["rows"][0]["windows"][window_index] for lane in LANE_ORDER
        ]
        if (
            len({int(window["starting_height"]) for window in compared}) != 1
            or len({int(window["final_height"]) for window in compared}) != 1
            or len({str(window["final_state_root"]) for window in compared}) != 1
        ):
            exact_pairing = False
            break
        identities = {
            window_transaction_identities(
                state.root / "lanes" / lane,
                state.value["completed_units"][
                    f"{lane}/height-{shared_height}-window-{window_index + 1}"
                ]["result"],
            )
            for lane in LANE_ORDER
        }
        if len(identities) != 1:
            exact_pairing = False
            break

    comparison_windows_pass = all(
        lane["comparison_windows_pass"] is True for lane in lanes.values()
    )
    selected_window_gates = selected["selected_storage_gates_pass"] is True
    source_clean = BASE.git_is_clean()
    release_pass = (
        not development_smoke
        and source_clean
        and exact_pairing
        and comparison_windows_pass
        and selected_window_gates
        and selected["no_positive_linear_height_relationship"] is True
        and all(value <= 1.10 for value in ratios.values())
        and state.elapsed() <= int(configuration["max_wall_seconds"])
    )
    smoke_pass = (
        development_smoke
        and exact_pairing
        and comparison_windows_pass
        and selected_window_gates
    )
    status = (
        "PASS"
        if release_pass
        else "DEVELOPMENT SMOKE PASS"
        if smoke_pass
        else "PUBLIC TESTNET BLOCKED"
        if not development_smoke
        else "DEVELOPMENT SMOKE BLOCKED"
    )
    materials = [
        copy.deepcopy(state.value["height_materials"][str(height)])
        for height in sorted(
            {
                height
                for _, height in matrix
            }
        )
    ]
    report = {
        "schema": SCHEMA,
        "status": status,
        "captured_at": utc_now(),
        "campaign_mode": (
            "development-smoke"
            if development_smoke
            else "release-qualification"
        ),
        "qualification_profile": configuration["qualification_profile"],
        "evidence_eligible": release_pass,
        "source_worktree_clean": source_clean,
        "source_revision": state.value["source_revision"],
        "runner_source_revision": state.value["runner_source_revision"],
        "node_binary_sha256": sha256(node_bin),
        "node_binary": node_bin.name,
        "node_binary_build": state.value["node_binary_build"],
        "batch_builder_binary_sha256": state.value[
            "batch_builder_binary_sha256"
        ],
        "batch_builder_binary": state.value["batch_builder_binary"],
        "batch_builder_build": {
            "git_revision": builder_revision,
            "profile": builder_profile,
        },
        **state.value["runner_bindings"],
        "validator_count": BASE.VALIDATORS,
        "windows_per_height": int(configuration["windows_per_height"]),
        "rounds_per_window": int(configuration["rounds_per_window"]),
        "timeout_ms": BASE.QUALIFICATION_TIMEOUT_MS,
        "max_wall_seconds": int(configuration["max_wall_seconds"]),
        "elapsed_wall_seconds": state.elapsed(),
        "lane_order": list(LANE_ORDER),
        "lane_height_matrix": configuration["lane_height_matrix"],
        "lanes": lanes,
        "materials_by_height": materials,
        "legacy_height_50_baseline": baseline,
        "rows": selected["rows"],
        "ratios": ratios,
        "comparison_windows_pass": comparison_windows_pass,
        "window_gates_pass": selected_window_gates,
        "height_relationship_model": selected["height_relationship_model"],
        "no_positive_linear_height_relationship": selected[
            "no_positive_linear_height_relationship"
        ],
        "pairing": {
            "shared_comparison_height": shared_height,
            "same_host": True,
            "same_source_revision": True,
            "same_binary": True,
            "same_chain_id": True,
            "same_validator_count": True,
            "same_validator_keys": True,
            "same_topology_file": True,
            "same_authenticated_snapshot_at_shared_height": True,
            "same_signed_transactions_at_shared_height": True,
            "same_wallet_and_recipient_accounts": True,
            "same_window_cardinality_at_shared_height": True,
            "same_full_vote_policy": True,
            "same_timeout_policy": True,
            "same_host_allocation": len(
                {
                    tuple(lane["environment"]["cpu_affinity"])
                    for lane in lanes.values()
                }
            )
            == 1,
            "same_storage_medium": len(
                {
                    (
                        lane["environment"]["filesystem_device"],
                        lane["environment"]["filesystem_block_size_bytes"],
                    )
                    for lane in lanes.values()
                }
            )
            == 1,
            "same_final_state_for_identical_inputs": exact_pairing,
            "changed_input_at_shared_height": (
                "authenticated node-local storage backend mode only"
            ),
        },
        "host": state.value["host"],
        "claims_not_made": [
            *(
                ["release qualification or a height-50 legacy baseline"]
                if development_smoke
                else []
            ),
            "bounded-JSONL performance qualification",
            "legacy performance above height 50",
            "byte-identical independently generated validator signatures or certificate IDs",
            "public WAN or devnet performance",
            "deployment authorization",
        ],
        "offline": True,
        "network_contacted": False,
        "devnet_queried_or_mutated": False,
    }
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--node-bin", type=Path, required=True)
    parser.add_argument("--batch-builder-bin", type=Path, required=True)
    parser.add_argument("--expected-source-revision", required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument(
        "--resume",
        action="store_true",
        help="resume an interrupted checkpoint after verifying every frozen binding",
    )
    parser.add_argument(
        "--development-smoke",
        action="store_true",
        help="run one round per required lane at height 2; never evidence eligible",
    )
    parser.add_argument(
        "--development-stop-after-units",
        type=int,
        help="development-only controlled interruption after N completed units",
    )
    args = parser.parse_args()

    if args.development_stop_after_units is not None and (
        not args.development_smoke
        or args.development_stop_after_units <= 0
        or args.resume
    ):
        raise ValueError(
            "--development-stop-after-units requires a new development smoke"
        )

    require_revision(args.expected_source_revision, "source revision")
    current_revision = BASE.run_git_revision()
    if not args.development_smoke and not BASE.git_is_clean():
        raise ValueError("release qualification requires a clean checkout")

    raw_root = args.output_dir.expanduser()
    if raw_root.is_symlink():
        raise ValueError("output path must not be a symlink")
    root = raw_root.resolve()
    if args.resume:
        if not root.is_dir():
            raise ValueError(f"resume directory does not exist: {root}")
    else:
        if root.exists():
            raise ValueError(f"refusing to overwrite output directory: {root}")
        root.mkdir(parents=True)

    lock_path = root / ".campaign.lock"
    with lock_path.open("a+b") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise RuntimeError("another campaign process owns this output") from error

        node_bin = release_binary(args.node_bin, "qualification binary")
        batch_builder_bin = release_binary(
            args.batch_builder_bin, "corpus batch builder binary"
        )
        if batch_builder_bin.name != "postfiat-storage-corpus-batches":
            raise ValueError(
                "--batch-builder-bin must name postfiat-storage-corpus-batches"
            )
        configuration = campaign_configuration(args.development_smoke)
        checkpoint_path = root / "campaign-checkpoint.json"
        if args.resume:
            checkpoint = BASE.read_json(checkpoint_path)
        else:
            checkpoint = initialize_campaign(
                root,
                node_bin=node_bin,
                batch_builder_bin=batch_builder_bin,
                expected_source_revision=args.expected_source_revision,
                runner_source_revision=current_revision,
                configuration=configuration,
            )
        state = CampaignState(
            root,
            checkpoint,
            stop_after_units=args.development_stop_after_units,
        )
        if args.resume:
            validate_checkpoint(
                state,
                expected_source_revision=args.expected_source_revision,
                node_bin=node_bin,
                batch_builder_bin=batch_builder_bin,
                configuration=configuration,
            )
            quarantine_current_unit(state)
        else:
            state.write()

        seed, wallet_key, wallet_address, recipient, topology = campaign_paths(
            root, state.value
        )
        matrix = DEVELOPMENT_MATRIX if args.development_smoke else RELEASE_MATRIX
        try:
            for lane, height in matrix:
                if str(height) not in state.value["height_materials"]:
                    advance_to(
                        state,
                        target_height=height,
                        node_bin=node_bin,
                        batch_builder_bin=batch_builder_bin,
                        seed=seed,
                        wallet_key=wallet_key,
                        wallet_address=wallet_address,
                        recipient=recipient,
                        topology=topology,
                    )
                    freeze_height_material(
                        state,
                        height=height,
                        node_bin=node_bin,
                        wallet_key=wallet_key,
                        wallet_address=wallet_address,
                        recipient=recipient,
                    )
                run_windows(
                    state,
                    lane=lane,
                    height=height,
                    node_bin=node_bin,
                    seed=seed,
                    wallet_key=wallet_key,
                    wallet_address=wallet_address,
                    recipient=recipient,
                    topology=topology,
                )
            report = build_report(
                state,
                node_bin=node_bin,
                development_smoke=args.development_smoke,
            )
            report_path = root / "campaign-report.json"
            write_json(report_path, report)
            state.value["status"] = "COMPLETE"
            state.value["current_unit"] = None
            state.value["final_report_sha256"] = sha256(report_path)
            state.write()
        except BaseException as error:
            state.mark_interrupted(error)
            if isinstance(error, DevelopmentStop):
                print(
                    "storage-scaling-paired-campaign=DEVELOPMENT CHECKPOINT STOP",
                    flush=True,
                )
                print(f"checkpoint={checkpoint_path}", flush=True)
                return 75
            raise

    print(f"storage-scaling-paired-campaign={report['status']}", flush=True)
    print(f"report={root / 'campaign-report.json'}", flush=True)
    return 0 if report["status"] in {"PASS", "DEVELOPMENT SMOKE PASS"} else 1


if __name__ == "__main__":
    raise SystemExit(main())
