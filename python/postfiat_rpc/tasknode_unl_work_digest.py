"""Signed Task Node work digests and a pure frozen-ledger verifier.

The verifier consumes JSON-shaped values supplied by its caller. It does not
read files, clocks, databases, credential stores, or networks, and it never
constructs or submits a transaction. A successful result proves only that the
signed claims reconcile with the supplied frozen ledger view and the shared
accountability engine.
"""

from __future__ import annotations

import hashlib
from dataclasses import dataclass
from datetime import datetime, timedelta
from typing import Any, Mapping, Protocol, Sequence

from eth_keys import keys
from eth_keys.constants import SECPK1_N

from .tasknode_unl_accountability import (
    AccountabilityEvidence,
    BadgeEvidence,
    DisputeEvidence,
    TaskEvidence,
    evaluate_accountability,
)
from .tasknode_unl_schema import (
    ACCOUNTABILITY_WINDOW_DAYS,
    SHADOW_MODE,
    TASKNODE_POINTER_SCHEMA,
    WORK_DIGEST_BODY_SCHEMA,
    WORK_DIGEST_DOMAIN,
    WORK_DIGEST_ENVELOPE_SCHEMA,
    WORK_DIGEST_LEDGER_SNAPSHOT_SCHEMA,
    WORK_DIGEST_PUBLISHING_KEYS_SCHEMA,
    WORK_DIGEST_SIGNATURE_ALGORITHM,
    WORK_DIGEST_VERIFICATION_RESULT_SCHEMA,
    TaskNodeUnlError,
    canonical_json_bytes,
    format_utc_timestamp,
    parse_utc_timestamp,
    require_closed_keys,
    require_identifier,
    require_int,
)

_SHA256_BYTES = 32
_PUBLIC_KEY_BYTES = 33
_SIGNATURE_BYTES = 65
_MAX_POINTERS = 16_384
_MAX_DISPUTES = 4_096
_MAX_PUBLISHING_KEYS = 1_024
_MAX_IDENTIFIER_BYTES = 128
_MAX_LEDGER_INDEX = (1 << 63) - 1
_TASK_KINDS = ("network", "personal")
_OUTCOMES = ("pass", "fail")
_INPUT_FIELDS = (
    "accepted_network_tasks",
    "verification_passes",
    "verification_total",
    "open_disputes",
    "first_rewarded_at",
    "badge_current",
)
EVIDENCE_LIMITATIONS = (
    "The frozen-ledger reconciliation proves pointer existence and sender, "
    "not the encrypted review outcome.",
    "Completeness is relative to the supplied frozen ledger view; the verifier "
    "does not read the live Task Node database.",
    "The signed digest does not remove the Foundation attestation boundary "
    "for work, quality, or standing.",
)


class WorkDigestSignerAdapter(Protocol):
    """Custody boundary for a caller-supplied publishing-key signer."""

    algorithm_id: str
    public_key_hex: str

    def sign_digest(self, digest: bytes) -> bytes:
        """Sign exactly one 32-byte digest without exposing private material."""


@dataclass(frozen=True, order=True)
class VerificationFailure:
    """One stable, field-addressed reason that verification held."""

    field: str
    code: str
    detail: str = ""

    def to_dict(self) -> dict[str, str]:
        return {
            "field": self.field,
            "code": self.code,
            "detail": self.detail,
        }


@dataclass(frozen=True)
class WorkDigestVerificationResult:
    """Deterministic verification output; inputs appear only after full success."""

    status: str
    failures: tuple[VerificationFailure, ...]
    digest_hash: str | None
    account_id: str | None
    bound_wallet_address: str | None
    reconciled_inputs: Mapping[str, Any] | None
    omitted_pointer_hashes: tuple[str, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": WORK_DIGEST_VERIFICATION_RESULT_SCHEMA,
            "mode": SHADOW_MODE,
            "status": self.status,
            "failures": [failure.to_dict() for failure in self.failures],
            "digest_hash": self.digest_hash,
            "account_id": self.account_id,
            "bound_wallet_address": self.bound_wallet_address,
            "reconciled_inputs": (
                dict(self.reconciled_inputs)
                if self.reconciled_inputs is not None
                else None
            ),
            "omitted_pointer_hashes": list(self.omitted_pointer_hashes),
            "evidence_limitations": list(EVIDENCE_LIMITATIONS),
        }

    def canonical_bytes(self) -> bytes:
        return canonical_json_bytes(self.to_dict())


@dataclass(frozen=True)
class _BodyContext:
    body: Mapping[str, Any]
    account_id: str
    bound_wallet_address: str
    window_start: datetime
    window_end: datetime
    publishing_key_id: str
    snapshot_id: str
    anchor_tx_hash: str
    pointers: tuple[Mapping[str, Any], ...]
    disputes: tuple[Mapping[str, Any], ...]
    badge: Mapping[str, Any]
    score_inputs: Mapping[str, Any]


