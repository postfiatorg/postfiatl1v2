"""Offline regression tests for the digest-gated chain-1 deployment driver."""
from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


REPO = Path(__file__).resolve().parents[2]
PYTHON_ROOT = REPO / "python"
if str(PYTHON_ROOT) not in sys.path:
    sys.path.insert(0, str(PYTHON_ROOT))
SCRIPT = REPO / "scripts/pfusdc-eth-mainnet-deploy.py"
SPEC = importlib.util.spec_from_file_location("pfusdc_eth_mainnet_deploy", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
DEPLOY = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = DEPLOY
SPEC.loader.exec_module(DEPLOY)

MANIFEST = REPO / "deployments/pfusdc-eth-mainnet-20260726/manifest.mainnet-epoch3.json"
AUTHORIZATION = REPO / "python/tests/fixtures/pfusdc-mainnet-epoch3-fallback-authorization.json"


def manifest_document() -> dict:
    return json.loads(MANIFEST.read_text(encoding="utf-8"))


class MainnetDeployTests(unittest.TestCase):
    def invoke(self, *arguments: str) -> int:
        with mock.patch.object(sys, "argv", [str(SCRIPT), *arguments]):
            return DEPLOY.main()

    def test_retired_epoch3_manifest_rejects_current_epoch4_artifacts_offline(self):
        with mock.patch.object(DEPLOY.MainnetDeployer, "_load_web3", side_effect=AssertionError("web3")), \
             mock.patch.object(DEPLOY.MainnetDeployer, "_load_agent_call", side_effect=AssertionError("agentd")):
            result = self.invoke(
                "--manifest", str(MANIFEST), "--auditor-authorization", str(AUTHORIZATION), "--check-only",
            )
        self.assertEqual(result, 1)

    def test_digest_failure_happens_before_network_or_sender(self):
        with tempfile.TemporaryDirectory() as directory:
            changed = Path(directory) / "manifest.json"
            changed.write_bytes(MANIFEST.read_bytes())
            with mock.patch.object(DEPLOY.MainnetDeployer, "initialize_network_clients", side_effect=AssertionError("network")) as initialize:
                result = self.invoke("--manifest", str(changed), "--auditor-authorization", str(AUTHORIZATION))
            self.assertEqual(result, 1)
            initialize.assert_not_called()

    def test_nonce_or_address_mismatch_fails_static_validation(self):
        with tempfile.TemporaryDirectory() as directory:
            changed = Path(directory) / "manifest.json"
            document = manifest_document()
            document["contracts"]["artifacts"][0]["address"] = "0x0000000000000000000000000000000000000000"
            changed.write_text(json.dumps(document), encoding="utf-8")
            digest = hashlib.sha256(changed.read_bytes()).hexdigest()
            with mock.patch.object(DEPLOY, "DEFAULT_MANIFEST", changed), \
                 mock.patch.object(DEPLOY, "REQUIRED_MANIFEST_SHA256", digest):
                with self.assertRaises(DEPLOY.DeploymentError):
                    DEPLOY.validate_offline_manifest(changed, DEPLOY._load_json(changed))

    def test_fallback_authorization_digest_mismatch_fails_locally(self):
        with tempfile.TemporaryDirectory() as directory:
            authorization = Path(directory) / "authorization.json"
            document = json.loads(AUTHORIZATION.read_text(encoding="utf-8"))
            document["authorized_manifest_sha256"] = "00" * 32
            authorization.write_text(json.dumps(document), encoding="utf-8")
            with self.assertRaises(DEPLOY.DeploymentError):
                DEPLOY.require_mainnet_authorization(MANIFEST, manifest_document(), authorization)

    def test_missing_authorization_and_credential_input_fail_before_network(self):
        with mock.patch.object(DEPLOY.MainnetDeployer, "initialize_network_clients", side_effect=AssertionError("network")) as initialize:
            missing = self.invoke("--manifest", str(MANIFEST))
            credential = self.invoke("--manifest", str(MANIFEST), "--credential-file", "/nonexistent")
            private_key = self.invoke("--manifest", str(MANIFEST), "--private-key-file", "/nonexistent")
        self.assertEqual(missing, 1)
        self.assertEqual(credential, 1)
        self.assertEqual(private_key, 1)
        initialize.assert_not_called()

    def test_missing_funding_reconciliation_blocks_before_network(self):
        with tempfile.TemporaryDirectory() as directory:
            absent = Path(directory) / "funding-reconciliation.json"
            with mock.patch.object(DEPLOY.MainnetDeployer, "initialize_network_clients", side_effect=AssertionError("network")) as initialize:
                result = self.invoke(
                    "--manifest", str(MANIFEST), "--auditor-authorization", str(AUTHORIZATION),
                    "--gas-preflight", str(absent),
                )
            self.assertEqual(result, 1)
            initialize.assert_not_called()

    def test_wrong_scope_values_fail_closed(self):
        for path, value in (
            (("network", "source_chain_id"), 11155111),
            (("network", "token", "address"), "0x0000000000000000000000000000000000000000"),
            (("network", "sp1_verifier_gateway", "address"), "0x0000000000000000000000000000000000000000"),
            (("route", "route_epoch"), 2),
        ):
            with self.subTest(path=path):
                document = copy.deepcopy(manifest_document())
                target = document
                for key in path[:-1]:
                    target = target[key]
                target[path[-1]] = value
                with self.assertRaises(DEPLOY.DeploymentError):
                    DEPLOY._validate_scope(document)

    def test_pinned_nonce_and_addresses_derive_exactly(self):
        document = manifest_document()
        artifacts = {entry["contract"]: entry for entry in document["contracts"]["artifacts"]}
        deployer = document["deployer"]["address"]
        self.assertEqual(DEPLOY.create_address(deployer, 153), artifacts["PFTLFinalityVerifierV1"]["address"])
        self.assertEqual(DEPLOY.create_address(deployer, 154), artifacts["ERC20BridgeVaultL1"]["address"])
        self.assertNotEqual(DEPLOY.create_address(deployer, 155), artifacts["ERC20BridgeVaultL1"]["address"])

    def test_budget_halts_before_send_and_after_hard_ceiling(self):
        with tempfile.TemporaryDirectory() as directory:
            reconciliation = Path(directory) / "funding-reconciliation.json"
            reconciliation.write_text(json.dumps({
                "schema": DEPLOY.BudgetGate.SCHEMA,
                "eth_usd_basis": {"usd_per_eth": 2000, "source": "offline-test"},
                "projected_remaining_gas_usd": {"verifier": 70, "vault": 70},
            }), encoding="utf-8")
            gate = DEPLOY.BudgetGate(reconciliation)
            with self.assertRaises(DEPLOY.DeploymentError):
                gate.require_before_send(["verifier", "vault"])
            reconciliation.write_text(json.dumps({
                "schema": DEPLOY.BudgetGate.SCHEMA,
                "eth_usd_basis": {"usd_per_eth": 2000, "source": "offline-test"},
                "projected_remaining_gas_usd": {"verifier": 30, "vault": 30},
            }), encoding="utf-8")
            gate = DEPLOY.BudgetGate(reconciliation)
            gate.require_before_send(["verifier", "vault"])
            receipt = {"gasUsed": 50_000_000, "effectiveGasPrice": 1_500_000_000}
            gate.record_receipt("verifier", receipt)
            with self.assertRaises(DEPLOY.DeploymentError):
                gate.require_after_receipt()


if __name__ == "__main__":
    unittest.main()
