"""Unit tests for scripts/pft-lane-export-relay.py with a mocked Base RPC."""
from __future__ import annotations

import argparse
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("pft_lane_export_relay", ROOT / "scripts/pft-lane-export-relay.py")
relay = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(relay)

LOCK = "0x" + "1a" * 20
TOKEN = "0x" + "2b" * 20
VERIFIER = "0x" + "3c" * 20
RECIPIENT = "0x" + "4d" * 20


def h48(byte):
    return byte * 48


VALUES = {
    "pftl_chain_id": "postfiat-lane",
    "pftl_genesis_hash": h48("11"),
    "pftl_protocol_version": 1,
    "route_profile_hash": h48("22"),
    "route_epoch": 1,
    "asset_id": h48("33"),
    "prior_checkpoint_block_id": h48("44"),
    "committee_root": h48("55"),
    "committee_transition_commitment": "",
    "withdrawal_id": h48("66"),
    "proof_nullifier": "77" * 32,
    "withdrawal_packet_digest": "88" * 32,
    "amount_atoms": 400_000,
    "recipient": RECIPIENT,
    "arbitrum_chain_id": 84532,
    "vault_address": LOCK,
    "vault_runtime_code_hash": "99" * 32,
    "token_address": TOKEN,
    "token_runtime_code_hash": "aa" * 32,
}


def word(value):
    if isinstance(value, bool):
        value = int(value)
    if isinstance(value, int):
        return f"0x{value:064x}"
    return "0x" + value.lower().removeprefix("0x").rjust(64, "0")


class MockRpc:
    """Answers the relay's reads from a table keyed by (address, calldata)."""

    url = "http://127.0.0.1:8545"

    def __init__(self, overrides=None):
        prior = relay.commitment(VALUES["prior_checkpoint_block_id"])
        withdrawal = relay.commitment(VALUES["withdrawal_id"])
        self.answers = {
            (LOCK, "finalityVerifier()"): word(VERIFIER),
            (LOCK, "token()"): word(TOKEN),
            (LOCK, "totalLocked()"): word(1_000_000),
            (LOCK, "totalReleased()"): word(0),
            (VERIFIER, "pftlChainIdHash()"): "0x" + relay.keccak256(b"postfiat-lane").hex(),
            (VERIFIER, "pftlGenesisHashCommitment()"): relay.commitment(VALUES["pftl_genesis_hash"]),
            (VERIFIER, "routeProfileHashCommitment()"): relay.commitment(VALUES["route_profile_hash"]),
            (VERIFIER, "assetIdCommitment()"): relay.commitment(VALUES["asset_id"]),
            (VERIFIER, "vaultRuntimeCodeHash()"): word("99" * 32),
            (VERIFIER, "tokenRuntimeCodeHash()"): word("aa" * 32),
            (VERIFIER, "latestCheckpointCommitment()"): prior,
            (VERIFIER, "pftlProtocolVersion()"): word(1),
            (VERIFIER, "routeEpoch()"): word(1),
            (VERIFIER, "arbitrumChainId()"): word(84532),
            (VERIFIER, "latestFinalizedHeight()"): word(5),
            (VERIFIER, "token()"): word(TOKEN),
            (VERIFIER, f"checkpointCommitteeRootCommitment(bytes32):{prior}"): relay.commitment(VALUES["committee_root"]),
            (VERIFIER, f"consumedWithdrawalIdCommitment(bytes32):{withdrawal}"): word(0),
            (VERIFIER, f"consumedProofNullifier(bytes32):{word(VALUES['proof_nullifier'])}"): word(0),
        }
        self.answers.update(overrides or {})
        self.chain = 84532

    def chain_id(self):
        return self.chain

    def call(self, address, signature, *args):
        key = (address, signature + "".join(f":{arg}" for arg in args))
        return self.answers[key].lower()


def refusals(rpc=None, **changes):
    values = dict(VALUES, **changes)
    reads = relay.read_release_state(rpc or MockRpc(), LOCK, values)
    return [refusal["code"] for refusal in relay.release_refusals(reads, values)]


