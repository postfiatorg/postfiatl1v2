"""Solana epoch-change tests for the health watcher, the custody relay and the re-anchor.

Fakes only: no Solana RPC, no Post Fiat node, no SSH, no systemd. Run from the repository root
with `python3 -m pytest -q ops/sol-live/test_reanchor_mainnet.py` or `python3 -m unittest`.
"""

import importlib.util
import json
import sys
import tempfile
import threading
import types
from collections import defaultdict
import unittest
from pathlib import Path
from unittest import mock

import requests

HERE = Path(__file__).resolve().parent
SLOTS = 432000
EPOCH = 1091
FIRST = EPOCH * SLOTS  # first slot of epoch 1091 on mainnet (no warmup)
NEXT = FIRST + SLOTS  # first slot of epoch 1092


class Invalid(ValueError):
    """Stand-in for storage.Invalid: a binding, signature or receipt check failed."""


class Rejected(Invalid):
    """Stand-in for storage.Rejected: the ledger committed a rejection receipt with this code."""

    def __init__(self, message, code):
        super().__init__(message)
        self.code = code


def _stub(name, **attrs):
    module = types.ModuleType(name)
    module.__dict__.update(attrs)
    module.__getattr__ = lambda attr: mock.MagicMock(name=f"{name}.{attr}")
    sys.modules[name] = module
    return module




def load(name):
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), HERE / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def setUpModule():
    global live
    modules = mock.patch.dict(sys.modules)
    paths = sys.path[:]
    modules.start()
    unittest.addModuleCleanup(modules.stop)
    unittest.addModuleCleanup(lambda: sys.path.__setitem__(slice(None), paths))
    # The scripts import Solana SDK and relay modules that are not needed under test.
    for _name in ("solders", "solders.hash", "solders.pubkey", "solders.transaction", "solders.system_program",
                  "solders.compute_budget", "keys", "bridge", "source", "measure", "recorder"):
        _stub(_name)
    _stub("route_binding", CHAIN=b"postfiat-wan-devnet-2", PFTL_GENESIS=bytes(48), PUBLIC_ROUTES={},
          route_binding=lambda *args, **kwargs: bytes(32))
    _stub("storage", Invalid=Invalid, Rejected=Rejected, AwaitingSource=type("AwaitingSource", (Exception,), {}))
    sys.path.insert(0, str(HERE))
    live = load("live")


class Reply:
    def __init__(self, body):
        self.body = body

    def json(self):
        return self.body


class FakeSolana:
    """Public mainnet RPC: finalized tip, epoch info and schedule; methods can be made to fail."""

    def __init__(self, tip):
        self.tip = tip
        self.calls = []
        self.fail = {}

    def post(self, url, json=None, timeout=None):
        method = json["method"]
        self.calls.append(method)
        if method in self.fail:
            if self.fail[method] == "down":
                raise requests.ConnectionError("connection refused")
            return Reply({"jsonrpc": "2.0", "id": 1, "error": {"code": 429, "message": "Too many requests"}})
        first = self.tip - self.tip % SLOTS
        result = {
            "getSlot": self.tip,
            "getEpochInfo": {"epoch": self.tip // SLOTS, "absoluteSlot": self.tip,
                             "slotIndex": self.tip - first, "slotsInEpoch": SLOTS},
            "getEpochSchedule": {"slotsPerEpoch": SLOTS, "leaderScheduleSlotOffset": SLOTS, "warmup": False,
                                 "firstNormalEpoch": 0, "firstNormalSlot": 0},
        }[method]
        return Reply({"jsonrpc": "2.0", "id": 1, "result": result})


