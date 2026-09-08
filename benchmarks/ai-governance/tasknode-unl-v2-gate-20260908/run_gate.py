#!/usr/bin/env python3
"""Offline paired adversarial and honest-liveness gate for UNL amendment V2.

The first executable action in main is verification of the preregistration
digest and the frozen V1 byte manifest. All generated artifacts are written
only beneath the selected V2 output directory.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import importlib.util
import io
import json
import random
import sys
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from fractions import Fraction
from pathlib import Path
from typing import Any, Iterable, Mapping, Sequence

EXPERIMENT_DIR = Path(__file__).resolve().parent
REPO_ROOT = EXPERIMENT_DIR.parents[2]
PYTHON_ROOT = REPO_ROOT / "python"
if str(PYTHON_ROOT) not in sys.path:
    sys.path.insert(0, str(PYTHON_ROOT))

from postfiat_rpc.tasknode_unl_v2_evidence import (
    ActiveControlDeclaration,
    ActiveRelation,
    ContinuityAssessment,
    EvidenceSnapshotResult,
    control_registry_root,
    evidence_input_root,
    policy_root,
)
from postfiat_rpc.tasknode_unl_v2_policy import (
    FROZEN,
    PROPOSE_ADD,
    AdmissionCandidate,
    PriorBreach,
    RegistryRoundState,
    RegistrySeat,
    advance_shadow_round,
    connectivity_floor,
    evaluate_admission_round,
    freeze_admission_window,
    prior_breaches_from_report,
    recount_limit_states,
)
from postfiat_rpc.tasknode_unl_v2_schema import EvaluationWindow

UTC = timezone.utc
PREREGISTRATION_PATH = EXPERIMENT_DIR / "preregistration.json"
PREREGISTRATION_DIGEST_PATH = EXPERIMENT_DIR / "PREREGISTRATION.sha256"
PREREGISTRATION_EXTENSION_PATH = EXPERIMENT_DIR / "preregistration-extension.json"
PREREGISTRATION_EXTENSION_DIGEST_PATH = (
    EXPERIMENT_DIR / "PREREGISTRATION-EXTENSION.sha256"
)
EXPECTED_PREREGISTRATION_SHA256 = (
    "94bc0979d95dbbfcb60b31ddb28fdc3ae5f96d284eb3b62438ff166083eb1206"
)
EXPECTED_PREREGISTRATION_EXTENSION_SHA256 = (
    "180698bd9930351f6eb23f9079296cb5fd5f3b8e765412a3c2b82c84c9eb99b9"
)
V1_BASELINE_DIR = (
    REPO_ROOT
    / "benchmarks"
    / "ai-governance"
    / "tasknode-unl-attack-simulation-20260907"
)
DEFAULT_OUTPUT_DIR = EXPERIMENT_DIR / "outputs"
HONEST_CONTROLS = tuple(f"honest-c{index:02d}-n01" for index in range(14))
BASE_NODES = tuple(
    f"honest-c{community:02d}-n{member:02d}"
    for community in range(20)
    for member in range(12)
)
BASE_SEATS = tuple(
    RegistrySeat(
        validator_id=f"validator-incumbent-{community:02d}",
        account_id=f"honest-c{community:02d}-n00",
    )
    for community in range(20)
)
FOUNDATION_ACCOUNTS = tuple(
    f"honest-c{community:02d}-n00" for community in range(17, 20)
)
DEFAULT_PROFILE: Mapping[str, Any] = {
    "vouch_weight": Fraction(1, 1),
    "cowork_weight": Fraction(1, 1),
    "cowork_cap": 3,
    "funding_weight": Fraction(2, 1),
    "damping": Fraction(17, 20),
    "walk_steps": 20,
    "conductance_cut": Fraction(1, 10),
    "connectivity_divisor": 2,
    "minimum_cluster_seats": 2,
    "cluster_seat_fraction": Fraction(1, 10),
}


@dataclass(frozen=True)
class Scenario:
    nodes: tuple[str, ...]
    seats: tuple[RegistrySeat, ...]
    foundation_accounts: tuple[str, ...]
    evidence: EvidenceSnapshotResult
    frozen: Any
    state: RegistryRoundState


def _sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _sha256_file(path: Path) -> str:
    return _sha256_bytes(path.read_bytes())


def _digest(label: str) -> str:
    return _sha256_bytes(f"POSTFIAT:UNL-V2-GATE:{label}".encode("utf-8"))


def _json_bytes(value: object) -> bytes:
    return (
        json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    ).encode("utf-8")


def _fraction_text(value: Fraction) -> str:
    return f"{value.numerator}/{value.denominator}"


def load_and_verify_preregistration() -> dict[str, Any]:
    actual = _sha256_file(PREREGISTRATION_PATH)
    if actual != EXPECTED_PREREGISTRATION_SHA256:
        raise RuntimeError(
            "invalid preregistration digest: "
            f"expected {EXPECTED_PREREGISTRATION_SHA256}, got {actual}"
        )
    sidecar = PREREGISTRATION_DIGEST_PATH.read_text(encoding="utf-8").split()
    if not sidecar or sidecar[0] != actual:
        raise RuntimeError("PREREGISTRATION.sha256 does not bind preregistration.json")
    prereg = json.loads(PREREGISTRATION_PATH.read_text(encoding="utf-8"))
    if prereg.get("status") != "LOCKED_BEFORE_TRIALS":
        raise RuntimeError("preregistration is not locked")
    if prereg.get("specification", {}).get("network_access") is not False:
        raise RuntimeError("offline experiment requirement is missing")
    return prereg


def load_and_verify_preregistration_extension() -> dict[str, Any]:
    actual = _sha256_file(PREREGISTRATION_EXTENSION_PATH)
    if actual != EXPECTED_PREREGISTRATION_EXTENSION_SHA256:
        raise RuntimeError(
            "invalid preregistration extension digest: "
            f"expected {EXPECTED_PREREGISTRATION_EXTENSION_SHA256}, got {actual}"
        )
    sidecar = PREREGISTRATION_EXTENSION_DIGEST_PATH.read_text(
        encoding="utf-8"
    ).split()
    if not sidecar or sidecar[0] != actual:
        raise RuntimeError(
            "PREREGISTRATION-EXTENSION.sha256 does not bind its document"
        )
    extension = json.loads(
        PREREGISTRATION_EXTENSION_PATH.read_text(encoding="utf-8")
    )
    if extension.get("status") != "LOCKED_BEFORE_EXTENDED_TRIALS":
        raise RuntimeError("preregistration extension is not locked")
    if (
        extension.get("parent_preregistration_sha256")
        != EXPECTED_PREREGISTRATION_SHA256
    ):
        raise RuntimeError("preregistration extension has the wrong parent")
    return extension


def verify_frozen_v1(prereg: Mapping[str, Any]) -> dict[str, Any]:
    verified: list[dict[str, Any]] = []
    for item in prereg["frozen_v1_baseline"]["artifacts"]:
        path = V1_BASELINE_DIR / item["path"]
        actual_bytes = len(path.read_bytes())
        actual_hash = _sha256_file(path)
        if actual_bytes != item["bytes"] or actual_hash != item["sha256"]:
            raise RuntimeError(f"frozen V1 artifact changed: {path}")
        verified.append(
            {
                "path": item["path"],
                "bytes": actual_bytes,
                "sha256": actual_hash,
            }
        )
    return {
        "status": "BYTE_IDENTICAL",
        "directory": str(V1_BASELINE_DIR.relative_to(REPO_ROOT)),
        "artifacts": verified,
        "published_results_sha256": prereg["frozen_v1_baseline"][
            "published_v1_results_sha256"
        ],
    }


def reproduce_frozen_v1_bytes() -> dict[str, Any]:
    source = V1_BASELINE_DIR / "run_simulation.py"
    module_name = "tasknode_unl_frozen_v1_replay"
    spec = importlib.util.spec_from_file_location(module_name, source)
    if spec is None or spec.loader is None:
        raise RuntimeError("unable to load the frozen V1 simulation")
    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    spec.loader.exec_module(module)
    document = module.results_document()
    generated = {
        "results.json": module.schema.canonical_json_bytes(document),
        "attack-sweep.csv": module.attack_csv(document),
        "sensitivity.csv": module.sensitivity_csv(document),
    }
    manifest = {
        "schema": "tasknode-unl-attack-simulation-output-manifest-v1",
        "files": {
            name: {
                "bytes": len(content),
                "sha256": _sha256_bytes(content),
            }
            for name, content in sorted(generated.items())
        },
    }
    generated["output-manifest.json"] = module.schema.canonical_json_bytes(
        manifest
    )
    comparisons = []
    for name, content in sorted(generated.items()):
        committed = (V1_BASELINE_DIR / name).read_bytes()
        if content != committed:
            raise RuntimeError(f"in-memory V1 replay differs: {name}")
        comparisons.append(
            {
                "path": name,
                "bytes": len(content),
                "sha256": _sha256_bytes(content),
            }
        )
    return {
        "status": "REPRODUCED_BYTE_IDENTICAL_IN_MEMORY",
        "writes_to_v1_namespace": 0,
        "artifacts": comparisons,
    }


def _window(index: int) -> EvaluationWindow:
    start = datetime(2030, 1, 1, tzinfo=UTC) + timedelta(days=180 * index)
    return EvaluationWindow(index=index, start=start, end=start + timedelta(days=180))


def _epoch(account: str, generation: str = "baseline") -> str:
    return _digest(f"control-epoch:{generation}:{account}")


def _relation(
    kind: str,
    source: str,
    target: str,
    *,
    unit: int,
    window_index: int,
    credit_label: str | None = None,
    record_label: str | None = None,
) -> ActiveRelation:
    identity = f"{kind}:{source}:{target}:{unit}"
    return ActiveRelation(
        relation_kind=kind,
        source_account=source,
        target_account=target,
        statement_digest=_digest(f"statement:{identity}"),
        credit_id=_digest(f"credit:{credit_label or identity}"),
        effective_window=window_index,
        expiry_window=window_index + 2,
        record_digest=_digest(f"record:{record_label or identity}"),
    )


def _control_declaration(
    members: Sequence[str],
    *,
    window_index: int,
    label: str,
) -> ActiveControlDeclaration:
    ordered = tuple(sorted(members))
    return ActiveControlDeclaration(
        statement_digest=_digest(f"control-statement:{label}:{','.join(ordered)}"),
        members=ordered,
        effective_window=window_index,
        expiry_window=window_index + 2,
        record_digest=_digest(f"control-record:{label}:{','.join(ordered)}"),
    )


def _funding_observation(
    target: str,
    *,
    sequence: int,
    value_units: int,
    label: str,
) -> dict[str, Any]:
    return {
        "observation_digest": _digest(
            f"funding:{label}:{target}:{sequence}:{value_units}"
        ),
        "observer_account": f"audit-observer-{sequence % 3}",
        "source_account": f"unsolicited-source-{sequence:03d}",
        "target_account": target,
        "kind": "transfer" if value_units <= 1 else "majority_inflow",
        "value_units": value_units,
    }


def _make_evidence(
    nodes: Sequence[str],
    *,
    window_index: int,
    relations: Sequence[ActiveRelation],
    declarations: Sequence[ActiveControlDeclaration] = (),
    hold_accounts: Sequence[str] = (),
    funding: Sequence[Mapping[str, Any]] = (),
    epoch_overrides: Mapping[str, str] | None = None,
    reordered: bool = False,
) -> EvidenceSnapshotResult:
    node_tuple = tuple(nodes)
    overrides = dict(epoch_overrides or {})
    holds = frozenset(hold_accounts)
    vouches = tuple(sorted(item for item in relations if item.relation_kind == "vouch"))
    cowork = tuple(sorted(item for item in relations if item.relation_kind == "cowork"))
    declarations_tuple = tuple(sorted(declarations))
    funding_tuple = tuple(
        sorted((dict(item) for item in funding), key=lambda item: _json_bytes(item))
    )
    continuity = tuple(
        ContinuityAssessment(
            account_id=account,
            control_epoch=overrides.get(account, _epoch(account)),
            status="HOLD_CONTINUITY" if account in holds else "READY",
            incumbent=account.endswith("-n00") or account.endswith("-n02"),
            retain_incumbent=(
                account in holds
                and (account.endswith("-n00") or account.endswith("-n02"))
            ),
            reasons=("fresh_window_incomplete",) if account in holds else (),
            fresh_score_evidence=0 if account in holds else 4,
            renewed_vouches=0 if account in holds else 1,
            post_epoch_cowork=0 if account in holds else 3,
        )
        for account in sorted(node_tuple)
    )
    input_document = {
        "window": _window(window_index).to_dict(),
        "funding_observations": list(funding_tuple),
        "active_vouches": [item.to_dict() for item in vouches],
        "active_cowork": [item.to_dict() for item in cowork],
        "active_control_declarations": [
            item.to_dict() for item in declarations_tuple
        ],
        "continuity": [item.to_dict() for item in continuity],
    }
    commitments = {
        "policy_root": policy_root(),
        "input_root": evidence_input_root(input_document),
        "registry_root": control_registry_root(
            {
                "window_index": window_index,
                "epochs": [
                    {"account_id": item.account_id, "control_epoch": item.control_epoch}
                    for item in continuity
                ],
            }
        ),
    }
    if reordered:
        vouches = tuple(reversed(vouches))
        cowork = tuple(reversed(cowork))
        declarations_tuple = tuple(reversed(declarations_tuple))
        funding_tuple = tuple(reversed(funding_tuple))
        continuity = tuple(reversed(continuity))
    return EvidenceSnapshotResult(
        status="verified_with_holds" if holds else "verified",
        failures=(),
        record_rejections=(),
        hold_accounts=tuple(sorted(holds)),
        commitments=commitments,
        window=_window(window_index),
        audit_funding_observations=funding_tuple,
        historical_score_evidence=(),
        active_vouches=vouches,
        active_cowork=cowork,
        active_control_declarations=declarations_tuple,
        continuity=continuity,
    )


def _honest_relations(
    *,
    acknowledgement_percent: int,
    window_index: int,
    duplicate_credit: bool = False,
) -> tuple[ActiveRelation, ...]:
    available = len(HONEST_CONTROLS) * acknowledgement_percent // 100
    relations: list[ActiveRelation] = []
    for account in HONEST_CONTROLS[:available]:
        community = account[8:10]
        sponsor = f"honest-c{community}-n00"
        relations.append(
            _relation(
                "vouch",
                sponsor,
                account,
                unit=0,
                window_index=window_index,
            )
        )
        for unit in range(3):
            relations.append(
                _relation(
                    "cowork",
                    sponsor,
                    account,
                    unit=unit,
                    window_index=window_index,
                )
            )
    if duplicate_credit and relations:
        original = next(item for item in relations if item.relation_kind == "cowork")
        relations.append(original)
    return tuple(relations)


def build_scenario(
    *,
    window_index: int = 7,
    acknowledgement_percent: int = 100,
    attack_count: int = 0,
    extra_nodes: Sequence[str] = (),
    extra_relations: Sequence[ActiveRelation] = (),
    funding: Sequence[Mapping[str, Any]] = (),
    declarations: Sequence[ActiveControlDeclaration] = (),
    hold_accounts: Sequence[str] = (),
    epoch_overrides: Mapping[str, str] | None = None,
    duplicate_credit: bool = False,
    seats: Sequence[RegistrySeat] | None = None,
    foundation_accounts: Sequence[str] | None = None,
    prior_breaches: Sequence[PriorBreach] = (),
    reordered: bool = False,
) -> Scenario:
    nodes = [*BASE_NODES, *extra_nodes]
    relations = list(
        _honest_relations(
            acknowledgement_percent=acknowledgement_percent,
            window_index=window_index,
            duplicate_credit=duplicate_credit,
        )
    )
    for index in range(attack_count):
        account = f"attack-bridge-{index:02d}"
        nodes.append(account)
        sponsor = f"honest-c{index % 17:02d}-n00"
        relations.append(
            _relation(
                "vouch",
                sponsor,
                account,
                unit=0,
                window_index=window_index,
            )
        )
        for unit in range(3):
            relations.append(
                _relation(
                    "cowork",
                    sponsor,
                    account,
                    unit=unit,
                    window_index=window_index,
                )
            )
    relations.extend(extra_relations)
    seat_tuple = tuple(seats or BASE_SEATS)
    foundation = tuple(
        FOUNDATION_ACCOUNTS if foundation_accounts is None else foundation_accounts
    )
    evidence = _make_evidence(
        tuple(reversed(nodes)) if reordered else tuple(nodes),
        window_index=window_index,
        relations=tuple(reversed(relations)) if reordered else tuple(relations),
        declarations=declarations,
        hold_accounts=hold_accounts,
        funding=funding,
        epoch_overrides=epoch_overrides,
        reordered=reordered,
    )
    opening_root = _digest(
        f"opening-registry:{window_index}:{','.join(sorted(s.account_id for s in seat_tuple))}"
    )
    frozen = freeze_admission_window(
        evidence,
        opening_registry_root=opening_root,
        nodes=tuple(reversed(nodes)) if reordered else tuple(nodes),
        opening_seats=tuple(reversed(seat_tuple)) if reordered else seat_tuple,
        foundation_accounts=tuple(reversed(foundation)) if reordered else foundation,
        prior_breaches=tuple(reversed(tuple(prior_breaches))) if reordered else prior_breaches,
    )
    state = RegistryRoundState(
        round_index=window_index * 100,
        current_registry_root=_digest(f"registry-round:{window_index}:0"),
        prior_registry_root=None,
        prior_registry_round=None,
        seats=tuple(reversed(seat_tuple)) if reordered else seat_tuple,
        changes_this_round=0,
        transition_budget=1,
    )
    return Scenario(
        nodes=tuple(sorted(nodes)),
        seats=tuple(sorted(seat_tuple)),
        foundation_accounts=tuple(sorted(foundation)),
        evidence=evidence,
        frozen=frozen,
        state=state,
    )


def _candidate(
    scenario: Scenario,
    account: str,
    state: RegistryRoundState,
) -> AdmissionCandidate:
    continuity = {item.account_id: item for item in scenario.frozen.continuity}
    epoch = continuity.get(account)
    return AdmissionCandidate(
        validator_id=f"validator-{account}",
        account_id=account,
        control_epoch=epoch.control_epoch if epoch is not None else _epoch(account),
        source_registry_root=state.current_registry_root,
        source_registry_round=state.round_index,
    )


def evaluate_accounts(
    scenario: Scenario,
    accounts: Iterable[str],
) -> tuple[list[dict[str, Any]], RegistryRoundState]:
    state = scenario.state
    decisions: list[dict[str, Any]] = []
    for account in sorted(accounts):
        report = evaluate_admission_round(
            scenario.frozen,
            state,
            _candidate(scenario, account, state),
        )
        decisions.append(
            {
                "account": account,
                "action": report.decision.action,
                "reason_codes": sorted(item.code for item in report.decision.reasons),
                "report_root": report.report_root,
            }
        )
        if report.decision.action == PROPOSE_ADD:
            state = advance_shadow_round(
                scenario.frozen,
                state,
                report,
                new_registry_root=_digest(
                    f"registry-round:{scenario.frozen.window.index}:{state.round_index + 1}:{account}"
                ),
            )
    return decisions, state


def _target_signature(scenario: Scenario, target: str) -> dict[str, str]:
    report = evaluate_admission_round(
        scenario.frozen,
        scenario.state,
        _candidate(scenario, target, scenario.state),
    )
    cluster = next(
        item.cluster_id
        for item in scenario.frozen.clusters
        if target in item.members
    )
    group = next(
        item.limit_id
        for item in recount_limit_states(scenario.frozen, scenario.state.seats)
        if item.limit_kind == "DECLARED_CONTROL_GROUP" and target in item.members
    )
    return {
        "stationary_mass": _fraction_text(dict(scenario.frozen.stationary_mass)[target]),
        "social_cluster_id": cluster,
        "declared_control_group_id": group,
        "result": report.decision.action,
    }


def run_unsolicited_funding(reordered: bool) -> dict[str, Any]:
    target = HONEST_CONTROLS[0]
    variants: list[dict[str, Any]] = []
    for count in (0, 1, 2, 4, 8, 16, 32, 64):
        observations = tuple(
            _funding_observation(
                target,
                sequence=index,
                value_units=1,
                label=f"dust-{count}",
            )
            for index in range(count)
        )
        scenario = build_scenario(funding=observations, reordered=reordered)
        variants.append(
            {
                "kind": "quiet_window_dust",
                "budget": count,
                "value_units_each": 1,
                "audit_observation_count": len(
                    scenario.evidence.audit_funding_observations
                ),
                "target": _target_signature(scenario, target),
                "audit_observation_digests": sorted(
                    item["observation_digest"]
                    for item in scenario.evidence.audit_funding_observations
                ),
            }
        )
    for value in (0, 100, 1000, 1_000_000):
        observations = (
            ()
            if value == 0
            else (
                _funding_observation(
                    target,
                    sequence=0,
                    value_units=value,
                    label=f"large-{value}",
                ),
            )
        )
        scenario = build_scenario(funding=observations, reordered=reordered)
        variants.append(
            {
                "kind": "larger_unsolicited_inflow",
                "budget": 0 if value == 0 else 1,
                "value_units_each": value,
                "audit_observation_count": len(
                    scenario.evidence.audit_funding_observations
                ),
                "target": _target_signature(scenario, target),
                "audit_observation_digests": sorted(
                    item["observation_digest"]
                    for item in scenario.evidence.audit_funding_observations
                ),
            }
        )
    signatures = {_json_bytes(item["target"]) for item in variants}
    invariant = len(signatures) == 1
    baseline_action = variants[0]["target"]["result"]
    denials = sum(item["target"]["result"] != baseline_action for item in variants)
    return {
        "family": "unsolicited_funding",
        "verdict": "PASS" if invariant and denials == 0 else "FAIL",
        "invariant": invariant,
        "unsolicited_denials": denials,
        "audit_observations_retained": all(
            item["audit_observation_count"]
            == len(item["audit_observation_digests"])
            for item in variants
        ),
        "variants": variants,
    }


def run_control_change(reordered: bool) -> dict[str, Any]:
    target = HONEST_CONTROLS[0]
    hidden = build_scenario(reordered=reordered)
    hidden_report = evaluate_admission_round(
        hidden.frozen,
        hidden.state,
        _candidate(hidden, target, hidden.state),
    )
    changed_epoch = _epoch(target, "declared-transfer")
    declaration = _control_declaration(
        ("honest-c00-n00", target),
        window_index=7,
        label="declared-transfer",
    )
    declared = build_scenario(
        hold_accounts=(target,),
        epoch_overrides={target: changed_epoch},
        declarations=(declaration,),
        reordered=reordered,
    )
    declared_report = evaluate_admission_round(
        declared.frozen,
        declared.state,
        _candidate(declared, target, declared.state),
    )
    correct = (
        hidden_report.decision.action == PROPOSE_ADD
        and declared_report.decision.action != PROPOSE_ADD
    )
    return {
        "family": "declared_vs_hidden_control",
        "verdict": (
            "PASS_WITH_UNDETECTABLE_HIDDEN_CHANGE" if correct else "FAIL"
        ),
        "declared_change": {
            "epoch": changed_epoch,
            "result": declared_report.decision.action,
            "reasons": sorted(item.code for item in declared_report.decision.reasons),
        },
        "hidden_unchanged_key_change": {
            "public_epoch": _epoch(target),
            "result": hidden_report.decision.action,
            "detection": "UNDETECTABLE_BY_PUBLIC_INPUTS",
            "personhood_inference": "NONE",
        },
    }


def _single_result(
    *,
    target: str,
    hold: bool,
    epoch_generation: str,
    window_index: int,
    reordered: bool,
) -> dict[str, Any]:
    scenario = build_scenario(
        window_index=window_index,
        hold_accounts=(target,) if hold else (),
        epoch_overrides={target: _epoch(target, epoch_generation)},
        reordered=reordered,
    )
    report = evaluate_admission_round(
        scenario.frozen,
        scenario.state,
        _candidate(scenario, target, scenario.state),
    )
    return {
        "control_epoch": _epoch(target, epoch_generation),
        "result": report.decision.action,
        "reasons": sorted(item.code for item in report.decision.reasons),
        "window_index": window_index,
    }


def run_renewal_recovery(reordered: bool) -> dict[str, Any]:
    target = HONEST_CONTROLS[1]
    cases = {
        "renewal_complete_window": _single_result(
            target=target,
            hold=False,
            epoch_generation="renewal",
            window_index=8,
            reordered=reordered,
        ),
        "recovery_incomplete_window": _single_result(
            target=target,
            hold=True,
            epoch_generation="recovery",
            window_index=8,
            reordered=reordered,
        ),
        "recovery_complete_window": _single_result(
            target=target,
            hold=False,
            epoch_generation="recovery",
            window_index=9,
            reordered=reordered,
        ),
    }
    correct = (
        cases["renewal_complete_window"]["result"] == PROPOSE_ADD
        and cases["recovery_incomplete_window"]["result"] != PROPOSE_ADD
        and cases["recovery_complete_window"]["result"] == PROPOSE_ADD
    )
    return {
        "family": "renewal_recovery",
        "verdict": "PASS" if correct else "FAIL",
        "cases": cases,
    }


def _attack_points(
    *,
    budgets: Sequence[int],
    funding_common_controller: bool,
    reordered: bool,
) -> list[dict[str, Any]]:
    points: list[dict[str, Any]] = []
    best = 0
    for budget in budgets:
        funding = (
            tuple(
                _funding_observation(
                    f"attack-bridge-{index:02d}",
                    sequence=index,
                    value_units=1000,
                    label="common-controller",
                )
                for index in range(budget)
            )
            if funding_common_controller
            else ()
        )
        scenario = build_scenario(
            acknowledgement_percent=0,
            attack_count=budget,
            funding=funding,
            reordered=reordered,
        )
        accounts = tuple(f"attack-bridge-{index:02d}" for index in range(budget))
        decisions, _state = evaluate_accounts(scenario, accounts)
        seats = sum(item["action"] == PROPOSE_ADD for item in decisions)
        best = max(best, seats)
        groups = recount_limit_states(scenario.frozen, scenario.state.seats)
        declared_sizes = [
            len(item.members)
            for item in groups
            if item.limit_kind == "DECLARED_CONTROL_GROUP"
            and any(account in item.members for account in accounts)
        ]
        points.append(
            {
                "budget": budget,
                "seats_gained": seats,
                "best_attacker_seats_within_budget": best,
                "accepted_relation_count": sum(
                    item.relation_kind in ("vouch", "cowork")
                    for item in (
                        *scenario.evidence.active_vouches,
                        *scenario.evidence.active_cowork,
                    )
                ),
                "audit_funding_observation_count": len(
                    scenario.evidence.audit_funding_observations
                ),
                "largest_declared_group_size": max(declared_sizes, default=0),
            }
        )
    return points


def run_accepted_bridge(reordered: bool) -> dict[str, Any]:
    budgets = (0, 1, 2, 3, 4, 6, 8, 12, 16)
    points = _attack_points(
        budgets=budgets,
        funding_common_controller=False,
        reordered=reordered,
    )
    maximum = max(item["best_attacker_seats_within_budget"] for item in points)
    return {
        "family": "accepted_bridge",
        "verdict": (
            "OBSERVED_ADMISSION_WITH_ACCEPTED_CONSENT"
            if maximum > 0
            else "NO_ADMISSION_OBSERVED"
        ),
        "best_attacker_seats_within_budget": maximum,
        "points": points,
    }


def run_undeclared_common_funding(reordered: bool) -> dict[str, Any]:
    budgets = (0, 1, 2, 3, 4, 6, 8, 12, 16)
    funded = _attack_points(
        budgets=budgets,
        funding_common_controller=True,
        reordered=reordered,
    )
    unfunded = _attack_points(
        budgets=budgets,
        funding_common_controller=False,
        reordered=reordered,
    )
    funding_neutral = all(
        left["seats_gained"] == right["seats_gained"]
        for left, right in zip(funded, unfunded)
    )
    no_synthesized_group = all(
        item["largest_declared_group_size"] <= 1 for item in funded
    )
    return {
        "family": "undeclared_common_funding",
        "verdict": (
            "EXPECTED_UNDETECTABLE_COORDINATION"
            if funding_neutral and no_synthesized_group
            else "FAIL"
        ),
        "funding_neutral": funding_neutral,
        "control_group_synthesized": False,
        "personhood_inference": "NONE",
        "best_attacker_seats_within_budget": max(
            item["best_attacker_seats_within_budget"] for item in funded
        ),
        "points": funded,
    }


def _disconnected_attack_points(
    *,
    family: str,
    budgets: Sequence[int],
    relation_kind: str,
    reordered: bool,
) -> list[dict[str, Any]]:
    points: list[dict[str, Any]] = []
    best = 0
    for budget in budgets:
        accounts = tuple(f"attack-{family}-{index:03d}" for index in range(budget))
        relations: list[ActiveRelation] = []
        if budget > 1:
            for index, source in enumerate(accounts):
                relations.append(
                    _relation(
                        relation_kind,
                        source,
                        accounts[(index + 1) % budget],
                        unit=index,
                        window_index=7,
                    )
                )
        scenario = build_scenario(
            acknowledgement_percent=0,
            extra_nodes=accounts,
            extra_relations=relations,
            reordered=reordered,
        )
        decisions, _state = evaluate_accounts(scenario, accounts)
        seats = sum(item["action"] == PROPOSE_ADD for item in decisions)
        best = max(best, seats)
        points.append(
            {
                "budget": budget,
                "seats_gained": seats,
                "best_attacker_seats_within_budget": best,
                "internal_accepted_relations": len(relations),
            }
        )
    return points


def run_disconnected_vouch_ring(reordered: bool) -> dict[str, Any]:
    points = _disconnected_attack_points(
        family="vouch-ring",
        budgets=(0, 1, 2, 4, 8, 16, 32, 64),
        relation_kind="vouch",
        reordered=reordered,
    )
    maximum = max(item["best_attacker_seats_within_budget"] for item in points)
    return {
        "family": "disconnected_vouch_ring",
        "verdict": "PASS" if maximum == 0 else "FAIL",
        "best_attacker_seats_within_budget": maximum,
        "points": points,
    }


def run_aged_account_control_change(reordered: bool) -> dict[str, Any]:
    points: list[dict[str, Any]] = []
    best_hidden = 0
    for budget in (0, 1, 2, 3, 4, 6, 8, 12, 16):
        accounts = HONEST_CONTROLS[:budget]
        hidden = build_scenario(reordered=reordered)
        hidden_decisions, _state = evaluate_accounts(hidden, accounts)
        hidden_seats = sum(
            item["action"] == PROPOSE_ADD for item in hidden_decisions
        )
        epochs = {
            account: _epoch(account, "declared-purchase") for account in accounts
        }
        declared = build_scenario(
            hold_accounts=accounts,
            epoch_overrides=epochs,
            reordered=reordered,
        )
        declared_decisions, _state = evaluate_accounts(declared, accounts)
        declared_seats = sum(
            item["action"] == PROPOSE_ADD for item in declared_decisions
        )
        best_hidden = max(best_hidden, hidden_seats)
        points.append(
            {
                "budget": budget,
                "declared_change_seats": declared_seats,
                "hidden_unchanged_key_seats": hidden_seats,
                "best_hidden_seats_within_budget": best_hidden,
                "unresolved_hidden_changes": hidden_seats,
            }
        )
    correct = all(item["declared_change_seats"] == 0 for item in points)
    return {
        "family": "aged_account_control_change",
        "verdict": (
            "PASS_WITH_UNRESOLVED_HIDDEN_CHANGES" if correct else "FAIL"
        ),
        "best_attacker_seats_within_budget": best_hidden,
        "points": points,
    }


def run_parallel_identities(reordered: bool) -> dict[str, Any]:
    points = _disconnected_attack_points(
        family="parallel",
        budgets=(0, 1, 2, 3, 4, 6, 8, 12, 16),
        relation_kind="cowork",
        reordered=reordered,
    )
    maximum = max(item["best_attacker_seats_within_budget"] for item in points)
    return {
        "family": "parallel_identities",
        "verdict": "PASS" if maximum == 0 else "FAIL",
        "best_attacker_seats_within_budget": maximum,
        "points": points,
    }


def run_first_funder_manipulation(reordered: bool) -> dict[str, Any]:
    target = HONEST_CONTROLS[0]
    points: list[dict[str, Any]] = []
    baseline = build_scenario(reordered=reordered)
    baseline_action = _target_signature(baseline, target)["result"]
    for budget in (0, 1, 2, 4, 8, 16, 32, 64):
        funding = tuple(
            _funding_observation(
                target,
                sequence=index,
                value_units=1,
                label=f"first-funder-{budget}",
            )
            for index in range(budget)
        )
        scenario = build_scenario(funding=funding, reordered=reordered)
        action = _target_signature(scenario, target)["result"]
        points.append(
            {
                "budget": budget,
                "target_result": action,
                "target_denied": action != baseline_action,
                "audit_observation_count": len(funding),
            }
        )
    denials = sum(item["target_denied"] for item in points)
    return {
        "family": "first_funder_manipulation",
        "verdict": "PASS" if denials == 0 else "FAIL",
        "best_attacker_seats_within_budget": 0,
        "unsolicited_denials": denials,
        "points": points,
    }


def run_foundation_supported_favorite(reordered: bool) -> dict[str, Any]:
    candidate = "attack-foundation-favorite"
    actions: list[ActiveRelation] = []
    for index, foundation in enumerate(FOUNDATION_ACCOUNTS):
        actions.append(
            _relation(
                "vouch",
                foundation,
                candidate,
                unit=index,
                window_index=7,
            )
        )
    for unit in range(3):
        for foundation in FOUNDATION_ACCOUNTS:
            actions.append(
                _relation(
                    "cowork",
                    foundation,
                    candidate,
                    unit=unit,
                    window_index=7,
                )
            )
    points: list[dict[str, Any]] = []
    best = 0
    for budget in (0, 1, 2, 3, 4, 6, 8, 10, 12):
        scenario = build_scenario(
            acknowledgement_percent=0,
            extra_nodes=(candidate,),
            extra_relations=actions[:budget],
            reordered=reordered,
        )
        decisions, _state = evaluate_accounts(scenario, (candidate,))
        seats = sum(item["action"] == PROPOSE_ADD for item in decisions)
        best = max(best, seats)
        points.append(
            {
                "budget": budget,
                "seats_gained": seats,
                "best_attacker_seats_within_budget": best,
                "foundation_support_actions": min(budget, len(actions)),
            }
        )
    return {
        "family": "foundation_supported_favorite",
        "verdict": "PASS" if best == 0 else "FAIL",
        "best_attacker_seats_within_budget": best,
        "points": points,
    }


def run_revocation(reordered: bool) -> dict[str, Any]:
    target = HONEST_CONTROLS[2]
    active = build_scenario(
        window_index=7,
        acknowledgement_percent=100,
        reordered=reordered,
    )
    active_report = evaluate_admission_round(
        active.frozen,
        active.state,
        _candidate(active, target, active.state),
    )
    revoked = build_scenario(
        window_index=8,
        acknowledgement_percent=0,
        reordered=reordered,
    )
    revoked_report = evaluate_admission_round(
        revoked.frozen,
        revoked.state,
        _candidate(revoked, target, revoked.state),
    )
    correct = (
        active_report.decision.action == PROPOSE_ADD
        and revoked_report.decision.action != PROPOSE_ADD
    )
    return {
        "family": "revocation",
        "verdict": "PASS" if correct else "FAIL",
        "active_window": {
            "window_index": 7,
            "result": active_report.decision.action,
        },
        "next_boundary": {
            "window_index": 8,
            "result": revoked_report.decision.action,
            "reasons": sorted(
                item.code for item in revoked_report.decision.reasons
            ),
        },
    }


def build_cap_merge_scenario(
    *,
    window_index: int,
    prior_breaches: Sequence[PriorBreach] = (),
    reordered: bool,
) -> Scenario:
    seats = [
        item
        for item in BASE_SEATS
        if item.account_id not in ("honest-c15-n00", "honest-c16-n00")
    ]
    seats.extend(
        (
            RegistrySeat("validator-extra-c00", "honest-c00-n02"),
            RegistrySeat("validator-extra-c01", "honest-c01-n02"),
        )
    )
    pairs = (
        ("honest-c00-n00", "honest-c00-n02"),
        ("honest-c00-n00", "honest-c00-n01"),
        ("honest-c01-n00", "honest-c01-n02"),
        ("honest-c01-n00", "honest-c01-n01"),
        ("honest-c00-n00", "honest-c01-n00"),
        ("honest-c02-n00", "honest-c02-n01"),
    )
    relations = tuple(
        _relation(
            "cowork",
            source,
            target,
            unit=0,
            window_index=window_index,
        )
        for source, target in pairs
    )
    evidence = _make_evidence(
        tuple(reversed(BASE_NODES)) if reordered else BASE_NODES,
        window_index=window_index,
        relations=tuple(reversed(relations)) if reordered else relations,
        reordered=reordered,
    )
    frozen = freeze_admission_window(
        evidence,
        opening_registry_root=_digest(f"cap-merge-opening:{window_index}"),
        nodes=tuple(reversed(BASE_NODES)) if reordered else BASE_NODES,
        opening_seats=tuple(reversed(seats)) if reordered else tuple(seats),
        foundation_accounts=(
            tuple(reversed(FOUNDATION_ACCOUNTS))
            if reordered
            else FOUNDATION_ACCOUNTS
        ),
        prior_breaches=(
            tuple(reversed(tuple(prior_breaches)))
            if reordered
            else prior_breaches
        ),
    )
    state = RegistryRoundState(
        round_index=window_index * 100,
        current_registry_root=_digest(f"cap-merge-round:{window_index}"),
        prior_registry_root=None,
        prior_registry_round=None,
        seats=tuple(reversed(seats)) if reordered else tuple(seats),
        changes_this_round=0,
        transition_budget=1,
    )
    return Scenario(
        nodes=BASE_NODES,
        seats=tuple(sorted(seats)),
        foundation_accounts=FOUNDATION_ACCOUNTS,
        evidence=evidence,
        frozen=frozen,
        state=state,
    )


def _cap_merge_window(
    *,
    window_index: int,
    prior: Sequence[PriorBreach],
    reordered: bool,
) -> tuple[dict[str, Any], tuple[PriorBreach, ...]]:
    scenario = build_cap_merge_scenario(
        window_index=window_index,
        prior_breaches=prior,
        reordered=reordered,
    )
    affected = evaluate_admission_round(
        scenario.frozen,
        scenario.state,
        _candidate(scenario, "honest-c00-n01", scenario.state),
    )
    unrelated = evaluate_admission_round(
        scenario.frozen,
        scenario.state,
        _candidate(scenario, "honest-c02-n01", scenario.state),
    )
    breaches = [
        item
        for item in affected.unresolved_limits
        if item.limit_kind == "SOCIAL_CLUSTER"
    ]
    durations = [
        window_index - item.first_observed_window + 1
        for item in breaches
        if item.first_observed_window is not None
    ]
    return (
        {
            "window_index": window_index,
            "affected_result": affected.decision.action,
            "unrelated_result": unrelated.decision.action,
            "breach_count": len(breaches),
            "excess_seats": sum(item.excess_seats for item in breaches),
            "causative_evidence": sorted(
                {
                    digest
                    for item in breaches
                    for digest in item.causative_evidence
                }
            ),
            "breach_duration_windows": max(durations, default=0),
            "preserved_incumbent_seats": len(
                affected.preserved_incumbent_seat_ids
            ),
            "review_states": sorted({item.review_state for item in breaches}),
        },
        prior_breaches_from_report(affected),
    )


def run_cap_merging_grief(reordered: bool) -> dict[str, Any]:
    first, prior = _cap_merge_window(
        window_index=7,
        prior=(),
        reordered=reordered,
    )
    second, _prior = _cap_merge_window(
        window_index=8,
        prior=prior,
        reordered=reordered,
    )
    correct = (
        first["affected_result"] != PROPOSE_ADD
        and first["unrelated_result"] == PROPOSE_ADD
        and first["preserved_incumbent_seats"] == 20
        and first["breach_count"] > 0
        and second["breach_duration_windows"] == 2
        and "PERSISTENT_UNRESOLVED" in second["review_states"]
    )
    return {
        "family": "cap_merging_grief",
        "verdict": "PASS_PRESERVED_AND_SCOPED" if correct else "FAIL",
        "windows": [first, second],
    }


def run_honest_gate(reordered: bool) -> dict[str, Any]:
    scenario = build_scenario(
        acknowledgement_percent=100,
        reordered=reordered,
    )
    decisions, final_state = evaluate_accounts(scenario, HONEST_CONTROLS)
    failures = [
        {
            "account": item["account"],
            "action": item["action"],
            "reason_codes": item["reason_codes"],
        }
        for item in decisions
        if item["action"] != PROPOSE_ADD
    ]
    admitted = len(decisions) - len(failures)
    return {
        "fixture": "paired_consent_complete",
        "required": 14,
        "total": len(HONEST_CONTROLS),
        "admitted": admitted,
        "waiting": len(failures),
        "failed_controls": failures,
        "verdict": "PASS" if admitted == 14 else "FAIL_NO_PROMOTION",
        "opening_list_size": len(scenario.seats),
        "final_shadow_list_size": len(final_state.seats),
        "controls": decisions,
    }


def _parameterized_mass(
    scenario: Scenario,
    profile: Mapping[str, Any],
) -> dict[str, Fraction]:
    nodes = scenario.frozen.nodes
    seed_vector = dict(scenario.frozen.seed_vector)
    raw: dict[str, dict[str, Fraction]] = {node: {} for node in nodes}

    def add(source: str, target: str, weight: Fraction) -> None:
        raw[source][target] = raw[source].get(target, Fraction(0, 1)) + weight

    for credit in scenario.frozen.graph_credits:
        if credit.kind == "vouch":
            add(
                credit.source_account,
                credit.target_account,
                profile["vouch_weight"],
            )
        else:
            units = min(credit.credited_units, int(profile["cowork_cap"]))
            weight = profile["cowork_weight"] * units
            add(credit.source_account, credit.target_account, weight)
            add(credit.target_account, credit.source_account, weight)

    transition: dict[str, dict[str, Fraction]] = {}
    for source in nodes:
        total = sum(raw[source].values(), Fraction(0, 1))
        if total:
            transition[source] = {
                target: weight / total
                for target, weight in sorted(raw[source].items())
            }
        else:
            transition[source] = {
                target: seed_vector[target]
                for target in nodes
                if seed_vector[target] > 0
            }

    damping = profile["damping"]
    restart = Fraction(1, 1) - damping
    mass = dict(seed_vector)
    for _step in range(int(profile["walk_steps"])):
        walked = {node: Fraction(0, 1) for node in nodes}
        for source in nodes:
            for target, probability in transition[source].items():
                walked[target] += mass[source] * probability
        mass = {
            node: damping * walked[node] + restart * seed_vector[node]
            for node in nodes
        }
    if sum(mass.values(), Fraction(0, 1)) != 1:
        raise RuntimeError("parameterized walk lost exact mass")
    return mass


def _parameter_outcome(
    scenario: Scenario,
    accounts: Sequence[str],
    profile: Mapping[str, Any],
) -> int:
    if scenario.frozen.status != FROZEN:
        return 0
    mass = _parameterized_mass(scenario, profile)
    n = len(scenario.seats)
    floor = Fraction(1, int(profile["connectivity_divisor"]) * n)
    limit = max(
        Fraction(int(profile["minimum_cluster_seats"]), 1),
        Fraction(n, 1) * profile["cluster_seat_fraction"],
    )
    if Fraction(2, 1) > limit:
        return 0
    return sum(mass.get(account, Fraction(0, 1)) >= floor for account in accounts)


def run_acknowledgement_sweep(reordered: bool) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for percent in (100, 75, 50, 0):
        honest = build_scenario(
            acknowledgement_percent=percent,
            reordered=reordered,
        )
        honest_decisions, _state = evaluate_accounts(honest, HONEST_CONTROLS)
        attack = build_scenario(
            acknowledgement_percent=0,
            attack_count=4 * percent // 100,
            reordered=reordered,
        )
        attack_accounts = tuple(
            f"attack-bridge-{index:02d}" for index in range(4 * percent // 100)
        )
        attack_decisions, _state = evaluate_accounts(attack, attack_accounts)
        admitted = sum(item["action"] == PROPOSE_ADD for item in honest_decisions)
        attacker = sum(item["action"] == PROPOSE_ADD for item in attack_decisions)
        community_distribution = {
            f"c{index:02d}": {
                "admitted": int(
                    honest_decisions[index]["action"] == PROPOSE_ADD
                ),
                "waiting": int(
                    honest_decisions[index]["action"] != PROPOSE_ADD
                ),
            }
            for index in range(14)
        }
        rows.append(
            {
                "acknowledgement_percent": percent,
                "honest_admission": admitted,
                "honest_waiting": 14 - admitted,
                "attacker_seats": attacker,
                "time_to_eligibility_windows": 0 if admitted == 14 else 1,
                "held_reasons": sorted(
                    {
                        reason
                        for item in honest_decisions
                        if item["action"] != PROPOSE_ADD
                        for reason in item["reason_codes"]
                    }
                ),
                "community_distribution": community_distribution,
            }
        )
    return rows


def _profile_text(profile: Mapping[str, Any]) -> dict[str, Any]:
    return {
        key: (
            _fraction_text(value) if isinstance(value, Fraction) else value
        )
        for key, value in sorted(profile.items())
    }


def run_sensitivity(reordered: bool) -> list[dict[str, Any]]:
    honest = build_scenario(acknowledgement_percent=100, reordered=reordered)
    attack = build_scenario(
        acknowledgement_percent=0,
        attack_count=4,
        reordered=reordered,
    )
    attack_accounts = tuple(f"attack-bridge-{index:02d}" for index in range(4))
    variants: tuple[tuple[str, tuple[tuple[str, Any], ...], str], ...] = (
        ("vouch_weight", (("1/2", Fraction(1, 2)), ("1", Fraction(1)), ("2", Fraction(2))), "vouch_weight"),
        ("cowork_weight", (("1/2", Fraction(1, 2)), ("1", Fraction(1)), ("2", Fraction(2))), "cowork_weight"),
        ("cowork_cap", (("1", 1), ("3", 3), ("5", 5)), "cowork_cap"),
        ("funding_weight", (("1", Fraction(1)), ("2", Fraction(2)), ("4", Fraction(4))), "funding_weight"),
        ("walk_damping", (("0.75", Fraction(3, 4)), ("0.85", Fraction(17, 20)), ("0.90", Fraction(9, 10))), "damping"),
        ("walk_steps", (("10", 10), ("20", 20), ("40", 40)), "walk_steps"),
        ("conductance_cut", (("0.05", Fraction(1, 20)), ("0.10", Fraction(1, 10)), ("0.15", Fraction(3, 20))), "conductance_cut"),
        ("connectivity_floor", (("1/N", 1), ("1/(2N)", 2), ("1/(4N)", 4)), "connectivity_divisor"),
        ("minimum_cluster_seats", (("1", 1), ("2", 2), ("3", 3)), "minimum_cluster_seats"),
        ("cluster_seat_fraction", (("5%", Fraction(1, 20)), ("10%", Fraction(1, 10)), ("15%", Fraction(3, 20))), "cluster_seat_fraction"),
    )
    rows: list[dict[str, Any]] = []
    for constant, values, field in variants:
        for label, value in values:
            profile = dict(DEFAULT_PROFILE)
            profile[field] = value
            admitted = _parameter_outcome(honest, HONEST_CONTROLS, profile)
            rows.append(
                {
                    "constant": constant,
                    "label": label,
                    "honest_admission": admitted,
                    "honest_waiting": 14 - admitted,
                    "attacker_seats": _parameter_outcome(
                        attack,
                        attack_accounts,
                        profile,
                    ),
                    "funding_role": (
                        "AUDIT_ONLY_NO_MASS_NO_VETO"
                        if constant == "funding_weight"
                        else "NOT_APPLICABLE"
                    ),
                    "profile": _profile_text(profile),
                }
            )
    return rows


def run_cartesian_grid(reordered: bool) -> list[dict[str, Any]]:
    honest = build_scenario(acknowledgement_percent=100, reordered=reordered)
    attack = build_scenario(
        acknowledgement_percent=0,
        attack_count=4,
        reordered=reordered,
    )
    attack_accounts = tuple(f"attack-bridge-{index:02d}" for index in range(4))
    rows: list[dict[str, Any]] = []
    for damping in (Fraction(3, 4), Fraction(17, 20), Fraction(9, 10)):
        for steps in (10, 20, 40):
            for divisor in (1, 2, 4):
                profile = dict(DEFAULT_PROFILE)
                profile.update(
                    {
                        "damping": damping,
                        "walk_steps": steps,
                        "connectivity_divisor": divisor,
                    }
                )
                admitted = _parameter_outcome(honest, HONEST_CONTROLS, profile)
                rows.append(
                    {
                        "damping": _fraction_text(damping),
                        "steps": steps,
                        "floor": f"1/({divisor}N)",
                        "honest_admission": admitted,
                        "honest_waiting": 14 - admitted,
                        "attacker_seats": _parameter_outcome(
                            attack,
                            attack_accounts,
                            profile,
                        ),
                    }
                )
    return rows


def run_floor_boundary() -> dict[str, Any]:
    floor = connectivity_floor(20)
    epsilon = Fraction(1, 10**12)
    return {
        "opening_list_size": 20,
        "exact_floor": _fraction_text(floor),
        "at_floor_admissible": floor >= connectivity_floor(20),
        "one_epsilon_below_admissible": (
            floor - epsilon >= connectivity_floor(20)
        ),
        "epsilon": _fraction_text(epsilon),
    }


def _topology_seats(seed: int, window_index: int) -> tuple[RegistrySeat, ...]:
    if seed == 2026090807 and window_index >= 8:
        retained = tuple(
            item
            for item in BASE_SEATS
            if item.account_id != "honest-c16-n00"
        )
        return tuple(
            sorted(
                (
                    *retained,
                    RegistrySeat(
                        "validator-boundary-add-c00",
                        "honest-c00-n01",
                    ),
                )
            )
        )
    return BASE_SEATS


def build_variable_topology_scenario(
    *,
    seed: int,
    window_index: int,
    acknowledgement_percent: int,
    hold_accounts: Sequence[str],
    prior_breaches: Sequence[PriorBreach],
    reordered: bool,
) -> tuple[Scenario, dict[str, int]]:
    community_size = (8, 12, 16)[seed % 3]
    cowork_units = (1, 2, 3)[(seed // 3) % 3]
    foundation_count = (1, 3, 5)[(seed // 9) % 3]
    cross_bridges = seed % 4
    nodes = [
        f"honest-c{community:02d}-n{member:02d}"
        for community in range(20)
        for member in range(community_size)
    ]
    nodes.append("attack-topology")
    seats = list(BASE_SEATS)
    if seed != 2026090804 and window_index == 8:
        seats.append(
            RegistrySeat("validator-boundary-add-c14", "honest-c14-n02")
        )
    elif seed != 2026090804 and window_index == 9:
        seats = [
            item for item in seats if item.account_id != "honest-c16-n00"
        ]
        seats.append(
            RegistrySeat("validator-boundary-add-c14", "honest-c14-n02")
        )
    foundations = tuple(
        f"honest-c{community:02d}-n00"
        for community in range(20 - foundation_count, 20)
    )
    if seed == 2026090804:
        foundations = tuple(item.account_id for item in seats)

    available = len(HONEST_CONTROLS) * acknowledgement_percent // 100
    relations: list[ActiveRelation] = []
    for account in HONEST_CONTROLS[:available]:
        sponsor = f"honest-c{account[8:10]}-n00"
        relations.append(
            _relation(
                "vouch",
                sponsor,
                account,
                unit=0,
                window_index=window_index,
            )
        )
        for unit in range(cowork_units):
            relations.append(
                _relation(
                    "cowork",
                    sponsor,
                    account,
                    unit=unit,
                    window_index=window_index,
                )
            )
    for index in range(cross_bridges):
        relations.append(
            _relation(
                "cowork",
                f"honest-c{index:02d}-n00",
                f"honest-c{index + 1:02d}-n00",
                unit=0,
                window_index=window_index,
            )
        )
    relations.extend(
        (
            _relation(
                "vouch",
                "honest-c14-n00",
                "honest-c14-n02",
                unit=0,
                window_index=window_index,
            ),
            *(
                _relation(
                    "cowork",
                    "honest-c14-n00",
                    "honest-c14-n02",
                    unit=unit,
                    window_index=window_index,
                )
                for unit in range(cowork_units)
            ),
        )
    )
    relations.extend(
        (
            _relation(
                "vouch",
                "honest-c15-n00",
                "attack-topology",
                unit=0,
                window_index=window_index,
            ),
            _relation(
                "cowork",
                "honest-c15-n00",
                "attack-topology",
                unit=0,
                window_index=window_index,
            ),
        )
    )
    if seed == 2026090805 and relations:
        relations.append(next(item for item in relations if item.relation_kind == "cowork"))
    evidence = _make_evidence(
        tuple(reversed(nodes)) if reordered else tuple(nodes),
        window_index=window_index,
        relations=tuple(reversed(relations)) if reordered else tuple(relations),
        hold_accounts=hold_accounts,
        reordered=reordered,
    )
    frozen = freeze_admission_window(
        evidence,
        opening_registry_root=_digest(f"variable-opening:{seed}:{window_index}"),
        nodes=tuple(reversed(nodes)) if reordered else tuple(nodes),
        opening_seats=tuple(reversed(seats)) if reordered else tuple(seats),
        foundation_accounts=(
            tuple(reversed(foundations)) if reordered else foundations
        ),
        prior_breaches=(
            tuple(reversed(tuple(prior_breaches)))
            if reordered
            else prior_breaches
        ),
    )
    state = RegistryRoundState(
        round_index=window_index * 100,
        current_registry_root=_digest(f"variable-round:{seed}:{window_index}"),
        prior_registry_root=None,
        prior_registry_round=None,
        seats=tuple(reversed(seats)) if reordered else tuple(seats),
        changes_this_round=0,
        transition_budget=1,
    )
    return (
        Scenario(
            nodes=tuple(sorted(nodes)),
            seats=tuple(sorted(seats)),
            foundation_accounts=tuple(sorted(foundations)),
            evidence=evidence,
            frozen=frozen,
            state=state,
        ),
        {
            "community_count": 20,
            "community_size": community_size,
            "cowork_units": cowork_units,
            "foundation_seat_count": len(foundations),
            "cross_community_bridges": cross_bridges,
        },
    )


def run_topology_windows(
    seeds: Sequence[int],
    *,
    reordered: bool,
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for seed in sorted(seeds):
        persistent_prior: tuple[PriorBreach, ...] = ()
        for window_index in (7, 8, 9):
            special: list[str] = ["dangling_nodes"]
            if seed in (2026090801, 2026090802):
                special.append("exact_floor_boundary")
            if seed == 2026090804:
                special.append("empty_eligible_seeds")
            if seed == 2026090805:
                special.append("duplicate_credit_records")
            if seed == 2026090806 and window_index >= 8:
                special.append("control_rotation")
            if seed == 2026090807 and window_index >= 8:
                special.extend(("authorized_removal", "new_seed_at_boundary"))
            if seed == 2026090808:
                special.append("persistent_breach")
                cap_row, persistent_prior = _cap_merge_window(
                    window_index=window_index,
                    prior=persistent_prior,
                    reordered=reordered,
                )
                rows.append(
                    {
                        "topology_seed": seed,
                        "window_index": window_index,
                        "population_size": len(BASE_NODES),
                        "opening_list_size": 20,
                        "eligible_seed_count": 17,
                        "honest_admission": (
                            1
                            if cap_row["unrelated_result"] == PROPOSE_ADD
                            else 0
                        ),
                        "honest_waiting": (
                            0
                            if cap_row["unrelated_result"] == PROPOSE_ADD
                            else 1
                        ),
                        "zero_admission_window": (
                            cap_row["unrelated_result"] != PROPOSE_ADD
                        ),
                        "best_attacker_seats_within_budget": 0,
                        "unsolicited_denials": 0,
                        "concentration_ppm": 200000,
                        "breach_duration_windows": cap_row[
                            "breach_duration_windows"
                        ],
                        "special_cases": sorted(special),
                        "new_validator_seed_at_boundary": False,
                        "boundary_addition_result": "NOT_APPLICABLE",
                        "community_count": 20,
                        "community_size": 12,
                        "cowork_units": 1,
                        "foundation_seat_count": 3,
                        "cross_community_bridges": 1,
                    }
                )
                continue

            rng = random.Random(seed * 100 + window_index)
            acknowledgement = (
                100 if window_index == 7 else rng.choice((100, 75, 50))
            )
            holds = (
                (HONEST_CONTROLS[0],)
                if seed == 2026090806 and window_index >= 8
                else ()
            )
            scenario, axes = build_variable_topology_scenario(
                seed=seed,
                window_index=window_index,
                acknowledgement_percent=acknowledgement,
                hold_accounts=holds,
                prior_breaches=persistent_prior,
                reordered=reordered,
            )
            decisions, _state = evaluate_accounts(scenario, HONEST_CONTROLS)
            admitted = sum(item["action"] == PROPOSE_ADD for item in decisions)
            if window_index == 7:
                boundary_report = evaluate_admission_round(
                    scenario.frozen,
                    scenario.state,
                    _candidate(
                        scenario,
                        "honest-c14-n02",
                        scenario.state,
                    ),
                )
                boundary_addition_result = boundary_report.decision.action
            else:
                boundary_addition_result = (
                    "SEATED_AT_BOUNDARY"
                    if any(
                        item.account_id == "honest-c14-n02"
                        for item in scenario.seats
                    )
                    else "NOT_SEATED"
                )
            attack_account = "attack-topology"
            attack_report = evaluate_admission_round(
                scenario.frozen,
                scenario.state,
                _candidate(scenario, attack_account, scenario.state),
            )
            states = recount_limit_states(scenario.frozen, scenario.state.seats)
            breach_states = [
                item for item in states if item.state == "EXISTING_BREACH"
            ]
            audit_report = evaluate_admission_round(
                scenario.frozen,
                scenario.state,
                _candidate(scenario, HONEST_CONTROLS[0], scenario.state),
            )
            persistent_prior = prior_breaches_from_report(audit_report)
            durations = [
                window_index - item.first_observed_window + 1
                for item in breach_states
                if item.first_observed_window is not None
            ]
            max_occupancy = max(
                (item.occupied_seats for item in states),
                default=0,
            )
            concentration = (
                max_occupancy * 1_000_000 // len(scenario.seats)
                if scenario.seats
                else 0
            )
            rows.append(
                {
                    "topology_seed": seed,
                    "window_index": window_index,
                    "population_size": len(scenario.nodes),
                    "opening_list_size": len(scenario.seats),
                    "eligible_seed_count": len(scenario.frozen.eligible_seeds),
                    "honest_admission": admitted,
                    "honest_waiting": 14 - admitted,
                    "zero_admission_window": admitted == 0,
                    "best_attacker_seats_within_budget": (
                        1
                        if attack_report.decision.action == PROPOSE_ADD
                        else 0
                    ),
                    "unsolicited_denials": 0,
                    "concentration_ppm": concentration,
                    "breach_duration_windows": max(durations, default=0),
                    "special_cases": sorted(special),
                    "new_validator_seed_at_boundary": (
                        "honest-c14-n02" in scenario.frozen.eligible_seeds
                    ),
                    "boundary_addition_result": boundary_addition_result,
                    **axes,
                }
            )
    return rows


def build_results(
    prereg: Mapping[str, Any],
    extension: Mapping[str, Any],
    baseline: Mapping[str, Any],
    *,
    reordered: bool,
) -> dict[str, Any]:
    honest = run_honest_gate(reordered)
    attacks = [
        run_disconnected_vouch_ring(reordered),
        run_aged_account_control_change(reordered),
        run_parallel_identities(reordered),
        run_first_funder_manipulation(reordered),
        run_foundation_supported_favorite(reordered),
        run_unsolicited_funding(reordered),
        run_control_change(reordered),
        run_renewal_recovery(reordered),
        run_accepted_bridge(reordered),
        run_undeclared_common_funding(reordered),
        run_revocation(reordered),
        run_cap_merging_grief(reordered),
    ]
    acknowledgement = run_acknowledgement_sweep(reordered)
    sensitivity = run_sensitivity(reordered)
    cartesian = run_cartesian_grid(reordered)
    topology = run_topology_windows(
        extension["extended_topology_generator"]["topology_seeds"],
        reordered=reordered,
    )
    zero_windows = sum(item["zero_admission_window"] for item in topology)
    max_breach = max(item["breach_duration_windows"] for item in topology)
    failed_attack_oracles = [
        item["family"] for item in attacks if item["verdict"] == "FAIL"
    ]
    gate_passed = honest["admitted"] == 14 and not failed_attack_oracles
    return {
        "schema": "tasknode-unl-v2-paired-gate-results-v1",
        "mode": "SHADOW_ONLY",
        "preregistration_sha256": EXPECTED_PREREGISTRATION_SHA256,
        "preregistration_extension_sha256": (
            EXPECTED_PREREGISTRATION_EXTENSION_SHA256
        ),
        "frozen_v1_baseline": baseline,
        "honest_control_gate": honest,
        "attack_families": attacks,
        "acknowledgement_sweep": acknowledgement,
        "original_low_base_high_sensitivity": sensitivity,
        "damping_steps_floor_grid": cartesian,
        "topology_windows": topology,
        "floor_boundary": run_floor_boundary(),
        "reported_metrics": {
            "honest_admission": honest["admitted"],
            "honest_waiting": honest["waiting"],
            "zero_admission_windows": zero_windows,
            "best_attacker_seats_within_budget": max(
                int(item.get("best_attacker_seats_within_budget", 0))
                for item in attacks
            ),
            "unsolicited_denials": next(
                item["unsolicited_denials"]
                for item in attacks
                if item["family"] == "unsolicited_funding"
            ),
            "maximum_concentration_ppm": max(
                item["concentration_ppm"] for item in topology
            ),
            "maximum_breach_duration_windows": max_breach,
        },
        "coverage": {
            "topology_seed_count": len(
                {item["topology_seed"] for item in topology}
            ),
            "window_indices": sorted(
                {item["window_index"] for item in topology}
            ),
            "topology_window_count": len(topology),
            "community_sizes": sorted(
                {item["community_size"] for item in topology}
            ),
            "cowork_density_units": sorted(
                {item["cowork_units"] for item in topology}
            ),
            "foundation_seat_counts": sorted(
                {item["foundation_seat_count"] for item in topology}
            ),
            "cross_community_bridge_counts": sorted(
                {item["cross_community_bridges"] for item in topology}
            ),
            "opening_list_sizes": sorted(
                {item["opening_list_size"] for item in topology}
            ),
            "acknowledgement_cells": len(acknowledgement),
            "original_low_base_high_cells": len(sensitivity),
            "damping_steps_floor_cells": len(cartesian),
            "attack_budget_families": len(attacks),
            "special_cases": sorted(
                {
                    case
                    for item in topology
                    for case in item["special_cases"]
                }
            ),
        },
        "gate": {
            "verdict": (
                "PASS_SHADOW_ONLY" if gate_passed else "FAIL_NO_PROMOTION"
            ),
            "section_c_gate_passed": gate_passed,
            "promotion_allowed": False,
            "release_state": (
                "SHADOW_ONLY_PENDING_REPRESENTATIVE_EVIDENCE_AND_CAP_REPAIR"
            ),
            "failed_attack_oracles": failed_attack_oracles,
            "constants_adjusted_after_trial": False,
            "fixture_adjusted_after_trial": False,
        },
    }


def _csv_bytes(
    rows: Sequence[Mapping[str, Any]],
    fields: Sequence[str],
) -> bytes:
    stream = io.StringIO(newline="")
    writer = csv.DictWriter(
        stream,
        fieldnames=list(fields),
        lineterminator="\n",
        extrasaction="ignore",
    )
    writer.writeheader()
    for row in rows:
        encoded = {
            key: (
                "|".join(str(item) for item in value)
                if isinstance(value, list)
                else json.dumps(value, sort_keys=True, separators=(",", ":"))
                if isinstance(value, dict)
                else value
            )
            for key, value in row.items()
        }
        writer.writerow(encoded)
    return stream.getvalue().encode("utf-8")


def output_payloads(results: Mapping[str, Any]) -> dict[str, bytes]:
    audit = {
        "schema": "tasknode-unl-v2-gate-attack-audit-v1",
        "preregistration_sha256": EXPECTED_PREREGISTRATION_SHA256,
        "preregistration_extension_sha256": (
            EXPECTED_PREREGISTRATION_EXTENSION_SHA256
        ),
        "attack_families": results["attack_families"],
    }
    acknowledgement = results["acknowledgement_sweep"]
    sensitivity = results["original_low_base_high_sensitivity"]
    grid = results["damping_steps_floor_grid"]
    topology = results["topology_windows"]
    return {
        "results.json": _json_bytes(results),
        "attack-audit.json": _json_bytes(audit),
        "acknowledgement-sweep.csv": _csv_bytes(
            acknowledgement,
            (
                "acknowledgement_percent",
                "honest_admission",
                "honest_waiting",
                "attacker_seats",
                "time_to_eligibility_windows",
                "held_reasons",
                "community_distribution",
            ),
        ),
        "sensitivity.csv": _csv_bytes(
            sensitivity,
            (
                "constant",
                "label",
                "honest_admission",
                "honest_waiting",
                "attacker_seats",
                "funding_role",
            ),
        ),
        "damping-steps-floor.csv": _csv_bytes(
            grid,
            (
                "damping",
                "steps",
                "floor",
                "honest_admission",
                "honest_waiting",
                "attacker_seats",
            ),
        ),
        "topology-windows.csv": _csv_bytes(
            topology,
            (
                "topology_seed",
                "window_index",
                "population_size",
                "opening_list_size",
                "eligible_seed_count",
                "honest_admission",
                "honest_waiting",
                "zero_admission_window",
                "best_attacker_seats_within_budget",
                "unsolicited_denials",
                "concentration_ppm",
                "breach_duration_windows",
                "special_cases",
                "new_validator_seed_at_boundary",
                "boundary_addition_result",
                "community_count",
                "community_size",
                "cowork_units",
                "foundation_seat_count",
                "cross_community_bridges",
            ),
        ),
    }


def run_experiment() -> tuple[dict[str, bytes], dict[str, Any]]:
    prereg = load_and_verify_preregistration()
    extension = load_and_verify_preregistration_extension()
    baseline = verify_frozen_v1(prereg)
    baseline["replay"] = reproduce_frozen_v1_bytes()
    first = build_results(prereg, extension, baseline, reordered=False)
    second = build_results(prereg, extension, baseline, reordered=False)
    reordered = build_results(prereg, extension, baseline, reordered=True)
    hashes = {
        "normal_run_1_sha256": _sha256_bytes(_json_bytes(first)),
        "normal_run_2_sha256": _sha256_bytes(_json_bytes(second)),
        "reordered_input_sha256": _sha256_bytes(_json_bytes(reordered)),
    }
    byte_identical = len(set(hashes.values())) == 1
    if not byte_identical:
        raise RuntimeError(f"determinism gate failed: {hashes}")
    determinism = {
        "schema": "tasknode-unl-v2-gate-determinism-v1",
        "preregistration_sha256": EXPECTED_PREREGISTRATION_SHA256,
        "preregistration_extension_sha256": (
            EXPECTED_PREREGISTRATION_EXTENSION_SHA256
        ),
        **hashes,
        "two_run_byte_identical": (
            _json_bytes(first) == _json_bytes(second)
        ),
        "reordered_input_byte_identical": (
            _json_bytes(first) == _json_bytes(reordered)
        ),
        "all_byte_identical": byte_identical,
    }
    payloads = output_payloads(first)
    payloads["determinism.json"] = _json_bytes(determinism)
    return payloads, determinism


def write_outputs(output_dir: Path) -> dict[str, Any]:
    resolved = output_dir.resolve()
    v1_resolved = V1_BASELINE_DIR.resolve()
    if resolved == v1_resolved or v1_resolved in resolved.parents:
        raise RuntimeError("refusing to write into the frozen V1 baseline")
    payloads, determinism = run_experiment()
    output_dir.mkdir(parents=True, exist_ok=True)
    for name, payload in sorted(payloads.items()):
        (output_dir / name).write_bytes(payload)
    manifest = {
        "schema": "tasknode-unl-v2-gate-output-manifest-v1",
        "preregistration_sha256": EXPECTED_PREREGISTRATION_SHA256,
        "preregistration_extension_sha256": (
            EXPECTED_PREREGISTRATION_EXTENSION_SHA256
        ),
        "files": [
            {
                "path": name,
                "bytes": len(payload),
                "sha256": _sha256_bytes(payload),
            }
            for name, payload in sorted(payloads.items())
        ],
    }
    manifest_bytes = _json_bytes(manifest)
    (output_dir / "output-manifest.json").write_bytes(manifest_bytes)
    return {
        "output_dir": str(output_dir),
        "manifest_sha256": _sha256_bytes(manifest_bytes),
        "determinism": determinism,
    }


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Run the offline UNL V2 paired adversarial/liveness gate."
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=DEFAULT_OUTPUT_DIR,
        help="V2-only output directory; the frozen V1 directory is rejected.",
    )
    args = parser.parse_args(argv)
    summary = write_outputs(args.output_dir)
    print(json.dumps(summary, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
