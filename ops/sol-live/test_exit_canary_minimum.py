"""The canary burns the chain's governed minimum withdrawal, not a fixed 0.02 USDC."""

import unittest

import exit_canary
import exit_payout as payout


class CanaryMinimumTest(unittest.TestCase):
    def test_readiness_reports_the_minimum_all_six_agree_on(self):
        rules = dict(covered_bucket_ids=[payout.BUCKET],
                     decision_signatures=dict(open=6, close=5, paid=5, min_withdrawal=5, retire=6))
        rows = [dict(rules, min_withdrawal_atoms=300000) for _ in range(6)]
        self.assertEqual(payout.governed_minimum(rows), 300000)
        for bad in (
            rows[:5] + [dict(rules, min_withdrawal_atoms=20000)],
            rows[:5] + [dict()],
            [dict(rules, min_withdrawal_atoms=0)] * 6,
            [dict(min_withdrawal_atoms=300000)] * 6,
        ):
            with self.assertRaises(ValueError):
                payout.governed_minimum(bad)
        # A first-release chain reports no rules and has no minimum.
        self.assertIsNone(payout.governed_minimum([dict(active=True)] * 6))

    def test_canary_amount_follows_the_governed_minimum(self):
        self.assertEqual(exit_canary.canary_amount(dict(min_withdrawal_atoms="300000")), 300000)
        self.assertEqual(exit_canary.canary_amount(dict(min_withdrawal_atoms="450000")), 450000)
        with self.assertRaises(ValueError):
            exit_canary.canary_amount(dict())
        # Against a first-release chain the canary burns that release's 0.02 USDC.
        self.assertEqual(
            exit_canary.canary_amount(dict(min_withdrawal_atoms=None, exit_release=1)), 20000
        )

    def test_canary_checks_use_the_planned_amount(self):
        plan = dict(fields=dict(operation=dict(amount_atoms=300000)))
        self.assertEqual(exit_canary.planned_amount(plan), 300000)
        # Plans written before the minimum burned the old fixed canary amount.
        self.assertEqual(exit_canary.planned_amount(dict(fields=dict(operation={}))), 20000)
        paid = dict(burn_tx_id="ab" * 48, withdrawal_packet_hash="cd" * 48, amount_atoms=300000)
        bucket = dict(
            bucket_id=payout.BUCKET,
            policy_hash=payout.POLICY,
            source_domain=payout.SOURCE_DOMAIN,
            counted_value_atoms=300000,
            redemption_queue_atoms=300000,
            outstanding_vault_bridge_atoms=0,
        )
        red = dict(
            burn_tx_id=paid["burn_tx_id"],
            withdrawal_packet_hash=paid["withdrawal_packet_hash"],
            bucket_id=payout.BUCKET,
            amount_atoms=300000,
            state="pending",
            settled_atoms=0,
            settlement_receipt_hash="",
        )
        allocation = dict(
            bucket_id=payout.BUCKET,
            purpose="vault_bridge_supply",
            amount_atoms=300000,
            remaining_atoms=300000,
        )
        status = dict(buckets=[bucket], allocations=[allocation], redemptions=[red], receipts=[])
        before = exit_canary.settlement_state(status, paid)
        red.update(state="settled", settled_atoms=300000, settlement_receipt_hash="id")
        bucket.update(counted_value_atoms=0, redemption_queue_atoms=0)
        allocation["remaining_atoms"] = 0
        exit_canary.check_settlement(before, exit_canary.settlement_state(status, paid), "id")


if __name__ == "__main__":
    unittest.main()
