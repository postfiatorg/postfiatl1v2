"""Focused tests for the Task Node UNL churn and overlap guard."""

from __future__ import annotations

import copy
import json
import unittest
from fractions import Fraction
from pathlib import Path

from postfiat_rpc.tasknode_unl_churn import evaluate_churn_guard

FIXTURE_DIR = (
    Path(__file__).resolve().parent / "fixtures" / "tasknode_unl"
)
EVALUATION_TIME = "2026-09-04T12:00:00Z"
MATURE_HOLD_START = "2026-03-08T12:00:00Z"
IMMATURE_HOLD_START = "2026-03-09T12:00:00Z"


def _fixture(name: str) -> dict:
    return json.loads(
        (FIXTURE_DIR / name).read_text(encoding="utf-8")
    )


def _baseline() -> dict:
    return copy.deepcopy(_fixture("baseline-list.json"))


def _history() -> dict:
    return copy.deepcopy(_fixture("registry-rounds.json"))


def _identity_failure(
    validator_id: str,
    *,
    started_at: str = MATURE_HOLD_START,
    reason: str = "revoked_binding",
) -> dict:
    return {
        "validator_id": validator_id,
        "reason": reason,
        "hold_started_at": started_at,
    }


def _removal_cause(
    validator_id: str,
    cause: str = "revoked_binding",
) -> dict:
    return {"validator_id": validator_id, "cause": cause}


def _proposal(
    validator_ids: list[str],
    *,
    source_round: int = 20,
    source_root: str | None = None,
    target_round: int = 21,
    transition_budget: int = 2,
    identity_failures: list[dict] | None = None,
    removal_causes: list[dict] | None = None,
) -> dict:
    return {
        "schema": "tasknode-unl-churn-proposal-v1",
        "mode": "SHADOW_ONLY",
        "source_round": source_round,
        "source_registry_root": (
            source_root if source_root is not None else "20" * 32
        ),
        "target_round": target_round,
        "proposed_validator_ids": validator_ids,
        "transition_budget": transition_budget,
        "evaluation_time": EVALUATION_TIME,
        "identity_failures": identity_failures or [],
        "removal_causes": removal_causes or [],
    }


def _rule(verdict, name: str):
    return next(rule for rule in verdict.rules if rule.rule == name)


def _reason_codes(verdict) -> set[str]:
    return {reason.code for reason in verdict.reasons}


class PreThresholdChurnTests(unittest.TestCase):
    def test_one_addition_is_allowed_below_39(self) -> None:
        baseline = _baseline()
        target = baseline["validator_ids"] + ["validator-22"]

        verdict = evaluate_churn_guard(
            baseline,
            _history(),
            _proposal(target, transition_budget=1),
        )

        self.assertEqual(verdict.status, "allow")
        self.assertEqual(verdict.additions, ("validator-22",))
        self.assertEqual(verdict.removals, ())
        churn = _rule(verdict, "safe_churn_budget")
        self.assertEqual(churn.verdict, "pass")
        self.assertEqual(
            churn.values["regime"], "pre_39_single_change"
        )
        self.assertEqual(churn.values["effective_change_budget"], 1)

    def test_one_mature_removal_is_allowed_below_39(self) -> None:
        baseline = _baseline()
        removed = baseline["validator_ids"][0]

        verdict = evaluate_churn_guard(
            baseline,
            _history(),
            _proposal(
                baseline["validator_ids"][1:],
                transition_budget=1,
                identity_failures=[_identity_failure(removed)],
                removal_causes=[_removal_cause(removed)],
            ),
        )

        self.assertEqual(verdict.status, "allow")
        self.assertEqual(verdict.additions, ())
        self.assertEqual(verdict.removals, (removed,))
        identity = _rule(
            verdict, "identity_hold_before_removal"
        )
        self.assertEqual(identity.verdict, "pass")
        self.assertEqual(
            identity.values["identity_states"][0]["stage"],
            "removal_candidate",
        )

    def test_addition_plus_removal_is_rejected_below_39(self) -> None:
        baseline = _baseline()
        removed = baseline["validator_ids"][0]
        target = baseline["validator_ids"][1:] + ["validator-22"]

        verdict = evaluate_churn_guard(
            baseline,
            _history(),
            _proposal(
                target,
                identity_failures=[_identity_failure(removed)],
                removal_causes=[_removal_cause(removed)],
            ),
        )

        self.assertEqual(verdict.status, "reject")
        self.assertIn(
            "pre_39_mixed_change_forbidden", _reason_codes(verdict)
        )
        self.assertIn(
            "pre_39_single_change_exceeded", _reason_codes(verdict)
        )

    def test_multiple_additions_are_rejected_below_39(self) -> None:
        baseline = _baseline()
        target = baseline["validator_ids"] + [
            "validator-22",
            "validator-23",
        ]

        verdict = evaluate_churn_guard(
            baseline,
            _history(),
            _proposal(target),
        )

        self.assertEqual(verdict.status, "reject")
        self.assertIn(
            "pre_39_single_change_exceeded", _reason_codes(verdict)
        )
        churn = _rule(verdict, "safe_churn_budget")
        self.assertEqual(churn.values["change_count"], 2)


