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
        self.assertEqual(identity["elf_sha256"], "8ac7696a5912eea37715b80798124baae0b81f8762d9351d1236737a980a1188")
        self.assertEqual(identity["checked_out_commit"], "abc")
        self.assertEqual(len(identity["runs"]), 3)

    def test_a_mismatching_elf_is_refused(self):
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.load_program_identity(fork=False, override=None, worktree_clean=True, head="abc",
                                          elf=dry_run.ROOT / "programs/pft-lane-export/Cargo.toml")

    def test_fork_mode_accepts_an_override(self):
        identity = dry_run.load_program_identity(fork=True, override=dry_run.PROGRAM_INFO, worktree_clean=False,
                                                 head="abc")
        self.assertEqual(identity["program_vkey"][:6], "0x00d5")


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
        allowance = dry_run.L1_DATA_FEE_ALLOWANCE_WEI
        verifier, lock = 3_000_000 * self.PRICE * 3 + allowance, 2_000_000 * self.PRICE * 3 + allowance
        own = (200_000 + 3 * 21_000) * self.PRICE * 3 + 5 * allowance
        relayer = 5 * 10 ** 15 + allowance
        self.assertEqual(funding["verifier deployer"]["fund_wei"], verifier)
        self.assertEqual(funding["lock deployer"]["fund_wei"], lock)
        self.assertEqual(funding["relayer"]["fund_wei"], relayer)
        holder = funding["holder"]
        self.assertEqual(holder["own_gas_wei"], own)
        self.assertEqual(holder["passed_on_wei"], verifier + lock + relayer)
        self.assertEqual(holder["starting_balance_wei"], own + verifier + lock + relayer)
        self.assertEqual(holder["formula"], "starting balance = own gas + verifier deployer + lock deployer + "
                                            "relayer (ceiling + one L1 data fee allowance)")
        for row in funding.values():
            self.assertTrue(row["formula"])
        markdown = dry_run.sheet_markdown(self.sheet(5 * 10 ** 15))
        self.assertIn("starting balance", markdown)
        self.assertIn("estimate", markdown)

    def test_the_relayer_row_recommends_the_ceiling_plus_one_allowance(self):
        allowance = dry_run.L1_DATA_FEE_ALLOWANCE_WEI
        sheet = self.sheet(5 * 10 ** 15)
        relayer = sheet["funding"]["relayer"]
        self.assertEqual((relayer["ceiling_wei"], relayer["surplus_wei"]), (5 * 10 ** 15, allowance))
        lines = dry_run.sheet_markdown(sheet).splitlines()
        funding_table = lines[lines.index("## Funding plan (wei)"):]
        row = next(line for line in funding_table if line.startswith("| relayer |"))
        print(f"\n{row}")
        self.assertEqual(row, (
            f"| relayer | {5 * 10 ** 15 + allowance} | ceiling 5000000000000000 wei (--relayer-ceiling-wei, a "
            "spending budget for this run, not a cap on the account's balance; the release's gas is not known "
            f"before a real proof) + one L1 data fee allowance {allowance} wei as surplus. Fund the relayer with "
            "this: the release never spends more than the ceiling, so the surplus is harmless slack for an L1 data "
            "fee above its allowance. The release is budgeted from the lesser of the relayer's balance and the "
            "ceiling, and sent with a gas limit and max fee whose product plus one L1 data fee allowance stays "
            "within it |"))
        self.assertNotIn("funded up to the ceiling", dry_run.sheet_markdown(sheet))

    def test_without_a_relayer_ceiling_the_plan_says_it_is_incomplete(self):
        funding = self.sheet(None)["funding"]
        self.assertIsNone(funding["relayer"]["fund_wei"])
        self.assertIsNone(funding["holder"]["starting_balance_wei"])
        self.assertIn("--relayer-ceiling-wei", funding["relayer"]["formula"])
        self.assertIn("not set: a fork run", funding["relayer"]["formula"])

    def test_the_sheet_prints_the_planned_release_spend_and_its_basis(self):
        planned = dry_run.planned_release_spend(self.PRICE)
        sheet = dry_run.deploy_sheet(plan=DeployAddressTests.PLAN, verifier_config={}, gas_used=self.GAS,
                                     gas_price_wei=self.PRICE, relayer_ceiling_wei=10 ** 16,
                                     release_planned_spend_wei=planned)
        self.assertEqual(sheet["release_planned_spend"]["wei"], planned)
        self.assertEqual(planned, 1_200_000 * self.PRICE * 3 + dry_run.L1_DATA_FEE_ALLOWANCE_WEI)
        line = next(line for line in dry_run.sheet_markdown(sheet).splitlines()
                    if line.startswith("Release planned spend"))
        print(f"\n{line}")
        self.assertEqual(line, (
            f"Release planned spend: {planned} wei = 1200000 gas (the planning figure 1000000, not a measurement, "
            f"plus a fifth) x the plan gas price {self.PRICE} wei x 3 (the gas guard) + one L1 data fee allowance "
            f"{dry_run.L1_DATA_FEE_ALLOWANCE_WEI} wei. The ceiling 10000000000000000 wei covers it; the release "
            "itself is quoted from the node's gas estimate."))

    def test_a_fork_run_without_a_ceiling_says_the_spend_was_not_planned(self):
        line = next(line for line in dry_run.sheet_markdown(self.sheet(None)).splitlines()
                    if line.startswith("Release planned spend"))
        self.assertIn("not planned", line)

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


