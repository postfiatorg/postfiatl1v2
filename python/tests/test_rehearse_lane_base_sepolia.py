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

    def test_no_mock_sp1_option_and_no_dead_imports(self):
        with self.assertRaises(SystemExit):
            dry_run.parser().parse_args(["--output", "x", "--mock-sp1-artifact", "m.json"])
        source = (ROOT / "scripts/rehearse-lane-base-sepolia.py").read_text()
        self.assertNotIn("import shutil", source)
        self.assertNotIn("urls = ", source)

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

        def derive(role):
            sent.append(role)  # the derivation is the only call; a broadcast would follow the check
            return "0x" + "99" * 20

        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.check_deployer_keystores(self.PLAN, derive)
            sent.append("broadcast")
        self.assertEqual(sent, ["verifier"])
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


class RealRunReadinessTests(unittest.TestCase):
    """A2, A13: one block of checks before the first intent of a real run."""

    PLAN = DeployAddressTests.PLAN
    SHEET = {"test_eth_per_key": {"verifier deployer": {"gas": 100}, "lock deployer": {"gas": 50},
                                  "holder": {"gas": 10 + 3 * 21_000}}}

    def ready(self, **change):
        values = dict(plan=self.PLAN, sheet=self.SHEET, holder_address=dry_run.HOLDER.upper().replace("0X", "0x"),
                      relayer_opens=True, eth_balance=lambda address: 10 ** 18, token_balance=lambda address: 10 ** 9,
                      gas_price_wei=7, node_bin=Path(__file__), driver_alive=lambda: True)
        values.update(change)
        return dry_run.check_real_run_ready(**values)

    def test_everything_ready_passes(self):
        self.ready()

    def test_each_missing_piece_refuses(self):
        for change in (dict(holder_address="0x" + "99" * 20), dict(relayer_opens=False),
                       dict(eth_balance=lambda address: 100 * 7 - 1), dict(token_balance=lambda address: 999_999),
                       dict(node_bin=Path("/nonexistent/postfiat-node")), dict(driver_alive=lambda: False)):
            with self.subTest(change=change), self.assertRaises(dry_run.RehearsalFailure):
                self.ready(**change)

    def test_deployers_differ_from_the_holder_and_each_other(self):
        dry_run.check_deployers_differ_from_holder(self.PLAN)
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.check_deployers_differ_from_holder({"verifier": {"deployer": dry_run.HOLDER},
                                                        "lock": self.PLAN["lock"]})
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.check_deployers_differ_from_holder({"verifier": self.PLAN["lock"], "lock": self.PLAN["lock"]})

    def test_the_lock_event_names_the_holder(self):
        topic = "0x" + "00" * 12 + dry_run.HOLDER[2:]
        dry_run.check_lock_event_holder({"topics": ["0xt", "0x00", topic]})
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.check_lock_event_holder({"topics": ["0xt", "0x00", "0x" + "00" * 12 + "99" * 20]})


