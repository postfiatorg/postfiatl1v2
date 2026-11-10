#!/usr/bin/env python3
"""Tests for nav_proof_watch and exit_capacity (plan 07, criterion 6).

Two classes of input, kept strictly apart and labelled:

* REAL: `fixtures/routes-live-20261110T1950Z.json` is a verbatim capture of
  the public wallet API taken 2026-11-10T19:50Z by this worker (read-only
  GET; sha256 435dab6e…dba, two routes). It is the dated live state the
  "current" tests run against.
* SYNTHETIC: programmatically built payloads/logs/terms that exercise the
  healthy paths. Every synthetic terms file carries
  `decisions_ref: "SYNTHETIC TEST VALUES — NOT D-SETTLEMENT"` so it can
  never be mistaken for founder terms.

Run:  python3 -m unittest -v test_nav_proof_watch
"""
from __future__ import annotations

import contextlib
import json
import subprocess
import sys
import unittest
from pathlib import Path
from unittest import mock

import exit_capacity as ec
import nav_proof_watch as npw

HERE = Path(__file__).resolve().parent
FIXTURE = HERE / "fixtures" / "routes-live-20261110T1950Z.json"

SYNTHETIC_REF = "SYNTHETIC TEST VALUES — NOT D-SETTLEMENT"


def load_fixture() -> dict:
    return json.loads(FIXTURE.read_text(encoding="utf-8"))


def synthetic_terms(max_age=4 * 3600, interval=4 * 3600, window=24 * 3600):
    return {
        "max_nav_age_seconds": max_age,
        "proof_interval_seconds": interval,
        "observation_window_seconds": window,
        "decisions_ref": SYNTHETIC_REF,
    }


def run_cli(module_main, *argv):
    proc = subprocess.run(
        [sys.executable, "-c",
         f"import sys; sys.path.insert(0, {str(HERE)!r}); "
         f"import {module_main}; sys.exit({module_main}.main())", *argv],
        capture_output=True, text=True, cwd=str(HERE))
    return proc


class ObservationLogTests(unittest.TestCase):
    def test_observe_appends_hash_chained_records(self):
        import tempfile
        payload = load_fixture()
        with tempfile.TemporaryDirectory() as tmp:
            with mock.patch.object(npw, "fetch_routes", return_value=payload), \
                 mock.patch.object(npw.time, "time", return_value=1794340224.0):
                code = npw.main(["observe", "--state-dir", tmp])
            self.assertEqual(code, 0)
            records = npw.read_log(Path(tmp))
            self.assertEqual(len(records), 2)
            a666 = [r for r in records if r["route_id"]
                    .startswith("pftl-a666")][0]
            self.assertEqual(a666["proof_epoch"], 9)
            self.assertIsNone(a666["operator_observation_unix_ms"])
            self.assertEqual(a666["proof_age_basis"], "submitted_height_only")
            self.assertEqual(a666["settlement_reserve_atoms"], 140344262)
            proc = run_cli("nav_proof_watch", "verify-log", "--state-dir", tmp)
            self.assertIn("OK: 2 records chain intact", proc.stdout)

    def test_verify_log_detects_tampering(self):
        import tempfile
        payload = load_fixture()
        with tempfile.TemporaryDirectory() as tmp:
            with mock.patch.object(npw, "fetch_routes", return_value=payload):
                npw.main(["observe", "--state-dir", tmp])
            log = Path(tmp) / "observations.jsonl"
            lines = log.read_text().splitlines()
            record = json.loads(lines[0])
            record["proof_epoch"] = 99  # tamper
            lines[0] = json.dumps(record, sort_keys=True,
                                  separators=(",", ":"))
            log.write_text("\n".join(lines) + "\n")
            records = npw.read_log(Path(tmp))
            ok, message = npw.verify_log_chain(records)
            self.assertFalse(ok)
            self.assertIn("record_sha256 mismatch", message)

    def test_observe_records_sampler_stale_alarm(self):
        import tempfile
        payload = json.loads(json.dumps(load_fixture()))  # deep copy
        for route in payload["routes"]:
            route["stale"] = True
        with tempfile.TemporaryDirectory() as tmp:
            with mock.patch.object(npw, "fetch_routes", return_value=payload):
                code = npw.main(["observe", "--state-dir", tmp])
            self.assertEqual(code, 0)
            alarms = [json.loads(line) for line in
                      (Path(tmp) / "alarms.jsonl").read_text().splitlines()]
            self.assertEqual(len(alarms), 2)
            self.assertTrue(all(
                a["code"] == "sampler_reported_stale" for a in alarms))


