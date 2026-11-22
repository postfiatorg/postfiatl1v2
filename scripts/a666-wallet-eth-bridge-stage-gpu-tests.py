#!/usr/bin/env python3
"""GPU readiness regressions; network and proof checks replaced by fixtures."""
import importlib.util
import os
from pathlib import Path
import unittest
from unittest.mock import Mock, patch


class GpuStageTests(unittest.TestCase):
    def setUp(self):
        spec = importlib.util.spec_from_file_location("gpu_adapter", Path(__file__).with_name("a666-wallet-eth-bridge-stage-gpu.py"))
        self.module = importlib.util.module_from_spec(spec)
        self.environment = {
            "A666_A100_HOST": "unused", "A666_VALIDATOR2_HOST": "unused",
            "B23_INGRESS_ROUTE_PROFILE_HASH": "ab" * 48, "B23_INGRESS_ROUTE_EPOCH": "11",
            "B23_INGRESS_CREDITED_ASSET_ID": "cd" * 48, "B23_INGRESS_ROUTE_BINDING": "ef" * 32,
            "B23_INGRESS_CAPTURE_BIN": "/unused/capture", "B23_INGRESS_PROVE_BIN": "/unused/prove",
            "B23_INGRESS_ELF": "/unused/guest", "B23_INGRESS_CAPTURE_SHA256": "11" * 32,
            "B23_INGRESS_PROVE_SHA256": "22" * 32, "PFTL_NODE_BIN": "/unused/node",
            "B23_PFTL_NODE_SHA256": "33" * 32, "PFTL_TOPOLOGY": "/unused/topology.json",
            "B23_INGRESS_NODE_VERIFY_BIN": "/unused/verify", "B23_INGRESS_NODE_VERIFY_SHA256": "44" * 32,
            "B23_INGRESS_GPU_PROOF_REPORT": "/unused/report", "B23_INGRESS_GPU_PROOF": "/unused/proof",
            "B23_INGRESS_GPU_PUBLIC_VALUES": "/unused/public",
        }
        with patch.dict(os.environ, self.environment):
            spec.loader.exec_module(self.module)

    def test_qualified_gpu_does_not_bypass_missing_anchor(self):
        with patch.object(self.module, "gpu_preflight"), patch.object(self.module.base, "live_beacon_anchor", side_effect=RuntimeError("anchor_missing")):
            result = self.module.readiness()
            self.assertFalse(result["ready"])
            self.assertTrue(result["prover_healthy"])
            self.assertFalse(result["beacon_anchor_authenticated"])

    def test_ready_requires_gpu_and_preserves_exact_anchor_identity(self):
        m = self.module
        anchor = {"route_profile_hash": m.base.ROUTE_PROFILE_HASH, "route_epoch": 11}
        with patch.object(m.base, "live_beacon_anchor", return_value=anchor), patch.object(m, "full_readiness", return_value={"ready": True}) as full:
            with patch.object(m, "gpu_preflight", side_effect=RuntimeError("gpu_unreachable")):
                result = m.readiness()
                self.assertFalse(result["ready"])
                self.assertFalse(result["prover_healthy"])
                full.assert_not_called()
            with patch.object(m, "gpu_preflight"):
                result = m.readiness()
                self.assertTrue(result["ready"])
                self.assertEqual(result["beacon_anchor_route_profile_hash"], m.base.ROUTE_PROFILE_HASH)
                self.assertEqual(result["beacon_anchor_route_epoch"], result["route_epoch"])

    def test_preflight_requires_live_gpu_and_real_node_verification(self):
        m = self.module
        report = {"program_vkey": m.base.PROGRAM_VKEY, "prover_backend": "cuda",
            "elf_sha256": m.base.A100_HASHES[m.base.A100_ELF], "proof_bytes": 356}
        with patch.dict(os.environ, self.environment), patch.object(m.base, "ssh") as ssh, \
             patch.object(m.base, "sha256_file", return_value="44" * 32), \
             patch.object(m.base, "read_json", return_value=report), \
             patch.object(m.base, "run", return_value=Mock(stdout='{"verified":false}')) as run:
            with self.assertRaisesRegex(RuntimeError, "rejected"):
                m.gpu_preflight()
            self.assertIn("nvidia-smi", ssh.call_args.args[1])
            self.assertEqual(run.call_args.args[0][-1], m.base.PROGRAM_VKEY)
            run.return_value.stdout = '{"verified":true,"program_vkey":"' + m.base.PROGRAM_VKEY + '"}'
            m.gpu_preflight()


if __name__ == "__main__":
    unittest.main()
