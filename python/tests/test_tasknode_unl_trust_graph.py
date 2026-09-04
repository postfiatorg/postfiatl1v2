"""Focused tests for the deterministic Task Node UNL trust graph."""

from __future__ import annotations

import copy
import json
import unittest
from fractions import Fraction
from pathlib import Path

from postfiat_rpc import tasknode_unl_schema as schema
from postfiat_rpc.tasknode_unl_trust_graph import (
    TrustEdge,
    build_raw_rows,
    derive_trust_graph_document,
    meets_connectivity,
    weighted_conductance,
)

FIXTURE_PATH = (
    Path(__file__).resolve().parent
    / "fixtures"
    / "tasknode_unl"
    / "trust-graphs.json"
)


def _fixtures() -> dict:
    return json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))


def _fraction(value: dict[str, int]) -> Fraction:
    return Fraction(value["numerator"], value["denominator"])


class EdgeWeightTests(unittest.TestCase):
    def test_edge_weights_directions_cap_and_vouch_deduplication(self) -> None:
        edges = [
            TrustEdge("a", "b", "vouch", "vouch-1"),
            TrustEdge("a", "b", "vouch", "vouch-2"),
            TrustEdge("a", "b", "cowork", "project-1"),
            TrustEdge("b", "a", "cowork", "project-2"),
            TrustEdge("a", "b", "cowork", "project-3"),
            TrustEdge("a", "b", "cowork", "project-4"),
            TrustEdge("a", "b", "funding", "funding-1"),
            TrustEdge("b", "a", "funding", "funding-2"),
        ]
        rows = build_raw_rows(("a", "b"), edges)
        self.assertEqual(rows["a"]["b"], 6)
        self.assertEqual(rows["b"]["a"], 5)

    def test_prior_window_penalty_halves_outgoing_vouch_only(self) -> None:
        edges = [
            TrustEdge("a", "b", "vouch", "vouch-a-b"),
            TrustEdge("b", "a", "vouch", "vouch-b-a"),
        ]
        rows = build_raw_rows(("a", "b"), edges, ("a",))
        self.assertEqual(rows["a"]["b"], Fraction(1, 2))
        self.assertEqual(rows["b"]["a"], 1)


