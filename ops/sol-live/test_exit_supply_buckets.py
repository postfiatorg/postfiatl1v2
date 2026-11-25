"""A revision-9 bucket of the shared custody passes the supply reconciliation like revision 8."""

import unittest
from unittest.mock import patch

import exit_govern as g
import exit_payout as p

HEIGHT = 1800
R9_POLICY = "09" * 48
R9 = p.bucket_id(R9_POLICY)
R9_SERIES = "99" * 48


def status_bucket(key, policy, series, outstanding):
    return dict(bucket_id=key, policy_hash=policy, source_series_id=series,
                outstanding_vault_bridge_atoms=outstanding)


def route(key, policy, supply, outstanding, height=HEIGHT):
    return dict(current_height=height, source_backing=dict(
        revision=8, bucket_id=key, policy_hash=policy,
        series_supply_atoms=supply, outstanding_vault_bridge_atoms=outstanding))


class SupplyBucketsTest(unittest.TestCase):
    def setUp(self):
        self.r9 = route(R9, R9_POLICY, 7000, 7000)

    def rpc(self, i, method, asset_id):
        self.assertEqual((method, asset_id), ("vault_bridge_route", R9_SERIES))
        return self.r9

    def test_readiness_reconciles_every_covered_bucket(self):
        covered = {
            p.BUCKET: status_bucket(p.BUCKET, p.POLICY, p.SERIES, 40000),
            R9: status_bucket(R9, R9_POLICY, R9_SERIES, 7000),
        }
        r8 = [route(p.BUCKET, p.POLICY, 40000, 40000)] * 5
        with patch.object(p, "rpc", side_effect=self.rpc):
            self.assertEqual(p.reconcile_covered(covered, r8, 5, HEIGHT), {p.BUCKET: 40000, R9: 7000})
            for altered in (route(R9, R9_POLICY, 7001, 7000), route(R9, R9_POLICY, 7001, 7001),
                            route(R9, p.POLICY, 7000, 7000)):
                self.r9 = altered
                with self.assertRaisesRegex(ValueError, "Series supply"):
                    p.reconcile_covered(covered, r8, 5, HEIGHT)
            self.r9 = route(R9, R9_POLICY, 7000, 7000, HEIGHT - 1)
            with self.assertRaisesRegex(ValueError, "Missing validator supply report"):
                p.reconcile_covered(covered, r8, 5, HEIGHT)

    def test_open_reconciles_the_service_report_of_every_covered_bucket(self):
        r8 = [route(p.BUCKET, p.POLICY, 40000, 40000)] * 6
        state = dict(series_supply_atoms="40000", bucket_outstanding_atoms="40000", buckets=[
            dict(bucket_id=p.BUCKET, series_asset_id=p.SERIES, outstanding_atoms="40000",
                 series_supply_atoms="40000"),
            dict(bucket_id=R9, series_asset_id=R9_SERIES, outstanding_atoms="7000",
                 series_supply_atoms="7000"),
        ])
        with patch.object(p, "rpc", side_effect=self.rpc):
            g.require_open_accounting(r8, state, HEIGHT)
            bad = dict(state, buckets=[dict(state["buckets"][1], series_supply_atoms="7001")])
            with self.assertRaisesRegex(ValueError, "Payout service lacks"):
                g.require_open_accounting(r8, bad, HEIGHT)
            self.r9 = route(R9, R9_POLICY, 7001, 7001)
            with self.assertRaisesRegex(ValueError, "Series supply"):
                g.require_open_accounting(r8, state, HEIGHT)


if __name__ == "__main__":
    unittest.main()
