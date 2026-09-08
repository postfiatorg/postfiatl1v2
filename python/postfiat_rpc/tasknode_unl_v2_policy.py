"""Deterministic UNL V2 graph and shadow admission state.

This module consumes the verified section-A evidence result.  It is deliberately
pure and SHADOW_ONLY: it performs no I/O, never mutates a live registry, and
emits only candidates for the existing authorized churn path.  Funding
observations are not accepted by any graph or admission API.
"""

from __future__ import annotations

import hashlib
from dataclasses import dataclass, replace
from fractions import Fraction
from typing import Any, Mapping, Sequence

from .tasknode_unl_schema import (
    SHADOW_MODE,
    TaskNodeUnlError,
    canonical_json_bytes,
)
from .tasknode_unl_v2_evidence import (
    ActiveControlDeclaration,
    ActiveRelation,
    ContinuityAssessment,
    EvidenceSnapshotResult,
)
from .tasknode_unl_v2_schema import (
    EvaluationWindow,
    V2_POLICY_ID,
    V2_VERSION,
    ValidationFailure,
    require_bounded_identifier,
    require_lower_hex,
)

V2_ADMISSION_POLICY_ID = "tasknode-unl-admission-v2"
V2_ADMISSION_POLICY_SCHEMA = "tasknode-unl-v2-admission-policy-v2"
V2_FROZEN_WINDOW_SCHEMA = "tasknode-unl-v2-frozen-admission-window-v2"
V2_ADMISSION_REPORT_SCHEMA = "tasknode-unl-v2-admission-report-v2"

VOUCH_WEIGHT = Fraction(1, 1)
COWORK_WEIGHT = Fraction(1, 1)
COWORK_CREDIT_CAP = 3
WALK_DAMPING = Fraction(17, 20)
SEED_DAMPING = Fraction(3, 20)
WALK_STEPS = 20
CONDUCTANCE_THRESHOLD = Fraction(1, 10)
CONNECTIVITY_DIVISOR = 2
MIN_SOCIAL_SEATS = 2
SOCIAL_SEAT_FRACTION = Fraction(1, 10)
CONTROL_GROUP_SEAT_LIMIT = Fraction(1, 1)
SINGLE_CHANGE_UNTIL_N = 39
PRE_THRESHOLD_CHANGE_LIMIT = 1
OLD_ROOT_OVERLAP_ROUNDS = 1
FULL_WINDOW_REVIEW_WINDOWS = 1

POLICY_ROOT_DOMAIN = "postfiat/tasknode-unl-v2/admission-policy-root/v2"
WINDOW_ROOT_DOMAIN = "postfiat/tasknode-unl-v2/admission-window-root/v2"
GRAPH_ROOT_DOMAIN = "postfiat/tasknode-unl-v2/graph-root/v2"
DECLARATION_ROOT_DOMAIN = "postfiat/tasknode-unl-v2/declaration-root/v2"
FROZEN_ROOT_DOMAIN = "postfiat/tasknode-unl-v2/frozen-root/v2"
LIMIT_ID_DOMAIN = "postfiat/tasknode-unl-v2/limit-id/v2"
REPORT_ROOT_DOMAIN = "postfiat/tasknode-unl-v2/admission-report-root/v2"

NO_PROPOSAL = "NO_PROPOSAL"
FROZEN = "FROZEN"
PROPOSAL = "PROPOSAL"
PROPOSE_ADD = "PROPOSE_ADD"
HOLD = "HOLD"
CLEAR = "CLEAR"
SATURATED = "SATURATED"
EXISTING_BREACH = "EXISTING_BREACH"


def _header(schema: str) -> dict[str, Any]:
    return {
        "schema": schema,
        "version": V2_VERSION,
        "mode": SHADOW_MODE,
        "policy_id": V2_ADMISSION_POLICY_ID,
    }


def _domain_hash(domain: str, value: object) -> str:
    return hashlib.sha256(
        domain.encode("utf-8") + b"\x00" + canonical_json_bytes(value)
    ).hexdigest()


def _failure(error: TaskNodeUnlError, fallback: str) -> ValidationFailure:
    field = error.detail or fallback
    return ValidationFailure(field=field, code=error.code, detail="")


def social_seat_limit(list_size: int) -> Fraction:
    """Return exact max(2, N/10), retaining a fractional result."""

    if isinstance(list_size, bool) or not isinstance(list_size, int):
        raise TaskNodeUnlError("invalid_integer", "opening_list_size")
    if list_size < 1:
        raise TaskNodeUnlError("integer_below_minimum", "opening_list_size")
    return max(
        Fraction(MIN_SOCIAL_SEATS, 1),
        Fraction(list_size, 1) * SOCIAL_SEAT_FRACTION,
    )


def permitted_integer_seats(limit: Fraction) -> int:
    """Largest integer occupancy that does not exceed an exact rational cap."""

    if not isinstance(limit, Fraction) or limit < 0:
        raise TaskNodeUnlError("invalid_rational_limit", "limit")
    return limit.numerator // limit.denominator


def connectivity_floor(list_size: int) -> Fraction:
    if isinstance(list_size, bool) or not isinstance(list_size, int):
        raise TaskNodeUnlError("invalid_integer", "opening_list_size")
    if list_size < 1:
        raise TaskNodeUnlError("integer_below_minimum", "opening_list_size")
    return Fraction(1, CONNECTIVITY_DIVISOR * list_size)


def admission_policy_document() -> dict[str, Any]:
    """Closed policy payload whose root binds every locked numerical rule."""

    return {
        **_header(V2_ADMISSION_POLICY_SCHEMA),
        "evidence_policy_id": V2_POLICY_ID,
        "graph_inputs": ["accepted_bilateral_vouch", "accepted_bilateral_cowork"],
        "funding_role": "AUDIT_ONLY_NO_MASS_NO_VETO",
        "constants": {
            "vouch_weight": VOUCH_WEIGHT,
            "cowork_weight": COWORK_WEIGHT,
            "cowork_distinct_credit_cap": COWORK_CREDIT_CAP,
            "walk_damping": WALK_DAMPING,
            "seed_damping": SEED_DAMPING,
            "walk_steps": WALK_STEPS,
            "conductance_threshold": CONDUCTANCE_THRESHOLD,
            "connectivity_floor_formula": "1/(2*N)",
            "social_seat_limit_formula": "max(2,N/10)",
            "control_group_seat_limit": CONTROL_GROUP_SEAT_LIMIT,
            "single_change_until_n": SINGLE_CHANGE_UNTIL_N,
            "pre_threshold_change_limit": PRE_THRESHOLD_CHANGE_LIMIT,
            "old_root_overlap_rounds": OLD_ROOT_OVERLAP_ROUNDS,
            "full_window_review_windows": FULL_WINDOW_REVIEW_WINDOWS,
        },
        "incumbent_breach_action": "PRESERVE_AND_REPORT",
        "removal_path": "EXISTING_AUTHORIZED_CHURN_ONLY",
        "automatic_eviction_selector": False,
        "correction_override": False,
        "new_validator_seed_timing": "NEXT_WINDOW_BOUNDARY",
    }


