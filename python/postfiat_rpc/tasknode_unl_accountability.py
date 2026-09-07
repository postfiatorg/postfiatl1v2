"""Pure accountability scoring for the Task Node identity-derived UNL shadow.

The evaluator consumes explicit, bounded-history evidence supplied by its
caller. It reads no clock, files, database, or network.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timedelta
from fractions import Fraction
from typing import Any, Mapping, Sequence

from .tasknode_unl_schema import (
    ACCOUNTABILITY_FLOOR,
    ACCOUNTABILITY_INPUT_SCHEMA,
    ACCOUNTABILITY_RESULT_SCHEMA,
    ACCOUNTABILITY_TENURE_DENOMINATOR_DAYS,
    ACCOUNTABILITY_TERM_WEIGHTS,
    ACCOUNTABILITY_TERMS,
    ACCOUNTABILITY_WINDOW_DAYS,
    ACCOUNTABILITY_WORK_DENOMINATOR,
    SHADOW_MODE,
    TaskNodeUnlError,
    canonical_json_bytes,
    clamp_unit,
    format_utc_timestamp,
    parse_utc_timestamp,
    require_closed_keys,
    require_identifier,
)

_NETWORK_KIND = "network"
_PERSONAL_KIND = "personal"
_TASK_KINDS = (_NETWORK_KIND, _PERSONAL_KIND)
_VERIFICATION_OUTCOMES = ("pass", "fail")
_MICROSECONDS_PER_DAY = 86_400 * 1_000_000


@dataclass(frozen=True)
class TaskEvidence:
    task_id: str
    kind: str
    accepted_at: datetime | None
    verification_outcome: str | None
    verified_at: datetime | None
    rewarded_at: datetime | None


@dataclass(frozen=True)
class DisputeEvidence:
    dispute_id: str
    opened_at: datetime
    resolved_at: datetime | None


@dataclass(frozen=True)
class BadgeEvidence:
    verified: bool
    valid_from: datetime
    expires_at: datetime | None
    revoked_at: datetime | None

    def is_current(self, window_end: datetime) -> bool:
        if not self.verified or self.valid_from > window_end:
            return False
        if self.expires_at is not None and self.expires_at <= window_end:
            return False
        if self.revoked_at is not None and self.revoked_at <= window_end:
            return False
        return True


@dataclass(frozen=True)
class AccountabilityEvidence:
    window_end: datetime
    tasks: tuple[TaskEvidence, ...]
    disputes: tuple[DisputeEvidence, ...] | None
    badge: BadgeEvidence | None


@dataclass(frozen=True)
class ScoreCalculation:
    raw_terms: tuple[tuple[str, Fraction], ...]
    clamped_terms: tuple[tuple[str, Fraction], ...]
    weighted_terms: tuple[tuple[str, Fraction], ...]
    exact_score: Fraction
    projected_score: int

    def to_dict(self) -> dict[str, Any]:
        return {
            "raw_terms": dict(self.raw_terms),
            "clamped_terms": dict(self.clamped_terms),
            "weighted_terms": dict(self.weighted_terms),
            "exact_score": self.exact_score,
            "projected_score": self.projected_score,
            "meets_floor": self.projected_score >= ACCOUNTABILITY_FLOOR,
        }


@dataclass(frozen=True)
class AccountabilityResult:
    window_start: datetime
    window_end: datetime
    status: str
    hold_reasons: tuple[str, ...]
    accepted_network_tasks: int
    verification_passes: int
    verification_total: int
    open_disputes: int | None
    first_rewarded_at: datetime | None
    badge_current: bool | None
    calculation: ScoreCalculation | None

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": ACCOUNTABILITY_RESULT_SCHEMA,
            "mode": SHADOW_MODE,
            "window": {
                "start": format_utc_timestamp(self.window_start),
                "end": format_utc_timestamp(self.window_end),
                "days": ACCOUNTABILITY_WINDOW_DAYS,
            },
            "status": self.status,
            "hold_reasons": list(self.hold_reasons),
            "inputs": {
                "accepted_network_tasks": self.accepted_network_tasks,
                "verification_passes": self.verification_passes,
                "verification_total": self.verification_total,
                "open_disputes": self.open_disputes,
                "first_rewarded_at": (
                    format_utc_timestamp(self.first_rewarded_at)
                    if self.first_rewarded_at is not None
                    else None
                ),
                "badge_current": self.badge_current,
            },
            "calculation": (
                self.calculation.to_dict()
                if self.calculation is not None
                else None
            ),
        }

    def canonical_bytes(self) -> bytes:
        return canonical_json_bytes(self.to_dict())


def _as_fraction(value: object, term: str) -> Fraction:
    if isinstance(value, bool) or not isinstance(value, (int, Fraction)):
        raise TaskNodeUnlError("non_rational_term", term)
    return Fraction(value)


def score_terms(raw_terms: Mapping[str, int | Fraction]) -> ScoreCalculation:
    """Clamp all five exact terms and apply the published weighted sum."""

    keys = set(raw_terms)
    expected = set(ACCOUNTABILITY_TERMS)
    missing = sorted(expected - keys)
    if missing:
        raise TaskNodeUnlError("missing_score_term", missing[0])
    unknown = sorted(keys - expected)
    if unknown:
        raise TaskNodeUnlError("unknown_score_term", unknown[0])

    raw: list[tuple[str, Fraction]] = []
    clamped: list[tuple[str, Fraction]] = []
    weighted: list[tuple[str, Fraction]] = []
    exact_score = Fraction(0, 1)
    for term, weight in ACCOUNTABILITY_TERM_WEIGHTS:
        raw_value = _as_fraction(raw_terms[term], term)
        clamped_value = clamp_unit(raw_value)
        contribution = weight * clamped_value
        raw.append((term, raw_value))
        clamped.append((term, clamped_value))
        weighted.append((term, contribution))
        exact_score += contribution

    return ScoreCalculation(
        raw_terms=tuple(raw),
        clamped_terms=tuple(clamped),
        weighted_terms=tuple(weighted),
        exact_score=exact_score,
        projected_score=exact_score.numerator // exact_score.denominator,
    )


def _fractional_days(start: datetime, end: datetime) -> Fraction:
    delta = end - start
    microseconds = (
        (delta.days * 86_400 + delta.seconds) * 1_000_000
        + delta.microseconds
    )
    return Fraction(microseconds, _MICROSECONDS_PER_DAY)


def _in_window(value: datetime | None, start: datetime, end: datetime) -> bool:
    return value is not None and start <= value <= end


def _validate_tasks(
    tasks: Sequence[TaskEvidence], window_end: datetime
) -> tuple[TaskEvidence, ...]:
    ordered = sorted(tasks, key=lambda task: task.task_id)
    seen: set[str] = set()
    for task in ordered:
        require_identifier(task.task_id, "tasks.task_id")
        if task.task_id in seen:
            raise TaskNodeUnlError("duplicate_task_id", task.task_id)
        seen.add(task.task_id)
        if task.kind not in _TASK_KINDS:
            raise TaskNodeUnlError("unknown_task_kind", task.kind)
        if (task.verification_outcome is None) != (task.verified_at is None):
            raise TaskNodeUnlError("incomplete_verification", task.task_id)
        if (
            task.verification_outcome is not None
            and task.verification_outcome not in _VERIFICATION_OUTCOMES
        ):
            raise TaskNodeUnlError(
                "unknown_verification_outcome", task.task_id
            )
        for field_name, timestamp in (
            ("accepted_at", task.accepted_at),
            ("verified_at", task.verified_at),
            ("rewarded_at", task.rewarded_at),
        ):
            if timestamp is not None and timestamp > window_end:
                raise TaskNodeUnlError(
                    "future_task_event", f"{task.task_id}.{field_name}"
                )
        if (
            task.accepted_at is not None
            and task.verified_at is not None
            and task.verified_at < task.accepted_at
        ):
            raise TaskNodeUnlError(
                "verification_before_acceptance", task.task_id
            )
        if (
            task.accepted_at is not None
            and task.rewarded_at is not None
            and task.rewarded_at < task.accepted_at
        ):
            raise TaskNodeUnlError(
                "reward_before_acceptance", task.task_id
            )
        if (
            task.verified_at is not None
            and task.rewarded_at is not None
            and task.rewarded_at < task.verified_at
        ):
            raise TaskNodeUnlError(
                "reward_before_verification", task.task_id
            )
    return tuple(ordered)


def _validate_disputes(
    disputes: Sequence[DisputeEvidence], window_end: datetime
) -> tuple[DisputeEvidence, ...]:
    ordered = sorted(disputes, key=lambda dispute: dispute.dispute_id)
    seen: set[str] = set()
    for dispute in ordered:
        require_identifier(dispute.dispute_id, "disputes.dispute_id")
        if dispute.dispute_id in seen:
            raise TaskNodeUnlError("duplicate_dispute_id", dispute.dispute_id)
        seen.add(dispute.dispute_id)
        if dispute.opened_at > window_end:
            raise TaskNodeUnlError("future_dispute_event", dispute.dispute_id)
        if (
            dispute.resolved_at is not None
            and dispute.resolved_at < dispute.opened_at
        ):
            raise TaskNodeUnlError(
                "dispute_resolved_before_opened", dispute.dispute_id
            )
        if dispute.resolved_at is not None and dispute.resolved_at > window_end:
            raise TaskNodeUnlError("future_dispute_event", dispute.dispute_id)
    return tuple(ordered)


def evaluate_accountability(
    evidence: AccountabilityEvidence,
) -> AccountabilityResult:
    """Derive and score accountability evidence for one explicit window."""

    window_end = evidence.window_end
    if window_end.tzinfo is None or window_end.utcoffset() is None:
        raise TaskNodeUnlError("timestamp_missing_timezone", "window_end")
    window_start = window_end - timedelta(days=ACCOUNTABILITY_WINDOW_DAYS)
    tasks = _validate_tasks(evidence.tasks, window_end)

    accepted_network_tasks = sum(
        1
        for task in tasks
        if task.kind == _NETWORK_KIND
        and _in_window(task.accepted_at, window_start, window_end)
    )
    verification_events = [
        task
        for task in tasks
        if _in_window(task.verified_at, window_start, window_end)
    ]
    verification_passes = sum(
        task.verification_outcome == "pass" for task in verification_events
    )
    verification_total = len(verification_events)

    rewarded_times = [
        task.rewarded_at for task in tasks if task.rewarded_at is not None
    ]
    first_rewarded_at = min(rewarded_times) if rewarded_times else None

    hold_reasons: list[str] = []
    if first_rewarded_at is None:
        hold_reasons.append("missing_first_rewarded_task")
    if verification_total == 0:
        hold_reasons.append("missing_verification_denominator")

    open_disputes: int | None = None
    if evidence.disputes is None:
        hold_reasons.append("missing_dispute_state")
    else:
        disputes = _validate_disputes(evidence.disputes, window_end)
        open_disputes = sum(
            dispute.resolved_at is None for dispute in disputes
        )

    badge_current: bool | None = None
    if evidence.badge is None:
        hold_reasons.append("missing_badge_state")
    else:
        badge_current = evidence.badge.is_current(window_end)

    if hold_reasons:
        return AccountabilityResult(
            window_start=window_start,
            window_end=window_end,
            status="hold",
            hold_reasons=tuple(sorted(hold_reasons)),
            accepted_network_tasks=accepted_network_tasks,
            verification_passes=verification_passes,
            verification_total=verification_total,
            open_disputes=open_disputes,
            first_rewarded_at=first_rewarded_at,
            badge_current=badge_current,
            calculation=None,
        )

    assert first_rewarded_at is not None
    assert open_disputes is not None
    assert badge_current is not None
    raw_terms = {
        "work": Fraction(
            accepted_network_tasks, ACCOUNTABILITY_WORK_DENOMINATOR
        ),
        "tenure": _fractional_days(
            first_rewarded_at, window_end
        )
        / ACCOUNTABILITY_TENURE_DENOMINATOR_DAYS,
        "quality": Fraction(verification_passes, verification_total),
        "standing": Fraction(1, 1) - Fraction(open_disputes, 3),
        "badge": Fraction(int(badge_current), 1),
    }
    return AccountabilityResult(
        window_start=window_start,
        window_end=window_end,
        status="scored",
        hold_reasons=(),
        accepted_network_tasks=accepted_network_tasks,
        verification_passes=verification_passes,
        verification_total=verification_total,
        open_disputes=open_disputes,
        first_rewarded_at=first_rewarded_at,
        badge_current=badge_current,
        calculation=score_terms(raw_terms),
    )


def _optional_timestamp(value: object, field: str) -> datetime | None:
    return None if value is None else parse_utc_timestamp(value, field)


def _task_from_dict(value: object, index: int) -> TaskEvidence:
    field = f"tasks[{index}]"
    row = require_closed_keys(
        value,
        required=(
            "task_id",
            "kind",
            "accepted_at",
            "verification_outcome",
            "verified_at",
            "rewarded_at",
        ),
        field=field,
    )
    outcome = row["verification_outcome"]
    if outcome is not None and not isinstance(outcome, str):
        raise TaskNodeUnlError("invalid_verification_outcome", field)
    kind = row["kind"]
    if not isinstance(kind, str):
        raise TaskNodeUnlError("unknown_task_kind", field)
    return TaskEvidence(
        task_id=require_identifier(row["task_id"], f"{field}.task_id"),
        kind=kind,
        accepted_at=_optional_timestamp(
            row["accepted_at"], f"{field}.accepted_at"
        ),
        verification_outcome=outcome,
        verified_at=_optional_timestamp(
            row["verified_at"], f"{field}.verified_at"
        ),
        rewarded_at=_optional_timestamp(
            row["rewarded_at"], f"{field}.rewarded_at"
        ),
    )


def _dispute_from_dict(value: object, index: int) -> DisputeEvidence:
    field = f"disputes[{index}]"
    row = require_closed_keys(
        value,
        required=("dispute_id", "opened_at", "resolved_at"),
        field=field,
    )
    return DisputeEvidence(
        dispute_id=require_identifier(
            row["dispute_id"], f"{field}.dispute_id"
        ),
        opened_at=parse_utc_timestamp(
            row["opened_at"], f"{field}.opened_at"
        ),
        resolved_at=_optional_timestamp(
            row["resolved_at"], f"{field}.resolved_at"
        ),
    )


def _badge_from_dict(value: object) -> BadgeEvidence:
    row = require_closed_keys(
        value,
        required=("verified", "valid_from", "expires_at", "revoked_at"),
        field="badge",
    )
    verified = row["verified"]
    if not isinstance(verified, bool):
        raise TaskNodeUnlError("invalid_boolean", "badge.verified")
    valid_from = parse_utc_timestamp(row["valid_from"], "badge.valid_from")
    expires_at = _optional_timestamp(row["expires_at"], "badge.expires_at")
    revoked_at = _optional_timestamp(row["revoked_at"], "badge.revoked_at")
    if expires_at is not None and expires_at <= valid_from:
        raise TaskNodeUnlError("badge_expiry_not_after_start")
    if revoked_at is not None and revoked_at < valid_from:
        raise TaskNodeUnlError("badge_revoked_before_start")
    return BadgeEvidence(
        verified=verified,
        valid_from=valid_from,
        expires_at=expires_at,
        revoked_at=revoked_at,
    )


def accountability_evidence_from_dict(
    document: object,
) -> AccountabilityEvidence:
    """Parse the closed fixture/input schema into immutable evidence."""

    row = require_closed_keys(
        document,
        required=("schema", "window_end", "tasks", "disputes", "badge"),
        field="accountability",
    )
    if row["schema"] != ACCOUNTABILITY_INPUT_SCHEMA:
        raise TaskNodeUnlError("unknown_schema", str(row["schema"]))
    tasks_value = row["tasks"]
    if not isinstance(tasks_value, list):
        raise TaskNodeUnlError("invalid_array", "tasks")
    disputes_value = row["disputes"]
    if disputes_value is not None and not isinstance(disputes_value, list):
        raise TaskNodeUnlError("invalid_array", "disputes")
    badge_value = row["badge"]
    return AccountabilityEvidence(
        window_end=parse_utc_timestamp(row["window_end"], "window_end"),
        tasks=tuple(
            _task_from_dict(task, index)
            for index, task in enumerate(tasks_value)
        ),
        disputes=(
            tuple(
                _dispute_from_dict(dispute, index)
                for index, dispute in enumerate(disputes_value)
            )
            if disputes_value is not None
            else None
        ),
        badge=(
            _badge_from_dict(badge_value)
            if badge_value is not None
            else None
        ),
    )


def evaluate_accountability_document(document: object) -> AccountabilityResult:
    """Parse and score one explicit JSON-shaped input document."""

    return evaluate_accountability(accountability_evidence_from_dict(document))
