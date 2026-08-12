#!/usr/bin/env python3
"""Regression tests for NAV-aware A666 primary issue arithmetic and binding."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest


SCRIPT = Path(__file__).with_name("a666-mainnet-primary-issue-ops.py")
SPEC = importlib.util.spec_from_file_location("a666_primary_issue_ops", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"cannot load {SCRIPT}")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class PrimaryIssueArithmeticTests(unittest.TestCase):
    def test_current_nav_one_and_one_hundred_units(self) -> None:
        self.assertEqual(
            MODULE.derive_issue_amounts(1_000_000, 90_103_113, 10_050),
            (901_032, 905_538, 4_506),
        )
        self.assertEqual(
            MODULE.derive_issue_amounts(100_000_000, 90_103_113, 10_050),
            (90_103_113, 90_553_629, 450_516),
        )

    def test_nav_below_equal_and_above_one_dollar(self) -> None:
        self.assertEqual(
            MODULE.derive_issue_amounts(1_000_000, 50_000_000, 10_050),
            (500_000, 502_500, 2_500),
        )
        self.assertEqual(
            MODULE.derive_issue_amounts(1_000_000, 100_000_000, 10_050),
            (1_000_000, 1_005_000, 5_000),
        )
        self.assertEqual(
            MODULE.derive_issue_amounts(1_000_000, 125_000_000, 10_050),
            (1_250_000, 1_256_250, 6_250),
        )

    def test_rounding_boundaries_are_ceil_then_ceil(self) -> None:
        self.assertEqual(
            MODULE.derive_issue_amounts(1, 1, 10_000),
            (1, 1, 0),
        )
        self.assertEqual(
            MODULE.derive_issue_amounts(1, 100_000_001, 10_001),
            (2, 3, 1),
        )

    def test_invalid_amounts_and_multiplier_reject(self) -> None:
        for arguments in (
            (0, 100_000_000, 10_050),
            (1_000_000, 0, 10_050),
            (1_000_000, 100_000_000, 9_999),
        ):
            with self.subTest(arguments=arguments):
                with self.assertRaises(RuntimeError):
                    MODULE.derive_issue_amounts(*arguments)

    def test_exact_settlement_inverse_tracks_live_nav(self) -> None:
        mint = MODULE.derive_mint_for_exact_settlement(
            10_000_000, 90_248_000, 10_050
        )
        base, settlement, spread = MODULE.derive_issue_amounts(
            mint, 90_248_000, 10_050
        )
        self.assertEqual(mint, 11_025_449)
        self.assertEqual(settlement, 10_000_000)
        self.assertEqual(spread, settlement - base)
        self.assertGreater(
            MODULE.derive_issue_amounts(mint + 1, 90_248_000, 10_050)[1],
            settlement,
        )

    def test_exact_settlement_inverse_rejects_unreachable_value(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "cannot purchase one NAV atom"):
            MODULE.derive_mint_for_exact_settlement(2, 100_000_001, 20_000)


class NavBindingTests(unittest.TestCase):
    def setUp(self) -> None:
        self.status = {
            "native_nav_asset_id": "aa",
            "pricing_nav_epoch": 2,
            "pricing_reserve_packet_hash": "bb",
        }
        self.nav = {
            "schema": MODULE.LIVE_NAV_SCHEMA,
            "asset_id": "aa",
            "epoch": 2,
            "reserve_packet_hash": "bb",
            "opening_constants_used": False,
            "uniswap_price_used": False,
            "nav_per_unit_usd_1e8": 90_103_113,
            "circulating_supply_atoms": 31_590_197_455,
            "verified_net_assets_usd_1e8": 2_846_375_143_580,
        }

    def test_exact_binding_passes(self) -> None:
        self.assertEqual(
            MODULE.validate_nav_binding(self.status, self.nav),
            (90_103_113, 31_590_197_455, 2_846_375_143_580),
        )

    def test_epoch_packet_asset_or_price_source_mismatch_rejects(self) -> None:
        mutations = {
            "asset_id": "cc",
            "epoch": 3,
            "reserve_packet_hash": "dd",
            "opening_constants_used": True,
            "uniswap_price_used": True,
        }
        for field, value in mutations.items():
            with self.subTest(field=field):
                candidate = dict(self.nav)
                candidate[field] = value
                with self.assertRaises(RuntimeError):
                    MODULE.validate_nav_binding(self.status, candidate)


if __name__ == "__main__":
    unittest.main()