def admission_policy_root() -> str:
    return _domain_hash(POLICY_ROOT_DOMAIN, admission_policy_document())


@dataclass(frozen=True, order=True)
class RegistrySeat:
    validator_id: str
    account_id: str

    def to_dict(self) -> dict[str, str]:
        return {
            "validator_id": self.validator_id,
            "account_id": self.account_id,
        }


@dataclass(frozen=True, order=True)
class PriorBreach:
    limit_id: str
    first_observed_window: int

    def to_dict(self) -> dict[str, Any]:
        return {
            "limit_id": self.limit_id,
            "first_observed_window": self.first_observed_window,
        }


@dataclass(frozen=True, order=True)
class GraphCredit:
    kind: str
    source_account: str
    target_account: str
    credited_units: int
    credit_ids: tuple[str, ...]
    evidence_digests: tuple[str, ...]

    @property
    def weight(self) -> Fraction:
        return (
            VOUCH_WEIGHT
            if self.kind == "vouch"
            else COWORK_WEIGHT * self.credited_units
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "kind": self.kind,
            "source_account": self.source_account,
            "target_account": self.target_account,
            "credited_units": self.credited_units,
            "credit_ids": list(self.credit_ids),
            "evidence_digests": list(self.evidence_digests),
            "weight": self.weight,
        }


@dataclass(frozen=True, order=True)
class ConductanceCut:
    left: tuple[str, ...]
    right: tuple[str, ...]
    conductance: Fraction

    def to_dict(self) -> dict[str, Any]:
        return {
            "left": list(self.left),
            "right": list(self.right),
            "conductance": self.conductance,
        }


@dataclass(frozen=True, order=True)
class FrozenCluster:
    cluster_id: str
    members: tuple[str, ...]
    causative_evidence: tuple[str, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "cluster_id": self.cluster_id,
            "members": list(self.members),
            "causative_evidence": list(self.causative_evidence),
        }


@dataclass(frozen=True)
class FrozenAdmissionWindow:
    status: str
    failures: tuple[ValidationFailure, ...]
    evidence_status: str
    evidence_commitments: tuple[tuple[str, str], ...]
    window: EvaluationWindow | None
    opening_registry_root: str
    opening_list_size: int
    nodes: tuple[str, ...]
    foundation_accounts: tuple[str, ...]
    eligible_seeds: tuple[str, ...]
    opening_seats: tuple[RegistrySeat, ...]
    hold_accounts: tuple[str, ...]
    continuity: tuple[ContinuityAssessment, ...]
    graph_credits: tuple[GraphCredit, ...]
    declarations: tuple[ActiveControlDeclaration, ...]
    seed_vector: tuple[tuple[str, Fraction], ...]
    raw_rows: tuple[tuple[str, tuple[tuple[str, Fraction], ...]], ...]
    transition_rows: tuple[tuple[str, tuple[tuple[str, Fraction], ...]], ...]
    stationary_mass: tuple[tuple[str, Fraction], ...]
    cuts: tuple[ConductanceCut, ...]
    clusters: tuple[FrozenCluster, ...]
    prior_breaches: tuple[PriorBreach, ...]
    roots: tuple[tuple[str, str], ...]
    frozen_window_root: str

    def _payload(self) -> dict[str, Any]:
        return {
            **_header(V2_FROZEN_WINDOW_SCHEMA),
            "status": self.status,
            "failures": [item.to_dict() for item in self.failures],
            "evidence_status": self.evidence_status,
            "evidence_commitments": dict(self.evidence_commitments),
            "window": self.window.to_dict() if self.window is not None else None,
            "opening_registry_root": self.opening_registry_root or None,
            "opening_list_size": self.opening_list_size,
            "nodes": list(self.nodes),
            "foundation_accounts": list(self.foundation_accounts),
            "eligible_seeds": list(self.eligible_seeds),
            "opening_seats": [item.to_dict() for item in self.opening_seats],
            "hold_accounts": list(self.hold_accounts),
            "continuity": [item.to_dict() for item in self.continuity],
            "graph_credits": [item.to_dict() for item in self.graph_credits],
            "seed_vector": dict(self.seed_vector),
            "raw_rows": {
                source: dict(row) for source, row in self.raw_rows
            },
            "transition_rows": {
                source: dict(row) for source, row in self.transition_rows
            },
            "stationary_mass": dict(self.stationary_mass),
            "cuts": [item.to_dict() for item in self.cuts],
            "clusters": [item.to_dict() for item in self.clusters],
            "declarations": [
                item.to_dict() for item in self.declarations
            ],
            "prior_breaches": [
                item.to_dict() for item in self.prior_breaches
            ],
            "roots": dict(self.roots),
        }

    def to_dict(self) -> dict[str, Any]:
        return {
            **self._payload(),
            "frozen_window_root": self.frozen_window_root,
        }

    def canonical_bytes(self) -> bytes:
        return canonical_json_bytes(self.to_dict())


@dataclass(frozen=True, order=True)
class AdmissionCandidate:
    validator_id: str
    account_id: str
    control_epoch: str
    source_registry_root: str
    source_registry_round: int

    def to_dict(self) -> dict[str, Any]:
        return {
            "validator_id": self.validator_id,
            "account_id": self.account_id,
            "control_epoch": self.control_epoch,
            "source_registry_root": self.source_registry_root,
            "source_registry_round": self.source_registry_round,
        }


