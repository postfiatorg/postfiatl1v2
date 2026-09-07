#!/usr/bin/env python3
"""Run one fixed-batch packet-input reputation pass against the local pinned SGLang server."""

from __future__ import annotations

import concurrent.futures
import hashlib
import json
import pathlib
import platform
import subprocess
import sys
import time
import urllib.request
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parent
URL = "http://127.0.0.1:8000/v1/chat/completions"


def canonical(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha(value: str) -> str:
    return hashlib.sha256(value.encode()).hexdigest()


def validate_result(request_row: dict[str, Any], content: str) -> dict[str, Any]:
    parsed = json.loads(content)
    if request_row["padding"]:
        if parsed != {"pad": True}:
            raise ValueError(f"slot {request_row['slot']}: bad padding response")
        return parsed

    score = parsed.get("score")
    if not isinstance(score, int) or isinstance(score, bool) or not 0 <= score <= 100:
        raise ValueError(f"slot {request_row['slot']}: score invalid")
    expected_band = f"B{min(score, 99) // 5 * 5:02d}"
    if parsed.get("band") != expected_band:
        raise ValueError(f"slot {request_row['slot']}: band mismatch")
    if parsed.get("recognized") is False and score != 0:
        raise ValueError(f"slot {request_row['slot']}: unrecognized entity did not score zero")
    return parsed


def one(request_row: dict[str, Any]) -> dict[str, Any]:
    body = canonical(request_row["body"]).encode()
    request = urllib.request.Request(
        URL, data=body, headers={"Content-Type": "application/json"}, method="POST"
    )
    with urllib.request.urlopen(request, timeout=1800) as response:
        document = json.load(response)
    choice = document["choices"][0]
    content = choice["message"]["content"] or ""
    parsed = validate_result(request_row, content)
    return {
        "slot": request_row["slot"],
        "validator_id": request_row["validator_id"],
        "network": request_row["network"],
        "packet_sha256": request_row["packet_sha256"],
        "padding": request_row["padding"],
        "request_sha256": request_row["request_sha256"],
        "content": content,
        "content_sha256": sha(content),
        "parsed": parsed,
        "finish_reason": choice.get("finish_reason"),
        "usage": document.get("usage"),
    }


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: python3 run_host.py <run-name>")
    run_name = sys.argv[1]
    requests = json.loads((ROOT / "inputs/requests.json").read_text())
    batches = json.loads((ROOT / "inputs/batch_schedule.json").read_text())
    manifest = json.loads((ROOT / "manifest.json").read_text())
    if hashlib.sha256((ROOT / "inputs/requests.json").read_bytes()).hexdigest() != manifest["requests_sha256"]:
        raise SystemExit("requests hash mismatch")
    if hashlib.sha256((ROOT / "inputs/prompt.txt").read_bytes()).hexdigest() != manifest["prompt_sha256"]:
        raise SystemExit("prompt hash mismatch")

    by_slot = {row["slot"]: row for row in requests}
    results = []
    started = time.time()
    for batch in batches:
        rows = [by_slot[slot] for slot in batch["slots"]]
        with concurrent.futures.ThreadPoolExecutor(max_workers=32) as executor:
            results.extend(executor.map(one, rows))
        print(f"batch {batch['batch']} done at {time.time() - started:.1f}s", flush=True)
    results.sort(key=lambda row: row["slot"])
    if len(results) != len(requests):
        raise SystemExit("incomplete run")

    gpu = subprocess.run(
        ["nvidia-smi", "--query-gpu=name,driver_version,uuid", "--format=csv,noheader"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    record = {
        "run_name": run_name,
        "started_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(started)),
        "wall_seconds": round(time.time() - started, 1),
        "gpu": gpu,
        "python": platform.python_version(),
        "requests_sha256": manifest["requests_sha256"],
        "prompt_sha256": manifest["prompt_sha256"],
        "identity_corpus_packet_set_sha256": manifest["identity_corpus"]["packet_set_sha256"],
        "results": results,
        "aggregate_sha256": sha("".join(row["content_sha256"] for row in results)),
    }
    (ROOT / "outputs").mkdir(exist_ok=True)
    (ROOT / "outputs" / f"{run_name}.json").write_text(canonical(record) + "\n")
    print(run_name, record["aggregate_sha256"], record["wall_seconds"])


if __name__ == "__main__":
    main()
