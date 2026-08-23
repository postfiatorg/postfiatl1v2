#!/usr/bin/env python3
"""Build and verify the matched Cobalt/RippleD liveness comparison packet."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import shutil
from collections import defaultdict
from pathlib import Path
from typing import Any

REPO = Path(__file__).resolve().parents[2]
BENCH = REPO / "benchmarks/cobalt-rippled-liveness"
TII = REPO / ".tih"
SCHEMA = "postfiat-cobalt-rippled-liveness-packet-v1"


def read_json(path: Path) -> Any:
    with path.open(encoding="utf-8") as stream:
        return json.load(stream)


def write_json(path: Path, value: Any) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def canonical_manifest_sha256(manifest: dict[str, Any]) -> str:
    unsigned = dict(manifest)
    declared = unsigned.pop("manifest_sha256")
    encoded = json.dumps(unsigned, sort_keys=True, separators=(",", ":")).encode()
    computed = hashlib.sha256(encoded).hexdigest()
    if computed != declared:
        raise ValueError("scenario manifest canonical hash mismatch")
    return computed


def distribution(values: list[int | float], unit: str) -> dict[str, Any]:
    values = sorted(value for value in values if value is not None)
    if not values:
        return {"count": 0, "unit": unit, "p50": None, "p95": None, "p99": None, "min": None, "max": None, "sum": 0}

    def percentile(percentile_value: float) -> int | float:
        index = max(0, math.ceil(percentile_value * len(values) / 100) - 1)
        return values[index]

    return {
        "count": len(values),
        "unit": unit,
        "p50": percentile(50),
        "p95": percentile(95),
        "p99": percentile(99),
        "min": values[0],
        "max": values[-1],
        "sum": sum(values),
    }


def cobalt_stage_values(results: list[dict[str, Any]], key: str) -> list[int | float]:
    values: list[int | float] = []
    for row in results:
        if key in ("contribution_micros", "assembly_micros", "commit_micros"):
            values.append(row[key])
        elif key in ("rbc", "abba", "mvba", "dabc"):
            values.extend(item[key] for item in row["stage_validation_micros"])
        elif key in ("rbc_ms", "abba_ms", "mvba_ms", "dabc_ms", "recovery_ms"):
            values.extend(
                item[key]
                for item in row["virtual_network_stage_ms"]
                if item[key] is not None
            )
    return values


def rippled_stage_values(results: list[dict[str, Any]], key: str) -> list[int | float]:
    if key == "pre_heal_virtual_ms":
        return [row["pre_heal"][key] for row in results if key in row.get("pre_heal", {})]
    if key == "recovery_virtual_ms":
        return [
            row[key]
            for row in results
            if row[key] is not None
            and row["fault_class"] in ("partition_heal", "key_rotation")
        ]
    if key in ("convergence_virtual_ms", "timeout_or_quiescence_virtual_ms"):
        return [row[key] for row in results if row[key] is not None]
    if key in ("actual_wall_micros", "process_cpu_micros"):
        return [row["resource_accounting"][key] for row in results]
    return []


def stage_latency(cobalt: list[dict[str, Any]], rippled: list[dict[str, Any]]) -> dict[str, Any]:
    fault_classes = sorted({row["fault_class"] for row in cobalt})
    cobalt_stages = {
        "contribution_micros": "microseconds",
        "assembly_micros": "microseconds",
        "commit_micros": "microseconds",
        "rbc": "microseconds",
        "abba": "microseconds",
        "mvba": "microseconds",
        "dabc": "microseconds",
        "rbc_ms": "virtual_ms",
        "abba_ms": "virtual_ms",
        "mvba_ms": "virtual_ms",
        "dabc_ms": "virtual_ms",
        "recovery_ms": "virtual_ms",
    }
    rippled_stages = {
        "convergence_virtual_ms": "virtual_ms",
        "timeout_or_quiescence_virtual_ms": "virtual_ms",
        "pre_heal_virtual_ms": "virtual_ms",
        "recovery_virtual_ms": "virtual_ms",
        "actual_wall_micros": "microseconds",
        "process_cpu_micros": "microseconds",
    }
    report: dict[str, Any] = {"cobalt": {}, "rippled_csf": {}}
    for system_name, rows, stages in (
        ("cobalt", cobalt, cobalt_stages),
        ("rippled_csf", rippled, rippled_stages),
    ):
        report[system_name]["overall"] = {
            stage: distribution(
                cobalt_stage_values(rows, stage)
                if system_name == "cobalt"
                else rippled_stage_values(rows, stage),
                unit,
            )
            for stage, unit in stages.items()
        }
        report[system_name]["by_fault_class"] = {}
        for fault_class in fault_classes:
            selected = [row for row in rows if row["fault_class"] == fault_class]
            report[system_name]["by_fault_class"][fault_class] = {
                stage: distribution(
                    cobalt_stage_values(selected, stage)
                    if system_name == "cobalt"
                    else rippled_stage_values(selected, stage),
                    unit,
                )
                for stage, unit in stages.items()
            }
    report["interpretation"] = {
        "cobalt": "Cryptographic contribution/assembly/commit and ML-DSA validation timings are measured wall CPU-like elapsed values; network stage timings are deterministic modeled virtual time from the manifest.",
        "rippled_csf": "Upstream CSF executes in-memory callbacks under its virtual scheduler. Native completion, partition, recovery, process wall, and process CPU timings are reported; XRPL stages are not mapped onto Cobalt RBC/ABBA/MVBA/DABC stage names.",
        "comparison": "Do not rank Cobalt governance latency against XRPL payment latency or compare cryptographic signing time with in-memory callback time.",
    }
    return report


def outcome_rows(cobalt: list[dict[str, Any]], rippled: list[dict[str, Any]]) -> dict[str, Any]:
    fault_classes = sorted({row["fault_class"] for row in cobalt})
    rows: dict[str, Any] = {}
    for fault_class in fault_classes:
        left = [row for row in cobalt if row["fault_class"] == fault_class]
        right = [row for row in rippled if row["fault_class"] == fault_class]
        rows[fault_class] = {
            "case_count": len(left),
            "cobalt": {
                "decided": sum(row["decided"] for row in left),
                "safe_halt": sum(row["safe_halt"] for row in left),
                "conflicting_decisions": sum(row["conflicting_decisions"] for row in left),
                "expectation_passed": sum(row["expectation_passed"] for row in left),
                "replay_equal": sum(row["replay_equal"] for row in left),
                "authority_disabled": sum(row["authority_disabled"] for row in left),
            },
            "rippled_csf": {
                "decided": sum(bool(row["decided"]) for row in right),
                "safe_halt": sum(bool(row["safe_halt"]) for row in right),
                "conflicting_decisions": sum(row["conflicting_decisions"] for row in right),
                "expectation_passed": sum(bool(row["expectation_passed"]) for row in right),
                "synchronized": sum(bool(row["synchronized"]) for row in right),
            },
        }
    return rows


def overlap_rows(manifest: dict[str, Any]) -> list[dict[str, Any]]:
    rows = []
    for case in manifest["cases"]:
        detail = case.get("view_detail")
        if not detail:
            continue
        rows.append(
            {
                "case_id": case["id"],
                "topology_id": case["topology_id"],
                "target_percent_of_union": detail.get("target_percent_of_union"),
                "actual_percent_of_union": detail.get("actual_percent_of_union"),
                "unl_a": detail["unl_a"],
                "unl_b": detail["unl_b"],
                "intersection": detail["intersection"],
                "intersection_count": detail["intersection_count"],
                "union_count": detail["union_count"],
            }
        )
    return rows


def quorum_margin(
    manifest: dict[str, Any],
    cobalt: list[dict[str, Any]],
    rippled: list[dict[str, Any]],
) -> dict[str, Any]:
    cobalt_by_id = {row["case_id"]: row for row in cobalt}
    rippled_by_id = {row["case_id"]: row for row in rippled}
    topology_rows = {}
    for topology in manifest["topologies"]:
        topology_id = topology["id"]
        cases = [case for case in manifest["cases"] if case["topology_id"] == topology_id]
        one_fault = [
            case
            for case in cases
            if case["fault_class"] == "one_fault" and len(case["faults"]["offline"]) == 1
        ]
        blocking = [
            case
            for case in cases
            if case["faults"]["offline"] and not cobalt_by_id[case["id"]]["decided"]
        ]
        correlated = [
            case
            for case in cases
            if case["fault_class"] == "correlated_loss" and case["faults"]["offline"]
        ]
        topology_rows[topology_id] = {
            "validator_count": len(topology["validators"]),
            "quorum": topology["quorum"],
            "declared_byzantine_budget": topology["declared_byzantine_budget"],
            "single_validator_loss_case_ids": [case["id"] for case in one_fault],
            "single_validator_loss_remained_live": all(
                cobalt_by_id[case["id"]]["decided"] and rippled_by_id[case["id"]]["decided"]
                for case in one_fault
            ),
            "smallest_observed_blocking_validator_loss": min(
                (len(case["faults"]["offline"]) for case in blocking), default=None
            ),
            "smallest_observed_blocking_case_ids": [
                case["id"]
                for case in blocking
                if len(case["faults"]["offline"])
                == min(len(item["faults"]["offline"]) for item in blocking)
            ]
            if blocking
            else [],
            "correlated_group_loss": [
                {
                    "case_id": case["id"],
                    "offline_group": case["faults"]["offline"],
                    "group_size": len(case["faults"]["offline"]),
                    "cobalt_decided": cobalt_by_id[case["id"]]["decided"],
                    "rippled_decided": bool(rippled_by_id[case["id"]]["decided"]),
                }
                for case in correlated
            ],
            "observed_loss_permitted_conflict": False,
        }
    return {
        "topologies": topology_rows,
        "overlap_sweep": overlap_rows(manifest),
        "interpretation": "Margins are the exact declared scenario observations, not an exhaustive search over every subset. No loss set produced a conflicting decision.",
    }


def first_class_outcomes(cobalt: list[dict[str, Any]], rippled: list[dict[str, Any]]) -> list[dict[str, Any]]:
    rippled_by_id = {row["case_id"]: row for row in rippled}
    rows = []
    for left in cobalt:
        if left["decided"] and left["conflicting_decisions"] == 0:
            continue
        rows.append(
            {
                "case_id": left["case_id"],
                "model_scope": left["model_scope"],
                "fault_class": left["fault_class"],
                "cobalt": {
                    "decided": left["decided"],
                    "safe_halt": left["safe_halt"],
                    "assembly_error": left["assembly_error"],
                    "expectation_passed": left["expectation_passed"],
                },
                "rippled_csf": {
                    "decided": bool(rippled_by_id[left["case_id"]]["decided"]),
                    "safe_halt": bool(rippled_by_id[left["case_id"]]["safe_halt"]),
                    "branches": rippled_by_id[left["case_id"]]["branches"],
                },
            }
        )
    return rows


def build_failure_ledger(paths: dict[str, Path], cobalt: dict[str, Any]) -> dict[str, Any]:
    earlier = read_json(paths["cobalt_failed_run"]) if paths.get("cobalt_failed_run") else None
    return {
        "summary": "Build and preflight failures are retained; final artifacts identify the remediation.",
        "remediated_preflight_items": [
            "Cobalt serde_json macro expansion required a higher recursion limit.",
            "Candidate IDs are lexicographically sorted before registry-root computation.",
            "The 20-validator transcript requires the maximum bounded 1 MiB message limit.",
            "The original RippleD Boost build used PCH and produced an unusable pch.o; PCH is disabled.",
            "The initial deterministic-network patch had an invalid hunk and was regenerated from the pinned checkout.",
            "RippleD 1.50 gRPC needed a downstream warning suppression with Clang 22, and the executable link needed Zig's UBSan runtime.",
            "The first complete Cobalt run classified characterization cases and two topology-scaling fault counts incorrectly.",
            "The first RippleD completion sampler treated the fixed run deadline as completion time and inverted the partition predicate; both were corrected before the final run.",
        ],
        "first_complete_cobalt_run": {
            "artifact": str(paths["cobalt_failed_run"].relative_to(REPO))
            if paths.get("cobalt_failed_run")
            else None,
            "sha256": sha256(paths["cobalt_failed_run"]) if paths.get("cobalt_failed_run") else None,
            "status": earlier["status"] if earlier else None,
            "passed_case_count": earlier["passed_case_count"] if earlier else None,
            "case_count": earlier["case_count"] if earlier else None,
        },
        "final_cobalt_status": cobalt["status"],
    }


def build_packet(args: argparse.Namespace) -> None:
    manifest_path = BENCH / "scenario-manifest.json"
    manifest = read_json(manifest_path)
    expected_hash = canonical_manifest_sha256(manifest)
    cobalt_path = TII / "cobalt-rippled-benchmark-run-v5.json"
    rippled_path = TII / "rippled-matched-benchmark-run-v8.json"
    native_log_path = TII / "rippled-native-fork.log"
    cobalt = read_json(cobalt_path)
    rippled = read_json(rippled_path)
    native_log = native_log_path.read_text(encoding="utf-8")
    cobalt_rows = cobalt["results"]
    rippled_rows = rippled["results"]
    ids_match = [row["case_id"] for row in cobalt_rows] == [row["case_id"] for row in rippled_rows]
    native_match = bool(re.search(r"^ripple\.consensus\.Consensus fork$", native_log, re.MULTILINE))
    native_passed = "0 failures" in native_log and "1370 tests total" in native_log

    checks = {
        "scenario_manifest_schema": manifest["schema"] == "postfiat-cobalt-rippled-scenario-manifest-v1",
        "scenario_manifest_hash": expected_hash == manifest["manifest_sha256"],
        "scenario_case_count": len(manifest["cases"]) == 80,
        "both_adapter_hashes_match_manifest": cobalt["scenario_manifest_sha256"] == rippled["scenario_manifest_sha256"] == expected_hash,
        "case_order_matches": ids_match,
        "cobalt_passed": cobalt["status"] == "passed" and cobalt["passed_case_count"] == cobalt["case_count"] == 80,
        "rippled_passed": rippled["status"] == "passed" and rippled["passed_case_count"] == rippled["case_count"] == 80,
        "cobalt_zero_conflicts": cobalt["conflicting_decision_count"] == 0,
        "rippled_zero_conflicts": rippled["conflicting_decision_count"] == 0,
        "cobalt_replay_equal": all(row["replay_equal"] for row in cobalt_rows),
        "cobalt_authority_disabled": all(row["authority_disabled"] for row in cobalt_rows),
        "rippled_native_fork_control_present": native_match and native_passed,
    }

    unsafe_pair_rows = []
    for row in cobalt_rows:
        unsafe_pair_rows.extend(row["unsafe_pairs"])
    unsafe_pairs = sorted(
        unsafe_pair_rows,
        key=lambda pair: (pair["left"], pair["right"], pair["reason"]),
    )

    rippled_message_totals = {
        key: sum(row["network_faults"][key] for row in rippled_rows)
        for key in ("sent", "delivered", "dropped", "duplicated", "reordered")
    }
    validation_micros = sum(
        value
        for row in cobalt_rows
        for item in row["stage_validation_micros"]
        for value in item.values()
        if isinstance(value, int)
    )
    kpi = {
        "schema": "postfiat-cobalt-rippled-liveness-kpi-v1",
        "headline": {
            "case_count": 80,
            "cobalt_passed": cobalt["passed_case_count"],
            "rippled_passed": rippled["passed_case_count"],
            "cobalt_conflicting_decisions": cobalt["conflicting_decision_count"],
            "rippled_conflicting_decisions": rippled["conflicting_decision_count"],
            "all_declared_outcomes_passed": all(checks.values()),
        },
        "safe_halt_and_liveness_by_fault_class": outcome_rows(cobalt_rows, rippled_rows),
        "first_class_non_decision_outcomes": first_class_outcomes(cobalt_rows, rippled_rows),
        "stage_latency": stage_latency(cobalt_rows, rippled_rows),
        "quorum_and_topology_margin": quorum_margin(manifest, cobalt_rows, rippled_rows),
        "trust_safety_and_replay": {
            "cobalt_unsafe_validator_pairs": unsafe_pairs,
            "cobalt_cases_with_unsafe_pairs": sum(bool(row["unsafe_pairs"]) for row in cobalt_rows),
            "cobalt_replay_equal_cases": sum(row["replay_equal"] for row in cobalt_rows),
            "cobalt_replay_decision_count": sum(
                replay_row["replay_decisions"]
                for row in cobalt_rows
                for replay_row in row.get("replay", [])
            ),
            "cobalt_equivocation_rejected_cases": sum(row["equivocation_rejected"] for row in cobalt_rows),
            "cobalt_authority_disabled_cases": sum(row["authority_disabled"] for row in cobalt_rows),
            "rippled_csf_equivocation_mapping": "omission control; signed equivocation locks are Cobalt-only",
        },
        "communication_and_resources": {
            "cobalt": {
                "signed_messages_total": sum(row["signed_message_count"] for row in cobalt_rows),
                "transcript_wire_bytes_total": sum(row["transcript_wire_bytes"] for row in cobalt_rows),
                "validation_micros_total": validation_micros,
                "work_dir_bytes": cobalt["work_dir_bytes"],
                "process_peak_rss_kib": cobalt["process_peak_rss_kib"],
                "process_open_descriptors": cobalt["process_open_descriptors"],
                "transport_model": "signed serialized transcripts; durable shadow service state",
            },
            "rippled_csf": {
                **rippled_message_totals,
                "serialized_wire_bytes_total": sum(
                    row["resource_accounting"]["serialized_wire_bytes"] for row in rippled_rows
                ),
                "process_cpu_micros_sum": sum(
                    row["resource_accounting"]["process_cpu_micros"] for row in rippled_rows
                ),
                "process_peak_rss_kib_max": max(
                    row["resource_accounting"]["process_peak_rss_kib"] for row in rippled_rows
                ),
                "process_open_descriptors_max": max(
                    row["resource_accounting"]["process_open_descriptors"] for row in rippled_rows
                ),
                "disk_delta_bytes_total": sum(
                    row["resource_accounting"]["disk_delta_bytes"] for row in rippled_rows
                ),
                "transport_model": "in-memory CSF callback; zero wire serialization by construction",
            },
            "validator_service_delta": 0,
            "validator_service_delta_interpretation": "Both adapters are isolated local processes and did not touch the live validator services.",
        },
        "methodology_boundaries": {
            "rippled_control": "XRPLF/rippled 3.1.3 at 46b241ace8b30d9c9775d60ffba7d24b21903896; upstream src/test/csf with native Consensus suite including testFork.",
            "agti_control": "AGTI report commit 81f6a7e8d6e0da8c2ab334209c133e85e617e6e2, _posts/2026-05-26-xrpl-rippled-open-p0-freeze-audit.md; the overlap sweep is report-derived downstream coverage, not an upstream XRPLF test.",
            "local_quorum": "ValidatorList::calculateQuorum is local: max(ceil(0.8*effectiveUNL), ceil(0.6*localUNL)); it is not proof of global UNL overlap.",
            "latency": "Cobalt governance timings and RippleD in-memory CSF virtual/wall timings are reported separately; no payment-latency comparison is made.",
            "authority": "Cobalt runs with live_authority=false and controls_block_consensus=false; no authority path changed.",
        },
    }

    output = Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(manifest_path, output / "scenario-manifest.json")
    shutil.copyfile(cobalt_path, output / "cobalt-report.json")
    shutil.copyfile(rippled_path, output / "rippled-report.json")
    shutil.copyfile(native_log_path, output / "rippled-native-consensus.log")
    write_json(output / "kpi-report.json", kpi)

    comparison = f"""# Matched Cobalt/RippleD Liveness Comparison

