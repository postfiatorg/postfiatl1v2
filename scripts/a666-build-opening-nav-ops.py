#!/usr/bin/env python3
"""Build the proof-bound A666 opening NAV, finalize, and mint operations."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path

ASSET_ID = (
    "521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b6"
    "2d20e18555642bec32174498cbee5e2c"
)
ISSUER = "pffcb93d9f87a843a8aa34e1adf241f5d58143e81b"
RESERVE_OPERATOR = "pfd0c86d9084915e1fefd22eab891806397d5a5937"
OPENING_HOLDER = "pfab9b9228942e5c529633a13aa271d5297bec6353"
PROFILE_ID = (
    "8c0244fe0cfb216fb5ab471d0c9e060a5c8ba052b5a29952d6e7aad76b24523a"
    "f2b7e0ed82885c11d2c6308ddfcc9118"
)
POLICY_HASH = "a13553ba6f1a48dbe02dbc34de4d8faed1afa962dc2d2b29ff6f0c6b7ac6fd5c"
PROGRAM_VKEY = "0x00f96064937f05d891b13a80667bdf5ecd62a7d5ed245724ab294bad311a2164"
PROOF_SHA256 = "c5db9bfa11fc09781f2e87d28a370b9d98b113bc740276897dfbb7ea00e7c56c"
PUBLIC_VALUES_SHA256 = (
    "e6d784099e01212c0f5df4143f867606e0b80d0eac91d14a8bce9ffbd1bca26a"
)
VERIFIED_NET_ASSETS = 3_138_619_745_591
CIRCULATING_SUPPLY = 31_386_197_455
NAV_PER_UNIT = 100_000_000
EPOCH = 1
ISSUER_KEY = (
    "/home/postfiat/tmp/navswap-ce22-venue-rebuild-20260719/private/faucet-key.json"
)
RESERVE_KEY = (
    "/home/postfiat/tmp/navswap-ce22-venue-rebuild-20260719/private/reserve-key.json"
)


def hash_domain(domain: str, payload: bytes) -> str:
    return hashlib.sha3_384(domain.encode() + b"\0" + payload).hexdigest()


def word_u128(value: bytes, offset: int) -> int:
    word = value[offset : offset + 32]
    if len(word) != 32 or any(word[:16]):
        raise RuntimeError(f"invalid uint128 ABI word at offset {offset}")
    return int.from_bytes(word[16:], "big")


def operation(label: str, source: str, key_file: str, body: dict, dependency: str) -> dict:
    return {
        "schema": "postfiat-certified-asset-ops-request-v1",
        "operations": [
            {
                "label": label,
                "source": source,
                "key_file": key_file,
                "dependencies": [
                    {
                        "label": dependency,
                        "mode": "prior_round",
                        "reason": "the preceding A666 opening operation must be finalized",
                    }
                ],
                "operation": body,
            }
        ],
    }


def write_json(path: Path, value: object) -> None:
    if path.exists():
        raise RuntimeError(f"refusing to overwrite {path}")
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    os.chmod(path, 0o600)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--proof-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()

    proof = (args.proof_dir / "aggregate-proof-calldata.bin").read_bytes()
    public_values = (args.proof_dir / "aggregate-public-values.bin").read_bytes()
    executed_public_values = (
        args.proof_dir / "aggregate-public-values-execute.bin"
    ).read_bytes()
    vkey = (args.proof_dir / "aggregate-vkey.txt").read_text().strip()

    if hashlib.sha256(proof).hexdigest() != PROOF_SHA256:
        raise RuntimeError("unexpected A666 aggregate proof calldata")
    if hashlib.sha256(public_values).hexdigest() != PUBLIC_VALUES_SHA256:
        raise RuntimeError("unexpected A666 aggregate public values")
    if public_values != executed_public_values:
        raise RuntimeError("proved and executed aggregate public values differ")
    if vkey != PROGRAM_VKEY:
        raise RuntimeError("aggregate program vkey differs from the registered profile")
    if len(proof) > 4096 or len(public_values) > 16384:
        raise RuntimeError("proof material exceeds the registered profile limits")

    tuple_offset = word_u128(public_values, 0)
    if tuple_offset != 32:
        raise RuntimeError("unexpected AggregatePublicValuesV2 tuple offset")
    base = tuple_offset
    if word_u128(public_values, base) != 2:
        raise RuntimeError("unexpected AggregatePublicValuesV2 schema")
    if public_values[base + 64 : base + 96].hex() != POLICY_HASH:
        raise RuntimeError("public-values policy hash differs from the registered profile")
    spot = word_u128(public_values, base + 96)
    cash = word_u128(public_values, base + 192)
    liability = word_u128(public_values, base + 320)
    if spot + cash - liability != VERIFIED_NET_ASSETS:
        raise RuntimeError("public-values net assets differ from the opening NAV")
    if (
        VERIFIED_NET_ASSETS * 1_000_000 // CIRCULATING_SUPPLY
        != NAV_PER_UNIT
    ):
        raise RuntimeError("opening NAV-per-unit does not floor to exactly $1.00")

    source_root = hash_domain(
        "postfiat.a666.stakehub_source_root.v1", proof + public_values
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
        f"asset_id={ASSET_ID}\n"
        f"issuer={ISSUER}\n"
        f"reserve_operator={RESERVE_OPERATOR}\n"
        f"epoch={EPOCH}\n"
        f"nav_per_unit={NAV_PER_UNIT}\n"
        f"circulating_supply={CIRCULATING_SUPPLY}\n"
        f"verified_net_assets={VERIFIED_NET_ASSETS}\n"
        f"proof_profile={PROFILE_ID}\n"
        f"source_root={source_root}\n"
        f"attestor_root={attestor_root}\n"
    ).encode()
    reserve_packet_hash = hash_domain(
        "postfiat.nav_roundtrip.reserve_packet_hash.v1", packet_preimage
    )

    args.output_dir.mkdir(parents=True, exist_ok=True)
    reserve = operation(
        "a666-v2-opening-reserve-submit",
        RESERVE_OPERATOR,
        RESERVE_KEY,
        {
            "operation": "nav_reserve_submit",
            "issuer": ISSUER,
            "submitter": RESERVE_OPERATOR,
            "asset_id": ASSET_ID,
            "epoch": EPOCH,
            "nav_per_unit": NAV_PER_UNIT,
            "circulating_supply": CIRCULATING_SUPPLY,
            "verified_net_assets": VERIFIED_NET_ASSETS,
            "proof_profile": PROFILE_ID,
            "source_root": source_root,
            "attestor_root": attestor_root,
            "reserve_packet_hash": reserve_packet_hash,
            "reserve_accounts": [],
            "sp1_proof_bytes": list(proof),
            "sp1_public_values": list(public_values),
        },
        "a666-v2-opening-holder-authorize",
    )
    finalize = operation(
        "a666-v2-opening-epoch-finalize",
        ISSUER,
        ISSUER_KEY,
        {
            "operation": "nav_epoch_finalize",
            "issuer": ISSUER,
            "asset_id": ASSET_ID,
            "epoch": EPOCH,
            "reserve_packet_hash": reserve_packet_hash,
        },
        "a666-v2-opening-reserve-submit",
    )
    mint = operation(
        "a666-v2-opening-nav-mint",
        ISSUER,
        ISSUER_KEY,
        {
            "operation": "nav_mint_at_nav",
            "issuer": ISSUER,
            "to": OPENING_HOLDER,
            "asset_id": ASSET_ID,
            "amount": CIRCULATING_SUPPLY,
            "epoch": EPOCH,
            "reserve_packet_hash": reserve_packet_hash,
        },
        "a666-v2-opening-epoch-finalize",
    )
    write_json(args.output_dir / "06-opening-reserve-submit.ops.json", reserve)
    write_json(args.output_dir / "07-opening-epoch-finalize.ops.json", finalize)
    write_json(args.output_dir / "08-opening-nav-mint.ops.json", mint)
    write_json(
        args.output_dir / "opening-nav-proof-manifest.json",
        {
            "schema": "postfiat-a666-opening-nav-proof-manifest-v1",
            "asset_id": ASSET_ID,
            "epoch": EPOCH,
            "profile_id": PROFILE_ID,
            "policy_hash": POLICY_HASH,
            "program_vkey": PROGRAM_VKEY,
            "proof_sha256": PROOF_SHA256,
            "public_values_sha256": PUBLIC_VALUES_SHA256,
            "source_root": source_root,
            "attestor_root": attestor_root,
            "reserve_packet_hash": reserve_packet_hash,
            "verified_net_assets_usd_1e8": VERIFIED_NET_ASSETS,
            "circulating_supply_atoms": CIRCULATING_SUPPLY,
            "nav_per_unit_usd_1e8": NAV_PER_UNIT,
        },
    )
    print(json.dumps({"reserve_packet_hash": reserve_packet_hash}, indent=2))


if __name__ == "__main__":
    main()
