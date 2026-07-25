"""Single-use, public-key-authorized real-value permits.

The coordinator has no API that can manufacture one of these permits.  An
operator signs the exact quote, cost ceiling, and expiry out of band.  The
runtime verifies the pinned public key and consumes the authorization in the
durable budget journal before initiating a value-moving side effect.
"""

from __future__ import annotations

import base64
from dataclasses import dataclass
import hashlib
import hmac
import json
import re
import time
from typing import Any, Mapping, Protocol

from .policy import MainnetQuoteView, RealValuePolicy, RealValuePolicyError


AUTHORIZATION_SCHEMA = "postfiat.lightning_value_authorization.v1"
AUTHORIZATION_DOMAIN = b"postfiat.lightning_value_authorization.v1\x00"
AUTHORIZATION_ALGORITHM = "Ed25519"
AUTHORIZATION_FIELDS = frozenset(
    {
        "schema",
        "authorization_id",
        "policy_id",
        "category",
        "quote_sha256",
        "swap_id",
        "direction",
        "principal_msat",
        "max_fee_msat",
        "max_all_in_usd_e8",
        "expires_unix",
        "authorized_by",
    }
)
ENVELOPE_FIELDS = frozenset({"algorithm", "public_key", "authorization", "signature"})
HEX_32 = re.compile(r"^[0-9a-f]{64}$")


class ValueAuthorizationError(RealValuePolicyError):
    """A value-moving action lacks an exact valid operator authorization."""


class AuthorizationSigner(Protocol):
    def public_key_bytes(self) -> bytes: ...

    def sign(self, message: bytes) -> bytes: ...


def _b64url_encode(value: bytes) -> str:
    return base64.urlsafe_b64encode(value).rstrip(b"=").decode("ascii")


def _b64url_decode(value: Any, name: str, length: int) -> bytes:
    if type(value) is not str or not value or "=" in value:
        raise ValueAuthorizationError(f"{name} is not canonical base64url")
    if any(
        character
        not in "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
        for character in value
    ):
        raise ValueAuthorizationError(f"{name} is not canonical base64url")
    padding = "=" * ((4 - len(value) % 4) % 4)
    try:
        decoded = base64.b64decode(value + padding, altchars=b"-_", validate=True)
    except (ValueError, base64.binascii.Error) as error:
        raise ValueAuthorizationError(f"{name} is invalid base64url") from error
    if len(decoded) != length or _b64url_encode(decoded) != value:
        raise ValueAuthorizationError(f"{name} has invalid length or encoding")
    return decoded


def _canonical_authorization(value: Mapping[str, Any]) -> bytes:
    if not isinstance(value, Mapping) or frozenset(value.keys()) != AUTHORIZATION_FIELDS:
        raise ValueAuthorizationError("authorization field set mismatch")
    if value["schema"] != AUTHORIZATION_SCHEMA:
        raise ValueAuthorizationError("unsupported authorization schema")
    for field in ("authorization_id", "policy_id", "quote_sha256", "swap_id"):
        if type(value[field]) is not str or HEX_32.fullmatch(value[field]) is None:
            raise ValueAuthorizationError(f"{field} must be lowercase 32-byte hex")
    if value["category"] not in {"SWAP", "LIQUIDITY_SETUP"}:
        raise ValueAuthorizationError("unsupported authorization category")
    if value["direction"] not in {
        "lightning_to_pftl",
        "pftl_to_lightning",
        "not_applicable",
    }:
        raise ValueAuthorizationError("unsupported authorization direction")
    if value["category"] == "SWAP" and value["direction"] == "not_applicable":
        raise ValueAuthorizationError("swap authorization requires a direction")
    if value["category"] == "LIQUIDITY_SETUP" and value["direction"] != "not_applicable":
        raise ValueAuthorizationError("liquidity authorization direction must be not_applicable")
    for field in (
        "principal_msat",
        "max_fee_msat",
        "max_all_in_usd_e8",
        "expires_unix",
    ):
        item = value[field]
        if type(item) is not int or item < 0 or item > (1 << 63) - 1:
            raise ValueAuthorizationError(f"{field} must be uint63")
    if value["principal_msat"] + value["max_fee_msat"] <= 0:
        raise ValueAuthorizationError("authorization cost must be positive")
    if value["max_all_in_usd_e8"] <= 0 or value["expires_unix"] <= 0:
        raise ValueAuthorizationError("authorization cap and expiry must be positive")
    if value["authorized_by"] != "nazgul":
        raise ValueAuthorizationError("authorization must be issued by nazgul")
    try:
        encoded = json.dumps(
            dict(value),
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        ).encode("ascii")
    except (TypeError, UnicodeEncodeError) as error:
        raise ValueAuthorizationError("authorization is not canonical ASCII JSON") from error
    if len(encoded) > 16 * 1024:
        raise ValueAuthorizationError("authorization is oversized")
    return encoded


