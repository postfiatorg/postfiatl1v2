#!/usr/bin/env python3
"""Build the A666 primary-market route epoch that prices from a fresh NAV mark."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
from typing import Any

ISSUER = "pffcb93d9f87a843a8aa34e1adf241f5d58143e81b"
ROUTE_ID = "pftl-a666-ethereum-wA666-usdc-v1"
ASSET_ID = (
    "521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b6"
    "2d20e18555642bec32174498cbee5e2c"
)
NAV_MARK_SCHEMA = "postfiat.a666.provider_neutral_nav_mark.v1"
SUCCESSOR_PROFILE_ID = (
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
POLICY_HASH_DOMAIN = "postfiat.pftl_uniswap.primary_market_policy.v2"
ISSUE_CAPACITY_ATOMS = 2_000_000_000_000
REDEEM_CAPACITY_ATOMS = 2_000_000_000_000


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--route-status", type=Path, required=True)
    parser.add_argument("--nav-manifest", type=Path, required=True)
    parser.add_argument("--issuer-key-file", type=Path, required=True)
    parser.add_argument("--valid-from-height", type=int, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    return parser.parse_args()


def hash_domain(domain: str, payload: bytes) -> str:
    return hashlib.sha3_384(domain.encode() + b"\0" + payload).hexdigest()


def write_json(path: Path, value: object) -> None:
    if path.exists():
        raise RuntimeError(f"refusing to overwrite {path}")
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    os.chmod(path, 0o600)


def policy_hash(policy: dict[str, Any]) -> str:
    preimage = (
        f"policy_epoch={policy['policy_epoch']}\n"
        f"issue_multiplier_bps={policy['issue_multiplier_bps']}\n"
        f"redeem_multiplier_bps={policy['redeem_multiplier_bps']}\n"
        f"issue_capacity_atoms={policy['issue_capacity_atoms']}\n"
        f"redeem_capacity_atoms={policy['redeem_capacity_atoms']}\n"
        f"max_order_atoms={policy['max_order_atoms']}\n"
        f"min_order_atoms={policy['min_order_atoms']}\n"
        f"valid_from_height={policy['valid_from_height']}\n"
        f"expires_at_height={policy['expires_at_height']}\n"
        f"max_nav_age_blocks={policy['max_nav_age_blocks']}\n"
        f"pricing_nav_epoch={policy['pricing_nav_epoch']}\n"
        f"pricing_reserve_packet_hash={policy['pricing_reserve_packet_hash']}\n"
    )
    return hash_domain(POLICY_HASH_DOMAIN, preimage.encode())


def main() -> None:
    args = parse_args()
    if args.output_dir.exists():
        raise RuntimeError(f"refusing to overwrite {args.output_dir}")
    if not args.issuer_key_file.is_file():
        raise RuntimeError("issuer key file is unavailable")
    if args.valid_from_height <= 0:
        raise RuntimeError("--valid-from-height must be positive")

    route = json.loads(args.route_status.read_text())
    nav = json.loads(args.nav_manifest.read_text())
    if route["route_id"] != ROUTE_ID:
        raise RuntimeError("route status is not the governed A666 route")
    if route.get("native_nav_asset_id") != ASSET_ID:
        raise RuntimeError("route status native asset is not A666")
    if route.get("paused") is not True:
        raise RuntimeError("A666 route must be paused before its pricing epoch advances")
    if not route["live_value_enabled"]:
        raise RuntimeError("A666 route must have live value enabled")
    if route["active_reservation_count"] or route["export_entitlement_count"]:
        raise RuntimeError("route epoch cannot advance with active order state")
    if nav.get("prior_epoch") != route["pricing_nav_epoch"]:
        raise RuntimeError("NAV manifest prior epoch does not match route pricing state")
    if nav["epoch"] <= route["pricing_nav_epoch"]:
        raise RuntimeError("NAV manifest does not advance the route pricing epoch")
    if nav.get("asset_id") != ASSET_ID:
        raise RuntimeError("NAV manifest does not describe A666")
    if nav.get("schema") != NAV_MARK_SCHEMA:
        raise RuntimeError("NAV manifest is not a provider-neutral A666 mark")
    if nav.get("profile_id") != SUCCESSOR_PROFILE_ID:
        raise RuntimeError("NAV manifest is not bound to the pinned successor profile")
    if nav.get("source_manifest_hash") != SOURCE_MANIFEST_HASH:
        raise RuntimeError("NAV manifest source identity is not the pinned successor")
    if nav.get("valuation_policy_hash") != VALUATION_POLICY_HASH:
        raise RuntimeError("NAV manifest valuation policy is not the pinned successor")
    if nav.get("program_vkey") != SP1_PROGRAM_VKEY:
        raise RuntimeError("NAV manifest program vkey is not the pinned successor")
    nav_per_unit = nav.get("nav_per_unit")
    if not isinstance(nav_per_unit, int) or nav_per_unit <= 0:
        raise RuntimeError("NAV manifest nav_per_unit must be a positive integer")
    reserve_packet_hash = nav.get("reserve_packet_hash")
    if (
        not isinstance(reserve_packet_hash, str)
        or len(reserve_packet_hash) != 96
        or any(character not in "0123456789abcdef" for character in reserve_packet_hash)
    ):
        raise RuntimeError("NAV manifest reserve packet hash is malformed")

    next_policy: dict[str, Any] = {
        "policy_hash": "",
        "policy_epoch": route["policy_epoch"] + 1,
        "issue_multiplier_bps": route["issue_multiplier_bps"],
        "redeem_multiplier_bps": route["redeem_multiplier_bps"],
        "issue_capacity_atoms": ISSUE_CAPACITY_ATOMS,
        "redeem_capacity_atoms": REDEEM_CAPACITY_ATOMS,
        "max_order_atoms": route["max_order_atoms"],
        "min_order_atoms": route["min_order_atoms"],
        "valid_from_height": args.valid_from_height,
        "expires_at_height": route["policy_expires_at_height"],
        "max_nav_age_blocks": route["max_nav_age_blocks"],
        "pricing_nav_epoch": nav["epoch"],
        "pricing_reserve_packet_hash": reserve_packet_hash,
    }
    if next_policy["expires_at_height"] <= args.valid_from_height:
        raise RuntimeError("existing policy expiry is too near for the next route epoch")
    next_policy["policy_hash"] = policy_hash(next_policy)

    body = {
        "operation": "pftl_uniswap_route_epoch_advance",
        "operator": ISSUER,
        "route_id": ROUTE_ID,
        "prior_route_epoch": route["route_epoch"],
        "next_route_epoch": route["route_epoch"] + 1,
        "next_route_config_digest": route["route_config_digest"],
        "live_value_enabled": True,
        "next_primary_market_policy": next_policy,
    }
    request = {
        "schema": "postfiat-certified-asset-ops-request-v1",
        "operations": [
            {
                "label": f"a666-production-route-activate-v{body['next_route_epoch']}",
                "source": ISSUER,
                "key_file": str(args.issuer_key_file.resolve()),
                "operation": body,
            }
        ],
    }
    args.output_dir.mkdir(parents=True, mode=0o700)
    write_json(args.output_dir / "route-epoch-advance.ops.json", request)
    manifest = {
        "schema": "postfiat.a666.route_epoch_advance.v1",
        "route_id": ROUTE_ID,
        "prior_route_epoch": body["prior_route_epoch"],
        "next_route_epoch": body["next_route_epoch"],
        "next_policy": next_policy,
        "nav_per_unit": nav_per_unit,
        "route_status_sha256": hashlib.sha256(args.route_status.read_bytes()).hexdigest(),
        "nav_manifest_sha256": hashlib.sha256(args.nav_manifest.read_bytes()).hexdigest(),
    }
    write_json(args.output_dir / "route-epoch-advance-manifest.json", manifest)
    print(json.dumps(manifest, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
