"""Account readiness is validated strictly, cached per account and bounded."""

import threading
import unittest
from unittest.mock import patch

import exit_payout as payout
import exit_service as svc

ACCOUNT = "pf" + "0b" * 20
BASE = "/solana/withdrawals/readiness"


class AccountReadinessTest(unittest.TestCase):
    def setUp(self):
        svc.ACCOUNT_CACHE.clear()

    def test_destination_check_makes_one_short_attempt_on_one_cluster(self):
        import exit_setup
        from unittest.mock import Mock

        posts = []

        class Cluster:
            def __init__(self, payer):
                self.http = Mock()
                self.rpc = "https://solana.invalid"
                self.level = {"commitment": "finalized"}

                def post(url, json, timeout):
                    posts.append(timeout)
                    return Mock(status_code=429, ok=False)

                self.http.post.side_effect = post

        with patch.object(exit_setup, "ExitCluster", Cluster), patch.object(svc, "DESTINATION_CLUSTER", None):
            c = svc.destination_cluster()
            self.assertIs(svc.destination_cluster(), c)
            with self.assertRaises(ValueError):
                c.call("getAccountInfo", "x")
        self.assertEqual(len(posts), 1)
        self.assertLessEqual(posts[0], 5)

    def test_malformed_queries_are_refused_with_400(self):
        for query in ("x", "account=PF" + "0B" * 20, "account=" + ACCOUNT + "&x=1",
                      "account=" + ACCOUNT + "&account=" + ACCOUNT, "account=",
                      "account=" + ACCOUNT + "&destination=0OIl"):
            code, body = svc.account_reply(None, BASE + "?" + query)
            self.assertEqual(code, 400, query)
            self.assertIn("error", body)

    def test_answers_are_cached_per_account(self):
        ready = dict(ok=True, ready=True, buckets=[])
        with (
            patch.object(svc, "service_readiness", return_value=ready),
            patch.object(svc, "holdings", return_value=[]) as held,
        ):
            first = svc.account_reply(None, BASE + "?account=" + ACCOUNT)
            second = svc.account_reply(None, BASE + "?account=" + ACCOUNT)
            self.assertEqual(first, second)
            self.assertEqual(held.call_count, 1)
            svc.account_reply(None, BASE + "?account=pf" + "0c" * 20)
            self.assertEqual(held.call_count, 2)

    def test_concurrent_reads_are_bounded(self):
        taken = [svc.ACCOUNT_SLOTS.acquire(blocking=False) for _ in range(svc.ACCOUNT_READERS)]
        try:
            self.assertTrue(all(taken))
            code, body = svc.account_reply(None, BASE + "?account=" + ACCOUNT)
            self.assertEqual(code, 429)
            self.assertIn("busy", body["error"])
        finally:
            for _ in taken:
                svc.ACCOUNT_SLOTS.release()

    def test_holdings_reuse_the_readiness_snapshot(self):
        buckets = [dict(bucket_id=payout.BUCKET, series_asset_id="5e" * 48, outstanding_atoms="400000")]

        def rpc(index, method, **args):
            self.assertEqual(method, "account_assets")
            return dict(assets=[dict(balance="500000", authorized=True, frozen=False)])

        with patch.object(payout, "rpc", side_effect=rpc):
            offer = payout.holdings(ACCOUNT, buckets)
        self.assertEqual(offer[0]["redeemable_atoms"], "400000")


    def test_series_below_the_governed_minimum_are_not_offered(self):
        buckets = [
            dict(bucket_id=payout.BUCKET, series_asset_id="5e" * 48, outstanding_atoms="400000"),
            dict(bucket_id="9a" * 48, series_asset_id="6e" * 48, outstanding_atoms="400000"),
        ]
        balances = {"5e" * 48: "500000", "6e" * 48: "20000"}

        def rpc(index, method, **args):
            return dict(assets=[dict(balance=balances[args["asset_id"]], authorized=True, frozen=False)])

        with patch.object(payout, "rpc", side_effect=rpc):
            offer = payout.holdings(ACCOUNT, buckets, minimum=300000)
        self.assertEqual([o["series_asset_id"] for o in offer], ["5e" * 48])
        ready = dict(ok=True, ready=True, buckets=buckets, min_withdrawal_atoms="300000")
        with (
            patch.object(svc, "service_readiness", return_value=ready),
            patch.object(svc, "holdings", return_value=[]) as held,
        ):
            svc.account_readiness(None, BASE + "?account=" + ACCOUNT)
        held.assert_called_once_with(ACCOUNT, buckets, minimum=300000)


if __name__ == "__main__":
    unittest.main()
