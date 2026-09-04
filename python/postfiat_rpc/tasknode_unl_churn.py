"""Pure churn, registry-freshness, and overlap guard for shadow UNL deltas.

The guard consumes explicit registry views, a candidate list, the existing
trust-graph transition budget, and identity-hold history. It performs no file,
clock, database, registry, or network access and has no live effect.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timedelta
from fractions import Fraction
from typing import Any, Mapping, Sequence

from .tasknode_unl_schema import (
    ACCOUNTABILITY_WINDOW_DAYS,
    CHURN_BASELINE_SCHEMA,
    CHURN_PROPOSAL_SCHEMA,
    CHURN_REGISTRY_HISTORY_SCHEMA,
    CHURN_RESULT_SCHEMA,
    MAX_CHANGES_BELOW_VALIDATOR_THRESHOLD,
    SHADOW_MODE,
    SINGLE_CHANGE_UNTIL_VALIDATOR_COUNT,
    TaskNodeUnlError,
    canonical_json_bytes,
    format_utc_timestamp,
    fraction_document,
    parse_utc_timestamp,
    require_closed_keys,
    require_identifier,
    require_int,
)

_SHA256_BYTES = 32
_MAX_VALIDATORS = 16_384
_MAX_HISTORY_ROUNDS = 4_096
_MAX_IDENTITY_FAILURES = 16_384
_MAX_IDENTIFIER_BYTES = 128
_MAX_ROUND = (1 << 63) - 1
_IDENTITY_CAUSES = ("revoked_binding", "new_control_group")
_REMOVAL_CAUSES = (*_IDENTITY_CAUSES, "other")
_MAX_SOURCE_ROOT_AGE_ROUNDS = 1
_MICROSECONDS_PER_DAY = 86_400 * 1_000_000


@dataclass(frozen=True)
class RegistryView:
    """One immutable registry view at a numbered round."""

    round: int
    root: str
    validator_ids: tuple[str, ...]


@dataclass(frozen=True)
class IdentityFailure:
    """The first hold instant for one identity-derived failure."""

    validator_id: str
    reason: str
    hold_started_at: datetime


@dataclass(frozen=True)
class RemovalCause:
    """The declared cause for one proposed removal."""

    validator_id: str
    cause: str


@dataclass(frozen=True, order=True)
class GuardReason:
    """One stable, field-addressed reason that rejects a proposal."""

    code: str
    field: str
    detail: str = ""

    def to_dict(self) -> dict[str, str]:
        return {
            "code": self.code,
            "field": self.field,
            "detail": self.detail,
        }


@dataclass(frozen=True)
class RuleApplication:
    """Named rule result with all decision-relevant observed values."""

    rule: str
    verdict: str
    values: Mapping[str, Any]
    reasons: tuple[GuardReason, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "rule": self.rule,
            "verdict": self.verdict,
            "values": dict(self.values),
            "reasons": [reason.to_dict() for reason in self.reasons],
        }


@dataclass(frozen=True)
class OverlapMetric:
    """Exact intersection-over-union for one lagged registry view."""

    lag_rounds: int
    view_round: int
    intersection_count: int
    union_count: int
    ratio: Fraction

    def to_dict(self) -> dict[str, Any]:
        return {
            "lag_rounds": self.lag_rounds,
            "view_round": self.view_round,
            "intersection_count": self.intersection_count,
            "union_count": self.union_count,
            "ratio": fraction_document(self.ratio),
            "percentage": fraction_document(self.ratio * 100),
            "percentage_text": _percentage_text(self.ratio),
        }


@dataclass(frozen=True)
class ChurnGuardVerdict:
    """Structured verdict for later rendering by the shadow runner."""

    status: str
    baseline_round: int
    baseline_root: str
    baseline_validator_ids: tuple[str, ...]
    source_round: int
    source_root: str
    current_round: int
    current_root: str
    current_validator_ids: tuple[str, ...]
    target_round: int
    proposed_validator_ids: tuple[str, ...]
    additions: tuple[str, ...]
    removals: tuple[str, ...]
    one_round_overlap: OverlapMetric
    two_round_overlap: OverlapMetric
    rules: tuple[RuleApplication, ...]
    reasons: tuple[GuardReason, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": CHURN_RESULT_SCHEMA,
            "mode": SHADOW_MODE,
            "status": self.status,
            "registry": {
                "baseline_round": self.baseline_round,
                "baseline_root": self.baseline_root,
                "baseline_validator_ids": list(
                    self.baseline_validator_ids
                ),
                "source_round": self.source_round,
                "source_root": self.source_root,
                "current_round": self.current_round,
                "current_root": self.current_root,
                "current_validator_ids": list(
                    self.current_validator_ids
                ),
                "target_round": self.target_round,
            },
            "delta": {
                "proposed_validator_ids": list(
                    self.proposed_validator_ids
                ),
                "additions": list(self.additions),
                "removals": list(self.removals),
                "change_count": len(self.additions) + len(self.removals),
            },
            "overlap": {
                "one_round_behind": self.one_round_overlap.to_dict(),
                "two_rounds_behind": self.two_round_overlap.to_dict(),
            },
            "rules": [rule.to_dict() for rule in self.rules],
            "reasons": [reason.to_dict() for reason in self.reasons],
        }

    def canonical_bytes(self) -> bytes:
        return canonical_json_bytes(self.to_dict())


@dataclass(frozen=True)
class _Proposal:
    source_round: int
    source_root: str
    target_round: int
    proposed_validator_ids: tuple[str, ...]
    transition_budget: int
    evaluation_time: datetime
    identity_failures: tuple[IdentityFailure, ...]
    removal_causes: tuple[RemovalCause, ...]


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


def _require_round(value: object, field: str) -> int:
    round_number = require_int(value, field, minimum=0)
    if round_number > _MAX_ROUND:
        raise TaskNodeUnlError("round_out_of_range", field)
    return round_number


def _require_transition_budget(value: object, field: str) -> int:
    budget = require_int(value, field, minimum=0)
    if budget > _MAX_VALIDATORS:
        raise TaskNodeUnlError("transition_budget_out_of_range", field)
    return budget


def _canonical_timestamp(value: object, field: str) -> datetime:
    parsed = parse_utc_timestamp(value, field)
    if format_utc_timestamp(parsed) != value:
        raise TaskNodeUnlError("non_canonical_timestamp", field)
    return parsed


def _identifier_list(
    value: object,
    field: str,
    *,
    allow_empty: bool,
) -> tuple[str, ...]:
    if not isinstance(value, list):
        raise TaskNodeUnlError("invalid_array", field)
    if len(value) > _MAX_VALIDATORS:
        raise TaskNodeUnlError("array_too_large", field)
    checked = sorted(
        _require_identifier(item, f"{field}[{index}]")
        for index, item in enumerate(value)
    )
    duplicate = next(
        (
            checked[index]
            for index in range(1, len(checked))
            if checked[index] == checked[index - 1]
        ),
        None,
    )
    if duplicate is not None:
        raise TaskNodeUnlError(
            "duplicate_validator_id", f"{field}.{duplicate}"
        )
    if not checked and not allow_empty:
        raise TaskNodeUnlError("empty_validator_list", field)
    return tuple(checked)


def _baseline_from_document(document: object) -> RegistryView:
    row = require_closed_keys(
        document,
        required=(
            "schema",
            "mode",
            "registry_round",
            "registry_root",
            "validator_ids",
        ),
        field="baseline",
    )
    if row["schema"] != CHURN_BASELINE_SCHEMA:
        raise TaskNodeUnlError("unknown_schema", "baseline.schema")
    if row["mode"] != SHADOW_MODE:
        raise TaskNodeUnlError("mode_mismatch", "baseline.mode")
    return RegistryView(
        round=_require_round(
            row["registry_round"], "baseline.registry_round"
        ),
        root=_require_lower_hex(
            row["registry_root"],
            "baseline.registry_root",
            byte_length=_SHA256_BYTES,
        ),
        validator_ids=_identifier_list(
            row["validator_ids"],
            "baseline.validator_ids",
            allow_empty=False,
        ),
    )


def _registry_view(value: object, index: int) -> RegistryView:
    field = f"registry_history.rounds[{index}]"
    row = require_closed_keys(
        value,
        required=("round", "root", "validator_ids"),
        field=field,
    )
    return RegistryView(
        round=_require_round(row["round"], f"{field}.round"),
        root=_require_lower_hex(
            row["root"], f"{field}.root", byte_length=_SHA256_BYTES
        ),
        validator_ids=_identifier_list(
            row["validator_ids"],
            f"{field}.validator_ids",
            allow_empty=False,
        ),
    )


def _history_from_document(
    document: object,
) -> tuple[int, str, Mapping[int, RegistryView]]:
    row = require_closed_keys(
        document,
        required=(
            "schema",
            "mode",
            "current_round",
            "current_root",
            "rounds",
        ),
        field="registry_history",
    )
    if row["schema"] != CHURN_REGISTRY_HISTORY_SCHEMA:
        raise TaskNodeUnlError(
            "unknown_schema", "registry_history.schema"
        )
    if row["mode"] != SHADOW_MODE:
        raise TaskNodeUnlError(
            "mode_mismatch", "registry_history.mode"
        )
    current_round = _require_round(
        row["current_round"], "registry_history.current_round"
    )
    if current_round < 1:
        raise TaskNodeUnlError(
            "insufficient_registry_history",
            "registry_history.current_round",
        )
    current_root = _require_lower_hex(
        row["current_root"],
        "registry_history.current_root",
        byte_length=_SHA256_BYTES,
    )
    values = row["rounds"]
    if not isinstance(values, list):
        raise TaskNodeUnlError(
            "invalid_array", "registry_history.rounds"
        )
    if len(values) > _MAX_HISTORY_ROUNDS:
        raise TaskNodeUnlError(
            "array_too_large", "registry_history.rounds"
        )
    views: dict[int, RegistryView] = {}
    for index, value in enumerate(values):
        view = _registry_view(value, index)
        previous = views.get(view.round)
        if previous is not None and previous != view:
            raise TaskNodeUnlError(
                "conflicting_registry_round", str(view.round)
            )
        views[view.round] = view
    current = views.get(current_round)
    if current is None:
        raise TaskNodeUnlError(
            "missing_current_registry_round",
            "registry_history.rounds",
        )
    if current.root != current_root:
        raise TaskNodeUnlError(
            "current_registry_root_mismatch",
            "registry_history.current_root",
        )
    if current_round - 1 not in views:
        raise TaskNodeUnlError(
            "missing_two_round_lag_view",
            "registry_history.rounds",
        )
    return current_round, current_root, views


def _identity_failure(
    value: object,
    index: int,
) -> IdentityFailure:
    field = f"proposal.identity_failures[{index}]"
    row = require_closed_keys(
        value,
        required=("validator_id", "reason", "hold_started_at"),
        field=field,
    )
    validator_id = _require_identifier(
        row["validator_id"], f"{field}.validator_id"
    )
    reason = row["reason"]
    if reason not in _IDENTITY_CAUSES:
        raise TaskNodeUnlError(
            "unknown_identity_failure_reason", f"{field}.reason"
        )
    return IdentityFailure(
        validator_id=validator_id,
        reason=reason,
        hold_started_at=_canonical_timestamp(
            row["hold_started_at"], f"{field}.hold_started_at"
        ),
    )


def _removal_cause(value: object, index: int) -> RemovalCause:
    field = f"proposal.removal_causes[{index}]"
    row = require_closed_keys(
        value,
        required=("validator_id", "cause"),
        field=field,
    )
    validator_id = _require_identifier(
        row["validator_id"], f"{field}.validator_id"
    )
    cause = row["cause"]
    if cause not in _REMOVAL_CAUSES:
        raise TaskNodeUnlError(
            "unknown_removal_cause", f"{field}.cause"
        )
    return RemovalCause(validator_id=validator_id, cause=cause)


def _unique_by_validator(
    values: Sequence[IdentityFailure] | Sequence[RemovalCause],
    field: str,
) -> None:
    identifiers = sorted(value.validator_id for value in values)
    duplicate = next(
        (
            identifiers[index]
            for index in range(1, len(identifiers))
            if identifiers[index] == identifiers[index - 1]
        ),
        None,
    )
    if duplicate is not None:
        raise TaskNodeUnlError(
            "duplicate_validator_record", f"{field}.{duplicate}"
        )


def _proposal_from_document(document: object) -> _Proposal:
    row = require_closed_keys(
        document,
        required=(
            "schema",
            "mode",
            "source_round",
            "source_registry_root",
            "target_round",
            "proposed_validator_ids",
            "transition_budget",
            "evaluation_time",
            "identity_failures",
            "removal_causes",
        ),
        field="proposal",
    )
    if row["schema"] != CHURN_PROPOSAL_SCHEMA:
        raise TaskNodeUnlError("unknown_schema", "proposal.schema")
    if row["mode"] != SHADOW_MODE:
        raise TaskNodeUnlError("mode_mismatch", "proposal.mode")
    failures_value = row["identity_failures"]
    causes_value = row["removal_causes"]
    if not isinstance(failures_value, list):
        raise TaskNodeUnlError(
            "invalid_array", "proposal.identity_failures"
        )
    if len(failures_value) > _MAX_IDENTITY_FAILURES:
        raise TaskNodeUnlError(
            "array_too_large", "proposal.identity_failures"
        )
    if not isinstance(causes_value, list):
        raise TaskNodeUnlError(
            "invalid_array", "proposal.removal_causes"
        )
    if len(causes_value) > _MAX_VALIDATORS:
        raise TaskNodeUnlError(
            "array_too_large", "proposal.removal_causes"
        )
    identity_failures = tuple(
        sorted(
            (
                _identity_failure(value, index)
                for index, value in enumerate(failures_value)
            ),
            key=lambda failure: failure.validator_id,
        )
    )
    removal_causes = tuple(
        sorted(
            (
                _removal_cause(value, index)
                for index, value in enumerate(causes_value)
            ),
            key=lambda cause: cause.validator_id,
        )
    )
    _unique_by_validator(
        identity_failures, "proposal.identity_failures"
    )
    _unique_by_validator(removal_causes, "proposal.removal_causes")
    return _Proposal(
        source_round=_require_round(
            row["source_round"], "proposal.source_round"
        ),
        source_root=_require_lower_hex(
            row["source_registry_root"],
            "proposal.source_registry_root",
            byte_length=_SHA256_BYTES,
        ),
        target_round=_require_round(
            row["target_round"], "proposal.target_round"
        ),
        proposed_validator_ids=_identifier_list(
            row["proposed_validator_ids"],
            "proposal.proposed_validator_ids",
            allow_empty=True,
        ),
        transition_budget=_require_transition_budget(
            row["transition_budget"], "proposal.transition_budget"
        ),
        evaluation_time=_canonical_timestamp(
            row["evaluation_time"], "proposal.evaluation_time"
        ),
        identity_failures=identity_failures,
        removal_causes=removal_causes,
    )


def _percentage_text(ratio: Fraction) -> str:
    scaled_numerator = ratio.numerator * 1_000
    whole, remainder = divmod(scaled_numerator, ratio.denominator)
    if remainder * 2 >= ratio.denominator:
        whole += 1
    return f"{whole // 10}.{whole % 10}%"


def _overlap(
    view: RegistryView,
    target: Sequence[str],
    lag_rounds: int,
) -> OverlapMetric:
    view_set = frozenset(view.validator_ids)
    target_set = frozenset(target)
    intersection = len(view_set & target_set)
    union = len(view_set | target_set)
    if union == 0:
        raise TaskNodeUnlError("empty_overlap_union")
    return OverlapMetric(
        lag_rounds=lag_rounds,
        view_round=view.round,
        intersection_count=intersection,
        union_count=union,
        ratio=Fraction(intersection, union),
    )


def _rule(
    name: str,
    values: Mapping[str, Any],
    reasons: Sequence[GuardReason] = (),
    *,
    reported: bool = False,
) -> RuleApplication:
    ordered_reasons = tuple(sorted(set(reasons)))
    verdict = (
        "reported"
        if reported
        else ("reject" if ordered_reasons else "pass")
    )
    return RuleApplication(
        rule=name,
        verdict=verdict,
        values=values,
        reasons=ordered_reasons,
    )


def _registry_rule(
    baseline: RegistryView,
    current_round: int,
    current_root: str,
    views: Mapping[int, RegistryView],
    proposal: _Proposal,
) -> RuleApplication:
    reasons: list[GuardReason] = []
    source_age = current_round - proposal.source_round
    source_view = views.get(proposal.source_round)
    if proposal.source_round != baseline.round:
        reasons.append(
            GuardReason(
                "baseline_round_mismatch",
                "proposal.source_round",
                f"baseline_round={baseline.round}",
            )
        )
    if proposal.source_root != baseline.root:
        reasons.append(
            GuardReason(
                "baseline_root_mismatch",
                "proposal.source_registry_root",
                f"baseline_root={baseline.root}",
            )
        )
    if source_view is None:
        reasons.append(
            GuardReason(
                "source_round_missing_from_history",
                "proposal.source_round",
            )
        )
    else:
        if source_view.root != proposal.source_root:
            reasons.append(
                GuardReason(
                    "source_root_history_mismatch",
                    "proposal.source_registry_root",
                    f"history_root={source_view.root}",
                )
            )
        if source_view.validator_ids != baseline.validator_ids:
            reasons.append(
                GuardReason(
                    "baseline_list_history_mismatch",
                    "baseline.validator_ids",
                )
            )
    if source_age < 0:
        reasons.append(
            GuardReason(
                "source_round_in_future",
                "proposal.source_round",
                f"source_age_rounds={source_age}",
            )
        )
    elif source_age > _MAX_SOURCE_ROOT_AGE_ROUNDS:
        reasons.append(
            GuardReason(
                "stale_registry_root",
                "proposal.source_registry_root",
                f"source_age_rounds={source_age}",
            )
        )
    if proposal.target_round != current_round + 1:
        reasons.append(
            GuardReason(
                "target_round_mismatch",
                "proposal.target_round",
                f"expected={current_round + 1}",
            )
        )
    return _rule(
        "registry_root_freshness_and_round_binding",
        {
            "baseline_round": baseline.round,
            "baseline_root": baseline.root,
            "source_round": proposal.source_round,
            "source_registry_root": proposal.source_root,
            "current_round": current_round,
            "current_root": current_root,
            "source_age_rounds": source_age,
            "maximum_source_age_rounds": (
                _MAX_SOURCE_ROOT_AGE_ROUNDS
            ),
            "target_round": proposal.target_round,
            "expected_target_round": current_round + 1,
        },
        reasons,
    )


def _delta_rule(
    current: RegistryView,
    proposed: Sequence[str],
) -> tuple[
    tuple[str, ...],
    tuple[str, ...],
    RuleApplication,
]:
    current_set = frozenset(current.validator_ids)
    proposed_set = frozenset(proposed)
    additions = tuple(sorted(proposed_set - current_set))
    removals = tuple(sorted(current_set - proposed_set))
    return (
        additions,
        removals,
        _rule(
            "canonical_delta_derivation",
            {
                "current_validator_count": len(current.validator_ids),
                "proposed_validator_count": len(proposed),
                "addition_count": len(additions),
                "removal_count": len(removals),
                "change_count": len(additions) + len(removals),
                "additions": list(additions),
                "removals": list(removals),
            },
        ),
    )


def _churn_rule(
    current_validator_count: int,
    additions: Sequence[str],
    removals: Sequence[str],
    transition_budget: int,
) -> RuleApplication:
    change_count = len(additions) + len(removals)
    below_threshold = (
        current_validator_count
        < SINGLE_CHANGE_UNTIL_VALIDATOR_COUNT
    )
    effective_budget = (
        min(
            MAX_CHANGES_BELOW_VALIDATOR_THRESHOLD,
            transition_budget,
        )
        if below_threshold
        else transition_budget
    )
    reasons: list[GuardReason] = []
    if below_threshold and additions and removals:
        reasons.append(
            GuardReason(
                "pre_39_mixed_change_forbidden",
                "proposal.proposed_validator_ids",
                (
                    f"additions={len(additions)},"
                    f"removals={len(removals)}"
                ),
            )
        )
    if (
        below_threshold
        and change_count > MAX_CHANGES_BELOW_VALIDATOR_THRESHOLD
    ):
        reasons.append(
            GuardReason(
                "pre_39_single_change_exceeded",
                "proposal.proposed_validator_ids",
                f"change_count={change_count}",
            )
        )
    if change_count > transition_budget:
        reasons.append(
            GuardReason(
                "transition_budget_exceeded",
                "proposal.transition_budget",
                (
                    f"change_count={change_count},"
                    f"transition_budget={transition_budget}"
                ),
            )
        )
    regime = (
        "pre_39_single_change"
        if below_threshold
        else "supplied_trust_graph_transition_budget"
    )
    return _rule(
        "safe_churn_budget",
        {
            "current_validator_count": current_validator_count,
            "single_change_threshold": (
                SINGLE_CHANGE_UNTIL_VALIDATOR_COUNT
            ),
            "regime": regime,
            "supplied_transition_budget": transition_budget,
            "effective_change_budget": effective_budget,
            "addition_count": len(additions),
            "removal_count": len(removals),
            "change_count": change_count,
            "permitted_pre_39_shapes": [
                "no_change",
                "single_addition",
                "single_removal",
            ],
        },
        reasons,
    )


def _elapsed_microseconds(start: datetime, end: datetime) -> int:
    delta = end - start
    return (
        (delta.days * 86_400 + delta.seconds) * 1_000_000
        + delta.microseconds
    )


def _identity_rule(
    current: RegistryView,
    removals: Sequence[str],
    proposal: _Proposal,
) -> RuleApplication:
    reasons: list[GuardReason] = []
    removal_set = frozenset(removals)
    current_set = frozenset(current.validator_ids)
    causes = {
        cause.validator_id: cause.cause
        for cause in proposal.removal_causes
    }
    failures = {
        failure.validator_id: failure
        for failure in proposal.identity_failures
    }

    for validator_id in sorted(set(causes) - removal_set):
        reasons.append(
            GuardReason(
                "removal_cause_without_removal",
                f"proposal.removal_causes.{validator_id}",
            )
        )
    for validator_id in removals:
        if validator_id not in causes:
            reasons.append(
                GuardReason(
                    "missing_removal_cause",
                    f"proposal.removal_causes.{validator_id}",
                )
            )
    for validator_id in sorted(set(failures) - current_set):
        reasons.append(
            GuardReason(
                "identity_failure_unknown_validator",
                f"proposal.identity_failures.{validator_id}",
            )
        )

    states: list[dict[str, Any]] = []
    required_microseconds = (
        ACCOUNTABILITY_WINDOW_DAYS * _MICROSECONDS_PER_DAY
    )
    for validator_id, failure in sorted(failures.items()):
        elapsed = _elapsed_microseconds(
            failure.hold_started_at, proposal.evaluation_time
        )
        if elapsed < 0:
            reasons.append(
                GuardReason(
                    "identity_hold_starts_in_future",
                    (
                        "proposal.identity_failures."
                        f"{validator_id}.hold_started_at"
                    ),
                )
            )
        mature = elapsed >= required_microseconds
        removal_requested = validator_id in removal_set
        stage = "removal_candidate" if mature else "hold"
        states.append(
            {
                "validator_id": validator_id,
                "reason": failure.reason,
                "hold_started_at": format_utc_timestamp(
                    failure.hold_started_at
                ),
                "evaluation_time": format_utc_timestamp(
                    proposal.evaluation_time
                ),
                "elapsed_microseconds": elapsed,
                "required_window_days": ACCOUNTABILITY_WINDOW_DAYS,
                "stage": stage,
                "removal_requested": (
                    "yes" if removal_requested else "no"
                ),
            }
        )
        cause = causes.get(validator_id)
        if removal_requested:
            if cause != failure.reason:
                reasons.append(
                    GuardReason(
                        "identity_removal_cause_mismatch",
                        f"proposal.removal_causes.{validator_id}",
                        (
                            f"cause={cause},"
                            f"failure_reason={failure.reason}"
                        ),
                    )
                )
            if not mature:
                reasons.append(
                    GuardReason(
                        "identity_hold_window_not_elapsed",
                        (
                            "proposal.identity_failures."
                            f"{validator_id}.hold_started_at"
                        ),
                        (
                            f"elapsed_microseconds={elapsed},"
                            f"required_microseconds="
                            f"{required_microseconds}"
                        ),
                    )
                )

    for validator_id in removals:
        cause = causes.get(validator_id)
        if cause in _IDENTITY_CAUSES and validator_id not in failures:
            reasons.append(
                GuardReason(
                    "identity_hold_missing",
                    f"proposal.identity_failures.{validator_id}",
                    f"cause={cause}",
                )
            )

    return _rule(
        "identity_hold_before_removal",
        {
            "evaluation_window_days": ACCOUNTABILITY_WINDOW_DAYS,
            "identity_removal_causes": list(_IDENTITY_CAUSES),
            "removal_count": len(removals),
            "identity_states": states,
        },
        reasons,
    )


def evaluate_churn_guard(
    baseline_document: object,
    registry_history_document: object,
    proposal_document: object,
) -> ChurnGuardVerdict:
    """Evaluate one candidate delta against freshness, churn, and hold rules."""

    baseline = _baseline_from_document(baseline_document)
    current_round, current_root, views = _history_from_document(
        registry_history_document
    )
    proposal = _proposal_from_document(proposal_document)

    current_view = views[current_round]
    additions, removals, delta_rule = _delta_rule(
        current_view, proposal.proposed_validator_ids
    )
    previous_view = views[current_round - 1]
    one_round = _overlap(
        current_view, proposal.proposed_validator_ids, 1
    )
    two_round = _overlap(
        previous_view, proposal.proposed_validator_ids, 2
    )

    rules = (
        _registry_rule(
            baseline,
            current_round,
            current_root,
            views,
            proposal,
        ),
        delta_rule,
        _churn_rule(
            len(current_view.validator_ids),
            additions,
            removals,
            proposal.transition_budget,
        ),
        _identity_rule(current_view, removals, proposal),
        _rule(
            "intersection_over_union_overlap_reporting",
            {
                "formula": "intersection_count/union_count",
                "one_round_behind": one_round.to_dict(),
                "two_rounds_behind": two_round.to_dict(),
            },
            reported=True,
        ),
    )
    reasons = tuple(
        sorted(
            {
                reason
                for rule in rules
                for reason in rule.reasons
            }
        )
    )
    return ChurnGuardVerdict(
        status="reject" if reasons else "allow",
        baseline_round=baseline.round,
        baseline_root=baseline.root,
        baseline_validator_ids=baseline.validator_ids,
        source_round=proposal.source_round,
        source_root=proposal.source_root,
        current_round=current_round,
        current_root=current_root,
        current_validator_ids=current_view.validator_ids,
        target_round=proposal.target_round,
        proposed_validator_ids=proposal.proposed_validator_ids,
        additions=additions,
        removals=removals,
        one_round_overlap=one_round,
        two_round_overlap=two_round,
        rules=rules,
        reasons=reasons,
    )
