"""Fail-closed wallet checks for the fully synthetic submarine-swap demo.

This module intentionally does not trust coordinator-derived interpretations
of either ledger.  It consumes the decoded result returned by the user's own
LND node and independent PFTL RPC reads.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import re
from typing import Any, Callable, Mapping, Sequence


QUOTE_SCHEMA = "postfiat.lightning_submarine_quote.v1"
CONDITION_PREFIX = "a0258020"
CONDITION_SUFFIX = "810120"
FULFILLMENT_PREFIX = "a0228020"
PAYMENT_HASH_RE = re.compile(r"^[0-9a-f]{64}$")
PFTL_HASH_RE = re.compile(r"^[0-9a-f]{96}$")
PFTL_ADDRESS_RE = re.compile(r"^pf[0-9a-f]{40}$")
ESCROW_CONDITION_HASH_DOMAIN = b"postfiat.escrow_condition_hash.v1"

REQUIRED_QUOTE_CLASSES = {
    "custody_class": "NON_CUSTODIAL_HASHLOCK",
    "atomicity_class": "CONDITIONAL_HTLC",
    "timeout_clock_class": "OFFCHAIN_CROSS_LEDGER_POLICY",
    "asset_control_class": "NON_FREEZABLE_TEST",
}


class ValidationError(ValueError):
    """An untrusted quote, invoice, or chain view failed validation."""


def _integer(value: Any, name: str, *, minimum: int = 0) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise ValidationError(f"{name} must be an integer")
    if value < minimum:
        raise ValidationError(f"{name} must be at least {minimum}")
    return value


def _text(value: Any, name: str) -> str:
    if not isinstance(value, str) or not value:
        raise ValidationError(f"{name} must be a nonempty string")
    return value


def _lower_hex_32(value: Any, name: str) -> str:
    value = _text(value, name)
    if PAYMENT_HASH_RE.fullmatch(value) is None:
        raise ValidationError(f"{name} must be canonical lowercase 32-byte hex")
    return value


def decode_preimage_sha256_condition(condition: Any) -> str:
    """Return the embedded SHA-256 fingerprint from one canonical condition."""

    condition = _text(condition, "condition")
    expected_length = len(CONDITION_PREFIX) + 64 + len(CONDITION_SUFFIX)
    if len(condition) != expected_length:
        raise ValidationError("condition has the wrong encoded length")
    if condition != condition.lower():
        raise ValidationError("condition must use canonical lowercase hex")
    if not condition.startswith(CONDITION_PREFIX):
        raise ValidationError("condition is not PREIMAGE-SHA-256")
    if not condition.endswith(CONDITION_SUFFIX):
        raise ValidationError("condition has a non-canonical suffix")
    fingerprint = condition[len(CONDITION_PREFIX) : len(CONDITION_PREFIX) + 64]
    return _lower_hex_32(fingerprint, "condition fingerprint")


def decode_preimage_sha256_fulfillment(fulfillment: Any) -> bytes:
    """Return the 32-byte preimage from one canonical fulfillment."""

    fulfillment = _text(fulfillment, "fulfillment")
    expected_length = len(FULFILLMENT_PREFIX) + 64
    if len(fulfillment) != expected_length:
        raise ValidationError("fulfillment has the wrong encoded length")
    if fulfillment != fulfillment.lower():
        raise ValidationError("fulfillment must use canonical lowercase hex")
    if not fulfillment.startswith(FULFILLMENT_PREFIX):
        raise ValidationError("fulfillment is not PREIMAGE-SHA-256")
    encoded = fulfillment[len(FULFILLMENT_PREFIX) :]
    _lower_hex_32(encoded, "fulfillment preimage")
    return bytes.fromhex(encoded)


def verify_fulfillment(condition: Any, fulfillment: Any) -> bool:
    fingerprint = decode_preimage_sha256_condition(condition)
    preimage = decode_preimage_sha256_fulfillment(fulfillment)
    return hashlib.sha256(preimage).hexdigest() == fingerprint


def escrow_condition_hash(condition: Any) -> str:
    """Match the consensus-side condition commitment exposed by escrow_info."""

    condition_text = _text(condition, "condition")
    digest = hashlib.sha3_384()
    digest.update(ESCROW_CONDITION_HASH_DOMAIN)
    digest.update(b"\x00")
    digest.update(condition_text.encode("utf-8"))
    return digest.hexdigest()


def _feature_is_amp(key: Any, value: Any) -> bool:
    key_text = str(key).strip().lower()
    if key_text in {"30", "31", "amp"} or "atomic multi-path" in key_text:
        return True
    if isinstance(value, str):
        value_text = value.lower()
        return value_text == "amp" or "atomic multi-path" in value_text
    if isinstance(value, Mapping):
        for name in ("name", "feature", "feature_name"):
            candidate = value.get(name)
            if isinstance(candidate, str):
                lowered = candidate.lower()
                if lowered == "amp" or "atomic multi-path" in lowered:
                    return True
    return False


def _reject_amp(decoded: Mapping[str, Any]) -> None:
    if decoded.get("is_amp") is True or decoded.get("amp") is True:
        raise ValidationError("AMP invoices are unsupported")
    features = decoded.get("features", {})
    if not isinstance(features, Mapping):
        raise ValidationError("decoded invoice features must be an object")
    if any(_feature_is_amp(key, value) for key, value in features.items()):
        raise ValidationError("AMP invoices are unsupported")


def _invoice_amount_msat(decoded: Mapping[str, Any]) -> int:
    if "num_msat" in decoded:
        value = decoded["num_msat"]
        if isinstance(value, str) and value.isdecimal():
            value = int(value)
        return _integer(value, "decoded invoice num_msat", minimum=1)
    value = decoded.get("num_satoshis")
    if isinstance(value, str) and value.isdecimal():
        value = int(value)
    satoshis = _integer(value, "decoded invoice num_satoshis", minimum=1)
    return satoshis * 1000


def _invoice_integer(decoded: Mapping[str, Any], *names: str) -> int:
    for name in names:
        if name in decoded:
            value = decoded[name]
            if isinstance(value, str) and value.isdecimal():
                value = int(value)
            return _integer(value, f"decoded invoice {name}")
    joined = " or ".join(names)
    raise ValidationError(f"decoded invoice is missing {joined}")


@dataclass(frozen=True)
class InvoiceView:
    payment_hash: str
    destination: str
    amount_msat: int
    timestamp: int
    expiry_seconds: int
    expiry_unix: int
    min_final_cltv_delta: int


def validate_invoice_against_quote(
    quote: Mapping[str, Any],
    decoded_invoice: Mapping[str, Any],
    *,
    now_unix: int,
    verify_quote_signature: Callable[[Mapping[str, Any]], bool],
) -> InvoiceView:
    """Verify the coordinator quote against the wallet's own LND decode."""

    _integer(now_unix, "now_unix")
    if quote.get("schema") != QUOTE_SCHEMA:
        raise ValidationError("unsupported quote schema")
    if quote.get("lightning_network") != "regtest":
        raise ValidationError("demo only permits Lightning regtest")
    if quote.get("direction") not in {"lightning_to_pftl", "pftl_to_lightning"}:
        raise ValidationError("unsupported swap direction")
    if not verify_quote_signature(quote):
        raise ValidationError("coordinator quote signature is invalid")
    _reject_amp(decoded_invoice)

    payment_hash = _lower_hex_32(decoded_invoice.get("payment_hash"), "payment_hash")
    if payment_hash != _lower_hex_32(quote.get("payment_hash"), "quote payment_hash"):
        raise ValidationError("invoice payment hash does not match quote")
    condition_hash = decode_preimage_sha256_condition(quote.get("condition"))
    if condition_hash != payment_hash:
        raise ValidationError("PFTL condition does not bind the invoice payment hash")

    destination = _text(decoded_invoice.get("destination"), "invoice destination")
    if destination != _text(quote.get("invoice_payee"), "quote invoice_payee"):
        raise ValidationError("invoice destination does not match quote")
    amount_msat = _invoice_amount_msat(decoded_invoice)
    if amount_msat != _integer(
        quote.get("invoice_amount_msat"), "quote invoice_amount_msat", minimum=1
    ):
        raise ValidationError("invoice amount does not match quote")

    timestamp = _invoice_integer(decoded_invoice, "timestamp")
    expiry_seconds = _invoice_integer(decoded_invoice, "expiry")
    expiry_unix = timestamp + expiry_seconds
    if expiry_unix != _integer(quote.get("invoice_expiry_unix"), "invoice_expiry_unix"):
        raise ValidationError("invoice expiry does not match quote")
    if now_unix >= expiry_unix:
        raise ValidationError("invoice is expired")
    if now_unix >= _integer(quote.get("quote_expires_unix"), "quote_expires_unix"):
        raise ValidationError("quote is expired")
    if now_unix >= _integer(
        quote.get("latest_lightning_start_unix"), "latest_lightning_start_unix"
    ):
        raise ValidationError("safe Lightning start cutoff has passed")

    min_final_cltv_delta = _invoice_integer(
        decoded_invoice, "cltv_expiry", "min_final_cltv_expiry"
    )
    if min_final_cltv_delta != _integer(
        quote.get("min_final_cltv_delta"), "min_final_cltv_delta", minimum=1
    ):
        raise ValidationError("invoice final CLTV delta does not match quote")
    max_total = _integer(
        quote.get("max_total_cltv_delta"), "max_total_cltv_delta", minimum=1
    )
    if max_total < min_final_cltv_delta:
        raise ValidationError("max total CLTV delta is below the final-hop delta")

    for field, expected in REQUIRED_QUOTE_CLASSES.items():
        if quote.get(field) != expected:
            raise ValidationError(f"quote {field} must be {expected}")

    _text(quote.get("invoice"), "invoice")
    _text(quote.get("swap_id"), "swap_id")
    _text(quote.get("pftl_chain_id"), "pftl_chain_id")
    if PFTL_HASH_RE.fullmatch(_text(quote.get("pftl_genesis_hash"), "pftl_genesis_hash")) is None:
        raise ValidationError("pftl_genesis_hash must be canonical 48-byte hex")
    _text(quote.get("pftl_asset_id"), "pftl_asset_id")
    _integer(quote.get("pftl_amount_atoms"), "pftl_amount_atoms", minimum=1)
    _integer(quote.get("pftl_owner_sequence"), "pftl_owner_sequence", minimum=1)
    for field in ("pftl_owner", "pftl_recipient"):
        if PFTL_ADDRESS_RE.fullmatch(_text(quote.get(field), field)) is None:
            raise ValidationError(f"{field} is not a canonical PFTL address")
    _integer(quote.get("finish_after"), "finish_after")
    _integer(quote.get("cancel_after"), "cancel_after", minimum=1)
    rate_denominator = _integer(quote.get("rate_denominator"), "rate_denominator", minimum=1)
    _integer(quote.get("rate_numerator"), "rate_numerator", minimum=1)
    if rate_denominator == 0:  # Kept explicit as a signed-quote invariant.
        raise ValidationError("rate denominator must not be zero")

    return InvoiceView(
        payment_hash=payment_hash,
        destination=destination,
        amount_msat=amount_msat,
        timestamp=timestamp,
        expiry_seconds=expiry_seconds,
        expiry_unix=expiry_unix,
        min_final_cltv_delta=min_final_cltv_delta,
    )


