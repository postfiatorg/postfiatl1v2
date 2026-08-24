#!/usr/bin/env python3
"""Static verifier for the independent-operator onboarding contract."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
CONTRACT = ROOT / "onboarding-contract.json"
EXPECTED_SOURCE_COMMIT = "3b01c2ad57fb0ce1c29e12edc88aece5b22548ae"
EXPECTED_RELEASE_BINARY_SHA256 = (
    "e036033d437d85c4f60fc8e6689a771fdda01dd2ce88456571e6c9092faf4caf"
)


def verify_sums() -> None:
    sums_file = ROOT / "SHA256SUMS.txt"
    for line in sums_file.read_text(encoding="utf-8").splitlines():
        expected, name = line.split("  ", 1)
        actual = hashlib.sha256((ROOT / name).read_bytes()).hexdigest()
        if actual != expected:
            raise SystemExit(f"SHA-256 mismatch for {name}")


def require_hex(label: str, value: object, length: int) -> str:
    if not isinstance(value, str) or len(value) != length:
        raise SystemExit(f"{label} must be {length} lowercase hex characters")
    if any(char not in "0123456789abcdef" for char in value):
        raise SystemExit(f"{label} must be lowercase hex")
    return value


def main() -> None:
    verify_sums()
    contract = json.loads(CONTRACT.read_text(encoding="utf-8"))
    if contract.get("schema") != "postfiat-cobalt-independent-operator-onboarding-v2":
        raise SystemExit("unsupported contract schema")
    if contract.get("status") != "awaiting_external_operator_receipts":
        raise SystemExit("contract must remain awaiting external operator receipts")

    binding = contract["evidence_binding"]
    require_hex("Section 2 packet root", binding["section2_packet_root"], 64)
    require_hex("source commit", binding["source_commit"], 40)
    require_hex("release binary SHA-256", binding["release_binary_sha256"], 64)
    if binding["source_commit"] != EXPECTED_SOURCE_COMMIT:
        raise SystemExit("contract source commit does not match verifier pin")
    if binding["release_binary_sha256"] != EXPECTED_RELEASE_BINARY_SHA256:
        raise SystemExit("contract release binary does not match verifier pin")
    if not isinstance(binding.get("release_binary_size_bytes"), int) or binding[
        "release_binary_size_bytes"
    ] <= 0:
        raise SystemExit("release binary size must be a positive integer")
    require_hex("registry root", binding["current_registry_root"], 96)
    require_hex("trust graph root", binding["trust_graph_root"], 96)

    gate = contract["topology_gate"]
    slots = contract["slots"]
    validator_ids = [slot["validator_id"] for slot in slots]
    expected_validators = [f"validator-{index}" for index in range(gate["validator_count"])]
    if validator_ids != expected_validators:
        raise SystemExit("validator slots must be complete, sorted, and unique")
    if len(slots) != gate["validator_count"]:
        raise SystemExit("validator count mismatch")
    if not 0 < gate["quorum"] <= len(slots):
        raise SystemExit("invalid quorum")
    if gate["required_operator_groups"] != len(slots):
        raise SystemExit("six-of-six topology must require one operator per validator")
    maximum = gate["maximum_validators_per_operator"]
    if maximum >= gate["quorum"]:
        raise SystemExit("an operator can reach quorum alone")
    if len(slots) - maximum < gate["quorum"]:
        raise SystemExit("an operator can halt quorum by withdrawing")

    receipt_contract = contract["receipt_contract"]
    if receipt_contract != {
        "manifest_schema": "postfiat-operator-manifest-v2",
        "control_attestation_schema": "postfiat-operator-control-attestation-v1",
        "signature_algorithm": "ML-DSA-65",
        "same_master_signer_required": True,
        "same_onboarding_challenge_required": True,
        "exclusive_control_required": True,
        "required_files": [
            "VALIDATOR_ID.provider-attestation.json",
            "VALIDATOR_ID.host-control-attestation.json",
            "VALIDATOR_ID.custody-attestation.json",
            "VALIDATOR_ID.operator-manifest.json",
        ],
    }:
        raise SystemExit("receipt contract is incomplete or unsupported")

    challenges = []
    trust_views = []
    for slot in slots:
        challenges.append(
            require_hex(
                f"{slot['validator_id']} onboarding challenge",
                slot["onboarding_challenge_id"],
                64,
            )
        )
        trust_views.append(
            require_hex(
                f"{slot['validator_id']} trust view id",
                slot["trust_view_id"],
                96,
            )
        )
        if slot.get("trust_view_version") != binding["trust_graph_version"]:
            raise SystemExit(f"{slot['validator_id']} trust view version mismatch")
    if len(set(challenges)) != len(challenges):
        raise SystemExit("onboarding challenge ids must be unique")
    if len(set(trust_views)) != len(trust_views):
        raise SystemExit("trust view ids must be unique")

    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    for value in (
        binding["section2_packet_root"],
        binding["source_commit"],
        binding["release_binary_sha256"],
        binding["trust_graph_root"],
    ):
        if value not in readme:
            raise SystemExit(f"README omits bound value {value}")
    for required_instruction in (
        "operator-attestation-create",
        "operator-attestation-verify",
        "--custody-attestation-hash",
        "--attestation-dir",
        "VALIDATOR_ID.custody-attestation.json",
    ):
        if required_instruction not in readme:
            raise SystemExit(f"README omits {required_instruction}")

    print(
        "contract-ok "
        f"validators={len(slots)} quorum={gate['quorum']} "
        f"operators={gate['required_operator_groups']} "
        f"max_per_operator={maximum} signed_receipts=3"
    )


if __name__ == "__main__":
    main()
