import unittest


class FifthReview(unittest.TestCase):
    def test_all_six_routes_reconcile_supply_before_open(self):
        from exit_payout import reconcile_supply, BUCKET, POLICY

        backing = dict(
            bucket_id=BUCKET,
            policy_hash=POLICY,
            revision=8,
            series_supply_atoms=1610000,
            outstanding_vault_bridge_atoms=1610000,
        )
        rows = [dict(source_backing=backing) for _ in range(6)]
        self.assertEqual(reconcile_supply(rows, 1610000), 1610000)
        for altered in (
            dict(backing, series_supply_atoms=1630000),
            dict(backing, outstanding_vault_bridge_atoms=1630000),
            dict(backing, bucket_id="other"),
            {},
            dict(backing, series_supply_atoms=True),
        ):
            with self.assertRaises(ValueError):
                reconcile_supply(rows[:-1] + [dict(source_backing=altered)], 1610000)
        with self.assertRaises(ValueError):
            reconcile_supply(rows, 1630000)

    def test_open_refuses_stale_service_missing_supply_evidence(self):
        from exit_govern import require_open_accounting
        from exit_setup import BUCKET, POLICY

        backing = dict(
            revision=8,
            bucket_id=BUCKET,
            policy_hash=POLICY,
            series_supply_atoms=1610000,
            outstanding_vault_bridge_atoms=1610000,
        )
        routes = [dict(current_height=9, source_backing=backing)] * 6
        good = dict(series_supply_atoms="1610000", bucket_outstanding_atoms="1610000")
        require_open_accounting(routes, good, 9)
        for state in ({}, dict(good, series_supply_atoms="1630000")):
            with self.assertRaises(ValueError):
                require_open_accounting(routes, state, 9)
        with self.assertRaises(ValueError):
            require_open_accounting(routes, good, 10)
