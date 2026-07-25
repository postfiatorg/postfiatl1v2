"""Conservative operator accounting for an externally executed LSP setup.

This module has no LSP, LND, wallet, or PFTL client.  It only reserves a
separately signed ``LIQUIDITY_SETUP`` authorization in the durable real-value
budget and, after an operator supplies strict public terminal evidence, marks
the authorization's full ceiling spent.  An unresolved reservation has no
release path here.
"""

from __future__ import annotations

import json
import os
from pathlib import Path
import re
import stat
import time
from typing import Any, Mapping

from .authorization import ValueAuthorization, verify_value_authorization
from .budget import RealValueBudget
from .composition import (
    SecureStatePaths,
    load_strict_json,
    validate_armed_source_release,
)
from .policy import (
    ExecutionMode,
    PriceObservation,
    RealValuePolicy,
    RealValuePolicyError,
    msat_to_usd_e8_ceil,
)


LIQUIDITY_EVIDENCE_SCHEMA = (
    "postfiat.lightning_liquidity_setup_terminal_evidence.v2"
)
LIQUIDITY_EVIDENCE_FIELDS = frozenset(
    {
        "schema",
        "authorization_id",
        "policy_id",
        "setup_id",
        "category",
        "direction",
        "provider",
        "outcome",
        "value_moved",
        "payment_status",
        "payment_hash",
        "actual_cost_msat",
        "payment_initiated_at_unix",
        "payment_settled_at_unix",
        "channel_active",
        "channel_point",
        "remote_pubkey",
        "capacity_sat",
        "inbound_msat",
        "funding_confirmations",
        "observed_at_unix",
    }
)
HEX_32 = re.compile(r"^[0-9a-f]{64}$")
COMPRESSED_PUBKEY = re.compile(r"^(02|03)[0-9a-f]{64}$")
CHANNEL_POINT = re.compile(r"^[0-9a-f]{64}:(?:0|[1-9][0-9]{0,9})$")
MAX_PUBLIC_INPUT_BYTES = 64 * 1024
# Liquidity setup has a distinct initiation horizon because its offline
# authorization ceremony is separate from executable swap quotes. Swap quotes
# remain capped by MAX_QUOTE_LIFETIME_SECONDS in policy.py.
MAX_LIQUIDITY_INITIATION_HORIZON_SECONDS = 15 * 60
# Once started, an external HODL payment may wait for channel confirmations
# under this separate, hard settlement grace.
MAX_LIQUIDITY_SETTLEMENT_GRACE_SECONDS = 6 * 60 * 60


class LiquidityBudgetError(RealValuePolicyError):
    """A liquidity setup permit or its terminal evidence failed closed."""


def _uint63(value: Any, name: str, *, minimum: int = 0) -> int:
    if type(value) is not int or value < minimum or value > (1 << 63) - 1:
        raise LiquidityBudgetError(f"{name} must be uint63 >= {minimum}")
    return value


def _hex32(value: Any, name: str) -> str:
    if type(value) is not str or HEX_32.fullmatch(value) is None:
        raise LiquidityBudgetError(f"{name} must be lowercase 32-byte hex")
    if value == "0" * 64:
        raise LiquidityBudgetError(f"{name} may not be all zero")
    return value


def _printable(value: Any, name: str, *, maximum: int) -> str:
    if (
        type(value) is not str
        or not value
        or len(value) > maximum
        or not value.isascii()
        or any(ord(character) < 0x20 or ord(character) == 0x7F for character in value)
    ):
        raise LiquidityBudgetError(f"{name} must be bounded printable ASCII")
    return value


