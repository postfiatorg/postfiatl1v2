"""Typed, methodology-neutral records for YOLO option-chain collection."""

from __future__ import annotations

import base64
import binascii
import json
from dataclasses import dataclass
from datetime import date, datetime, timedelta, timezone
from decimal import Decimal
from typing import Any, Literal

from .canonical import canonical_value, domain_sha256, sha256_bytes


QUERY_SCHEMA = "postfiat.yolo.option_chain_query.v1"
EPOCH_SCHEMA = "postfiat.yolo.collection_epoch.v2"
PRIVATE_SNAPSHOT_SCHEMA = "postfiat.yolo.private_option_chain_snapshot.v2"
SNAPSHOT_COMMITMENT_SCHEMA = "postfiat.yolo.snapshot_commitment.v2"
COLLECTION_STATUS_SCHEMA = "postfiat.yolo.collection_status.v1"


def _utc_text(value: datetime) -> str:
    if value.tzinfo is None or value.utcoffset() is None:
        raise ValueError("timestamps must be timezone-aware")
    return value.astimezone(timezone.utc).isoformat().replace("+00:00", "Z")


def _sha256(value: str, field: str) -> str:
    if not isinstance(value, str):
        raise ValueError(f"{field} must be a lowercase SHA-256 hex digest")
    original = str(value)
    normalized = original.lower()
    if len(normalized) != 64 or any(
        character not in "0123456789abcdef" for character in normalized
    ) or normalized != original:
        raise ValueError(f"{field} must be a lowercase SHA-256 hex digest")
    return normalized


def _strict_keys(
    value: Any,
    *,
    required: set[str],
    optional: set[str] | None = None,
    name: str,
) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"{name} must be an object")
    optional = optional or set()
    unknown = set(value) - required - optional
    missing = required - set(value)
    if unknown:
        raise ValueError(f"{name} has unknown field {sorted(unknown)[0]!r}")
    if missing:
        raise ValueError(f"{name} is missing field {sorted(missing)[0]!r}")
    return value


def _parse_utc_text(value: Any, field: str) -> datetime:
    try:
        parsed = datetime.fromisoformat(str(value).replace("Z", "+00:00"))
    except ValueError as exc:
        raise ValueError(f"{field} must be an ISO-8601 timestamp") from exc
    if parsed.tzinfo is None or parsed.utcoffset() is None:
        raise ValueError(f"{field} must include a timezone")
    normalized = parsed.astimezone(timezone.utc)
    if str(value) != _utc_text(normalized):
        raise ValueError(f"{field} must use canonical UTC Z form")
    return normalized


