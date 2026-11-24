"""Under the second release each host signs paid and refund only through its own reviewer;
the first release keeps the go-live review files and every validator's signature."""

import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import exit_canary
import govern
from exit_setup import read
from test_exit_lost_validator import BIN, LOST, REV, Fleet, run


class ReviewedFleet(Fleet):
    """Each host's reviewer approves only the decisions in `approved[i]`; the signer wrapper
    on the host runs the node's signing command only for those."""

    def __init__(self, *args, approved=None, **kwargs):
        super().__init__(*args, **kwargs)
        self.approved = approved
        self.central_reads = 0
        self.wrapped = []

    def remote(self, i, args, data=None):
        args = [str(a) for a in args]
        if i != LOST and args[-1] == "/etc/postfiat/sol-exit-paid-review.json":
            self.central_reads += 1
        rest = args[4:] if args[:1] == ["runuser"] else []
        if i != LOST and rest[:3] == [govern.REVIEWER_PYTHON, "-I", govern.REVIEWER]:
            assert rest[3] == "sign" and rest[4] == "--amendment-file", rest
            self.wrapped.append(i)
            kind = json.loads(self.files[(i, rest[5])])["kind"]
            approved = self.approved(i) if self.approved else {kind}
            if kind not in approved:
                raise subprocess.CalledProcessError(2, args, stderr="refused: no review of this decision")
            # No command is passed: the reviewer builds the signing command itself.
            assert "--" not in rest and BIN not in rest, rest
            return super().remote(i, args, data)
        return super().remote(i, args, data)


def paid(fleet, d):
    from test_exit_lost_validator import payout

    burn = "e7" * 48
    packet = "f7" * 48
    bucket = dict(
        bucket_id=payout.BUCKET, policy_hash=payout.POLICY, source_domain=payout.SOURCE_DOMAIN,
        status="active", redeemable_at_par=True, impairment_factor_bps=10000,
        outstanding_vault_bridge_atoms=0, redemption_queue_atoms=300000,
        other_allocations_atoms=0, nav_subscription_allocations_atoms=0, counted_value_atoms=300000,
    )
    fleet.status = dict(
        buckets=[bucket], allocations=[], receipts=[],
        redemptions=[dict(burn_tx_id=burn, bucket_id=payout.BUCKET, amount_atoms=300000, state="pending",
                          settled_atoms=0, settlement_receipt_hash="", owner=exit_canary.OWNER,
                          destination_ref=exit_canary.DEST, withdrawal_packet_hash=packet)])
    Path(d, "paid.json").write_text(json.dumps(dict(burn_tx_id=burn, solana_tx="sig", withdrawal_packet_hash=packet,
                                                    amount_atoms=300000)))
    ref = dict(packet=packet, slot=9, ordinal=0, signature_hex="ab" * 64)
    ref["kind"] = f"solana_exit_r8_paid:{packet}:9:0:{ref['signature_hex']}"
    with (
        patch.object(exit_canary, "cluster"),
        patch.object(exit_canary, "finalized_reference", return_value=ref),
        patch.object(exit_canary, "signature", return_value="sig"),
    ):
        run(fleet, d, ["paid", "--revision", REV, "--release-height", "1", "--paid", str(Path(d, "paid.json")),
                       "--output", str(Path(d, "out.json"))])
    return ref["kind"]


class IndependentSignTest(unittest.TestCase):
    def test_second_release_signs_through_each_hosts_own_reviewer(self):
        fleet = ReviewedFleet()
        with tempfile.TemporaryDirectory() as d:
            paid(fleet, d)
            self.assertEqual(len(read(Path(d, "out.json"))["signers"]), 5)
        self.assertEqual(sorted(set(fleet.wrapped)), [0, 1, 2, 4, 5])
        # The relay no longer reads central review files as the hosts' approvals.
        self.assertEqual(fleet.central_reads, 0)

    def test_hosts_without_their_own_review_do_not_sign(self):
        # Only three hosts reviewed this decision: five signatures cannot be reached.
        fleet = ReviewedFleet(approved=lambda i: {fleet.kind} if i in (0, 1, 2) else set())
        with tempfile.TemporaryDirectory() as d, self.assertRaisesRegex(RuntimeError, "required"):
            paid(fleet, d)

    def test_first_release_keeps_the_go_live_reviews_and_six_signatures(self):
        # A first-release chain reports no rules: every validator must answer and sign, and the
        # per-host reviewer is not involved (one lost validator stops the round as before).
        fleet = ReviewedFleet(second=False)
        with tempfile.TemporaryDirectory() as d, self.assertRaisesRegex(RuntimeError, "6 required"):
            paid(fleet, d)
        self.assertEqual(fleet.wrapped, [])

    def test_first_release_signing_is_unchanged(self):
        self.assertFalse(govern.independent_review("exit-paid", None))
        self.assertFalse(govern.independent_review("exit-paid", dict(release=1)))
        self.assertTrue(govern.independent_review("exit-paid", dict(release=2)))
        self.assertTrue(govern.independent_review("exit-refund", dict(release=2)))
        self.assertFalse(govern.independent_review("exit-close", dict(release=2)))


class ClearanceTest(unittest.TestCase):
    def head(self):
        return dict(block_height=1800, block_tip_hash="tip", state_root="root", build_git_revision=REV)

    def run_clearance(self, answer):
        import exit_govern

        asked = []

        def remote(i, args, data=None):
            args = [str(a) for a in args]
            if args[0] == "find":
                return "height-1800.json\n"
            if args[:4] == [govern.REVIEWER_PYTHON, "-I", govern.REVIEWER, "show-clearance"]:
                asked.append(i)
                return json.dumps(answer(i))
            raise AssertionError(args)

        with (
            patch.object(govern, "remote", side_effect=remote),
            patch.object(exit_govern, "require_closed"),
            patch.object(exit_govern, "routes", return_value=[]),
            patch.object(exit_govern, "fleet", return_value=self.head()),
        ):
            rows = exit_govern.rotation_clearance(self.head(), 6, rules=dict(release=2))
        return asked, rows

    def test_second_release_clearance_comes_from_each_hosts_own_reviewer(self):
        parent = dict(block_height=1800, block_tip_hash="tip", state_root="root")
        asked, rows = self.run_clearance(lambda i: dict(
            validator=f"validator-{i}", parent=parent, revision=REV, outstanding_signed_rotation_rounds=[]))
        self.assertEqual(sorted(asked), list(range(6)))
        self.assertEqual(len(rows), 6)

    def test_a_clearance_for_another_parent_is_refused(self):
        stale = dict(block_height=1799, block_tip_hash="old", state_root="root")
        with self.assertRaisesRegex(ValueError, "rotation clearance"):
            self.run_clearance(lambda i: dict(validator=f"validator-{i}", parent=stale, revision=REV,
                                              outstanding_signed_rotation_rounds=[]))


if __name__ == "__main__":
    unittest.main()