@dataclass(frozen=True)
class _SnapshotContext:
    snapshot_id: str
    account_id: str
    bound_wallet_address: str
    window_start: datetime
    window_end: datetime
    complete: bool
    pointers: tuple[Mapping[str, Any], ...]
    anchor: Mapping[str, Any]


def _failure(
    field: str,
    code: str,
    detail: str = "",
) -> VerificationFailure:
    return VerificationFailure(field=field, code=code, detail=detail)


def _from_error(error: TaskNodeUnlError, fallback: str) -> VerificationFailure:
    field = error.detail if error.detail else fallback
    return _failure(field, error.code)


def _hold(
    failures: Sequence[VerificationFailure],
    *,
    digest_hash: str | None = None,
    account_id: str | None = None,
    bound_wallet_address: str | None = None,
    omitted_pointer_hashes: Sequence[str] = (),
) -> WorkDigestVerificationResult:
    return WorkDigestVerificationResult(
        status="hold",
        failures=tuple(sorted(set(failures))),
        digest_hash=digest_hash,
        account_id=account_id,
        bound_wallet_address=bound_wallet_address,
        reconciled_inputs=None,
        omitted_pointer_hashes=tuple(sorted(set(omitted_pointer_hashes))),
    )


def _require_identifier(
    value: object,
    field: str,
    *,
    maximum_bytes: int = _MAX_IDENTIFIER_BYTES,
) -> str:
    checked = require_identifier(value, field)
    if len(checked.encode("utf-8")) > maximum_bytes:
        raise TaskNodeUnlError("identifier_too_long", field)
    return checked


def _require_lower_hex(
    value: object,
    field: str,
    *,
    byte_length: int,
) -> str:
    if not isinstance(value, str) or len(value) != byte_length * 2:
        raise TaskNodeUnlError("invalid_hex_length", field)
    if value != value.lower():
        raise TaskNodeUnlError("non_canonical_hex", field)
    try:
        bytes.fromhex(value)
    except ValueError as exc:
        raise TaskNodeUnlError("invalid_hex", field) from exc
    return value


def _require_bool(value: object, field: str) -> bool:
    if not isinstance(value, bool):
        raise TaskNodeUnlError("invalid_boolean", field)
    return value


def _require_array(
    value: object,
    field: str,
    *,
    maximum: int,
) -> list[Any]:
    if not isinstance(value, list):
        raise TaskNodeUnlError("invalid_array", field)
    if len(value) > maximum:
        raise TaskNodeUnlError("array_too_large", field)
    return value


def _canonical_timestamp(value: object, field: str) -> datetime:
    parsed = parse_utc_timestamp(value, field)
    if format_utc_timestamp(parsed) != value:
        raise TaskNodeUnlError("non_canonical_timestamp", field)
    return parsed


def _optional_timestamp(value: object, field: str) -> datetime | None:
    return None if value is None else _canonical_timestamp(value, field)


def _parse_window(
    value: object,
    field: str,
) -> tuple[datetime, datetime]:
    row = require_closed_keys(
        value,
        required=("start", "end", "days"),
        field=field,
    )
    start = _canonical_timestamp(row["start"], f"{field}.start")
    end = _canonical_timestamp(row["end"], f"{field}.end")
    days = require_int(row["days"], f"{field}.days", minimum=1)
    if days != ACCOUNTABILITY_WINDOW_DAYS:
        raise TaskNodeUnlError("window_days_mismatch", f"{field}.days")
    if end - start != timedelta(days=ACCOUNTABILITY_WINDOW_DAYS):
        raise TaskNodeUnlError("window_bounds_mismatch", field)
    return start, end


def _parse_outcome(
    value: object,
    pointer_index: int,
) -> Mapping[str, Any]:
    field = f"body.pointers[{pointer_index}].outcome"
    row = require_closed_keys(
        value,
        required=(
            "task_id",
            "kind",
            "accepted_at",
            "verification_outcome",
            "verified_at",
            "rewarded_at",
            "verdict_hash",
        ),
        field=field,
    )
    _require_identifier(row["task_id"], f"{field}.task_id")
    if row["kind"] not in _TASK_KINDS:
        raise TaskNodeUnlError("unknown_task_kind", f"{field}.kind")
    if row["verification_outcome"] not in _OUTCOMES:
        raise TaskNodeUnlError(
            "unknown_verification_outcome",
            f"{field}.verification_outcome",
        )
    _optional_timestamp(row["accepted_at"], f"{field}.accepted_at")
    _canonical_timestamp(row["verified_at"], f"{field}.verified_at")
    _optional_timestamp(row["rewarded_at"], f"{field}.rewarded_at")
    _require_lower_hex(
        row["verdict_hash"],
        f"{field}.verdict_hash",
        byte_length=_SHA256_BYTES,
    )
    return row


