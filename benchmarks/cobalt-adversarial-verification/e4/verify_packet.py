#!/usr/bin/env python3
"""Verify the frozen Cobalt E4 finality-isolation evidence packet."""

from __future__ import annotations

import hashlib
import json
import math
import subprocess
from pathlib import Path


PACKET = Path(__file__).resolve().parent
REPO = PACKET.parents[2]
FROZEN_SOURCE_REVISION = "451c2ad0e924f8be72feeac69c1356b3828a4f58"
RUN_SOURCE_REVISION = "add07a7cce416daeaa61073085734937477f2b71"
RUN_SCRIPT_SHA256 = "92595b0817cec7a625065d200e6c812f24fa68511a0b0a03987deea0f7efa289"
COMPARATOR_TEST_SHA256 = "316996550ffe8bd6af92309cb1bf3c64f63064d64ddd018fe8a8b4693fd8418e"
CAMPAIGN_MANIFEST_SHA256 = (
    "838a0bccda40f13c6f999fd119706739d9384509bc9495165e0cd6f04fc4c68d"
)
NODE_SHA256 = "634f08368c174a288bfc42211dc52ef0725c7f6933acc816e4a9006606189a41"
COBALT_SHA256 = "6bef2df8a2ef18c11c774309713c878470f54819b98e15face09a9f9ffa62028"
PACKET_FILES = {
    "README.md",
    "campaign-manifest.json",
    "clean-rerun/attack-report.json",
    "clean-rerun/baseline-report.json",
    "clean-rerun/consensus-v2-cobalt-integration.json",
    "clean-rerun/topology.json",
    "normalization-receipt.json",
    "remediation/cross-lane-hash-comparator-failure.json",
    "remediation/initial-failure.json",
    "verify_packet.py",
}


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def object_file(path: Path) -> dict:
    value = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(value, dict), path
    return value


def frozen_source_digest(path: str) -> str:
    completed = subprocess.run(
        ["git", "show", f"{FROZEN_SOURCE_REVISION}:{path}"],
        cwd=REPO,
        check=True,
        capture_output=True,
    )
    return hashlib.sha256(completed.stdout).hexdigest()


def revision_source_digest(revision: str, path: str) -> str:
    completed = subprocess.run(
        ["git", "show", f"{revision}:{path}"],
        cwd=REPO,
        check=True,
        capture_output=True,
    )
    return hashlib.sha256(completed.stdout).hexdigest()


def workload_fingerprint(lane: dict) -> dict:
    config = lane["config"]
    return {
        "config": {
            key: config[key]
            for key in (
                "validators",
                "rounds",
                "vote_policy",
                "wallet_address",
                "recipient",
                "amount",
            )
        },
        "iterations": [
            {
                key: row[key]
                for key in (
                    "iteration",
                    "source_node",
                    "block_height",
                    "vote_policy",
                    "validators",
                    "quorum",
                    "vote_count",
                    "receipt_accepted",
                    "finality_confirmed",
                    "round_ok",
                    "all_vote_requests_verified",
                    "all_sends_verified",
                )
            }
            for row in lane["iterations"]
        ],
    }


def nearest_rank(values: list[float], percentile: float) -> float:
    assert values
    ordered = sorted(values)
    rank = math.ceil((percentile / 100.0) * len(ordered))
    return ordered[max(0, min(rank - 1, len(ordered) - 1))]


manifest = object_file(PACKET / "campaign-manifest.json")
baseline = object_file(PACKET / "clean-rerun/baseline-report.json")
attack = object_file(PACKET / "clean-rerun/attack-report.json")
report = object_file(PACKET / "clean-rerun/consensus-v2-cobalt-integration.json")
normalization = object_file(PACKET / "normalization-receipt.json")
failure = object_file(PACKET / "remediation/initial-failure.json")
comparator_failure = object_file(
    PACKET / "remediation/cross-lane-hash-comparator-failure.json"
)

assert normalization["schema"] == "postfiat-cobalt-adversarial-e4-path-normalization-v1"
assert normalization["status"] == "passed"
assert normalization["replacement_token"] == "$E4_RUN_ROOT"
assert normalization["benchmark_semantics_changed"] is False
assert normalization["evidence_digests_rebound_to_normalized_files"] is True
assert set(normalization["reports"]) == {
    "attack-report.json",
    "baseline-report.json",
    "consensus-v2-cobalt-integration.json",
    "topology.json",
}
for name, receipt in normalization["reports"].items():
    assert len(receipt["raw_sha256"]) == 64
    assert receipt["replacement_count"] >= 0
    if receipt["replacement_count"] > 0 or name == "consensus-v2-cobalt-integration.json":
        assert receipt["raw_sha256"] != receipt["normalized_sha256"]
    assert receipt["normalized_sha256"] == digest(PACKET / "clean-rerun" / name)