@dataclass(frozen=True)
class RegistryRoundState:
    round_index: int
    current_registry_root: str
    prior_registry_root: str | None
    prior_registry_round: int | None
    seats: tuple[RegistrySeat, ...]
    changes_this_round: int = 0
    transition_budget: int = 1

    def to_dict(self) -> dict[str, Any]:
        return {
            "round_index": self.round_index,
            "current_registry_root": self.current_registry_root,
            "prior_registry_root": self.prior_registry_root,
            "prior_registry_round": self.prior_registry_round,
            "seats": [item.to_dict() for item in sorted(self.seats)],
            "changes_this_round": self.changes_this_round,
            "transition_budget": self.transition_budget,
        }


@dataclass(frozen=True, order=True)
class LimitState:
    limit_kind: str
    limit_id: str
    members: tuple[str, ...]
    state: str
    exact_limit: Fraction
    permitted_integer_seats: int
    occupied_seats: int
    excess_seats: int
    seat_ids: tuple[str, ...]
    causative_evidence: tuple[str, ...]
    first_observed_window: int | None
    review_state: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "limit_kind": self.limit_kind,
            "limit_id": self.limit_id,
            "members": list(self.members),
            "state": self.state,
            "exact_limit": self.exact_limit,
            "permitted_integer_seats": self.permitted_integer_seats,
            "occupied_seats": self.occupied_seats,
            "excess_seats": self.excess_seats,
            "seat_ids": list(self.seat_ids),
            "causative_evidence": list(self.causative_evidence),
            "first_observed_window": self.first_observed_window,
            "review_state": self.review_state,
        }


@dataclass(frozen=True, order=True)
class CandidateReason:
    code: str
    field: str
    limit_id: str = ""
    causative_evidence: tuple[str, ...] = ()

    def to_dict(self) -> dict[str, Any]:
        return {
            "code": self.code,
            "field": self.field,
            "limit_id": self.limit_id or None,
            "causative_evidence": list(self.causative_evidence),
        }


@dataclass(frozen=True)
class CandidateDecision:
    action: str
    candidate: AdmissionCandidate
    reasons: tuple[CandidateReason, ...]
    affected_limit_ids: tuple[str, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "action": self.action,
            "candidate": self.candidate.to_dict(),
            "reasons": [item.to_dict() for item in self.reasons],
            "affected_limit_ids": list(self.affected_limit_ids),
        }


@dataclass(frozen=True)
class ShadowAdmissionReport:
    status: str
    roots: tuple[tuple[str, str], ...]
    registry_state: RegistryRoundState
    frozen_seeds: tuple[str, ...]
    limit_states: tuple[LimitState, ...]
    unresolved_limits: tuple[LimitState, ...]
    preserved_incumbent_seat_ids: tuple[str, ...]
    incumbent_continuity_holds: tuple[str, ...]
    decision: CandidateDecision
    churn: Mapping[str, Any]
    report_root: str

    def _payload(self) -> dict[str, Any]:
        return {
            **_header(V2_ADMISSION_REPORT_SCHEMA),
            "status": self.status,
            "roots": dict(self.roots),
            "registry_state": self.registry_state.to_dict(),
            "frozen_seeds": list(self.frozen_seeds),
            "limit_states": [item.to_dict() for item in self.limit_states],
            "unresolved_limits": [
                item.to_dict() for item in self.unresolved_limits
            ],
            "preserved_incumbent_seat_ids": list(
                self.preserved_incumbent_seat_ids
            ),
            "incumbent_continuity_holds": list(
                self.incumbent_continuity_holds
            ),
            "decision": self.decision.to_dict(),
            "churn": dict(self.churn),
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._payload(), "report_root": self.report_root}

    def canonical_bytes(self) -> bytes:
        return canonical_json_bytes(self.to_dict())


def _validated_ids(values: Sequence[str], field: str) -> tuple[str, ...]:
    checked = tuple(
        require_bounded_identifier(value, f"{field}[{index}]")
        for index, value in enumerate(values)
    )
    if len(set(checked)) != len(checked):
        raise TaskNodeUnlError("duplicate_identifier", field)
    return tuple(sorted(checked))


def _validated_seats(
    values: Sequence[RegistrySeat],
    nodes: frozenset[str],
    field: str,
) -> tuple[RegistrySeat, ...]:
    seats: list[RegistrySeat] = []
    validator_ids: set[str] = set()
    for index, value in enumerate(values):
        if not isinstance(value, RegistrySeat):
            raise TaskNodeUnlError("invalid_registry_seat", f"{field}[{index}]")
        validator_id = require_bounded_identifier(
            value.validator_id, f"{field}[{index}].validator_id"
        )
        account_id = require_bounded_identifier(
            value.account_id, f"{field}[{index}].account_id"
        )
        if validator_id in validator_ids:
            raise TaskNodeUnlError(
                "duplicate_validator_seat", f"{field}[{index}].validator_id"
            )
        if account_id not in nodes:
            raise TaskNodeUnlError(
                "seat_unknown_account", f"{field}[{index}].account_id"
            )
        validator_ids.add(validator_id)
        seats.append(RegistrySeat(validator_id, account_id))
    return tuple(sorted(seats))


