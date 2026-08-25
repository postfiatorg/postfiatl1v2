#!/usr/bin/env python3
"""Verify the frozen Cobalt E2 Byzantine-validator evidence packet."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

PACKET = Path(__file__).resolve().parent
REPO = PACKET.parents[2]
SOURCE_REVISION = "15ef2307732cf46ff3b921bf02f3ad096dda15f3"
CLASSIFICATION = "60ab419fc6cb165088c31e221a4d1a3247ad7e8d9fff9d9877bdf807b6590e93"
MANIFEST_SHA256 = "dc5e6e4ff2b54726db090e118f2ce01f1b9e6b47b4cd4ab4b0fd73ddc057e9df"
VALIDATORS = [f"validator-{index}" for index in range(6)]
STRATEGIES = [
    "rbc_propose_equivocation",
    "rbc_echo_equivocation",
    "rbc_ready_equivocation",
    "rbc_accept_equivocation",
    "abba_init_equivocation",
    "abba_aux_equivocation",
    "abba_conf_equivocation",
    "abba_finish_equivocation",
    "mvba_candidate_equivocation",
    "dabc_full_knowledge_equivocation",
    "combined_all_stages",
    "selective_withholding",
    "trust_view_lie",
    "trust_view_change",
    "competing_proposals",
    "late_vote",
    "reproposal",
    "incompatible_trust_boundary",
]
PACKET_FILES = {
    "README.md",
    "campaign-manifest.json",
    "clean-rerun/summary.json",
    "initial/campaign.json",
    "verify_packet.py",
}
ZERO_SUMMARY_FIELDS = {
    "conflicting_root_schedule_count",
    "false_accept_schedule_count",
    "false_halt_schedule_count",
    "synchrony_violation_schedule_count",
    "conflicting_root_count",
    "false_accept_count",
    "false_halt_count",
    "synchrony_violation_count",
    "rejected_state_mutation_count",
}
ZERO_COVERAGE_FIELDS = {
    "conflicting_root_schedules",
    "false_accept_schedules",
    "false_halt_schedules",
    "synchrony_violation_schedules",
}


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def object_file(path: Path) -> dict:
    value = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(value, dict), path
    return value


manifest = object_file(PACKET / "campaign-manifest.json")
initial = object_file(PACKET / "initial/campaign.json")
clean = object_file(PACKET / "clean-rerun/summary.json")

assert manifest["schema"] == "postfiat-cobalt-adversarial-e2-campaign-manifest-v1"
assert manifest["live_binding"]["validators"] == VALIDATORS
assert manifest["strategies"] == STRATEGIES
assert manifest["expected_case_count"] == len(VALIDATORS) * len(STRATEGIES)
assert manifest["fault_model"] == {
    "validator_count": 6,
    "quorum": 5,
    "local_max_active_byzantine": 1,
    "derived_f": 1,
    "first_inequality": "1 < 2*5 - 6",
    "second_inequality": "2*1 < 5",
}
assert manifest["schedule_search"]["schedules_per_case"] == 4096
assert manifest["schedule_search"]["synchrony_bound_steps"] == 40
assert digest(PACKET / "campaign-manifest.json") == MANIFEST_SHA256

for source_name in ("topology_source", "activation_source"):
    source = manifest["live_binding"][source_name]
    assert digest(REPO / source["path"]) == source["sha256"], source_name

initial_summary = initial["summary"]
clean_summary = clean["summary"]
assert initial_summary["schema"] == "postfiat-cobalt-adversarial-e2-campaign-v1"
assert initial_summary["manifest_sha256"] == MANIFEST_SHA256
assert initial_summary["source_revision"] == SOURCE_REVISION
assert initial_summary["classification_sha256"] == CLASSIFICATION
assert initial_summary["case_count"] == 108
assert initial_summary["compatible_case_count"] == 102
assert initial_summary["incompatible_case_count"] == 6
assert initial_summary["schedule_candidates"] == 442368
assert initial_summary["signed_evidence_pairs"] == 120
assert initial_summary["signed_evidence_verified"] is True
assert initial_summary["pass"] is True
assert initial_summary["summary_only"] is False
assert all(initial_summary[field] == 0 for field in ZERO_SUMMARY_FIELDS)

assert clean_summary["summary_only"] is True
comparable_initial = dict(initial_summary)
comparable_clean = dict(clean_summary)
comparable_initial.pop("summary_only")
comparable_clean.pop("summary_only")
assert comparable_initial == comparable_clean
assert clean["cases"] == []
assert initial["source_audit"] == clean["source_audit"]

source_audit = initial["source_audit"]
assert source_audit["subset_validator_count"] == 6
assert source_audit["subset_quorum"] == 5
assert source_audit["subset_max_active_byzantine"] == 1
assert source_audit["derived_f"] == 1
assert source_audit["inequalities_hold"] is True
assert source_audit["key_rotation_preserved_membership"] is True
assert source_audit["topology_source_sha256"] == manifest["live_binding"]["topology_source"]["sha256"]
assert source_audit["activation_source_sha256"] == manifest["live_binding"]["activation_source"]["sha256"]
assert source_audit["current_registry_root"] == manifest["live_binding"]["registry_root"]
assert source_audit["current_trust_graph_root"] == manifest["live_binding"]["trust_graph_root"]

cases = initial["cases"]
assert len(cases) == 108
expected_matrix = {(validator, strategy) for validator in VALIDATORS for strategy in STRATEGIES}
observed_matrix = {(case["byzantine_validator"], case["strategy"]) for case in cases}
assert observed_matrix == expected_matrix
assert len(observed_matrix) == len(cases)

evidence_pairs = 0
schedule_candidates = 0
for case in cases:
    byzantine = case["byzantine_validator"]
    compatible = case["strategy"] != "incompatible_trust_boundary"
    assert case["compatible"] is compatible
    assert case["ok"] is True
    assert case["production_transcript_accepted"] is compatible
    assert case["duplicate_contributor_rejected"] is True
    assert case["conflicting_transcript_rejected"] is True
    assert case["registry_mutated_on_rejection"] is False
    assert case["conflicting_root_count"] == 0
    assert case["false_accept"] is False
    assert case["false_halt"] is False
    assert case["synchrony_violation"] is False

    coverage = case["search_coverage"]
    assert coverage["candidates"] == 4096
    assert coverage["executed_event_schedules"] == coverage["candidates"]
    assert all(coverage[field] == 0 for field in ZERO_COVERAGE_FIELDS)
    assert all(
        coverage[field] is True
        for field in (
            "delay_varied",
            "drop_varied",
            "reorder_varied",
            "duplicate_varied",
            "partition_varied",
        )
    )
    schedule_candidates += coverage["candidates"]

    outcomes = case["per_validator"]
    assert len(outcomes) == 6
    assert {outcome["validator"] for outcome in outcomes} == set(VALIDATORS)
    correct = [outcome for outcome in outcomes if outcome["validator"] != byzantine]
    faulty = [outcome for outcome in outcomes if outcome["validator"] == byzantine]
    assert len(correct) == 5 and len(faulty) == 1
    assert faulty[0]["correct"] is False
    if compatible:
        assert len(case["accepted_registry_roots"]) == 1
        assert all(outcome["correct"] is True and outcome["decided"] is True for outcome in correct)
        assert all(
            outcome["decision_step"] is not None and outcome["decision_step"] <= 40
            for outcome in correct
        )
    else:
        assert case["accepted_registry_roots"] == []
        assert all(outcome["correct"] is True and outcome["decided"] is False for outcome in correct)

    assert case["signed_evidence"]
    for evidence in case["signed_evidence"]:
        assert evidence["signer"] == byzantine
        assert evidence["signature_verified"] is True
        assert evidence["left_message_id"] != evidence["right_message_id"]
        assert evidence["left"]["signature_hex"]
        assert evidence["right"]["signature_hex"]
        evidence_pairs += 1

assert schedule_candidates == initial_summary["schedule_candidates"]
assert evidence_pairs == initial_summary["signed_evidence_pairs"]

checksum_lines = (PACKET / "SHA256SUMS.txt").read_text(encoding="ascii").splitlines()
assert checksum_lines
listed = set()
for line in checksum_lines:
    expected, separator, name = line.partition("  ")
    path = PACKET / name
    assert separator == "  "
    assert name not in listed
    assert path.is_file()
    assert digest(path) == expected
    listed.add(name)
assert listed == PACKET_FILES

redaction_files = PACKET_FILES - {"verify_packet.py"}
scan = b"\n".join((PACKET / name).read_bytes().lower() for name in redaction_files)
assert b'"private_key"' not in scan
assert b'"secret_key"' not in scan
assert b'"api_key"' not in scan

print("e2-packet-ok")
print(f"sha256sums_sha256={digest(PACKET / 'SHA256SUMS.txt')}")
