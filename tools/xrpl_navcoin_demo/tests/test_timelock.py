from __future__ import annotations

import unittest

from tools.xrpl_navcoin_demo.timelock import (
    Direction,
    LedgerClocks,
    TimeoutPlan,
    TimingGateError,
    TimingPolicy,
)


class TimeoutOrderingTests(unittest.TestCase):
    def setUp(self) -> None:
        self.policy = TimingPolicy(
            min_xrpl_submit_margin_seconds=20,
            min_pftl_submit_margin_blocks=2,
            cross_ledger_claim_margin_seconds=30,
            coordinator_seconds_per_pftl_height=5,
            max_clock_observation_age_seconds=10,
        )
        self.clocks = LedgerClocks(
            xrpl_close_time=1_000,
            pftl_height=100,
            observed_unix=10_000,
        )

    def test_xrp_to_nav_first_xrpl_lock_is_longer(self) -> None:
        plan = TimeoutPlan(
            Direction.XRP_TO_NAV,
            xrpl_cancel_after=1_100,
            pftl_cancel_after=110,
        )
        report = plan.validate_second_lock(
            self.clocks, self.policy, coordinator_observed_unix=10_005
        )
        self.assertEqual(report["first_ledger"], "xrpl")
        self.assertEqual(report["second_ledger"], "pftl")

    def test_nav_to_xrp_first_pftl_lock_is_longer(self) -> None:
        plan = TimeoutPlan(
            Direction.NAV_TO_XRP,
            xrpl_cancel_after=1_040,
            pftl_cancel_after=120,
        )
        report = plan.validate_second_lock(
            self.clocks, self.policy, coordinator_observed_unix=10_005
        )
        self.assertEqual(report["first_ledger"], "pftl")
        self.assertEqual(report["second_ledger"], "xrpl")

    def test_inverted_or_stale_plans_fail_closed(self) -> None:
        cases = [
            TimeoutPlan(
                Direction.XRP_TO_NAV,
                xrpl_cancel_after=1_080,
                pftl_cancel_after=110,
            ),
            TimeoutPlan(
                Direction.NAV_TO_XRP,
                xrpl_cancel_after=1_060,
                pftl_cancel_after=110,
            ),
        ]
        for plan in cases:
            with self.subTest(direction=plan.direction):
                with self.assertRaises(TimingGateError):
                    plan.validate_second_lock(
                        self.clocks,
                        self.policy,
                        coordinator_observed_unix=10_005,
                    )
        with self.assertRaises(TimingGateError):
            cases[0].validate_second_lock(
                self.clocks,
                self.policy,
                coordinator_observed_unix=10_011,
            )

    def test_finish_and_cancel_boundaries_fail_closed(self) -> None:
        plan = TimeoutPlan(
            Direction.XRP_TO_NAV,
            xrpl_cancel_after=1_100,
            pftl_cancel_after=110,
        )
        for close_time in (1_100, 1_101):
            with self.assertRaises(TimingGateError):
                plan.assert_finish_open(
                    ledger="xrpl",
                    clocks=LedgerClocks(close_time, 109, 10_000),
                )
        for height in (110, 111):
            with self.assertRaises(TimingGateError):
                plan.assert_finish_open(
                    ledger="pftl",
                    clocks=LedgerClocks(1_099, height, 10_000),
                )
        with self.assertRaises(TimingGateError):
            plan.assert_cancel_open(ledger="xrpl", clocks=self.clocks)
        with self.assertRaises(TimingGateError):
            plan.assert_cancel_open(ledger="pftl", clocks=self.clocks)


if __name__ == "__main__":
    unittest.main()
