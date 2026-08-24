#!/usr/bin/env python3
"""Build the verifier-backed Cobalt decisive-run Section 2 packet."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import statistics
import subprocess
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
TASK_ID = "task_690f0c63d1c0d175a4e47d947825402b"
MANIFEST_ID = "78fc3f92d460f45a4941d40ef705af6c761e3782155a5b599dbd78c90396bde3"
RAW_MANIFEST_SHA256 = "3df59da71f0f52553bfa1d4919a50a180a4ec2aaf88a250bfb320c438932a14d"
SOURCE_FILES = [
    "benchmarks/cobalt-activate-or-retire/oracle-contract.md",
    "benchmarks/cobalt-activate-or-retire/scenario-manifest.json",
    "benchmarks/cobalt-activate-or-retire/rippled/DecisiveGovernanceBenchmark_test.cpp",
    "crates/cobalt_decision_oracle/src/lib.rs",
    "crates/consensus_cobalt/src/dabc_registry.rs",
    "crates/consensus_cobalt/src/internal_validation.rs",
    "crates/consensus_cobalt/src/tests.rs",
    "crates/node/src/bin/postfiat_cobalt_decisive_benchmark.rs",
]


def digest(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def read_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_bytes())
    if not isinstance(value, dict):
        raise ValueError(f"expected JSON object: {path}")
    return value


def write_json(path: Path, value: Any) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def git_output(*args: str) -> str:
    completed = subprocess.run(
        ["git", *args],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout.strip()


def node_mismatch_count(result: dict[str, Any]) -> int:
    actual = result["actual_nodes"]
    expected = result["expected_nodes"]
    mismatches = 0
    for validator, expected_node in expected.items():
        actual_node = actual.get(validator, {})
        if actual_node.get("outcome") != expected_node.get("outcome"):
            mismatches += 1
            continue
        if (
            expected_node.get("outcome") == "decide"
            and actual_node.get("registry_root") != expected_node.get("registry_root")
        ):
            mismatches += 1
    return mismatches


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cobalt-report", type=Path, required=True)
    parser.add_argument("--rippled-report", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    output = args.output.resolve()
    if output.exists():
        raise ValueError(f"refusing to overwrite packet: {output}")
    output.mkdir(parents=True)

    manifest = read_json(HERE / "scenario-manifest.json")
    cobalt = read_json(args.cobalt_report)
    rippled = read_json(args.rippled_report)

    if manifest.get("manifest_sha256") != MANIFEST_ID:
        raise ValueError("frozen manifest ID changed")
    if digest((HERE / "scenario-manifest.json").read_bytes()) != RAW_MANIFEST_SHA256:
        raise ValueError("frozen manifest bytes changed")

    cobalt_results = cobalt.get("results", [])
    rippled_results = rippled.get("results", [])
    if len(cobalt_results) != 18 or len(rippled_results) != 18:
        raise ValueError("both adapters must report all 18 frozen cases")

    cobalt_ids = [row["case_id"] for row in cobalt_results]
    rippled_ids = [row["case_id"] for row in rippled_results]
    manifest_ids = [row["id"] for row in manifest["cases"]]
    if cobalt_ids != manifest_ids or rippled_ids != manifest_ids:
        raise ValueError("adapter case order differs from frozen manifest")

    mismatch_count = sum(node_mismatch_count(row) for row in cobalt_results)
    candidate_runs = [
        run for row in cobalt_results for run in row.get("candidate_runs", [])
    ]
    elapsed = [run["elapsed_micros"] for run in candidate_runs]
    wire_bytes = [run["wire_bytes"] for run in candidate_runs]

    material_delta = [
        row["case_id"] for row in rippled_results if row.get("material_safety_delta")
    ]
    overlap_rows = [
        row for row in cobalt_results if row["case_id"].startswith("twenty-overlap-090")
    ]
    divergent_cobalt = next(
        row for row in cobalt_results if row["case_id"] == "six-divergent-local-quorums"
    )
    divergent_rippled = next(
        row for row in rippled_results if row["case_id"] == "six-divergent-local-quorums"
    )

    checks = {
        "cobalt_schema": cobalt.get("schema") == "postfiat-cobalt-decisive-benchmark-report-v1",
        "cobalt_oracle_not_called": cobalt.get("oracle_called") is False,
        "cobalt_all_cases_pass": cobalt.get("passed_case_count") == 18,
        "cobalt_zero_conflicts": cobalt.get("conflicting_root_count") == 0,
        "cobalt_zero_node_mismatches": mismatch_count == 0,
        "cobalt_replay_deterministic": bool(candidate_runs)
        and all(run.get("replay_equal") is True for run in candidate_runs),
        "cobalt_duplicate_rejection": bool(candidate_runs)
        and all(run.get("duplicate_rejected") is True for run in candidate_runs),
        "cobalt_shadow_authority_only": bool(candidate_runs)
        and all(run.get("authority_disabled") is True for run in candidate_runs),
        "compatible_nonuniform_decides": any(
            row["fault_class"] == "compatible_nonuniform"
            and all(node["outcome"] == "decide" for node in row["actual_nodes"].values())
            for row in cobalt_results
        ),
        "incompatible_cobalt_halts": any(
            row["classification"] == "incompatible"
            and all(node["outcome"] == "halt" for node in row["actual_nodes"].values())
            for row in cobalt_results
        ),
        "ninety_percent_cases_match_oracle": len(overlap_rows) == 3
        and all(row.get("expectation_passed") is True for row in overlap_rows),
        "rippled_schema": rippled.get("schema") == "postfiat-rippled-decisive-benchmark-report-v1",
        "rippled_oracle_not_called": rippled.get("oracle_called") is False,
        "rippled_all_cases_pass": rippled.get("passed_case_count") == 18,
        "rippled_one_expected_conflict": rippled.get("conflicting_root_count") == 1,
        "material_safety_delta_is_frozen_case": material_delta
        == ["six-divergent-local-quorums"],
        "cobalt_halts_divergent_graph_without_conflict": (
            divergent_cobalt.get("graph_safe") is False
            and divergent_cobalt.get("conflicting_roots") == 0
            and all(
                node["outcome"] == "halt"
                for node in divergent_cobalt["actual_nodes"].values()
            )
        ),
        "rippled_local_quorums_diverge": (
            divergent_rippled["validator_governance"]["conflicting_roots"] == 1
        ),
        "rippled_native_csf_control_stays_separate": all(
            row["native_ledger_consensus"]["decision_scope"]
            == "ledger consensus control, not validator-governance admission"
            and row["native_ledger_consensus"]["synchronized"] is True
            and row["native_ledger_consensus"]["branches"] == 1
            for row in rippled_results
        ),
    }
    if not all(checks.values()):
        failed = [name for name, passed in checks.items() if not passed]
        raise ValueError(f"Section 2 checks failed: {failed}")

    source_commit = git_output("rev-parse", "HEAD")
    source_manifest = {
        "schema": "postfiat-cobalt-section2-source-manifest-v1",
        "source_commit": source_commit,
        "task_id": TASK_ID,
        "rippled": {
            "version": manifest["source_pins"]["rippled_version"],
            "commit": manifest["source_pins"]["rippled_commit"],
        },
        "frozen_manifest": {
            "canonical_id": MANIFEST_ID,
            "raw_sha256": RAW_MANIFEST_SHA256,
        },
        "files": {
            path: digest((ROOT / path).read_bytes()) for path in SOURCE_FILES
        },
    }
    summary = {
        "schema": "postfiat-cobalt-section2-summary-v1",
        "status": "PASS",
        "task_id": TASK_ID,
        "source_commit": source_commit,
        "comparison_scope": manifest["source_pins"]["comparison_scope"],
        "frozen_manifest": source_manifest["frozen_manifest"],
        "checks": checks,
        "cobalt": {
            "cases": 18,
            "passed": cobalt["passed_case_count"],
            "conflicting_roots": cobalt["conflicting_root_count"],
            "node_mismatches": mismatch_count,
            "candidate_runs": len(candidate_runs),
            "deterministic_replays": sum(
                run.get("replay_equal") is True for run in candidate_runs
            ),
            "elapsed_micros": {
                "min": min(elapsed),
                "median": int(statistics.median(elapsed)),
                "max": max(elapsed),
            },
            "wire_bytes": {
                "min": min(wire_bytes),
                "median": int(statistics.median(wire_bytes)),
                "max": max(wire_bytes),
            },
        },
        "rippled": {
            "cases": 18,
            "passed": rippled["passed_case_count"],
            "validator_governance_conflicting_roots": rippled["conflicting_root_count"],
            "native_csf_scope": "separate ledger-consensus control",
        },
        "material_safety_delta": {
            "case_id": "six-divergent-local-quorums",
            "cobalt": "unsafe trust graph rejected before commitment; zero conflicting roots",
            "rippled": "two local-UNL quorums admit two registry roots; one conflict",
        },
        "ninety_percent_overlap": [
            {
                "case_id": row["case_id"],
                "graph_safe": row["graph_safe"],
                "conflicting_roots": row["conflicting_roots"],
                "outcomes": sorted(
                    {node["outcome"] for node in row["actual_nodes"].values()}
                ),
                "expectation_passed": row["expectation_passed"],
            }
            for row in overlap_rows
        ],
    }

    shutil.copyfile(args.cobalt_report, output / "cobalt-report.json")
    shutil.copyfile(args.rippled_report, output / "rippled-report.json")
    write_json(output / "source-manifest.json", source_manifest)
    write_json(output / "section2-summary.json", summary)

    names = [
        "cobalt-report.json",
        "rippled-report.json",
        "section2-summary.json",
        "source-manifest.json",
    ]
    checksum_lines = [
        f"{digest((output / name).read_bytes())}  {name}" for name in names
    ]
    (output / "SHA256SUMS.txt").write_text(
        "\n".join(checksum_lines) + "\n", encoding="ascii"
    )
    print(
        json.dumps(
            {
                "status": "packet-built",
                "output": str(output),
                "sha256sums_sha256": digest(
                    (output / "SHA256SUMS.txt").read_bytes()
                ),
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
