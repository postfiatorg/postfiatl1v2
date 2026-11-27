"""Deterministic FxFixPacketV1 wire model, hashing, validation, and op builders.

Mirrors ``crates/types/src/fx_fix_types.rs`` and
``crates/types/src/transactions_mempool_receipts.rs`` byte-for-byte, plus the
Pillar 7 NAVCoin FX fix fee ceiling (Contract C4 Section 3.4).
"""

from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from decimal import Decimal, InvalidOperation
from fractions import Fraction
from typing import Any, Iterable

FX_FIX_PACKET_VERSION_V1 = 1
FX_FIX_PACKET_SCHEMA_V1 = "postfiat.fx_fix.packet.v1"
FX_FIX_PACKET_HASH_DOMAIN_V1 = "postfiat.fx_fix.packet_hash.v1"
FX_FIX_RESERVATION_ID_DOMAIN_V1 = "postfiat.fx_fix.reservation_id.v1"
FX_FIX_OBSERVATION_COMMITMENT_DOMAIN_V1 = "postfiat.fx_fix.source_observation.v1"

FX_FIX_PACKET_HASH_HEX_LEN = 96
FX_FIX_ACTION_BINDING_HASH_HEX_LEN = 128
FX_FIX_RESERVATION_ID_HEX_LEN = 96
FX_FIX_RESERVATION_NONCE_HEX_LEN = 96
FX_FIX_WALLET_INTENT_HASH_HEX_LEN = 96
FX_FIX_SOURCE_COMMITMENT_HEX_LEN = 96
FX_FIX_GOVERNANCE_POLICY_HASH_HEX_LEN = 96
ISSUED_ASSET_ID_HEX_LEN = 96
MAX_FX_FIX_FILLS = 1_024
FX_FIX_NAV_MAX_FEE_BPS = 100  # Locked Pillar 7 protocol cap (1%)
MAX_U64 = (1 << 64) - 1
MAX_TEXT_FIELD_BYTES = 128

_LOWER_HEX_RE = re.compile(r"^[0-9a-f]+$")
_CANONICAL_PF_ADDRESS_RE = re.compile(r"^pf[0-9a-f]{40}$")


def validate_postfiat_address(label: str, value: str) -> None:
    if not isinstance(value, str) or not _CANONICAL_PF_ADDRESS_RE.match(value):
        raise ValueError(f"{label} must be a canonical lowercase pf + 40-hex address")


def validate_lower_hex_len(label: str, value: str, expected_len: int) -> None:
    if (
        not isinstance(value, str)
        or len(value) != expected_len
        or not _LOWER_HEX_RE.match(value)
    ):
        raise ValueError(f"{label} must be {expected_len} lowercase hex characters")


def validate_text_field(label: str, value: str) -> None:
    if (
        not isinstance(value, str)
        or not value
        or len(value.encode("utf-8")) > MAX_TEXT_FIELD_BYTES
        or any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in value)
    ):
        raise ValueError(f"{label} must be a non-empty printable UTF-8 field <= {MAX_TEXT_FIELD_BYTES} bytes")


def _sha3_384_domain_hex(domain: str, payload: bytes) -> str:
    hasher = hashlib.sha3_384()
    hasher.update(domain.encode("utf-8"))
    hasher.update(b"\x00")
    hasher.update(payload)
    return hasher.hexdigest()