@dataclass(frozen=True)
class OptionChainQuery:
    """Exact Schwab option-chain request parameters, with no policy defaults."""

    symbol: str
    contract_type: Literal["ALL", "CALL", "PUT"]
    strategy: str
    include_underlying_quote: bool
    from_date: date
    to_date: date
    strike_count: int | None

    def __post_init__(self) -> None:
        if (
            not isinstance(self.symbol, str)
            or not isinstance(self.strategy, str)
            or not isinstance(self.contract_type, str)
        ):
            raise ValueError(
                "option-chain symbol, strategy, and contract_type must be strings"
            )
        if not isinstance(self.include_underlying_quote, bool):
            raise ValueError("include_underlying_quote must be boolean")
        if not isinstance(self.from_date, date) or not isinstance(self.to_date, date):
            raise ValueError("option-chain dates must be date values")
        symbol = self.symbol.strip().upper()
        strategy = self.strategy.strip().upper()
        contract_type = self.contract_type.strip().upper()
        if not symbol:
            raise ValueError("option-chain symbol is required")
        if contract_type not in {"ALL", "CALL", "PUT"}:
            raise ValueError("contract_type must be ALL, CALL, or PUT")
        if not strategy:
            raise ValueError("option-chain strategy is required")
        if self.from_date > self.to_date:
            raise ValueError("option-chain date interval is inverted")
        if self.strike_count is not None and (
            not isinstance(self.strike_count, int)
            or isinstance(self.strike_count, bool)
            or self.strike_count <= 0
        ):
            raise ValueError("strike_count must be a positive integer when supplied")
        object.__setattr__(self, "symbol", symbol)
        object.__setattr__(self, "contract_type", contract_type)
        object.__setattr__(self, "strategy", strategy)

    def to_params(self) -> dict[str, str | int]:
        params: dict[str, str | int] = {
            "symbol": self.symbol,
            "contractType": self.contract_type,
            "strategy": self.strategy,
            "includeUnderlyingQuote": (
                "true" if self.include_underlying_quote else "false"
            ),
            "fromDate": self.from_date.isoformat(),
            "toDate": self.to_date.isoformat(),
        }
        if self.strike_count is not None:
            params["strikeCount"] = self.strike_count
        return params

    def to_dict(self) -> dict[str, Any]:
        return {"schema": QUERY_SCHEMA, **self.to_params()}

    @property
    def sha256(self) -> str:
        return domain_sha256(QUERY_SCHEMA, self.to_dict())

    @classmethod
    def from_dict(cls, value: Any) -> OptionChainQuery:
        query = _strict_keys(
            value,
            required={
                "schema",
                "symbol",
                "contractType",
                "strategy",
                "includeUnderlyingQuote",
                "fromDate",
                "toDate",
            },
            optional={"strikeCount"},
            name="option-chain query",
        )
        if query["schema"] != QUERY_SCHEMA:
            raise ValueError("option-chain query schema mismatch")
        include_quote = query["includeUnderlyingQuote"]
        if not isinstance(include_quote, str) or include_quote not in {"true", "false"}:
            raise ValueError("includeUnderlyingQuote must be 'true' or 'false'")
        if not isinstance(query["fromDate"], str) or not isinstance(
            query["toDate"], str
        ):
            raise ValueError("option-chain query dates must be strings")
        try:
            from_date = date.fromisoformat(query["fromDate"])
            to_date = date.fromisoformat(query["toDate"])
        except ValueError as exc:
            raise ValueError("option-chain query dates must use ISO-8601") from exc
        if (
            from_date.isoformat() != query["fromDate"]
            or to_date.isoformat() != query["toDate"]
        ):
            raise ValueError("option-chain query dates must use canonical ISO-8601")
        return cls(
            symbol=query["symbol"],
            contract_type=query["contractType"],
            strategy=query["strategy"],
            include_underlying_quote=include_quote == "true",
            from_date=from_date,
            to_date=to_date,
            strike_count=query.get("strikeCount"),
        )


