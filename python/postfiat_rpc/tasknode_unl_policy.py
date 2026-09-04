"""Pure fixture-driven Task Node UNL shadow derivation.

The orchestrator composes the step-one through step-six modules over explicit
JSON-shaped inputs. It performs no file, clock, network, registry, transaction,
or key-store I/O. Its only output is deterministic decision-support data marked
SHADOW_ONLY.
"""

from __future__ import annotations

import hashlib
from dataclasses import dataclass
from fractions import Fraction
from typing import Any, Mapping, Sequence

from .tasknode_unl_accountability import (
    AccountabilityResult,
    evaluate_accountability_document,
)
from .tasknode_unl_binding import (
    ActiveBinding,
    BindingReplayResult,
    replay_bindings_document,
)
from .tasknode_unl_churn import evaluate_churn_guard
from .tasknode_unl_edges import EdgeExtractionResult, extract_public_edges
from .tasknode_unl_schema import (
    ACCOUNTABILITY_FLOOR,
    ACCOUNTABILITY_INPUT_SCHEMA,
    CHURN_BASELINE_SCHEMA,
    CHURN_PROPOSAL_SCHEMA,
    CHURN_REGISTRY_HISTORY_SCHEMA,
    CLUSTER_SEAT_FRACTION,
    CONDUCTANCE_CUT_THRESHOLD,
    CONNECTIVITY_FLOOR_DIVISOR,
    COWORK_EDGE_CAP,
    COWORK_EDGE_WEIGHT,
    FUNDING_EDGE_WEIGHT,
    MIN_CLUSTER_SEATS,
    SHADOW_LEDGER_SNAPSHOT_BUNDLE_SCHEMA,
    SHADOW_MODE,
    SHADOW_POLICY_EVIDENCE_SCHEMA,
    SHADOW_REPORT_SCHEMA,
    SHADOW_WORK_DIGEST_BUNDLE_SCHEMA,
    TRUST_WALK_DAMPING,
    TRUST_WALK_ITERATIONS,
    TRUST_WALK_SEED_DAMPING,
    VOUCH_EDGE_WEIGHT,
    TaskNodeUnlError,
    canonical_json_bytes,
    fraction_document,
    require_closed_keys,
    require_identifier,
    require_int,
)
from .tasknode_unl_trust_graph import (
    SeatAssignment,
    TrustGraphEvidence,
    TrustGraphResult,
    derive_trust_graph,
)
from .tasknode_unl_work_digest import (
    WorkDigestVerificationResult,
    verify_work_digest,
)

SHADOW_INPUT_FILES = (
    "binding_replay",
    "work_digests",
    "ledger_snapshots",
    "publishing_keys",
    "vouch_ledger",
    "cowork_pointers",
    "funding_transfers",
    "funding_exclusions",
    "policy_evidence",
    "baseline_list",
    "registry_history",
)

V1_POLICY_SCHEMA = "postfiat.validator_admission_policy.v1"
V1_DECISION_SCHEMA = "postfiat.validator_admission_decision.v1"
V1_MIN_RELIABILITY_BPS = 9_950
V1_MAX_RHO_SCORE = 0
V1_MAX_ADDS_PER_ROUND = 1

FIELD_RELIABILITY = "validator.performance.uptime_window_bps"
FIELD_ACCOUNTABILITY = "validator.admission.accountability_score"
FIELD_OPERATOR_MANIFEST = "validator.operator_manifest.signature_valid"
FIELD_DOMAIN_CONTROL = "validator.identity.key_domain_binding.status"
FIELD_OPERATOR_GROUP = "validator.topology.operator_group"
FIELD_RELEASE_MANAGER = "validator.topology.release_manager_group"
FIELD_KEY_MANAGEMENT = "validator.topology.key_management_group"
FIELD_FUNDING_SOURCE = "validator.topology.funding_source_group"
FIELD_RHO = "validator.admission.rho_score"
FIELD_COBALT_LINKEDNESS = "validator.cobalt.linkedness_safe"
FIELD_MODEL_CLASSIFICATION = (
    "validator.model.operator_independence_classification"
)
V1_REQUIRED_FIELDS = tuple(
    sorted(
        (
            FIELD_RELIABILITY,
            FIELD_ACCOUNTABILITY,
            FIELD_OPERATOR_MANIFEST,
            FIELD_DOMAIN_CONTROL,
            FIELD_OPERATOR_GROUP,
            FIELD_RELEASE_MANAGER,
            FIELD_KEY_MANAGEMENT,
            FIELD_FUNDING_SOURCE,
            FIELD_RHO,
            FIELD_COBALT_LINKEDNESS,
            FIELD_MODEL_CLASSIFICATION,
        )
    )
)

_FORBIDDEN_INPUT_KEYS = frozenset(
    {
        "social_media_reputation",
        "private_kyc_status",
        "private_messages",
        "uncollected_search_results",
        "unbounded_browsing",
        "unredacted_secrets",
        "raw_ip_geolocation",
        "human_labels",
        "unrelated_packet_fields",
    }
)
_CONTROL_FIELDS = (
    ("operator_group", FIELD_OPERATOR_GROUP, "shared_operator_group"),
    (
        "release_manager_group",
        FIELD_RELEASE_MANAGER,
        "shared_release_manager",
    ),
    (
        "key_management_group",
        FIELD_KEY_MANAGEMENT,
        "shared_key_management",
    ),
    (
        "funding_source_group",
        FIELD_FUNDING_SOURCE,
        "shared_funding_source",
    ),
)
_MAX_CANDIDATES = 4_096
_MAX_ACTIVE_VALIDATORS = 4_096


@dataclass(frozen=True)
class ControlGroup:
    validator_id: str
    operator_group: str
    release_manager_group: str
    key_management_group: str
    funding_source_group: str

    def to_dict(self) -> dict[str, str]:
        return {
            "validator_id": self.validator_id,
            "operator_group": self.operator_group,
            "release_manager_group": self.release_manager_group,
            "key_management_group": self.key_management_group,
            "funding_source_group": self.funding_source_group,
        }


@dataclass(frozen=True)
class ActiveValidator:
    validator_id: str
    account_id: str
    control_group: ControlGroup

    def to_dict(self) -> dict[str, Any]:
        return {
            "validator_id": self.validator_id,
            "account_id": self.account_id,
            "control_group": self.control_group.to_dict(),
        }


@dataclass(frozen=True)
class ModelOutput:
    classification: str
    cited_fields: tuple[str, ...]
    parsed_output_root: str
    replay_certificate_root: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "classification": self.classification,
            "cited_fields": list(self.cited_fields),
            "parsed_output_root": self.parsed_output_root,
            "replay_certificate_root": self.replay_certificate_root,
        }


@dataclass(frozen=True)
class CandidateFacts:
    validator_id: str
    account_id: str
    public_key_hash: str
    reliability_bps: int | None
    operator_manifest_signed: bool | None
    domain_control_proved: bool | None
    cobalt_linkedness_safe: bool | None
    control_group: ControlGroup
    model_output: ModelOutput | None

    def to_dict(self) -> dict[str, Any]:
        return {
            "validator_id": self.validator_id,
            "account_id": self.account_id,
            "public_key_hash": self.public_key_hash,
            "reliability_bps": self.reliability_bps,
            "operator_manifest_signed": self.operator_manifest_signed,
            "domain_control_proved": self.domain_control_proved,
            "cobalt_linkedness_safe": self.cobalt_linkedness_safe,
            "control_group": self.control_group.to_dict(),
            "model_output": (
                self.model_output.to_dict()
                if self.model_output is not None
                else None
            ),
        }


