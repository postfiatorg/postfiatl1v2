#!/usr/bin/env python3
"""Measure and verify the high-height prepared-fleet restore path.

This is an offline evidence-harness preflight. It does not start a node,
execute consensus, contact a network, or mutate the frozen source fleet.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import shutil
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

REPO = Path(__file__).resolve().parents[2]
BASE_RUNNER = REPO / "benchmarks" / "storage-scaling" / "run_campaign.py"
SCHEMA = "postfiat-storage-prepared-fleet-restore-preflight-v1"


def load_base_runner() -> Any:
    spec = importlib.util.spec_from_file_location(
        "postfiat_storage_preflight_base",
        BASE_RUNNER,
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load storage runner: {BASE_RUNNER}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


BASE = load_base_runner()


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def utc_now() -> str:
    return (
        datetime.now(timezone.utc)
        .isoformat(timespec="seconds")
        .replace("+00:00", "Z")
    )


def git_revision() -> str:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=REPO,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    )
    revision = completed.stdout.strip()
    if len(revision) != 40:
        raise RuntimeError("git revision is not a full object ID")
    return revision


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--expected-sha256", required=True)
    parser.add_argument("--workspace", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()

    source = args.source.expanduser().resolve()
    workspace = args.workspace.expanduser().resolve()
    report_path = args.report.expanduser().resolve()
    if workspace.exists() or workspace.is_symlink():
        raise ValueError("preflight workspace must not already exist")
    if report_path.exists() or report_path.is_symlink():
        raise ValueError("preflight report must not already exist")
    if report_path.is_relative_to(workspace):
        raise ValueError("preflight report must be outside its disposable workspace")
    BASE.validate_prepared_fleet(source)

    source_files = [path for path in source.rglob("*") if path.is_file()]
    source_bytes = sum(path.stat().st_size for path in source_files)
    database_relatives = [
        Path(f"validator-{index}")
        / "transactional-snapshot-generation-v1"
        / "postfiat-state-v1.redb"
        for index in range(BASE.VALIDATORS)
    ]
    database_bytes = 0
    initial_elapsed = 0.0
    reset_elapsed = 0.0
    cleanup_elapsed = 0.0
    source_after = ""
    try:
        started = time.monotonic()
        initial_digest = BASE.clone_prepared_fleet(
            source,
            workspace,
            args.expected_sha256,
        )
        initial_elapsed = time.monotonic() - started

        for relative in database_relatives:
            database = workspace / relative
            if not database.is_file() or database.is_symlink():
                raise RuntimeError(f"prepared fleet omitted database {relative}")
            database_bytes += database.stat().st_size
            metadata = database.stat()
            os.utime(
                database,
                ns=(metadata.st_atime_ns, metadata.st_mtime_ns + 1),
            )

        started = time.monotonic()
        reset_digest = BASE.clone_prepared_fleet(
            source,
            workspace,
            args.expected_sha256,
        )
        reset_elapsed = time.monotonic() - started
        source_after = BASE.directory_digest(source)
        if (
            initial_digest != args.expected_sha256
            or reset_digest != args.expected_sha256
            or source_after != args.expected_sha256
        ):
            raise RuntimeError("prepared fleet preflight digest binding failed")
    finally:
        if workspace.exists():
            started = time.monotonic()
            shutil.rmtree(workspace)
            cleanup_elapsed = time.monotonic() - started
    if workspace.exists():
        raise RuntimeError("prepared fleet preflight workspace was not removed")

    report = {
        "schema": SCHEMA,
        "status": "PASS",
        "captured_at": utc_now(),
        "runner_revision": git_revision(),
        "runner_sha256": sha256(BASE_RUNNER),
        "preflight_sha256": sha256(Path(__file__).resolve()),
        "prepared_fleet_sha256": args.expected_sha256,
        "source_after_sha256": source_after,
        "source_file_count": len(source_files),
        "source_bytes": source_bytes,
        "forced_reset_database_count": len(database_relatives),
        "forced_reset_database_bytes": database_bytes,
        "initial_clone_seconds": initial_elapsed,
        "forced_incremental_reset_seconds": reset_elapsed,
        "cleanup_seconds": cleanup_elapsed,
        "workspace_removed": True,
        "offline": True,
        "network_contacted": False,
        "node_process_started": False,
        "consensus_executed": False,
    }
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    report_path.chmod(0o644)
    print("storage-prepared-fleet-preflight=PASS", flush=True)
    print(f"report={report_path}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
