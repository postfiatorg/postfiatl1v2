#!/usr/bin/env python3
"""Regression tests for A666 wrapper/claim accounting on redemption."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest


PROGRAM = Path(__file__).with_name("a666-private-roundtrip-supply-check.py")


def load_module():
    spec = importlib.util.spec_from_file_location("supply_check", PROGRAM)
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load supply check")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def status(
    *,
    authorized: int,
    claims: int,
    ethereum: int,
    wrapped: int,
) -> dict[str, object]:
    return {
        "invariant_holds": True,
        "authorized_valid_supply_atoms": authorized,
        "outstanding_bridge_claims_atoms": claims,
        "ethereum_spendable_supply_atoms": ethereum,
        "wrapped_exposure_atoms": wrapped,
        "committed_wrapped_exposure_atoms": wrapped,
        "live_supply_sum_atoms": authorized,
        "active_reservation_atoms": 0,
        "export_entitlement_atoms": 0,
    }


class SupplyCheckTests(unittest.TestCase):
    def setUp(self) -> None:
        self.module = load_module()
        self.before = status(
            authorized=31_490_197_455,
            claims=31_489_197_455,
            ethereum=1_000_000,
            wrapped=31_490_197_455,
        )
        self.after = status(
            authorized=31_489_197_455,
            claims=31_489_197_455,
            ethereum=0,
            wrapped=31_489_197_455,
        )

    def test_claims_remain_constant_after_spendable_wrapper_is_redeemed(self) -> None:
        result = self.module.validate(self.before, self.after, 1_000_000)
        self.assertEqual(result["verdict"], "PASS")
        self.assertEqual(
            result["observed"]["outstanding_bridge_claims_atoms"],
            self.before["outstanding_bridge_claims_atoms"],
        )

    def test_rejects_double_decrement_of_outstanding_claims(self) -> None:
        self.after["outstanding_bridge_claims_atoms"] = 31_488_197_455
        with self.assertRaisesRegex(ValueError, "supply delta mismatch"):
            self.module.validate(self.before, self.after, 1_000_000)

    def test_rejects_wrapper_supply_not_retired(self) -> None:
        self.after["wrapped_exposure_atoms"] = 31_490_197_455
        self.after["committed_wrapped_exposure_atoms"] = 31_490_197_455
        with self.assertRaisesRegex(ValueError, "supply delta mismatch"):
            self.module.validate(self.before, self.after, 1_000_000)


if __name__ == "__main__":
    unittest.main()
