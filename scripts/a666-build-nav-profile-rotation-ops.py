#!/usr/bin/env python3
"""Build the governed A666 StakeHub proof-profile rotation and asset rebind."""

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
ISSUER_KEY = (
    "/home/postfiat/tmp/navswap-ce22-venue-rebuild-20260719/private/faucet-key.json"
)
PROFILE_ID_DOMAIN = "postfiat.nav_proof_profile_id.v1"
DEFAULT_SOURCE_CLASS = "stakehub-six-leg-reserves-v3"

PROFILE_FIELDS: dict[str, Any] = {
    "verifier_kind": "sp1-groth16",
    "max_snapshot_age_blocks": 900,
    "challenge_window_blocks": 1,
    "max_epoch_gap_blocks": 128,
    "settle_deadline_blocks": 256,
    "min_challenge_bond": 0,
    "min_attestations": 0,
    "tolerance_bp": 0,
    "sp1_proof_encoding": "groth16",
    "max_proof_bytes": 4096,
    "max_public_values_bytes": 16384,
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--proof-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--source-class", default=DEFAULT_SOURCE_CLASS)
    return parser.parse_args()


def hash_domain(domain: str, payload: bytes) -> str:
    return hashlib.sha3_384(domain.encode() + b"\0" + payload).hexdigest()


def write_json(path: Path, value: object) -> None:
    if path.exists():
        raise RuntimeError(f"refusing to overwrite {path}")
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    os.chmod(path, 0o600)


def operation(label: str, body: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema": "postfiat-certified-asset-ops-request-v1",
        "operations": [
            {
                "label": label,
                "source": ISSUER,
                "key_file": ISSUER_KEY,
                "operation": body,
            }
        ],
    }


def profile_id(source_class: str, policy_hash: str, program_vkey: str) -> str:
    preimage = (
        f"verifier_kind={PROFILE_FIELDS['verifier_kind']}\n"
        f"source_class={source_class}\n"
        f"max_snapshot_age_blocks={PROFILE_FIELDS['max_snapshot_age_blocks']}\n"
        f"challenge_window_blocks={PROFILE_FIELDS['challenge_window_blocks']}\n"
        f"max_epoch_gap_blocks={PROFILE_FIELDS['max_epoch_gap_blocks']}\n"
        f"settle_deadline_blocks={PROFILE_FIELDS['settle_deadline_blocks']}\n"
        f"min_challenge_bond={PROFILE_FIELDS['min_challenge_bond']}\n"
        f"min_attestations={PROFILE_FIELDS['min_attestations']}\n"
        f"tolerance_bp={PROFILE_FIELDS['tolerance_bp']}\n"
        f"valuation_policy_hash={policy_hash}\n"
        f"sp1_program_vkey={program_vkey}\n"
        f"sp1_proof_encoding={PROFILE_FIELDS['sp1_proof_encoding']}\n"
        f"max_proof_bytes={PROFILE_FIELDS['max_proof_bytes']}\n"
        f"max_public_values_bytes={PROFILE_FIELDS['max_public_values_bytes']}\n"
    )
    return hash_domain(PROFILE_ID_DOMAIN, preimage.encode())


def main() -> None:
    args = parse_args()
    if args.output_dir.exists():
        raise RuntimeError(f"refusing to overwrite {args.output_dir}")

    report = json.loads(
        (args.proof_dir / "aggregate-witness-report.json").read_text()
    )
    policy_hash = str(report["policy_hash"]).removeprefix("0x").lower()
    program_vkey = (args.proof_dir / "aggregate-vkey.txt").read_text().strip()
    proof = (args.proof_dir / "aggregate-proof-calldata.bin").read_bytes()
    public_values = (args.proof_dir / "aggregate-public-values.bin").read_bytes()
    executed = (args.proof_dir / "aggregate-public-values-execute.bin").read_bytes()

    if not re.fullmatch(r"[0-9a-f]{64}", policy_hash):
        raise RuntimeError("aggregate policy hash is not a 32-byte lowercase hex value")
    if not re.fullmatch(r"0x[0-9a-f]{64}", program_vkey):
        raise RuntimeError("aggregate program vkey is not a 0x-prefixed bytes32")
    if public_values != executed:
        raise RuntimeError("proved and executed aggregate public values differ")
    if len(proof) > PROFILE_FIELDS["max_proof_bytes"]:
        raise RuntimeError("aggregate proof exceeds governed profile limit")
    if len(public_values) > PROFILE_FIELDS["max_public_values_bytes"]:
        raise RuntimeError("aggregate public values exceed governed profile limit")

    next_profile_id = profile_id(args.source_class, policy_hash, program_vkey)
    args.output_dir.mkdir(parents=True, mode=0o700)

    profile_body = {
        "operation": "nav_profile_register",
        "registrant": ISSUER,
        **PROFILE_FIELDS,
        "source_class": args.source_class,
        "valuation_policy_hash": policy_hash,
        "sp1_program_vkey": program_vkey,
    }
    rebind_body = {
        "operation": "nav_asset_register",
        "issuer": ISSUER,
        "asset_id": ASSET_ID,
        "reserve_operator": RESERVE_OPERATOR,
        "proof_profile": next_profile_id,
        "valuation_unit": "USD_1E8",
        "redemption_account": ISSUER,
    }
    write_json(
        args.output_dir / "01-nav-profile-register.ops.json",
        operation("a666-stakehub-v3-nav-profile-register", profile_body),
    )
    write_json(
        args.output_dir / "02-nav-asset-rebind.ops.json",
        operation("a666-stakehub-v3-nav-asset-rebind", rebind_body),
    )

    manifest = {
        "schema": "postfiat.a666.nav_profile_rotation.v1",
        "asset_id": ASSET_ID,
        "profile_id": next_profile_id,
        "source_class": args.source_class,
        "valuation_policy_hash": policy_hash,
        "sp1_program_vkey": program_vkey,
        "sp1_proof_encoding": PROFILE_FIELDS["sp1_proof_encoding"],
        "max_proof_bytes": PROFILE_FIELDS["max_proof_bytes"],
        "max_public_values_bytes": PROFILE_FIELDS["max_public_values_bytes"],
        "proof_sha256": hashlib.sha256(proof).hexdigest(),
        "public_values_sha256": hashlib.sha256(public_values).hexdigest(),
        "reason": (
            "rotate from the per-snapshot NEAR-head-bound policy to a stable "
            "policy identity while retaining exact witness head verification"
        ),
    }
    write_json(args.output_dir / "profile-rotation-manifest.json", manifest)
    print(json.dumps(manifest, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
