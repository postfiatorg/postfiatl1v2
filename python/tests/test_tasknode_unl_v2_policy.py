"""Focused policy, graph-state, and admission tests for UNL V2 section B."""

from __future__ import annotations

import hashlib
import json
import unittest
from dataclasses import replace
from datetime import datetime, timedelta, timezone
from fractions import Fraction
from pathlib import Path

from postfiat_rpc.tasknode_unl_v2_evidence import (
    ActiveControlDeclaration,
    ActiveRelation,
    ContinuityAssessment,
    EvidenceSnapshotResult,
)
from postfiat_rpc.tasknode_unl_v2_policy import (
    CLEAR,
    EXISTING_BREACH,
    FROZEN,
    NO_PROPOSAL,
    PROPOSAL,
    PROPOSE_ADD,
    SATURATED,
    AdmissionCandidate,
    PriorBreach,
    RegistryRoundState,
    RegistrySeat,
    advance_shadow_round,
    admission_policy_document,
    connectivity_floor,
    evaluate_admission_round,
    freeze_admission_window,
    permitted_integer_seats,
    prior_breaches_from_report,
    recount_limit_states,
    social_seat_limit,
)
from postfiat_rpc.tasknode_unl_v2_schema import (
    EvaluationWindow,
    ValidationFailure,
)

UTC = timezone.utc
FIXTURE_PATH = (
    Path(__file__).resolve().parent
    / "fixtures"
    / "tasknode_unl_v2"
    / "policy-golden.json"
)


def _digest(label: str) -> str:
    return hashlib.sha256(f"TEST-ONLY:V2-POLICY:{label}".encode()).hexdigest()


def _relation(kind: str, source: str, target: str, unit: int = 0) -> ActiveRelation:
    label = f"{kind}:{source}:{target}:{unit}"
    return ActiveRelation(
        relation_kind=kind,
        source_account=source,
        target_account=target,
        statement_digest=_digest(f"statement:{label}"),
        credit_id=_digest(f"credit:{label}"),
        effective_window=7,
        expiry_window=20,
        record_digest=_digest(f"record:{label}"),
    )


def _continuity(account: str, status: str = "READY") -> ContinuityAssessment:
    return ContinuityAssessment(
        account_id=account,
        control_epoch=_digest(f"epoch:{account}"),
        status=status,
        incumbent=account.startswith("account-"),
        retain_incumbent=account.startswith("account-"),
        reasons=() if status == "READY" else ("fresh_window_incomplete",),
        fresh_score_evidence=4 if status == "READY" else 0,
        renewed_vouches=1 if status == "READY" else 0,
        post_epoch_cowork=1 if status == "READY" else 0,
    )


def _evidence(
    nodes: tuple[str, ...],
    *,
    window_index: int = 7,
    relations: tuple[ActiveRelation, ...] = (),
    declarations: tuple[ActiveControlDeclaration, ...] = (),
    hold_accounts: tuple[str, ...] = (),
    funding: tuple[dict, ...] = (),
) -> EvidenceSnapshotResult:
    start = datetime(2030, 1, 1, tzinfo=UTC) + timedelta(
        days=180 * window_index
    )
    return EvidenceSnapshotResult(
        status="verified_with_holds" if hold_accounts else "verified",
        failures=(),
        record_rejections=(),
        hold_accounts=tuple(sorted(hold_accounts)),
        commitments={
            "policy_root": _digest("section-a-policy"),
            "input_root": _digest("section-a-input"),
            "registry_root": _digest("section-a-control-registry"),
        },
        window=EvaluationWindow(
            index=window_index,
            start=start,
            end=start + timedelta(days=180),
        ),
        audit_funding_observations=funding,
        historical_score_evidence=(),
        active_vouches=tuple(
            item for item in relations if item.relation_kind == "vouch"
        ),
        active_cowork=tuple(
            item for item in relations if item.relation_kind == "cowork"
        ),
        active_control_declarations=declarations,
        continuity=tuple(
            _continuity(
                node,
                "HOLD_CONTINUITY" if node in hold_accounts else "READY",
            )
            for node in sorted(nodes)
        ),
    )


