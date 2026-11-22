"""Relay steps before the proof: source lookups and the native check.

Fakes only: no Solana RPC, no prover, no host binary. Run from the repository root with
`python3 -m unittest discover -s tools/pfusdc-sol-relay -p 'test_*.py'`.
"""

import hashlib
import io
import json
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
import threading
import unittest
import urllib.error
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import jobs  # noqa: E402
import prover  # noqa: E402
import source  # noqa: E402
from storage import Invalid, load, save  # noqa: E402

ALPHABET = source.ALPHABET
SIG = "5" * 88
VAULT = "11" * 32


def b58(raw):
    number, out = int.from_bytes(raw, "big"), ""
    while number:
        number, rest = divmod(number, 58)
        out = ALPHABET[rest] + out
    return "1" * (len(raw) - len(raw.lstrip(b"\0"))) + out


GENESIS = "22" * 32


def transaction(slot=7):
    """A finalized getTransaction result with one deposit to VAULT."""
    data = bytes([1]) + (5).to_bytes(8, "little") + bytes(20) + bytes(32)
    keys = [b58(bytes([i + 1]) * 32) for i in range(8)] + [b58(bytes.fromhex(VAULT))]
    return {
        "slot": slot,
        "transaction": {
            "signatures": [SIG],
            "message": {
                "accountKeys": keys,
                "recentBlockhash": keys[0],
                "instructions": [{"programIdIndex": 8, "accounts": list(range(8)), "data": b58(data)}],
            },
        },
        "meta": {"err": None},
    }


class FakeRpc:
    """Solana JSON-RPC by method: a list of replies (None, a result or an exception) per method."""

    def __init__(self, **replies):
        self.replies = replies
        self.calls = []

    def __call__(self, config, method, params):
        self.calls.append(method)
        queue = self.replies[method]
        reply = queue.pop(0) if len(queue) > 1 else queue[0]
        if isinstance(reply, Exception):
            raise reply
        return reply


def too_many_requests():
    return urllib.error.HTTPError("https://rpc", 429, "Too Many Requests", {}, io.BytesIO(b""))


