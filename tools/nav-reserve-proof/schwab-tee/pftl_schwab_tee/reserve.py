"""PostFiat-compatible commitments for private YOLO brokerage reserves."""

from __future__ import annotations

import base64
import hashlib
import json
from dataclasses import dataclass
from collections.abc import Callable, Mapping
from datetime import datetime, timezone
from decimal import Decimal
from typing import Any, Protocol

from .canonical import canonical_value, domain_sha256, sha256_bytes


YOLO_BROKER_ADAPTER_KIND_V1 = "yolo-broker-attested-v1"
YOLO_BROKER_DISCLOSURE_SCHEMA_V1 = "postfiat.yolo.broker_reserve_disclosure.v1"
_OPAQUE_COMMITMENT_DOMAIN = b"postfiat.reserve_opaque_commitment.v1"
_QUANTITY_LABEL = "yolo-broker-quantity-v1"
_VALUATION_LABEL = "yolo-broker-valuation-v1"
_DISCLOSURE_LABEL = "yolo-broker-disclosure-v1"
_MAX_U64 = (1 << 64) - 1
PRIVATE_BROKER_API_RESPONSE_SCHEMA = "postfiat.yolo.private_broker_api_response.v1"
PRIVATE_BROKER_RESERVE_SNAPSHOT_SCHEMA = (
    "postfiat.yolo.private_broker_reserve_snapshot.v1"
)


def _append_bytes(output: bytearray, value: bytes) -> None:
    if len(value) > (1 << 32) - 1:
        raise ValueError("canonical field is too long")
    output.extend(len(value).to_bytes(4, "big"))
    output.extend(value)


