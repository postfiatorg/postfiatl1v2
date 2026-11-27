"""Open NAVCoin SDK and exact-atom deterministic fee & NAV engine (Pillar 7).

Implements the frozen L1 Upgrade Train B Pillar 7 contract in exact arbitrary-
precision integer arithmetic (matching Rust ``u256`` intermediates and the
JavaScript/TypeScript ``BigInt`` engine in ``sdk/navcoin/`` to the single atom):

- 100% opt-in ``NavFeeScheduleV1`` (zero fee logic or state when disabled).
- Locked protocol caps: 5%/yr annual (500 bps), 50% HWM gain (5,000 bps),
  5% issue/redeem spread (500 bps), 1% FX fix fee cap (100 bps), and
  7-day minimum increase notice (604,800 blocks at 1s nominal cadence).
- Universal ``nav_per_unit == floor(V * U / S)`` verification across all
  NAVCoin proof profiles (scheduled and zero-fee).
- Neutral, factual protocol API to create zero-fee or fee-enabled NAVCoins,
  inspect pending hikes, simulate multi-epoch accrual, and recompute/verify
  on-chain ``NavFeeAccrualV1`` receipts.
"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass, replace
from typing import Any, Mapping, Sequence

from .address import AddressError, parse_address
from .wallet import ISSUED_ASSET_ID_DOMAIN, _validate_asset_code

# ---------------------------------------------------------------------------
# Frozen protocol constants (L1 Upgrade Train B, Sections 1 & 3.1)
# ---------------------------------------------------------------------------

PROTOCOL_BLOCKS_PER_DAY = 86_400
PROTOCOL_BLOCKS_PER_7_DAYS = 604_800
PROTOCOL_BLOCKS_PER_30_DAYS = 2_592_000
PROTOCOL_BLOCKS_PER_YEAR = 31_536_000

NAV_FEE_BPS_DENOMINATOR = 10_000
NAV_FEE_MAX_ANNUAL_RATE_BPS = 500
NAV_FEE_MAX_HWM_RATE_BPS = 5_000
NAV_FEE_MAX_SPREAD_BPS = 500
NAV_FEE_MAX_FIX_FEE_CAP_BPS = 100
NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS = PROTOCOL_BLOCKS_PER_7_DAYS
NAV_FEE_MAX_DISCLOSURE_REF_BYTES = 128

U32_MAX = (1 << 32) - 1
U64_MAX = (1 << 64) - 1
MAX_PRECISION = 18

NAV_FEE_SCHEDULE_ATTACH_TRANSACTION_KIND = "nav_fee_schedule_attach"
NAV_FEE_SCHEDULE_SET_TRANSACTION_KIND = "nav_fee_schedule_set"
NAV_FEE_SCHEDULE_CANCEL_TRANSACTION_KIND = "nav_fee_schedule_cancel"
NAV_FEE_CLAIM_TRANSACTION_KIND = "nav_fee_claim"

NAV_FEE_SCHEDULE_RPC_METHOD = "nav_fee_schedule"
NAV_FEE_HISTORY_RPC_METHOD = "nav_fee_history"
NAV_FEE_SCHEDULE_SCHEMA = "postfiat-nav-fee-schedule-v1"
NAV_FEE_HISTORY_SCHEMA = "postfiat-nav-fee-history-v1"

DEFAULT_VALUATION_UNIT = "usd_1e6"
DEFAULT_PROOF_PROFILE = "ledger-transparent-v1"

REASON_CODES = frozenset(
    {
        "accrued",
        "zero_supply",
        "zero_net_assets",
        "zero_rates",
        "zero_delta_blocks",
        "clamped_max_supply",
        "clamped_full_value",
    }
)

# Frozen receipt/error codes (Train B Section 3.5 + input validation guards)
ERR_SCHEDULE_ALREADY_ATTACHED = "nav_fee_schedule_already_attached"
ERR_SCHEDULE_MISSING = "nav_fee_schedule_missing"
ERR_RATE_CAP_EXCEEDED = "nav_fee_rate_cap_exceeded"
ERR_NOTICE_TOO_SHORT = "nav_fee_notice_too_short"
ERR_RECIPIENT_EQUALS_ISSUER = "nav_fee_recipient_equals_issuer"
ERR_INVALID_RECIPIENT = "nav_fee_invalid_recipient"
ERR_INVALID_ACCRUAL_CAP = "nav_fee_invalid_accrual_cap"
ERR_DISCLOSURE_REF_TOO_LONG = "nav_fee_disclosure_ref_too_long"
ERR_SCHEDULE_RATES_LOCKED = "nav_fee_schedule_rates_locked"
ERR_NO_PENDING_SCHEDULE = "nav_fee_no_pending_schedule"
ERR_PENDING_INCREASE_EXIT_CAPACITY_LOCKED = "nav_fee_pending_increase_exit_capacity_locked"
ERR_INSUFFICIENT_UNCLAIMED = "nav_fee_insufficient_unclaimed"
ERR_UNAUTHORIZED_RECIPIENT = "nav_fee_unauthorized_recipient"
ERR_INVALID_CLAIM_AMOUNT = "nav_fee_invalid_claim_amount"
ERR_NAV_PER_UNIT_FORMULA_MISMATCH = "nav_per_unit_formula_mismatch"
ERR_NAV_SUPPLY_SNAPSHOT_STALE = "nav_supply_snapshot_stale"
ERR_ISSUER_SELF_ATTESTATION_FORBIDDEN = "nav_fee_issuer_self_attestation_forbidden"
ERR_FX_FIX_FEE_EXCEEDS_NAV_CAP = "fx_fix_fee_exceeds_nav_cap"


class NavFeeError(ValueError):
    """Deterministic protocol validation or consensus rule error."""

    def __init__(self, receipt_code: str, message: str | None = None) -> None:
        super().__init__(message or receipt_code)
        self.receipt_code = receipt_code


# ---------------------------------------------------------------------------
# Integer & address coercion helpers (zero float drift)
# ---------------------------------------------------------------------------


def parse_exact_uint(value: Any, name: str = "value", *, max_value: int = U64_MAX) -> int:
    """Parse an exact non-negative integer from ``int`` or decimal ``str``.

    Booleans and floats are always rejected to guarantee zero floating-point drift.
    """
    if isinstance(value, bool):
        raise ValueError(f"{name} must be an integer, not bool")
    if isinstance(value, int):
        parsed = value
    elif isinstance(value, str):
        if not value or not value.isascii() or not value.isdigit():
            raise ValueError(f"{name} must be an unsigned base-10 integer string")
        parsed = int(value, 10)
    else:
        raise ValueError(f"{name} must be an exact integer or decimal string")
    if parsed < 0 or parsed > max_value:
        raise ValueError(f"{name} out of bounds [0, {max_value}]: {parsed}")
    return parsed


def _parse_u32(value: Any, name: str) -> int:
    return parse_exact_uint(value, name, max_value=U32_MAX)


def _parse_u64(value: Any, name: str) -> int:
    return parse_exact_uint(value, name, max_value=U64_MAX)


def _parse_precision(precision: Any) -> int:
    return parse_exact_uint(precision, "precision", max_value=MAX_PRECISION)


def normalize_account(address: str, *, field_name: str = "account") -> str:
    """Validate and return canonical lowercase ``pf`` + 40 hex account address."""
    try:
        return parse_address(address)
    except AddressError as exc:
        if field_name == "recipient":
            raise NavFeeError(ERR_INVALID_RECIPIENT, f"invalid recipient address: {exc}") from exc
        raise ValueError(f"invalid {field_name}: {exc}") from exc


# ---------------------------------------------------------------------------
# Canonical JSON & cryptographic identity helpers
# ---------------------------------------------------------------------------


def canonical_json_bytes(value: Any) -> bytes:
    """Serialize JSON canonically (sorted keys, no extra whitespace, UTF-8)."""
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")


def sha3_384_hex(data: bytes) -> str:
    return hashlib.sha3_384(data).hexdigest()


def derived_asset_id(chain_id: str, issuer: str, code: str, version: int = 1) -> str:
    """Chain-bound NAVCoin asset_id matching ``crates/types`` exactly."""
    if not chain_id:
        raise ValueError("chain_id is required")
    if not issuer:
        raise ValueError("issuer is required")
    _validate_asset_code(code)
    version_int = parse_exact_uint(version, "version", max_value=U32_MAX)
    if version_int < 1:
        raise ValueError("version must be >= 1")
    code_bytes = code.encode("utf-8")
    preimage = (
        f"chain_id={chain_id}\nissuer={issuer}\ncode_bytes={len(code_bytes)}\n"
        f"code={code}\nversion={version_int}\n"
    ).encode("utf-8")
    hasher = hashlib.sha3_384()
    hasher.update(ISSUED_ASSET_ID_DOMAIN.encode("utf-8"))
    hasher.update(b"\x00")
    hasher.update(preimage)
    return hasher.hexdigest()


def nav_reserve_packet_id(asset_id: str, epoch: int, reserve_packet_hash: str) -> str:
    """Deterministic SHA3-384 reserve packet ID matching Rust consensus."""
    epoch_int = _parse_u64(epoch, "epoch")
    hasher = hashlib.sha3_384()
    hasher.update(b"postfiat.nav_reserve_packet_id.v1")
    hasher.update(b"\x00")
    hasher.update(
        f"asset_id={asset_id}\nepoch={epoch_int}\nreserve_packet_hash={reserve_packet_hash}\n".encode(
            "utf-8"
        )
    )
    return hasher.hexdigest()


# ---------------------------------------------------------------------------
# Frozen Train B state & input structures
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class NavFeeScheduleInputV1:
    annual_rate_bps: int
    hwm_rate_bps: int
    issue_spread_bps: int
    redeem_spread_bps: int
    fix_fee_cap_bps: int
    recipient: str
    increase_notice_blocks: int = NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS
    accrual_cap_blocks: int = PROTOCOL_BLOCKS_PER_DAY
    rates_locked: bool = False
    disclosure_ref: str | None = None

    def to_dict(self) -> dict[str, Any]:
        out: dict[str, Any] = {
            "annual_rate_bps": self.annual_rate_bps,
            "hwm_rate_bps": self.hwm_rate_bps,
            "issue_spread_bps": self.issue_spread_bps,
            "redeem_spread_bps": self.redeem_spread_bps,
            "fix_fee_cap_bps": self.fix_fee_cap_bps,
            "recipient": self.recipient,
            "increase_notice_blocks": self.increase_notice_blocks,
            "accrual_cap_blocks": self.accrual_cap_blocks,
            "rates_locked": self.rates_locked,
        }
        if self.disclosure_ref is not None:
            out["disclosure_ref"] = self.disclosure_ref
        return out

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> NavFeeScheduleInputV1:
        return cls(
            annual_rate_bps=_parse_u32(data["annual_rate_bps"], "annual_rate_bps"),
            hwm_rate_bps=_parse_u32(data["hwm_rate_bps"], "hwm_rate_bps"),
            issue_spread_bps=_parse_u32(data["issue_spread_bps"], "issue_spread_bps"),
            redeem_spread_bps=_parse_u32(data["redeem_spread_bps"], "redeem_spread_bps"),
            fix_fee_cap_bps=_parse_u32(data["fix_fee_cap_bps"], "fix_fee_cap_bps"),
            recipient=str(data["recipient"]),
            increase_notice_blocks=_parse_u64(
                data.get("increase_notice_blocks", NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS),
                "increase_notice_blocks",
            ),
            accrual_cap_blocks=_parse_u64(
                data.get("accrual_cap_blocks", PROTOCOL_BLOCKS_PER_DAY),
                "accrual_cap_blocks",
            ),
            rates_locked=bool(data.get("rates_locked", False)),
            disclosure_ref=data.get("disclosure_ref"),
        )


@dataclass(frozen=True)
class NavFeeScheduleV1:
    annual_rate_bps: int
    hwm_rate_bps: int
    issue_spread_bps: int
    redeem_spread_bps: int
    fix_fee_cap_bps: int
    recipient: str
    increase_notice_blocks: int
    accrual_cap_blocks: int
    rates_locked: bool
    disclosure_ref: str | None
    attached_at_height: int

    def to_dict(self) -> dict[str, Any]:
        out: dict[str, Any] = {
            "annual_rate_bps": self.annual_rate_bps,
            "hwm_rate_bps": self.hwm_rate_bps,
            "issue_spread_bps": self.issue_spread_bps,
            "redeem_spread_bps": self.redeem_spread_bps,
            "fix_fee_cap_bps": self.fix_fee_cap_bps,
            "recipient": self.recipient,
            "increase_notice_blocks": self.increase_notice_blocks,
            "accrual_cap_blocks": self.accrual_cap_blocks,
            "rates_locked": self.rates_locked,
            "attached_at_height": self.attached_at_height,
        }
        if self.disclosure_ref is not None:
            out["disclosure_ref"] = self.disclosure_ref
        return out

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> NavFeeScheduleV1:
        return cls(
            annual_rate_bps=_parse_u32(data["annual_rate_bps"], "annual_rate_bps"),
            hwm_rate_bps=_parse_u32(data["hwm_rate_bps"], "hwm_rate_bps"),
            issue_spread_bps=_parse_u32(data["issue_spread_bps"], "issue_spread_bps"),
            redeem_spread_bps=_parse_u32(data["redeem_spread_bps"], "redeem_spread_bps"),
            fix_fee_cap_bps=_parse_u32(data["fix_fee_cap_bps"], "fix_fee_cap_bps"),
            recipient=str(data["recipient"]),
            increase_notice_blocks=_parse_u64(
                data["increase_notice_blocks"], "increase_notice_blocks"
            ),
            accrual_cap_blocks=_parse_u64(data["accrual_cap_blocks"], "accrual_cap_blocks"),
            rates_locked=bool(data.get("rates_locked", False)),
            disclosure_ref=data.get("disclosure_ref"),
            attached_at_height=_parse_u64(
                data.get("attached_at_height", 0), "attached_at_height"
            ),
        )


@dataclass(frozen=True)
class PendingNavFeeScheduleV1:
    schedule: NavFeeScheduleV1
    proposed_at_height: int
    effective_at_height: int
    is_increase: bool

    def to_dict(self) -> dict[str, Any]:
        return {
            "schedule": self.schedule.to_dict(),
            "proposed_at_height": self.proposed_at_height,
            "effective_at_height": self.effective_at_height,
            "is_increase": self.is_increase,
        }

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> PendingNavFeeScheduleV1:
        return cls(
            schedule=NavFeeScheduleV1.from_dict(data["schedule"]),
            proposed_at_height=_parse_u64(data["proposed_at_height"], "proposed_at_height"),
            effective_at_height=_parse_u64(data["effective_at_height"], "effective_at_height"),
            is_increase=bool(data["is_increase"]),
        )


@dataclass(frozen=True)
class NavFeeStateV1:
    active: NavFeeScheduleV1
    pending: PendingNavFeeScheduleV1 | None
    high_water_mark_nav_per_unit: int
    last_accrual_height: int
    unclaimed_fee_units: int = 0
    total_accrued_fee_units: int = 0
    total_claimed_fee_units: int = 0

    def to_dict(self) -> dict[str, Any]:
        return {
            "active": self.active.to_dict(),
            "pending": self.pending.to_dict() if self.pending is not None else None,
            "high_water_mark_nav_per_unit": self.high_water_mark_nav_per_unit,
            "last_accrual_height": self.last_accrual_height,
            "unclaimed_fee_units": self.unclaimed_fee_units,
            "total_accrued_fee_units": self.total_accrued_fee_units,
            "total_claimed_fee_units": self.total_claimed_fee_units,
        }

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> NavFeeStateV1:
        pending_raw = data.get("pending")
        return cls(
            active=NavFeeScheduleV1.from_dict(data["active"]),
            pending=(
                PendingNavFeeScheduleV1.from_dict(pending_raw)
                if pending_raw is not None
                else None
            ),
            high_water_mark_nav_per_unit=_parse_u64(
                data["high_water_mark_nav_per_unit"], "high_water_mark_nav_per_unit"
            ),
            last_accrual_height=_parse_u64(data["last_accrual_height"], "last_accrual_height"),
            unclaimed_fee_units=_parse_u64(
                data.get("unclaimed_fee_units", 0), "unclaimed_fee_units"
            ),
            total_accrued_fee_units=_parse_u64(
                data.get("total_accrued_fee_units", 0), "total_accrued_fee_units"
            ),
            total_claimed_fee_units=_parse_u64(
                data.get("total_claimed_fee_units", 0), "total_claimed_fee_units"
            ),
        )


@dataclass(frozen=True)
class NavFeeAccrualV1:
    asset_id: str
    epoch: int
    block_height: int
    blocks_per_year: int
    delta_blocks: int
    verified_net_assets: int
    supply_before: int
    nav_before: int
    hwm_before: int
    annual_rate_bps_applied: int
    hwm_rate_bps_applied: int
    time_fee_value: int
    nav_after_time_fee: int
    hwm_fee_value: int
    total_fee_value: int
    minted_fee_units: int
    clipped_fee_units: int
    supply_after: int
    nav_after: int
    hwm_after: int
    reason_code: str

    def to_dict(self, *, stringify_u64: bool = False) -> dict[str, Any]:
        def fmt(v: int) -> int | str:
            return str(v) if stringify_u64 else v

        return {
            "asset_id": self.asset_id,
            "epoch": self.epoch,
            "block_height": fmt(self.block_height),
            "blocks_per_year": fmt(self.blocks_per_year),
            "delta_blocks": fmt(self.delta_blocks),
            "verified_net_assets": fmt(self.verified_net_assets),
            "supply_before": fmt(self.supply_before),
            "nav_before": fmt(self.nav_before),
            "hwm_before": fmt(self.hwm_before),
            "annual_rate_bps_applied": self.annual_rate_bps_applied,
            "hwm_rate_bps_applied": self.hwm_rate_bps_applied,
            "time_fee_value": fmt(self.time_fee_value),
            "nav_after_time_fee": fmt(self.nav_after_time_fee),
            "hwm_fee_value": fmt(self.hwm_fee_value),
            "total_fee_value": fmt(self.total_fee_value),
            "minted_fee_units": fmt(self.minted_fee_units),
            "clipped_fee_units": fmt(self.clipped_fee_units),
            "supply_after": fmt(self.supply_after),
            "nav_after": fmt(self.nav_after),
            "hwm_after": fmt(self.hwm_after),
            "reason_code": self.reason_code,
        }

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> NavFeeAccrualV1:
        reason = str(data["reason_code"])
        if reason not in REASON_CODES:
            raise ValueError(f"unknown reason_code: {reason}")
        return cls(
            asset_id=str(data["asset_id"]),
            epoch=_parse_u64(data["epoch"], "epoch"),
            block_height=_parse_u64(data["block_height"], "block_height"),
            blocks_per_year=_parse_u64(data["blocks_per_year"], "blocks_per_year"),
            delta_blocks=_parse_u64(data["delta_blocks"], "delta_blocks"),
            verified_net_assets=_parse_u64(data["verified_net_assets"], "verified_net_assets"),
            supply_before=_parse_u64(data["supply_before"], "supply_before"),
            nav_before=_parse_u64(data["nav_before"], "nav_before"),
            hwm_before=_parse_u64(data["hwm_before"], "hwm_before"),
            annual_rate_bps_applied=_parse_u32(
                data["annual_rate_bps_applied"], "annual_rate_bps_applied"
            ),
            hwm_rate_bps_applied=_parse_u32(
                data["hwm_rate_bps_applied"], "hwm_rate_bps_applied"
            ),
            time_fee_value=_parse_u64(data["time_fee_value"], "time_fee_value"),
            nav_after_time_fee=_parse_u64(data["nav_after_time_fee"], "nav_after_time_fee"),
            hwm_fee_value=_parse_u64(data["hwm_fee_value"], "hwm_fee_value"),
            total_fee_value=_parse_u64(data["total_fee_value"], "total_fee_value"),
            minted_fee_units=_parse_u64(data["minted_fee_units"], "minted_fee_units"),
            clipped_fee_units=_parse_u64(data["clipped_fee_units"], "clipped_fee_units"),
            supply_after=_parse_u64(data["supply_after"], "supply_after"),
            nav_after=_parse_u64(data["nav_after"], "nav_after"),
            hwm_after=_parse_u64(data["hwm_after"], "hwm_after"),
            reason_code=reason,
        )


# ---------------------------------------------------------------------------
# Universal floor(V * U / S) NAV math & integrity checks (ALL NAVCoins)
# ---------------------------------------------------------------------------


def unit_scale(precision: int) -> int:
    """Return ``U = 10^precision`` as an exact integer."""
    p = _parse_precision(precision)
    return 10**p


def compute_nav_per_unit(
    verified_net_assets: int | str,
    circulating_supply: int | str,
    precision: int,
) -> int:
    """Compute exact ``floor(V * 10^precision / S)`` using arbitrary-precision integers.

    Returns ``0`` when ``circulating_supply == 0``.
    """
    v = _parse_u64(verified_net_assets, "verified_net_assets")
    s = _parse_u64(circulating_supply, "circulating_supply")
    u = unit_scale(precision)
    if s == 0:
        return 0
    return (v * u) // s


def verify_nav_per_unit(
    verified_net_assets: int | str,
    circulating_supply: int | str,
    precision: int,
    claimed_nav_per_unit: int | str,
    *,
    live_ledger_supply: int | str | None = None,
) -> dict[str, Any]:
    """Enforce universal ``nav_per_unit == floor(V * U / S)`` and supply freshness.

    Applies equally to zero-fee NAVCoins (no fee schedule) and fee-enabled NAVCoins.
    Raises :class:`NavFeeError` with frozen Train B receipt codes on mismatch.
    """
    v = _parse_u64(verified_net_assets, "verified_net_assets")
    s = _parse_u64(circulating_supply, "circulating_supply")
    claimed = _parse_u64(claimed_nav_per_unit, "claimed_nav_per_unit")
    u = unit_scale(precision)

    if live_ledger_supply is not None:
        live_s = _parse_u64(live_ledger_supply, "live_ledger_supply")
        if s != live_s:
            raise NavFeeError(
                ERR_NAV_SUPPLY_SNAPSHOT_STALE,
                f"packet circulating_supply ({s}) != live_ledger_supply ({live_s})",
            )

    expected = 0 if s == 0 else (v * u) // s
    if s > 0 and claimed != expected:
        raise NavFeeError(
            ERR_NAV_PER_UNIT_FORMULA_MISMATCH,
            f"claimed nav_per_unit ({claimed}) != floor(V*U/S) ({expected})",
        )

    remainder = 0 if s == 0 else (v * u) - (expected * s)
    return {
        "valid": True,
        "verified_net_assets": v,
        "circulating_supply": s,
        "precision": _parse_precision(precision),
        "unit_scale": u,
        "expected_nav_per_unit": expected,
        "claimed_nav_per_unit": claimed,
        "floor_remainder_scaled": remainder,
    }


# ---------------------------------------------------------------------------
# Schedule validation, cap enforcement & lifecycle transitions
# ---------------------------------------------------------------------------


def validate_fee_schedule_input(
    schedule: NavFeeScheduleInputV1 | Mapping[str, Any],
    *,
    issuer: str,
    max_epoch_gap_blocks: int | None = None,
) -> NavFeeScheduleInputV1:
    """Validate ``NavFeeScheduleInputV1`` against all locked Pillar 7 caps."""
    inp = (
        schedule
        if isinstance(schedule, NavFeeScheduleInputV1)
        else NavFeeScheduleInputV1.from_dict(schedule)
    )
    canonical_issuer = normalize_account(issuer, field_name="issuer")
    canonical_recipient = normalize_account(inp.recipient, field_name="recipient")

    if canonical_recipient == canonical_issuer:
        raise NavFeeError(
            ERR_RECIPIENT_EQUALS_ISSUER,
            "fee recipient account must not equal issuer account",
        )

    if (
        inp.annual_rate_bps > NAV_FEE_MAX_ANNUAL_RATE_BPS
        or inp.hwm_rate_bps > NAV_FEE_MAX_HWM_RATE_BPS
        or inp.issue_spread_bps > NAV_FEE_MAX_SPREAD_BPS
        or inp.redeem_spread_bps > NAV_FEE_MAX_SPREAD_BPS
        or inp.fix_fee_cap_bps > NAV_FEE_MAX_FIX_FEE_CAP_BPS
    ):
        raise NavFeeError(
            ERR_RATE_CAP_EXCEEDED,
            "fee schedule rate or spread exceeds protocol cap",
        )

    if inp.increase_notice_blocks < NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS:
        raise NavFeeError(
            ERR_NOTICE_TOO_SHORT,
            f"increase_notice_blocks ({inp.increase_notice_blocks}) < minimum ({NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS})",
        )

    if inp.accrual_cap_blocks == 0:
        raise NavFeeError(
            ERR_INVALID_ACCRUAL_CAP,
            "accrual_cap_blocks must be > 0",
        )
    if max_epoch_gap_blocks is not None:
        gap_cap = _parse_u64(max_epoch_gap_blocks, "max_epoch_gap_blocks")
        if gap_cap > 0 and inp.accrual_cap_blocks > gap_cap:
            raise NavFeeError(
                ERR_INVALID_ACCRUAL_CAP,
                f"accrual_cap_blocks ({inp.accrual_cap_blocks}) exceeds max_epoch_gap_blocks ({gap_cap})",
            )

    if inp.disclosure_ref is not None:
        if not isinstance(inp.disclosure_ref, str):
            raise ValueError("disclosure_ref must be a string when provided")
        if len(inp.disclosure_ref.encode("utf-8")) > NAV_FEE_MAX_DISCLOSURE_REF_BYTES:
            raise NavFeeError(
                ERR_DISCLOSURE_REF_TOO_LONG,
                f"disclosure_ref exceeds {NAV_FEE_MAX_DISCLOSURE_REF_BYTES} UTF-8 bytes",
            )

    return replace(inp, recipient=canonical_recipient)


def is_schedule_increase(
    current: NavFeeScheduleV1,
    proposed: NavFeeScheduleInputV1 | NavFeeScheduleV1,
) -> bool:
    """Return True if ``proposed`` increases any holder cost or shortens notice."""
    return (
        proposed.annual_rate_bps > current.annual_rate_bps
        or proposed.hwm_rate_bps > current.hwm_rate_bps
        or proposed.issue_spread_bps > current.issue_spread_bps
        or proposed.redeem_spread_bps > current.redeem_spread_bps
        or proposed.fix_fee_cap_bps > current.fix_fee_cap_bps
        or proposed.accrual_cap_blocks > current.accrual_cap_blocks
        or proposed.increase_notice_blocks < current.increase_notice_blocks
    )


def attach_fee_schedule(
    *,
    existing_fee_state: NavFeeStateV1 | None,
    issuer: str,
    schedule_input: NavFeeScheduleInputV1 | Mapping[str, Any],
    block_height: int | str,
    current_nav_per_unit: int | str = 0,
    max_epoch_gap_blocks: int | None = None,
) -> NavFeeStateV1:
    """Attach an initial ``NavFeeScheduleV1`` with exact zero-retroactive initialization."""
    if existing_fee_state is not None:
        raise NavFeeError(
            ERR_SCHEDULE_ALREADY_ATTACHED,
            "asset already has an attached fee schedule",
        )
    h_attach = _parse_u64(block_height, "block_height")
    nav_now = _parse_u64(current_nav_per_unit, "current_nav_per_unit")
    validated = validate_fee_schedule_input(
        schedule_input,
        issuer=issuer,
        max_epoch_gap_blocks=max_epoch_gap_blocks,
    )
    active = NavFeeScheduleV1(
        annual_rate_bps=validated.annual_rate_bps,
        hwm_rate_bps=validated.hwm_rate_bps,
        issue_spread_bps=validated.issue_spread_bps,
        redeem_spread_bps=validated.redeem_spread_bps,
        fix_fee_cap_bps=validated.fix_fee_cap_bps,
        recipient=validated.recipient,
        increase_notice_blocks=validated.increase_notice_blocks,
        accrual_cap_blocks=validated.accrual_cap_blocks,
        rates_locked=validated.rates_locked,
        disclosure_ref=validated.disclosure_ref,
        attached_at_height=h_attach,
    )
    return NavFeeStateV1(
        active=active,
        pending=None,
        high_water_mark_nav_per_unit=nav_now,
        last_accrual_height=h_attach,
        unclaimed_fee_units=0,
        total_accrued_fee_units=0,
        total_claimed_fee_units=0,
    )


def propose_fee_schedule_update(
    *,
    fee_state: NavFeeStateV1 | None,
    issuer: str,
    schedule_input: NavFeeScheduleInputV1 | Mapping[str, Any],
    block_height: int | str,
    max_epoch_gap_blocks: int | None = None,
    recipient_cosigned: bool = False,
) -> NavFeeStateV1:
    """Propose a ``nav_fee_schedule_set`` update respecting lock and 7-day notice rules."""
    if fee_state is None:
        raise NavFeeError(ERR_SCHEDULE_MISSING, "no fee schedule attached to asset")

    h_now = _parse_u64(block_height, "block_height")
    validated = validate_fee_schedule_input(
        schedule_input,
        issuer=issuer,
        max_epoch_gap_blocks=max_epoch_gap_blocks,
    )
    active = fee_state.active
    increase = is_schedule_increase(active, validated)

    if active.rates_locked:
        if not validated.rates_locked or increase:
            raise NavFeeError(
                ERR_SCHEDULE_RATES_LOCKED,
                "fee schedule rates are permanently locked against increases or unlocking",
            )

    if increase:
        notice = max(active.increase_notice_blocks, NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS)
        effective_at_height = h_now + notice
    elif (
        active.rates_locked
        and validated.recipient != active.recipient
        and not recipient_cosigned
    ):
        effective_at_height = h_now + NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS
    else:
        effective_at_height = h_now

    if effective_at_height > U64_MAX:
        raise ValueError("effective_at_height exceeds u64::MAX")

    pending_schedule = NavFeeScheduleV1(
        annual_rate_bps=validated.annual_rate_bps,
        hwm_rate_bps=validated.hwm_rate_bps,
        issue_spread_bps=validated.issue_spread_bps,
        redeem_spread_bps=validated.redeem_spread_bps,
        fix_fee_cap_bps=validated.fix_fee_cap_bps,
        recipient=validated.recipient,
        increase_notice_blocks=validated.increase_notice_blocks,
        accrual_cap_blocks=validated.accrual_cap_blocks,
        rates_locked=validated.rates_locked,
        disclosure_ref=validated.disclosure_ref,
        attached_at_height=active.attached_at_height,
    )
    pending = PendingNavFeeScheduleV1(
        schedule=pending_schedule,
        proposed_at_height=h_now,
        effective_at_height=effective_at_height,
        is_increase=increase,
    )
    return replace(fee_state, pending=pending)


def cancel_pending_fee_schedule(fee_state: NavFeeStateV1 | None) -> NavFeeStateV1:
    """Cancel a pending schedule update (``nav_fee_schedule_cancel``)."""
    if fee_state is None:
        raise NavFeeError(ERR_SCHEDULE_MISSING, "no fee schedule attached to asset")
    if fee_state.pending is None:
        raise NavFeeError(ERR_NO_PENDING_SCHEDULE, "no pending fee schedule to cancel")
    return replace(fee_state, pending=None)


def claim_accrued_fees(
    *,
    fee_state: NavFeeStateV1 | None,
    caller: str,
    amount: int | str,
) -> tuple[NavFeeStateV1, dict[str, Any]]:
    """Claim accrued fee units from the unclaimed bucket (``nav_fee_claim``)."""
    if fee_state is None:
        raise NavFeeError(ERR_SCHEDULE_MISSING, "no fee schedule attached to asset")
    canonical_caller = normalize_account(caller, field_name="caller")
    if canonical_caller != fee_state.active.recipient:
        raise NavFeeError(
            ERR_UNAUTHORIZED_RECIPIENT,
            f"caller {canonical_caller} does not match active fee recipient {fee_state.active.recipient}",
        )
    claim_amount = _parse_u64(amount, "amount")
    if claim_amount == 0:
        raise NavFeeError(ERR_INVALID_CLAIM_AMOUNT, "claim amount must be > 0")
    if claim_amount > fee_state.unclaimed_fee_units:
        raise NavFeeError(
            ERR_INSUFFICIENT_UNCLAIMED,
            f"claim amount ({claim_amount}) exceeds unclaimed_fee_units ({fee_state.unclaimed_fee_units})",
        )
    updated = replace(
        fee_state,
        unclaimed_fee_units=fee_state.unclaimed_fee_units - claim_amount,
        total_claimed_fee_units=fee_state.total_claimed_fee_units + claim_amount,
    )
    receipt = {
        "recipient": canonical_caller,
        "claimed_amount": claim_amount,
        "unclaimed_remaining": updated.unclaimed_fee_units,
        "total_claimed_fee_units": updated.total_claimed_fee_units,
    }
    return updated, receipt


# ---------------------------------------------------------------------------
# Anti-bypass guards (exit capacity, unscheduled spreads, FX fix, attestors)
# ---------------------------------------------------------------------------


def check_exit_capacity_update(
    fee_state: NavFeeStateV1 | None,
    current_redeem_capacity_atoms: int | str,
    new_redeem_capacity_atoms: int | str,
) -> bool:
    """Reject exit capacity cuts while a fee increase is pending."""
    cur = _parse_u64(current_redeem_capacity_atoms, "current_redeem_capacity_atoms")
    nxt = _parse_u64(new_redeem_capacity_atoms, "new_redeem_capacity_atoms")
    if (
        fee_state is not None
        and fee_state.pending is not None
        and fee_state.pending.is_increase
        and nxt < cur
    ):
        raise NavFeeError(
            ERR_PENDING_INCREASE_EXIT_CAPACITY_LOCKED,
            "cannot reduce redeem exit capacity while a fee increase is pending",
        )
    return True


def check_unscheduled_spread_update(
    *,
    current_issue_spread_bps: int,
    current_redeem_spread_bps: int,
    new_issue_spread_bps: int,
    new_redeem_spread_bps: int,
    proposed_at_height: int | str,
    effective_at_height: int | str,
) -> bool:
    """Close legacy spread-hike bypass on unscheduled NAVCoins (7-day notice on hikes)."""
    cur_issue = _parse_u32(current_issue_spread_bps, "current_issue_spread_bps")
    cur_redeem = _parse_u32(current_redeem_spread_bps, "current_redeem_spread_bps")
    nxt_issue = _parse_u32(new_issue_spread_bps, "new_issue_spread_bps")
    nxt_redeem = _parse_u32(new_redeem_spread_bps, "new_redeem_spread_bps")
    h_prop = _parse_u64(proposed_at_height, "proposed_at_height")
    h_eff = _parse_u64(effective_at_height, "effective_at_height")

    if nxt_issue > NAV_FEE_MAX_SPREAD_BPS or nxt_redeem > NAV_FEE_MAX_SPREAD_BPS:
        raise NavFeeError(ERR_RATE_CAP_EXCEEDED, "spread exceeds 500 bps protocol cap")

    is_hike = nxt_issue > cur_issue or nxt_redeem > cur_redeem
    if is_hike and h_eff < h_prop + NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS:
        raise NavFeeError(
            ERR_NOTICE_TOO_SHORT,
            "spread increase on unscheduled NAVCoin requires at least 604,800 blocks notice",
        )
    return True


def check_fx_fix_fee_cap(
    fee_bps: int,
    fee_state: NavFeeStateV1 | None = None,
) -> bool:
    """Enforce 1% (100 bps) protocol cap and optional schedule ``fix_fee_cap_bps``."""
    bps = _parse_u32(fee_bps, "fee_bps")
    if bps > NAV_FEE_MAX_FIX_FEE_CAP_BPS:
        raise NavFeeError(
            ERR_FX_FIX_FEE_EXCEEDS_NAV_CAP,
            f"FX fix fee_bps ({bps}) exceeds protocol cap ({NAV_FEE_MAX_FIX_FEE_CAP_BPS})",
        )
    if fee_state is not None and bps > fee_state.active.fix_fee_cap_bps:
        raise NavFeeError(
            ERR_FX_FIX_FEE_EXCEEDS_NAV_CAP,
            f"FX fix fee_bps ({bps}) exceeds asset fix_fee_cap_bps ({fee_state.active.fix_fee_cap_bps})",
        )
    return True


def check_non_issuer_attestation(
    *,
    issuer: str,
    fee_state: NavFeeStateV1 | None,
    attested_value: int | str,
    attestations: Sequence[Mapping[str, Any]],
) -> bool:
    """Enforce non-issuer attestor requirement when ``attested_value > 0`` on fee-enabled NAVCoins."""
    att_val = _parse_u64(attested_value, "attested_value")
    if fee_state is None or att_val == 0:
        return True
    canonical_issuer = normalize_account(issuer, field_name="issuer")
    for item in attestations:
        passed = bool(item.get("pass", item.get("passed", False)))
        if not passed:
            continue
        attestor_raw = item.get("attestor")
        if not isinstance(attestor_raw, str):
            continue
        try:
            attestor = normalize_account(attestor_raw, field_name="attestor")
        except ValueError:
            continue
        if attestor != canonical_issuer:
            return True
    raise NavFeeError(
        ERR_ISSUER_SELF_ATTESTATION_FORBIDDEN,
        "fee-enabled NAVCoin with attested_value > 0 requires a passing non-issuer attestor",
    )


# ---------------------------------------------------------------------------
# Exact 4-step u256 deterministic epoch accrual (Train B Section 3.4)
# ---------------------------------------------------------------------------


def accrue_epoch(
    *,
    asset_id: str,
    epoch: int | str,
    block_height: int | str,
    verified_net_assets: int | str,
    supply_before: int | str,
    precision: int,
    fee_state: NavFeeStateV1,
    max_supply: int | str | None = None,
    blocks_per_year: int = PROTOCOL_BLOCKS_PER_YEAR,
) -> tuple[NavFeeStateV1, NavFeeAccrualV1]:
    """Execute the exact 4-step deterministic integer epoch fee accrual.

    Uses exact Python arbitrary-precision integers (matching Rust ``u256`` and
    JavaScript ``BigInt``) and floors at every division so dust always remains
    with holders.
    """
    epoch_int = _parse_u64(epoch, "epoch")
    h_now = _parse_u64(block_height, "block_height")
    v = _parse_u64(verified_net_assets, "verified_net_assets")
    s = _parse_u64(supply_before, "supply_before")
    u = unit_scale(precision)
    y = _parse_u64(blocks_per_year, "blocks_per_year")
    if y == 0:
        raise ValueError("blocks_per_year must be > 0")
    cap_supply = _parse_u64(max_supply, "max_supply") if max_supply is not None else None

    old_sched = fee_state.active
    pending = fee_state.pending

    if pending is not None and h_now >= pending.effective_at_height:
        new_sched = pending.schedule
        m = min(old_sched.annual_rate_bps, new_sched.annual_rate_bps)
        p = min(old_sched.hwm_rate_bps, new_sched.hwm_rate_bps)
        accrual_cap = min(old_sched.accrual_cap_blocks, new_sched.accrual_cap_blocks)
        next_active = new_sched
        next_pending: PendingNavFeeScheduleV1 | None = None
    else:
        m = old_sched.annual_rate_bps
        p = old_sched.hwm_rate_bps
        accrual_cap = old_sched.accrual_cap_blocks
        next_active = old_sched
        next_pending = pending

    elapsed = h_now - fee_state.last_accrual_height if h_now > fee_state.last_accrual_height else 0
    delta_blocks = min(elapsed, accrual_cap)

    # Edge fail-safe 1: S == 0
    if s == 0:
        accrual = NavFeeAccrualV1(
            asset_id=asset_id,
            epoch=epoch_int,
            block_height=h_now,
            blocks_per_year=y,
            delta_blocks=delta_blocks,
            verified_net_assets=v,
            supply_before=0,
            nav_before=0,
            hwm_before=fee_state.high_water_mark_nav_per_unit,
            annual_rate_bps_applied=m,
            hwm_rate_bps_applied=p,
            time_fee_value=0,
            nav_after_time_fee=0,
            hwm_fee_value=0,
            total_fee_value=0,
            minted_fee_units=0,
            clipped_fee_units=0,
            supply_after=0,
            nav_after=0,
            hwm_after=fee_state.high_water_mark_nav_per_unit,
            reason_code="zero_supply",
        )
        next_state = NavFeeStateV1(
            active=next_active,
            pending=next_pending,
            high_water_mark_nav_per_unit=fee_state.high_water_mark_nav_per_unit,
            last_accrual_height=h_now,
            unclaimed_fee_units=fee_state.unclaimed_fee_units,
            total_accrued_fee_units=fee_state.total_accrued_fee_units,
            total_claimed_fee_units=fee_state.total_claimed_fee_units,
        )
        return next_state, accrual

    # Edge fail-safe 2: V == 0
    if v == 0:
        accrual = NavFeeAccrualV1(
            asset_id=asset_id,
            epoch=epoch_int,
            block_height=h_now,
            blocks_per_year=y,
            delta_blocks=delta_blocks,
            verified_net_assets=0,
            supply_before=s,
            nav_before=0,
            hwm_before=fee_state.high_water_mark_nav_per_unit,
            annual_rate_bps_applied=m,
            hwm_rate_bps_applied=p,
            time_fee_value=0,
            nav_after_time_fee=0,
            hwm_fee_value=0,
            total_fee_value=0,
            minted_fee_units=0,
            clipped_fee_units=0,
            supply_after=s,
            nav_after=0,
            hwm_after=fee_state.high_water_mark_nav_per_unit,
            reason_code="zero_net_assets",
        )
        next_state = NavFeeStateV1(
            active=next_active,
            pending=next_pending,
            high_water_mark_nav_per_unit=fee_state.high_water_mark_nav_per_unit,
            last_accrual_height=h_now,
            unclaimed_fee_units=fee_state.unclaimed_fee_units,
            total_accrued_fee_units=fee_state.total_accrued_fee_units,
            total_claimed_fee_units=fee_state.total_claimed_fee_units,
        )
        return next_state, accrual

    nav_before = (v * u) // s
    # Exact initialization on first S > 0 finalization: zero retroactive HWM taxation
    hwm_before = (
        nav_before
        if fee_state.high_water_mark_nav_per_unit == 0
        else fee_state.high_water_mark_nav_per_unit
    )

    # Edge fail-safe 3: both applied rates are 0
    if m == 0 and p == 0:
        hwm_after = max(hwm_before, nav_before)
        accrual = NavFeeAccrualV1(
            asset_id=asset_id,
            epoch=epoch_int,
            block_height=h_now,
            blocks_per_year=y,
            delta_blocks=delta_blocks,
            verified_net_assets=v,
            supply_before=s,
            nav_before=nav_before,
            hwm_before=hwm_before,
            annual_rate_bps_applied=0,
            hwm_rate_bps_applied=0,
            time_fee_value=0,
            nav_after_time_fee=nav_before,
            hwm_fee_value=0,
            total_fee_value=0,
            minted_fee_units=0,
            clipped_fee_units=0,
            supply_after=s,
            nav_after=nav_before,
            hwm_after=hwm_after,
            reason_code="zero_rates",
        )
        next_state = NavFeeStateV1(
            active=next_active,
            pending=next_pending,
            high_water_mark_nav_per_unit=hwm_after,
            last_accrual_height=h_now,
            unclaimed_fee_units=fee_state.unclaimed_fee_units,
            total_accrued_fee_units=fee_state.total_accrued_fee_units,
            total_claimed_fee_units=fee_state.total_claimed_fee_units,
        )
        return next_state, accrual

    # Step 1: Time-based fee
    f_m = (v * m * delta_blocks) // (NAV_FEE_BPS_DENOMINATOR * y)
    if f_m >= v:
        hwm_after = max(hwm_before, nav_before)
        accrual = NavFeeAccrualV1(
            asset_id=asset_id,
            epoch=epoch_int,
            block_height=h_now,
            blocks_per_year=y,
            delta_blocks=delta_blocks,
            verified_net_assets=v,
            supply_before=s,
            nav_before=nav_before,
            hwm_before=hwm_before,
            annual_rate_bps_applied=m,
            hwm_rate_bps_applied=p,
            time_fee_value=0,
            nav_after_time_fee=nav_before,
            hwm_fee_value=0,
            total_fee_value=0,
            minted_fee_units=0,
            clipped_fee_units=0,
            supply_after=s,
            nav_after=nav_before,
            hwm_after=hwm_after,
            reason_code="clamped_full_value",
        )
        next_state = NavFeeStateV1(
            active=next_active,
            pending=next_pending,
            high_water_mark_nav_per_unit=hwm_after,
            last_accrual_height=h_now,
            unclaimed_fee_units=fee_state.unclaimed_fee_units,
            total_accrued_fee_units=fee_state.total_accrued_fee_units,
            total_claimed_fee_units=fee_state.total_claimed_fee_units,
        )
        return next_state, accrual

    nav_after_time_fee = ((v - f_m) * u) // s

    # Step 2: High-water-mark gain fee
    gain = nav_after_time_fee - hwm_before if nav_after_time_fee > hwm_before else 0
    f_p = (gain * s * p) // (NAV_FEE_BPS_DENOMINATOR * u)

    # Step 3: Atomic unit mint
    f_total = f_m + f_p
    if f_total >= v:
        hwm_after = max(hwm_before, nav_before)
        accrual = NavFeeAccrualV1(
            asset_id=asset_id,
            epoch=epoch_int,
            block_height=h_now,
            blocks_per_year=y,
            delta_blocks=delta_blocks,
            verified_net_assets=v,
            supply_before=s,
            nav_before=nav_before,
            hwm_before=hwm_before,
            annual_rate_bps_applied=m,
            hwm_rate_bps_applied=p,
            time_fee_value=0,
            nav_after_time_fee=nav_before,
            hwm_fee_value=0,
            total_fee_value=0,
            minted_fee_units=0,
            clipped_fee_units=0,
            supply_after=s,
            nav_after=nav_before,
            hwm_after=hwm_after,
            reason_code="clamped_full_value",
        )
        next_state = NavFeeStateV1(
            active=next_active,
            pending=next_pending,
            high_water_mark_nav_per_unit=hwm_after,
            last_accrual_height=h_now,
            unclaimed_fee_units=fee_state.unclaimed_fee_units,
            total_accrued_fee_units=fee_state.total_accrued_fee_units,
            total_claimed_fee_units=fee_state.total_claimed_fee_units,
        )
        return next_state, accrual

    delta_s_raw = (f_total * s) // (v - f_total)

    effective_supply_cap = min(cap_supply, U64_MAX) if cap_supply is not None else U64_MAX
    if s + delta_s_raw > effective_supply_cap:
        minted = effective_supply_cap - s if effective_supply_cap > s else 0
        clipped = delta_s_raw - minted
        reason_code = "clamped_max_supply"
    else:
        minted = delta_s_raw
        clipped = 0
        reason_code = "zero_delta_blocks" if (delta_blocks == 0 and f_total == 0) else "accrued"

    # Step 4: State update
    s_after = s + minted
    nav_after = (v * u) // s_after
    hwm_after = max(hwm_before, nav_after)

    accrual = NavFeeAccrualV1(
        asset_id=asset_id,
        epoch=epoch_int,
        block_height=h_now,
        blocks_per_year=y,
        delta_blocks=delta_blocks,
        verified_net_assets=v,
        supply_before=s,
        nav_before=nav_before,
        hwm_before=hwm_before,
        annual_rate_bps_applied=m,
        hwm_rate_bps_applied=p,
        time_fee_value=f_m,
        nav_after_time_fee=nav_after_time_fee,
        hwm_fee_value=f_p,
        total_fee_value=f_total,
        minted_fee_units=minted,
        clipped_fee_units=clipped,
        supply_after=s_after,
        nav_after=nav_after,
        hwm_after=hwm_after,
        reason_code=reason_code,
    )
    next_state = NavFeeStateV1(
        active=next_active,
        pending=next_pending,
        high_water_mark_nav_per_unit=hwm_after,
        last_accrual_height=h_now,
        unclaimed_fee_units=fee_state.unclaimed_fee_units + minted,
        total_accrued_fee_units=fee_state.total_accrued_fee_units + minted,
        total_claimed_fee_units=fee_state.total_claimed_fee_units,
    )
    return next_state, accrual


# ---------------------------------------------------------------------------
# Inspection, multi-epoch simulation & independent receipt verification
# ---------------------------------------------------------------------------


def inspect_pending_hike(
    fee_state: NavFeeStateV1 | None,
    current_block_height: int | str,
) -> dict[str, Any]:
    """Return a neutral, factual inspection summary of any pending schedule change."""
    h_now = _parse_u64(current_block_height, "current_block_height")
    if fee_state is None:
        return {
            "has_fee_schedule": False,
            "has_pending": False,
            "is_increase": False,
            "exit_capacity_locked": False,
            "blocks_remaining": 0,
            "ready_to_activate": False,
            "changes": {},
        }
    pending = fee_state.pending
    if pending is None:
        return {
            "has_fee_schedule": True,
            "has_pending": False,
            "is_increase": False,
            "exit_capacity_locked": False,
            "blocks_remaining": 0,
            "ready_to_activate": False,
            "changes": {},
        }

    blocks_remaining = (
        pending.effective_at_height - h_now if pending.effective_at_height > h_now else 0
    )
    active = fee_state.active
    proposed = pending.schedule
    tracked_fields = (
        "annual_rate_bps",
        "hwm_rate_bps",
        "issue_spread_bps",
        "redeem_spread_bps",
        "fix_fee_cap_bps",
        "recipient",
        "increase_notice_blocks",
        "accrual_cap_blocks",
        "rates_locked",
        "disclosure_ref",
    )
    changes: dict[str, dict[str, Any]] = {}
    for field in tracked_fields:
        old_val = getattr(active, field)
        new_val = getattr(proposed, field)
        if old_val != new_val:
            changes[field] = {"from": old_val, "to": new_val}

    return {
        "has_fee_schedule": True,
        "has_pending": True,
        "is_increase": pending.is_increase,
        "exit_capacity_locked": pending.is_increase,
        "proposed_at_height": pending.proposed_at_height,
        "effective_at_height": pending.effective_at_height,
        "blocks_remaining": blocks_remaining,
        "ready_to_activate": h_now >= pending.effective_at_height,
        "changes": changes,
    }


def simulate_epochs(
    *,
    asset_id: str,
    issuer: str,
    precision: int,
    initial_fee_state: NavFeeStateV1,
    initial_supply: int | str,
    epochs: Sequence[Mapping[str, Any]],
    max_supply: int | str | None = None,
    max_epoch_gap_blocks: int | None = None,
    blocks_per_year: int = PROTOCOL_BLOCKS_PER_YEAR,
) -> dict[str, Any]:
    """Simulate a sequence of epoch finalizations and optional schedule/claim actions."""
    state = initial_fee_state
    supply = _parse_u64(initial_supply, "initial_supply")
    accruals: list[NavFeeAccrualV1] = []
    events: list[dict[str, Any]] = []

    for step in epochs:
        # Optional pre-epoch actions (propose_update / cancel_pending / claim / supply_delta)
        if "propose_update" in step and step["propose_update"] is not None:
            upd = step["propose_update"]
            state = propose_fee_schedule_update(
                fee_state=state,
                issuer=issuer,
                schedule_input=upd["schedule"],
                block_height=upd["block_height"],
                max_epoch_gap_blocks=max_epoch_gap_blocks,
                recipient_cosigned=bool(upd.get("recipient_cosigned", False)),
            )
            events.append(
                {
                    "kind": NAV_FEE_SCHEDULE_SET_TRANSACTION_KIND,
                    "block_height": _parse_u64(upd["block_height"], "block_height"),
                    "effective_at_height": state.pending.effective_at_height if state.pending else 0,
                    "is_increase": state.pending.is_increase if state.pending else False,
                }
            )

        if step.get("cancel_pending"):
            state = cancel_pending_fee_schedule(state)
            events.append(
                {
                    "kind": NAV_FEE_SCHEDULE_CANCEL_TRANSACTION_KIND,
                    "block_height": _parse_u64(
                        step.get("cancel_block_height", step["block_height"]),
                        "cancel_block_height",
                    ),
                }
            )

        if "supply_before" in step and step["supply_before"] is not None:
            supply = _parse_u64(step["supply_before"], "supply_before")
        else:
            if "external_mint_units" in step and step["external_mint_units"]:
                supply += _parse_u64(step["external_mint_units"], "external_mint_units")
            if "external_redeem_units" in step and step["external_redeem_units"]:
                redeem = _parse_u64(step["external_redeem_units"], "external_redeem_units")
                if redeem > supply:
                    raise ValueError("external_redeem_units exceeds current supply")
                supply -= redeem

        state, accrual = accrue_epoch(
            asset_id=asset_id,
            epoch=step["epoch"],
            block_height=step["block_height"],
            verified_net_assets=step["verified_net_assets"],
            supply_before=supply,
            precision=precision,
            fee_state=state,
            max_supply=max_supply,
            blocks_per_year=blocks_per_year,
        )
        supply = accrual.supply_after
        accruals.append(accrual)

        if "claim" in step and step["claim"] is not None:
            claim_spec = step["claim"]
            state, receipt = claim_accrued_fees(
                fee_state=state,
                caller=claim_spec["caller"],
                amount=claim_spec["amount"],
            )
            events.append({"kind": NAV_FEE_CLAIM_TRANSACTION_KIND, **receipt})

    return {
        "final_fee_state": state,
        "final_supply": supply,
        "accruals": accruals,
        "events": events,
    }


def recompute_fees(
    *,
    asset_id: str,
    epoch: int | str,
    block_height: int | str,
    verified_net_assets: int | str,
    supply_before: int | str,
    precision: int,
    fee_state_before: NavFeeStateV1,
    max_supply: int | str | None = None,
    blocks_per_year: int = PROTOCOL_BLOCKS_PER_YEAR,
) -> tuple[NavFeeStateV1, NavFeeAccrualV1]:
    """Independently recompute an epoch's exact ``NavFeeAccrualV1``."""
    return accrue_epoch(
        asset_id=asset_id,
        epoch=epoch,
        block_height=block_height,
        verified_net_assets=verified_net_assets,
        supply_before=supply_before,
        precision=precision,
        fee_state=fee_state_before,
        max_supply=max_supply,
        blocks_per_year=blocks_per_year,
    )