def _credits(
    nodes: frozenset[str],
    vouches: Sequence[ActiveRelation],
    cowork: Sequence[ActiveRelation],
) -> tuple[GraphCredit, ...]:
    vouch_pairs: dict[tuple[str, str], list[ActiveRelation]] = {}
    cowork_pairs: dict[tuple[str, str], dict[str, ActiveRelation]] = {}

    for index, relation in enumerate(vouches):
        if relation.relation_kind != "vouch":
            raise TaskNodeUnlError(
                "relation_kind_mismatch", f"active_vouches[{index}].relation_kind"
            )
        if (
            relation.source_account not in nodes
            or relation.target_account not in nodes
        ):
            raise TaskNodeUnlError(
                "relation_unknown_account", f"active_vouches[{index}]"
            )
        if relation.source_account == relation.target_account:
            raise TaskNodeUnlError("self_relation", f"active_vouches[{index}]")
        vouch_pairs.setdefault(
            (relation.source_account, relation.target_account), []
        ).append(relation)

    for index, relation in enumerate(cowork):
        if relation.relation_kind != "cowork":
            raise TaskNodeUnlError(
                "relation_kind_mismatch", f"active_cowork[{index}].relation_kind"
            )
        if (
            relation.source_account not in nodes
            or relation.target_account not in nodes
        ):
            raise TaskNodeUnlError(
                "relation_unknown_account", f"active_cowork[{index}]"
            )
        if relation.source_account == relation.target_account:
            raise TaskNodeUnlError("self_relation", f"active_cowork[{index}]")
        pair = tuple(sorted((relation.source_account, relation.target_account)))
        by_credit = cowork_pairs.setdefault(pair, {})
        prior = by_credit.get(relation.credit_id)
        if prior is None or relation.record_digest < prior.record_digest:
            by_credit[relation.credit_id] = relation

    result: list[GraphCredit] = []
    for (source, target), records in sorted(vouch_pairs.items()):
        ordered = sorted(records, key=lambda item: (
            item.credit_id, item.record_digest
        ))
        result.append(
            GraphCredit(
                "vouch",
                source,
                target,
                1,
                tuple(sorted({item.credit_id for item in ordered})),
                tuple(sorted({item.record_digest for item in ordered})),
            )
        )
    for (source, target), by_credit in sorted(cowork_pairs.items()):
        selected_ids = tuple(sorted(by_credit))[:COWORK_CREDIT_CAP]
        result.append(
            GraphCredit(
                "cowork",
                source,
                target,
                len(selected_ids),
                selected_ids,
                tuple(
                    sorted(
                        {by_credit[credit_id].record_digest for credit_id in selected_ids}
                    )
                ),
            )
        )
    return tuple(sorted(result))


def _add_weight(
    rows: dict[str, dict[str, Fraction]],
    source: str,
    target: str,
    weight: Fraction,
) -> None:
    rows[source][target] = rows[source].get(target, Fraction(0, 1)) + weight


def _raw_rows(
    nodes: tuple[str, ...],
    credits: Sequence[GraphCredit],
) -> dict[str, dict[str, Fraction]]:
    rows = {node: {} for node in nodes}
    for credit in credits:
        _add_weight(rows, credit.source_account, credit.target_account, credit.weight)
        if credit.kind == "cowork":
            _add_weight(
                rows, credit.target_account, credit.source_account, credit.weight
            )
    return {
        source: {
            target: rows[source][target] for target in sorted(rows[source])
        }
        for source in nodes
    }


def _normalize_rows(
    raw_rows: Mapping[str, Mapping[str, Fraction]],
    seed_vector: Mapping[str, Fraction],
) -> dict[str, dict[str, Fraction]]:
    result: dict[str, dict[str, Fraction]] = {}
    for source in sorted(raw_rows):
        total = sum(raw_rows[source].values(), Fraction(0, 1))
        if total == 0:
            result[source] = {
                target: seed_vector[target]
                for target in sorted(seed_vector)
                if seed_vector[target] > 0
            }
        else:
            result[source] = {
                target: raw_rows[source][target] / total
                for target in sorted(raw_rows[source])
            }
        if sum(result[source].values(), Fraction(0, 1)) != 1:
            raise TaskNodeUnlError("row_normalization_failed", source)
    return result


def _walk(
    transition_rows: Mapping[str, Mapping[str, Fraction]],
    seed_vector: Mapping[str, Fraction],
) -> dict[str, Fraction]:
    nodes = tuple(sorted(seed_vector))
    mass = dict(seed_vector)
    for _step in range(WALK_STEPS):
        walked = {node: Fraction(0, 1) for node in nodes}
        for source in nodes:
            for target in sorted(transition_rows[source]):
                walked[target] += mass[source] * transition_rows[source][target]
        mass = {
            node: WALK_DAMPING * walked[node] + SEED_DAMPING * seed_vector[node]
            for node in nodes
        }
        if sum(mass.values(), Fraction(0, 1)) != 1:
            raise TaskNodeUnlError("stationary_mass_not_one", "graph")
    return mass


def _undirected_weights(
    raw_rows: Mapping[str, Mapping[str, Fraction]],
) -> dict[tuple[str, str], Fraction]:
    weights: dict[tuple[str, str], Fraction] = {}
    for source in sorted(raw_rows):
        for target, weight in sorted(raw_rows[source].items()):
            pair = tuple(sorted((source, target)))
            weights[pair] = weights.get(pair, Fraction(0, 1)) + weight
    return {
        pair: weight for pair, weight in sorted(weights.items()) if weight > 0
    }


def _conductance(
    members: frozenset[str],
    left: frozenset[str],
    weights: Mapping[tuple[str, str], Fraction],
) -> Fraction | None:
    right = members - left
    if not left or not right:
        return None
    left_volume = Fraction(0, 1)
    right_volume = Fraction(0, 1)
    cut_weight = Fraction(0, 1)
    for (source, target), weight in sorted(weights.items()):
        if source not in members or target not in members:
            continue
        left_volume += weight if source in left else 0
        right_volume += weight if source in right else 0
        left_volume += weight if target in left else 0
        right_volume += weight if target in right else 0
        if (source in left) != (target in left):
            cut_weight += weight
    denominator = min(left_volume, right_volume)
    return None if denominator == 0 else cut_weight / denominator


def _components(
    nodes: Sequence[str],
    weights: Mapping[tuple[str, str], Fraction],
) -> tuple[tuple[str, ...], ...]:
    neighbors = {node: set() for node in nodes}
    for (source, target), weight in sorted(weights.items()):
        if weight > 0:
            neighbors[source].add(target)
            neighbors[target].add(source)
    remaining = set(nodes)
    result: list[tuple[str, ...]] = []
    while remaining:
        stack = [min(remaining)]
        component: set[str] = set()
        while stack:
            node = stack.pop()
            if node in component:
                continue
            component.add(node)
            stack.extend(sorted(neighbors[node] - component, reverse=True))
        remaining -= component
        result.append(tuple(sorted(component)))
    return tuple(sorted(result))


