#!/usr/bin/env python3
"""Compare four host runs byte-for-byte and publish canonical scores."""

from __future__ import annotations

import hashlib
import json
import pathlib
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parent
RUN_NAMES = ("primary-run1", "primary-run2", "replay-run1", "replay-run2")


def canonical(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha(value: str) -> str:
    return hashlib.sha256(value.encode()).hexdigest()


def main() -> None:
    runs = {
        name: json.loads((ROOT / "outputs" / f"{name}.json").read_text())
        for name in RUN_NAMES
    }
    baseline = runs["primary-run1"]
    by_run = {
        name: {row["slot"]: row for row in document["results"]}
        for name, document in runs.items()
    }
    comparisons = []
    scoring_identical = 0
    padding_identical = 0
    for slot, expected in sorted(by_run["primary-run1"].items()):
        for name in RUN_NAMES[1:]:
            actual = by_run[name].get(slot)
            identical = bool(
                actual
                and actual["request_sha256"] == expected["request_sha256"]
                and actual["content"].encode() == expected["content"].encode()
            )
            comparisons.append(
                {
                    "slot": slot,
                    "validator_id": expected["validator_id"],
                    "network": expected["network"],
                    "padding": expected["padding"],
                    "against": name,
                    "request_sha256": expected["request_sha256"],
                    "expected_content_sha256": expected["content_sha256"],
                    "actual_content_sha256": actual["content_sha256"] if actual else None,
                    "byte_identical": identical,
                }
            )
            if identical:
                if expected["padding"]:
                    padding_identical += 1
                else:
                    scoring_identical += 1
    failures = [row for row in comparisons if not row["byte_identical"]]
    scoring_count = sum(not row["padding"] for row in comparisons)
    padding_count = sum(row["padding"] for row in comparisons)
    report = {
        "verdict": "PASS" if not failures else "FAIL",
        "comparison_method": "raw UTF-8 choices[0].message.content bytes",
        "run_aggregate_sha256": {
            name: document["aggregate_sha256"] for name, document in runs.items()
        },
        "scoring_byte_identical_count": scoring_identical,
        "scoring_comparison_count": scoring_count,
        "padding_byte_identical_count": padding_identical,
        "padding_comparison_count": padding_count,
        "byte_identical_count": scoring_identical + padding_identical,
        "comparison_count": len(comparisons),
        "failure_count": len(failures),
        "comparisons": comparisons,
    }
    report["comparison_sha256"] = sha(canonical(report["comparisons"]))
    (ROOT / "outputs" / "comparison.json").write_text(canonical(report) + "\n")

    scores = []
    for row in baseline["results"]:
        if row["padding"]:
            continue
        scores.append(
            {
                "validator_id": row["validator_id"],
                "network": row["network"],
                "entity": row["entity"],
                **row["parsed"],
                "content_sha256": row["content_sha256"],
            }
        )
    score_document = {
        "source_run": "primary-run1",
        "source_aggregate_sha256": baseline["aggregate_sha256"],
        "scores": scores,
    }
    (ROOT / "outputs" / "scores.json").write_text(canonical(score_document) + "\n")
    summary = {key: report[key] for key in (
        "verdict", "byte_identical_count", "comparison_count",
        "scoring_byte_identical_count", "scoring_comparison_count",
        "failure_count", "comparison_sha256",
    )}
    print(canonical(summary))
    if failures:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