class TermsRefusalTests(unittest.TestCase):
    def test_evaluate_refuses_without_terms_file(self):
        import tempfile
        payload = load_fixture()
        with tempfile.TemporaryDirectory() as tmp:
            with mock.patch.object(npw, "fetch_routes", return_value=payload):
                npw.main(["observe", "--state-dir", tmp])
            proc = run_cli("nav_proof_watch", "evaluate", "--state-dir", tmp)
            self.assertEqual(proc.returncode, 3)
            self.assertIn("REFUSED", proc.stdout)
            self.assertIn("D-SETTLEMENT", proc.stdout)
            for field in ("max_nav_age_seconds", "proof_interval_seconds",
                          "observation_window_seconds", "decisions_ref"):
                self.assertIn(field, proc.stdout)

    def test_evaluate_refuses_on_partial_or_zero_terms(self):
        import tempfile
        payload = load_fixture()
        with tempfile.TemporaryDirectory() as tmp:
            with mock.patch.object(npw, "fetch_routes", return_value=payload):
                npw.main(["observe", "--state-dir", tmp])
            terms_path = Path(tmp) / "terms.json"
            terms_path.write_text(json.dumps({
                "max_nav_age_seconds": 14400,
                "proof_interval_seconds": 0,  # invalid: zero
                "decisions_ref": "partial",
            }), encoding="utf-8")
            proc = run_cli("nav_proof_watch", "evaluate", "--state-dir", tmp)
            self.assertEqual(proc.returncode, 3)
            self.assertIn("INVALID: proof_interval_seconds", proc.stdout)
            self.assertIn("MISSING: observation_window_seconds", proc.stdout)

    def test_terms_template_ships_unset(self):
        proc = run_cli("nav_proof_watch", "terms-template")
        template = json.loads(proc.stdout)
        for key in npw.TERMS_FIELDS:
            self.assertIsNone(template[key])