@dataclass(frozen=True)
class TimelockPolicy:
    expected_validator_count: int = 6
    minimum_available_validators: int = 5
    max_total_cltv_delta: int = 288
    claim_margin_bitcoin_blocks: int = 36
    pftl_blocks_per_bitcoin_block: int = 1
    pftl_finality_margin_blocks: int = 4

    def required_pftl_blocks(self) -> int:
        for name, value in (
            ("expected_validator_count", self.expected_validator_count),
            ("minimum_available_validators", self.minimum_available_validators),
            ("max_total_cltv_delta", self.max_total_cltv_delta),
            ("claim_margin_bitcoin_blocks", self.claim_margin_bitcoin_blocks),
            ("pftl_blocks_per_bitcoin_block", self.pftl_blocks_per_bitcoin_block),
            ("pftl_finality_margin_blocks", self.pftl_finality_margin_blocks),
        ):
            _integer(value, name, minimum=1)
        if self.minimum_available_validators > self.expected_validator_count:
            raise ValidationError("available-validator minimum exceeds validator count")
        return (
            (self.max_total_cltv_delta + self.claim_margin_bitcoin_blocks)
            * self.pftl_blocks_per_bitcoin_block
            + self.pftl_finality_margin_blocks
        )


@dataclass(frozen=True)
class PftlEscrowView:
    available_validators: int
    height: int
    tip_hash: str
    state_root: str
    escrow_id: str
    asset_id: str
    amount_atoms: int
    cancel_after: int
    recipient_asset_balance: int
    recipient_asset_headroom: int
    recipient_native_balance: int
    finish_minimum_fee: int