def _parse_pointer(
    value: object,
    index: int,
) -> Mapping[str, Any]:
    field = f"body.pointers[{index}]"
    row = require_closed_keys(
        value,
        required=("pointer_hash", "outcome"),
        field=field,
    )
    _require_lower_hex(
        row["pointer_hash"],
        f"{field}.pointer_hash",
        byte_length=_SHA256_BYTES,
    )
    _parse_outcome(row["outcome"], index)
    return row


def _parse_dispute(value: object, index: int) -> Mapping[str, Any]:
    field = f"body.disputes[{index}]"
    row = require_closed_keys(
        value,
        required=("dispute_id", "opened_at", "resolved_at"),
        field=field,
    )
    _require_identifier(row["dispute_id"], f"{field}.dispute_id")
    opened_at = _canonical_timestamp(row["opened_at"], f"{field}.opened_at")
    resolved_at = _optional_timestamp(
        row["resolved_at"], f"{field}.resolved_at"
    )
    if resolved_at is not None and resolved_at < opened_at:
        raise TaskNodeUnlError("dispute_resolved_before_opened", field)
    return row


def _parse_badge(value: object) -> Mapping[str, Any]:
    field = "body.badge"
    row = require_closed_keys(
        value,
        required=("verified", "valid_from", "expires_at", "revoked_at"),
        field=field,
    )
    _require_bool(row["verified"], f"{field}.verified")
    valid_from = _canonical_timestamp(
        row["valid_from"], f"{field}.valid_from"
    )
    expires_at = _optional_timestamp(
        row["expires_at"], f"{field}.expires_at"
    )
    revoked_at = _optional_timestamp(
        row["revoked_at"], f"{field}.revoked_at"
    )
    if expires_at is not None and expires_at <= valid_from:
        raise TaskNodeUnlError("badge_expiry_not_after_start", field)
    if revoked_at is not None and revoked_at < valid_from:
        raise TaskNodeUnlError("badge_revoked_before_start", field)
    return row


def _parse_score_inputs(value: object) -> Mapping[str, Any]:
    field = "body.score_inputs"
    row = require_closed_keys(value, required=_INPUT_FIELDS, field=field)
    accepted = require_int(
        row["accepted_network_tasks"],
        f"{field}.accepted_network_tasks",
        minimum=0,
    )
    passes = require_int(
        row["verification_passes"],
        f"{field}.verification_passes",
        minimum=0,
    )
    total = require_int(
        row["verification_total"],
        f"{field}.verification_total",
        minimum=0,
    )
    if passes > total:
        raise TaskNodeUnlError(
            "verification_passes_exceed_total",
            f"{field}.verification_passes",
        )
    require_int(
        row["open_disputes"],
        f"{field}.open_disputes",
        minimum=0,
    )
    first_rewarded = _optional_timestamp(
        row["first_rewarded_at"],
        f"{field}.first_rewarded_at",
    )
    _require_bool(row["badge_current"], f"{field}.badge_current")
    if accepted < 0 or first_rewarded is None:
        raise TaskNodeUnlError(
            "missing_accountability_input",
            f"{field}.first_rewarded_at",
        )
    return row


def _parse_body(value: object) -> _BodyContext:
    row = require_closed_keys(
        value,
        required=(
            "schema",
            "mode",
            "account_id",
            "bound_wallet_address",
            "window",
            "publishing_key_id",
            "snapshot_id",
            "anchor_tx_hash",
            "pointers",
            "disputes",
            "badge",
            "score_inputs",
        ),
        field="body",
    )
    if row["schema"] != WORK_DIGEST_BODY_SCHEMA:
        raise TaskNodeUnlError("unknown_schema", "body.schema")
    if row["mode"] != SHADOW_MODE:
        raise TaskNodeUnlError("mode_mismatch", "body.mode")
    account_id = _require_identifier(row["account_id"], "body.account_id")
    wallet = _require_identifier(
        row["bound_wallet_address"],
        "body.bound_wallet_address",
    )
    window_start, window_end = _parse_window(row["window"], "body.window")
    publishing_key_id = _require_identifier(
        row["publishing_key_id"], "body.publishing_key_id"
    )
    snapshot_id = _require_identifier(row["snapshot_id"], "body.snapshot_id")
    anchor_tx_hash = _require_lower_hex(
        row["anchor_tx_hash"],
        "body.anchor_tx_hash",
        byte_length=_SHA256_BYTES,
    )
    pointer_values = _require_array(
        row["pointers"], "body.pointers", maximum=_MAX_POINTERS
    )
    pointers = tuple(
        _parse_pointer(pointer, index)
        for index, pointer in enumerate(pointer_values)
    )
    pointer_hashes = [pointer["pointer_hash"] for pointer in pointers]
    if pointer_hashes != sorted(pointer_hashes):
        raise TaskNodeUnlError("non_canonical_order", "body.pointers")
    if len(pointer_hashes) != len(set(pointer_hashes)):
        raise TaskNodeUnlError("duplicate_pointer", "body.pointers")

    dispute_values = _require_array(
        row["disputes"], "body.disputes", maximum=_MAX_DISPUTES
    )
    disputes = tuple(
        _parse_dispute(dispute, index)
        for index, dispute in enumerate(dispute_values)
    )
    dispute_ids = [dispute["dispute_id"] for dispute in disputes]
    if dispute_ids != sorted(dispute_ids):
        raise TaskNodeUnlError("non_canonical_order", "body.disputes")
    if len(dispute_ids) != len(set(dispute_ids)):
        raise TaskNodeUnlError("duplicate_dispute", "body.disputes")

    return _BodyContext(
        body=row,
        account_id=account_id,
        bound_wallet_address=wallet,
        window_start=window_start,
        window_end=window_end,
        publishing_key_id=publishing_key_id,
        snapshot_id=snapshot_id,
        anchor_tx_hash=anchor_tx_hash,
        pointers=pointers,
        disputes=disputes,
        badge=_parse_badge(row["badge"]),
        score_inputs=_parse_score_inputs(row["score_inputs"]),
    )


