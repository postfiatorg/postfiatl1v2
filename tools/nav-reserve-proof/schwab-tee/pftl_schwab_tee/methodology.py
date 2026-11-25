"""Deterministic portfolio-target calculator for the YOLO reference methodology.

This module creates theoretical target quantities only. It has no broker client,
order type, execution loop, or account mutation capability.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
from datetime import date, datetime, timezone
from typing import Any, Literal

from .canonical import domain_sha256


PPB = 1_000_000_000
PARAMETER_SCHEMA = "postfiat.yolo.portfolio_parameters.v1"
TARGET_INPUT_SCHEMA = "postfiat.yolo.portfolio_target_input.v1"
TARGET_OUTPUT_SCHEMA = "postfiat.yolo.portfolio_target.v1"


def _utc(value: datetime, field: str) -> datetime:
    if (
        not isinstance(value, datetime)
        or value.tzinfo is None
        or value.utcoffset() is None
    ):
        raise ValueError(f"{field} must be timezone-aware")
    return value.astimezone(timezone.utc)


def _utc_text(value: datetime) -> str:
    return _utc(value, "timestamp").isoformat().replace("+00:00", "Z")


def _positive_integer(value: Any, field: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        raise ValueError(f"{field} must be a positive integer")
    return value


def _nonnegative_integer(value: Any, field: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ValueError(f"{field} must be a non-negative integer")
    return value


def _strict_dict(value: Any, fields: tuple[str, ...], name: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"{name} must be an object")
    expected = set(fields)
    actual = set(value)
    if actual != expected:
        missing = sorted(expected - actual)
        unknown = sorted(actual - expected)
        detail = f"missing {missing[0]!r}" if missing else f"unknown {unknown[0]!r}"
        raise ValueError(f"{name} has {detail}")
    return value


def _median(values: list[int]) -> int:
    if not values:
        raise ValueError("median requires at least one value")
    ordered = sorted(values)
    middle = len(ordered) // 2
    if len(ordered) % 2:
        return ordered[middle]
    return (ordered[middle - 1] + ordered[middle]) // 2


def _parse_date(value: Any, field: str) -> date:
    if not isinstance(value, str):
        raise ValueError(f"{field} must be an ISO-8601 date")
    try:
        parsed = date.fromisoformat(value)
    except ValueError as exc:
        raise ValueError(f"{field} must be an ISO-8601 date") from exc
    if parsed.isoformat() != value:
        raise ValueError(f"{field} must be a canonical ISO-8601 date")
    return parsed


def _parse_utc(value: Any, field: str) -> datetime:
    if not isinstance(value, str) or not value.endswith("Z"):
        raise ValueError(f"{field} must use canonical UTC Z form")
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as exc:
        raise ValueError(f"{field} must be an ISO-8601 timestamp") from exc
    parsed = _utc(parsed, field)
    if _utc_text(parsed) != value:
        raise ValueError(f"{field} must use canonical UTC Z form")
    return parsed


@dataclass(frozen=True)
class PortfolioParameters:
    target_premium_ppb: int
    basket_size: int
    observation_count: int
    per_contract_valid_quotes: int
    maximum_quote_age_seconds: int
    scheduled_time_tolerance_seconds: int
    successor_minimum_dte: int
    successor_maximum_dte: int
    target_dte: int
    roll_dte: int
    minimum_moneyness_ppb: int
    maximum_moneyness_ppb: int
    minimum_open_interest: int
    minimum_session_volume: int
    minimum_two_sided_size: int
    maximum_relative_spread_bps: int
    spread_weight_ppb: int
    open_interest_weight_ppb: int
    volume_weight_ppb: int
    displayed_size_weight_ppb: int
    open_interest_cap: int
    volume_cap: int
    displayed_size_cap: int
    strike_retention_rank: int

    def __post_init__(self) -> None:
        for field in self.__dataclass_fields__:
            _positive_integer(getattr(self, field), field)
        if self.target_premium_ppb > PPB:
            raise ValueError("target_premium_ppb must not exceed PPB")
        if self.per_contract_valid_quotes > self.observation_count:
            raise ValueError("valid quote requirement exceeds observation count")
        if self.successor_minimum_dte > self.successor_maximum_dte:
            raise ValueError("successor DTE interval is inverted")
        if not (
            self.successor_minimum_dte
            <= self.target_dte
            <= self.successor_maximum_dte
        ):
            raise ValueError("target DTE must be inside the successor interval")
        if self.minimum_moneyness_ppb > self.maximum_moneyness_ppb:
            raise ValueError("moneyness interval is inverted")
        if sum(
            (
                self.spread_weight_ppb,
                self.open_interest_weight_ppb,
                self.volume_weight_ppb,
                self.displayed_size_weight_ppb,
            )
        ) != PPB:
            raise ValueError("liquidity weights must sum to PPB")
        if self.strike_retention_rank < self.basket_size:
            raise ValueError("retention rank must be at least basket size")

    @classmethod
    def from_dict(cls, value: Any) -> PortfolioParameters:
        fields = tuple(cls.__dataclass_fields__)
        item = _strict_dict(value, ("schema", *fields), "portfolio parameters")
        if item["schema"] != PARAMETER_SCHEMA:
            raise ValueError("portfolio parameter schema mismatch")
        return cls(**{field: item[field] for field in fields})

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": PARAMETER_SCHEMA,
            **{field: getattr(self, field) for field in self.__dataclass_fields__},
        }

    @property
    def sha256(self) -> str:
        return domain_sha256(PARAMETER_SCHEMA, self.to_dict())


@dataclass(frozen=True)
class ContractQuote:
    occ_symbol: str
    expiration_date: date
    strike_microdollars: int
    bid_microdollars: int
    ask_microdollars: int
    bid_size: int
    ask_size: int
    session_volume: int
    open_interest: int
    quote_timestamp_utc: datetime
    option_type: str
    multiplier: int
    deliverable_shares: int
    exercise_style: str
    standard: bool
    option_halted: bool

    def __post_init__(self) -> None:
        if not isinstance(self.occ_symbol, str) or not self.occ_symbol:
            raise ValueError("occ_symbol is required")
        if not isinstance(self.expiration_date, date):
            raise ValueError("expiration_date must be a date")
        _positive_integer(self.strike_microdollars, "strike_microdollars")
        for field in (
            "bid_microdollars",
            "ask_microdollars",
            "bid_size",
            "ask_size",
            "session_volume",
            "open_interest",
        ):
            _nonnegative_integer(getattr(self, field), field)
        _positive_integer(self.multiplier, "multiplier")
        _positive_integer(self.deliverable_shares, "deliverable_shares")
        _utc(self.quote_timestamp_utc, "quote_timestamp_utc")
        if not isinstance(self.standard, bool) or not isinstance(
            self.option_halted, bool
        ):
            raise ValueError("standard and option_halted must be booleans")

    @property
    def static_terms(self) -> tuple[Any, ...]:
        return (
            self.expiration_date,
            self.strike_microdollars,
            self.option_type,
            self.multiplier,
            self.deliverable_shares,
            self.exercise_style,
            self.standard,
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "occSymbol": self.occ_symbol,
            "expirationDate": self.expiration_date.isoformat(),
            "strikeMicrodollars": self.strike_microdollars,
            "bidMicrodollars": self.bid_microdollars,
            "askMicrodollars": self.ask_microdollars,
            "bidSize": self.bid_size,
            "askSize": self.ask_size,
            "sessionVolume": self.session_volume,
            "openInterest": self.open_interest,
            "quoteTimestampUtc": _utc_text(self.quote_timestamp_utc),
            "optionType": self.option_type,
            "multiplier": self.multiplier,
            "deliverableShares": self.deliverable_shares,
            "exerciseStyle": self.exercise_style,
            "standard": self.standard,
            "optionHalted": self.option_halted,
        }

    @classmethod
    def from_dict(cls, value: Any) -> ContractQuote:
        fields = (
            "occSymbol",
            "expirationDate",
            "strikeMicrodollars",
            "bidMicrodollars",
            "askMicrodollars",
            "bidSize",
            "askSize",
            "sessionVolume",
            "openInterest",
            "quoteTimestampUtc",
            "optionType",
            "multiplier",
            "deliverableShares",
            "exerciseStyle",
            "standard",
            "optionHalted",
        )
        item = _strict_dict(value, fields, "contract quote")
        return cls(
            occ_symbol=item["occSymbol"],
            expiration_date=_parse_date(item["expirationDate"], "expirationDate"),
            strike_microdollars=item["strikeMicrodollars"],
            bid_microdollars=item["bidMicrodollars"],
            ask_microdollars=item["askMicrodollars"],
            bid_size=item["bidSize"],
            ask_size=item["askSize"],
            session_volume=item["sessionVolume"],
            open_interest=item["openInterest"],
            quote_timestamp_utc=_parse_utc(
                item["quoteTimestampUtc"], "quoteTimestampUtc"
            ),
            option_type=item["optionType"],
            multiplier=item["multiplier"],
            deliverable_shares=item["deliverableShares"],
            exercise_style=item["exerciseStyle"],
            standard=item["standard"],
            option_halted=item["optionHalted"],
        )


@dataclass(frozen=True)
class MethodologySnapshot:
    sequence: int
    scheduled_at_utc: datetime
    observed_at_utc: datetime
    underlier_midpoint_microdollars: int
    underlier_halted: bool
    contracts: tuple[ContractQuote, ...]

    def __post_init__(self) -> None:
        _nonnegative_integer(self.sequence, "sequence")
        _utc(self.scheduled_at_utc, "scheduled_at_utc")
        _utc(self.observed_at_utc, "observed_at_utc")
        _positive_integer(
            self.underlier_midpoint_microdollars,
            "underlier_midpoint_microdollars",
        )
        if not isinstance(self.underlier_halted, bool):
            raise ValueError("underlier_halted must be boolean")
        symbols = [quote.occ_symbol for quote in self.contracts]
        if len(set(symbols)) != len(symbols):
            raise ValueError("contract identifiers must be unique in each snapshot")

    def to_dict(self) -> dict[str, Any]:
        return {
            "sequence": self.sequence,
            "scheduledAtUtc": _utc_text(self.scheduled_at_utc),
            "observedAtUtc": _utc_text(self.observed_at_utc),
            "underlierMidpointMicrodollars": self.underlier_midpoint_microdollars,
            "underlierHalted": self.underlier_halted,
            "contracts": [contract.to_dict() for contract in self.contracts],
        }

    @classmethod
    def from_dict(cls, value: Any) -> MethodologySnapshot:
        fields = (
            "sequence",
            "scheduledAtUtc",
            "observedAtUtc",
            "underlierMidpointMicrodollars",
            "underlierHalted",
            "contracts",
        )
        item = _strict_dict(value, fields, "methodology snapshot")
        if not isinstance(item["contracts"], list):
            raise ValueError("methodology snapshot contracts must be an array")
        return cls(
            sequence=item["sequence"],
            scheduled_at_utc=_parse_utc(item["scheduledAtUtc"], "scheduledAtUtc"),
            observed_at_utc=_parse_utc(item["observedAtUtc"], "observedAtUtc"),
            underlier_midpoint_microdollars=item[
                "underlierMidpointMicrodollars"
            ],
            underlier_halted=item["underlierHalted"],
            contracts=tuple(
                ContractQuote.from_dict(contract) for contract in item["contracts"]
            ),
        )


@dataclass(frozen=True)
class CurrentPosition:
    occ_symbol: str
    quantity: int

    def __post_init__(self) -> None:
        if not isinstance(self.occ_symbol, str) or not self.occ_symbol:
            raise ValueError("position occ_symbol is required")
        _nonnegative_integer(self.quantity, "position quantity")

    def to_dict(self) -> dict[str, Any]:
        return {"occSymbol": self.occ_symbol, "quantity": self.quantity}

    @classmethod
    def from_dict(cls, value: Any) -> CurrentPosition:
        item = _strict_dict(value, ("occSymbol", "quantity"), "current position")
        return cls(occ_symbol=item["occSymbol"], quantity=item["quantity"])


@dataclass(frozen=True)
class PortfolioTargetInput:
    series_id: str
    underlier_id: str
    trade_date: date
    methodology_sha256: str
    collection_sha256: str
    snapshots: tuple[MethodologySnapshot, ...]
    positions: tuple[CurrentPosition, ...]
    settled_cash_microdollars: int
    unsettled_cash_microdollars: int
    accrued_liabilities_microdollars: int
    incumbent_expiration: date | None
    incumbent_symbols: tuple[str, ...]

    def __post_init__(self) -> None:
        if not self.series_id or not self.underlier_id:
            raise ValueError("series_id and underlier_id are required")
        if not isinstance(self.trade_date, date):
            raise ValueError("trade_date must be a date")
        for field in ("methodology_sha256", "collection_sha256"):
            value = getattr(self, field)
            if (
                not isinstance(value, str)
                or len(value) != 64
                or any(character not in "0123456789abcdef" for character in value)
            ):
                raise ValueError(f"{field} must be a lowercase SHA-256 digest")
        for field in (
            "settled_cash_microdollars",
            "unsettled_cash_microdollars",
            "accrued_liabilities_microdollars",
        ):
            _nonnegative_integer(getattr(self, field), field)
        symbols = [position.occ_symbol for position in self.positions]
        if len(set(symbols)) != len(symbols):
            raise ValueError("positions must be unique by OCC symbol")
        if len(set(self.incumbent_symbols)) != len(self.incumbent_symbols):
            raise ValueError("incumbent symbols must be unique")
        if self.incumbent_expiration is not None and not isinstance(
            self.incumbent_expiration, date
        ):
            raise ValueError("incumbent_expiration must be null or a date")
        if any(
            not isinstance(symbol, str) or not symbol
            for symbol in self.incumbent_symbols
        ):
            raise ValueError("incumbent symbols must be non-empty strings")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": TARGET_INPUT_SCHEMA,
            "seriesId": self.series_id,
            "underlierId": self.underlier_id,
            "tradeDate": self.trade_date.isoformat(),
            "methodologySha256": self.methodology_sha256,
            "collectionSha256": self.collection_sha256,
            "snapshots": [snapshot.to_dict() for snapshot in self.snapshots],
            "positions": [position.to_dict() for position in self.positions],
            "settledCashMicrodollars": self.settled_cash_microdollars,
            "unsettledCashMicrodollars": self.unsettled_cash_microdollars,
            "accruedLiabilitiesMicrodollars": (
                self.accrued_liabilities_microdollars
            ),
            "incumbentExpiration": (
                None
                if self.incumbent_expiration is None
                else self.incumbent_expiration.isoformat()
            ),
            "incumbentSymbols": list(self.incumbent_symbols),
        }

    @classmethod
    def from_dict(cls, value: Any) -> PortfolioTargetInput:
        fields = (
            "schema",
            "seriesId",
            "underlierId",
            "tradeDate",
            "methodologySha256",
            "collectionSha256",
            "snapshots",
            "positions",
            "settledCashMicrodollars",
            "unsettledCashMicrodollars",
            "accruedLiabilitiesMicrodollars",
            "incumbentExpiration",
            "incumbentSymbols",
        )
        item = _strict_dict(value, fields, "portfolio target input")
        if item["schema"] != TARGET_INPUT_SCHEMA:
            raise ValueError("portfolio target input schema mismatch")
        for field in ("snapshots", "positions", "incumbentSymbols"):
            if not isinstance(item[field], list):
                raise ValueError(f"{field} must be an array")
        incumbent = item["incumbentExpiration"]
        if incumbent is not None:
            incumbent = _parse_date(incumbent, "incumbentExpiration")
        return cls(
            series_id=item["seriesId"],
            underlier_id=item["underlierId"],
            trade_date=_parse_date(item["tradeDate"], "tradeDate"),
            methodology_sha256=item["methodologySha256"],
            collection_sha256=item["collectionSha256"],
            snapshots=tuple(
                MethodologySnapshot.from_dict(snapshot)
                for snapshot in item["snapshots"]
            ),
            positions=tuple(
                CurrentPosition.from_dict(position) for position in item["positions"]
            ),
            settled_cash_microdollars=item["settledCashMicrodollars"],
            unsettled_cash_microdollars=item["unsettledCashMicrodollars"],
            accrued_liabilities_microdollars=item[
                "accruedLiabilitiesMicrodollars"
            ],
            incumbent_expiration=incumbent,
            incumbent_symbols=tuple(item["incumbentSymbols"]),
        )


@dataclass(frozen=True)
class AggregatedContract:
    occ_symbol: str
    expiration_date: date
    strike_microdollars: int
    multiplier: int
    bid_microdollars: int
    ask_microdollars: int
    midpoint_microdollars: int
    two_sided_size: int
    session_volume: int
    open_interest: int
    spread_bps: int
    moneyness_ppb: int
    dte: int
    valid_quote_count: int
    option_halted: bool
    mandate_eligible: bool
    liquidity_ppb: int
    exclusions: tuple[str, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "occSymbol": self.occ_symbol,
            "expirationDate": self.expiration_date.isoformat(),
            "strikeMicrodollars": self.strike_microdollars,
            "multiplier": self.multiplier,
            "bidMicrodollars": self.bid_microdollars,
            "askMicrodollars": self.ask_microdollars,
            "midpointMicrodollars": self.midpoint_microdollars,
            "twoSidedSize": self.two_sided_size,
            "sessionVolume": self.session_volume,
            "openInterest": self.open_interest,
            "spreadBps": self.spread_bps,
            "moneynessPpb": self.moneyness_ppb,
            "dte": self.dte,
            "validQuoteCount": self.valid_quote_count,
            "optionHalted": self.option_halted,
            "mandateEligible": self.mandate_eligible,
            "liquidityPpb": self.liquidity_ppb,
            "exclusions": list(self.exclusions),
        }


@dataclass(frozen=True)
class TargetPosition:
    occ_symbol: str
    expiration_date: date
    midpoint_microdollars: int
    multiplier: int
    current_quantity: int
    target_quantity: int
    target_delta: int
    liquidity_ppb: int
    retained_incumbent: bool

    def to_dict(self) -> dict[str, Any]:
        return {
            "occSymbol": self.occ_symbol,
            "expirationDate": self.expiration_date.isoformat(),
            "midpointMicrodollars": self.midpoint_microdollars,
            "multiplier": self.multiplier,
            "currentQuantity": self.current_quantity,
            "targetQuantity": self.target_quantity,
            "targetDelta": self.target_delta,
            "liquidityPpb": self.liquidity_ppb,
            "retainedIncumbent": self.retained_incumbent,
        }


@dataclass(frozen=True)
class PortfolioTarget:
    status: Literal[
        "TARGET_COMPUTED",
        "NO_RECONSTITUTION",
        "NO_ELIGIBLE_SUCCESSOR",
        "HALTED_INSTRUMENT",
        "UNRESOLVED_CAPITAL_POLICY",
    ]
    reason: str
    parameter_manifest_sha256: str
    methodology_sha256: str
    collection_sha256: str
    selected_expiration: date | None
    decision_nav_microdollars: int | None
    premium_budget_microdollars: int | None
    sleeve_microdollars: int | None
    positions: tuple[TargetPosition, ...]
    excluded_contracts: tuple[AggregatedContract, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": TARGET_OUTPUT_SCHEMA,
            "status": self.status,
            "reason": self.reason,
            "parameterManifestSha256": self.parameter_manifest_sha256,
            "methodologySha256": self.methodology_sha256,
            "collectionSha256": self.collection_sha256,
            "selectedExpiration": (
                None
                if self.selected_expiration is None
                else self.selected_expiration.isoformat()
            ),
            "decisionNavMicrodollars": self.decision_nav_microdollars,
            "premiumBudgetMicrodollars": self.premium_budget_microdollars,
            "sleeveMicrodollars": self.sleeve_microdollars,
            "positions": [position.to_dict() for position in self.positions],
            "excludedContracts": [
                contract.to_dict() for contract in self.excluded_contracts
            ],
        }

    @property
    def sha256(self) -> str:
        return domain_sha256(TARGET_OUTPUT_SCHEMA, self.to_dict())


def _valid_quote(
    quote: ContractQuote,
    observed_at: datetime,
    parameters: PortfolioParameters,
) -> bool:
    age = (_utc(observed_at, "observed_at") - _utc(
        quote.quote_timestamp_utc, "quote_timestamp"
    )).total_seconds()
    return (
        0 < quote.bid_microdollars <= quote.ask_microdollars
        and 0 <= age <= parameters.maximum_quote_age_seconds
    )


def _liquidity(
    *,
    spread_bps: int,
    open_interest: int,
    volume: int,
    size: int,
    parameters: PortfolioParameters,
) -> int:
    spread_quality = max(
        0,
        PPB - PPB * spread_bps // parameters.maximum_relative_spread_bps,
    )
    oi = min(PPB, PPB * open_interest // parameters.open_interest_cap)
    vol = min(PPB, PPB * volume // parameters.volume_cap)
    displayed = min(PPB, PPB * size // parameters.displayed_size_cap)
    return (
        parameters.spread_weight_ppb * spread_quality
        + parameters.open_interest_weight_ppb * oi
        + parameters.volume_weight_ppb * vol
        + parameters.displayed_size_weight_ppb * displayed
    ) // PPB


def target_quantity_from_midpoint(
    *,
    sleeve_microdollars: int,
    midpoint_microdollars: int,
    multiplier: int,
) -> int:
    """Apply the Section 10 side-independent integer sizing formula."""

    _nonnegative_integer(sleeve_microdollars, "sleeve_microdollars")
    _positive_integer(midpoint_microdollars, "midpoint_microdollars")
    _positive_integer(multiplier, "multiplier")
    return sleeve_microdollars // (midpoint_microdollars * multiplier)


def _aggregate(
    target: PortfolioTargetInput,
    parameters: PortfolioParameters,
) -> tuple[int, tuple[AggregatedContract, ...]]:
    snapshots = tuple(sorted(target.snapshots, key=lambda item: item.sequence))
    if len(snapshots) != parameters.observation_count or tuple(
        item.sequence for item in snapshots
    ) != tuple(range(parameters.observation_count)):
        raise ValueError("collection does not contain the required snapshot sequence")
    for snapshot in snapshots:
        drift = abs(
            (
                _utc(snapshot.observed_at_utc, "observed")
                - _utc(snapshot.scheduled_at_utc, "scheduled")
            ).total_seconds()
        )
        if drift > parameters.scheduled_time_tolerance_seconds:
            raise ValueError("snapshot exceeds the scheduled-time tolerance")
    underlier = _median(
        [snapshot.underlier_midpoint_microdollars for snapshot in snapshots]
    )
    symbols = sorted(
        {
            quote.occ_symbol
            for snapshot in snapshots
            for quote in snapshot.contracts
        }
    )
    contracts: list[AggregatedContract] = []
    for symbol in symbols:
        observations = [
            (snapshot, quote)
            for snapshot in snapshots
            for quote in snapshot.contracts
            if quote.occ_symbol == symbol
            and _valid_quote(quote, snapshot.observed_at_utc, parameters)
        ]
        if not observations:
            continue
        terms = {quote.static_terms for _, quote in observations}
        consistent = len(terms) == 1
        representative = observations[0][1]
        valid_count = len(observations) if consistent else 0
        bids = [quote.bid_microdollars for _, quote in observations]
        asks = [quote.ask_microdollars for _, quote in observations]
        sizes = [min(quote.bid_size, quote.ask_size) for _, quote in observations]
        interests = [quote.open_interest for _, quote in observations]
        volumes = [quote.session_volume for _, quote in observations]
        bid = _median(bids)
        ask = _median(asks)
        midpoint = (bid + ask) // 2
        spread_bps = 10_000 * (ask - bid) // midpoint
        size = _median(sizes)
        interest = _median(interests)
        volume = max(volumes)
        dte = representative.expiration_date.toordinal() - target.trade_date.toordinal()
        moneyness = representative.strike_microdollars * PPB // underlier
        final_quote = next(
            (
                quote
                for quote in snapshots[-1].contracts
                if quote.occ_symbol == symbol
            ),
            None,
        )
        halted = final_quote is None or final_quote.option_halted
        mandate = (
            consistent
            and representative.standard
            and representative.option_type == "CALL"
            and representative.multiplier == 100
            and representative.deliverable_shares == 100
            and representative.exercise_style == "AMERICAN"
        )
        exclusions: list[str] = []
        for condition, reason in (
            (not consistent, "inconsistent_contract_terms"),
            (not mandate, "mandate_terms"),
            (
                valid_count < parameters.per_contract_valid_quotes,
                "insufficient_valid_quotes",
            ),
            (halted, "halted_or_missing_target_flag"),
            (
                not (
                    parameters.minimum_moneyness_ppb
                    <= moneyness
                    <= parameters.maximum_moneyness_ppb
                ),
                "moneyness",
            ),
            (
                spread_bps > parameters.maximum_relative_spread_bps,
                "relative_spread",
            ),
            (interest < parameters.minimum_open_interest, "open_interest"),
            (volume < parameters.minimum_session_volume, "session_volume"),
            (size < parameters.minimum_two_sided_size, "two_sided_size"),
        ):
            if condition:
                exclusions.append(reason)
        contracts.append(
            AggregatedContract(
                occ_symbol=symbol,
                expiration_date=representative.expiration_date,
                strike_microdollars=representative.strike_microdollars,
                multiplier=representative.multiplier,
                bid_microdollars=bid,
                ask_microdollars=ask,
                midpoint_microdollars=midpoint,
                two_sided_size=size,
                session_volume=volume,
                open_interest=interest,
                spread_bps=spread_bps,
                moneyness_ppb=moneyness,
                dte=dte,
                valid_quote_count=valid_count,
                option_halted=halted,
                mandate_eligible=mandate,
                liquidity_ppb=_liquidity(
                    spread_bps=spread_bps,
                    open_interest=interest,
                    volume=volume,
                    size=size,
                    parameters=parameters,
                ),
                exclusions=tuple(exclusions),
            )
        )
    return underlier, tuple(contracts)


def _rank(contract: AggregatedContract, underlier: int) -> tuple[Any, ...]:
    return (
        -contract.liquidity_ppb,
        abs(contract.strike_microdollars - underlier),
        contract.strike_microdollars,
        contract.occ_symbol,
    )


def create_portfolio_target(
    target: PortfolioTargetInput,
    parameters: PortfolioParameters,
) -> PortfolioTarget:
    """Create one theoretical long-call portfolio target without placing orders."""

    common = {
        "parameter_manifest_sha256": parameters.sha256,
        "methodology_sha256": target.methodology_sha256,
        "collection_sha256": target.collection_sha256,
    }
    if len(target.incumbent_symbols) > parameters.basket_size:
        return PortfolioTarget(
            status="NO_RECONSTITUTION",
            reason="prior target contains more incumbents than basket_size",
            selected_expiration=None,
            decision_nav_microdollars=None,
            premium_budget_microdollars=None,
            sleeve_microdollars=None,
            positions=(),
            excluded_contracts=(),
            **common,
        )
    try:
        underlier, aggregated = _aggregate(target, parameters)
    except ValueError as exc:
        return PortfolioTarget(
            status="NO_RECONSTITUTION",
            reason=str(exc),
            selected_expiration=None,
            decision_nav_microdollars=None,
            premium_budget_microdollars=None,
            sleeve_microdollars=None,
            positions=(),
            excluded_contracts=(),
            **common,
        )
    if sorted(target.snapshots, key=lambda item: item.sequence)[-1].underlier_halted:
        return PortfolioTarget(
            status="HALTED_INSTRUMENT",
            reason="target-time underlier halt flag is true",
            selected_expiration=None,
            decision_nav_microdollars=None,
            premium_budget_microdollars=None,
            sleeve_microdollars=None,
            positions=(),
            excluded_contracts=aggregated,
            **common,
        )

    base_eligible = [contract for contract in aggregated if not contract.exclusions]
    by_expiration: dict[date, list[AggregatedContract]] = {}
    for contract in base_eligible:
        by_expiration.setdefault(contract.expiration_date, []).append(contract)

    selected_expiration: date | None = None
    applicable: list[AggregatedContract] = []
    incumbent_set = set(target.incumbent_symbols)
    if target.incumbent_expiration is not None:
        retention = [
            contract
            for contract in by_expiration.get(target.incumbent_expiration, [])
            if contract.dte > parameters.roll_dte
        ]
        if len(retention) >= parameters.basket_size:
            selected_expiration = target.incumbent_expiration
            applicable = retention

    if selected_expiration is None:
        candidates: list[tuple[tuple[Any, ...], date, list[AggregatedContract]]] = []
        for expiration, contracts in by_expiration.items():
            successor = [
                contract
                for contract in contracts
                if parameters.successor_minimum_dte
                <= contract.dte
                <= parameters.successor_maximum_dte
            ]
            if len(successor) < parameters.basket_size:
                continue
            ranked = sorted(successor, key=lambda item: _rank(item, underlier))
            top_scores = sorted(
                (item.liquidity_ppb for item in ranked[: parameters.basket_size])
            )
            median_score = _median(top_scores)
            key = (
                abs(ranked[0].dte - parameters.target_dte),
                -median_score,
                expiration.isoformat(),
                expiration.isoformat(),
            )
            candidates.append((key, expiration, successor))
        if not candidates:
            return PortfolioTarget(
                status="NO_ELIGIBLE_SUCCESSOR",
                reason="no expiration contains the required successor-eligible calls",
                selected_expiration=None,
                decision_nav_microdollars=None,
                premium_budget_microdollars=None,
                sleeve_microdollars=None,
                positions=(),
                excluded_contracts=aggregated,
                **common,
            )
        _, selected_expiration, applicable = min(candidates, key=lambda item: item[0])

    ranked = sorted(applicable, key=lambda item: _rank(item, underlier))
    rank_by_symbol = {
        contract.occ_symbol: index + 1 for index, contract in enumerate(ranked)
    }
    retained = [
        contract
        for contract in ranked
        if contract.occ_symbol in incumbent_set
        and rank_by_symbol[contract.occ_symbol] <= parameters.strike_retention_rank
    ]
    selected = retained[: parameters.basket_size]
    selected_symbols = {contract.occ_symbol for contract in selected}
    for contract in ranked:
        if len(selected) == parameters.basket_size:
            break
        if contract.occ_symbol not in selected_symbols:
            selected.append(contract)
            selected_symbols.add(contract.occ_symbol)
    if len(selected) != parameters.basket_size:
        return PortfolioTarget(
            status="NO_RECONSTITUTION",
            reason="fewer than the required number of calls can be selected",
            selected_expiration=selected_expiration,
            decision_nav_microdollars=None,
            premium_budget_microdollars=None,
            sleeve_microdollars=None,
            positions=(),
            excluded_contracts=aggregated,
            **common,
        )
    selected.sort(key=lambda item: item.occ_symbol)

    aggregated_by_symbol = {contract.occ_symbol: contract for contract in aggregated}
    position_by_symbol = {
        position.occ_symbol: position.quantity for position in target.positions
    }
    missing_marks = [
        symbol
        for symbol, quantity in position_by_symbol.items()
        if quantity > 0
        and (
            symbol not in aggregated_by_symbol
            or aggregated_by_symbol[symbol].valid_quote_count
            < parameters.per_contract_valid_quotes
        )
    ]
    if missing_marks:
        return PortfolioTarget(
            status="NO_RECONSTITUTION",
            reason=f"current holding lacks a valid decision mark: {missing_marks[0]}",
            selected_expiration=selected_expiration,
            decision_nav_microdollars=None,
            premium_budget_microdollars=None,
            sleeve_microdollars=None,
            positions=(),
            excluded_contracts=aggregated,
            **common,
        )
    option_value = sum(
        quantity
        * aggregated_by_symbol[symbol].midpoint_microdollars
        * aggregated_by_symbol[symbol].multiplier
        for symbol, quantity in position_by_symbol.items()
        if quantity > 0
    )
    nav = (
        target.settled_cash_microdollars
        + target.unsettled_cash_microdollars
        + option_value
        - target.accrued_liabilities_microdollars
    )
    if nav <= 0:
        return PortfolioTarget(
            status="UNRESOLVED_CAPITAL_POLICY",
            reason="decision-time NAV is not positive",
            selected_expiration=selected_expiration,
            decision_nav_microdollars=nav,
            premium_budget_microdollars=0,
            sleeve_microdollars=0,
            positions=(),
            excluded_contracts=aggregated,
            **common,
        )
    premium_budget = parameters.target_premium_ppb * nav // PPB
    sleeve = premium_budget // parameters.basket_size
    selected_positions = [
        TargetPosition(
            occ_symbol=contract.occ_symbol,
            expiration_date=contract.expiration_date,
            midpoint_microdollars=contract.midpoint_microdollars,
            multiplier=contract.multiplier,
            current_quantity=position_by_symbol.get(contract.occ_symbol, 0),
            target_quantity=target_quantity_from_midpoint(
                sleeve_microdollars=sleeve,
                midpoint_microdollars=contract.midpoint_microdollars,
                multiplier=contract.multiplier,
            ),
            target_delta=(
                target_quantity_from_midpoint(
                    sleeve_microdollars=sleeve,
                    midpoint_microdollars=contract.midpoint_microdollars,
                    multiplier=contract.multiplier,
                )
                - position_by_symbol.get(contract.occ_symbol, 0)
            ),
            liquidity_ppb=contract.liquidity_ppb,
            retained_incumbent=contract.occ_symbol in {
                item.occ_symbol for item in retained
            },
        )
        for contract in selected
    ]
    selected_symbol_set = {contract.occ_symbol for contract in selected}
    exit_positions = [
        TargetPosition(
            occ_symbol=symbol,
            expiration_date=aggregated_by_symbol[symbol].expiration_date,
            midpoint_microdollars=aggregated_by_symbol[symbol].midpoint_microdollars,
            multiplier=aggregated_by_symbol[symbol].multiplier,
            current_quantity=quantity,
            target_quantity=0,
            target_delta=-quantity,
            liquidity_ppb=aggregated_by_symbol[symbol].liquidity_ppb,
            retained_incumbent=False,
        )
        for symbol, quantity in position_by_symbol.items()
        if quantity > 0 and symbol not in selected_symbol_set
    ]
    positions = tuple(
        sorted(selected_positions + exit_positions, key=lambda item: item.occ_symbol)
    )
    status: Literal["TARGET_COMPUTED", "UNRESOLVED_CAPITAL_POLICY"]
    reason: str
    if all(position.target_quantity == 0 for position in positions):
        status = "UNRESOLVED_CAPITAL_POLICY"
        reason = "every selected premium sleeve rounds to zero contracts"
    else:
        status = "TARGET_COMPUTED"
        reason = "reference methodology target computed"
    excluded_output = []
    for contract in aggregated:
        if contract.occ_symbol in selected_symbols:
            continue
        if contract.exclusions:
            excluded_output.append(contract)
        elif contract.expiration_date != selected_expiration:
            excluded_output.append(
                replace(contract, exclusions=("expiration_not_selected",))
            )
        else:
            excluded_output.append(replace(contract, exclusions=("ranked_out",)))
    return PortfolioTarget(
        status=status,
        reason=reason,
        selected_expiration=selected_expiration,
        decision_nav_microdollars=nav,
        premium_budget_microdollars=premium_budget,
        sleeve_microdollars=sleeve,
        positions=positions,
        excluded_contracts=tuple(excluded_output),
        **common,
    )
