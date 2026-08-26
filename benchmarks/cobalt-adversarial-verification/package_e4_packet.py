#!/usr/bin/env python3
"""Package a passing raw E4 run into a relocatable checksum-bound packet."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


REPO = Path(__file__).resolve().parents[2]
DEFAULT_PACKET = REPO / "benchmarks/cobalt-adversarial-verification/e4"
TOKEN = "$E4_RUN_ROOT"
OUTPUT_NAMES = {
    "baseline-report.json": "output/baseline/report.json",
    "attack-report.json": "output/attack/report.json",
    "consensus-v2-cobalt-integration.json": (
        "output/consensus-v2-cobalt-integration.json"
    ),
    "topology.json": "output/topology.json",
}


def digest_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def digest(path: Path) -> str:
    return digest_bytes(path.read_bytes())


def object_bytes(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def replace_strings(value: Any, needle: str) -> tuple[Any, int]:
    if isinstance(value, str):
        return value.replace(needle, TOKEN), value.count(needle)
    if isinstance(value, list):
        output: list[Any] = []
        count = 0
        for row in value:
            normalized, replacements = replace_strings(row, needle)
            output.append(normalized)
            count += replacements
        return output, count
    if isinstance(value, dict):
        output: dict[str, Any] = {}
        count = 0
        for key, row in value.items():
            normalized, replacements = replace_strings(row, needle)
            output[key] = normalized
            count += replacements
        return output, count
    return value, 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--run-root", type=Path, required=True)
    parser.add_argument("--packet-dir", type=Path, default=DEFAULT_PACKET)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    run_root = args.run_root.resolve()
    packet = args.packet_dir.resolve()
    if run_root == Path("/") or len(run_root.parts) < 4:
        raise ValueError("--run-root must be one explicit disposable campaign directory")
    if not packet.is_dir():
        raise ValueError(f"packet directory does not exist: {packet}")

    raw: dict[str, bytes] = {}
    normalized: dict[str, Any] = {}
    replacement_counts: dict[str, int] = {}
    for name, relative in OUTPUT_NAMES.items():
        source = run_root / relative
        raw[name] = source.read_bytes()
        value = json.loads(raw[name])
        value, replacement_counts[name] = replace_strings(value, str(run_root))
        normalized[name] = value

    baseline = normalized["baseline-report.json"]
    attack = normalized["attack-report.json"]
    report = normalized["consensus-v2-cobalt-integration.json"]
    if baseline["status"] != "passed" or attack["status"] != "passed":
        raise ValueError("both E4 lanes must pass before packaging")
    if report["status"] != "passed":
        raise ValueError("the integrated E4 report must pass before packaging")

    clean = packet / "clean-rerun"
    clean.mkdir(parents=True, exist_ok=True)
    baseline_bytes = object_bytes(baseline)
    attack_bytes = object_bytes(attack)
    topology_bytes = object_bytes(normalized["topology.json"])
    report["evidence"]["baseline_report_sha256"] = digest_bytes(baseline_bytes)
    report["evidence"]["attack_report_sha256"] = digest_bytes(attack_bytes)
    report["evidence"]["topology_sha256"] = digest_bytes(topology_bytes)
    normalized_bytes = {
        "baseline-report.json": baseline_bytes,
        "attack-report.json": attack_bytes,
        "topology.json": topology_bytes,
        "consensus-v2-cobalt-integration.json": object_bytes(report),
    }
    for name, value in normalized_bytes.items():
        (clean / name).write_bytes(value)

    receipt = {
        "schema": "postfiat-cobalt-adversarial-e4-path-normalization-v1",
        "status": "passed",
        "replacement_token": TOKEN,
        "raw_run_root_sha256": digest_bytes(str(run_root).encode()),
        "benchmark_semantics_changed": False,
        "evidence_digests_rebound_to_normalized_files": True,
        "reports": {
            name: {
                "raw_sha256": digest_bytes(raw[name]),
                "normalized_sha256": digest_bytes(normalized_bytes[name]),
                "replacement_count": replacement_counts[name],
            }
            for name in sorted(OUTPUT_NAMES)
        },
    }
    (packet / "normalization-receipt.json").write_bytes(object_bytes(receipt))

    files = sorted(
        path.relative_to(packet).as_posix()
        for path in packet.rglob("*")
        if (
            path.is_file()
            and path.name != "SHA256SUMS.txt"
            and path.suffix != ".pyc"
            and "__pycache__" not in path.parts
        )
    )
    checksum_lines = [f"{digest(packet / name)}  {name}" for name in files]
    (packet / "SHA256SUMS.txt").write_text(
        "\n".join(checksum_lines) + "\n", encoding="ascii"
    )
    print(f"e4-packet-packaged={packet}")
    print(f"files={len(files)}")
    print(f"sha256sums_sha256={digest(packet / 'SHA256SUMS.txt')}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