@dataclass(frozen=True)
class CollectionEpoch:
    """Strict precommitted collection manifest for one option-chain query."""

    epoch_id: str
    methodology_sha256: str
    underlier: str
    source_id: str
    account_application_identity_sha256: str
    collector_code_sha256: str
    prior_epoch_sha256: str | None
    required_response_fields: tuple[str, ...]
    query: OptionChainQuery
    window_start_utc: datetime
    window_end_utc: datetime
    expected_snapshot_count: int
    snapshot_offsets_seconds: tuple[int, ...]

    def __post_init__(self) -> None:
        if not isinstance(self.epoch_id, str) or not self.epoch_id.strip():
            raise ValueError("epoch_id is required")
        if not isinstance(self.source_id, str) or not self.source_id.strip():
            raise ValueError("source_id is required")
        if not isinstance(self.underlier, str):
            raise ValueError("underlier is required")
        underlier = self.underlier.strip().upper()
        if not underlier:
            raise ValueError("underlier is required")
        if underlier != self.query.symbol:
            raise ValueError("underlier must equal the option-chain query symbol")
        object.__setattr__(self, "underlier", underlier)
        object.__setattr__(
            self,
            "methodology_sha256",
            _sha256(self.methodology_sha256, "methodology_sha256"),
        )
        object.__setattr__(
            self,
            "account_application_identity_sha256",
            _sha256(
                self.account_application_identity_sha256,
                "account_application_identity_sha256",
            ),
        )
        object.__setattr__(
            self,
            "collector_code_sha256",
            _sha256(self.collector_code_sha256, "collector_code_sha256"),
        )
        if self.prior_epoch_sha256 is not None:
            object.__setattr__(
                self,
                "prior_epoch_sha256",
                _sha256(self.prior_epoch_sha256, "prior_epoch_sha256"),
            )
        if not self.required_response_fields:
            raise ValueError("required_response_fields must not be empty")
        if tuple(sorted(set(self.required_response_fields))) != (
            self.required_response_fields
        ):
            raise ValueError("required_response_fields must be sorted and unique")
        if any(
            not isinstance(field, str)
            or not field
            or any(character.isspace() for character in field)
            for field in self.required_response_fields
        ):
            raise ValueError("required_response_fields contain an invalid field name")
        _utc_text(self.window_start_utc)
        _utc_text(self.window_end_utc)
        start = self.window_start_utc.astimezone(timezone.utc)
        end = self.window_end_utc.astimezone(timezone.utc)
        if start > end:
            raise ValueError("collection window is inverted")
        object.__setattr__(self, "window_start_utc", start)
        object.__setattr__(self, "window_end_utc", end)
        if (
            not isinstance(self.expected_snapshot_count, int)
            or isinstance(self.expected_snapshot_count, bool)
            or self.expected_snapshot_count <= 0
        ):
            raise ValueError("expected_snapshot_count must be a positive integer")
        if not self.snapshot_offsets_seconds:
            raise ValueError("at least one scheduled snapshot is required")
        if len(self.snapshot_offsets_seconds) != self.expected_snapshot_count:
            raise ValueError("snapshot offsets must match expected_snapshot_count")
        for offset in self.snapshot_offsets_seconds:
            if (
                not isinstance(offset, int)
                or isinstance(offset, bool)
                or offset < 0
            ):
                raise ValueError("snapshot offsets must be non-negative integers")
        if tuple(sorted(self.snapshot_offsets_seconds)) != self.snapshot_offsets_seconds:
            raise ValueError("snapshot offsets must be chronological")
        if len(set(self.snapshot_offsets_seconds)) != len(self.snapshot_offsets_seconds):
            raise ValueError("snapshot schedule must not contain duplicates")
        duration_seconds = int((end - start).total_seconds())
        if self.snapshot_offsets_seconds[-1] > duration_seconds:
            raise ValueError("snapshot offset falls outside the collection window")

    @property
    def scheduled_at_utc(self) -> tuple[datetime, ...]:
        return tuple(
            self.window_start_utc + timedelta(seconds=offset)
            for offset in self.snapshot_offsets_seconds
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": EPOCH_SCHEMA,
            "epochId": self.epoch_id,
            "methodologySha256": self.methodology_sha256,
            "underlier": self.underlier,
            "sourceId": self.source_id,
            "accountApplicationIdentitySha256": (
                self.account_application_identity_sha256
            ),
            "collectorCodeSha256": self.collector_code_sha256,
            "priorEpochSha256": self.prior_epoch_sha256,
            "requiredResponseFields": list(self.required_response_fields),
            "query": self.query.to_dict(),
            "windowStartUtc": _utc_text(self.window_start_utc),
            "windowEndUtc": _utc_text(self.window_end_utc),
            "expectedSnapshotCount": self.expected_snapshot_count,
            "snapshotOffsetsSeconds": list(self.snapshot_offsets_seconds),
            "scheduledAtUtc": [_utc_text(value) for value in self.scheduled_at_utc],
        }

    @property
    def sha256(self) -> str:
        return domain_sha256(EPOCH_SCHEMA, self.to_dict())

    @classmethod
    def from_dict(cls, value: Any) -> CollectionEpoch:
        manifest = _strict_keys(
            value,
            required={
                "schema",
                "epochId",
                "methodologySha256",
                "underlier",
                "sourceId",
                "accountApplicationIdentitySha256",
                "collectorCodeSha256",
                "priorEpochSha256",
                "requiredResponseFields",
                "query",
                "windowStartUtc",
                "windowEndUtc",
                "expectedSnapshotCount",
                "snapshotOffsetsSeconds",
                "scheduledAtUtc",
            },
            name="collection epoch manifest",
        )
        if manifest["schema"] != EPOCH_SCHEMA:
            raise ValueError("collection epoch manifest schema mismatch")
        offsets = manifest["snapshotOffsetsSeconds"]
        if not isinstance(offsets, list):
            raise ValueError("snapshotOffsetsSeconds must be an array")
        scheduled = manifest["scheduledAtUtc"]
        if not isinstance(scheduled, list):
            raise ValueError("scheduledAtUtc must be an array")
        required_response_fields = manifest["requiredResponseFields"]
        if not isinstance(required_response_fields, list):
            raise ValueError("requiredResponseFields must be an array")
        result = cls(
            epoch_id=manifest["epochId"],
            methodology_sha256=manifest["methodologySha256"],
            underlier=manifest["underlier"],
            source_id=manifest["sourceId"],
            account_application_identity_sha256=(
                manifest["accountApplicationIdentitySha256"]
            ),
            collector_code_sha256=manifest["collectorCodeSha256"],
            prior_epoch_sha256=manifest["priorEpochSha256"],
            required_response_fields=tuple(required_response_fields),
            query=OptionChainQuery.from_dict(manifest["query"]),
            window_start_utc=_parse_utc_text(
                manifest["windowStartUtc"], "windowStartUtc"
            ),
            window_end_utc=_parse_utc_text(
                manifest["windowEndUtc"], "windowEndUtc"
            ),
            expected_snapshot_count=manifest["expectedSnapshotCount"],
            snapshot_offsets_seconds=tuple(offsets),
        )
        expected_schedule = [_utc_text(value) for value in result.scheduled_at_utc]
        if scheduled != expected_schedule:
            raise ValueError("scheduledAtUtc does not match the committed offsets")
        return result


