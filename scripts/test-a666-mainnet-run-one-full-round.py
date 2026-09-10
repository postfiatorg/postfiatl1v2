#!/usr/bin/env python3
"""Regression tests for the live A666 round-trip execution interlock."""

from __future__ import annotations

import subprocess
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/a666-mainnet-run-one-full-round.sh"
BASE = [
    "bash",
    str(SCRIPT),
    "--campaign-dir",
    "/tmp/postfiat-a666-interlock-test",
    "--run-label",
    "qa-interlock",
    "--workflow-id",
    "qa-interlock",
]


class A666MainnetRoundInterlockTests(unittest.TestCase):
    def run_script(self, *extra: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [*BASE, *extra],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

    def test_execute_flag_is_required_before_environment_or_network_access(self) -> None:
        result = self.run_script()
        self.assertEqual(result.returncode, 2)
        self.assertIn("without --execute", result.stderr)
        self.assertNotIn("A666_PFTL_RELEASE_ID", result.stderr)

    def test_typed_confirmation_is_required_after_execute_flag(self) -> None:
        result = self.run_script("--execute")
        self.assertEqual(result.returncode, 2)
        self.assertIn("without --confirm 'RUN A666 MAINNET ROUND'", result.stderr)
        self.assertNotIn("A666_PFTL_RELEASE_ID", result.stderr)


if __name__ == "__main__":
    unittest.main()
