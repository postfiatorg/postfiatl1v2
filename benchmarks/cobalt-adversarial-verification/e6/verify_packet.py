#!/usr/bin/env python3
"""Verify the locked Cobalt E6 proposal-path and independence decision packet."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

PACKET = Path(__file__).resolve().parent
REPO = PACKET.parents[2]
DECISION_SHA256 = "744ba0bba3b9286aa98c3591f73a71a0d695507af366234180f9ad616de0a980"
PACKET_FILES = {"README.md", "decision.json", "verify_packet.py"}
EXPECTED_SOURCES = {
    "docs/governance/cobalt-independent-operator-proposal-path-research-spec.md":
        "91ad402672653f3e76489f7e7de719d5597553111985d939a9e90b52a1edec89",
    "docs/governance/cobalt-adversarial-verification-research-spec.md":
        "3e3d31c5d45283651e5174a3679510cbac6f51d33a730225a7bb92923957ba0d",
    "benchmarks/cobalt-independent-operators/onboarding-contract.json":
        "7d25c182abd08267cb47a3847035453445d27aaf5e1b4f7b432ff9c0a1c74143",
    "benchmarks/cobalt-activation-live/packet/activation-status.json":
        "625cbe7c2a265cebea4ce43779bde10d803ca240b383b106513acac10676e18c",
    "crates/consensus_cobalt/src/trust_graph_governance.rs":
        "abbed44850d26afae6abfa54e2f452b0120eb9ca7b3127c49c926297c4c52bf2",
    "crates/consensus_cobalt/src/dabc_registry.rs":
        "d10b29dc2ac3929529d18f32d9e9d90b7df1c9593d0b1a1c8387dc0ef47fce8f",
    "crates/node/src/cobalt_handoff.rs":
        "5413efd03c25bd2e07a403aa0a78bfc4a199ce251a3041bbd9b527e6ba95ffa2",
    "crates/node/src/cobalt_authority_certificate.rs":
        "c468cccc1a8ca77040b6dd46f2a5f7ebdfb7193ea379ff56bab7a488a0b9b234",
}
FAIL_CLOSED_CASES = [
    "unregistered_proposer",
    "wrong_chain",
    "stale_authority_lineage",
    "stale_registry_root",
    "stale_trust_graph_root",
    "reused_nonce",
    "expired_slot",
    "payload_mismatch",
    "operator_manifest_mismatch",
    "missing_signature",
    "non_canonical_encoding",
    "coordinator_rewrite",
    "self_authorization",
]


def digest(path: Path) -> str:
    assert path.is_file() and not path.is_symlink(), path
    return hashlib.sha256(path.read_bytes()).hexdigest()


def object_file(path: Path) -> dict:
    assert path.stat().st_size <= 128 * 1024, path
    value = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(value, dict), path
    return value


decision = object_file(PACKET / "decision.json")
assert digest(PACKET / "decision.json") == DECISION_SHA256
assert decision["schema"] == "postfiat-cobalt-adversarial-e6-decision-v1"
assert decision["recorded_at"] == "2026-08-25"
assert decision["status"] == "locked"
assert decision["decision"] == "REINSTATE_INDEPENDENT_OPERATOR_GATE"

boundary = decision["current_boundary"]
assert boundary == {
    "network": "controlled-testnet",
    "cobalt_role": "validator-registry and trust-graph ratification",
    "block_finality": "consensus-v2",
    "proposal_origin": "Foundation-administered process",
    "protocol_and_authorization_signers":
        "six Foundation-administered validators",
    "operator_decentralization_proven": False,
    "protocol_capability_only": True,
}

quorum = decision["quorum_design"]
assert quorum["validator_count"] == 6
assert quorum["quorum"] == 5
assert quorum["tolerated_byzantine"] == 1
assert quorum["required_operator_groups"] == 6
assert quorum["maximum_validators_per_operator"] == 1
assert quorum["minimum_infrastructure_domains"] >= 3
assert quorum["single_operator_can_reach_quorum"] is False
assert quorum["single_operator_can_block_quorum"] is False
assert quorum["single_operator_outage_remaining_validators"] == 5
assert quorum["first_cobalt_inequality"] == "1 < 2*5 - 6"
assert quorum["second_cobalt_inequality"] == "2*1 < 5"

proposal = decision["proposal_path_design"]
assert proposal["admission_is_ratification"] is False
assert proposal["records_proposal_and_authorization_identities"] is True
assert proposal["foundation_coordinator_can_rewrite"] is False
assert "dual-signed" in proposal["proposal_envelope"]
assert "authenticated Cobalt RPC" in proposal["transport"]
assert "deterministic proposer" in proposal["selection"]
assert "RBC -> ABBA -> MVBA -> DABC" in proposal["ratification"]
assert proposal["ordering"] == "Consensus v2 governance batch finality"
assert decision["fail_closed_cases"] == FAIL_CLOSED_CASES

follow_on = decision["follow_on"]
assert follow_on["own_milestone_required"] is True
assert follow_on["begins_after_adversarial_verification"] is True
assert follow_on["recruits_operators_in_e6"] is False
assert follow_on["authorizes_live_migration"] is False
assert follow_on["gates_mainnet_recommendation"] is True
assert follow_on["gates_operator_decentralization_claim"] is True
assert follow_on["gates_original_activation_program_compliance_claim"] is True
assert follow_on["missing_independence_alone_triggers_live_rollback"] is False
assert decision["checks"] and all(decision["checks"].values())

sources = {
    row["path"]: row["sha256"]
    for row in decision["source_files"]
}
assert sources == EXPECTED_SOURCES
for name, expected in EXPECTED_SOURCES.items():
    assert digest(REPO / name) == expected, name

spec = (
    REPO
    / "docs/governance/cobalt-independent-operator-proposal-path-research-spec.md"
).read_text(encoding="utf-8")
normalized_spec = " ".join(spec.split())
assert "**Status:** Locked on 2026-08-25" in normalized_spec
assert "**reinstated as a separate mandatory milestone**" in normalized_spec
assert "one operator cannot reach quorum" in normalized_spec
assert "one operator cannot block quorum alone" in normalized_spec
assert "does not authorize a live migration" in normalized_spec
assert (
    "protocol capability on a Foundation-administered controlled testnet"
    in normalized_spec
)

onboarding = object_file(
    REPO / "benchmarks/cobalt-independent-operators/onboarding-contract.json"
)
assert onboarding["schema"] == "postfiat-cobalt-independent-operator-onboarding-v2"
gate = onboarding["topology_gate"]
assert gate["validator_count"] == 6
assert gate["quorum"] == 5
assert gate["required_operator_groups"] == 6
assert gate["maximum_validators_per_operator"] == 1
assert gate["minimum_infrastructure_domains"] == 3
assert gate["every_operator_below_quorum"] is True
assert gate["every_operator_withdrawal_must_preserve_quorum"] is True
assert len(onboarding["slots"]) == 6

activation = object_file(
    REPO / "benchmarks/cobalt-activation-live/packet/activation-status.json"
)
assert activation["status"] == "ACTIVATED"
assert activation["authority"]["mode"] == "cobalt-validator-trust"
assert activation["authority"]["writes_validator_registry"] is True
assert activation["authority"]["controls_block_consensus"] is False
assert activation["block_finality"] == "consensus-v2"
assert activation["verifier"]["active_validator_count"] == 6

trust_source = (
    REPO / "crates/consensus_cobalt/src/trust_graph_governance.rs"
).read_text(encoding="utf-8")
registry_source = (
    REPO / "crates/consensus_cobalt/src/dabc_registry.rs"
).read_text(encoding="utf-8")
handoff_source = (
    REPO / "crates/node/src/cobalt_handoff.rs"
).read_text(encoding="utf-8")
assert ".trust_views\n        .first()" in trust_source
assert "let proposer = config\n        .validators\n        .first()" in registry_source
assert "update.proposer != update.validators[0]" in registry_source
assert "update.cobalt_authorizations.len() < quorum" in handoff_source

checksum_lines = (
    PACKET / "SHA256SUMS.txt"
).read_text(encoding="ascii").splitlines()
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
for forbidden in (
    b'"private_key"',
    b'"secret_key"',
    b'"api_key"',
    b'"seed_hex"',
    b'"signature_hex"',
):
    assert forbidden not in scan, forbidden

print("e6-packet-ok")
print(f"decision_sha256={DECISION_SHA256}")
print(f"sha256sums_sha256={digest(PACKET / 'SHA256SUMS.txt')}")
