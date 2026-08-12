#!/usr/bin/env python3
"""Regression tests for the fail-closed A666 public NAV packet builder."""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


PROGRAM = Path(__file__).with_name("a666-build-live-nav-mark-ops.py")
ASSET_ID = (
    "521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b6"
    "2d20e18555642bec32174498cbee5e2c"
)
SETTLEMENT_ASSET_ID = (
    "02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c2"
    "33f6830bd5221fe2717fb6a1a7005d7b"
)
PROFILE_ID = "f8" * 48


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value), encoding="utf-8")


class LiveNavMarkBuilderTests(unittest.TestCase):
    def run_builder(
        self,
        *,
        nav_profile: str = PROFILE_ID,
        route_overrides: dict[str, object] | None = None,
    ) -> tuple[subprocess.CompletedProcess[str], Path]:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        proof_cli = root / "proof-cli"
        proof_cli.write_text(
            """#!/usr/bin/env python3
import json
from pathlib import Path
import sys

def value(flag):
    return sys.argv[sys.argv.index(flag) + 1]

if sys.argv[1:3] == ["packet", "prepare"]:
    output = Path(value("--output"))
    output.write_text(json.dumps({
        "schema": "postfiat.reserve_packet_template.v2",
        "issuer": value("--issuer"),
        "submitter": value("--submitter"),
        "nav_per_unit": 90000000,
        "circulating_supply": int(value("--circulating-supply")),
        "source_root": "44" * 48,
        "attestor_root": "55" * 48,
        "reserve_packet_hash": "66" * 48,
        "asset_precision": int(value("--asset-precision")),
        "subscription_overlay_source_root": value("--subscription-overlay-source-root"),
        "subscription_overlay_value": int(value("--subscription-overlay-value")),
    }))
elif sys.argv[1:3] == ["packet", "build"]:
    template = json.loads(Path(value("--template")).read_text())
    output = Path(value("--output"))
    output.write_text(json.dumps({
        "issuer": template["issuer"],
        "submitter": template["submitter"],
        "asset_id": "521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c",
        "epoch": 7,
        "nav_per_unit": template["nav_per_unit"],
        "circulating_supply": template["circulating_supply"],
        "verified_net_assets": 9000000000,
        "proof_profile": "f8" * 48,
        "source_root": template["source_root"],
        "attestor_root": template["attestor_root"],
        "reserve_packet_hash": template["reserve_packet_hash"],
        "reserve_accounts": [],
        "sp1_proof_bytes": [1],
        "sp1_public_values": [0] * 584,
    }))
else:
    raise SystemExit("unexpected command")
""",
            encoding="utf-8",
        )
        proof_cli.chmod(0o755)

        paths = {name: root / f"{name}.json" for name in (
            "public-values", "proof-calldata", "route", "vault", "nav", "status"
        )}
        paths["public-values"].write_bytes(b"public-values")
        paths["proof-calldata"].write_bytes(b"proof")
        route = {
            "route_id": "pftl-a666-ethereum-wA666-usdc-v1",
            "route_config_digest": "11" * 48,
            "native_nav_asset_id": ASSET_ID,
            "settlement_asset_id": SETTLEMENT_ASSET_ID,
            "settlement_reserve_atoms": 10,
            "live_value_enabled": True,
            "paused": True,
            "invariant_holds": True,
        }
        route.update(route_overrides or {})
        write_json(paths["route"], route)
        write_json(
            paths["vault"],
            {
                "asset_id": SETTLEMENT_ASSET_ID,
                "valuation_unit": "USDC",
                "buckets": [
                    {
                        "bucket_id": "bucket-1",
                        "asset_id": SETTLEMENT_ASSET_ID,
                        "status": "active",
                        "outstanding_vault_bridge_atoms": 10,
                    }
                ],
                "receipts": [],
                "allocations": [],
            },
        )
        write_json(
            paths["nav"],
            {
                "asset_id": ASSET_ID,
                "proof_profile": nav_profile,
                "valuation_unit": "USD_1E8",
                "issued_supply_atoms": 100,
            },
        )
        write_json(
            paths["status"],
            {
                "genesis_hash": "ce" * 48,
                "block_height": 784,
                "state_root": "77" * 48,
                "active_nav_profiles": [
                    {
                        "asset_id": ASSET_ID,
                        "profile_id": PROFILE_ID,
                        "verifier_kind": "sp1-nav-reserve-v1",
                        "public_values_schema": "postfiat.nav_reserve_public_values.v1",
                        "halted": False,
                        "finalized_epoch": 5,
                        "max_proof_bytes": 1024,
                        "max_public_values_bytes": 1024,
                        "source_manifest_hash": "88" * 48,
                        "valuation_policy_hash": "99" * 32,
                        "sp1_program_vkey": "0x" + "aa" * 32,
                    }
                ],
            },
        )
        issuer_key = root / "issuer.key"
        reserve_key = root / "reserve.key"
        issuer_key.write_text("fixture", encoding="utf-8")
        reserve_key.write_text("fixture", encoding="utf-8")
        output = root / "output"
        result = subprocess.run(
            [
                sys.executable,
                str(PROGRAM),
                "--public-values", str(paths["public-values"]),
                "--proof-calldata", str(paths["proof-calldata"]),
                "--proof-cli", str(proof_cli),
                "--route-status", str(paths["route"]),
                "--settlement-vault-status", str(paths["vault"]),
                "--nav-status", str(paths["nav"]),
                "--pftl-status", str(paths["status"]),
                "--issuer-key-file", str(issuer_key),
                "--reserve-key-file", str(reserve_key),
                "--output-dir", str(output),
            ],
            text=True,
            capture_output=True,
            check=False,
        )
        return result, output

    def test_derives_overlay_supply_packet_and_operations(self) -> None:
        result, output = self.run_builder()
        self.assertEqual(result.returncode, 0, result.stderr)
        template = json.loads((output / "reserve-packet-template.v2.json").read_text())
        self.assertEqual(template["circulating_supply"], 100)
        self.assertEqual(template["asset_precision"], 6)
        self.assertEqual(template["subscription_overlay_value"], 1_000)
        self.assertEqual(len(template["subscription_overlay_source_root"]), 96)

    def test_excludes_unbacked_primary_reserve_from_nav_overlay(self) -> None:
        result, output = self.run_builder(
            route_overrides={"settlement_reserve_atoms": 12}
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        overlay = json.loads(
            (output / "finalized-subscription-overlay.json").read_text()
        )["overlay"]
        row = overlay["primary_market_rows"][0]
        self.assertEqual(row["reported_settlement_reserve_atoms"], 12)
        self.assertEqual(row["settlement_reserve_atoms"], 10)
        self.assertEqual(row["excluded_unbacked_reserve_atoms"], 2)
        self.assertEqual(overlay["value_nav_units"], 1_000)
        self.assertTrue((output / "01-reserve-submit.ops.json").is_file())
        self.assertTrue((output / "02-epoch-finalize.ops.json").is_file())
        manifest = json.loads((output / "live-nav-mark-manifest.json").read_text())
        self.assertEqual(manifest["subscription_overlay"]["value_nav_units"], 1_000)

    def test_rejects_nav_status_from_a_different_profile(self) -> None:
        result, _ = self.run_builder(nav_profile="aa" * 48)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("NAV status and active profile disagree", result.stderr)

    def test_rejects_unpaused_route(self) -> None:
        result, _ = self.run_builder(route_overrides={"paused": False})
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("route must be paused", result.stderr)

    def test_rejects_wrong_settlement_asset(self) -> None:
        result, _ = self.run_builder(
            route_overrides={"settlement_asset_id": "02" * 48}
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("not the governed pfUSDC asset", result.stderr)

    def test_rejects_wrong_route(self) -> None:
        result, _ = self.run_builder(route_overrides={"route_id": "other-route"})
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("governed A666 route", result.stderr)

    def test_rejects_malformed_route_config_digest(self) -> None:
        result, _ = self.run_builder(route_overrides={"route_config_digest": "11"})
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("route config digest is malformed", result.stderr)

    def test_rejects_broken_supply_invariant(self) -> None:
        result, _ = self.run_builder(route_overrides={"invariant_holds": False})
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("supply invariant does not hold", result.stderr)

    def test_rejects_live_value_disabled(self) -> None:
        result, _ = self.run_builder(
            route_overrides={"live_value_enabled": False}
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("live-value mode is not enabled", result.stderr)


if __name__ == "__main__":
    unittest.main()
