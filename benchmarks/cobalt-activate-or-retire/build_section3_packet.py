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
SOURCE_FILES = [
    "benchmarks/cobalt-activate-or-retire/section2-packet/SHA256SUMS.txt",
    "benchmarks/cobalt-activate-or-retire/section2-packet/section2-summary.json",
    "benchmarks/cobalt-activate-or-retire/build_section3_packet.py",
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


def unique(domains: list[dict[str, Any]], field: str) -> bool:
    values = [row.get(field) for row in domains]
    return len(values) == 6 and None not in values and len(set(values)) == 6


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--simulation-report", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    output = args.output.resolve()
    if output.exists():
        raise ValueError(f"refusing to overwrite packet: {output}")
    output.mkdir(parents=True)

    report = read_json(args.simulation_report)
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
        "task_id": TASK_ID,
        "simulation_report_sha256": digest(args.simulation_report.read_bytes()),
        "section2_sha256sums_sha256": digest(
            (SECTION2_PACKET / "SHA256SUMS.txt").read_bytes()
        ),
        "files": {path: digest((ROOT / path).read_bytes()) for path in SOURCE_FILES},
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
        "consensus_v2_finality_gate": "measured separately; not inferred from Cobalt timing",
    }

    shutil.copyfile(args.simulation_report, output / "isolated-validator-simulation.json")
    write_json(output / "section3-summary.json", summary)
    write_json(output / "source-manifest.json", source)
    names = [
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
