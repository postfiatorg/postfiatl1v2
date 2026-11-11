#!/usr/bin/env python3
"""Tests for nav_proof_watch and exit_capacity.

Two classes of input, kept apart:

* RECORDED: `fixtures/routes-live-20261110T1950Z.json` is a capture of the
  wallet API routes answer taken 2026-11-10T19:50Z (two routes), with no
  endpoint or host detail. It is the dated live shape.
* SYNTHETIC: payloads, logs and terms built here. Every synthetic terms file
  carries `terms_ref: "SYNTHETIC TEST VALUES"`; no terms file is shipped.

Run:  python3 -m unittest -v test_nav_proof_watch
"""
from __future__ import annotations

import contextlib
import io
import json
import re
import tempfile
import types
import unittest
from pathlib import Path
from unittest import mock

import exit_capacity as ec
import nav_proof_watch as npw

HERE = Path(__file__).resolve().parent
FIXTURE = HERE / "fixtures" / "routes-live-20261110T1950Z.json"

SYNTHETIC_REF = "SYNTHETIC TEST VALUES"
URL = "https://wallet.invalid/api/navcoin/routes"
NOW_S = 1794340224.0


def load_fixture() -> dict:
    return json.loads(FIXTURE.read_text(encoding="utf-8"))


def synthetic_terms(max_age=4 * 3600, interval=4 * 3600, window=24 * 3600,
                    sample=1800):
    return {
        "max_nav_age_seconds": max_age,
        "proof_interval_seconds": interval,
        "observation_window_seconds": window,
        "sample_interval_seconds": sample,
        "terms_ref": SYNTHETIC_REF,
    }


def at(seconds):
    return mock.patch.object(npw.time, "time", return_value=seconds)


def run_cli(module_main, *argv, now=NOW_S):
    """Run a command in-process with an injected clock."""
    module = {"nav_proof_watch": npw, "exit_capacity": ec}[module_main]
    out, err = io.StringIO(), io.StringIO()
    with at(now), contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        code = module.main(list(argv))
    return types.SimpleNamespace(returncode=code, stdout=out.getvalue(),
                                 stderr=err.getvalue())


class ObservationLogTests(unittest.TestCase):
    def test_observe_appends_hash_chained_records(self):
        import tempfile
        payload = load_fixture()
        with tempfile.TemporaryDirectory() as tmp:
            with mock.patch.object(npw, "fetch_routes", return_value=payload), \
                 mock.patch.object(npw.time, "time", return_value=1794340224.0):
                code = npw.main(["observe", "--url", URL, "--state-dir", tmp])
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
                npw.main(["observe", "--url", URL, "--state-dir", tmp])
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

    def test_sampler_stale_flag_raises_its_own_alarm(self):
        payload = json.loads(json.dumps(load_fixture()))  # deep copy
        for route in payload["routes"]:
            route["stale"] = True
        with tempfile.TemporaryDirectory() as tmp:
            with mock.patch.object(npw, "fetch_routes", return_value=payload), at(NOW_S):
                self.assertEqual(npw.main(["observe", "--url", URL, "--state-dir", tmp]), 0)
            Path(tmp, "terms.json").write_text(json.dumps(synthetic_terms()))
            proc = run_cli("nav_proof_watch", "evaluate", "--state-dir", tmp)
            evaluation = json.loads(Path(tmp, "evaluation.json").read_text())
            stale = [a for a in evaluation["alarms"] if a["code"] == "sampler_reported_stale"]
            self.assertEqual(len(stale), 2, proc.stdout)


