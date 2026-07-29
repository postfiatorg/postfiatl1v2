#!/usr/bin/env python3
"""Build one exact transparent A666 primary redemption at the governed NAV."""

from __future__ import annotations

import argparse
import json
import os
import secrets
from pathlib import Path

JOE = "pfab9b9228942e5c529633a13aa271d5297bec6353"
JOE_KEY = "/home/postfiat/tmp/pfusdc-closed-roundtrip-20260720/keys/holder.json"
ROUTE_ID = "pftl-a666-ethereum-wA666-usdc-v1"
NAV_USD_E8_SCALE = 100_000_000
BPS_SCALE = 10_000


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--route-status", type=Path, required=True)
    parser.add_argument("--nav-manifest", type=Path, required=True)
    parser.add_argument("--nav-amount-atoms", type=int, required=True)
    parser.add_argument("--expires-at-height", type=int, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    return parser.parse_args()


def write_json(path: Path, value: object) -> None:
    if path.exists():
        raise RuntimeError(f"refusing to overwrite {path}")
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    os.chmod(path, 0o600)


def main() -> None:
    args = parse_args()
    if args.output_dir.exists():
        raise RuntimeError(f"refusing to overwrite {args.output_dir}")
    route = json.loads(args.route_status.read_text())
    nav = json.loads(args.nav_manifest.read_text())
    if route["route_id"] != ROUTE_ID:
        raise RuntimeError("route status is not the governed A666 route")
    if route["pricing_nav_epoch"] != nav["epoch"]:
        raise RuntimeError("route and NAV manifest pricing epochs differ")
    if route["pricing_reserve_packet_hash"] != nav["reserve_packet_hash"]:
        raise RuntimeError("route and NAV manifest reserve packets differ")
    if not route["min_order_atoms"] <= args.nav_amount_atoms <= route["max_order_atoms"]:
        raise RuntimeError("redemption amount is outside governed order bounds")
    if args.expires_at_height <= route["policy_valid_from_height"]:
        raise RuntimeError("redemption expiry is not after policy activation")

    base_value_numerator = args.nav_amount_atoms * nav["nav_per_unit_usd_1e8"]
    base_value_atoms = (
        base_value_numerator + NAV_USD_E8_SCALE - 1
    ) // NAV_USD_E8_SCALE
    settlement_output_atoms = (
        base_value_atoms * route["redeem_multiplier_bps"] // BPS_SCALE
    )
    if settlement_output_atoms <= 0:
        raise RuntimeError("redemption output rounds to zero")
    redemption_nonce = secrets.token_hex(32)
    body = {
        "operation": "pftl_uniswap_primary_redeem",
        "owner": JOE,
        "settlement_recipient": JOE,
        "route_id": ROUTE_ID,
        "redemption_nonce": redemption_nonce,
        "nav_amount_atoms": args.nav_amount_atoms,
        "min_settlement_value_atoms": settlement_output_atoms,
        "route_epoch": route["route_epoch"],
        "policy_epoch": route["policy_epoch"],
        "policy_hash": route["policy_hash"],
        "pricing_nav_epoch": route["pricing_nav_epoch"],
        "pricing_reserve_packet_hash": route["pricing_reserve_packet_hash"],
        "expires_at_height": args.expires_at_height,
    }
    request = {
        "schema": "postfiat-certified-asset-ops-request-v1",
        "operations": [
            {
                "label": "a666-variable-roundtrip-transparent-primary-redeem",
                "source": JOE,
                "key_file": JOE_KEY,
                "operation": body,
            }
        ],
    }
    args.output_dir.mkdir(parents=True, mode=0o700)
    write_json(args.output_dir / "primary-redeem.ops.json", request)
    manifest = {
        "schema": "postfiat.a666.transparent_primary_redeem.v1",
        "nav_amount_atoms": args.nav_amount_atoms,
        "nav_per_unit_usd_1e8": nav["nav_per_unit_usd_1e8"],
        "base_value_atoms": base_value_atoms,
        "base_value_rounding": "ceil",
        "settlement_output_atoms": settlement_output_atoms,
        "settlement_output_rounding": "floor",
        "redemption_nonce": redemption_nonce,
        "route_epoch": route["route_epoch"],
        "policy_epoch": route["policy_epoch"],
        "policy_hash": route["policy_hash"],
    }
    write_json(args.output_dir / "primary-redeem-manifest.json", manifest)
    print(json.dumps(manifest, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
