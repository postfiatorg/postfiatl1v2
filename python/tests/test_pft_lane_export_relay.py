"""Unit tests for scripts/pft-lane-export-relay.py with a mocked Base RPC."""
from __future__ import annotations

import argparse
import subprocess
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
SP1 = "0x" + "5f" * 20


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
    "proof_program_version": 1,
    "burn_tx_id": h48("67"),
    "withdrawal_finalized_height": 4,
    "finalized_block_height": 4,
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
            (VERIFIER, "sp1Verifier()"): word(SP1),
            (VERIFIER, "programVKey()"): word("0b" * 32),
            (VERIFIER, "maxProofBytes()"): word(4096),
            (VERIFIER, "maxPublicValuesBytes()"): word(16384),
            (VERIFIER, f"checkpointCommitteeRootCommitment(bytes32):{prior}"): relay.commitment(VALUES["committee_root"]),
            (VERIFIER, f"consumedWithdrawalIdCommitment(bytes32):{withdrawal}"): word(0),
            (VERIFIER, f"consumedProofNullifier(bytes32):{word(VALUES['proof_nullifier'])}"): word(0),
        }
        self.answers.update(overrides or {})
        self.chain = 84532
        self.code_hashes = {LOCK: "0x" + "99" * 32, TOKEN: "0x" + "aa" * 32}

    def chain_id(self):
        return self.chain

    def code_hash(self, address):
        return self.code_hashes.get(address, "0x" + relay.keccak256(b"").hex())

    def call(self, address, signature, *args):
        key = (address, signature + "".join(f":{arg}" for arg in args))
        return self.answers[key].lower()


def refusals(rpc=None, proof_bytes=100, public_values_bytes=1500, expected=None, **changes):
    values = dict(VALUES, **changes)
    reads = relay.read_release_state(rpc or MockRpc(), LOCK, values)
    return [refusal["code"] for refusal in relay.release_refusals(
        reads, values, proof_bytes=proof_bytes, public_values_bytes=public_values_bytes, expected=expected or {})]


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
    def test_placeholder_proof_needs_the_mock_sp1_code_hash_and_a_loopback_anvil(self):
        proof = {"mock_verifier_only": True}
        mock = "0x" + "aa" * 32

        def refusal(**kwargs):
            values = dict(loopback=True, anvil=True, sp1_code_hash=mock, mock_code_hash=mock,
                          lock_verifier=VERIFIER, acknowledged=None)
            values.update(kwargs)
            return relay.mock_proof_refusal(proof, **values)

        self.assertIsNone(refusal())
        self.assertIsNotNone(refusal(loopback=False))
        self.assertIsNotNone(refusal(anvil=False))
        self.assertIsNotNone(refusal(sp1_code_hash="0x" + "bb" * 32))
        self.assertIsNotNone(refusal(mock_code_hash=None))
        # The override names the lock's verifier and still needs the mock's code.
        self.assertIsNone(refusal(loopback=False, anvil=False, acknowledged=VERIFIER))
        self.assertIsNotNone(refusal(loopback=False, anvil=False, acknowledged="0x" + "00" * 20))
        self.assertIsNotNone(refusal(loopback=False, anvil=False, acknowledged=VERIFIER,
                                     sp1_code_hash="0x" + "bb" * 32))
        # A real proof needs none of it.
        self.assertIsNone(relay.mock_proof_refusal({"mock_verifier_only": False}, loopback=False, anvil=False,
                                                   sp1_code_hash=None, mock_code_hash=None,
                                                   lock_verifier=VERIFIER, acknowledged=None))

    def test_the_chain_id_constant_is_gone(self):
        self.assertFalse(hasattr(relay, "LOCAL_ANVIL_CHAIN_IDS"))

    def test_mock_code_hash_from_the_artifact(self):
        artifact = {"deployedBytecode": {"object": "0x6001"}}
        self.assertEqual(relay.artifact_code_hash(artifact), "0x" + relay.keccak256(bytes.fromhex("6001")).hex())

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


class RedactionTests(unittest.TestCase):
    KEY = "0x" + "ab" * 32

    def failing(self, error):
        original = relay.subprocess.run

        def fake(*args, **kwargs):
            raise error

        relay.subprocess.run = fake
        try:
            with self.assertRaises(relay.RelayRefusal) as caught:
                relay.run(["cast", "send", "--private-key", self.KEY], secret=self.KEY)
        finally:
            relay.subprocess.run = original
        return str(caught.exception)

    def test_timeout_does_not_print_the_key(self):
        message = self.failing(subprocess.TimeoutExpired(["cast", "send", "--private-key", self.KEY], 1))
        self.assertNotIn(self.KEY, message)
        self.assertIn("<redacted>", message)

    def test_os_error_does_not_print_the_key(self):
        message = self.failing(OSError(2, "No such file", f"cast --private-key {self.KEY}"))
        self.assertNotIn(self.KEY, message)