def verify_nav_receipt(
    *,
    accrual_receipt: NavFeeAccrualV1 | Mapping[str, Any],
    precision: int,
    fee_state_before: NavFeeStateV1,
    max_supply: int | str | None = None,
) -> dict[str, Any]:
    """Verify an on-chain ``NavFeeAccrualV1`` receipt and universal NAV math to the atom."""
    actual = (
        accrual_receipt
        if isinstance(accrual_receipt, NavFeeAccrualV1)
        else NavFeeAccrualV1.from_dict(accrual_receipt)
    )
    next_state, expected = recompute_fees(
        asset_id=actual.asset_id,
        epoch=actual.epoch,
        block_height=actual.block_height,
        verified_net_assets=actual.verified_net_assets,
        supply_before=actual.supply_before,
        precision=precision,
        fee_state_before=fee_state_before,
        max_supply=max_supply,
        blocks_per_year=actual.blocks_per_year,
    )
    expected_dict = expected.to_dict()
    actual_dict = actual.to_dict()
    mismatches: dict[str, dict[str, Any]] = {}
    for key, exp_val in expected_dict.items():
        act_val = actual_dict[key]
        if exp_val != act_val:
            mismatches[key] = {"expected": exp_val, "actual": act_val}

    # Universal post-fee NAV check
    if actual.supply_after > 0:
        expected_post_nav = compute_nav_per_unit(
            actual.verified_net_assets, actual.supply_after, precision
        )
        if actual.nav_after != expected_post_nav:
            mismatches["universal_post_fee_nav"] = {
                "expected": expected_post_nav,
                "actual": actual.nav_after,
            }

    return {
        "valid": len(mismatches) == 0,
        "mismatches": mismatches,
        "expected": expected,
        "actual": actual,
        "next_fee_state": next_state,
    }