def work_digest_hash(body: object) -> bytes:
    """Return the domain-separated SHA-256 digest for a canonical body."""

    context = _parse_body(body)
    signing_bytes = (
        WORK_DIGEST_DOMAIN.encode("ascii")
        + b"\x00"
        + canonical_json_bytes(context.body)
    )
    return hashlib.sha256(signing_bytes).digest()


def sign_work_digest(
    body: object,
    signer: WorkDigestSignerAdapter,
) -> dict[str, Any]:
    """Sign one validated body using only the caller's signer adapter."""

    if signer.algorithm_id != WORK_DIGEST_SIGNATURE_ALGORITHM:
        raise TaskNodeUnlError(
            "unknown_signature_algorithm",
            "signer.algorithm_id",
        )
    _public_key(signer.public_key_hex, "signer.public_key_hex")
    digest = work_digest_hash(body)
    signature = signer.sign_digest(digest)
    if not isinstance(signature, bytes):
        raise TaskNodeUnlError(
            "signer_returned_non_bytes", "signer.sign_digest"
        )
    envelope = {
        "schema": WORK_DIGEST_ENVELOPE_SCHEMA,
        "mode": SHADOW_MODE,
        "digest_hash": digest.hex(),
        "signature_algorithm": signer.algorithm_id,
        "signature_hex": signature.hex(),
        "body": body,
    }
    failure = _verify_signature(
        signer.public_key_hex,
        envelope["signature_hex"],
        digest,
    )
    if failure is not None:
        raise TaskNodeUnlError(failure.code, failure.field)
    return envelope


def _public_key(public_key_hex: object, field: str) -> keys.PublicKey:
    checked = _require_lower_hex(
        public_key_hex,
        field,
        byte_length=_PUBLIC_KEY_BYTES,
    )
    raw = bytes.fromhex(checked)
    if raw[0] not in (2, 3):
        raise TaskNodeUnlError("invalid_compressed_public_key", field)
    try:
        public_key = keys.PublicKey.from_compressed_bytes(raw)
    except Exception as exc:
        raise TaskNodeUnlError("invalid_secp256k1_public_key", field) from exc
    if public_key.to_compressed_bytes() != raw:
        raise TaskNodeUnlError("non_canonical_public_key", field)
    return public_key


def _verify_signature(
    public_key_hex: object,
    signature_hex: object,
    digest: bytes,
) -> VerificationFailure | None:
    try:
        public_key = _public_key(
            public_key_hex, "publishing_keys.public_key_hex"
        )
        checked_signature = _require_lower_hex(
            signature_hex,
            "signature_hex",
            byte_length=_SIGNATURE_BYTES,
        )
        signature = keys.Signature(bytes.fromhex(checked_signature))
    except TaskNodeUnlError as error:
        return _from_error(error, "signature")
    except Exception:
        return _failure("signature_hex", "invalid_signature_encoding")
    if signature.s > SECPK1_N // 2:
        return _failure("signature_hex", "non_canonical_signature")
    if not public_key.verify_msg_hash(digest, signature):
        return _failure("signature_hex", "signature_verification_failed")
    try:
        recovered = signature.recover_public_key_from_msg_hash(digest)
    except Exception:
        return _failure("signature_hex", "signature_recovery_failed")
    if recovered != public_key:
        return _failure("signature_hex", "signature_recovery_mismatch")
    return None


