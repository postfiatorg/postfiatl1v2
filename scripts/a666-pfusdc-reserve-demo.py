#!/usr/bin/env python3
"""Build and verify the narrow A666/pfUSDC reserve demonstration.

This driver intentionally does not create an Ethereum export packet.  Its
state machine is:

    reserve -> subscribe -> release export entitlement
      -> fresh NAV mark -> route epoch advance -> partial/full redeem

Consensus submission remains delegated to ``a666-ce22-remote-finality-op.py``.
This file makes the operation packets and verifies the economic deltas between
authoritative readbacks.

Required inputs
---------------
Every build command requires --identities with schema
postfiat.reserve_demo_identities.v1 and these public fields:
route_id, native_nav_asset_id, settlement_asset_id (family),
settlement_source_asset_id, source_bucket_id, source_profile_hash,
pftl_chain_id, source_chain_id, source_vault_address, source_token_address,
source_route_epoch, ethereum_chain_id (export route), outbound_verification_class,
and return_verification_class. Also pin nav_profile_id, nav_source_manifest_hash,
nav_valuation_policy_hash, nav_program_vkey, nav_public_values_schema
(postfiat.nav_reserve_public_values.v1), and nav_valuation_unit (USD_1E8).
The source profile hash is the bridge route-profile
hash, not its verifier policy or the NAV proof profile. Source-series and bucket
IDs must match the canonical chain/family/source tuple.

build-issue also requires --subscriber and --ethereum-recipient (reservation
binding); build-redeem requires --owner; cleanup requires --releaser. Existing
--route-status, --nav-manifest, amount/height, and output arguments still apply.
Verification consumes identities embedded in the corresponding build manifest.
Missing or mismatched identities and existing outputs fail closed. No production
identity is supplied implicitly. --holder-key-file is a signer-local file path:
this driver checks its existence but never reads or copies its contents, signs,
or submits. There is no private-key command-line argument.

The current NAV builder's postfiat.a666.provider_neutral_nav_mark.v1 schema uses
nav_per_unit and verified_net_assets; its values are USD_1E8. The legacy
postfiat.a666.live_nav_mark.v1 fields remain supported with their original false
opening_constants_used/uniswap_price_used flags and explicit proof identities.
Unknown schemas, mismatched asset/epoch/packet/profile/source/valuation/key, and
contradictory units fail before output. A fresh NAV packet and advanced route
epoch are required for redemption.

Route readbacks must include source_settlement_custody: issue requires the exact
source's enabled row; redemption consumes that row's principal, excluding spread,
reservation escrow, and other sources. Redemption base is bounded by both the
same-cycle issue base reserve and remaining source principal, plus aggregate
reserve, native wallet balance, and policy/order limits. Default redemption is
clamped; an explicit excess fails before operation construction. Verification
checks exact source balances and principal/spread/escrow deltas; output plus
redemption spread equals reserve reduction. Whole route-readback SHA-256 hashes
include the custody rows; ledger_hash alone does not bind them. Finalized-state,
receipt, family-supply, and composite-overlay evidence belongs to the G3 cycle
manifest/verifier still to be built; these local checks do not establish it.
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


IDENTITY_SCHEMA = "postfiat.reserve_demo_identities.v1"
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
    issue.add_argument("--identities", type=Path, required=True)
    issue.add_argument("--subscriber", required=True)
    issue.add_argument(
        "--ethereum-recipient",
        required=True,
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
    cleanup.add_argument("--identities", type=Path, required=True)
    cleanup.add_argument("--releaser", required=True)

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
    redeem.add_argument("--identities", type=Path, required=True)
    redeem.add_argument("--owner", required=True)

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
    try:
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, mode)
    except FileExistsError as error:
        raise DemoError(f"refusing to overwrite {path}") from error
    with os.fdopen(fd, "w") as output:
        output.write(json.dumps(value, indent=2, sort_keys=True) + "\n")


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
    if not isinstance(account, str) or not PFTL_ACCOUNT_RE.fullmatch(account):
        raise DemoError(f"{label} is not a canonical PFTL account")


def validate_identities(value: Any) -> dict[str, Any]:
    """Validate only public identities; never copy arbitrary input into evidence."""
    if not isinstance(value, dict) or value.get("schema") != IDENTITY_SCHEMA:
        raise DemoError(f"identities must use schema {IDENTITY_SCHEMA}")
    text_fields = (
        "route_id", "pftl_chain_id", "outbound_verification_class",
        "return_verification_class", "nav_public_values_schema", "nav_valuation_unit",
    )
    hash_fields = (
        "native_nav_asset_id", "settlement_asset_id", "settlement_source_asset_id",
        "source_bucket_id", "source_profile_hash", "nav_profile_id",
        "nav_source_manifest_hash",
    )
    address_fields = ("source_vault_address", "source_token_address")
    int_fields = ("ethereum_chain_id", "source_chain_id", "source_route_epoch")
    fields = ("schema", "nav_valuation_policy_hash", "nav_program_vkey") + text_fields + hash_fields + address_fields + int_fields
    missing = [field for field in fields if field not in value]
    if missing:
        raise DemoError("missing explicit identities: " + ", ".join(missing))
    if set(value) - set(fields):
        raise DemoError("unknown identity fields; only public identity fields are allowed")
    for field in text_fields:
        item = value[field]
        if (
            not isinstance(item, str) or not item or item != item.strip()
            or len(item.encode()) > 256 or any(ord(c) < 32 or ord(c) == 127 for c in item)
        ):
            raise DemoError(f"identity {field} must be nonempty canonical text")
    for field in hash_fields:
        if not isinstance(value[field], str) or not HASH48_RE.fullmatch(value[field]):
            raise DemoError(f"identity {field} must be 96 lowercase hex characters")
    if not isinstance(value["nav_valuation_policy_hash"], str) or not re.fullmatch(
        r"[0-9a-f]{64}", value["nav_valuation_policy_hash"]
    ):
        raise DemoError("identity nav_valuation_policy_hash must be 64 lowercase hex characters")
    if not isinstance(value["nav_program_vkey"], str) or not re.fullmatch(
        r"0x[0-9a-f]{64}", value["nav_program_vkey"]
    ):
        raise DemoError("identity nav_program_vkey must be a 0x-prefixed 32-byte key")
    if value["nav_valuation_unit"] != "USD_1E8":
        raise DemoError("reserve demo requires explicit NAV valuation unit USD_1E8")
    if value["nav_public_values_schema"] != "postfiat.nav_reserve_public_values.v1":
        raise DemoError("unsupported NAV public-values schema")
    for field in address_fields:
        if (
            not isinstance(value[field], str)
            or not EVM_ADDRESS_RE.fullmatch(value[field])
            or value[field] != value[field].lower()
        ):
            raise DemoError(f"identity {field} must be a lowercase EVM address")
    for field in int_fields:
        require_positive_int(value[field], f"identity {field}")
    if value["source_route_epoch"] > (1 << 32) - 1:
        raise DemoError("source route epoch exceeds u32")
    family = value["settlement_asset_id"]
    chain = value["pftl_chain_id"]
    source_chain = value["source_chain_id"]
    vault = value["source_vault_address"]
    token = value["source_token_address"]
    profile = value["source_profile_hash"]
    # Canonical preimages in crates/types/src/market_nav_asset_types.rs.
    source_preimage = (
        f"pftl_chain_id_bytes={len(chain.encode())}\npftl_chain_id={chain}\n"
        f"asset_family_id={family}\nsource_chain_id={source_chain}\n"
        f"vault_address={vault}\ntoken_address={token}\n"
        f"route_epoch={value['source_route_epoch']}\npolicy_hash={profile}\n"
    )
    source_domain = f"erc20_bridge_vault:{source_chain}:{vault}:{token}"
    bucket_preimage = (
        f"asset_id={family}\nsource_domain_bytes={len(source_domain.encode())}\n"
        f"source_domain={source_domain}\npolicy_hash={profile}\n"
    )
    for field, domain, preimage in (
        ("settlement_source_asset_id", "postfiat.pfusdc.source_series.v1", source_preimage),
        ("source_bucket_id", "postfiat.vault_bridge_bucket_id.v1", bucket_preimage),
    ):
        expected = hashlib.sha3_384(domain.encode() + b"\0" + preimage.encode()).hexdigest()
        if value[field] != expected:
            raise DemoError(f"identity {field} does not match the selected chain/family/source profile")
    return dict(value)


def validate_route(route: dict[str, Any], identities: dict[str, Any]) -> None:
    required = {
        "schema": "postfiat-pftl-uniswap-supply-status-v2",
        "route_id": identities["route_id"],
        "native_nav_asset_id": identities["native_nav_asset_id"],
        "settlement_asset_id": identities["settlement_asset_id"],
        "live_value_enabled": True,
        "paused": False,
        "invariant_holds": True,
        "route_schema_version": 2,
        "outbound_verification_class": identities["outbound_verification_class"],
        "return_verification_class": identities["return_verification_class"],
        "ethereum_chain_id": identities["ethereum_chain_id"],
    }
    for field, expected in required.items():
        if route.get(field) != expected:
            raise DemoError(f"route status {field} differs from {expected!r}")


def validate_nav_binding(
    route: dict[str, Any], nav: dict[str, Any], identities: dict[str, Any]
) -> tuple[int, int, int]:
    schema = nav.get("schema")
    if schema == "postfiat.a666.provider_neutral_nav_mark.v1":
        nav_field, assets_field = "nav_per_unit", "verified_net_assets"
    elif schema == "postfiat.a666.live_nav_mark.v1":
        nav_field, assets_field = "nav_per_unit_usd_1e8", "verified_net_assets_usd_1e8"
        for flag in ("opening_constants_used", "uniswap_price_used"):
            if nav.get(flag) is not False:
                raise DemoError(f"legacy NAV manifest requires {flag}=false")
    else:
        raise DemoError(f"unknown NAV manifest schema: {schema!r}")
    required = {
        "asset_id": identities["native_nav_asset_id"],
        "epoch": route["pricing_nav_epoch"],
        "reserve_packet_hash": route["pricing_reserve_packet_hash"],
        "profile_id": identities["nav_profile_id"],
        "source_manifest_hash": identities["nav_source_manifest_hash"],
        "valuation_policy_hash": identities["nav_valuation_policy_hash"],
        "program_vkey": identities["nav_program_vkey"],
        "public_values_schema": identities["nav_public_values_schema"],
    }
    for field, expected in required.items():
        if nav.get(field) != expected:
            raise DemoError(f"NAV manifest {field} differs from {expected!r}")
    # The current builder fixes USD_1E8 and omits valuation_unit from its manifest.
    # An explicitly supplied contradictory unit must never be silently renamed.
    if nav.get("valuation_unit", "USD_1E8") != identities["nav_valuation_unit"]:
        raise DemoError("NAV manifest valuation unit differs from the selected unit")
    require_positive_int(nav["epoch"], "NAV epoch")
    if not isinstance(nav["reserve_packet_hash"], str) or not HASH48_RE.fullmatch(
        nav["reserve_packet_hash"]
    ):
        raise DemoError("NAV reserve packet hash is malformed")
    nav_per_unit = require_positive_int(nav.get(nav_field), f"NAV manifest {nav_field}")
    circulating_supply = require_positive_int(
        nav.get("circulating_supply_atoms"), "NAV manifest circulating_supply_atoms"
    )
    verified_assets = require_positive_int(
        nav.get(assets_field), f"NAV manifest {assets_field}"
    )
    return nav_per_unit, circulating_supply, verified_assets


def selected_source_custody(
    route: dict[str, Any], identities: dict[str, Any], *, for_issue: bool = False
) -> dict[str, Any]:
    """Consume the node's existing rows, which live outside the route ledger hash."""
    rows = route.get("source_settlement_custody")
    if not isinstance(rows, list):
        raise DemoError("route status is missing source_settlement_custody rows")
    seen: set[str] = set()
    selected = None
    principal_total = spread_total = 0
    for row in rows:
        if not isinstance(row, dict) or row.get("route_id") != identities["route_id"]:
            raise DemoError("source custody row describes the wrong route")
        asset = row.get("asset_id")
        if not isinstance(asset, str) or not HASH48_RE.fullmatch(asset) or asset in seen:
            raise DemoError("source custody asset is invalid or duplicated")
        if asset == identities["settlement_asset_id"]:
            raise DemoError("source custody row cannot select the pooled family")
        seen.add(asset)
        principal = require_nonnegative_int(row.get("principal_atoms"), "source principal")
        spread = require_nonnegative_int(row.get("spread_atoms"), "source spread")
        principal_total += principal
        spread_total += spread
        if not isinstance(row.get("enabled_for_issue"), bool):
            raise DemoError("source custody enabled_for_issue must be boolean")
        escrows = row.get("reservation_escrows")
        if not isinstance(escrows, dict):
            raise DemoError("source custody reservation_escrows must be an object")
        for reservation, amount in escrows.items():
            if not HASH48_RE.fullmatch(reservation):
                raise DemoError("source custody reservation ID is malformed")
            require_nonnegative_int(amount, "source reservation escrow")
        if asset == identities["settlement_source_asset_id"]:
            selected = row
    if principal_total > route_counter(route, "settlement_reserve_atoms"):
        raise DemoError("source custody principal exceeds the aggregate reserve")
    if spread_total > route_counter(route, "non_nav_spread_atoms"):
        raise DemoError("source custody spread exceeds the aggregate spread")
    if selected is None:
        raise DemoError("selected source series has no custody row on this route")
    if for_issue and selected["enabled_for_issue"] is not True:
        raise DemoError("selected source series is not enabled for issue")
    return selected


