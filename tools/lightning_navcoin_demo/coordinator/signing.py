"""Ed25519 signatures over canonical submarine-swap quotes.

The implementation isolates the third-party primitive behind ``QuoteSigner``.
The current synthetic demo uses ``cryptography``'s Ed25519 implementation; it
does not contain a home-grown or symmetric "test signature" fallback.
"""

from __future__ import annotations

import base64
import hashlib
import hmac
import json
from typing import Any, Mapping, Protocol, runtime_checkable

from .quote import (
    MAX_QUOTE_BYTES,
    QuoteValidationError,
    canonical_quote_bytes,
    validate_quote,
)


SIGNATURE_DOMAIN = b"postfiat.lightning_submarine_quote.v1\x00"
SIGNATURE_ALGORITHM = "Ed25519"
ENVELOPE_FIELDS = frozenset(
    {"algorithm", "key_id", "public_key", "quote", "signature"}
)
MAX_ENVELOPE_BYTES = MAX_QUOTE_BYTES + 4096


class QuoteSignatureError(ValueError):
    """A quote envelope or signature is invalid."""


@runtime_checkable
class QuoteSigner(Protocol):
    """Injected public-key signer used by ``sign_quote``."""

    @property
    def algorithm(self) -> str: ...

    def public_key_bytes(self) -> bytes: ...

    def sign(self, message: bytes) -> bytes: ...


def _signature_message(canonical_quote: bytes) -> bytes:
    return (
        SIGNATURE_DOMAIN
        + len(canonical_quote).to_bytes(4, byteorder="big", signed=False)
        + canonical_quote
    )


def _b64url_encode(value: bytes) -> str:
    return base64.urlsafe_b64encode(value).rstrip(b"=").decode("ascii")


def _b64url_decode(value: Any, field: str, expected_length: int) -> bytes:
    if type(value) is not str or not value or "=" in value:
        raise QuoteSignatureError(f"{field} is not canonical unpadded base64url")
    if any(character not in "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_" for character in value):
        raise QuoteSignatureError(f"{field} is not canonical unpadded base64url")
    padding = "=" * ((4 - len(value) % 4) % 4)
    try:
        decoded = base64.b64decode(
            value + padding, altchars=b"-_", validate=True
        )
    except (ValueError, base64.binascii.Error) as error:
        raise QuoteSignatureError(f"{field} is invalid base64url") from error
    if len(decoded) != expected_length or _b64url_encode(decoded) != value:
        raise QuoteSignatureError(f"{field} has invalid length or encoding")
    return decoded


def _key_id(public_key: bytes) -> str:
    return hashlib.sha256(
        b"postfiat.lightning_submarine_quote.key.v1\x00" + public_key
    ).hexdigest()


class Ed25519Signer:
    """Thin adapter around ``cryptography`` Ed25519 private keys."""

    def __init__(self, private_key: Any) -> None:
        self._private_key = private_key

    @classmethod
    def generate(cls) -> "Ed25519Signer":
        try:
            from cryptography.hazmat.primitives.asymmetric.ed25519 import (
                Ed25519PrivateKey,
            )
        except ImportError as error:
            raise RuntimeError(
                "cryptography with Ed25519 support is required"
            ) from error
        return cls(Ed25519PrivateKey.generate())

    @classmethod
    def from_private_bytes(cls, seed: bytes) -> "Ed25519Signer":
        if type(seed) is not bytes or len(seed) != 32:
            raise QuoteSignatureError("Ed25519 private seed must be 32 bytes")
        try:
            from cryptography.hazmat.primitives.asymmetric.ed25519 import (
                Ed25519PrivateKey,
            )
        except ImportError as error:
            raise RuntimeError(
                "cryptography with Ed25519 support is required"
            ) from error
        return cls(Ed25519PrivateKey.from_private_bytes(seed))

    @property
    def algorithm(self) -> str:
        return SIGNATURE_ALGORITHM

    def public_key_bytes(self) -> bytes:
        from cryptography.hazmat.primitives import serialization

        return self._private_key.public_key().public_bytes(
            encoding=serialization.Encoding.Raw,
            format=serialization.PublicFormat.Raw,
        )

    def sign(self, message: bytes) -> bytes:
        return self._private_key.sign(message)


