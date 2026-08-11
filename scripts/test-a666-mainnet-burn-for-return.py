#!/usr/bin/env python3
"""Unit tests for proof-gated A666 return-burn capacity checks."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("a666-mainnet-burn-for-return.py")
SPEC = importlib.util.spec_from_file_location("a666_mainnet_burn_for_return", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
burn = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(burn)


def status(**updates: object) -> dict[str, object]:
    value: dict[str, object] = {
        "schema": "postfiat-pftl-uniswap-supply-status-v2",
        "route_id": burn.ROUTE_ID,
        "handoff_controller": burn.CONTROLLER.lower(),
        "wrapped_navcoin_token": burn.TOKEN.lower(),
        "native_nav_asset_id": burn.A666,
        "live_value_enabled": True,
        "paused": False,
        "invariant_holds": True,
        "ethereum_spendable_supply_atoms": 9_096_189,
        "available_return_import_atoms": 9_096_189,
    }
    value.update(updates)
    return value


class ReturnCapacityTests(unittest.TestCase):
    def validate(self, value: object, amount: int = 1_000_000) -> dict[str, object]:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "supply-status.json"
            path.write_text(json.dumps(value))
            return burn.validate_return_capacity(path, amount)

    def test_accepts_exact_capacity_boundary(self) -> None:
        value = status(
            ethereum_spendable_supply_atoms=13_571_391,
            available_return_import_atoms=13_571_391,
        )
        self.assertEqual(
            self.validate(value, 13_571_391)["available_return_import_atoms"],
            13_571_391,
        )

    def test_rejects_burn_larger_than_acknowledged_ethereum_supply(self) -> None:
        with self.assertRaisesRegex(
            RuntimeError,
            "requested 13571391 atoms, available 9096189 atoms",
        ):
            self.validate(status(), 13_571_391)

    def test_rejects_paused_or_misbound_status(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "route is not active"):
            self.validate(status(paused=True))
        with self.assertRaisesRegex(RuntimeError, "route_id binding mismatch"):
            self.validate(status(route_id="wrong-route"))

    def test_legacy_status_uses_ethereum_spendable_capacity(self) -> None:
        value = status()
        del value["available_return_import_atoms"]
        self.assertEqual(
            self.validate(value)["ethereum_spendable_supply_atoms"],
            9_096_189,
        )


if __name__ == "__main__":
    unittest.main()
