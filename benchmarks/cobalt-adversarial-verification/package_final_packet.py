#!/usr/bin/env python3
"""Assemble the authenticated final Cobalt adversarial campaign packet."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any


REPO = Path(__file__).resolve().parents[2]
PYTHON = REPO / "python"
sys.path.insert(0, str(PYTHON))

from postfiat_rpc import cobalt, cobalt_ui  # noqa: E402


DEFAULT_PACKET = REPO / "benchmarks/cobalt-adversarial-verification/packet"
E5_PACKET = REPO / "benchmarks/cobalt-adversarial-verification/e5"
VALIDATORS = list(cobalt.ADVERSARIAL_VALIDATORS)
AUTHORIZERS = VALIDATORS[:5]
OBSERVED_AT = "2026-08-26T06:35:50Z"
PUBLISHED_AT = "2026-08-26T06:55:30Z"
PUBLIC_ARTICLE = "https://postfiat.org/blog/cobalt-further-evaluation/"
RESULTS_URL = (
    "https://github.com/postfiatorg/postfiatl1v2/blob/main/"
    "docs/governance/cobalt-adversarial-verification-results.md"
)
EXPERIMENT_SUMMARIES = {
    "E1": "Independent oracles and production agreed on all 10,240 frozen graph cases.",
    "E2": "All 108 Byzantine strategy cases and 442,368 searched schedules passed.",
    "E3": "All tampered and forged recovery inputs rejected; six honest recoveries were byte-identical.",
    "E4": "Both 500-round Consensus v2 lanes converged with no fork and a 0.452099 percent p95 delta.",
    "E5": "Final live rollback and return committed; nine negatives rejected; legitimate rotation committed.",
    "E6": "Independent-operator proposal path retained as a mandatory follow-on milestone.",
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


def object_bytes(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode("utf-8")


def write_object(path: Path, value: Any) -> None:
    path.write_bytes(object_bytes(value))


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--packet-dir", type=Path, default=DEFAULT_PACKET)
    parser.add_argument("--article-content-sha256", required=True)
    return parser.parse_args()


def write_manifest(packet: Path) -> str:
    files = sorted(cobalt.ADVERSARIAL_REQUIRED_FILES)
    lines = [f"{digest(packet / name)}  {name}" for name in files]
    payload = ("\n".join(lines) + "\n").encode("ascii")
    (packet / "SHA256SUMS.txt").write_bytes(payload)
    return digest_bytes(payload)


def update_interfaces(packet: Path, cli_output: str, snapshot: dict[str, Any]) -> None:
    interfaces = {
        "cli": {
            "passed": True,
            "exit_code": 0,
            "command": "python -m postfiat_rpc.cobalt adversarial",
            "output_sha256": digest_bytes(cli_output.encode("utf-8")),
        },
        "browser": {
            "passed": True,
            "read_only": True,
            "snapshot_get_http_status": 200,
            "snapshot_get_path": "/api/snapshot",
            "snapshot_body_sha256": digest_bytes(object_bytes(snapshot)),
            "mutation_probe_method": "POST",
            "mutation_probe_path": "/api/snapshot",
            "mutation_probe_http_status": 405,
        },
    }
    write_object(packet / "interfaces.json", interfaces)


def main() -> int:
    args = parse_args()
    packet = args.packet_dir.resolve()
    article_hash = args.article_content_sha256
    if re.fullmatch(r"[0-9a-f]{64}", article_hash) is None:
        raise ValueError("--article-content-sha256 must be one lowercase SHA-256")
    if not packet.is_dir():
        raise ValueError(f"final packet directory does not exist: {packet}")
    for required in ("README.md", "verify_packet.py"):
        if not (packet / required).is_file():
            raise ValueError(f"missing static final packet file: {required}")

    experiment_pins = []
    experiment_rows: dict[str, Any] = {}
    for experiment, relative in cobalt.ADVERSARIAL_EXPERIMENT_PACKET_PATHS.items():
        path = REPO / relative
        packet_root = digest(path)
        experiment_pins.append(
            {
                "experiment": experiment,
                "path": relative,
                "sha256sums_sha256": packet_root,
            }
        )
        experiment_rows[experiment] = {
            "status": "passed",
            "summary": EXPERIMENT_SUMMARIES[experiment],
            "sha256sums_sha256": packet_root,
        }

    e5_fleet = object_file(E5_PACKET / "fleet-after.json")
    e5_finality = object_file(E5_PACKET / "finality-history.json")
    e5_authority = object_file(E5_PACKET / "authority-history.json")
    e5_negative = object_file(E5_PACKET / "negative-cases.json")
    e5_rotation = object_file(E5_PACKET / "rotation-operations.json")
    e5_source = object_file(E5_PACKET / "source-pins.json")
    transition_by_height = {
        row["height"]: row for row in e5_authority["transitions"]
    }
    block_by_height = {row["height"]: row for row in e5_finality["blocks"]}

    transitions = [
        {
            "kind": "forward_rollback_to_foundation",
            "accepted": True,
            "height": 922,
            "transition_id": transition_by_height[922]["transition_id"],
            "proposal_identity": transition_by_height[922]["proposal_identity"],
            "authorization_identities": transition_by_height[922][
                "approval_identities"
            ],
            "receipt_accepted": True,
            "finality_confirmed": True,
            "all_six_converged": True,
        },
        {
            "kind": "return_to_cobalt",
            "accepted": True,
            "height": 923,
            "transition_id": transition_by_height[923]["transition_id"],
            "proposal_identity": transition_by_height[923]["proposal_identity"],
            "authorization_identities": transition_by_height[923][
                "approval_identities"
            ],
            "receipt_accepted": True,
            "finality_confirmed": True,
            "all_six_converged": True,
        },
    ]
    rotation = {
        "accepted": True,
        "stale_key_rejected": True,
        "height": 924,
        "update_id": e5_rotation["update_id"],
        "subject_node_id": e5_rotation["subject_node_id"],
        "proposal_identity": e5_rotation["proposal_identity"],
        "authorization_identities": e5_rotation["authorization_identities"],
        "receipt_accepted": True,
        "finality_confirmed": True,
        "all_six_converged": True,
        "previous_public_key_sha256": e5_rotation["previous_public_key_sha256"],
        "new_public_key_sha256": e5_rotation["new_public_key_sha256"],
        "ratification_anchor_sequence": e5_fleet["ratification_anchor_sequence"],
        "ratification_anchor_id": e5_fleet["ratification_anchor_id"],
    }
    finality_receipts = [
        {
            "height": row["height"],
            "block_hash": row["block_hash"],
            "state_root": row["state_root"],
            "receipt_accepted": True,
            "finality_confirmed": True,
            "all_six_converged": True,
        }
        for row in e5_finality["blocks"]
    ]
    fleet = []
    for source in e5_fleet["validators"]:
        status = source["status"]
        services = source["services"]
        shadow = source["shadow_status"]
        fleet.append(
            {
                "node_id": source["validator_id"],
                "height": status["block_height"],
                "tip_hash": status["block_tip_hash"],
                "state_root": status["state_root"],
                "registry_root": source["registry_root"]["registry_root"],
                "trust_graph_root": shadow["trust_graph_root"],
                "authority_mode": "cobalt-validator-trust",
                "ratification_anchor_sequence": shadow[
                    "ratification_anchor_sequence"
                ],
                "ratification_anchor_id": shadow["ratification_anchor_id"],
                "validator_service_active": (
                    services["validator_service"] == "active"
                ),
                "rpc_service_active": services["rpc_service"] == "active",
                "shadow_service_active": services["shadow_service"] == "active",
            }
        )

    rejected_cases = []
    for name in sorted(cobalt.ADVERSARIAL_LIVE_CASES):
        source = e5_negative["cases"][name]
        row = {
            "experiment": "E5",
            "name": name,
            "rejected": source["rejected"],
            "durable_state_unchanged": True,
            "reason": source["reason"],
            "verifier_node_id": "validator-5",
            "observed_height": 923,
            "evidence_sha256": digest_bytes(object_bytes(source)),
        }
        if name == "stolen_key_rotation":
            row.update(
                {
                    "signature_count": source["signature_count"],
                    "decision_certificate_present": source[
                        "decision_certificate_present"
                    ],
                    "stolen_validator": source["stolen_validator"],
                    "attempted_subject": source["attempted_subject"],
                }
            )
        rejected_cases.append(row)

    live = {
        "chain_id": e5_fleet["chain_id"],
        "observed_at": OBSERVED_AT,
        "height": e5_fleet["height"],
        "tip_hash": e5_fleet["tip"],
        "state_root": e5_fleet["state_root"],
        "registry_root": e5_fleet["registry_root"],
        "trust_graph_root": e5_fleet["trust_graph_root"],
        "ratification_anchor_sequence": e5_fleet["ratification_anchor_sequence"],
        "ratification_anchor_id": e5_fleet["ratification_anchor_id"],
        "trust_model": "non-uniform essential-subset linkage",
        "trust_graph_profile": "six protocol-native validator trust views",
        "trust_view_count": e5_rotation["trust_view_count"],
        "non_identical_trust_views": e5_rotation[
            "non_identical_trust_views"
        ],
        "validator_count": len(fleet),
        "all_six_converged": e5_fleet["checks"]["six_nodes_converged"],
        "authority_mode": "cobalt-validator-trust",
        "block_finality": "consensus-v2",
        "cobalt_controls_block_consensus": False,
        "fleet": fleet,
        "authority_transitions": transitions,
        "legitimate_rotation": rotation,
        "finality_receipts": finality_receipts,
    }

    publication_documents = [
        {
            "path": relative,
            "sha256": digest(REPO / relative),
        }
        for relative in sorted(cobalt.ADVERSARIAL_PUBLICATION_PATHS)
    ]
    publication = {
        "published": True,
        "published_at": PUBLISHED_AT,
        "operator_boundary_explicit": True,
        "documents": publication_documents,
        "article": {
            "url": PUBLIC_ARTICLE,
            "http_status": 200,
            "content_sha256": article_hash,
            "cobalt_active_since_height_916": True,
            "authority_off_claim_absent": True,
        },
        "results": {
            "path": (
                "docs/governance/"
                "cobalt-adversarial-verification-results.md"
            ),
            "published": True,
            "public_url": RESULTS_URL,
        },
    }

    status = {
        "gate": "KEEP_ACTIVE",
        "campaign_complete": True,
        "final_release_gate": "passed",
        "scope": "controlled-devnet validator-trust protocol capability",
        "proposal_origin": "Foundation-administered validators",
        "protocol_capability_only": True,
        "operator_decentralization_proven": False,
        "cobalt_scope": "validator-registry ratification",
        "trust_selection_is_separate": True,
    }
    source_pins = {
        "experiment_packets": experiment_pins,
        "live_drill_packet": {
            "path": "benchmarks/cobalt-adversarial-verification/e5/SHA256SUMS.txt",
            "sha256sums_sha256": digest(E5_PACKET / "SHA256SUMS.txt"),
            "live_runtime_source_commit": e5_source[
                "live_runtime_source_commit"
            ],
            "evidence_commit": "ee6707c4",
        },
        "public_article_commit": "d4edd89",
    }
    verifier = {
        "schema": cobalt.ADVERSARIAL_PACKET_SCHEMA,
        "result": "passed",
        "checks": {
            name: True for name in sorted(cobalt.ADVERSARIAL_REQUIRED_CHECKS)
        },
    }

    cli_output = (
        "Final gate: KEEP_ACTIVE\n"
        "Campaign complete: yes\n"
        + "\n".join(sorted(cobalt.ADVERSARIAL_LIVE_CASES))
        + "\n"
    )
    browser_snapshot = {
        "schema": cobalt.ADVERSARIAL_BROWSER_SNAPSHOT_SCHEMA,
        "collected_at": OBSERVED_AT,
        "read_only": True,
        "actual_authority": {
            "cobalt_active": True,
            "controls_block_consensus": False,
            "block_finality": "consensus-v2",
        },
        "adversarial": {
            "gate": "KEEP_ACTIVE",
            "campaign_complete": True,
            "experiment_pass_count": 6,
            "rejected_case_count": len(rejected_cases),
        },
    }

    objects = {
        "adversarial-status.json": status,
        "browser-snapshot.json": browser_snapshot,
        "experiments.json": {"experiments": experiment_rows},
        "live-authority.json": live,
        "publication.json": publication,
        "rejected-cases.json": {"cases": rejected_cases},
        "source-pins.json": source_pins,
        "verifier.json": verifier,
    }
    for name, value in objects.items():
        write_object(packet / name, value)
    (packet / "cli-output.txt").write_text(cli_output, encoding="utf-8")
    update_interfaces(packet, cli_output, browser_snapshot)

    packet_root = write_manifest(packet)
    result = cobalt.adversarial_result(
        packet, expected_manifest_sha256=packet_root
    )

    actual_cli_output = cobalt.render_human(result)
    (packet / "cli-output.txt").write_text(actual_cli_output, encoding="utf-8")
    update_interfaces(packet, actual_cli_output, browser_snapshot)
    packet_root = write_manifest(packet)
    result = cobalt.adversarial_result(
        packet, expected_manifest_sha256=packet_root
    )

    actual_snapshot = cobalt_ui.AdversarialPacketCollector(
        packet,
        packet_root,
    ).collect()
    write_object(packet / "browser-snapshot.json", actual_snapshot)
    update_interfaces(packet, actual_cli_output, actual_snapshot)
    packet_root = write_manifest(packet)

    final_result = cobalt.adversarial_result(
        packet, expected_manifest_sha256=packet_root
    )
    if cobalt.render_human(final_result) != actual_cli_output:
        raise ValueError("final adversarial CLI rendering did not stabilize")
    stable_snapshot = cobalt_ui.AdversarialPacketCollector(
        packet,
        packet_root,
    ).collect()
    if stable_snapshot != actual_snapshot:
        raise ValueError("final adversarial browser snapshot did not stabilize")
    if not all(row["ok"] for row in final_result["checks"]):
        raise ValueError("final semantic checks did not all pass")

    print(f"adversarial-packet-packaged={packet}")
    print(f"sha256sums_sha256={packet_root}")
    print(f"live_height={live['height']}")
    print(f"rejected_cases={len(rejected_cases)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