def _publishing_key(
    document: object,
    publishing_key_id: str,
) -> tuple[
    tuple[str, datetime, datetime | None] | None,
    tuple[VerificationFailure, ...],
]:
    try:
        row = require_closed_keys(
            document,
            required=("schema", "mode", "keys"),
            field="publishing_keys",
        )
        if row["schema"] != WORK_DIGEST_PUBLISHING_KEYS_SCHEMA:
            raise TaskNodeUnlError(
                "unknown_schema", "publishing_keys.schema"
            )
        if row["mode"] != SHADOW_MODE:
            raise TaskNodeUnlError("mode_mismatch", "publishing_keys.mode")
        values = _require_array(
            row["keys"],
            "publishing_keys.keys",
            maximum=_MAX_PUBLISHING_KEYS,
        )
        parsed: list[tuple[str, str, datetime, datetime | None]] = []
        for index, value in enumerate(values):
            field = f"publishing_keys.keys[{index}]"
            key = require_closed_keys(
                value,
                required=(
                    "publishing_key_id",
                    "algorithm",
                    "public_key_hex",
                    "valid_from",
                    "valid_until",
                ),
                field=field,
            )
            key_id = _require_identifier(
                key["publishing_key_id"],
                f"{field}.publishing_key_id",
            )
            if key["algorithm"] != WORK_DIGEST_SIGNATURE_ALGORITHM:
                raise TaskNodeUnlError(
                    "unknown_signature_algorithm", f"{field}.algorithm"
                )
            public_key_hex = _require_lower_hex(
                key["public_key_hex"],
                f"{field}.public_key_hex",
                byte_length=_PUBLIC_KEY_BYTES,
            )
            _public_key(public_key_hex, f"{field}.public_key_hex")
            valid_from = _canonical_timestamp(
                key["valid_from"], f"{field}.valid_from"
            )
            valid_until = _optional_timestamp(
                key["valid_until"], f"{field}.valid_until"
            )
            if valid_until is not None and valid_until <= valid_from:
                raise TaskNodeUnlError(
                    "publishing_key_interval_invalid", field
                )
            parsed.append(
                (key_id, public_key_hex, valid_from, valid_until)
            )
        ids = [item[0] for item in parsed]
        if ids != sorted(ids):
            raise TaskNodeUnlError(
                "non_canonical_order", "publishing_keys.keys"
            )
        if len(ids) != len(set(ids)):
            raise TaskNodeUnlError(
                "duplicate_publishing_key", "publishing_keys.keys"
            )
        match = next(
            (item for item in parsed if item[0] == publishing_key_id),
            None,
        )
        if match is None:
            return None, (
                _failure(
                    "body.publishing_key_id",
                    "publishing_key_not_found",
                ),
            )
        _, public_key_hex, valid_from, valid_until = match
        return (public_key_hex, valid_from, valid_until), ()
    except TaskNodeUnlError as error:
        return None, (_from_error(error, "publishing_keys"),)


def _parse_snapshot_pointer(
    value: object,
    index: int,
) -> Mapping[str, Any]:
    field = f"ledger_snapshot.pointers[{index}]"
    row = require_closed_keys(
        value,
        required=(
            "pointer_hash",
            "pointer_schema",
            "sender_wallet_address",
            "account_id",
            "ledger_index",
            "transaction_index",
            "close_time",
        ),
        field=field,
    )
    _require_lower_hex(
        row["pointer_hash"],
        f"{field}.pointer_hash",
        byte_length=_SHA256_BYTES,
    )
    if row["pointer_schema"] != TASKNODE_POINTER_SCHEMA:
        raise TaskNodeUnlError("unknown_pointer_schema", f"{field}.pointer_schema")
    _require_identifier(
        row["sender_wallet_address"], f"{field}.sender_wallet_address"
    )
    _require_identifier(row["account_id"], f"{field}.account_id")
    ledger_index = require_int(
        row["ledger_index"], f"{field}.ledger_index", minimum=1
    )
    transaction_index = require_int(
        row["transaction_index"],
        f"{field}.transaction_index",
        minimum=0,
    )
    if (
        ledger_index > _MAX_LEDGER_INDEX
        or transaction_index > _MAX_LEDGER_INDEX
    ):
        raise TaskNodeUnlError("ledger_position_out_of_range", field)
    _canonical_timestamp(row["close_time"], f"{field}.close_time")
    return row