class HealthTest(unittest.TestCase):
    def setUp(self):
        self.base = Path(tempfile.mkdtemp())
        self.health = load("mainnet-health")
        self.health.BASE = self.base
        live.save(self.base / "activation.json", {"profile": {"solana": {"anchor": {"epoch": EPOCH}}}})
        self.sol = FakeSolana(FIRST + 1000)
        self.capture = {"observer_slot": lambda: self.sol.tip, "check_prover": lambda path: None}
        patcher = mock.patch.object(self.health.requests, "post", self.sol.post)
        patcher.start()
        self.addCleanup(patcher.stop)

    def check(self):
        return self.health.check(self.capture)

    def test_open_inside_the_anchored_epoch(self):
        self.assertTrue(self.check()["ok"])

    def test_public_rpc_down_closes_deposits(self):
        self.sol.fail["getSlot"] = "down"
        state = self.check()
        self.assertFalse(state["ok"])
        self.assertIn("unavailable", state["message"])

    def test_rate_limited_epoch_read_closes_deposits(self):
        self.sol.fail.update(getEpochInfo="429", getEpochSchedule="429")
        self.assertFalse(self.check()["ok"])

    def test_closes_before_the_next_epoch(self):
        self.sol.tip = NEXT - 100
        state = self.check()
        self.assertFalse(state["ok"])
        self.assertIn(str(NEXT), state["message"])

    def test_closes_at_the_margin_and_not_before(self):
        margin = getattr(self.health, "CLOSE_SLOTS", None)
        self.assertIsNotNone(margin)
        self.sol.tip = NEXT - margin - 1
        self.assertTrue(self.check()["ok"])
        self.sol.tip = NEXT - margin
        self.assertFalse(self.check()["ok"])

    def test_margin_covers_the_readiness_close_even_at_fast_slots(self):
        # A durable-nonce deposit does not expire by block height. What keeps one from landing
        # after the epoch changes is the wallet refusing to send while readiness is closed, and
        # the close takes up to one stale window, one health period and one re-send window.
        h = self.health
        close = h.READINESS_STALE_SECONDS + h.HEALTH_PERIOD_SECONDS + h.RESEND_SECONDS
        self.assertGreaterEqual(h.CLOSE_SLOTS * h.FASTEST_SLOT_SECONDS, close)
        self.assertEqual(h.CLOSE_SLOTS, round(12 * 60 / h.SLOT_SECONDS))

    def test_new_epoch_waits_for_its_validator_set(self):
        self.sol.tip = NEXT + 10
        state = self.check()
        self.assertFalse(state["ok"])
        self.assertIn(f"epoch {EPOCH + 1}", state["message"])

    def test_schedule_is_read_once(self):
        self.check()
        self.check()
        self.assertEqual(self.sol.calls.count("getEpochSchedule"), 1)



class Stop(Exception):
    """Ends a service loop after one pass."""


def custody_page(rows, params):
    """getSignaturesForAddress over newest-first rows, honouring limit and before."""
    start = 0
    if "before" in params:
        start = [r["signature"] for r in rows].index(params["before"]) + 1
    return rows[start : start + params.get("limit", 1000)]


def row(sig, slot, err=None):
    return {"signature": sig, "slot": slot, "err": err}


