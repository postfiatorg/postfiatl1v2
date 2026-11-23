from pathlib import Path
from live import read
import unittest


class FifthReview(unittest.TestCase):
    def test_all_six_routes_reconcile_supply_before_open(self):
        from exit_payout import reconcile_supply, BUCKET, POLICY

        route = read(Path(__file__).parent / "fixtures/fork-open-route.json")
        rows = [route] * 6
        backing = route["source_backing"]
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

        route = read(Path(__file__).parent / "fixtures/fork-open-route.json")
        height = route["current_height"]
        routes = [route] * 6
        good = dict(series_supply_atoms="1610000", bucket_outstanding_atoms="1610000")
        require_open_accounting(routes, good, height)
        for state in ({}, dict(good, series_supply_atoms="1630000")):
            with self.assertRaises(ValueError):
                require_open_accounting(routes, state, height)
        with self.assertRaises(ValueError):
            require_open_accounting(routes, good, height + 1)
