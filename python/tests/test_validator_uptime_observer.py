"""The uptime observer against a simulated chain, three weeks of it.

Three observers read the same chain every ten minutes. The chain has a
validator that is healthy but slow (left out of certificates, present in the
proposers' vote records), one that goes offline for good, and one that goes
offline for three days and comes back. One observer cannot reach the vote
records, and has an outage of its own.

At the end the three sign with the real `postfiat-node`, the attestations are
bundled into a feed, and the feed is read twice: by the node, and by the
Python reference with another ML-DSA-65 implementation. Both must agree.
"""

from __future__ import annotations

import datetime as dt
import hashlib
import importlib.machinery
import importlib.util
import json
import os
from pathlib import Path
import socketserver
import subprocess
import sys
import threading
import unittest

REPO = Path(__file__).resolve().parents[2]
SCRIPTS = REPO / "scripts"
sys.path.insert(0, str(SCRIPTS))
import validator_uptime_contract as contract  # noqa: E402

UTC = dt.timezone.utc
START = dt.datetime(2026, 11, 1, 10, 0, tzinfo=UTC)
STEP = dt.timedelta(minutes=10)
VALIDATORS = ["validator-0", "validator-1", "validator-2", "validator-3"]
KEYS = {v: hashlib.shake_256(v.encode()).hexdigest(1952) for v in VALIDATORS}


