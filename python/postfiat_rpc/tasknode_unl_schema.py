"""Shared deterministic types and constants for the Task Node UNL shadow path.

This module has no I/O. Published proposal constants are represented as
integers or fractions.Fraction values so callers cannot introduce binary
floating-point behavior.
"""

from __future__ import annotations

import json
from datetime import datetime, timezone
from fractions import Fraction
from typing import Any, Mapping, Sequence

SHADOW_MODE = "SHADOW_ONLY"

ACCOUNTABILITY_INPUT_SCHEMA = "tasknode-unl-accountability-input-v1"
ACCOUNTABILITY_RESULT_SCHEMA = "tasknode-unl-accountability-result-v1"
TRUST_GRAPH_INPUT_SCHEMA = "tasknode-unl-trust-graph-input-v1"
TRUST_GRAPH_RESULT_SCHEMA = "tasknode-unl-trust-graph-result-v1"
BINDING_CHALLENGE_SCHEMA = "tasknode-unl-binding-challenge-v1"
BINDING_SIGNATURE_SCHEMA = "tasknode-unl-binding-signature-v1"
BINDING_MEMO_SCHEMA = "tnub1"
BINDING_MEMO_ARTIFACT_SCHEMA = "tasknode-unl-binding-memo-artifact-v1"
BINDING_LEDGER_RECORD_SCHEMA = "tasknode-unl-binding-ledger-record-v1"
BINDING_REPLAY_INPUT_SCHEMA = "tasknode-unl-binding-replay-input-v1"
BINDING_REPLAY_RESULT_SCHEMA = "tasknode-unl-binding-replay-result-v1"
BINDING_VERIFICATION_RESULT_SCHEMA = (
    "tasknode-unl-binding-verification-result-v1"
)
WORK_DIGEST_BODY_SCHEMA = "tasknode-unl-work-digest-body-v1"
WORK_DIGEST_ENVELOPE_SCHEMA = "tasknode-unl-work-digest-envelope-v1"
WORK_DIGEST_LEDGER_SNAPSHOT_SCHEMA = (
    "tasknode-unl-work-digest-ledger-snapshot-v1"
)
WORK_DIGEST_PUBLISHING_KEYS_SCHEMA = (
    "tasknode-unl-work-digest-publishing-keys-v1"
)
WORK_DIGEST_VERIFICATION_RESULT_SCHEMA = (
    "tasknode-unl-work-digest-verification-result-v1"
)

ACCOUNTABILITY_WINDOW_DAYS = 180
ACCOUNTABILITY_WORK_DENOMINATOR = 40
ACCOUNTABILITY_TENURE_DENOMINATOR_DAYS = 365
ACCOUNTABILITY_FLOOR = 70
ACCOUNTABILITY_TERM_WEIGHTS = (
    ("work", 35),
    ("tenure", 25),
    ("quality", 20),
    ("standing", 10),
    ("badge", 10),
)

VOUCH_EDGE_WEIGHT = Fraction(1, 1)
COWORK_EDGE_WEIGHT = Fraction(1, 1)
COWORK_EDGE_CAP = 3
FUNDING_EDGE_WEIGHT = Fraction(2, 1)
TRUST_WALK_ITERATIONS = 20
TRUST_WALK_DAMPING = Fraction(85, 100)
TRUST_WALK_SEED_DAMPING = Fraction(15, 100)
CONDUCTANCE_CUT_THRESHOLD = Fraction(1, 10)
CONNECTIVITY_FLOOR_DIVISOR = 2
MIN_CLUSTER_SEATS = 2
CLUSTER_SEAT_FRACTION = Fraction(1, 10)
SINGLE_CHANGE_UNTIL_VALIDATOR_COUNT = 39
MAX_CHANGES_BELOW_VALIDATOR_THRESHOLD = 1

BINDING_CHALLENGE_DOMAIN = "postfiat/tasknode-unl/binding-challenge/v1"
BINDING_SIGNATURE_ALGORITHM = "secp256k1-recoverable-sha256"
PFT_LEDGER_MEMO_MAX_BYTES = 512
BINDING_EVALUATION_WINDOW_DAYS = ACCOUNTABILITY_WINDOW_DAYS
WORK_DIGEST_DOMAIN = "postfiat/tasknode-unl/work-digest/v1"
WORK_DIGEST_SIGNATURE_ALGORITHM = BINDING_SIGNATURE_ALGORITHM
TASKNODE_POINTER_SCHEMA = "pf.ptr/v4"
TASKNODE_BINDING_EVIDENCE_FIELDS = (
    "validator.identity.tasknode_binding.wallet_address",
    "validator.identity.tasknode_binding.tx_hash",
    "validator.identity.tasknode_binding.challenge_digest",
    "validator.identity.tasknode_binding.validator_signature",
    "validator.identity.tasknode_binding.wallet_signature",
)

ACCOUNTABILITY_TERMS = tuple(name for name, _weight in ACCOUNTABILITY_TERM_WEIGHTS)
EDGE_KINDS = ("vouch", "cowork", "funding")