class ReleaseRefusalTests(unittest.TestCase):
    def test_matching_values_pass(self):
        self.assertEqual(refusals(), [])

    def test_wrong_vault_token_and_chain_id(self):
        self.assertIn("wrong_vault", refusals(vault_address="0x" + "5e" * 20))
        self.assertIn("wrong_token", refusals(token_address="0x" + "5e" * 20))
        self.assertIn("wrong_chain_id", refusals(arbitrum_chain_id=8453))
        rpc = MockRpc()
        rpc.chain = 8453
        self.assertIn("wrong_chain_id", refusals(rpc))

    def test_wrong_route_profile_asset_and_genesis(self):
        self.assertIn("wrong_route_profile", refusals(route_profile_hash=h48("23")))
        self.assertIn("wrong_route_profile", refusals(route_epoch=2))
        self.assertIn("wrong_asset_id", refusals(asset_id=h48("34")))
        self.assertIn("wrong_genesis", refusals(pftl_genesis_hash=h48("12")))
        self.assertIn("wrong_pftl_chain", refusals(pftl_chain_id="other"))

    def test_unknown_prior_checkpoint(self):
        other = h48("45")
        rpc = MockRpc({(VERIFIER, f"checkpointCommitteeRootCommitment(bytes32):{relay.commitment(other)}"): word(0)})
        self.assertIn("unknown_prior_checkpoint", refusals(rpc, prior_checkpoint_block_id=other))

    def test_already_consumed_withdrawal(self):
        withdrawal = relay.commitment(VALUES["withdrawal_id"])
        rpc = MockRpc({(VERIFIER, f"consumedWithdrawalIdCommitment(bytes32):{withdrawal}"): word(1)})
        self.assertEqual(refusals(rpc), ["already_consumed"])

    def test_amount_above_locked_minus_released(self):
        rpc = MockRpc({(LOCK, "totalReleased()"): word(700_000)})
        self.assertEqual(refusals(rpc), ["above_locked"])
        self.assertEqual(refusals(MockRpc({(LOCK, "totalReleased()"): word(600_000)})), [])


class SafetyTests(unittest.TestCase):
    def test_placeholder_proof_only_on_local_anvil_or_named_verifier(self):
        proof = {"mock_verifier_only": True}
        self.assertIsNone(relay.mock_proof_refusal(proof, local_anvil=True, lock_verifier=VERIFIER, acknowledged=None))
        self.assertIsNotNone(relay.mock_proof_refusal(proof, local_anvil=False, lock_verifier=VERIFIER,
                                                      acknowledged=None))
        self.assertIsNotNone(relay.mock_proof_refusal(proof, local_anvil=False, lock_verifier=VERIFIER,
                                                      acknowledged="0x" + "00" * 20))
        self.assertIsNone(relay.mock_proof_refusal(proof, local_anvil=False, lock_verifier=VERIFIER,
                                                   acknowledged=VERIFIER.upper().replace("0X", "0x")))
        self.assertIsNone(relay.mock_proof_refusal({"mock_verifier_only": False}, local_anvil=False,
                                                   lock_verifier=VERIFIER, acknowledged=None))

    def test_private_key_only_on_local_anvil(self):
        args = argparse.Namespace(private_key_file=Path("/nonexistent"), account=None, password_file=None)
        with self.assertRaises(relay.RelayRefusal):
            relay.signer_args(args, local_anvil=False)
        args = argparse.Namespace(private_key_file=None, account="relayer", password_file=Path("pw"))
        self.assertEqual(relay.signer_args(args, local_anvil=False),
                         (["--account", "relayer", "--password-file", "pw"], None))

    def test_loopback_urls(self):
        self.assertTrue(relay.is_loopback_url("http://127.0.0.1:8545"))
        self.assertTrue(relay.is_loopback_url("http://localhost:8545"))
        self.assertFalse(relay.is_loopback_url("https://sepolia.base.org"))

    def test_output_never_inside_docs(self):
        with self.assertRaises(relay.RelayRefusal):
            relay.confined(ROOT / "docs/review/x")

    def test_decode_released(self):
        topic = relay.RELEASED_TOPIC
        receipt = {"logs": [{"address": LOCK, "topics": [topic, word("01"), word("02"), word("88" * 32)],
                             "data": "0x" + RECIPIENT[2:].rjust(64, "0") + f"{400_000:064x}"}]}
        released = relay.decode_released(receipt, LOCK)
        self.assertEqual((released["recipient"], released["amount"]), (RECIPIENT, 400_000))
        self.assertEqual(released["packet_digest"], word("88" * 32))
        with self.assertRaises(relay.RelayRefusal):
            relay.decode_released({"logs": []}, LOCK)


if __name__ == "__main__":
    unittest.main()
