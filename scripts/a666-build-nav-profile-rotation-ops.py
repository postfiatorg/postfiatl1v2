#!/usr/bin/env python3
"""Build the governed A666 provider-neutral proof-profile rotation."""

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
PROFILE_SCHEMA = "postfiat.reserve_derived_profile.v1"
VERIFIER_KIND = "sp1-nav-reserve-v1"
PUBLIC_VALUES_SCHEMA = "postfiat.nav_reserve_public_values.v1"
PROFILE_ID_DOMAIN = "postfiat.nav_proof_profile_id.v2"


PROFILE_FIELDS: tuple[tuple[str, object], ...] = (
    ("verifier_kind", ""),
    ("source_class", ""),
    ("max_snapshot_age_blocks", 0),
    ("challenge_window_blocks", 0),
    ("max_epoch_gap_blocks", 0),
    ("settle_deadline_blocks", 0),
    ("min_challenge_bond", 0),
    ("min_attestations", 0),
    ("tolerance_bp", 0),
    ("bridge_observer_min_confirmations", 0),
    ("valuation_policy_hash", ""),
    ("vault_bridge_route_policy_hash", ""),
    ("sp1_program_vkey", ""),
    ("sp1_proof_encoding", ""),
    ("max_proof_bytes", 0),
    ("max_public_values_bytes", 0),
    ("public_values_schema", ""),
    ("source_manifest_hash", ""),
    ("valuation_unit_id", ""),
    ("max_observation_span_blocks", 0),
    ("allow_controlled_sources", False),
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--derived-profile", type=Path, required=True)
    parser.add_argument("--expected-profile-id", required=True)
    parser.add_argument("--issuer-key-file", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    return parser.parse_args()


def load_json(path: Path) -> tuple[dict[str, Any], str]:
    raw = path.read_bytes()
    value = json.loads(raw)
    if not isinstance(value, dict):
        raise RuntimeError(f"{path} must contain a JSON object")
    return value, hashlib.sha256(raw).hexdigest()


def write_json(path: Path, value: object) -> None:
    if path.exists():
        raise RuntimeError(f"refusing to overwrite {path}")
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    os.chmod(path, 0o600)


def derive_profile_id(profile: dict[str, Any]) -> str:
    preimage = "".join(
        f"{field}={str(profile.get(field, default)).lower() if isinstance(default, bool) else profile.get(field, default)}\n"
        for field, default in PROFILE_FIELDS
    )
    return hashlib.sha3_384(
        PROFILE_ID_DOMAIN.encode() + b"\0" + preimage.encode()
    ).hexdigest()


def certified_operation(label: str, key_file: Path, body: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema": "postfiat-certified-asset-ops-request-v1",
        "operations": [
            {
                "label": label,
                "source": ISSUER,
                "key_file": str(key_file.resolve()),
                "operation": body,
            }
        ],
    }


def validate_profile(
    derived: dict[str, Any], expected_profile_id: str
) -> tuple[dict[str, Any], str]:
    if derived.get("schema") != PROFILE_SCHEMA:
        raise RuntimeError("derived profile schema mismatch")
    operation = derived.get("operation")
    profile = derived.get("profile")
    if not isinstance(operation, dict) or not isinstance(profile, dict):
        raise RuntimeError("derived profile must contain operation and profile objects")
    if operation.get("registrant") != ISSUER:
        raise RuntimeError("A666 successor profile registrant must be the A666 issuer")
    if operation.get("verifier_kind") != VERIFIER_KIND:
        raise RuntimeError("A666 successor must use the provider-neutral reserve verifier")
    if operation.get("public_values_schema") != PUBLIC_VALUES_SCHEMA:
        raise RuntimeError("A666 successor public-values schema mismatch")
    if operation.get("allow_controlled_sources", False) is not False:
        raise RuntimeError("live A666 successor must reject controlled reserve sources")
    operation["allow_controlled_sources"] = False
    for field, size in (
        ("source_manifest_hash", 96),
        ("valuation_unit_id", 96),
        ("valuation_policy_hash", 64),
    ):
        value = operation.get(field)
        if not isinstance(value, str) or not re.fullmatch(rf"[0-9a-f]{{{size}}}", value):
            raise RuntimeError(f"derived profile {field} is malformed")
    vkey = operation.get("sp1_program_vkey")
    if not isinstance(vkey, str) or not re.fullmatch(r"0x[0-9a-f]{64}", vkey):
        raise RuntimeError("derived profile program vkey is malformed")
    profile_id = profile.get("profile_id")
    if not isinstance(profile_id, str) or not re.fullmatch(r"[0-9a-f]{96}", profile_id):
        raise RuntimeError("derived profile ID is malformed")
    if profile_id != expected_profile_id:
        raise RuntimeError("derived profile ID differs from the independently pinned ID")
    if profile.get("registered_by") != ISSUER or profile.get("verifier_kind") != VERIFIER_KIND:
        raise RuntimeError("derived profile identity does not match its registration")
    for field, default in PROFILE_FIELDS:
        if operation.get(field, default) != profile.get(field, default):
            raise RuntimeError(f"derived profile {field} differs from its registration")
    if profile.get("allow_controlled_sources", False) is not False:
        raise RuntimeError("derived profile must reject controlled reserve sources")
    independently_derived_profile_id = derive_profile_id(profile)
    if independently_derived_profile_id != profile_id:
        raise RuntimeError(
            "derived profile ID does not match the canonical registration preimage"
        )
    return operation, profile_id


def main() -> None:
    args = parse_args()
    if args.output_dir.exists():
        raise RuntimeError(f"refusing to overwrite {args.output_dir}")
    if not args.issuer_key_file.is_file():
        raise RuntimeError("issuer key file is unavailable")
    if not re.fullmatch(r"[0-9a-f]{96}", args.expected_profile_id):
        raise RuntimeError("expected profile ID is malformed")
    derived, derived_profile_sha256 = load_json(args.derived_profile)
    operation, profile_id = validate_profile(derived, args.expected_profile_id)
    profile_body = {"operation": "nav_profile_register", **operation}
    rebind_body = {
        "operation": "nav_asset_register",
        "issuer": ISSUER,
        "asset_id": ASSET_ID,
        "reserve_operator": RESERVE_OPERATOR,
        "proof_profile": profile_id,
        "valuation_unit": "USD_1E8",
        "redemption_account": ISSUER,
    }

    args.output_dir.mkdir(parents=True, mode=0o700)
    write_json(
        args.output_dir / "01-nav-profile-register.ops.json",
        certified_operation(
            "a666-provider-neutral-v1-nav-profile-register",
            args.issuer_key_file,
            profile_body,
        ),
    )
    write_json(
        args.output_dir / "02-nav-asset-rebind.ops.json",
        certified_operation(
            "a666-provider-neutral-v1-nav-asset-rebind",
            args.issuer_key_file,
            rebind_body,
        ),
    )
    manifest = {
        "schema": "postfiat.a666.provider_neutral_profile_rotation.v1",
        "asset_id": ASSET_ID,
        "issuer": ISSUER,
        "reserve_operator": RESERVE_OPERATOR,
        "profile_id": profile_id,
        "verifier_kind": operation["verifier_kind"],
        "source_class": operation["source_class"],
        "source_manifest_hash": operation["source_manifest_hash"],
        "valuation_policy_hash": operation["valuation_policy_hash"],
        "valuation_unit_id": operation["valuation_unit_id"],
        "sp1_program_vkey": operation["sp1_program_vkey"],
        "public_values_schema": operation["public_values_schema"],
        "derived_profile_sha256": derived_profile_sha256,
        "history_policy": "register immutable successor; never mutate the deployed legacy profile",
    }
    write_json(args.output_dir / "profile-rotation-manifest.json", manifest)
    print(json.dumps(manifest, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
