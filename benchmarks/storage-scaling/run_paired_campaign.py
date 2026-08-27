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
SCHEMA = "postfiat-storage-scaling-time-budgeted-six-validator-campaign-v1"
CHECKPOINT_SCHEMA = "postfiat-storage-scaling-campaign-checkpoint-v1"
QUALIFICATION_PROFILE = "time-budgeted-redb-v1"
RELEASE_MATRIX = (
    ("selected-indexed", 50),
    ("selected-indexed", 5_000),
    ("legacy-jsonl", 50),
)
DEVELOPMENT_MATRIX = (
    ("selected-indexed", 2),
    ("legacy-jsonl", 2),
)
ADVANCE_CHUNK_ROUNDS = 5_000
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

    def finish_unit(self) -> None:
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
        self.value["status"] = (
            "TIME_BUDGET_EXCEEDED"
            if isinstance(error, TimeBudgetExceeded)
            else "INTERRUPTED"
            if isinstance(error, (KeyboardInterrupt, DevelopmentStop))
            else "FAILED"
        )
        self.value["last_stop"] = {
            "at": utc_now(),
            "type": type(error).__name__,
        }
        self.write()


def completed_unit_record(
    root: Path,
    runner_root: Path,
    *,
    kind: str,
    result: dict[str, Any],
    result_snapshot: Path,
) -> dict[str, Any]:
    receipt = runner_root / "receipts" / f"{result['label']}.json"
    if BASE.read_json(receipt) != result:
        raise RuntimeError("completed-unit receipt differs from in-memory result")
    return {
        "kind": kind,
        "runner_root": runner_root.relative_to(root).as_posix(),
        "result": result,
        "result_snapshot": result_snapshot.relative_to(root).as_posix(),
        "result_snapshot_sha256": BASE.directory_digest(result_snapshot),
        "receipt": receipt.relative_to(root).as_posix(),
        "receipt_sha256": sha256(receipt),
    }


def verify_completed_unit(root: Path, record: dict[str, Any]) -> None:
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
    snapshot = safe_campaign_path(
        root, str(record.get("result_snapshot", "")), "completed unit snapshot"
    )
    if BASE.directory_digest(snapshot) != record.get("result_snapshot_sha256"):
        raise ValueError("completed unit snapshot digest changed")
    for field in ("normalized_report", "resource_samples"):
        artifact = runner_root / str(result.get(field, ""))
        expected = result.get(f"{field}_sha256")
        if artifact.is_symlink() or not artifact.is_file() or sha256(artifact) != expected:
            raise ValueError(f"completed unit {field} changed")
    corpus = Path(str(result.get("signed_transfer_corpus", "")))
    if not corpus.is_absolute() or not corpus.resolve().is_relative_to(root):
        raise ValueError("completed unit corpus path is outside the campaign")
    if sha256(corpus) != result.get("signed_transfer_corpus_sha256"):
        raise ValueError("completed unit corpus changed")


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
            "quarantined_at": utc_now(),
            "moved_entries": moved,
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
    if checkpoint.get("node_binary_sha256") != sha256(node_bin):
        raise ValueError("campaign release binary changed")
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
    current_snapshot = safe_campaign_path(
        state.root,
        str(checkpoint.get("current_snapshot", "")),
        "campaign current snapshot",
    )
    if BASE.directory_digest(current_snapshot) != checkpoint.get(
        "current_snapshot_sha256"
    ):
        raise ValueError("campaign current snapshot changed")
    materials = checkpoint.get("height_materials")
    if not isinstance(materials, dict):
        raise ValueError("campaign checkpoint omitted height materials")
    for raw_height, material in materials.items():
        if not isinstance(material, dict) or str(int(raw_height)) != raw_height:
            raise ValueError("campaign height material is malformed")
        snapshot = safe_campaign_path(
            state.root, str(material.get("snapshot", "")), "height snapshot"
        )
        corpus = safe_campaign_path(
            state.root,
            str(material.get("signed_transfer_corpus", "")),
            "height signed corpus",
        )
        if BASE.directory_digest(snapshot) != material.get("snapshot_sha256"):
            raise ValueError(f"campaign height {raw_height} snapshot changed")
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
    completed = checkpoint.get("completed_units")
    if not isinstance(completed, dict):
        raise ValueError("campaign checkpoint omitted completed units")
    for record in completed.values():
        if not isinstance(record, dict):
            raise ValueError("campaign completed unit is malformed")
        verify_completed_unit(state.root, record)
    if float(checkpoint.get("elapsed_wall_seconds", 0.0)) >= int(
        configuration["max_wall_seconds"]
    ):
        raise TimeBudgetExceeded("campaign checkpoint has exhausted its time budget")


