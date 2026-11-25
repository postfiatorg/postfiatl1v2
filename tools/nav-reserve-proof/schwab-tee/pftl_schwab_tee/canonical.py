"""Canonical JSON and domain-separated commitments for private YOLO inputs."""

from __future__ import annotations

import hashlib
import json
import math
from collections.abc import Mapping, Sequence
from decimal import Decimal, InvalidOperation
from typing import Any


def _decimal_text(value: Decimal) -> str:
    if not value.is_finite():
        raise ValueError("non-finite numbers are not canonical")
    if value == 0:
        return "0"
    normalized = value.normalize()
    rendered = format(normalized, "f")
    if "." in rendered:
        rendered = rendered.rstrip("0").rstrip(".")
    return rendered


def canonical_value(value: Any) -> Any:
    """Return JSON-safe values with stable cross-runtime numeric semantics.

    Source integers remain integers. Decimal and floating-point values become
    normalized decimal strings so JSON encoders in different languages do not
    disagree about binary-float formatting.
    """

    if value is None or isinstance(value, (bool, str)):
        return value
    if isinstance(value, int):
        return value
    if isinstance(value, Decimal):
        return _decimal_text(value)
    if isinstance(value, float):
        if not math.isfinite(value):
            raise ValueError("non-finite numbers are not canonical")
        try:
            return _decimal_text(Decimal(str(value)))
        except InvalidOperation as exc:
            raise ValueError("invalid decimal value") from exc
    if isinstance(value, Mapping):
        output: dict[str, Any] = {}
        for key, item in value.items():
            if not isinstance(key, str):
                raise TypeError("canonical JSON object keys must be strings")
            output[key] = canonical_value(item)
        return output
    if isinstance(value, Sequence) and not isinstance(
        value, (bytes, bytearray, memoryview, str)
    ):
        return [canonical_value(item) for item in value]
    raise TypeError(f"unsupported canonical JSON value: {type(value).__name__}")


def canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(
        canonical_value(value),
        ensure_ascii=False,
        allow_nan=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def domain_sha256(domain: str, value: Any) -> str:
    if not domain or "\x00" in domain:
        raise ValueError("commitment domain must be non-empty and contain no NUL")
    return sha256_bytes(domain.encode("utf-8") + b"\x00" + canonical_json_bytes(value))
