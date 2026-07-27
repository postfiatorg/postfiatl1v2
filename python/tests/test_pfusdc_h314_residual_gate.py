from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "pfusdc-h314-residual-gate.py"
SPEC = importlib.util.spec_from_file_location("pfusdc_h314_residual_gate", SCRIPT)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class H314ResidualGateTests(unittest.TestCase):
    def test_nonce68_mapping_is_live_verified_from_rev6_immutable_input(self) -> None:
        config = MODULE.load_config(
            ROOT / "docs/evidence/pfusdc-eth-campaign-20260725/lane-b/conservation-h314/gate-config.json"
        )
        _, _, route = MODULE.rev6_route(config)
        mapping = MODULE.validate_live_verified_lineage(
            config, route, {"vault_interface_abi_class": "camel_case_v2"}
        )
        self.assertEqual(config["pins"]["vault_interface_lineage"], mapping)

    def test_missing_phase2_proof_fails_before_subprocess(self) -> None:
        config = ROOT / "docs/evidence/pfusdc-eth-campaign-20260725/lane-b/conservation-h314/gate-config.json"
        with mock.patch.object(sys, "argv", [str(SCRIPT), "--config", str(config), "--phase2-proof", "/tmp/no-h314-phase2-proof.json"]):
            with mock.patch.object(MODULE.subprocess, "run", side_effect=AssertionError("checker must not run before proof")):
                self.assertEqual(2, MODULE.main())

    def test_phase2_proof_requires_six_common_parents_and_negative_gate(self) -> None:
        config = {"target_height": 314, "phase2_proof_schema": "postfiat.pfusdc.h314_phase2_proof.v1"}
        route = {
            "route_id": "ethereum-sepolia-usdc-v1",
            "route_epoch": 2,
            "vault_address": "0x1111111111111111111111111111111111111111",
            "vault_runtime_code_hash": "0x" + "22" * 32,
        }
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            certificate = root / "certificate.json"
            certificate.write_text("{}", encoding="utf-8")
            negative = root / "negative.log"
            negative.write_text("PASS\n", encoding="utf-8")
            validators = []
            for index in range(6):
                evidence = root / f"validator-{index}.json"
                evidence.write_text("{}", encoding="utf-8")
                validators.append({
                    "validator_id": f"validator-{index}", "height": 314, "tip": "tip", "state_root": "root",
                    "evidence": {"path": str(evidence), "sha256": MODULE.sha256_file(evidence)},
                })
            proof = root / "phase2.json"
            proof.write_text(json.dumps({
                "schema": config["phase2_proof_schema"],
                "signed_artifact": {"signer": "Snaga", "certificate": {"path": str(certificate), "sha256": MODULE.sha256_file(certificate)}},
                "finalized": {"height": 314, "tip": "tip", "state_root": "root", "validators": validators},
                "active_route": {**route, "vault_interface_abi_class": "camel_case_v2"},
                "arbitrum_ingress_negative_gate": {"status": "PASS", "evidence": {"path": str(negative), "sha256": MODULE.sha256_file(negative)}},
            }), encoding="utf-8")
            accepted = MODULE.validate_phase2_proof(proof, config, route)
            self.assertEqual("camel_case_v2", accepted["vault_interface_abi_class"])
            validators[-1]["state_root"] = "other-root"
            proof.write_text(json.dumps({
                "schema": config["phase2_proof_schema"],
                "signed_artifact": {"signer": "Snaga", "certificate": {"path": str(certificate), "sha256": MODULE.sha256_file(certificate)}},
                "finalized": {"height": 314, "tip": "tip", "state_root": "root", "validators": validators},
                "active_route": {**route, "vault_interface_abi_class": "camel_case_v2"},
                "arbitrum_ingress_negative_gate": {"status": "PASS", "evidence": {"path": str(negative), "sha256": MODULE.sha256_file(negative)}},
            }), encoding="utf-8")
            with self.assertRaises(MODULE.GateError):
                MODULE.validate_phase2_proof(proof, config, route)
            validators[-1]["state_root"] = "root"
            proof.write_text(json.dumps({
                "schema": config["phase2_proof_schema"],
                "signed_artifact": {"signer": "Snaga", "certificate": {"path": str(certificate), "sha256": MODULE.sha256_file(certificate)}},
                "finalized": {"height": 314, "tip": "tip", "state_root": "root", "validators": validators},
                "active_route": {**route, "route_epoch": 3, "vault_interface_abi_class": "camel_case_v2"},
                "arbitrum_ingress_negative_gate": {"status": "PASS", "evidence": {"path": str(negative), "sha256": MODULE.sha256_file(negative)}},
            }), encoding="utf-8")
            with self.assertRaises(MODULE.GateError):
                MODULE.validate_phase2_proof(proof, config, route)
            proof.write_text(json.dumps({
                "schema": config["phase2_proof_schema"],
                "signed_artifact": {"signer": "Snaga", "certificate": {"path": str(certificate), "sha256": MODULE.sha256_file(certificate)}},
                "finalized": {"height": 314, "tip": "tip", "state_root": "root", "validators": validators},
                "active_route": {**route, "vault_interface_abi_class": "camel_case_v2"},
                "arbitrum_ingress_negative_gate": {"status": "FAIL", "evidence": {"path": str(negative), "sha256": MODULE.sha256_file(negative)}},
            }), encoding="utf-8")
            with self.assertRaises(MODULE.GateError):
                MODULE.validate_phase2_proof(proof, config, route)

    def test_exact_baseline_residual_releases_and_one_atom_delta_blocks(self) -> None:
        config = {"h310_baseline": {"components": {"V": "6000020", "S": "1000000", "D": "0", "B": "10", "R": "0"}, "rhs": "1000010", "residual_atoms": "5000010"}}
        proof = {"proof_path": "phase2.json", "proof_sha256": "a" * 64, "height": 314, "tip": "tip", "state_root": "root"}
        lineage = {"path": "lineage.json", "sha256": "b" * 64}
        with tempfile.TemporaryDirectory() as directory:
            identity = Path(directory) / "identity.json"
            document = {
                "height": 314, "tip": "tip", "state_root": "root",
                "components": {"V": "6000020", "S": "1000000", "D": "0", "B": "10", "R": "0"},
                "lhs": "6000020", "rhs": "1000010", "residual_atoms": "5000010",
                "source_rpc": {"raw_response_paths": [{"validator_id": f"validator-{index}"} for index in range(6)]},
                "snapshot": {"signed_import_transcript": "import.json", "verification_transcript": "verify.json"},
                "opening_bracket": {"residual_delta_from_h310": "0"},
            }
            identity.write_text(json.dumps(document), encoding="utf-8")
            self.assertEqual("H315_RELEASED", MODULE.exact_h314_verdict(identity, config, proof, lineage)["status"])
            document["residual_atoms"] = "5000011"
            document["opening_bracket"]["residual_delta_from_h310"] = "1"
            identity.write_text(json.dumps(document), encoding="utf-8")
            self.assertEqual("H315_BLOCKED", MODULE.exact_h314_verdict(identity, config, proof, lineage)["status"])


if __name__ == "__main__":
    unittest.main()