# ---------------------------------------------------------------------------
# Open NAVCoin SDK operation builders & RPC helpers (neutral framing)
# ---------------------------------------------------------------------------


def build_nav_fee_schedule_attach_operation(
    *,
    issuer: str,
    asset_id: str,
    schedule: NavFeeScheduleInputV1 | Mapping[str, Any],
    max_epoch_gap_blocks: int | None = None,
) -> dict[str, Any]:
    canonical_issuer = normalize_account(issuer, field_name="issuer")
    validated = validate_fee_schedule_input(
        schedule,
        issuer=canonical_issuer,
        max_epoch_gap_blocks=max_epoch_gap_blocks,
    )
    return {
        "operation": NAV_FEE_SCHEDULE_ATTACH_TRANSACTION_KIND,
        "issuer": canonical_issuer,
        "asset_id": asset_id,
        "schedule": validated.to_dict(),
    }


def build_nav_fee_schedule_set_operation(
    *,
    issuer: str,
    asset_id: str,
    schedule: NavFeeScheduleInputV1 | Mapping[str, Any],
    max_epoch_gap_blocks: int | None = None,
) -> dict[str, Any]:
    canonical_issuer = normalize_account(issuer, field_name="issuer")
    validated = validate_fee_schedule_input(
        schedule,
        issuer=canonical_issuer,
        max_epoch_gap_blocks=max_epoch_gap_blocks,
    )
    return {
        "operation": NAV_FEE_SCHEDULE_SET_TRANSACTION_KIND,
        "issuer": canonical_issuer,
        "asset_id": asset_id,
        "schedule": validated.to_dict(),
    }