def _scenario(
    *,
    window_index: int = 7,
    prior_breaches: tuple[PriorBreach, ...] = (),
    hold_accounts: tuple[str, ...] = (),
    reverse: bool = False,
):
    incumbents = tuple(f"account-{index:02d}" for index in range(30))
    candidates = (
        "candidate-breach",
        "candidate-clear",
        "candidate-control",
        "candidate-next",
        "candidate-saturated",
        "candidate-third",
        "malformed-other",
    )
    nodes = (*incumbents, *candidates)
    relations = (
        _relation("cowork", "candidate-clear", "account-00"),
        *(
            _relation("cowork", "candidate-saturated", f"account-{index:02d}")
            for index in range(1, 4)
        ),
        *(
            _relation("cowork", "candidate-breach", f"account-{index:02d}")
            for index in range(4, 8)
        ),
        _relation("cowork", "candidate-control", "account-09"),
        _relation("cowork", "candidate-next", "account-00"),
        _relation("cowork", "candidate-third", "account-11"),
    )
    declaration = ActiveControlDeclaration(
        statement_digest=_digest("control:08+candidate"),
        members=("account-08", "candidate-control"),
        effective_window=7,
        expiry_window=20,
        record_digest=_digest("control-record:08+candidate"),
    )
    evidence = _evidence(
        tuple(reversed(nodes)) if reverse else nodes,
        window_index=window_index,
        relations=tuple(reversed(relations)) if reverse else relations,
        declarations=(declaration,),
        hold_accounts=hold_accounts,
    )
    if reverse:
        evidence = replace(
            evidence,
            continuity=tuple(reversed(evidence.continuity)),
        )
    seats = tuple(
        RegistrySeat(f"validator-{index:02d}", account)
        for index, account in enumerate(incumbents)
    )
    frozen = freeze_admission_window(
        evidence,
        opening_registry_root=_digest("validator-registry-opening"),
        nodes=tuple(reversed(nodes)) if reverse else nodes,
        opening_seats=tuple(reversed(seats)) if reverse else seats,
        prior_breaches=prior_breaches,
    )
    state = RegistryRoundState(
        round_index=100,
        current_registry_root=_digest("registry-round-100"),
        prior_registry_root=_digest("registry-round-99"),
        prior_registry_round=99,
        seats=tuple(reversed(seats)) if reverse else seats,
        changes_this_round=0,
        transition_budget=1,
    )
    return frozen, state


def _candidate(name: str, *, root_label: str = "registry-round-100", round_: int = 100):
    return AdmissionCandidate(
        validator_id=f"validator-{name}",
        account_id=name,
        control_epoch=_digest(f"epoch:{name}"),
        source_registry_root=_digest(root_label),
        source_registry_round=round_,
    )


class TestLockedConstants(unittest.TestCase):
    def test_fractional_social_cap_never_rounds_up(self) -> None:
        expected = {
            20: (social_seat_limit(20), permitted_integer_seats(social_seat_limit(20))),
            30: (social_seat_limit(30), permitted_integer_seats(social_seat_limit(30))),
            25: (social_seat_limit(25), permitted_integer_seats(social_seat_limit(25))),
        }
        self.assertEqual(expected[20], (2, 2))
        self.assertEqual(expected[30], (3, 3))
        self.assertEqual(str(expected[25][0]), "5/2")
        self.assertEqual(expected[25][1], 2)
        self.assertEqual(connectivity_floor(25), Fraction(1, 50))

    def test_policy_has_exact_constants_and_no_funding_power(self) -> None:
        policy = admission_policy_document()
        self.assertEqual(policy["constants"]["walk_damping"], Fraction(17, 20))
        self.assertEqual(policy["constants"]["seed_damping"], Fraction(3, 20))
        self.assertEqual(policy["constants"]["walk_steps"], 20)
        self.assertEqual(
            policy["constants"]["conductance_threshold"], Fraction(1, 10)
        )
        self.assertEqual(policy["funding_role"], "AUDIT_ONLY_NO_MASS_NO_VETO")
        self.assertEqual(
            policy["graph_inputs"],
            ["accepted_bilateral_vouch", "accepted_bilateral_cowork"],
        )

    def test_duplicate_vouch_and_cowork_credit_caps_are_exact(self) -> None:
        nodes = ("account-a", "account-b")
        first_vouch = _relation("vouch", "account-a", "account-b", 0)
        second_vouch = _relation("vouch", "account-a", "account-b", 1)
        cowork = tuple(
            _relation("cowork", "account-a", "account-b", index)
            for index in range(5)
        )
        duplicate = replace(
            cowork[0], record_digest=_digest("duplicate-record")
        )
        evidence = _evidence(
            nodes,
            relations=(first_vouch, second_vouch, *cowork, duplicate),
        )
        frozen = freeze_admission_window(
            evidence,
            opening_registry_root=_digest("duplicate-registry"),
            nodes=nodes,
            opening_seats=(
                RegistrySeat("validator-a", "account-a"),
                RegistrySeat("validator-b", "account-b"),
            ),
        )
        self.assertEqual(frozen.status, FROZEN)
        credits = {item.kind: item for item in frozen.graph_credits}
        self.assertEqual(credits["vouch"].credited_units, 1)
        self.assertEqual(credits["vouch"].weight, Fraction(1, 1))
        self.assertEqual(credits["cowork"].credited_units, 3)
        self.assertEqual(credits["cowork"].weight, Fraction(3, 1))
        raw = {source: dict(row) for source, row in frozen.raw_rows}
        self.assertEqual(raw["account-a"]["account-b"], Fraction(4, 1))
        self.assertEqual(raw["account-b"]["account-a"], Fraction(3, 1))