class LocateTest(unittest.TestCase):
    def setUp(self):
        self.dir = Path(tempfile.mkdtemp())
        self.cfg = {
            "solana_rpc": "https://rpc.example",
            "solana_genesis_hash": GENESIS,
            "vault_program": VAULT,
            "archived_transactions": str(self.dir / "finalized-transactions"),
        }
        (self.dir / "finalized-transactions").mkdir()
        self.sleeps = []
        getattr(source, "GENESIS_CHECKED", set()).clear()
        patcher = mock.patch.object(source.time, "sleep", self.sleeps.append)
        patcher.start()
        self.addCleanup(patcher.stop)

    def locate(self, rpc):
        with mock.patch.object(source, "rpc", rpc):
            return source.locate(self.cfg, "deposit", SIG)

    def test_held_transaction_and_checked_genesis_need_no_public_rpc_call(self):
        first = FakeRpc(getTransaction=[transaction()], getGenesisHash=[b58(bytes.fromhex(GENESIS))])
        self.locate(first)
        # The relay loop already holds the next deposit's finalized transaction.
        save(self.dir / "finalized-transactions" / (SIG + ".json"), transaction(slot=9))
        again = FakeRpc(getTransaction=[AssertionError("refetched")], getGenesisHash=[AssertionError("refetched")])
        self.assertEqual(self.locate(again)["slot"], 9)
        self.assertEqual(again.calls, [])

    def test_genesis_is_read_once_per_process_and_rpc(self):
        rpc = FakeRpc(getTransaction=[transaction()], getGenesisHash=[b58(bytes.fromhex(GENESIS))])
        self.locate(rpc)
        (self.dir / "finalized-transactions" / (SIG + ".json")).unlink()
        self.locate(rpc)
        self.assertEqual(rpc.calls.count("getGenesisHash"), 1)
        self.assertEqual(rpc.calls.count("getTransaction"), 2)

    def test_a_held_transaction_still_checks_the_rpc_genesis_once(self):
        save(self.dir / "finalized-transactions" / (SIG + ".json"), transaction())
        rpc = FakeRpc(getTransaction=[AssertionError("refetched")], getGenesisHash=[b58(bytes(32))])
        with self.assertRaisesRegex(Invalid, "genesis differs"):
            self.locate(rpc)

    def test_null_results_back_off_up_to_a_cap(self):
        rpc = FakeRpc(getTransaction=[None] * 7 + [transaction()], getGenesisHash=[b58(bytes.fromhex(GENESIS))])
        self.assertEqual(self.locate(rpc)["slot"], 7)
        first, cap = source.NULL_POLL_FIRST_SECONDS, source.NULL_POLL_MAX_SECONDS
        self.assertEqual(self.sleeps, [min(first * 2**i, cap) for i in range(7)])
        self.assertEqual(self.sleeps[-1], cap)
        # The method limit is 40 calls per 10 s; the poll stays far below it.
        self.assertGreaterEqual(first, 0.5)

    def test_rate_limited_poll_backs_off_instead_of_crashing(self):
        limited = RuntimeError("Solana getTransaction failed: {'code': 429, 'message': 'Too many requests'}")
        rpc = FakeRpc(
            getTransaction=[too_many_requests(), None, limited, transaction()],
            getGenesisHash=[b58(bytes.fromhex(GENESIS))],
        )
        self.assertEqual(self.locate(rpc)["slot"], 7)
        self.assertEqual(len(self.sleeps), 3)

    def test_other_rpc_errors_still_fail(self):
        rpc = FakeRpc(getTransaction=[None, RuntimeError("Solana getTransaction failed: invalid param")],
                      getGenesisHash=[b58(bytes.fromhex(GENESIS))])
        with self.assertRaisesRegex(RuntimeError, "invalid param"):
            self.locate(rpc)

    def test_rate_limited_genesis_check_backs_off_instead_of_crashing(self):
        save(self.dir / "finalized-transactions" / (SIG + ".json"), transaction())
        limited = RuntimeError("Solana getGenesisHash failed: {'code': 429, 'message': 'Too many requests'}")
        rpc = FakeRpc(getTransaction=[AssertionError("refetched")],
                      getGenesisHash=[too_many_requests(), limited, b58(bytes.fromhex(GENESIS))])
        self.assertEqual(self.locate(rpc)["slot"], 7)
        self.assertEqual(rpc.calls, ["getGenesisHash"] * 3)
        self.assertEqual(len(self.sleeps), 2)


