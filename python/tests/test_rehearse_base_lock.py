"""The base lock rehearsal's success predicates (scripts/rehearse-base-lock.py)."""

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "rehearse-base-lock.py"
SPEC = importlib.util.spec_from_file_location("rehearse_base_lock", SCRIPT)
rehearsal = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(rehearsal)

ALARM_CODES = {"release_without_lock": 0, "release_amount_mismatch": 0}


def summary(**fields):
    line = {"kind": "summary", "release_count": 1, "releases_checked": 1, "alarms": 0, "alarm_codes": dict(ALARM_CODES)}
    line.update(fields)
    return line


BACKED = {"kind": "release", "status": "backed", "amount_atoms": 4331}


class WatcherSummaryTests(unittest.TestCase):
    def test_one_backed_release_and_no_alarm_passes(self) -> None:
        self.assertEqual(rehearsal.watcher_summary([BACKED, summary()], releases_checked=1)["release_count"], 1)

    def test_the_repeat_checks_nothing_again(self) -> None:
        rehearsal.watcher_summary([summary(releases_checked=0)], releases_checked=0)

    def test_a_zero_release_run_fails(self) -> None:
        canned = [summary(release_count=0, releases_checked=0)]
        with self.assertRaisesRegex(rehearsal.RehearsalFailure, "one release"):
            rehearsal.watcher_summary(canned, releases_checked=1)

    def test_a_release_line_without_a_summary_fails(self) -> None:
        with self.assertRaisesRegex(rehearsal.RehearsalFailure, "one summary line"):
            rehearsal.watcher_summary([BACKED], releases_checked=1)

    def test_an_alarm_or_an_unbacked_release_fails(self) -> None:
        with self.assertRaisesRegex(rehearsal.RehearsalFailure, "no alarm"):
            rehearsal.watcher_summary([BACKED, summary(alarms=1)], releases_checked=1)
        alarmed = summary(alarm_codes={"release_without_lock": 1})
        with self.assertRaisesRegex(rehearsal.RehearsalFailure, "alarm codes"):
            rehearsal.watcher_summary([BACKED, alarmed], releases_checked=1)
        with self.assertRaisesRegex(rehearsal.RehearsalFailure, "backed"):
            rehearsal.watcher_summary([dict(BACKED, status="unbacked"), summary()], releases_checked=1)

    def test_a_repeat_that_rechecks_fails(self) -> None:
        with self.assertRaisesRegex(rehearsal.RehearsalFailure, "check 0"):
            rehearsal.watcher_summary([BACKED, summary()], releases_checked=0)

    def test_require_survives_python_optimize(self) -> None:
        with self.assertRaises(rehearsal.RehearsalFailure):
            rehearsal.require(False, "must raise")


class PortRetryTests(unittest.TestCase):
    def test_a_child_that_exits_before_ready_is_started_again_on_a_new_port(self) -> None:
        calls: list[int] = []

        def argv(port: int) -> list[str]:
            calls.append(port)
            body = "raise SystemExit(1)" if len(calls) == 1 else "import time; time.sleep(30)"
            return [sys.executable, "-c", body]

        children: list = []
        record: list = []
        try:
            rehearsal.start_on_free_port("probe", argv, lambda port: len(calls) == 2, children, record)
        finally:
            for child in children:
                child.kill()
                child.wait()
        self.assertEqual([entry["attempt"] for entry in record], [1, 2])

    def test_the_retries_are_bounded(self) -> None:
        children: list = []
        with self.assertRaisesRegex(rehearsal.RehearsalFailure, "exited before ready"):
            rehearsal.start_on_free_port(
                "dead", lambda port: [sys.executable, "-c", "raise SystemExit(1)"], lambda port: False, children, []
            )
        self.assertEqual(len(children), rehearsal.PORT_ATTEMPTS)


if __name__ == "__main__":
    unittest.main()