class RelayTest(unittest.TestCase):
    ANCHOR = FIRST + 5000

    def setUp(self):
        self.base = Path(tempfile.mkdtemp())
        self.relay = load("mainnet-relay")
        self.relay.BASE = self.base
        self.rows = []
        self.credited = []
        self.failing = {}
        self.not_deposits = set()
        self.events = []
        self.started = defaultdict(threading.Event)
        self.receipts = {}
        activation = {"profile": {"solana": {"anchor_slot": self.ANCHOR, "slots_per_epoch": SLOTS, "epoch_warmup": False,
                                             "anchor": {"epoch": EPOCH}}}, "profile_hash": "H5"}
        live.save(self.base / "activation.json", activation)
        cfg = {"vault_program": "00" * 32, "state": str(self.base / "relay"),
               "archived_transactions": str(self.base / "finalized-transactions")}

        class Missed(Exception):
            pass

        def credit(relay, activation, sig, slot, observed):
            self.events.append("credit " + sig)
            self.started[sig].set()
            if sig in self.failing:
                raise self.failing[sig]
            self.credited.append(sig)
            if sig not in self.receipts:
                live.save(self.base / "credits" / (sig + ".json"), {"signature": sig})
                return None

            def six_receipts():
                self.receipts[sig]()
                self.events.append("receipts " + sig)
                live.save(self.base / "credits" / (sig + ".json"), {"signature": sig})

            return six_receipts

        def deposit_location(tx, sig, vault):
            if sig in self.not_deposits:
                raise Invalid("finalized transaction contains no deposit to the configured vault")

        modules = {
            "relay-service.py": {"configure": lambda *args, **kwargs: cfg, "Relay": lambda cfg: object()},
            "mainnet-capture.py": {"credit": credit, "Missed": Missed},
        }
        sys.modules["source"].deposit_location = deposit_location
        self.relay.runpy = types.SimpleNamespace(run_path=lambda path: modules[Path(path).name])
        self.relay.sol = self.sol
        self.relay.pda = lambda program, seed: "custody"
        self.relay.time = types.SimpleNamespace(time=lambda: 0.0, sleep=self.sleep)

    def sol(self, url, method, *params):
        if method == "getSignaturesForAddress":
            return custody_page(self.rows, params[1])
        if method == "getTransaction":
            return {"slot": next(r["slot"] for r in self.rows if r["signature"] == params[0])}
        raise AssertionError(method)

    def sleep(self, seconds):
        raise Stop()

    def run_once(self):
        with self.assertRaises(Stop):
            self.relay.run()

    def test_final_rejection_and_non_deposit_are_filed_not_crash_looped(self):
        self.rows = [row("C", self.ANCHOR + 30), row("B", self.ANCHOR + 20), row("A", self.ANCHOR + 10)]
        self.failing["A"] = Rejected("ledger rejected signed transaction 00ab: solana_source_mismatch",
                                  "solana_source_mismatch")
        self.not_deposits.add("B")
        self.run_once()
        missed = live.read(self.base / "missed" / "A.json")
        self.assertIn("solana_source_mismatch", missed["reason"])
        self.assertEqual(live.read(self.base / "skipped" / "B.json")["slot"], self.ANCHOR + 20)
        self.assertEqual(self.credited, ["C"])
        self.assertEqual(live.read(self.base / "relay" / "custody-cursor.json")["signature"], "C")

    def assert_retried(self, sig):
        self.assertFalse((self.base / "missed" / (sig + ".json")).exists())
        self.assertFalse((self.base / "relay" / "custody-cursor.json").exists())
        self.assertTrue((self.base / "retrying" / (sig + ".json")).exists())

    def test_prover_and_configuration_failures_are_retried_not_filed(self):
        # Messages the relay library raises as Invalid for a crashed native host, a relay
        # configuration that differs from the governed route and a lagging validator 0.
        for message in ("native verifier refused captured witness",
                        "governed Solana route differs from relay configuration",
                        "Solana activation receipt mismatch"):
            with self.subTest(message=message):
                self.setUp()
                self.rows = [row("A", self.ANCHOR + 10)]
                self.failing["A"] = Invalid(message)
                with self.assertRaises(Invalid):
                    self.relay.run()
                self.assert_retried("A")

    def test_a_rejection_code_outside_the_final_list_is_retried(self):
        self.rows = [row("A", self.ANCHOR + 10)]
        self.failing["A"] = Rejected("ledger rejected signed transaction 00ab: sp1_proof_invalid", "sp1_proof_invalid")
        with self.assertRaises(Invalid):
            self.relay.run()
        self.assert_retried("A")

    def test_two_identical_failures_stop_the_relay_without_filing(self):
        self.rows = [row("A", self.ANCHOR + 10)]
        self.failing["A"] = Invalid("native verifier refused captured witness")
        with self.assertRaises(Invalid):
            self.relay.run()
        with self.assertRaises(SystemExit) as stop:
            self.relay.run()
        self.assertEqual(stop.exception.code, self.relay.RELAY_STOPPED)
        stopped = live.read(self.base / "relay-stopped.json")
        self.assertEqual((stopped["signature"], stopped["error"]), ("A", "native verifier refused captured witness"))
        self.assertFalse((self.base / "missed" / "A.json").exists())

    def test_a_different_failure_is_retried_again_and_a_credit_clears_the_retry(self):
        self.rows = [row("A", self.ANCHOR + 10)]
        self.failing["A"] = Invalid("native verifier refused captured witness")
        with self.assertRaises(Invalid):
            self.relay.run()
        self.failing["A"] = RuntimeError("mainnet proof credit pending")
        with self.assertRaises(RuntimeError):
            self.relay.run()
        del self.failing["A"]
        self.run_once()
        self.assertEqual(self.credited, ["A"])
        self.assertFalse((self.base / "retrying" / "A.json").exists())

    def test_relay_stops_at_the_next_epoch_until_the_reanchor(self):
        self.rows = [row("N", NEXT + 40), row("A", self.ANCHOR + 10)]
        self.run_once()
        self.assertEqual(self.credited, ["A"])
        self.assertFalse((self.base / "missed" / "N.json").exists())
        self.assertEqual(live.read(self.base / "relay" / "custody-cursor.json")["signature"], "A")

    def test_relay_reports_the_profile_it_loaded(self):
        self.run_once()
        self.assertEqual(live.read(self.base / "relay-running.json")["profile_hash"], "H5")

    def test_retryable_failure_still_restarts_on_the_same_row(self):
        self.rows = [row("A", self.ANCHOR + 10)]
        self.failing["A"] = RuntimeError("mainnet proof credit pending")
        with self.assertRaises(RuntimeError):
            self.relay.run()
        self.assertFalse((self.base / "missed" / "A.json").exists())
        self.assertFalse((self.base / "relay" / "custody-cursor.json").exists())

    def after(self, event, then=None):
        """A six-receipt wait that ends once `event` is set; it fails if the loop never gets there."""

        def wait():
            if not event.wait(5):
                raise RuntimeError("the receipt wait held up the next deposit")
            if then:
                then()

        return wait

    def cursor(self):
        checkpoint = self.base / "relay" / "custody-cursor.json"
        return live.read(checkpoint)["signature"] if checkpoint.exists() else None

    def test_the_loop_hands_its_finalized_transaction_to_the_job(self):
        self.rows = [row("A", self.ANCHOR + 10)]
        self.run_once()
        held = live.read(self.base / "finalized-transactions" / "A.json")
        self.assertEqual(held["slot"], self.ANCHOR + 10)

    def test_next_deposit_starts_while_an_earlier_one_waits_for_its_receipts(self):
        self.rows = [row("B", self.ANCHOR + 20), row("A", self.ANCHOR + 10)]
        self.receipts["A"] = self.after(self.started["B"])
        self.run_once()
        self.assertEqual(self.events, ["credit A", "credit B", "receipts A"])
        self.assertTrue((self.base / "credits" / "A.json").exists())
        self.assertEqual(self.cursor(), "B")

    def test_checkpoint_never_passes_a_deposit_still_waiting_for_its_receipts(self):
        self.rows = [row("C", self.ANCHOR + 30), row("B", self.ANCHOR + 20), row("A", self.ANCHOR + 10)]
        b_done, seen = threading.Event(), []
        self.receipts["B"] = self.after(self.started["C"], b_done.set)
        self.receipts["A"] = self.after(b_done, lambda: seen.append(self.cursor()))
        self.run_once()
        self.assertEqual(seen, [None])
        self.assertEqual(self.cursor(), "C")
        self.assertEqual(self.events[-2:], ["receipts B", "receipts A"])

    def test_a_failed_receipt_wait_is_retried_on_the_same_row(self):
        self.rows = [row("B", self.ANCHOR + 20), row("A", self.ANCHOR + 10)]

        def disagree():
            self.started["B"].wait(5)
            raise RuntimeError("validators disagree")

        self.receipts["A"] = disagree
        with self.assertRaisesRegex(RuntimeError, "validators disagree"):
            self.relay.run()
        self.assertEqual(live.read(self.base / "retrying" / "A.json")["error"], "validators disagree")
        self.assertIsNone(self.cursor())
        self.assertFalse((self.base / "credits" / "A.json").exists())
        # After the restart the accepted job only waits for its receipts again.
        self.setUp()
        self.rows = [row("B", self.ANCHOR + 20), row("A", self.ANCHOR + 10)]
        self.receipts["A"] = lambda: None
        live.save(self.base / "retrying" / "A.json", dict(signature="A", slot=self.ANCHOR + 10, error="x"))
        self.run_once()
        self.assertFalse((self.base / "retrying" / "A.json").exists())
        self.assertEqual(self.cursor(), "B")

    def test_a_failed_receipt_wait_stops_the_next_submit(self):
        self.rows = [row("C", self.ANCHOR + 30), row("B", self.ANCHOR + 20), row("A", self.ANCHOR + 10)]
        release, b_failed = threading.Event(), threading.Event()
        self.addCleanup(release.set)

        def disagree():
            b_failed.set()
            raise RuntimeError("validators disagree")

        self.receipts.update(A=lambda: release.wait(5), B=disagree)
        sol = self.sol

        def sol_after_b_fails(url, method, *params):
            if method == "getTransaction" and params[0] == "C":
                b_failed.wait(5)
                threading.Event().wait(0.2)  # let B's wait finish failing
            return sol(url, method, *params)

        self.relay.sol = sol_after_b_fails
        with self.assertRaisesRegex(RuntimeError, "validators disagree"):
            self.relay.run()
        self.assertEqual(self.events[:2], ["credit A", "credit B"])
        self.assertNotIn("credit C", self.events)
        self.assertEqual(live.read(self.base / "retrying" / "B.json")["error"], "validators disagree")
        self.assertIsNone(self.cursor())

    def test_a_restart_does_not_rerun_rows_filed_behind_an_unsettled_wait(self):
        # The relay exited while A waited for its receipts; B and C were filed meanwhile, and a
        # kill between record() and settled() left a retry record for B.
        self.rows = [row("C", self.ANCHOR + 30), row("B", self.ANCHOR + 20), row("A", self.ANCHOR + 10)]
        live.save(self.base / "missed" / "B.json", dict(signature="B", slot=self.ANCHOR + 20, reason="x"))
        live.save(self.base / "retrying" / "B.json", dict(signature="B", slot=self.ANCHOR + 20, error="x"))
        live.save(self.base / "skipped" / "C.json", dict(signature="C", slot=self.ANCHOR + 30, reason="x"))
        self.receipts["A"] = lambda: None
        self.run_once()
        self.assertEqual(self.credited, ["A"])
        self.assertFalse((self.base / "credits" / "B.json").exists())
        self.assertFalse((self.base / "credits" / "C.json").exists())
        self.assertFalse((self.base / "retrying" / "B.json").exists())
        self.assertEqual(self.cursor(), "C")

    def test_receipt_waits_off_the_loop_are_bounded(self):
        self.relay.RECEIPT_WAITS = 2
        self.rows = [row("C", self.ANCHOR + 30), row("B", self.ANCHOR + 20), row("A", self.ANCHOR + 10)]
        running, peak, lock = [0], [], threading.Lock()

        def slow():
            with lock:
                running[0] += 1
            threading.Event().wait(0.5)
            with lock:
                running[0] -= 1

        self.receipts.update(A=slow, B=slow)
        original = self.relay.runpy.run_path("mainnet-capture.py")["credit"]

        def credit(*args):
            with lock:
                peak.append(running[0])
            return original(*args)

        self.relay.runpy.run_path("mainnet-capture.py")["credit"] = credit
        self.run_once()
        self.assertEqual(self.cursor(), "C")
        self.assertLess(peak[-1], self.relay.RECEIPT_WAITS)


