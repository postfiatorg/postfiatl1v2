"""Complete-universe Schwab option-chain collection for private proof inputs."""

from __future__ import annotations

import base64
from collections.abc import Callable, Mapping
from datetime import datetime, timezone
from typing import Any, Protocol

from .canonical import canonical_value, sha256_bytes
from .models import CollectionEpoch, PrivateOptionChainSnapshot


class SchwabRequestClient(Protocol):
    def request(
        self,
        method: str,
        path: str,
        *,
        params: Mapping[str, Any] | None = None,
        payload: Mapping[str, Any] | None = None,
    ) -> tuple[Any, Any]: ...


def _flatten_contract_maps(payload: Mapping[str, Any]) -> tuple[dict[str, Any], ...]:
    contracts: list[dict[str, Any]] = []
    for side, map_name in (("CALL", "callExpDateMap"), ("PUT", "putExpDateMap")):
        expiration_map = payload.get(map_name) or {}
        if not isinstance(expiration_map, Mapping):
            raise ValueError(f"{map_name} must be an object")
        for expiration_key in sorted(expiration_map):
            strike_map = expiration_map[expiration_key]
            if not isinstance(strike_map, Mapping):
                raise ValueError(f"{map_name}.{expiration_key} must be an object")
            for strike_key in sorted(strike_map):
                rows = strike_map[strike_key]
                if not isinstance(rows, list):
                    raise ValueError(
                        f"{map_name}.{expiration_key}.{strike_key} must be a list"
                    )
                for row_index, row in enumerate(rows):
                    if not isinstance(row, Mapping):
                        raise ValueError("option contract rows must be objects")
                    contracts.append(canonical_value({
                        "mapSide": side,
                        "expirationKey": str(expiration_key),
                        "strikeKey": str(strike_key),
                        "rowIndex": row_index,
                        "source": dict(row),
                    }))
    return tuple(contracts)


def _response_headers(response: Any) -> dict[str, str]:
    headers = getattr(response, "headers", {}) or {}
    safe_names = {
        "content-type",
        "date",
        "x-request-id",
        "x-correlation-id",
    }
    return {
        str(name).lower(): str(value)
        for name, value in headers.items()
        if str(name).lower() in safe_names
    }


def _response_coverage(
    payload: Mapping[str, Any],
    epoch: CollectionEpoch,
) -> dict[str, Any]:
    missing = [field for field in epoch.required_response_fields if field not in payload]
    if missing:
        raise ValueError(f"Schwab response is missing required field {missing[0]!r}")
    provider_status = payload.get("status")
    if provider_status is not None and not isinstance(provider_status, str):
        raise ValueError("Schwab response status field must be a string when present")
    is_delayed = payload.get("isDelayed")
    if is_delayed is not None and not isinstance(is_delayed, bool):
        raise ValueError("Schwab response isDelayed field must be boolean when present")
    returned_side_maps = [
        side
        for side, field in (("CALL", "callExpDateMap"), ("PUT", "putExpDateMap"))
        if field in payload
    ]
    return canonical_value({
        "schema": "postfiat.yolo.response_coverage.v1",
        "querySha256": epoch.query.sha256,
        "requestedContractType": epoch.query.contract_type,
        "requiredFieldPresence": {
            field: field in payload for field in epoch.required_response_fields
        },
        "topLevelFields": sorted(str(field) for field in payload),
        "returnedSideMaps": returned_side_maps,
        "returnedSymbol": payload.get("symbol"),
        "providerStatus": provider_status,
        "isDelayed": is_delayed,
    })
class SchwabOptionChainCollector:
    """Collect one scheduled snapshot; scheduling is deliberately external."""

    def __init__(
        self,
        client: SchwabRequestClient,
        *,
        clock: Callable[[], datetime] | None = None,
    ) -> None:
        self.client = client
        self.clock = clock or (lambda: datetime.now(timezone.utc))

    def collect(
        self,
        epoch: CollectionEpoch,
        *,
        sequence: int,
        previous_snapshot_sha256: str | None,
    ) -> PrivateOptionChainSnapshot:
        if sequence < 0 or sequence >= len(epoch.scheduled_at_utc):
            raise ValueError("snapshot sequence is outside the epoch schedule")
        payload, response = self.client.request(
            "GET",
            "/marketdata/v1/chains",
            params=epoch.query.to_params(),
        )
        if not isinstance(payload, Mapping):
            raise ValueError("Schwab option-chain response must be an object")
        if not hasattr(response, "status_code"):
            raise ValueError("Schwab response is missing status_code")
        status_code = getattr(response, "status_code")
        if not isinstance(status_code, int) or isinstance(status_code, bool):
            raise ValueError("Schwab response status_code must be an integer")
        if not 200 <= status_code <= 299:
            raise RuntimeError(f"Schwab option-chain request returned HTTP {status_code}")
        headers = _response_headers(response)
        content = getattr(response, "content", None)
        if not isinstance(content, (bytes, bytearray, memoryview)):
            raise ValueError("Schwab response is missing exact raw content bytes")
        raw_bytes = bytes(content)
        normalized_payload = canonical_value(dict(payload))
        returned_symbol = normalized_payload.get("symbol")
        if not isinstance(returned_symbol, str) or returned_symbol.strip().upper() != (
            epoch.underlier
        ):
            raise ValueError("Schwab response symbol does not match the epoch underlier")
        response_coverage = _response_coverage(payload, epoch)
        contracts = _flatten_contract_maps(payload)
        chain_metadata = {
            key: value
            for key, value in normalized_payload.items()
            if key not in {"callExpDateMap", "putExpDateMap"}
        }
        observed_at = self.clock()
        if observed_at.tzinfo is None or observed_at.utcoffset() is None:
            raise ValueError("collector clock must return a timezone-aware timestamp")
        return PrivateOptionChainSnapshot(
            epoch_sha256=epoch.sha256,
            sequence=sequence,
            scheduled_at_utc=epoch.scheduled_at_utc[sequence],
            observed_at_utc=observed_at,
            source_id=epoch.source_id,
            query=epoch.query.to_dict(),
            http_status=status_code,
            response_date=headers.get("date"),
            response_headers=headers,
            raw_response_sha256=sha256_bytes(raw_bytes),
            raw_response_base64=base64.b64encode(raw_bytes).decode("ascii"),
            response_coverage=response_coverage,
            source_payload=normalized_payload,
            chain_metadata=chain_metadata,
            contracts=contracts,
            previous_snapshot_sha256=previous_snapshot_sha256,
        )