class DriverTimeoutTests(unittest.TestCase):
    """A1, A12: the driver outlives every wait of a real run and reports the fixed values."""

    def test_the_driver_waits_longer_than_every_budget(self):
        budgets = (dry_run.CONFIRMATION_BUDGET_SECONDS + dry_run.SEND_BUDGET_SECONDS
                   + dry_run.FINALITY_BUDGET_SECONDS)
        self.assertGreater(dry_run.DRIVER_HANDSHAKE_TIMEOUT_SECONDS, budgets)
        self.assertGreater(dry_run.FINALITY_BUDGET_SECONDS, 3600)
        driver = (ROOT / "crates/node/src/tests/external_lane_roundtrip_tests.rs").read_text()
        self.assertIn('config["handshake_timeout_seconds"]', driver)
        self.assertNotIn("0..12_000", driver)

    def test_the_driver_report_must_match(self):
        ready = {"protocol_version": 1, "lane_release_cap_atoms": 6_000_000, "handshake_timeout_seconds": 10}
        dry_run.check_driver_report(ready, max_lock_amount=6_000_000, handshake_timeout=10)
        for change in ({"protocol_version": 2}, {"lane_release_cap_atoms": 5_999_999},
                       {"handshake_timeout_seconds": 1200}):
            with self.subTest(change=change), self.assertRaises(dry_run.RehearsalFailure):
                dry_run.check_driver_report({**ready, **change}, max_lock_amount=6_000_000, handshake_timeout=10)

    def test_the_confirmation_waits_while_the_driver_lives(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "confirm-deploy"
            with self.assertRaises(dry_run.RehearsalFailure):
                dry_run.wait_for_confirmation(path, expected="ab", alive=lambda: False, budget=5, sleep=lambda s: None)
            with self.assertRaises(dry_run.RehearsalFailure):
                dry_run.wait_for_confirmation(path, expected="ab", alive=lambda: True, budget=3, sleep=lambda s: None)
            path.write_text("cd\n")
            with self.assertRaises(dry_run.RehearsalFailure):
                dry_run.wait_for_confirmation(path, expected="ab", alive=lambda: True, budget=3, sleep=lambda s: None)
            path.write_text("ab\n")
            dry_run.wait_for_confirmation(path, expected="ab", alive=lambda: True, budget=3, sleep=lambda s: None)


class RunShapeTests(unittest.TestCase):
    """A5, A6, A8, A9, A10."""

    def test_off_the_fork_no_stop_between_deploy_and_release(self):
        for stage in ("deploy", "lock", "release-on-pftl"):
            with self.subTest(stage=stage), self.assertRaises(dry_run.RehearsalFailure):
                dry_run.refuse_through_off_fork(fork=False, through=stage)
            dry_run.refuse_through_off_fork(fork=True, through=stage)
        for stage in ("plan", "export", "record"):
            dry_run.refuse_through_off_fork(fork=False, through=stage)

    def test_the_chain_id_must_be_base_sepolia(self):
        dry_run.check_chain_id(84532)
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.check_chain_id(8453)

    def test_the_gas_guard_is_within_the_funding_margin(self):
        self.assertLessEqual(dry_run.GAS_PRICE_FACTOR, dry_run.GAS_MARGIN)
        with self.assertRaises(dry_run.RehearsalFailure) as caught:
            dry_run.check_gas_price(plan_wei=1, current_wei=dry_run.GAS_MARGIN + 1)
        self.assertNotIn("Rerun preflight", str(caught.exception))
        self.assertIn("fresh output directory", str(caught.exception))

    def test_state_is_written_atomically(self):
        import json
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "state.json"
            dry_run.save_state(path, {"completed": ["plan"], "data": {}})
            self.assertEqual(json.loads(path.read_text())["completed"], ["plan"])
            self.assertEqual(sorted(p.name for p in Path(tmp).iterdir()), ["state.json"])

    def test_a_later_start_needs_the_deploy_commit(self):
        dry_run.check_commit({"data": {}}, "abc")
        dry_run.check_commit({"data": {"commit": "abc"}}, "abc")
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.check_commit({"data": {"commit": "abc"}}, "def")


class RetryTests(unittest.TestCase):
    """A4: bounded retries for reads, bounded polls after a deployment."""

    def test_a_read_is_retried_with_back_off(self):
        calls, sleeps = [], []

        def read():
            calls.append(1)
            if len(calls) < 3:
                raise OSError("connection reset")
            return "0x1"

        self.assertEqual(dry_run.with_retry(read, attempts=5, sleep=sleeps.append), "0x1")
        self.assertEqual(sleeps, [1.0, 2.0])
        with self.assertRaises(OSError):
            dry_run.with_retry(lambda: (_ for _ in ()).throw(OSError("down")), attempts=2, sleep=sleeps.append)

    def test_a_missing_receipt_is_polled_not_judged(self):
        answers = iter([None, None, {"status": "0x1"}])
        self.assertEqual(dry_run.poll(lambda: next(answers), "receipt", seconds=5, sleep=lambda s: None),
                         {"status": "0x1"})
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.poll(lambda: None, "receipt", seconds=3, sleep=lambda s: None)


class RestartGuardTests(unittest.TestCase):
    """A11: a restarted run never sends; a recorded hash without a receipt is pending."""

    def state(self, *intents):
        data = {"plan": {"verifier": {"predicted": "0x" + "22" * 20}, "lock": {"predicted": "0x" + "44" * 20}},
                "preflight": {"verifier_runtime_code_hash": "0xc0de", "lock_runtime_code_hash": "0xc0de"},
                "deploy": {"lock": "0x" + "44" * 20}, "fleet": {"recipient": "pf" + "00" * 20}}
        return {"completed": ["plan", "fleet"], "data": data, "intents": list(intents)}

    def test_a_restart_reconciles_records_and_refuses(self):
        state = self.state({"stage": "deploy", "role": "verifier", "sender": "0x" + "11" * 20, "nonce": 0,
                            "hash": None, "outcome": None},
                           {"stage": "lock", "role": "lock", "sender": dry_run.HOLDER, "nonce": 3, "hash": "0xaa",
                            "outcome": "sent", "next_nonce_before": 0})
        chain = FakeChain(nonces={"0x" + "11" * 20: 1}, codes={"0x" + "22" * 20: "0xc0de"})
        with self.assertRaises(dry_run.RehearsalFailure) as caught:
            dry_run.reconcile_after_restart(state, chain)
        self.assertIn("start a fresh output directory", str(caught.exception))
        self.assertEqual(state["intents"][0]["reconciled"], "mined and matches the plan")
        self.assertIn("pending", state["intents"][1]["reconciled"])

    def test_a_pending_lock_or_approve_is_never_sent_again(self):
        intent = {"role": "lock", "hash": "0xaa", "next_nonce_before": 0}
        with self.assertRaises(dry_run.RehearsalFailure) as caught:
            dry_run.reconcile_lock(intent, FakeChain(next_nonce=0), lock="l", holder="0xh", amount=10,
                                   recipient_hash="0xr")
        self.assertIn("pending", str(caught.exception))
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.reconcile_approve({"role": "approve", "hash": "0xbb"}, FakeChain(allowance=0), owner="h",
                                      spender="l", amount=10)
        reverted = FakeChain(next_nonce=0, receipts={"0xaa": {"status": "0x0"}})
        self.assertEqual(dry_run.reconcile_lock(intent, reverted, lock="l", holder="0xh", amount=10,
                                                recipient_hash="0xr"), "send")
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.reconcile_lock(intent, FakeChain(next_nonce=0, receipts={"0xaa": {"status": "0x1"}}), lock="l",
                                   holder="0xh", amount=10, recipient_hash="0xr")


class RecordTests(unittest.TestCase):
    def test_the_rehearsal_lock_is_never_the_lanes_lock(self):
        for path in ("docs/runbooks/lane-base-sepolia-dry-run.md",
                     "docs/review/lane-base-sepolia-fork-rehearsal-20261110/README.md"):
            self.assertIn("must never be named as the lane's lock", (ROOT / path).read_text())


class ExactAllowanceTests(unittest.TestCase):
    """B1: the Lock leaves no allowance behind, also after an oversized or unlimited one."""

    UNLIMITED = 2 ** 256 - 1

    def settle(self, allowance):
        state, chain, sent = {"completed": [], "data": {}}, FakeChain(allowance=allowance), []

        def send(intent):
            sent.append(intent["role"])
            chain.allowance_value = 10
        dry_run.settle_allowance(state, chain, owner="h", spender="l", amount=10, send=send)
        return state, chain, sent

    def test_an_unlimited_or_larger_allowance_is_first_set_to_the_exact_amount(self):
        for allowance in (self.UNLIMITED, 11):
            state, chain, sent = self.settle(allowance)
            self.assertEqual(sent, ["reduce-allowance"])
            self.assertEqual([i["role"] for i in state["intents"]], ["approve", "reduce-allowance"])
            self.assertEqual(chain.allowance_value, 10)
            self.assertIn("adopted", state["intents"][0]["outcome"])

    def test_an_exact_allowance_is_adopted_and_nothing_is_sent(self):
        state, _, sent = self.settle(10)
        self.assertEqual(sent, [])
        self.assertIn("adopted", state["intents"][0]["outcome"])

    def test_a_smaller_allowance_is_approved(self):
        _, chain, sent = self.settle(3)
        self.assertEqual(sent, ["approve"])
        self.assertEqual(chain.allowance_value, 10)

    def test_after_the_lock_the_allowance_must_be_zero(self):
        dry_run.require_allowance_spent(FakeChain(allowance=0), owner="h", spender="l")
        for left in (self.UNLIMITED, 1):
            with self.assertRaises(dry_run.RehearsalFailure):
                dry_run.require_allowance_spent(FakeChain(allowance=left), owner="h", spender="l")

    def test_a_mined_lock_is_reconciled_before_any_further_approve(self):
        recipient = "pf-recipient"
        recipient_hash = "0x" + dry_run.keccak256(recipient.encode()).hex()
        state = {"completed": [], "data": {"deploy": {"lock": "l"}, "fleet": {"recipient": recipient}}}
        dry_run.begin_intent(state, stage="lock", role="approve", sender=dry_run.HOLDER, nonce=6, purpose="approve",
                             outcome="sent")
        dry_run.begin_intent(state, stage="lock", role="lock", sender=dry_run.HOLDER, nonce=7, purpose="Lock",
                             next_nonce_before=0)
        chain = FakeChain(allowance=0, next_nonce=1, records={0: (dry_run.HOLDER, 10, recipient_hash)})
        self.assertTrue(dry_run.lock_already_mined(state, chain, lock="l", holder=dry_run.HOLDER, amount=10,
                                                   recipient_hash=recipient_hash))
        self.assertFalse(dry_run.lock_already_mined({"intents": []}, chain, lock="l", holder=dry_run.HOLDER,
                                                    amount=10, recipient_hash=recipient_hash))
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.reconcile_after_restart(state, chain, amount=10)
        self.assertEqual(state["intents"][0]["reconciled"], "spent by the mined Lock")
        self.assertEqual(state["intents"][1]["reconciled"], "mined and matches the plan")


class FundingPlanTests(unittest.TestCase):
    """B2: one bounded funding plan for the four keys."""

    GAS = {"deploy-verifier": 3_000_000, "deploy-lock": 2_000_000, "approve": 50_000, "lock": 150_000}
    PRICE = 6_000_000

    def sheet(self, ceiling):
        return dry_run.deploy_sheet(plan=DeployAddressTests.PLAN, verifier_config={}, gas_used=self.GAS,
                                    gas_price_wei=self.PRICE, relayer_ceiling_wei=ceiling)

    def test_every_key_has_an_amount_and_its_formula(self):
        funding = self.sheet(5 * 10 ** 15)["funding"]
        verifier, lock = 3_000_000 * self.PRICE * 3, 2_000_000 * self.PRICE * 3
        own = (200_000 + 3 * 21_000) * self.PRICE * 3
        self.assertEqual(funding["verifier deployer"]["fund_wei"], verifier)
        self.assertEqual(funding["lock deployer"]["fund_wei"], lock)
        self.assertEqual(funding["relayer"]["fund_wei"], 5 * 10 ** 15)
        holder = funding["holder"]
        self.assertEqual(holder["own_gas_wei"], own)
        self.assertEqual(holder["passed_on_wei"], verifier + lock + 5 * 10 ** 15)
        self.assertEqual(holder["starting_balance_wei"], own + verifier + lock + 5 * 10 ** 15)
        for row in funding.values():
            self.assertTrue(row["formula"])
        markdown = dry_run.sheet_markdown(self.sheet(5 * 10 ** 15))
        self.assertIn("starting balance", markdown)
        self.assertIn("estimate", markdown)

    def test_without_a_relayer_ceiling_the_plan_says_it_is_incomplete(self):
        funding = self.sheet(None)["funding"]
        self.assertIsNone(funding["relayer"]["fund_wei"])
        self.assertIsNone(funding["holder"]["starting_balance_wei"])
        self.assertIn("--relayer-ceiling-wei", funding["relayer"]["formula"])

    def test_gas_times_price_is_called_an_estimate(self):
        note = self.sheet(1)["fee_note"]
        self.assertIn("estimate", note)
        self.assertIn("L1 data", note)

    def test_the_record_reads_each_keys_balance(self):
        keys = {"verifier deployer": "0xa", "lock deployer": "0xb", "holder": "0xc", "relayer": None}
        balances = dry_run.key_balances(keys, lambda address: {"0xa": 1, "0xb": 2, "0xc": 3}[address])
        self.assertEqual(balances["holder"], {"address": "0xc", "wei": 3})
        self.assertIn("not known", balances["relayer"]["wei"])

    def test_the_runbook_says_what_happens_to_what_is_left(self):
        text = (ROOT / "docs/runbooks/lane-base-sepolia-dry-run.md").read_text()
        for phrase in ("--relayer-ceiling-wei", "--leftover-eth", "returned to the holder", "kept on the"):
            self.assertIn(phrase, text)


class PayoutRecipientTests(unittest.TestCase):
    """B3: the default payout recipient has no key; a real run can name one."""

    def test_the_default_is_unchanged_and_an_operator_can_name_one(self):
        self.assertEqual(dry_run.payout_recipient(None, fork_block=5),
                         dry_run.RT.fresh_base_address("base-sepolia-dry-run-5"))
        self.assertEqual(dry_run.payout_recipient("0x" + "aB" * 20, fork_block=5), "0x" + "ab" * 20)
        for bad in ("0x12", "ab" * 20, "0x" + "zz" * 20):
            with self.assertRaises(dry_run.RehearsalFailure):
                dry_run.payout_recipient(bad, fork_block=5)

    def test_the_docs_say_the_default_payout_can_never_be_moved(self):
        for path in ("docs/runbooks/lane-base-sepolia-dry-run.md",
                     "docs/review/lane-base-sepolia-fork-rehearsal-20261110/README.md"):
            text = (ROOT / path).read_text()
            self.assertIn("can never be moved", text)
            self.assertIn("--payout-recipient", text)


def no_network():
    """Any RPC read or child process fails the test instead of reaching a network."""
    from unittest import mock

    def refuse(*args, **kwargs):
        raise AssertionError("the refusal must come before any read or child process")
    patches = [mock.patch("urllib.request.urlopen", refuse), mock.patch.object(dry_run.subprocess, "run", refuse),
               mock.patch.object(dry_run.ORDER, "git", refuse)]

    class Both:
        def __enter__(self):
            for patch in patches:
                patch.start()

        def __exit__(self, *exc):
            for patch in patches:
                patch.stop()
    return Both()


class RelayerCeilingPlanTests(unittest.TestCase):
    """N2: every real run carries a positive relayer ceiling in its plan from the start."""

    def test_every_real_run_needs_a_positive_ceiling_whatever_through_says(self):
        for ceiling in (None, 0, -1):
            with self.subTest(ceiling=ceiling), self.assertRaises(dry_run.RehearsalFailure):
                dry_run.resolve_relayer_ceiling(fork=False, requested=ceiling, plan=None)
        self.assertEqual(dry_run.resolve_relayer_ceiling(fork=False, requested=10 ** 15, plan=None), 10 ** 15)
        self.assertIsNone(dry_run.resolve_relayer_ceiling(fork=True, requested=None, plan=None))
        self.assertFalse(hasattr(dry_run, "check_relayer_ceiling"))

    def test_a_later_invocation_takes_the_plans_ceiling_or_names_the_same_one(self):
        plan = {"relayer_ceiling_wei": 10 ** 15}
        self.assertEqual(dry_run.resolve_relayer_ceiling(fork=False, requested=None, plan=plan), 10 ** 15)
        self.assertEqual(dry_run.resolve_relayer_ceiling(fork=False, requested=10 ** 15, plan=plan), 10 ** 15)
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.resolve_relayer_ceiling(fork=False, requested=2 * 10 ** 15, plan=plan)
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.resolve_relayer_ceiling(fork=False, requested=10 ** 15, plan={"relayer_ceiling_wei": None})

    def test_a_real_run_without_a_ceiling_stops_before_anything(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "run"
            with no_network(), self.assertRaises(dry_run.RehearsalFailure):
                dry_run.main(["--output", str(out), "--through", "export"])
            self.assertFalse(out.exists())


class PayoutRecipientPlanTests(unittest.TestCase):
    """N3: the payout recipient is checked, normalised and fixed in plan."""

    PLAN = DeployAddressTests.PLAN

    def test_known_contracts_and_the_zero_address_are_refused_in_plan(self):
        self.assertEqual(dry_run.plan_payout_recipient("0x" + "aB" * 20, fork_block=5, plan=self.PLAN),
                         "0x" + "ab" * 20)
        self.assertEqual(dry_run.plan_payout_recipient(None, fork_block=5, plan=self.PLAN),
                         dry_run.RT.fresh_base_address("base-sepolia-dry-run-5"))
        for bad in ("0x" + "00" * 20, self.PLAN["verifier"]["predicted"], self.PLAN["lock"]["predicted"].upper()
                    .replace("0X", "0x"), dry_run.TOKEN, "0x12"):
            with self.subTest(bad=bad), self.assertRaises(dry_run.RehearsalFailure):
                dry_run.plan_payout_recipient(bad, fork_block=5, plan=self.PLAN)

    def test_a_malformed_recipient_stops_before_anything(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "run"
            with no_network(), self.assertRaises(dry_run.RehearsalFailure):
                dry_run.main(["--output", str(out), "--fork", "--payout-recipient", "0x12"])
            self.assertFalse(out.exists())

    def test_a_later_invocation_keeps_the_plans_recipient(self):
        plan = {"payout_recipient": "0x" + "ab" * 20}
        self.assertEqual(dry_run.resolve_payout_recipient(None, plan=plan), "0x" + "ab" * 20)
        self.assertEqual(dry_run.resolve_payout_recipient("0x" + "AB" * 20, plan=plan), "0x" + "ab" * 20)
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.resolve_payout_recipient("0x" + "cd" * 20, plan=plan)

    def test_the_sheet_the_operator_confirms_names_the_recipient(self):
        sheet = dry_run.deploy_sheet(plan=self.PLAN, verifier_config={}, gas_used={}, gas_price_wei=1,
                                     relayer_ceiling_wei=10 ** 15, payout_recipient="0x" + "ab" * 20,
                                     payout_recipient_key="named by the operator")
        self.assertEqual(sheet["payout_recipient"], {"address": "0x" + "ab" * 20, "key": "named by the operator"})
        self.assertIn("0x" + "ab" * 20, dry_run.sheet_markdown(sheet))