def _normalized_validator_view(
    view: Mapping[str, Any],
    quote: Mapping[str, Any],
) -> tuple[
    int,
    str,
    str,
    Mapping[str, Any],
    Mapping[str, Any],
    Mapping[str, Any],
    Mapping[str, Any],
    Mapping[str, Any],
]:
    status = view.get("status")
    escrow_report = view.get("escrow")
    asset_report = view.get("asset")
    account_reports = view.get("accounts")
    native_accounts = view.get("native_accounts")
    finish_fee_quote = view.get("finish_fee_quote")
    if not isinstance(status, Mapping):
        raise ValidationError("validator view is missing status")
    if not isinstance(escrow_report, Mapping):
        raise ValidationError("validator view is missing escrow")
    if not isinstance(asset_report, Mapping):
        raise ValidationError("validator view is missing asset")
    if not isinstance(account_reports, Mapping):
        raise ValidationError("validator view is missing issued-asset accounts")
    if not isinstance(native_accounts, Mapping):
        raise ValidationError("validator view is missing native accounts")
    if not isinstance(finish_fee_quote, Mapping):
        raise ValidationError("validator view is missing finish fee quote")
    if status.get("chain_id") != quote.get("pftl_chain_id"):
        raise ValidationError("PFTL chain id mismatch")
    if status.get("genesis_hash") != quote.get("pftl_genesis_hash"):
        raise ValidationError("PFTL genesis hash mismatch")
    if status.get("validator_count") != 6:
        raise ValidationError("PFTL status does not declare six validators")
    height = _integer(status.get("block_height"), "PFTL block height")
    tip_hash = _text(status.get("block_tip_hash"), "PFTL tip hash")
    state_root = _text(status.get("state_root"), "PFTL state root")
    if PFTL_HASH_RE.fullmatch(tip_hash) is None:
        raise ValidationError("PFTL tip hash is not canonical")
    if PFTL_HASH_RE.fullmatch(state_root) is None:
        raise ValidationError("PFTL state root is not canonical")
    escrow = escrow_report.get("escrow")
    asset = asset_report.get("asset", asset_report)
    if not isinstance(escrow, Mapping):
        raise ValidationError("escrow_info does not contain an escrow")
    if not isinstance(asset, Mapping):
        raise ValidationError("asset_info does not contain an asset")
    return (
        height,
        tip_hash,
        state_root,
        escrow,
        asset,
        account_reports,
        native_accounts,
        finish_fee_quote,
    )