assert sum(row["replacement_count"] for row in normalization["reports"].values()) > 0

assert digest(PACKET / "campaign-manifest.json") == CAMPAIGN_MANIFEST_SHA256
assert manifest["schema"] == "postfiat-cobalt-adversarial-e4-campaign-manifest-v1"
assert manifest["lanes"]["baseline"]["rounds"] == 500
assert manifest["lanes"]["attack"]["rounds"] == 500
assert manifest["lanes"]["vote_policy"] == "full"
assert manifest["lanes"]["shared_transport_retry_policy"] == {
    "retry_backoff_ms": 100,
    "send_retries": 16,
}
assert manifest["attack_lane"]["cobalt_cpu_quota_percent"] == 25
assert manifest["gates"]["attack_p95_delta_budget_percent"] == 5.0
for source in manifest["source_files"]:
    assert frozen_source_digest(source["path"]) == source["sha256"], source["path"]
assert revision_source_digest(
    RUN_SOURCE_REVISION,
    "benchmarks/cobalt-activate-or-retire/run_consensus_v2_cobalt_integration.py",
) == RUN_SCRIPT_SHA256
assert revision_source_digest(
    RUN_SOURCE_REVISION,
    "benchmarks/cobalt-activate-or-retire/test_consensus_v2_cobalt_integration.py",
) == COMPARATOR_TEST_SHA256

assert failure["schema"] == "postfiat-cobalt-adversarial-e4-initial-failure-v1"
assert failure["status"] == "remediated_rerun_required"
assert failure["baseline"]["status"] == "passed"
assert failure["baseline"]["iterations"] == 500
assert failure["attack"]["iterations"] == 0
assert failure["attack"]["failure_stage"] == "first_prepare_vote_collection"
assert failure["diagnosis"]["protocol_safety_failure_observed"] is False
assert failure["diagnosis"]["fork_observed"] is False
assert failure["diagnosis"]["durable_state_divergence_observed"] is False
assert failure["remediation_validation"]["status"] == "passed"
assert failure["rerun"]["corpus_parameters_unchanged"] is True

assert comparator_failure["schema"] == (
    "postfiat-cobalt-adversarial-e4-cross-lane-comparator-failure-v1"
)
assert comparator_failure["status"] == "remediated_rerun_required"
assert comparator_failure["source_revision"] == FROZEN_SOURCE_REVISION
assert comparator_failure["campaign"]["baseline"]["iterations"] == 500
assert comparator_failure["campaign"]["attack"]["iterations"] == 500
assert comparator_failure["campaign"]["baseline"]["six_validator_convergence"] is True
assert comparator_failure["campaign"]["attack"]["six_validator_convergence"] is True
assert comparator_failure["campaign"]["matched_semantic_workload"] is True
assert comparator_failure["diagnosis"]["protocol_safety_failure_observed"] is False
assert comparator_failure["diagnosis"]["fork_observed"] is False
assert comparator_failure["remediation"]["corpus_parameters_changed"] is False
assert comparator_failure["remediation"]["protocol_or_binary_changed"] is False
assert comparator_failure["remediation"]["postprocessor_changed"] is True
assert comparator_failure["rerun"]["same_500_plus_500_rounds"] is True

for lane, name in ((baseline, "baseline"), (attack, "attack")):
    assert lane["schema"] == "postfiat-real-transaction-latency-benchmark-v1"
    assert lane["status"] == "passed"
    assert lane["config"]["rounds"] == 500
    assert lane["config"]["validators"] == 6
    assert lane["config"]["vote_policy"] == "full"
    assert lane["config"]["send_retries"] == 16
    assert lane["config"]["retry_backoff_ms"] == 100
    assert lane["final_state"]["height"] == 501
    assert lane["final_state"]["state_verification_count"] == 6
    assert all(lane["checks"].values()), name
    iterations = lane["iterations"]
    assert len(iterations) == 500
    assert [row["iteration"] for row in iterations] == list(range(1, 501))
    assert all(
        row["round_ok"] is True
        and row["receipt_accepted"] is True
        and row["finality_confirmed"] is True
        and row["vote_policy"] == "full"
        and row["vote_count"] == 6
        for row in iterations
    )
    for metric in (
        "wallet_to_finality_ms",
        "consensus_round_ms",
        "admitted_to_finality_ms",
    ):
        values = [float(row[metric]) for row in iterations]
        assert len(values) == lane["latency"][metric]["count"] == 500
        assert math.isclose(
            nearest_rank(values, 95.0),
            float(lane["latency"][metric]["p95_ms"]),
            rel_tol=0.0,
            abs_tol=1e-9,
        )