@dataclass(frozen=True)
class SnapshotCommitment:
    schema: str
    epoch_sha256: str
    sequence: int
    scheduled_at_utc: str
    observed_at_utc: str
    source_id: str
    http_status: int
    response_date: str | None
    source_payload_sha256: str
    raw_response_sha256: str
    response_coverage_sha256: str
    contracts_sha256: str
    flattened_contract_count: int
    reported_contract_count: int | None
    previous_snapshot_sha256: str | None
    private_snapshot_sha256: str

    def __post_init__(self) -> None:
        if self.schema != SNAPSHOT_COMMITMENT_SCHEMA:
            raise ValueError("snapshot commitment schema mismatch")
        for field in (
            "epoch_sha256",
            "source_payload_sha256",
            "raw_response_sha256",
            "response_coverage_sha256",
            "contracts_sha256",
            "private_snapshot_sha256",
        ):
            _sha256(getattr(self, field), field)
        if self.previous_snapshot_sha256 is not None:
            _sha256(self.previous_snapshot_sha256, "previous_snapshot_sha256")
        if (
            not isinstance(self.sequence, int)
            or isinstance(self.sequence, bool)
            or self.sequence < 0
        ):
            raise ValueError("snapshot sequence must be non-negative")
        _parse_utc_text(self.scheduled_at_utc, "scheduled_at_utc")
        _parse_utc_text(self.observed_at_utc, "observed_at_utc")
        if not isinstance(self.source_id, str) or not self.source_id.strip():
            raise ValueError("snapshot source_id is required")
        if self.response_date is not None and not isinstance(self.response_date, str):
            raise ValueError("response_date must be null or a string")
        if (
            not isinstance(self.http_status, int)
            or isinstance(self.http_status, bool)
            or not 100 <= self.http_status <= 599
        ):
            raise ValueError("snapshot HTTP status must be in 100..599")
        if (
            not isinstance(self.flattened_contract_count, int)
            or isinstance(self.flattened_contract_count, bool)
            or self.flattened_contract_count < 0
        ):
            raise ValueError("flattened contract count must be non-negative")
        if self.reported_contract_count is not None and (
            not isinstance(self.reported_contract_count, int)
            or isinstance(self.reported_contract_count, bool)
            or self.reported_contract_count < 0
        ):
            raise ValueError("reported contract count must be null or non-negative")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "epochSha256": self.epoch_sha256,
            "sequence": self.sequence,
            "scheduledAtUtc": self.scheduled_at_utc,
            "observedAtUtc": self.observed_at_utc,
            "sourceId": self.source_id,
            "httpStatus": self.http_status,
            "responseDate": self.response_date,
            "sourcePayloadSha256": self.source_payload_sha256,
            "rawResponseSha256": self.raw_response_sha256,
            "responseCoverageSha256": self.response_coverage_sha256,
            "contractsSha256": self.contracts_sha256,
            "flattenedContractCount": self.flattened_contract_count,
            "reportedContractCount": self.reported_contract_count,
            "previousSnapshotSha256": self.previous_snapshot_sha256,
            "privateSnapshotSha256": self.private_snapshot_sha256,
        }

    @classmethod
    def from_dict(cls, value: Any) -> SnapshotCommitment:
        commitment = _strict_keys(
            value,
            required={
                "schema",
                "epochSha256",
                "sequence",
                "scheduledAtUtc",
                "observedAtUtc",
                "sourceId",
                "httpStatus",
                "responseDate",
                "sourcePayloadSha256",
                "rawResponseSha256",
                "responseCoverageSha256",
                "contractsSha256",
                "flattenedContractCount",
                "reportedContractCount",
                "previousSnapshotSha256",
                "privateSnapshotSha256",
            },
            name="snapshot commitment",
        )
        return cls(
            schema=commitment["schema"],
            epoch_sha256=commitment["epochSha256"],
            sequence=commitment["sequence"],
            scheduled_at_utc=commitment["scheduledAtUtc"],
            observed_at_utc=commitment["observedAtUtc"],
            source_id=commitment["sourceId"],
            http_status=commitment["httpStatus"],
            response_date=commitment["responseDate"],
            source_payload_sha256=commitment["sourcePayloadSha256"],
            raw_response_sha256=commitment["rawResponseSha256"],
            response_coverage_sha256=commitment["responseCoverageSha256"],
            contracts_sha256=commitment["contractsSha256"],
            flattened_contract_count=commitment["flattenedContractCount"],
            reported_contract_count=commitment["reportedContractCount"],
            previous_snapshot_sha256=commitment["previousSnapshotSha256"],
            private_snapshot_sha256=commitment["privateSnapshotSha256"],
        )

    @property
    def sha256(self) -> str:
        return domain_sha256(SNAPSHOT_COMMITMENT_SCHEMA, self.to_dict())


