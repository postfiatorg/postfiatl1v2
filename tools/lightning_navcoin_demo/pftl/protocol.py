"""Strict PFTL PREIMAGE-SHA-256 wire helpers for the Lightning demo.

The consensus implementation remains authoritative.  These helpers exist so
the harness can reject non-canonical coordinator/wallet inputs before they are
signed or submitted.
"""

from __future__ import annotations

import hashlib
import re


PREIMAGE_BYTES = 32
PAYMENT_HASH_BYTES = 32
CONDITION_PREFIX = "a0258020"
CONDITION_SUFFIX = "810120"
FULFILLMENT_PREFIX = "a0228020"
CONDITION_HEX_LEN = len(CONDITION_PREFIX) + (PAYMENT_HASH_BYTES * 2) + len(
    CONDITION_SUFFIX
)
FULFILLMENT_HEX_LEN = len(FULFILLMENT_PREFIX) + (PREIMAGE_BYTES * 2)
_LOWER_HEX = re.compile(r"^[0-9a-f]+$")


class ProtocolEncodingError(ValueError):
    """An HTLC condition or fulfillment is not canonical."""


def _require_bytes32(value: bytes, label: str) -> bytes:
    if not isinstance(value, bytes) or len(value) != PREIMAGE_BYTES:
        raise ProtocolEncodingError(f"{label} must be exactly 32 bytes")
    return value


def _require_lower_hex(value: str, expected_len: int, label: str) -> str:
    if not isinstance(value, str):
        raise ProtocolEncodingError(f"{label} must be a lowercase hex string")
    if len(value) != expected_len or not _LOWER_HEX.fullmatch(value):
        raise ProtocolEncodingError(
            f"{label} must be canonical lowercase hex of length {expected_len}"
        )
    return value


def payment_hash(preimage: bytes) -> bytes:
    """Return the Lightning/PFTL SHA-256 payment hash."""

    return hashlib.sha256(_require_bytes32(preimage, "preimage")).digest()


def encode_condition_from_hash(digest: bytes) -> str:
    """Encode a 32-byte hash as canonical PREIMAGE-SHA-256 condition hex."""

    _require_bytes32(digest, "payment hash")
    return f"{CONDITION_PREFIX}{digest.hex()}{CONDITION_SUFFIX}"


def encode_condition(preimage: bytes) -> str:
    """Encode the condition for ``preimage``."""

    return encode_condition_from_hash(payment_hash(preimage))


def encode_fulfillment(preimage: bytes) -> str:
    """Encode a 32-byte preimage as canonical fulfillment hex."""

    _require_bytes32(preimage, "preimage")
    return f"{FULFILLMENT_PREFIX}{preimage.hex()}"


def decode_condition(condition: str) -> bytes:
    """Decode a canonical condition and return its payment hash."""

    value = _require_lower_hex(condition, CONDITION_HEX_LEN, "condition")
    if not value.startswith(CONDITION_PREFIX) or not value.endswith(CONDITION_SUFFIX):
        raise ProtocolEncodingError("condition has the wrong PREIMAGE-SHA-256 profile")
    return bytes.fromhex(value[len(CONDITION_PREFIX) : -len(CONDITION_SUFFIX)])


def decode_fulfillment(fulfillment: str) -> bytes:
    """Decode a canonical fulfillment and return its preimage."""

    value = _require_lower_hex(fulfillment, FULFILLMENT_HEX_LEN, "fulfillment")
    if not value.startswith(FULFILLMENT_PREFIX):
        raise ProtocolEncodingError("fulfillment has the wrong PREIMAGE-SHA-256 profile")
    return bytes.fromhex(value[len(FULFILLMENT_PREFIX) :])


def fulfillment_satisfies(condition: str, fulfillment: str) -> bool:
    """Return whether the strict canonical fulfillment opens the condition."""

    return payment_hash(decode_fulfillment(fulfillment)) == decode_condition(condition)


def canonical_vector(preimage: bytes) -> dict[str, str]:
    """Return a JSON-safe cross-ledger vector."""

    secret = _require_bytes32(preimage, "preimage")
    digest = payment_hash(secret)
    return {
        "preimage_hex": secret.hex(),
        "payment_hash": digest.hex(),
        "condition": encode_condition_from_hash(digest),
        "fulfillment": encode_fulfillment(secret),
    }
