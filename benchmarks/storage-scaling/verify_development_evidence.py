#!/usr/bin/env python3
"""Verify the bounded-work development evidence packet."""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent
MANIFEST = ROOT / "SHA256SUMS.txt"
EXPECTED_FILES = {
    "README.md",
    "campaign-stop-f3907ad5.json",
    "e2-bounded-work.json",
    "verify_development_evidence.py",
}
HEIGHTS = [50, 100, 500, 1000, 5000]


def fail(message: str) -> None:
    raise SystemExit(f"storage-scaling-development-evidence-failed: {message}")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_manifest() -> dict[str, str]:
    entries: dict[str, str] = {}
    for line in MANIFEST.read_text(encoding="utf-8").splitlines():
        digest, separator, name = line.partition("  ")
        if not separator or not re.fullmatch(r"[0-9a-f]{64}", digest):
            fail(f"malformed checksum line: {line!r}")
        if name in entries:
            fail(f"duplicate checksum entry: {name}")
        entries[name] = digest
    if set(entries) != EXPECTED_FILES:
        fail(
            f"checksum files differ: expected {sorted(EXPECTED_FILES)}, "
            f"observed {sorted(entries)}"
        )
    for name, expected in entries.items():
        path = ROOT / name
        if not path.is_file():
            fail(f"missing file: {name}")
        observed = sha256(path)
        if observed != expected:
            fail(f"checksum mismatch for {name}: {observed} != {expected}")
    return entries


def require_heights(rows: list[dict[str, object]], label: str) -> None:
    observed = [row.get("height") for row in rows]
    if observed != HEIGHTS:
        fail(f"{label} heights differ: {observed}")


def main() -> None:
    load_manifest()
    stop = json.loads(
        (ROOT / "campaign-stop-f3907ad5.json").read_text(encoding="utf-8")
    )
    if stop.get("schema") != "postfiat-storage-scaling-campaign-stop-receipt-v1":
        fail("campaign stop receipt schema differs")
    if (
        stop.get("evidence_eligible") is not False
        or stop.get("final_campaign_report_present") is not False
        or stop.get("controller_processes_after_stop") != 0
        or stop.get("child_processes_after_stop") != 0
        or stop.get("offline") is not True
        or stop.get("network_contacted") is not False
        or stop.get("devnet_queried_or_mutated") is not False
    ):
        fail("campaign stop receipt overstates or omits its boundary")
    completed = stop.get("completed_windows")
    partial = stop.get("partial_window")
    if (
        not isinstance(completed, dict)
        or completed.get("count") != 32
        or not isinstance(partial, dict)
        or partial.get("completed_rounds") != 7
        or stop.get("measured_rounds_completed") != 1607
    ):
        fail("campaign stop receipt inventory differs")

    evidence = json.loads((ROOT / "e2-bounded-work.json").read_text(encoding="utf-8"))
    if evidence.get("schema") != "postfiat-storage-scaling-development-evidence-v1":
        fail("unsupported evidence schema")
    if evidence.get("status") != "PUBLIC_TESTNET_BLOCKED":
        fail("development evidence must retain the public-testnet block")
    if not re.fullmatch(r"[0-9a-f]{40}", str(evidence.get("source_commit", ""))):
        fail("source commit is not a full Git object id")

    jsonl_rows = evidence.get("bounded_jsonl", {}).get("rows", [])
    if not isinstance(jsonl_rows, list):
        fail("bounded JSONL rows are missing")
    require_heights(jsonl_rows, "bounded JSONL")
    for row in jsonl_rows:
        if row.get("legacy_prefix_bytes_read") != 0:
            fail(f"height {row.get('height')} JSONL read accepted-prefix bytes")
        if row.get("legacy_prefix_records_verified") != 0:
            fail(f"height {row.get('height')} JSONL verified accepted-prefix records")
        if row.get("crash_suffix_bytes_read") != 0:
            fail(f"height {row.get('height')} JSONL read an unexpected crash suffix")
        if row.get("checkpoint_bytes_read", 2048) > 1024:
            fail(f"height {row.get('height')} JSONL checkpoint exceeded one KiB")

    index = evidence.get("ordered_history_index", {})
    if index.get("fixed_limits", {}).get("probe_limit") != 64:
        fail("ordered-history probe limit differs")
    index_rows = index.get("rows", [])
    if not isinstance(index_rows, list):
        fail("ordered-history rows are missing")
    require_heights(index_rows, "ordered-history")
    for row in index_rows:
        if row.get("legacy_prefix_bytes_read") != 0:
            fail(f"height {row.get('height')} index read historical-prefix bytes")
        if row.get("legacy_prefix_records_verified") != 0:
            fail(f"height {row.get('height')} index verified historical-prefix records")
        if row.get("bitmap_bytes_read", 4 * 1024 * 1024) >= 4 * 1024 * 1024:
            fail(f"height {row.get('height')} index bitmap read exceeded fixed bound")
        if row.get("bitmap_bytes_written", 2 * 1024 * 1024) >= 2 * 1024 * 1024:
            fail(f"height {row.get('height')} index bitmap write exceeded fixed bound")
        if row.get("slots_read", 129) > 128:
            fail(f"height {row.get('height')} index probes exceeded two bounded operations")
        if row.get("slots_written") != 1:
            fail(f"height {row.get('height')} index wrote a non-unit slot count")

    not_proven = evidence.get("scope", {}).get("not_proven", [])
    required_limit = "six-validator finality p95 at any height"
    if required_limit not in not_proven:
        fail("six-validator finality limitation is missing")
    print("storage-scaling-development-evidence-ok")
    print(f"sha256sums_sha256={sha256(MANIFEST)}")


if __name__ == "__main__":
    main()