def verify_source_custody_delta(
    before: dict[str, Any], after: dict[str, Any], identities: dict[str, Any],
    principal_delta: int, spread_delta: int,
) -> None:
    selected_source_custody(before, identities)
    selected_source_custody(after, identities)
    prior = {row["asset_id"]: row for row in before["source_settlement_custody"]}
    following = {row["asset_id"]: row for row in after["source_settlement_custody"]}
    if set(prior) != set(following):
        raise DemoError("source custody membership changed during the operation")
    for asset, row in prior.items():
        expected = dict(row)
        if asset == identities["settlement_source_asset_id"]:
            expected["principal_atoms"] += principal_delta
            expected["spread_atoms"] += spread_delta
        assert_equal(following[asset], expected, "source principal/spread/escrow delta")


def wallet_nav_balance(route: dict[str, Any], owner: str) -> int:
    rows = route.get("native_spendable_balances")
    if not isinstance(rows, list) or route.get("native_spendable_balances_truncated") is not False:
        raise DemoError("route native wallet balances are missing or truncated")
    balances: dict[str, int] = {}
    for row in rows:
        wallet = row.get("wallet")
        validate_account(wallet, "native balance wallet")
        if wallet in balances:
            raise DemoError("native wallet balance is duplicated")
        balances[wallet] = require_nonnegative_int(row.get("amount_atoms"), "native wallet balance")
    return balances.get(owner, 0)


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
    identities = validate_identities(load_json(args.identities))
    ensure_key(args.holder_key_file)
    validate_account(args.subscriber, "subscriber")
    if not EVM_ADDRESS_RE.fullmatch(args.ethereum_recipient):
        raise DemoError("Ethereum recipient is not a canonical address")
    route = load_json(args.route_status)
    nav = load_json(args.nav_manifest)
    validate_route(route, identities)
    nav_per_unit, circulating_supply, verified_assets = validate_nav_binding(
        route, nav, identities
    )
    custody = selected_source_custody(route, identities, for_issue=True)
    if custody["reservation_escrows"]:
        raise DemoError("selected source has reservation escrow before issue")
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
        "route_id": identities["route_id"],
        "reservation_id": reservation_id,
        "ethereum_recipient": args.ethereum_recipient.lower(),
        "route_epoch": route["route_epoch"],
        "policy_epoch": route["policy_epoch"],
        "policy_hash": route["policy_hash"],
        "mint_amount_atoms": amount,
        "max_settlement_value_atoms": settlement,
        "settlement_source_asset_id": identities["settlement_source_asset_id"],
        "expires_at_height": expires_at_height,
    }
    subscribe = {
        "operation": "pftl_uniswap_primary_subscribe_v2",
        "subscriber": args.subscriber,
        "route_id": identities["route_id"],
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
        "route_id": identities["route_id"],
        "reservation_id": reservation_id,
    }
    manifest = {
        "schema": "postfiat.a666.pfusdc_reserve_demo_issue.v1",
        "identities": identities,
        "route_id": identities["route_id"],
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
    identities = validate_identities(load_json(args.identities))
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
        "identities": identities,
        "route_id": identities["route_id"],
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
                "route_id": identities["route_id"],
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
    identities = validate_identities(manifest.get("identities"))
    validate_route(before, identities)
    validate_route(after, identities)
    if (
        manifest.get("schema")
        != "postfiat.a666.expired_export_entitlement_cleanup.v1"
    ):
        raise DemoError("cleanup manifest schema mismatch")
    verify_source_custody_delta(before, after, identities, 0, 0)
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


def account_balance(
    report: dict[str, Any], expected_asset_id: str, account: str, chain_id: str
) -> int:
    if report.get("schema") != "postfiat-account-assets-v1":
        raise DemoError("account balance report schema mismatch")
    if report.get("asset_id") != expected_asset_id:
        raise DemoError("account balance report describes the wrong asset")
    if report.get("account") != account or report.get("chain_id") != chain_id:
        raise DemoError("account balance report describes the wrong account or PFTL chain")
    if report.get("truncated") is not False:
        raise DemoError("account balance report is incomplete")
    assets = report.get("assets")
    if not isinstance(assets, list) or len(assets) > 1:
        raise DemoError("account balance report must contain at most one exact asset row")
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
    identities = validate_identities(manifest.get("identities"))
    for route in (before, subscribed, released):
        validate_route(route, identities)
    if manifest.get("schema") != "postfiat.a666.pfusdc_reserve_demo_issue.v1":
        raise DemoError("issue manifest schema mismatch")
    amount = require_positive_int(manifest["mint_amount_atoms"], "mint amount")
    base = require_positive_int(manifest["base_value_atoms"], "base value")
    settlement = require_positive_int(
        manifest["settlement_value_atoms"], "settlement value"
    )
    spread = require_nonnegative_int(manifest["issue_spread_atoms"], "issue spread")
    assert_equal(settlement, base + spread, "issue settlement decomposition")
    verify_source_custody_delta(before, subscribed, identities, base, spread)
    verify_source_custody_delta(subscribed, released, identities, 0, 0)
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
    def read_balance(path: Path, asset_id: str) -> int:
        account = manifest.get("subscriber", manifest.get("owner"))
        validate_account(account, "balance owner")
        return account_balance(load_json(path), asset_id, account, identities["pftl_chain_id"])

    settlement_asset = identities["settlement_source_asset_id"]
    balances = {
        "pfusdc_before": read_balance(args.before_pfusdc, settlement_asset),
        "pfusdc_after_subscribe": read_balance(
            args.after_subscribe_pfusdc, settlement_asset
        ),
        "pfusdc_after_release": read_balance(
            args.after_release_pfusdc, settlement_asset
        ),
        "a666_before": read_balance(args.before_a666, identities["native_nav_asset_id"]),
        "a666_after_subscribe": read_balance(
            args.after_subscribe_a666, identities["native_nav_asset_id"]
        ),
        "a666_after_release": read_balance(
            args.after_release_a666, identities["native_nav_asset_id"]
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
    identities = validate_identities(load_json(args.identities))
    ensure_key(args.holder_key_file)
    validate_account(args.owner, "redemption owner")
    route = load_json(args.route_status)
    nav = load_json(args.nav_manifest)
    issue = load_json(args.issue_manifest)
    validate_route(route, identities)
    nav_per_unit, _, _ = validate_nav_binding(route, nav, identities)
    if issue.get("schema") != "postfiat.a666.pfusdc_reserve_demo_issue.v1":
        raise DemoError("issue manifest schema mismatch")
    if validate_identities(issue.get("identities")) != identities:
        raise DemoError("issue identities differ from the selected redemption identities")
    if issue.get("route_id") != identities["route_id"]:
        raise DemoError("issue manifest describes the wrong route")
    if issue.get("subscriber") != args.owner:
        raise DemoError("redemption owner differs from the issue subscriber")
    if (
        nav["epoch"] <= issue["pricing_nav_epoch"]
        or nav["reserve_packet_hash"] == issue["pricing_reserve_packet_hash"]
        or route["route_epoch"] <= issue["route_epoch"]
    ):
        raise DemoError("redemption requires a fresh NAV packet and advanced route epoch")
    custody = selected_source_custody(route, identities)
    if custody["reservation_escrows"]:
        raise DemoError("selected source has reservation escrow before redemption")
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
    source_principal = custody["principal_atoms"]
    bounded_reserve = min(
        incremental_reserve, source_principal, route_counter(route, "settlement_reserve_atoms")
    )
    if bounded_reserve == 0:
        raise DemoError("selected source custody / same-run reserve cannot fund redemption")
    max_from_reserve = maximum_nav_for_base_reserve(bounded_reserve, nav_per_unit)
    maximum = min(
        issued,
        max_from_reserve,
        wallet_nav_balance(route, args.owner),
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
            f"requested redemption {amount} exceeds same-run/source-custody and policy/wallet safe maximum {maximum}"
        )
    if amount < require_positive_int(route["min_order_atoms"], "minimum order"):
        raise DemoError("redemption amount is below the governed minimum")
    base_value, settlement_output, spread = derive_redeem_amounts(
        amount,
        nav_per_unit,
        route["redeem_multiplier_bps"],
    )
    if base_value > bounded_reserve:
        raise DemoError("redemption exceeds the same-run reserve or selected source custody")
    nonce = secrets.token_hex(32)
    body = {
        "operation": "pftl_uniswap_primary_redeem",
        "owner": args.owner,
        "settlement_recipient": args.owner,
        "route_id": identities["route_id"],
        "redemption_nonce": nonce,
        "nav_amount_atoms": amount,
        "min_settlement_value_atoms": settlement_output,
        "settlement_source_asset_id": identities["settlement_source_asset_id"],
        "route_epoch": route["route_epoch"],
        "policy_epoch": route["policy_epoch"],
        "policy_hash": route["policy_hash"],
        "pricing_nav_epoch": route["pricing_nav_epoch"],
        "pricing_reserve_packet_hash": route["pricing_reserve_packet_hash"],
        "expires_at_height": expires_at_height,
    }
    manifest = {
        "schema": "postfiat.a666.pfusdc_reserve_demo_redeem.v1",
        "identities": identities,
        "route_id": identities["route_id"],
        "owner": args.owner,
        "route_epoch": route["route_epoch"],
        "policy_epoch": route["policy_epoch"],
        "policy_hash": route["policy_hash"],
        "pricing_nav_epoch": route["pricing_nav_epoch"],
        "pricing_reserve_packet_hash": route["pricing_reserve_packet_hash"],
        "nav_per_unit_usd_1e8": nav_per_unit,
        "issued_amount_atoms": issued,
        "incremental_base_reserve_atoms": incremental_reserve,
        "selected_source_principal_atoms": source_principal,
        "bounded_base_reserve_atoms": bounded_reserve,
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
    identities = validate_identities(manifest.get("identities"))
    validate_route(before, identities)
    validate_route(after, identities)
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
    verify_source_custody_delta(before, after, identities, -base, spread)
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
    def read_balance(path: Path, asset_id: str) -> int:
        account = manifest.get("subscriber", manifest.get("owner"))
        validate_account(account, "balance owner")
        return account_balance(load_json(path), asset_id, account, identities["pftl_chain_id"])

    settlement_asset = identities["settlement_source_asset_id"]
    balances = {
        "pfusdc_before": read_balance(args.before_pfusdc, settlement_asset),
        "pfusdc_after": read_balance(args.after_pfusdc, settlement_asset),
        "a666_before": read_balance(args.before_a666, identities["native_nav_asset_id"]),
        "a666_after": read_balance(args.after_a666, identities["native_nav_asset_id"]),
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
    try:
        main()
    except (DemoError, KeyError, TypeError) as error:
        raise SystemExit(f"reserve demo: {error}") from None