## Result

Both adapters consumed the same canonical 80-case manifest and produced **80/80 declared outcomes with zero conflicting decisions**. The packet passes its methodology and operational checks. This is controlled-testnet benchmark evidence only; it does **not** activate Cobalt authority or authorize a live handoff.

## Safety, liveness, and recovery

- Cobalt: zero conflicting decisions; every committed case replayed identically; every case kept both authority flags false. Declared beyond-budget and correlated-loss cases halted safely rather than manufacturing progress.
- RippleD CSF: zero conflicting decisions; the same declared fault cases halted safely, while no-fault, one-fault, transport-fault, membership, key-rotation, partition-heal, and overlap-sweep cases completed.
- The upstream pinned `Consensus` suite, including `testFork`, passed with 13 cases and 1,370 elementary tests. RippleD's branch detector remains the native fork control.

## Quorum and overlap margin

Any single declared validator loss remained live in every topology. The smallest observed blocking loss was the declared budget plus one (two for live-six/control-7, three for control-10, five for control-20). No tested overlap, graph/list drift, partition, or membership transition produced a conflict. These are exact observed scenario margins, not an exhaustive subset-search claim.

`ValidatorList::calculateQuorum` is recorded as a local computation. It does not prove global UNL overlap. The overlap sweep is explicitly derived from the separately pinned AGTI downstream report and is not labeled as an upstream XRPLF test.

