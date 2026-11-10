"""Unit tests for scripts/rehearse-lane-base-sepolia.py without a chain."""
from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("rehearse_lane_base_sepolia", ROOT / "scripts/rehearse-lane-base-sepolia.py")
dry_run = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(dry_run)


class StateMachineTests(unittest.TestCase):
    def test_stages_run_in_order_and_once(self):
        state = {"completed": [], "data": {}}
        dry_run.check_stage(state, "plan")
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.check_stage(state, "fleet")
        state["completed"].append("plan")
        dry_run.check_stage(state, "fleet")
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.check_stage(state, "plan")
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.check_stage(state, "deploy")

    def test_record_may_follow_a_stopped_prove_and_release(self):
        state = {"completed": list(dry_run.STAGES[:7]), "data": {}}
        dry_run.check_stage(state, "record")
        state = {"completed": list(dry_run.STAGES[:6]), "data": {}}
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.check_stage(state, "record")


class RefusalTests(unittest.TestCase):
    def test_no_raw_private_key(self):
        with self.assertRaises(SystemExit):
            dry_run.parser().parse_args(["--output", "x", "--private-key", "0x01"])
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.signer_flags(None, None)
        self.assertEqual(dry_run.signer_flags("lock-deployer", Path("pw")),
                         ["--account", "lock-deployer", "--password-file", "pw"])

    def test_mock_verifier_only_on_a_loopback_anvil(self):
        dry_run.refuse_mock_off_loopback(mock_artifact=None, rpc_url="https://sepolia.base.org", client_version="")
        dry_run.refuse_mock_off_loopback(mock_artifact=Path("m.json"), rpc_url="http://127.0.0.1:8545",
                                         client_version="anvil/v1.8.1")
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.refuse_mock_off_loopback(mock_artifact=Path("m.json"), rpc_url="https://sepolia.base.org",
                                             client_version="anvil/v1.8.1")
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.refuse_mock_off_loopback(mock_artifact=Path("m.json"), rpc_url="http://127.0.0.1:8545",
                                             client_version="geth")

    def test_the_sp1_verifier_is_the_versioned_one(self):
        self.assertEqual(dry_run.SP1_VERIFIER["name"], "V6_1_0_SP1_VERIFIER_GROTH16")
        self.assertNotIn("GATEWAY", dry_run.SP1_VERIFIER["name"])


class DeploySheetTests(unittest.TestCase):
    def test_sheet_names_every_key_and_its_test_eth(self):
        plan = {"verifier": {"deployer": "0x" + "11" * 20, "planned_nonce": 0, "predicted": "0x" + "22" * 20},
                "lock": {"deployer": "0x" + "33" * 20, "planned_nonce": 0, "predicted": "0x" + "44" * 20}}
        gas = {"deploy-verifier": 3_000_000, "deploy-lock": 2_000_000, "approve": 50_000, "lock": 150_000}
        sheet = dry_run.deploy_sheet(plan=plan, verifier_config={"programVKey": "0x01"}, gas_used=gas,
                                     gas_price_wei=6_000_000)
        needs = sheet["test_eth_per_key"]
        self.assertEqual(needs["verifier deployer"]["gas"], 3_000_000)
        self.assertEqual(needs["holder"]["gas"], 200_000 + 3 * 21_000)
        self.assertEqual(needs["lock deployer"]["wei_with_margin"], 2_000_000 * 6_000_000 * 3)
        markdown = dry_run.sheet_markdown(sheet)
        for text in ("0x" + "22" * 20, "0x" + "44" * 20, "programVKey", dry_run.SP1_VERIFIER["address"]):
            self.assertIn(text, markdown)


class IdentityTests(unittest.TestCase):
    def test_the_committed_elf_matches_the_committed_program_info(self):
        import json
        info = json.loads(dry_run.PROGRAM_INFO.read_text())
        self.assertEqual(dry_run.check_committed_identity(info, dry_run.COMMITTED_ELF), info["elf_sha256"])
        self.assertIsNone(dry_run.parser().parse_args(["--output", "x"]).program_info)

    def test_a_different_elf_is_refused(self):
        import json
        info = dict(json.loads(dry_run.PROGRAM_INFO.read_text()), elf_sha256="00" * 32)
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.check_committed_identity(info, dry_run.COMMITTED_ELF)


if __name__ == "__main__":
    unittest.main()


