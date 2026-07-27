#!/usr/bin/env python3

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("pfusdc-mainnet-latency-gate.py")
SPEC = importlib.util.spec_from_file_location("pfusdc_mainnet_latency_gate", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
gate = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gate)


def summary() -> dict:
    return {
        "schema": gate.SUMMARY_SCHEMA,
        "legs": {
            "ethereum_deposit": {"status": "PASS", "deposit_block": 1},
            "ingress_finality_proof": {"status": "PASS"},
            "pftl_claim": {"status": "PASS"},
            "pftl_burn": {"status": "PASS"},
            "pftl_egress_proof": {"status": "PASS"},
            "ethereum_withdrawal": {
                "status": "PASS",
                "ethereum_block": 2,
                "replay_rejected": True,
            },
        },
        "terminal_state": {"campaign_conservation_residual_delta_atoms": 0},
    }


class LatencyGateTests(unittest.TestCase):
    def test_1500_seconds_passes(self) -> None:
        report = gate.measure_report(summary(), 100, 1600)
        self.assertEqual(report["verdict"], "PASS")
        self.assertEqual(report["elapsed_seconds"], 1500)

    def test_1501_seconds_fails(self) -> None:
        report = gate.measure_report(summary(), 100, 1601)
        self.assertEqual(report["verdict"], "FAIL")

    def test_replay_must_be_rejected(self) -> None:
        value = summary()
        value["legs"]["ethereum_withdrawal"]["replay_rejected"] = False
        with self.assertRaises(gate.GateError):
            gate.measure_report(value, 100, 200)

    def test_conservation_must_be_exact(self) -> None:
        value = summary()
        value["terminal_state"]["campaign_conservation_residual_delta_atoms"] = 1
        with self.assertRaises(gate.GateError):
            gate.measure_report(value, 100, 200)

    def test_all_legs_must_pass(self) -> None:
        value = summary()
        value["legs"]["pftl_claim"]["status"] = "FAIL"
        with self.assertRaises(gate.GateError):
            gate.measure_report(value, 100, 200)

    def test_reversed_timestamps_fail_closed(self) -> None:
        with self.assertRaises(gate.GateError):
            gate.measure_report(summary(), 200, 100)

    def test_fresh_checkpoint_passes(self) -> None:
        self.assertEqual(gate.preflight_report(326, 326)["verdict"], "PASS")
        self.assertEqual(gate.preflight_report(326, 325)["verdict"], "PASS")

    def test_stale_checkpoint_fails(self) -> None:
        self.assertEqual(gate.preflight_report(326, 324)["verdict"], "FAIL")

    def test_verifier_cannot_be_ahead(self) -> None:
        with self.assertRaises(gate.GateError):
            gate.preflight_report(325, 326)


if __name__ == "__main__":
    unittest.main()