def build_nav_fee_schedule_cancel_operation(*, issuer: str, asset_id: str) -> dict[str, Any]:
    canonical_issuer = normalize_account(issuer, field_name="issuer")
    return {
        "operation": NAV_FEE_SCHEDULE_CANCEL_TRANSACTION_KIND,
        "issuer": canonical_issuer,
        "asset_id": asset_id,
    }


def build_nav_fee_claim_operation(
    *,
    recipient: str,
    asset_id: str,
    amount: int | str,
) -> dict[str, Any]:
    canonical_recipient = normalize_account(recipient, field_name="recipient")
    claim_amount = _parse_u64(amount, "amount")
    if claim_amount == 0:
        raise NavFeeError(ERR_INVALID_CLAIM_AMOUNT, "claim amount must be > 0")
    return {
        "operation": NAV_FEE_CLAIM_TRANSACTION_KIND,
        "recipient": canonical_recipient,
        "asset_id": asset_id,
        "amount": claim_amount,
    }


def create_navcoin(
    *,
    chain_id: str,
    issuer: str,
    code: str,
    display_name: str,
    precision: int = 6,
    version: int = 1,
    max_supply: int | str | None = None,
    proof_profile: str = DEFAULT_PROOF_PROFILE,
    valuation_unit: str = DEFAULT_VALUATION_UNIT,
    reserve_operator: str | None = None,
    redemption_account: str | None = None,
    requires_authorization: bool = False,
    freeze_enabled: bool = False,
    clawback_enabled: bool = False,
    fee_schedule: NavFeeScheduleInputV1 | Mapping[str, Any] | None = None,
    access_policy: Mapping[str, Any] | None = None,
    max_epoch_gap_blocks: int | None = None,
) -> dict[str, Any]:
    """Build deterministic launch operations for a zero-fee or fee-enabled NAVCoin.

    When ``fee_schedule`` is ``None`` (the default), zero fee schedule operations
    and zero fee metadata are emitted so unscheduled NAVCoins stay uncluttered.
    """
    canonical_issuer = normalize_account(issuer, field_name="issuer")
    canonical_operator = (
        normalize_account(reserve_operator, field_name="reserve_operator")
        if reserve_operator is not None
        else canonical_issuer
    )
    canonical_redemption = (
        normalize_account(redemption_account, field_name="redemption_account")
        if redemption_account is not None
        else canonical_issuer
    )
    prec = _parse_precision(precision)
    ver = parse_exact_uint(version, "version", max_value=U32_MAX)
    asset_id = derived_asset_id(chain_id, canonical_issuer, code, ver)

    asset_create: dict[str, Any] = {
        "operation": "asset_create",
        "issuer": canonical_issuer,
        "code": code,
        "version": ver,
        "precision": prec,
        "display_name": display_name,
        "requires_authorization": bool(requires_authorization),
        "freeze_enabled": bool(freeze_enabled),
        "clawback_enabled": bool(clawback_enabled),
    }
    if max_supply is not None:
        asset_create["max_supply"] = _parse_u64(max_supply, "max_supply")

    nav_asset_register: dict[str, Any] = {
        "operation": "nav_asset_register",
        "issuer": canonical_issuer,
        "asset_id": asset_id,
        "reserve_operator": canonical_operator,
        "proof_profile": proof_profile,
        "valuation_unit": valuation_unit,
        "redemption_account": canonical_redemption,
    }

    operations: list[dict[str, Any]] = [asset_create, nav_asset_register]

    if access_policy is not None:
        if not requires_authorization:
            raise ValueError(
                "access_policy requires requires_authorization=True on asset_create"
            )
        operations.append(
            {
                "operation": "asset_access_set",
                "issuer": canonical_issuer,
                "asset_id": asset_id,
                **dict(access_policy),
            }
        )

    fee_enabled = fee_schedule is not None
    if fee_schedule is not None:
        operations.append(
            build_nav_fee_schedule_attach_operation(
                issuer=canonical_issuer,
                asset_id=asset_id,
                schedule=fee_schedule,
                max_epoch_gap_blocks=max_epoch_gap_blocks,
            )
        )

    plan: dict[str, Any] = {
        "schema": "postfiat-navcoin-launch-plan-v1",
        "chain_id": chain_id,
        "asset_id": asset_id,
        "issuer": canonical_issuer,
        "code": code,
        "version": ver,
        "precision": prec,
        "fee_enabled": fee_enabled,
        "operations": operations,
    }
    return plan