def _parse_snapshot(value: object) -> _SnapshotContext:
    row = require_closed_keys(
        value,
        required=(
            "schema",
            "mode",
            "snapshot_id",
            "account_id",
            "bound_wallet_address",
            "window",
            "complete_for_account_window",
            "pointers",
            "anchor",
        ),
        field="ledger_snapshot",
    )
    if row["schema"] != WORK_DIGEST_LEDGER_SNAPSHOT_SCHEMA:
        raise TaskNodeUnlError("unknown_schema", "ledger_snapshot.schema")
    if row["mode"] != SHADOW_MODE:
        raise TaskNodeUnlError("mode_mismatch", "ledger_snapshot.mode")
    snapshot_id = _require_identifier(
        row["snapshot_id"], "ledger_snapshot.snapshot_id"
    )
    account_id = _require_identifier(
        row["account_id"], "ledger_snapshot.account_id"
    )
    wallet = _require_identifier(
        row["bound_wallet_address"],
        "ledger_snapshot.bound_wallet_address",
    )
    start, end = _parse_window(row["window"], "ledger_snapshot.window")
    complete = _require_bool(
        row["complete_for_account_window"],
        "ledger_snapshot.complete_for_account_window",
    )
    values = _require_array(
        row["pointers"],
        "ledger_snapshot.pointers",
        maximum=_MAX_POINTERS,
    )
    pointers = tuple(
        _parse_snapshot_pointer(pointer, index)
        for index, pointer in enumerate(values)
    )
    positions = [
        (
            pointer["ledger_index"],
            pointer["transaction_index"],
            pointer["pointer_hash"],
        )
        for pointer in pointers
    ]
    if positions != sorted(positions):
        raise TaskNodeUnlError(
            "non_canonical_order", "ledger_snapshot.pointers"
        )
    hashes = [pointer["pointer_hash"] for pointer in pointers]
    if len(hashes) != len(set(hashes)):
        raise TaskNodeUnlError(
            "duplicate_pointer", "ledger_snapshot.pointers"
        )

    anchor = require_closed_keys(
        row["anchor"],
        required=(
            "tx_hash",
            "ledger_index",
            "transaction_index",
            "close_time",
            "anchored_digest_hash",
        ),
        field="ledger_snapshot.anchor",
    )
    _require_lower_hex(
        anchor["tx_hash"],
        "ledger_snapshot.anchor.tx_hash",
        byte_length=_SHA256_BYTES,
    )
    ledger_index = require_int(
        anchor["ledger_index"],
        "ledger_snapshot.anchor.ledger_index",
        minimum=1,
    )
    transaction_index = require_int(
        anchor["transaction_index"],
        "ledger_snapshot.anchor.transaction_index",
        minimum=0,
    )
    if (
        ledger_index > _MAX_LEDGER_INDEX
        or transaction_index > _MAX_LEDGER_INDEX
    ):
        raise TaskNodeUnlError(
            "ledger_position_out_of_range", "ledger_snapshot.anchor"
        )
    _canonical_timestamp(
        anchor["close_time"], "ledger_snapshot.anchor.close_time"
    )
    _require_lower_hex(
        anchor["anchored_digest_hash"],
        "ledger_snapshot.anchor.anchored_digest_hash",
        byte_length=_SHA256_BYTES,
    )
    return _SnapshotContext(
        snapshot_id=snapshot_id,
        account_id=account_id,
        bound_wallet_address=wallet,
        window_start=start,
        window_end=end,
        complete=complete,
        pointers=pointers,
        anchor=anchor,
    )


def _body_evidence(context: _BodyContext) -> AccountabilityEvidence:
    tasks: list[TaskEvidence] = []
    for pointer in context.pointers:
        outcome = pointer["outcome"]
        tasks.append(
            TaskEvidence(
                task_id=outcome["task_id"],
                kind=outcome["kind"],
                accepted_at=_optional_timestamp(
                    outcome["accepted_at"], "outcome.accepted_at"
                ),
                verification_outcome=outcome["verification_outcome"],
                verified_at=_canonical_timestamp(
                    outcome["verified_at"], "outcome.verified_at"
                ),
                rewarded_at=_optional_timestamp(
                    outcome["rewarded_at"], "outcome.rewarded_at"
                ),
            )
        )
    disputes = tuple(
        DisputeEvidence(
            dispute_id=dispute["dispute_id"],
            opened_at=_canonical_timestamp(
                dispute["opened_at"], "dispute.opened_at"
            ),
            resolved_at=_optional_timestamp(
                dispute["resolved_at"], "dispute.resolved_at"
            ),
        )
        for dispute in context.disputes
    )
    badge = BadgeEvidence(
        verified=context.badge["verified"],
        valid_from=_canonical_timestamp(
            context.badge["valid_from"], "badge.valid_from"
        ),
        expires_at=_optional_timestamp(
            context.badge["expires_at"], "badge.expires_at"
        ),
        revoked_at=_optional_timestamp(
            context.badge["revoked_at"], "badge.revoked_at"
        ),
    )
    return AccountabilityEvidence(
        window_end=context.window_end,
        tasks=tuple(tasks),
        disputes=disputes,
        badge=badge,
    )