class TransitionBudgetTests(unittest.TestCase):
    @staticmethod
    def _exactly_39_case(budget: int):
        identifiers = lambda start, end: [
            f"validator-{index:02d}"
            for index in range(start, end + 1)
        ]
        baseline = {
            "schema": "tasknode-unl-churn-baseline-v1",
            "mode": "SHADOW_ONLY",
            "registry_round": 30,
            "registry_root": "aa" * 32,
            "validator_ids": identifiers(1, 39),
        }
        history = {
            "schema": "tasknode-unl-registry-history-v1",
            "mode": "SHADOW_ONLY",
            "current_round": 30,
            "current_root": "aa" * 32,
            "rounds": [
                {
                    "round": 29,
                    "root": "99" * 32,
                    "validator_ids": identifiers(0, 38),
                },
                {
                    "round": 30,
                    "root": "aa" * 32,
                    "validator_ids": identifiers(1, 39),
                },
            ],
        }
        proposal = _proposal(
            identifiers(2, 40),
            source_round=30,
            source_root="aa" * 32,
            target_round=31,
            transition_budget=budget,
            identity_failures=[
                _identity_failure("validator-01")
            ],
            removal_causes=[_removal_cause("validator-01")],
        )
        return baseline, history, proposal

    def test_exactly_39_uses_supplied_transition_budget(self) -> None:
        baseline, history, proposal = self._exactly_39_case(2)

        verdict = evaluate_churn_guard(
            baseline, history, proposal
        )

        self.assertEqual(verdict.status, "allow")
        churn = _rule(verdict, "safe_churn_budget")
        self.assertEqual(
            churn.values["regime"],
            "supplied_trust_graph_transition_budget",
        )
        self.assertEqual(churn.values["effective_change_budget"], 2)
        self.assertEqual(churn.values["change_count"], 2)

    def test_exactly_39_rejects_delta_above_supplied_budget(self) -> None:
        baseline, history, proposal = self._exactly_39_case(1)

        verdict = evaluate_churn_guard(
            baseline, history, proposal
        )

        self.assertEqual(verdict.status, "reject")
        self.assertIn(
            "transition_budget_exceeded", _reason_codes(verdict)
        )