@dataclass(frozen=True)
class PrivateOptionChainSnapshot:
    epoch_sha256: str
    sequence: int
    scheduled_at_utc: datetime
    observed_at_utc: datetime
    source_id: str
    query: dict[str, Any]
    http_status: int
    response_date: str | None
    response_headers: dict[str, str]
    raw_response_sha256: str
    raw_response_base64: str
    response_coverage: dict[str, Any]
    source_payload: dict[str, Any]
    chain_metadata: dict[str, Any]
    contracts: tuple[dict[str, Any], ...]
    previous_snapshot_sha256: str | None

    def __post_init__(self) -> None:
        _sha256(self.epoch_sha256, "epoch_sha256")
        _sha256(self.raw_response_sha256, "raw_response_sha256")
        if not isinstance(self.raw_response_base64, str):
            raise ValueError("raw_response_base64 must be canonical base64")
        try:
            raw_response = base64.b64decode(
                self.raw_response_base64.encode("ascii"),
                validate=True,
            )
        except (UnicodeEncodeError, binascii.Error) as exc:
            raise ValueError("raw_response_base64 must be canonical base64") from exc
        if base64.b64encode(raw_response).decode("ascii") != self.raw_response_base64:
            raise ValueError("raw_response_base64 must be canonical base64")
        if sha256_bytes(raw_response) != self.raw_response_sha256:
            raise ValueError("raw response bytes do not match raw_response_sha256")
        try:
            raw_payload = json.loads(raw_response.decode("utf-8"), parse_float=Decimal)
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise ValueError("raw response bytes must contain JSON") from exc
        if self.previous_snapshot_sha256 is not None:
            _sha256(self.previous_snapshot_sha256, "previous_snapshot_sha256")
        if (
            not isinstance(self.sequence, int)
            or isinstance(self.sequence, bool)
            or self.sequence < 0
        ):
            raise ValueError("snapshot sequence must be non-negative")
        _utc_text(self.scheduled_at_utc)
        _utc_text(self.observed_at_utc)
        if not isinstance(self.source_id, str) or not self.source_id.strip():
            raise ValueError("snapshot source_id is required")
        if self.response_date is not None and not isinstance(self.response_date, str):
            raise ValueError("response_date must be null or a string")
        if (
            not isinstance(self.http_status, int)
            or isinstance(self.http_status, bool)
            or not 100 <= self.http_status <= 599
        ):
            raise ValueError("snapshot HTTP status must be in 100..599")
        if not isinstance(self.query, dict):
            raise TypeError("snapshot query must be an object")
        normalized_query = OptionChainQuery.from_dict(self.query).to_dict()
        if normalized_query != self.query:
            raise ValueError("snapshot query must use canonical values")
        if not isinstance(self.source_payload, dict):
            raise TypeError("snapshot source payload must be an object")
        if canonical_value(raw_payload) != canonical_value(self.source_payload):
            raise ValueError("snapshot source payload does not match raw response bytes")
        if not isinstance(self.response_coverage, dict):
            raise TypeError("snapshot response coverage must be an object")
        coverage = _strict_keys(
            self.response_coverage,
            required={
                "schema",
                "querySha256",
                "requestedContractType",
                "requiredFieldPresence",
                "topLevelFields",
                "returnedSideMaps",
                "returnedSymbol",
                "providerStatus",
                "isDelayed",
            },
            name="snapshot response coverage",
        )
        if coverage["schema"] != "postfiat.yolo.response_coverage.v1":
            raise ValueError("snapshot response coverage schema mismatch")
        if coverage["querySha256"] != OptionChainQuery.from_dict(self.query).sha256:
            raise ValueError("snapshot response coverage query hash does not match")
        if coverage["requestedContractType"] != self.query["contractType"]:
            raise ValueError("snapshot response coverage contract type does not match")
        presence = coverage["requiredFieldPresence"]
        if not isinstance(presence, dict) or any(
            not isinstance(field, str)
            or not isinstance(is_present, bool)
            or not is_present
            or field not in self.source_payload
            for field, is_present in presence.items()
        ):
            raise ValueError("snapshot response coverage required fields do not match")
        expected_top_level = sorted(self.source_payload)
        if coverage["topLevelFields"] != expected_top_level:
            raise ValueError("snapshot response coverage top-level fields do not match")
        expected_sides = [
            side
            for side, field in (("CALL", "callExpDateMap"), ("PUT", "putExpDateMap"))
            if field in self.source_payload
        ]
        if coverage["returnedSideMaps"] != expected_sides:
            raise ValueError("snapshot response coverage side maps do not match")
        if coverage["returnedSymbol"] != self.source_payload.get("symbol"):
            raise ValueError("snapshot response coverage symbol does not match")
        if coverage["providerStatus"] != self.source_payload.get("status"):
            raise ValueError("snapshot response coverage provider status does not match")
        if coverage["isDelayed"] != self.source_payload.get("isDelayed"):
            raise ValueError("snapshot response coverage delayed flag does not match")
        if not isinstance(self.chain_metadata, dict):
            raise TypeError("snapshot chain metadata must be an object")
        expected_metadata = {
            key: value
            for key, value in self.source_payload.items()
            if key not in {"callExpDateMap", "putExpDateMap"}
        }
        if canonical_value(self.chain_metadata) != canonical_value(expected_metadata):
            raise ValueError("snapshot chain metadata does not match source payload")
        if not isinstance(self.response_headers, dict) or any(
            not isinstance(key, str) or not isinstance(value, str)
            for key, value in self.response_headers.items()
        ):
            raise TypeError("snapshot response headers must be a string map")
        if not isinstance(self.contracts, tuple) or any(
            not isinstance(contract, dict) for contract in self.contracts
        ):
            raise TypeError("snapshot contracts must be a tuple of objects")

    def to_private_dict(self) -> dict[str, Any]:
        return canonical_value({
            "schema": PRIVATE_SNAPSHOT_SCHEMA,
            "epochSha256": self.epoch_sha256,
            "sequence": self.sequence,
            "scheduledAtUtc": _utc_text(self.scheduled_at_utc),
            "observedAtUtc": _utc_text(self.observed_at_utc),
            "sourceId": self.source_id,
            "query": self.query,
            "httpStatus": self.http_status,
            "responseDate": self.response_date,
            "responseHeaders": self.response_headers,
            "rawResponseSha256": self.raw_response_sha256,
            "rawResponseBase64": self.raw_response_base64,
            "responseCoverage": self.response_coverage,
            "sourcePayload": self.source_payload,
            "chainMetadata": self.chain_metadata,
            "contracts": self.contracts,
            "previousSnapshotSha256": self.previous_snapshot_sha256,
        })

    @property
    def sha256(self) -> str:
        return domain_sha256(PRIVATE_SNAPSHOT_SCHEMA, self.to_private_dict())

    def commitment(self) -> SnapshotCommitment:
        reported = self.chain_metadata.get("numberOfContracts")
        if reported is not None and (
            not isinstance(reported, int)
            or isinstance(reported, bool)
            or reported < 0
        ):
            raise ValueError("numberOfContracts must be null or a non-negative integer")
        reported_count = reported
        return SnapshotCommitment(
            schema=SNAPSHOT_COMMITMENT_SCHEMA,
            epoch_sha256=self.epoch_sha256,
            sequence=self.sequence,
            scheduled_at_utc=_utc_text(self.scheduled_at_utc),
            observed_at_utc=_utc_text(self.observed_at_utc),
            source_id=self.source_id,
            http_status=self.http_status,
            response_date=self.response_date,
            source_payload_sha256=domain_sha256(
                "postfiat.yolo.schwab_source_payload.v1", self.source_payload
            ),
            raw_response_sha256=self.raw_response_sha256,
            response_coverage_sha256=domain_sha256(
                "postfiat.yolo.response_coverage.v1",
                self.response_coverage,
            ),
            contracts_sha256=domain_sha256(
                "postfiat.yolo.option_contracts.v1", self.contracts
            ),
            flattened_contract_count=len(self.contracts),
            reported_contract_count=reported_count,
            previous_snapshot_sha256=self.previous_snapshot_sha256,
            private_snapshot_sha256=self.sha256,
        )

    @property
    def raw_response_bytes(self) -> bytes:
        return base64.b64decode(self.raw_response_base64.encode("ascii"), validate=True)

    @classmethod
    def from_private_dict(cls, value: Any) -> PrivateOptionChainSnapshot:
        snapshot = _strict_keys(
            value,
            required={
                "schema",
                "epochSha256",
                "sequence",
                "scheduledAtUtc",
                "observedAtUtc",
                "sourceId",
                "query",
                "httpStatus",
                "responseDate",
                "responseHeaders",
                "rawResponseSha256",
                "rawResponseBase64",
                "responseCoverage",
                "sourcePayload",
                "chainMetadata",
                "contracts",
                "previousSnapshotSha256",
            },
            name="private option-chain snapshot",
        )
        if snapshot["schema"] != PRIVATE_SNAPSHOT_SCHEMA:
            raise ValueError("private option-chain snapshot schema mismatch")
        contracts = snapshot["contracts"]
        if not isinstance(contracts, list):
            raise ValueError("private snapshot contracts must be an array")
        query = OptionChainQuery.from_dict(snapshot["query"]).to_dict()
        return cls(
            epoch_sha256=snapshot["epochSha256"],
            sequence=snapshot["sequence"],
            scheduled_at_utc=_parse_utc_text(
                snapshot["scheduledAtUtc"], "scheduledAtUtc"
            ),
            observed_at_utc=_parse_utc_text(
                snapshot["observedAtUtc"], "observedAtUtc"
            ),
            source_id=snapshot["sourceId"],
            query=query,
            http_status=snapshot["httpStatus"],
            response_date=snapshot["responseDate"],
            response_headers=snapshot["responseHeaders"],
            raw_response_sha256=snapshot["rawResponseSha256"],
            raw_response_base64=snapshot["rawResponseBase64"],
            response_coverage=snapshot["responseCoverage"],
            source_payload=snapshot["sourcePayload"],
            chain_metadata=snapshot["chainMetadata"],
            contracts=tuple(contracts),
            previous_snapshot_sha256=snapshot["previousSnapshotSha256"],
        )


