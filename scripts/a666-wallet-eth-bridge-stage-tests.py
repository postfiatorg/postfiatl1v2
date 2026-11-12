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
            with patch.dict(os.environ, {"A666_A100_HOST": "unused", "A666_VALIDATOR2_HOST": "unused"}), patch.dict(sys.modules, {"web3": web3}):
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


if __name__ == "__main__":
    unittest.main()