def initialize_campaign(
    root: Path,
    *,
    node_bin: Path,
    expected_source_revision: str,
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
        "node_binary_sha256": sha256(node_bin),
        "node_binary": node_bin.name,
        "node_binary_build": binary_build,
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
        "height_materials": {},
        "completed_units": {},
        "current_unit": None,
        "interrupted_units": [],
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
    result_snapshot: Path,
) -> None:
    state.value["completed_units"][unit_id] = completed_unit_record(
        state.root,
        runner_root,
        kind=kind,
        result=result,
        result_snapshot=result_snapshot,
    )


def advance_to(
    state: CampaignState,
    *,
    target_height: int,
    node_bin: Path,
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
        source_snapshot = safe_campaign_path(
            state.root,
            str(state.value["current_snapshot"]),
            "current canonical snapshot",
        )
        corpus = corpora_root / f"{label}.json"
        state.begin_unit(
            unit_id,
            runner_root=canonical_root,
            label=label,
            owned_corpus=corpus,
        )
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
        result, result_snapshot = BASE.run_rounds(
            node_bin=node_bin,
            root=canonical_root,
            seed=seed,
            topology=topology,
            source_snapshot=source_snapshot,
            wallet_key=wallet_key,
            wallet_address=wallet_address,
            recipient=recipient,
            signed_transfer_corpus=corpus,
            label=label,
            rounds=next_height - current_height,
            storage_lane=BASE.SELECTED_STORAGE_LANE,
        )
        if (
            int(result["starting_height"]) != current_height
            or int(result["final_height"]) != next_height
        ):
            raise RuntimeError("canonical advance ended at the wrong height")
        record_result(
            state,
            unit_id=unit_id,
            runner_root=canonical_root,
            kind="advance",
            result=result,
            result_snapshot=result_snapshot,
        )
        state.value["current_height"] = next_height
        state.value["current_snapshot"] = result_snapshot.relative_to(
            state.root
        ).as_posix()
        state.value["current_snapshot_sha256"] = BASE.directory_digest(
            result_snapshot
        )
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
    source_snapshot = safe_campaign_path(
        state.root, str(state.value["current_snapshot"]), "height snapshot"
    )
    prepared_fleet_source = state.root / "canonical" / "nodes"
    BASE.validate_prepared_fleet(prepared_fleet_source)
    prepared_fleet = state.root / "prepared-fleets" / f"height-{height}"
    corpus = state.root / "corpora" / f"height-{height}.json"
    unit_id = f"material/height-{height}"
    state.begin_unit(
        unit_id,
        owned_corpus=corpus,
        owned_prepared_fleet=prepared_fleet,
    )
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
    prepared_fleet_sha256 = BASE.directory_digest(prepared_fleet_source)
    BASE.clone_prepared_fleet(
        prepared_fleet_source,
        prepared_fleet,
        prepared_fleet_sha256,
    )
    state.value["height_materials"][key] = {
        "height": height,
        "snapshot": source_snapshot.relative_to(state.root).as_posix(),
        "snapshot_sha256": BASE.directory_digest(source_snapshot),
        "prepared_fleet": prepared_fleet.relative_to(state.root).as_posix(),
        "prepared_fleet_sha256": prepared_fleet_sha256,
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
    source_snapshot = safe_campaign_path(
        state.root, material["snapshot"], "window source snapshot"
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
            result["source_snapshot_sha256"] != material["snapshot_sha256"]
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


def build_report(
    state: CampaignState,
    *,
    node_bin: Path,
    development_smoke: bool,
) -> dict[str, Any]:
    configuration = state.value["configuration"]
    matrix = DEVELOPMENT_MATRIX if development_smoke else RELEASE_MATRIX
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
        "node_binary_sha256": sha256(node_bin),
        "node_binary": node_bin.name,
        "node_binary_build": state.value["node_binary_build"],
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
        "snapshots_by_height": materials,
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
    if args.expected_source_revision != current_revision:
        raise ValueError("source revision does not match HEAD")
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
        configuration = campaign_configuration(args.development_smoke)
        checkpoint_path = root / "campaign-checkpoint.json"
        if args.resume:
            checkpoint = BASE.read_json(checkpoint_path)
        else:
            checkpoint = initialize_campaign(
                root,
                node_bin=node_bin,
                expected_source_revision=args.expected_source_revision,
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
