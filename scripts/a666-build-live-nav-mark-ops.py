#!/usr/bin/env python3
"""Build A666 reserve-submit/finalize requests from an open-kit packet."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
from pathlib import Path
from typing import Any


ASSET_ID = (
    "521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b6"
    "2d20e18555642bec32174498cbee5e2c"
)
ISSUER = "pffcb93d9f87a843a8aa34e1adf241f5d58143e81b"
RESERVE_OPERATOR = "pfd0c86d9084915e1fefd22eab891806397d5a5937"
VERIFIER_KIND = "sp1-nav-reserve-v1"
PUBLIC_VALUES_SCHEMA = "postfiat.nav_reserve_public_values.v1"
NAV_VALUATION_UNIT = "USD_1E8"


def hash_domain(domain: str, payload: bytes) -> str:
    return hashlib.sha3_384(domain.encode() + b"\0" + payload).hexdigest()


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


def build_overlay(
    route: dict[str, Any], vault: dict[str, Any]
) -> tuple[int, str, dict[str, Any]]:
    """Commit proof-backed settlement inventory into the NAV overlay.

    This remains provider-neutral: it consumes only finalized PFTL route and
    vault status, and refuses to count primary-market inventory beyond the
    active proof-backed vault balance.
    """

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
    if settlement_reserve < 0:
        raise RuntimeError("primary-market reserve cannot be negative")
    if settlement_reserve > active_bucket_backing:
        raise RuntimeError("primary-market reserve exceeds proof-backed vault backing")
    route_rows: list[dict[str, Any]] = []
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


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--packet-operation", type=Path, required=True)
    parser.add_argument("--pftl-status", type=Path, required=True)
    parser.add_argument("--issuer-key-file", type=Path, required=True)
    parser.add_argument("--reserve-key-file", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    return parser.parse_args()


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise RuntimeError(f"{path} must contain a JSON object")
    return value


def write_json(path: Path, value: object) -> None:
    if path.exists():
        raise RuntimeError(f"refusing to overwrite {path}")
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    os.chmod(path, 0o600)


def certified_operation(
    label: str, source: str, key_file: Path, body: dict[str, Any]
) -> dict[str, Any]:
    return {
        "schema": "postfiat-certified-asset-ops-request-v1",
        "operations": [
            {
                "label": label,
                "source": source,
                "key_file": str(key_file.resolve()),
                "operation": body,
            }
        ],
    }


def active_profile(status: dict[str, Any]) -> dict[str, Any]:
    matches = [
        profile
        for profile in status.get("active_nav_profiles", [])
        if profile.get("asset_id") == ASSET_ID
    ]
    if len(matches) != 1:
        raise RuntimeError("PFTL status must contain exactly one active A666 profile")
    profile = matches[0]
    if profile.get("verifier_kind") != VERIFIER_KIND:
        raise RuntimeError("active A666 profile is not provider-neutral reserve-proof v1")
    if profile.get("public_values_schema") != PUBLIC_VALUES_SCHEMA:
        raise RuntimeError("active A666 public-values schema mismatch")
    if profile.get("halted") is not False:
        raise RuntimeError("active A666 profile is halted")
    return profile


def validate_packet(packet: dict[str, Any], profile: dict[str, Any]) -> None:
    required = {
        "issuer": ISSUER,
        "submitter": RESERVE_OPERATOR,
        "asset_id": ASSET_ID,
        "proof_profile": profile.get("profile_id"),
    }
    for field, expected in required.items():
        if packet.get(field) != expected:
            raise RuntimeError(f"packet {field} differs from governed A666 state")
    epoch = packet.get("epoch")
    if not isinstance(epoch, int) or epoch != int(profile.get("finalized_epoch", 0)) + 1:
        raise RuntimeError("packet epoch must immediately follow the finalized A666 epoch")
    for field in ("nav_per_unit", "verified_net_assets"):
        if not isinstance(packet.get(field), int) or packet[field] <= 0:
            raise RuntimeError(f"packet {field} must be positive")
    if not isinstance(packet.get("circulating_supply"), int) or packet["circulating_supply"] < 0:
        raise RuntimeError("packet circulating_supply is invalid")
    for field in ("source_root", "attestor_root", "reserve_packet_hash"):
        value = packet.get(field)
        if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{96}", value):
            raise RuntimeError(f"packet {field} is malformed")
    proof = packet.get("sp1_proof_bytes")
    public_values = packet.get("sp1_public_values")
    if not isinstance(proof, list) or not proof or len(proof) > int(profile["max_proof_bytes"]):
        raise RuntimeError("packet proof bytes violate the active profile bound")
    if (
        not isinstance(public_values, list)
        or len(public_values) != 584
        or len(public_values) > int(profile["max_public_values_bytes"])
    ):
        raise RuntimeError("packet public values violate the active profile bound")
    for label, values in (("proof", proof), ("public values", public_values)):
        if any(not isinstance(value, int) or value < 0 or value > 255 for value in values):
            raise RuntimeError(f"packet {label} must be a byte array")


def main() -> None:
    args = parse_args()
    if args.output_dir.exists():
        raise RuntimeError(f"refusing to overwrite {args.output_dir}")
    for label, path in (
        ("issuer", args.issuer_key_file),
        ("reserve operator", args.reserve_key_file),
    ):
        if not path.is_file():
            raise RuntimeError(f"{label} key file is unavailable")
    packet = load_json(args.packet_operation)
    status = load_json(args.pftl_status)
    profile = active_profile(status)
    validate_packet(packet, profile)
    epoch = packet["epoch"]
    reserve_body = {"operation": "nav_reserve_submit", **packet}
    finalize_body = {
        "operation": "nav_epoch_finalize",
        "issuer": ISSUER,
        "asset_id": ASSET_ID,
        "epoch": epoch,
        "reserve_packet_hash": packet["reserve_packet_hash"],
    }

    args.output_dir.mkdir(parents=True, mode=0o700)
    write_json(
        args.output_dir / "01-reserve-submit.ops.json",
        certified_operation(
            f"a666-provider-neutral-e{epoch}-reserve-submit",
            RESERVE_OPERATOR,
            args.reserve_key_file,
            reserve_body,
        ),
    )
    write_json(
        args.output_dir / "02-epoch-finalize.ops.json",
        certified_operation(
            f"a666-provider-neutral-e{epoch}-epoch-finalize",
            ISSUER,
            args.issuer_key_file,
            finalize_body,
        ),
    )
    proof_bytes = bytes(packet["sp1_proof_bytes"])
    public_values = bytes(packet["sp1_public_values"])
    manifest = {
        "schema": "postfiat.a666.provider_neutral_nav_mark.v1",
        "asset_id": ASSET_ID,
        "prior_epoch": profile["finalized_epoch"],
        "epoch": epoch,
        "profile_id": profile["profile_id"],
        "source_manifest_hash": profile["source_manifest_hash"],
        "valuation_policy_hash": profile["valuation_policy_hash"],
        "program_vkey": profile["sp1_program_vkey"],
        "public_values_schema": profile["public_values_schema"],
        "proof_sha256": hashlib.sha256(proof_bytes).hexdigest(),
        "public_values_sha256": hashlib.sha256(public_values).hexdigest(),
        "verified_net_assets": packet["verified_net_assets"],
        "circulating_supply_atoms": packet["circulating_supply"],
        "nav_per_unit": packet["nav_per_unit"],
        "source_root": packet["source_root"],
        "attestor_root": packet["attestor_root"],
        "reserve_packet_hash": packet["reserve_packet_hash"],
    }
    write_json(args.output_dir / "live-nav-mark-manifest.json", manifest)
    print(json.dumps(manifest, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