def sign_quote(quote: Mapping[str, Any], signer: QuoteSigner) -> dict[str, Any]:
    if not isinstance(signer, QuoteSigner):
        raise QuoteSignatureError("signer does not implement QuoteSigner")
    if signer.algorithm != SIGNATURE_ALGORITHM:
        raise QuoteSignatureError("unsupported quote signature algorithm")
    validated = validate_quote(quote)
    canonical = canonical_quote_bytes(validated)
    public_key = signer.public_key_bytes()
    if type(public_key) is not bytes or len(public_key) != 32:
        raise QuoteSignatureError("Ed25519 public key must be 32 bytes")
    signature = signer.sign(_signature_message(canonical))
    if type(signature) is not bytes or len(signature) != 64:
        raise QuoteSignatureError("Ed25519 signature must be 64 bytes")
    return {
        "algorithm": SIGNATURE_ALGORITHM,
        "key_id": _key_id(public_key),
        "public_key": _b64url_encode(public_key),
        "quote": validated,
        "signature": _b64url_encode(signature),
    }


def verify_signed_quote(
    envelope: Mapping[str, Any],
    *,
    expected_public_key: bytes | None = None,
) -> dict[str, Any]:
    if not isinstance(envelope, Mapping):
        raise QuoteSignatureError("signed quote must be a mapping")
    if frozenset(envelope.keys()) != ENVELOPE_FIELDS:
        raise QuoteSignatureError("signed quote envelope field set mismatch")
    if envelope.get("algorithm") != SIGNATURE_ALGORITHM:
        raise QuoteSignatureError("unsupported quote signature algorithm")
    public_key = _b64url_decode(envelope.get("public_key"), "public_key", 32)
    signature = _b64url_decode(envelope.get("signature"), "signature", 64)
    if envelope.get("key_id") != _key_id(public_key):
        raise QuoteSignatureError("quote signer key_id mismatch")
    if expected_public_key is not None:
        if type(expected_public_key) is not bytes or len(expected_public_key) != 32:
            raise QuoteSignatureError("expected public key must be 32 bytes")
        if not hmac.compare_digest(public_key, expected_public_key):
            raise QuoteSignatureError("quote signer is not the expected public key")
    try:
        canonical = canonical_quote_bytes(envelope.get("quote"))
    except QuoteValidationError as error:
        raise QuoteSignatureError(str(error)) from error
    try:
        from cryptography.exceptions import InvalidSignature
        from cryptography.hazmat.primitives.asymmetric.ed25519 import (
            Ed25519PublicKey,
        )

        Ed25519PublicKey.from_public_bytes(public_key).verify(
            signature, _signature_message(canonical)
        )
    except ImportError as error:
        raise RuntimeError("cryptography with Ed25519 support is required") from error
    except InvalidSignature as error:
        raise QuoteSignatureError("quote signature verification failed") from error
    return validate_quote(envelope["quote"])


def encode_signed_quote(envelope: Mapping[str, Any]) -> bytes:
    # Verification also validates all quote fields and the public key signature.
    verify_signed_quote(envelope)
    encoded = json.dumps(
        envelope,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
        allow_nan=False,
    ).encode("ascii")
    if len(encoded) > MAX_ENVELOPE_BYTES:
        raise QuoteSignatureError("signed quote exceeds size limit")
    return encoded


def _reject_duplicate_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise QuoteSignatureError(f"duplicate JSON field: {key}")
        result[key] = value
    return result


def parse_signed_quote(
    encoded: bytes,
    *,
    expected_public_key: bytes | None = None,
) -> dict[str, Any]:
    if type(encoded) is not bytes or not encoded or len(encoded) > MAX_ENVELOPE_BYTES:
        raise QuoteSignatureError("signed quote bytes are empty or oversized")
    try:
        value = json.loads(
            encoded.decode("ascii"), object_pairs_hook=_reject_duplicate_object
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise QuoteSignatureError("signed quote is invalid JSON") from error
    verify_signed_quote(value, expected_public_key=expected_public_key)
    if encode_signed_quote(value) != encoded:
        raise QuoteSignatureError("signed quote JSON is not canonical")
    return dict(value)