@dataclass(frozen=True)
class PriceObservation:
    """Single deterministic venue observation contributing to a fixing window."""

    venue: str
    pair: str
    bid: str
    ask: str
    observed_at_unix: int
    sequence: int = 0

    def validate(self) -> None:
        validate_text_field("observation.venue", self.venue)
        validate_text_field("observation.pair", self.pair)
        if not isinstance(self.observed_at_unix, int) or self.observed_at_unix <= 0:
            raise ValueError("observation.observed_at_unix must be a positive integer")
        if not isinstance(self.sequence, int) or self.sequence < 0:
            raise ValueError("observation.sequence must be non-negative")
        bid_dec = _parse_positive_decimal("observation.bid", self.bid)
        ask_dec = _parse_positive_decimal("observation.ask", self.ask)
        if bid_dec > ask_dec:
            raise ValueError(f"observation bid {self.bid} exceeds ask {self.ask} on {self.venue}")

    def mid_fraction(self) -> Fraction:
        self.validate()
        return (Fraction(Decimal(self.bid)) + Fraction(Decimal(self.ask))) / 2

    def spread_bps(self) -> int:
        self.validate()
        bid = Decimal(self.bid)
        ask = Decimal(self.ask)
        mid = (bid + ask) / Decimal(2)
        return int(((ask - bid) * Decimal(10_000)) // mid)

    def to_canonical_dict(self) -> dict[str, Any]:
        self.validate()
        return {
            "venue": self.venue,
            "pair": self.pair,
            "bid": str(Decimal(self.bid).normalize()),
            "ask": str(Decimal(self.ask).normalize()),
            "observed_at_unix": self.observed_at_unix,
            "sequence": self.sequence,
        }


def _parse_positive_decimal(label: str, raw: str) -> Decimal:
    if not isinstance(raw, str) or not raw.strip():
        raise ValueError(f"{label} must be a non-empty decimal string")
    try:
        val = Decimal(raw)
    except InvalidOperation as exc:
        raise ValueError(f"{label} is not a valid decimal: {raw!r}") from exc
    if not val.is_finite() or val <= 0:
        raise ValueError(f"{label} must be finite and positive")
    return val


def compute_source_observation_commitment(
    window_id: str,
    observations: Iterable[PriceObservation],
) -> str:
    """Compute canonical 96-hex SHA3-384 source_observation_commitment."""
    validate_text_field("window_id", window_id)
    rows = [obs.to_canonical_dict() for obs in observations]
    if not rows:
        raise ValueError("at least one price observation is required")
    rows.sort(key=lambda item: (item["venue"], item["pair"], item["observed_at_unix"], item["sequence"]))
    canonical = json.dumps(
        {"window_id": window_id, "observations": rows},
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    return _sha3_384_domain_hex(FX_FIX_OBSERVATION_COMMITMENT_DOMAIN_V1, canonical)


def compute_median_ratio(
    observations: list[PriceObservation],
    *,
    base_precision: int,
    quote_precision: int,
    fix_timestamp_unix: int,
    max_observation_age_seconds: int = 120,
    max_venue_spread_bps: int = 200,
    max_dispersion_bps: int = 150,
    min_venues: int = 1,
    max_denominator: int = 10**12,
) -> tuple[int, int]:
    """Derive exact integer (ratio_numerator, ratio_denominator) from observations.

    Converts human price ``quote_units / base_unit`` to atomic ratio
    ``quote_atoms / base_atoms = price * 10^quote_precision / 10^base_precision``
    using exact ``Fraction`` arithmetic with zero floating-point rounding drift.
    """
    if len(observations) < min_venues:
        raise ValueError(f"need at least {min_venues} observations, got {len(observations)}")
    if base_precision < 0 or quote_precision < 0 or base_precision > 18 or quote_precision > 18:
        raise ValueError("asset precision must be in 0..=18")

    mids: list[Fraction] = []
    seen_venues: set[str] = set()
    for obs in observations:
        obs.validate()
        if obs.venue in seen_venues:
            raise ValueError(f"duplicate venue observation in fix window: {obs.venue}")
        seen_venues.add(obs.venue)
        age = fix_timestamp_unix - obs.observed_at_unix
        if age < 0:
            raise ValueError(
                f"observation from {obs.venue} is after fix cutoff ({obs.observed_at_unix} > {fix_timestamp_unix})"
            )
        if age > max_observation_age_seconds:
            raise ValueError(
                f"observation from {obs.venue} is stale ({age}s > {max_observation_age_seconds}s)"
            )
        spread = obs.spread_bps()
        if spread > max_venue_spread_bps:
            raise ValueError(
                f"observation from {obs.venue} exceeds max spread ({spread} bps > {max_venue_spread_bps} bps)"
            )
        mids.append(obs.mid_fraction())

    mids.sort()
    count = len(mids)
    median_price = mids[count // 2] if count % 2 == 1 else (mids[count // 2 - 1] + mids[count // 2]) / 2

    if count > 1:
        dispersion_bps = int(((mids[-1] - mids[0]) * 10_000) / median_price)
        if dispersion_bps > max_dispersion_bps:
            raise ValueError(
                f"cross-venue dispersion {dispersion_bps} bps exceeds max {max_dispersion_bps} bps"
            )

    atomic_ratio = median_price * Fraction(10**quote_precision, 10**base_precision)
    bounded = atomic_ratio.limit_denominator(max_denominator)
    num, den = bounded.numerator, bounded.denominator
    if num <= 0 or den <= 0 or num > MAX_U64 or den > MAX_U64:
        raise ValueError(f"derived ratio out of u64 bounds: {num}/{den}")
    return num, den


@dataclass(frozen=True)
class FxFixPacketV1:
    """Canonical Python mirror of ``postfiat_types::FxFixPacketV1``."""

    version: int
    schema: str
    operator: str
    base_asset_id: str
    quote_asset_id: str
    epoch: int
    ratio_numerator: int
    ratio_denominator: int
    band_bps: int
    fee_bps: int
    valid_from_height: int
    expires_at_height: int
    minimum_base_atoms: int
    capacity_base_atoms: int
    capacity_quote_atoms: int
    max_fills: int
    source_label: str
    source_observation_commitment: str
    governance_policy_hash: str
    previous_fix_hash: str | None
    packet_hash: str = ""

    def canonical_preimage(self) -> str:
        previous = self.previous_fix_hash if self.previous_fix_hash is not None else "none"
        return (
            f"version={self.version}\n"
            f"schema={self.schema}\n"
            f"operator={self.operator}\n"
            f"base_asset_id={self.base_asset_id}\n"
            f"quote_asset_id={self.quote_asset_id}\n"
            f"epoch={self.epoch}\n"
            f"ratio_numerator={self.ratio_numerator}\n"
            f"ratio_denominator={self.ratio_denominator}\n"
            f"band_bps={self.band_bps}\n"
            f"fee_bps={self.fee_bps}\n"
            f"valid_from_height={self.valid_from_height}\n"
            f"expires_at_height={self.expires_at_height}\n"
            f"minimum_base_atoms={self.minimum_base_atoms}\n"
            f"capacity_base_atoms={self.capacity_base_atoms}\n"
            f"capacity_quote_atoms={self.capacity_quote_atoms}\n"
            f"max_fills={self.max_fills}\n"
            f"source_label={self.source_label}\n"
            f"source_observation_commitment={self.source_observation_commitment}\n"
            f"governance_policy_hash={self.governance_policy_hash}\n"
            f"previous_fix_hash={previous}\n"
        )

    def canonical_hash(self) -> str:
        return _sha3_384_domain_hex(
            FX_FIX_PACKET_HASH_DOMAIN_V1,
            self.canonical_preimage().encode("utf-8"),
        )

    def with_computed_hash(self) -> FxFixPacketV1:
        return FxFixPacketV1(
            version=self.version,
            schema=self.schema,
            operator=self.operator,
            base_asset_id=self.base_asset_id,
            quote_asset_id=self.quote_asset_id,
            epoch=self.epoch,
            ratio_numerator=self.ratio_numerator,
            ratio_denominator=self.ratio_denominator,
            band_bps=self.band_bps,
            fee_bps=self.fee_bps,
            valid_from_height=self.valid_from_height,
            expires_at_height=self.expires_at_height,
            minimum_base_atoms=self.minimum_base_atoms,
            capacity_base_atoms=self.capacity_base_atoms,
            capacity_quote_atoms=self.capacity_quote_atoms,
            max_fills=self.max_fills,
            source_label=self.source_label,
            source_observation_commitment=self.source_observation_commitment,
            governance_policy_hash=self.governance_policy_hash,
            previous_fix_hash=self.previous_fix_hash,
            packet_hash=self.canonical_hash(),
        )

    def validate(
        self,
        *,
        nav_tracked: bool = False,
        nav_fix_fee_cap_bps: int | None = None,
    ) -> None:
        if self.version != FX_FIX_PACKET_VERSION_V1:
            raise ValueError(f"fx_fix_packet.version must equal {FX_FIX_PACKET_VERSION_V1}")
        if self.schema != FX_FIX_PACKET_SCHEMA_V1:
            raise ValueError(f"fx_fix_packet.schema must equal {FX_FIX_PACKET_SCHEMA_V1}")
        validate_postfiat_address("fx_fix_packet.operator", self.operator)
        validate_lower_hex_len(
            "fx_fix_packet.base_asset_id",
            self.base_asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )
        validate_lower_hex_len(
            "fx_fix_packet.quote_asset_id",
            self.quote_asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )
        if self.base_asset_id == self.quote_asset_id:
            raise ValueError("fx_fix_packet asset pair must be distinct")
        if self.epoch <= 0 or self.epoch > MAX_U64:
            raise ValueError("fx_fix_packet.epoch must be nonzero u64")
        if (
            self.ratio_numerator <= 0
            or self.ratio_denominator <= 0
            or self.ratio_numerator > MAX_U64
            or self.ratio_denominator > MAX_U64
        ):
            raise ValueError("fx_fix_packet ratio terms must be nonzero u64")
        if self.band_bps < 0 or self.band_bps > 10_000 or self.fee_bps < 0 or self.fee_bps > 10_000:
            raise ValueError("fx_fix_packet band_bps and fee_bps must be in 0..=10000")
        if nav_tracked:
            if self.fee_bps > FX_FIX_NAV_MAX_FEE_BPS:
                raise ValueError(
                    f"fx_fix_fee_exceeds_nav_cap: fee_bps {self.fee_bps} > protocol cap {FX_FIX_NAV_MAX_FEE_BPS}"
                )
            if nav_fix_fee_cap_bps is not None and self.fee_bps > nav_fix_fee_cap_bps:
                raise ValueError(
                    f"fx_fix_fee_exceeds_nav_cap: fee_bps {self.fee_bps} > schedule cap {nav_fix_fee_cap_bps}"
                )
        if self.valid_from_height < 0 or self.valid_from_height > self.expires_at_height:
            raise ValueError("fx_fix_packet.valid_from_height must be <= expires_at_height")
        if self.expires_at_height <= 0 or self.expires_at_height > MAX_U64:
            raise ValueError("fx_fix_packet.expires_at_height must be nonzero u64")
        if self.capacity_base_atoms <= 0 or self.capacity_quote_atoms <= 0:
            raise ValueError("fx_fix_packet capacities must be nonzero")
        if self.minimum_base_atoms <= 0 or self.minimum_base_atoms > self.capacity_base_atoms:
            raise ValueError("fx_fix_packet.minimum_base_atoms must be in 1..=capacity_base_atoms")
        if self.max_fills <= 0 or self.max_fills > MAX_FX_FIX_FILLS:
            raise ValueError(f"fx_fix_packet.max_fills must be in 1..={MAX_FX_FIX_FILLS}")
        validate_text_field("fx_fix_packet.source_label", self.source_label)
        validate_lower_hex_len(
            "fx_fix_packet.source_observation_commitment",
            self.source_observation_commitment,
            FX_FIX_SOURCE_COMMITMENT_HEX_LEN,
        )
        validate_lower_hex_len(
            "fx_fix_packet.governance_policy_hash",
            self.governance_policy_hash,
            FX_FIX_GOVERNANCE_POLICY_HASH_HEX_LEN,
        )
        if self.previous_fix_hash is not None:
            validate_lower_hex_len(
                "fx_fix_packet.previous_fix_hash",
                self.previous_fix_hash,
                FX_FIX_PACKET_HASH_HEX_LEN,
            )
        if self.epoch == 1 and self.previous_fix_hash is not None:
            raise ValueError("fx_fix_packet epoch 1 cannot name a previous fix")
        if self.epoch > 1 and self.previous_fix_hash is None:
            raise ValueError("fx_fix_packet epoch greater than 1 requires previous_fix_hash")
        validate_lower_hex_len(
            "fx_fix_packet.packet_hash",
            self.packet_hash,
            FX_FIX_PACKET_HASH_HEX_LEN,
        )
        expected_hash = self.canonical_hash()
        if self.packet_hash != expected_hash:
            raise ValueError("fx_fix_packet.packet_hash does not match canonical fields")

    def quote_atoms_for_base(self, base_atoms: int) -> tuple[int, bool]:
        if base_atoms < self.minimum_base_atoms or base_atoms > self.capacity_base_atoms:
            raise ValueError(
                f"base_atoms must be in {self.minimum_base_atoms}..={self.capacity_base_atoms} for this fix"
            )
        scaled = base_atoms * self.ratio_numerator
        quote_atoms = scaled // self.ratio_denominator
        if quote_atoms <= 0 or quote_atoms > self.capacity_quote_atoms or quote_atoms > MAX_U64:
            raise ValueError("computed quote amount is outside the fix capacity")
        return quote_atoms, (scaled % self.ratio_denominator == 0)

    def to_dict(self) -> dict[str, Any]:
        return {
            "version": self.version,
            "schema": self.schema,
            "operator": self.operator,
            "base_asset_id": self.base_asset_id,
            "quote_asset_id": self.quote_asset_id,
            "epoch": self.epoch,
            "ratio_numerator": self.ratio_numerator,
            "ratio_denominator": self.ratio_denominator,
            "band_bps": self.band_bps,
            "fee_bps": self.fee_bps,
            "valid_from_height": self.valid_from_height,
            "expires_at_height": self.expires_at_height,
            "minimum_base_atoms": self.minimum_base_atoms,
            "capacity_base_atoms": self.capacity_base_atoms,
            "capacity_quote_atoms": self.capacity_quote_atoms,
            "max_fills": self.max_fills,
            "source_label": self.source_label,
            "source_observation_commitment": self.source_observation_commitment,
            "governance_policy_hash": self.governance_policy_hash,
            "previous_fix_hash": self.previous_fix_hash,
            "packet_hash": self.packet_hash,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> FxFixPacketV1:
        return cls(
            version=int(data["version"]),
            schema=str(data["schema"]),
            operator=str(data["operator"]),
            base_asset_id=str(data["base_asset_id"]),
            quote_asset_id=str(data["quote_asset_id"]),
            epoch=int(data["epoch"]),
            ratio_numerator=int(data["ratio_numerator"]),
            ratio_denominator=int(data["ratio_denominator"]),
            band_bps=int(data["band_bps"]),
            fee_bps=int(data["fee_bps"]),
            valid_from_height=int(data["valid_from_height"]),
            expires_at_height=int(data["expires_at_height"]),
            minimum_base_atoms=int(data["minimum_base_atoms"]),
            capacity_base_atoms=int(data["capacity_base_atoms"]),
            capacity_quote_atoms=int(data["capacity_quote_atoms"]),
            max_fills=int(data["max_fills"]),
            source_label=str(data["source_label"]),
            source_observation_commitment=str(data["source_observation_commitment"]),
            governance_policy_hash=str(data["governance_policy_hash"]),
            previous_fix_hash=data.get("previous_fix_hash"),
            packet_hash=str(data["packet_hash"]),
        )


def fx_fix_reservation_id(
    *,
    fix_packet_hash: str,
    operator: str,
    action_binding_hash: str,
    base_atoms: int,
    quote_atoms: int,
    wallet_intent_hash: str,
    reservation_nonce: str,
) -> str:
    """Exact mirror of ``postfiat_types::fx_fix_reservation_id``."""
    validate_lower_hex_len(
        "fx_fix_reservation.fix_packet_hash",
        fix_packet_hash,
        FX_FIX_PACKET_HASH_HEX_LEN,
    )
    validate_postfiat_address("fx_fix_reservation.operator", operator)
    validate_lower_hex_len(
        "fx_fix_reservation.action_binding_hash",
        action_binding_hash,
        FX_FIX_ACTION_BINDING_HASH_HEX_LEN,
    )
    if base_atoms <= 0:
        raise ValueError("fx_fix_reservation.base_atoms must be nonzero")
    if quote_atoms <= 0:
        raise ValueError("fx_fix_reservation.quote_atoms must be nonzero")
    validate_lower_hex_len(
        "fx_fix_reservation.wallet_intent_hash",
        wallet_intent_hash,
        FX_FIX_WALLET_INTENT_HASH_HEX_LEN,
    )
    validate_lower_hex_len(
        "fx_fix_reservation.reservation_nonce",
        reservation_nonce,
        FX_FIX_RESERVATION_NONCE_HEX_LEN,
    )
    preimage = (
        f"fix_packet_hash={fix_packet_hash}\n"
        f"operator={operator}\n"
        f"action_binding_hash={action_binding_hash}\n"
        f"base_atoms={base_atoms}\n"
        f"quote_atoms={quote_atoms}\n"
        f"wallet_intent_hash={wallet_intent_hash}\n"
        f"reservation_nonce={reservation_nonce}\n"
    )
    return _sha3_384_domain_hex(FX_FIX_RESERVATION_ID_DOMAIN_V1, preimage.encode("utf-8"))


def build_fx_fix_register_op(
    packet: FxFixPacketV1,
    *,
    nav_tracked: bool = False,
    nav_fix_fee_cap_bps: int | None = None,
) -> dict[str, Any]:
    packet.validate(nav_tracked=nav_tracked, nav_fix_fee_cap_bps=nav_fix_fee_cap_bps)
    return {
        "operation": "fx_fix_register_v1",
        "operator": packet.operator,
        "packet": packet.to_dict(),
    }


def build_fx_fix_pause_op(operator: str, fix_packet_hash: str, paused: bool) -> dict[str, Any]:
    validate_postfiat_address("fx_fix_pause_v1.operator", operator)
    validate_lower_hex_len("fx_fix_pause_v1.fix_packet_hash", fix_packet_hash, FX_FIX_PACKET_HASH_HEX_LEN)
    return {
        "operation": "fx_fix_pause_v1",
        "operator": operator,
        "fix_packet_hash": fix_packet_hash,
        "paused": bool(paused),
    }


def build_fx_fix_reservation_create_op(
    *,
    operator: str,
    fix_packet_hash: str,
    action_binding_hash: str,
    base_atoms: int,
    quote_atoms: int,
    wallet_intent_hash: str,
    reservation_nonce: str,
    expires_at_height: int,
) -> tuple[dict[str, Any], str]:
    if expires_at_height <= 0:
        raise ValueError("fx_fix_reservation_create_v1.expires_at_height must be nonzero")
    reservation_id = fx_fix_reservation_id(
        fix_packet_hash=fix_packet_hash,
        operator=operator,
        action_binding_hash=action_binding_hash,
        base_atoms=base_atoms,
        quote_atoms=quote_atoms,
        wallet_intent_hash=wallet_intent_hash,
        reservation_nonce=reservation_nonce,
    )
    op = {
        "operation": "fx_fix_reservation_create_v1",
        "operator": operator,
        "fix_packet_hash": fix_packet_hash,
        "action_binding_hash": action_binding_hash,
        "base_atoms": base_atoms,
        "quote_atoms": quote_atoms,
        "wallet_intent_hash": wallet_intent_hash,
        "reservation_nonce": reservation_nonce,
        "expires_at_height": expires_at_height,
    }
    return op, reservation_id


def build_fx_fix_reservation_release_op(operator: str, reservation_id: str) -> dict[str, Any]:
    validate_postfiat_address("fx_fix_reservation_release_v1.operator", operator)
    validate_lower_hex_len(
        "fx_fix_reservation_release_v1.reservation_id",
        reservation_id,
        FX_FIX_RESERVATION_ID_HEX_LEN,
    )
    return {
        "operation": "fx_fix_reservation_release_v1",
        "operator": operator,
        "reservation_id": reservation_id,
    }