@dataclass(frozen=True)
class CollectionStatus:
    schema: str
    epoch_sha256: str
    state: Literal["EMPTY", "INCOMPLETE", "COMPLETE", "INVALID"]
    expected_sequences: tuple[int, ...]
    observed_sequences: tuple[int, ...]
    issues: tuple[str, ...]
    commitments_sha256: str | None

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "epochSha256": self.epoch_sha256,
            "state": self.state,
            "expectedSequences": self.expected_sequences,
            "observedSequences": self.observed_sequences,
            "issues": self.issues,
            "commitmentsSha256": self.commitments_sha256,
        }


def assess_collection(
    epoch: CollectionEpoch,
    commitments: tuple[SnapshotCommitment, ...],
) -> CollectionStatus:
    """Report completeness without deciding how an incomplete epoch is handled."""

    expected = tuple(range(len(epoch.scheduled_at_utc)))
    ordered = tuple(sorted(commitments, key=lambda value: value.sequence))
    observed = tuple(value.sequence for value in ordered)
    issues: list[str] = []
    if len(set(observed)) != len(observed):
        issues.append("duplicate_sequence")
    for sequence in sorted(set(observed) - set(expected)):
        issues.append(f"unexpected_sequence:{sequence}")
    if any(value.epoch_sha256 != epoch.sha256 for value in ordered):
        issues.append("epoch_mismatch")
    if any(value.source_id != epoch.source_id for value in ordered):
        issues.append("source_mismatch")
    for value in ordered:
        if value.sequence in expected:
            scheduled = _utc_text(epoch.scheduled_at_utc[value.sequence])
            if value.scheduled_at_utc != scheduled:
                issues.append(f"scheduled_time_mismatch:{value.sequence}")
        if not 200 <= value.http_status <= 299:
            issues.append(f"http_status:{value.sequence}:{value.http_status}")
        if (
            value.reported_contract_count is not None
            and value.reported_contract_count != value.flattened_contract_count
        ):
            issues.append(f"contract_count_mismatch:{value.sequence}")
    by_sequence = {
        value.sequence: value
        for value in ordered
        if observed.count(value.sequence) == 1
    }
    for value in ordered:
        if value.sequence == 0:
            expected_previous = None
        elif value.sequence - 1 in by_sequence:
            expected_previous = by_sequence[value.sequence - 1].sha256
        else:
            continue
        if value.previous_snapshot_sha256 != expected_previous:
            issues.append(f"broken_previous_commitment:{value.sequence}")
    if issues:
        state: Literal["EMPTY", "INCOMPLETE", "COMPLETE", "INVALID"] = "INVALID"
    elif not ordered:
        state = "EMPTY"
    elif observed == expected:
        state = "COMPLETE"
    else:
        state = "INCOMPLETE"
    aggregate = (
        domain_sha256(
            "postfiat.yolo.collection_commitments.v1",
            [value.to_dict() for value in ordered],
        )
        if ordered
        else None
    )
    return CollectionStatus(
        schema=COLLECTION_STATUS_SCHEMA,
        epoch_sha256=epoch.sha256,
        state=state,
        expected_sequences=expected,
        observed_sequences=observed,
        issues=tuple(issues),
        commitments_sha256=aggregate,
    )
