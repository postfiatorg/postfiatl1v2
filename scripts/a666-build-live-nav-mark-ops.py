#!/usr/bin/env python3
"""Build a non-opening A666 NAV mark from a live proof and reserve overlays."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
from typing import Any

ASSET_ID = (
    "521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b6"
    "2d20e18555642bec32174498cbee5e2c"
)
ISSUER = "pffcb93d9f87a843a8aa34e1adf241f5d58143e81b"
RESERVE_OPERATOR = "pfd0c86d9084915e1fefd22eab891806397d5a5937"
PROFILE_ID = (
    "8c0244fe0cfb216fb5ab471d0c9e060a5c8ba052b5a29952d6e7aad76b24523a"
    "f2b7e0ed82885c11d2c6308ddfcc9118"
)
POLICY_HASH = "a13553ba6f1a48dbe02dbc34de4d8faed1afa962dc2d2b29ff6f0c6b7ac6fd5c"
PROGRAM_VKEY = "0x00f96064937f05d891b13a80667bdf5ecd62a7d5ed245724ab294bad311a2164"
ISSUER_KEY = (
    "/home/postfiat/tmp/navswap-ce22-venue-rebuild-20260719/private/faucet-key.json"
)
RESERVE_KEY = (
    "/home/postfiat/tmp/navswap-ce22-venue-rebuild-20260719/private/reserve-key.json"
)
NAV_VALUATION_UNIT = "USD_1E8"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--proof-dir", type=Path, required=True)
    parser.add_argument("--pftl-status", type=Path, required=True)
    parser.add_argument("--route-status", type=Path, required=True)
    parser.add_argument("--vault-status", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    return parser.parse_args()


def hash_domain(domain: str, payload: bytes) -> str:
    return hashlib.sha3_384(domain.encode() + b"\0" + payload).hexdigest()


def word_u128(value: bytes, offset: int) -> int:
    word = value[offset : offset + 32]
    if len(word) != 32 or any(word[:16]):
        raise RuntimeError(f"invalid uint128 ABI word at offset {offset}")
    return int.from_bytes(word[16:], "big")


def valuation_scale(unit: str, precision: int) -> int:
    normalized = unit.strip().lower()
    if normalized.startswith("usd_1e"):
        return 10 ** int(normalized.removeprefix("usd_1e"))
    if normalized in {"usdc", "micro_usd"}:
        return 10**precision
    return 10**precision


def settlement_to_nav_value(
    atoms: int, settlement_unit: str, settlement_precision: int
) -> int:
    nav_scale = valuation_scale(NAV_VALUATION_UNIT, settlement_precision)
    settlement_scale = valuation_scale(settlement_unit, settlement_precision)
    return atoms * nav_scale // settlement_scale


def operation(
    label: str, source: str, key_file: str, body: dict[str, Any]
) -> dict[str, Any]:
    return {
        "schema": "postfiat-certified-asset-ops-request-v1",
        "operations": [
            {
                "label": label,
                "source": source,
                "key_file": key_file,
                "operation": body,
            }
        ],
    }


def write_json(path: Path, value: object) -> None:
    if path.exists():
        raise RuntimeError(f"refusing to overwrite {path}")
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    os.chmod(path, 0o600)


def active_a666_profile(status: dict[str, Any]) -> dict[str, Any]:
    matches = [
        profile
        for profile in status.get("active_nav_profiles", [])
        if profile.get("asset_id") == ASSET_ID
    ]
    if len(matches) != 1:
        raise RuntimeError("PFTL status must contain exactly one active A666 profile")
    profile = matches[0]
    required = {
        "profile_id": PROFILE_ID,
        "verifier_kind": "sp1-groth16",
        "source_class": "stakehub-six-leg-reserves-v2",
        "valuation_policy_hash": POLICY_HASH,
        "halted": False,
    }
    for field, expected in required.items():
        if profile.get(field) != expected:
            raise RuntimeError(f"A666 profile {field} differs from governed value")
    return profile


def build_overlay(
    route: dict[str, Any], vault: dict[str, Any]
) -> tuple[int, str, dict[str, Any]]:
    settlement_asset = route["settlement_asset_id"]
    if vault.get("asset_id") != settlement_asset:
        raise RuntimeError("vault status does not describe the route settlement asset")
    settlement_unit = vault["valuation_unit"]
    precision = 6

    buckets = {bucket["bucket_id"]: bucket for bucket in vault["buckets"]}
    receipts = {receipt["receipt_id"]: receipt for receipt in vault["receipts"]}
    allocation_rows: list[dict[str, Any]] = []
    for allocation in vault["allocations"]:
        if (
            allocation.get("purpose") != "nav_subscription"
            or not allocation.get("consumer_id", "").startswith(
                f"nav_subscription:{ASSET_ID}"
            )
            or allocation.get("retired_at_height", 0) == 0
            or allocation.get("remaining_atoms", 0) == 0
        ):
            continue
        bucket = buckets[allocation["bucket_id"]]
        receipt = receipts[allocation["receipt_id"]]
        if (
            bucket["status"] != "active"
            or receipt["status"] != "counted"
            or receipt.get("asset_id", settlement_asset) != settlement_asset
        ):
            raise RuntimeError("NAV subscription overlay is not active and counted")
        row = dict(allocation)
        row["value_nav_units"] = settlement_to_nav_value(
            allocation["remaining_atoms"], settlement_unit, precision
        )
        row["bucket"] = bucket
        allocation_rows.append(row)
    allocation_rows.sort(key=lambda row: row["allocation_id"])

    active_bucket_backing = sum(
        bucket["outstanding_vault_bridge_atoms"]
        for bucket in vault["buckets"]
        if bucket["status"] == "active"
        and bucket.get("asset_id", settlement_asset) == settlement_asset
    )
    settlement_reserve = int(route["settlement_reserve_atoms"])
    if settlement_reserve > active_bucket_backing:
        raise RuntimeError("primary-market reserve exceeds proof-backed vault backing")
    route_rows = []
    if settlement_reserve:
        route_rows.append(
            {
                "route_id": route["route_id"],
                "route_config_digest": route["route_config_digest"],
                "settlement_asset_id": settlement_asset,
                "settlement_reserve_atoms": settlement_reserve,
                "value_nav_units": settlement_to_nav_value(
                    settlement_reserve, settlement_unit, precision
                ),
                "active_bucket_backing_atoms": active_bucket_backing,
                "live_value_enabled": route["live_value_enabled"],
                "paused": route["paused"],
            }
        )

    overlay_value = 0
    preimage = (
        f"nav_asset_id={ASSET_ID}\n"
        f"nav_valuation_unit_bytes={len(NAV_VALUATION_UNIT)}\n"
        f"nav_valuation_unit={NAV_VALUATION_UNIT}\n"
        f"allocation_count={len(allocation_rows)}\n"
        f"primary_market_route_count={len(route_rows)}\n"
    )
    for index, row in enumerate(allocation_rows):
        bucket = row["bucket"]
        overlay_value += row["value_nav_units"]
        preimage += (
            f"allocation[{index}].allocation_id={row['allocation_id']}\n"
            f"allocation[{index}].settlement_asset_id={settlement_asset}\n"
            f"allocation[{index}].bucket_id={row['bucket_id']}\n"
            f"allocation[{index}].receipt_id={row['receipt_id']}\n"
            f"allocation[{index}].amount_atoms={row['amount_atoms']}\n"
            f"allocation[{index}].released_atoms={row['released_atoms']}\n"
            f"allocation[{index}].remaining_atoms={row['remaining_atoms']}\n"
            f"allocation[{index}].value_nav_units={row['value_nav_units']}\n"
            f"allocation[{index}].retired_at_height={row['retired_at_height']}\n"
            f"allocation[{index}].bucket_source_domain_bytes={len(bucket['source_domain'])}\n"
            f"allocation[{index}].bucket_source_domain={bucket['source_domain']}\n"
            f"allocation[{index}].bucket_policy_hash={bucket['policy_hash']}\n"
            f"allocation[{index}].bucket_gross_receipt_atoms={bucket['gross_receipt_atoms']}\n"
            f"allocation[{index}].bucket_counted_value_atoms={bucket['counted_value_atoms']}\n"
            f"allocation[{index}].bucket_nav_subscription_allocations_atoms={bucket['nav_subscription_allocations_atoms']}\n"
            f"allocation[{index}].bucket_redemption_queue_atoms={bucket['redemption_queue_atoms']}\n"
            f"allocation[{index}].bucket_outstanding_vault_bridge_atoms={bucket['outstanding_vault_bridge_atoms']}\n"
            f"allocation[{index}].bucket_status={bucket['status']}\n"
        )
    for index, row in enumerate(route_rows):
        overlay_value += row["value_nav_units"]
        preimage += (
            f"primary_market[{index}].route_id_bytes={len(row['route_id'])}\n"
            f"primary_market[{index}].route_id={row['route_id']}\n"
            f"primary_market[{index}].route_config_digest={row['route_config_digest']}\n"
            f"primary_market[{index}].settlement_asset_id={row['settlement_asset_id']}\n"
            f"primary_market[{index}].settlement_reserve_atoms={row['settlement_reserve_atoms']}\n"
            f"primary_market[{index}].value_nav_units={row['value_nav_units']}\n"
            f"primary_market[{index}].active_bucket_backing_atoms={row['active_bucket_backing_atoms']}\n"
            f"primary_market[{index}].live_value_enabled={str(row['live_value_enabled']).lower()}\n"
            f"primary_market[{index}].paused={str(row['paused']).lower()}\n"
        )
    overlay_root = hash_domain(
        "postfiat.nav_subscription_source_root.v1", preimage.encode()
    )
    return overlay_value, overlay_root, {
        "allocation_rows": allocation_rows,
        "primary_market_rows": route_rows,
        "value_nav_units": overlay_value,
        "source_root": overlay_root,
    }


def main() -> None:
    args = parse_args()
    if args.output_dir.exists():
        raise RuntimeError(f"refusing to overwrite {args.output_dir}")
    status = json.loads(args.pftl_status.read_text())
    route = json.loads(args.route_status.read_text())
    vault = json.loads(args.vault_status.read_text())
    profile = active_a666_profile(status)

    proof = (args.proof_dir / "aggregate-proof-calldata.bin").read_bytes()
    public_values = (args.proof_dir / "aggregate-public-values.bin").read_bytes()
    executed = args.proof_dir / "aggregate-public-values-execute.bin"
    if executed.exists() and public_values != executed.read_bytes():
        raise RuntimeError("proved and executed aggregate public values differ")
    vkey = (args.proof_dir / "aggregate-vkey.txt").read_text().strip()
    if vkey != PROGRAM_VKEY:
        raise RuntimeError("aggregate vkey differs from the governed A666 profile")
    if len(proof) > 4096 or len(public_values) > 16384:
        raise RuntimeError("proof material exceeds the governed profile limits")

    tuple_offset = word_u128(public_values, 0)
    if tuple_offset != 32 or word_u128(public_values, tuple_offset) != 2:
        raise RuntimeError("unexpected AggregatePublicValuesV2 encoding")
    base = tuple_offset
    if public_values[base + 64 : base + 96].hex() != POLICY_HASH:
        raise RuntimeError("public-values policy differs from the governed profile")
    spot = word_u128(public_values, base + 96)
    cash = word_u128(public_values, base + 192)
    liability = word_u128(public_values, base + 320)
    proof_verified_net_assets = spot + cash - liability

    overlay_value, overlay_root, overlay_report = build_overlay(route, vault)
    total_verified_net_assets = proof_verified_net_assets + overlay_value
    circulating_supply = int(route["authorized_valid_supply_atoms"])
    if circulating_supply <= 0:
        raise RuntimeError("live valid supply must be positive")
    nav_per_unit = total_verified_net_assets * 1_000_000 // circulating_supply
    epoch = int(profile["finalized_epoch"]) + 1

    public_values_hash = hash_domain(
        "postfiat.nav_sp1_public_values.v1", public_values
    )
    source_preimage = (
        f"asset_id={ASSET_ID}\n"
        f"profile_id={PROFILE_ID}\n"
        f"profile_source_class_bytes={len(profile['source_class'])}\n"
        f"profile_source_class={profile['source_class']}\n"
        f"policy_hash={POLICY_HASH}\n"
        f"sp1_public_values_hash={public_values_hash}\n"
        f"sp1_verified_net_assets={proof_verified_net_assets}\n"
        f"subscription_overlay_source_root={overlay_root}\n"
        f"subscription_overlay_value_nav_units={overlay_value}\n"
        f"total_verified_net_assets={total_verified_net_assets}\n"
    )
    source_root = hash_domain(
        "postfiat.nav_sp1_subscription_composite_source_root.v1",
        source_preimage.encode(),
    )
    attestor_preimage = (
        "operator=0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0\n"
        f"policy_hash={POLICY_HASH}\n"
        f"program_vkey={PROGRAM_VKEY}\n"
    ).encode()
    attestor_root = hash_domain(
        "postfiat.a666.stakehub_attestor_root.v1", attestor_preimage
    )
    packet_preimage = (
        f"asset_id={ASSET_ID}\nissuer={ISSUER}\n"
        f"reserve_operator={RESERVE_OPERATOR}\nepoch={epoch}\n"
        f"nav_per_unit={nav_per_unit}\n"
        f"circulating_supply={circulating_supply}\n"
        f"verified_net_assets={total_verified_net_assets}\n"
        f"proof_profile={PROFILE_ID}\nsource_root={source_root}\n"
        f"attestor_root={attestor_root}\n"
    ).encode()
    packet_hash = hash_domain(
        "postfiat.nav_roundtrip.reserve_packet_hash.v1", packet_preimage
    )

    args.output_dir.mkdir(parents=True, mode=0o700)
    reserve = operation(
        f"a666-live-nav-e{epoch}-reserve-submit",
        RESERVE_OPERATOR,
        RESERVE_KEY,
        {
            "operation": "nav_reserve_submit",
            "issuer": ISSUER,
            "submitter": RESERVE_OPERATOR,
            "asset_id": ASSET_ID,
            "epoch": epoch,
            "nav_per_unit": nav_per_unit,
            "circulating_supply": circulating_supply,
            "verified_net_assets": total_verified_net_assets,
            "proof_profile": PROFILE_ID,
            "source_root": source_root,
            "attestor_root": attestor_root,
            "reserve_packet_hash": packet_hash,
            "reserve_accounts": [],
            "sp1_proof_bytes": list(proof),
            "sp1_public_values": list(public_values),
        },
    )
    finalize = operation(
        f"a666-live-nav-e{epoch}-epoch-finalize",
        ISSUER,
        ISSUER_KEY,
        {
            "operation": "nav_epoch_finalize",
            "issuer": ISSUER,
            "asset_id": ASSET_ID,
            "epoch": epoch,
            "reserve_packet_hash": packet_hash,
        },
    )
    write_json(args.output_dir / "01-reserve-submit.ops.json", reserve)
    write_json(args.output_dir / "02-epoch-finalize.ops.json", finalize)
    proof_sha = hashlib.sha256(proof).hexdigest()
    public_sha = hashlib.sha256(public_values).hexdigest()
    manifest = {
        "schema": "postfiat.a666.live_nav_mark.v1",
        "asset_id": ASSET_ID,
        "prior_epoch": profile["finalized_epoch"],
        "epoch": epoch,
        "profile_id": PROFILE_ID,
        "policy_hash": POLICY_HASH,
        "program_vkey": PROGRAM_VKEY,
        "proof_sha256": proof_sha,
        "public_values_sha256": public_sha,
        "proof_verified_net_assets_usd_1e8": proof_verified_net_assets,
        "primary_reserve_overlay": overlay_report,
        "verified_net_assets_usd_1e8": total_verified_net_assets,
        "circulating_supply_atoms": circulating_supply,
        "nav_per_unit_usd_1e8": nav_per_unit,
        "source_root": source_root,
        "attestor_root": attestor_root,
        "reserve_packet_hash": packet_hash,
        "uniswap_price_used": False,
        "opening_constants_used": False,
    }
    write_json(args.output_dir / "live-nav-mark-manifest.json", manifest)
    print(json.dumps(manifest, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