def validate_pftl_lock_views(
    quote: Mapping[str, Any],
    validator_views: Sequence[Mapping[str, Any]],
    *,
    policy: TimelockPolicy,
) -> PftlEscrowView:
    """Require independently read, converged PFTL state before Lightning pays."""

    if len(validator_views) < policy.minimum_available_validators:
        raise ValidationError("fewer than the required PFTL validators are available")
    if len(validator_views) > policy.expected_validator_count:
        raise ValidationError("too many PFTL validator views were supplied")
    node_ids = [view.get("node_id") for view in validator_views]
    if any(
        type(node_id) is not str or not node_id
        for node_id in node_ids
    ) or len(set(node_ids)) != len(node_ids):
        raise ValidationError("PFTL validator views are not from distinct identities")
    if _integer(
        quote.get("max_total_cltv_delta"), "max_total_cltv_delta", minimum=1
    ) > policy.max_total_cltv_delta:
        raise ValidationError("quote exceeds the wallet's maximum total CLTV delta")

    normalized = [_normalized_validator_view(view, quote) for view in validator_views]
    heads = {
        (height, tip_hash, root)
        for height, tip_hash, root, *_ in normalized
    }
    if len(heads) != 1:
        raise ValidationError("PFTL validators are not converged")
    height, tip_hash, state_root = next(iter(heads))

    escrow_id = _text(quote.get("expected_escrow_id"), "expected_escrow_id")
    expected = {
        "escrow_id": escrow_id,
        "owner": quote.get("pftl_owner"),
        "recipient": quote.get("pftl_recipient"),
        "asset_id": quote.get("pftl_asset_id"),
        "amount": quote.get("pftl_amount_atoms"),
        "condition_hash": escrow_condition_hash(quote.get("condition")),
        "finish_after": quote.get("finish_after"),
        "cancel_after": quote.get("cancel_after"),
        "state": "open",
    }
    canonical_escrow: tuple[tuple[str, Any], ...] | None = None
    recipient = _text(quote.get("pftl_recipient"), "pftl_recipient")
    asset_id = _text(quote.get("pftl_asset_id"), "pftl_asset_id")
    amount_atoms = _integer(
        quote.get("pftl_amount_atoms"), "pftl_amount_atoms", minimum=1
    )
    recipient_balance: int | None = None
    recipient_headroom: int | None = None
    recipient_native_balance: int | None = None
    finish_minimum_fee: int | None = None
    for (
        _,
        _,
        _,
        escrow,
        asset,
        account_reports,
        native_accounts,
        fee_quote,
    ) in normalized:
        for field, expected_value in expected.items():
            if escrow.get(field) != expected_value:
                raise ValidationError(f"finalized escrow {field} does not match quote")
        current = tuple(sorted((key, escrow.get(key)) for key in expected))
        if canonical_escrow is None:
            canonical_escrow = current
        elif current != canonical_escrow:
            raise ValidationError("validators disagree on the finalized escrow")
        if asset.get("asset_id") != quote.get("pftl_asset_id"):
            raise ValidationError("asset report does not match quote")
        for flag in ("requires_authorization", "freeze_enabled", "clawback_enabled"):
            if asset.get(flag) is not False:
                raise ValidationError(f"demo test asset must have {flag}=false")

        account_report = account_reports.get(recipient)
        if not isinstance(account_report, Mapping):
            raise ValidationError("recipient issued-asset account is absent")
        lines = account_report.get("lines")
        if not isinstance(lines, list):
            raise ValidationError("recipient trustline list is malformed")
        matching_lines = [
            line
            for line in lines
            if isinstance(line, Mapping) and line.get("asset_id") == asset_id
        ]
        if len(matching_lines) != 1:
            raise ValidationError("recipient must have exactly one matching trustline")
        line = matching_lines[0]
        if line.get("authorized") is not True or line.get("frozen") is not False:
            raise ValidationError("recipient trustline is not movable")
        balance = _integer(line.get("balance"), "recipient trustline balance")
        limit = _integer(line.get("limit"), "recipient trustline limit", minimum=1)
        if limit < balance or limit - balance < amount_atoms:
            raise ValidationError("recipient trustline lacks finish headroom")

        native_account = native_accounts.get(recipient)
        if not isinstance(native_account, Mapping):
            raise ValidationError("recipient native account is absent")
        if native_account.get("address") != recipient:
            raise ValidationError("recipient native account address mismatch")
        native_balance = _integer(
            native_account.get("balance"), "recipient native balance"
        )
        native_sequence = _integer(
            native_account.get("sequence"), "recipient native sequence"
        )
        minimum_fee = _integer(
            fee_quote.get("minimum_fee"), "finish minimum fee", minimum=1
        )
        account_reserve = _integer(
            fee_quote.get("account_reserve"), "finish account reserve"
        )
        if (
            fee_quote.get("schema") != "postfiat-escrow-fee-quote-v1"
            or fee_quote.get("source") != recipient
            or fee_quote.get("transaction_kind") != "escrow_finish"
            or fee_quote.get("sender_balance") != native_balance
            or fee_quote.get("sender_sequence") != native_sequence
            or fee_quote.get("sequence") != native_sequence + 1
            or fee_quote.get("sender_meets_reserve_after_fee") is not True
            or native_balance < minimum_fee + account_reserve
        ):
            raise ValidationError("recipient lacks native PFT for finish and reserve")
        fee_operation = fee_quote.get("operation")
        if (
            not isinstance(fee_operation, Mapping)
            or fee_operation.get("operation") != "escrow_finish"
            or fee_operation.get("escrow_id") != escrow_id
            or fee_operation.get("owner") != quote.get("pftl_owner")
            or fee_operation.get("recipient") != recipient
        ):
            raise ValidationError("finish fee quote is not bound to the escrow")

        current = (balance, limit - balance, native_balance, minimum_fee)
        if recipient_balance is None:
            (
                recipient_balance,
                recipient_headroom,
                recipient_native_balance,
                finish_minimum_fee,
            ) = current
        elif current != (
            recipient_balance,
            recipient_headroom,
            recipient_native_balance,
            finish_minimum_fee,
        ):
            raise ValidationError("validators disagree on recipient finish capacity")

    cancel_after = _integer(quote.get("cancel_after"), "cancel_after", minimum=1)
    required_blocks = policy.required_pftl_blocks()
    if cancel_after - height < required_blocks:
        raise ValidationError(
            "PFTL refund boundary does not outlast maximum Lightning CLTV plus margin"
        )

    return PftlEscrowView(
        available_validators=len(validator_views),
        height=height,
        tip_hash=tip_hash,
        state_root=state_root,
        escrow_id=escrow_id,
        asset_id=asset_id,
        amount_atoms=amount_atoms,
        cancel_after=cancel_after,
        recipient_asset_balance=(
            recipient_balance if recipient_balance is not None else 0
        ),
        recipient_asset_headroom=(
            recipient_headroom if recipient_headroom is not None else 0
        ),
        recipient_native_balance=(
            recipient_native_balance
            if recipient_native_balance is not None
            else 0
        ),
        finish_minimum_fee=(
            finish_minimum_fee if finish_minimum_fee is not None else 0
        ),
    )
