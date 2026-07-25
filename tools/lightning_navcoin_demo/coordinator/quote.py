"""Canonical, all-fields submarine-swap quote validation."""

from __future__ import annotations

import json
from typing import Any, Mapping

from .protocol import ProtocolEncodingError, decode_condition


QUOTE_SCHEMA = "postfiat.lightning_submarine_quote.v1"
MAX_QUOTE_BYTES = 64 * 1024
MAX_SQLITE_INTEGER = (1 << 63) - 1

QUOTE_FIELDS = frozenset(
    {
        "schema",
        "swap_id",
        "quote_expires_unix",
        "direction",
        "payment_hash",
        "lightning_network",
        "invoice",
        "invoice_payee",
        "invoice_amount_msat",
        "invoice_expiry_unix",
        "min_final_cltv_delta",
        "max_total_cltv_delta",
        "pftl_chain_id",
        "pftl_genesis_hash",
        "pftl_asset_id",
        "pftl_amount_atoms",
        "pftl_owner",
        "pftl_owner_sequence",
        "pftl_recipient",
        "expected_escrow_id",
        "condition",
        "finish_after",
        "cancel_after",
        "latest_lightning_start_unix",
        "rate_numerator",
        "rate_denominator",
        "coordinator_fee_atoms",
        "nav_epoch",
        "nav_reserve_packet_hash",
        "custody_class",
        "atomicity_class",
        "timeout_clock_class",
        "asset_control_class",
    }
)

UINT_FIELDS = frozenset(
    {
        "quote_expires_unix",
        "invoice_amount_msat",
        "invoice_expiry_unix",
        "min_final_cltv_delta",
        "max_total_cltv_delta",
        "pftl_amount_atoms",
        "pftl_owner_sequence",
        "finish_after",
        "cancel_after",
        "latest_lightning_start_unix",
        "rate_numerator",
        "rate_denominator",
        "coordinator_fee_atoms",
        "nav_epoch",
    }
)

NONEMPTY_TEXT_FIELDS = frozenset(
    {
        "schema",
        "swap_id",
        "direction",
        "payment_hash",
        "lightning_network",
        "invoice",
        "invoice_payee",
        "pftl_chain_id",
        "pftl_genesis_hash",
        "pftl_asset_id",
        "pftl_owner",
        "pftl_recipient",
        "expected_escrow_id",
        "condition",
        "custody_class",
        "atomicity_class",
        "timeout_clock_class",
        "asset_control_class",
    }
)


class QuoteValidationError(ValueError):
    """A quote is malformed, incomplete, inconsistent, or non-canonical."""


def _reject_duplicate_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise QuoteValidationError(f"duplicate JSON field: {key}")
        result[key] = value
    return result


def _lower_hex(value: Any, field: str, byte_length: int) -> str:
    if type(value) is not str or len(value) != byte_length * 2:
        raise QuoteValidationError(f"{field} must encode {byte_length} bytes")
    if any(character not in "0123456789abcdef" for character in value):
        raise QuoteValidationError(f"{field} must be lowercase hexadecimal")
    return value


def _text(value: Any, field: str, *, allow_empty: bool = False) -> str:
    if type(value) is not str:
        raise QuoteValidationError(f"{field} must be a string")
    if not allow_empty and not value:
        raise QuoteValidationError(f"{field} must not be empty")
    if len(value) > 4096 or not value.isascii():
        raise QuoteValidationError(f"{field} is too long or non-ASCII")
    if any(ord(character) < 0x20 or ord(character) == 0x7F for character in value):
        raise QuoteValidationError(f"{field} contains a control character")
    return value


def _uint(value: Any, field: str) -> int:
    if type(value) is not int:
        raise QuoteValidationError(f"{field} must be an integer")
    if value < 0 or value > MAX_SQLITE_INTEGER:
        raise QuoteValidationError(f"{field} is outside supported uint63")
    return value


