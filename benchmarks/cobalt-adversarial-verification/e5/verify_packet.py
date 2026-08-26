#!/usr/bin/env python3
"""Verify the frozen Cobalt E5 live authority-drill evidence packet."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any


PACKET = Path(__file__).resolve().parent
VALIDATORS = [f"validator-{index}" for index in range(6)]
AUTHORIZERS = [f"validator-{index}" for index in range(5)]
CHAIN_ID = "postfiat-wan-devnet-2"
GENESIS_HASH = (
    "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25"
    "521aff3ed334da07e150a7233a3e90a9"
)
LIVE_RUNTIME_SOURCE = "8cc7d15edc58b5f5a0b745143fef2d45203465ff"
DRILL_FIX_SOURCE = "4cd9f91eb751ab6d05ed8c4094e92c63a50735ab"
REGISTRY_ROOT = (
    "08a451e07aeaf9ada41a69e7c26dfd3fd86fce11c02f5567127c598b3cf775ac"
    "054b2add85295cc8c0d429bb6d2b9b1d"
)
TRUST_ROOT = (
    "89f18aef2c5726ae43043407eb4d638ee8f3b6027e58ec3553296478602232cf3"
    "c2fc5d1dfebc4058d720b16508f0307"
)
TIP = (
    "ebeb0e1ee27f30ba480255728832719d94eac1a89d762a7aa7019eae269008fac"
    "53098cf6495f477a241d63a7649fbef"
)
STATE_ROOT = (
    "0854bc47f78996b2dcd279206cbdcc0b4858395c5937e0e0d56b3d645ca6b6a9"
    "d9c9578f5ac77bb14bea9dd1ee6f413e"
)
UPDATE_ID = (
    "a6b806eb304ffc5d4c329fc179fa628745a2e609724cd67d654805ebfc4cc12bc4"
    "338ed5fa1f3bf301384ca8aaf8f18a"
)
ANCHOR_ID = (
    "5eada38d23c83709a44f2cfa7eb7897d9d4b1da906e6ef66fc5dfec7e64102edd"
    "a2e82b33d71346c1d8f75ccc21153c8"
)
TRANSITION_IDS = {
    920: (
        "93647599a9b56265cb484939e3d611b0b79c500fe8fa1ffe21095c70da36aabc4"
        "93af78ed8338c3089e6649d0fdfcf85"
    ),
    921: (
        "11e3cd3e47e8fe6b11ffdf7350888ca8ad982f92a18587583666a7f88ee926d0"
        "d1acf45745ca96aa1ac82362eb5eea9a"
    ),
    922: (
        "941b4cdd9bd0196c85bcd38c15208e7e001c13a46c64c7d076bd7bf68a234adf"
        "b071be281462e43f74c9719260ae552e"
    ),
    923: (
        "fc0fe2f7660ab430fa06e0542ebceb0502b3467b6464ee813196eb62d739a40c2"
        "5609888f874725c9e1de9d5e46cc62b"
    ),
}
PROPOSERS = {
    920: "validator-2",
    921: "validator-3",
    922: "validator-4",
    923: "validator-5",
    924: "validator-0",
}
REQUIRED_CASES = {
    "early",
    "stale",
    "replayed",
    "wrong_root",
    "cross_chain",
    "mixed_authority",
    "self_authorized",
    "replayed_rollback",
    "stolen_key_rotation",
}
PACKET_FILES = {
    "README.md",
    "authority-history.json",
    "finality-history.json",
    "fleet-after.json",
    "negative-cases.json",
    "h920-transition.json",
    "h921-transition.json",
    "h922-transition.json",
    "h923-transition.json",
    "h924-block-certificate.json",
    "h924-block-proposal.json",
    "h924-registry-update.json",
    "rotation-operations.json",
    "source-pins.json",
    "verifier.json",
    "verify_packet.py",
}


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def object_file(name: str) -> dict[str, Any]:
    value = json.loads((PACKET / name).read_text(encoding="utf-8"))
    assert isinstance(value, dict), name
    return value


manifest = (PACKET / "SHA256SUMS.txt").read_text(encoding="ascii").splitlines()
listed: set[str] = set()
for line in manifest:
    expected, separator, name = line.partition("  ")
    path = PACKET / name
    assert separator == "  "
    assert len(expected) == 64
    assert name not in listed
    assert path.is_file() and not path.is_symlink()
    assert digest(path) == expected
    listed.add(name)
assert listed == PACKET_FILES

authority = object_file("authority-history.json")
finality = object_file("finality-history.json")
fleet = object_file("fleet-after.json")
negative = object_file("negative-cases.json")
rotation = object_file("rotation-operations.json")
pins = object_file("source-pins.json")
recorded_verifier = object_file("verifier.json")
proposal = object_file("h924-block-proposal.json")
block_certificate = object_file("h924-block-certificate.json")
update = object_file("h924-registry-update.json")
transitions = {
    height: object_file(f"h{height}-transition.json")
    for height in range(920, 924)
}

assert authority["schema"] == "postfiat-cobalt-adversarial-e5-authority-history-v1"
assert authority["status"] == "passed"
assert authority["chain_id"] == CHAIN_ID
assert authority["initial_rehearsal"] == {
    "accepted": True,
    "finality_interruption_observed": False,
    "heights": [920, 921],
    "reason": (
        "the first return used a trust binding that did not match the "
        "protocol-native post-return graph"
    ),
    "remediation_required": True,
    "safety_failure_observed": False,
}
assert authority["corrective_pair"] == {
    "accepted": True,
    "authority_after": "cobalt-validator-trust",
    "final_gate_pair": True,
    "heights": [922, 923],
}

expected_modes = {
    920: (1, 0, "rollback_to_foundation"),
    921: (0, 1, "activate_cobalt"),
    922: (1, 0, "rollback_to_foundation"),
    923: (0, 1, "activate_cobalt"),
}
for height, transition in transitions.items():
    source_mode, target_mode, kind = expected_modes[height]
    assert transition["schema"] == "postfiat.cobalt_governance_authority_transition.v1"
    assert transition["transition_id"] == TRANSITION_IDS[height]
    assert transition["chain_id"] == CHAIN_ID
    assert transition["genesis_hash"] == GENESIS_HASH
    assert transition["activation_height"] == height
    assert transition["from_authority_mode"] == source_mode
    assert transition["to_authority_mode"] == target_mode
    assert transition["transition_kind"] == kind
    assert transition["validators"] == VALIDATORS
    assert transition["approval_quorum"] == 5
    assert [row["validator"] for row in transition["approvals"]] == AUTHORIZERS
    assert all(
        row["algorithm_id"] == "ML-DSA-65"
        and isinstance(row["signature_hex"], str)
        and len(row["signature_hex"]) > 1000
        for row in transition["approvals"]
    )
    if height > 920:
        assert transition["previous_transition_id"] == TRANSITION_IDS[height - 1]

rows = authority["transitions"]
assert [row["height"] for row in rows] == list(range(920, 924))
for row in rows:
    height = row["height"]
    assert row["accepted"] is True
    assert row["transition_id"] == TRANSITION_IDS[height]
    assert row["receipt_id"] == TRANSITION_IDS[height]
    assert row["proposal_identity"] == PROPOSERS[height]
    assert row["approval_identities"] == AUTHORIZERS
    assert row["approval_quorum"] == 5
    assert row["final_gate_transition"] is (height in {922, 923})
    assert row["corrective"] is (height in {922, 923})
    assert row["signed_artifact_sha256"] == digest(
        PACKET / row["signed_artifact"]
    )

assert finality["schema"] == "postfiat-cobalt-adversarial-e5-finality-history-v1"
assert finality["status"] == "passed"
assert finality["all_six_histories_identical"] is True
assert set(finality["node_history_sha256"]) == set(VALIDATORS)
blocks = finality["blocks"]
assert [row["height"] for row in blocks] == list(range(920, 925))
for index, block in enumerate(blocks):
    height = block["height"]
    assert block["batch_kind"] == "governance"
    assert block["proposer"] == PROPOSERS[height]
    assert block["certificate_quorum"] == 5
    assert block["certificate_validators"] == VALIDATORS
    assert len(block["certificate_vote_validators"]) >= 5
    assert len(block["certificate_vote_validators"]) == len(
        set(block["certificate_vote_validators"])
    )
    assert set(block["certificate_vote_validators"]).issubset(set(VALIDATORS))
    if index:
        assert block["parent_hash"] == blocks[index - 1]["block_hash"]
    if height <= 923:
        assert block["receipt_ids"] == [TRANSITION_IDS[height]]
assert blocks[-1]["receipt_ids"] == [UPDATE_ID]
assert blocks[-1]["block_hash"] == TIP
assert blocks[-1]["state_root"] == STATE_ROOT

assert update["schema"] == "postfiat.validator_registry_update.v1"
assert update["chain_id"] == CHAIN_ID
assert update["genesis_hash"] == GENESIS_HASH
assert update["activation_height"] == 924
assert update["update_id"] == UPDATE_ID
assert update["operation"] == "rotate_key"
assert update["subject_node_id"] == "validator-5"
assert update["proposer"] == "validator-0"
assert update["validators"] == VALIDATORS
assert update["quorum"] == 5
assert update["support"] == AUTHORIZERS
assert [row["validator"] for row in update["votes"]] == AUTHORIZERS
assert all(row["accept"] is True for row in update["votes"])
assert sorted(row["validator"] for row in update["cobalt_authorizations"]) == AUTHORIZERS
assert update["previous_record"]["node_id"] == "validator-5"
assert update["new_record"]["node_id"] == "validator-5"
assert update["previous_record"]["public_key_hex"] != update["new_record"]["public_key_hex"]
assert update["new_registry_root"] == REGISTRY_ROOT
assert update["new_trust_graph_root"] == TRUST_ROOT
assert update["cobalt_decision_certificate"]["schema"] == (
    "postfiat.cobalt_validator_update_decision_certificate.v1"
)

assert proposal["schema"] == "postfiat.block_proposal.v1"
assert proposal["chain_id"] == CHAIN_ID
assert proposal["genesis_hash"] == GENESIS_HASH
assert proposal["block_height"] == 924
assert proposal["parent_hash"] == blocks[-2]["block_hash"]
assert proposal["proposer"] == "validator-0"
assert proposal["batch_kind"] == "governance"
assert proposal["batch_id"] == blocks[-1]["batch_id"]
assert proposal["state_root"] == STATE_ROOT
assert proposal["receipt_ids"] == [UPDATE_ID]
assert block_certificate["schema"] == "postfiat.block_certificate.v1"
assert block_certificate["chain_id"] == CHAIN_ID
assert block_certificate["genesis_hash"] == GENESIS_HASH
assert block_certificate["block_height"] == 924
assert block_certificate["proposer"] == "validator-0"
assert block_certificate["certificate_id"] == blocks[-1]["certificate_id"]
certificate = block_certificate["certificate"]
assert certificate["validators"] == VALIDATORS
assert certificate["quorum"] == 5
assert [row["validator"] for row in certificate["votes"]] == VALIDATORS
assert all(row["accept"] is True for row in certificate["votes"])
assert block_certificate["consensus_v2_commit"]["schema"] == "postfiat-consensus-commit-v2"

assert negative["schema"] == "postfiat-cobalt-adversarial-e5-live-negative-v1"
assert negative["status"] == "passed"
assert negative["source_commit"] == LIVE_RUNTIME_SOURCE
assert set(negative["cases"]) == REQUIRED_CASES
assert all(
    row["rejected"] is True and bool(row["reason"])
    for row in negative["cases"].values()
)
assert all(negative["checks"].values())
state_files = negative["state_files"]
assert state_files["governance_sha256_before"] == state_files["governance_sha256_after"]
assert state_files["registry_sha256_before"] == state_files["registry_sha256_after"]
stolen = negative["cases"]["stolen_key_rotation"]
assert stolen["attempted_subject"] == "validator-5"
assert stolen["stolen_validator"] == "validator-5"
assert stolen["signature_count"] == 1
assert stolen["decision_certificate_present"] is True

assert rotation["schema"] == "postfiat-cobalt-adversarial-e5-rotation-operations-v1"
assert rotation["status"] == "passed"
assert rotation["height"] == 924
assert rotation["update_id"] == UPDATE_ID
assert rotation["subject_node_id"] == "validator-5"
assert rotation["proposal_identity"] == "validator-0"
assert rotation["authorization_identities"] == AUTHORIZERS
assert rotation["decision_certificate_present"] is True
assert rotation["private_key_material_in_packet"] is False
assert rotation["previous_public_key_sha256"] != rotation["new_public_key_sha256"]
assert rotation["key_stage"]["registry_public_key_matched"] is True
assert rotation["local_key_validation"]["validator_keys_valid"] is True
assert rotation["local_key_validation"]["validator_key_permissions_valid"] is True
assert rotation["old_key_retry"]["rejected"] is True
assert rotation["old_key_retry"]["durable_key_file_unchanged"] is True
assert rotation["old_key_retry"]["current_key_file_sha256_before"] == (
    rotation["old_key_retry"]["current_key_file_sha256_after"]
)
assert rotation["trust_view_count"] == 6
assert rotation["non_identical_trust_views"] is True
shadow_receipts = rotation["shadow_lineage"]
assert [row["validator_id"] for row in shadow_receipts] == VALIDATORS
for row in shadow_receipts:
    assert row["schema"] == "postfiat-cobalt-shadow-registry-lineage-reset-v1"
    assert row["registry_root"] == REGISTRY_ROOT
    assert row["trust_graph_root"] == TRUST_ROOT
    assert row["update_id"] == UPDATE_ID
    assert row["ratification_anchor_sequence"] == 2
    assert row["ratification_anchor_id"] == ANCHOR_ID
    assert "archive_dir" not in row

assert fleet["schema"] == "postfiat-cobalt-adversarial-e5-fleet-post-rotation-v1"
assert fleet["status"] == "passed"
assert fleet["chain_id"] == CHAIN_ID
assert fleet["height"] == 924
assert fleet["tip"] == TIP
assert fleet["state_root"] == STATE_ROOT
assert fleet["registry_root"] == REGISTRY_ROOT
assert fleet["trust_graph_root"] == TRUST_ROOT
assert fleet["update_id"] == UPDATE_ID
assert fleet["ratification_anchor_sequence"] == 2
assert fleet["ratification_anchor_id"] == ANCHOR_ID
assert fleet["observed_from"] == "2026-08-26T06:34:55Z"
assert fleet["observed_through"] == "2026-08-26T06:35:50Z"
assert all(fleet["checks"].values())
assert [row["validator_id"] for row in fleet["validators"]] == VALIDATORS
for row in fleet["validators"]:
    status = row["status"]
    services = row["services"]
    governance = row["governance_verification"]
    shadow = row["shadow_status"]
    assert status["node_id"] == row["validator_id"]
    assert status["chain_id"] == CHAIN_ID
    assert status["block_height"] == 924
    assert status["block_tip_hash"] == TIP
    assert status["state_root"] == STATE_ROOT
    assert status["mempool_pending"] == 0
    assert status["status"] == "running"
    assert services["validator_service"] == "active"
    assert services["rpc_service"] == "active"
    assert services["shadow_service"] == "active"
    assert services["node_binary_sha256"] == (
        "d5e5ef630155e61b001b84edb404a4def7d29a9205f23d33d2ad9c37c2696caf"
    )
    assert services["shadow_binary_sha256"] == (
        "d61e6d0f6767998c4abfbf4f85e1f6bd5edfeef8a7a27cf965c17b676b1a0a4a"
    )
    assert governance["verified"] is True
    assert governance["authority_mode"] == 1
    assert governance["active_validators"] == VALIDATORS
    assert governance["validator_registry_update_count"] == 2
    assert governance["latest_validator_registry_update_id"] == UPDATE_ID
    assert row["registry_root"]["registry_root"] == REGISTRY_ROOT
    assert shadow["registry_root"] == REGISTRY_ROOT
    assert shadow["trust_graph_root"] == TRUST_ROOT
    assert shadow["ratification_anchor_sequence"] == 2
    assert shadow["ratification_anchor_id"] == ANCHOR_ID
    assert shadow["transport_healthy"] is True
    assert shadow["catch_up_status"] == "current"
    assert shadow["controls_block_consensus"] is False

assert pins["schema"] == "postfiat-cobalt-adversarial-e5-source-pins-v1"
assert pins["chain_id"] == CHAIN_ID
assert pins["genesis_hash"] == GENESIS_HASH
assert pins["observed_from"] == fleet["observed_from"]
assert pins["observed_through"] == fleet["observed_through"]
assert pins["live_runtime_source_commit"] == LIVE_RUNTIME_SOURCE
assert pins["drill_fix_source_commit"] == DRILL_FIX_SOURCE
assert pins["binaries"] == {
    "deployed_node_sha256": "d5e5ef630155e61b001b84edb404a4def7d29a9205f23d33d2ad9c37c2696caf",
    "deployed_shadow_sha256": "d61e6d0f6767998c4abfbf4f85e1f6bd5edfeef8a7a27cf965c17b676b1a0a4a",
    "e5_live_drill_sha256": "2393f44f6b495851c44e9da32372149688914cc5ef9eda95ea08605b750e9e90",
    "handoff_rehearsal_sha256": "bf4b267efa5f0229aa0b4427885d56f1fb92c1b78d6a9051137fc378d532a644",
}
pin_by_path = {
    row["path"]: row["sha256"] for row in pins["operational_artifacts"]
}
assert len(pin_by_path) == len(pins["operational_artifacts"])
packet_source_map = {
    "rollback-920/transition-signed.json": "h920-transition.json",
    "return-921/transition-signed.json": "h921-transition.json",
    "corrective-rollback-922/transition-signed.json": "h922-transition.json",
    "corrective-return-923/transition-signed.json": "h923-transition.json",
    "rotation-924/finality-history-920-924.json": "finality-history.json",
    "rotation-924/fleet-post-rotation.json": "fleet-after.json",
    "rotation-924/negative-cases.json": "negative-cases.json",
    "rotation-924/transport-artifacts-final/block-certificate.json": (
        "h924-block-certificate.json"
    ),
    "rotation-924/transport-artifacts-final/block-proposal.json": (
        "h924-block-proposal.json"
    ),
    "rotation-924/registry-update-finalized.json": "h924-registry-update.json",
}
for source_name, packet_name in packet_source_map.items():
    assert pin_by_path[source_name] == digest(PACKET / packet_name)
assert pin_by_path["rotation-924/validator-5-key-stage.json"] == (
    rotation["key_stage"]["source_sha256"]
)
assert pin_by_path["rotation-924/validator-5-local-key-validation.json"] == (
    rotation["local_key_validation"]["source_sha256"]
)
assert pin_by_path["rotation-924/validator-5-old-key-stale-rejection.json"] == (
    rotation["old_key_retry"]["source_sha256"]
)
assert pin_by_path["rotation-924/next-registry-binding.json"] == (
    rotation["trust_binding_source_sha256"]
)
for row in shadow_receipts:
    assert pin_by_path[
        f"rotation-924/{row['validator_id']}.shadow-registry-lineage-reset.json"
    ] == row["source_sha256"]

computed_checks = {
    "all_six_converged": fleet["checks"]["six_nodes_converged"] is True,
    "authority_transitions_committed": all(
        row["accepted"] is True for row in authority["transitions"]
    ),
    "canonical_governance_verified": (
        fleet["checks"]["canonical_governance_verified"] is True
    ),
    "consensus_v2_uninterrupted": (
        finality["status"] == "passed"
        and finality["all_six_histories_identical"] is True
        and [row["height"] for row in blocks] == list(range(920, 925))
    ),
    "decision_certificate_present": rotation["decision_certificate_present"] is True,
    "legitimate_rotation_committed": (
        update["activation_height"] == 924
        and update["subject_node_id"] == "validator-5"
        and rotation["authorization_identities"] == AUTHORIZERS
    ),
    "live_negative_cases_rejected": (
        set(negative["cases"]) == REQUIRED_CASES
        and all(row["rejected"] is True for row in negative["cases"].values())
        and negative["checks"]["durable_state_unchanged"] is True
    ),
    "private_material_absent": rotation["private_key_material_in_packet"] is False,
    "shadow_lineage_converged": (
        len(shadow_receipts) == 6
        and {row["validator_id"] for row in shadow_receipts} == set(VALIDATORS)
    ),
    "stolen_key_rejected": (
        stolen["rejected"] is True and stolen["signature_count"] == 1
    ),
    "transition_signatures_present": all(
        len(value["approvals"]) == 5
        and all(row.get("signature_hex") for row in value["approvals"])
        for value in transitions.values()
    ),
    "updated_key_validated": (
        rotation["key_stage"]["registry_public_key_matched"] is True
        and rotation["local_key_validation"]["validator_keys_valid"] is True
        and rotation["old_key_retry"]["durable_key_file_unchanged"] is True
    ),
}
assert all(computed_checks.values())
assert recorded_verifier == {
    "schema": "postfiat-cobalt-adversarial-e5-verifier-v1",
    "status": "passed",
    "checks": computed_checks,
}

scan = b"\n".join(
    (PACKET / name).read_bytes().lower()
    for name in PACKET_FILES
    if name != "verify_packet.py"
)
for forbidden in (
    b"/home/",
    b'"api_key"',
    b'"private_key"',
    b'"private_key_hex"',
    b'"secret_key"',
    b'"seed_hex"',
    b"replacement-master-key",
):
    assert forbidden not in scan, forbidden

print("e5-packet-ok")
print(f"sha256sums_sha256={digest(PACKET / 'SHA256SUMS.txt')}")