def _partition(
    nodes: tuple[str, ...],
    raw_rows: Mapping[str, Mapping[str, Fraction]],
    mass: Mapping[str, Fraction],
) -> tuple[tuple[tuple[str, ...], ...], tuple[ConductanceCut, ...]]:
    weights = _undirected_weights(raw_rows)
    clusters: list[tuple[str, ...]] = []
    cuts: list[ConductanceCut] = []

    def split(members: tuple[str, ...]) -> None:
        if len(members) < 2:
            clusters.append(members)
            return
        ranked = tuple(sorted(members, key=lambda node: (-mass[node], node)))
        member_set = frozenset(members)
        candidates: list[
            tuple[Fraction, tuple[str, ...], tuple[str, ...]]
        ] = []
        for index in range(1, len(ranked)):
            left_set = frozenset(ranked[:index])
            conductance = _conductance(member_set, left_set, weights)
            if conductance is None or conductance >= CONDUCTANCE_THRESHOLD:
                continue
            first = tuple(sorted(left_set))
            second = tuple(sorted(member_set - left_set))
            left, right = (first, second) if first < second else (second, first)
            candidates.append((conductance, left, right))
        if not candidates:
            clusters.append(tuple(sorted(members)))
            return
        conductance, left, right = min(candidates)
        cut = ConductanceCut(left, right, conductance)
        cuts.append(cut)
        split(left)
        split(right)

    for component in _components(nodes, weights):
        split(component)
    return tuple(sorted(clusters)), tuple(sorted(cuts))


def _row_tuples(
    rows: Mapping[str, Mapping[str, Fraction]],
) -> tuple[tuple[str, tuple[tuple[str, Fraction], ...]], ...]:
    return tuple(
        (
            source,
            tuple((target, rows[source][target]) for target in sorted(rows[source])),
        )
        for source in sorted(rows)
    )


def _invalid_frozen(
    evidence: object,
    failure: ValidationFailure,
    *,
    opening_registry_root: str = "",
    window: EvaluationWindow | None = None,
) -> FrozenAdmissionWindow:
    is_result = isinstance(evidence, EvidenceSnapshotResult)
    commitments = ()
    if is_result and isinstance(evidence.commitments, Mapping):
        commitments = tuple(
            sorted(
                (str(key), str(value))
                for key, value in evidence.commitments.items()
                if isinstance(key, str) and isinstance(value, str)
            )
        )
    evidence_failures = evidence.failures if is_result else ()
    evidence_status = evidence.status if is_result else "invalid"
    evidence_window = evidence.window if is_result else None
    hold_accounts = evidence.hold_accounts if is_result else ()
    continuity = evidence.continuity if is_result else ()
    roots = (("admission_policy_root", admission_policy_root()),)
    frozen = FrozenAdmissionWindow(
        status=NO_PROPOSAL,
        failures=tuple(sorted((*evidence_failures, failure))),
        evidence_status=evidence_status,
        evidence_commitments=commitments,
        window=window or evidence_window,
        opening_registry_root=opening_registry_root,
        opening_list_size=0,
        nodes=(),
        foundation_accounts=(),
        eligible_seeds=(),
        opening_seats=(),
        hold_accounts=tuple(sorted(hold_accounts)),
        continuity=tuple(sorted(continuity)),
        graph_credits=(),
        declarations=(),
        seed_vector=(),
        raw_rows=(),
        transition_rows=(),
        stationary_mass=(),
        cuts=(),
        clusters=(),
        prior_breaches=(),
        roots=roots,
        frozen_window_root="",
    )
    return replace(
        frozen,
        frozen_window_root=_domain_hash(FROZEN_ROOT_DOMAIN, frozen._payload()),
    )


