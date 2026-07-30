#!/usr/bin/env python3
"""Build and verify the narrow A666/pfUSDC reserve demonstration.

This driver intentionally does not create an Ethereum export packet.  Its
state machine is:

    reserve -> subscribe -> release export entitlement
      -> fresh NAV mark -> route epoch advance -> partial/full redeem

Consensus submission remains delegated to ``a666-ce22-remote-finality-op.py``.
This file makes the operation packets and verifies the economic deltas between
authoritative readbacks.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import secrets
from pathlib import Path
from typing import Any


ROUTE_ID = "pftl-a666-ethereum-wA666-usdc-v1"
A666_ASSET_ID = (
    "521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b6"
    "2d20e18555642bec32174498cbee5e2c"
)
DEFAULT_SUBSCRIBER = "pfab9b9228942e5c529633a13aa271d5297bec6353"
DEFAULT_ETHEREUM_RECIPIENT = "0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0"
NAV_USD_E8_SCALE = 100_000_000
BPS_SCALE = 10_000
MAX_U64 = (1 << 64) - 1
HASH48_RE = re.compile(r"^[0-9a-f]{96}$")
PFTL_ACCOUNT_RE = re.compile(r"^pf[0-9a-f]{40}$")
EVM_ADDRESS_RE = re.compile(r"^0x[0-9a-fA-F]{40}$")


class DemoError(RuntimeError):
    """A fail-closed preparation or verification error."""


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    issue = subparsers.add_parser(
        "build-issue",
        help="build reserve, subscribe, and entitlement-release operations",
    )
    issue.add_argument("--route-status", type=Path, required=True)
    issue.add_argument("--nav-manifest", type=Path, required=True)
    issue.add_argument("--holder-key-file", type=Path, required=True)
    issue.add_argument("--output-dir", type=Path, required=True)
    issue.add_argument("--mint-amount-atoms", type=int, required=True)
    issue.add_argument("--current-height", type=int, required=True)
    issue.add_argument("--reservation-ttl-blocks", type=int, default=128)
    issue.add_argument("--subscriber", default=DEFAULT_SUBSCRIBER)
    issue.add_argument(
        "--ethereum-recipient",
        default=DEFAULT_ETHEREUM_RECIPIENT,
        help="reservation binding only; this demo does not export",
    )

    cleanup = subparsers.add_parser(
        "build-expired-releases",
        help="build one release operation for each expired export entitlement",
    )
    cleanup.add_argument("--entitlements-file", type=Path, required=True)
    cleanup.add_argument("--holder-key-file", type=Path, required=True)
    cleanup.add_argument("--output-dir", type=Path, required=True)
    cleanup.add_argument("--current-height", type=int, required=True)
    cleanup.add_argument("--releaser", default=DEFAULT_SUBSCRIBER)

    verify_cleanup = subparsers.add_parser(
        "verify-expired-releases",
        help="prove cleanup removed only the selected export entitlements",
    )
    verify_cleanup.add_argument("--before-route", type=Path, required=True)
    verify_cleanup.add_argument("--after-route", type=Path, required=True)
    verify_cleanup.add_argument("--cleanup-manifest", type=Path, required=True)
    verify_cleanup.add_argument("--output", type=Path, required=True)

    verify_issue = subparsers.add_parser(
        "verify-issue",
        help="verify subscription and entitlement release accounting",
    )
    verify_issue.add_argument("--before-route", type=Path, required=True)
    verify_issue.add_argument("--after-subscribe-route", type=Path, required=True)
    verify_issue.add_argument("--after-release-route", type=Path, required=True)
    verify_issue.add_argument("--before-pfusdc", type=Path, required=True)
    verify_issue.add_argument("--after-subscribe-pfusdc", type=Path, required=True)
    verify_issue.add_argument("--after-release-pfusdc", type=Path, required=True)
    verify_issue.add_argument("--before-a666", type=Path, required=True)
    verify_issue.add_argument("--after-subscribe-a666", type=Path, required=True)
    verify_issue.add_argument("--after-release-a666", type=Path, required=True)
    verify_issue.add_argument("--issue-manifest", type=Path, required=True)
    verify_issue.add_argument("--output", type=Path, required=True)

    redeem = subparsers.add_parser(
        "build-redeem",
        help="build a redemption bounded by the same-run incremental reserve",
    )
    redeem.add_argument("--route-status", type=Path, required=True)
    redeem.add_argument("--nav-manifest", type=Path, required=True)
    redeem.add_argument("--issue-manifest", type=Path, required=True)
    redeem.add_argument("--holder-key-file", type=Path, required=True)
    redeem.add_argument("--output-dir", type=Path, required=True)
    redeem.add_argument("--current-height", type=int, required=True)
    redeem.add_argument("--expiry-ttl-blocks", type=int, default=128)
    redeem.add_argument("--nav-amount-atoms", type=int)
    redeem.add_argument("--owner", default=DEFAULT_SUBSCRIBER)

    verify_redeem = subparsers.add_parser(
        "verify-redeem",
        help="verify A666 retirement and pfUSDC reserve release",
    )
    verify_redeem.add_argument("--before-route", type=Path, required=True)
    verify_redeem.add_argument("--after-route", type=Path, required=True)
    verify_redeem.add_argument("--before-pfusdc", type=Path, required=True)
    verify_redeem.add_argument("--after-pfusdc", type=Path, required=True)
    verify_redeem.add_argument("--before-a666", type=Path, required=True)
    verify_redeem.add_argument("--after-a666", type=Path, required=True)
    verify_redeem.add_argument("--redeem-manifest", type=Path, required=True)
    verify_redeem.add_argument("--output", type=Path, required=True)

    return parser.parse_args()


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise DemoError(f"cannot read JSON from {path}: {error}") from error


def write_json(path: Path, value: object, mode: int = 0o600) -> None:
    if path.exists():
        raise DemoError(f"refusing to overwrite {path}")
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    os.chmod(path, mode)


def make_output_dir(path: Path) -> None:
    if path.exists():
        raise DemoError(f"refusing to overwrite {path}")
    path.mkdir(parents=True, mode=0o700)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def require_positive_int(value: Any, label: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        raise DemoError(f"{label} must be a positive integer")
    if value > MAX_U64:
        raise DemoError(f"{label} exceeds the u64 protocol range")
    return value


def require_nonnegative_int(value: Any, label: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise DemoError(f"{label} must be a nonnegative integer")
    if value > MAX_U64:
        raise DemoError(f"{label} exceeds the u64 protocol range")
    return value


def checked_ceil_div(numerator: int, denominator: int, label: str) -> int:
    if numerator < 0 or denominator <= 0:
        raise DemoError(f"{label} inputs are invalid")
    result = (numerator + denominator - 1) // denominator
    if result > MAX_U64:
        raise DemoError(f"{label} exceeds the u64 protocol range")
    return result


def derive_issue_amounts(
    mint_amount_atoms: int,
    nav_per_unit_usd_1e8: int,
    issue_multiplier_bps: int,
) -> tuple[int, int, int]:
    amount = require_positive_int(mint_amount_atoms, "mint amount")
    nav = require_positive_int(nav_per_unit_usd_1e8, "NAV per unit")
    multiplier = require_positive_int(issue_multiplier_bps, "issue multiplier")
    if multiplier < BPS_SCALE:
        raise DemoError("issue multiplier cannot price below base NAV")
    base_value = checked_ceil_div(
        amount * nav,
        NAV_USD_E8_SCALE,
        "base NAV value",
    )
    settlement = checked_ceil_div(
        base_value * multiplier,
        BPS_SCALE,
        "issue settlement",
    )
    return base_value, settlement, settlement - base_value


def derive_redeem_amounts(
    nav_amount_atoms: int,
    nav_per_unit_usd_1e8: int,
    redeem_multiplier_bps: int,
) -> tuple[int, int, int]:
    amount = require_positive_int(nav_amount_atoms, "redemption amount")
    nav = require_positive_int(nav_per_unit_usd_1e8, "NAV per unit")
    multiplier = require_positive_int(redeem_multiplier_bps, "redeem multiplier")
    if multiplier > BPS_SCALE:
        raise DemoError("redeem multiplier cannot price above base NAV")
    base_value = checked_ceil_div(
        amount * nav,
        NAV_USD_E8_SCALE,
        "redemption base NAV value",
    )
    settlement_output = base_value * multiplier // BPS_SCALE
    if settlement_output <= 0:
        raise DemoError("redemption output rounds to zero")
    return base_value, settlement_output, base_value - settlement_output


def validate_account(account: str, label: str) -> None:
    if not PFTL_ACCOUNT_RE.fullmatch(account):
        raise DemoError(f"{label} is not a canonical PFTL account")


def validate_route(route: dict[str, Any]) -> None:
    required = {
        "schema": "postfiat-pftl-uniswap-supply-status-v2",
        "route_id": ROUTE_ID,
        "native_nav_asset_id": A666_ASSET_ID,
        "live_value_enabled": True,
        "paused": False,
        "invariant_holds": True,
        "route_schema_version": 2,
        "outbound_verification_class": "TRUSTLESS_FINALITY",
        "return_verification_class": "BFT_CHECKPOINT",
        "ethereum_chain_id": 1,
    }
    for field, expected in required.items():
        if route.get(field) != expected:
            raise DemoError(f"route status {field} differs from {expected!r}")


def validate_nav_binding(
    route: dict[str, Any], nav: dict[str, Any]
) -> tuple[int, int, int]:
    required = {
        "schema": "postfiat.a666.live_nav_mark.v1",
        "asset_id": A666_ASSET_ID,
        "epoch": route["pricing_nav_epoch"],
        "reserve_packet_hash": route["pricing_reserve_packet_hash"],
        "opening_constants_used": False,
        "uniswap_price_used": False,
    }
    for field, expected in required.items():
        if nav.get(field) != expected:
            raise DemoError(f"NAV manifest {field} differs from {expected!r}")
    nav_per_unit = require_positive_int(
        nav.get("nav_per_unit_usd_1e8"),
        "NAV manifest nav_per_unit_usd_1e8",
    )
    circulating_supply = require_positive_int(
        nav.get("circulating_supply_atoms"),
        "NAV manifest circulating_supply_atoms",
    )
    verified_assets = require_positive_int(
        nav.get("verified_net_assets_usd_1e8"),
        "NAV manifest verified_net_assets_usd_1e8",
    )
    return nav_per_unit, circulating_supply, verified_assets


def operation_request(
    label: str,
    source: str,
    key_file: Path,
    operation: dict[str, Any],
) -> dict[str, Any]:
    return {
        "schema": "postfiat-certified-asset-ops-request-v1",
        "operations": [
            {
                "label": label,
                "source": source,
                "key_file": str(key_file.resolve()),
                "operation": operation,
            }
        ],
    }


def ensure_key(path: Path) -> None:
    if not path.is_file():
        raise DemoError(f"signing key is unavailable: {path}")


def cmd_build_issue(args: argparse.Namespace) -> dict[str, Any]:
    ensure_key(args.holder_key_file)
    validate_account(args.subscriber, "subscriber")
    if not EVM_ADDRESS_RE.fullmatch(args.ethereum_recipient):
        raise DemoError("Ethereum recipient is not a canonical address")
    route = load_json(args.route_status)
    nav = load_json(args.nav_manifest)
    validate_route(route)
    nav_per_unit, circulating_supply, verified_assets = validate_nav_binding(
        route, nav
    )
    current_height = require_positive_int(args.current_height, "current height")
    ttl = require_positive_int(args.reservation_ttl_blocks, "reservation TTL")
    expires_at_height = current_height + ttl
    if expires_at_height >= require_positive_int(
        route["policy_expires_at_height"], "policy expiry"
    ):
        raise DemoError("reservation would reach or exceed policy expiry")
    if route["active_reservation_count"] != 0:
        raise DemoError("route has an active reservation before demo issue")
    amount = require_positive_int(args.mint_amount_atoms, "mint amount")
    if not route["min_order_atoms"] <= amount <= route["max_order_atoms"]:
        raise DemoError("mint amount is outside governed order bounds")
    if amount > route["available_issue_atoms"]:
        raise DemoError("mint amount exceeds available issue capacity")
    base_value, settlement, spread = derive_issue_amounts(
        amount,
        nav_per_unit,
        route["issue_multiplier_bps"],
    )
    reservation_id = secrets.token_hex(48)
    subscription_nonce = secrets.token_hex(32)
    reserve = {
        "operation": "pftl_uniswap_order_reserve",
        "subscriber": args.subscriber,
        "route_id": ROUTE_ID,
        "reservation_id": reservation_id,
        "ethereum_recipient": args.ethereum_recipient.lower(),
        "route_epoch": route["route_epoch"],
        "policy_epoch": route["policy_epoch"],
        "policy_hash": route["policy_hash"],
        "mint_amount_atoms": amount,
        "max_settlement_value_atoms": settlement,
        "expires_at_height": expires_at_height,
    }
    subscribe = {
        "operation": "pftl_uniswap_primary_subscribe_v2",
        "subscriber": args.subscriber,
        "route_id": ROUTE_ID,
        "reservation_id": reservation_id,
        "subscription_nonce": subscription_nonce,
        "settlement_asset_id": route["settlement_asset_id"],
        "settlement_value_atoms": settlement,
        "pricing_nav_epoch": route["pricing_nav_epoch"],
        "pricing_reserve_packet_hash": route["pricing_reserve_packet_hash"],
    }
    release = {
        "operation": "pftl_uniswap_order_release",
        "releaser": args.subscriber,
        "route_id": ROUTE_ID,
        "reservation_id": reservation_id,
    }
    manifest = {
        "schema": "postfiat.a666.pfusdc_reserve_demo_issue.v1",
        "route_id": ROUTE_ID,
        "subscriber": args.subscriber,
        "ethereum_recipient_binding": args.ethereum_recipient.lower(),
        "current_height": current_height,
        "reservation_expires_at_height": expires_at_height,
        "route_epoch": route["route_epoch"],
        "policy_epoch": route["policy_epoch"],
        "policy_hash": route["policy_hash"],
        "pricing_nav_epoch": route["pricing_nav_epoch"],
        "pricing_reserve_packet_hash": route["pricing_reserve_packet_hash"],
        "nav_per_unit_usd_1e8": nav_per_unit,
        "nav_circulating_supply_atoms": circulating_supply,
        "nav_verified_net_assets_usd_1e8": verified_assets,
        "mint_amount_atoms": amount,
        "base_value_atoms": base_value,
        "settlement_value_atoms": settlement,
        "issue_spread_atoms": spread,
        "reservation_id": reservation_id,
        "subscription_nonce": subscription_nonce,
        "creates_ethereum_export": False,
        "input_sha256": {
            "route_status": sha256_file(args.route_status),
            "nav_manifest": sha256_file(args.nav_manifest),
        },
        "files": {
            "reserve": "01-reserve.ops.json",
            "subscribe": "02-subscribe.ops.json",
            "release": "03-release-entitlement.ops.json",
        },
    }
    make_output_dir(args.output_dir)
    write_json(
        args.output_dir / "01-reserve.ops.json",
        operation_request(
            "a666-reserve-demo-reserve",
            args.subscriber,
            args.holder_key_file,
            reserve,
        ),
    )
    write_json(
        args.output_dir / "02-subscribe.ops.json",
        operation_request(
            "a666-reserve-demo-subscribe",
            args.subscriber,
            args.holder_key_file,
            subscribe,
        ),
    )
    write_json(
        args.output_dir / "03-release-entitlement.ops.json",
        operation_request(
            "a666-reserve-demo-release",
            args.subscriber,
            args.holder_key_file,
            release,
        ),
    )
    write_json(args.output_dir / "issue-manifest.json", manifest)
    return manifest


def normalize_entitlements(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list) or not value:
        raise DemoError("entitlements file must contain a non-empty JSON array")
    normalized: list[dict[str, Any]] = []
    seen: set[str] = set()
    for index, row in enumerate(value):
        if not isinstance(row, dict):
            raise DemoError(f"entitlement {index} is not an object")
        reservation_id = row.get("reservation_id")
        if not isinstance(reservation_id, str) or not HASH48_RE.fullmatch(
            reservation_id
        ):
            raise DemoError(f"entitlement {index} has an invalid reservation id")
        if reservation_id in seen:
            raise DemoError("entitlements file contains a duplicate reservation id")
        seen.add(reservation_id)
        subscriber = row.get("subscriber")
        if not isinstance(subscriber, str):
            raise DemoError(f"entitlement {index} has no subscriber")
        validate_account(subscriber, f"entitlement {index} subscriber")
        normalized.append(
            {
                "reservation_id": reservation_id,
                "subscriber": subscriber,
                "remaining_amount_atoms": require_positive_int(
                    row.get("remaining_amount_atoms"),
                    f"entitlement {index} remaining amount",
                ),
                "expires_at_height": require_positive_int(
                    row.get("expires_at_height"),
                    f"entitlement {index} expiry",
                ),
            }
        )
    normalized.sort(key=lambda row: row["reservation_id"])
    return normalized


def cmd_build_expired_releases(args: argparse.Namespace) -> dict[str, Any]:
    ensure_key(args.holder_key_file)
    validate_account(args.releaser, "releaser")
    current_height = require_positive_int(args.current_height, "current height")
    entitlements = normalize_entitlements(load_json(args.entitlements_file))
    for row in entitlements:
        if current_height <= row["expires_at_height"]:
            raise DemoError(
                "cleanup is restricted to expired entitlements; "
                f"{row['reservation_id']} has not expired"
            )
    manifest = {
        "schema": "postfiat.a666.expired_export_entitlement_cleanup.v1",
        "route_id": ROUTE_ID,
        "releaser": args.releaser,
        "current_height": current_height,
        "entitlement_count": len(entitlements),
        "entitlement_atoms": sum(
            row["remaining_amount_atoms"] for row in entitlements
        ),
        "entitlements": entitlements,
        "source_sha256": sha256_file(args.entitlements_file),
        "files": [],
    }
    make_output_dir(args.output_dir)
    for index, row in enumerate(entitlements, start=1):
        filename = f"{index:03d}-release-{row['reservation_id'][:12]}.ops.json"
        request = operation_request(
            f"a666-expired-release-{index:03d}-{row['reservation_id'][:8]}",
            args.releaser,
            args.holder_key_file,
            {
                "operation": "pftl_uniswap_order_release",
                "releaser": args.releaser,
                "route_id": ROUTE_ID,
                "reservation_id": row["reservation_id"],
            },
        )
        write_json(args.output_dir / filename, request)
        manifest["files"].append(filename)
    write_json(args.output_dir / "cleanup-manifest.json", manifest)
    return manifest


def route_counter(route: dict[str, Any], field: str) -> int:
    return require_nonnegative_int(route.get(field), f"route {field}")


def assert_equal(actual: Any, expected: Any, label: str) -> None:
    if actual != expected:
        raise DemoError(f"{label}: expected {expected!r}, observed {actual!r}")


def economic_route_snapshot(route: dict[str, Any]) -> dict[str, int]:
    fields = (
        "authorized_valid_supply_atoms",
        "pftl_spendable_supply_atoms",
        "ethereum_spendable_supply_atoms",
        "other_registered_venue_supply_atoms",
        "outstanding_bridge_claims_atoms",
        "settlement_reserve_atoms",
        "non_nav_spread_atoms",
        "active_reservation_count",
        "active_reservation_atoms",
        "export_entitlement_count",
        "export_entitlement_atoms",
    )
    return {field: route_counter(route, field) for field in fields}


def cmd_verify_expired_releases(args: argparse.Namespace) -> dict[str, Any]:
    before = load_json(args.before_route)
    after = load_json(args.after_route)
    manifest = load_json(args.cleanup_manifest)
    validate_route(before)
    validate_route(after)
    if (
        manifest.get("schema")
        != "postfiat.a666.expired_export_entitlement_cleanup.v1"
    ):
        raise DemoError("cleanup manifest schema mismatch")
    before_economic = economic_route_snapshot(before)
    after_economic = economic_route_snapshot(after)
    count = require_positive_int(
        manifest.get("entitlement_count"), "cleanup entitlement count"
    )
    atoms = require_positive_int(
        manifest.get("entitlement_atoms"), "cleanup entitlement atoms"
    )
    assert_equal(
        after_economic["export_entitlement_count"],
        before_economic["export_entitlement_count"] - count,
        "export entitlement count after cleanup",
    )
    assert_equal(
        after_economic["export_entitlement_atoms"],
        before_economic["export_entitlement_atoms"] - atoms,
        "export entitlement atoms after cleanup",
    )
    unchanged = set(before_economic) - {
        "export_entitlement_count",
        "export_entitlement_atoms",
    }
    for field in sorted(unchanged):
        assert_equal(
            after_economic[field],
            before_economic[field],
            f"{field} changed during cleanup",
        )
    report = {
        "schema": "postfiat.a666.expired_export_entitlement_cleanup_verify.v1",
        "verdict": "PASS",
        "removed_count": count,
        "removed_atoms": atoms,
        "remaining_count": after_economic["export_entitlement_count"],
        "remaining_atoms": after_economic["export_entitlement_atoms"],
        "economic_state_unchanged": True,
    }
    write_json(args.output, report, 0o644)
    return report


def account_balance(report: dict[str, Any], expected_asset_id: str) -> int:
    if report.get("schema") != "postfiat-account-assets-v1":
        raise DemoError("account balance report schema mismatch")
    if report.get("asset_id") != expected_asset_id:
        raise DemoError("account balance report describes the wrong asset")
    assets = report.get("assets")
    if not isinstance(assets, list):
        raise DemoError("account balance report assets must be an array")
    total = 0
    for row in assets:
        if row.get("asset_id") != expected_asset_id:
            raise DemoError("account balance row describes the wrong asset")
        total += require_nonnegative_int(row.get("balance"), "account balance")
    return total


def cmd_verify_issue(args: argparse.Namespace) -> dict[str, Any]:
    before = load_json(args.before_route)
    subscribed = load_json(args.after_subscribe_route)
    released = load_json(args.after_release_route)
    manifest = load_json(args.issue_manifest)
    for route in (before, subscribed, released):
        validate_route(route)
    if manifest.get("schema") != "postfiat.a666.pfusdc_reserve_demo_issue.v1":
        raise DemoError("issue manifest schema mismatch")
    amount = require_positive_int(manifest["mint_amount_atoms"], "mint amount")
    base = require_positive_int(manifest["base_value_atoms"], "base value")
    settlement = require_positive_int(
        manifest["settlement_value_atoms"], "settlement value"
    )
    spread = require_nonnegative_int(manifest["issue_spread_atoms"], "issue spread")
    assert_equal(settlement, base + spread, "issue settlement decomposition")
    before_state = economic_route_snapshot(before)
    subscribed_state = economic_route_snapshot(subscribed)
    released_state = economic_route_snapshot(released)
    deltas = {
        "authorized_valid_supply_atoms": amount,
        "pftl_spendable_supply_atoms": amount,
        "settlement_reserve_atoms": base,
        "non_nav_spread_atoms": spread,
        "export_entitlement_count": 1,
        "export_entitlement_atoms": amount,
    }
    for field, delta in deltas.items():
        assert_equal(
            subscribed_state[field],
            before_state[field] + delta,
            f"post-subscribe {field}",
        )
    for field in (
        "ethereum_spendable_supply_atoms",
        "other_registered_venue_supply_atoms",
        "outstanding_bridge_claims_atoms",
        "active_reservation_count",
        "active_reservation_atoms",
    ):
        assert_equal(
            subscribed_state[field],
            before_state[field],
            f"post-subscribe {field}",
        )
    for field in released_state:
        expected = (
            before_state[field]
            if field in {"export_entitlement_count", "export_entitlement_atoms"}
            else subscribed_state[field]
        )
        assert_equal(released_state[field], expected, f"post-release {field}")
    settlement_asset = before["settlement_asset_id"]
    balances = {
        "pfusdc_before": account_balance(load_json(args.before_pfusdc), settlement_asset),
        "pfusdc_after_subscribe": account_balance(
            load_json(args.after_subscribe_pfusdc), settlement_asset
        ),
        "pfusdc_after_release": account_balance(
            load_json(args.after_release_pfusdc), settlement_asset
        ),
        "a666_before": account_balance(load_json(args.before_a666), A666_ASSET_ID),
        "a666_after_subscribe": account_balance(
            load_json(args.after_subscribe_a666), A666_ASSET_ID
        ),
        "a666_after_release": account_balance(
            load_json(args.after_release_a666), A666_ASSET_ID
        ),
    }
    assert_equal(
        balances["pfusdc_after_subscribe"],
        balances["pfusdc_before"] - settlement,
        "subscriber pfUSDC after subscribe",
    )
    assert_equal(
        balances["pfusdc_after_release"],
        balances["pfusdc_after_subscribe"],
        "subscriber pfUSDC after entitlement release",
    )
    assert_equal(
        balances["a666_after_subscribe"],
        balances["a666_before"] + amount,
        "subscriber A666 after subscribe",
    )
    assert_equal(
        balances["a666_after_release"],
        balances["a666_after_subscribe"],
        "subscriber A666 after entitlement release",
    )
    report = {
        "schema": "postfiat.a666.pfusdc_reserve_demo_issue_verify.v1",
        "verdict": "PASS",
        "mint_amount_atoms": amount,
        "base_reserve_increase_atoms": base,
        "issue_spread_atoms": spread,
        "export_entitlement_released": True,
        "creates_ethereum_export": False,
        "balances": balances,
    }
    write_json(args.output, report, 0o644)
    return report


def maximum_nav_for_base_reserve(reserve_atoms: int, nav_usd_e8: int) -> int:
    reserve = require_positive_int(reserve_atoms, "incremental reserve")
    nav = require_positive_int(nav_usd_e8, "NAV per unit")
    candidate = reserve * NAV_USD_E8_SCALE // nav
    while candidate > 0:
        base, _, _ = derive_redeem_amounts(candidate, nav, BPS_SCALE)
        if base <= reserve:
            return candidate
        candidate -= 1
    raise DemoError("incremental reserve cannot support a nonzero redemption")


def cmd_build_redeem(args: argparse.Namespace) -> dict[str, Any]:
    ensure_key(args.holder_key_file)
    validate_account(args.owner, "redemption owner")
    route = load_json(args.route_status)
    nav = load_json(args.nav_manifest)
    issue = load_json(args.issue_manifest)
    validate_route(route)
    nav_per_unit, _, _ = validate_nav_binding(route, nav)
    if issue.get("schema") != "postfiat.a666.pfusdc_reserve_demo_issue.v1":
        raise DemoError("issue manifest schema mismatch")
    if issue.get("subscriber") != args.owner:
        raise DemoError("redemption owner differs from the issue subscriber")
    if route["active_reservation_count"] or route["export_entitlement_count"]:
        raise DemoError("route has active order state before redemption")
    current_height = require_positive_int(args.current_height, "current height")
    ttl = require_positive_int(args.expiry_ttl_blocks, "redemption expiry TTL")
    expires_at_height = current_height + ttl
    if expires_at_height >= require_positive_int(
        route["policy_expires_at_height"], "policy expiry"
    ):
        raise DemoError("redemption would reach or exceed policy expiry")
    issued = require_positive_int(issue["mint_amount_atoms"], "issued amount")
    incremental_reserve = require_positive_int(
        issue["base_value_atoms"], "same-run incremental base reserve"
    )
    max_from_incremental = maximum_nav_for_base_reserve(
        incremental_reserve, nav_per_unit
    )
    maximum = min(
        issued,
        max_from_incremental,
        require_nonnegative_int(route["available_redeem_atoms"], "available redeem"),
        require_nonnegative_int(
            route["redeem_capacity_remaining_atoms"], "redeem capacity"
        ),
        require_positive_int(route["max_order_atoms"], "maximum order"),
    )
    requested = args.nav_amount_atoms if args.nav_amount_atoms is not None else maximum
    amount = require_positive_int(requested, "requested redemption amount")
    if amount > maximum:
        raise DemoError(
            f"requested redemption {amount} exceeds same-run safe maximum {maximum}"
        )
    if amount < require_positive_int(route["min_order_atoms"], "minimum order"):
        raise DemoError("redemption amount is below the governed minimum")
    base_value, settlement_output, spread = derive_redeem_amounts(
        amount,
        nav_per_unit,
        route["redeem_multiplier_bps"],
    )
    if base_value > incremental_reserve:
        raise DemoError("redemption consumes more than the same-run incremental reserve")
    nonce = secrets.token_hex(32)
    body = {
        "operation": "pftl_uniswap_primary_redeem",
        "owner": args.owner,
        "settlement_recipient": args.owner,
        "route_id": ROUTE_ID,
        "redemption_nonce": nonce,
        "nav_amount_atoms": amount,
        "min_settlement_value_atoms": settlement_output,
        "route_epoch": route["route_epoch"],
        "policy_epoch": route["policy_epoch"],
        "policy_hash": route["policy_hash"],
        "pricing_nav_epoch": route["pricing_nav_epoch"],
        "pricing_reserve_packet_hash": route["pricing_reserve_packet_hash"],
        "expires_at_height": expires_at_height,
    }
    manifest = {
        "schema": "postfiat.a666.pfusdc_reserve_demo_redeem.v1",
        "route_id": ROUTE_ID,
        "owner": args.owner,
        "route_epoch": route["route_epoch"],
        "policy_epoch": route["policy_epoch"],
        "policy_hash": route["policy_hash"],
        "pricing_nav_epoch": route["pricing_nav_epoch"],
        "pricing_reserve_packet_hash": route["pricing_reserve_packet_hash"],
        "nav_per_unit_usd_1e8": nav_per_unit,
        "issued_amount_atoms": issued,
        "incremental_base_reserve_atoms": incremental_reserve,
        "maximum_same_run_redeem_atoms": maximum,
        "nav_amount_atoms": amount,
        "base_value_atoms": base_value,
        "settlement_output_atoms": settlement_output,
        "redemption_spread_atoms": spread,
        "redemption_nonce": nonce,
        "expires_at_height": expires_at_height,
        "retained_a666_atoms": issued - amount,
        "retained_same_run_reserve_atoms": incremental_reserve - base_value,
        "input_sha256": {
            "route_status": sha256_file(args.route_status),
            "nav_manifest": sha256_file(args.nav_manifest),
            "issue_manifest": sha256_file(args.issue_manifest),
        },
    }
    make_output_dir(args.output_dir)
    write_json(
        args.output_dir / "primary-redeem.ops.json",
        operation_request(
            "a666-reserve-demo-redeem",
            args.owner,
            args.holder_key_file,
            body,
        ),
    )
    write_json(args.output_dir / "redeem-manifest.json", manifest)
    return manifest


def cmd_verify_redeem(args: argparse.Namespace) -> dict[str, Any]:
    before = load_json(args.before_route)
    after = load_json(args.after_route)
    manifest = load_json(args.redeem_manifest)
    validate_route(before)
    validate_route(after)
    if manifest.get("schema") != "postfiat.a666.pfusdc_reserve_demo_redeem.v1":
        raise DemoError("redeem manifest schema mismatch")
    amount = require_positive_int(manifest["nav_amount_atoms"], "redeemed amount")
    base = require_positive_int(manifest["base_value_atoms"], "redeem base value")
    output = require_positive_int(
        manifest["settlement_output_atoms"], "settlement output"
    )
    spread = require_nonnegative_int(
        manifest["redemption_spread_atoms"], "redemption spread"
    )
    assert_equal(base, output + spread, "redemption value decomposition")
    before_state = economic_route_snapshot(before)
    after_state = economic_route_snapshot(after)
    deltas = {
        "authorized_valid_supply_atoms": -amount,
        "pftl_spendable_supply_atoms": -amount,
        "settlement_reserve_atoms": -base,
        "non_nav_spread_atoms": spread,
    }
    for field, delta in deltas.items():
        assert_equal(
            after_state[field],
            before_state[field] + delta,
            f"post-redeem {field}",
        )
    for field in (
        "ethereum_spendable_supply_atoms",
        "other_registered_venue_supply_atoms",
        "outstanding_bridge_claims_atoms",
        "active_reservation_count",
        "active_reservation_atoms",
        "export_entitlement_count",
        "export_entitlement_atoms",
    ):
        assert_equal(after_state[field], before_state[field], f"post-redeem {field}")
    if after_state["active_reservation_count"] or after_state["export_entitlement_count"]:
        raise DemoError("route retained active order state after redemption")
    settlement_asset = before["settlement_asset_id"]
    balances = {
        "pfusdc_before": account_balance(load_json(args.before_pfusdc), settlement_asset),
        "pfusdc_after": account_balance(load_json(args.after_pfusdc), settlement_asset),
        "a666_before": account_balance(load_json(args.before_a666), A666_ASSET_ID),
        "a666_after": account_balance(load_json(args.after_a666), A666_ASSET_ID),
    }
    assert_equal(
        balances["pfusdc_after"],
        balances["pfusdc_before"] + output,
        "owner pfUSDC after redemption",
    )
    assert_equal(
        balances["a666_after"],
        balances["a666_before"] - amount,
        "owner A666 after redemption",
    )
    report = {
        "schema": "postfiat.a666.pfusdc_reserve_demo_redeem_verify.v1",
        "verdict": "PASS",
        "retired_a666_atoms": amount,
        "released_pfusdc_atoms": output,
        "base_reserve_decrease_atoms": base,
        "redemption_spread_atoms": spread,
        "retained_a666_atoms": manifest["retained_a666_atoms"],
        "retained_same_run_reserve_atoms": manifest[
            "retained_same_run_reserve_atoms"
        ],
        "zero_active_order_state": True,
        "balances": balances,
    }
    write_json(args.output, report, 0o644)
    return report


def main() -> None:
    args = parse_args()
    commands = {
        "build-issue": cmd_build_issue,
        "build-expired-releases": cmd_build_expired_releases,
        "verify-expired-releases": cmd_verify_expired_releases,
        "verify-issue": cmd_verify_issue,
        "build-redeem": cmd_build_redeem,
        "verify-redeem": cmd_verify_redeem,
    }
    result = commands[args.command](args)
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
