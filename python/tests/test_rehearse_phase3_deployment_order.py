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
LOCK_DEPLOYER = "0x" + "77" * 20


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
            rehearsal.refuse_unless_planned_nonce(DEPLOYER, {"latest": 0, "pending": 0}, 0, (self.verifier, self.lock)),
            (self.verifier, self.lock),
        )

    def test_the_lock_guard_passes_at_the_next_nonce(self):
        self.assertEqual(
            rehearsal.refuse_unless_planned_nonce(DEPLOYER, {"latest": 1, "pending": 1}, 1, (self.lock,)),
            (self.lock,),
        )

    def test_a_nonce_off_by_one_is_refused(self):
        with self.assertRaises(rehearsal.RehearsalFailure) as refusal:
            rehearsal.refuse_unless_planned_nonce(DEPLOYER, {"latest": 1, "pending": 1}, 0, (self.verifier, self.lock))
        message = str(refusal.exception)
        self.assertIn("refusing to deploy", message)
        self.assertIn(rehearsal.create_address(DEPLOYER, 1), message)
        self.assertIn(rehearsal.create_address(DEPLOYER, 2), message)

    def test_the_lock_guard_refuses_when_the_verifier_did_not_take_its_nonce(self):
        with self.assertRaises(rehearsal.RehearsalFailure):
            rehearsal.refuse_unless_planned_nonce(DEPLOYER, {"latest": 0, "pending": 0}, 1, (self.lock,))

    def test_a_pending_transaction_is_refused_even_when_latest_matches(self):
        with self.assertRaises(rehearsal.RehearsalFailure) as refusal:
            rehearsal.refuse_unless_planned_nonce(DEPLOYER, {"latest": 0, "pending": 1}, 0, (self.verifier, self.lock))
        message = str(refusal.exception)
        self.assertIn("pending", message)
        self.assertIn("refusing to deploy", message)

    def test_a_pending_count_below_latest_is_refused(self):
        with self.assertRaises(rehearsal.RehearsalFailure):
            rehearsal.refuse_unless_planned_nonce(DEPLOYER, {"latest": 1, "pending": 0}, 1, (self.lock,))

    def test_predictions_that_do_not_match_the_nonce_are_refused(self):
        with self.assertRaises(rehearsal.RehearsalFailure):
            rehearsal.refuse_unless_planned_nonce(DEPLOYER, {"latest": 0, "pending": 0}, 0, (self.lock, self.verifier))


class TwoDeployerPlanTests(unittest.TestCase):
    def setUp(self):
        self.plan = rehearsal.deployment_plan(verifier_deployer=DEPLOYER, verifier_nonce=0,
                                              lock_deployer=LOCK_DEPLOYER, lock_nonce=0)
        self.at_plan = {DEPLOYER: {"latest": 0, "pending": 0}, LOCK_DEPLOYER: {"latest": 0, "pending": 0}}

    def test_each_address_comes_from_its_own_deployer(self):
        self.assertEqual(self.plan["verifier"]["predicted"], rehearsal.create_address(DEPLOYER, 0))
        self.assertEqual(self.plan["lock"]["predicted"], rehearsal.create_address(LOCK_DEPLOYER, 0))
        status = rehearsal.refuse_unless_plan_intact(self.plan, self.at_plan)
        self.assertTrue(status["verifier"]["intact"] and status["lock"]["intact"])

    def test_one_deployer_for_both_is_refused(self):
        with self.assertRaises(rehearsal.RehearsalFailure):
            rehearsal.deployment_plan(verifier_deployer=DEPLOYER, verifier_nonce=0, lock_deployer=DEPLOYER,
                                      lock_nonce=1)

    def test_a_stray_transaction_on_one_deployer_leaves_the_other_intact_but_refuses_the_plan(self):
        for role, deployer, other in (("verifier", DEPLOYER, "lock"), ("lock", LOCK_DEPLOYER, "verifier")):
            with self.subTest(role=role):
                moved = dict(self.at_plan, **{deployer: {"latest": 1, "pending": 1}})
                status = rehearsal.nonce_plan_status(self.plan, moved)
                self.assertFalse(status[role]["intact"])
                self.assertEqual(status[role]["would_land_at"], rehearsal.create_address(deployer, 1))
                self.assertTrue(status[other]["intact"])
                self.assertEqual(status[other]["would_land_at"], self.plan[other]["predicted"])
                with self.assertRaises(rehearsal.RehearsalFailure) as refusal:
                    rehearsal.refuse_unless_plan_intact(self.plan, moved)
                self.assertIn(f"{role}:", str(refusal.exception))
                self.assertNotIn(f"{other}:", str(refusal.exception))

    def test_a_pending_transaction_on_either_deployer_refuses_the_plan(self):
        for deployer in (DEPLOYER, LOCK_DEPLOYER):
            with self.assertRaises(rehearsal.RehearsalFailure):
                rehearsal.refuse_unless_plan_intact(
                    self.plan, dict(self.at_plan, **{deployer: {"latest": 0, "pending": 1}}))


