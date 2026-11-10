"""Unit tests for the pure functions of scripts/rehearse-lane-roundtrip.py."""
from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("rehearse_lane_roundtrip", ROOT / "scripts/rehearse-lane-roundtrip.py")
roundtrip = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(roundtrip)

PINS = {
    "pftl_chain_id_hash": "0x" + "01" * 32,
    "pftl_genesis_hash_commitment": "0x" + "02" * 32,
    "route_profile_hash_commitment": "0x" + "03" * 32,
    "asset_id_commitment": "0x" + "04" * 32,
    "route_profile": {"route_epoch": 1},
}
CHECKPOINT = {"block_id": "aa" * 48, "height": 1, "committee_root": "bb" * 48}


class RoundTripTests(unittest.TestCase):
    def test_verifier_config_takes_the_pins_and_the_checkpoint(self):
        config = roundtrip.roundtrip_verifier_config(
            sp1_verifier="0x" + "f1" * 20, pins=PINS, checkpoint=CHECKPOINT, token="0x" + "2b" * 20,
            token_code_hash="0x" + "3c" * 32, lock_code_hash="0x" + "4d" * 32)
        self.assertEqual(set(config), set(roundtrip.ORDER.VERIFIER_CONFIG_FIELDS))
        self.assertEqual(config["assetIdCommitment"], PINS["asset_id_commitment"])
        self.assertEqual(config["initialCheckpointCommitment"],
                         "0x" + roundtrip.keccak256(bytes.fromhex("aa" * 48)).hex())
        self.assertEqual(config["initialCommitteeRootCommitment"],
                         "0x" + roundtrip.keccak256(bytes.fromhex("bb" * 48)).hex())
        self.assertEqual(config["programVKey"], roundtrip.PLACEHOLDER_PROGRAM_VKEY)
        self.assertEqual(config["arbitrumChainId"], 84532)

    def test_export_operation_is_the_tagged_asset_operation(self):
        operation = roundtrip.export_operation(sender="pf" + "1" * 40, base_recipient="0x" + "5e" * 20, amount=7)
        self.assertEqual(operation["operation"], "external_lane_export_v1")
        self.assertEqual(set(operation), {"operation", "sender", "lane_id", "amount_atoms", "base_recipient"})

    def test_fresh_base_addresses_differ_and_are_addresses(self):
        first, second = roundtrip.fresh_base_address("a"), roundtrip.fresh_base_address("b")
        self.assertNotEqual(first, second)
        self.assertRegex(first, r"^0x[0-9a-f]{40}$")

    def test_base_conservation(self):
        roundtrip.check_base_conservation(lock_balance=600_000, total_locked=1_000_000, total_released=400_000)
        with self.assertRaises(roundtrip.RehearsalFailure):
            roundtrip.check_base_conservation(lock_balance=600_001, total_locked=1_000_000, total_released=400_000)

    def test_lane_conservation_and_cross_chain(self):
        lane = {"reserved_atoms": 300_000_000, "released_atoms": 1_000_000, "exported_atoms": 400_000,
                "remainder_atoms": 299_400_000, "blocks_and_state_replayed": True}
        roundtrip.check_lane_conservation(lane)
        roundtrip.check_cross_chain(lane, {"total_locked": 1_000_000, "total_released": 400_000})
        with self.assertRaises(roundtrip.RehearsalFailure):
            roundtrip.check_lane_conservation(dict(lane, remainder_atoms=299_000_000))
        with self.assertRaises(roundtrip.RehearsalFailure):
            roundtrip.check_cross_chain(lane, {"total_locked": 1_000_000, "total_released": 0})


if __name__ == "__main__":
    unittest.main()
