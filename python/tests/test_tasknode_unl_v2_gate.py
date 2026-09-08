"""Section C tests for the offline UNL V2 paired adversarial/liveness gate."""

from __future__ import annotations

import ast
import hashlib
import importlib.util
import json
import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
EXPERIMENT_DIR = (
    REPO_ROOT
    / "benchmarks"
    / "ai-governance"
    / "tasknode-unl-v2-gate-20260908"
)
OUTPUT_DIR = EXPERIMENT_DIR / "outputs"
FIXTURE_PATH = (
    Path(__file__).resolve().parent
    / "fixtures"
    / "tasknode_unl_v2"
    / "gate-golden.json"
)
DRIVER_PATH = EXPERIMENT_DIR / "run_gate.py"


def _digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _load_driver():
    spec = importlib.util.spec_from_file_location("tasknode_unl_v2_gate", DRIVER_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("unable to load V2 gate driver")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class TestPreregistrationAndBaseline(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.driver = _load_driver()
        cls.prereg = cls.driver.load_and_verify_preregistration()
        cls.extension = cls.driver.load_and_verify_preregistration_extension()
        cls.results = json.loads((OUTPUT_DIR / "results.json").read_text())
        cls.determinism = json.loads(
            (OUTPUT_DIR / "determinism.json").read_text()
        )
        cls.golden = json.loads(FIXTURE_PATH.read_text())

    def test_preregistrations_are_hash_locked(self) -> None:
        self.assertEqual(
            _digest(EXPERIMENT_DIR / "preregistration.json"),
            self.driver.EXPECTED_PREREGISTRATION_SHA256,
        )
        self.assertEqual(
            _digest(EXPERIMENT_DIR / "preregistration-extension.json"),
            self.driver.EXPECTED_PREREGISTRATION_EXTENSION_SHA256,
        )
        self.assertEqual(
            self.extension["parent_preregistration_sha256"],
            self.driver.EXPECTED_PREREGISTRATION_SHA256,
        )

    def test_frozen_v1_baseline_is_byte_identical(self) -> None:
        verified = self.driver.verify_frozen_v1(self.prereg)
        self.assertEqual(verified["status"], "BYTE_IDENTICAL")
        self.assertEqual(len(verified["artifacts"]), 7)
        self.assertEqual(
            verified["published_results_sha256"],
            "81e27d4c16d689e2a1361b76720abe10030faee0e8962efe6387fd543137c2bd",
        )
        self.assertEqual(
            self.results["frozen_v1_baseline"]["replay"]["status"],
            "REPRODUCED_BYTE_IDENTICAL_IN_MEMORY",
        )
        self.assertEqual(
            self.results["frozen_v1_baseline"]["replay"]["writes_to_v1_namespace"],
            0,
        )

    def test_output_manifest_binds_every_generated_artifact(self) -> None:
        manifest = json.loads(
            (OUTPUT_DIR / "output-manifest.json").read_text()
        )
        for item in manifest["files"]:
            path = OUTPUT_DIR / item["path"]
            self.assertEqual(len(path.read_bytes()), item["bytes"])
            self.assertEqual(_digest(path), item["sha256"])

    def test_every_executed_trial_is_reported_without_parameter_tuning(self) -> None:
        history = json.loads((EXPERIMENT_DIR / "trial-history.json").read_text())
        self.assertEqual(
            [item["sequence"] for item in history["entries"]],
            [1, 2, 3, 4, 5, 6],
        )
        self.assertTrue(
            all(
                item["parameter_adjustments_after_trial"] is False
                for item in history["entries"]
            )
        )
        self.assertEqual(
            history["entries"][-1]["results_sha256"],
            self.golden["results_sha256"],
        )

    def test_driver_has_no_network_import(self) -> None:
        tree = ast.parse(DRIVER_PATH.read_text())
        forbidden = {
            "aiohttp",
            "http",
            "httpx",
            "requests",
            "socket",
            "urllib",
            "websockets",
        }
        imports = set()
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                imports.update(alias.name.split(".", 1)[0] for alias in node.names)
            elif isinstance(node, ast.ImportFrom) and node.module:
                imports.add(node.module.split(".", 1)[0])
        self.assertTrue(imports.isdisjoint(forbidden))

    def test_no_synthetic_sale_detection_field(self) -> None:
        self.assertNotIn("sale_detected", DRIVER_PATH.read_text())
        self.assertNotIn("sale_detected", (OUTPUT_DIR / "results.json").read_text())


class TestPairedGate(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.driver = _load_driver()
        cls.results = json.loads((OUTPUT_DIR / "results.json").read_text())
        cls.determinism = json.loads(
            (OUTPUT_DIR / "determinism.json").read_text()
        )
        cls.golden = json.loads(FIXTURE_PATH.read_text())
        cls.attacks = {
            item["family"]: item for item in cls.results["attack_families"]
        }

    def test_all_fourteen_paired_honest_controls_are_admissible(self) -> None:
        gate = self.results["honest_control_gate"]
        self.assertEqual((gate["admitted"], gate["waiting"]), (14, 0))
        self.assertEqual(gate["failed_controls"], [])
        self.assertEqual(
            [item["account"] for item in gate["controls"]],
            [f"honest-c{index:02d}-n01" for index in range(14)],
        )
        self.assertEqual(gate["verdict"], "PASS")
        self.assertEqual(self.results["gate"]["verdict"], "PASS_SHADOW_ONLY")
        self.assertFalse(self.results["gate"]["promotion_allowed"])

    def test_attack_family_verdicts_match_golden(self) -> None:
        actual = {
            family: item["verdict"] for family, item in self.attacks.items()
        }
        self.assertEqual(actual, self.golden["attack_family_verdicts"])
        self.assertEqual(
            self.attacks["aged_account_control_change"][
                "best_attacker_seats_within_budget"
            ],
            14,
        )
        self.assertEqual(
            self.attacks["accepted_bridge"]["best_attacker_seats_within_budget"],
            16,
        )

    def test_unsolicited_funding_is_audit_only_and_invariant(self) -> None:
        attack = self.attacks["unsolicited_funding"]
        self.assertTrue(attack["invariant"])
        self.assertEqual(attack["unsolicited_denials"], 0)
        self.assertTrue(attack["audit_observations_retained"])
        signatures = {
            json.dumps(item["target"], sort_keys=True)
            for item in attack["variants"]
        }
        self.assertEqual(len(signatures), 1)

    def test_control_change_recovery_and_revocation_paths(self) -> None:
        changed = self.attacks["declared_vs_hidden_control"]
        self.assertEqual(
            changed["hidden_unchanged_key_change"]["detection"],
            "UNDETECTABLE_BY_PUBLIC_INPUTS",
        )
        self.assertEqual(
            changed["hidden_unchanged_key_change"]["personhood_inference"],
            "NONE",
        )
        renewal = self.attacks["renewal_recovery"]["cases"]
        self.assertEqual(renewal["renewal_complete_window"]["result"], "PROPOSE_ADD")
        self.assertNotEqual(
            renewal["recovery_incomplete_window"]["result"],
            "PROPOSE_ADD",
        )
        self.assertEqual(
            renewal["recovery_complete_window"]["result"],
            "PROPOSE_ADD",
        )
        revocation = self.attacks["revocation"]
        self.assertEqual(revocation["active_window"]["result"], "PROPOSE_ADD")
        self.assertNotEqual(
            revocation["next_boundary"]["result"],
            "PROPOSE_ADD",
        )

    def test_cap_merge_preserves_incumbents_and_scopes_holds(self) -> None:
        attack = self.attacks["cap_merging_grief"]
        first, second = attack["windows"]
        self.assertNotEqual(first["affected_result"], "PROPOSE_ADD")
        self.assertEqual(first["unrelated_result"], "PROPOSE_ADD")
        self.assertEqual(first["preserved_incumbent_seats"], 20)
        self.assertGreater(first["excess_seats"], 0)
        self.assertTrue(first["causative_evidence"])
        self.assertEqual(second["breach_duration_windows"], 2)
        self.assertIn("PERSISTENT_UNRESOLVED", second["review_states"])

    def test_common_funding_creates_neither_mass_nor_control_group(self) -> None:
        attack = self.attacks["undeclared_common_funding"]
        self.assertTrue(attack["funding_neutral"])
        self.assertFalse(attack["control_group_synthesized"])
        self.assertEqual(attack["personhood_inference"], "NONE")


class TestSweepsAndDeterminism(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.driver = _load_driver()
        cls.results = json.loads((OUTPUT_DIR / "results.json").read_text())
        cls.determinism = json.loads(
            (OUTPUT_DIR / "determinism.json").read_text()
        )
        cls.golden = json.loads(FIXTURE_PATH.read_text())

    def test_acknowledgement_sweep_reports_paired_outcomes(self) -> None:
        rows = self.results["acknowledgement_sweep"]
        self.assertEqual(
            [item["acknowledgement_percent"] for item in rows],
            [100, 75, 50, 0],
        )
        self.assertEqual(
            [(item["honest_admission"], item["honest_waiting"]) for item in rows],
            [(14, 0), (10, 4), (7, 7), (0, 14)],
        )
        for item in rows:
            self.assertIn("time_to_eligibility_windows", item)
            self.assertIn("held_reasons", item)
            self.assertEqual(len(item["community_distribution"]), 14)

    def test_all_preregistered_parameter_cells_are_reported(self) -> None:
        self.assertEqual(
            len(self.results["original_low_base_high_sensitivity"]),
            30,
        )
        self.assertEqual(len(self.results["damping_steps_floor_grid"]), 27)
        funding_rows = [
            item
            for item in self.results["original_low_base_high_sensitivity"]
            if item["constant"] == "funding_weight"
        ]
        self.assertEqual(
            {item["funding_role"] for item in funding_rows},
            {"AUDIT_ONLY_NO_MASS_NO_VETO"},
        )
        self.assertEqual(
            len(
                {
                    (
                        item["damping"],
                        item["steps"],
                        item["floor"],
                    )
                    for item in self.results["damping_steps_floor_grid"]
                }
            ),
            27,
        )

    def test_seeded_multi_window_topologies_cover_boundaries(self) -> None:
        coverage = self.results["coverage"]
        self.assertEqual(coverage["topology_seed_count"], 30)
        self.assertEqual(coverage["topology_window_count"], 90)
        self.assertEqual(coverage["window_indices"], [7, 8, 9])
        self.assertEqual(coverage["community_sizes"], [8, 12, 16])
        self.assertEqual(coverage["cowork_density_units"], [1, 2, 3])
        self.assertEqual(coverage["cross_community_bridge_counts"], [0, 1, 2, 3])
        self.assertEqual(coverage["opening_list_sizes"], [20, 21])
        required = {
            "authorized_removal",
            "control_rotation",
            "dangling_nodes",
            "duplicate_credit_records",
            "empty_eligible_seeds",
            "exact_floor_boundary",
            "new_seed_at_boundary",
            "persistent_breach",
        }
        self.assertTrue(required.issubset(set(coverage["special_cases"])))
        rows = self.results["topology_windows"]
        self.assertTrue(any(item["eligible_seed_count"] == 0 for item in rows))
        self.assertTrue(
            any(
                item["window_index"] == 7
                and item["boundary_addition_result"] == "PROPOSE_ADD"
                for item in rows
            )
        )
        self.assertTrue(
            any(item["new_validator_seed_at_boundary"] for item in rows)
        )
        self.assertTrue(
            any(
                item["window_index"] == 8
                and item["boundary_addition_result"] == "SEATED_AT_BOUNDARY"
                and item["new_validator_seed_at_boundary"]
                for item in rows
            )
        )
        self.assertGreaterEqual(
            max(item["breach_duration_windows"] for item in rows),
            3,
        )

    def test_exact_floor_equality_and_deficit(self) -> None:
        boundary = self.results["floor_boundary"]
        self.assertEqual(boundary["exact_floor"], "1/40")
        self.assertTrue(boundary["at_floor_admissible"])
        self.assertFalse(boundary["one_epsilon_below_admissible"])

    def test_two_runs_and_reordered_inputs_are_byte_identical(self) -> None:
        hashes = {
            self.determinism["normal_run_1_sha256"],
            self.determinism["normal_run_2_sha256"],
            self.determinism["reordered_input_sha256"],
        }
        self.assertEqual(len(hashes), 1)
        self.assertTrue(self.determinism["two_run_byte_identical"])
        self.assertTrue(self.determinism["reordered_input_byte_identical"])
        self.assertEqual(
            next(iter(hashes)),
            self.golden["results_sha256"],
        )

    def test_small_policy_fixture_is_order_independent(self) -> None:
        normal = self.driver.build_scenario(reordered=False)
        reordered = self.driver.build_scenario(reordered=True)
        self.assertEqual(normal.frozen.canonical_bytes(), reordered.frozen.canonical_bytes())


if __name__ == "__main__":
    unittest.main()
