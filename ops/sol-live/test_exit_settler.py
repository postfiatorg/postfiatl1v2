"""The installed settlement loop (exit_settler.py): a lock, a bound, a backoff with one alert when a
burn starts failing and one when it gives up, every failure in the journal, proposals only."""

import contextlib
import fcntl
import io
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import exit_settler as s

BURN, OTHER = "b1" * 48, "c2" * 48


class Pass(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.state = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def once(self, now, burns=(BURN,), ok=False, find=None):
        calls, out = [], io.StringIO()

        def propose(burn):
            calls.append(burn)
            return ok, "Validators disagree on the burn."

        with contextlib.redirect_stdout(out):
            code = s.run_pass(now, find or (lambda: list(burns)), propose, self.state)
        return code, calls, out.getvalue()

    def test_a_failing_burn_backs_off_with_one_alert_at_the_start_and_one_at_the_end(self):
        now, alerts, attempts = 0, [], []
        while now < 10 * 86400:
            code, calls, out = self.once(now)
            attempts += [now] * len(calls)
            alerts += [line for line in out.splitlines() if "ALERT" in line]
            now += 90
        self.assertEqual(len(attempts), s.GIVE_UP)
        gaps = [b - a for a, b in zip(attempts, attempts[1:])]
        self.assertEqual(gaps[:4], [90, 180, 360, 720])
        self.assertTrue(all(gap <= s.MAX_RETRY + 90 for gap in gaps))
        self.assertEqual(len(alerts), 2)
        self.assertIn("failing", alerts[0])
        self.assertIn("gave up after 12 attempts", alerts[1])

    def test_a_settled_burn_leaves_the_backoff_and_others_go_on(self):
        self.once(0, burns=(BURN, OTHER))
        code, calls, out = self.once(90, burns=(BURN, OTHER), ok=True)
        self.assertEqual((code, calls), (0, [BURN, OTHER]))
        self.assertEqual((self.state / "backoff.json").read_text(), "{}")

    def test_a_discovery_failure_is_an_alert_and_proposes_nothing(self):
        def broken():
            raise RuntimeError("no validator answered vault_bridge_status")
        code, calls, out = self.once(0, find=broken)
        self.assertEqual((code, calls), (1, []))
        self.assertIn("ALERT discovery failed: RuntimeError: no validator answered", out)

    def test_two_passes_never_run_at_once(self):
        with open(self.state / "lock", "a") as held:
            fcntl.flock(held, fcntl.LOCK_EX)
            code, calls, out = self.once(0)
        self.assertEqual((code, calls), (0, []))
        self.assertIn("holds the lock", out)

    def test_each_settlement_is_the_reviewed_restricted_path_and_bounded(self):
        with patch.object(s, "STATE", self.state), patch.object(s.subprocess, "run") as run:
            run.return_value.returncode, run.return_value.stdout, run.return_value.stderr = 0, "signed by 5 of 6\n", ""
            self.assertEqual(s.settle(BURN), (True, "signed by 5 of 6"))
            command = run.call_args.args[0]
            self.assertEqual(command[1:5], [str(s.HERE / "exit_govern.py"), "paid", "--burn", BURN])
            self.assertIn("--restricted", command)
            self.assertEqual(run.call_args.kwargs["timeout"], s.ATTEMPT_SECONDS)
            run.side_effect = s.subprocess.TimeoutExpired(command, 1)
            self.assertEqual(s.settle(BURN), (False, f"stopped after {s.ATTEMPT_SECONDS} s"))


if __name__ == "__main__":
    unittest.main()