class TermsRefusalTests(unittest.TestCase):
    def test_evaluate_refuses_without_terms_file(self):
        import tempfile
        payload = load_fixture()
        with tempfile.TemporaryDirectory() as tmp:
            with mock.patch.object(npw, "fetch_routes", return_value=payload):
                npw.main(["observe", "--url", URL, "--state-dir", tmp])
            proc = run_cli("nav_proof_watch", "evaluate", "--state-dir", tmp)
            self.assertEqual(proc.returncode, 3)
            self.assertIn("REFUSED", proc.stdout)
            self.assertIn("product owner's settlement terms", proc.stdout)
            for field in npw.TERMS_FIELDS:
                self.assertIn(field, proc.stdout)

    def test_evaluate_refuses_on_partial_or_zero_terms(self):
        import tempfile
        payload = load_fixture()
        with tempfile.TemporaryDirectory() as tmp:
            with mock.patch.object(npw, "fetch_routes", return_value=payload):
                npw.main(["observe", "--url", URL, "--state-dir", tmp])
            terms_path = Path(tmp) / "terms.json"
            terms_path.write_text(json.dumps({
                "max_nav_age_seconds": 14400,
                "proof_interval_seconds": 0,  # invalid: zero
                "terms_ref": "partial",
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
                        npw.main(["observe", "--url", URL, "--state-dir", tmp])
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
            self.assertGreaterEqual(len(facts["new_epochs_in_window"]), 11)

    def build_log_single(self, tmp, *, recorded_ms, epoch, observation_ms,
                         height, packet):
        record = {
            "schema": npw.SCHEMA_OBSERVATION,
            "recorded_at_unix_ms": recorded_ms,
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
        """Dated live state: the lane was operator-test-only on 2026-11-10,
        so the buyer-exit verdict must fail, naming that gate."""
        import io
        import tempfile
        lane = {
            "live_value_enabled": False,
            "operator_test_only": True,
            "observed_at_unix_ms": 1794300000000,
            "source": "operator capture 2026-11-10",
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

    def test_measured_live_capacity(self):
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


def single(tmp, recorded_ms, *, route="synthetic", epoch=10, observation_ms=None,
           height=100, packet=None):
    EvaluationTests.build_log_single(
        None, tmp, recorded_ms=recorded_ms, epoch=epoch,
        observation_ms=observation_ms, height=height,
        packet=packet or ("a" * 80 + str(epoch)))
    if route != "synthetic":
        log = Path(tmp, "observations.jsonl")
        lines = log.read_text().splitlines()
        record = json.loads(lines[-1])
        record["route_id"] = route
        lines.pop()
        log.write_text("".join(line + "\n" for line in lines))
        Path(tmp, "observations.head.json").unlink(missing_ok=True)
        record.pop("record_sha256"), record.pop("prev_sha256")
        npw.append_record(Path(tmp), record)


def evaluate_with(tmp, terms, now_s=NOW_S):
    Path(tmp, "terms.json").write_text(json.dumps(terms))
    proc = run_cli("nav_proof_watch", "evaluate", "--state-dir", tmp, now=now_s)
    path = Path(tmp, "evaluation.json")
    evaluation = json.loads(path.read_text()) if path.exists() else None
    return proc, evaluation


def codes_of(evaluation, route=None):
    return {a["code"] for a in evaluation["alarms"]
            if route is None or a.get("route_id") == route}


class PublicTreeTests(unittest.TestCase):
    """Nothing private: no IPv4 literal, no host paths, no non-public docs."""

    def test_tool_directory_has_no_private_references(self):
        patterns = [re.compile(p) for p in (
            r"\b\d{1,3}(?:\.\d{1,3}){3}\b", "/ho" + "me/", "exec" + "ution/",
            "inven" + "tory/", "pla" + "ns/0", "DECI" + "SIONS", "D-SETT" + "LEMENT")]
        hits = []
        for path in sorted(HERE.rglob("*")):
            if not path.is_file() or "__pycache__" in path.parts:
                continue
            text = path.read_text(encoding="utf-8", errors="replace")
            for pattern in patterns:
                for match in pattern.finditer(text):
                    hits.append(f"{path.relative_to(HERE)}: {match.group(0)}")
        self.assertEqual(hits, [])

    def test_no_terms_values_are_shipped(self):
        shipped = [p for p in HERE.rglob("*.json") if "terms" in p.name]
        self.assertEqual([p.name for p in shipped], ["terms.template.json"])
        template = json.loads((HERE / "terms.template.json").read_text())
        self.assertTrue(all(template[key] is None for key in npw.TERMS_FIELDS))


class StoppedLogTests(unittest.TestCase):
    def test_log_that_stopped_days_ago_alarms(self):
        with tempfile.TemporaryDirectory() as tmp:
            start = int((NOW_S - 30 * 3600) * 1000)
            for index in range(13):  # healthy: new epoch every 2h, then stops
                single(tmp, start + index * 7200_000, epoch=10 + index,
                       observation_ms=start + index * 7200_000, height=100 + index)
            terms = synthetic_terms(interval=3 * 3600, window=24 * 3600, sample=600)
            # judged at the last record: healthy
            proc, evaluation = evaluate_with(tmp, terms, now_s=(start + 12 * 7200_000) / 1000)
            self.assertEqual(proc.returncode, 0, proc.stdout)
            # three days later nothing new was recorded: must not look healthy
            proc, evaluation = evaluate_with(tmp, terms,
                                             now_s=(start + 12 * 7200_000) / 1000 + 3 * 86400)
            self.assertEqual(proc.returncode, 2, proc.stdout)
            self.assertIn("observation_log_stale", codes_of(evaluation))

    def test_one_missed_run_is_tolerated(self):
        with tempfile.TemporaryDirectory() as tmp:
            start = int((NOW_S - 30 * 3600) * 1000)
            for index in range(13):
                single(tmp, start + index * 7200_000, epoch=10 + index,
                       observation_ms=start + index * 7200_000, height=100 + index)
            last = (start + 12 * 7200_000) / 1000
            terms = synthetic_terms(interval=3 * 3600, window=24 * 3600, sample=600)
            proc, evaluation = evaluate_with(tmp, terms, now_s=last + 1100)
            self.assertNotIn("observation_log_stale", codes_of(evaluation))
            proc, evaluation = evaluate_with(tmp, terms, now_s=last + 1300)
            self.assertIn("observation_log_stale", codes_of(evaluation))


class CadenceTests(unittest.TestCase):
    def test_first_sample_alone_is_not_a_gap_and_not_idle(self):
        with tempfile.TemporaryDirectory() as tmp:
            now_ms = int(NOW_S * 1000)
            single(tmp, now_ms - 1000, epoch=10, observation_ms=now_ms - 2000)
            proc, evaluation = evaluate_with(
                tmp, synthetic_terms(interval=1800, window=7200, sample=300))
            codes = codes_of(evaluation)
            self.assertNotIn("proof_cadence_breach", codes)
            self.assertNotIn("chain_idle_over_window", codes)
            self.assertEqual(evaluation["outcome"], "window_not_yet_covered")
            self.assertEqual(proc.returncode, 5, proc.stdout)

    def test_route_that_appears_mid_window_has_no_gap_before_it(self):
        with tempfile.TemporaryDirectory() as tmp:
            now_ms = int(NOW_S * 1000)
            start = now_ms - 7200_000
            for index in range(13):  # covered healthy route
                single(tmp, start + index * 600_000, epoch=10 + index // 3,
                       observation_ms=start + index * 600_000, height=100 + index)
            single(tmp, now_ms - 60_000, route="newcomer", epoch=1,
                   observation_ms=now_ms - 60_000, height=200)
            proc, evaluation = evaluate_with(
                tmp, synthetic_terms(interval=1800, window=7200, sample=600))
            self.assertNotIn("proof_cadence_breach", codes_of(evaluation, "newcomer"))
            self.assertEqual(evaluation["routes_not_yet_covered"], ["newcomer"])

    def test_terms_that_could_never_alarm_are_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            single(tmp, int(NOW_S * 1000), observation_ms=int(NOW_S * 1000))
            proc, _ = evaluate_with(
                tmp, synthetic_terms(interval=14400, window=3600, sample=300))
            self.assertEqual(proc.returncode, 3, proc.stdout)
            self.assertIn("observation_window_seconds must be longer", proc.stdout)

    def test_old_epoch_displayed_all_window_alarms(self):
        """Dated live shape: the October epoch shown for the whole window."""
        payload = load_fixture()
        with tempfile.TemporaryDirectory() as tmp:
            start_s = NOW_S - 26 * 3600
            with mock.patch.object(npw, "fetch_routes", return_value=payload):
                for index in range(53):
                    with at(start_s + index * 1800):
                        npw.main(["observe", "--url", URL, "--state-dir", tmp])
            proc, evaluation = evaluate_with(
                tmp, synthetic_terms(interval=4 * 3600, window=24 * 3600),
                now_s=start_s + 52 * 1800)
            for route in evaluation["per_route"]:
                self.assertIn("no_proof_finalized_in_window", codes_of(evaluation, route))
                self.assertIn("proof_cadence_breach", codes_of(evaluation, route))
            self.assertEqual(proc.returncode, 2)


class ExitCapacityGateTests(unittest.TestCase):
    def test_paused_vault_alarms_even_without_lane_status(self):
        fake = ExitCapacityTests.fake_eth(None, obligations=76_067_140,
                                          vault_usdc=1, paused=1)
        with mock.patch.object(ec, "fetch_routes", return_value=load_fixture()), \
             mock.patch.object(ec, "eth_call", fake), \
             contextlib.redirect_stdout(io.StringIO()) as captured:
            code = ec.main(["--routes-url", URL, "--eth-rpc", "file://eth"])
        self.assertEqual(code, 2, captured.getvalue())
        self.assertIn("vault_usdc_coverage", captured.getvalue())

    def test_missing_settlement_precision_is_a_read_error(self):
        payload = json.loads(json.dumps(load_fixture()))
        del payload["routes"][0]["source"]["route"]["settlement_asset_precision"]
        with mock.patch.object(ec, "fetch_routes", return_value=payload), \
             mock.patch.object(ec, "eth_call", ExitCapacityTests.fake_eth(None)), \
             contextlib.redirect_stdout(io.StringIO()), \
             contextlib.redirect_stderr(io.StringIO()):
            code = ec.main(["--routes-url", URL, "--eth-rpc", "file://eth"])
        self.assertEqual(code, 4)


class LogIntegrityTests(unittest.TestCase):
    def observed(self, tmp, runs=3):
        with mock.patch.object(npw, "fetch_routes", return_value=load_fixture()):
            for index in range(runs):
                with at(NOW_S + index * 300):
                    npw.main(["observe", "--url", URL, "--state-dir", tmp])
        return Path(tmp, "observations.jsonl")

    def test_truncated_tail_is_detected(self):
        with tempfile.TemporaryDirectory() as tmp:
            log = self.observed(tmp)
            lines = log.read_text().splitlines(keepends=True)
            log.write_text("".join(lines[:-2]))
            proc = run_cli("nav_proof_watch", "verify-log", "--state-dir", tmp)
            self.assertEqual(proc.returncode, 1)
            self.assertIn("truncated", proc.stdout)

    def test_duplicate_key_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            log = self.observed(tmp, runs=1)
            lines = log.read_text().splitlines(keepends=True)
            lines[0] = lines[0].replace('{"chain_height":1170', '{"chain_height":999,"chain_height":1170', 1)
            log.write_text("".join(lines))
            proc = run_cli("nav_proof_watch", "verify-log", "--state-dir", tmp)
            self.assertEqual(proc.returncode, 1)
            self.assertIn("duplicate key", proc.stdout)

    def test_torn_last_line_is_reported_and_repairable(self):
        with tempfile.TemporaryDirectory() as tmp:
            log = self.observed(tmp, runs=1)
            with open(log, "a") as handle:
                handle.write('{"schema":"postf')
            with mock.patch.object(npw, "fetch_routes", return_value=load_fixture()), \
                 contextlib.redirect_stderr(io.StringIO()) as err:
                code = npw.main(["observe", "--url", URL, "--state-dir", tmp])
            self.assertEqual(code, 1)
            self.assertIn("repair-tail", err.getvalue())
            self.assertEqual(run_cli("nav_proof_watch", "repair-tail", "--state-dir", tmp).returncode, 0)
            self.assertEqual(self.observed(tmp, runs=1).read_text().count("\n"), 4)
            self.assertEqual(run_cli("nav_proof_watch", "verify-log", "--state-dir", tmp).returncode, 0)

    def test_malformed_api_answer_exits_4(self):
        payload = json.loads(json.dumps(load_fixture()))
        payload["routes"][0]["proof"] = "not an object"
        with tempfile.TemporaryDirectory() as tmp, \
             mock.patch.object(npw, "fetch_routes", return_value=payload), \
             contextlib.redirect_stderr(io.StringIO()):
            code = npw.main(["observe", "--url", URL, "--state-dir", tmp])
        self.assertEqual(code, 4)

    def test_observe_requires_an_endpoint(self):
        with self.assertRaises(SystemExit), contextlib.redirect_stderr(io.StringIO()):
            npw.main(["observe", "--state-dir", "unused"])


if __name__ == "__main__":
    unittest.main()
