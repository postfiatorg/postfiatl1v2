#!/usr/bin/env python3
"""Verify the compact Cobalt Section 3 isolated-validator simulation packet."""

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
    "consensus-v2-finality-receipt.json",
    "isolated-validator-simulation.json",
    "section3-summary.json",
    "source-manifest.json",
}
TASK_ID = "task_043e009b196aea0b685b3f09a6ebb45d"
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


def unique(domains: list[dict[str, Any]], field: str) -> bool:
    values = [row.get(field) for row in domains]
    return len(values) == 6 and None not in values and len(set(values)) == 6


def verify(packet: Path) -> dict[str, Any]:
    packet = packet.resolve()
    summary = read_json(packet / "section3-summary.json")
    source = read_json(packet / "source-manifest.json")
    report = read_json(packet / "isolated-validator-simulation.json")
    finality = read_json(packet / "consensus-v2-finality-receipt.json")
    domains = report.get("validator_domains", [])
    omitted = report.get("omitted_domain_receipts", [])
    transitions = report.get("transition_receipts", [])
    probes = report.get("probes", [])
    report_checks = report.get("checks", {})
    source_commit = source.get("source_commit")
    source_files = source.get("files", {})
    benchmark_source_ref = source.get("benchmark_source_ref")
    benchmark_source_commit = source.get("benchmark_source_commit")
    benchmark_files = source.get("benchmark_files", {})
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
    benchmark_hashes_ok = (
        isinstance(benchmark_source_commit, str)
        and len(benchmark_source_commit) == 40
        and isinstance(benchmark_files, dict)
        and bool(benchmark_files)
    )
    if benchmark_hashes_ok:
        try:
            benchmark_hashes_ok = all(
                digest(git_blob(benchmark_source_commit, path)) == expected
                for path, expected in benchmark_files.items()
            )
        except ValueError:
            benchmark_hashes_ok = False

    expected_faults = {
        "delay",
        "loss",
        "reorder",
        "duplicate",
        "stale_replay",
        "equivocation",
        "crash_restart",
        "partition_healing",
    }
    finality_config = finality.get("config", {})
    finality_checks = finality.get("checks", {})
    finality_metric = finality.get("metric", {})
    finality_state = finality.get("matched_initial_state", {})
    finality_runs = finality.get("cobalt_runs", [])
    finality_hashes = [
        *finality.get("binaries", {}).values(),
        *finality.get("source_report_hashes", {}).values(),
        *(row.get("report_sha256") for row in finality_runs),
    ]
    checks = {
        "checksums": verify_checksums(packet),
        "task": source.get("task_id") == TASK_ID and summary.get("task_id") == TASK_ID,
        "source_commit": (
            source_hashes_ok
            and benchmark_hashes_ok
            and summary.get("source_commit") == source_commit
            and isinstance(benchmark_source_ref, str)
            and benchmark_source_commit.startswith(benchmark_source_ref)
            and finality.get("benchmark_source_ref") == benchmark_source_ref
            and finality.get("benchmark_source_commit") == benchmark_source_commit
        ),
        "report_hash": source.get("simulation_report_sha256")
        == digest(read_bytes(packet / "isolated-validator-simulation.json")),
        "finality_source_hash": source.get("consensus_integration_report_sha256")
        == finality.get("source_aggregate_sha256"),
        "simulation_passed": (
            report.get("schema")
            == "postfiat-cobalt-isolated-validator-liveness-simulation-v1"
            and report.get("status") == "passed"
            and report.get("ok") is True
            and isinstance(report_checks, dict)
            and all(report_checks.values())
        ),
        "no_operator_claim": (
            report.get("operator_independence_claimed") is False
            and report.get("real_world_decentralization_claimed") is False
            and all(row.get("human_operator_required") is False for row in domains)
            and summary.get("scope")
            == "protocol-capability simulation; no independent-operator claim"
        ),
        "isolated_domains": (
            len(domains) == report.get("validator_count") == 6
            and all(row.get("scope") == "isolated-simulation-domain" for row in domains)
            and all(
                unique(domains, field)
                for field in (
                    "node_id",
                    "cobalt_identity_fingerprint",
                    "validator_identity_fingerprint",
                    "data_dir",
                    "transport_endpoint",
                    "trust_view_id",
                    "message_schedule_id",
                    "fault_control_channel",
                )
            )
        ),
        "liveness_boundary": (
            report.get("quorum") == 5
            and len(omitted) == 6
            and all(
                row.get("five_of_six_progress") is True
                and row.get("four_of_six_rejected") is True
                for row in omitted
            )
        ),
        "recovery_and_history": (
            report.get("round_count") == report.get("final_contiguous_sequence") == 14
            and len(probes) == 6
            and len({row.get("history_head") for row in probes}) == 1
            and len({row.get("contiguous_sequence") for row in probes}) == 1
            and all(
                row.get("catch_up_required_before_mutation") is True
                and row.get("proof_carrying_catch_up_entries") == 2
                and row.get("durable_history_equal_after_restart") is True
                for row in omitted
            )
        ),
        "fault_matrix": set(report.get("fault_classes", [])) == expected_faults,
        "transitions": (
            len(transitions) == 4
            and {row.get("operation") for row in transitions}
            == {"admit", "remove", "rotate_key", "trust_view_transition"}
            and all(row.get("verified") is True for row in transitions)
        ),
        "original_failure_closed": set(report.get("original_failure_contract", {}))
        == {
            "source",
            "pre_fix_all_six_override",
            "pre_fix_history_failure",
            "corrected_acceptance",
        },
        "consensus_v2_finality": (
            finality.get("schema")
            == "postfiat-consensus-v2-cobalt-finality-receipt-v1"
            and finality.get("status") == "PASS"
            and finality.get("scope")
            == "six isolated simulated validator domains; no external operators"
            and finality_config.get("rounds_per_lane") == 50
            and finality_config.get("validators") == 6
            and finality_config.get("simulated_validator_domains") == 6
            and finality_config.get("external_operators_required") is False
            and finality_config.get("vote_policy") == "full"
            and finality_config.get(
                "cobalt_simulation_process_cpu_quota_percent"
            )
            == finality_config.get("production_cobalt_service_cpu_quota_percent")
            == 25
            and finality_config.get("quota_matches_production_service_unit") is True
            and isinstance(finality_checks, dict)
            and bool(finality_checks)
            and all(finality_checks.values())
            and finality_metric.get("name") == "consensus_round_ms"
            and finality_metric.get("budget_percent") == 5.0
            and isinstance(finality_metric.get("delta_percent"), (int, float))
            and -100.0 < finality_metric["delta_percent"] <= 5.0
            and finality_state.get("equal") is True
            and finality_state.get("validators") == 6
            and finality_state.get("block_height") == 1
            and all(
                isinstance(finality_state.get(field), str)
                and len(finality_state[field]) == 96
                for field in ("block_tip_hash", "state_root")
            )
            and finality.get("heights", {}).get("baseline_final") == 51
            and finality.get("heights", {}).get("integration_final") == 51
            and finality.get("timing", {}).get("cobalt_coverage_ratio", 0) >= 0.95
            and len(finality_runs) >= 1
            and all(
                row.get("status") == "passed" and row.get("rounds") == 14
                for row in finality_runs
            )
            and all(
                isinstance(value, str)
                and len(value) == 64
                and all(char in "0123456789abcdef" for char in value)
                for value in finality_hashes
            )
            and set(finality.get("claims_not_made", []))
            >= {
                "independent human operators",
                "provider or geographic decentralization",
                "public WAN latency",
                "mainnet readiness",
            }
        ),
        "section2_comparison": (
            summary.get("comparison", {}).get("cobalt_cases") == 18
            and summary.get("comparison", {}).get("cobalt_conflicting_roots") == 0
            and summary.get("comparison", {}).get("rippled_cases") == 18
            and summary.get("comparison", {}).get(
                "rippled_validator_governance_conflicting_roots"
            )
            == 1
        ),
        "summary": (
            summary.get("schema") == "postfiat-cobalt-section3-summary-v1"
            and summary.get("status") == "PASS"
            and isinstance(summary.get("checks"), dict)
            and all(summary["checks"].values())
            and summary.get("consensus_v2_finality_gate", {}).get("status")
            == "PASS"
            and summary.get("consensus_v2_finality_gate", {}).get(
                "external_operators_required"
            )
            is False
            and summary.get("consensus_v2_finality_gate", {}).get("delta_percent")
            == finality_metric.get("delta_percent")
        ),
    }
    return {
        "schema": "postfiat-cobalt-section3-verification-v1",
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
        default=HERE / "section3-packet",
    )
    args = parser.parse_args()
    result = verify(args.packet)
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if result["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