TOKEN = "0x" + "12" * 20
VERIFIER = "0x" + "ab" * 20
LOCK = "0x" + "cd" * 20


def fake_chain(verifier_values, lock_values):
    """`call_word` over a verifier and a lock deployed with the given values."""
    words = {(VERIFIER, f"{getter}()"): rehearsal.abi_word(verifier_values[field])
             for field, getter in rehearsal.VERIFIER_GETTERS.items()}
    words |= {(LOCK, f"{name}()"): rehearsal.abi_word(lock_values[name]) for name in rehearsal.LOCK_GETTERS}
    return lambda address, signature: words[(address, signature)]


class ConstructorReadBackTests(unittest.TestCase):
    def setUp(self):
        self.verifier_values = rehearsal.verifier_config(
            token=TOKEN, token_code_hash="0x" + "34" * 32, vault_code_hash="0x" + "56" * 32)
        self.lock_values = {"token": TOKEN, "finalityVerifier": VERIFIER, "maxLockAmount": 6_000_000,
                            "minimumLockAmount": 10}

    def check(self, chain):
        return rehearsal.check_constructor_values(chain, verifier=VERIFIER, verifier_values=self.verifier_values,
                                                  lock=LOCK, lock_values=self.lock_values)

    def test_every_constructor_value_is_read_back(self):
        read = self.check(fake_chain(self.verifier_values, self.lock_values))
        self.assertEqual(len(read), len(rehearsal.VERIFIER_CONFIG_FIELDS) + len(rehearsal.LOCK_GETTERS))
        for getter in ("programVKey", "pftlChainIdHash", "pftlGenesisHashCommitment", "pftlProtocolVersion",
                       "routeProfileHashCommitment", "routeEpoch", "assetIdCommitment", "maxProofBytes",
                       "maxPublicValuesBytes", "latestCommitteeRootCommitment", "sp1Verifier"):
            self.assertIn(f"verifier.{getter}()", read)
        self.assertIn("lock.maxLockAmount()", read)
        self.assertIn("lock.minimumLockAmount()", read)

    def test_a_swapped_route_profile_and_asset_id_fail(self):
        deployed = dict(self.verifier_values, routeProfileHashCommitment=self.verifier_values["assetIdCommitment"],
                        assetIdCommitment=self.verifier_values["routeProfileHashCommitment"])
        with self.assertRaises(rehearsal.RehearsalFailure) as refusal:
            self.check(fake_chain(deployed, self.lock_values))
        self.assertIn("routeProfileHashCommitment", str(refusal.exception))

    def test_a_swapped_max_proof_and_public_values_bound_fail(self):
        deployed = dict(self.verifier_values, maxProofBytes=self.verifier_values["maxPublicValuesBytes"],
                        maxPublicValuesBytes=self.verifier_values["maxProofBytes"])
        with self.assertRaises(rehearsal.RehearsalFailure):
            self.check(fake_chain(deployed, self.lock_values))

    def test_a_swapped_lock_max_and_minimum_fail(self):
        deployed = dict(self.lock_values, maxLockAmount=10, minimumLockAmount=6_000_000)
        with self.assertRaises(rehearsal.RehearsalFailure) as refusal:
            self.check(fake_chain(self.verifier_values, deployed))
        self.assertIn("maxLockAmount", str(refusal.exception))

    def test_the_tuple_follows_the_config_struct_order(self):
        source = (ROOT / "crates/ethereum-contracts/src/PFTLFinalityVerifierV1.sol").read_text()
        struct = source[source.index("struct Config {"):]
        struct = struct[:struct.index("}")]
        names = tuple(line.split()[-1].rstrip(";") for line in struct.splitlines()[1:] if line.strip())
        self.assertEqual(names, rehearsal.VERIFIER_CONFIG_FIELDS)
        fields = rehearsal.verifier_constructor_tuple(self.verifier_values)[1:-1].split(",")
        self.assertEqual(fields, [str(self.verifier_values[name]) for name in names])


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

    def test_the_lock_cap_is_the_lane_window_cap(self):
        fixture = json.loads((ROOT / rehearsal.GENESIS_FIXTURE).read_text())
        lane = rehearsal.lane(fixture)
        self.assertEqual(rehearsal.lane_window_cap(fixture), lane["reserved_atoms"] * lane["release_cap_bps"] // 10_000)
        self.assertEqual(rehearsal.refuse_unless_cap_matches(fixture, 6_000_000), 6_000_000)

    def test_a_lock_cap_other_than_the_window_cap_is_refused(self):
        fixture = json.loads((ROOT / rehearsal.GENESIS_FIXTURE).read_text())
        cap = rehearsal.lane_window_cap(fixture)
        with self.assertRaises(rehearsal.RehearsalFailure):
            rehearsal.refuse_unless_cap_matches(fixture, cap + 1)
        rehearsal.lane(fixture)["release_cap_bps"] += 1
        with self.assertRaises(rehearsal.RehearsalFailure):
            rehearsal.refuse_unless_cap_matches(fixture, cap)

    def test_a_window_cap_below_the_minimum_lock_is_refused(self):
        fixture = json.loads((ROOT / rehearsal.GENESIS_FIXTURE).read_text())
        rehearsal.lane(fixture)["reserved_atoms"] = 100
        with self.assertRaises(rehearsal.RehearsalFailure):
            rehearsal.lane_window_cap(fixture)

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

    def test_manifest_inventories_nested_files_recursively(self):
        with tempfile.TemporaryDirectory() as directory:
            out = Path(directory)
            (out / "summary.json").write_text("{}\n")
            (out / "manifest.json").write_text("{}\n")
            nested = out / "forge-out-x" / "A.sol"
            nested.mkdir(parents=True)
            (nested / "A.json").write_text("{\"a\": 1}\n")
            (out / "forge-cache-x").mkdir()
            (out / "forge-cache-x" / "summary.json").write_text("{}\n")
            manifest = rehearsal.build_manifest(out, "c" * 40, "t" * 40, {}, {})
        self.assertEqual(set(manifest["evidence_files"]), {"summary.json"})
        self.assertEqual(set(manifest["not_published"]), {"forge-out-x/A.sol/A.json", "forge-cache-x/summary.json"})
        self.assertEqual(manifest["not_published"]["forge-out-x/A.sol/A.json"]["bytes"], 9)

    def test_a_failed_run_records_what_landed(self):
        deployment = {"chain_id": 84532, "complete": False, "steps_completed": []}
        rehearsal.record_step(deployment, "token deployed", token=TOKEN)
        deployment["deployed"] = {"verifier": VERIFIER}
        rehearsal.record_step(deployment, "verifier deployed")
        results = {"passed": False, "error": "deploy-lock: exit 1", "deployment": deployment}
        with tempfile.TemporaryDirectory() as directory:
            out = Path(directory)
            manifest = rehearsal.write_records(out, results, "c" * 40, "t" * 40)
            summary = json.loads((out / "summary.json").read_text())
            written = json.loads((out / "manifest.json").read_text())
        for record in (summary["deployment"], written["deployment"], manifest["deployment"]):
            self.assertEqual(record["steps_completed"], ["token deployed", "verifier deployed"])
            self.assertEqual(record["deployed"], {"verifier": VERIFIER})
            self.assertFalse(record["complete"])
        self.assertEqual(summary["error"], "deploy-lock: exit 1")

    def test_build_provenance_names_the_host_and_rust_toolchain(self):
        provenance = rehearsal.build_provenance("dev (debug)")
        self.assertEqual(set(provenance), {"cargo", "rustc", "python", "host_os", "postfiat_node_build_profile"})
        self.assertEqual(provenance["postfiat_node_build_profile"], "dev (debug)")
        self.assertTrue(provenance["python"] and provenance["host_os"])

    def test_fixture_checkpoint_is_non_zero(self):
        for key in ("initialCheckpointCommitment", "initialCommitteeRootCommitment", "pftlGenesisHashCommitment"):
            self.assertNotEqual(int(rehearsal.FIXTURE_CHECKPOINT[key], 16), 0)
        self.assertGreater(rehearsal.FIXTURE_CHECKPOINT["initialFinalizedHeight"], 0)

    def test_output_inside_docs_is_refused(self):
        with self.assertRaises(rehearsal.RehearsalFailure):
            rehearsal.confined_output(ROOT / "docs" / "review" / "fresh-run-directory")


if __name__ == "__main__":
    unittest.main()