class L1DataFeeTests(unittest.TestCase):
    """N4: the L1 data fee is its own line; the 3x margin is the execution part's."""

    GAS = FundingPlanTests.GAS
    PRICE = FundingPlanTests.PRICE

    def test_each_key_budgets_its_l1_data_fee_per_transaction(self):
        allowance = dry_run.L1_DATA_FEE_ALLOWANCE_WEI
        self.assertGreater(allowance, 0)
        funding = dry_run.deploy_sheet(plan=DeployAddressTests.PLAN, verifier_config={}, gas_used=self.GAS,
                                       gas_price_wei=self.PRICE, relayer_ceiling_wei=10 ** 15)["funding"]
        for key, gas, transactions in (("verifier deployer", 3_000_000, 1), ("lock deployer", 2_000_000, 1)):
            row = funding[key]
            self.assertEqual(row["execution_wei"], gas * self.PRICE * dry_run.GAS_MARGIN)
            self.assertEqual(row["l1_data_fee_wei"], transactions * allowance)
            self.assertEqual(row["fund_wei"], row["execution_wei"] + row["l1_data_fee_wei"])
        holder = funding["holder"]
        self.assertEqual(holder["own_l1_data_fee_wei"], 5 * allowance)
        self.assertEqual(holder["own_gas_wei"], (200_000 + 3 * 21_000) * self.PRICE * 3 + 5 * allowance)
        self.assertIn("L1 data fee", dry_run.sheet_markdown(dry_run.deploy_sheet(
            plan=DeployAddressTests.PLAN, verifier_config={}, gas_used=self.GAS, gas_price_wei=self.PRICE,
            relayer_ceiling_wei=10 ** 15)))

    def test_at_the_guard_execution_takes_the_margin_and_the_l1_line_is_left(self):
        gas, plan_price = 100, 10
        current = plan_price * dry_run.GAS_PRICE_FACTOR
        dry_run.check_gas_price(plan_wei=plan_price, current_wei=current)
        funding = dry_run.funding_plan({"verifier deployer": {"gas": gas, "steps": ["deploy-verifier"]},
                                        "lock deployer": {"gas": gas, "steps": ["deploy-lock"]},
                                        "holder": {"gas": gas, "steps": ["approve", "lock"]}},
                                       gas_price_wei=plan_price, relayer_ceiling_wei=1)
        row = funding["verifier deployer"]
        self.assertEqual(row["fund_wei"] - gas * current, dry_run.L1_DATA_FEE_ALLOWANCE_WEI)

    def test_the_text_says_what_the_numbers_do(self):
        for text in (dry_run.FEE_NOTE, (ROOT / "docs/runbooks/lane-base-sepolia-dry-run.md").read_text()):
            self.assertNotIn("covers it and a price rise", text)
            self.assertIn("L1 data fee", text)
            self.assertIn("allowance", text)

    def test_readiness_needs_the_l1_line_too(self):
        plan = DeployAddressTests.PLAN
        sheet = RealRunReadinessTests.SHEET
        allowance = dry_run.L1_DATA_FEE_ALLOWANCE_WEI
        values = dict(plan=plan, sheet=sheet, holder_address=dry_run.HOLDER, relayer_opens=True,
                      token_balance=lambda address: 10 ** 9, gas_price_wei=7, node_bin=Path(__file__),
                      driver_alive=lambda: True)
        with self.assertRaises(dry_run.RehearsalFailure):
            dry_run.check_real_run_ready(eth_balance=lambda address: 100 * 7 + allowance - 1, **values)



