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
            "reported_settlement_reserve_atoms": 204_000_000,
            "excluded_unbacked_reserve_atoms": 0,
            "value_nav_units": 20_400_000_000,
            "active_bucket_backing_atoms": 205_036_000,
            "live_value_enabled": True,
            "paused": False,
        }
    ]


def test_primary_market_reserve_excludes_unbacked_amount() -> None:
    value, _, report = MODULE.build_overlay(
        route_status(204_000_001), vault_status(204_000_000)
    )
    assert value == 20_400_000_000
    assert report["primary_market_rows"][0]["settlement_reserve_atoms"] == 204_000_000
    assert report["primary_market_rows"][0]["reported_settlement_reserve_atoms"] == 204_000_001
    assert report["primary_market_rows"][0]["excluded_unbacked_reserve_atoms"] == 1


def test_packet_epoch_may_skip_shadow_only_epochs() -> None:
    packet = {
        "issuer": MODULE.ISSUER,
        "submitter": MODULE.RESERVE_OPERATOR,
        "asset_id": MODULE.ASSET_ID,
        "proof_profile": "11" * 48,
        "epoch": 7,
        "nav_per_unit": 90_000_000,
        "verified_net_assets": 2_800_000_000_000,
        "circulating_supply": 31_000_000_000,
        "source_root": "22" * 48,
        "attestor_root": "33" * 48,
        "reserve_packet_hash": "44" * 48,
        "sp1_proof_bytes": [1],
        "sp1_public_values": [0] * 584,
    }
    profile = {
        "profile_id": "11" * 48,
        "finalized_epoch": 5,
        "max_proof_bytes": 4096,
        "max_public_values_bytes": 584,
    }
    MODULE.validate_packet(packet, profile)


def test_packet_epoch_must_be_newer_than_finalized_epoch() -> None:
    packet = {
        "issuer": MODULE.ISSUER,
        "submitter": MODULE.RESERVE_OPERATOR,
        "asset_id": MODULE.ASSET_ID,
        "proof_profile": "11" * 48,
        "epoch": 5,
        "nav_per_unit": 90_000_000,
        "verified_net_assets": 2_800_000_000_000,
        "circulating_supply": 31_000_000_000,
        "source_root": "22" * 48,
        "attestor_root": "33" * 48,
        "reserve_packet_hash": "44" * 48,
        "sp1_proof_bytes": [1],
        "sp1_public_values": [0] * 584,
    }
    profile = {
        "profile_id": "11" * 48,
        "finalized_epoch": 5,
        "max_proof_bytes": 4096,
        "max_public_values_bytes": 584,
    }
    with pytest.raises(RuntimeError, match="newer than"):
        MODULE.validate_packet(packet, profile)