class NativeCheckTest(unittest.TestCase):
    """The host's native check runs while the proof is requested and still gates the submit."""

    def setUp(self):
        self.dir = Path(tempfile.mkdtemp())
        cfg = {"state": str(self.dir / "state"), "ledger": "stub", "anchor": {}, "prover": "warm",
               "deposit_vkey": "0x01", "profile_hash": "H"}
        self.relay = jobs.Relay(cfg)
        self.job = self.relay.create("deposit", SIG)[0]
        self.events = []
        self.proof_started = threading.Event()
        self.native_error = None
        self.prove_error = None
        location = {"kind": "deposit", "slot": 7, "signature": SIG, "receipts": []}
        fakes = {
            "locate": lambda *args: location,
            "capture": lambda *args: None,
            "build": lambda cfg, location, job: (job / "witness.bin").write_bytes(b"witness"),
            "native_journal": self.native,
            "journal": lambda public: {},
            "bind": lambda location, decoded: [],
            "prove": self.prove,
        }
        for name, fake in fakes.items():
            patcher = mock.patch.object(jobs, name, fake)
            patcher.start()
            self.addCleanup(patcher.stop)

    def native(self, cfg, kind, job):
        self.events.append("native start")
        overlapped = self.proof_started.wait(2)
        self.events.append("native end" if overlapped else "native end before proof")
        if self.native_error:
            raise self.native_error
        (job / "native.bin").write_bytes(b"journal")
        return b"journal"

    def prove(self, cfg, kind, job, native=None):
        self.events.append("proof start")
        self.proof_started.set()
        if self.prove_error:
            raise self.prove_error
        (job / "public.bin").write_bytes(b"journal")
        (job / "proof.bin").write_bytes(b"proof")
        save(job / "prover-timing.json", {})
        if native is not None:
            native()  # the warm prover compares its journal with the native one here
        if (job / "public.bin").read_bytes() != (job / "native.bin").read_bytes():
            raise Invalid("warm prover journal differs from native verification")
        return "warm"

    def run_job(self):
        return self.relay.run(self.job, proof_only=True)

    def test_native_check_runs_while_the_proof_is_requested(self):
        state = self.run_job()
        self.assertIn("proof_done", state["timings"])
        self.assertIn("native end", self.events)
        self.assertNotIn("native end before proof", self.events)

    def test_native_refusal_blocks_the_submit_whatever_the_proof_says(self):
        self.native_error = Invalid("native verifier refused captured witness")
        with self.assertRaisesRegex(Invalid, "native verifier refused"):
            self.run_job()
        state = load(self.job / "job.json")
        self.assertEqual(state["status"], "failed")
        self.assertFalse(state["retryable"])
        self.assertNotIn("proof_done", state["timings"])
        # A proof whose witness the native check refused is never offered to other deposits.
        self.assertEqual(list((self.relay.root / "proof-cache").glob("*/complete")), [])

    def test_native_refusal_wins_over_a_failed_proof(self):
        self.native_error = Invalid("native verifier refused captured witness")
        self.prove_error = RuntimeError("prover socket refused")
        with self.assertRaisesRegex(Invalid, "native verifier refused"):
            self.run_job()
        self.assertFalse(load(self.job / "job.json")["retryable"])

    def test_route_binding_from_the_native_journal_still_gates(self):
        self.relay.cfg["route_binding"] = "ff" * 32
        with self.assertRaisesRegex(Invalid, "vault binding differs"):
            self.run_job()
        self.assertNotIn("proof_done", load(self.job / "job.json")["timings"])

    def test_signing_preparation_waits_for_the_native_journal(self):
        # The signing preparation starts before the proof stage and reads native.bin, which the
        # native check now writes during the proof. It must wait for it, not crash the relay.
        class Reached(Exception):
            pass

        def prepare(kind, path, generation):
            (path / "native.bin").read_bytes()
            raise Reached()

        self.relay.cfg["warm_signer"] = "signer"
        self.relay.ledger.pool = ThreadPoolExecutor(2)
        self.addCleanup(self.relay.ledger.pool.shutdown)
        self.relay.ledger.prepare = prepare
        with mock.patch.object(jobs, "journal", lambda public: {"slot": 7, "block_id": "b"}):
            with self.assertRaises(Reached):
                self.relay.run(self.job)

    def test_a_cached_proof_reaches_the_job_only_after_the_native_check(self):
        self.run_job()
        # Another deposit in the same block has the same witness and hits the proof cache.
        job = self.relay.create("deposit", "6" * 88)[0]
        self.native_error = Invalid("native verifier refused captured witness")
        with self.assertRaisesRegex(Invalid, "native verifier refused"):
            self.relay.run(job, proof_only=True)
        self.assertFalse((job / "public.bin").exists())
        self.assertFalse((job / "proof.bin").exists())


class WarmProveTest(unittest.TestCase):
    def test_warm_prove_waits_for_the_native_check_before_comparing(self):
        job = Path(tempfile.mkdtemp())
        (job / "witness.bin").write_bytes(b"witness")
        cfg = {"deposit_elf": "elf", "deposit_vkey": "0x01", "prover": "warm", "prover_socket": "sock", "host": "host"}

        def run(argv, check):
            report = {
                "witness_sha256": hashlib.sha256(b"witness").hexdigest(),
                "public_values": list(b"journal"),
                "proof_hex": "00",
                "program_vkey": "0x01",
            }
            Path(argv[-1]).write_text(json.dumps(report))

        def native():
            (job / "native.bin").write_bytes(b"journal")

        with mock.patch.object(prover.subprocess, "run", run), mock.patch.object(prover, "verify"):
            self.assertEqual(prover.prove(cfg, "deposit", job, native=native), "warm")
        self.assertEqual((job / "public.bin").read_bytes(), b"journal")


if __name__ == "__main__":
    unittest.main()