class DeployAddressTests(unittest.TestCase):
    PLAN = {"verifier": {"deployer": "0x" + "11" * 20, "planned_nonce": 0, "predicted": "0x" + "22" * 20},
            "lock": {"deployer": "0x" + "33" * 20, "planned_nonce": 0, "predicted": "0x" + "44" * 20}}

    def test_a_keystore_for_another_address_is_refused_before_any_broadcast(self):
        sent = []
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.check_deployer_keystores(self.PLAN, lambda role: "0x" + "99" * 20)
        self.assertEqual(sent, [])
        dry_run.check_deployer_keystores(self.PLAN, lambda role: self.PLAN[role]["deployer"].upper().replace("0X", "0x"))

    def test_nonces_must_equal_the_plan(self):
        dry_run.check_planned_nonces(self.PLAN, lambda address, tag: 0)
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.check_planned_nonces(self.PLAN, lambda address, tag: 1 if tag == "pending" else 0)

    def test_a_verifier_that_lands_elsewhere_stops_before_the_lock(self):
        sent = []

        def send(role):
            sent.append(role)
            return ("0x" + "55" * 20 if role == "verifier" else self.PLAN[role]["predicted"]), "0xhash"

        with self.assertRaises(dry_run.RehearsalFailure) as caught:
            dry_run.deploy_pair(self.PLAN, send=send, code_hash=lambda address: "0xc0de",
                                expected_code={"verifier": "0xc0de", "lock": "0xc0de"})
        self.assertEqual(sent, ["verifier"])
        self.assertIn("plan is dead", str(caught.exception))

    def test_a_wrong_code_hash_also_stops_before_the_lock_and_both_must_match(self):
        sent = []

        def send(role):
            sent.append(role)
            return self.PLAN[role]["predicted"], "0xhash"

        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.deploy_pair(self.PLAN, send=send, code_hash=lambda address: "0xbad",
                                expected_code={"verifier": "0xc0de", "lock": "0xc0de"})
        self.assertEqual(sent, ["verifier"])
        sent.clear()
        result = dry_run.deploy_pair(self.PLAN, send=send, code_hash=lambda address: "0xc0de",
                                     expected_code={"verifier": "0xc0de", "lock": "0xc0de"})
        self.assertEqual(sent, ["verifier", "lock"])
        self.assertEqual(result["lock"]["address"], self.PLAN["lock"]["predicted"])


class ProgramIdentityTests(unittest.TestCase):
    def test_non_fork_mode_refuses_an_override(self):
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.load_program_identity(fork=False, override=Path("other.json"), worktree_clean=True,
                                          head="abc")

    def test_non_fork_mode_needs_a_clean_worktree(self):
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.load_program_identity(fork=False, override=None, worktree_clean=False, head="abc")

    def test_non_fork_mode_takes_the_committed_identity(self):
        identity = dry_run.load_program_identity(fork=False, override=None, worktree_clean=True, head="abc")
        self.assertEqual(identity["elf_sha256"], "e92f20046c20761bb7a26ec47540000a88fad28b51874453437c3dd390f448ab")
        self.assertEqual(identity["checked_out_commit"], "abc")
        self.assertEqual(len(identity["runs"]), 2)

    def test_a_mismatching_elf_is_refused(self):
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.load_program_identity(fork=False, override=None, worktree_clean=True, head="abc",
                                          elf=dry_run.ROOT / "programs/pft-lane-export/Cargo.toml")

    def test_fork_mode_accepts_an_override(self):
        identity = dry_run.load_program_identity(fork=True, override=dry_run.PROGRAM_INFO, worktree_clean=False,
                                                 head="abc")
        self.assertEqual(identity["program_vkey"][:6], "0x000c")


class FakeChain:
    """Base state after a crash: what a mined transaction left behind."""

    def __init__(self, *, receipts=None, nonces=None, codes=None, allowance=0, next_nonce=0, records=None):
        self.receipts, self.nonces, self.codes = receipts or {}, nonces or {}, codes or {}
        self.allowance_value, self.next_nonce_value, self.records = allowance, next_nonce, records or {}

    def receipt(self, transaction):
        return self.receipts.get(transaction)

    def nonce(self, address, tag):
        return self.nonces.get((address, tag), self.nonces.get(address, 0))

    def code_hash(self, address):
        return self.codes.get(address, "0x" + "00" * 32)

    def allowance(self, owner, spender):
        return self.allowance_value

    def next_nonce(self, lock):
        return self.next_nonce_value

    def lock_record(self, lock, nonce):
        return self.records.get(nonce)


