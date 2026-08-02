#!/usr/bin/env python3
"""Regression tests for safe A666 route migration while paused."""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


PROGRAM = Path(__file__).with_name("a666-build-route-epoch-advance.py")


def route(*, paused: bool, live_value_enabled: bool = True) -> dict[str, object]:
    return {
        "route_id": "pftl-a666-ethereum-wA666-usdc-v1",
        "paused": paused,
        "live_value_enabled": live_value_enabled,
        "active_reservation_count": 0,
        "export_entitlement_count": 0,
        "pricing_nav_epoch": 6,
        "policy_epoch": 9,
        "issue_multiplier_bps": 10_050,
        "redeem_multiplier_bps": 9_995,
        "max_order_atoms": 2_000_000_000_000,
        "min_order_atoms": 1,
        "policy_expires_at_height": 10_000,
        "max_nav_age_blocks": 64,
        "route_epoch": 8,
        "route_config_digest": "11" * 48,
    }


class RouteEpochAdvanceTests(unittest.TestCase):
    def run_builder(
        self,
        route_status: dict[str, object],
        *,
        nav_epoch: int = 7,
        nav_prior_epoch: int = 6,
    ) -> subprocess.CompletedProcess[str]:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        route_path = root / "route.json"
        nav_path = root / "nav.json"
        route_path.write_text(json.dumps(route_status))
        nav_path.write_text(
            json.dumps(
                {
                    "prior_epoch": nav_prior_epoch,
                    "epoch": nav_epoch,
                    "reserve_packet_hash": "22" * 48,
                    "nav_per_unit_usd_1e8": 100_000_000,
                }
            )
        )
        return subprocess.run(
            [
                sys.executable,
                str(PROGRAM),
                "--route-status",
                str(route_path),
                "--nav-manifest",
                str(nav_path),
                "--valid-from-height",
                "800",
                "--output-dir",
                str(root / "output"),
            ],
            text=True,
            capture_output=True,
            check=False,
        )

    def test_paused_live_route_can_advance_during_migration(self) -> None:
        result = self.run_builder(route(paused=True))
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_unpaused_live_route_remains_supported(self) -> None:
        result = self.run_builder(route(paused=False))
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_route_without_live_value_cannot_advance(self) -> None:
        result = self.run_builder(route(paused=True, live_value_enabled=False))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("must have live value enabled", result.stderr)

    def test_active_reservation_still_blocks_advance(self) -> None:
        status = route(paused=True)
        status["active_reservation_count"] = 1
        result = self.run_builder(status)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("active order state", result.stderr)

    def test_shadow_epoch_gap_can_advance_from_exact_prior_route_epoch(self) -> None:
        result = self.run_builder(route(paused=True), nav_epoch=9)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_mismatched_prior_route_epoch_is_rejected(self) -> None:
        result = self.run_builder(route(paused=True), nav_prior_epoch=5)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("prior epoch does not match", result.stderr)


if __name__ == "__main__":
    unittest.main()
