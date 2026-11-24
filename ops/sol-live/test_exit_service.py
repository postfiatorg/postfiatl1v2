import tempfile
import unittest
from pathlib import Path

from exit_service import Journal, parse_request
from exit_payout import marker, VERIFIER, validate_burn, BUCKET, PREFIX
from bridge import pda
import hashlib


class JobsTest(unittest.TestCase):
    def test_restart_and_retry(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "jobs.sqlite"
            j = Journal(path)
            burn = "ab" * 48
            self.assertEqual(j.enqueue(burn)["stage"], "queued")
            j.update(burn, "paying", amount_atoms="20000")
            self.assertEqual(j.enqueue(burn)["stage"], "paying")
            j = Journal(path)
            self.assertEqual(j.pending(), burn)
            j.update(burn, "paid", solana_tx="signature")
            self.assertEqual(j.enqueue(burn)["solana_tx"], "signature")
            self.assertIsNone(j.pending())

    def test_bounds(self):
        for data in (
            b"{}",
            b"[]",
            b'{"burn_tx_id":"../"}',
            b'{"burn_tx_id":"aa","extra":2}',
        ):
            with self.assertRaises(ValueError):
                parse_request(data)

    def test_full_burn_address_and_bucket(self):
        tx = "47" * 48
        self.assertEqual(
            marker(tx),
            pda(VERIFIER, b"egress-burn", hashlib.sha256(bytes.fromhex(tx)).digest()),
        )
        row = dict(
            burn_tx_id=tx,
            bucket_id=BUCKET,
            destination_ref=PREFIX + "12" * 32,
            amount_atoms=20000,
        )
        self.assertTrue(validate_burn(row, tx))
        for field, value in [
            ("bucket_id", "ff" * 48),
            ("destination_ref", "evm-erc20:1:0x12"),
            ("burn_tx_id", "33" * 48),
        ]:
            with self.assertRaises(ValueError):
                validate_burn(dict(row, **{field: value}), tx)

    def test_failed_job_can_retry_and_queue_has_a_limit(self):
        with tempfile.TemporaryDirectory() as directory:
            j = Journal(Path(directory) / "jobs.sqlite")
            first = "01" * 48
            j.enqueue(first)
            j.update(first, "failed", error="prover busy")
            self.assertIsNone(j.pending())
            self.assertIsNone(j.enqueue(first)["error"])
            for i in range(31):
                j.enqueue(f"{i:096x}")
            with self.assertRaises(OverflowError):
                j.enqueue("ef" * 48)
            self.assertEqual(j.enqueue(first)["stage"], "queued")

    def test_unfunded_relayer_stops_before_proving(self):
        from unittest.mock import Mock, patch
        import exit_payout as payout

        burn = "34" * 48
        row = dict(
            burn_tx_id=burn,
            bucket_id=BUCKET,
            amount_atoms=20000,
            destination_ref=PREFIX + "88" * 32,
            created_at_height=7,
        )
        c = Mock()
        c.account.return_value = None
        c.call.return_value = {"value": payout.FLOOR - 1}
        with (
            patch.object(payout, "cluster", return_value=c),
            patch.object(payout, "rpc", return_value={"redemptions": [row]}),
            patch.object(payout, "prove") as proof,
        ):
            with self.assertRaisesRegex(ValueError, "floor"):
                payout.process(burn, Path("unused"), Mock())
            proof.assert_not_called()

    def test_wallet_example_is_the_reviewed_canary(self):
        from exit_canary import DEST, PHANTOM

        self.assertEqual(
            str(
                validate_burn(
                    dict(
                        burn_tx_id="ab" * 48,
                        bucket_id=BUCKET,
                        amount_atoms=20000,
                        destination_ref=DEST,
                    ),
                    "ab" * 48,
                )
            ),
            str(PHANTOM),
        )
        self.assertEqual(str(PHANTOM), "Ce5Yv528pKVrVq4JbQchZ8zasCAzxT3f9uCfatTZcfo8")

    def test_http_errors_and_pending_job_shape(self):
        import json
        import threading
        from http.client import HTTPConnection
        from http.server import ThreadingHTTPServer
        from unittest.mock import patch
        from exit_service import BASE, Handler, Admissions

        burn = "ac" * 48
        with (
            tempfile.TemporaryDirectory() as directory,
            ThreadingHTTPServer(("127.0.0.1", 0), Handler) as server,
            patch(
                "exit_service.service_readiness", return_value={"payout_ready": True}
            ) as ready,
            patch(
                "exit_service.redemptions",
                return_value=[
                    dict(
                        burn_tx_id=burn,
                        bucket_id=BUCKET,
                        amount_atoms=20000,
                        destination_ref=PREFIX + "00" * 32,
                    )
                ],
            ),
        ):
            server.journal = Journal(Path(directory) / "jobs.sqlite")
            server.wake = threading.Event()
            server.admissions = Admissions()
            threading.Thread(target=server.serve_forever, daemon=True).start()
            try:

                def request(method, path, body=b"", headers=None):
                    conn = HTTPConnection("127.0.0.1", server.server_port, timeout=3)
                    conn.putrequest(method, path)
                    for name, value in (headers or {}).items():
                        conn.putheader(name, value)
                    conn.endheaders(body)
                    reply = conn.getresponse()
                    try:
                        return reply.status, json.loads(reply.read())
                    finally:
                        conn.close()

                hdr = {"Content-Type": "application/json"}
                for expected, args in (
                    (404, ("GET", BASE + "/" + burn)),
                    (400, ("GET", BASE + "/invalid")),
                    (415, ("POST", BASE)),
                    (411, ("POST", BASE, b"", hdr)),
                    (413, ("POST", BASE, b"", dict(hdr, **{"Content-Length": "257"}))),
                    (400, ("POST", BASE, b"{}", dict(hdr, **{"Content-Length": "2"}))),
                ):
                    code, payload = request(*args)
                    self.assertEqual(code, expected)
                    self.assertTrue(payload["error"])
                raw = json.dumps({"burn_tx_id": burn}).encode()
                ready.return_value = {"payout_ready": False}
                post_args = (
                    "POST",
                    BASE,
                    raw,
                    dict(hdr, **{"Content-Length": str(len(raw))}),
                )
                with patch("exit_service.redemptions", return_value=[]):
                    self.assertEqual(request(*post_args)[0], 404)
                    self.assertIsNone(server.journal.get(burn))
                code, row = request(
                    "POST", BASE, raw, dict(hdr, **{"Content-Length": str(len(raw))})
                )
                self.assertEqual(
                    (code, row["stage"], row["burn_height"]), (202, "queued", None)
                )
                self.assertEqual(request("GET", BASE + "/" + burn), (200, row))
                ready.assert_not_called()
            finally:
                server.shutdown()

    def test_readiness_requires_the_governed_series_and_vault_coverage(self):
        from unittest.mock import Mock, patch
        import exit_payout as payout

        c = Mock()
        b = dict(
            bucket_id=BUCKET,
            policy_hash=payout.POLICY,
            source_series_id=payout.SERIES,
            status="active",
            outstanding_vault_bridge_atoms=40000,
            source_domain="shared",
            redeemable_at_par=True,
            impairment_factor_bps=10000,
            redemption_queue_atoms=0,
            other_allocations_atoms=0,
            nav_subscription_allocations_atoms=0,
            counted_value_atoms=40000,
        )
        live_status = dict(
            buckets=[b],
            issuer="issuer",
            finalized_epoch=7,
            finalized_reserve_packet_hash="15" * 48,
        )
        route = dict(
            active=True,
            profile_hash=payout.EXIT,
            profile={"route_epoch": 8},
            source_backing=dict(revision=8, bucket_id=BUCKET, policy_hash=payout.POLICY,
                                series_supply_atoms=40000, outstanding_vault_bridge_atoms=40000),
            current_height=1755,
            burns_open=True,
        )

        def rpc(index, method, asset_id):
            if method == "vault_bridge_route":
                self.assertEqual(asset_id, payout.SERIES)
                return route
            self.assertEqual((method, asset_id), ("vault_bridge_status", payout.ASSET))
            return live_status

        account = dict(
            owner=str(payout.TOKEN),
            data=bytes(payout.MINT) + bytes(32) + (40000).to_bytes(8, "little"),
        )
        c.account.return_value = account
        with (
            patch.object(payout, "cluster", return_value=c),
            patch.object(payout, "light", return_value=(1753, "anchor")),
            patch.object(
                payout, "inspect_vault", return_value={"custody_atoms": 40000}
            ),
            patch.object(payout, "payout_budget", return_value=1000),
            patch.object(payout, "balance", return_value=payout.FLOOR + 10000000),
            patch.object(payout, "available"),
            patch.object(
                payout,
                "fleet",
                return_value=dict(
                    block_height=1755, block_tip_hash="tip", state_root="root"
                ),
            ),
            patch.object(payout, "check_light_path"),
            patch.object(payout.FINALIZED_PATH, "check", side_effect=payout.parent_of),
            patch.object(payout, "rpc", side_effect=rpc),
        ):
            value = payout.readiness()
            self.assertTrue(value["ready"], value)
            self.assertEqual(
                (
                    value["series_asset_id"],
                    value["bucket_id"],
                    value["destination_prefix"],
                ),
                (payout.SERIES, BUCKET, PREFIX),
            )
            route["source_backing"]["series_supply_atoms"] += 20000
            value = payout.readiness()
            self.assertFalse(value["payout_ready"])
            self.assertIn("supply", value["message"])
            b["outstanding_vault_bridge_atoms"] += 1
            self.assertIn("custody", payout.readiness()["message"])
            route["active"] = False
            self.assertIn("six validators", payout.readiness()["message"])

    def test_fees_and_rent_cannot_take_the_relayer_below_its_floor(self):
        from unittest.mock import Mock
        from solders.keypair import Keypair
        from exit_payout import FLOOR, Hash, funded_send

        c = Mock()
        c.owner = Keypair().pubkey()
        c.call.side_effect = [
            {"value": {"blockhash": str(Hash.default())}},
            {"value": 5000},
            {"value": FLOOR + 5900},
        ]
        with self.assertRaisesRegex(ValueError, "floor"):
            funded_send(c, "test", [], rent=1000)
        c.send.assert_not_called()

    def test_reclaim_failure_preserves_finalized_transaction(self):
        from unittest.mock import Mock, patch
        from solders.keypair import Keypair
        import exit_payout as payout

        c = Mock()
        c.owner = Keypair().pubkey()
        c.account.return_value = None
        c.rent.return_value = 1_000_000
        w = {
            "prior_checkpoint_block_id": "ae" * 48,
            "block": {"header": {"block_hash": "cc" * 48}},
        }

        def send(cluster, label, *args, **kwargs):
            if label == "exit.reclaim":
                raise RuntimeError("RPC unavailable")
            return "finalized"

        with (
            tempfile.TemporaryDirectory() as directory,
            patch.object(payout, "check_custody"),
            patch.object(payout, "prove", return_value=(bytes(8), bytes(356))),
            patch.object(payout, "public_values", return_value=bytes(8)),
            patch.object(payout, "balance"),
            patch.object(payout, "funded_send", side_effect=send),
            self.assertLogs(level="WARNING"),
        ):
            self.assertEqual(
                payout.submit(c, "checkpoint", w, Path(directory)), "finalized"
            )
            self.assertEqual(len(list(Path(directory).glob("*.key"))), 1)

    def test_payment_checks_only_its_destination_before_proving(self):
        from unittest.mock import Mock, patch
        import exit_payout as p
        from exit_precheck import PHANTOM
        from solders.keypair import Keypair

        other = Keypair().pubkey()
        w = {
            "prior_checkpoint_block_id": "11" * 48,
            "block": {"header": {"block_hash": "22" * 48}},
            "receipt": {"tx_id": "33" * 48},
        }
        c = Mock(owner=p.FEE_PAYER)
        delegated = p.pda(p.ATA, bytes(PHANTOM), bytes(p.TOKEN), bytes(p.MINT))
        clean = p.pda(p.ATA, bytes(other), bytes(p.TOKEN), bytes(p.MINT))
        d = bytearray(165)
        d[:64] = bytes(p.MINT) + bytes(PHANTOM)
        d[108] = d[72] = 1

        def account(key):
            if key == delegated:
                return dict(owner=str(p.TOKEN), executable=False, data=bytes(d))

        c.account.side_effect = account
        c.rent.return_value = 1
        with (
            tempfile.TemporaryDirectory() as directory,
            patch.object(p, "check_custody"),
            patch.object(p, "prove", return_value=(b"test", bytes(356))) as proof,
            patch.object(p, "public_values", return_value=b"test"),
            patch.object(p, "upload", return_value=Keypair()),
            patch.object(p, "balance"),
            patch.object(p, "funded_send", return_value="ok"),
        ):
            work = Path(directory)
            self.assertEqual(p.submit(c, "withdrawal", w, work, PHANTOM), "ok")
            proof.assert_called_once()
            proof.reset_mock()
            d[108] = 2
            with self.assertRaisesRegex(ValueError, "state"):
                p.submit(c, "withdrawal", w, work, PHANTOM)
            proof.assert_not_called()
            c.account.reset_mock()
            self.assertEqual(p.submit(c, "withdrawal", w, work, other), "ok")
            self.assertIn(clean, [call.args[0] for call in c.account.call_args_list])
            self.assertNotIn(
                delegated, [call.args[0] for call in c.account.call_args_list]
            )
