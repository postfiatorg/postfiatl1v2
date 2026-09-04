"""Focused tests for deterministic Task Node UNL accountability scoring."""

from __future__ import annotations

import copy
import json
import unittest
from fractions import Fraction
from pathlib import Path

from postfiat_rpc import tasknode_unl_schema as schema
from postfiat_rpc.tasknode_unl_accountability import (
    evaluate_accountability_document,
    score_terms,
)

FIXTURE_PATH = (
    Path(__file__).resolve().parent
    / "fixtures"
    / "tasknode_unl"
    / "accountability.json"
)


def _fixtures() -> dict:
    return json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))


class PublishedConstantTests(unittest.TestCase):
    def test_accountability_constants_match_the_plan(self) -> None:
        self.assertEqual(schema.ACCOUNTABILITY_WINDOW_DAYS, 180)
        self.assertEqual(schema.ACCOUNTABILITY_WORK_DENOMINATOR, 40)
        self.assertEqual(
            schema.ACCOUNTABILITY_TENURE_DENOMINATOR_DAYS, 365
        )
        self.assertEqual(schema.ACCOUNTABILITY_FLOOR, 70)
        self.assertEqual(
            schema.ACCOUNTABILITY_TERM_WEIGHTS,
            (
                ("work", 35),
                ("tenure", 25),
                ("quality", 20),
                ("standing", 10),
                ("badge", 10),
            ),
        )

    def test_shared_graph_and_future_churn_constants_match_the_plan(self) -> None:
        self.assertEqual(schema.VOUCH_EDGE_WEIGHT, 1)
        self.assertEqual(schema.COWORK_EDGE_WEIGHT, 1)
        self.assertEqual(schema.COWORK_EDGE_CAP, 3)
        self.assertEqual(schema.FUNDING_EDGE_WEIGHT, 2)
        self.assertEqual(schema.TRUST_WALK_ITERATIONS, 20)
        self.assertEqual(schema.TRUST_WALK_DAMPING, Fraction(85, 100))
        self.assertEqual(
            schema.TRUST_WALK_SEED_DAMPING, Fraction(15, 100)
        )
        self.assertEqual(
            schema.CONDUCTANCE_CUT_THRESHOLD, Fraction(1, 10)
        )
        self.assertEqual(schema.connectivity_floor(20), Fraction(1, 40))
        self.assertEqual(schema.cluster_seat_limit(20), 2)
        self.assertEqual(schema.cluster_seat_limit(50), 5)
        self.assertEqual(
            schema.SINGLE_CHANGE_UNTIL_VALIDATOR_COUNT, 39
        )
        self.assertEqual(
            schema.MAX_CHANGES_BELOW_VALIDATOR_THRESHOLD, 1
        )


class FormulaTests(unittest.TestCase):
    def _zero_terms(self) -> dict[str, Fraction]:
        return {
            term: Fraction(0, 1)
            for term in schema.ACCOUNTABILITY_TERMS
        }

    def test_every_term_clamps_at_both_ends(self) -> None:
        for term, _weight in schema.ACCOUNTABILITY_TERM_WEIGHTS:
            with self.subTest(term=term, edge="lower"):
                values = self._zero_terms()
                values[term] = Fraction(-1, 1)
                calculated = score_terms(values)
                self.assertEqual(
                    dict(calculated.clamped_terms)[term], Fraction(0, 1)
                )
            with self.subTest(term=term, edge="upper"):
                values = self._zero_terms()
                values[term] = Fraction(2, 1)
                calculated = score_terms(values)
                self.assertEqual(
                    dict(calculated.clamped_terms)[term], Fraction(1, 1)
                )

    def test_floor_70_boundary_is_reachable_and_69_does_not_pass(self) -> None:
        score_70 = score_terms(
            {
                "work": 1,
                "tenure": 1,
                "quality": 0,
                "standing": 0,
                "badge": 1,
            }
        )
        score_69 = score_terms(
            {
                "work": 1,
                "tenure": 1,
                "quality": Fraction(9, 20),
                "standing": 0,
                "badge": 0,
            }
        )
        self.assertEqual(score_70.exact_score, 70)
        self.assertEqual(score_70.projected_score, 70)
        self.assertTrue(score_70.to_dict()["meets_floor"])
        self.assertEqual(score_69.exact_score, 69)
        self.assertEqual(score_69.projected_score, 69)
        self.assertFalse(score_69.to_dict()["meets_floor"])

    def test_final_score_is_floored_once_after_the_weighted_sum(self) -> None:
        calculated = score_terms(
            {
                "work": Fraction(1, 2),
                "tenure": Fraction(1, 2),
                "quality": Fraction(37, 40),
                "standing": 1,
                "badge": 1,
            }
        )
        self.assertEqual(calculated.exact_score, Fraction(137, 2))
        self.assertEqual(calculated.projected_score, 68)

    def test_float_terms_are_rejected(self) -> None:
        terms = self._zero_terms()
        terms["quality"] = 0.9  # type: ignore[assignment]
        with self.assertRaisesRegex(
            schema.TaskNodeUnlError, "non_rational_term"
        ):
            score_terms(terms)