def load_owner_only_public_json(
    path: str | Path, label: str
) -> Mapping[str, Any]:
    """Load a bounded public artifact from an owner-only regular file."""

    candidate = Path(path)
    try:
        metadata = candidate.lstat()
    except OSError as error:
        raise LiquidityBudgetError(f"{label} is unavailable") from error
    if (
        stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or metadata.st_mode & (stat.S_IRWXG | stat.S_IRWXO)
    ):
        raise LiquidityBudgetError(
            f"{label} must be a coordinator-owned mode-0600 regular file"
        )
    if metadata.st_size < 2 or metadata.st_size > MAX_PUBLIC_INPUT_BYTES:
        raise LiquidityBudgetError(f"{label} size is invalid")
    # Reuse the canonical-path, duplicate-key, ASCII, and finite-number checks
    # used by the coordinator's other operator inputs.
    return load_strict_json(candidate, label)


def _load_armed_policy(
    paths: SecureStatePaths,
    policy_path: str | Path | None,
) -> RealValuePolicy:
    policy = RealValuePolicy.from_mapping(
        load_strict_json(policy_path or paths.policy, "real-value policy")
    )
    if policy.mode is not ExecutionMode.ARMED:
        raise LiquidityBudgetError(
            "LIQUIDITY_SETUP accounting requires an ARMED policy"
        )
    # This checks the exact clean commit/tree and every release target.  It
    # intentionally does not compose the runtime, connect LND/PFTL, or load a
    # signer.
    validate_armed_source_release(paths.source_release)
    return policy


def _load_fresh_price(
    paths: SecureStatePaths,
    policy: RealValuePolicy,
    price_path: str | Path | None,
    *,
    now_unix: int,
) -> PriceObservation:
    price = PriceObservation.from_mapping(
        load_strict_json(price_path or paths.price, "BTC price observation")
    )
    if price.observed_at_unix > now_unix:
        raise LiquidityBudgetError("BTC price observation is from the future")
    if now_unix - price.observed_at_unix > policy.max_price_age_seconds:
        raise LiquidityBudgetError("BTC price observation is stale")
    return price


def _require_liquidity_authorization(
    authorization: ValueAuthorization,
    policy: RealValuePolicy,
    price: PriceObservation,
    *,
    now_unix: int,
) -> None:
    if (
        authorization.category != "LIQUIDITY_SETUP"
        or authorization.direction != "not_applicable"
    ):
        raise LiquidityBudgetError(
            "permit must have category=LIQUIDITY_SETUP and direction=not_applicable"
        )
    for value, name in (
        (authorization.authorization_id, "authorization_id"),
        (authorization.quote_sha256, "quote_sha256"),
        (authorization.swap_id, "setup_id"),
    ):
        _hex32(value, name)
    if (
        authorization.expires_unix - now_unix
        > MAX_LIQUIDITY_INITIATION_HORIZON_SECONDS
    ):
        raise LiquidityBudgetError(
            "liquidity authorization exceeds the hard initiation horizon"
        )
    priced_ceiling = msat_to_usd_e8_ceil(
        authorization.maximum_all_in_msat,
        price.btc_usd_e8,
    )
    if priced_ceiling > authorization.max_all_in_usd_e8:
        raise LiquidityBudgetError(
            "liquidity authorization USD ceiling is below its msat ceiling"
        )


def reserve_liquidity_setup(
    *,
    state_dir: str | Path,
    authorization_path: str | Path,
    policy_path: str | Path | None = None,
    price_path: str | Path | None = None,
    now_unix: int | None = None,
) -> dict[str, Any]:
    """Durably reserve a signed setup ceiling before any manual LSP order."""

    now = int(time.time()) if now_unix is None else _uint63(
        now_unix, "now_unix"
    )
    paths = SecureStatePaths.under(state_dir)
    policy = _load_armed_policy(paths, policy_path)
    price = _load_fresh_price(paths, policy, price_path, now_unix=now)
    envelope = load_owner_only_public_json(
        authorization_path, "LIQUIDITY_SETUP authorization"
    )
    authorization = verify_value_authorization(
        envelope,
        policy,
        quote=None,
        now_unix=now,
    )
    _require_liquidity_authorization(
        authorization,
        policy,
        price,
        now_unix=now,
    )
    with RealValueBudget(paths.budget, policy) as budget:
        reservation = budget.reserve(envelope, quote=None, now_unix=now)
        if reservation["state"] != "RESERVED":
            raise LiquidityBudgetError(
                "liquidity authorization is no longer an active reservation"
            )
        summary = budget.summary()
    return {
        "schema": "postfiat.lightning_liquidity_setup_reservation.v1",
        "status": "RESERVED",
        "authorization_id": reservation["authorization_id"],
        "policy_id": reservation["policy_id"],
        "setup_id": reservation["swap_id"],
        "category": reservation["category"],
        "direction": reservation["direction"],
        "ceiling_msat": (
            reservation["principal_msat"] + reservation["max_fee_msat"]
        ),
        "ceiling_usd_e8": reservation["max_all_in_usd_e8"],
        "authorization_expires_unix": reservation["expires_unix"],
        "remaining_lifetime_usd_e8": summary["remaining_usd_e8"],
        "manual_external_order_permitted": True,
        "order_created_by_command": False,
        "payment_initiated_by_command": False,
        "pftl_signer_loaded": False,
        "value_moved_by_command": False,
        "ambiguity_policy": "KEEP_RESERVED_AND_HOLD",
    }