OLD_ANCHOR = FIRST + 193929


def route_profile(route_epoch, anchor_epoch, anchor_slot, activation_height):
    return {
        "schema": "postfiat.vault_bridge.route_profile.v1", "route_id": "solana-mainnet-usdc-v1",
        "asset_id": "aa", "source_chain_id": 0, "vault_address": "11", "vault_runtime_code_hash": "",
        "token_address": "22", "token_runtime_code_hash": "", "route_epoch": route_epoch,
        "verifier_kind": "sp1-solana-alpenglow-finality-v1", "evidence_tier": "receipt-proven",
        "verifier_policy_hash": "", "verifier_program_vkey": "0x01", "verifier_proof_encoding": "groth16",
        "max_proof_bytes": 4096, "max_public_values_bytes": 16384, "max_snapshot_age_blocks": 100,
        "challenge_window_blocks": 1, "max_epoch_gap_blocks": 100, "settle_deadline_blocks": 10,
        "min_challenge_bond": 0, "min_attestations": 0, "minimum_confirmations": 0,
        "activation_height": activation_height, "expires_at_height": 4294967295,
        "solana": {"genesis_hash": "33", "shred_version": 50093, "handoff_program_vkey": "0x02",
                   "slots_per_epoch": SLOTS, "epoch_warmup": False, "recorder_program": "44",
                   "anchor_slot": anchor_slot, "route_binding": "55",
                   "anchor": {"epoch": anchor_epoch, "validator_set_hash": f"set{anchor_epoch}",
                              "total_stake": "1000", "known_key_table_hash": f"keys{anchor_epoch}"}},
    }


