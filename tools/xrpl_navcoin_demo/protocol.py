"""Strict PREIMAGE-SHA-256 encodings shared by XRPL and hardened PFTL.

XRPL serializes crypto-condition blobs as uppercase hexadecimal in JSON.
Hardened PFTL deliberately accepts only canonical lowercase hexadecimal.  The
bytes are identical; this module makes the ledger-specific casing explicit and
rejects all normalization at verification boundaries.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import hmac
import secrets
from typing import Any, Mapping


PREIMAGE_BYTES = 32
HASH_BYTES = 32
CONDITION_PREFIX = bytes.fromhex("a0258020")
CONDITION_SUFFIX = bytes.fromhex("810120")
FULFILLMENT_PREFIX = bytes.fromhex("a0228020")
CONDITION_BYTES = len(CONDITION_PREFIX) + HASH_BYTES + len(CONDITION_SUFFIX)
FULFILLMENT_BYTES = len(FULFILLMENT_PREFIX) + PREIMAGE_BYTES


class ProtocolEncodingError(ValueError):
    """A cross-ledger hashlock value is malformed or non-canonical."""


@dataclass(frozen=True, repr=False)
class SecretPreimage:
    """A 32-byte preimage whose normal representation cannot disclose it."""

    _value: bytes

    def __post_init__(self) -> None:
        if type(self._value) is not bytes or len(self._value) != PREIMAGE_BYTES:
            raise ProtocolEncodingError("preimage must be exactly 32 bytes")

    @classmethod
    def generate(cls) -> "SecretPreimage":
        return cls(secrets.token_bytes(PREIMAGE_BYTES))

    @classmethod
    def from_hex(cls, value: str) -> "SecretPreimage":
        return cls(_decode_hex(value, PREIMAGE_BYTES, lowercase=True, label="preimage"))

    def reveal_for_protocol(self) -> bytes:
        return self._value

    def protocol_hex(self) -> str:
        return self._value.hex()

    def __repr__(self) -> str:
        return "SecretPreimage(<redacted>)"

    def __str__(self) -> str:
        return "<redacted>"


def _decode_hex(
    value: Any,
    expected_bytes: int,
    *,
    lowercase: bool,
    label: str,
) -> bytes:
    alphabet = "0123456789abcdef" if lowercase else "0123456789ABCDEF"
    case_name = "lowercase" if lowercase else "uppercase"
    if (
        type(value) is not str
        or len(value) != expected_bytes * 2
        or any(character not in alphabet for character in value)
    ):
        raise ProtocolEncodingError(
            f"{label} must be canonical {case_name} hex encoding "
            f"exactly {expected_bytes} bytes"
        )
    return bytes.fromhex(value)


def payment_hash(preimage: SecretPreimage | bytes) -> bytes:
    raw = (
        preimage.reveal_for_protocol()
        if isinstance(preimage, SecretPreimage)
        else preimage
    )
    if type(raw) is not bytes or len(raw) != PREIMAGE_BYTES:
        raise ProtocolEncodingError("preimage must be exactly 32 bytes")
    return hashlib.sha256(raw).digest()


def _condition_bytes(digest: bytes) -> bytes:
    if type(digest) is not bytes or len(digest) != HASH_BYTES:
        raise ProtocolEncodingError("payment hash must be exactly 32 bytes")
    return CONDITION_PREFIX + digest + CONDITION_SUFFIX


def _fulfillment_bytes(preimage: SecretPreimage | bytes) -> bytes:
    raw = (
        preimage.reveal_for_protocol()
        if isinstance(preimage, SecretPreimage)
        else preimage
    )
    if type(raw) is not bytes or len(raw) != PREIMAGE_BYTES:
        raise ProtocolEncodingError("preimage must be exactly 32 bytes")
    return FULFILLMENT_PREFIX + raw


def pftl_condition(digest: bytes) -> str:
    return _condition_bytes(digest).hex()


def pftl_fulfillment(preimage: SecretPreimage | bytes) -> str:
    return _fulfillment_bytes(preimage).hex()


def xrpl_condition(digest: bytes) -> str:
    return _condition_bytes(digest).hex().upper()


def xrpl_fulfillment(preimage: SecretPreimage | bytes) -> str:
    return _fulfillment_bytes(preimage).hex().upper()


def _decode_condition(value: str, *, xrpl: bool) -> bytes:
    raw = _decode_hex(
        value,
        CONDITION_BYTES,
        lowercase=not xrpl,
        label="XRPL condition" if xrpl else "PFTL condition",
    )
    if not raw.startswith(CONDITION_PREFIX) or not raw.endswith(CONDITION_SUFFIX):
        raise ProtocolEncodingError("condition is not canonical PREIMAGE-SHA-256")
    start = len(CONDITION_PREFIX)
    return raw[start : start + HASH_BYTES]


def _decode_fulfillment(value: str, *, xrpl: bool) -> SecretPreimage:
    raw = _decode_hex(
        value,
        FULFILLMENT_BYTES,
        lowercase=not xrpl,
        label="XRPL fulfillment" if xrpl else "PFTL fulfillment",
    )
    if not raw.startswith(FULFILLMENT_PREFIX):
        raise ProtocolEncodingError("fulfillment is not canonical PREIMAGE-SHA-256")
    return SecretPreimage(raw[len(FULFILLMENT_PREFIX) :])


def verify_pair(condition: str, fulfillment: str, *, xrpl: bool) -> bool:
    digest = _decode_condition(condition, xrpl=xrpl)
    preimage = _decode_fulfillment(fulfillment, xrpl=xrpl)
    return hmac.compare_digest(digest, payment_hash(preimage))


@dataclass(frozen=True, repr=False)
class CrossLedgerHashlock:
    """One preimage and the exact wire encodings used on both ledgers."""

    secret: SecretPreimage

    @classmethod
    def generate(cls) -> "CrossLedgerHashlock":
        return cls(SecretPreimage.generate())

    @classmethod
    def from_secret_hex(cls, value: str) -> "CrossLedgerHashlock":
        return cls(SecretPreimage.from_hex(value))

    @property
    def digest(self) -> bytes:
        return payment_hash(self.secret)

    def public_values(self) -> dict[str, str]:
        return {
            "payment_hash": self.digest.hex(),
            "pftl_condition": pftl_condition(self.digest),
            "xrpl_condition": xrpl_condition(self.digest),
        }

    def pftl_fulfillment(self) -> str:
        return pftl_fulfillment(self.secret)

    def xrpl_fulfillment(self) -> str:
        return xrpl_fulfillment(self.secret)

    def __repr__(self) -> str:
        return (
            "CrossLedgerHashlock("
            f"payment_hash={self.digest.hex()}, secret=<redacted>)"
        )


def extract_xrpl_finish_preimage(
    transaction: Mapping[str, Any],
    *,
    expected_condition: str,
) -> SecretPreimage:
    """Extract and authenticate the public preimage from EscrowFinish JSON."""

    if not isinstance(transaction, Mapping):
        raise ProtocolEncodingError("XRPL transaction must be a mapping")
    if transaction.get("TransactionType") != "EscrowFinish":
        raise ProtocolEncodingError("XRPL transaction is not EscrowFinish")
    condition = transaction.get("Condition")
    fulfillment = transaction.get("Fulfillment")
    if condition != expected_condition:
        raise ProtocolEncodingError("XRPL EscrowFinish condition mismatch")
    if not verify_pair(condition, fulfillment, xrpl=True):
        raise ProtocolEncodingError("XRPL EscrowFinish fulfillment mismatch")
    return _decode_fulfillment(fulfillment, xrpl=True)