class MoreRefusalTests(unittest.TestCase):
    def test_zero_recipient(self):
        self.assertIn("zero_recipient", refusals(recipient="0x" + "00" * 20))

    def test_live_code_hashes(self):
        rpc = MockRpc()
        rpc.code_hashes[TOKEN] = "0x" + "ab" * 32
        self.assertIn("live_token_code", refusals(rpc))
        rpc = MockRpc()
        rpc.code_hashes[LOCK] = "0x" + "ab" * 32
        self.assertIn("live_lock_code", refusals(rpc))

    def test_program_version_and_withdrawal_height(self):
        self.assertIn("wrong_proof_program_version", refusals(proof_program_version=2))
        self.assertIn("wrong_withdrawal_height", refusals(withdrawal_finalized_height=5))
        self.assertIn("wrong_withdrawal_height", refusals(withdrawal_finalized_height=0))

    def test_sizes_and_empty_proof(self):
        self.assertIn("proof_too_large", refusals(proof_bytes=4097))
        self.assertIn("proof_too_large", refusals(proof_bytes=0))
        self.assertIn("public_values_too_large", refusals(public_values_bytes=16385))
        self.assertEqual(refusals(proof_bytes=4096, public_values_bytes=16384), [])

    def test_expected_program_key_and_sp1_verifier(self):
        self.assertEqual(refusals(expected={"program_vkey": "0x" + "0b" * 32, "sp1_verifier": SP1}), [])
        self.assertIn("wrong_program_vkey", refusals(expected={"program_vkey": "0x" + "0c" * 32}))
        self.assertIn("wrong_sp1_verifier", refusals(expected={"sp1_verifier": "0x" + "00" * 20}))


class ReleaseResultTests(unittest.TestCase):
    def receipt(self, amount=400_000, status="0x1", address=LOCK):
        topics = [relay.RELEASED_TOPIC, relay.commitment(VALUES["withdrawal_id"]), relay.commitment(VALUES["burn_tx_id"]),
                  word(VALUES["withdrawal_packet_digest"])]
        data = "0x" + RECIPIENT[2:].rjust(64, "0") + f"{amount:064x}"
        return {"status": status, "logs": [{"address": address, "topics": topics, "data": data}]}

    def test_the_event_is_the_result_and_a_balance_mismatch_only_warns(self):
        result = relay.release_result(self.receipt(), LOCK, VALUES, balance_before=0, balance_after=400_000)
        self.assertEqual(result["warnings"], [])
        # An unrelated transfer between the two reads: still released, with a warning.
        result = relay.release_result(self.receipt(), LOCK, VALUES, balance_before=0, balance_after=400_007)
        self.assertEqual(result["released"]["amount"], 400_000)
        self.assertEqual(len(result["warnings"]), 1)

    def test_a_wrong_event_or_failed_receipt_is_refused(self):
        with self.assertRaises(relay.RelayRefusal):
            relay.release_result(self.receipt(amount=1), LOCK, VALUES, balance_before=0, balance_after=1)
        with self.assertRaises(relay.RelayRefusal):
            relay.release_result(self.receipt(status="0x0"), LOCK, VALUES, balance_before=0, balance_after=0)
        with self.assertRaises(relay.RelayRefusal):
            relay.release_result(self.receipt(address=TOKEN), LOCK, VALUES, balance_before=0, balance_after=400_000)


class StatusAndCheckpointTests(unittest.TestCase):
    def test_checkpoint_report_says_how_far_behind_the_verifier_is(self):
        values = dict(VALUES)
        reads = relay.read_release_state(MockRpc(), LOCK, values)
        report = relay.checkpoint_report(reads, values)
        self.assertTrue(report["prior_is_latest"])
        self.assertEqual(report["latest_finalized_height"], 5)
        self.assertEqual(report["proof_finalized_height"], 4)

    def test_status_takes_from_block_and_a_proof_path(self):
        args = relay.parser().parse_args(["status", "--rpc", "http://127.0.0.1:1", "--lock", LOCK, "--input", "x",
                                          "--from-block", "17"])
        self.assertEqual(args.from_block, 17)
        with self.assertRaises(SystemExit):
            relay.parser().parse_args(["status", "--rpc", "http://127.0.0.1:1", "--lock", LOCK, "--input", "x"])


if __name__ == "__main__":
    unittest.main()