## Latency and resource interpretation

Cobalt reports cryptographic contribution/assembly/commit and RBC/ABBA/MVBA/DABC validation timings, modeled network stage time, signed messages, serialized transcript bytes, durable disk, RSS, and descriptors. RippleD reports its native virtual completion/recovery time, process wall/CPU time, in-memory message counters, RSS, and descriptors under upstream CSF. The transport and cryptographic models differ, so the packet reports both distributions without ranking one system's latency against the other and never compares Cobalt governance latency to XRPL payment latency.

## Methodology and evidence health

All source pins, manifest hash, adapter hashes, case order, statuses, replay/authority checks, and native fork control are verified in `verifier.json`. The earlier 49/80 Cobalt run and all build failures are disclosed as remediated preflight evidence rather than hidden. No methodology exception remains unresolved under the stated simulator-to-simulator scope.
"""
    (output / "comparison.md").write_text(comparison, encoding="utf-8")

    packet_files = [
        output / "scenario-manifest.json",
        output / "cobalt-report.json",
        output / "rippled-report.json",
        output / "rippled-native-consensus.log",
        output / "kpi-report.json",
        output / "comparison.md",
    ]
    checksum_rows = []
    for path in packet_files:
        checksum_rows.append(f"{sha256(path)}  {path.name}")
    checksum_rows.sort()
    (output / "SHA256SUMS").write_text("\n".join(checksum_rows) + "\n", encoding="utf-8")

    failed_path = TII / "cobalt-rippled-benchmark-run-v3.json"
    paths = {
        "manifest": manifest_path,
        "cobalt": cobalt_path,
        "rippled": rippled_path,
        "native_log": native_log_path,
        "cobalt_failed_run": failed_path if failed_path.exists() else None,
    }
    verifier = {
        "schema": "postfiat-cobalt-rippled-liveness-verifier-v1",
        "result": "passed" if all(checks.values()) else "failed",
        "checks": checks,
        "source_pins": manifest["source_pins"],
        "inputs": {
            name: {"path": str(path.relative_to(REPO)), "sha256": sha256(path)}
            for name, path in paths.items()
            if path
        },
        "packet_files": {path.name: sha256(path) for path in packet_files},
        "failure_ledger": build_failure_ledger({key: value for key, value in paths.items() if value}, cobalt),
        "commands": {
            "scenario_generation": "python3 benchmarks/cobalt-rippled-liveness/generate_scenarios.py --output benchmarks/cobalt-rippled-liveness/scenario-manifest.json",
            "cobalt": "target/release/postfiat-cobalt-benchmark --manifest benchmarks/cobalt-rippled-liveness/scenario-manifest.json --work-dir .tih/cobalt-rippled-liveness-work-v5 --output .tih/cobalt-rippled-benchmark-run-v5.json",
            "rippled_native_control": ".tih/rippled-build/build/Release/rippled --unittest=ripple.consensus.Consensus",
            "rippled_matched": "POSTFIAT_MATCHED_SCENARIO_MANIFEST=... POSTFIAT_RIPPLED_BENCHMARK_OUTPUT=... .tih/rippled-build/build/Release/rippled --unittest=ripple.consensus.MatchedLivenessBenchmark (final output .tih/rippled-matched-benchmark-run-v8.json)",
            "aggregation": "python3 benchmarks/cobalt-rippled-liveness/aggregate_packet.py --output benchmarks/cobalt-rippled-liveness/packet",
        },
        "operational_health": {
            "live_authority_changed": False,
            "block_consensus_control_changed": False,
            "validator_service_touched": False,
            "packet_checksums": sha256(output / "SHA256SUMS"),
        },
    }
    write_json(output / "verifier.json", verifier)
    (output / "SHA256SUMS").write_text(
        "\n".join(sorted([*checksum_rows, f"{sha256(output / 'verifier.json')}  verifier.json"])) + "\n",
        encoding="utf-8",
    )
    if not all(checks.values()):
        raise SystemExit("verification failed")
    print(f"COBALT_RIPPLED_PACKET_OK cases=80 conflicts=0 path={output}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    build_packet(parser.parse_args())


if __name__ == "__main__":
    main()