def build_nav_fee_schedule_rpc_request(asset_id: str, *, request_id: str = "nav-fee-1") -> dict[str, Any]:
    return {
        "id": request_id,
        "method": NAV_FEE_SCHEDULE_RPC_METHOD,
        "params": {"asset_id": asset_id},
    }


def parse_nav_fee_schedule_response(payload: Mapping[str, Any]) -> dict[str, Any]:
    result = payload.get("result", payload)
    if result.get("schema") != NAV_FEE_SCHEDULE_SCHEMA:
        raise ValueError(
            f"unexpected nav_fee_schedule schema: {result.get('schema')!r}"
        )
    has_fee_schedule = bool(result.get("has_fee_schedule", False))
    fee_state_raw = result.get("fee_state")
    fee_state = (
        NavFeeStateV1.from_dict(fee_state_raw)
        if has_fee_schedule and fee_state_raw is not None
        else None
    )
    return {
        "schema": NAV_FEE_SCHEDULE_SCHEMA,
        "chain_id": result.get("chain_id"),
        "block_height": _parse_u64(result.get("block_height", 0), "block_height"),
        "asset_id": result["asset_id"],
        "found": bool(result.get("found", False)),
        "has_fee_schedule": has_fee_schedule,
        "blocks_per_year": _parse_u64(
            result.get("blocks_per_year", PROTOCOL_BLOCKS_PER_YEAR),
            "blocks_per_year",
        ),
        "fee_state": fee_state,
    }


