import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch
from solders.hash import Hash
from solders.keypair import Keypair
from solders.transaction import Transaction
import base64
import exit_payout as p
import exit_service as svc

BURN = "ad" * 48


class RecoveryTest(unittest.TestCase):
    def test_journal_signature_commits_before_ambiguous_broadcast(self):
        c = Mock(level={"commitment": "finalized"})
        c.payer = Keypair()
        c.owner = c.payer.pubkey()
        with tempfile.TemporaryDirectory() as d:
            j = svc.Journal(Path(d) / "db")
            j.enqueue(BURN)

            def call(method, *args):
                if method == "getLatestBlockhash":
                    return {
                        "value": {
                            "blockhash": str(Hash.default()),
                            "lastValidBlockHeight": 50,
                        }
                    }
                if method == "getFeeForMessage":
                    return {"value": 5000}
                if method == "getBalance":
                    return {"value": p.FLOOR + 6000}
                self.assertEqual(method, "sendTransaction")
                tx = Transaction.from_bytes(base64.b64decode(args[0]))
                self.assertEqual(
                    svc.Journal(j.path).attempt(BURN), (str(tx.signatures[0]), 50)
                )
                raise TimeoutError()

            c.call.side_effect = call
            with patch.object(p, "check_custody"), self.assertRaises(TimeoutError):
                p.funded_send(
                    c,
                    "exit.withdrawal",
                    [],
                    before_send=lambda sig, h: j.record_attempt(BURN, sig, h),
                )
            self.assertIsNone(j.get(BURN)["solana_tx"])

    def test_paid_marker_with_unavailable_history(self):
        c = Mock()
        c.account.return_value = dict(owner=str(p.VERIFIER), data=b"\1")
        c.call.side_effect = TimeoutError("internal user@some.host")
        self.assertIs(p.signature(c, BURN), p.PAID_NO_HISTORY)
        c.account.return_value = None
        self.assertIsNone(p.signature(c, BURN))

    def test_transient_job_retries_without_leaking_errors(self):
        with tempfile.TemporaryDirectory() as d:
            j = svc.Journal(Path(d) / "db")
            with (
                patch.object(
                    svc, "process", side_effect=TimeoutError("root@secret:/private")
                ),
                patch.object(svc.time, "time", return_value=1000),
            ):
                j.enqueue(BURN)
                self.assertTrue(svc.process_next(j, Path(d)))
                self.assertIsNone(j.pending())
            r = j.get(BURN)
            self.assertEqual(r["stage"], "queued")
            self.assertNotIn("secret", str(r))
            self.assertLessEqual(j.next_due(BURN), 1300)
            self.assertEqual(j.pending(), BURN)

    def test_worker_recovers_from_database_error(self):
        import sqlite3

        wake = Mock()
        with patch.object(
            svc,
            "process_next",
            side_effect=[sqlite3.OperationalError("/private"), KeyboardInterrupt()],
        ):
            with self.assertRaises(KeyboardInterrupt):
                svc.worker(Mock(), Path("."), wake)
        self.assertTrue(wake.wait.called)

    def test_catchup_yields_and_other_burn_runs(self):
        with tempfile.TemporaryDirectory() as d:
            j = svc.Journal(Path(d) / "db")
            j.enqueue(BURN)
            second = "01" * 48
            j.enqueue(second)
            with patch.object(svc, "process", side_effect=p.CheckpointAdvanced()):
                svc.process_next(j, Path(d))
            self.assertEqual(j.pending(), second)
            self.assertEqual(j.get(BURN)["stage"], "queued")

    def test_public_paid_status_is_reconciled_and_can_be_rebuilt(self):
        with tempfile.TemporaryDirectory() as d:
            j = svc.Journal(Path(d) / "db")
            j.enqueue(BURN)
            j.update(BURN, "paid", solana_tx="bogus")
            with patch.object(svc, "chain_payment", return_value=None):
                self.assertEqual(svc.checked_row(j, BURN)["stage"], "queued")
            row = dict(
                burn_tx_id=BURN,
                bucket_id=p.BUCKET,
                amount_atoms=20000,
                destination_ref=p.PREFIX + "12" * 32,
                created_at_height=5,
            )
            with patch.object(svc, "chain_payment", return_value=p.PAID_NO_HISTORY):
                svc.restore_redemption(j, row)
                status = svc.checked_row(j, BURN)
                self.assertEqual(status["stage"], "paid")
                self.assertIsNone(status["solana_tx"])

    def test_prune_terminal_work_preserves_live_buffer(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            tmp = root / ("11" * 32)
            tmp.mkdir()
            (tmp / "witness.json").write_text("{}")
            key = root / ("22" * 32 + ".key")
            from exit_keys import create_key

            create_key(key)
            c = Mock()
            c.account.return_value = {"owner": str(p.VERIFIER)}
            p.prune_work(c, root)
            self.assertTrue(key.exists())
            self.assertFalse(tmp.exists())
            c.account.return_value = None
            p.prune_work(c, root)
            self.assertFalse(key.exists())

    def test_admission_rate_and_artifact_caps(self):
        gate = svc.Admissions(limit=2)
        self.assertTrue(gate.take())
        self.assertTrue(gate.take())
        self.assertFalse(gate.take())
        with tempfile.TemporaryDirectory() as d:
            work = Path(d) / BURN
            work.mkdir()
            for i in range(4):
                (work / f"{i:064x}").mkdir()
            with self.assertRaises(ValueError):
                p.check_work_capacity(work)

    def test_readiness_checks_worker_and_journal(self):
        server = Mock()
        server.thread.is_alive.return_value = False
        self.assertFalse(svc.local_healthy(server))
        server.thread.is_alive.return_value = True
        server.journal.probe.side_effect = OSError("private-path")
        self.assertFalse(svc.local_healthy(server))

    def test_pruning_reconciles_before_retiring_paid_rows(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            j = svc.Journal(root / "jobs.sqlite")
            for burn in (BURN, "11" * 48):
                j.enqueue(burn)
                j.update(burn, "paid")
            with patch.object(svc, "chain_payment", return_value=None):
                svc.prune_terminal(j, root, retain=1)
            self.assertEqual(j.get(BURN)["stage"], "queued")
            j.update(BURN, "paid")
            with patch.object(svc, "chain_payment", return_value=p.PAID_NO_HISTORY):
                svc.prune_terminal(j, root, retain=1)
            self.assertIsNone(j.get(BURN))
            self.assertEqual(j.get("11" * 48)["stage"], "paid")

    def test_rebuild_replaces_damaged_database_after_chain_readback(self):
        with tempfile.TemporaryDirectory() as d:
            state = Path(d)
            path = state / "jobs.sqlite"
            path.write_bytes(b"damaged sqlite fixture")
            row = dict(
                burn_tx_id=BURN,
                bucket_id=p.BUCKET,
                amount_atoms=20000,
                destination_ref=p.PREFIX + "12" * 32,
                created_at_height=9,
            )
            with (
                patch.object(svc, "redemptions", return_value=[row]),
                patch.object(svc, "chain_payment", return_value=p.PAID_NO_HISTORY),
                patch.object(svc, "cluster"),
            ):
                svc.rebuild(state)
            self.assertEqual(svc.Journal(path).get(BURN)["stage"], "paid")
            self.assertEqual(path.read_bytes(), b"damaged sqlite fixture")

    def test_rebuild_waits_out_unknown_inflight_signatures(self):
        with tempfile.TemporaryDirectory() as d:
            state = Path(d)
            row = dict(
                burn_tx_id=BURN,
                bucket_id=p.BUCKET,
                amount_atoms=20000,
                destination_ref=p.PREFIX + "12" * 32,
                created_at_height=7,
            )
            rpc = Mock()
            rpc.call.return_value = {"value": {"lastValidBlockHeight": 100}}
            with (
                patch.object(svc, "cluster", return_value=rpc),
                patch.object(svc, "chain_payment", return_value=None),
                patch.object(svc, "redemptions", return_value=[row]),
            ):
                svc.rebuild(state)
            journal = svc.Journal(state / "jobs.sqlite")
            with (
                patch.object(svc, "cluster", return_value=rpc),
                patch.object(svc, "process") as work,
            ):
                rpc.call.return_value = 100
                svc.process_next(journal, state)
                work.assert_not_called()
                rpc.call.return_value = 101
                svc.process_next(journal, state)
                work.assert_called_once()

    def test_terminal_history_cannot_fill_database_without_limit(self):
        with tempfile.TemporaryDirectory() as d:
            j = svc.Journal(Path(d) / "db")
            with j.connect() as db:
                db.executemany(
                    "INSERT INTO jobs(burn_tx_id,stage) VALUES(?,'paid')",
                    [(f"{i:096x}",) for i in range(1280)],
                )
            with self.assertRaises(OverflowError):
                j.enqueue("ab" * 48)

    def test_rebuild_preserves_hot_journal_and_selects_new_database(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            (root / "jobs.sqlite").write_bytes(b"damaged")
            sidecar = root / "jobs.sqlite-wal"
            sidecar.write_bytes(b"forensic evidence")
            with patch.object(svc, "redemptions", return_value=[]):
                svc.rebuild(root)
            self.assertEqual(sidecar.read_bytes(), b"forensic evidence")
            self.assertIsNone(svc.Journal(root / "jobs.sqlite").pending())
            self.assertNotEqual(
                svc.Journal(root / "jobs.sqlite").path, root / "jobs.sqlite"
            )

    def test_later_admissions_do_not_starve_a_due_checkpoint(self):
        with tempfile.TemporaryDirectory() as d:
            j = svc.Journal(Path(d) / "db")
            with patch.object(svc.time, "time", return_value=200):
                j.enqueue(BURN)
                j.defer(BURN, progress=True)
            with patch.object(svc.time, "time", return_value=205):
                j.enqueue("bc" * 48)
                self.assertEqual(j.pending(), BURN)

    def test_pending_job_reclaims_only_confirmed_checkpoint_buffers_before_cap(self):
        import hashlib

        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            raws = [bytes([i]) for i in range(4)]
            keys = [root / (hashlib.sha256(raw).hexdigest() + ".key") for raw in raws]
            for key in keys:
                key.touch()
            c = Mock()
            signers = {key: Mock() for key in keys}
            buffers = {s.pubkey(): raw for s, raw in zip(signers.values(), raws)}
            result = "ea" * 48

            def account(key):
                if key == p.checkpoint_pda(result):
                    return dict(owner=str(p.VERIFIER), data=bytes(48))
                if key in buffers:
                    return dict(owner=str(p.VERIFIER), data=buffers[key])

            c.account.side_effect = account

            def reclaim(cluster, label, ix, signer):
                buffers.pop(signer.pubkey())

            def decode(raw):
                if raw == raws[0]:
                    raise ValueError("unfinished upload")
                return dict(result=result, end="00" * 48)

            c.owner = p.FEE_PAYER
            with (
                patch.object(p, "load", side_effect=signers.get),
                patch("exit_precheck.checkpoint_values", side_effect=decode),
                patch.object(p, "funded_send", side_effect=reclaim),
            ):
                from solders.keypair import Keypair

                buffers.clear()
                for signer, raw in zip(signers.values(), raws):
                    signer.pubkey.return_value = Keypair().pubkey()
                    buffers[signer.pubkey()] = raw
                p.reclaim_completed(c, root)
                p.check_work_capacity(root)
            self.assertEqual(list(root.glob("*.key")), keys[:1])

    def test_paid_polling_caches_marker_and_limits_history_reads(self):
        with tempfile.TemporaryDirectory() as d:
            j = svc.Journal(Path(d) / "db")
            j.enqueue(BURN)
            j.update(BURN, "paid", solana_tx="missing")
            c = Mock()
            c.account.return_value = dict(owner=str(p.VERIFIER), data=b"\1")
            history = [dict(signature=str(i), err=None) for i in range(100)]

            def call(method, *args):
                if method == "getSignaturesForAddress":
                    return history
                self.assertEqual(method, "getTransaction")
                return None

            c.call.side_effect = call
            with patch.object(svc, "cluster", return_value=c):
                for _ in range(20):
                    self.assertIsNone(svc.checked_row(j, BURN)["solana_tx"])
            self.assertLessEqual(
                sum(call.args[0] == "getTransaction" for call in c.call.call_args_list),
                5,
            )
            c.account.assert_called_once()

    def test_queue_capacity_overrides_cached_readiness(self):
        import threading

        with tempfile.TemporaryDirectory() as d:
            j = svc.Journal(Path(d) / "db")
            server = Mock(
                journal=j,
                ready_lock=threading.Lock(),
                valid_until=float("inf"),
                ready=dict(ready=True, payout_ready=True),
            )
            server.thread.is_alive.return_value = True
            self.assertTrue(svc.service_readiness(server)["ready"])
            for i in range(32):
                j.enqueue(f"{i:096x}")
            self.assertFalse(svc.service_readiness(server)["ready"])
            self.assertFalse(svc.service_readiness(server)["payout_ready"])

    def test_sweep_recovers_pending_jobs_even_when_other_jobs_are_paid(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            j = svc.Journal(root / "db")
            for burn in [BURN, "bc" * 48]:
                j.enqueue(burn)
                (root / burn).mkdir()
                (root / burn / ("ab" * 32 + ".key")).touch()
            j.update(BURN, "paid")
            with (
                patch.object(svc, "cluster"),
                patch.object(svc, "chain_payment", return_value=p.PAID_NO_HISTORY),
                patch.object(svc, "prune_work"),
                patch.object(p, "reclaim_completed") as reclaim,
            ):
                svc.prune_terminal(j, root)
                svc.prune_terminal(j, root)
                self.assertIn(
                    root / ("bc" * 48),
                    [args.args[1] for args in reclaim.call_args_list],
                )

    def test_confirmed_closed_buffer_does_not_consume_pending_capacity(self):
        with tempfile.TemporaryDirectory() as d:
            import base58

            work = Path(d)
            path = work / ("cd" * 32 + ".key")
            path.touch()
            signer = Keypair()
            c = Mock(owner=p.FEE_PAYER, level={"commitment": "finalized"})
            c.account.return_value = None

            def call(method, *args):
                if method == "getSignaturesForAddress":
                    return [dict(err=None, signature="sig")]
                return dict(
                    meta={"err": None},
                    transaction=dict(
                        signatures=["sig"],
                        message=dict(
                            accountKeys=[
                                str(c.owner),
                                str(signer.pubkey()),
                                str(p.VERIFIER),
                            ],
                            instructions=[
                                dict(
                                    programIdIndex=2,
                                    accounts=[1, 0],
                                    data=base58.b58encode(b"\x06").decode(),
                                )
                            ],
                        ),
                    ),
                )

            c.call.side_effect = call
            with patch.object(p, "load", return_value=signer):
                p.reclaim_completed(c, work)
            self.assertFalse(path.exists())

    def test_reconciles_confirmed_proof_directories_before_pending_capacity(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            for i in range(4):
                cache = root / f"{i:064x}"
                cache.mkdir()
                (cache / "native-public-values.bin").write_bytes(bytes([i]))
            c = Mock()
            c.account.return_value = dict(owner=str(p.VERIFIER), data=bytes(48))

            def decode(raw):
                if raw == b"\0":
                    raise ValueError("a withdrawal, not a checkpoint")
                return dict(result="cd" * 48, end="00" * 48)

            with patch("exit_precheck.checkpoint_values", side_effect=decode):
                p.reclaim_completed(c, root)
                p.check_work_capacity(root)
            self.assertEqual([path.name for path in root.iterdir()], ["0" * 64])
