#!/usr/bin/env python3
"""Run one Corbanu exec identity-research session per frozen validator."""

from __future__ import annotations

import argparse
import concurrent.futures
import datetime as dt
import hashlib
import json
import os
import pathlib
import shutil
import signal
import subprocess
import threading
import time
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parent
REPO = ROOT.parents[2]
INDEX = ROOT / "inputs" / "index.json"
MODEL = "gpt-5.6-sol"
PRINT_LOCK = threading.Lock()


def canonical(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def utc_now() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat()


def load_jsonl(path: pathlib.Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def existing_run_is_current(row: dict[str, Any]) -> bool:
    run_path = ROOT / "runs" / row["network"] / f"{row['validator_id']}.json"
    if not run_path.exists():
        return False
    try:
        run = json.loads(run_path.read_text())
        files = run["files"]
        return bool(
            run["status"] == "PASS"
            and run["prompt_sha256"] == row["prompt_sha256"]
            and sha256(ROOT / files["packet"]) == run["packet_sha256"]
            and sha256(ROOT / files["exec_log"]) == run["exec_log_sha256"]
            and sha256(ROOT / files["stderr_log"]) == run["stderr_log_sha256"]
        )
    except (OSError, KeyError, ValueError, json.JSONDecodeError):
        return False


def run_one(
    corbanu: str,
    row: dict[str, Any],
    timeout_seconds: int,
    force: bool,
) -> dict[str, Any]:
    validator_id = row["validator_id"]
    network = row["network"]
    if not force and existing_run_is_current(row):
        return {"validator_id": validator_id, "network": network, "status": "SKIP"}

    prompt = ROOT / row["prompt_path"]
    packet = ROOT / "packets" / network / f"{validator_id}.md"
    exec_log = ROOT / "logs" / network / f"{validator_id}.jsonl"
    stderr_log = ROOT / "logs" / network / f"{validator_id}.stderr.log"
    run_path = ROOT / "runs" / network / f"{validator_id}.json"
    for path in (packet, exec_log, stderr_log, run_path):
        path.parent.mkdir(parents=True, exist_ok=True)

    command = [
        corbanu,
        "--search",
        "-a",
        "never",
        "exec",
        "-m",
        MODEL,
        "-c",
        "model_provider=openai",
        "--json",
        "--color",
        "never",
        "-s",
        "read-only",
        "-C",
        str(REPO),
        "-o",
        str(packet),
        "-",
    ]
    started_at = utc_now()
    started = time.monotonic()
    returncode = -1
    timed_out = False
    error = None
    with (
        prompt.open("rb") as stdin,
        exec_log.open("wb") as stdout,
        stderr_log.open("wb") as stderr,
    ):
        process = subprocess.Popen(
            command,
            cwd=REPO,
            stdin=stdin,
            stdout=stdout,
            stderr=stderr,
            start_new_session=True,
        )
        try:
            returncode = process.wait(timeout=timeout_seconds)
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(process.pid, signal.SIGTERM)
            try:
                returncode = process.wait(timeout=30)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                returncode = process.wait()

    rows: list[dict[str, Any]] = []
    thread_id = None
    usage = None
    if returncode == 0:
        try:
            rows = load_jsonl(exec_log)
            thread_id = next(
                row["thread_id"] for row in rows if row.get("type") == "thread.started"
            )
            usage = next(
                row["usage"] for row in rows if row.get("type") == "turn.completed"
            )
            if not packet.exists() or not packet.read_text().strip():
                error = "missing or empty final packet"
        except (OSError, KeyError, StopIteration, json.JSONDecodeError) as exc:
            error = f"invalid Corbanu JSONL: {type(exc).__name__}: {exc}"
    else:
        error = f"Corbanu exec exited {returncode}"
    if timed_out:
        error = f"Corbanu exec timed out after {timeout_seconds}s"

    status = "PASS" if returncode == 0 and error is None else "FAIL"
    receipt = {
        "validator_id": validator_id,
        "network": network,
        "status": status,
        "started_at": started_at,
        "finished_at": utc_now(),
        "duration_seconds": round(time.monotonic() - started, 3),
        "corbanu_version": "0.1.37",
        "configured_model": MODEL,
        "search_enabled": True,
        "sandbox": "read-only",
        "approval_policy": "never",
        "fallback_used": False,
        "timeout_seconds": timeout_seconds,
        "timed_out": timed_out,
        "returncode": returncode,
        "error": error,
        "thread_id": thread_id,
        "usage": usage,
        "prompt_sha256": sha256(prompt),
        "packet_sha256": sha256(packet) if packet.exists() else None,
        "exec_log_sha256": sha256(exec_log),
        "stderr_log_sha256": sha256(stderr_log),
        "files": {
            "prompt": str(prompt.relative_to(ROOT)),
            "packet": str(packet.relative_to(ROOT)),
            "exec_log": str(exec_log.relative_to(ROOT)),
            "stderr_log": str(stderr_log.relative_to(ROOT)),
        },
    }
    run_path.write_text(canonical(receipt) + "\n")
    with PRINT_LOCK:
        print(
            f"{status:4} {network:8} {validator_id} "
            f"{receipt['duration_seconds']:.1f}s"
            + (f" — {error}" if error else ""),
            flush=True,
        )
    return receipt


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workers", type=int, default=6)
    parser.add_argument("--timeout-seconds", type=int, default=1200)
    parser.add_argument("--network", choices=("xrpl", "postfiat"))
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()

    if args.workers < 1 or args.workers > 12:
        raise SystemExit("--workers must be between 1 and 12")
    corbanu = shutil.which("corbanu")
    if not corbanu:
        raise SystemExit("corbanu executable not found; Codex fallback was not invoked")

    rows = json.loads(INDEX.read_text())
    if args.network:
        rows = [row for row in rows if row["network"] == args.network]
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.workers) as executor:
        futures = [
            executor.submit(
                run_one, corbanu, row, args.timeout_seconds, args.force
            )
            for row in rows
        ]
        results = [future.result() for future in concurrent.futures.as_completed(futures)]

    failures = [row for row in results if row["status"] == "FAIL"]
    summary = {
        "requested": len(rows),
        "pass": sum(row["status"] == "PASS" for row in results),
        "skip": sum(row["status"] == "SKIP" for row in results),
        "fail": len(failures),
        "failed_validator_ids": [row["validator_id"] for row in failures],
    }
    print(canonical(summary), flush=True)
    if failures:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
