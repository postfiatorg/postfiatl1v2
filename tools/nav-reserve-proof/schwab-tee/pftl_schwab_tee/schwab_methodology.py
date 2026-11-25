"""Strict Schwab-capture adapter for the portfolio-target calculator."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date, datetime, timezone
from decimal import Decimal, InvalidOperation
from typing import Any, Mapping

from .methodology import ContractQuote, MethodologySnapshot
from .models import PrivateOptionChainSnapshot


MICRODOLLARS = 1_000_000


def _integer(value: Any, field: str, *, positive: bool = False) -> int:
    if isinstance(value, bool):
        raise ValueError(f"{field} must be an integer")
    try:
        decimal = Decimal(str(value))
    except (InvalidOperation, ValueError) as exc:
        raise ValueError(f"{field} must be an integer") from exc
    integral = decimal.to_integral_value()
    if decimal != integral:
        raise ValueError(f"{field} must be an integer")
    result = int(integral)
    if result < (1 if positive else 0):
        qualifier = "positive" if positive else "non-negative"
        raise ValueError(f"{field} must be {qualifier}")
    return result


def _microdollars(value: Any, field: str, *, positive: bool = False) -> int:
    if isinstance(value, bool):
        raise ValueError(f"{field} must be an exact decimal price")
    try:
        scaled = Decimal(str(value)) * MICRODOLLARS
    except (InvalidOperation, ValueError) as exc:
        raise ValueError(f"{field} must be an exact decimal price") from exc
    integral = scaled.to_integral_value()
    if scaled != integral:
        raise ValueError(f"{field} exceeds microdollar precision")
    result = int(integral)
    if result < (1 if positive else 0):
        qualifier = "positive" if positive else "non-negative"
        raise ValueError(f"{field} must be {qualifier}")
    return result


def _date(value: Any, field: str) -> date:
    if not isinstance(value, str):
        raise ValueError(f"{field} must be an ISO-8601 date")
    try:
        if len(value) > 10 and value[10] == "T":
            # Schwab encodes expiration calendar dates as offset-aware ISO
            # timestamps. Preserve the provider's declared date component.
            timestamp = datetime.fromisoformat(value)
            if timestamp.tzinfo is None or timestamp.utcoffset() is None:
                raise ValueError("expiration timestamp must include its offset")
            return timestamp.date()
        parsed = date.fromisoformat(value)
    except ValueError as exc:
        raise ValueError(f"{field} must be an ISO-8601 date or offset-aware timestamp") from exc
    if parsed.isoformat() != value:
        raise ValueError(f"{field} must be a canonical ISO-8601 date")
    return parsed


def _quote_time(value: Any) -> datetime:
    milliseconds = _integer(value, "quoteTimeInLong")
    try:
        return datetime.fromtimestamp(milliseconds / 1000, tz=timezone.utc)
    except (OverflowError, OSError, ValueError) as exc:
        raise ValueError("quoteTimeInLong is outside the timestamp range") from exc


@dataclass(frozen=True)
class ContractReferenceTerms:
    occ_symbol: str
    expiration_date: date
    strike_microdollars: int
    option_type: str
    multiplier: int
    deliverable_shares: int
    exercise_style: str
    standard: bool

    def __post_init__(self) -> None:
        if not self.occ_symbol:
            raise ValueError("reference OCC symbol is required")
        if self.option_type not in {"CALL", "PUT"}:
            raise ValueError("reference option type must be CALL or PUT")
        if self.multiplier <= 0 or self.deliverable_shares <= 0:
            raise ValueError("reference multiplier and deliverable must be positive")
        if self.strike_microdollars <= 0:
            raise ValueError("reference strike must be positive")
        if not isinstance(self.standard, bool):
            raise ValueError("reference standard flag must be boolean")


def methodology_snapshot_from_schwab(
    snapshot: PrivateOptionChainSnapshot,
    *,
    underlier_midpoint_microdollars: int,
    underlier_halted: bool,
    option_halt_flags: Mapping[str, bool],
    contract_reference: Mapping[str, ContractReferenceTerms],
) -> MethodologySnapshot:
    """Normalize one captured Schwab snapshot using explicit reference/halt facts.

    No missing contract term or halt flag is inferred from another field.
    Put rows remain in the private collection but are outside this long-call
    target input.
    """

    if (
        not isinstance(underlier_midpoint_microdollars, int)
        or isinstance(underlier_midpoint_microdollars, bool)
        or underlier_midpoint_microdollars <= 0
    ):
        raise ValueError("underlier midpoint must be positive integer microdollars")
    if not isinstance(underlier_halted, bool):
        raise ValueError("underlier halt flag must be boolean")
    call_rows = [
        item
        for item in snapshot.contracts
        if item.get("mapSide") == "CALL"
    ]
    symbols = []
    normalized = []
    for item in call_rows:
        source = item.get("source")
        if not isinstance(source, dict):
            raise ValueError("Schwab contract source must be an object")
        symbol = source.get("symbol")
        if not isinstance(symbol, str) or not symbol:
            raise ValueError("Schwab call is missing its OCC symbol")
        if symbol in symbols:
            raise ValueError("Schwab call identifiers must be unique")
        symbols.append(symbol)
        if symbol not in contract_reference:
            raise ValueError(f"contract reference is missing {symbol!r}")
        if symbol not in option_halt_flags:
            raise ValueError(f"option halt facts are missing {symbol!r}")
        halted = option_halt_flags[symbol]
        if not isinstance(halted, bool):
            raise ValueError("option halt facts must be booleans")
        reference = contract_reference[symbol]
        source_expiration = _date(source.get("expirationDate"), "expirationDate")
        source_strike = _microdollars(
            source.get("strikePrice"), "strikePrice", positive=True
        )
        source_multiplier = _integer(
            source.get("multiplier"), "multiplier", positive=True
        )
        source_type = source.get("putCall")
        if (
            reference.occ_symbol != symbol
            or reference.expiration_date != source_expiration
            or reference.strike_microdollars != source_strike
            or reference.multiplier != source_multiplier
            or reference.option_type != source_type
        ):
            raise ValueError(f"Schwab terms do not match contract reference for {symbol}")
        try:
            normalized_quote = ContractQuote(
                occ_symbol=symbol,
                expiration_date=reference.expiration_date,
                strike_microdollars=reference.strike_microdollars,
                bid_microdollars=_microdollars(
                    source.get("bid"), "bid", positive=False
                ),
                ask_microdollars=_microdollars(
                    source.get("ask"), "ask", positive=False
                ),
                bid_size=_integer(source.get("bidSize"), "bidSize"),
                ask_size=_integer(source.get("askSize"), "askSize"),
                session_volume=_integer(
                    source.get("totalVolume"), "totalVolume"
                ),
                open_interest=_integer(
                    source.get("openInterest"), "openInterest"
                ),
                quote_timestamp_utc=_quote_time(source.get("quoteTimeInLong")),
                option_type=reference.option_type,
                multiplier=reference.multiplier,
                deliverable_shares=reference.deliverable_shares,
                exercise_style=reference.exercise_style,
                standard=reference.standard,
                option_halted=halted,
            )
        except ValueError:
            # The complete source row remains in the private snapshot. Omitting
            # it here records an invalid contract observation, allowing the
            # methodology's explicit 4-of-5 rule to decide eligibility.
            continue
        normalized.append(normalized_quote)
    if set(option_halt_flags) != set(symbols):
        raise ValueError("option halt facts must exactly cover the Schwab call rows")
    return MethodologySnapshot(
        sequence=snapshot.sequence,
        scheduled_at_utc=snapshot.scheduled_at_utc,
        observed_at_utc=snapshot.observed_at_utc,
        underlier_midpoint_microdollars=underlier_midpoint_microdollars,
        underlier_halted=underlier_halted,
        contracts=tuple(sorted(normalized, key=lambda item: item.occ_symbol)),
    )