def _validate_terminal_evidence(
    value: Mapping[str, Any],
    *,
    policy: RealValuePolicy,
    reservation: Mapping[str, Any],
    now_unix: int,
) -> dict[str, Any]:
    if frozenset(value.keys()) != LIQUIDITY_EVIDENCE_FIELDS:
        raise LiquidityBudgetError("liquidity terminal evidence field set mismatch")
    if value["schema"] != LIQUIDITY_EVIDENCE_SCHEMA:
        raise LiquidityBudgetError("unsupported liquidity terminal evidence schema")
    expected = {
        "authorization_id": reservation["authorization_id"],
        "policy_id": policy.policy_id,
        "setup_id": reservation["swap_id"],
        "category": "LIQUIDITY_SETUP",
        "direction": "not_applicable",
        "outcome": "EXTERNAL_PAYMENT_CONFIRMED_AND_CHANNEL_ACTIVE",
        "value_moved": True,
        "payment_status": "SUCCEEDED",
        "channel_active": True,
    }
    for field, expected_value in expected.items():
        if value[field] != expected_value:
            raise LiquidityBudgetError(
                f"liquidity terminal evidence {field} mismatch"
            )
    _hex32(value["authorization_id"], "authorization_id")
    _hex32(value["policy_id"], "policy_id")
    _hex32(value["setup_id"], "setup_id")
    _printable(value["provider"], "provider", maximum=128)
    _hex32(value["payment_hash"], "payment_hash")
    channel_point = value["channel_point"]
    if type(channel_point) is not str or CHANNEL_POINT.fullmatch(channel_point) is None:
        raise LiquidityBudgetError("channel_point must be canonical txid:index")
    output_index = int(channel_point.rsplit(":", 1)[1])
    if output_index > 0xFFFF_FFFF:
        raise LiquidityBudgetError("channel_point output index exceeds uint32")
    remote_pubkey = value["remote_pubkey"]
    if (
        type(remote_pubkey) is not str
        or COMPRESSED_PUBKEY.fullmatch(remote_pubkey) is None
    ):
        raise LiquidityBudgetError(
            "remote_pubkey must be a compressed public key"
        )
    actual_cost_msat = _uint63(
        value["actual_cost_msat"], "actual_cost_msat", minimum=1
    )
    initiated = _uint63(
        value["payment_initiated_at_unix"],
        "payment_initiated_at_unix",
        minimum=1,
    )
    settled = _uint63(
        value["payment_settled_at_unix"],
        "payment_settled_at_unix",
        minimum=1,
    )
    capacity_sat = _uint63(value["capacity_sat"], "capacity_sat", minimum=1)
    inbound_msat = _uint63(value["inbound_msat"], "inbound_msat", minimum=1)
    confirmations = _uint63(
        value["funding_confirmations"],
        "funding_confirmations",
        minimum=1,
    )
    observed = _uint63(value["observed_at_unix"], "observed_at_unix", minimum=1)
    if actual_cost_msat > (
        reservation["principal_msat"] + reservation["max_fee_msat"]
    ):
        raise LiquidityBudgetError(
            "observed liquidity cost exceeds the signed msat ceiling"
        )
    if initiated < reservation["created_unix"]:
        raise LiquidityBudgetError(
            "external payment initiated before durable reservation"
        )
    if initiated > reservation["expires_unix"]:
        raise LiquidityBudgetError(
            "external payment initiated after authorization expiry"
        )
    if settled < initiated:
        raise LiquidityBudgetError(
            "external payment settled before it was initiated"
        )
    settlement_deadline = (
        initiated + MAX_LIQUIDITY_SETTLEMENT_GRACE_SECONDS
    )
    if settlement_deadline > (1 << 63) - 1:
        raise LiquidityBudgetError("liquidity settlement deadline overflows uint63")
    if settled > settlement_deadline:
        raise LiquidityBudgetError(
            "external payment exceeded the bounded settlement grace"
        )
    if observed < settled or observed > now_unix:
        raise LiquidityBudgetError("liquidity evidence timestamps are inconsistent")
    if inbound_msat > capacity_sat * 1000:
        raise LiquidityBudgetError("observed inbound exceeds channel capacity")
    # Canonicalize before the budget hashes and stores the public evidence.
    try:
        encoded = json.dumps(
            dict(value),
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        ).encode("ascii")
    except (TypeError, UnicodeEncodeError) as error:
        raise LiquidityBudgetError(
            "liquidity terminal evidence is not canonical ASCII JSON"
        ) from error
    if len(encoded) > MAX_PUBLIC_INPUT_BYTES:
        raise LiquidityBudgetError("liquidity terminal evidence is oversized")
    return dict(value)


