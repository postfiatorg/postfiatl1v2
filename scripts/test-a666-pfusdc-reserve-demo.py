#!/usr/bin/env python3
"""Tests for the narrow A666/pfUSDC reserve demonstration driver."""

from __future__ import annotations

import importlib.util
import json
from argparse import Namespace
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("a666-pfusdc-reserve-demo.py")
SPEC = importlib.util.spec_from_file_location("a666_pfusdc_reserve_demo", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
demo = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(demo)


PFUSDC = (
    "02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c"
    "233f6830bd5221fe2717fb6a1a7005d7b"
)


def dump(path: Path, value: object) -> Path:
    path.write_text(json.dumps(value))
    return path


def route(**updates: object) -> dict[str, object]:
    value: dict[str, object] = {
        "schema": "postfiat-pftl-uniswap-supply-status-v2",
        "route_id": demo.ROUTE_ID,
        "native_nav_asset_id": demo.A666_ASSET_ID,
        "settlement_asset_id": PFUSDC,
        "route_config_digest": "12" * 48,
        "live_value_enabled": True,
        "paused": False,
        "invariant_holds": True,
        "route_schema_version": 2,
        "outbound_verification_class": "TRUSTLESS_FINALITY",
        "return_verification_class": "BFT_CHECKPOINT",
        "ethereum_chain_id": 1,
        "route_epoch": 3,
        "policy_epoch": 3,
        "policy_hash": "34" * 48,
        "issue_multiplier_bps": 10050,
        "redeem_multiplier_bps": 9995,
        "max_order_atoms": 1_000_000_000_000,
        "min_order_atoms": 1_000_000,
        "policy_expires_at_height": 10_000,
        "pricing_nav_epoch": 2,
        "pricing_reserve_packet_hash": "56" * 48,
        "available_issue_atoms": 1_000_000_000_000,
        "available_redeem_atoms": 1_000_000_000_000,
        "redeem_capacity_remaining_atoms": 1_000_000_000_000,
        "authorized_valid_supply_atoms": 31_489_197_455,
        "pftl_spendable_supply_atoms": 0,
        "ethereum_spendable_supply_atoms": 0,
        "other_registered_venue_supply_atoms": 0,
        "outstanding_bridge_claims_atoms": 31_489_197_455,
        "settlement_reserve_atoms": 112_995_855,
        "non_nav_spread_atoms": 1_176_186,
        "active_reservation_count": 0,
        "active_reservation_atoms": 0,
        "export_entitlement_count": 0,
        "export_entitlement_atoms": 0,
    }
    value.update(updates)
    return value


def nav(nav_per_unit: int = 90_103_113, epoch: int = 2) -> dict[str, object]:
    return {
        "schema": "postfiat.a666.live_nav_mark.v1",
        "asset_id": demo.A666_ASSET_ID,
        "epoch": epoch,
        "reserve_packet_hash": "56" * 48,
        "nav_per_unit_usd_1e8": nav_per_unit,
        "circulating_supply_atoms": 31_489_197_455,
        "verified_net_assets_usd_1e8": 2_846_375_143_580,
        "opening_constants_used": False,
        "uniswap_price_used": False,
    }


def balance(asset_id: str, atoms: int) -> dict[str, object]:
    assets = []
    if atoms:
        assets.append({"asset_id": asset_id, "balance": atoms})
    return {
        "schema": "postfiat-account-assets-v1",
        "asset_id": asset_id,
        "assets": assets,
    }


class ReserveDemoTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.key = dump(self.root / "holder.json", {"test": True})

    def tearDown(self) -> None:
        self.temp.cleanup()

    def build_issue(self, amount: int = 100_000_000) -> tuple[Path, dict[str, object]]:
        route_file = dump(self.root / "route.json", route())
        nav_file = dump(self.root / "nav.json", nav())
        output = self.root / "issue"
        result = demo.cmd_build_issue(
            Namespace(
                route_status=route_file,
                nav_manifest=nav_file,
                holder_key_file=self.key,
                output_dir=output,
                mint_amount_atoms=amount,
                current_height=528,
                reservation_ttl_blocks=128,
                subscriber=demo.DEFAULT_SUBSCRIBER,
                ethereum_recipient=demo.DEFAULT_ETHEREUM_RECIPIENT,
            )
        )
        return output, result

    def test_build_issue_has_no_export_and_binds_release(self) -> None:
        output, manifest = self.build_issue()
        self.assertFalse(manifest["creates_ethereum_export"])
        self.assertFalse((output / "03-export.ops.json").exists())
        self.assertFalse((output / "03-mint-packet.json").exists())
        reserve = json.loads((output / "01-reserve.ops.json").read_text())
        subscribe = json.loads((output / "02-subscribe.ops.json").read_text())
        release = json.loads((output / "03-release-entitlement.ops.json").read_text())
        reservation_ids = {
            reserve["operations"][0]["operation"]["reservation_id"],
            subscribe["operations"][0]["operation"]["reservation_id"],
            release["operations"][0]["operation"]["reservation_id"],
        }
        self.assertEqual(len(reservation_ids), 1)
        self.assertEqual(manifest["base_value_atoms"], 90_103_113)
        self.assertEqual(manifest["settlement_value_atoms"], 90_553_629)
        self.assertEqual(manifest["issue_spread_atoms"], 450_516)
        with self.assertRaises(demo.DemoError):
            self.build_issue()

    def test_build_expired_releases_rejects_live_entitlement(self) -> None:
        rows = [
            {
                "reservation_id": "ab" * 48,
                "subscriber": demo.DEFAULT_SUBSCRIBER,
                "remaining_amount_atoms": 1_000_000,
                "expires_at_height": 527,
            }
        ]
        entitlements = dump(self.root / "entitlements.json", rows)
        result = demo.cmd_build_expired_releases(
            Namespace(
                entitlements_file=entitlements,
                holder_key_file=self.key,
                output_dir=self.root / "cleanup",
                current_height=528,
                releaser=demo.DEFAULT_SUBSCRIBER,
            )
        )
        self.assertEqual(result["entitlement_count"], 1)
        with self.assertRaises(demo.DemoError):
            demo.cmd_build_expired_releases(
                Namespace(
                    entitlements_file=entitlements,
                    holder_key_file=self.key,
                    output_dir=self.root / "cleanup-live",
                    current_height=527,
                    releaser=demo.DEFAULT_SUBSCRIBER,
                )
            )

    def test_cleanup_verification_rejects_economic_change(self) -> None:
        manifest = dump(
            self.root / "cleanup-manifest.json",
            {
                "schema": "postfiat.a666.expired_export_entitlement_cleanup.v1",
                "entitlement_count": 2,
                "entitlement_atoms": 2_000_000,
            },
        )
        before = dump(
            self.root / "before-route.json",
            route(export_entitlement_count=2, export_entitlement_atoms=2_000_000),
        )
        after = dump(self.root / "after-route.json", route())
        report = demo.cmd_verify_expired_releases(
            Namespace(
                before_route=before,
                after_route=after,
                cleanup_manifest=manifest,
                output=self.root / "cleanup-verify.json",
            )
        )
        self.assertEqual(report["verdict"], "PASS")
        bad_after = dump(
            self.root / "bad-after-route.json",
            route(settlement_reserve_atoms=112_995_854),
        )
        with self.assertRaises(demo.DemoError):
            demo.cmd_verify_expired_releases(
                Namespace(
                    before_route=before,
                    after_route=bad_after,
                    cleanup_manifest=manifest,
                    output=self.root / "bad-cleanup-verify.json",
                )
            )

    def test_issue_verification_tracks_reserve_and_balances(self) -> None:
        issue_dir, manifest = self.build_issue()
        amount = manifest["mint_amount_atoms"]
        base = manifest["base_value_atoms"]
        settlement = manifest["settlement_value_atoms"]
        spread = manifest["issue_spread_atoms"]
        before_route = route()
        subscribed = route(
            authorized_valid_supply_atoms=before_route[
                "authorized_valid_supply_atoms"
            ]
            + amount,
            pftl_spendable_supply_atoms=amount,
            settlement_reserve_atoms=before_route["settlement_reserve_atoms"] + base,
            non_nav_spread_atoms=before_route["non_nav_spread_atoms"] + spread,
            export_entitlement_count=1,
            export_entitlement_atoms=amount,
        )
        released = dict(subscribed)
        released["export_entitlement_count"] = 0
        released["export_entitlement_atoms"] = 0
        files = {
            "before_route": dump(self.root / "i-before-route.json", before_route),
            "after_subscribe_route": dump(
                self.root / "i-subscribed-route.json", subscribed
            ),
            "after_release_route": dump(
                self.root / "i-released-route.json", released
            ),
            "before_pfusdc": dump(
                self.root / "i-before-pfusdc.json", balance(PFUSDC, settlement)
            ),
            "after_subscribe_pfusdc": dump(
                self.root / "i-subscribed-pfusdc.json", balance(PFUSDC, 0)
            ),
            "after_release_pfusdc": dump(
                self.root / "i-released-pfusdc.json", balance(PFUSDC, 0)
            ),
            "before_a666": dump(
                self.root / "i-before-a666.json", balance(demo.A666_ASSET_ID, 0)
            ),
            "after_subscribe_a666": dump(
                self.root / "i-subscribed-a666.json",
                balance(demo.A666_ASSET_ID, amount),
            ),
            "after_release_a666": dump(
                self.root / "i-released-a666.json",
                balance(demo.A666_ASSET_ID, amount),
            ),
        }
        report = demo.cmd_verify_issue(
            Namespace(
                **files,
                issue_manifest=issue_dir / "issue-manifest.json",
                output=self.root / "issue-verify.json",
            )
        )
        self.assertEqual(report["verdict"], "PASS")
        bad_release = dict(released)
        bad_release["settlement_reserve_atoms"] += base
        files["after_release_route"] = dump(
            self.root / "i-bad-released-route.json", bad_release
        )
        with self.assertRaises(demo.DemoError):
            demo.cmd_verify_issue(
                Namespace(
                    **files,
                    issue_manifest=issue_dir / "issue-manifest.json",
                    output=self.root / "bad-issue-verify.json",
                )
            )

    def test_partial_redeem_leaves_supply_and_same_run_reserve(self) -> None:
        issue_dir, issue = self.build_issue()
        fresh_nav = nav(nav_per_unit=90_200_000, epoch=3)
        fresh_nav["reserve_packet_hash"] = "78" * 48
        advanced = route(
            route_epoch=4,
            policy_epoch=4,
            policy_hash="9a" * 48,
            pricing_nav_epoch=3,
            pricing_reserve_packet_hash="78" * 48,
            authorized_valid_supply_atoms=route()["authorized_valid_supply_atoms"]
            + issue["mint_amount_atoms"],
            pftl_spendable_supply_atoms=issue["mint_amount_atoms"],
            settlement_reserve_atoms=route()["settlement_reserve_atoms"]
            + issue["base_value_atoms"],
            non_nav_spread_atoms=route()["non_nav_spread_atoms"]
            + issue["issue_spread_atoms"],
            available_redeem_atoms=issue["mint_amount_atoms"],
        )
        advanced_file = dump(self.root / "advanced.json", advanced)
        fresh_nav_file = dump(self.root / "fresh-nav.json", fresh_nav)
        redeem_dir = self.root / "redeem"
        redeem = demo.cmd_build_redeem(
            Namespace(
                route_status=advanced_file,
                nav_manifest=fresh_nav_file,
                issue_manifest=issue_dir / "issue-manifest.json",
                holder_key_file=self.key,
                output_dir=redeem_dir,
                current_height=534,
                expiry_ttl_blocks=128,
                nav_amount_atoms=1_000_000,
                owner=demo.DEFAULT_SUBSCRIBER,
            )
        )
        self.assertEqual(redeem["nav_amount_atoms"], 1_000_000)
        self.assertEqual(redeem["retained_a666_atoms"], 99_000_000)
        self.assertGreater(redeem["retained_same_run_reserve_atoms"], 0)
        self.assertLessEqual(
            redeem["base_value_atoms"], issue["base_value_atoms"]
        )

        final_route = dict(advanced)
        final_route["authorized_valid_supply_atoms"] -= redeem["nav_amount_atoms"]
        final_route["pftl_spendable_supply_atoms"] -= redeem["nav_amount_atoms"]
        final_route["settlement_reserve_atoms"] -= redeem["base_value_atoms"]
        final_route["non_nav_spread_atoms"] += redeem["redemption_spread_atoms"]
        before_pfusdc = 10
        before_a666 = issue["mint_amount_atoms"]
        report = demo.cmd_verify_redeem(
            Namespace(
                before_route=advanced_file,
                after_route=dump(self.root / "final-route.json", final_route),
                before_pfusdc=dump(
                    self.root / "r-before-pfusdc.json",
                    balance(PFUSDC, before_pfusdc),
                ),
                after_pfusdc=dump(
                    self.root / "r-after-pfusdc.json",
                    balance(
                        PFUSDC,
                        before_pfusdc + redeem["settlement_output_atoms"],
                    ),
                ),
                before_a666=dump(
                    self.root / "r-before-a666.json",
                    balance(demo.A666_ASSET_ID, before_a666),
                ),
                after_a666=dump(
                    self.root / "r-after-a666.json",
                    balance(
                        demo.A666_ASSET_ID,
                        before_a666 - redeem["nav_amount_atoms"],
                    ),
                ),
                redeem_manifest=redeem_dir / "redeem-manifest.json",
                output=self.root / "redeem-verify.json",
            )
        )
        self.assertEqual(report["verdict"], "PASS")
        self.assertEqual(report["retained_a666_atoms"], 99_000_000)


if __name__ == "__main__":
    unittest.main()
