"""Validator reads of the payout worker: a hanging validator is skipped, one attempt has a
deadline, ancestry pages must link, oversized pages shrink, and a rotation is named."""

import socket
import time
import unittest
from pathlib import Path
from unittest.mock import patch

import exit_payout as payout
import live


def header(height, parent, committee="c1"):
    return dict(height=height, block_hash=f"h{height}", parent_hash=parent, committee=committee)


def chain(start, end, committee="c1"):
    return [dict(header=header(h, f"h{h - 1}", committee)) for h in range(start, end)]


class Reads:
    """Validator 0 hangs; the others answer from one chain."""

    def __init__(self, blocks=None, too_large_above=None, empty=False):
        self.calls = []
        self.blocks = {b["header"]["height"]: b for b in (blocks or chain(1, 200))}
        self.too_large_above = too_large_above
        self.empty = empty

    def __call__(self, i, method, timeout=None, **params):
        self.calls.append((i, method, timeout))
        if i == 0:
            raise socket.timeout("timed out")
        if method == "blocks":
            limit = params["limit"]
            if self.too_large_above is not None and limit > self.too_large_above:
                if self.empty:
                    raise live.EmptyReply(f"{i} blocks: empty reply")
                raise live.ReplyTooLarge(f"{i} blocks: rpc response exceeds the frame limit")
            first = params["from_height"]
            return [self.blocks[h] for h in range(first, first + limit) if h in self.blocks]
        return dict(method=method)


class ValidatorReadTest(unittest.TestCase):
    def setUp(self):
        payout.reset_reads()

    def test_a_hanging_validator_is_skipped_for_a_while(self):
        reads = Reads()
        with patch.object(payout, "rpc", reads):
            payout.rpc_any("status")
            payout.rpc_any("status")
            payout.rpc_any("vault_bridge_status")
        self.assertEqual([c[0] for c in reads.calls], [0, 1, 1, 1])

    def test_small_reads_have_a_short_timeout_and_witness_reads_a_long_one(self):
        reads = Reads()
        with patch.object(payout, "rpc", reads):
            payout.rpc_any("status")
            payout.rpc_any("pfusdc_egress_witness", withdrawal_id="r", prior_checkpoint="p")
        timeouts = {method: t for i, method, t in reads.calls if i == 1}
        self.assertLessEqual(timeouts["status"], 30)
        self.assertEqual(timeouts["pfusdc_egress_witness"], 120)

    def test_one_payout_attempt_has_a_deadline(self):
        reads = Reads()
        with patch.object(payout, "rpc", reads):
            with payout.attempt_deadline(time.monotonic() - 1):
                with self.assertRaises(payout.AttemptExpired):
                    payout.rpc_any("status")
            # Outside an attempt (admission, status) there is no deadline.
            payout.rpc_any("status")
        self.assertEqual(len([c for c in reads.calls if c[1] == "status"]), 2)

    def test_a_reply_too_large_is_not_retried_on_the_other_validators(self):
        reads = Reads(too_large_above=0)
        payout.reset_reads()
        with patch.object(payout, "rpc", reads):
            with self.assertRaises(live.ReplyTooLarge):
                payout.rpc_any("blocks", from_height=1, limit=1)
        # Validator 0 hangs; validator 1 answers too large; nobody else is asked.
        self.assertEqual([c[0] for c in reads.calls if c[1] == "blocks"], [0, 1])


class AncestryPageTest(unittest.TestCase):
    def setUp(self):
        payout.reset_reads()

    def test_pages_shrink_to_one_block_when_a_reply_is_too_large(self):
        for empty in (False, True):
            reads = Reads(too_large_above=2, empty=empty)
            with patch.object(payout, "rpc", reads):
                blocks = payout.ancestry_pages(100, 120)
            self.assertEqual([b["header"]["height"] for b in blocks], list(range(100, 120)))

    def test_a_block_too_large_for_one_reply_ends_the_attempt_by_name(self):
        reads = Reads(too_large_above=0)
        with patch.object(payout, "rpc", reads):
            with self.assertRaises(payout.BlockTooLarge):
                payout.ancestry_pages(100, 120)

    def test_pages_that_do_not_link_fail_before_proving(self):
        blocks = chain(1, 200)
        blocks[109] = dict(header=header(110, "other"))
        reads = Reads(blocks=blocks)
        with patch.object(payout, "rpc", reads):
            with self.assertRaisesRegex(ValueError, "do not link"):
                payout.ancestry_pages(100, 120)

    def test_a_rotation_in_the_block_before_the_burn_is_named(self):
        blocks = chain(1, 120) + [dict(header=header(120, "h119", "c2"))]
        reads = Reads(blocks=blocks)
        witness = dict(prior_checkpoint_block_id="h119", block=dict(header=header(120, "h119", "c2")))

        def read(i, method, timeout=None, **params):
            if method == "pfusdc_egress_witness":
                return witness
            return reads(i, method, timeout, **params)

        with (
            patch.object(payout, "rpc", read),
            patch.object(payout, "committee", side_effect=lambda h: h["committee"]),
        ):
            with self.assertRaises(payout.CommitteeRotated):
                payout.egress_witness(None, dict(redemption_id="r"), "h100", 120, Path("/nonexistent/w"))


class ServiceTest(unittest.TestCase):
    def test_named_read_failures_are_filed_once_and_an_expired_attempt_is_retried(self):
        import tempfile
        from contextlib import closing

        import exit_service as svc

        burn = "d1" * 48
        for error, stage in ((payout.BlockTooLarge("block 9"), "failed"),
                             (payout.CommitteeRotated(), "failed"),
                             (live.ReplyTooLarge("0 blocks: too large"), "failed"),
                             (payout.AttemptExpired("deadline"), "queued")):
            with tempfile.TemporaryDirectory() as d:
                j = svc.Journal(Path(d) / "db")
                j.enqueue(burn)
                with patch.object(svc, "process", side_effect=error):
                    svc.process_next(j, Path(d))
                with closing(j.connect()) as db:
                    row = db.execute("SELECT stage, error FROM jobs WHERE burn_tx_id=?", (burn,)).fetchone()
                self.assertEqual(row[0], stage, error)
                if stage == "failed":
                    shown = svc.public_row(j.get(burn))["error"]
                    self.assertTrue(shown.startswith((payout.BLOCK_TOO_LARGE, payout.ROTATED,
                                                      "Node reply too large")), shown)


if __name__ == "__main__":
    unittest.main()
