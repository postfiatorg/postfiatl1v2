#!/usr/bin/env python3
"""Verify A666 supply transitions across an Ethereum burn and redemption."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--before", type=Path, required=True)
    parser.add_argument("--after", type=Path, required=True)
    parser.add_argument("--amount-atoms", type=int, required=True)
    return parser.parse_args()


def integer_field(document: dict[str, Any], name: str) -> int:
    value = document.get(name)
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ValueError(f"{name} must be a non-negative integer")
    return value


def validate(
    before: dict[str, Any],
    after: dict[str, Any],
    amount_atoms: int,
) -> dict[str, Any]:
    if amount_atoms <= 0:
        raise ValueError("amount-atoms must be positive")
    if before.get("invariant_holds") is not True:
        raise ValueError("before state does not hold the route invariant")
    if after.get("invariant_holds") is not True:
        raise ValueError("after state does not hold the route invariant")

    before_authorized = integer_field(before, "authorized_valid_supply_atoms")
    before_claims = integer_field(before, "outstanding_bridge_claims_atoms")
    before_ethereum = integer_field(before, "ethereum_spendable_supply_atoms")
    before_wrapped = integer_field(before, "wrapped_exposure_atoms")
    before_committed = integer_field(before, "committed_wrapped_exposure_atoms")

    after_authorized = integer_field(after, "authorized_valid_supply_atoms")
    after_claims = integer_field(after, "outstanding_bridge_claims_atoms")
    after_ethereum = integer_field(after, "ethereum_spendable_supply_atoms")
    after_wrapped = integer_field(after, "wrapped_exposure_atoms")
    after_committed = integer_field(after, "committed_wrapped_exposure_atoms")
    after_live = integer_field(after, "live_supply_sum_atoms")

    if before_ethereum < amount_atoms:
        raise ValueError("before state has insufficient Ethereum-spendable supply")
    if before_wrapped != before_claims + before_ethereum:
        raise ValueError("before wrapped exposure does not partition into claims and spendable")
    if before_committed != before_wrapped:
        raise ValueError("before committed wrapped exposure mismatch")

    expected = {
        "authorized_valid_supply_atoms": before_authorized - amount_atoms,
        "outstanding_bridge_claims_atoms": before_claims,
        "ethereum_spendable_supply_atoms": before_ethereum - amount_atoms,
        "wrapped_exposure_atoms": before_wrapped - amount_atoms,
        "committed_wrapped_exposure_atoms": before_committed - amount_atoms,
    }
    observed = {
        name: integer_field(after, name)
        for name in expected
    }
    if observed != expected:
        raise ValueError(
            f"roundtrip supply delta mismatch: expected {expected}, observed {observed}"
        )
    if after_claims != after_wrapped or after_committed != after_wrapped:
        raise ValueError("terminal bridge claims and wrapped exposure do not reconcile")
    if after_live != after_authorized:
        raise ValueError("terminal live supply does not equal authorized supply")
    if integer_field(after, "active_reservation_atoms") != 0:
        raise ValueError("terminal state retains active reservation atoms")
    if integer_field(after, "export_entitlement_atoms") != 0:
        raise ValueError("terminal state retains export entitlement atoms")
    return {
        "schema": "postfiat.a666.private_roundtrip_supply_check.v1",
        "verdict": "PASS",
        "amount_atoms": amount_atoms,
        "expected": expected,
        "observed": observed,
    }


def main() -> None:
    args = parse_args()
    try:
        before = json.loads(args.before.read_text())
        after = json.loads(args.after.read_text())
        if not isinstance(before, dict) or not isinstance(after, dict):
            raise ValueError("supply status documents must be objects")
        print(json.dumps(validate(before, after, args.amount_atoms), indent=2))
    except (OSError, json.JSONDecodeError, ValueError) as error:
        print(f"a666-private-roundtrip-supply-check: {error}", file=sys.stderr)
        raise SystemExit(1) from error


if __name__ == "__main__":
    main()
