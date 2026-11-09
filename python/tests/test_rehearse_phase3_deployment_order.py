"""Unit tests for scripts/rehearse-phase3-deployment-order.py without Anvil."""
from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "rehearse_phase3_deployment_order", ROOT / "scripts/rehearse-phase3-deployment-order.py"
)
rehearsal = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(rehearsal)

DEPLOYER = "0x6ac7ea33f8831ea9dcc53393aaa88b25a785dbf0"


class KeccakAndCreateAddressTests(unittest.TestCase):
    def test_keccak256_known_vectors(self):
        self.assertEqual(
            rehearsal.keccak256(b"").hex(),
            "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470",
        )
        self.assertEqual(
            rehearsal.keccak256(b"abc").hex(),
            "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45",
        )
        # A message of exactly one rate block minus one byte takes the 0x81 pad.
        self.assertEqual(len(rehearsal.keccak256(b"\x00" * 135)), 32)

    def test_create_address_known_vectors(self):
        # The widely published vectors for this deployer, which `cast compute-address` also gives.
        self.assertEqual(rehearsal.create_address(DEPLOYER, 0), "0xcd234a471b72ba2f1ccf0a70fcaba648a5eecd8d")
        self.assertEqual(rehearsal.create_address(DEPLOYER, 1), "0x343c43a37d37dff08ae8c4a11544c718abb4fcf8")
        self.assertEqual(rehearsal.create_address(DEPLOYER, 2), "0xf778b86fa74e846c4f0a1fbd1335fe81c00a0c91")
        self.assertEqual(rehearsal.create_address(DEPLOYER, 3), "0xfffd933a0bc612844eaf0c6fe3e5b8e9b6c1d19c")
        # A nonce above 0x7f is RLP-encoded with a length prefix.
        self.assertEqual(rehearsal.create_address(DEPLOYER, 200), "0xeb7facd118466c9acbcb4ee964a0ac0b0b2ef256")

    def test_create_address_refuses_a_bad_deployer_or_nonce(self):
        with self.assertRaises(rehearsal.RehearsalFailure):
            rehearsal.create_address("0x1234", 0)
        with self.assertRaises(rehearsal.RehearsalFailure):
            rehearsal.create_address(DEPLOYER, -1)


class NonceGuardTests(unittest.TestCase):
    def setUp(self):
        self.verifier = rehearsal.create_address(DEPLOYER, 0)
        self.lock = rehearsal.create_address(DEPLOYER, 1)

    def test_the_planned_nonce_passes(self):
        self.assertEqual(
            rehearsal.refuse_unless_planned_nonce(DEPLOYER, 0, 0, self.verifier, self.lock),
            (self.verifier, self.lock),
        )

    def test_a_nonce_off_by_one_is_refused(self):
        with self.assertRaises(rehearsal.RehearsalFailure) as refusal:
            rehearsal.refuse_unless_planned_nonce(DEPLOYER, 1, 0, self.verifier, self.lock)
        message = str(refusal.exception)
        self.assertIn("refusing to deploy", message)
        self.assertIn(rehearsal.create_address(DEPLOYER, 1), message)
        self.assertIn(rehearsal.create_address(DEPLOYER, 2), message)

    def test_predictions_that_do_not_match_the_nonce_are_refused(self):
        with self.assertRaises(rehearsal.RehearsalFailure):
            rehearsal.refuse_unless_planned_nonce(DEPLOYER, 0, 0, self.lock, self.verifier)


class GenesisAndManifestTests(unittest.TestCase):
    def test_pin_lane_genesis_replaces_only_the_lane_pins(self):
        fixture = json.loads((ROOT / rehearsal.GENESIS_FIXTURE).read_text())
        pinned = rehearsal.pin_lane_genesis(
            fixture,
            lock_address="0x" + "AB" * 20,
            lock_code_hash="0x" + "CD" * 32,
            token_address="0x" + "12" * 20,
            token_code_hash="0x" + "34" * 32,
        )
        policy = rehearsal.lane_policy(pinned)
        self.assertEqual(policy["lock_contract_address"], "0x" + "ab" * 20)
        self.assertEqual(policy["lock_runtime_code_hash"], "0x" + "cd" * 32)
        self.assertEqual(policy["chain_id"], 84532)
        original = rehearsal.lane_policy(fixture)
        for key in ("committee_root", "committee_epoch", "confirmation_depth"):
            self.assertEqual(policy[key], original[key])
        self.assertNotEqual(original["lock_contract_address"], policy["lock_contract_address"])

    def test_manifest_shape(self):
        with tempfile.TemporaryDirectory() as directory:
            out = Path(directory)
            (out / "summary.json").write_text("{}\n")
            (out / "deploy-lock.stdout").write_text("{}\n")
            deployment = {"predicted": {"lock": "0x" + "00" * 20}, "deployed": {"lock": "0x" + "00" * 20}}
            manifest = rehearsal.build_manifest(out, "c" * 40, "t" * 40, {"anvil": "anvil 1"}, deployment)
        self.assertEqual(manifest["source_commit"], "c" * 40)
        self.assertEqual(manifest["source_tree"], "t" * 40)
        self.assertEqual(manifest["toolchain"], {"anvil": "anvil 1"})
        self.assertEqual(manifest["deployment"], deployment)
        self.assertEqual(set(manifest["source_files"]), set(rehearsal.SOURCE_FILES))
        self.assertEqual(set(manifest["evidence_files"]), {"summary.json"})
        self.assertEqual(set(manifest["not_published"]), {"deploy-lock.stdout"})

    def test_fixture_checkpoint_is_non_zero(self):
        for key in ("initialCheckpointCommitment", "initialCommitteeRootCommitment", "pftlGenesisHashCommitment"):
            self.assertNotEqual(int(rehearsal.FIXTURE_CHECKPOINT[key], 16), 0)
        self.assertGreater(rehearsal.FIXTURE_CHECKPOINT["initialFinalizedHeight"], 0)

    def test_output_inside_docs_is_refused(self):
        with self.assertRaises(rehearsal.RehearsalFailure):
            rehearsal.confined_output(ROOT / "docs" / "review" / "fresh-run-directory")


if __name__ == "__main__":
    unittest.main()
