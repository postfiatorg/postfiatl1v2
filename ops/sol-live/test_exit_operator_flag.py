"""The go-live settle.py writes review files on other hosts only with an explicit
single-owner flag; the per-host reviewer replaces it."""

import os
import subprocess
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent


class OperatorFlagTest(unittest.TestCase):
    def test_settle_refuses_remote_review_writes_without_the_flag(self):
        env = dict(os.environ, PYTHONPATH=f"{HERE}:{os.environ.get('PYTHONPATH', '')}")
        done = subprocess.run([sys.executable, str(HERE / "operator/settle.py"), "rev", "ab" * 48],
                              capture_output=True, text=True, env=env, timeout=60)
        self.assertNotEqual(done.returncode, 0)
        self.assertIn("--single-owner-fleet", done.stderr)
        # It says what to run instead, with the arguments it was given.
        self.assertIn(f"settle.py rev {'ab' * 48} --single-owner-fleet", done.stderr)

    def test_the_settler_reports_a_failed_settle_instead_of_looping_silently(self):
        text = (HERE / "operator/settler.sh").read_text()
        self.assertIn("PIPESTATUS", text)
        self.assertIn("ALERT settle.py failed", text)

    def test_go_live_tools_name_their_replacement(self):
        for name in ("settle.py", "clearance.py"):
            head = (HERE / "operator" / name).read_text()[:600]
            self.assertIn("exit_review.py", head, name)


if __name__ == "__main__":
    unittest.main()
