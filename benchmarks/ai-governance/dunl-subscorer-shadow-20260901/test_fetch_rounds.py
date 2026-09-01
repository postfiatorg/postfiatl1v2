"""Regression tests for the fetcher's manifest handling and exit codes.

No network and no fork clone: the replay harness is a stub, and the
manifest/rounds paths are pointed at a temporary directory. Run from this
directory: ``python3 -m unittest test_fetch_rounds``.

Defects under test:

- a partial re-run (``--rounds 12``) rewrote ``rounds-manifest.json`` with
  only the fetched rounds, destroying the committed pins of every other
  round; the manifest now merges with its previous content;
- a failed fetch of ``outputs/validator_scores.json`` (required by the
  evaluation) was recorded in the manifest but still exited 0.
"""

from __future__ import annotations

import contextlib
import io
import json
import shutil
import sys
import tempfile
import types
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import fetch_rounds  # noqa: E402


def _stub_harness(payload, status: int = 0) -> types.SimpleNamespace:
    return types.SimpleNamespace(
        SERVICE_URLS={"testnet": "https://scoring-testnet.postfiat.org"},
        fetch_round=lambda network, round_number, out_dir: status,
        _fetch_json=lambda url: payload,
    )


class FetchRoundsManifestTest(unittest.TestCase):
    def setUp(self) -> None:
        tmp = Path(tempfile.mkdtemp(prefix="fetch-rounds-test-"))
        self.addCleanup(shutil.rmtree, tmp, ignore_errors=True)
        self._saved = (
            fetch_rounds.MANIFEST_PATH,
            fetch_rounds.ROUNDS_DIR,
            fetch_rounds._load_harness,
        )
        self.addCleanup(self._restore)
        fetch_rounds.MANIFEST_PATH = tmp / "rounds-manifest.json"
        fetch_rounds.ROUNDS_DIR = tmp / "rounds"

    def _restore(self) -> None:
        (
            fetch_rounds.MANIFEST_PATH,
            fetch_rounds.ROUNDS_DIR,
            fetch_rounds._load_harness,
        ) = self._saved

    @staticmethod
    def _fetch(rounds: list[int], payload, status: int = 0) -> int:
        fetch_rounds._load_harness = lambda: _stub_harness(payload, status)
        with contextlib.redirect_stdout(io.StringIO()):
            return fetch_rounds.fetch(rounds)

    def _manifest(self) -> dict:
        return json.loads(fetch_rounds.MANIFEST_PATH.read_text())

    def test_partial_rerun_preserves_unfetched_round_pins(self) -> None:
        fetch_rounds.MANIFEST_PATH.write_text(
            json.dumps(
                {
                    "network": "testnet",
                    "service": "https://scoring-testnet.postfiat.org",
                    "harness": "stub",
                    "rounds": {
                        "12": {"old.json": {"sha256": "aa", "bytes": 1}},
                        "13": {"kept.json": {"sha256": "bb", "bytes": 2}},
                    },
                    "failures": [
                        {"round": 12, "file": "stale.json", "reason": "old"},
                        {"round": 13, "file": "kept.json", "reason": "kept"},
                    ],
                }
            )
        )
        code = self._fetch([12], payload={"validator_scores": []})
        self.assertEqual(code, 0)
        manifest = self._manifest()
        # Round 13's pins and failure survive; round 12's stale ones do not.
        self.assertEqual(
            manifest["rounds"]["13"], {"kept.json": {"sha256": "bb", "bytes": 2}}
        )
        self.assertIn("outputs/validator_scores.json", manifest["rounds"]["12"])
        self.assertNotIn("old.json", manifest["rounds"]["12"])
        self.assertEqual(
            manifest["failures"],
            [{"round": 13, "file": "kept.json", "reason": "kept"}],
        )

    def test_missing_required_artifact_fails_the_run(self) -> None:
        code = self._fetch([12], payload=None)
        self.assertEqual(code, 1)
        self.assertEqual(
            self._manifest()["failures"],
            [
                {
                    "round": 12,
                    "file": "outputs/validator_scores.json",
                    "reason": "fetch failed",
                }
            ],
        )

    def test_manifest_round_order_is_numeric_not_fetch_order(self) -> None:
        code = self._fetch([13, 9, 12], payload={"validator_scores": []})
        self.assertEqual(code, 0)
        self.assertEqual(list(self._manifest()["rounds"]), ["9", "12", "13"])


if __name__ == "__main__":
    unittest.main()
