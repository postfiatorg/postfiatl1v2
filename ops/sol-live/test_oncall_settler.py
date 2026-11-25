"""oncall/settler.sh shows every failure and bounds each settlement (stubbed python3)."""
import os
import subprocess
import tempfile
import time
import unittest
from pathlib import Path

SETTLER = Path(__file__).resolve().parent / "oncall/settler.sh"
BURN = "b1" * 48
STUB = """#!/bin/bash
if [[ $1 == - ]]; then
  cat > /dev/null
  [[ -n $STUB_DISCOVERY_ERROR ]] && { echo "Traceback (most recent call last):" >&2; echo "$STUB_DISCOVERY_ERROR" >&2; exit 1; }
  echo "$STUB_BURNS"; exit 0
fi
echo "verified payout $3"
sleep "${STUB_SETTLE_SECONDS:-0}"
exit "${STUB_SETTLE_EXIT:-0}"
"""


class SettlerTest(unittest.TestCase):
    def run_once(self, **env):
        with tempfile.TemporaryDirectory() as tmp:
            stub = Path(tmp) / "bin"
            stub.mkdir()
            (stub / "python3").write_text(STUB)
            (stub / "python3").chmod(0o755)
            env = dict(os.environ, PATH=f"{stub}:{os.environ['PATH']}", SETTLER_DIR=tmp, SETTLER_ONCE="1", **env)
            start = time.monotonic()
            out = subprocess.run(["bash", SETTLER], env=env, capture_output=True, text=True, timeout=20)
            return out.stdout + out.stderr, time.monotonic() - start

    def test_a_discovery_failure_is_reported(self):
        out, _ = self.run_once(STUB_DISCOVERY_ERROR="RuntimeError: no validator answered vault_bridge_status")
        self.assertIn("discovery failed: RuntimeError: no validator answered vault_bridge_status", out)

    def test_each_settlement_reports_its_exit_status(self):
        out, _ = self.run_once(STUB_BURNS=BURN, STUB_SETTLE_EXIT="3")
        self.assertIn(f"settling {BURN}", out)
        self.assertIn(f"settle {BURN} exit 3", out)

    def test_a_hung_settlement_is_stopped_at_its_bound(self):
        out, seconds = self.run_once(STUB_BURNS=BURN, STUB_SETTLE_SECONDS="30", SETTLER_ATTEMPT_SECONDS="1")
        self.assertIn(f"settle {BURN} exit 124", out)
        self.assertLess(seconds, 10)


if __name__ == "__main__":
    unittest.main()
