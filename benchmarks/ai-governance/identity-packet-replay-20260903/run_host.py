#!/usr/bin/env python3
"""Run one fixed-batch packet-scoring pass against the local pinned SGLang server.

Before any request is sent, every scoring request's embedded packet text is
re-hashed and checked against the bound packet SHA-256 and the shipped packet
index, and the request/prompt/index files are checked against manifest.json.
Validation failures on individual responses are recorded, not fatal, so the
raw byte comparison across hosts is always available.
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
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parent
URL = "http://127.0.0.1:8000/v1/chat/completions"
BEGIN_MARK = "----- BEGIN FROZEN IDENTITY PACKET (exact bytes) -----\n"
END_MARK = "\n----- END FROZEN IDENTITY PACKET -----\n"
SERVER_INFO_KEYS = (
    "version",
    "model_path",
    "served_model_name",
    "revision",
    "enable_deterministic_inference",
    "disable_radix_cache",
    "random_seed",
    "attention_backend",
    "linear_attn_backend",
    "disable_cuda_graph",
    "disable_overlap_schedule",
    "chunked_prefill_size",
    "max_running_requests",
    "context_length",
    "reasoning_parser",
    "tp_size",
    "dtype",
    "quantization",
    "host",
    "port",
)


def canonical(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha(value: str | bytes) -> str:
    if isinstance(value, str):
        value = value.encode()
    return hashlib.sha256(value).hexdigest()


def embedded_packet(request_row: dict[str, Any]) -> str:
    content = request_row["body"]["messages"][1]["content"]
    start = content.index(BEGIN_MARK) + len(BEGIN_MARK)
    end = content.rindex(END_MARK)
    return content[start:end]


def verify_inputs(manifest: dict[str, Any], requests: list[dict[str, Any]]) -> None:
    for name, key in (
        ("inputs/requests.json", "requests_sha256"),
        ("inputs/prompt.txt", "prompt_sha256"),
        ("inputs/packet_index.json", "packet_index_sha256"),
        ("inputs/batch_schedule.json", "batch_schedule_sha256"),
    ):
        if sha((ROOT / name).read_bytes()) != manifest[key]:
            raise SystemExit(f"{name} hash mismatch against manifest")
    packet_index = {
        (row["network"], row["validator_id"]): row
        for row in json.loads((ROOT / "inputs/packet_index.json").read_text())
    }
    prompt = (ROOT / "inputs/prompt.txt").read_text()
    corpus_hash = manifest["identity_corpus"]["packet_set_sha256"]
    checked = 0
    for row in requests:
        if sha(canonical(row["body"])) != row["request_sha256"]:
            raise SystemExit(f"slot {row['slot']}: request hash mismatch")
        if row["padding"]:
            continue
        indexed = packet_index[(row["network"], row["validator_id"])]
        if indexed["packet_sha256"] != row["packet_sha256"]:
            raise SystemExit(f"slot {row['slot']}: bound packet hash not in packet index")
        if row["corpus_packet_set_sha256"] != corpus_hash:
            raise SystemExit(f"slot {row['slot']}: corpus packet-set hash mismatch")
        if row["body"]["messages"][0]["content"] != prompt:
            raise SystemExit(f"slot {row['slot']}: system prompt mismatch")
        packet = embedded_packet(row)
        if sha(packet) != row["packet_sha256"] or len(packet.encode()) != indexed["packet_bytes"]:
            raise SystemExit(f"slot {row['slot']}: embedded packet bytes do not match bound hash")
        user = row["body"]["messages"][1]["content"]
        for line in (
            f"packet_sha256: {row['packet_sha256']}\n",
            f"corpus_packet_set_sha256: {corpus_hash}\n",
            f"validator_id: {row['validator_id']}\n",
        ):
            if line not in user:
                raise SystemExit(f"slot {row['slot']}: binding line missing: {line.strip()}")
        checked += 1
    if checked != manifest["counts"]["scoring"]:
        raise SystemExit("scoring request count mismatch")
    print(f"verified {checked} packet-bound requests against manifest and packet index", flush=True)


def validate_result(request_row: dict[str, Any], content: str) -> tuple[Any, str | None]:
    try:
        parsed = json.loads(content)
    except json.JSONDecodeError as error:
        return None, f"invalid JSON: {error}"
    if request_row["padding"]:
        return parsed, None if parsed == {"pad": True} else "bad padding response"
    for field in ("validator_id", "network", "packet_sha256"):
        if parsed.get(field) != request_row[field]:
            return parsed, f"{field} mismatch"
    score = parsed.get("score")
    if not isinstance(score, int) or isinstance(score, bool) or not 0 <= score <= 100:
        return parsed, "score invalid"
    expected_band = f"B{min(score, 99) // 5 * 5:02d}"
    if parsed.get("band") != expected_band:
        return parsed, "band mismatch"
    if parsed.get("recognized") is False and score != 0:
        return parsed, "unrecognized institution did not score zero"
    if parsed.get("recognized") is False and parsed.get("institution") is not None:
        return parsed, "unrecognized institution carries a name"
    return parsed, None


def one(request_row: dict[str, Any]) -> dict[str, Any]:
    body = canonical(request_row["body"]).encode()
    request = urllib.request.Request(
        URL, data=body, headers={"Content-Type": "application/json"}, method="POST"
    )
    with urllib.request.urlopen(request, timeout=3600) as response:
        document = json.load(response)
    choice = document["choices"][0]
    content = choice["message"]["content"] or ""
    parsed, error = validate_result(request_row, content)
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
        "valid": error is None,
        "validation_error": error,
        "finish_reason": choice.get("finish_reason"),
        "usage": document.get("usage"),
    }


def server_info() -> dict[str, Any]:
    try:
        with urllib.request.urlopen("http://127.0.0.1:8000/get_server_info", timeout=60) as response:
            info = json.load(response)
    except Exception as error:  # noqa: BLE001 - evidence only
        return {"error": str(error)}
    return {key: info.get(key) for key in SERVER_INFO_KEYS if key in info}


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: python3 run_host.py <run-name>")
    run_name = sys.argv[1]
    manifest = json.loads((ROOT / "manifest.json").read_text())
    requests = json.loads((ROOT / "inputs/requests.json").read_text())
    batches = json.loads((ROOT / "inputs/batch_schedule.json").read_text())
    verify_inputs(manifest, requests)

    by_slot = {row["slot"]: row for row in requests}
    results = []
    started = time.time()
    for batch in batches:
        rows = [by_slot[slot] for slot in batch["slots"]]
        with concurrent.futures.ThreadPoolExecutor(max_workers=len(rows)) as executor:
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
        "server": server_info(),
        "requests_sha256": manifest["requests_sha256"],
        "prompt_sha256": manifest["prompt_sha256"],
        "packet_index_sha256": manifest["packet_index_sha256"],
        "corpus_packet_set_sha256": manifest["identity_corpus"]["packet_set_sha256"],
        "invalid_count": sum(not row["valid"] for row in results),
        "results": results,
        "aggregate_sha256": sha("".join(row["content_sha256"] for row in results)),
    }
    (ROOT / "outputs").mkdir(exist_ok=True)
    (ROOT / "outputs" / f"{run_name}.json").write_text(canonical(record) + "\n")
    print(run_name, record["aggregate_sha256"], record["wall_seconds"], "invalid", record["invalid_count"])


if __name__ == "__main__":
    main()
