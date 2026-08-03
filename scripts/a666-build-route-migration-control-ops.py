#!/usr/bin/env python3
"""Build fail-closed pause/resume controls for an A666 route migration."""

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
RESERVE_OPERATOR = "pfd0c86d9084915e1fefd22eab891806397d5a5937"
ROUTE_ID = "pftl-a666-ethereum-wA666-usdc-v1"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--route-status", type=Path, required=True)
    parser.add_argument("--reserve-key-file", type=Path, required=True)
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


def pause_request(*, key_file: Path, paused: bool, label: str) -> dict[str, Any]:
    return {
        "schema": "postfiat-certified-asset-ops-request-v1",
        "operations": [
            {
                "label": label,
                "source": RESERVE_OPERATOR,
                "key_file": str(key_file.resolve()),
                "operation": {
                    "operation": "pftl_uniswap_route_pause",
                    "operator": RESERVE_OPERATOR,
                    "route_id": ROUTE_ID,
                    "paused": paused,
                },
            }
        ],
    }


def validate_route(route: dict[str, Any]) -> None:
    if route.get("route_id") != ROUTE_ID:
        raise RuntimeError("route status is not the governed A666 route")
    if route.get("native_nav_asset_id") != ASSET_ID:
        raise RuntimeError("route status native asset is not A666")
    if route.get("live_value_enabled") is not True:
        raise RuntimeError("A666 route must have live value enabled before migration")
    if not isinstance(route.get("paused"), bool):
        raise RuntimeError("route status must contain a boolean pause state")
    if route.get("active_reservation_count") != 0:
        raise RuntimeError("route migration requires zero active reservations")
    if route.get("export_entitlement_count") != 0:
        raise RuntimeError("route migration requires zero export entitlements")
    for field in ("route_epoch", "policy_epoch", "pricing_nav_epoch"):
        if not isinstance(route.get(field), int) or route[field] <= 0:
            raise RuntimeError(f"route status {field} must be a positive integer")


def main() -> None:
    args = parse_args()
    if args.output_dir.exists():
        raise RuntimeError(f"refusing to overwrite {args.output_dir}")
    if not args.reserve_key_file.is_file():
        raise RuntimeError("reserve-operator key file is unavailable")
    route, route_status_sha256 = load_json(args.route_status)
    validate_route(route)

    args.output_dir.mkdir(parents=True, mode=0o700)
    operations: dict[str, str] = {}
    if not route["paused"]:
        name = "01-pause-before-migration.ops.json"
        write_json(
            args.output_dir / name,
            pause_request(
                key_file=args.reserve_key_file,
                paused=True,
                label="a666-public-successor-pause-before-migration",
            ),
        )
        operations["pause_before_migration"] = name

    resume_name = "90-resume-after-verification.ops.json"
    write_json(
        args.output_dir / resume_name,
        pause_request(
            key_file=args.reserve_key_file,
            paused=False,
            label="a666-public-successor-resume-after-verification",
        ),
    )
    operations["resume_after_verification"] = resume_name

    emergency_name = "99-emergency-pause-after-resume.ops.json"
    write_json(
        args.output_dir / emergency_name,
        pause_request(
            key_file=args.reserve_key_file,
            paused=True,
            label="a666-public-successor-emergency-pause",
        ),
    )
    operations["emergency_pause_after_resume"] = emergency_name

    manifest = {
        "schema": "postfiat.a666.public_successor_migration_controls.v1",
        "route_id": ROUTE_ID,
        "native_nav_asset_id": ASSET_ID,
        "operator": RESERVE_OPERATOR,
        "route_status_sha256": route_status_sha256,
        "preconditions": {
            "paused": route["paused"],
            "live_value_enabled": route["live_value_enabled"],
            "route_epoch": route["route_epoch"],
            "policy_epoch": route["policy_epoch"],
            "pricing_nav_epoch": route["pricing_nav_epoch"],
            "active_reservation_count": route["active_reservation_count"],
            "export_entitlement_count": route["export_entitlement_count"],
        },
        "operations": operations,
        "execution_policy": (
            "pause before any profile or packet mutation; resume only after six-validator "
            "convergence and lifecycle verification; on any post-resume failure, submit the "
            "emergency pause and leave the route fail-closed"
        ),
    }
    write_json(args.output_dir / "migration-control-manifest.json", manifest)
    print(json.dumps(manifest, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
