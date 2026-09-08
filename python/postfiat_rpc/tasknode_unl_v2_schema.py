"""Closed schema primitives for the Task Node UNL V2 evidence contract.

V2 is a separately selected ``SHADOW_ONLY`` policy.  This module has no I/O,
does not read clocks or keys, and deliberately contains no identity, personhood,
or private account-sale detector.  It reuses V1's canonical JSON and stable
validation-error type without changing any V1 interpretation.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timedelta
from typing import Any, Mapping, Sequence

from .tasknode_unl_schema import (
    SHADOW_MODE,
    TaskNodeUnlError,
    canonical_json_bytes,
    format_utc_timestamp,
    parse_utc_timestamp,
    require_closed_keys,
    require_identifier,
    require_int,
)

V2_VERSION = 2
V2_POLICY_ID = "tasknode-unl-evidence-v2"
V2_WINDOW_DAYS = 180

V2_POLICY_SCHEMA = "tasknode-unl-v2-policy-v2"
V2_WINDOW_SCHEMA = "tasknode-unl-v2-window-v2"
V2_CONTROL_EVENT_SCHEMA = "tasknode-unl-v2-control-event-v2"
V2_CONTROL_REGISTRY_SCHEMA = "tasknode-unl-v2-control-registry-v2"
V2_SCORE_EVIDENCE_SCHEMA = "tasknode-unl-v2-score-evidence-v2"
V2_FUNDING_OBSERVATION_SCHEMA = "tasknode-unl-v2-funding-observation-v2"
V2_BILATERAL_RECORD_SCHEMA = "tasknode-unl-v2-bilateral-record-v2"
V2_CONTROL_DECLARATION_SCHEMA = (
    "tasknode-unl-v2-control-declaration-v2"
)
V2_SIGNATURE_SCHEMA = "tasknode-unl-v2-signature-v2"
V2_EVIDENCE_INPUT_SCHEMA = "tasknode-unl-v2-evidence-input-v2"
V2_EVIDENCE_SNAPSHOT_SCHEMA = "tasknode-unl-v2-evidence-snapshot-v2"
V2_EVIDENCE_RESULT_SCHEMA = "tasknode-unl-v2-evidence-result-v2"

V2_SIGNATURE_ALGORITHM = "secp256k1-recoverable-sha256"
V2_POLICY_ROOT_DOMAIN = "postfiat/tasknode-unl-v2/policy-root/v2"
V2_INPUT_ROOT_DOMAIN = "postfiat/tasknode-unl-v2/input-root/v2"
V2_REGISTRY_ROOT_DOMAIN = "postfiat/tasknode-unl-v2/registry-root/v2"
V2_VOUCH_DOMAIN = "postfiat/tasknode-unl-v2/accepted-vouch/v2"
V2_COWORK_DOMAIN = "postfiat/tasknode-unl-v2/accepted-cowork/v2"
V2_CONTROL_DECLARE_DOMAIN = (
    "postfiat/tasknode-unl-v2/control-declaration/v2"
)
V2_CONTROL_DISSOLVE_DOMAIN = (
    "postfiat/tasknode-unl-v2/control-dissolution/v2"
)

CONTROL_EVENT_KINDS = (
    "baseline_binding",
    "declared_transfer",
    "validator_key_replacement",
    "wallet_binding_replacement",
    "recovery",
)
CONTROL_EVENT_AUTHORITY = {
    "baseline_binding": "binding",
    "declared_transfer": "binding",
    "validator_key_replacement": "binding",
    "wallet_binding_replacement": "binding",
    "recovery": "recovery",
}
RELATION_KINDS = ("vouch", "cowork")
FUNDING_OBSERVATION_KINDS = ("transfer", "first_funder", "majority_inflow")
SCORE_EVIDENCE_KINDS = ("work", "quality", "standing", "accountability")
BILATERAL_ACTIONS = ("grant", "renew", "revoke")
CONTROL_DECLARATION_ACTIONS = ("declare", "dissolve")

MAX_IDENTIFIER_BYTES = 128
MAX_RECORDS = 16_384
MAX_CONTROL_MEMBERS = 128
MAX_CANONICAL_DOCUMENT_BYTES = 8 * 1024 * 1024
MAX_WINDOW_INDEX = (1 << 63) - 1
SHA256_BYTES = 32
SECP256K1_PUBLIC_KEY_BYTES = 33
SECP256K1_SIGNATURE_BYTES = 65

EVIDENCE_LIMITATIONS = (
    "An unchanged-key control transfer is undetectable by these public inputs.",
    "A valid custody signature proves key control, not personhood or continuity "
    "of a human operator.",
    "Funding observations are audit-only and cannot create admission trust, "
    "control-group membership, or a veto.",
)


def require_bounded_identifier(value: object, field: str) -> str:
    """Validate an identifier and its UTF-8 byte bound without rewriting it."""

    checked = require_identifier(value, field)
    if len(checked.encode("utf-8")) > MAX_IDENTIFIER_BYTES:
        raise TaskNodeUnlError("identifier_too_long", field)
    return checked


def require_lower_hex(
    value: object,
    field: str,
    *,
    byte_length: int = SHA256_BYTES,
) -> str:
    """Validate fixed-width, lowercase canonical hexadecimal text."""

    if not isinstance(value, str) or len(value) != byte_length * 2:
        raise TaskNodeUnlError("invalid_hex_length", field)
    if value != value.lower():
        raise TaskNodeUnlError("non_canonical_hex", field)
    try:
        bytes.fromhex(value)
    except ValueError as exc:
        raise TaskNodeUnlError("invalid_hex", field) from exc
    return value


def require_array(
    value: object,
    field: str,
    *,
    maximum: int = MAX_RECORDS,
) -> list[Any]:
    """Validate a bounded JSON array."""

    if not isinstance(value, list):
        raise TaskNodeUnlError("invalid_array", field)
    if len(value) > maximum:
        raise TaskNodeUnlError("array_too_large", field)
    return value


def require_canonical_timestamp(value: object, field: str) -> datetime:
    """Parse a timestamp and reject non-canonical spellings."""

    parsed = parse_utc_timestamp(value, field)
    if format_utc_timestamp(parsed) != value:
        raise TaskNodeUnlError("non_canonical_timestamp", field)
    return parsed


def require_v2_object(
    value: object,
    *,
    schema: str,
    required: Sequence[str],
    optional: Sequence[str] = (),
    field: str,
) -> Mapping[str, Any]:
    """Validate a closed V2 object, including its explicit version fence."""

    row = require_closed_keys(
        value,
        required=("schema", "version", "mode", *required),
        optional=optional,
        field=field,
    )
    if row["schema"] != schema:
        raise TaskNodeUnlError("unknown_schema", f"{field}.schema")
    version = require_int(row["version"], f"{field}.version", minimum=0)
    if version != V2_VERSION:
        raise TaskNodeUnlError("unknown_version", f"{field}.version")
    if row["mode"] != SHADOW_MODE:
        raise TaskNodeUnlError("mode_mismatch", f"{field}.mode")
    return row


def v2_header(schema: str) -> dict[str, Any]:
    """Return the common explicit V2 header."""

    return {"schema": schema, "version": V2_VERSION, "mode": SHADOW_MODE}


@dataclass(frozen=True, order=True)
class ValidationFailure:
    """One stable, field-addressed reason that a snapshot held."""

    field: str
    code: str
    detail: str = ""

    def to_dict(self) -> dict[str, str]:
        return {"field": self.field, "code": self.code, "detail": self.detail}


@dataclass(frozen=True, order=True)
class RecordRejection:
    """An isolated invalid record that did not invalidate its snapshot."""

    record_class: str
    index: int
    field: str
    code: str
    accounts: tuple[str, ...] = ()

    def to_dict(self) -> dict[str, Any]:
        return {
            "record_class": self.record_class,
            "index": self.index,
            "field": self.field,
            "code": self.code,
            "accounts": list(self.accounts),
        }


@dataclass(frozen=True, order=True)
class EvaluationWindow:
    """One exact 180-day V2 evaluation window."""

    index: int
    start: datetime
    end: datetime

    def validate(self, field: str = "window") -> None:
        index = require_int(self.index, f"{field}.index", minimum=0)
        if index > MAX_WINDOW_INDEX:
            raise TaskNodeUnlError("window_index_out_of_range", f"{field}.index")
        if self.start.tzinfo is None or self.start.utcoffset() is None:
            raise TaskNodeUnlError("timestamp_missing_timezone", f"{field}.start")
        if self.end.tzinfo is None or self.end.utcoffset() is None:
            raise TaskNodeUnlError("timestamp_missing_timezone", f"{field}.end")
        if self.end - self.start != timedelta(days=V2_WINDOW_DAYS):
            raise TaskNodeUnlError("window_not_180_days", field)

    def to_dict(self) -> dict[str, Any]:
        self.validate()
        return {
            **v2_header(V2_WINDOW_SCHEMA),
            "index": self.index,
            "start": format_utc_timestamp(self.start),
            "end": format_utc_timestamp(self.end),
            "days": V2_WINDOW_DAYS,
        }

    @classmethod
    def from_dict(
        cls,
        value: object,
        field: str = "window",
    ) -> "EvaluationWindow":
        row = require_v2_object(
            value,
            schema=V2_WINDOW_SCHEMA,
            required=("index", "start", "end", "days"),
            field=field,
        )
        if require_int(row["days"], f"{field}.days", minimum=1) != V2_WINDOW_DAYS:
            raise TaskNodeUnlError("window_days_mismatch", f"{field}.days")
        window = cls(
            index=require_int(row["index"], f"{field}.index", minimum=0),
            start=require_canonical_timestamp(row["start"], f"{field}.start"),
            end=require_canonical_timestamp(row["end"], f"{field}.end"),
        )
        window.validate(field)
        return window


def require_document_bound(value: object, field: str) -> None:
    """Reject a canonical document above the frozen V2 byte bound."""

    if len(canonical_json_bytes(value)) > MAX_CANONICAL_DOCUMENT_BYTES:
        raise TaskNodeUnlError("document_too_large", field)