def load_observer():
    loader = importlib.machinery.SourceFileLoader("validator_uptime_observer", str(SCRIPTS / "validator-uptime-observer.py"))
    spec = importlib.util.spec_from_loader(loader.name, loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


observer = load_observer()


def node_binary() -> str:
    configured = os.environ.get("POSTFIAT_NODE_BIN")
    if configured:
        return configured
    target = Path(os.environ.get("CARGO_TARGET_DIR", REPO / "target"))
    subprocess.run(
        ["cargo", "build", "--offline", "--locked", "--release", "-p", "postfiat-node", "--bin", "postfiat-node"],
        cwd=REPO,
        check=True,
    )
    return str(target / "release/postfiat-node")


class Chain:
    """A chain as an observer sees it over RPC: status, validators, blocks, block_vote_records."""

    def __init__(self) -> None:
        self.blocks: list[dict] = []
        self.records: dict[int, dict] = {}
        self.lock = threading.Lock()
        self.chain_id = "postfiat-observer-test-1"
        self.genesis_hash = hashlib.sha3_384(b"observer test genesis").hexdigest()
        self.add_block(set(VALIDATORS), set())  # height 1 exists before any observer starts

    def add_block(self, in_certificate: set[str], late: set[str]) -> None:
        """One block: who is in its certificates, and who voted too late for them."""
        with self.lock:
            height = len(self.blocks) + 1
            proposer = VALIDATORS[height % len(VALIDATORS)]
            block_hash = hashlib.sha3_384(f"block {height}".encode()).hexdigest()
            parent = self.blocks[-1]["block_hash"] if self.blocks else "0" * 96
            votes = [{"validator": v, "phase": "precommit"} for v in sorted(in_certificate)]
            self.blocks.append({
                "height": height,
                "block_hash": block_hash,
                "parent_hash": parent,
                "proposer": proposer,
                "consensus_v2_commit": {
                    "precommit_qc": {"votes": votes, "domain": {"committee_epoch": 3}},
                    "prepare_qc": {"votes": votes},
                },
            })
            self.records[height] = {
                "schema": "postfiat.block_vote_record.v1",
                "block_height": height,
                "block_hash": block_hash,
                "proposer": proposer,
                "entries": [
                    {
                        "validator": v,
                        "outcome": "accepted" if v in in_certificate | late else "no_answer",
                        "answered_after_quorum": v in late,
                    }
                    for v in VALIDATORS
                ],
            }

    def handle(self, method: str, params: dict, records: bool) -> object:
        with self.lock:
            if method == "status":
                return {"chain_id": self.chain_id, "genesis_hash": self.genesis_hash,
                        "block_height": len(self.blocks), "block_tip_hash": self.blocks[-1]["block_hash"]}
            if method == "validators":
                return {"registry_root": hashlib.sha3_384(b"registry").hexdigest(),
                        "validators": [{"node_id": v, "public_key_hex": KEYS[v], "algorithm_id": "ML-DSA-65"} for v in VALIDATORS]}
            if method == "blocks":
                start, limit = params["from_height"], params.get("limit", 8)
                assert limit <= 8, "the node serves at most 8 blocks a request"
                return [{"header": b} for b in self.blocks[start - 1 : start - 1 + limit]]
            if method == "block_vote_records" and records:
                start, limit = params["from_height"], params.get("limit", 8)
                return {"schema": "postfiat-block-vote-records-v1", "records": [
                    {"height": h, "available": h in self.records, "record": self.records.get(h)}
                    for h in range(start, start + limit)
                ]}
            raise observer.ObserverError(f"rpc_method_not_allowed: {method}")


class InProcessSource:
    def __init__(self, chain: Chain, records: bool, validator_id: str | None = None) -> None:
        self.chain, self.records, self.spec, self.validator_id = chain, records, "in-process", validator_id

    def call(self, method: str, params: dict | None = None) -> object:
        return self.chain.handle(method, params or {}, self.records)


def serve(chain: Chain, records: bool) -> tuple[socketserver.ThreadingTCPServer, str]:
    """The same chain over the node's wire format: one JSON request per line."""

    class Handler(socketserver.StreamRequestHandler):
        def handle(self) -> None:
            request = json.loads(self.rfile.readline())
            try:
                reply = {"ok": True, "result": chain.handle(request["method"], request.get("params") or {}, records)}
            except observer.ObserverError as exc:
                reply = {"ok": False, "error": {"code": "rpc_method_not_allowed", "message": str(exc)}}
            self.wfile.write(json.dumps(reply).encode() + b"\n")

    socketserver.ThreadingTCPServer.allow_reuse_address = True
    server = socketserver.ThreadingTCPServer(("127.0.0.1", 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server, f"tcp://127.0.0.1:{server.server_address[1]}"


def own_rpcs(chain: Chain) -> list[InProcessSource]:
    """Every validator's own RPC: the only place its vote records are taken from."""
    return [InProcessSource(chain, True, validator_id=v) for v in VALIDATORS]


def day(n: int) -> dt.date:
    """Day 1 is 1 November."""
    return dt.date(2026, 11, n)


def voters_at(now: dt.datetime, height: int) -> tuple[set[str], set[str]]:
    """Who is in the certificates of a block made at `now`, and who voted too late for them."""
    today = now.date()
    in_certificate, late = {"validator-0"}, set()
    # validator-1 is healthy but slow: every fifth block closes without it.
    (late if height % 5 == 0 else in_certificate).add("validator-1")
    # validator-2 goes offline on 5 November and never returns.
    if today < day(5):
        in_certificate.add("validator-2")
    # validator-3 is offline on 5, 6 and 7 November.
    if not day(5) <= today <= day(7):
        in_certificate.add("validator-3")
    return in_certificate, late


class ObserverSimulation(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.chain = Chain()
        # A and B read the vote records. C cannot reach them and sees certificates only.
        cls.sources = {"a": own_rpcs(cls.chain), "b": own_rpcs(cls.chain), "c": [InProcessSource(cls.chain, False)]}
        # The validators were ratified the day the observers started.
        activation = {v: START.date().isoformat() for v in VALIDATORS}
        cls.states: dict[str, dict | None] = {"a": None, "b": None, "c": None}
        cls.rows: dict[str, dict[str, dict]] = {}
        cls.history: dict[str, list[tuple[str, str, str]]] = {v: [] for v in VALIDATORS}
        now = START
        cls.end = dt.datetime(2026, 11, 21, 1, 0, tzinfo=UTC)
        while now <= cls.end:
            today = now.date()
            idle = today == day(13)  # the chain makes no block on 13 November
            # A block every 20 minutes, never in the ten minutes around midnight,
            # except once: at 00:00 on 10 November the head moves across midnight.
            minute_of_day = now.hour * 60 + now.minute
            crosses_midnight = today == day(10) and minute_of_day == 0
            if not idle and (crosses_midnight or (minute_of_day % 20 == 10)):
                cls.chain.add_block(*voters_at(now, len(cls.chain.blocks) + 1))
            for name in "abc":
                # C is down for two hours on 12 November.
                if name == "c" and today == day(12) and 8 <= now.hour < 10:
                    continue
                state = observer.poll(cls.states[name], cls.sources[name], "l1-testnet", now, activation)
                rows = observer.build_rows(state, now)
                # A JSON round trip between runs must change nothing (per-day counters once got lost here).
                cls.states[name] = json.loads(json.dumps(state))
                cls.rows[name] = {row["validator_id"]: row for row in rows}
                if name == "a":
                    for row in rows:
                        entry = (today.isoformat(), row["access_state"], row["access_state_reason"])
                        log = cls.history[row["validator_id"]]
                        if not log or log[-1][1:] != entry[1:]:
                            log.append(entry)
            now += STEP
        cls.now = now - STEP

    def transitions(self, vid: str) -> list[tuple[str, str, str]]:
        return self.history[vid]

    def test_a_healthy_validator_is_active_throughout(self) -> None:
        self.assertEqual([t[1:] for t in self.transitions("validator-0")], [("active", "day0_ramp_in")])
        row = self.rows["a"]["validator-0"]
        self.assertEqual(row["first_active_date"], "2026-11-01")
        self.assertEqual(row["mean_daily_participation_bps"], 10000)

    def test_the_first_day_and_an_undatable_midnight_are_not_observed_days(self) -> None:
        days = {d["date"]: d for d in self.rows["a"]["validator-0"]["days"]}
        self.assertFalse(days["2026-11-01"]["complete"], "the observer started during this day")
        # The head moved between the 23:50 and the 00:00 read: neither day can be dated.
        self.assertFalse(days["2026-11-09"]["complete"])
        self.assertFalse(days["2026-11-10"]["complete"])
        self.assertIsNone(days["2026-11-10"]["participation_bps"])
        # A day without a block is complete and says nothing.
        self.assertTrue(days["2026-11-13"]["complete"])
        self.assertEqual(days["2026-11-13"]["committed_blocks"], 0)
        self.assertIsNone(days["2026-11-13"]["participation_bps"])
        self.assertEqual((days["2026-11-13"]["from_height"], days["2026-11-13"]["to_height"]), (None, None))
        # An ordinary day: 72 blocks, three an hour.
        self.assertEqual(days["2026-11-03"]["committed_blocks"], 72)
        self.assertEqual(self.rows["a"]["validator-0"]["observed_days"], 20 - 1 - 2 - 1)

    def test_vote_records_keep_a_slow_validator_whole_and_certificates_alone_do_not(self) -> None:
        with_records = self.rows["a"]["validator-1"]
        self.assertEqual(with_records["mean_daily_participation_bps"], 10000)
        self.assertEqual((with_records["access_state"], with_records["liveness_met"]), ("active", True))
        certificates_only = self.rows["c"]["validator-1"]
        self.assertLess(certificates_only["mean_daily_participation_bps"], 9500)
        self.assertEqual(certificates_only["access_state"], "revoked", "what counting certificates alone would do to it")
        some_day = next(d for d in certificates_only["days"] if d["date"] == "2026-11-03")
        self.assertEqual((some_day["committed_blocks"], some_day["signed_blocks"]), (72, 58))

    def test_an_offline_validator_lapses_then_is_revoked_when_grace_runs_out(self) -> None:
        self.assertEqual(self.transitions("validator-2"), [
            ("2026-11-01", "active", "day0_ramp_in"),
            # 5 November is the first day without a vote; it is evaluated on the 6th.
            ("2026-11-06", "lapsed", "below_liveness_threshold"),
            ("2026-11-20", "revoked", "grace_expired"),
        ])
        row = self.rows["a"]["validator-2"]
        self.assertEqual((row["finance_hub_access"], row["research_api_keys_active"], row["reward_eligible"]), (False, False, False))
        self.assertTrue(row["removal_proposal_eligible"])

    def test_three_passing_days_recover_a_validator(self) -> None:
        self.assertEqual(self.transitions("validator-3"), [
            ("2026-11-01", "active", "day0_ramp_in"),
            ("2026-11-06", "lapsed", "below_liveness_threshold"),
            # Back on the 8th. The 9th and 10th cannot be dated, so the three
            # passing days are the 8th, 11th and 12th, evaluated on the 13th.
            # It is still in its first 30 days, and the contract names that reason first.
            ("2026-11-13", "active", "day0_ramp_in"),
        ])
        row = self.rows["a"]["validator-3"]
        self.assertEqual(row["recovery_floor_date"], "2026-11-08")
        self.assertTrue(row["reward_eligible"])

    def test_an_observer_outage_costs_that_observer_one_day_and_nobody_their_access(self) -> None:
        mine = {d["date"]: d for d in self.rows["c"]["validator-0"]["days"]}
        theirs = {d["date"]: d for d in self.rows["a"]["validator-0"]["days"]}
        self.assertFalse(mine["2026-11-12"]["complete"])
        self.assertTrue(theirs["2026-11-12"]["complete"])
        self.assertEqual(self.rows["c"]["validator-0"]["access_state"], "active")

    def test_signed_bundled_and_read_by_two_implementations(self) -> None:
        import tempfile

        from dilithium_py.ml_dsa import ML_DSA_65
        from jsonschema import Draft202012Validator

        node = node_binary()
        chain = self.chain
        now = self.now + STEP
        servers = []
        with tempfile.TemporaryDirectory() as tmp_name:
            tmp = Path(tmp_name)
            observers = {"a": ("observer-us-east-1", "us-east", "group-a", True),
                         "b": ("observer-eu-central-2", "eu-central", "group-b", True),
                         "c": ("observer-ap-northeast-3", "ap-northeast", "group-c", False)}
            keyset = {"schema": "postfiat.observer_keyset.v1", "keyset_id": "postfiat-observer-keyset-test-v1",
                      "chain": {"network": "l1-testnet", "chain_id": chain.chain_id, "genesis_hash": chain.genesis_hash},
                      "threshold": 2, "total_observers": 3, "algorithm_id": "ML-DSA-65",
                      "signing_context": "postfiat-l1-v2/validator-uptime-feed/v1", "observers": []}
            attestation_files = []
            try:
                for name, (observer_id, region, group, records) in observers.items():
                    key_file = tmp / f"{name}.key.json"
                    created = json.loads(subprocess.run(
                        [node, "observer-key-create", "--key-file", str(key_file), "--observer-id", observer_id,
                         "--region", region, "--operator-group", group],
                        capture_output=True, text=True, check=True).stdout)
                    self.assertEqual(oct(key_file.stat().st_mode & 0o777), "0o600")
                    keyset["observers"].append({"observer_id": observer_id, "region": region, "operator_group": group,
                                                "public_key_hex": created["public_key_hex"]})
                    server, spec = serve(chain, records)
                    servers.append(server)
                    state_file = tmp / f"{name}.state.json"
                    state_file.write_text(json.dumps(self.states[name]))
                    output = tmp / f"{name}.attestation.json"
                    result = subprocess.run(
                        [sys.executable, str(SCRIPTS / "validator-uptime-observer.py"), "run", "--source", spec,
                         "--network", "l1-testnet", "--keyset-id", keyset["keyset_id"], "--key-file", str(key_file),
                         "--state-file", str(state_file), "--output", str(output), "--node-bin", node,
                         "--now", contract.format_utc(now)],
                        capture_output=True, text=True, check=False)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    attestation_files.append(str(output))
                    # The state on disk carries on from where the simulation stopped.
                    self.assertEqual(json.loads(state_file.read_text())["last_poll_at"], contract.format_utc(now))
            finally:
                for server in servers:
                    server.shutdown()
            keyset_file = tmp / "observer-keyset.json"
            keyset_file.write_text(json.dumps(keyset))
            feed_file = tmp / "feed.json"
            run = [sys.executable, str(SCRIPTS / "validator-uptime-observer.py")]
            bundle = subprocess.run(
                run + ["bundle", "--keyset-file", str(keyset_file), "--output", str(feed_file), "--node-bin", node,
                       "--now", contract.format_utc(now)] + [arg for f in attestation_files for arg in ("--attestation", f)],
                capture_output=True, text=True, check=False)
            self.assertEqual(bundle.returncode, 0, bundle.stderr)
            self.assertEqual(json.loads(bundle.stdout)["rejected_observers"], {})

            # Read by the node.
            verify = subprocess.run(
                run + ["verify", "--feed-file", str(feed_file), "--keyset-file", str(keyset_file), "--node-bin", node,
                       "--now", contract.format_utc(now)], capture_output=True, text=True, check=False)
            self.assertEqual(verify.returncode, 0, verify.stderr)
            by_node = json.loads(verify.stdout)
            self.assertEqual(len(by_node["verified_observers"]), 3)
            self.assertFalse(by_node["indeterminate"])

            # Read by the Python reference, with another ML-DSA-65 implementation.
            feed = json.loads(feed_file.read_text())
            schema = Draft202012Validator(json.loads(
                (REPO / "docs/specs/schemas/postfiat-validator-uptime-v1.schema.json").read_text()))
            Draft202012Validator(json.loads(
                (REPO / "docs/specs/schemas/postfiat-observer-keyset-v1.schema.json").read_text())).validate(keyset)
            by_reference = contract.verify_feed(
                feed, schema=schema, keyset=keyset, now_utc=now,
                verify_signature=lambda key, message, signature: ML_DSA_65.verify(
                    bytes.fromhex(key), message, bytes.fromhex(signature), ctx=contract.SIGNING_CONTEXT))
            self.assertEqual(by_reference["verified_observers"], by_node["verified_observers"])
            self.assertEqual(by_reference["validators"], feed["validators"])
            self.assertEqual([row["claim"] if "claim" in row else row for row in by_node["validators"]] and
                             [row["validator_id"] for row in by_node["validators"]],
                             [row["validator_id"] for row in by_reference["validators"]])

            rows = {row["validator_id"]: row for row in by_reference["validators"]}
            self.assertEqual({v: (r["access_state"], r["access_state_reason"]) for v, r in rows.items()}, {
                "validator-0": ("active", "day0_ramp_in"),
                # A and B carry the slow validator; C, who saw certificates only, is outvoted.
                "validator-1": ("active", "day0_ramp_in"),
                "validator-2": ("revoked", "grace_expired"),
                "validator-3": ("active", "day0_ramp_in"),
            })
            self.assertEqual(rows["validator-3"]["recovery_floor_date"], "2026-11-08")
            self.assertEqual(rows["validator-1"]["agreeing_observer_ids"], ["observer-eu-central-2", "observer-us-east-1"])
            self.assertEqual(len(rows["validator-0"]["agreeing_observer_ids"]), 3)

            # A changed byte in the served feed is refused by both.
            feed["validators"][2]["access_state"] = "active"
            tampered = tmp / "tampered.json"
            tampered.write_text(json.dumps(feed))
            refused = subprocess.run(
                run + ["verify", "--feed-file", str(tampered), "--keyset-file", str(keyset_file), "--node-bin", node,
                       "--now", contract.format_utc(now)], capture_output=True, text=True, check=False)
            self.assertNotEqual(refused.returncode, 0)
            with self.assertRaises(contract.FeedRejected):
                contract.verify_feed(feed, schema=schema, keyset=keyset, now_utc=now,
                                     verify_signature=lambda *_: True)


class ObserverRefusals(unittest.TestCase):
    def test_it_stops_rather_than_write_something_false(self) -> None:
        chain = Chain()
        source = InProcessSource(chain, True)
        state = observer.poll(None, [source], "l1-testnet", START)
        with self.assertRaisesRegex(observer.ObserverError, "clock went backwards"):
            observer.poll(json.loads(json.dumps(state)), [source], "l1-testnet", START - STEP)
        other = Chain()
        other.chain_id = "some-other-chain"
        with self.assertRaisesRegex(observer.ObserverError, "no source answers for this chain"):
            observer.poll(json.loads(json.dumps(state)), [InProcessSource(other, True)], "l1-testnet", START + STEP)
        # A source that is behind this observer is not used; the one ahead is.
        behind = Chain()
        chain.add_block(set(VALIDATORS), set())
        ahead = observer.poll(json.loads(json.dumps(state)), [InProcessSource(behind, True), source], "l1-testnet", START + STEP)
        self.assertEqual(ahead["last_height"], 2)
        with self.assertRaisesRegex(observer.ObserverError, "behind this observer"):
            observer.poll(json.loads(json.dumps(ahead)), [InProcessSource(behind, True)], "l1-testnet", START + 2 * STEP)
        # A chain that does not continue the one seen before.
        forked = Chain()
        forked.add_block(set(VALIDATORS), set())
        forked.blocks[1]["block_hash"] = "f" * 96  # another block 2 than the one this observer saw
        forked.add_block(set(VALIDATORS), set())
        with self.assertRaisesRegex(observer.ObserverError, "does not follow"):
            observer.poll(json.loads(json.dumps(ahead)), [InProcessSource(forked, True)], "l1-testnet", START + 2 * STEP)

    def test_a_vote_record_counts_only_for_its_own_block_and_from_its_proposer(self) -> None:
        chain = Chain()
        source = InProcessSource(chain, True)
        proposers = own_rpcs(chain)
        state = observer.poll(None, [source], "l1-testnet", dt.datetime(2026, 11, 2, 9, 0, tzinfo=UTC))
        state = observer.poll(state, [InProcessSource(chain, False)], "l1-testnet", dt.datetime(2026, 11, 2, 23, 55, tzinfo=UTC))
        # A whole day begins. Three blocks close without validator-1; the records say it voted late.
        now = dt.datetime(2026, 11, 3, 0, 5, tzinfo=UTC)
        state = observer.poll(state, [InProcessSource(chain, False)], "l1-testnet", now)
        for _ in range(3):
            chain.add_block({"validator-0", "validator-2", "validator-3"}, {"validator-1"})
        chain.records[2]["block_hash"] = "e" * 96          # about another block
        chain.records[3]["proposer"] = "validator-0"        # not from the block's proposer
        # A source that is nobody's own RPC, and a validator speaking for blocks
        # it did not propose, add nothing: every record here is somebody else's.
        elsewhere = [InProcessSource(chain, True), InProcessSource(chain, True, validator_id="validator-9")]
        state = observer.poll(state, elsewhere, "l1-testnet", now + STEP)
        blocks = state["days"]["2026-11-03"]["blocks"]
        self.assertEqual([blocks[h]["record"] for h in "234"], [False, False, False])
        self.assertFalse(any("validator-1" in blocks[h]["credited"] for h in "234"))
        state = observer.poll(json.loads(json.dumps(state)), proposers, "l1-testnet", now + STEP)
        blocks = state["days"]["2026-11-03"]["blocks"]
        self.assertNotIn("validator-1", blocks["2"]["credited"])
        self.assertNotIn("validator-1", blocks["3"]["credited"])
        self.assertIn("validator-1", blocks["4"]["credited"])
        self.assertEqual([blocks[h]["record"] for h in "234"], [False, False, True])
        # A record that arrives later is still taken, and never read twice.
        chain.records[2]["block_hash"] = chain.blocks[1]["block_hash"]
        state = observer.poll(json.loads(json.dumps(state)), proposers, "l1-testnet", now + 2 * STEP)
        self.assertIn("validator-1", state["days"]["2026-11-03"]["blocks"]["2"]["credited"])

    def test_a_restarted_observer_hands_out_no_new_ramp_in(self) -> None:
        chain = Chain()
        sources = own_rpcs(chain)
        now = dt.datetime(2026, 11, 2, 9, 0, tzinfo=UTC)
        # No saved state and no activation dates: the validators were in the
        # registry before this observer saw anything.
        state = observer.poll(None, sources, "l1-testnet", now)
        rows = {row["validator_id"]: row for row in observer.build_rows(state, now)}
        for vid in VALIDATORS:
            row = rows[vid]
            self.assertEqual([row["access_state"], row["access_state_reason"]], ["active", "insufficient_observed_days"])
            self.assertEqual([row["ramp_in_active"], row["reward_eligible"], row["finance_hub_access"]], [False, False, True])
            self.assertEqual(row["observed_days"], 0)
        # With the ratification dates, a validator ratified ten days ago is in its ramp-in and one from spring is not.
        dates = {"validator-0": "2026-10-23", "validator-1": "2026-04-01"}
        state = observer.poll(None, sources, "l1-testnet", now, dates)
        rows = {row["validator_id"]: row for row in observer.build_rows(state, now)}
        self.assertEqual([rows["validator-0"]["access_state_reason"], rows["validator-0"]["first_active_date"]],
                         ["day0_ramp_in", "2026-10-23"])
        self.assertEqual(rows["validator-1"]["access_state_reason"], "insufficient_observed_days")
        with self.assertRaisesRegex(observer.ObserverError, "after today"):
            observer.poll(None, sources, "l1-testnet", now, {"validator-0": "2026-11-03"})
        # A validator that enters the registry while the observer runs is new from that day.
        VALIDATORS.append("validator-new")
        KEYS["validator-new"] = hashlib.sha3_256(b"validator-new").hexdigest() * 61
        try:
            later = now + STEP
            state = observer.poll(json.loads(json.dumps(state)), sources, "l1-testnet", later)
            rows = {row["validator_id"]: row for row in observer.build_rows(state, later)}
            self.assertEqual([rows["validator-new"]["access_state_reason"], rows["validator-new"]["first_active_date"]],
                             ["day0_ramp_in", "2026-11-02"])
        finally:
            VALIDATORS.remove("validator-new")
            del KEYS["validator-new"]

    def test_a_block_is_scored_against_the_validators_of_its_own_certificate(self) -> None:
        chain = Chain()
        source = InProcessSource(chain, False)
        now = dt.datetime(2026, 11, 2, 9, 0, tzinfo=UTC)
        state = observer.poll(None, [source], "l1-testnet", now)
        chain.add_block(set(VALIDATORS), set())
        # This block's certificate was drawn from three validators only.
        chain.blocks[-1]["consensus_v2_commit"]["precommit_qc"]["validators"] = VALIDATORS[:3]
        chain.add_block(set(VALIDATORS), set())
        state = observer.poll(state, [source], "l1-testnet", now + STEP)
        blocks = state["days"]["2026-11-02"]["blocks"]
        self.assertEqual(blocks["2"]["registry"], sorted(VALIDATORS[:3]))
        self.assertEqual(blocks["2"]["credited"], sorted(VALIDATORS[:3]))
        self.assertEqual(blocks["3"]["registry"], sorted(VALIDATORS))

    def test_a_validator_long_gone_from_the_registry_is_dropped(self) -> None:
        chain = Chain()
        source = InProcessSource(chain, False)
        now = dt.datetime(2026, 11, 2, 9, 0, tzinfo=UTC)
        state = observer.poll(None, [source], "l1-testnet", now)
        observer.build_rows(state, now)
        gone = VALIDATORS.pop()
        try:
            seen = []
            for days in (1, 13, 16, 30, 32):
                at = now + dt.timedelta(days=days)
                # Each read starts from a recent one, so only the registry differs.
                state["last_poll_at"] = observer.contract.format_utc(at - STEP)
                state = observer.poll(json.loads(json.dumps(state)), [source], "l1-testnet", at)
                rows = {row["validator_id"]: row for row in observer.build_rows(state, at)}
                seen.append(rows[gone]["access_state"] if gone in rows else None)
            self.assertEqual(seen, ["lapsed", "lapsed", "revoked", "revoked", None])
            self.assertNotIn(gone, state["validators"])
        finally:
            VALIDATORS.append(gone)

    def test_an_observer_writes_nothing_into_a_validator_data_directory(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / "validator_keys.json").write_text("{}")
            with self.assertRaisesRegex(observer.ObserverError, "validator data directory"):
                observer.refuse_validator_data_dir(Path(tmp) / "observer")


if __name__ == "__main__":
    unittest.main()
