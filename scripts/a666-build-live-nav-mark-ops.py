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