def freeze_admission_window(
    evidence: EvidenceSnapshotResult,
    *,
    opening_registry_root: str,
    nodes: Sequence[str],
    opening_seats: Sequence[RegistrySeat],
    foundation_accounts: Sequence[str] = (),
    prior_breaches: Sequence[PriorBreach] = (),
) -> FrozenAdmissionWindow:
    """Freeze one boundary's evidence graph, partition, N, and seed set."""

    try:
        if not isinstance(evidence, EvidenceSnapshotResult):
            raise TaskNodeUnlError("invalid_evidence_result", "evidence")
        if (
            evidence.status not in ("verified", "verified_with_holds")
            or evidence.failures
            or evidence.commitments is None
            or evidence.window is None
        ):
            field = (
                evidence.failures[0].field
                if evidence.failures
                else "evidence.status"
            )
            raise TaskNodeUnlError("invalid_snapshot_commitments", field)
        evidence.window.validate("evidence.window")
        registry_root = require_lower_hex(
            opening_registry_root, "opening_registry_root"
        )
        expected_commitments = {"policy_root", "input_root", "registry_root"}
        if set(evidence.commitments) != expected_commitments:
            raise TaskNodeUnlError(
                "commitment_shape_mismatch", "evidence.commitments"
            )
        checked_commitments = tuple(
            (
                name,
                require_lower_hex(
                    evidence.commitments[name], f"evidence.commitments.{name}"
                ),
            )
            for name in sorted(expected_commitments)
        )
        checked_nodes = _validated_ids(nodes, "nodes")
        if not checked_nodes:
            raise TaskNodeUnlError("empty_nodes", "nodes")
        node_set = frozenset(checked_nodes)
        foundation = _validated_ids(
            foundation_accounts, "foundation_accounts"
        )
        if not set(foundation).issubset(node_set):
            raise TaskNodeUnlError(
                "foundation_unknown_account", "foundation_accounts"
            )
        seats = _validated_seats(opening_seats, node_set, "opening_seats")
        if not seats:
            raise TaskNodeUnlError("empty_opening_registry", "opening_seats")
        seeds = tuple(
            sorted(
                {
                    seat.account_id
                    for seat in seats
                    if seat.account_id not in set(foundation)
                }
            )
        )
        if not seeds:
            raise TaskNodeUnlError(
                "empty_eligible_seeds", "eligible_seeds"
            )

        histories: list[PriorBreach] = []
        for index, prior in enumerate(prior_breaches):
            if not isinstance(prior, PriorBreach):
                raise TaskNodeUnlError(
                    "invalid_prior_breach", f"prior_breaches[{index}]"
                )
            limit_id = require_lower_hex(
                prior.limit_id, f"prior_breaches[{index}].limit_id"
            )
            if (
                isinstance(prior.first_observed_window, bool)
                or not isinstance(prior.first_observed_window, int)
                or prior.first_observed_window < 0
                or prior.first_observed_window > evidence.window.index
            ):
                raise TaskNodeUnlError(
                    "invalid_prior_breach_window",
                    f"prior_breaches[{index}].first_observed_window",
                )
            histories.append(
                PriorBreach(limit_id, prior.first_observed_window)
            )
        if len({item.limit_id for item in histories}) != len(histories):
            raise TaskNodeUnlError(
                "duplicate_prior_breach", "prior_breaches"
            )

        credits = _credits(
            node_set, evidence.active_vouches, evidence.active_cowork
        )
        declarations: list[ActiveControlDeclaration] = []
        for index, declaration in enumerate(evidence.active_control_declarations):
            if not set(declaration.members).issubset(node_set):
                raise TaskNodeUnlError(
                    "declaration_unknown_account",
                    f"active_control_declarations[{index}].members",
                )
            declarations.append(declaration)
        declarations_tuple = tuple(sorted(declarations))

        seed_mass = Fraction(1, len(seeds))
        seed_vector = {
            node: seed_mass if node in seeds else Fraction(0, 1)
            for node in checked_nodes
        }
        raw = _raw_rows(checked_nodes, credits)
        transitions = _normalize_rows(raw, seed_vector)
        mass = _walk(transitions, seed_vector)
        partition, cuts = _partition(checked_nodes, raw, mass)

        clusters = tuple(
            FrozenCluster(
                cluster_id=_domain_hash(
                    LIMIT_ID_DOMAIN,
                    {"limit_kind": "SOCIAL_CLUSTER", "members": list(members)},
                ),
                members=members,
                causative_evidence=tuple(
                    sorted(
                        {
                            digest
                            for credit in credits
                            if credit.source_account in members
                            and credit.target_account in members
                            for digest in credit.evidence_digests
                        }
                    )
                ),
            )
            for members in partition
        )

        graph_document = {
            "nodes": list(checked_nodes),
            "eligible_seeds": list(seeds),
            "graph_credits": [item.to_dict() for item in credits],
            "seed_vector": seed_vector,
            "raw_rows": raw,
            "transition_rows": transitions,
            "stationary_mass": mass,
            "cuts": [item.to_dict() for item in cuts],
            "clusters": [item.to_dict() for item in clusters],
        }
        roots = tuple(
            sorted(
                {
                    "admission_policy_root": admission_policy_root(),
                    "evidence_policy_root": dict(checked_commitments)["policy_root"],
                    "evidence_input_root": dict(checked_commitments)["input_root"],
                    "control_registry_root": dict(checked_commitments)["registry_root"],
                    "validator_registry_root": registry_root,
                    "window_root": _domain_hash(
                        WINDOW_ROOT_DOMAIN, evidence.window.to_dict()
                    ),
                    "graph_root": _domain_hash(
                        GRAPH_ROOT_DOMAIN, graph_document
                    ),
                    "declaration_root": _domain_hash(
                        DECLARATION_ROOT_DOMAIN,
                        [item.to_dict() for item in declarations_tuple],
                    ),
                }.items()
            )
        )
        frozen = FrozenAdmissionWindow(
            status=FROZEN,
            failures=(),
            evidence_status=evidence.status,
            evidence_commitments=checked_commitments,
            window=evidence.window,
            opening_registry_root=registry_root,
            opening_list_size=len(seats),
            nodes=checked_nodes,
            foundation_accounts=foundation,
            eligible_seeds=seeds,
            opening_seats=seats,
            hold_accounts=tuple(sorted(evidence.hold_accounts)),
            continuity=tuple(sorted(evidence.continuity)),
            graph_credits=credits,
            declarations=declarations_tuple,
            seed_vector=tuple(
                (node, seed_vector[node]) for node in checked_nodes
            ),
            raw_rows=_row_tuples(raw),
            transition_rows=_row_tuples(transitions),
            stationary_mass=tuple(
                (node, mass[node]) for node in checked_nodes
            ),
            cuts=cuts,
            clusters=clusters,
            prior_breaches=tuple(sorted(histories)),
            roots=roots,
            frozen_window_root="",
        )
        return replace(
            frozen,
            frozen_window_root=_domain_hash(
                FROZEN_ROOT_DOMAIN, frozen._payload()
            ),
        )
    except TaskNodeUnlError as error:
        return _invalid_frozen(
            evidence,
            _failure(error, "evidence"),
            opening_registry_root=(
                opening_registry_root
                if isinstance(opening_registry_root, str)
                else ""
            ),
        )


def _control_groups(
    frozen: FrozenAdmissionWindow,
) -> tuple[tuple[str, tuple[str, ...], tuple[str, ...]], ...]:
    parent = {node: node for node in frozen.nodes}

    def find(node: str) -> str:
        while parent[node] != node:
            parent[node] = parent[parent[node]]
            node = parent[node]
        return node

    def union(left: str, right: str) -> None:
        left_root = find(left)
        right_root = find(right)
        if left_root == right_root:
            return
        first, second = sorted((left_root, right_root))
        parent[second] = first

    for declaration in frozen.declarations:
        for member in declaration.members[1:]:
            union(declaration.members[0], member)

    grouped: dict[str, list[str]] = {}
    for node in frozen.nodes:
        grouped.setdefault(find(node), []).append(node)

    result = []
    for members_list in grouped.values():
        members = tuple(sorted(members_list))
        evidence = tuple(
            sorted(
                {
                    declaration.record_digest
                    for declaration in frozen.declarations
                    if set(declaration.members).issubset(set(members))
                }
            )
        )
        limit_id = _domain_hash(
            LIMIT_ID_DOMAIN,
            {
                "limit_kind": "DECLARED_CONTROL_GROUP",
                "members": list(members),
                "causative_evidence": list(evidence),
            },
        )
        result.append((limit_id, members, evidence))
    return tuple(sorted(result))


def _limit_state(
    *,
    kind: str,
    limit_id: str,
    members: tuple[str, ...],
    exact_limit: Fraction,
    evidence: tuple[str, ...],
    seats: Sequence[RegistrySeat],
    window_index: int,
    prior: Mapping[str, PriorBreach],
) -> LimitState:
    seat_ids = tuple(
        sorted(
            seat.validator_id for seat in seats if seat.account_id in members
        )
    )
    occupied = len(seat_ids)
    permitted = permitted_integer_seats(exact_limit)
    if occupied > exact_limit:
        state = EXISTING_BREACH
    elif occupied + 1 > exact_limit:
        state = SATURATED
    else:
        state = CLEAR
    excess = max(0, occupied - permitted)
    previous = prior.get(limit_id)
    if state != EXISTING_BREACH:
        first_observed = None
        review_state = "NOT_APPLICABLE"
    else:
        first_observed = (
            previous.first_observed_window
            if previous is not None
            else window_index
        )
        review_state = (
            "PERSISTENT_UNRESOLVED"
            if window_index >= first_observed + FULL_WINDOW_REVIEW_WINDOWS
            else "FULL_WINDOW_REVIEW"
        )
    return LimitState(
        limit_kind=kind,
        limit_id=limit_id,
        members=members,
        state=state,
        exact_limit=exact_limit,
        permitted_integer_seats=permitted,
        occupied_seats=occupied,
        excess_seats=excess,
        seat_ids=seat_ids,
        causative_evidence=evidence,
        first_observed_window=first_observed,
        review_state=review_state,
    )


