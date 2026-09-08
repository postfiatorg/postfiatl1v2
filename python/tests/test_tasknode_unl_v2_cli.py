"""Actual CLI and human-report tests for UNL V2 milestone sections D/E."""

from __future__ import annotations

import copy
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from postfiat_rpc.tasknode_unl_schema import TaskNodeUnlError, canonical_json_bytes
from postfiat_rpc.tasknode_unl_v2 import (
    derive_v2_cli_report,
    render_v2_markdown,
    validate_v2_cli_report,
)
from postfiat_rpc.tasknode_unl_v2_schema import MAX_RECORDS

REPO_ROOT = Path(__file__).resolve().parents[2]
FIXTURE_DIR = Path(__file__).resolve().parent / "fixtures" / "tasknode_unl_v2"
EVIDENCE_PATH = FIXTURE_DIR / "evidence-golden.json"
ADMISSION_PATH = FIXTURE_DIR / "cli-admission-input.json"
MARKDOWN_PATH = FIXTURE_DIR / "cli-report.md"


def _load(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def _cli(*args: object) -> subprocess.CompletedProcess[bytes]:
    env = dict(os.environ)
    python_path = str(REPO_ROOT / "python")
    env["PYTHONPATH"] = (
        python_path
        if not env.get("PYTHONPATH")
        else f"{python_path}{os.pathsep}{env['PYTHONPATH']}"
    )
    return subprocess.run(
        [sys.executable, "-m", "postfiat_rpc.tasknode_unl_v2", *(str(arg) for arg in args)],
        cwd=REPO_ROOT,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )


class TestV2CliDerivation(unittest.TestCase):
    def setUp(self) -> None:
        self.evidence = _load(EVIDENCE_PATH)
        self.admission = _load(ADMISSION_PATH)

    def test_same_section_a_fixture_derives_all_required_visible_states(self) -> None:
        report = derive_v2_cli_report(self.evidence, self.admission)
        self.assertEqual(report["version"], 2)
        self.assertEqual(report["mode"], "SHADOW_ONLY")
        self.assertEqual(
            report["overall_status"],
            "SHADOW_ONLY_WITH_UNRESOLVED_IDENTITY_LIMITS",
        )
        self.assertEqual(report["evidence_status"], "verified")
        self.assertEqual(report["frozen_status"], "FROZEN")
        self.assertEqual(len(report["continuity_holds"]), 1)
        self.assertEqual(len(report["admission_denials"]), 2)
        self.assertEqual(len(report["existing_breaches"]), 1)
        self.assertEqual(
            report["existing_breaches"][0]["state"], "EXISTING_BREACH"
        )
        self.assertIn(
            "unchanged-key control transfer is undetectable",
            " ".join(report["known_control_limits"]),
        )
        self.assertFalse(report["authority_boundary"]["submission_supported"])
        self.assertFalse(report["authority_boundary"]["live_mutation_supported"])
        self.assertFalse(report["authority_boundary"]["promotion_authorized"])
        self.assertFalse(report["authority_boundary"]["v1_inputs_reinterpreted"])
        validate_v2_cli_report(report)

    def test_two_derivations_are_byte_identical(self) -> None:
        first = canonical_json_bytes(
            derive_v2_cli_report(self.evidence, self.admission)
        )
        second = canonical_json_bytes(
            derive_v2_cli_report(
                copy.deepcopy(self.evidence), copy.deepcopy(self.admission)
            )
        )
        self.assertEqual(first, second)
        report = json.loads(first)
        self.assertEqual(
            report["report_root"],
            "b7742613b99021988205e3b8a3f2d127b90d33f1b0ed1b495cdd0d790ea198fc",
        )

    def test_bad_snapshot_commitment_fails_closed_to_no_proposal(self) -> None:
        evidence = copy.deepcopy(self.evidence)
        evidence["snapshot"]["commitments"]["input_root"] = "00" * 32
        report = derive_v2_cli_report(evidence, self.admission)
        self.assertEqual(report["evidence_status"], "hold")
        self.assertEqual(report["frozen_status"], "NO_PROPOSAL")
        self.assertTrue(report["admission_denials"])
        self.assertTrue(
            all(item["action"] == "NO_PROPOSAL" for item in report["admission_denials"])
        )

    def test_malformed_field_and_v1_commitment_name_the_failure(self) -> None:
        admission = copy.deepcopy(self.admission)
        admission["unexpected"] = True
        with self.assertRaisesRegex(TaskNodeUnlError, r"admission\.unexpected"):
            derive_v2_cli_report(self.evidence, admission)

        admission = copy.deepcopy(self.admission)
        admission["v1_reference"]["reference_root"] = "00" * 32
        with self.assertRaisesRegex(
            TaskNodeUnlError, r"admission\.v1_reference\.reference_root"
        ):
            derive_v2_cli_report(self.evidence, admission)

    def test_bounded_arrays_reject_before_policy_derivation(self) -> None:
        admission = copy.deepcopy(self.admission)
        admission["nodes"] = [f"account-{index}" for index in range(MAX_RECORDS + 1)]
        with self.assertRaisesRegex(TaskNodeUnlError, "array_too_large"):
            derive_v2_cli_report(self.evidence, admission)


class TestV2ActualCli(unittest.TestCase):
    def test_derive_and_render_are_deterministic_and_fixture_exact(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            first_json = directory / "first.json"
            second_json = directory / "second.json"
            first_md = directory / "first.md"
            second_md = directory / "second.md"
            base = (
                "derive",
                "--policy-version",
                "v2",
                "--evidence-bundle",
                EVIDENCE_PATH,
                "--admission-input",
                ADMISSION_PATH,
            )
            first = _cli(*base, "--output", first_json, "--markdown-output", first_md)
            second = _cli(*base, "--output", second_json, "--markdown-output", second_md)
            self.assertEqual(first.returncode, 0, first.stderr.decode())
            self.assertEqual(second.returncode, 0, second.stderr.decode())
            self.assertEqual(first_json.read_bytes(), second_json.read_bytes())
            self.assertEqual(first_md.read_bytes(), second_md.read_bytes())
            self.assertEqual(first_md.read_bytes(), MARKDOWN_PATH.read_bytes())

            rendered = directory / "rendered.md"
            result = _cli(
                "render",
                "--policy-version",
                "v2",
                "--input",
                first_json,
                "--output",
                rendered,
            )
            self.assertEqual(result.returncode, 0, result.stderr.decode())
            self.assertEqual(rendered.read_bytes(), first_md.read_bytes())

    def test_v2_selection_is_explicit_and_unknown_versions_fail_closed(self) -> None:
        result = _cli(
            "derive",
            "--policy-version",
            "v3",
            "--evidence-bundle",
            EVIDENCE_PATH,
            "--admission-input",
            ADMISSION_PATH,
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn(b"unknown_version: policy_version", result.stderr)

        missing = _cli(
            "derive",
            "--evidence-bundle",
            EVIDENCE_PATH,
            "--admission-input",
            ADMISSION_PATH,
        )
        self.assertEqual(missing.returncode, 2)
        self.assertIn(b"--policy-version", missing.stderr)

    def test_malformed_input_reports_the_exact_field(self) -> None:
        admission = _load(ADMISSION_PATH)
        admission["registry_state"]["round_index"] = True
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "bad.json"
            path.write_text(json.dumps(admission), encoding="utf-8")
            result = _cli(
                "derive",
                "--policy-version",
                "v2",
                "--evidence-bundle",
                EVIDENCE_PATH,
                "--admission-input",
                path,
            )
        self.assertEqual(result.returncode, 2)
        self.assertIn(b"admission.registry_state.round_index", result.stderr)

    def test_render_rejects_tampered_cli_report(self) -> None:
        report = derive_v2_cli_report(_load(EVIDENCE_PATH), _load(ADMISSION_PATH))
        report["overall_status"] = "ALL_GREEN"
        with self.assertRaisesRegex(TaskNodeUnlError, r"report\.report_root"):
            render_v2_markdown(report)

    def test_cli_has_only_read_only_derive_and_render_commands(self) -> None:
        help_result = _cli("--help")
        self.assertEqual(help_result.returncode, 0)
        self.assertIn(b"{derive,render}", help_result.stdout)
        self.assertEqual(_cli("submit").returncode, 2)
        self.assertEqual(_cli("ratify").returncode, 2)
        self.assertEqual(_cli("fund").returncode, 2)


if __name__ == "__main__":
    unittest.main()