def _message(canonical: bytes) -> bytes:
    return AUTHORIZATION_DOMAIN + len(canonical).to_bytes(4, "big") + canonical


@dataclass(frozen=True)
class ValueAuthorization:
    authorization_id: str
    policy_id: str
    category: str
    quote_sha256: str
    swap_id: str
    direction: str
    principal_msat: int
    max_fee_msat: int
    max_all_in_usd_e8: int
    expires_unix: int
    authorized_by: str
    envelope_sha256: str

    @property
    def maximum_all_in_msat(self) -> int:
        return self.principal_msat + self.max_fee_msat


def sign_value_authorization(
    authorization: Mapping[str, Any], signer: AuthorizationSigner
) -> dict[str, Any]:
    """Sign with an injected key; production code need not load a private key."""

    canonical = _canonical_authorization(authorization)
    public_key = signer.public_key_bytes()
    if type(public_key) is not bytes or len(public_key) != 32:
        raise ValueAuthorizationError("authorization public key must be 32 bytes")
    signature = signer.sign(_message(canonical))
    if type(signature) is not bytes or len(signature) != 64:
        raise ValueAuthorizationError("authorization signature must be 64 bytes")
    return {
        "algorithm": AUTHORIZATION_ALGORITHM,
        "public_key": _b64url_encode(public_key),
        "authorization": dict(authorization),
        "signature": _b64url_encode(signature),
    }


def verify_value_authorization(
    envelope: Mapping[str, Any],
    policy: RealValuePolicy,
    *,
    quote: MainnetQuoteView | None = None,
    now_unix: int | None = None,
) -> ValueAuthorization:
    if not isinstance(envelope, Mapping) or frozenset(envelope.keys()) != ENVELOPE_FIELDS:
        raise ValueAuthorizationError("authorization envelope field set mismatch")
    if envelope["algorithm"] != AUTHORIZATION_ALGORITHM:
        raise ValueAuthorizationError("unsupported authorization algorithm")
    canonical = _canonical_authorization(envelope["authorization"])
    public_key = _b64url_decode(envelope["public_key"], "public_key", 32)
    expected_key = bytes.fromhex(policy.authorization_public_key_hex)
    if not hmac.compare_digest(public_key, expected_key):
        raise ValueAuthorizationError("authorization key is not pinned by policy")
    signature = _b64url_decode(envelope["signature"], "signature", 64)
    try:
        from cryptography.exceptions import InvalidSignature
        from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey

        Ed25519PublicKey.from_public_bytes(public_key).verify(
            signature, _message(canonical)
        )
    except InvalidSignature as error:
        raise ValueAuthorizationError("authorization signature is invalid") from error

    value = envelope["authorization"]
    now = int(time.time()) if now_unix is None else now_unix
    if type(now) is not int or now < 0:
        raise ValueAuthorizationError("now_unix must be a nonnegative integer")
    if value["expires_unix"] <= now:
        raise ValueAuthorizationError("authorization is expired")
    if value["policy_id"] != policy.policy_id:
        raise ValueAuthorizationError("authorization policy id mismatch")
    if value["max_fee_msat"] > policy.max_fee_msat:
        raise ValueAuthorizationError("authorization fee cap exceeds policy")
    if value["max_all_in_usd_e8"] > policy.max_per_run_usd_e8:
        raise ValueAuthorizationError("authorization exceeds per-run policy")

    if quote is not None:
        if value["category"] != "SWAP":
            raise ValueAuthorizationError("quote requires a SWAP authorization")
        expected = {
            "quote_sha256": quote.quote_sha256,
            "swap_id": quote.swap_id,
            "direction": quote.direction,
            "principal_msat": quote.invoice_amount_msat,
            "max_all_in_usd_e8": quote.maximum_all_in_usd_e8,
        }
        for field, expected_value in expected.items():
            if value[field] != expected_value:
                raise ValueAuthorizationError(
                    f"authorization {field} does not match quote"
                )
        if value["max_fee_msat"] < (
            quote.maximum_all_in_msat - quote.invoice_amount_msat
        ):
            raise ValueAuthorizationError("authorization fee budget is below quote policy")

    envelope_bytes = json.dumps(
        dict(envelope),
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
        allow_nan=False,
    ).encode("ascii")
    return ValueAuthorization(
        authorization_id=value["authorization_id"],
        policy_id=value["policy_id"],
        category=value["category"],
        quote_sha256=value["quote_sha256"],
        swap_id=value["swap_id"],
        direction=value["direction"],
        principal_msat=value["principal_msat"],
        max_fee_msat=value["max_fee_msat"],
        max_all_in_usd_e8=value["max_all_in_usd_e8"],
        expires_unix=value["expires_unix"],
        authorized_by=value["authorized_by"],
        envelope_sha256=hashlib.sha256(envelope_bytes).hexdigest(),
    )