def _reconcile_pointers(
    body: _BodyContext,
    snapshot: _SnapshotContext,
) -> tuple[list[VerificationFailure], tuple[str, ...]]:
    failures: list[VerificationFailure] = []
    ledger_by_hash = {
        pointer["pointer_hash"]: pointer for pointer in snapshot.pointers
    }
    body_hashes = {pointer["pointer_hash"] for pointer in body.pointers}
    for pointer in body.pointers:
        pointer_hash = pointer["pointer_hash"]
        field = f"body.pointers[{pointer_hash}]"
        ledger_pointer = ledger_by_hash.get(pointer_hash)
        if ledger_pointer is None:
            failures.append(
                _failure(field, "pointer_missing_from_ledger")
            )
            continue
        if ledger_pointer["sender_wallet_address"] != body.bound_wallet_address:
            failures.append(
                _failure(
                    f"{field}.sender_wallet_address",
                    "pointer_wrong_sender",
                )
            )
        if ledger_pointer["account_id"] != body.account_id:
            failures.append(
                _failure(f"{field}.account_id", "pointer_wrong_account")
            )
        close_time = _canonical_timestamp(
            ledger_pointer["close_time"], "ledger_pointer.close_time"
        )
        outcome = pointer["outcome"]
        for event_field in ("accepted_at", "verified_at", "rewarded_at"):
            event_time = _optional_timestamp(
                outcome[event_field],
                f"{field}.outcome.{event_field}",
            )
            if event_time is not None and event_time > close_time:
                failures.append(
                    _failure(
                        f"{field}.outcome.{event_field}",
                        "task_event_after_pointer_close",
                    )
                )
        if not body.window_start <= close_time <= body.window_end:
            failures.append(
                _failure(f"{field}.close_time", "pointer_outside_window")
            )

    eligible_hashes = {
        pointer["pointer_hash"]
        for pointer in snapshot.pointers
        if pointer["sender_wallet_address"] == body.bound_wallet_address
        and body.window_start
        <= _canonical_timestamp(
            pointer["close_time"], "ledger_pointer.close_time"
        )
        <= body.window_end
    }
    omitted = tuple(sorted(eligible_hashes - body_hashes))
    for pointer_hash in omitted:
        failures.append(
            _failure(
                f"body.pointers[{pointer_hash}]",
                "eligible_pointer_omitted",
            )
        )
    return failures, omitted


def _reconciled_inputs(
    body: _BodyContext,
) -> tuple[Mapping[str, Any] | None, list[VerificationFailure]]:
    try:
        accountability = evaluate_accountability(_body_evidence(body))
    except TaskNodeUnlError as error:
        return None, [_from_error(error, "body.pointers")]
    if accountability.status != "scored":
        failures = [
            _failure(
                "body.score_inputs",
                "accountability_inputs_hold",
                reason,
            )
            for reason in accountability.hold_reasons
        ]
        return None, failures
    actual = accountability.to_dict()["inputs"]
    failures: list[VerificationFailure] = []
    for field in _INPUT_FIELDS:
        if body.score_inputs[field] != actual[field]:
            failures.append(
                _failure(
                    f"body.score_inputs.{field}",
                    "score_input_mismatch",
                )
            )
    return actual, failures


