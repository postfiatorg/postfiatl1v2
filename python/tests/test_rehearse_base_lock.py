"""The base lock rehearsal's success predicates (scripts/rehearse-base-lock.py)."""

from __future__ import annotations

import importlib.util
import sys
import tempfile
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


    def test_readiness_counts_only_while_the_child_runs(self) -> None:
        children: list = []
        record: list = []

        def argv(port: int) -> list[str]:
            body = "raise SystemExit(1)" if not children else "import time; time.sleep(30)"
            return [sys.executable, "-c", body]

        def ready(port: int) -> bool:
            # The first child reads as ready only once it has exited.
            if len(children) == 1:
                children[0].wait()
            return True

        try:
            _, child = rehearsal.start_on_free_port("probe", argv, ready, children, record)
            self.assertIsNone(child.poll())
        finally:
            for started in children:
                started.kill()
                started.wait()
        self.assertEqual([entry["attempt"] for entry in record], [1, 2])

    def test_a_ready_file_left_by_an_exited_attempt_is_not_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            ready_file = Path(directory) / "ready.json"
            ready_file.write_text("{}")  # left behind by an earlier attempt
            children: list = []
            record: list = []
            write_and_wait = f"open({str(ready_file)!r}, 'w').close(); import time; time.sleep(30)"

            def argv(port: int) -> list[str]:
                body = "raise SystemExit(1)" if not children else write_and_wait
                return [sys.executable, "-c", body]

            try:
                _, child = rehearsal.start_on_free_port(
                    "probe", argv, lambda port: ready_file.exists(), children, record, ready_file=ready_file
                )
                self.assertIsNone(child.poll())
            finally:
                for started in children:
                    started.kill()
                    started.wait()
            self.assertEqual([entry["attempt"] for entry in record], [1, 2])


class PathConfinementTests(unittest.TestCase):
    def test_output_and_target_are_confined(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            base = Path(root)
            target = base / "target"
            (target / "debug").mkdir(parents=True)
            out, resolved = rehearsal.confined_paths(base / "run", target)
            self.assertEqual((out, resolved), ((base / "run").resolve(), target.resolve()))
            for output, target_dir, message in [
                (target, target, "same path"),
                (target / "run", target, "contain one another"),
                (base / "run", base / "run" / "target", "contain one another"),
                (base, base / "elsewhere", "contain one another"),
            ]:
                with self.assertRaisesRegex(rehearsal.RehearsalFailure, message):
                    rehearsal.confined_paths(output, target_dir)
            unrelated = base / "unrelated"
            unrelated.mkdir()
            (unrelated / "notes.txt").write_text("not a cargo target")
            with self.assertRaisesRegex(rehearsal.RehearsalFailure, "not a cargo target"):
                rehearsal.confined_paths(base / "run", unrelated)
            (base / "existing").mkdir()
            with self.assertRaisesRegex(rehearsal.RehearsalFailure, "already exists"):
                rehearsal.confined_paths(base / "existing", target)
            rehearsal.confined_paths(base / "run", base / "fresh-target")


if __name__ == "__main__":
    unittest.main()
