"""A payout finalized on Solana but not yet settled no longer counts as a custody claim."""

import unittest
from unittest.mock import patch

import exit_payout as payout


def setup():
    burns = ["a1" * 48, "b2" * 48]
    bucket = dict(
        bucket_id=payout.BUCKET, policy_hash=payout.POLICY, source_domain=payout.SOURCE_DOMAIN,
        status="active", redeemable_at_par=True, impairment_factor_bps=10000,
        outstanding_vault_bridge_atoms=0, redemption_queue_atoms=600000,
        other_allocations_atoms=0, nav_subscription_allocations_atoms=0,
        counted_value_atoms=600000,
    )
    reds = [
        dict(burn_tx_id=b, bucket_id=payout.BUCKET, amount_atoms=300000, state="pending",
             settled_atoms=0)
        for b in burns
    ]
    return burns, dict(buckets=[bucket], redemptions=reds)


class PaidUnsettledTest(unittest.TestCase):
    def test_only_verifier_markers_of_pending_covered_burns_count(self):
        from unittest.mock import Mock

        r9 = payout.bucket_id("09" * 48)
        covered = {payout.BUCKET: {}, r9: {}}
        burns = [("a1", payout.BUCKET, 300000, "pending", 0), ("b2", r9, 400000, "pending", 0),
                 ("c3", payout.BUCKET, 300000, "pending", 0), ("d4", r9, 500000, "pending", 0),
                 ("e5", payout.BUCKET, 700000, "settled", 700000), ("f6", "other", 900000, "pending", 0)]
        reds = [dict(burn_tx_id=b * 48, bucket_id=k, amount_atoms=a, state=s, settled_atoms=x)
                for b, k, a, s, x in burns]
        verifier = dict(owner=str(payout.VERIFIER), data="AQ==")
        # a1, b2 and c3 paid; d4's marker address pre-funded by anyone (system-owned).
        owners = {"a1": verifier, "b2": verifier, "c3": verifier,
                  "d4": dict(owner=str(payout.SYSTEM), data="")}
        by_key = {str(payout.marker(r["burn_tx_id"])): owners.get(r["burn_tx_id"][:2]) for r in reds}
        c = Mock(level={"commitment": "finalized"})
        c.call.side_effect = lambda method, keys, _cfg: dict(value=[by_key[k] for k in keys])
        self.assertEqual(payout.paid_unsettled(c, reds, covered), 1000000)

    def test_second_payout_proceeds_before_the_first_is_settled(self):
        burns, status = setup()

        class Solana:
            level = {"commitment": "finalized"}
            paid = {burns[0]}

            def call(self, method, keys, *rest):
                assert method == "getMultipleAccounts"
                owners = [str(payout.marker(b)) for b in burns if b in self.paid]
                return dict(value=[dict(owner=str(payout.VERIFIER), data=["AQ==", "base64"]) if k in owners else None for k in keys])

        # The first payout left custody; the second is still payable before settlement.
        with (
            patch.object(payout, "rpc", return_value=status),
            patch.object(payout, "inspect_vault", return_value={"custody_atoms": 300000}),
        ):
            _, _, required = payout.check_custody(Solana())
            self.assertEqual(required, 300000)
            # Without a finalized marker the unsettled claim still counts.
            Solana.paid = set()
            with self.assertRaisesRegex(ValueError, "Shared custody"):
                payout.check_custody(Solana())


if __name__ == "__main__":
    unittest.main()