class TestClusterAndControlState(unittest.TestCase):
    def test_clear_saturated_and_existing_breach_are_local(self) -> None:
        frozen, state = _scenario()
        self.assertEqual(frozen.status, FROZEN)

        clear = evaluate_admission_round(
            frozen, state, _candidate("candidate-clear")
        )
        saturated = evaluate_admission_round(
            frozen, state, _candidate("candidate-saturated")
        )
        breach = evaluate_admission_round(
            frozen, state, _candidate("candidate-breach")
        )

        self.assertEqual(clear.status, PROPOSAL)
        self.assertEqual(clear.decision.action, PROPOSE_ADD)
        self.assertTrue(clear.unresolved_limits)
        self.assertNotIn(
            clear.decision.candidate.account_id,
            {
                member
                for item in clear.unresolved_limits
                for member in item.members
            },
        )

        social = {
            report.decision.candidate.account_id: next(
                item
                for item in report.limit_states
                if item.limit_kind == "SOCIAL_CLUSTER"
                and report.decision.candidate.account_id in item.members
            )
            for report in (clear, saturated, breach)
        }
        self.assertEqual(social["candidate-clear"].state, CLEAR)
        self.assertEqual(social["candidate-saturated"].state, SATURATED)
        self.assertEqual(social["candidate-breach"].state, EXISTING_BREACH)
        self.assertEqual(social["candidate-breach"].excess_seats, 1)
        for item in social.values():
            self.assertTrue(item.causative_evidence)

        saturated_codes = {item.code for item in saturated.decision.reasons}
        breach_codes = {item.code for item in breach.decision.reasons}
        self.assertIn("SOCIAL_CLUSTER_SATURATED", saturated_codes)
        self.assertIn("SOCIAL_CLUSTER_EXISTING_BREACH", breach_codes)

    def test_declared_control_group_one_seat_restriction(self) -> None:
        frozen, state = _scenario()
        report = evaluate_admission_round(
            frozen, state, _candidate("candidate-control")
        )
        control = next(
            item
            for item in report.limit_states
            if item.limit_kind == "DECLARED_CONTROL_GROUP"
            and "candidate-control" in item.members
        )
        self.assertEqual(control.state, SATURATED)
        self.assertEqual(control.occupied_seats, 1)
        self.assertEqual(
            control.causative_evidence,
            (_digest("control-record:08+candidate"),),
        )
        self.assertIn(
            "DECLARED_CONTROL_GROUP_SATURATED",
            {item.code for item in report.decision.reasons},
        )


