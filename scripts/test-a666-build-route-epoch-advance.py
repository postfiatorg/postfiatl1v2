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
ASSET_ID = (
    "521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b6"
    "2d20e18555642bec32174498cbee5e2c"
)
SETTLEMENT_ASSET_ID = (
    "02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c2"
    "33f6830bd5221fe2717fb6a1a7005d7b"
)
PROFILE_ID = (
    "f8784629ff7338002d836c1988b8e2c0f19caf448429e0eb7fdc39fa2b08f7d9a"
    "44171fc1e7239bc25e06ad833c14e91"
)
SOURCE_MANIFEST_HASH = (
    "8abe3e59198b72945d4778a7fa91e5af157a6c65032d8940cca486850ffe59fcb"
    "567268ca5942669ff6977ef32dd3a41"
)
VALUATION_POLICY_HASH = (
    "350eaee0a1ca12ba51637781ba52661b8685f868657a7c5e7d07c31b2899869c"
)
SP1_PROGRAM_VKEY = (
    "0x00f3857f96ef97e00bd15b4030acd8d6b0a72740b28c6160d154bc2c9bb141bf"
)


def nav_mark(*, asset_id: str = ASSET_ID, profile_id: str = PROFILE_ID) -> dict[str, object]:
    return {
        "schema": "postfiat.a666.provider_neutral_nav_mark.v1",
        "asset_id": asset_id,
        "prior_epoch": 6,
        "epoch": 7,
        "profile_id": profile_id,
        "source_manifest_hash": SOURCE_MANIFEST_HASH,
        "valuation_policy_hash": VALUATION_POLICY_HASH,
        "program_vkey": SP1_PROGRAM_VKEY,
        "reserve_packet_hash": "22" * 48,
        "nav_per_unit": 100_000_000,
    }


def route(*, paused: bool, live_value_enabled: bool = True) -> dict[str, object]:
    return {
        "route_id": "pftl-a666-ethereum-wA666-usdc-v1",
        "native_nav_asset_id": ASSET_ID,
        "settlement_asset_id": SETTLEMENT_ASSET_ID,
        "invariant_holds": True,
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
        nav = nav_mark()
        nav["prior_epoch"] = nav_prior_epoch
        nav["epoch"] = nav_epoch
        nav_path.write_text(json.dumps(nav))
        issuer_key = root / "issuer-key.json"
        issuer_key.write_text("fixture")
        return subprocess.run(
            [
                sys.executable,
                str(PROGRAM),
                "--route-status",
                str(route_path),
                "--nav-manifest",
                str(nav_path),
                "--issuer-key-file",
                str(issuer_key),
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

    def test_unpaused_live_route_is_rejected(self) -> None:
        result = self.run_builder(route(paused=False))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("must be paused", result.stderr)

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

    def test_wrong_native_asset_is_rejected(self) -> None:
        status = route(paused=True)
        status["native_nav_asset_id"] = "00" * 48
        result = self.run_builder(status)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("native asset is not A666", result.stderr)

    def test_wrong_settlement_asset_is_rejected(self) -> None:
        status = route(paused=True)
        status["settlement_asset_id"] = "00" * 48
        result = self.run_builder(status)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("not the governed pfUSDC asset", result.stderr)

    def test_broken_supply_invariant_is_rejected(self) -> None:
        status = route(paused=True)
        status["invariant_holds"] = False
        result = self.run_builder(status)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("supply invariant does not hold", result.stderr)

    def test_malformed_route_config_digest_is_rejected(self) -> None:
        status = route(paused=True)
        status["route_config_digest"] = "11"
        result = self.run_builder(status)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("route config digest is malformed", result.stderr)

    def test_wrong_nav_asset_is_rejected(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        route_path = root / "route.json"
        nav_path = root / "nav.json"
        key_path = root / "issuer-key.json"
        route_path.write_text(json.dumps(route(paused=True)))
        nav_path.write_text(json.dumps(nav_mark(asset_id="00" * 48)))
        key_path.write_text("fixture")
        result = subprocess.run(
            [
                sys.executable,
                str(PROGRAM),
                "--route-status",
                str(route_path),
                "--nav-manifest",
                str(nav_path),
                "--issuer-key-file",
                str(key_path),
                "--valid-from-height",
                "800",
                "--output-dir",
                str(root / "output"),
            ],
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("does not describe A666", result.stderr)

    def test_wrong_successor_profile_is_rejected(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        route_path = root / "route.json"
        nav_path = root / "nav.json"
        key_path = root / "issuer-key.json"
        route_path.write_text(json.dumps(route(paused=True)))
        nav_path.write_text(json.dumps(nav_mark(profile_id="00" * 48)))
        key_path.write_text("fixture")
        result = subprocess.run(
            [
                sys.executable,
                str(PROGRAM),
                "--route-status",
                str(route_path),
                "--nav-manifest",
                str(nav_path),
                "--issuer-key-file",
                str(key_path),
                "--valid-from-height",
                "800",
                "--output-dir",
                str(root / "output"),
            ],
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("pinned successor profile", result.stderr)


if __name__ == "__main__":
    unittest.main()
