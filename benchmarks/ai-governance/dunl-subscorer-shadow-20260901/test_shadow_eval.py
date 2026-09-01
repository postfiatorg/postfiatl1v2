"""Regression tests for the shadow evaluation's fail-loud guards.

These tests need the read-only ``dynamic-unl-scoring`` fork clone (for the
imports at the top of ``shadow_eval``) and the fetched ``rounds/`` data
(gitignored); they skip cleanly when either is unavailable. Run from this
directory: ``python3 -m unittest test_shadow_eval`` (venv with the fork's
parser deps).

Guards under test, each of which previously produced a plausible but wrong
number instead of failing:

- cutoff flips were computed against the hardcoded 40 even when the round
  pinned a different ``score_cutoff``;
- the imported fork modules were never verified against the round's
  content-hash pins, so a drifted fork clone silently changed the numbers
  (the UNL-reproduction control does not catch formula drift);
- a validator present in the frozen entries but missing from the published
  model scores (or vice versa) was silently dropped from every metric, as
  was any duplicate-key collapse.

The final test pins the round-16 report to the committed ``results.json``
and ``results-v2.json`` entries, proving the guards changed no numbers.
"""

from __future__ import annotations

import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROUND_16 = HERE / "rounds" / "testnet-r16"

sys.path.insert(0, str(HERE))

try:
    import shadow_eval
    import subscorer
    _IMPORT_ERROR: Exception | None = None
except ImportError as exc:  # fork clone or its deps unavailable
    shadow_eval = None
    _IMPORT_ERROR = exc


@unittest.skipIf(shadow_eval is None, f"fork imports unavailable: {_IMPORT_ERROR}")
@unittest.skipUnless(ROUND_16.exists(), "frozen rounds not fetched (rounds/ is gitignored)")
class ShadowEvalGuardTest(unittest.TestCase):
    def _round_copy(self) -> Path:
        tmp = Path(tempfile.mkdtemp(prefix="shadow-eval-test-"))
        self.addCleanup(shutil.rmtree, tmp, ignore_errors=True)
        dst = tmp / "testnet-r16"
        shutil.copytree(ROUND_16, dst)
        return dst

    @staticmethod
    def _edit_manifest(round_dir: Path, edit) -> None:
        path = round_dir / "runtime" / "execution_manifest.json"
        manifest = json.loads(path.read_text())
        edit(manifest)
        path.write_text(json.dumps(manifest))

    def test_cutoff_pin_mismatch_fails_loudly(self) -> None:
        round_dir = self._round_copy()
        self._edit_manifest(
            round_dir,
            lambda m: m["code"]["selector"]["parameters"].__setitem__(
                "score_cutoff", 60
            ),
        )
        with self.assertRaisesRegex(ValueError, "score_cutoff 60"):
            shadow_eval.evaluate_round(round_dir)

    def test_fork_module_drift_fails_loudly(self) -> None:
        round_dir = self._round_copy()
        self._edit_manifest(
            round_dir,
            lambda m: m["code"]["score_formula"].__setitem__(
                "content_sha256", "0" * 64
            ),
        )
        with self.assertRaisesRegex(ValueError, "score_formula"):
            shadow_eval.evaluate_round(round_dir)

    def test_missing_model_score_fails_loudly(self) -> None:
        round_dir = self._round_copy()
        path = round_dir / "outputs" / "validator_scores.json"
        data = json.loads(path.read_text())
        del data["validator_scores"][7]
        path.write_text(json.dumps(data))
        with self.assertRaisesRegex(ValueError, "validator sets differ"):
            shadow_eval.evaluate_round(round_dir)

    def test_duplicate_validator_id_fails_loudly(self) -> None:
        round_dir = self._round_copy()
        path = round_dir / "inputs" / "model_request.json"
        request = json.loads(path.read_text())
        content = request["messages"][-1]["content"]
        entries = subscorer.extract_validator_entries(request)
        index = content.find(subscorer.VALIDATOR_DATA_MARKER)
        request["messages"][-1]["content"] = (
            content[:index]
            + subscorer.VALIDATOR_DATA_MARKER
            + "\n"
            + json.dumps(entries + [entries[0]])
        )
        path.write_text(json.dumps(request))
        with self.assertRaisesRegex(ValueError, "duplicate validator_id"):
            shadow_eval.evaluate_round(round_dir)

    def test_round_16_reproduces_committed_results(self) -> None:
        for scorer_version, results_name in ((1, "results.json"), (2, "results-v2.json")):
            committed = {
                r["round"]: r
                for r in json.loads((HERE / results_name).read_text())["rounds"]
            }["testnet-r16"]
            report = shadow_eval.evaluate_round(ROUND_16, scorer_version)
            self.assertEqual(report, committed)


if __name__ == "__main__":
    unittest.main()