assert workload_fingerprint(baseline) == workload_fingerprint(attack)
assert report["schema"] == "postfiat-cobalt-adversarial-e4-v2"
assert report["status"] == "passed"
assert report["source_commit"] == "add07a7c"
assert report["binaries"] == {
    "postfiat_node_sha256": NODE_SHA256,
    "cobalt_liveness_simulation_sha256": COBALT_SHA256,
}
assert report["config"]["rounds_per_lane"] == 500
assert report["config"]["vote_policy"] == "full"
assert report["config"]["quota_matches_production_service_unit"] is True
assert report["matched_initial_state"]["equal"] is True
assert report["matched_semantic_workload"]["equal"] is True
safety = report["final_state_safety"]
assert safety["passed"] is True
assert safety["baseline_fleet_converged"] is True
assert safety["attack_fleet_converged"] is True
assert safety["same_final_height"] is True
assert safety["cross_lane_hash_equality_required"] is False
assert isinstance(safety["cross_lane_hashes_equal"], bool)
assert report["heights"] == {"attack_final": 501, "baseline_final": 501}
assert all(report["checks"].values())

baseline_p95 = float(baseline["latency"]["wallet_to_finality_ms"]["p95_ms"])
attack_p95 = float(attack["latency"]["wallet_to_finality_ms"]["p95_ms"])
delta_percent = ((attack_p95 / baseline_p95) - 1.0) * 100.0
assert math.isclose(report["metric"]["baseline_p95_ms"], baseline_p95, abs_tol=1e-9)
assert math.isclose(report["metric"]["attack_p95_ms"], attack_p95, abs_tol=1e-9)
assert math.isclose(report["metric"]["delta_percent"], delta_percent, abs_tol=1e-9)
assert delta_percent <= report["metric"]["budget_percent"] == 5.0

stress = report["governance_stress"]
assert stress["proposal_count"] >= 20
assert stress["safe_halt_count"] >= 7
assert stress["view_change_count"] >= 7
rejections = report["rejections"]
assert rejections["boundary_rejection_count"] >= 21
assert rejections["named_limit_rejection_count"] >= 18
assert rejections["flood_rejection_count"] >= 16
assert all(row["durable_state_unchanged"] is True for row in rejections["receipts"])
crash = report["validator_crash_loop"]
assert crash["target"] == "validator-5"
assert crash["restart_count"] >= 3
assert {row["node_id"] for row in crash["receipts"]} == {"validator-5"}
for lane_name in ("baseline", "attack"):
    resources = report["resources"][lane_name]
    assert resources["sample_count"] >= 2
    for key in (
        "host_cpu_ticks",
        "validator_cpu_ticks",
        "validator_peak_rss_kib",
        "network_received_bytes",
        "network_transmitted_bytes",
        "node_disk_delta_bytes",
        "validator_read_bytes",
        "validator_write_bytes",
    ):
        assert isinstance(resources[key], int) and resources[key] >= 0, (lane_name, key)
assert report["operator_actions"]["manual_action_count"] == 0
assert report["operator_actions"]["automated_validator_restarts"] >= 3
assert report["evidence"]["baseline_report_sha256"] == digest(
    PACKET / "clean-rerun/baseline-report.json"
)
assert report["evidence"]["attack_report_sha256"] == digest(
    PACKET / "clean-rerun/attack-report.json"
)
assert report["evidence"]["topology_sha256"] == digest(
    PACKET / "clean-rerun/topology.json"
)

checksum_lines = (PACKET / "SHA256SUMS.txt").read_text(encoding="ascii").splitlines()
assert checksum_lines
listed: set[str] = set()
for line in checksum_lines:
    expected, separator, name = line.partition("  ")
    path = PACKET / name
    assert separator == "  "
    assert name not in listed
    assert path.is_file() and not path.is_symlink()
    assert digest(path) == expected
    listed.add(name)
assert listed == PACKET_FILES

scan = b"\n".join(
    (PACKET / name).read_bytes().lower()
    for name in PACKET_FILES
    if name not in {"README.md", "verify_packet.py"}
)
for forbidden in (
    b"/home/",
    b'"api_key"',
    b'"private_key"',
    b'"private_key_hex"',
    b'"secret_key"',
    b'"seed_hex"',
    b'"signature_hex"',
):
    assert forbidden not in scan, forbidden

print("e4-packet-ok")
print(f"sha256sums_sha256={digest(PACKET / 'SHA256SUMS.txt')}")