@dataclass(frozen=True)
class PolicyEvidence:
    evaluation_end: str
    target_round: int
    transition_budget: int
    foundation_bound_validator_ids: tuple[str, ...]
    active_validators: tuple[ActiveValidator, ...]
    candidates: tuple[CandidateFacts, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": SHADOW_POLICY_EVIDENCE_SCHEMA,
            "mode": SHADOW_MODE,
            "evaluation_end": self.evaluation_end,
            "target_round": self.target_round,
            "transition_budget": self.transition_budget,
            "foundation_bound_validator_ids": list(
                self.foundation_bound_validator_ids
            ),
            "active_validators": [
                active.to_dict() for active in self.active_validators
            ],
            "candidates": [
                candidate.to_dict() for candidate in self.candidates
            ],
        }


def _sha256_document(value: object, domain: str) -> str:
    return hashlib.sha256(
        domain.encode("ascii") + b"\x00" + canonical_json_bytes(value)
    ).hexdigest()


def _require_hex(value: object, field: str, length: int = 64) -> str:
    if (
        not isinstance(value, str)
        or len(value) != length
        or value != value.lower()
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise TaskNodeUnlError("invalid_lower_hex", field)
    return value


def _optional_bool(value: object, field: str) -> bool | None:
    if value is None:
        return None
    if not isinstance(value, bool):
        raise TaskNodeUnlError("invalid_boolean", field)
    return value


def _require_mapping(value: object, field: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise TaskNodeUnlError("invalid_object", field)
    return value


def _require_list(value: object, field: str) -> list[Any]:
    if not isinstance(value, list):
        raise TaskNodeUnlError("invalid_array", field)
    return value


def _optional_bounded_int(
    value: object,
    field: str,
    *,
    maximum: int,
) -> int | None:
    if value is None:
        return None
    checked = require_int(value, field, minimum=0)
    if checked > maximum:
        raise TaskNodeUnlError("integer_above_maximum", field)
    return checked


def _reject_forbidden_inputs(value: object, path: str = "policy_evidence") -> None:
    if isinstance(value, Mapping):
        for key in sorted(value):
            if not isinstance(key, str):
                raise TaskNodeUnlError("invalid_field_name", path)
            if key in _FORBIDDEN_INPUT_KEYS:
                raise TaskNodeUnlError("forbidden_rule_input", f"{path}.{key}")
            _reject_forbidden_inputs(value[key], f"{path}.{key}")
    elif isinstance(value, list):
        for index, item in enumerate(value):
            _reject_forbidden_inputs(item, f"{path}[{index}]")


def _control_group(value: object, field: str, validator_id: str) -> ControlGroup:
    row = require_closed_keys(
        value,
        required=(
            "operator_group",
            "release_manager_group",
            "key_management_group",
            "funding_source_group",
        ),
        field=field,
    )
    return ControlGroup(
        validator_id=validator_id,
        operator_group=require_identifier(
            row["operator_group"], f"{field}.operator_group"
        ),
        release_manager_group=require_identifier(
            row["release_manager_group"],
            f"{field}.release_manager_group",
        ),
        key_management_group=require_identifier(
            row["key_management_group"],
            f"{field}.key_management_group",
        ),
        funding_source_group=require_identifier(
            row["funding_source_group"],
            f"{field}.funding_source_group",
        ),
    )


def _model_output(value: object, field: str) -> ModelOutput | None:
    if value is None:
        return None
    row = require_closed_keys(
        value,
        required=(
            "classification",
            "cited_fields",
            "parsed_output_root",
            "replay_certificate_root",
        ),
        field=field,
    )
    cited = row["cited_fields"]
    if not isinstance(cited, list):
        raise TaskNodeUnlError("invalid_array", f"{field}.cited_fields")
    cited_fields = tuple(
        require_identifier(item, f"{field}.cited_fields[{index}]")
        for index, item in enumerate(cited)
    )
    if cited_fields != tuple(sorted(set(cited_fields))):
        raise TaskNodeUnlError(
            "non_canonical_order", f"{field}.cited_fields"
        )
    return ModelOutput(
        classification=require_identifier(
            row["classification"], f"{field}.classification"
        ),
        cited_fields=cited_fields,
        parsed_output_root=_require_hex(
            row["parsed_output_root"], f"{field}.parsed_output_root"
        ),
        replay_certificate_root=_require_hex(
            row["replay_certificate_root"],
            f"{field}.replay_certificate_root",
        ),
    )


def _active_validator(value: object, index: int) -> ActiveValidator:
    field = f"policy_evidence.active_validators[{index}]"
    row = require_closed_keys(
        value,
        required=("validator_id", "account_id", "control_group"),
        field=field,
    )
    validator_id = require_identifier(
        row["validator_id"], f"{field}.validator_id"
    )
    return ActiveValidator(
        validator_id=validator_id,
        account_id=require_identifier(
            row["account_id"], f"{field}.account_id"
        ),
        control_group=_control_group(
            row["control_group"], f"{field}.control_group", validator_id
        ),
    )


def _candidate(value: object, index: int) -> CandidateFacts:
    field = f"policy_evidence.candidates[{index}]"
    row = require_closed_keys(
        value,
        required=(
            "validator_id",
            "account_id",
            "public_key_hash",
            "reliability_bps",
            "operator_manifest_signed",
            "domain_control_proved",
            "cobalt_linkedness_safe",
            "control_group",
            "model_output",
        ),
        field=field,
    )
    validator_id = require_identifier(
        row["validator_id"], f"{field}.validator_id"
    )
    return CandidateFacts(
        validator_id=validator_id,
        account_id=require_identifier(
            row["account_id"], f"{field}.account_id"
        ),
        public_key_hash=_require_hex(
            row["public_key_hash"], f"{field}.public_key_hash"
        ),
        reliability_bps=_optional_bounded_int(
            row["reliability_bps"],
            f"{field}.reliability_bps",
            maximum=10_000,
        ),
        operator_manifest_signed=_optional_bool(
            row["operator_manifest_signed"],
            f"{field}.operator_manifest_signed",
        ),
        domain_control_proved=_optional_bool(
            row["domain_control_proved"],
            f"{field}.domain_control_proved",
        ),
        cobalt_linkedness_safe=_optional_bool(
            row["cobalt_linkedness_safe"],
            f"{field}.cobalt_linkedness_safe",
        ),
        control_group=_control_group(
            row["control_group"], f"{field}.control_group", validator_id
        ),
        model_output=_model_output(
            row["model_output"], f"{field}.model_output"
        ),
    )


def policy_evidence_from_dict(document: object) -> PolicyEvidence:
    _reject_forbidden_inputs(document)
    row = require_closed_keys(
        document,
        required=(
            "schema",
            "mode",
            "evaluation_end",
            "target_round",
            "transition_budget",
            "foundation_bound_validator_ids",
            "active_validators",
            "candidates",
        ),
        field="policy_evidence",
    )
    if row["schema"] != SHADOW_POLICY_EVIDENCE_SCHEMA:
        raise TaskNodeUnlError("unknown_schema", "policy_evidence.schema")
    if row["mode"] != SHADOW_MODE:
        raise TaskNodeUnlError("mode_mismatch", "policy_evidence.mode")
    evaluation_end = require_identifier(
        row["evaluation_end"], "policy_evidence.evaluation_end"
    )
    active_values = row["active_validators"]
    candidate_values = row["candidates"]
    foundation_values = row["foundation_bound_validator_ids"]
    if not isinstance(active_values, list):
        raise TaskNodeUnlError(
            "invalid_array", "policy_evidence.active_validators"
        )
    if len(active_values) > _MAX_ACTIVE_VALIDATORS:
        raise TaskNodeUnlError(
            "array_too_large", "policy_evidence.active_validators"
        )
    if not isinstance(candidate_values, list):
        raise TaskNodeUnlError(
            "invalid_array", "policy_evidence.candidates"
        )
    if not candidate_values:
        raise TaskNodeUnlError("empty_candidates", "policy_evidence.candidates")
    if len(candidate_values) > _MAX_CANDIDATES:
        raise TaskNodeUnlError(
            "array_too_large", "policy_evidence.candidates"
        )
    if not isinstance(foundation_values, list):
        raise TaskNodeUnlError(
            "invalid_array",
            "policy_evidence.foundation_bound_validator_ids",
        )

    active = tuple(
        sorted(
            (
                _active_validator(value, index)
                for index, value in enumerate(active_values)
            ),
            key=lambda item: item.validator_id,
        )
    )
    candidates = tuple(
        sorted(
            (
                _candidate(value, index)
                for index, value in enumerate(candidate_values)
            ),
            key=lambda item: item.validator_id,
        )
    )
    foundations = tuple(
        sorted(
            {
                require_identifier(
                    value,
                    (
                        "policy_evidence.foundation_bound_validator_ids"
                        f"[{index}]"
                    ),
                )
                for index, value in enumerate(foundation_values)
            }
        )
    )
    active_ids = [item.validator_id for item in active]
    active_accounts = [item.account_id for item in active]
    candidate_ids = [item.validator_id for item in candidates]
    candidate_accounts = [item.account_id for item in candidates]
    for values, code in (
        (active_ids, "duplicate_active_validator"),
        (active_accounts, "duplicate_active_account"),
        (candidate_ids, "duplicate_candidate_validator"),
        (candidate_accounts, "duplicate_candidate_account"),
    ):
        if len(values) != len(set(values)):
            raise TaskNodeUnlError(code)
    if set(active_ids) & set(candidate_ids):
        raise TaskNodeUnlError("candidate_already_active")
    if set(active_accounts) & set(candidate_accounts):
        raise TaskNodeUnlError("candidate_account_already_active")
    if not set(foundations).issubset(active_ids):
        raise TaskNodeUnlError("unknown_foundation_bound_validator")
    return PolicyEvidence(
        evaluation_end=evaluation_end,
        target_round=require_int(
            row["target_round"],
            "policy_evidence.target_round",
            minimum=1,
        ),
        transition_budget=require_int(
            row["transition_budget"],
            "policy_evidence.transition_budget",
            minimum=1,
        ),
        foundation_bound_validator_ids=foundations,
        active_validators=active,
        candidates=candidates,
    )


def _bundle_rows(
    document: object,
    *,
    schema: str,
    field: str,
    array_field: str,
) -> tuple[Mapping[str, Any], ...]:
    row = require_closed_keys(
        document,
        required=("schema", "mode", array_field),
        field=field,
    )
    if row["schema"] != schema:
        raise TaskNodeUnlError("unknown_schema", f"{field}.schema")
    if row["mode"] != SHADOW_MODE:
        raise TaskNodeUnlError("mode_mismatch", f"{field}.mode")
    values = row[array_field]
    if not isinstance(values, list):
        raise TaskNodeUnlError("invalid_array", f"{field}.{array_field}")
    if len(values) > _MAX_CANDIDATES:
        raise TaskNodeUnlError("array_too_large", f"{field}.{array_field}")
    result: list[Mapping[str, Any]] = []
    for index, value in enumerate(values):
        if not isinstance(value, Mapping):
            raise TaskNodeUnlError(
                "invalid_object", f"{field}.{array_field}[{index}]"
            )
        result.append(value)
    return tuple(result)


def _by_account(
    values: Sequence[Mapping[str, Any]],
    *,
    account_path: Sequence[str],
    field: str,
) -> dict[str, Mapping[str, Any]]:
    result: dict[str, Mapping[str, Any]] = {}
    for index, value in enumerate(values):
        selected: object = value
        for component in account_path:
            if not isinstance(selected, Mapping) or component not in selected:
                raise TaskNodeUnlError(
                    "missing_field",
                    f"{field}[{index}].{'.'.join(account_path)}",
                )
            selected = selected[component]
        account_id = require_identifier(
            selected,
            f"{field}[{index}].{'.'.join(account_path)}",
        )
        if account_id in result:
            raise TaskNodeUnlError("duplicate_account_document", account_id)
        result[account_id] = value
    return result


def _accountability_document(
    digest: Mapping[str, Any],
) -> Mapping[str, Any]:
    body = _require_mapping(digest.get("body"), "work_digest.body")
    pointers = _require_list(
        body.get("pointers"), "work_digest.body.pointers"
    )
    tasks = []
    for index, value in enumerate(pointers):
        pointer = _require_mapping(
            value, f"work_digest.body.pointers[{index}]"
        )
        outcome = _require_mapping(
            pointer.get("outcome"),
            f"work_digest.body.pointers[{index}].outcome",
        )
        tasks.append(
            {
                "task_id": outcome["task_id"],
                "kind": outcome["kind"],
                "accepted_at": outcome["accepted_at"],
                "verification_outcome": outcome["verification_outcome"],
                "verified_at": outcome["verified_at"],
                "rewarded_at": outcome["rewarded_at"],
            }
        )
    window = _require_mapping(body.get("window"), "work_digest.body.window")
    return {
        "schema": ACCOUNTABILITY_INPUT_SCHEMA,
        "window_end": window["end"],
        "tasks": tasks,
        "disputes": body["disputes"],
        "badge": body["badge"],
    }


def _active_binding_map(
    result: BindingReplayResult,
) -> dict[str, ActiveBinding]:
    return {
        binding.validator_id: binding
        for binding in result.active_bindings
    }


def _binding_issue_codes(
    result: BindingReplayResult,
    validator_id: str,
) -> tuple[str, ...]:
    scoped = tuple(
        sorted(
            reason
            for reason in result.hold_reasons
            if reason.endswith(f":{validator_id}")
        )
    )
    generic = tuple(
        sorted(
            reason for reason in result.hold_reasons if ":" not in reason
        )
    )
    return tuple(sorted(set(scoped + generic)))


def _funding_links(
    edges: EdgeExtractionResult,
    candidate_account: str,
    active_accounts: frozenset[str],
) -> tuple[str, ...]:
    linked: set[str] = set()
    for edge in edges.edges:
        if edge.kind != "funding":
            continue
        if edge.source == candidate_account and edge.target in active_accounts:
            linked.add(edge.target)
        if edge.target == candidate_account and edge.source in active_accounts:
            linked.add(edge.source)
    return tuple(sorted(linked))


def _control_conflicts(
    candidate: CandidateFacts,
    active: Sequence[ActiveValidator],
) -> tuple[tuple[str, str, str], ...]:
    conflicts: list[tuple[str, str, str]] = []
    for current in active:
        for attribute, field, reason in _CONTROL_FIELDS:
            candidate_value = getattr(candidate.control_group, attribute)
            active_value = getattr(current.control_group, attribute)
            if candidate_value and candidate_value == active_value:
                conflicts.append((reason, field, current.validator_id))
    return tuple(sorted(conflicts))


def _graph_projection(
    account_id: str,
    graph: TrustGraphResult | None,
) -> dict[str, Any]:
    if graph is None:
        return {
            "status": "hold",
            "stationary_mass": None,
            "connectivity_floor": None,
            "cluster_id": None,
            "cluster_members": [],
            "current_cluster_seats": None,
            "projected_cluster_seats": None,
            "cluster_seat_cap": None,
            "reason_codes": ["trust_graph_unavailable"],
        }
    mass = dict(graph.stationary_mass).get(account_id)
    cluster = next(
        (
            item
            for item in graph.clusters
            if account_id in item.members
        ),
        None,
    )
    reasons: list[str] = []
    if graph.status != "scored":
        reasons.extend(graph.hold_reasons)
    if account_id in graph.connectivity_holds:
        reasons.append("connectivity_below_floor")
    projected = None
    if cluster is None:
        reasons.append("cluster_assignment_missing")
    else:
        projected = cluster.seat_count + 1
        if Fraction(projected, 1) > graph.cluster_seat_cap:
            reasons.append("cluster_seat_cap_exceeded")
    return {
        "status": "hold" if reasons else "pass",
        "stationary_mass": mass,
        "connectivity_floor": graph.connectivity_mass_floor,
        "cluster_id": cluster.cluster_id if cluster is not None else None,
        "cluster_members": (
            list(cluster.members) if cluster is not None else []
        ),
        "current_cluster_seats": (
            cluster.seat_count if cluster is not None else None
        ),
        "projected_cluster_seats": projected,
        "cluster_seat_cap": graph.cluster_seat_cap,
        "reason_codes": sorted(set(reasons)),
    }


def _reason(
    code: str,
    field: str,
    evidence_references: Sequence[str],
    detail: str = "",
) -> dict[str, Any]:
    return {
        "code": code,
        "field": field,
        "detail": detail,
        "evidence_references": sorted(set(evidence_references)),
    }


def _evidence_ref(
    field_id: str,
    source_hash: str,
    *,
    missing: bool = False,
    stale: bool = False,
    conflicting: bool = False,
) -> dict[str, Any]:
    return {
        "field_id": field_id,
        "source_hash": source_hash,
        "missing": missing,
        "stale": stale,
        "conflicting": conflicting,
    }


def _policy_packet(
    candidate: CandidateFacts,
    policy: PolicyEvidence,
    registry_root: str,
    accountability_score: int | None,
    rho_score: int | None,
    roots: Mapping[str, str],
) -> dict[str, Any]:
    accountability_missing = accountability_score is None
    rho_missing = rho_score is None
    references = (
        _evidence_ref(
            FIELD_ACCOUNTABILITY,
            roots["work_digest_verifications"],
            missing=accountability_missing,
        ),
        _evidence_ref(
            FIELD_COBALT_LINKEDNESS,
            roots["trust_graph"],
            missing=candidate.cobalt_linkedness_safe is None,
        ),
        _evidence_ref(
            FIELD_DOMAIN_CONTROL,
            roots["policy_evidence"],
            missing=candidate.domain_control_proved is None,
        ),
        _evidence_ref(FIELD_FUNDING_SOURCE, roots["public_edges"]),
        _evidence_ref(FIELD_KEY_MANAGEMENT, roots["policy_evidence"]),
        _evidence_ref(
            FIELD_MODEL_CLASSIFICATION,
            roots["policy_evidence"],
            missing=candidate.model_output is None,
        ),
        _evidence_ref(FIELD_OPERATOR_GROUP, roots["policy_evidence"]),
        _evidence_ref(
            FIELD_OPERATOR_MANIFEST,
            roots["policy_evidence"],
            missing=candidate.operator_manifest_signed is None,
        ),
        _evidence_ref(FIELD_RELEASE_MANAGER, roots["policy_evidence"]),
        _evidence_ref(
            FIELD_RELIABILITY,
            roots["policy_evidence"],
            missing=candidate.reliability_bps is None,
        ),
        _evidence_ref(
            FIELD_RHO,
            roots["independence"],
            missing=rho_missing,
        ),
    )
    return {
        "packet_id": f"tasknode-unl-shadow-{candidate.validator_id}",
        "registry_root": registry_root,
        "candidate": {
            "validator_id": candidate.validator_id,
            "public_key_hash": candidate.public_key_hash,
            "reliability_bps": candidate.reliability_bps,
            "accountability_score": accountability_score,
            "rho_score": rho_score,
            "operator_manifest_signed": (
                candidate.operator_manifest_signed
            ),
            "domain_control_proved": candidate.domain_control_proved,
            "cobalt_linkedness_safe": candidate.cobalt_linkedness_safe,
            "control_group": candidate.control_group.to_dict(),
        },
        "active_validators": [
            item.control_group.to_dict() for item in policy.active_validators
        ],
        "evidence_refs": sorted(
            references, key=lambda item: item["field_id"]
        ),
        "model_output": (
            candidate.model_output.to_dict()
            if candidate.model_output is not None
            else None
        ),
    }


def _evaluate_v1_projection(
    packet: Mapping[str, Any],
    upstream_reasons: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    candidate = _require_mapping(
        packet.get("candidate"), "evidence_packet.candidate"
    )
    holds: set[str] = {
        str(reason["code"]) for reason in upstream_reasons
    }
    rejects: set[str] = set()
    failed: set[str] = {
        str(reason["field"]) for reason in upstream_reasons
    }
    followup: set[str] = set()
    correlation: set[str] = {str(candidate["validator_id"])}

    references = _require_list(
        packet.get("evidence_refs"), "evidence_packet.evidence_refs"
    )
    for index, value in enumerate(references):
        reference = _require_mapping(
            value, f"evidence_packet.evidence_refs[{index}]"
        )
        field_id = str(reference["field_id"])
        if reference["missing"]:
            holds.add("missing_required_evidence")
            failed.add(field_id)
            followup.add(field_id)
        if reference["stale"]:
            holds.add("stale_required_evidence")
            failed.add(field_id)
            followup.add(field_id)
        if reference["conflicting"]:
            holds.add("conflicting_required_evidence")
            failed.add(field_id)
            followup.add(field_id)

    reliability = candidate["reliability_bps"]
    if reliability is None:
        holds.add("missing_reliability")
        failed.add(FIELD_RELIABILITY)
        followup.add(FIELD_RELIABILITY)
    elif reliability < V1_MIN_RELIABILITY_BPS:
        rejects.add("reliability_below_floor")
        failed.add(FIELD_RELIABILITY)

    accountability = candidate["accountability_score"]
    if accountability is None:
        holds.add("missing_accountability")
        failed.add(FIELD_ACCOUNTABILITY)
        followup.add(FIELD_ACCOUNTABILITY)
    elif accountability < ACCOUNTABILITY_FLOOR:
        rejects.add("accountability_below_floor")
        failed.add(FIELD_ACCOUNTABILITY)

    rho = candidate["rho_score"]
    if rho is None:
        holds.add("missing_rho")
        failed.add(FIELD_RHO)
        followup.add(FIELD_RHO)
    elif rho > V1_MAX_RHO_SCORE:
        rejects.add("rho_above_cap")
        failed.add(FIELD_RHO)

    for value_field, evidence_field, missing_code, false_code in (
        (
            "operator_manifest_signed",
            FIELD_OPERATOR_MANIFEST,
            "missing_operator_manifest_signature",
            "operator_manifest_signature_false",
        ),
        (
            "domain_control_proved",
            FIELD_DOMAIN_CONTROL,
            "missing_domain_control",
            "domain_control_false",
        ),
        (
            "cobalt_linkedness_safe",
            FIELD_COBALT_LINKEDNESS,
            "missing_cobalt_linkedness",
            "cobalt_linkedness_unsafe",
        ),
    ):
        value = candidate[value_field]
        if value is None:
            holds.add(missing_code)
            failed.add(evidence_field)
            followup.add(evidence_field)
        elif value is False:
            rejects.add(false_code)
            failed.add(evidence_field)

    control = _require_mapping(
        candidate.get("control_group"), "evidence_packet.candidate.control_group"
    )
    active_values = _require_list(
        packet.get("active_validators"), "evidence_packet.active_validators"
    )
    for index, value in enumerate(active_values):
        active = _require_mapping(
            value, f"evidence_packet.active_validators[{index}]"
        )
        for attribute, field, reason in _CONTROL_FIELDS:
            if control[attribute] == active[attribute]:
                rejects.add(reason)
                failed.add(field)
                correlation.add(str(active["validator_id"]))

    model = packet["model_output"]
    if model is None:
        holds.add("missing_model_classification")
        failed.add(FIELD_MODEL_CLASSIFICATION)
        followup.add(FIELD_MODEL_CLASSIFICATION)
    else:
        model = _require_mapping(model, "evidence_packet.model_output")
        classification = model["classification"]
        if classification != "independent":
            code = {
                "cosmetic_diversity": "model_classified_cosmetic_diversity",
                "contradictory": "model_classified_contradictory",
                "insufficient": "model_classified_insufficient",
            }.get(str(classification), "model_classification_unknown")
            holds.add(code)
            failed.add(FIELD_MODEL_CLASSIFICATION)
        for field in model["cited_fields"]:
            if field not in V1_REQUIRED_FIELDS:
                holds.add("model_cited_unknown_field")
                failed.add(str(field))
                followup.add(str(field))

    action = "reject" if rejects else ("hold" if holds else "admit")
    reasons = sorted(rejects | holds)
    if not reasons:
        reasons = ["all_gates_passed"]
    packet_root = _sha256_document(
        packet, "postfiat/tasknode-unl/shadow-policy-packet/v1"
    )
    candidate_record_hash = (
        _sha256_document(
            {
                "validator_id": candidate["validator_id"],
                "public_key_hash": candidate["public_key_hash"],
                "control_group": control,
            },
            "postfiat/tasknode-unl/shadow-candidate-record/v1",
        )
        if action == "admit"
        else None
    )
    delta = {
        "delta_kind": "add" if action == "admit" else "no_op",
        "mutation_count": 1 if action == "admit" else 0,
        "subject_validator": candidate["validator_id"],
        "previous_registry_root": packet["registry_root"],
        "candidate_record_hash": candidate_record_hash,
        "mode": SHADOW_MODE,
        "submission_supported": False,
    }
    decision_body = {
        "schema": V1_DECISION_SCHEMA,
        "policy_root": _sha256_document(
            {
                "schema": V1_POLICY_SCHEMA,
                "policy_version": 1,
                "min_reliability_bps": V1_MIN_RELIABILITY_BPS,
                "min_accountability_score": ACCOUNTABILITY_FLOOR,
                "max_rho_score": V1_MAX_RHO_SCORE,
                "max_adds_per_round": V1_MAX_ADDS_PER_ROUND,
            },
            "postfiat/tasknode-unl/shadow-policy/v1",
        ),
        "packet_root": packet_root,
        "validator_id": candidate["validator_id"],
        "action": action,
        "reason_codes": reasons,
        "failed_fields": sorted(failed),
        "correlation_cluster": sorted(correlation),
        "required_followup_evidence": sorted(followup),
        "registry_delta_candidate": delta,
    }
    return {
        **decision_body,
        "decision_id": _sha256_document(
            decision_body,
            "postfiat/tasknode-unl/shadow-decision/v1",
        ),
    }


def _require_documents(
    documents: Mapping[str, object],
) -> dict[str, object]:
    keys = set(documents)
    required = set(SHADOW_INPUT_FILES)
    missing = sorted(required - keys)
    if missing:
        raise TaskNodeUnlError("missing_input_document", missing[0])
    unknown = sorted(keys - required)
    if unknown:
        raise TaskNodeUnlError("unknown_input_document", unknown[0])
    return {key: documents[key] for key in SHADOW_INPUT_FILES}


def _registry_root(document: object) -> str:
    row = require_closed_keys(
        document,
        required=(
            "schema",
            "mode",
            "registry_round",
            "registry_root",
            "validator_ids",
        ),
        field="baseline_list",
    )
    if row["schema"] != CHURN_BASELINE_SCHEMA:
        raise TaskNodeUnlError("unknown_schema", "baseline_list.schema")
    if row["mode"] != SHADOW_MODE:
        raise TaskNodeUnlError("mode_mismatch", "baseline_list.mode")
    return _require_hex(row["registry_root"], "baseline_list.registry_root")


def _normalized_registry_history(document: object) -> dict[str, Any]:
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
        raise TaskNodeUnlError("unknown_schema", "registry_history.schema")
    if row["mode"] != SHADOW_MODE:
        raise TaskNodeUnlError("mode_mismatch", "registry_history.mode")
    values = row["rounds"]
    if not isinstance(values, list):
        raise TaskNodeUnlError("invalid_array", "registry_history.rounds")
    rounds: list[dict[str, Any]] = []
    for index, value in enumerate(values):
        field = f"registry_history.rounds[{index}]"
        view = require_closed_keys(
            value,
            required=("round", "root", "validator_ids"),
            field=field,
        )
        identifiers = view["validator_ids"]
        if not isinstance(identifiers, list):
            raise TaskNodeUnlError(
                "invalid_array", f"{field}.validator_ids"
            )
        rounds.append(
            {
                "round": require_int(
                    view["round"], f"{field}.round", minimum=1
                ),
                "root": _require_hex(view["root"], f"{field}.root"),
                "validator_ids": sorted(
                    require_identifier(
                        item, f"{field}.validator_ids[{item_index}]"
                    )
                    for item_index, item in enumerate(identifiers)
                ),
            }
        )
    return {
        "schema": row["schema"],
        "mode": row["mode"],
        "current_round": require_int(
            row["current_round"],
            "registry_history.current_round",
            minimum=1,
        ),
        "current_root": _require_hex(
            row["current_root"], "registry_history.current_root"
        ),
        "rounds": sorted(rounds, key=lambda item: item["round"]),
    }


def _baseline_ids(document: object) -> tuple[str, ...]:
    row = _require_mapping(document, "baseline_list")
    values = row["validator_ids"]
    if not isinstance(values, list):
        raise TaskNodeUnlError("invalid_array", "baseline_list.validator_ids")
    result = tuple(
        sorted(
            require_identifier(
                value, f"baseline_list.validator_ids[{index}]"
            )
            for index, value in enumerate(values)
        )
    )
    if len(result) != len(set(result)):
        raise TaskNodeUnlError(
            "duplicate_validator_id", "baseline_list.validator_ids"
        )
    return result


def derive_shadow_report(
    documents: Mapping[str, object],
) -> dict[str, Any]:
    """Compose verified local evidence into one deterministic shadow report."""

    source = _require_documents(documents)
    policy = policy_evidence_from_dict(source["policy_evidence"])
    baseline_ids = _baseline_ids(source["baseline_list"])
    registry_root = _registry_root(source["baseline_list"])
    active_ids = tuple(
        item.validator_id for item in policy.active_validators
    )
    if active_ids != baseline_ids:
        raise TaskNodeUnlError(
            "active_baseline_mismatch", "policy_evidence.active_validators"
        )

    binding_result = replay_bindings_document(source["binding_replay"])
    binding_by_validator = _active_binding_map(binding_result)
    shared_control_validators = {
        validator_id
        for _wallet, validator_ids in binding_result.shared_control_evidence
        for validator_id in validator_ids
    }

    digest_values = _bundle_rows(
        source["work_digests"],
        schema=SHADOW_WORK_DIGEST_BUNDLE_SCHEMA,
        field="work_digests",
        array_field="digests",
    )
    snapshot_values = _bundle_rows(
        source["ledger_snapshots"],
        schema=SHADOW_LEDGER_SNAPSHOT_BUNDLE_SCHEMA,
        field="ledger_snapshots",
        array_field="snapshots",
    )
    digests = _by_account(
        digest_values,
        account_path=("body", "account_id"),
        field="work_digests.digests",
    )
    snapshots = _by_account(
        snapshot_values,
        account_path=("account_id",),
        field="ledger_snapshots.snapshots",
    )

    digest_results: dict[str, WorkDigestVerificationResult] = {}
    accountabilities: dict[str, AccountabilityResult] = {}
    for candidate in policy.candidates:
        binding = binding_by_validator.get(candidate.validator_id)
        digest = digests.get(candidate.account_id)
        snapshot = snapshots.get(candidate.account_id)
        if binding is None or digest is None or snapshot is None:
            continue
        result = verify_work_digest(
            digest,
            snapshot,
            source["publishing_keys"],
            expected_account_id=candidate.account_id,
            bound_wallet_address=binding.wallet_address,
        )
        digest_results[candidate.account_id] = result
        if result.status == "verified":
            accountabilities[candidate.account_id] = (
                evaluate_accountability_document(
                    _accountability_document(digest)
                )
            )

    edge_result = extract_public_edges(
        vouch_ledger=source["vouch_ledger"],
        cowork_pointers=source["cowork_pointers"],
        funding_transfers=source["funding_transfers"],
        funding_exclusions=source["funding_exclusions"],
    )

    active_accounts = tuple(
        item.account_id for item in policy.active_validators
    )
    candidate_accounts = tuple(
        item.account_id for item in policy.candidates
    )
    graph: TrustGraphResult | None = None
    if edge_result.status == "extracted":
        active_account_by_validator = {
            item.validator_id: item.account_id
            for item in policy.active_validators
        }
        graph = derive_trust_graph(
            TrustGraphEvidence(
                nodes=tuple(sorted(active_accounts + candidate_accounts)),
                ratified_nodes=active_accounts,
                foundation_bound_nodes=tuple(
                    sorted(
                        active_account_by_validator[validator_id]
                        for validator_id
                        in policy.foundation_bound_validator_ids
                    )
                ),
                edges=edge_result.edges,
                baseline_list_size=len(baseline_ids),
                seats=tuple(
                    SeatAssignment(item.validator_id, item.account_id)
                    for item in policy.active_validators
                ),
            )
        )

    policy_root = _sha256_document(
        policy.to_dict(), "postfiat/tasknode-unl/policy-evidence/v1"
    )
    binding_root = _sha256_document(
        binding_result.to_dict(), "postfiat/tasknode-unl/bindings/v1"
    )
    digest_root = _sha256_document(
        [
            {
                "account_id": account,
                "result": digest_results[account].to_dict(),
            }
            for account in sorted(digest_results)
        ],
        "postfiat/tasknode-unl/work-digest-verifications/v1",
    )
    edge_root = _sha256_document(
        edge_result.to_dict(), "postfiat/tasknode-unl/public-edges/v1"
    )
    graph_root = _sha256_document(
        graph.to_dict() if graph is not None else {"status": "hold"},
        "postfiat/tasknode-unl/trust-graph/v1",
    )
    baseline_root = _sha256_document(
        {
            "registry_root": registry_root,
            "validator_ids": list(baseline_ids),
        },
        "postfiat/tasknode-unl/baseline-list/v1",
    )
    registry_history_root = _sha256_document(
        _normalized_registry_history(source["registry_history"]),
        "postfiat/tasknode-unl/registry-history/v1",
    )

    candidate_intermediate: list[dict[str, Any]] = []
    independence_rows: list[dict[str, Any]] = []
    active_account_set = frozenset(active_accounts)
    for candidate in policy.candidates:
        binding = binding_by_validator.get(candidate.validator_id)
        binding_issues = _binding_issue_codes(
            binding_result, candidate.validator_id
        )
        digest_result = digest_results.get(candidate.account_id)
        accountability = accountabilities.get(candidate.account_id)
        graph_projection = _graph_projection(candidate.account_id, graph)
        funding_links = _funding_links(
            edge_result, candidate.account_id, active_account_set
        )
        control_conflicts = _control_conflicts(
            candidate, policy.active_validators
        )
        independence_complete = (
            binding is not None
            and not binding_issues
            and candidate.validator_id not in shared_control_validators
            and edge_result.status == "extracted"
        )
        correlation_detected = bool(
            funding_links
            or control_conflicts
            or candidate.validator_id in shared_control_validators
        )
        rho_score = (
            None
            if not independence_complete
            else (1 if correlation_detected else 0)
        )
        independence = {
            "status": (
                "incomplete"
                if not independence_complete
                else ("correlated" if correlation_detected else "independent")
            ),
            "one_wallet_one_validator": (
                "pass"
                if binding is not None
                and not binding_issues
                and candidate.validator_id not in shared_control_validators
                else "hold"
            ),
            "shared_funding_accounts": list(funding_links),
            "control_group_conflicts": [
                {
                    "reason_code": reason,
                    "field": field,
                    "active_validator_id": active_validator,
                }
                for reason, field, active_validator in control_conflicts
            ],
            "rho_score": rho_score,
        }
        independence_rows.append(
            {
                "validator_id": candidate.validator_id,
                "independence": independence,
            }
        )
        candidate_intermediate.append(
            {
                "facts": candidate,
                "binding": binding,
                "binding_issues": binding_issues,
                "digest_result": digest_result,
                "accountability": accountability,
                "graph": graph_projection,
                "independence": independence,
            }
        )

    independence_root = _sha256_document(
        independence_rows,
        "postfiat/tasknode-unl/independence/v1",
    )
    roots = {
        "baseline_list": baseline_root,
        "binding_replay": binding_root,
        "independence": independence_root,
        "policy_evidence": policy_root,
        "public_edges": edge_root,
        "registry_history": registry_history_root,
        "trust_graph": graph_root,
        "work_digest_verifications": digest_root,
    }

    candidates: list[dict[str, Any]] = []
    eligible_ids: list[str] = []
    for intermediate in candidate_intermediate:
        candidate = intermediate["facts"]
        if not isinstance(candidate, CandidateFacts):
            raise TaskNodeUnlError("invalid_internal_candidate")
        binding = intermediate["binding"]
        binding_issues = intermediate["binding_issues"]
        if not isinstance(binding_issues, tuple):
            raise TaskNodeUnlError("invalid_internal_binding_issues")
        digest_result = intermediate["digest_result"]
        accountability = intermediate["accountability"]
        graph_projection = _require_mapping(
            intermediate["graph"], "candidate.trust_graph"
        )
        independence = _require_mapping(
            intermediate["independence"], "candidate.independence"
        )
        upstream: list[dict[str, Any]] = []

        if binding is None:
            upstream.append(
                _reason(
                    "binding_missing",
                    "validator.identity.tasknode_binding",
                    ("input_roots.binding_replay",),
                )
            )
        for issue in binding_issues:
            upstream.append(
                _reason(
                    "binding_replay_hold",
                    "validator.identity.tasknode_binding",
                    ("input_roots.binding_replay",),
                    issue,
                )
            )
        if candidate.validator_id in shared_control_validators:
            upstream.append(
                _reason(
                    "wallet_shared_control",
                    "validator.identity.tasknode_binding.wallet_address",
                    ("input_roots.binding_replay",),
                )
            )
        if digest_result is None:
            upstream.append(
                _reason(
                    "work_digest_missing",
                    "validator.admission.accountability_score",
                    ("input_roots.work_digest_verifications",),
                )
            )
        elif digest_result.status != "verified":
            for failure in digest_result.failures:
                upstream.append(
                    _reason(
                        f"work_digest_{failure.code}",
                        failure.field,
                        ("input_roots.work_digest_verifications",),
                        failure.detail,
                    )
                )
        for code in graph_projection["reason_codes"]:
            upstream.append(
                _reason(
                    str(code),
                    (
                        "validator.trust_graph.stationary_mass"
                        if code == "connectivity_below_floor"
                        else "validator.trust_graph.cluster_seat_cap"
                    ),
                    ("input_roots.trust_graph",),
                )
            )
        if edge_result.status != "extracted":
            for reason in edge_result.hold_reasons:
                upstream.append(
                    _reason(
                        "edge_extraction_hold",
                        "validator.trust_graph.edges",
                        ("input_roots.public_edges",),
                        reason,
                    )
                )

        accountability_score = (
            accountability.calculation.projected_score
            if accountability is not None
            and accountability.calculation is not None
            else None
        )
        rho_score = independence["rho_score"]
        if rho_score is not None and (
            isinstance(rho_score, bool) or not isinstance(rho_score, int)
        ):
            raise TaskNodeUnlError("invalid_internal_rho_score")
        packet = _policy_packet(
            candidate,
            policy,
            registry_root,
            accountability_score,
            rho_score,
            roots,
        )
        decision = _evaluate_v1_projection(packet, upstream)
        status = decision["action"]
        if status == "admit":
            eligible_ids.append(candidate.validator_id)
        candidates.append(
            {
                "validator_id": candidate.validator_id,
                "account_id": candidate.account_id,
                "status": status,
                "binding": {
                    "status": (
                        "verified"
                        if binding is not None and not binding_issues
                        else "hold"
                    ),
                    "wallet_address": (
                        binding.wallet_address
                        if isinstance(binding, ActiveBinding)
                        else None
                    ),
                    "binding_tx_hash": (
                        binding.tx_hash
                        if isinstance(binding, ActiveBinding)
                        else None
                    ),
                },
                "work_digest": (
                    digest_result.to_dict()
                    if isinstance(
                        digest_result, WorkDigestVerificationResult
                    )
                    else {
                        "status": "hold",
                        "failures": [
                            {
                                "field": "work_digest",
                                "code": "work_digest_missing",
                                "detail": "",
                            }
                        ],
                    }
                ),
                "accountability": (
                    accountability.to_dict()
                    if isinstance(accountability, AccountabilityResult)
                    else None
                ),
                "trust_graph": dict(graph_projection),
                "independence": dict(independence),
                "upstream_holds": sorted(
                    upstream,
                    key=lambda item: (
                        item["code"],
                        item["field"],
                        item["detail"],
                    ),
                ),
                "evidence_packet": packet,
                "admission_decision": decision,
            }
        )

    eligible_ids.sort()
    selected = eligible_ids[:V1_MAX_ADDS_PER_ROUND]
    proposed_ids = tuple(sorted(set(baseline_ids) | set(selected)))
    history = _require_mapping(
        source["registry_history"], "registry_history"
    )
    current_round = require_int(
        history.get("current_round"),
        "registry_history.current_round",
        minimum=1,
    )
    current_root = _require_hex(
        history.get("current_root"), "registry_history.current_root"
    )
    proposal = {
        "schema": CHURN_PROPOSAL_SCHEMA,
        "mode": SHADOW_MODE,
        "source_round": current_round,
        "source_registry_root": current_root,
        "target_round": policy.target_round,
        "proposed_validator_ids": list(proposed_ids),
        "transition_budget": policy.transition_budget,
        "evaluation_time": policy.evaluation_end,
        "identity_failures": [],
        "removal_causes": [],
    }
    churn = evaluate_churn_guard(
        source["baseline_list"], source["registry_history"], proposal
    )
    final_ids = list(proposed_ids) if churn.status == "allow" else list(baseline_ids)

    candidate_by_id = {
        item["validator_id"]: item for item in candidates
    }
    additions = []
    for validator_id in sorted(set(final_ids) - set(baseline_ids)):
        selected_candidate = candidate_by_id[validator_id]
        decision = selected_candidate["admission_decision"]
        additions.append(
            {
                "validator_id": validator_id,
                "reason_codes": [
                    "eligible_admission_candidate",
                    *decision["reason_codes"],
                    "selected_by_canonical_order",
                    "churn_guard_allow",
                ],
                "evidence_references": [
                    "input_roots.binding_replay",
                    "input_roots.work_digest_verifications",
                    "input_roots.public_edges",
                    "input_roots.trust_graph",
                    "input_roots.policy_evidence",
                    "churn_guard",
                ],
            }
        )
    removals = [
        {
            "validator_id": validator_id,
            "reason_codes": ["churn_guard_approved_removal"],
            "evidence_references": ["churn_guard"],
        }
        for validator_id in sorted(set(baseline_ids) - set(final_ids))
    ]
    holds = [
        {
            "validator_id": item["validator_id"],
            "reason_codes": item["admission_decision"]["reason_codes"],
            "evidence_references": sorted(
                {
                    reference
                    for reason in item["upstream_holds"]
                    for reference in reason["evidence_references"]
                }
                or {"evidence_packet"}
            ),
        }
        for item in candidates
        if item["status"] == "hold"
    ]
    rejections = [
        {
            "validator_id": item["validator_id"],
            "reason_codes": item["admission_decision"]["reason_codes"],
            "failed_fields": item["admission_decision"]["failed_fields"],
        }
        for item in candidates
        if item["status"] == "reject"
    ]

    report = {
        "schema": SHADOW_REPORT_SCHEMA,
        "mode": SHADOW_MODE,
        "authority_boundary": {
            "live_authority": "none",
            "registry_write_supported": False,
            "transaction_submission_supported": False,
            "ratification_supported": False,
            "signable_delta_emitted": False,
            "notice": (
                "Decision support only; this report authorizes no live action."
            ),
        },
        "input_roots": roots,
        "constants": {
            "accountability_floor": ACCOUNTABILITY_FLOOR,
            "rho_maximum": V1_MAX_RHO_SCORE,
            "reliability_floor_bps": V1_MIN_RELIABILITY_BPS,
            "vouch_weight": VOUCH_EDGE_WEIGHT,
            "cowork_weight_per_shared_unit": COWORK_EDGE_WEIGHT,
            "cowork_weight_cap": COWORK_EDGE_CAP,
            "funding_weight": FUNDING_EDGE_WEIGHT,
            "walk_iterations": TRUST_WALK_ITERATIONS,
            "walk_damping": TRUST_WALK_DAMPING,
            "walk_seed_damping": TRUST_WALK_SEED_DAMPING,
            "conductance_cut_threshold": CONDUCTANCE_CUT_THRESHOLD,
            "connectivity_floor_formula": (
                f"1/({CONNECTIVITY_FLOOR_DIVISOR}N)"
            ),
            "cluster_seat_cap_formula": (
                f"max({MIN_CLUSTER_SEATS},{fraction_document(CLUSTER_SEAT_FRACTION)})"
            ),
        },
        "binding_replay": binding_result.to_dict(),
        "edge_extraction": edge_result.to_dict(),
        "trust_graph": (
            graph.to_dict() if graph is not None else {"status": "hold"}
        ),
        "candidates": candidates,
        "eligible_set": eligible_ids,
        "selection": {
            "rule": "canonical_validator_id",
            "admission_max_adds_per_round": V1_MAX_ADDS_PER_ROUND,
            "selected_additions": selected,
            "eligible_not_selected": eligible_ids[V1_MAX_ADDS_PER_ROUND:],
        },
        "baseline_validator_ids": list(baseline_ids),
        "shadow_candidate_list": final_ids,
        "baseline_diff": {
            "additions": additions,
            "removals": removals,
            "holds": holds,
            "rejections": rejections,
        },
        "churn_guard": churn.to_dict(),
    }
    report["report_hash"] = _sha256_document(
        report, "postfiat/tasknode-unl/shadow-report/v1"
    )
    return report


def render_shadow_markdown(report: object) -> str:
    """Render one verified in-memory shadow report for an operator."""

    row = require_closed_keys(
        report,
        required=(
            "schema",
            "mode",
            "authority_boundary",
            "input_roots",
            "constants",
            "binding_replay",
            "edge_extraction",
            "trust_graph",
            "candidates",
            "eligible_set",
            "selection",
            "baseline_validator_ids",
            "shadow_candidate_list",
            "baseline_diff",
            "churn_guard",
            "report_hash",
        ),
        field="shadow_report",
    )
    if row["schema"] != SHADOW_REPORT_SCHEMA or row["mode"] != SHADOW_MODE:
        raise TaskNodeUnlError("invalid_shadow_report")
    authority = row["authority_boundary"]
    if not isinstance(authority, Mapping):
        raise TaskNodeUnlError("invalid_object", "authority_boundary")
    if any(
        authority.get(field) is not False
        for field in (
            "registry_write_supported",
            "transaction_submission_supported",
            "ratification_supported",
            "signable_delta_emitted",
        )
    ):
        raise TaskNodeUnlError("live_authority_forbidden")

    diff = row["baseline_diff"]
    if not isinstance(diff, Mapping):
        raise TaskNodeUnlError("invalid_object", "baseline_diff")
    lines = [
        "# Task Node UNL fixture shadow derivation",
        "",
        "**SHADOW_ONLY — no live authority, registry write, transaction, "
        "signable delta, or ratification.**",
        "",
        f"Report hash: `{row['report_hash']}`",
        "",
        "## Proposed change",
        "",
    ]
    additions = diff.get("additions")
    removals = diff.get("removals")
    if not isinstance(additions, list) or not isinstance(removals, list):
        raise TaskNodeUnlError("invalid_array", "baseline_diff")
    if not additions and not removals:
        lines.append("- No change.")
    for index, value in enumerate(additions):
        addition = _require_mapping(
            value, f"baseline_diff.additions[{index}]"
        )
        reasons = "; ".join(str(item) for item in addition["reason_codes"])
        lines.append(
            f"- Add `{addition['validator_id']}` — {reasons}."
        )
    for index, value in enumerate(removals):
        removal = _require_mapping(
            value, f"baseline_diff.removals[{index}]"
        )
        reasons = "; ".join(str(item) for item in removal["reason_codes"])
        lines.append(
            f"- Remove `{removal['validator_id']}` — {reasons}."
        )

    lines.extend(["", "## Holds", ""])
    holds = diff.get("holds")
    if not isinstance(holds, list):
        raise TaskNodeUnlError("invalid_array", "baseline_diff.holds")
    if not holds:
        lines.append("- None.")
    for index, value in enumerate(holds):
        hold = _require_mapping(value, f"baseline_diff.holds[{index}]")
        reasons = "; ".join(str(item) for item in hold["reason_codes"])
        lines.append(f"- `{hold['validator_id']}` — {reasons}.")

    lines.extend(["", "## Rejections", ""])
    rejections = diff.get("rejections")
    if not isinstance(rejections, list):
        raise TaskNodeUnlError("invalid_array", "baseline_diff.rejections")
    if not rejections:
        lines.append("- None.")
    for index, value in enumerate(rejections):
        rejection = _require_mapping(
            value, f"baseline_diff.rejections[{index}]"
        )
        reasons = "; ".join(
            str(item) for item in rejection["reason_codes"]
        )
        lines.append(
            f"- `{rejection['validator_id']}` — {reasons}."
        )

    churn = _require_mapping(row["churn_guard"], "churn_guard")
    overlap = _require_mapping(churn.get("overlap"), "churn_guard.overlap")
    one = _require_mapping(
        overlap.get("one_round_behind"),
        "churn_guard.overlap.one_round_behind",
    )
    two = _require_mapping(
        overlap.get("two_rounds_behind"),
        "churn_guard.overlap.two_rounds_behind",
    )
    lines.extend(
        [
            "",
            "## Churn guard",
            "",
            f"- Verdict: `{churn['status']}`.",
            f"- One-round overlap: {one['percentage_text']}.",
            f"- Two-round overlap: {two['percentage_text']}.",
            "",
        ]
    )
    return "\n".join(lines)