def build_nav_fee_history_rpc_request(
    asset_id: str,
    *,
    from_epoch: int | None = None,
    limit: int = 50,
    cursor: str | None = None,
    request_id: str = "nav-fee-hist-1",
) -> dict[str, Any]:
    params: dict[str, Any] = {
        "asset_id": asset_id,
        "limit": _parse_u32(limit, "limit"),
        "cursor": cursor,
    }
    if from_epoch is not None:
        params["from_epoch"] = _parse_u64(from_epoch, "from_epoch")
    return {
        "id": request_id,
        "method": NAV_FEE_HISTORY_RPC_METHOD,
        "params": params,
    }


def parse_nav_fee_history_response(payload: Mapping[str, Any]) -> dict[str, Any]:
    result = payload.get("result", payload)
    if result.get("schema") != NAV_FEE_HISTORY_SCHEMA:
        raise ValueError(
            f"unexpected nav_fee_history schema: {result.get('schema')!r}"
        )
    accruals = [NavFeeAccrualV1.from_dict(row) for row in result.get("accruals", [])]
    return {
        "schema": NAV_FEE_HISTORY_SCHEMA,
        "chain_id": result.get("chain_id"),
        "asset_id": result["asset_id"],
        "accrual_count": len(accruals),
        "truncated": bool(result.get("truncated", False)),
        "next_cursor": result.get("next_cursor"),
        "accruals": accruals,
    }


