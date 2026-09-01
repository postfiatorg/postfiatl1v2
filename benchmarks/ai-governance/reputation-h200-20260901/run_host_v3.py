#!/usr/bin/env python3
"""Executes one full fixed-batch run against the local SGLang server.

Usage: python3 run_host.py <run_name>
Reads ./inputs/{requests,batch_schedule,rubric.md,manifest-pin}, writes
./outputs/<run_name>.json. Batch discipline: all 32 requests of a batch are
in flight together; the next batch starts only when every response of the
current batch has completed. Comparison surface per prior art
(compare_sglang_cross_hardware_replay.py): raw choices[0].message.content
bytes, sha256 over UTF-8.
"""
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

ROOT = pathlib.Path(__file__).resolve().parent
URL = "http://127.0.0.1:8000/v1/chat/completions"


def sha_s(s: str) -> str:
    return hashlib.sha256(s.encode("utf-8")).hexdigest()


def one(req: dict) -> dict:
    body = json.dumps(req["body"], sort_keys=True, separators=(",", ":")).encode()
    r = urllib.request.urlopen(urllib.request.Request(
        URL, data=body, headers={"Content-Type": "application/json"}), timeout=1800)
    resp = json.loads(r.read())
    ch = resp["choices"][0]
    content = ch["message"]["content"] or ""
    reasoning = ch["message"].get("reasoning_content") or ""
    return {
        "slot": req["slot"], "lane": req["lane"], "validator_id": req["validator_id"],
        "padding": req["padding"], "request_sha256": req["request_sha256"],
        "content": content, "content_sha256": sha_s(content),
        "reasoning_sha256": sha_s(reasoning), "reasoning_tokens_proxy": len(reasoning),
        "finish_reason": ch.get("finish_reason"),
        "usage": resp.get("usage"),
    }


def main() -> None:
    run_name = sys.argv[1]
    requests = json.loads((ROOT / "inputs/requests.json").read_text())
    batches = json.loads((ROOT / "inputs/batch_schedule.json").read_text())
    manifest = json.loads((ROOT / "manifest.json").read_text())

    # rubric-compile preflight: rendered rubric bytes must match the manifest
    rub = (ROOT / "inputs/rubric.md").read_text()
    assert sha_s(rub) == manifest["rubric_sha256"], "rubric byte mismatch: abort before scoring"

    by_slot = {r["slot"]: r for r in requests}
    results = []
    t0 = time.time()
    for b in batches:
        batch_reqs = [by_slot[s] for s in b["slots"]]
        with concurrent.futures.ThreadPoolExecutor(max_workers=32) as ex:
            out = list(ex.map(one, batch_reqs))
        results.extend(out)
        print(f"batch {b['batch']} done t={time.time()-t0:.1f}s", flush=True)
    results.sort(key=lambda r: r["slot"])
    assert len(results) == 288

    gpu = subprocess.run(["nvidia-smi", "--query-gpu=name,driver_version,uuid", "--format=csv,noheader"],
                         capture_output=True, text=True).stdout.strip()
    record = {
        "run_name": run_name,
        "started_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(t0)),
        "wall_seconds": round(time.time() - t0, 1),
        "gpu": gpu, "python": platform.python_version(),
        "requests_sha256": manifest["requests_sha256"],
        "rubric_sha256": manifest["rubric_sha256"],
        "results": results,
        "aggregate_sha256": sha_s("".join(r["content_sha256"] for r in results)),
    }
    (ROOT / "outputs").mkdir(exist_ok=True)
    (ROOT / f"outputs/{run_name}.json").write_text(json.dumps(record, sort_keys=True, separators=(",", ":")))
    print(run_name, "aggregate", record["aggregate_sha256"][:16], "wall", record["wall_seconds"])


if __name__ == "__main__":
    main()
