#!/usr/bin/env python3
"""Assemble node-validated public H317/H318 epoch-4 operation payloads."""

from __future__ import annotations

import argparse
import importlib.util
import json
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
BASE_ASSEMBLER = (
    ROOT
    / "docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/pftl-payloads"
    / "assemble-payloads.py"
)
PROFILE_PATH = HERE / "package/planned-nav-profile.mainnet-epoch4.json"
BIND_PATH = HERE / "package/planned-nav-bind.mainnet-epoch4.json"
REGISTRATION_PATH = HERE / "package/route-registration.mainnet-epoch4.json"
C8_PATH = HERE / "package/consumer-c8-simulation.log"
NODE_PATH = (
    ROOT
    / "docs/evidence/pfusdc-eth-campaign-20260725/lane-a/rollout-stage/"
    "validator-stage/rootfs/opt/postfiat/releases/pfusdc-eth-l1-f30d368/postfiat-node"
)


def load_base():
    spec = importlib.util.spec_from_file_location("pfusdc_public_payload_base", BASE_ASSEMBLER)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load base assembler: {BASE_ASSEMBLER}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


B = load_base()


def source(path: Path) -> dict[str, str]:
    return {"path": str(path.relative_to(ROOT)), "sha256": B.sha256_file(path)}


def build(output_dir: Path, node: Path) -> None:
    profile = B.read_json(PROFILE_PATH)
    bind = B.read_json(BIND_PATH)
    registration = B.read_json(REGISTRATION_PATH)
    if "CONSUMER_C8: MATCH 15/15" not in C8_PATH.read_text(encoding="utf-8"):
        raise B.PayloadError("epoch-4 C8 consumer simulation is not 15/15")
    if (
        bind.get("height"),
        bind.get("route_epoch"),
        registration.get("height"),
        registration.get("activation_height"),
        registration.get("route_epoch"),
    ) != (317, 4, 318, 318, 4):
        raise B.PayloadError("epoch-4 bind/registration schedule is not H317/H318")
    if registration.get("route_id") != "ethereum-mainnet-usdc-v1":
        raise B.PayloadError("epoch-4 registration route id drifted")

    expected_asset_id = str(bind["asset_id"])
    profile_operation, asset_operation, profile_id = B.canonical_operations(
        node, profile, expected_asset_id
    )
    B.validate_profile_operation(profile_operation, profile)
    if asset_operation.get("operation") != "nav_asset_register":
        raise B.PayloadError("H317 second operation is not nav_asset_register")
    if asset_operation.get("asset_id") != expected_asset_id:
        raise B.PayloadError("H317 asset id differs from the planned bind")

    sources = {
        "nav_profile": source(PROFILE_PATH),
        "nav_bind": source(BIND_PATH),
        "route_registration": source(REGISTRATION_PATH),
        "c8": source(C8_PATH),
    }
    h317 = {
        "schema": "postfiat.public_unsigned_asset_operations.v1",
        "height": 317,
        "operation_count": 2,
        "operations": [
            {
                "dependencies": [],
                "label": "h317-nav-profile-register",
                "operation": profile_operation,
                "source": B.ISSUER,
                "source_object": sources["nav_profile"],
            },
            {
                "dependencies": [
                    {
                        "label": "h317-nav-profile-register",
                        "mode": "same_round",
                        "reason": (
                            "pfUSDC binds the epoch-4 NAV profile registered earlier "
                            "in the ordered H317 asset batch"
                        ),
                    }
                ],
                "label": "h317-nav-asset-bind",
                "operation": asset_operation,
                "planned_nav_bind": bind,
                "source": B.ISSUER,
                "source_object": sources["nav_bind"],
            },
        ],
        "profile_id": profile_id,
        "provenance": {
            "canonical_serializer": "postfiat-node vault-bridge-bootstrap-bundle",
            "c8": sources["c8"],
        },
        "signing_material_included": False,
    }
    h318 = {
        "schema": "postfiat.public_unsigned_route_registration.v1",
        "height": 318,
        "operation_count": 1,
        "operations": [
            {
                "dependencies": [],
                "label": "h318-epoch4-route-registration",
                "route_registration": registration,
                "source_object": sources["route_registration"],
            }
        ],
        "signing_material_included": False,
    }
    B.assert_no_key_material(h317)
    B.assert_no_key_material(h318)
    output_dir.mkdir(parents=True, exist_ok=True)
    h317_path = output_dir / "h317-bind-ops.public.json"
    h318_path = output_dir / "h318-register-ops.public.json"
    B.write_json(h317_path, h317)
    B.write_json(h318_path, h318)
    summary = {
        "schema": "postfiat.pfusdc.epoch4_public_payload_summary.v1",
        "verdict": "PASS",
        "contains_key_material": False,
        "c8": "15/15",
        "h317": {
            "height": 317,
            "operation_count": 2,
            "order": ["nav_profile_register", "nav_asset_bind"],
            "payload": h317_path.name,
            "sha256": B.sha256_file(h317_path),
        },
        "h318": {
            "height": 318,
            "operation_count": 1,
            "route_epoch": 4,
            "activation_height": 318,
            "route_id": "ethereum-mainnet-usdc-v1",
            "payload": h318_path.name,
            "sha256": B.sha256_file(h318_path),
        },
        "source_provenance": sources,
    }
    B.assert_no_key_material(summary)
    B.write_json(output_dir / "payload-summary.json", summary)

    rebuilt_profile, rebuilt_asset, rebuilt_profile_id = B.canonical_operations(
        node, profile, expected_asset_id
    )
    if h317["operations"][0]["operation"] != rebuilt_profile:
        raise B.PayloadError("H317 profile operation is not canonical on rebuild")
    if h317["operations"][1]["operation"] != rebuilt_asset:
        raise B.PayloadError("H317 asset operation is not canonical on rebuild")
    if h317["profile_id"] != rebuilt_profile_id:
        raise B.PayloadError("H317 profile id is not canonical on rebuild")
    if h318["operations"][0]["route_registration"] != registration:
        raise B.PayloadError("H318 registration differs from the generated source")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=HERE / "pftl-payloads")
    parser.add_argument("--node", type=Path, default=NODE_PATH)
    args = parser.parse_args()
    try:
        build(args.output_dir.resolve(), args.node.resolve())
    except B.PayloadError as exc:
        print(f"EPOCH4_PUBLIC_PAYLOAD_VALIDATION: FAIL: {exc}")
        return 1
    print("CANONICAL_SERIALIZER_VALIDATION: PASS")
    print("H317_ORDER: nav_profile_register,nav_asset_bind")
    print("H318_ROUTE: ethereum-mainnet-usdc-v1 epoch=4 activation=318")
    print("C8: 15/15")
    print("EPOCH4_PUBLIC_PAYLOAD_VALIDATION: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