class FixtureEvaluationTests(unittest.TestCase):
    def test_proposal_example_reaches_the_accountability_floor(self) -> None:
        case = _fixtures()["cases"][0]
        result = evaluate_accountability_document(case["input"])
        expected = case["expected"]

        self.assertEqual(result.status, "scored")
        self.assertEqual(
            result.accepted_network_tasks,
            expected["accepted_network_tasks"],
        )
        self.assertEqual(
            result.verification_passes,
            expected["verification_passes"],
        )
        self.assertEqual(
            result.verification_total,
            expected["verification_total"],
        )
        self.assertIsNotNone(result.calculation)
        assert result.calculation is not None
        exact = expected["exact_score"]
        self.assertEqual(
            result.calculation.exact_score,
            Fraction(exact["numerator"], exact["denominator"]),
        )
        self.assertEqual(
            result.calculation.projected_score,
            expected["projected_score"],
        )
        self.assertEqual(
            result.calculation.to_dict()["meets_floor"],
            expected["meets_floor"],
        )
        self.assertGreaterEqual(
            result.calculation.projected_score,
            schema.ACCOUNTABILITY_FLOOR,
        )

    def test_personal_task_is_excluded_from_work(self) -> None:
        case = _fixtures()["cases"][0]
        input_document = copy.deepcopy(case["input"])
        personal_count = sum(
            task["kind"] == "personal"
            and task["accepted_at"] is not None
            for task in input_document["tasks"]
        )
        self.assertEqual(personal_count, 1)
        result = evaluate_accountability_document(input_document)
        self.assertEqual(result.accepted_network_tasks, 20)

    def test_missing_required_evidence_holds(self) -> None:
        case = _fixtures()["cases"][1]
        result = evaluate_accountability_document(case["input"])
        self.assertEqual(result.status, case["expected"]["status"])
        self.assertEqual(
            list(result.hold_reasons),
            case["expected"]["hold_reasons"],
        )
        self.assertIsNone(result.calculation)

    def test_dispute_open_before_window_still_reduces_standing(self) -> None:
        document = copy.deepcopy(_fixtures()["cases"][0]["input"])
        document["disputes"] = [
            {
                "dispute_id": "old-but-open",
                "opened_at": "2025-01-01T00:00:00Z",
                "resolved_at": None,
            }
        ]

        result = evaluate_accountability_document(document)

        self.assertEqual(result.status, "scored")
        self.assertEqual(result.open_disputes, 1)
        assert result.calculation is not None
        self.assertEqual(
            dict(result.calculation.raw_terms)["standing"],
            Fraction(2, 3),
        )

    def test_reward_before_task_lifecycle_cannot_manufacture_tenure(self) -> None:
        document = copy.deepcopy(_fixtures()["cases"][0]["input"])
        task = next(
            task
            for task in document["tasks"]
            if task["verified_at"] is not None
        )
        task["rewarded_at"] = "2024-01-01T00:00:00Z"

        with self.assertRaisesRegex(
            schema.TaskNodeUnlError, "reward_before_"
        ):
            evaluate_accountability_document(document)

    def test_input_order_does_not_change_canonical_output(self) -> None:
        original = copy.deepcopy(_fixtures()["cases"][0]["input"])
        reordered = copy.deepcopy(original)
        reordered["tasks"].reverse()
        reordered["disputes"].reverse()
        first = evaluate_accountability_document(original)
        second = evaluate_accountability_document(reordered)
        self.assertEqual(first, second)
        self.assertEqual(first.canonical_bytes(), second.canonical_bytes())
        self.assertEqual(
            first.canonical_bytes(),
            first.canonical_bytes(),
        )

    def test_per_run_constant_override_is_rejected(self) -> None:
        document = copy.deepcopy(_fixtures()["cases"][0]["input"])
        document["weights"] = {"work": 99}
        with self.assertRaisesRegex(
            schema.TaskNodeUnlError, "unknown_field"
        ):
            evaluate_accountability_document(document)

    def test_canonical_output_contains_no_binary_float(self) -> None:
        result = evaluate_accountability_document(
            _fixtures()["cases"][0]["input"]
        )
        payload = json.loads(result.canonical_bytes())
        self.assertEqual(payload["mode"], "SHADOW_ONLY")
        self.assertEqual(
            payload["calculation"]["exact_score"],
            {"denominator": 2, "numerator": 161},
        )
        self.assertEqual(
            payload["calculation"]["clamped_terms"]["quality"],
            {"denominator": 10, "numerator": 9},
        )


if __name__ == "__main__":
    unittest.main()
