"""A burn whose Solana marker reads cancelled is never paid, never counted as paid, and ends."""

import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

import exit_payout as payout
import exit_service as svc

BURN = "c4" * 48


class CancelledBurnTest(unittest.TestCase):
    def test_a_cancelled_marker_stops_the_payout(self):
        c = Mock()
        c.account.return_value = dict(owner=str(payout.VERIFIER), data=b"\2")
        with self.assertRaises(payout.BurnCancelled):
            payout.signature(c, BURN)
        # A paid marker is still a payout.
        c.account.return_value = dict(owner=str(payout.VERIFIER), data=b"\1")
        c.call.side_effect = TimeoutError("history unavailable")
        self.assertIs(payout.signature(c, BURN), payout.PAID_NO_HISTORY)

    def test_a_cancelled_marker_is_not_counted_as_paid_custody(self):
        reds = [dict(burn_tx_id=b * 48, bucket_id=payout.BUCKET, amount_atoms=a, state="pending",
                     settled_atoms=0) for b, a in (("a1", 300000), ("b2", 400000))]
        markers = {str(payout.marker("a1" * 48)): dict(owner=str(payout.VERIFIER), data="AQ=="),
                   str(payout.marker("b2" * 48)): dict(owner=str(payout.VERIFIER), data="Ag==")}
        c = Mock(level={"commitment": "finalized"})
        c.call.side_effect = lambda method, keys, _cfg: dict(value=[markers[k] for k in keys])
        self.assertEqual(payout.paid_unsettled(c, reds, {payout.BUCKET: {}}), 300000)

    def test_service_ends_a_cancelled_burn_with_its_named_reason(self):
        with tempfile.TemporaryDirectory() as d:
            journal = svc.Journal(Path(d) / "jobs.sqlite")
            journal.enqueue(BURN)
            with patch.object(svc, "process", side_effect=payout.BurnCancelled()):
                svc.process_next(journal, Path(d))
            row = journal.get(BURN)
        self.assertEqual(row["stage"], "failed")
        self.assertTrue(row["error"].startswith(payout.CANCELLED))
        self.assertTrue(svc.public_row(row)["error"].startswith(payout.CANCELLED))


if __name__ == "__main__":
    unittest.main()
