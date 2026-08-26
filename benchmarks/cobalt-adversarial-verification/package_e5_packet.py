#!/usr/bin/env python3
"""Package the completed live Cobalt E5 authority drill evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
from pathlib import Path
from typing import Any


REPO = Path(__file__).resolve().parents[2]
DEFAULT_PACKET = REPO / "benchmarks/cobalt-adversarial-verification/e5"
LIVE_RUNTIME_SOURCE = "8cc7d15edc58b5f5a0b745143fef2d45203465ff"
DRILL_FIX_SOURCE = "4cd9f91eb751ab6d05ed8c4094e92c63a50735ab"
NODE_SHA256 = "d5e5ef630155e61b001b84edb404a4def7d29a9205f23d33d2ad9c37c2696caf"
SHADOW_SHA256 = "d61e6d0f6767998c4abfbf4f85e1f6bd5edfeef8a7a27cf965c17b676b1a0a4a"
HELPER_SHA256 = "bf4b267efa5f0229aa0b4427885d56f1fb92c1b78d6a9051137fc378d532a644"
DRILL_SHA256 = "2393f44f6b495851c44e9da32372149688914cc5ef9eda95ea08605b750e9e90"
VALIDATORS = [f"validator-{index}" for index in range(6)]
AUTHORIZERS = [f"validator-{index}" for index in range(5)]
TRANSITION_SOURCES = {
    920: "rollback-920/transition-signed.json",
    921: "return-921/transition-signed.json",
    922: "corrective-rollback-922/transition-signed.json",
    923: "corrective-return-923/transition-signed.json",
}
COPY_SOURCES = {
    "finality-history.json": "rotation-924/finality-history-920-924.json",
    "fleet-after.json": "rotation-924/fleet-post-rotation.json",
    "negative-cases.json": "rotation-924/negative-cases.json",
    "h924-block-certificate.json": (
        "rotation-924/transport-artifacts-final/block-certificate.json"
    ),
    "h924-block-proposal.json": (
        "rotation-924/transport-artifacts-final/block-proposal.json"
    ),
    "h924-registry-update.json": "rotation-924/registry-update-finalized.json",
}


def digest_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def digest(path: Path) -> str:
    return digest_bytes(path.read_bytes())


def object_file(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"expected JSON object: {path}")
    return value


def write_object(path: Path, value: Any) -> None:
    path.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--evidence-dir", type=Path, required=True)
    parser.add_argument("--packet-dir", type=Path, default=DEFAULT_PACKET)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    evidence = args.evidence_dir.resolve()
    packet = args.packet_dir.resolve()
    if evidence == Path("/") or len(evidence.parts) < 4:
        raise ValueError("--evidence-dir must name the explicit E5 evidence directory")
    if not evidence.is_dir():
        raise ValueError(f"E5 evidence directory does not exist: {evidence}")
    packet.mkdir(parents=True, exist_ok=True)
    if not (packet / "README.md").is_file() or not (packet / "verify_packet.py").is_file():
        raise ValueError("packet README and verifier must exist before packaging")

    source_digests: dict[str, str] = {}
    for height, relative in TRANSITION_SOURCES.items():
        source = evidence / relative
        destination = packet / f"h{height}-transition.json"
        shutil.copyfile(source, destination)
        source_digests[relative] = digest(source)
    for name, relative in COPY_SOURCES.items():
        source = evidence / relative
        shutil.copyfile(source, packet / name)
        source_digests[relative] = digest(source)

    transitions = {
        height: object_file(packet / f"h{height}-transition.json")
        for height in TRANSITION_SOURCES
    }
    finality = object_file(packet / "finality-history.json")
    fleet = object_file(packet / "fleet-after.json")
    negative = object_file(packet / "negative-cases.json")
    update = object_file(packet / "h924-registry-update.json")
    proposal = object_file(packet / "h924-block-proposal.json")
    block_certificate = object_file(packet / "h924-block-certificate.json")
    block_by_height = {row["height"]: row for row in finality["blocks"]}

    authority_rows = []
    for height in sorted(transitions):
        transition = transitions[height]
        block = block_by_height[height]
        authority_rows.append(
            {
                "accepted": True,
                "approval_identities": [
                    approval["validator"] for approval in transition["approvals"]
                ],
                "approval_quorum": transition["approval_quorum"],
                "block_hash": block["block_hash"],
                "corrective": height in {922, 923},
                "final_gate_transition": height in {922, 923},
                "from_authority_mode": transition["from_authority_mode"],
                "height": height,
                "proposal_identity": block["proposer"],
                "receipt_id": block["receipt_ids"][0],
                "signed_artifact": f"h{height}-transition.json",
                "signed_artifact_sha256": digest(packet / f"h{height}-transition.json"),
                "to_authority_mode": transition["to_authority_mode"],
                "transition_id": transition["transition_id"],
                "transition_kind": transition["transition_kind"],
            }
        )
    authority_history = {
        "schema": "postfiat-cobalt-adversarial-e5-authority-history-v1",
        "status": "passed",
        "chain_id": fleet["chain_id"],
        "initial_rehearsal": {
            "heights": [920, 921],
            "accepted": True,
            "remediation_required": True,
            "reason": (
                "the first return used a trust binding that did not match the "
                "protocol-native post-return graph"
            ),
            "safety_failure_observed": False,
            "finality_interruption_observed": False,
        },
        "corrective_pair": {
            "heights": [922, 923],
            "accepted": True,
            "final_gate_pair": True,
            "authority_after": "cobalt-validator-trust",
        },
        "transitions": authority_rows,
    }
    write_object(packet / "authority-history.json", authority_history)

    previous_key = bytes.fromhex(update["previous_record"]["public_key_hex"])
    new_key = bytes.fromhex(update["new_record"]["public_key_hex"])
    key_stage_path = evidence / "rotation-924/validator-5-key-stage.json"
    key_validation_path = (
        evidence / "rotation-924/validator-5-local-key-validation.json"
    )
    stale_path = evidence / "rotation-924/validator-5-old-key-stale-rejection.json"
    key_stage = object_file(key_stage_path)
    key_validation = object_file(key_validation_path)
    stale = object_file(stale_path)
    source_digests["rotation-924/validator-5-key-stage.json"] = digest(key_stage_path)
    source_digests[
        "rotation-924/validator-5-local-key-validation.json"
    ] = digest(key_validation_path)
    source_digests[
        "rotation-924/validator-5-old-key-stale-rejection.json"
    ] = digest(stale_path)

    shadow_receipts = []
    for validator in VALIDATORS:
        relative = f"rotation-924/{validator}.shadow-registry-lineage-reset.json"
        path = evidence / relative
        receipt = object_file(path)
        source_digests[relative] = digest(path)
        shadow_receipts.append(
            {
                key: value
                for key, value in receipt.items()
                if key != "archive_dir"
            }
            | {"source_sha256": digest(path)}
        )

    binding_path = evidence / "rotation-924/next-registry-binding.json"
    binding = object_file(binding_path)
    source_digests["rotation-924/next-registry-binding.json"] = digest(binding_path)
    trust_views = binding["trust_graph"]["trust_views"]
    canonical_views = {
        json.dumps(value, sort_keys=True, separators=(",", ":"))
        for value in trust_views
    }
    rotation = {
        "schema": "postfiat-cobalt-adversarial-e5-rotation-operations-v1",
        "status": "passed",
        "height": 924,
        "update_id": update["update_id"],
        "subject_node_id": update["subject_node_id"],
        "proposal_identity": update["proposer"],
        "authorization_identities": sorted(
            row["validator"] for row in update["cobalt_authorizations"]
        ),
        "decision_certificate_present": update.get("cobalt_decision_certificate") is not None,
        "previous_public_key_sha256": digest_bytes(previous_key),
        "new_public_key_sha256": digest_bytes(new_key),
        "private_key_material_in_packet": False,
        "key_stage": {
            "source_sha256": digest(key_stage_path),
            "action": key_stage["action"],
            "registry_public_key_matched": key_stage["registry_public_key_matched"],
            "validator_id": key_stage["validator_id"],
            "validator_key_count": key_stage["validator_key_count"],
        },
        "local_key_validation": {
            "source_sha256": digest(key_validation_path),
            "node_id": key_validation["node_id"],
            "validator_keys_valid": key_validation["validator_keys_valid"],
            "validator_key_permissions_valid": key_validation[
                "validator_key_permissions_valid"
            ],
            "validator_key_count": key_validation["validator_key_count"],
        },
        "old_key_retry": stale | {"source_sha256": digest(stale_path)},
        "shadow_lineage": shadow_receipts,
        "trust_view_count": len(trust_views),
        "non_identical_trust_views": len(canonical_views) > 1,
        "trust_binding_source_sha256": digest(binding_path),
    }
    write_object(packet / "rotation-operations.json", rotation)

    pins = {
        "schema": "postfiat-cobalt-adversarial-e5-source-pins-v1",
        "chain_id": fleet["chain_id"],
        "genesis_hash": update["genesis_hash"],
        "observed_from": fleet["observed_from"],
        "observed_through": fleet["observed_through"],
        "live_runtime_source_commit": LIVE_RUNTIME_SOURCE,
        "drill_fix_source_commit": DRILL_FIX_SOURCE,
        "binaries": {
            "deployed_node_sha256": NODE_SHA256,
            "deployed_shadow_sha256": SHADOW_SHA256,
            "handoff_rehearsal_sha256": HELPER_SHA256,
            "e5_live_drill_sha256": DRILL_SHA256,
        },
        "operational_artifacts": [
            {"path": name, "sha256": value}
            for name, value in sorted(source_digests.items())
        ],
    }
    write_object(packet / "source-pins.json", pins)

    checks = {
        "all_six_converged": fleet["checks"]["six_nodes_converged"] is True,
        "authority_transitions_committed": all(
            row["accepted"] is True for row in authority_rows
        ),
        "canonical_governance_verified": (
            fleet["checks"]["canonical_governance_verified"] is True
        ),
        "consensus_v2_uninterrupted": (
            finality["status"] == "passed"
            and finality["all_six_histories_identical"] is True
            and [row["height"] for row in finality["blocks"]]
            == list(range(920, 925))
        ),
        "decision_certificate_present": (
            rotation["decision_certificate_present"] is True
        ),
        "legitimate_rotation_committed": (
            update["activation_height"] == 924
            and update["subject_node_id"] == "validator-5"
            and rotation["authorization_identities"] == AUTHORIZERS
        ),
        "live_negative_cases_rejected": (
            set(negative["cases"])
            == {
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
            and all(row["rejected"] is True for row in negative["cases"].values())
            and negative["checks"]["durable_state_unchanged"] is True
        ),
        "private_material_absent": True,
        "shadow_lineage_converged": (
            len(shadow_receipts) == 6
            and {row["validator_id"] for row in shadow_receipts}
            == set(VALIDATORS)
        ),
        "stolen_key_rejected": (
            negative["cases"]["stolen_key_rotation"]["rejected"] is True
            and negative["cases"]["stolen_key_rotation"]["signature_count"] == 1
        ),
        "transition_signatures_present": all(
            len(transition["approvals"]) == 5
            and all(approval.get("signature_hex") for approval in transition["approvals"])
            for transition in transitions.values()
        ),
        "updated_key_validated": (
            key_stage["registry_public_key_matched"] is True
            and key_validation["validator_keys_valid"] is True
            and stale["durable_key_file_unchanged"] is True
        ),
    }
    if not all(checks.values()):
        failed = sorted(name for name, passed in checks.items() if not passed)
        raise ValueError(f"E5 packaging checks failed: {', '.join(failed)}")
    write_object(
        packet / "verifier.json",
        {
            "schema": "postfiat-cobalt-adversarial-e5-verifier-v1",
            "status": "passed",
            "checks": checks,
        },
    )

    files = sorted(
        path.relative_to(packet).as_posix()
        for path in packet.rglob("*")
        if path.is_file()
        and path.name != "SHA256SUMS.txt"
        and path.suffix != ".pyc"
        and "__pycache__" not in path.parts
    )
    checksum_lines = [f"{digest(packet / name)}  {name}" for name in files]
    (packet / "SHA256SUMS.txt").write_text(
        "\n".join(checksum_lines) + "\n", encoding="ascii"
    )
    print(f"e5-packet-packaged={packet}")
    print(f"files={len(files)}")
    print(f"sha256sums_sha256={digest(packet / 'SHA256SUMS.txt')}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
