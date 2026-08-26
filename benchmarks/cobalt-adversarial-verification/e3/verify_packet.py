#!/usr/bin/env python3
"""Verify the frozen Cobalt E3 adversarial-recovery evidence packet."""

from __future__ import annotations

import hashlib
import json
import subprocess
from pathlib import Path

PACKET = Path(__file__).resolve().parent
REPO = PACKET.parents[2]
SOURCE_REVISION = "5c9e543ea0f56e7e6dda85d3a27093e810fdc111"
CLASSIFICATION = "ab53b5ddd5134e8fbbbb359b65c249ccbb1eb85a7ad034e496efa10bd85b90d3"
MANIFEST_SHA256 = "c23320d47d631efdd74c1e5c6c541951f452a4de9b14eb583f9d888b77167fa7"
LIVE_REGISTRY_ROOT = (
    "945768d593497541f59961d1ba3920560cfde7bf5037e40eb89dd5466637f2217"
    "09bff05b69d2d40a36d5cff8505c37e"
)
LIVE_TRUST_ROOT = (
    "9221316ae7f0f0e7e58d734700167f73f29aa1240377a8d61c637e7f36c5deb7"
    "28203fcbb283c9f8f3398fc41d6b8b13"
)
VALIDATORS = [f"validator-{index}" for index in range(6)]
TAMPERS = ["truncated", "padded", "reordered", "one_entry_modified"]
FORGED = [
    "fabricated_transition",
    "wrong_root_certificate",
    "omitted_latest_update",
]
PACKET_FILES = {
    "README.md",
    "campaign-manifest.json",
    "clean-rerun/summary.json",
    "initial/campaign.json",
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
        ["git", "show", f"{SOURCE_REVISION}:{path}"],
        cwd=REPO,
        check=True,
        capture_output=True,
    )
    return hashlib.sha256(completed.stdout).hexdigest()


def valid_hex(value: str, byte_count: int) -> bool:
    if len(value) != byte_count * 2:
        return False
    try:
        bytes.fromhex(value)
    except ValueError:
        return False
    return True


manifest = object_file(PACKET / "campaign-manifest.json")
initial = object_file(PACKET / "initial/campaign.json")
clean = object_file(PACKET / "clean-rerun/summary.json")

assert manifest["schema"] == "postfiat-cobalt-adversarial-e3-campaign-manifest-v1"
assert manifest["live_binding"]["validators"] == VALIDATORS
assert manifest["live_binding"]["registry_root"] == LIVE_REGISTRY_ROOT
assert manifest["live_binding"]["recorded_trust_graph_root"] == LIVE_TRUST_ROOT
assert manifest["live_binding"]["quorum"] == 5
assert manifest["history_entry_count"] == 4
assert manifest["tamper_cases"] == TAMPERS
assert manifest["forged_catch_up_cases"] == FORGED
assert digest(PACKET / "campaign-manifest.json") == MANIFEST_SHA256

for source_name in ("activation_source", "state_source"):
    source = manifest["live_binding"][source_name]
    assert digest(REPO / source["path"]) == source["sha256"], source_name
for source in manifest["source_files"]:
    assert frozen_source_digest(source["path"]) == source["sha256"], source["path"]

assert initial["schema"] == "postfiat-cobalt-adversarial-e3-campaign-v1"
assert clean["schema"] == initial["schema"]
initial_summary = initial["summary"]
clean_summary = clean["summary"]
for summary in (initial_summary, clean_summary):
    assert summary["schema"] == "postfiat-cobalt-adversarial-e3-summary-v1"
    assert summary["manifest_sha256"] == MANIFEST_SHA256
    assert summary["source_revision"] == SOURCE_REVISION
    assert summary["validator_count"] == 6
    assert summary["tamper_case_count"] == 24
    assert summary["forged_catch_up_case_count"] == 18
    assert summary["recovery_case_count"] == 6
    assert summary["rejected_case_count"] == 42
    assert summary["durable_mutation_count"] == 0
    assert summary["signed_evidence_count"] == 18
    assert summary["signed_evidence_verified"] is True
    assert summary["byte_identical_recovery_count"] == 6
    assert summary["manual_repair_action_count"] == 0
    assert summary["classification_sha256"] == CLASSIFICATION
    assert summary["pass"] is True