class TestBreachContinuityAndChurn(unittest.TestCase):
    def test_breach_survives_full_window_without_eviction(self) -> None:
        frozen, state = _scenario(window_index=7)
        first = evaluate_admission_round(
            frozen, state, _candidate("candidate-breach")
        )
        first_breach = next(
            item
            for item in first.unresolved_limits
            if "candidate-breach" in item.members
        )
        self.assertEqual(first_breach.review_state, "FULL_WINDOW_REVIEW")
        self.assertEqual(first_breach.first_observed_window, 7)

        next_frozen, next_state = _scenario(
            window_index=8,
            prior_breaches=prior_breaches_from_report(first),
        )
        second = evaluate_admission_round(
            next_frozen, next_state, _candidate("candidate-breach")
        )
        persistent = next(
            item
            for item in second.unresolved_limits
            if item.limit_id == first_breach.limit_id
        )
        self.assertEqual(persistent.review_state, "PERSISTENT_UNRESOLVED")
        self.assertEqual(persistent.first_observed_window, 7)
        self.assertEqual(
            second.preserved_incumbent_seat_ids,
            tuple(f"validator-{index:02d}" for index in range(30)),
        )
        self.assertFalse(second.churn["automatic_eviction_selector"])
        self.assertFalse(second.churn["correction_override"])
        self.assertTrue(second.churn["requires_existing_authorized_churn"])

    def test_seed_set_is_frozen_and_old_root_overlap_is_one_round(self) -> None:
        frozen, state = _scenario()
        first = evaluate_admission_round(
            frozen, state, _candidate("candidate-clear")
        )
        next_state = advance_shadow_round(
            frozen,
            state,
            first,
            new_registry_root=_digest("registry-round-101"),
        )
        self.assertNotIn("candidate-clear", frozen.eligible_seeds)
        self.assertEqual(dict(frozen.seed_vector)["candidate-clear"], 0)
        self.assertIn(
            "candidate-clear",
            {seat.account_id for seat in next_state.seats},
        )

        one_old = evaluate_admission_round(
            frozen,
            next_state,
            _candidate(
                "candidate-next",
                root_label="registry-round-100",
                round_=100,
            ),
        )
        self.assertEqual(one_old.status, PROPOSAL)
        next_next_state = advance_shadow_round(
            frozen,
            next_state,
            one_old,
            new_registry_root=_digest("registry-round-102"),
        )
        too_old = evaluate_admission_round(
            frozen,
            next_next_state,
            _candidate(
                "candidate-third",
                root_label="registry-round-100",
                round_=100,
            ),
        )
        self.assertIn(
            "REGISTRY_ROOT_OUTSIDE_ONE_ROUND_OVERLAP",
            {item.code for item in too_old.decision.reasons},
        )

    def test_current_seats_are_recounted_after_conceptual_round(self) -> None:
        frozen, state = _scenario()
        first = evaluate_admission_round(
            frozen, state, _candidate("candidate-clear")
        )
        next_state = advance_shadow_round(
            frozen,
            state,
            first,
            new_registry_root=_digest("registry-round-101"),
        )
        before = next(
            item
            for item in first.limit_states
            if item.limit_kind == "SOCIAL_CLUSTER"
            and "candidate-clear" in item.members
        )
        second = evaluate_admission_round(
            frozen,
            next_state,
            _candidate(
                "candidate-next",
                root_label="registry-round-101",
                round_=101,
            ),
        )
        self.assertEqual(second.decision.action, PROPOSE_ADD)
        final_state = advance_shadow_round(
            frozen,
            next_state,
            second,
            new_registry_root=_digest("registry-round-102"),
        )
        after = next(
            item
            for item in recount_limit_states(frozen, final_state.seats)
            if item.limit_id == before.limit_id
        )
        self.assertEqual(after.occupied_seats, before.occupied_seats + 2)
        self.assertEqual(after.state, SATURATED)


