#!/usr/bin/env python3
"""CPU adapter regressions; no chain writes, network calls or proving."""
import importlib.util
import io
import json
import os
from pathlib import Path
from tempfile import TemporaryDirectory
import time
import unittest
from unittest.mock import Mock, patch
from contextlib import ExitStack


class CpuStageTests(unittest.TestCase):
    def setUp(self):
        path = Path(__file__).with_name("a666-wallet-eth-bridge-stage-cpu.py")
        spec = importlib.util.spec_from_file_location("cpu_stages", path)
        self.module = importlib.util.module_from_spec(spec)
        with patch.dict(os.environ, {
            "A666_A100_HOST": "unused", "A666_VALIDATOR2_HOST": "unused",
            "B23_INGRESS_ROUTE_PROFILE_HASH": "ab" * 48, "B23_INGRESS_ROUTE_EPOCH": "11",
            "B23_INGRESS_CREDITED_ASSET_ID": "cd" * 48, "B23_INGRESS_ROUTE_BINDING": "ef" * 32,
            "B23_INGRESS_CAPTURE_BIN": "/unused/capture", "B23_INGRESS_PROVE_BIN": "/unused/prove",
            "B23_INGRESS_ELF": "/unused/guest", "B23_INGRESS_CAPTURE_SHA256": "11" * 32,
            "B23_INGRESS_PROVE_SHA256": "22" * 32, "PFTL_NODE_BIN": "/unused/node",
            "B23_PFTL_NODE_SHA256": "33" * 32, "PFTL_TOPOLOGY": "/unused/topology.json",
            "B23_INGRESS_CPU_QUALIFICATION": "/unused/qualification.json",
        }):
            spec.loader.exec_module(self.module)

    def readiness(self, missing_anchor=False, qualified=True, relay=True, tamper=False):
        module, base = self.module, self.module.base
        qualification = {"node_verifier_accepted": qualified, "program_vkey": base.PROGRAM_VKEY,
            "elf_sha256": base.A100_HASHES[base.A100_ELF], "proof_file": "/proof",
            "proof_sha256": "proof-hash", "public_values_file": "/public", "public_values_sha256": "public-hash"}
        hashes = {**base.A100_HASHES, "/proof": "proof-hash", "/public": "changed" if tamper else "public-hash"}
        web3 = Mock()
        web3.eth.contract.return_value.functions.paused.return_value.call.return_value = False
        slot = int((time.time() - 1606824023) // 12) - 64
        anchor = {"route_profile_hash": base.ROUTE_PROFILE_HASH, "route_epoch": base.ROUTE_EPOCH}
        with ExitStack() as stack:
            for name, options in {
                "live_route": {"return_value": {}},
                "live_beacon_anchor": {"side_effect": RuntimeError("anchor_missing")} if missing_anchor else {"return_value": anchor},
                "sha256_file": {"side_effect": lambda path: hashes[str(path)]},
                "read_json": {"return_value": qualification}, "checked_web3": {"return_value": web3},
                "ssh": {"return_value": ""} if relay else {"side_effect": RuntimeError("unavailable")},
            }.items():
                stack.enter_context(patch.object(base, name, **options))
            stack.enter_context(patch.object(module.urllib.request, "urlopen", return_value=io.StringIO(json.dumps({"data": {"header": {"message": {"slot": slot}}}}))))
            return module.readiness()

    def test_ready_requires_anchor_real_proof_qualification_and_relay(self):
        self.assertTrue(self.readiness()["ready"])
        for changes in ({"missing_anchor": True}, {"qualified": False}, {"relay": False}, {"tamper": True}):
            with self.subTest(changes=changes):
                self.assertFalse(self.readiness(**changes)["ready"])

    def test_local_proof_uses_cpu_and_rejects_mismatched_cached_report(self):
        base = self.module.base
        with TemporaryDirectory() as directory:
            path = Path(directory)
            report = {"program_vkey": base.PROGRAM_VKEY, "prover_backend": "cpu",
                "elf_sha256": base.A100_HASHES[base.A100_ELF], "host_execute_skipped": False, "proof_bytes": 356}
            with patch.object(base, "read_json", return_value=report), patch.object(base, "run") as run:
                self.module.prove({}, path, "job")
                self.assertEqual(run.call_args.kwargs["env"]["SP1_PROVER"], "cpu")
                self.assertEqual(run.call_args.args[0][-2:], ["--require-prover", "cpu"])
            (path / "proof-cuda").mkdir()
            (path / "proof-cuda/proof-report.json").touch()
            for field, value in (("program_vkey", "wrong"), ("prover_backend", "mock"), ("host_execute_skipped", True)):
                with patch.object(base, "read_json", return_value={**report, field: value}), patch.object(base, "run") as run:
                    with self.assertRaisesRegex(RuntimeError, "differs"):
                        self.module.prove({}, path, "job")
                    run.assert_not_called()

    def test_stale_artifacts_are_preserved_and_stage_hooks_use_local_cpu(self):
        with TemporaryDirectory() as directory:
            path = Path(directory)
            for name in ("ingress", "proof-cuda"):
                (path / name).mkdir()
                (path / name / "artifact").write_text("existing")
            self.module.archive(path, "job")
            self.assertFalse((path / "ingress").exists())
            archived = next(path.glob("stale-*"))
            self.assertEqual((archived / "proof-cuda/artifact").read_text(), "existing")
            self.assertIs(self.module.base.capture_once, self.module.capture)
            self.assertIs(self.module.base.ensure_proof, self.module.prove)


if __name__ == "__main__":
    unittest.main()
