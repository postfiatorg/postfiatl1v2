#!/usr/bin/env python3

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import unittest


SCRIPT_DIR = Path(__file__).resolve().parent


def load(name: str, file: str):
    spec = importlib.util.spec_from_file_location(name, SCRIPT_DIR / file)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


successor = load("pnok_fix_successor", "pnok-fix-successor.py")
demo = successor.load_demo_module(SCRIPT_DIR)


class PnokFixSuccessorTests(unittest.TestCase):
    def test_packet_hash_matches_the_finalized_epoch_one_vector(self) -> None:
        packet = json.loads(
            (
                SCRIPT_DIR.parent
                / "deployments/pnok-private-fix-20260801/fix-market/public/fix-packet.json"
            ).read_text()
        )
        self.assertEqual(successor.packet_hash(packet, demo), packet["packet_hash"])

    def test_successor_is_exact_per_fill_and_aggregate_fill_bounded(self) -> None:
        prior = {
            "epoch": 1,
            "packet_hash": "11" * 48,
            "source_observation_commitment": "22" * 48,
        }
        packet, policy, commitments = successor.build_packet(
            demo, prior, current_height=700, max_fills=19, validity_blocks=2_000
        )

        successor.validate_packet(packet, demo, 19)
        self.assertEqual(packet["capacity_base_atoms"], 20_000_000)
        self.assertEqual(packet["capacity_quote_atoms"], 210)
        self.assertEqual(packet["max_fills"], 19)
        self.assertEqual(packet["previous_fix_hash"], "11" * 48)
        self.assertEqual(packet["valid_from_height"], 701)
        self.assertEqual(packet["expires_at_height"], 2_700)
        self.assertEqual(policy["max_fills"], 19)
        self.assertEqual(commitments["packet"]["hash"], packet["packet_hash"])


if __name__ == "__main__":
    unittest.main()
