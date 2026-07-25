from __future__ import annotations

from pathlib import Path
import tempfile
import time
import unittest

from tools.lightning_navcoin_demo.coordinator.journal import ExposureLimits
from tools.lightning_navcoin_demo.coordinator.tests.common import envelope_for
from tools.lightning_navcoin_demo.crash_matrix import run_crash_matrix


class CrashMatrixTests(unittest.TestCase):
    def test_unclean_exit_at_every_transition_recovers(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            now_unix = int(time.time())
            report = run_crash_matrix(
                Path(directory) / "matrix",
                happy_signed_quote=envelope_for(201, now_unix=now_unix),
                refund_signed_quote=envelope_for(202, now_unix=now_unix),
                limits=ExposureLimits(2_000_000, 4_000_000),
            )
        self.assertEqual(report["result"], "PASS")
        self.assertEqual(report["transition_crash_count"], 12)
        self.assertTrue(report["happy"]["all_unclean_restarts_recovered"])
        self.assertTrue(report["refund"]["all_unclean_restarts_recovered"])


if __name__ == "__main__":
    unittest.main()