class OverlapTests(unittest.TestCase):
    def test_worked_swap_reports_19_over_21_and_18_over_22(
        self,
    ) -> None:
        baseline = _baseline()
        removed = baseline["validator_ids"][0]
        target = baseline["validator_ids"][1:] + ["validator-22"]

        verdict = evaluate_churn_guard(
            baseline,
            _history(),
            _proposal(
                target,
                identity_failures=[_identity_failure(removed)],
                removal_causes=[_removal_cause(removed)],
            ),
        )

        self.assertEqual(
            verdict.one_round_overlap.ratio, Fraction(19, 21)
        )
        self.assertEqual(
            verdict.two_round_overlap.ratio, Fraction(18, 22)
        )
        overlap = verdict.to_dict()["overlap"]
        self.assertEqual(
            overlap["one_round_behind"]["percentage_text"], "90.5%"
        )
        self.assertEqual(
            overlap["two_rounds_behind"]["percentage_text"], "81.8%"
        )
        self.assertEqual(
            overlap["one_round_behind"]["intersection_count"], 19
        )
        self.assertEqual(
            overlap["one_round_behind"]["union_count"], 21
        )

    def test_one_change_reports_19_over_20_or_20_over_21(
        self,
    ) -> None:
        baseline = _baseline()
        removed = baseline["validator_ids"][0]
        removal = evaluate_churn_guard(
            baseline,
            _history(),
            _proposal(
                baseline["validator_ids"][1:],
                transition_budget=1,
                identity_failures=[_identity_failure(removed)],
                removal_causes=[_removal_cause(removed)],
            ),
        )
        addition = evaluate_churn_guard(
            baseline,
            _history(),
            _proposal(
                baseline["validator_ids"] + ["validator-22"],
                transition_budget=1,
            ),
        )

        self.assertEqual(
            removal.one_round_overlap.ratio, Fraction(19, 20)
        )
        self.assertEqual(
            addition.one_round_overlap.ratio, Fraction(20, 21)
        )
        self.assertEqual(
            removal.one_round_overlap.to_dict()["percentage_text"],
            "95.0%",
        )
        self.assertEqual(
            addition.one_round_overlap.to_dict()["percentage_text"],
            "95.2%",
        )

    def test_two_consecutive_removals_report_18_over_20_floor(
        self,
    ) -> None:
        previous = [
            f"validator-{index:02d}" for index in range(1, 21)
        ]
        current = previous[1:]
        target = current[1:]
        baseline = {
            "schema": "tasknode-unl-churn-baseline-v1",
            "mode": "SHADOW_ONLY",
            "registry_round": 20,
            "registry_root": "20" * 32,
            "validator_ids": current,
        }
        history = {
            "schema": "tasknode-unl-registry-history-v1",
            "mode": "SHADOW_ONLY",
            "current_round": 20,
            "current_root": "20" * 32,
            "rounds": [
                {
                    "round": 19,
                    "root": "19" * 32,
                    "validator_ids": previous,
                },
                {
                    "round": 20,
                    "root": "20" * 32,
                    "validator_ids": current,
                },
            ],
        }
        removed = current[0]

        verdict = evaluate_churn_guard(
            baseline,
            history,
            _proposal(
                target,
                transition_budget=1,
                identity_failures=[_identity_failure(removed)],
                removal_causes=[_removal_cause(removed)],
            ),
        )

        self.assertEqual(verdict.status, "allow")
        self.assertEqual(
            verdict.two_round_overlap.ratio, Fraction(18, 20)
        )
        self.assertEqual(
            verdict.two_round_overlap.to_dict()["percentage_text"],
            "90.0%",
        )


