"""End-to-end tests for the fixture-driven Task Node UNL shadow runner."""

from __future__ import annotations

import copy
import json
import tempfile
import unittest
from pathlib import Path

from postfiat_rpc.tasknode_unl import main
from postfiat_rpc.tasknode_unl_policy import (
    SHADOW_INPUT_FILES,
    derive_shadow_report,
    render_shadow_markdown,
)
from postfiat_rpc.tasknode_unl_schema import canonical_json_bytes

FIXTURE_DIR = (
    Path(__file__).resolve().parent / "fixtures" / "tasknode_unl"
)
EXPECTED_JSON = FIXTURE_DIR / "expected-shadow-output.json"
EXPECTED_MARKDOWN = FIXTURE_DIR / "expected-shadow-output.md"


def _load(name: str) -> dict:
    return json.loads((FIXTURE_DIR / name).read_text(encoding="utf-8"))


def _documents() -> dict[str, object]:
    manifest = _load("shadow-input.json")
    return {
        logical_name: _load(manifest["files"][logical_name])
        for logical_name in SHADOW_INPUT_FILES
    }


def _candidate(report: dict, validator_id: str) -> dict:
    return next(
        candidate
        for candidate in report["candidates"]
        if candidate["validator_id"] == validator_id
    )


class ShadowDerivationFixtureTests(unittest.TestCase):
    def test_end_to_end_report_matches_full_committed_fixture(self) -> None:
        derived = derive_shadow_report(_documents())
        report = json.loads(canonical_json_bytes(derived))
        expected = json.loads(EXPECTED_JSON.read_text(encoding="utf-8"))

        self.assertEqual(report, expected)
        self.assertEqual(report["mode"], "SHADOW_ONLY")
        self.assertEqual(report["eligible_set"], ["validator-22"])
        self.assertEqual(
            report["selection"]["selected_additions"],
            ["validator-22"],
        )
        self.assertEqual(
            report["baseline_diff"]["additions"][0]["validator_id"],
            "validator-22",
        )
        self.assertEqual(report["churn_guard"]["status"], "allow")

        passing = _candidate(report, "validator-22")
        self.assertEqual(passing["status"], "admit")
        self.assertEqual(
            passing["admission_decision"]["reason_codes"],
            ["all_gates_passed"],
        )

        digest_hold = _candidate(report, "validator-23")
        self.assertEqual(digest_hold["status"], "hold")
        self.assertIn(
            "work_digest_signature_verification_failed",
            digest_hold["admission_decision"]["reason_codes"],
        )

        below_floor = _candidate(report, "validator-24")
        self.assertEqual(below_floor["status"], "reject")
        self.assertEqual(
            below_floor["admission_decision"]["reason_codes"],
            ["accountability_below_floor"],
        )

        cluster_hold = _candidate(report, "validator-25")
        self.assertEqual(cluster_hold["status"], "hold")
        self.assertEqual(
            cluster_hold["trust_graph"]["reason_codes"],
            ["cluster_seat_cap_exceeded"],
        )

        connectivity_hold = _candidate(report, "validator-26")
        self.assertEqual(connectivity_hold["status"], "hold")
        self.assertEqual(
            connectivity_hold["trust_graph"]["reason_codes"],
            ["connectivity_below_floor"],
        )

        authority = report["authority_boundary"]
        self.assertEqual(authority["live_authority"], "none")
        for field in (
            "registry_write_supported",
            "transaction_submission_supported",
            "ratification_supported",
            "signable_delta_emitted",
        ):
            with self.subTest(field=field):
                self.assertFalse(authority[field])

    def test_repeated_runs_are_byte_identical(self) -> None:
        first = canonical_json_bytes(derive_shadow_report(_documents()))
        second = canonical_json_bytes(derive_shadow_report(_documents()))

        self.assertEqual(first, second)
        self.assertEqual(first, EXPECTED_JSON.read_bytes())

    def test_input_order_does_not_change_report_bytes(self) -> None:
        original = _documents()
        reordered = copy.deepcopy(original)
        reordered["binding_replay"]["records"].reverse()
        reordered["work_digests"]["digests"].reverse()
        reordered["ledger_snapshots"]["snapshots"].reverse()
        reordered["vouch_ledger"]["memos"].reverse()
        reordered["cowork_pointers"]["pointers"].reverse()
        reordered["funding_transfers"]["wallet_accounts"].reverse()
        reordered["funding_transfers"]["transfers"].reverse()
        reordered["funding_exclusions"]["addresses"].reverse()
        reordered["policy_evidence"]["active_validators"].reverse()
        reordered["policy_evidence"]["candidates"].reverse()
        reordered["policy_evidence"][
            "foundation_bound_validator_ids"
        ].reverse()
        reordered["baseline_list"]["validator_ids"].reverse()
        reordered["registry_history"]["rounds"].reverse()
        for view in reordered["registry_history"]["rounds"]:
            view["validator_ids"].reverse()

        self.assertEqual(
            canonical_json_bytes(derive_shadow_report(original)),
            canonical_json_bytes(derive_shadow_report(reordered)),
        )

    def test_cli_writes_only_the_requested_shadow_reports(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "shadow.json"
            markdown = Path(directory) / "shadow.md"

            return_code = main(
                [
                    "shadow-derive",
                    "--input-dir",
                    str(FIXTURE_DIR),
                    "--output",
                    str(output),
                    "--markdown-output",
                    str(markdown),
                ]
            )

            self.assertEqual(return_code, 0)
            self.assertEqual(output.read_bytes(), EXPECTED_JSON.read_bytes())
            self.assertEqual(
                markdown.read_text(encoding="utf-8"),
                EXPECTED_MARKDOWN.read_text(encoding="utf-8"),
            )

    def test_markdown_names_every_fixture_difference_and_hold(self) -> None:
        report = derive_shadow_report(_documents())
        rendered = render_shadow_markdown(report)

        self.assertEqual(rendered, EXPECTED_MARKDOWN.read_text())
        self.assertIn(
            "- Add `validator-22` — eligible_admission_candidate; "
            "all_gates_passed; selected_by_canonical_order; "
            "churn_guard_allow.",
            rendered,
        )
        for validator_id in ("validator-23", "validator-25", "validator-26"):
            with self.subTest(validator_id=validator_id):
                self.assertIn(f"`{validator_id}`", rendered)
        self.assertIn("accountability_below_floor", rendered)

    def test_runner_modules_have_no_network_or_live_action_clients(self) -> None:
        module_root = Path(__file__).resolve().parents[1] / "postfiat_rpc"
        source = "\n".join(
            (module_root / name).read_text(encoding="utf-8")
            for name in ("tasknode_unl.py", "tasknode_unl_policy.py")
        )
        for forbidden in (
            "import requests",
            "import urllib",
            "import socket",
            "import http.client",
            "import aiohttp",
            "import subprocess",
            "tasknode.postfiat.org",
            "submit_transaction",
            "ratify",
        ):
            with self.subTest(forbidden=forbidden):
                self.assertNotIn(forbidden, source)


if __name__ == "__main__":
    unittest.main()
