import importlib.util
from pathlib import Path

import pytest


SCRIPT = Path(__file__).parents[2] / "scripts" / "a666-build-live-nav-mark-ops.py"
SPEC = importlib.util.spec_from_file_location("a666_live_nav_mark", SCRIPT)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def route_status(reserve: int) -> dict:
    return {
        "route_id": "pftl-a666-ethereum-wA666-usdc-v1",
        "route_config_digest": "12" * 48,
        "settlement_asset_id": "02" * 48,
        "settlement_reserve_atoms": reserve,
        "live_value_enabled": True,
        "paused": False,
    }


def vault_status(backing: int) -> dict:
    return {
        "asset_id": "02" * 48,
        "valuation_unit": "USDC",
        "allocations": [],
        "receipts": [],
        "buckets": [
            {
                "bucket_id": "03" * 48,
                "status": "active",
                "outstanding_vault_bridge_atoms": backing,
            }
        ],
    }


def test_primary_market_reserve_is_scaled_and_bound() -> None:
    value, root, report = MODULE.build_overlay(
        route_status(204_000_000), vault_status(205_036_000)
    )
    assert value == 20_400_000_000
    assert len(root) == 96
    assert report["primary_market_rows"] == [
        {
            "route_id": "pftl-a666-ethereum-wA666-usdc-v1",
            "route_config_digest": "12" * 48,
            "settlement_asset_id": "02" * 48,
            "settlement_reserve_atoms": 204_000_000,
            "value_nav_units": 20_400_000_000,
            "active_bucket_backing_atoms": 205_036_000,
            "live_value_enabled": True,
            "paused": False,
        }
    ]


def test_primary_market_reserve_cannot_exceed_vault_backing() -> None:
    with pytest.raises(RuntimeError, match="exceeds proof-backed vault backing"):
        MODULE.build_overlay(route_status(204_000_001), vault_status(204_000_000))