def validate_quote(quote: Mapping[str, Any]) -> dict[str, Any]:
    """Validate and return a detached quote dictionary.

    Unknown and missing fields are rejected so the signature meaning cannot
    vary between implementations.
    """

    if not isinstance(quote, Mapping):
        raise QuoteValidationError("quote must be a mapping")
    actual_fields = frozenset(quote.keys())
    if any(type(key) is not str for key in quote.keys()):
        raise QuoteValidationError("quote field names must be strings")
    if actual_fields != QUOTE_FIELDS:
        missing = sorted(QUOTE_FIELDS - actual_fields)
        unknown = sorted(actual_fields - QUOTE_FIELDS)
        raise QuoteValidationError(f"quote field set mismatch; missing={missing}, unknown={unknown}")

    detached: dict[str, Any] = {}
    for field in UINT_FIELDS:
        detached[field] = _uint(quote[field], field)
    for field in NONEMPTY_TEXT_FIELDS:
        detached[field] = _text(quote[field], field)
    detached["nav_reserve_packet_hash"] = _text(
        quote["nav_reserve_packet_hash"],
        "nav_reserve_packet_hash",
        allow_empty=True,
    )

    if detached["schema"] != QUOTE_SCHEMA:
        raise QuoteValidationError("unsupported quote schema")
    if detached["direction"] not in {"lightning_to_pftl", "pftl_to_lightning"}:
        raise QuoteValidationError("unsupported swap direction")
    if detached["lightning_network"] not in {"regtest", "signet", "bitcoin"}:
        raise QuoteValidationError("unsupported Lightning network")
    if detached["custody_class"] != "NON_CUSTODIAL_HASHLOCK":
        raise QuoteValidationError("unexpected custody class")
    if detached["atomicity_class"] != "CONDITIONAL_HTLC":
        raise QuoteValidationError("unexpected atomicity class")
    if detached["timeout_clock_class"] != "OFFCHAIN_CROSS_LEDGER_POLICY":
        raise QuoteValidationError("unexpected timeout clock class")
    if detached["asset_control_class"] not in {
        "NON_FREEZABLE_TEST",
        "CONTROLLED_ISSUED_ASSET",
    }:
        raise QuoteValidationError("unexpected asset control class")

    swap_id = _lower_hex(detached["swap_id"], "swap_id", 32)
    payment_hash_hex = _lower_hex(detached["payment_hash"], "payment_hash", 32)
    detached["swap_id"] = swap_id
    detached["payment_hash"] = payment_hash_hex
    detached["pftl_genesis_hash"] = _lower_hex(
        detached["pftl_genesis_hash"], "pftl_genesis_hash", 48
    )
    detached["pftl_asset_id"] = _lower_hex(
        detached["pftl_asset_id"], "pftl_asset_id", 48
    )
    detached["expected_escrow_id"] = _lower_hex(
        detached["expected_escrow_id"], "expected_escrow_id", 48
    )
    for field in ("pftl_owner", "pftl_recipient"):
        address = detached[field]
        if (
            len(address) != 42
            or not address.startswith("pf")
            or any(character not in "0123456789abcdef" for character in address[2:])
        ):
            raise QuoteValidationError(
                f"{field} must be canonical pf + 20-byte lowercase hex"
            )
    invoice_payee = detached["invoice_payee"]
    if (
        len(invoice_payee) != 66
        or invoice_payee[:2] not in {"02", "03"}
        or any(character not in "0123456789abcdef" for character in invoice_payee)
    ):
        raise QuoteValidationError(
            "invoice_payee must be a compressed 33-byte secp256k1 key"
        )
    try:
        condition_hash = decode_condition(detached["condition"]).hex()
    except ProtocolEncodingError as error:
        raise QuoteValidationError(str(error)) from error
    if condition_hash != payment_hash_hex:
        raise QuoteValidationError("condition does not bind quote payment_hash")

    nav_hash = detached["nav_reserve_packet_hash"]
    if nav_hash:
        detached["nav_reserve_packet_hash"] = _lower_hex(
            nav_hash, "nav_reserve_packet_hash", 48
        )
    elif detached["nav_epoch"] != 0:
        raise QuoteValidationError("nonzero nav_epoch requires a reserve packet hash")

    positive_fields = (
        "quote_expires_unix",
        "invoice_amount_msat",
        "invoice_expiry_unix",
        "min_final_cltv_delta",
        "max_total_cltv_delta",
        "pftl_amount_atoms",
        "latest_lightning_start_unix",
        "rate_numerator",
        "rate_denominator",
        "cancel_after",
    )
    for field in positive_fields:
        if detached[field] == 0:
            raise QuoteValidationError(f"{field} must be positive")
    if detached["coordinator_fee_atoms"] > detached["pftl_amount_atoms"]:
        raise QuoteValidationError("coordinator fee exceeds PFTL principal")
    if detached["finish_after"] >= detached["cancel_after"]:
        raise QuoteValidationError("PFTL finish window must be nonempty")
    if (
        detached["quote_expires_unix"] > detached["invoice_expiry_unix"]
        or detached["latest_lightning_start_unix"]
        > detached["invoice_expiry_unix"]
    ):
        raise QuoteValidationError(
            "quote expiry and safe-start cutoff must not outlast invoice expiry"
        )
    if detached["min_final_cltv_delta"] > detached["max_total_cltv_delta"]:
        raise QuoteValidationError("minimum final CLTV exceeds maximum total CLTV")

    # Preserve precisely the documented field set, independent of input map type.
    return {field: detached[field] for field in sorted(QUOTE_FIELDS)}


def canonical_quote_bytes(quote: Mapping[str, Any]) -> bytes:
    validated = validate_quote(quote)
    encoded = json.dumps(
        validated,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
        allow_nan=False,
    ).encode("ascii")
    if len(encoded) > MAX_QUOTE_BYTES:
        raise QuoteValidationError("canonical quote exceeds size limit")
    return encoded


def parse_canonical_quote(encoded: bytes) -> dict[str, Any]:
    if type(encoded) is not bytes or not encoded or len(encoded) > MAX_QUOTE_BYTES:
        raise QuoteValidationError("canonical quote bytes are empty or oversized")
    try:
        text = encoded.decode("ascii")
        value = json.loads(text, object_pairs_hook=_reject_duplicate_object)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise QuoteValidationError("canonical quote is invalid JSON") from error
    validated = validate_quote(value)
    if canonical_quote_bytes(validated) != encoded:
        raise QuoteValidationError("quote JSON is not canonical")
    return validated