class FakeCluster:
    def __init__(self, test):
        self.test = test

    def call(self, method, *params):
        t = self.test
        if method == "getEpochInfo":
            return {"epoch": t.tip // SLOTS, "absoluteSlot": t.tip, "slotIndex": t.tip % SLOTS, "slotsInEpoch": SLOTS}
        if method == "getEpochSchedule":
            return {"slotsPerEpoch": SLOTS, "warmup": False, "firstNormalEpoch": 0, "firstNormalSlot": 0}
        if method == "getSignaturesForAddress":
            t.custody_reads += 1
            return custody_page(t.rows, params[1])
        raise AssertionError(method)


class ReanchorTest(unittest.TestCase):
    """reanchor-mainnet.py main() against a fake cluster, fake chain and fake systemd."""

    def setUp(self):
        self.base = Path(tempfile.mkdtemp())
        self.rm = load("reanchor-mainnet")
        self.rm.BASE = self.base
        self.rm.RELAY = self.base / "relay"
        self.tip = NEXT + 3000
        self.capture = NEXT + 2000  # the certified bank the observer yields in the new epoch
        self.rows = []
        self.custody_reads = 0
        self.governed = []
        self.restarts = []
        self.after_governance = []
        self.govern_fail = None
        old = route_profile(5, EPOCH, OLD_ANCHOR, 1709)
        self.old_hash = self.rm.profile_hash(old)
        live.save(self.base / "activation.json",
                  {"tx_id": "T5", "height": 1709, "validators": ["v"], "profile": old, "profile_hash": self.old_hash})
        live.save(self.base / "wallet-relay.json", {"profile_hash": self.old_hash})
        live.save(self.base / "relay-running.json", {"profile_hash": self.old_hash})
        archive = self.base / "archive"

        def anchor(c):
            (archive / str(self.capture)).mkdir(parents=True, exist_ok=True)
            return archive / str(self.capture)

        go = {
            "Cluster": lambda args: FakeCluster(self), "URL": "fake-rpc", "DIRECTORY": self.base,
            "pda": lambda key, seed: "custody", "load": lambda path: mock.MagicMock(),
            "anchor": anchor, "finalized_anchor": lambda c, archive: {"slot": int(archive.name)},
        }
        self.rm.runpy = types.SimpleNamespace(run_path=lambda path: go)
        self.rm.fleet = lambda: {"block_height": 1800, "build_git_revision": "3fe988e6"}
        self.rm.profile = lambda url, archive, height, mainnet: route_profile(
            1, self.tip // SLOTS, int(archive.name), height)
        self.rm.recover = lambda p: None
        self.rm.govern = self.govern
        self.rm.receipt_at = lambda tx, height: ["v"]
        self.rm.subprocess = types.SimpleNamespace(run=lambda args, check: self.restarts.append(args))
        self.now = 1_000_000.0
        self.rm.time = types.SimpleNamespace(time=lambda: self.now)
        self.rm.rpc = self.chain_rpc
        # The fake ledger: blocks by height, governance batches and receipts (oldest first).
        self.blocks = {}
        self.batches = {}
        self.receipts = []
        argv = mock.patch.object(sys, "argv", ["reanchor-mainnet.py"])
        argv.start()
        self.addCleanup(argv.stop)

    def govern(self, kind, p, compatible=None):
        self.governed.append(p)
        if self.govern_fail:
            error, self.govern_fail = self.govern_fail, None
            raise error
        self.commit("T6", 1801, p)
        self.rows[:0] = self.after_governance
        return {"tx_id": "T6", "height": 1801, "validators": ["v"], "profile": p,
                "profile_hash": self.rm.profile_hash(p)}

    def commit(self, tx, height, p, accepted=True, batch=None):
        """One route activation in the governance batch at height; its amendment id is tx."""
        batch = batch or tx
        self.blocks[height] = {"height": height, "batch_kind": "governance", "batch_id": batch}
        kind = f"vault_bridge_route_profile_activation:{self.rm.profile_hash(p)}"
        self.batches.setdefault(batch, []).append({"profile": p, "amendment": {"amendment_id": tx, "kind": kind}})
        message = "vault bridge route profile activated" if accepted else "Solana route revision must advance"
        self.receipts.append({"tx_id": tx, "accepted": accepted, "message": message})

    def chain_rpc(self, i, method, **params):
        """Validator RPC: receipts, tx, blocks and the governance batch archive."""
        if method == "receipts":
            found = [r for r in self.receipts if params.get("tx_id") in (None, r["tx_id"])]
            return found[-params.get("limit", 512):]
        if method == "tx":
            height = next(b["height"] for b in self.blocks.values()
                          if any(a["amendment"]["amendment_id"] == params["tx_id"]
                                 for a in self.batches.get(b["batch_id"], [])))
            return {"confirmed": True, "block": {"header": self.blocks[height]}}
        if method == "blocks":
            heights = sorted(h for h in self.blocks if h >= params["from_height"])[: params["limit"]]
            return [{"header": self.blocks[h]} for h in heights]
        if method == "batch_archive":
            payload = {"vault_bridge_route_profile_activations": self.batches[params["batch_id"]]}
            return [{"batch_id": params["batch_id"], "payload_json": json.dumps(payload)}]
        raise AssertionError(method)

    def file(self, kind, sig):
        live.save(self.base / kind / (sig + ".json"), {"signature": sig})

    def test_gate_ignores_skipped_and_rejected_rows(self):
        self.rows = [row("S", OLD_ANCHOR + 200), row("R", OLD_ANCHOR + 100)]
        self.file("skipped", "S")
        self.file("missed", "R")
        self.rm.main()
        self.assertEqual(len(self.governed), 1)

    def test_gate_waits_for_an_open_old_epoch_deposit(self):
        self.rows = [row("D", OLD_ANCHOR + 100)]
        with self.assertRaises(SystemExit) as stop:
            self.rm.main()
        self.assertIn("waiting for the relay to finish D", str(stop.exception))
        self.assertEqual(self.governed, [])


    def test_gate_reads_past_the_first_page(self):
        # Sixty failed custody transactions in the new epoch push the open deposit off one page.
        self.rm.PAGE = 25
        self.rows = [row(f"X{i}", NEXT + 900 - i, err={"InstructionError": [0, "Custom"]}) for i in range(60)]
        self.rows.append(row("D", OLD_ANCHOR + 100))
        with self.assertRaises(SystemExit) as stop:
            self.rm.main()
        self.assertIn("waiting for the relay to finish D", str(stop.exception))
        self.assertGreater(self.custody_reads, 1)

    def test_missed_filing_rereads_custody_after_governance(self):
        # A same-epoch re-anchor cannot move back to the epoch start, so a gap remains to file.
        sys.argv.append("--same-epoch")
        self.tip = OLD_ANCHOR + 3000
        self.capture = OLD_ANCHOR + 2000
        self.after_governance = [row("LATE", self.capture + 5), row("GAP", OLD_ANCHOR + 500)]
        self.rm.main()
        missed = live.read(self.base / "missed" / "GAP.json")
        self.assertIn(f"below re-anchor slot {self.capture}", missed["reason"])
        self.assertFalse((self.base / "missed" / "LATE.json").exists())

    def test_new_epoch_deposits_made_before_the_reanchor_are_not_stranded(self):
        self.after_governance = [row("GAP", NEXT + 500)]
        self.rm.main()
        pins = live.read(self.base / "activation.json")["profile"]["solana"]
        self.assertEqual((pins["anchor_slot"], pins["anchor"]["epoch"]), (NEXT, EPOCH + 1))
        # The validator set still comes from the certified bank the observer captured.
        self.assertEqual(pins["anchor"]["validator_set_hash"], f"set{EPOCH + 1}")
        self.assertFalse((self.base / "missed" / "GAP.json").exists())

    def test_readback_names_the_certified_bank_and_the_pinned_slot(self):
        self.rm.main()
        readback = live.read(self.base / f"anchor-readback-{self.capture}.json")
        self.assertEqual((readback["slot"], readback.get("anchor_slot")), (self.capture, NEXT))


    def test_second_run_is_refused_while_one_runs(self):
        import fcntl

        with (self.base / "reanchor.lock").open("a") as held:
            fcntl.flock(held, fcntl.LOCK_EX)
            with self.assertRaises(SystemExit) as stop:
                self.rm.main()
        self.assertIn("another re-anchor is running", str(stop.exception))
        self.assertEqual(self.governed, [])

    def test_anchored_route_restarts_services_on_a_stale_profile(self):
        self.tip = FIRST + 300000  # still epoch 1091: nothing to re-anchor
        live.save(self.base / "wallet-relay.json", {"profile_hash": "an-older-profile"})
        self.rm.main()
        self.assertEqual(len(self.restarts), 1)
        self.assertIn("pft-sol-mainnet-wallet-api", self.restarts[0])
        self.assertEqual(self.governed, [])

    def test_anchored_route_with_current_services_restarts_nothing(self):
        self.tip = FIRST + 300000
        self.rm.main()
        self.assertEqual(self.restarts, [])


    def test_adopts_a_committed_revision_whose_local_evidence_was_lost(self):
        committed = route_profile(6, EPOCH + 1, NEXT + 1500, 1790)
        self.commit("T6", 1801, committed)
        self.rm.main()
        self.assertEqual(self.governed, [])
        activation = live.read(self.base / "activation.json")
        self.assertEqual((activation["tx_id"], activation["profile"]), ("T6", committed))
        self.assertEqual(activation["profile_hash"], self.rm.profile_hash(committed))
        self.assertEqual(len(self.restarts), 1)

    def test_plan_is_kept_for_the_epoch_when_a_newer_capture_appears(self):
        self.govern_fail = RuntimeError("tip advanced during signing")
        with self.assertRaises(RuntimeError):
            self.rm.main()
        planned = self.governed[0]["solana"]["anchor_slot"]
        self.capture += 600  # the relay archived a newer certified block meanwhile
        self.rm.main()
        self.assertEqual(self.governed[1]["solana"]["anchor_slot"], planned)
        self.assertEqual(self.governed[1], self.governed[0])


    def test_gate_passes_rows_the_relay_checkpoint_is_past(self):
        # Rows the relay passed before it began recording skips leave no file behind.
        self.rows = [row("LAST", OLD_ANCHOR + 150), row("OLD", OLD_ANCHOR + 100)]
        live.save(self.base / "relay" / self.old_hash / "custody-cursor.json",
                  {"signature": "LAST", "slot": OLD_ANCHOR + 150})
        self.rm.main()
        self.assertEqual(len(self.governed), 1)

    def test_restarts_only_the_stale_service(self):
        self.tip = FIRST + 300000
        live.save(self.base / "wallet-relay.json", {"profile_hash": "an-older-profile"})
        self.rm.main()
        self.assertEqual(self.restarts, [["systemctl", "--user", "restart", "pft-sol-mainnet-wallet-api"]])

    def test_a_service_that_stays_stale_is_not_restarted_again_within_the_back_off(self):
        # The wallet API fails before it rewrites its status file: its file stays stale.
        self.tip = FIRST + 300000
        live.save(self.base / "wallet-relay.json", {"profile_hash": "an-older-profile"})
        self.rm.main()
        self.now += 120  # the next timer run
        self.rm.main()
        self.assertEqual(len(self.restarts), 1)
        self.now += self.rm.RESTART_BACKOFF_SECONDS
        self.rm.main()
        self.assertEqual(len(self.restarts), 2)

    def test_a_new_profile_restarts_at_once_despite_the_back_off(self):
        self.tip = FIRST + 300000
        live.save(self.base / "relay-running.json", {"profile_hash": "an-older-profile"})
        self.rm.main()
        self.tip = NEXT + 3000
        self.rm.main()  # re-anchors: both services now run an older profile
        self.assertEqual(self.restarts[-1][3:], ["pft-sol-mainnet-relay", "pft-sol-mainnet-wallet-api"])

    def test_adoption_takes_the_profile_of_the_accepted_transactions_own_amendment(self):
        # One governance batch holds a rejected activation and the accepted one.
        rejected = route_profile(6, EPOCH + 1, NEXT + 900, 1790)
        committed = route_profile(6, EPOCH + 1, NEXT + 1500, 1790)
        self.commit("T6-REJECTED", 1801, rejected, accepted=False, batch="G1801")
        self.commit("T6", 1801, committed, batch="G1801")
        self.rm.main()
        activation = live.read(self.base / "activation.json")
        self.assertEqual((activation["tx_id"], activation["profile"]), ("T6", committed))

    def test_adoption_refuses_a_profile_that_differs_from_its_amendment(self):
        committed = route_profile(6, EPOCH + 1, NEXT + 1500, 1790)
        self.commit("T6", 1801, committed)
        self.batches["T6"][0]["amendment"]["kind"] = "vault_bridge_route_profile_activation:" + "00" * 48
        with self.assertRaises(RuntimeError) as error:
            self.rm.main()
        self.assertIn("amendment", str(error.exception))
        self.assertEqual(self.governed, [])

    def test_adoption_reads_past_the_newest_receipts(self):
        committed = route_profile(6, EPOCH + 1, NEXT + 1500, 1790)
        self.commit("T6", 1801, committed)
        for n in range(600):  # later transactions push the activation out of a 512-receipt tail
            self.blocks[1802 + n] = {"height": 1802 + n, "batch_kind": "transparent", "batch_id": f"B{n}"}
            self.receipts.append({"tx_id": f"X{n}", "accepted": True, "message": "accepted"})
        self.rm.main()
        self.assertEqual(self.governed, [])
        self.assertEqual(live.read(self.base / "activation.json")["tx_id"], "T6")

    def test_gate_holds_while_a_deposit_is_retrying(self):
        # The relay is retrying D; the public RPC no longer lists it, so only its record shows it.
        live.save(self.base / "retrying" / "D.json", {"signature": "D", "slot": OLD_ANCHOR + 100, "error": "x"})
        with self.assertRaises(SystemExit) as stop:
            self.rm.main()
        self.assertIn("retrying D", str(stop.exception))
        self.assertEqual(self.governed, [])



class ReceiptLookupTest(unittest.TestCase):
    """The six-receipt wait asks again while validator 0 has not stored the receipt yet."""

    def setUp(self):
        import runpy
        self.height = runpy.run_path(str(HERE / "mainnet-capture.py"))["committed_height"]

    def test_a_receipt_not_stored_yet_is_asked_for_again(self):
        replies = [RuntimeError("0 tx: {'code': 'rpc_tx_not_found', 'message': 'has no receipt'}")] * 2 + [{"block": {"header": {"height": 9}}}]

        def lookup():
            reply = replies.pop(0)
            if isinstance(reply, Exception):
                raise reply
            return reply

        self.assertEqual(self.height("t", pause=0, lookup=lookup), 9)
        self.assertEqual(replies, [])

    def test_other_errors_and_a_receipt_that_never_appears_fail(self):
        def refused():
            raise RuntimeError("0 tx: {'code': 'rpc_unavailable'}")

        with self.assertRaisesRegex(RuntimeError, "rpc_unavailable"):
            self.height("t", pause=0, lookup=refused)
        calls = []

        def missing():
            calls.append(1)
            raise RuntimeError("0 tx: {'code': 'rpc_tx_not_found'}")

        with self.assertRaisesRegex(RuntimeError, "rpc_tx_not_found"):
            self.height("t", attempts=3, pause=0, lookup=missing)
        self.assertEqual(len(calls), 3)

if __name__ == "__main__":
    unittest.main()