class EvaluationTests(unittest.TestCase):
    def test_live_state_alarms(self):
        """The dated live capture must alarm: no observation time, one stale
        October epoch over a long window, idle chain."""
        import tempfile
        payload = load_fixture()
        with tempfile.TemporaryDirectory() as tmp:
            # 24h window sampled every 30 min from the live capture's facts
            start_ms = 1794340224000 - 24 * 3600 * 1000
            with mock.patch.object(npw, "fetch_routes", return_value=payload):
                for index in range(48):
                    with mock.patch.object(npw.time, "time",
                                           return_value=(start_ms + index * 1800 * 1000) / 1000):
                        npw.main(["observe", "--state-dir", tmp])
            terms_path = Path(tmp) / "terms.json"
            terms_path.write_text(json.dumps(synthetic_terms()), encoding="utf-8")
            proc = run_cli("nav_proof_watch", "evaluate", "--state-dir", tmp)
            self.assertEqual(proc.returncode, 2)
            evaluation = json.loads(
                (Path(tmp) / "evaluation.json").read_text())
            self.assertGreater(evaluation["alarm_count"], 0)
            codes = {a["code"] for a in evaluation["alarms"]}
            self.assertIn("nav_age_not_wall_clock_verifiable", codes)
            self.assertIn("chain_idle_over_window", codes)
            self.assertIn("proof_cadence_breach", codes)
            # alarms are also persisted, hash-chained
            alarms = [json.loads(line) for line in
                      (Path(tmp) / "alarms.jsonl").read_text().splitlines()]
            self.assertEqual(len(alarms), evaluation["alarm_count"])

    def test_healthy_synthetic_window_passes(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            # fresh proof every 2h inside a 24h window sampled every 10 min;
            # each new proof advances the epoch and observation time
            start_ms = 1794340224000 - 24 * 3600 * 1000
            for index in range(144):
                epoch = 10 + index // 12  # new epoch every 2h
                obs = start_ms + (index // 12) * 2 * 3600 * 1000
                self.build_log_single(
                    tmp, recorded_ms=start_ms + index * 600 * 1000,
                    epoch=epoch, observation_ms=obs,
                    height=100 + index * 10, packet="a" * 80 + str(epoch))
            terms_path = Path(tmp) / "terms.json"
            terms_path.write_text(json.dumps(
                synthetic_terms(max_age=4 * 3600, interval=3 * 3600,
                                window=24 * 3600)), encoding="utf-8")
            proc = run_cli("nav_proof_watch", "evaluate", "--state-dir", tmp)
            self.assertEqual(proc.returncode, 0, proc.stdout)
            evaluation = json.loads(
                (Path(tmp) / "evaluation.json").read_text())
            self.assertEqual(evaluation["alarm_count"], 0)
            facts = evaluation["per_route"]["synthetic"]
            self.assertEqual(facts["freshness_verdict"], "fresh")
            self.assertGreaterEqual(facts["epochs_in_window_count"], 12)

    def build_log_single(self, tmp, *, recorded_ms, epoch, observation_ms,
                         height, packet):
        record = {
            "schema": npw.SCHEMA_OBSERVATION,
            "recorded_at_unix_ms": recorded_ms,
            "source_url": "file://test",
            "route_id": "synthetic",
            "name": "Test Coin",
            "chain_height": height,
            "chain_state_root": "f" * 96,
            "sample_observed_at_ms": recorded_ms,
            "sampler_reported_stale": False,
            "state": {"buy": True, "sell": True, "state": "Open"},
            "proof_epoch": epoch,
            "proof_state": "finalized",
            "proof_submitted_at_height": 10,
            "proof_age_blocks": 5,
            "proof_age_basis": "operator_attested",
            "proof_age_ms": None,
            "operator_observation_unix_ms": observation_ms,
            "reserve_packet_hash": packet,
            "profile_max_snapshot_age_blocks": 900,
            "settlement_reserve_atoms": "1000",
            "reserve_backed_redeem_atoms": "1000",
        }
        npw.append_record(Path(tmp), record)

    def test_stale_nav_alarms_when_observation_time_exists(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            start_ms = 1794340224000 - 24 * 3600 * 1000
            # one proof, observed 12h before the window end; max age 4h
            for index in range(48):
                self.build_log_single(
                    tmp, recorded_ms=start_ms + index * 1800 * 1000,
                    epoch=10,
                    observation_ms=start_ms + 12 * 3600 * 1000,
                    height=100 + index, packet="a" * 80)
            terms_path = Path(tmp) / "terms.json"
            terms_path.write_text(json.dumps(
                synthetic_terms(max_age=4 * 3600, interval=3 * 3600,
                                window=24 * 3600)), encoding="utf-8")
            proc = run_cli("nav_proof_watch", "evaluate", "--state-dir", tmp)
            self.assertEqual(proc.returncode, 2)
            evaluation = json.loads(
                (Path(tmp) / "evaluation.json").read_text())
            codes = {a["code"] for a in evaluation["alarms"]}
            self.assertIn("nav_stale", codes)
            self.assertEqual(
                evaluation["per_route"]["synthetic"]["freshness_verdict"],
                "stale")

    def test_evaluate_refuses_on_empty_log(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            terms_path = Path(tmp) / "terms.json"
            terms_path.write_text(json.dumps(synthetic_terms()),
                                   encoding="utf-8")
            proc = run_cli("nav_proof_watch", "evaluate", "--state-dir", tmp)
            self.assertEqual(proc.returncode, 3)
            self.assertIn("no observations recorded yet", proc.stdout)


class ExitCapacityTests(unittest.TestCase):
    def fake_eth(self, vault_usdc=76_067_140, obligations=76_067_140,
                 paused=0):
        def fake_call(rpc, to, data, timeout=20.0):
            if data.startswith("0x70a08231"):
                return "0x" + hex(vault_usdc)[2:].rjust(64, "0")
            if data.startswith("0xfbe1f9eb"):
                return "0x" + hex(obligations)[2:].rjust(64, "0")
            if data.startswith("0x5c975abb"):
                return "0x" + hex(paused)[2:].rjust(64, "0")
            raise AssertionError(f"unexpected call {data}")
        return fake_call

    def test_refuses_without_operator_lane_status(self):
        with mock.patch.object(ec, "fetch_routes",
                               return_value=load_fixture()), \
             mock.patch.object(ec, "eth_call", self.fake_eth()):
            verdict = ec.check("file://test", "file://eth", dict(ec.DEFAULT_CONFIG),
                               None, ["USDC lane status file not found"])
        self.assertFalse(verdict["buyer_exit_verified"])
        self.assertTrue(verdict["gates"]["usdc_exit_lane"]["refused"])

    def test_live_state_not_verified_operator_test_only(self):
        """Dated live state: lane operator-test-only per inventory D → the
        buyer-exit verdict must fail, naming that gate."""
        import io
        import tempfile
        lane = {
            "live_value_enabled": False,
            "operator_test_only": True,
            "observed_at_unix_ms": 1794300000000,
            "source": "inventory/D-navcoin-checkout-schwab.md (2026-11-10)",
        }
        with tempfile.TemporaryDirectory() as tmp, \
             mock.patch.object(ec, "fetch_routes",
                               return_value=load_fixture()), \
             mock.patch.object(ec, "eth_call", self.fake_eth()), \
             contextlib.redirect_stdout(io.StringIO()) as captured:
            lane_path = Path(tmp) / "lane.json"
            lane_path.write_text(json.dumps(lane), encoding="utf-8")
            code = ec.main(["--routes-url", "file://test",
                            "--eth-rpc", "file://eth",
                            "--usdc-lane-status", str(lane_path)])
        self.assertEqual(code, 2)
        self.assertIn("NOT VERIFIED", captured.getvalue())
        self.assertIn("operator_test_only", captured.getvalue())

    def test_measured_live_capacity_matches_review_03(self):
        with mock.patch.object(ec, "fetch_routes",
                               return_value=load_fixture()), \
             mock.patch.object(ec, "eth_call", self.fake_eth()):
            verdict = ec.check("file://test", "file://eth",
                               dict(ec.DEFAULT_CONFIG),
                               {"live_value_enabled": False,
                                "operator_test_only": True,
                                "observed_at_unix_ms": 1, "source": "t"},
                               [])
        a666 = [r for r in verdict["routes"]
                if r["route_id"].startswith("pftl-a666")][0]
        bmnrc = [r for r in verdict["routes"]
                 if r["route_id"].startswith("pftl-bmnrc")][0]
        self.assertEqual(a666["redemption_capacity_pfusdc"], 140.344262)
        self.assertEqual(a666["reserve_backed_redeem_pfusdc"], 120.222695)
        self.assertFalse(bmnrc["sell_allowed"])
        self.assertEqual(bmnrc["redemption_capacity_pfusdc"], 0.0)
        self.assertEqual(
            verdict["gates"]["vault_usdc_coverage"]["measured"]
            ["vault_usdc"], 76.06714)

    def test_synthetic_healthy_exit_verifies(self):
        import io
        import tempfile
        payload = json.loads(json.dumps(load_fixture()))
        for route in payload["routes"]:
            route["state"] = {"buy": True, "sell": True, "state": "Open"}
            supply = (route.get("source") or {}).get("supply") or {}
            supply["settlement_reserve_atoms"] = "50000000"
        lane = {
            "live_value_enabled": True,
            "operator_test_only": False,
            "observed_at_unix_ms": 1794340224000,
            "source": "synthetic test",
        }
        with tempfile.TemporaryDirectory() as tmp, \
             mock.patch.object(ec, "fetch_routes", return_value=payload), \
             mock.patch.object(ec, "eth_call", self.fake_eth()), \
             contextlib.redirect_stdout(io.StringIO()) as captured:
            lane_path = Path(tmp) / "lane.json"
            lane_path.write_text(json.dumps(lane), encoding="utf-8")
            code = ec.main(["--routes-url", "file://test",
                            "--eth-rpc", "file://eth",
                            "--usdc-lane-status", str(lane_path)])
        self.assertEqual(code, 0, captured.getvalue())
        self.assertIn("VERIFIED", captured.getvalue())

    def test_selectors_match_cast_derivation(self):
        # cast sig, derived 2026-11-10 (see selector() comment)
        self.assertEqual(ec.selector("totalObligations()"), "0xfbe1f9eb")
        self.assertEqual(ec.selector("paused()"), "0x5c975abb")
        self.assertEqual(ec.balance_of_data(
            "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")[:10], "0x70a08231")


if __name__ == "__main__":
    unittest.main()
