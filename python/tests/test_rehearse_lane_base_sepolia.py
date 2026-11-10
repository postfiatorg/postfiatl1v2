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
        self.assertEqual(needs["holder"]["gas"], 200_000)
        self.assertEqual(needs["lock deployer"]["wei_with_margin"], 2_000_000 * 6_000_000 * 3)
        markdown = dry_run.sheet_markdown(sheet)
        for text in ("0x" + "22" * 20, "0x" + "44" * 20, "programVKey", dry_run.SP1_VERIFIER["address"]):
            self.assertIn(text, markdown)


class IdentityTests(unittest.TestCase):
    def test_the_committed_elf_matches_the_committed_program_info(self):
        import json
        info = json.loads(dry_run.PROGRAM_INFO.read_text())
        self.assertEqual(dry_run.check_committed_identity(info, dry_run.COMMITTED_ELF), info["elf_sha256"])
        self.assertEqual(dry_run.parser().parse_args(["--output", "x"]).program_info, dry_run.PROGRAM_INFO)

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
