#!/usr/bin/env python3
"""Focused adapter regressions; all chain/prover calls are replaced with fixtures."""
import importlib.util
import os
from pathlib import Path
import sys
import types
import unittest
from unittest.mock import patch


class IngressStageTests(unittest.TestCase):
    def adapters(self, environment=None):
        for name in ("a666-wallet-eth-bridge-stage.py", "a666-wallet-eth-bridge-stage-serialized.py"):
            spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name))
            module = importlib.util.module_from_spec(spec)
            web3 = types.ModuleType("web3")
            web3.Web3 = object
            with patch.dict(os.environ, {
                "A666_A100_HOST": "unused", "A666_VALIDATOR2_HOST": "unused",
                "B23_INGRESS_ROUTE_PROFILE_HASH": "ab" * 48, "B23_INGRESS_ROUTE_EPOCH": "11",
                "B23_INGRESS_CREDITED_ASSET_ID": "cd" * 48, "B23_INGRESS_ROUTE_BINDING": "ef" * 32,
                "B23_INGRESS_CAPTURE_BIN": "/unused/capture", "B23_INGRESS_PROVE_BIN": "/unused/prove",
                "B23_INGRESS_ELF": "/unused/guest", "B23_INGRESS_CAPTURE_SHA256": "11" * 32,
                "B23_INGRESS_PROVE_SHA256": "22" * 32, "PFTL_NODE_BIN": "/unused/node",
                "B23_PFTL_NODE_SHA256": "33" * 32, "PFTL_TOPOLOGY": "/unused/topology.json",
                **(environment or {}),
            }), patch.dict(sys.modules, {"web3": web3}):
                spec.loader.exec_module(module)
            yield module

    def anchor(self, module):
        return {
            "route_profile_hash": module.ROUTE_PROFILE_HASH, "route_epoch": 11,
            "source_chain_id": 1, "vault_address": module.VAULT,
            "token_address": module.TOKEN, "route_binding": module.ROUTE_BINDING[2:],
            "finalized_beacon_root": "11" * 32, "finalized_slot": 100,
            "current_sync_committee_root": "22" * 32,
        }

    def test_missing_anchor_fails_closed(self):
        for module in self.adapters():
            with self.subTest(adapter=module.__name__), patch.object(module, "live_route", return_value={"profile": {"route_epoch": 11}}):
                with self.assertRaisesRegex(RuntimeError, "anchor_missing"):
                    module.live_beacon_anchor()

    def test_capture_reads_each_new_ledger_anchor_and_rejects_stale_witness(self):
        for module in self.adapters():
            first = self.anchor(module)
            second = {**first, "finalized_beacon_root": "33" * 32, "finalized_slot": 132}
            responses = [{"profile": {"route_epoch": 11}, "beacon_finality_anchor": a} for a in (first, second)]
            with self.subTest(adapter=module.__name__), patch.object(module, "live_route", side_effect=responses):
                self.assertEqual(module.live_beacon_anchor(), first)
                self.assertEqual(module.live_beacon_anchor(), second)
            old = {"prior_finalized_beacon_root": first["finalized_beacon_root"],
                   "prior_finalized_slot": 100, "prior_sync_committee_root": first["current_sync_committee_root"], "route_epoch": 11}
            module.require_witness_anchor(old, first)
            with self.assertRaisesRegex(RuntimeError, "recapture_required"):
                module.require_witness_anchor(old, second)

    def test_wrong_epoch_and_committee_are_rejected(self):
        for module in self.adapters():
            anchor = self.anchor(module)
            with patch.object(module, "live_route", return_value={"profile": {"route_epoch": 10}, "beacon_finality_anchor": anchor}):
                with self.assertRaisesRegex(RuntimeError, "anchor_invalid"):
                    module.live_beacon_anchor()
            public = {"prior_finalized_beacon_root": anchor["finalized_beacon_root"],
                      "prior_finalized_slot": 100, "prior_sync_committee_root": "ff" * 32, "route_epoch": 11}
            with self.assertRaisesRegex(RuntimeError, "recapture_required"):
                module.require_witness_anchor(public, anchor)

    def test_credited_asset_matches_observed_ops_source_series(self):
        route = {
            "chain_id": "postfiat-wan-devnet-2",
            "profile_hash": "b2ca6cda737d68955b951deb223018b850b8d0ed6b4a2f1c451bd0eff9da76060d5a6eec3d46618616e630ef67859211",
            "profile": {"asset_id": "02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b",
                "source_chain_id": 1, "route_epoch": 10,
                "vault_address": "0xe7a432a28b20621a70e845c7f202004cf98e7a2c",
                "token_address": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"}
        }
        for module in self.adapters():
            self.assertEqual(module.credited_asset_for_route(route),
                "2bae082a6703375b9405af44715e1e64623265392627767b040fa2c30abb100a09da403724a6f105317292d9c0073df7")

    def test_claim_does_not_report_success_without_credited_balance_delta(self):
        from tempfile import TemporaryDirectory
        import json
        for module in self.adapters():
            with TemporaryDirectory() as directory:
                job_dir = Path(directory)
                (job_dir / "pftl-baseline.json").write_text(json.dumps({"balance_atoms": 20}))
                request = {"pftl_recipient": "pf-test", "amount_atoms": "10"}
                with patch.object(module, "load_job", return_value=({}, request, job_dir, "job")), \
                     patch.object(module, "common", return_value={}), \
                     patch.object(module, "pftl_deposit", return_value={"status": "finalized", "evidence_root": "root"}), \
                     patch.object(module, "pftl_receipt_for_evidence", return_value={"status": "counted", "amount_atoms": 10, "receipt_id": "receipt"}), \
                     patch.object(module, "pftl_balance", return_value=20):
                    with self.assertRaisesRegex(RuntimeError, "balance delta"):
                        module.execute_stage("claiming", job_dir / "job.json")


    def test_stale_proof_is_recaptured_and_reproved_for_same_deposit(self):
        from tempfile import TemporaryDirectory
        import json
        for module in self.adapters():
            with self.subTest(adapter=module.__name__), TemporaryDirectory() as directory:
                job_dir = Path(directory)
                first = self.anchor(module)
                second = {**first, "finalized_beacon_root": "33" * 32, "finalized_slot": 132}
                def values(anchor, evidence):
                    return {"prior_finalized_beacon_root": anchor["finalized_beacon_root"],
                        "prior_finalized_slot": anchor["finalized_slot"], "prior_sync_committee_root": anchor["current_sync_committee_root"],
                        "route_epoch": 11, "evidence_root": evidence, "deposit_nullifier": "aa" * 32,
                        "finalized_execution_block_hash": "bb" * 32, "finalized_execution_block_number": 123}
                old, new = values(first, "old"), values(second, "new")
                module.write_json(job_dir / "ingress/witness.public-values.json", old)
                module.write_json(job_dir / "proof-cuda/proof-report.json", {"old": True})
                immutable = {"deposit_id": "deposit", "deposit_tx_hash": "tx", "nonce": "nonce"}
                module.write_json(job_dir / "deposit-result.json", immutable)
                module.write_json(job_dir / "pftl-baseline.json", {"balance_atoms": 20})
                def capture(request, directory, key):
                    self.assertEqual(request, immutable)
                    self.assertEqual(module.read_json(directory / "deposit-result.json"), immutable)
                    self.assertFalse((directory / "proof-cuda/proof-report.json").exists())
                    module.write_json(directory / "ingress/witness.public-values.json", new)
                    module.write_json(directory / "ingress/witness.json", {"fresh": True})
                    return new
                def prove_fresh(request, directory, key):
                    for name in ("proof-report.json", "proof-calldata.bin", "public-values.bin"):
                        module.write_json(directory / "proof-cuda" / name, {"fresh": True})
                with patch.object(module, "load_job", return_value=({}, immutable, job_dir, "same-job")), \
                     patch.object(module, "common", return_value={}), \
                     patch.object(module, "live_beacon_anchor", return_value=second), \
                     patch.object(module, "pftl_deposit", side_effect=[None, {"status": "pending", "evidence_root": "new"}]), \
                     patch.object(module, "ensure_capture", side_effect=capture) as recapture, \
                     patch.object(module, "ensure_proof", side_effect=prove_fresh) as prove, \
                     patch.object(module, "relay_phase", return_value={}) as relay, \
                     patch.object(module, "confirmed_deposit") as deposit, \
                     patch.object(module, "ssh", return_value="") as remote:
                    if hasattr(module, "A100_PROVER_LOCK"):
                        module.A100_PROVER_LOCK = job_dir / "prover.lock"
                    result = module.execute_stage("verifying", job_dir / "job.json")
                    self.assertTrue(result["proof_verified"])
                    self.assertEqual(result["evidence_root"], "new")
                    self.assertEqual(result["proof_sha256"], module.sha256_file(job_dir / "proof-cuda/proof-calldata.bin"))
                    recapture.assert_called_once_with(immutable, job_dir, "same-job")
                    prove.assert_called_once_with(immutable, job_dir, "same-job")
                    relay.assert_called_once_with("propose", immutable, job_dir, "same-job")
                    deposit.assert_not_called()
                    self.assertIn("/same-job/", remote.call_args.args[1])
                self.assertEqual(module.read_json(job_dir / "deposit-result.json"), immutable)
                self.assertEqual(module.read_json(job_dir / "pftl-baseline.json"), {"balance_atoms": 20})
                archive = list(job_dir.glob("stale-*/ingress/witness.public-values.json"))
                self.assertEqual(len(archive), 1)
                self.assertEqual(module.read_json(archive[0]), old)

    def test_anchor_race_during_proposal_recovers_and_unrelated_reject_does_not(self):
        for module in self.adapters():
            anchor = self.anchor(module)
            old = {"prior_finalized_beacon_root": "00" * 32, "prior_finalized_slot": 99,
                "prior_sync_committee_root": anchor["current_sync_committee_root"], "route_epoch": 11}
            fresh = {**old, "prior_finalized_beacon_root": anchor["finalized_beacon_root"], "prior_finalized_slot": anchor["finalized_slot"]}
            accepted = {"status": "pending", "evidence_root": "fresh"}
            with patch.object(module, "refresh_stale_proof", side_effect=[old, fresh]) as refresh, \
                 patch.object(module, "relay_phase", side_effect=[RuntimeError("stale receipt"), {}]) as relay, \
                 patch.object(module, "pftl_deposit", side_effect=[None, accepted]), \
                 patch.object(module, "live_beacon_anchor", return_value=anchor):
                self.assertEqual(module.propose_current_proof({}, Path("unused"), "same-job"), (fresh, accepted))
                self.assertEqual(refresh.call_count, 2)
                self.assertEqual(relay.call_count, 2)
            with patch.object(module, "refresh_stale_proof", return_value=fresh), \
                 patch.object(module, "relay_phase", side_effect=RuntimeError("unrelated reject")) as relay, \
                 patch.object(module, "pftl_deposit", return_value=None), \
                 patch.object(module, "live_beacon_anchor", return_value=anchor):
                with self.assertRaisesRegex(RuntimeError, "unrelated reject"):
                    module.propose_current_proof({}, Path("unused"), "same-job")
                relay.assert_called_once()

    def test_interrupted_recovery_resumes_capture_and_missing_proof(self):
        from tempfile import TemporaryDirectory
        for module in self.adapters():
            for captured in (False, True):
                with self.subTest(adapter=module.__name__, captured=captured), TemporaryDirectory() as directory:
                    job_dir = Path(directory)
                    anchor = self.anchor(module)
                    public = {"prior_finalized_beacon_root": anchor["finalized_beacon_root"],
                        "prior_finalized_slot": anchor["finalized_slot"], "prior_sync_committee_root": anchor["current_sync_committee_root"], "route_epoch": 11}
                    if captured:
                        module.write_json(job_dir / "ingress/witness.public-values.json", public)
                    def capture(request, directory, key):
                        module.write_json(directory / "ingress/witness.public-values.json", public)
                    with patch.object(module, "ensure_capture", side_effect=capture) as recapture, \
                         patch.object(module, "prove_with_lock") as prove, \
                         patch.object(module, "live_beacon_anchor", return_value=anchor):
                        self.assertEqual(module.refresh_stale_proof({}, job_dir, "same-job"), public)
                        self.assertEqual(recapture.call_count, 0 if captured else 1)
                        prove.assert_called_once_with({}, job_dir, "same-job")

    def test_stale_capture_retries_but_unrelated_errors_do_not(self):
        for module in self.adapters():
            with patch.object(module, "capture_once", side_effect=[RuntimeError("beacon_anchor_changed_recapture_required"), {"fresh": True}]) as capture, \
                 patch.object(module, "archive_stale_proof") as archive:
                self.assertEqual(module.ensure_capture({}, Path("unused"), "job"), {"fresh": True})
                self.assertEqual(capture.call_count, 2)
                archive.assert_called_once()
            with patch.object(module, "capture_once", side_effect=RuntimeError("beacon_anchor_changed_recapture_required")) as capture, \
                 patch.object(module, "archive_stale_proof"):
                with self.assertRaisesRegex(RuntimeError, "keeps_advancing"):
                    module.ensure_capture({}, Path("unused"), "job")
                self.assertEqual(capture.call_count, 3)
            with patch.object(module, "capture_once", side_effect=RuntimeError("bad deposit")), \
                 patch.object(module, "archive_stale_proof") as archive:
                with self.assertRaisesRegex(RuntimeError, "bad deposit"):
                    module.ensure_capture({}, Path("unused"), "job")
                archive.assert_not_called()


def check_real_report():
    # Input is serialized directly from Rust VaultBridgeRouteReport. Exercise
    # the actual socket RPC parser, route pins and anchor reader in both adapters.
    import json
    import socket
    import threading
    report = json.load(sys.stdin)
    for module in IngressStageTests().adapters({
        "B23_INGRESS_ROUTE_PROFILE_HASH": report["profile_hash"],
        "B23_INGRESS_ROUTE_EPOCH": str(report["profile"]["route_epoch"]),
        "B23_INGRESS_ROUTE_BINDING": report["route_binding"],
        "B23_INGRESS_CREDITED_ASSET_ID": sys.argv[2],
    }):
        with socket.socket() as server:
            server.bind(("127.0.0.1", 0))
            server.listen(1)
            module.PFTL_RPC_PORT = server.getsockname()[1]
            requests = []
            def serve():
                with server.accept()[0] as stream:
                    requests.append(json.loads(stream.makefile("rb").readline()))
                    stream.sendall(json.dumps({"ok": True, "result": report}).encode() + b"\n")
            thread = threading.Thread(target=serve)
            thread.start()
            try:
                assert module.live_beacon_anchor() == report["beacon_finality_anchor"]
            finally:
                thread.join(timeout=10)
            assert requests[0]["method"] == "vault_bridge_route"
            print(module.__name__ + ": real Rust report -> socket RPC -> live_beacon_anchor passed")


if __name__ == "__main__":
    if sys.argv[1:2] == ["--report-stdin"]:
        check_real_report()
    else:
        unittest.main()
