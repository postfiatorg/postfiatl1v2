"""Canonical Lightning/PFTL hashlock encodings and LND invoice facts.

The PREIMAGE-SHA-256 wire values match the hardened PFTL escrow profile:

    condition   = a0258020 || SHA256(s) || 810120
    fulfillment = a0228020 || s

Only lowercase, fixed-width hexadecimal is accepted.  Permissive decoding is
dangerous here because differently encoded values would bind the same hash.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import hmac
import secrets
from typing import Any, Mapping


SECRET_BYTES = 32
HASH_BYTES = 32
CONDITION_PREFIX = bytes.fromhex("a0258020")
CONDITION_SUFFIX = bytes.fromhex("810120")
FULFILLMENT_PREFIX = bytes.fromhex("a0228020")
CONDITION_BYTES = len(CONDITION_PREFIX) + HASH_BYTES + len(CONDITION_SUFFIX)
FULFILLMENT_BYTES = len(FULFILLMENT_PREFIX) + SECRET_BYTES
MAX_U64 = (1 << 64) - 1
AMP_FEATURE_BITS = frozenset({30, 31})


class ProtocolEncodingError(ValueError):
    """A protocol value is malformed or non-canonical."""


class InvoiceBindingError(ValueError):
    """Decoded invoice facts do not match the signed quote."""


class AmpInvoiceRejected(InvoiceBindingError):
    """AMP cannot bind one invoice to one PFTL preimage condition."""


@dataclass(frozen=True, repr=False)
class SecretPreimage:
    """A 32-byte preimage whose ordinary representation is always redacted."""

    _value: bytes

    def __post_init__(self) -> None:
        if type(self._value) is not bytes or len(self._value) != SECRET_BYTES:
            raise ProtocolEncodingError("preimage must be exactly 32 bytes")

    @classmethod
    def generate(cls) -> "SecretPreimage":
        return cls(secrets.token_bytes(SECRET_BYTES))

    @classmethod
    def from_hex(cls, encoded: str) -> "SecretPreimage":
        return cls(_decode_canonical_hex(encoded, "preimage", SECRET_BYTES))

    def reveal_for_protocol(self) -> bytes:
        """Return the secret for an explicit protocol operation."""

        return self._value

    def protocol_hex(self) -> str:
        """Return secret hex only for a dedicated secret/test-vector artifact."""

        return self._value.hex()

    def __repr__(self) -> str:
        return "SecretPreimage(<redacted>)"

    def __str__(self) -> str:
        return "<redacted>"


def _secret_bytes(secret: SecretPreimage | bytes) -> bytes:
    if isinstance(secret, SecretPreimage):
        return secret.reveal_for_protocol()
    if type(secret) is not bytes or len(secret) != SECRET_BYTES:
        raise ProtocolEncodingError("preimage must be exactly 32 bytes")
    return secret


def _hash_bytes(value: bytes | str) -> bytes:
    if type(value) is bytes:
        if len(value) != HASH_BYTES:
            raise ProtocolEncodingError("payment hash must be exactly 32 bytes")
        return value
    return _decode_canonical_hex(value, "payment hash", HASH_BYTES)


def _decode_canonical_hex(value: Any, name: str, expected_bytes: int) -> bytes:
    if type(value) is not str:
        raise ProtocolEncodingError(f"{name} must be lowercase hexadecimal")
    if len(value) != expected_bytes * 2:
        raise ProtocolEncodingError(
            f"{name} must encode exactly {expected_bytes} bytes"
        )
    if any(character not in "0123456789abcdef" for character in value):
        raise ProtocolEncodingError(f"{name} is not canonical lowercase hexadecimal")
    try:
        return bytes.fromhex(value)
    except ValueError as error:  # Defensive; the alphabet check is authoritative.
        raise ProtocolEncodingError(f"{name} is invalid hexadecimal") from error


def payment_hash(secret: SecretPreimage | bytes) -> bytes:
    """Compute the Lightning/PFTL SHA-256 payment hash."""

    return hashlib.sha256(_secret_bytes(secret)).digest()


def encode_condition(payment_hash_value: bytes | str) -> str:
    digest = _hash_bytes(payment_hash_value)
    return (CONDITION_PREFIX + digest + CONDITION_SUFFIX).hex()


def decode_condition(encoded: str) -> bytes:
    raw = _decode_canonical_hex(encoded, "condition", CONDITION_BYTES)
    if not raw.startswith(CONDITION_PREFIX) or not raw.endswith(CONDITION_SUFFIX):
        raise ProtocolEncodingError("condition is not canonical PREIMAGE-SHA-256")
    digest_start = len(CONDITION_PREFIX)
    digest_end = digest_start + HASH_BYTES
    return raw[digest_start:digest_end]


def encode_fulfillment(secret: SecretPreimage | bytes) -> str:
    return (FULFILLMENT_PREFIX + _secret_bytes(secret)).hex()


def decode_fulfillment(encoded: str) -> SecretPreimage:
    raw = _decode_canonical_hex(encoded, "fulfillment", FULFILLMENT_BYTES)
    if not raw.startswith(FULFILLMENT_PREFIX):
        raise ProtocolEncodingError("fulfillment is not canonical PREIMAGE-SHA-256")
    return SecretPreimage(raw[len(FULFILLMENT_PREFIX) :])


def verify_fulfillment(condition: str, fulfillment: str) -> bool:
    """Return whether a canonical fulfillment satisfies a canonical condition.

    Malformed inputs raise ``ProtocolEncodingError`` rather than being silently
    normalized into a valid value.
    """

    expected_hash = decode_condition(condition)
    secret = decode_fulfillment(fulfillment)
    return hmac.compare_digest(expected_hash, payment_hash(secret))


def _parse_uint(value: Any, field: str) -> int:
    if type(value) is int:
        parsed = value
    elif type(value) is str and value and value.isascii() and value.isdecimal():
        parsed = int(value, 10)
    else:
        raise ProtocolEncodingError(f"{field} must be an unsigned decimal integer")
    if parsed < 0 or parsed > MAX_U64:
        raise ProtocolEncodingError(f"{field} is outside uint64")
    return parsed


def _parse_bool(value: Any, field: str) -> bool:
    if type(value) is bool:
        return value
    if value in (0, "0", "false", "False", ""):
        return False
    if value in (1, "1", "true", "True"):
        return True
    raise ProtocolEncodingError(f"{field} must be boolean")


def _features_indicate_amp(features: Any) -> bool:
    if features is None:
        return False
    if isinstance(features, Mapping):
        entries = features.items()
    elif isinstance(features, (list, tuple)):
        entries = enumerate(features)
    else:
        raise ProtocolEncodingError("LND features must be a map or list")

    for key, value in entries:
        key_text = str(key).lower()
        value_text = str(value).lower()
        try:
            numeric_key = int(key)
        except (TypeError, ValueError):
            numeric_key = -1
        if numeric_key in AMP_FEATURE_BITS or "amp" in key_text or "amp" in value_text:
            return True
    return False


@dataclass(frozen=True)
class LndInvoiceFacts:
    """The subset of ``lncli decodepayreq``/LND DecodePayReq used by the demo."""

    payment_hash: bytes
    amount_msat: int
    payee: str
    timestamp_unix: int
    expiry_seconds: int
    min_final_cltv_delta: int
    network: str
    is_amp: bool

    @property
    def expiry_unix(self) -> int:
        total = self.timestamp_unix + self.expiry_seconds
        if total > MAX_U64:
            raise ProtocolEncodingError("invoice expiry overflows uint64")
        return total

    @classmethod
    def from_decode_pay_req(
        cls,
        response: Mapping[str, Any],
        *,
        network: str,
        is_amp: bool | None = None,
    ) -> "LndInvoiceFacts":
        if not isinstance(response, Mapping):
            raise ProtocolEncodingError("decoded LND invoice must be a mapping")
        response_amp = _parse_bool(response.get("is_amp", False), "is_amp")
        explicit_amp = (
            False if is_amp is None else _parse_bool(is_amp, "is_amp")
        )
        legacy_amp = (
            _parse_bool(response["amp"], "amp") if "amp" in response else False
        )
        amp = response_amp or explicit_amp or legacy_amp or _features_indicate_amp(
            response.get("features")
        )
        payment_hash_value = _hash_bytes(response.get("payment_hash"))
        amount_msat = _parse_uint(response.get("num_msat"), "num_msat")
        timestamp_unix = _parse_uint(response.get("timestamp"), "timestamp")
        expiry_seconds = _parse_uint(response.get("expiry"), "expiry")
        min_final_cltv_delta = _parse_uint(
            response.get("cltv_expiry"), "cltv_expiry"
        )
        payee = response.get("destination")
        if (
            type(payee) is not str
            or len(payee) != 66
            or payee[:2] not in {"02", "03"}
            or any(character not in "0123456789abcdef" for character in payee)
        ):
            raise ProtocolEncodingError(
                "invoice destination must be a compressed secp256k1 public key"
            )
        if network not in {"regtest", "signet", "bitcoin"}:
            raise ProtocolEncodingError("unsupported Lightning network")
        return cls(
            payment_hash=payment_hash_value,
            amount_msat=amount_msat,
            payee=payee,
            timestamp_unix=timestamp_unix,
            expiry_seconds=expiry_seconds,
            min_final_cltv_delta=min_final_cltv_delta,
            network=network,
            is_amp=amp,
        )


def validate_invoice_binding(
    facts: LndInvoiceFacts,
    *,
    expected_payment_hash: bytes | str,
    expected_amount_msat: int,
    expected_payee: str,
    expected_expiry_unix: int,
    expected_min_final_cltv_delta: int,
    expected_network: str,
) -> None:
    """Cross-check decoded LND facts against every invoice-bound quote field."""

    if facts.is_amp:
        raise AmpInvoiceRejected("AMP invoices are unsupported")
    expected_hash = _hash_bytes(expected_payment_hash)
    comparisons = (
        (
            hmac.compare_digest(facts.payment_hash, expected_hash),
            "payment_hash mismatch",
        ),
        (facts.amount_msat == expected_amount_msat, "amount_msat mismatch"),
        (facts.payee == expected_payee, "invoice payee mismatch"),
        (facts.expiry_unix == expected_expiry_unix, "invoice expiry mismatch"),
        (
            facts.min_final_cltv_delta == expected_min_final_cltv_delta,
            "minimum final CLTV mismatch",
        ),
        (facts.network == expected_network, "Lightning network mismatch"),
    )
    for matches, message in comparisons:
        if not matches:
            raise InvoiceBindingError(message)