class WalkFixtureTests(unittest.TestCase):
    def test_current_list_has_17_equal_seeds_and_dangling_rows(self) -> None:
        case = _fixtures()["cases"][0]
        result = derive_trust_graph_document(case["input"])
        expected = case["expected"]
        seeds = dict(result.seed_vector)
        stationary = dict(result.stationary_mass)
        transitions = {
            node: dict(row) for node, row in result.transition_rows
        }

        self.assertEqual(result.status, "scored")
        self.assertEqual(
            sum(mass > 0 for mass in seeds.values()),
            expected["seed_count"],
        )
        self.assertEqual(seeds["account-01"], _fraction(expected["seed_mass"]))
        self.assertEqual(
            stationary["account-01"],
            _fraction(expected["seed_mass"]),
        )
        for node in ("account-18", "account-19", "account-20"):
            self.assertEqual(
                stationary[node],
                _fraction(expected["foundation_mass"]),
            )
        self.assertEqual(
            result.connectivity_mass_floor,
            _fraction(expected["connectivity_floor"]),
        )
        self.assertEqual(
            list(result.connectivity_holds),
            expected["connectivity_holds"],
        )
        self.assertEqual(len(result.clusters), expected["cluster_count"])
        self.assertEqual(
            [cluster.members for cluster in result.clusters if cluster.over_seat_cap],
            expected["over_cap_clusters"],
        )

        expected_dangling_row = {
            node: Fraction(1, 17) for node in sorted(seeds)[:17]
        }
        self.assertEqual(transitions["account-20"], expected_dangling_row)
        self.assertEqual(sum(transitions["account-20"].values()), 1)

    def test_low_conductance_cut_and_cluster_seat_cap(self) -> None:
        case = _fixtures()["cases"][1]
        result = derive_trust_graph_document(case["input"])
        expected = case["expected"]

        self.assertEqual(
            [list(cluster.members) for cluster in result.clusters],
            expected["clusters"],
        )
        self.assertEqual(len(result.cuts), 1)
        cut = result.cuts[0]
        expected_cut = expected["cuts"][0]
        self.assertEqual(list(cut.left), expected_cut["left"])
        self.assertEqual(list(cut.right), expected_cut["right"])
        self.assertEqual(
            cut.conductance,
            _fraction(expected_cut["conductance"]),
        )
        self.assertLess(cut.conductance, schema.CONDUCTANCE_CUT_THRESHOLD)
        self.assertEqual(
            list(result.connectivity_holds),
            expected["connectivity_holds"],
        )
        self.assertEqual(
            result.cluster_seat_cap,
            _fraction(expected["cluster_seat_cap"]),
        )
        self.assertEqual(
            [
                list(cluster.members)
                for cluster in result.clusters
                if cluster.over_seat_cap
            ],
            expected["over_cap_clusters"],
        )

    def test_input_order_does_not_change_walk_or_output_bytes(self) -> None:
        original = copy.deepcopy(_fixtures()["cases"][1]["input"])
        reordered = copy.deepcopy(original)
        for field in (
            "nodes",
            "ratified_nodes",
            "foundation_bound_nodes",
            "edges",
            "seats",
            "penalized_vouchers",
        ):
            reordered[field].reverse()

        first = derive_trust_graph_document(original)
        second = derive_trust_graph_document(reordered)
        self.assertEqual(first, second)
        self.assertEqual(first.canonical_bytes(), second.canonical_bytes())
        self.assertEqual(first.canonical_bytes(), first.canonical_bytes())

    def test_two_distinct_seed_vouches_can_meet_connectivity_floor(self) -> None:
        document = {
            "schema": "tasknode-unl-trust-graph-input-v1",
            "nodes": ["candidate", "seed-1", "seed-2", "seed-3", "seed-4"],
            "ratified_nodes": ["seed-1", "seed-2", "seed-3", "seed-4"],
            "foundation_bound_nodes": [],
            "edges": [
                {
                    "source": "seed-1",
                    "target": "candidate",
                    "kind": "vouch",
                    "evidence_id": "vouch-1",
                },
                {
                    "source": "seed-2",
                    "target": "candidate",
                    "kind": "vouch",
                    "evidence_id": "vouch-2",
                },
            ],
            "baseline_list_size": 4,
            "seats": [],
            "penalized_vouchers": [],
        }
        result = derive_trust_graph_document(document)
        candidate_mass = dict(result.stationary_mass)["candidate"]
        self.assertGreaterEqual(candidate_mass, Fraction(1, 8))
        self.assertNotIn("candidate", result.connectivity_holds)

    def test_per_run_walk_constant_override_is_rejected(self) -> None:
        document = copy.deepcopy(_fixtures()["cases"][0]["input"])
        document["iterations"] = 21
        with self.assertRaisesRegex(
            schema.TaskNodeUnlError, "unknown_field"
        ):
            derive_trust_graph_document(document)

    def test_empty_non_foundation_seed_set_holds(self) -> None:
        document = {
            "schema": "tasknode-unl-trust-graph-input-v1",
            "nodes": ["a", "b"],
            "ratified_nodes": ["a", "b"],
            "foundation_bound_nodes": ["a", "b"],
            "edges": [],
            "baseline_list_size": 2,
            "seats": [],
            "penalized_vouchers": [],
        }
        result = derive_trust_graph_document(document)
        self.assertEqual(result.status, "hold")
        self.assertEqual(result.hold_reasons, ("empty_seed_set",))
        self.assertEqual(result.connectivity_holds, ("a", "b"))
        self.assertEqual(result.stationary_mass, ())


class ClusterControlTests(unittest.TestCase):
    def test_connectivity_floor_is_inclusive(self) -> None:
        floor = Fraction(1, 40)
        self.assertTrue(meets_connectivity(floor, 20))
        self.assertFalse(
            meets_connectivity(
                floor - Fraction(1, 10_000),
                20,
            )
        )
        self.assertTrue(
            meets_connectivity(
                floor + Fraction(1, 10_000),
                20,
            )
        )

    def test_cluster_seat_cap_uses_exact_maximum(self) -> None:
        self.assertEqual(schema.cluster_seat_limit(20), 2)
        self.assertEqual(schema.cluster_seat_limit(39), Fraction(39, 10))
        self.assertLessEqual(3, schema.cluster_seat_limit(39))
        self.assertGreater(4, schema.cluster_seat_limit(39))
        self.assertEqual(schema.cluster_seat_limit(40), 4)
        self.assertEqual(schema.cluster_seat_limit(50), 5)

    def test_conductance_threshold_is_strict(self) -> None:
        members = ("a", "b", "c", "d")
        weights = {
            ("a", "b"): Fraction(9, 2),
            ("c", "d"): Fraction(9, 2),
            ("a", "c"): Fraction(1, 1),
        }
        self.assertEqual(
            weighted_conductance(members, ("a", "b"), weights),
            Fraction(1, 10),
        )
        self.assertFalse(
            Fraction(1, 10) < schema.CONDUCTANCE_CUT_THRESHOLD
        )

    def test_result_is_exact_json_without_floats(self) -> None:
        result = derive_trust_graph_document(
            _fixtures()["cases"][1]["input"]
        )
        payload = json.loads(result.canonical_bytes())
        self.assertEqual(payload["mode"], "SHADOW_ONLY")
        self.assertEqual(
            payload["constants"]["damping"],
            {"denominator": 20, "numerator": 17},
        )
        self.assertEqual(
            payload["cuts"][0]["conductance"],
            {"denominator": 37, "numerator": 1},
        )


if __name__ == "__main__":
    unittest.main()
