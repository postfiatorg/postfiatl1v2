#!/usr/bin/env python3
"""Verify the Cobalt decisive-run Section 2 packet."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path, PurePosixPath
from typing import Any

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
EXPECTED_FILES = {
    "cobalt-report.json",
    "rippled-report.json",
    "section2-summary.json",
    "source-manifest.json",
}
TASK_ID = "task_690f0c63d1c0d175a4e47d947825402b"
MANIFEST_ID = "78fc3f92d460f45a4941d40ef705af6c761e3782155a5b599dbd78c90396bde3"
RAW_MANIFEST_SHA256 = "3df59da71f0f52553bfa1d4919a50a180a4ec2aaf88a250bfb320c438932a14d"
MAX_BYTES = 2 * 1024 * 1024


def digest(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def read_bytes(path: Path) -> bytes:
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"missing or non-regular packet file: {path.name}")
    payload = path.read_bytes()
    if len(payload) > MAX_BYTES:
        raise ValueError(f"oversized packet file: {path.name}")
    return payload


def read_json(path: Path) -> dict[str, Any]:
    value = json.loads(read_bytes(path))
    if not isinstance(value, dict):
        raise ValueError(f"packet JSON is not an object: {path.name}")
    return value


def verify_checksums(packet: Path) -> bool:
    lines = read_bytes(packet / "SHA256SUMS.txt").decode("ascii").splitlines()
    seen: set[str] = set()
    for line in lines:
        expected, separator, name = line.partition("  ")
        candidate = PurePosixPath(name)
        if (
            separator != "  "
            or len(expected) != 64
            or any(char not in "0123456789abcdef" for char in expected)
            or candidate.is_absolute()
            or len(candidate.parts) != 1
            or name in seen
        ):
            return False
        seen.add(name)
        if digest(read_bytes(packet / name)) != expected:
            return False
    return seen == EXPECTED_FILES


def git_blob(commit: str, path: str) -> bytes:
    completed = subprocess.run(
        ["git", "show", f"{commit}:{path}"],
        cwd=ROOT,
        check=False,
        capture_output=True,
    )
    if completed.returncode != 0:
        raise ValueError(f"cannot read {path} at source commit {commit}")
    return completed.stdout


def verify(packet: Path) -> dict[str, Any]:
    packet = packet.resolve()
    checksum_ok = verify_checksums(packet)
    summary = read_json(packet / "section2-summary.json")
    source = read_json(packet / "source-manifest.json")
    cobalt = read_json(packet / "cobalt-report.json")
    rippled = read_json(packet / "rippled-report.json")

    source_commit = source.get("source_commit")
    source_files = source.get("files", {})
    source_hashes_ok = (
        isinstance(source_commit, str)
        and len(source_commit) == 40
        and isinstance(source_files, dict)
        and bool(source_files)
    )
    if source_hashes_ok:
        try:
            source_hashes_ok = all(
                digest(git_blob(source_commit, path)) == expected
                for path, expected in source_files.items()
            )
        except ValueError:
            source_hashes_ok = False

    cobalt_results = cobalt.get("results", [])
    rippled_results = rippled.get("results", [])
    all_candidate_runs = [
        run for row in cobalt_results for run in row.get("candidate_runs", [])
    ]
    material = [
        row.get("case_id")
        for row in rippled_results
        if row.get("material_safety_delta") is True
    ]
    divergent_cobalt = next(
        (row for row in cobalt_results if row.get("case_id") == "six-divergent-local-quorums"),
        {},
    )
    divergent_rippled = next(
        (row for row in rippled_results if row.get("case_id") == "six-divergent-local-quorums"),
        {},
    )
    overlap = [
        row for row in cobalt_results if row.get("case_id", "").startswith("twenty-overlap-090")
    ]

    checks = {
        "checksums": checksum_ok,
        "task": source.get("task_id") == TASK_ID and summary.get("task_id") == TASK_ID,
        "source_commit": source_hashes_ok and summary.get("source_commit") == source_commit,
        "manifest": (
            source.get("frozen_manifest", {}).get("canonical_id") == MANIFEST_ID
            and source.get("frozen_manifest", {}).get("raw_sha256") == RAW_MANIFEST_SHA256
            and cobalt.get("scenario_manifest_sha256") == MANIFEST_ID
        ),
        "cobalt": (
            cobalt.get("case_count") == 18
            and cobalt.get("passed_case_count") == 18
            and cobalt.get("conflicting_root_count") == 0
            and len(cobalt_results) == 18
            and all(row.get("expectation_passed") is True for row in cobalt_results)
        ),
        "replay": bool(all_candidate_runs)
        and all(run.get("replay_equal") is True for run in all_candidate_runs),
        "rippled": (
            rippled.get("case_count") == 18
            and rippled.get("passed_case_count") == 18
            and rippled.get("conflicting_root_count") == 1
            and len(rippled_results) == 18
            and all(row.get("expectation_passed") is True for row in rippled_results)
        ),
        "material_delta": (
            material == ["six-divergent-local-quorums"]
            and divergent_cobalt.get("graph_safe") is False
            and divergent_cobalt.get("conflicting_roots") == 0
            and divergent_rippled.get("validator_governance", {}).get("conflicting_roots") == 1
        ),
        "ninety_percent_overlap": len(overlap) == 3
        and all(row.get("expectation_passed") is True for row in overlap),
        "summary": summary.get("schema") == "postfiat-cobalt-section2-summary-v1"
        and summary.get("status") == "PASS"
        and isinstance(summary.get("checks"), dict)
        and all(summary["checks"].values()),
    }
    return {
        "schema": "postfiat-cobalt-section2-verification-v1",
        "status": "PASS" if all(checks.values()) else "FAIL",
        "checks": checks,
        "packet_sha256sums_sha256": digest(
            read_bytes(packet / "SHA256SUMS.txt")
        ),
        "source_commit": source_commit,
        "task_id": TASK_ID,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--packet",
        type=Path,
        default=HERE / "section2-packet",
    )
    args = parser.parse_args()
    result = verify(args.packet)
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if result["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
