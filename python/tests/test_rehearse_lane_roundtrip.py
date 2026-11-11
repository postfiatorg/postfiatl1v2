"""Unit tests for the pure functions of scripts/rehearse-lane-roundtrip.py."""
from __future__ import annotations

import importlib.util
import json
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


class PortablePathTests(unittest.TestCase):
    def test_paths_outside_the_checkout_and_output_become_roles(self):
        out, root = "/home/op/scratch/run", "/home/op/repos/l1"
        roles = {"/home/op/scratch/elf/pft-lane-export-program": "<elf sha256:abcd>"}
        text = json.dumps({
            "elf": "/home/op/scratch/elf/pft-lane-export-program",
            "python": "/usr/bin/python3",
            "data_dir": "/home/op/scratch/run/tmp/chain",
            "script": "/home/op/repos/l1/scripts/x.py",
            "elsewhere": "/home/op/other/thing.json",
            "rpc": "http://127.0.0.1:8545",
            "relative": "crates/node/src/lib.rs",
            "key": "0xsecret",
        })
        clean = roundtrip.portable_text(text, out=out, root=root, roles=roles, secrets=["0xsecret"])
        document = json.loads(clean)
        self.assertEqual(document["elf"], "<elf sha256:abcd>")
        self.assertEqual(document["python"], "<path-outside-checkout>")
        self.assertEqual(document["data_dir"], "<output>/tmp/chain")
        self.assertEqual(document["script"], "<checkout>/scripts/x.py")
        self.assertEqual(document["elsewhere"], "<path-outside-checkout>")
        self.assertEqual(document["rpc"], "http://127.0.0.1:8545")
        self.assertEqual(document["relative"], "crates/node/src/lib.rs")
        self.assertEqual(document["key"], "<disposable-key>")
        for fragment in ("/home", "/usr", "op/scratch"):
            self.assertNotIn(fragment, clean)


if __name__ == "__main__":
    unittest.main()


class DriverProcessGroupTests(unittest.TestCase):
    """cargo runs the driver's test binary as its child: stopping cargo alone leaves it running."""

    def test_stopping_the_driver_stops_its_children(self):
        import os
        import subprocess
        import time
        parent = subprocess.Popen(["sh", "-c", "sleep 30 & echo $!; wait"], stdout=subprocess.PIPE, text=True,
                                  **roundtrip.DRIVER_POPEN)
        grandchild = int(parent.stdout.readline())
        roundtrip.stop_driver(parent)
        for _ in range(50):
            try:
                os.kill(grandchild, 0)
            except ProcessLookupError:
                break
            time.sleep(0.1)
        else:
            os.kill(grandchild, 9)
            self.fail("the driver's child outlived the stop")
        self.assertIsNotNone(parent.poll())