class ReleaseCapTests(unittest.TestCase):
    """N1: the relayer ceiling bounds what a release can spend."""

    CEILING = 10 ** 15
    LOCK = "0x" + "44" * 20
    RELAYER = "0x" + "55" * 20

    def chain(self, *, balance, estimate=500_000, price=10 ** 6):
        calls = []

        def read(method, params):
            calls.append(method)
            return hex({"eth_getBalance": balance, "eth_estimateGas": estimate, "eth_gasPrice": price}[method])
        return read, calls

    def quote(self, read):
        return dry_run.quote_release(ceiling_wei=self.CEILING, relayer=self.RELAYER, lock=self.LOCK,
                                     calldata="0x1234", read=read)

    def test_the_release_is_budgeted_from_the_lesser_of_balance_and_ceiling(self):
        for balance in (self.CEILING + 1, 10 * self.CEILING, self.CEILING // 2):
            with self.subTest(balance=balance):
                read, _ = self.chain(balance=balance)
                caps = self.quote(read)
                spendable = min(balance, self.CEILING)
                self.assertEqual(caps["budget_wei"], spendable)
                self.assertLessEqual(caps["gas_limit"] * caps["max_fee_per_gas_wei"]
                                     + dry_run.L1_DATA_FEE_ALLOWANCE_WEI, spendable)

    def test_a_quote_above_a_balance_below_the_ceiling_is_refused(self):
        read, _ = self.chain(balance=dry_run.L1_DATA_FEE_ALLOWANCE_WEI + 10, estimate=200_000, price=10 ** 6)
        with self.assertRaises(dry_run.RehearsalFailure) as refused:
            self.quote(read)
        self.assertIn("nothing was sent", str(refused.exception))

    def test_the_caps_keep_the_release_within_the_ceiling(self):
        read, _ = self.chain(balance=self.CEILING)
        caps = self.quote(read)
        bound = caps["gas_limit"] * caps["max_fee_per_gas_wei"] + dry_run.L1_DATA_FEE_ALLOWANCE_WEI
        self.assertLessEqual(bound, self.CEILING)
        self.assertEqual(caps["bound_wei"], bound)
        self.assertGreaterEqual(caps["gas_limit"], 500_000)
        self.assertGreaterEqual(caps["max_fee_per_gas_wei"], 10 ** 6)

    def test_a_quoted_cost_above_the_ceiling_is_refused_before_signing(self):
        read, calls = self.chain(balance=self.CEILING, price=10 ** 10)
        with self.assertRaises(dry_run.RehearsalFailure) as refused:
            self.quote(read)
        self.assertIn("nothing was sent", str(refused.exception))
        self.assertNotIn("cast send", calls)

    def test_the_send_carries_the_caps(self):
        read, _ = self.chain(balance=self.CEILING)
        caps = self.quote(read)
        argv = dry_run.relay_release_argv(rpc_url="http://base", lock=self.LOCK, input_dir=Path("in"),
                                          proof=Path("proof.json"), signer=["--account", "r"],
                                          sender=self.RELAYER, vkey="0x01", caps=caps)
        for flag, value in (("--gas-limit", caps["gas_limit"]), ("--max-fee-per-gas", caps["max_fee_per_gas_wei"])):
            self.assertEqual(argv[argv.index(flag) + 1], str(value))
        relay_args = RELAY.parser().parse_args(["release", "--rpc", "http://base", "--lock", self.LOCK,
                                                "--input", "in", "--proof", "proof.json",
                                                "--gas-limit", "7", "--max-fee-per-gas", "9"])
        self.assertEqual(RELAY.send_caps(relay_args, local_anvil=False),
                         ["--gas-limit", "7", "--gas-price", "9"])


RELAY = dry_run._load("pft_lane_export_relay", "scripts/pft-lane-export-relay.py")


def resume_release(test, *, account, balance, estimate=200_000, price=10 ** 6, ceiling=10 ** 15):
    """A resumed run's prove-and-release through main(), with every read and child
    command mocked: keystores are files under a temporary HOME (only their public
    `address` field exists), RPC reads answer from `balance`, `estimate` and `price`,
    and the relay is never run. Returns (refusal, rpc methods read, child argv)."""
    import io
    import json
    import os
    import subprocess
    import tempfile
    from unittest import mock

    tmp = Path(tempfile.mkdtemp())
    test.addCleanup(__import__("shutil").rmtree, tmp)
    keystores = tmp / ".foundry/keystores"
    keystores.mkdir(parents=True)
    for name, address in (("relayer-a", ResumedRelayerTests.A), ("relayer-b", ResumedRelayerTests.B)):
        (keystores / name).write_text(json.dumps({"address": address[2:], "crypto": {}, "id": name, "version": 3}))
    (keystores / "relayer-no-address").write_text(json.dumps({"crypto": {}, "id": "n", "version": 3}))
    out = tmp / "run"
    out.mkdir()
    recipient = "0x" + "ab" * 20
    state = {"mode": "base-sepolia", "completed": list(dry_run.STAGES[:dry_run.STAGES.index("prove-and-release")]),
             "data": {"commit": "c0ffee", "relayer_ceiling_wei": ceiling, "relayer_address": ResumedRelayerTests.A,
                      "plan": {"relayer_ceiling_wei": ceiling, "payout_recipient": recipient},
                      "deploy": {"lock": ResumedRelayerTests.LOCK},
                      "export": {"base_recipient": recipient, "input_sha256": "00", "public_values_sha256": "00"}}}
    (out / "state.json").write_text(json.dumps(state))
    proof = tmp / "proof.json"
    proof.write_text(json.dumps({"public_values_hex": "ab", "proof_hex": "0xcd"}))
    password = tmp / "password"
    password.write_text("never read")
    reads, children = [], []

    def urlopen(request, timeout=None):
        method = json.loads(request.data)["method"]
        reads.append(method)
        value = {"eth_getBalance": balance, "eth_estimateGas": estimate, "eth_gasPrice": price}[method]
        return io.BytesIO(json.dumps({"result": hex(value)}).encode())

    def run(argv, **kwargs):
        argv = [str(a) for a in argv]
        children.append(argv)
        if argv[:2] == ["git", "status"]:
            return subprocess.CompletedProcess(argv, 0, stdout="", stderr="")
        if argv[:2] == ["cast", "calldata"]:
            kwargs["stdout"].write("0x1234\n")
        elif argv[1].endswith("pft-lane-export-relay.py"):
            kwargs["stdout"].write(json.dumps({"sent": True, "transaction": "0x" + "77" * 32, "block": 9,
                                               "released": True}))
        else:
            raise AssertionError(f"unexpected child {argv}")
        return subprocess.CompletedProcess(argv, 0)

    refusal = None
    with mock.patch.dict(os.environ, {"HOME": str(tmp)}), mock.patch("urllib.request.urlopen", urlopen), \
            mock.patch.object(dry_run.subprocess, "run", run), mock.patch.object(dry_run.ORDER, "git",
                                                                                 lambda *a: "c0ffee"), \
            mock.patch.object(dry_run, "load_program_identity", lambda **k: {"program_vkey": "0x01"}):
        try:
            dry_run.main(["--output", str(out), "--proof", str(proof), "--through", "prove-and-release",
                          "--relayer-account", account, "--relayer-password-file", str(password)])
        except dry_run.RehearsalFailure as failure:
            refusal = failure
    return refusal, reads, children


def relay_dispatch(children):
    return [argv for argv in children if len(argv) > 1 and argv[1].endswith("pft-lane-export-relay.py")]


class ResumedRelayerTests(unittest.TestCase):
    """C1: the relayer whose balance and estimate are checked is the one that signs."""

    A = "0x" + "a1" * 20
    B = "0x" + "b2" * 20
    LOCK = "0x" + "44" * 20

    def test_a_resume_with_another_relayer_is_refused_before_anything_is_read_or_signed(self):
        refusal, reads, children = resume_release(self, account="relayer-b", balance=10 ** 15)
        self.assertIsNotNone(refusal, "the release went ahead with another relayer")
        self.assertEqual(type(refusal).__name__, "RelayerMismatch")
        self.assertIn(self.B, str(refusal))
        self.assertIn(self.A, str(refusal))
        self.assertNotIn("relayer-b", str(refusal))
        self.assertEqual(reads, [])
        self.assertEqual(relay_dispatch(children), [])

    def test_a_keystore_without_a_public_address_is_refused(self):
        refusal, reads, children = resume_release(self, account="relayer-no-address", balance=10 ** 15)
        self.assertIsNotNone(refusal)
        self.assertIn("relayer-address-unknown", str(refusal))
        self.assertEqual((reads, relay_dispatch(children)), ([], []))

    def test_the_recorded_relayer_is_quoted_and_bound_in_the_relay(self):
        refusal, reads, children = resume_release(self, account="relayer-a", balance=10 ** 15)
        self.assertIsNone(refusal)
        self.assertEqual(reads, ["eth_getBalance", "eth_estimateGas", "eth_gasPrice"])
        (argv,) = relay_dispatch(children)
        self.assertEqual(argv[argv.index("--expect-sender") + 1], self.A)
        self.assertEqual(argv[argv.index("--account") + 1], "relayer-a")

    def test_the_runbook_says_a_change_of_relayer_is_not_supported(self):
        text = (ROOT / "docs/runbooks/lane-base-sepolia-dry-run.md").read_text()
        self.assertIn("A change of relayer is not supported", text)
        self.assertIn("`address` field", text)


class ReleaseBudgetTests(unittest.TestCase):
    """C2: the ceiling is a spending budget for the run, checked in plan, not a cap on the balance."""

    CEILING = 10 ** 15

    def test_one_wei_above_the_ceiling_with_a_fitting_quote_is_released(self):
        refusal, reads, children = resume_release(self, account="relayer-a", balance=self.CEILING + 1,
                                                  estimate=200_000, price=10 ** 6, ceiling=self.CEILING)
        self.assertIsNone(refusal, f"a dust top-up refused the release late: {refusal}")
        (argv,) = relay_dispatch(children)
        gas_limit = int(argv[argv.index("--gas-limit") + 1])
        max_fee = int(argv[argv.index("--max-fee-per-gas") + 1])
        self.assertLessEqual(gas_limit * max_fee + dry_run.L1_DATA_FEE_ALLOWANCE_WEI, self.CEILING)

    def test_a_ceiling_that_cannot_fund_a_release_is_refused_before_anything(self):
        for ceiling in (1, dry_run.L1_DATA_FEE_ALLOWANCE_WEI):
            with self.subTest(ceiling=ceiling), self.assertRaises(dry_run.RehearsalFailure):
                dry_run.resolve_relayer_ceiling(fork=False, requested=ceiling, plan=None)

    def test_the_planned_spend_is_checked_against_the_ceiling(self):
        price = 10 ** 6
        planned = dry_run.planned_release_spend(price)
        gas_limit = -(-dry_run.RELEASE_GAS_PLANNED * 6 // 5)
        self.assertEqual(planned, gas_limit * price * dry_run.GAS_PRICE_FACTOR + dry_run.L1_DATA_FEE_ALLOWANCE_WEI)
        self.assertEqual(dry_run.check_release_budget(ceiling_wei=planned, gas_price_wei=price), planned)
        with self.assertRaises(dry_run.RehearsalFailure) as refused:
            dry_run.check_release_budget(ceiling_wei=planned - 1, gas_price_wei=price)
        self.assertIn("nothing was deployed", str(refused.exception))

    def test_plan_refuses_a_short_ceiling_before_any_fork_or_deployment(self):
        import io
        import json
        import subprocess
        import tempfile
        from unittest import mock
        tmp = Path(tempfile.mkdtemp())
        self.addCleanup(__import__("shutil").rmtree, tmp)
        price = 10 ** 9
        ceiling = dry_run.planned_release_spend(price) - 1
        reads = []

        def urlopen(request, timeout=None):
            method = json.loads(request.data)["method"]
            reads.append(method)
            value = {"eth_blockNumber": 16, "eth_gasPrice": price}[method]
            return io.BytesIO(json.dumps({"result": hex(value)}).encode())

        def no_fork(*args, **kwargs):
            raise AssertionError("a fork was started before the ceiling was checked")
        with mock.patch("urllib.request.urlopen", urlopen), \
                mock.patch.object(dry_run.subprocess, "run",
                                  lambda argv, **k: subprocess.CompletedProcess(argv, 0, stdout="", stderr="")), \
                mock.patch.object(dry_run.ORDER, "git", lambda *a: "c0ffee"), \
                mock.patch.object(dry_run, "BASE_LOCK_START", no_fork), \
                mock.patch.object(dry_run, "load_program_identity", lambda **k: {"program_vkey": "0x01"}), \
                self.assertRaises(dry_run.RehearsalFailure):
            dry_run.main(["--output", str(tmp / "run"), "--through", "plan", "--relayer-ceiling-wei", str(ceiling),
                          "--verifier-deployer", "0x" + "11" * 20, "--lock-deployer", "0x" + "22" * 20])
        self.assertEqual(reads, ["eth_blockNumber", "eth_gasPrice"])

    def test_the_runbook_says_what_the_ceiling_and_the_allowance_are(self):
        text = (ROOT / "docs/runbooks/lane-base-sepolia-dry-run.md").read_text()
        self.assertIn("a spending budget for this run, not a cap on the account's balance", text)
        self.assertIn("an estimate, not a hard limit", text)
        self.assertNotIn("refuses a relayer holding more than the ceiling", text)
