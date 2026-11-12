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
    def adapters(self):
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


if __name__ == "__main__":
    unittest.main()
