#!/usr/bin/env python3
"""Build the compact verifier-backed Cobalt Section 3 simulation packet."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
TASK_ID = "task_043e009b196aea0b685b3f09a6ebb45d"
SECTION2_PACKET = HERE / "section2-packet"
BENCHMARK_SOURCE_FILES = [
    "benchmarks/cobalt-activate-or-retire/run_consensus_v2_cobalt_integration.py",
    "crates/node/src/bin/postfiat_cobalt_liveness_simulation.rs",
    "crates/node/src/main.rs",
    "crates/node/src/main_parts/cli_dispatch.rs",
    "crates/node/src/main_parts/cli_dispatch_parts/group_02.rs",
    "crates/node/src/transport_cli.rs",
]
SOURCE_FILES = [
    "benchmarks/cobalt-activate-or-retire/section2-packet/SHA256SUMS.txt",
    "benchmarks/cobalt-activate-or-retire/section2-packet/section2-summary.json",
    "benchmarks/cobalt-activate-or-retire/build_section3_packet.py",
    "benchmarks/cobalt-activate-or-retire/run_consensus_v2_cobalt_integration.py",
    "benchmarks/cobalt-activate-or-retire/verify_section3_packet.py",
    "crates/node/Cargo.toml",
    "crates/node/src/bin/postfiat_cobalt_liveness_simulation.rs",
    "crates/node/src/cobalt_shadow_runtime.rs",
    "crates/consensus_cobalt/src/dabc_registry.rs",
    "crates/consensus_cobalt/src/internal_validation.rs",
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
    return subprocess.run(
        ["git", *args],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


def git_blob(commit: str, path: str) -> bytes:
    completed = subprocess.run(
        ["git", "show", f"{commit}:{path}"],
        cwd=ROOT,
        check=True,
        capture_output=True,
    )
    return completed.stdout


def unique(domains: list[dict[str, Any]], field: str) -> bool:
    values = [row.get(field) for row in domains]
    return len(values) == 6 and None not in values and len(set(values)) == 6


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--simulation-report", type=Path, required=True)
    parser.add_argument("--consensus-integration-report", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    output = args.output.resolve()
    if output.exists():
        raise ValueError(f"refusing to overwrite packet: {output}")
    output.mkdir(parents=True)

    report = read_json(args.simulation_report)
    consensus = read_json(args.consensus_integration_report)
    section2 = read_json(SECTION2_PACKET / "section2-summary.json")
    domains = report.get("validator_domains", [])
    omitted = report.get("omitted_domain_receipts", [])
    transitions = report.get("transition_receipts", [])
    probes = report.get("probes", [])
    report_checks = report.get("checks", {})
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
    consensus_config = consensus.get("config", {})
    consensus_metric = consensus.get("metric", {})
    consensus_checks = consensus.get("checks", {})
    matched_state = consensus.get("matched_initial_state", {})
    baseline_state = matched_state.get("baseline", [])
    integration_state = matched_state.get("integration", [])
    cobalt_runs = consensus.get("cobalt_runs", [])
    consensus_evidence = consensus.get("evidence", {})
    consensus_binaries = consensus.get("binaries", {})
    benchmark_source_ref = consensus.get("source_commit")
    benchmark_source_commit = (
        git_output("rev-parse", "--verify", f"{benchmark_source_ref}^{{commit}}")
        if isinstance(benchmark_source_ref, str)
        else ""
    )
    hash_values = [
        *consensus_evidence.values(),
        *consensus_binaries.values(),
        *(row.get("report_sha256") for row in cobalt_runs),
    ]
    consensus_gate = (
        consensus.get("schema")
        == "postfiat-consensus-v2-cobalt-paired-integration-v1"
        and consensus.get("status") == "passed"
        and consensus.get("scope")
        == "six-validator local protocol-capability integration simulation"
        and consensus_config.get("rounds_per_lane") == 50
        and consensus_config.get("validators") == 6
        and consensus_config.get("simulated_validator_domains") == 6
        and consensus_config.get("external_operators_required") is False
        and consensus_config.get("vote_policy") == "full"
        and consensus_config.get("cobalt_simulation_process_cpu_quota_percent")
        == consensus_config.get("production_cobalt_service_cpu_quota_percent")
        == 25
        and consensus_config.get("quota_matches_production_service_unit") is True
        and consensus_metric.get("name") == "consensus_round_ms"
        and consensus_metric.get("budget_percent") == 5.0
        and isinstance(consensus_metric.get("delta_percent"), (int, float))
        and -100.0 < consensus_metric["delta_percent"] <= 5.0
        and isinstance(consensus_checks, dict)
        and bool(consensus_checks)
        and all(consensus_checks.values())
        and matched_state.get("equal") is True
        and len(baseline_state) == len(integration_state) == 6
        and baseline_state == integration_state
        and {row.get("node_id") for row in baseline_state}
        == {f"validator-{index}" for index in range(6)}
        and len(
            {
                (
                    row.get("block_height"),
                    row.get("block_tip_hash"),
                    row.get("state_root"),
                )
                for row in baseline_state
            }
        )
        == 1
        and baseline_state[0].get("block_height") == 1
        and consensus.get("heights", {}).get("baseline_final") == 51
        and consensus.get("heights", {}).get("integration_final") == 51
        and consensus.get("timing", {}).get("cobalt_coverage_ratio", 0) >= 0.95
        and len(cobalt_runs) >= 1
        and all(
            row.get("status") == "passed" and row.get("rounds") == 14
            for row in cobalt_runs
        )
        and isinstance(benchmark_source_commit, str)
        and len(benchmark_source_commit) == 40
        and all(
            isinstance(value, str)
            and len(value) == 64
            and all(char in "0123456789abcdef" for char in value)
            for value in hash_values
        )
        and set(consensus.get("claims_not_made", []))
        >= {
            "independent human operators",
            "provider or geographic decentralization",
            "public WAN latency",
            "mainnet readiness",
        }
    )
    checks = {
        "simulation_passed": (
            report.get("schema")
            == "postfiat-cobalt-isolated-validator-liveness-simulation-v1"
            and report.get("status") == "passed"
            and report.get("ok") is True
        ),
        "simulation_scope_only": (
            report.get("operator_independence_claimed") is False
            and report.get("real_world_decentralization_claimed") is False
            and report_checks.get("simulation_only") is True
            and all(row.get("human_operator_required") is False for row in domains)
        ),
        "six_isolated_domains": (
            report.get("validator_count") == 6
            and len(domains) == 6
            and all(row.get("scope") == "isolated-simulation-domain" for row in domains)
            and unique(domains, "node_id")
            and unique(domains, "cobalt_identity_fingerprint")
            and unique(domains, "validator_identity_fingerprint")
            and unique(domains, "data_dir")
            and unique(domains, "transport_endpoint")
            and unique(domains, "message_schedule_id")
            and unique(domains, "fault_control_channel")
        ),
        "compatible_nonuniform_views": (
            unique(domains, "trust_view_id")
            and report_checks.get("non_identical_compatible_trust_views") is True
        ),
        "five_of_six_progress": (
            report.get("quorum") == 5
            and report_checks.get("five_of_six_progress") is True
            and len(omitted) == 6
            and all(row.get("five_of_six_progress") is True for row in omitted)
        ),
        "four_of_six_safe_halt": (
            report_checks.get("four_of_six_safe_halt") is True
            and all(row.get("four_of_six_rejected") is True for row in omitted)
        ),
        "every_domain_recovers_exact_history": (
            report_checks.get("every_domain_omitted_and_recovered") is True
            and report_checks.get("consistent_durable_history") is True
            and report_checks.get("crash_restart_recovered") is True
            and all(
                row.get("catch_up_required_before_mutation") is True
                and row.get("proof_carrying_catch_up_entries") == 2
                and row.get("durable_history_equal_after_restart") is True
                for row in omitted
            )
            and len(probes) == 6
            and len({row.get("history_head") for row in probes}) == 1
            and len({row.get("contiguous_sequence") for row in probes}) == 1
        ),
        "fault_matrix": (
            set(report.get("fault_classes", [])) == expected_faults
            and report_checks.get("deterministic_reorder") is True
            and report_checks.get("duplicate_rejected_or_idempotent") is True
            and report_checks.get("stale_replay_idempotent") is True
            and report_checks.get("equivocation_rejected") is True
            and report_checks.get("partition_healed") is True
        ),
        "production_protocol_paths": set(report.get("production_paths", [])) >= {
            "CobaltShadowService::create_protocol_proposal",
            "CobaltShadowService::create_protocol_contribution",
            "assemble_protocol_transcript_extending",
            "CobaltShadowService::commit_protocol_transcript",
            "CobaltShadowService::verify_history_range",
            "CobaltShadowService::catch_up_history",
            "serve_listener",
        },
        "signed_transitions_verified": (
            len(transitions) == 4
            and {row.get("operation") for row in transitions}
            == {"admit", "remove", "rotate_key", "trust_view_transition"}
            and all(row.get("verified") is True for row in transitions)
            and all(row.get("trust_graph_transition_id") for row in transitions)
            and all(
                row.get("certificate_id")
                for row in transitions
                if row.get("operation") != "trust_view_transition"
            )
        ),
        "original_failure_closed": (
            set(report.get("original_failure_contract", {}))
            == {
                "source",
                "pre_fix_all_six_override",
                "pre_fix_history_failure",
                "corrected_acceptance",
            }
            and report.get("round_count") == 14
            and report.get("final_contiguous_sequence") == 14
        ),
        "consensus_v2_finality": consensus_gate,
        "section2_comparison_passed": (
            section2.get("status") == "PASS"
            and section2.get("cobalt", {}).get("cases") == 18
            and section2.get("cobalt", {}).get("conflicting_roots") == 0
            and section2.get("rippled", {}).get("cases") == 18
            and section2.get("rippled", {}).get(
                "validator_governance_conflicting_roots"
            )
            == 1
        ),
    }
    if not all(checks.values()):
        failed = [name for name, passed in checks.items() if not passed]
        raise ValueError(f"Section 3 checks failed: {failed}")

    source_commit = git_output("rev-parse", "HEAD")
    source = {
        "schema": "postfiat-cobalt-section3-source-manifest-v1",
        "source_commit": source_commit,
        "benchmark_source_ref": benchmark_source_ref,
        "benchmark_source_commit": benchmark_source_commit,
        "task_id": TASK_ID,
        "simulation_report_sha256": digest(args.simulation_report.read_bytes()),
        "consensus_integration_report_sha256": digest(
            args.consensus_integration_report.read_bytes()
        ),
        "section2_sha256sums_sha256": digest(
            (SECTION2_PACKET / "SHA256SUMS.txt").read_bytes()
        ),
        "files": {path: digest(git_blob(source_commit, path)) for path in SOURCE_FILES},
        "benchmark_files": {
            path: digest(git_blob(benchmark_source_commit, path))
            for path in BENCHMARK_SOURCE_FILES
        },
    }
    initial_state = baseline_state[0]
    finality = {
        "schema": "postfiat-consensus-v2-cobalt-finality-receipt-v1",
        "status": "PASS",
        "scope": "six isolated simulated validator domains; no external operators",
        "benchmark_source_ref": benchmark_source_ref,
        "benchmark_source_commit": benchmark_source_commit,
        "source_aggregate_sha256": digest(
            args.consensus_integration_report.read_bytes()
        ),
        "config": consensus_config,
        "checks": consensus_checks,
        "metric": consensus_metric,
        "secondary_metrics": consensus.get("secondary_metrics", {}),
        "heights": consensus.get("heights", {}),
        "matched_initial_state": {
            "equal": matched_state["equal"],
            "validators": 6,
            "block_height": initial_state["block_height"],
            "block_tip_hash": initial_state["block_tip_hash"],
            "state_root": initial_state["state_root"],
        },
        "timing": consensus.get("timing", {}),
        "cobalt_runs": [
            {
                "report_sha256": row["report_sha256"],
                "rounds": row["rounds"],
                "status": row["status"],
            }
            for row in cobalt_runs
        ],
        "binaries": consensus_binaries,
        "source_report_hashes": consensus_evidence,
        "claims_not_made": consensus.get("claims_not_made", []),
    }
    summary = {
        "schema": "postfiat-cobalt-section3-summary-v1",
        "status": "PASS",
        "task_id": TASK_ID,
        "source_commit": source_commit,
        "scope": "protocol-capability simulation; no independent-operator claim",
        "checks": checks,
        "validator_count": 6,
        "quorum": 5,
        "rounds": report["round_count"],
        "history": {
            "sequence": report["final_contiguous_sequence"],
            "head": report["final_history_head"],
            "governance_digest": report["final_governance_digest"],
        },
        "transitions": [row["operation"] for row in transitions],
        "fault_classes": sorted(expected_faults),
        "p95_cobalt_round_validation_wall_micros": report[
            "p95_round_validation_wall_micros"
        ],
        "comparison": {
            "cobalt_cases": 18,
            "cobalt_conflicting_roots": 0,
            "rippled_cases": 18,
            "rippled_validator_governance_conflicting_roots": 1,
            "native_rippled_consensus_scope": "separate ledger-consensus control",
        },
        "consensus_v2_finality_gate": {
            "status": finality["status"],
            "rounds_per_lane": consensus_config["rounds_per_lane"],
            "baseline_p95_ms": consensus_metric["baseline_p95_ms"],
            "integration_p95_ms": consensus_metric["integration_p95_ms"],
            "delta_percent": consensus_metric["delta_percent"],
            "budget_percent": consensus_metric["budget_percent"],
            "cobalt_coverage_ratio": finality["timing"]["cobalt_coverage_ratio"],
            "external_operators_required": False,
        },
    }

    shutil.copyfile(args.simulation_report, output / "isolated-validator-simulation.json")
    write_json(output / "consensus-v2-finality-receipt.json", finality)
    write_json(output / "section3-summary.json", summary)
    write_json(output / "source-manifest.json", source)
    names = [
        "consensus-v2-finality-receipt.json",
        "isolated-validator-simulation.json",
        "section3-summary.json",
        "source-manifest.json",
    ]
    (output / "SHA256SUMS.txt").write_text(
        "".join(f"{digest((output / name).read_bytes())}  {name}\n" for name in names),
        encoding="ascii",
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