# ---------------------------------------------------------------------------
# Deterministic cross-language vector evaluator
# ---------------------------------------------------------------------------


def evaluate_vector(vector: Mapping[str, Any]) -> dict[str, Any]:
    """Execute one vector from ``sdk/navcoin/vectors/nav_fee_vectors_v1.json``."""
    kind = vector["kind"]
    inp = vector["input"]

    if kind == "universal_nav":
        try:
            res = verify_nav_per_unit(
                inp["verified_net_assets"],
                inp["circulating_supply"],
                inp["precision"],
                inp["claimed_nav_per_unit"],
                live_ledger_supply=inp.get("live_ledger_supply"),
            )
            return {
                "ok": True,
                "expected_nav_per_unit": str(res["expected_nav_per_unit"]),
                "floor_remainder_scaled": str(res["floor_remainder_scaled"]),
            }
        except NavFeeError as exc:
            return {"ok": False, "error_code": exc.receipt_code}

    if kind == "schedule_validation":
        subkind = inp.get("subkind", "attach")
        try:
            if subkind == "attach":
                existing = (
                    NavFeeStateV1.from_dict(inp["existing_fee_state"])
                    if inp.get("existing_fee_state") is not None
                    else None
                )
                state = attach_fee_schedule(
                    existing_fee_state=existing,
                    issuer=inp["issuer"],
                    schedule_input=inp["schedule"],
                    block_height=inp["block_height"],
                    current_nav_per_unit=inp.get("current_nav_per_unit", 0),
                    max_epoch_gap_blocks=inp.get("max_epoch_gap_blocks"),
                )
                return {
                    "ok": True,
                    "attached_at_height": str(state.active.attached_at_height),
                    "last_accrual_height": str(state.last_accrual_height),
                    "high_water_mark_nav_per_unit": str(state.high_water_mark_nav_per_unit),
                }
            if subkind == "unscheduled_spread":
                check_unscheduled_spread_update(
                    current_issue_spread_bps=inp["current_issue_spread_bps"],
                    current_redeem_spread_bps=inp["current_redeem_spread_bps"],
                    new_issue_spread_bps=inp["new_issue_spread_bps"],
                    new_redeem_spread_bps=inp["new_redeem_spread_bps"],
                    proposed_at_height=inp["proposed_at_height"],
                    effective_at_height=inp["effective_at_height"],
                )
                return {"ok": True}
            if subkind == "fx_fix_cap":
                fs = (
                    NavFeeStateV1.from_dict(inp["fee_state"])
                    if inp.get("fee_state") is not None
                    else None
                )
                check_fx_fix_fee_cap(inp["fee_bps"], fs)
                return {"ok": True}
            if subkind == "non_issuer_attestor":
                fs = (
                    NavFeeStateV1.from_dict(inp["fee_state"])
                    if inp.get("fee_state") is not None
                    else None
                )
                check_non_issuer_attestation(
                    issuer=inp["issuer"],
                    fee_state=fs,
                    attested_value=inp["attested_value"],
                    attestations=inp["attestations"],
                )
                return {"ok": True}
            raise ValueError(f"unknown schedule_validation subkind: {subkind}")
        except NavFeeError as exc:
            return {"ok": False, "error_code": exc.receipt_code}

    if kind == "single_epoch_accrual":
        fs = NavFeeStateV1.from_dict(inp["fee_state"])
        next_state, accrual = accrue_epoch(
            asset_id=inp["asset_id"],
            epoch=inp["epoch"],
            block_height=inp["block_height"],
            verified_net_assets=inp["verified_net_assets"],
            supply_before=inp["supply_before"],
            precision=inp["precision"],
            fee_state=fs,
            max_supply=inp.get("max_supply"),
            blocks_per_year=parse_exact_uint(
                inp.get("blocks_per_year", PROTOCOL_BLOCKS_PER_YEAR),
                "blocks_per_year",
            ),
        )
        return {
            "ok": True,
            "accrual": accrual.to_dict(stringify_u64=True),
            "next_active_annual_rate_bps": next_state.active.annual_rate_bps,
            "next_active_hwm_rate_bps": next_state.active.hwm_rate_bps,
            "pending_cleared": next_state.pending is None,
            "unclaimed_fee_units": str(next_state.unclaimed_fee_units),
            "total_accrued_fee_units": str(next_state.total_accrued_fee_units),
            "last_accrual_height": str(next_state.last_accrual_height),
        }

    if kind == "pending_notice_and_lifecycle":
        action = inp["action"]
        fs = (
            NavFeeStateV1.from_dict(inp["fee_state"])
            if inp.get("fee_state") is not None
            else None
        )
        try:
            if action == "propose":
                nxt = propose_fee_schedule_update(
                    fee_state=fs,
                    issuer=inp["issuer"],
                    schedule_input=inp["schedule"],
                    block_height=inp["block_height"],
                    max_epoch_gap_blocks=inp.get("max_epoch_gap_blocks"),
                    recipient_cosigned=bool(inp.get("recipient_cosigned", False)),
                )
                assert nxt.pending is not None
                return {
                    "ok": True,
                    "is_increase": nxt.pending.is_increase,
                    "effective_at_height": str(nxt.pending.effective_at_height),
                    "proposed_at_height": str(nxt.pending.proposed_at_height),
                }
            if action == "cancel":
                nxt = cancel_pending_fee_schedule(fs)
                return {"ok": True, "pending_cleared": nxt.pending is None}
            if action == "exit_capacity":
                check_exit_capacity_update(
                    fs,
                    inp["current_redeem_capacity_atoms"],
                    inp["new_redeem_capacity_atoms"],
                )
                return {"ok": True}
            if action == "claim":
                nxt, receipt = claim_accrued_fees(
                    fee_state=fs,
                    caller=inp["caller"],
                    amount=inp["amount"],
                )
                return {
                    "ok": True,
                    "claimed_amount": str(receipt["claimed_amount"]),
                    "unclaimed_remaining": str(nxt.unclaimed_fee_units),
                    "total_claimed_fee_units": str(nxt.total_claimed_fee_units),
                }
            raise ValueError(f"unknown lifecycle action: {action}")
        except NavFeeError as exc:
            return {"ok": False, "error_code": exc.receipt_code}

    if kind == "multi_epoch_compounding":
        fs = NavFeeStateV1.from_dict(inp["initial_fee_state"])
        sim = simulate_epochs(
            asset_id=inp["asset_id"],
            issuer=inp["issuer"],
            precision=inp["precision"],
            initial_fee_state=fs,
            initial_supply=inp["initial_supply"],
            epochs=inp["epochs"],
            max_supply=inp.get("max_supply"),
            max_epoch_gap_blocks=inp.get("max_epoch_gap_blocks"),
            blocks_per_year=parse_exact_uint(
                inp.get("blocks_per_year", PROTOCOL_BLOCKS_PER_YEAR),
                "blocks_per_year",
            ),
        )
        final_state: NavFeeStateV1 = sim["final_fee_state"]
        return {
            "ok": True,
            "final_supply": str(sim["final_supply"]),
            "final_hwm": str(final_state.high_water_mark_nav_per_unit),
            "final_unclaimed_fee_units": str(final_state.unclaimed_fee_units),
            "final_total_accrued_fee_units": str(final_state.total_accrued_fee_units),
            "final_total_claimed_fee_units": str(final_state.total_claimed_fee_units),
            "final_active_annual_rate_bps": final_state.active.annual_rate_bps,
            "final_active_hwm_rate_bps": final_state.active.hwm_rate_bps,
            "final_active_recipient": final_state.active.recipient,
            "final_has_pending": final_state.pending is not None,
            "accruals": [a.to_dict(stringify_u64=True) for a in sim["accruals"]],
        }

    raise ValueError(f"unknown vector kind: {kind}")
