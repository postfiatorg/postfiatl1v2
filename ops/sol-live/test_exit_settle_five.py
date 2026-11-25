"""Paid and refund settle with five of six: the decision is re-drafted with the five signers as
its supporters. Two hosts down settle nothing, and six signers draft once, as before."""

import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import exit_canary
import exit_govern
import exit_payout as payout
import govern
from exit_setup import read
from test_exit_lost_validator import REV, Fleet, run
from test_exit_refund_govern import BURN, KIND, RefundFleet, status as refund_status

SIX = [f"validator-{i}" for i in range(6)]
PACKET = "77" * 48


def without(*hosts):
    return [v for k, v in enumerate(SIX) if k not in hosts]


def paid_fleet(**options):
    bucket = dict(
        bucket_id=payout.BUCKET, policy_hash=payout.POLICY, source_domain=payout.SOURCE_DOMAIN,
        status="active", redeemable_at_par=True, impairment_factor_bps=10000,
        outstanding_vault_bridge_atoms=0, redemption_queue_atoms=300000,
        other_allocations_atoms=0, nav_subscription_allocations_atoms=0, counted_value_atoms=300000,
    )
    red = dict(burn_tx_id="a1" * 48, bucket_id=payout.BUCKET, amount_atoms=300000, state="pending",
               settled_atoms=0, settlement_receipt_hash="", owner=exit_canary.OWNER,
               destination_ref=exit_canary.DEST, withdrawal_packet_hash=PACKET)
    supply = dict(purpose="vault_bridge_supply", bucket_id=payout.BUCKET, amount_atoms=300000,
                  remaining_atoms=300000)
    return Fleet(dict(buckets=[bucket], redemptions=[red], allocations=[supply], receipts=[]), **options)


def run_paid(fleet, d):
    paid = dict(burn_tx_id="a1" * 48, solana_tx="sig", withdrawal_packet_hash=PACKET, amount_atoms=300000)
    ref = dict(packet=PACKET, slot=9, ordinal=0, signature_hex="ab" * 64)
    ref["kind"] = "solana_exit_r8_paid:{}:9:0:{}".format(PACKET, ref["signature_hex"])
    Path(d, "paid.json").write_text(json.dumps(paid))
    with (
        patch.object(exit_canary, "cluster"),
        patch.object(exit_canary, "finalized_reference", return_value=ref),
        patch.object(exit_canary, "signature", return_value="sig"),
    ):
        run(fleet, d, ["paid", "--revision", REV, "--release-height", "1", "--paid", str(Path(d, "paid.json")),
                       "--output", str(Path(d, "out.json"))])


def run_refund(fleet, d):
    with (
        patch.object(exit_govern, "cluster", create=True),
        patch.object(exit_govern, "refund_kind", create=True, return_value=KIND),
    ):
        run(fleet, d, ["refund", "--revision", REV, "--release-height", "1", "--burn", BURN,
                       "--output", str(Path(d, "out.json"))])


CASES = (("paid", paid_fleet, run_paid), ("refund", lambda **o: RefundFleet(refund_status(), **o), run_refund))


# The proposer of the next height (1801 % 6) is validator-1, so it stays up in these runs.
class SettleWithFiveTest(unittest.TestCase):
    def settle(self, make, go, **options):
        fleet = make(**options)
        with tempfile.TemporaryDirectory() as d:
            go(fleet, d)
            return fleet, read(Path(d, "out.json"))

    def test_one_host_down_settles_with_the_other_five(self):
        for name, make, go in CASES:
            for down in (0, 3, 5):
                with self.subTest(name, down=down):
                    fleet, out = self.settle(make, go, down=[down])
                    self.assertEqual(fleet.drafts, [SIX, without(down)])
                    self.assertEqual(fleet.assembled, without(down))
                    self.assertEqual(out["signers"], without(down))
                    self.assertEqual(fleet.submitted, 1)

    def test_one_reviewer_refusing_settles_with_the_other_five(self):
        for name, make, go in CASES:
            with self.subTest(name):
                fleet, out = self.settle(make, go, down=[], refusing=4)
                self.assertEqual(fleet.drafts, [SIX, without(4)])
                self.assertEqual(out["signers"], without(4))
                self.assertEqual(fleet.submitted, 1)

    def test_two_hosts_down_settle_nothing(self):
        for name, make, go in CASES:
            with self.subTest(name):
                fleet = make(down=[2, 4])
                with tempfile.TemporaryDirectory() as d:
                    with self.assertRaisesRegex(RuntimeError, "4 of 6 .*5 required.*validator-2, validator-4"):
                        go(fleet, d)
                self.assertIsNone(fleet.assembled)
                self.assertEqual(fleet.submitted, 0)

    def test_one_down_and_one_refusing_settle_nothing(self):
        for name, make, go in CASES:
            with self.subTest(name):
                fleet = make(down=[2], refusing=4)
                with tempfile.TemporaryDirectory() as d:
                    with self.assertRaisesRegex(RuntimeError, "validator-2.*refused by its own reviewer: validator-4"):
                        go(fleet, d)
                self.assertEqual(fleet.submitted, 0)

    def test_six_signers_draft_once_as_before(self):
        for name, make, go in CASES:
            with self.subTest(name):
                fleet, out = self.settle(make, go, down=[])
                self.assertEqual(fleet.drafts, [SIX])
                self.assertEqual(fleet.assembled, SIX)
                self.assertEqual(fleet.submitted, 1)

    def test_a_drafting_node_without_support_settles_nothing(self):
        # A node binary older than `--support` drafts six supporters again: named, not submitted.
        for name, make, go in CASES:
            with self.subTest(name):
                fleet = make(ignore_support=True)
                with tempfile.TemporaryDirectory() as d:
                    with self.assertRaisesRegex(ValueError, "supporters"):
                        go(fleet, d)
                self.assertEqual(fleet.submitted, 0)

    def test_redraft_may_change_only_supporters_votes_and_certificate(self):
        first = dict(kind="k", quorum=5, validators=SIX, support=SIX, votes=SIX, certificate_id="a",
                     amendment_id="x", activation_height=9)
        again = dict(first, support=without(2), votes=without(2), certificate_id="b", amendment_id="y")
        govern.check_redraft(first, again, without(2))
        for change in (dict(kind="other"), dict(quorum=4), dict(activation_height=10), dict(support=without(3))):
            with self.subTest(change=change), self.assertRaises(ValueError):
                govern.check_redraft(first, dict(again, **change), without(2))


if __name__ == "__main__":
    unittest.main()