class FreshnessAndIdentityTests(unittest.TestCase):
    def test_source_root_one_round_old_is_allowed(self) -> None:
        history = _history()
        source = next(
            row for row in history["rounds"] if row["round"] == 19
        )
        current = next(
            row for row in history["rounds"] if row["round"] == 20
        )
        baseline = {
            "schema": "tasknode-unl-churn-baseline-v1",
            "mode": "SHADOW_ONLY",
            "registry_round": 19,
            "registry_root": source["root"],
            "validator_ids": source["validator_ids"],
        }
        target = current["validator_ids"] + ["validator-22"]

        verdict = evaluate_churn_guard(
            baseline,
            history,
            _proposal(
                target,
                source_round=19,
                source_root=source["root"],
                transition_budget=1,
            ),
        )

        self.assertEqual(verdict.status, "allow")
        self.assertEqual(verdict.additions, ("validator-22",))
        freshness = _rule(
            verdict, "registry_root_freshness_and_round_binding"
        )
        self.assertEqual(freshness.values["source_age_rounds"], 1)

    def test_stale_root_is_rejected_with_source_root_field_named(
        self,
    ) -> None:
        history = _history()
        source = next(
            row for row in history["rounds"] if row["round"] == 18
        )
        baseline = {
            "schema": "tasknode-unl-churn-baseline-v1",
            "mode": "SHADOW_ONLY",
            "registry_round": 18,
            "registry_root": source["root"],
            "validator_ids": source["validator_ids"],
        }

        verdict = evaluate_churn_guard(
            baseline,
            history,
            _proposal(
                source["validator_ids"],
                source_round=18,
                source_root=source["root"],
            ),
        )

        self.assertEqual(verdict.status, "reject")
        stale = next(
            reason
            for reason in verdict.reasons
            if reason.code == "stale_registry_root"
        )
        self.assertEqual(stale.field, "proposal.source_registry_root")
        freshness = _rule(
            verdict, "registry_root_freshness_and_round_binding"
        )
        self.assertEqual(freshness.values["source_age_rounds"], 2)
        self.assertEqual(
            freshness.values["maximum_source_age_rounds"], 1
        )
        self.assertEqual(
            verdict.to_dict()["registry"]["current_root"],
            history["current_root"],
        )

    def test_new_identity_failure_stays_hold_before_removal(
        self,
    ) -> None:
        baseline = _baseline()
        validator_id = baseline["validator_ids"][0]
        no_removal = evaluate_churn_guard(
            baseline,
            _history(),
            _proposal(
                baseline["validator_ids"],
                transition_budget=1,
                identity_failures=[
                    _identity_failure(
                        validator_id,
                        started_at=IMMATURE_HOLD_START,
                        reason="new_control_group",
                    )
                ],
            ),
        )
        premature_removal = evaluate_churn_guard(
            baseline,
            _history(),
            _proposal(
                baseline["validator_ids"][1:],
                transition_budget=1,
                identity_failures=[
                    _identity_failure(
                        validator_id,
                        started_at=IMMATURE_HOLD_START,
                        reason="new_control_group",
                    )
                ],
                removal_causes=[
                    _removal_cause(
                        validator_id, "new_control_group"
                    )
                ],
            ),
        )

        self.assertEqual(no_removal.status, "allow")
        state = _rule(
            no_removal, "identity_hold_before_removal"
        ).values["identity_states"][0]
        self.assertEqual(state["stage"], "hold")
        self.assertEqual(state["removal_requested"], "no")
        self.assertEqual(premature_removal.status, "reject")
        self.assertIn(
            "identity_hold_window_not_elapsed",
            _reason_codes(premature_removal),
        )
        rejected_state = _rule(
            premature_removal, "identity_hold_before_removal"
        ).values["identity_states"][0]
        self.assertEqual(rejected_state["stage"], "hold")
        self.assertEqual(rejected_state["removal_requested"], "yes")


class VerdictShapeAndDeterminismTests(unittest.TestCase):
    def test_every_applied_rule_is_named_and_carries_values(self) -> None:
        baseline = _baseline()
        verdict = evaluate_churn_guard(
            baseline,
            _history(),
            _proposal(
                baseline["validator_ids"] + ["validator-22"],
                transition_budget=1,
            ),
        )

        self.assertEqual(
            [rule.rule for rule in verdict.rules],
            [
                "registry_root_freshness_and_round_binding",
                "canonical_delta_derivation",
                "safe_churn_budget",
                "identity_hold_before_removal",
                "intersection_over_union_overlap_reporting",
            ],
        )
        for rule in verdict.rules:
            self.assertTrue(rule.values)
            self.assertIn(
                rule.verdict, {"pass", "reject", "reported"}
            )

    def test_input_order_does_not_change_verdict_bytes(self) -> None:
        baseline = _baseline()
        history = _history()
        removed = baseline["validator_ids"][0]
        proposal = _proposal(
            baseline["validator_ids"][1:] + ["validator-22"],
            identity_failures=[
                _identity_failure(removed),
                _identity_failure(
                    "validator-03",
                    started_at=IMMATURE_HOLD_START,
                    reason="new_control_group",
                ),
            ],
            removal_causes=[_removal_cause(removed)],
        )
        first = evaluate_churn_guard(
            baseline, history, proposal
        )

        baseline["validator_ids"].reverse()
        history["rounds"].reverse()
        for view in history["rounds"]:
            view["validator_ids"].reverse()
        proposal["proposed_validator_ids"].reverse()
        proposal["identity_failures"].reverse()
        proposal["removal_causes"].reverse()
        second = evaluate_churn_guard(
            baseline, history, proposal
        )

        self.assertEqual(
            first.canonical_bytes(), second.canonical_bytes()
        )


if __name__ == "__main__":
    unittest.main()