def _validate_digest(value: str, field: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise ValueError(f"{field} must be exactly 32 bytes of lowercase hex")
    return value


def _validate_identifier(value: str, field: str) -> str:
    if not isinstance(value, str) or not value or len(value.encode("utf-8")) > 256:
        raise ValueError(f"{field} must be bounded canonical lowercase ASCII")
    for index, character in enumerate(value):
        allowed = character.isascii() and (
            character.islower()
            or character.isdigit()
            or (index > 0 and character in "._:-")
        )
        if not allowed:
            raise ValueError(f"{field} must be bounded canonical lowercase ASCII")
    return value


def _validate_u64(value: int, field: str, *, nonzero: bool = False) -> int:
    if (
        not isinstance(value, int)
        or isinstance(value, bool)
        or value < (1 if nonzero else 0)
        or value > _MAX_U64
    ):
        qualifier = "nonzero " if nonzero else ""
        raise ValueError(f"{field} must be a {qualifier}u64")
    return value


def _utc_text(value: datetime) -> str:
    if not isinstance(value, datetime) or value.tzinfo is None or value.utcoffset() is None:
        raise ValueError("broker response timestamps must be timezone-aware")
    return value.astimezone(timezone.utc).isoformat().replace("+00:00", "Z")


def _parse_utc_text(value: Any, field: str) -> datetime:
    if not isinstance(value, str):
        raise ValueError(f"{field} must be a canonical UTC timestamp")
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as exc:
        raise ValueError(f"{field} must be a canonical UTC timestamp") from exc
    if parsed.tzinfo is None or parsed.utcoffset() is None or _utc_text(parsed) != value:
        raise ValueError(f"{field} must be a canonical UTC timestamp")
    return parsed.astimezone(timezone.utc)


def _strict_object(value: Any, *, fields: set[str], name: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"{name} must be an object")
    unknown = set(value) - fields
    missing = fields - set(value)
    if unknown:
        raise ValueError(f"{name} has unknown field {sorted(unknown)[0]!r}")
    if missing:
        raise ValueError(f"{name} is missing field {sorted(missing)[0]!r}")
    return value


def _private_root(label: str, value: Any) -> str:
    return domain_sha256(f"postfiat.yolo.broker_{label}_root.v1", value)


def _opaque_commitment(label: str, payload: bytes) -> str:
    label_bytes = _validate_identifier(label, "commitment label").encode("ascii")
    if not payload or len(payload) > 16 * 1024:
        raise ValueError("opaque commitment input must be in 1..=16384 bytes")
    hasher = hashlib.sha3_384()
    hasher.update(len(_OPAQUE_COMMITMENT_DOMAIN).to_bytes(4, "big"))
    hasher.update(_OPAQUE_COMMITMENT_DOMAIN)
    hasher.update(len(label_bytes).to_bytes(8, "big"))
    hasher.update(label_bytes)
    hasher.update(len(payload).to_bytes(8, "big"))
    hasher.update(payload)
    return hasher.hexdigest()


@dataclass(frozen=True)
class YoloBrokerEvidenceCommitments:
    quantity_evidence_commitment: str
    valuation_evidence_commitment: str
    disclosure_commitment: str


@dataclass(frozen=True)
class PrivateBrokerApiResponse:
    endpoint_id: str
    method: str
    path: str
    params: dict[str, Any]
    requested_at_utc: datetime
    observed_at_utc: datetime
    http_status: int
    response_headers: dict[str, str]
    raw_response_base64: str
    payload: Any

    def __post_init__(self) -> None:
        _validate_identifier(self.endpoint_id, "endpoint_id")
        if self.method != "GET":
            raise ValueError("broker reserve collection permits only GET requests")
        if (
            not isinstance(self.path, str)
            or not self.path.startswith("/")
            or any(character.isspace() for character in self.path)
        ):
            raise ValueError("broker reserve request path must be an absolute API path")
        if not isinstance(self.params, dict) or any(
            not isinstance(key, str) for key in self.params
        ):
            raise ValueError("broker reserve request params must be a string-keyed object")
        canonical_value(self.params)
        _utc_text(self.requested_at_utc)
        _utc_text(self.observed_at_utc)
        if self.observed_at_utc < self.requested_at_utc:
            raise ValueError("broker response predates its request")
        if (
            not isinstance(self.http_status, int)
            or isinstance(self.http_status, bool)
            or not 200 <= self.http_status <= 299
        ):
            raise ValueError("broker reserve response must have a successful HTTP status")
        if not isinstance(self.response_headers, dict) or any(
            not isinstance(key, str) or not isinstance(value, str)
            for key, value in self.response_headers.items()
        ):
            raise ValueError("broker response headers must be a string map")
        sensitive_headers = {"authorization", "cookie", "set-cookie", "proxy-authorization"}
        if any(key.strip().lower() in sensitive_headers for key in self.response_headers):
            raise ValueError("broker response headers contain a prohibited credential field")
        if not isinstance(self.raw_response_base64, str):
            raise ValueError("raw broker response must be canonical base64")
        try:
            raw = base64.b64decode(self.raw_response_base64.encode("ascii"), validate=True)
        except (UnicodeEncodeError, ValueError) as exc:
            raise ValueError("raw broker response must be canonical base64") from exc
        if base64.b64encode(raw).decode("ascii") != self.raw_response_base64:
            raise ValueError("raw broker response must be canonical base64")
        try:
            parsed = json.loads(raw.decode("utf-8"), parse_float=Decimal)
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise ValueError("raw broker response must contain JSON") from exc
        if canonical_value(parsed) != canonical_value(self.payload):
            raise ValueError("broker payload does not match the exact raw response")

    @classmethod
    def from_wire(
        cls,
        *,
        endpoint_id: str,
        method: str,
        path: str,
        params: dict[str, Any],
        requested_at_utc: datetime,
        observed_at_utc: datetime,
        http_status: int,
        response_headers: dict[str, str],
        raw_response: bytes,
        payload: Any,
    ) -> PrivateBrokerApiResponse:
        if not isinstance(raw_response, bytes) or not raw_response:
            raise ValueError("raw broker response must be non-empty bytes")
        return cls(
            endpoint_id=endpoint_id,
            method=method,
            path=path,
            params=canonical_value(params),
            requested_at_utc=requested_at_utc,
            observed_at_utc=observed_at_utc,
            http_status=http_status,
            response_headers=response_headers,
            raw_response_base64=base64.b64encode(raw_response).decode("ascii"),
            payload=canonical_value(payload),
        )

    @property
    def raw_response_sha256(self) -> str:
        return sha256_bytes(
            base64.b64decode(self.raw_response_base64.encode("ascii"), validate=True)
        )

    def to_private_dict(self) -> dict[str, Any]:
        return canonical_value({
            "schema": PRIVATE_BROKER_API_RESPONSE_SCHEMA,
            "endpointId": self.endpoint_id,
            "method": self.method,
            "path": self.path,
            "params": self.params,
            "requestedAtUtc": _utc_text(self.requested_at_utc),
            "observedAtUtc": _utc_text(self.observed_at_utc),
            "httpStatus": self.http_status,
            "responseHeaders": self.response_headers,
            "rawResponseSha256": self.raw_response_sha256,
            "rawResponseBase64": self.raw_response_base64,
            "payload": self.payload,
        })

    @classmethod
    def from_private_dict(cls, value: Any) -> PrivateBrokerApiResponse:
        response = _strict_object(
            value,
            fields={
                "schema",
                "endpointId",
                "method",
                "path",
                "params",
                "requestedAtUtc",
                "observedAtUtc",
                "httpStatus",
                "responseHeaders",
                "rawResponseSha256",
                "rawResponseBase64",
                "payload",
            },
            name="private broker API response",
        )
        if response["schema"] != PRIVATE_BROKER_API_RESPONSE_SCHEMA:
            raise ValueError("private broker API response schema mismatch")
        result = cls(
            endpoint_id=response["endpointId"],
            method=response["method"],
            path=response["path"],
            params=response["params"],
            requested_at_utc=_parse_utc_text(
                response["requestedAtUtc"], "requestedAtUtc"
            ),
            observed_at_utc=_parse_utc_text(
                response["observedAtUtc"], "observedAtUtc"
            ),
            http_status=response["httpStatus"],
            response_headers=response["responseHeaders"],
            raw_response_base64=response["rawResponseBase64"],
            payload=response["payload"],
        )
        if response["rawResponseSha256"] != result.raw_response_sha256:
            raise ValueError("raw broker response hash does not match its bytes")
        return result


@dataclass(frozen=True)
class BrokerReserveReadRequest:
    endpoint_id: str
    path: str
    params: dict[str, Any]

    def __post_init__(self) -> None:
        _validate_identifier(self.endpoint_id, "endpoint_id")
        if (
            not isinstance(self.path, str)
            or not self.path.startswith("/")
            or any(character.isspace() for character in self.path)
        ):
            raise ValueError("broker reserve request path must be an absolute API path")
        if not isinstance(self.params, dict) or any(
            not isinstance(key, str) for key in self.params
        ):
            raise ValueError("broker reserve request params must be a string-keyed object")
        canonical_value(self.params)


class BrokerReserveRequestClient(Protocol):
    def request(
        self,
        method: str,
        path: str,
        *,
        params: Mapping[str, Any] | None = None,
        payload: Mapping[str, Any] | None = None,
    ) -> tuple[Any, Any]: ...


class BrokerReserveResponseCollector:
    """Execute only caller-precommitted, read-only broker requests."""

    def __init__(
        self,
        client: BrokerReserveRequestClient,
        *,
        clock: Callable[[], datetime] | None = None,
    ) -> None:
        self.client = client
        self.clock = clock or (lambda: datetime.now(timezone.utc))

    def collect(
        self,
        requests: tuple[BrokerReserveReadRequest, ...],
    ) -> tuple[PrivateBrokerApiResponse, ...]:
        endpoint_ids = tuple(request.endpoint_id for request in requests)
        if not endpoint_ids or tuple(sorted(set(endpoint_ids))) != endpoint_ids:
            raise ValueError("broker reserve requests must be sorted and unique")
        responses: list[PrivateBrokerApiResponse] = []
        for request in requests:
            requested_at = self.clock()
            payload, response = self.client.request(
                "GET",
                request.path,
                params=request.params,
            )
            observed_at = self.clock()
            status = getattr(response, "status_code", None)
            content = getattr(response, "content", None)
            if not isinstance(content, bytes) or not content:
                raise ValueError("broker reserve response is missing exact raw JSON bytes")
            headers = getattr(response, "headers", {}) or {}
            safe_header_names = {
                "content-type",
                "date",
                "x-request-id",
                "x-correlation-id",
            }
            safe_headers = {
                str(key).lower(): str(value)
                for key, value in headers.items()
                if str(key).lower() in safe_header_names
            }
            responses.append(PrivateBrokerApiResponse.from_wire(
                endpoint_id=request.endpoint_id,
                method="GET",
                path=request.path,
                params=request.params,
                requested_at_utc=requested_at,
                observed_at_utc=observed_at,
                http_status=status,
                response_headers=safe_headers,
                raw_response=content,
                payload=payload,
            ))
        return tuple(responses)


@dataclass(frozen=True)
class PrivateBrokerReserveSnapshot:
    broker_source_id: str
    account_application_identity_sha256: str
    observed_at_utc: datetime
    collection_epoch_sha256: str
    basket_decision_sha256: str
    execution_receipts_root_sha256: str
    required_response_ids: tuple[str, ...]
    responses: tuple[PrivateBrokerApiResponse, ...]
    positions: tuple[dict[str, Any], ...]
    cash: tuple[dict[str, Any], ...]
    liabilities: tuple[dict[str, Any], ...]
    open_orders: tuple[dict[str, Any], ...]
    fills: tuple[dict[str, Any], ...]
    pending_events: tuple[dict[str, Any], ...]
    valuation_inputs: tuple[dict[str, Any], ...]

    def __post_init__(self) -> None:
        _validate_identifier(self.broker_source_id, "broker_source_id")
        _validate_digest(
            self.account_application_identity_sha256,
            "account_application_identity_sha256",
        )
        _utc_text(self.observed_at_utc)
        for field in (
            "collection_epoch_sha256",
            "basket_decision_sha256",
            "execution_receipts_root_sha256",
        ):
            _validate_digest(getattr(self, field), field)
        if not self.required_response_ids:
            raise ValueError("required_response_ids must not be empty")
        for response_id in self.required_response_ids:
            _validate_identifier(response_id, "required_response_id")
        if tuple(sorted(set(self.required_response_ids))) != self.required_response_ids:
            raise ValueError("required_response_ids must be sorted and unique")
        observed_ids = tuple(response.endpoint_id for response in self.responses)
        if observed_ids != self.required_response_ids:
            raise ValueError("broker responses must exactly match required_response_ids")
        for response in self.responses:
            if response.observed_at_utc > self.observed_at_utc:
                raise ValueError("reserve snapshot predates one of its broker responses")
        for field in (
            "positions",
            "cash",
            "liabilities",
            "open_orders",
            "fills",
            "pending_events",
            "valuation_inputs",
        ):
            rows = getattr(self, field)
            if not isinstance(rows, tuple) or any(not isinstance(row, dict) for row in rows):
                raise ValueError(f"{field} must be a tuple of objects")

    def to_private_dict(self) -> dict[str, Any]:
        return canonical_value({
            "schema": PRIVATE_BROKER_RESERVE_SNAPSHOT_SCHEMA,
            "brokerSourceId": self.broker_source_id,
            "accountApplicationIdentitySha256": (
                self.account_application_identity_sha256
            ),
            "observedAtUtc": _utc_text(self.observed_at_utc),
            "collectionEpochSha256": self.collection_epoch_sha256,
            "basketDecisionSha256": self.basket_decision_sha256,
            "executionReceiptsRootSha256": self.execution_receipts_root_sha256,
            "requiredResponseIds": self.required_response_ids,
            "responses": [response.to_private_dict() for response in self.responses],
            "positions": self.positions,
            "cash": self.cash,
            "liabilities": self.liabilities,
            "openOrders": self.open_orders,
            "fills": self.fills,
            "pendingEvents": self.pending_events,
            "valuationInputs": self.valuation_inputs,
        })

    @property
    def sha256(self) -> str:
        return domain_sha256(
            PRIVATE_BROKER_RESERVE_SNAPSHOT_SCHEMA,
            self.to_private_dict(),
        )

    def disclosure(self) -> YoloBrokerReserveDisclosure:
        return YoloBrokerReserveDisclosure(
            broker_source_id=self.broker_source_id,
            account_application_identity_sha256=(
                self.account_application_identity_sha256
            ),
            observed_at_unix_millis=int(
                self.observed_at_utc.astimezone(timezone.utc).timestamp() * 1000
            ),
            collection_epoch_sha256=self.collection_epoch_sha256,
            basket_decision_sha256=self.basket_decision_sha256,
            execution_receipts_root_sha256=self.execution_receipts_root_sha256,
            broker_response_root_sha256=_private_root(
                "responses",
                [response.to_private_dict() for response in self.responses],
            ),
            positions_root_sha256=_private_root("positions", self.positions),
            cash_root_sha256=_private_root("cash", self.cash),
            liabilities_root_sha256=_private_root("liabilities", self.liabilities),
            open_orders_root_sha256=_private_root("open_orders", self.open_orders),
            fills_root_sha256=_private_root("fills", self.fills),
            pending_events_root_sha256=_private_root(
                "pending_events",
                self.pending_events,
            ),
            valuation_inputs_root_sha256=_private_root(
                "valuation_inputs",
                self.valuation_inputs,
            ),
        )

    @classmethod
    def from_private_dict(cls, value: Any) -> PrivateBrokerReserveSnapshot:
        snapshot = _strict_object(
            value,
            fields={
                "schema",
                "brokerSourceId",
                "accountApplicationIdentitySha256",
                "observedAtUtc",
                "collectionEpochSha256",
                "basketDecisionSha256",
                "executionReceiptsRootSha256",
                "requiredResponseIds",
                "responses",
                "positions",
                "cash",
                "liabilities",
                "openOrders",
                "fills",
                "pendingEvents",
                "valuationInputs",
            },
            name="private broker reserve snapshot",
        )
        if snapshot["schema"] != PRIVATE_BROKER_RESERVE_SNAPSHOT_SCHEMA:
            raise ValueError("private broker reserve snapshot schema mismatch")
        array_fields = {
            "requiredResponseIds",
            "responses",
            "positions",
            "cash",
            "liabilities",
            "openOrders",
            "fills",
            "pendingEvents",
            "valuationInputs",
        }
        for field in array_fields:
            if not isinstance(snapshot[field], list):
                raise ValueError(f"{field} must be an array")
        return cls(
            broker_source_id=snapshot["brokerSourceId"],
            account_application_identity_sha256=(
                snapshot["accountApplicationIdentitySha256"]
            ),
            observed_at_utc=_parse_utc_text(
                snapshot["observedAtUtc"], "observedAtUtc"
            ),
            collection_epoch_sha256=snapshot["collectionEpochSha256"],
            basket_decision_sha256=snapshot["basketDecisionSha256"],
            execution_receipts_root_sha256=(
                snapshot["executionReceiptsRootSha256"]
            ),
            required_response_ids=tuple(snapshot["requiredResponseIds"]),
            responses=tuple(
                PrivateBrokerApiResponse.from_private_dict(response)
                for response in snapshot["responses"]
            ),
            positions=tuple(snapshot["positions"]),
            cash=tuple(snapshot["cash"]),
            liabilities=tuple(snapshot["liabilities"]),
            open_orders=tuple(snapshot["openOrders"]),
            fills=tuple(snapshot["fills"]),
            pending_events=tuple(snapshot["pendingEvents"]),
            valuation_inputs=tuple(snapshot["valuationInputs"]),
        )


@dataclass(frozen=True)
class YoloBrokerReserveDisclosure:
    broker_source_id: str
    account_application_identity_sha256: str
    observed_at_unix_millis: int
    collection_epoch_sha256: str
    basket_decision_sha256: str
    execution_receipts_root_sha256: str
    broker_response_root_sha256: str
    positions_root_sha256: str
    cash_root_sha256: str
    liabilities_root_sha256: str
    open_orders_root_sha256: str
    fills_root_sha256: str
    pending_events_root_sha256: str
    valuation_inputs_root_sha256: str

    def __post_init__(self) -> None:
        _validate_identifier(self.broker_source_id, "broker_source_id")
        _validate_u64(
            self.observed_at_unix_millis,
            "observed_at_unix_millis",
            nonzero=True,
        )
        for field in (
            "account_application_identity_sha256",
            "collection_epoch_sha256",
            "basket_decision_sha256",
            "execution_receipts_root_sha256",
            "broker_response_root_sha256",
            "positions_root_sha256",
            "cash_root_sha256",
            "liabilities_root_sha256",
            "open_orders_root_sha256",
            "fills_root_sha256",
            "pending_events_root_sha256",
            "valuation_inputs_root_sha256",
        ):
            _validate_digest(getattr(self, field), field)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": YOLO_BROKER_DISCLOSURE_SCHEMA_V1,
            "broker_source_id": self.broker_source_id,
            "account_application_identity_sha256": (
                self.account_application_identity_sha256
            ),
            "observed_at_unix_millis": self.observed_at_unix_millis,
            "collection_epoch_sha256": self.collection_epoch_sha256,
            "basket_decision_sha256": self.basket_decision_sha256,
            "execution_receipts_root_sha256": self.execution_receipts_root_sha256,
            "broker_response_root_sha256": self.broker_response_root_sha256,
            "positions_root_sha256": self.positions_root_sha256,
            "cash_root_sha256": self.cash_root_sha256,
            "liabilities_root_sha256": self.liabilities_root_sha256,
            "open_orders_root_sha256": self.open_orders_root_sha256,
            "fills_root_sha256": self.fills_root_sha256,
            "pending_events_root_sha256": self.pending_events_root_sha256,
            "valuation_inputs_root_sha256": self.valuation_inputs_root_sha256,
        }

    def canonical_bytes(self) -> bytes:
        output = bytearray()
        _append_bytes(output, YOLO_BROKER_DISCLOSURE_SCHEMA_V1.encode("ascii"))
        _append_bytes(output, self.broker_source_id.encode("ascii"))
        _append_bytes(
            output,
            self.account_application_identity_sha256.encode("ascii"),
        )
        output.extend(self.observed_at_unix_millis.to_bytes(8, "big"))
        for field in (
            "collection_epoch_sha256",
            "basket_decision_sha256",
            "execution_receipts_root_sha256",
            "broker_response_root_sha256",
            "positions_root_sha256",
            "cash_root_sha256",
            "liabilities_root_sha256",
            "open_orders_root_sha256",
            "fills_root_sha256",
            "pending_events_root_sha256",
            "valuation_inputs_root_sha256",
        ):
            _append_bytes(output, getattr(self, field).encode("ascii"))
        return bytes(output)

    def evidence_commitments(
        self,
        *,
        gross_assets: int,
        total_liabilities: int,
        valuation_scale: int,
    ) -> YoloBrokerEvidenceCommitments:
        gross_assets = _validate_u64(gross_assets, "gross_assets")
        total_liabilities = _validate_u64(total_liabilities, "total_liabilities")
        valuation_scale = _validate_u64(
            valuation_scale,
            "valuation_scale",
            nonzero=True,
        )
        if total_liabilities > gross_assets:
            raise ValueError("YOLO broker liabilities exceed gross assets")

        quantity = bytearray()
        _append_bytes(
            quantity,
            self.account_application_identity_sha256.encode("ascii"),
        )
        quantity.extend(self.observed_at_unix_millis.to_bytes(8, "big"))
        for field in (
            "broker_response_root_sha256",
            "positions_root_sha256",
            "cash_root_sha256",
            "liabilities_root_sha256",
            "open_orders_root_sha256",
            "fills_root_sha256",
            "pending_events_root_sha256",
        ):
            _append_bytes(quantity, getattr(self, field).encode("ascii"))
        quantity_commitment = _opaque_commitment(_QUANTITY_LABEL, bytes(quantity))

        valuation = bytearray()
        _append_bytes(valuation, quantity_commitment.encode("ascii"))
        _append_bytes(
            valuation,
            self.valuation_inputs_root_sha256.encode("ascii"),
        )
        valuation.extend(gross_assets.to_bytes(8, "big"))
        valuation.extend(total_liabilities.to_bytes(8, "big"))
        valuation.extend(valuation_scale.to_bytes(8, "big"))
        valuation_commitment = _opaque_commitment(_VALUATION_LABEL, bytes(valuation))

        disclosure = bytearray(self.canonical_bytes())
        _append_bytes(disclosure, quantity_commitment.encode("ascii"))
        _append_bytes(disclosure, valuation_commitment.encode("ascii"))
        disclosure.extend(gross_assets.to_bytes(8, "big"))
        disclosure.extend(total_liabilities.to_bytes(8, "big"))
        disclosure.extend(valuation_scale.to_bytes(8, "big"))
        disclosure_commitment = _opaque_commitment(
            _DISCLOSURE_LABEL,
            bytes(disclosure),
        )
        return YoloBrokerEvidenceCommitments(
            quantity_evidence_commitment=quantity_commitment,
            valuation_evidence_commitment=valuation_commitment,
            disclosure_commitment=disclosure_commitment,
        )