class TestFailClosedAndEvidenceFlow(unittest.TestCase):
    def test_empty_non_foundation_seeds_produce_no_proposal(self) -> None:
        accounts = ("account-a", "candidate-a")
        evidence = _evidence(accounts)
        frozen = freeze_admission_window(
            evidence,
            opening_registry_root=_digest("registry"),
            nodes=accounts,
            opening_seats=(RegistrySeat("validator-a", "account-a"),),
            foundation_accounts=("account-a",),
        )
        self.assertEqual(frozen.status, NO_PROPOSAL)
        self.assertEqual(frozen.failures[0].field, "eligible_seeds")
        state = RegistryRoundState(
            1, _digest("registry"), None, None,
            (RegistrySeat("validator-a", "account-a"),),
        )
        report = evaluate_admission_round(
            frozen,
            state,
            AdmissionCandidate(
                "validator-candidate",
                "candidate-a",
                _digest("epoch:candidate-a"),
                _digest("registry"),
                1,
            ),
        )
        self.assertEqual(report.status, NO_PROPOSAL)
        self.assertEqual(report.decision.action, NO_PROPOSAL)

    def test_bad_section_a_commitment_result_produces_no_proposal(self) -> None:
        bad = EvidenceSnapshotResult(
            status="hold",
            failures=(
                ValidationFailure(
                    "snapshot.commitments.input_root",
                    "commitment_mismatch",
                ),
            ),
            record_rejections=(),
            hold_accounts=(),
            commitments=None,
            window=None,
            audit_funding_observations=(),
            historical_score_evidence=(),
            active_vouches=(),
            active_cowork=(),
            active_control_declarations=(),
            continuity=(),
        )
        frozen = freeze_admission_window(
            bad,
            opening_registry_root=_digest("registry"),
            nodes=("account-a",),
            opening_seats=(RegistrySeat("validator-a", "account-a"),),
        )
        self.assertEqual(frozen.status, NO_PROPOSAL)
        self.assertIn(
            "snapshot.commitments.input_root",
            {item.field for item in frozen.failures},
        )

    def test_unrelated_malformed_record_hold_does_not_poison_candidate(self) -> None:
        frozen, state = _scenario(hold_accounts=("malformed-other",))
        report = evaluate_admission_round(
            frozen, state, _candidate("candidate-clear")
        )
        self.assertEqual(report.status, PROPOSAL)
        self.assertNotIn(
            "HOLD_CONTINUITY",
            {item.code for item in report.decision.reasons},
        )

    def test_incumbent_continuity_hold_preserves_its_seat(self) -> None:
        frozen, state = _scenario(hold_accounts=("account-29",))
        report = evaluate_admission_round(
            frozen, state, _candidate("candidate-clear")
        )
        self.assertEqual(report.status, PROPOSAL)
        self.assertIn("validator-29", report.incumbent_continuity_holds)
        self.assertIn("validator-29", report.preserved_incumbent_seat_ids)
        self.assertEqual(report.churn["delta_kind"], "ADD")

    def test_control_epoch_hold_flows_into_admission(self) -> None:
        frozen, state = _scenario()
        report = evaluate_admission_round(
            frozen,
            state,
            replace(
                _candidate("candidate-clear"),
                control_epoch=_digest("wrong-epoch"),
            ),
        )
        self.assertEqual(report.status, NO_PROPOSAL)
        self.assertIn(
            "HOLD_CONTINUITY",
            {item.code for item in report.decision.reasons},
        )

    def test_funding_observations_cannot_change_graph_or_decision(self) -> None:
        frozen, state = _scenario()
        funding_evidence = _evidence(
            tuple(frozen.nodes),
            relations=tuple(
                relation
                for relation in (
                    *[
                        _relation("cowork", "candidate-clear", "account-00")
                    ],
                    *[
                        _relation(
                            "cowork",
                            "candidate-saturated",
                            f"account-{index:02d}",
                        )
                        for index in range(1, 4)
                    ],
                    *[
                        _relation(
                            "cowork",
                            "candidate-breach",
                            f"account-{index:02d}",
                        )
                        for index in range(4, 8)
                    ],
                    _relation("cowork", "candidate-control", "account-09"),
                    _relation("cowork", "candidate-next", "account-00"),
                    _relation("cowork", "candidate-third", "account-11"),
                )
            ),
            declarations=frozen.declarations,
            funding=({"observation_digest": _digest("hostile-funding")},),
        )
        with_funding = freeze_admission_window(
            funding_evidence,
            opening_registry_root=frozen.opening_registry_root,
            nodes=frozen.nodes,
            opening_seats=frozen.opening_seats,
        )
        self.assertEqual(
            dict(frozen.roots)["graph_root"],
            dict(with_funding.roots)["graph_root"],
        )
        left = evaluate_admission_round(
            frozen, state, _candidate("candidate-clear")
        )
        right = evaluate_admission_round(
            with_funding, state, _candidate("candidate-clear")
        )
        self.assertEqual(left.decision.action, right.decision.action)


class TestDeterminism(unittest.TestCase):
    def test_order_independent_and_byte_identical(self) -> None:
        frozen_a, state_a = _scenario()
        frozen_b, state_b = _scenario(reverse=True)
        self.assertEqual(frozen_a.canonical_bytes(), frozen_b.canonical_bytes())

        report_a = evaluate_admission_round(
            frozen_a, state_a, _candidate("candidate-clear")
        )
        report_b = evaluate_admission_round(
            frozen_b, state_b, _candidate("candidate-clear")
        )
        self.assertEqual(report_a.canonical_bytes(), report_b.canonical_bytes())
        self.assertEqual(
            report_a.canonical_bytes(),
            evaluate_admission_round(
                frozen_a, state_a, _candidate("candidate-clear")
            ).canonical_bytes(),
        )

    def test_golden_policy_vector(self) -> None:
        frozen, state = _scenario()
        report = evaluate_admission_round(
            frozen, state, _candidate("candidate-clear")
        )
        actual = {
            "frozen_window_root": frozen.frozen_window_root,
            "graph_root": dict(frozen.roots)["graph_root"],
            "declaration_root": dict(frozen.roots)["declaration_root"],
            "report_root": report.report_root,
            "frozen_sha256": hashlib.sha256(
                frozen.canonical_bytes()
            ).hexdigest(),
            "report_sha256": hashlib.sha256(
                report.canonical_bytes()
            ).hexdigest(),
        }
        self.assertEqual(
            actual,
            json.loads(FIXTURE_PATH.read_text(encoding="utf-8")),
        )


if __name__ == "__main__":
    unittest.main()