assert initial_summary["summary_only"] is False
assert clean_summary["summary_only"] is True
comparable_initial = dict(initial_summary)
comparable_clean = dict(clean_summary)
comparable_initial.pop("summary_only")
comparable_clean.pop("summary_only")
assert comparable_initial == comparable_clean
assert clean["cases"] == []
assert clean["recoveries"] == []
assert initial["source_audit"] == clean["source_audit"]

audit = initial["source_audit"]
assert audit["live_registry_root"] == LIVE_REGISTRY_ROOT
assert audit["recorded_live_trust_graph_root"] == LIVE_TRUST_ROOT
assert valid_hex(audit["clone_trust_graph_root"], 48)
assert audit["clone_trust_graph_root"] != LIVE_TRUST_ROOT
assert audit["live_registry_root_exact_match"] is True
assert audit["source_files_verified"] == len(manifest["source_files"])
assert audit["validator_count"] == 6
assert audit["quorum"] == 5
assert audit["activation_source_sha256"] == manifest["live_binding"]["activation_source"]["sha256"]
assert audit["state_source_sha256"] == manifest["live_binding"]["state_source"]["sha256"]

cases = initial["cases"]
assert len(cases) == 42
expected_matrix = {
    (validator, "durable_restart", attack)
    for validator in VALIDATORS
    for attack in TAMPERS
} | {
    (validator, "forged_catch_up", attack)
    for validator in VALIDATORS
    for attack in FORGED
}
observed_matrix = {
    (case["validator"], case["category"], case["attack"]) for case in cases
}
assert observed_matrix == expected_matrix
assert len(observed_matrix) == len(cases)

reason_fragments = {
    "journal_truncated": "truncated",
    "journal_record_invalid": "missing field",
    "journal_chain_mismatch": "persisted protocol history chain mismatch",
    "transition_proof_mismatch": "DABC ratified amendment id mismatch",
    "certificate_root_mismatch": "DABC ratified amendment registry root mismatch",
    "required_latest_update_omitted": "omits required latest update",
}
signed_count = 0
for case in cases:
    assert case["ok"] is True
    assert case["detected_before_rejoin"] is True
    assert case["durable_state_mutated"] is False
    assert case["journal_sha256_before"] == case["journal_sha256_after"]
    assert case["state_hash_before"] == case["state_hash_after"]
    assert reason_fragments[case["expected_reason_code"]] in case["rejection_reason"]
    assert case["elapsed_micros"] > 0
    evidence = case["signed_evidence"]
    if case["category"] == "durable_restart":
        assert evidence is None
    else:
        assert evidence["schema"] == "postfiat-cobalt-adversarial-e3-signed-evidence-v1"
        assert evidence["attack"] == case["attack"]
        assert evidence["sender"] in VALIDATORS
        assert evidence["sender"] != case["validator"]
        assert valid_hex(evidence["sender_public_key_hex"], 1952)
        assert valid_hex(evidence["range_hash"], 48)
        assert valid_hex(evidence["statement_hash"], 48)
        assert valid_hex(evidence["signature_hex"], 3309)
        assert evidence["signature_verified"] is True
        signed_count += 1
assert signed_count == 18

recoveries = initial["recoveries"]
assert len(recoveries) == 6
assert {recovery["validator"] for recovery in recoveries} == set(VALIDATORS)
for recovery in recoveries:
    index = VALIDATORS.index(recovery["validator"])
    assert recovery["first_peer"] == VALIDATORS[(index + 1) % 6]
    assert recovery["second_peer"] == VALIDATORS[(index + 2) % 6]
    assert recovery["interrupted_after_sequence"] == 2
    assert recovery["final_sequence"] == 4
    assert recovery["final_registry_root"] == LIVE_REGISTRY_ROOT
    assert recovery["final_trust_graph_root"] == audit["clone_trust_graph_root"]
    assert recovery["honest_history_sha256"] == recovery["restored_history_sha256"]
    assert recovery["byte_identical"] is True
    assert recovery["restart_succeeded"] is True
    assert recovery["no_manual_repair"] is True
    assert recovery["elapsed_micros"] > 0
    assert recovery["ok"] is True

checksum_lines = (PACKET / "SHA256SUMS.txt").read_text(encoding="ascii").splitlines()
assert checksum_lines
listed = set()
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
    if name != "verify_packet.py"
)
for forbidden in (b'"private_key"', b'"secret_key"', b'"api_key"', b'"seed_hex"'):
    assert forbidden not in scan, forbidden

print("e3-packet-ok")
print(f"sha256sums_sha256={digest(PACKET / 'SHA256SUMS.txt')}")
