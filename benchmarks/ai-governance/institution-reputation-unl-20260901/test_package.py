from __future__ import annotations

import json
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parent


class FrozenPackageTests(unittest.TestCase):
    def test_current_lists_and_request_accounting_are_frozen(self) -> None:
        manifest = json.loads((ROOT / "manifest.json").read_text())
        self.assertEqual(
            manifest["counts"],
            {"xrpl": 35, "postfiat": 20, "scoring": 55, "padding": 9, "slots": 64},
        )
        self.assertEqual(manifest["source_summary"]["xrpl"]["intersection_count"], 35)
        self.assertEqual(manifest["source_summary"]["postfiat"]["round_number"], 20)
        self.assertFalse(manifest["openrouter_used"])

    def test_prompt_defines_every_five_point_band_and_zero_rule(self) -> None:
        prompt = (ROOT / "inputs" / "prompt.txt").read_text()
        for lower in range(0, 100, 5):
            self.assertIn(f"B{lower:02d}", prompt)
        self.assertIn("score=0", prompt)
        self.assertIn("sanctions", prompt.lower())
        self.assertIn("prestige", prompt.lower())
        self.assertIn("Layer-1", prompt)

    def test_requests_use_only_the_pinned_local_model_contract(self) -> None:
        requests = json.loads((ROOT / "inputs" / "requests.json").read_text())
        self.assertEqual(len(requests), 64)
        for row in requests:
            body = row["body"]
            self.assertEqual(body["model"], "Qwen/Qwen3.8-27B-FP8")
            self.assertNotIn("url", body)
            self.assertNotIn("api_key", body)


if __name__ == "__main__":
    unittest.main()
