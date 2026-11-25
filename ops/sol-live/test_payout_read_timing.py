"""operator/payout_read_timing.py picks its burn and checks it as the payout service does (F12):
validate_burn with the buckets the chain covers."""

import importlib.util
import json
import unittest
from pathlib import Path

HERE = Path(__file__).parent
FIX = json.loads((HERE / "fixtures/r2-withdrawals.json").read_text())


def load():
    spec = importlib.util.spec_from_file_location("payout_read_timing", HERE / "operator/payout_read_timing.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)  # reads nothing at import
    return module


class PickTest(unittest.TestCase):
    def test_a_recorded_burn_is_picked_and_checked_against_the_covered_buckets(self):
        import exit_rules
        timing = load()
        rules = exit_rules.of_route(FIX["route"])
        burn = next(b for b, v in FIX["solana"].items() if v["height"] == 1896)
        red, owner = timing.pick(FIX["status"], burn[:12], rules)
        self.assertEqual(red["burn_tx_id"], burn)
        self.assertEqual(owner.__class__.__name__, "Pubkey")
        uncovered = dict(rules, covered=tuple(b for b in rules["covered"] if b != red["bucket_id"]))
        with self.assertRaisesRegex(ValueError, "outside the supported"):
            timing.pick(FIX["status"], burn[:12], uncovered)


if __name__ == "__main__":
    unittest.main()