def mark_liquidity_setup_spent(
    *,
    state_dir: str | Path,
    evidence_path: str | Path,
    policy_path: str | Path | None = None,
    now_unix: int | None = None,
) -> dict[str, Any]:
    """Charge the full reserved ceiling after strict external terminal proof."""

    now = int(time.time()) if now_unix is None else _uint63(
        now_unix, "now_unix"
    )
    paths = SecureStatePaths.under(state_dir)
    policy = _load_armed_policy(paths, policy_path)
    evidence = load_owner_only_public_json(
        evidence_path, "LIQUIDITY_SETUP terminal evidence"
    )
    setup_id = _hex32(evidence.get("setup_id"), "setup_id")
    with RealValueBudget(paths.budget, policy) as budget:
        reservation = budget.authorization_for_swap(setup_id)
        if reservation is None:
            raise LiquidityBudgetError(
                "terminal evidence has no durable setup reservation"
            )
        if (
            reservation["category"] != "LIQUIDITY_SETUP"
            or reservation["direction"] != "not_applicable"
        ):
            raise LiquidityBudgetError(
                "durable reservation is not a LIQUIDITY_SETUP permit"
            )
        terminal = _validate_terminal_evidence(
            evidence,
            policy=policy,
            reservation=reservation,
            now_unix=now,
        )
        spent = budget.mark_spent(
            reservation["authorization_id"],
            terminal_evidence=terminal,
            now_unix=now,
        )
        summary = budget.summary()
    return {
        "schema": "postfiat.lightning_liquidity_setup_spend.v1",
        "status": "SPENT",
        "authorization_id": spent["authorization_id"],
        "policy_id": spent["policy_id"],
        "setup_id": spent["swap_id"],
        "category": spent["category"],
        "direction": spent["direction"],
        "charged_ceiling_usd_e8": spent["max_all_in_usd_e8"],
        "remaining_lifetime_usd_e8": summary["remaining_usd_e8"],
        "terminal_outcome": terminal["outcome"],
        "channel_point": terminal["channel_point"],
        "payment_hash": terminal["payment_hash"],
        "pftl_signer_loaded": False,
        "external_api_called_by_command": False,
        "value_moved_by_command": False,
        "ambiguity_release_available": False,
    }
