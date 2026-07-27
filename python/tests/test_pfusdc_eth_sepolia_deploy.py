"""Offline tests for the auditor-gated Sepolia deployment entrypoint."""
from __future__ import annotations

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
SCRIPT = REPO / "scripts/pfusdc-eth-sepolia-deploy.py"
SPEC = importlib.util.spec_from_file_location("pfusdc_eth_sepolia_deploy", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
DEPLOY = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = DEPLOY
SPEC.loader.exec_module(DEPLOY)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


class SepoliaDeployAuthorizationTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.original_repository_root = DEPLOY.REPOSITORY_ROOT
        DEPLOY.REPOSITORY_ROOT = self.root
        self.manifest_path = self.root / "manifest.json"
        self.evidence_dir = self.root / "evidence"
        self.write_manifest()

    def tearDown(self):
        DEPLOY.REPOSITORY_ROOT = self.original_repository_root
        self.tmp.cleanup()

    def artifact(self, contract: str, nonce: int, address: str) -> dict:
        relative = f"artifacts/{contract}.json"
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        artifact = {
            "abi": [],
            "bytecode": {"object": "60006000"},
            "deployedBytecode": {"object": "6000"},
        }
        path.write_text(json.dumps(artifact), encoding="utf-8")
        return {
            "contract": contract,
            "address": address,
            "precomputed_create_nonce": nonce,
            "path": relative,
            "artifact_sha256": sha256(path),
            "creation_bytecode_keccak256": "0x" + hashlib.sha256(
                artifact["bytecode"]["object"].encode()
            ).hexdigest(),
            "unlinked_deployed_bytecode_keccak256": "0x" + hashlib.sha256(
                artifact["deployedBytecode"]["object"].encode()
            ).hexdigest(),
        }

    def write_manifest(self):
        self.manifest_path.write_text(json.dumps({
            "schema": "postfiat.pfusdc.eth_sepolia_deployment_manifest.v1",
            "revision": "test-generated-nonce67-68",
            "status": "generated-not-deployed",
            "deployer": {"address": "0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0"},
            "network": {
                "source_chain_id": 11155111,
                "execution_rpc_default": "https://example.invalid/sepolia-execution",
                "beacon_rpc_default": "https://example.invalid/sepolia-beacon",
                "token": {
                    "address": "0x1c7d4b196cb0c7b01d743fbc6116a902379c7238",
                    "runtime_code_hash": "0xcd3f29e2ea9c61dadd48bfeaf8b2884b6de9dfee7bf45329452c4c33d0868ceb",
                },
                "sp1_verifier_gateway": {
                    "address": "0x3b6041173b80e77f038f3f2c0f9744f04837185e",
                    "runtime_code_hash": "0xdcba737cf430260fdbc8010a56d97a9f29e64465155819e74d75da8f95eb24ed",
                },
            },
            "programs": {
                "egress": {"program_vkey": "0x" + "11" * 32},
                "max_proof_bytes": 4096,
                "max_public_values_bytes": 4096,
            },
            "pftl": {
                "chain_id_hash": "0x" + "12" * 32,
                "genesis_hash_commitment": "0x" + "13" * 32,
                "protocol_version": 1,
                "route_profile_hash_commitment": "0x" + "14" * 32,
                "route_epoch": 2,
                "asset_id_commitment": "0x" + "15" * 32,
                "vault_runtime_code_hash": "0x4229aaa000168f82d842313d81d867f0350a2c05e5d49eaa9654da84039e8727",
                "initial_checkpoint_commitment": "0x" + "16" * 32,
                "initial_finalized_height": 314,
                "initial_committee_root_commitment": "0x" + "17" * 32,
            },
            "route": {"vault_runtime_code_hash": "0x4229aaa000168f82d842313d81d867f0350a2c05e5d49eaa9654da84039e8727"},
            "contracts": {"artifacts": [
                self.artifact("PFTLFinalityVerifierV1", 67, "0x1111111111111111111111111111111111111111"),
                self.artifact("ERC20BridgeVaultL1", 68, "0x2222222222222222222222222222222222222222"),
            ]},
        }, indent=2) + "\n", encoding="utf-8")

    def authorization_document(self) -> dict:
        manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
        artifacts = {item["contract"]: item for item in manifest["contracts"]["artifacts"]}
        return {
            "schema": "postfiat.deployment_auditor_authorization.v1",
            "status": "PASS",
            "manifest_path": str(self.manifest_path.resolve()),
            "manifest_sha256": sha256(self.manifest_path),
            "route_revision": manifest["revision"],
            "chain_id": 11155111,
            "planned_deployments": {
                "verifier": {"nonce": artifacts["PFTLFinalityVerifierV1"]["precomputed_create_nonce"], "address": artifacts["PFTLFinalityVerifierV1"]["address"]},
                "vault": {"nonce": artifacts["ERC20BridgeVaultL1"]["precomputed_create_nonce"], "address": artifacts["ERC20BridgeVaultL1"]["address"]},
            },
            "auditor": {"identity": "test-auditor", "name": "Test Auditor"},
            "audit_artifact_sha256": "ab" * 32,
            "timestamp": "2026-07-26T00:00:00Z",
        }

    def write_authorization(self, document: dict | None = None) -> Path:
        path = self.root / "authorization.json"
        path.write_text(json.dumps(document or self.authorization_document(), indent=2) + "\n", encoding="utf-8")
        return path

    def invoke(self, *arguments: str) -> int:
        with mock.patch.object(sys, "argv", [str(SCRIPT), *arguments]):
            return DEPLOY.main()

    def test_check_only_is_offline_when_network_initializers_fail(self):
        with mock.patch.object(DEPLOY.SepoliaDeployer, "_load_web3", side_effect=AssertionError("web3")), \
             mock.patch.object(DEPLOY.SepoliaDeployer, "_load_agent_call", side_effect=AssertionError("agentd")):
            result = self.invoke("--manifest", str(self.manifest_path), "--check-only")
        self.assertEqual(result, 0)

    def test_missing_authorization_fails_before_network_or_sender(self):
        with mock.patch.object(DEPLOY.SepoliaDeployer, "initialize_network_clients", side_effect=AssertionError("network")) as initialize:
            result = self.invoke("--manifest", str(self.manifest_path))
        self.assertEqual(result, 1)
        initialize.assert_not_called()

    def test_invalid_authorizations_fail_before_network_or_sender(self):
        invalid = []
        wrong_digest = self.authorization_document(); wrong_digest["manifest_sha256"] = "00" * 32; invalid.append(wrong_digest)
        wrong_path = self.authorization_document(); wrong_path["manifest_path"] = "/wrong/manifest.json"; invalid.append(wrong_path)
        wrong_revision = self.authorization_document(); wrong_revision["route_revision"] = "wrong"; invalid.append(wrong_revision)
        wrong_chain = self.authorization_document(); wrong_chain["chain_id"] = 1; invalid.append(wrong_chain)
        wrong_nonce = self.authorization_document(); wrong_nonce["planned_deployments"]["verifier"]["nonce"] = 99; invalid.append(wrong_nonce)
        wrong_address = self.authorization_document(); wrong_address["planned_deployments"]["vault"]["address"] = "0x3333333333333333333333333333333333333333"; invalid.append(wrong_address)
        failed = self.authorization_document(); failed["status"] = "FAIL"; invalid.append(failed)
        for document in invalid:
            path = self.write_authorization(document)
            with self.subTest(document=document), mock.patch.object(DEPLOY.SepoliaDeployer, "initialize_network_clients", side_effect=AssertionError("network")) as initialize:
                result = self.invoke("--manifest", str(self.manifest_path), "--auditor-authorization", str(path))
            self.assertEqual(result, 1)
            initialize.assert_not_called()
        malformed = self.root / "malformed.json"
        malformed.write_text("{", encoding="utf-8")
        with mock.patch.object(DEPLOY.SepoliaDeployer, "initialize_network_clients", side_effect=AssertionError("network")) as initialize:
            result = self.invoke("--manifest", str(self.manifest_path), "--auditor-authorization", str(malformed))
        self.assertEqual(result, 1)
        initialize.assert_not_called()

    def test_exact_authorization_passes_gate_with_broadcast_mocked(self):
        authorization = self.write_authorization()
        with mock.patch.object(DEPLOY.SepoliaDeployer, "initialize_network_clients") as initialize, \
             mock.patch.object(DEPLOY.SepoliaDeployer, "run") as run:
            result = self.invoke(
                "--manifest", str(self.manifest_path),
                "--auditor-authorization", str(authorization),
                "--evidence-dir", str(self.evidence_dir),
            )
        self.assertEqual(result, 0)
        initialize.assert_called_once()
        run.assert_called_once()
        self.assertTrue(next(self.evidence_dir.glob("manifest.pre-broadcast.sha256-*.json")).is_file())

    def test_retained_prebroadcast_manifest_is_immutable_after_enrichment(self):
        authorization = self.write_authorization()
        document = json.loads(self.manifest_path.read_text(encoding="utf-8"))
        gate = DEPLOY.require_auditor_authorization(self.manifest_path, document, authorization, 11155111)
        retained = DEPLOY.retain_prebroadcast_manifest(self.manifest_path, self.evidence_dir, gate["manifest_sha256"])
        self.manifest_path.write_text('{"enriched":true}\n', encoding="utf-8")
        self.assertEqual(sha256(retained), gate["manifest_sha256"])
        self.assertNotEqual(sha256(self.manifest_path), gate["manifest_sha256"])

    def test_constructor_consumes_current_runtime_key_without_placeholder(self):
        document = json.loads(self.manifest_path.read_text(encoding="utf-8"))
        constructor = DEPLOY.build_verifier_constructor_inputs(document)
        self.assertEqual(
            constructor[9],
            document["pftl"]["vault_runtime_code_hash"],
        )

        def keys(value):
            if isinstance(value, dict):
                for key, child in value.items():
                    yield key
                    yield from keys(child)
            elif isinstance(value, list):
                for child in value:
                    yield from keys(child)

        self.assertFalse(any("placeholder" in key.lower() for key in keys(document)))


if __name__ == "__main__":
    unittest.main()
