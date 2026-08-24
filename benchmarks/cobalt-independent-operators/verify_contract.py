#!/usr/bin/env python3
"""Static verifier for the independent-operator onboarding contract."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
CONTRACT = ROOT / "onboarding-contract.json"


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
    if contract.get("schema") != "postfiat-cobalt-independent-operator-onboarding-v1":
        raise SystemExit("unsupported contract schema")

    binding = contract["evidence_binding"]
    require_hex("Section 2 packet root", binding["section2_packet_root"], 64)
    require_hex("source commit", binding["source_commit"], 40)
    require_hex("release binary SHA-256", binding["release_binary_sha256"], 64)
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

    print(
        "contract-ok "
        f"validators={len(slots)} quorum={gate['quorum']} "
        f"operators={gate['required_operator_groups']} "
        f"max_per_operator={maximum}"
    )


if __name__ == "__main__":
    main()