def recount_limit_states(
    frozen: FrozenAdmissionWindow,
    seats: Sequence[RegistrySeat],
) -> tuple[LimitState, ...]:
    """Recount all social and declared-group seats for the current round."""

    if frozen.status != FROZEN or frozen.window is None:
        return ()
    checked_seats = _validated_seats(
        seats, frozenset(frozen.nodes), "registry_state.seats"
    )
    prior = {item.limit_id: item for item in frozen.prior_breaches}
    states = [
        _limit_state(
            kind="SOCIAL_CLUSTER",
            limit_id=cluster.cluster_id,
            members=cluster.members,
            exact_limit=social_seat_limit(frozen.opening_list_size),
            evidence=cluster.causative_evidence,
            seats=checked_seats,
            window_index=frozen.window.index,
            prior=prior,
        )
        for cluster in frozen.clusters
    ]
    states.extend(
        _limit_state(
            kind="DECLARED_CONTROL_GROUP",
            limit_id=limit_id,
            members=members,
            exact_limit=CONTROL_GROUP_SEAT_LIMIT,
            evidence=evidence,
            seats=checked_seats,
            window_index=frozen.window.index,
            prior=prior,
        )
        for limit_id, members, evidence in _control_groups(frozen)
    )
    return tuple(sorted(states))


def _validate_round_state(
    frozen: FrozenAdmissionWindow,
    state: RegistryRoundState,
) -> RegistryRoundState:
    if isinstance(state.round_index, bool) or not isinstance(state.round_index, int):
        raise TaskNodeUnlError(
            "invalid_integer", "registry_state.round_index"
        )
    if state.round_index < 0:
        raise TaskNodeUnlError(
            "integer_below_minimum", "registry_state.round_index"
        )
    current_root = require_lower_hex(
        state.current_registry_root,
        "registry_state.current_registry_root",
    )
    prior_root = None
    if state.prior_registry_root is not None:
        prior_root = require_lower_hex(
            state.prior_registry_root,
            "registry_state.prior_registry_root",
        )
        if (
            isinstance(state.prior_registry_round, bool)
            or not isinstance(state.prior_registry_round, int)
            or state.prior_registry_round != state.round_index - 1
        ):
            raise TaskNodeUnlError(
                "old_root_overlap_exceeded",
                "registry_state.prior_registry_round",
            )
    elif state.prior_registry_round is not None:
        raise TaskNodeUnlError(
            "prior_root_round_without_root",
            "registry_state.prior_registry_round",
        )
    for name, value in (
        ("changes_this_round", state.changes_this_round),
        ("transition_budget", state.transition_budget),
    ):
        if isinstance(value, bool) or not isinstance(value, int) or value < 0:
            raise TaskNodeUnlError(
                "invalid_integer", f"registry_state.{name}"
            )
    seats = _validated_seats(
        state.seats, frozenset(frozen.nodes), "registry_state.seats"
    )
    return RegistryRoundState(
        state.round_index,
        current_root,
        prior_root,
        state.prior_registry_round,
        seats,
        state.changes_this_round,
        state.transition_budget,
    )


def _candidate_reason_for_limit(
    item: LimitState,
) -> CandidateReason | None:
    if item.state == CLEAR:
        return None
    return CandidateReason(
        code=(
            f"{item.limit_kind}_{item.state}"
        ),
        field="candidate.account_id",
        limit_id=item.limit_id,
        causative_evidence=item.causative_evidence,
    )