class TaskNodeUnlError(ValueError):
    """A deterministic validation failure with a stable reason code."""

    def __init__(self, code: str, detail: str = "") -> None:
        self.code = code
        self.detail = detail
        message = code if not detail else f"{code}: {detail}"
        super().__init__(message)


def require_int(value: object, field: str, *, minimum: int | None = None) -> int:
    """Return a strict integer, rejecting booleans and values below minimum."""

    if isinstance(value, bool) or not isinstance(value, int):
        raise TaskNodeUnlError("invalid_integer", field)
    if minimum is not None and value < minimum:
        raise TaskNodeUnlError("integer_below_minimum", field)
    return value


def require_identifier(value: object, field: str) -> str:
    """Return a non-empty identifier without changing its bytes."""

    if not isinstance(value, str) or not value:
        raise TaskNodeUnlError("invalid_identifier", field)
    if value != value.strip():
        raise TaskNodeUnlError("non_canonical_identifier", field)
    return value


def require_closed_keys(
    value: object,
    *,
    required: Sequence[str],
    optional: Sequence[str] = (),
    field: str,
) -> Mapping[str, Any]:
    """Validate a JSON-object-shaped mapping with no unknown keys."""

    if not isinstance(value, Mapping):
        raise TaskNodeUnlError("invalid_object", field)
    keys = set(value)
    if any(not isinstance(key, str) for key in keys):
        raise TaskNodeUnlError("invalid_field_name", field)
    required_keys = set(required)
    allowed = required_keys | set(optional)
    missing = sorted(required_keys - keys)
    if missing:
        raise TaskNodeUnlError("missing_field", f"{field}.{missing[0]}")
    unknown = sorted(keys - allowed)
    if unknown:
        raise TaskNodeUnlError("unknown_field", f"{field}.{unknown[0]}")
    return value


def parse_utc_timestamp(value: object, field: str) -> datetime:
    """Parse a timezone-aware ISO-8601 timestamp and normalize it to UTC."""

    if not isinstance(value, str) or not value:
        raise TaskNodeUnlError("invalid_timestamp", field)
    candidate = value[:-1] + "+00:00" if value.endswith("Z") else value
    try:
        parsed = datetime.fromisoformat(candidate)
    except ValueError as exc:
        raise TaskNodeUnlError("invalid_timestamp", field) from exc
    if parsed.tzinfo is None or parsed.utcoffset() is None:
        raise TaskNodeUnlError("timestamp_missing_timezone", field)
    return parsed.astimezone(timezone.utc)


def format_utc_timestamp(value: datetime) -> str:
    """Return a canonical UTC timestamp, retaining sub-second precision if set."""

    if value.tzinfo is None or value.utcoffset() is None:
        raise TaskNodeUnlError("timestamp_missing_timezone", "datetime")
    normalized = value.astimezone(timezone.utc)
    timespec = "microseconds" if normalized.microsecond else "seconds"
    return normalized.isoformat(timespec=timespec).replace("+00:00", "Z")


def clamp_unit(value: Fraction) -> Fraction:
    """Clamp an exact rational value to the closed interval from zero to one."""

    if value < 0:
        return Fraction(0, 1)
    if value > 1:
        return Fraction(1, 1)
    return value


def fraction_document(value: Fraction) -> dict[str, int]:
    """Serialize a rational without precision loss."""

    return {"numerator": value.numerator, "denominator": value.denominator}


def connectivity_floor(list_size: int) -> Fraction:
    """Return the proposal's exact stationary-mass floor of 1 divided by 2N."""

    size = require_int(list_size, "list_size", minimum=1)
    return Fraction(1, CONNECTIVITY_FLOOR_DIVISOR * size)


def cluster_seat_limit(list_size: int) -> Fraction:
    """Return the larger of two seats and ten percent of N, without rounding."""

    size = require_int(list_size, "list_size", minimum=1)
    return max(Fraction(MIN_CLUSTER_SEATS, 1), CLUSTER_SEAT_FRACTION * size)


def _canonical_value(value: Any) -> Any:
    if isinstance(value, Fraction):
        return fraction_document(value)
    if isinstance(value, datetime):
        return format_utc_timestamp(value)
    if value is None or isinstance(value, (str, bool, int)):
        return value
    if isinstance(value, float):
        raise TaskNodeUnlError("floating_point_forbidden")
    if isinstance(value, Mapping):
        if any(not isinstance(key, str) for key in value):
            raise TaskNodeUnlError("invalid_field_name", "canonical_json")
        return {key: _canonical_value(value[key]) for key in sorted(value)}
    if isinstance(value, (list, tuple)):
        return [_canonical_value(item) for item in value]
    raise TaskNodeUnlError("unsupported_canonical_type", type(value).__name__)


def canonical_json_bytes(value: Any) -> bytes:
    """Encode canonical, newline-terminated UTF-8 JSON."""

    normalized = _canonical_value(value)
    return (
        json.dumps(
            normalized,
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        )
        + "\n"
    ).encode("utf-8")