def verify_work_digest(
    document: object,
    ledger_snapshot: object,
    publishing_keys: object,
    *,
    expected_account_id: str,
    bound_wallet_address: str,
    expected_window_end: datetime,
) -> WorkDigestVerificationResult:
    """Verify one signed digest against explicit local registry and ledger data."""

    try:
        expected_account = _require_identifier(
            expected_account_id, "expected_account_id"
        )
        expected_wallet = _require_identifier(
            bound_wallet_address, "bound_wallet_address"
        )
        expected_end = _canonical_timestamp(
            format_utc_timestamp(expected_window_end),
            "expected_window_end",
        )
        envelope = require_closed_keys(
            document,
            required=(
                "schema",
                "mode",
                "digest_hash",
                "signature_algorithm",
                "signature_hex",
                "body",
            ),
            field="work_digest",
        )
        if envelope["schema"] != WORK_DIGEST_ENVELOPE_SCHEMA:
            raise TaskNodeUnlError("unknown_schema", "work_digest.schema")
        if envelope["mode"] != SHADOW_MODE:
            raise TaskNodeUnlError("mode_mismatch", "work_digest.mode")
        body = _parse_body(envelope["body"])
        claimed_digest = _require_lower_hex(
            envelope["digest_hash"],
            "work_digest.digest_hash",
            byte_length=_SHA256_BYTES,
        )
        actual_digest = work_digest_hash(body.body)
    except TaskNodeUnlError as error:
        return _hold((_from_error(error, "work_digest"),))

    failures: list[VerificationFailure] = []
    if body.account_id != expected_account:
        failures.append(
            _failure("body.account_id", "account_binding_mismatch")
        )
    if body.bound_wallet_address != expected_wallet:
        failures.append(
            _failure(
                "body.bound_wallet_address", "wallet_binding_mismatch"
            )
        )
    if body.window_end != expected_end:
        failures.append(
            _failure("body.window.end", "window_binding_mismatch")
        )
    if claimed_digest != actual_digest.hex():
        failures.append(
            _failure("work_digest.digest_hash", "digest_hash_mismatch")
        )
    if envelope["signature_algorithm"] != WORK_DIGEST_SIGNATURE_ALGORITHM:
        failures.append(
            _failure(
                "work_digest.signature_algorithm",
                "unknown_signature_algorithm",
            )
        )

    publishing_key, key_failures = _publishing_key(
        publishing_keys,
        body.publishing_key_id,
    )
    failures.extend(key_failures)
    if publishing_key is not None:
        public_key_hex, _valid_from, _valid_until = publishing_key
        signature_failure = _verify_signature(
            public_key_hex,
            envelope["signature_hex"],
            actual_digest,
        )
        if signature_failure is not None:
            failures.append(signature_failure)

    if failures:
        return _hold(
            failures,
            digest_hash=actual_digest.hex(),
            account_id=body.account_id,
            bound_wallet_address=body.bound_wallet_address,
        )

    try:
        snapshot = _parse_snapshot(ledger_snapshot)
    except TaskNodeUnlError as error:
        return _hold(
            (_from_error(error, "ledger_snapshot"),),
            digest_hash=actual_digest.hex(),
            account_id=body.account_id,
            bound_wallet_address=body.bound_wallet_address,
        )

    if snapshot.snapshot_id != body.snapshot_id:
        failures.append(
            _failure("body.snapshot_id", "snapshot_binding_mismatch")
        )
    if snapshot.account_id != body.account_id:
        failures.append(
            _failure(
                "ledger_snapshot.account_id", "account_binding_mismatch"
            )
        )
    if snapshot.bound_wallet_address != body.bound_wallet_address:
        failures.append(
            _failure(
                "ledger_snapshot.bound_wallet_address",
                "wallet_binding_mismatch",
            )
        )
    if (
        snapshot.window_start != body.window_start
        or snapshot.window_end != body.window_end
    ):
        failures.append(
            _failure("ledger_snapshot.window", "window_binding_mismatch")
        )
    if not snapshot.complete:
        failures.append(
            _failure(
                "ledger_snapshot.complete_for_account_window",
                "incomplete_frozen_view",
            )
        )
    anchor = snapshot.anchor
    if anchor["tx_hash"] != body.anchor_tx_hash:
        failures.append(
            _failure(
                "ledger_snapshot.anchor.tx_hash",
                "anchor_transaction_mismatch",
            )
        )
    if anchor["anchored_digest_hash"] != actual_digest.hex():
        failures.append(
            _failure(
                "ledger_snapshot.anchor.anchored_digest_hash",
                "anchor_digest_mismatch",
            )
        )
    anchor_time = _canonical_timestamp(
        anchor["close_time"], "ledger_snapshot.anchor.close_time"
    )
    if anchor_time < body.window_end:
        failures.append(
            _failure(
                "ledger_snapshot.anchor.close_time",
                "anchor_precedes_window_end",
            )
        )
    if publishing_key is not None:
        _public_key_hex, valid_from, valid_until = publishing_key
        if anchor_time < valid_from or (
            valid_until is not None and anchor_time >= valid_until
        ):
            failures.append(
                _failure(
                    "body.publishing_key_id",
                    "publishing_key_not_current",
                )
            )
    anchor_position = (
        anchor["ledger_index"],
        anchor["transaction_index"],
    )
    for pointer in snapshot.pointers:
        pointer_position = (
            pointer["ledger_index"],
            pointer["transaction_index"],
        )
        if pointer_position >= anchor_position:
            failures.append(
                _failure(
                    (
                        "ledger_snapshot.pointers["
                        f"{pointer['pointer_hash']}].ledger_position"
                    ),
                    "pointer_not_frozen_by_anchor",
                )
            )

    if failures:
        return _hold(
            failures,
            digest_hash=actual_digest.hex(),
            account_id=body.account_id,
            bound_wallet_address=body.bound_wallet_address,
        )

    pointer_failures, omitted = _reconcile_pointers(body, snapshot)
    if pointer_failures:
        return _hold(
            pointer_failures,
            digest_hash=actual_digest.hex(),
            account_id=body.account_id,
            bound_wallet_address=body.bound_wallet_address,
            omitted_pointer_hashes=omitted,
        )

    reconciled, input_failures = _reconciled_inputs(body)
    if input_failures:
        return _hold(
            input_failures,
            digest_hash=actual_digest.hex(),
            account_id=body.account_id,
            bound_wallet_address=body.bound_wallet_address,
        )

    assert reconciled is not None
    return WorkDigestVerificationResult(
        status="verified",
        failures=(),
        digest_hash=actual_digest.hex(),
        account_id=body.account_id,
        bound_wallet_address=body.bound_wallet_address,
        reconciled_inputs=reconciled,
        omitted_pointer_hashes=(),
    )