class RecoveryTests(unittest.TestCase):
    """Fault injection: the transaction was mined, the stage was not marked complete."""

    SENDER, PREDICTED, CODE = "0x" + "11" * 20, "0x" + "22" * 20, "0xc0de"

    def intent(self, **extra):
        state = {"completed": [], "data": {}}
        return state, dry_run.begin_intent(state, stage="deploy", role="verifier", sender=self.SENDER, nonce=0,
                                           purpose="deploy the verifier", **extra)

    def test_the_intent_is_recorded_once(self):
        state, first = self.intent()
        again = dry_run.begin_intent(state, stage="deploy", role="verifier", sender=self.SENDER, nonce=0,
                                     purpose="deploy the verifier")
        self.assertIs(first, again)
        self.assertEqual(len(state["intents"]), 1)

    def test_a_mined_verifier_is_adopted_by_hash_or_by_nonce(self):
        _, intent = self.intent()
        chain = FakeChain(nonces={self.SENDER: 1}, codes={self.PREDICTED: self.CODE})
        self.assertEqual(dry_run.reconcile_deployment(intent, chain, predicted=self.PREDICTED,
                                                      expected_code=self.CODE), "adopt")
        intent["hash"] = "0xabc"
        chain.receipts["0xabc"] = {"status": "0x1", "contractAddress": self.PREDICTED}
        self.assertEqual(dry_run.reconcile_deployment(intent, chain, predicted=self.PREDICTED,
                                                      expected_code=self.CODE), "adopt")

    def test_a_mined_lock_deployment_with_other_code_is_refused(self):
        _, intent = self.intent()
        chain = FakeChain(nonces={self.SENDER: 1}, codes={self.PREDICTED: "0xother"})
        with self.assertRaises(dry_run.RehearsalFailure) as caught:
            dry_run.reconcile_deployment(intent, chain, predicted=self.PREDICTED, expected_code=self.CODE)
        self.assertIn("plan is dead", str(caught.exception))

    def test_an_unsent_deployment_is_sent_and_a_pending_one_waits(self):
        _, intent = self.intent()
        self.assertEqual(dry_run.reconcile_deployment(intent, FakeChain(), predicted=self.PREDICTED,
                                                      expected_code=self.CODE), "send")
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.reconcile_deployment(intent, FakeChain(nonces={(self.SENDER, "pending"): 1}),
                                         predicted=self.PREDICTED, expected_code=self.CODE)

    def test_a_mined_approve_is_adopted(self):
        self.assertEqual(dry_run.reconcile_approve({}, FakeChain(allowance=10), owner="h", spender="l", amount=10),
                         "adopt")
        self.assertEqual(dry_run.reconcile_approve({}, FakeChain(allowance=0), owner="h", spender="l", amount=10),
                         "send")

    def test_a_mined_lock_is_adopted_and_never_sent_twice(self):
        intent = {"next_nonce_before": 3}
        chain = FakeChain(next_nonce=4, records={3: ("0xh", 10, "0xr")})
        self.assertEqual(dry_run.reconcile_lock(intent, chain, lock="l", holder="0xh", amount=10,
                                                recipient_hash="0xr"), "adopt")
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.reconcile_lock(intent, FakeChain(next_nonce=4, records={3: ("0xother", 10, "0xr")}), lock="l",
                                   holder="0xh", amount=10, recipient_hash="0xr")
        self.assertEqual(dry_run.reconcile_lock(intent, FakeChain(next_nonce=3), lock="l", holder="0xh", amount=10,
                                                recipient_hash="0xr"), "send")

    def test_deploy_needs_the_no_resume_acknowledgement(self):
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.require_no_resume_acknowledged(False)
        dry_run.require_no_resume_acknowledged(True)


class SheetAndGasTests(unittest.TestCase):
    def test_sheet_names_the_relayer_and_the_funding_transfers(self):
        plan = DeployAddressTests.PLAN
        sheet = dry_run.deploy_sheet(plan=plan, verifier_config={}, gas_used={"approve": 1, "lock": 1},
                                     gas_price_wei=1)
        self.assertIn("relayer", sheet["test_eth_per_key"])
        self.assertIn("real proof", sheet["test_eth_per_key"]["relayer"]["note"])
        self.assertEqual(sheet["funding_transfers"]["count"], 3)
        self.assertEqual(sheet["test_eth_per_key"]["holder"]["gas"], 2 + 3 * 21_000)

    def test_deploy_refuses_a_gas_price_far_above_the_plan(self):
        dry_run.check_gas_price(plan_wei=6_000_000, current_wei=6_000_000 * dry_run.GAS_PRICE_FACTOR)
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.check_gas_price(plan_wei=6_000_000, current_wei=6_000_000 * dry_run.GAS_PRICE_FACTOR + 1)

    def test_errors_never_carry_keystore_names_or_password_files(self):
        text = dry_run.redact_all("cast failed: --account pft-lock-deployer --password-file /k/pw",
                                  ["pft-lock-deployer", "/k/pw"])
        self.assertNotIn("pft-lock-deployer", text)
        self.assertNotIn("/k/pw", text)