def evaluate_admission_round(
    frozen: FrozenAdmissionWindow,
    registry_state: RegistryRoundState,
    candidate: AdmissionCandidate,
) -> ShadowAdmissionReport:
    """Evaluate one addition without mutating registry or incumbent seats."""

    roots = dict(frozen.roots)
    roots["frozen_window_root"] = frozen.frozen_window_root
    reasons: list[CandidateReason] = []
    limit_states: tuple[LimitState, ...] = ()
    checked_state = registry_state

    try:
        validator_id = require_bounded_identifier(
            candidate.validator_id, "candidate.validator_id"
        )
        account_id = require_bounded_identifier(
            candidate.account_id, "candidate.account_id"
        )
        control_epoch = require_lower_hex(
            candidate.control_epoch, "candidate.control_epoch"
        )
        source_root = require_lower_hex(
            candidate.source_registry_root,
            "candidate.source_registry_root",
        )
        if (
            isinstance(candidate.source_registry_round, bool)
            or not isinstance(candidate.source_registry_round, int)
            or candidate.source_registry_round < 0
        ):
            raise TaskNodeUnlError(
                "invalid_integer", "candidate.source_registry_round"
            )
        checked_candidate = AdmissionCandidate(
            validator_id,
            account_id,
            control_epoch,
            source_root,
            candidate.source_registry_round,
        )
    except TaskNodeUnlError as error:
        checked_candidate = candidate
        reasons.append(
            CandidateReason(error.code, error.detail or "candidate")
        )

    if frozen.status != FROZEN:
        reasons.extend(
            CandidateReason(
                item.code, item.field
            )
            for item in frozen.failures
        )
    else:
        try:
            checked_state = _validate_round_state(frozen, registry_state)
            limit_states = recount_limit_states(frozen, checked_state.seats)
        except TaskNodeUnlError as error:
            reasons.append(
                CandidateReason(error.code, error.detail or "registry_state")
            )

    if frozen.status == FROZEN and not reasons:
        if checked_candidate.account_id not in frozen.nodes:
            reasons.append(
                CandidateReason(
                    "CANDIDATE_NOT_IN_FROZEN_GRAPH",
                    "candidate.account_id",
                )
            )
        if any(
            seat.validator_id == checked_candidate.validator_id
            for seat in checked_state.seats
        ):
            reasons.append(
                CandidateReason(
                    "VALIDATOR_ALREADY_SEATED", "candidate.validator_id"
                )
            )
        if any(
            seat.account_id == checked_candidate.account_id
            for seat in checked_state.seats
        ):
            reasons.append(
                CandidateReason(
                    "ACCOUNT_ALREADY_SEATED", "candidate.account_id"
                )
            )

        current_source = (
            checked_candidate.source_registry_root
            == checked_state.current_registry_root
            and checked_candidate.source_registry_round
            == checked_state.round_index
        )
        prior_source = (
            checked_state.prior_registry_root is not None
            and checked_candidate.source_registry_root
            == checked_state.prior_registry_root
            and checked_candidate.source_registry_round
            == checked_state.prior_registry_round
        )
        if not current_source and not prior_source:
            reasons.append(
                CandidateReason(
                    "REGISTRY_ROOT_OUTSIDE_ONE_ROUND_OVERLAP",
                    "candidate.source_registry_root",
                )
            )

        effective_budget = checked_state.transition_budget
        if len(checked_state.seats) < SINGLE_CHANGE_UNTIL_N:
            effective_budget = min(
                effective_budget, PRE_THRESHOLD_CHANGE_LIMIT
            )
        if checked_state.changes_this_round + 1 > effective_budget:
            reasons.append(
                CandidateReason(
                    "AUTHORIZED_CHURN_BUDGET_EXHAUSTED",
                    "registry_state.changes_this_round",
                )
            )

        continuity = {
            item.account_id: item for item in frozen.continuity
        }.get(checked_candidate.account_id)
        if continuity is None:
            reasons.append(
                CandidateReason(
                    "HOLD_CONTINUITY_MISSING",
                    "candidate.control_epoch",
                )
            )
        elif (
            continuity.control_epoch != checked_candidate.control_epoch
            or continuity.status != "READY"
            or checked_candidate.account_id in frozen.hold_accounts
        ):
            reasons.append(
                CandidateReason(
                    "HOLD_CONTINUITY",
                    "candidate.control_epoch",
                )
            )

        mass = dict(frozen.stationary_mass)
        if (
            checked_candidate.account_id in mass
            and mass[checked_candidate.account_id]
            < connectivity_floor(frozen.opening_list_size)
        ):
            reasons.append(
                CandidateReason(
                    "CONNECTIVITY_BELOW_FLOOR",
                    "candidate.account_id",
                )
            )

        for item in limit_states:
            if checked_candidate.account_id not in item.members:
                continue
            reason = _candidate_reason_for_limit(item)
            if reason is not None:
                reasons.append(reason)

    canonical_reasons = tuple(sorted(set(reasons)))
    affected_limit_ids = tuple(
        sorted({item.limit_id for item in canonical_reasons if item.limit_id})
    )
    action = PROPOSE_ADD if not canonical_reasons else (
        NO_PROPOSAL if frozen.status != FROZEN else HOLD
    )
    decision = CandidateDecision(
        action=action,
        candidate=checked_candidate,
        reasons=canonical_reasons,
        affected_limit_ids=affected_limit_ids,
    )
    unresolved = tuple(
        item for item in limit_states if item.state == EXISTING_BREACH
    )
    incumbent_holds = tuple(
        sorted(
            seat.validator_id
            for seat in checked_state.seats
            if seat.account_id in frozen.hold_accounts
        )
    )
    churn = {
        "result": "REGISTRY_DELTA_CANDIDATE" if action == PROPOSE_ADD else "NO_DELTA",
        "delta_kind": "ADD" if action == PROPOSE_ADD else "NO_OP",
        "mutation_count": 1 if action == PROPOSE_ADD else 0,
        "applied": False,
        "requires_existing_authorized_churn": True,
        "preserve_incumbents": True,
        "automatic_eviction_selector": False,
        "correction_override": False,
        "old_root_overlap_rounds": OLD_ROOT_OVERLAP_ROUNDS,
        "single_change_until_n": SINGLE_CHANGE_UNTIL_N,
    }
    report = ShadowAdmissionReport(
        status=PROPOSAL if action == PROPOSE_ADD else NO_PROPOSAL,
        roots=tuple(sorted(roots.items())),
        registry_state=checked_state,
        frozen_seeds=frozen.eligible_seeds,
        limit_states=limit_states,
        unresolved_limits=unresolved,
        preserved_incumbent_seat_ids=tuple(
            sorted(seat.validator_id for seat in checked_state.seats)
        ),
        incumbent_continuity_holds=incumbent_holds,
        decision=decision,
        churn=churn,
        report_root="",
    )
    return replace(
        report,
        report_root=_domain_hash(REPORT_ROOT_DOMAIN, report._payload()),
    )


def advance_shadow_round(
    frozen: FrozenAdmissionWindow,
    registry_state: RegistryRoundState,
    report: ShadowAdmissionReport,
    *,
    new_registry_root: str,
) -> RegistryRoundState:
    """Apply a proposed addition only to a caller-owned hypothetical state."""

    if frozen.status != FROZEN or report.decision.action != PROPOSE_ADD:
        raise TaskNodeUnlError(
            "shadow_addition_not_proposed", "report.decision.action"
        )
    state = _validate_round_state(frozen, registry_state)
    root = require_lower_hex(new_registry_root, "new_registry_root")
    candidate = report.decision.candidate
    seats = _validated_seats(
        (
            *state.seats,
            RegistrySeat(candidate.validator_id, candidate.account_id),
        ),
        frozenset(frozen.nodes),
        "registry_state.seats",
    )
    return RegistryRoundState(
        round_index=state.round_index + 1,
        current_registry_root=root,
        prior_registry_root=state.current_registry_root,
        prior_registry_round=state.round_index,
        seats=seats,
        changes_this_round=0,
        transition_budget=state.transition_budget,
    )


def prior_breaches_from_report(
    report: ShadowAdmissionReport,
) -> tuple[PriorBreach, ...]:
    """Carry only still-unresolved breach clocks into the next window."""

    return tuple(
        sorted(
            PriorBreach(item.limit_id, item.first_observed_window)
            for item in report.unresolved_limits
            if item.first_observed_window is not None
        )
    )
