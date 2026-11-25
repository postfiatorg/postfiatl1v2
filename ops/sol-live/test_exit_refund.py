"""A refund decision is drafted only for a burn whose Solana marker reads cancelled."""

import unittest
from unittest.mock import Mock

import base58
import exit_payout as payout

BURN = "c5" * 48
PACKET = "ab" * 48
SIG = base58.b58encode(bytes(range(64))).decode()


def cluster(marker_data, tag):
    key = payout.marker(BURN)
    c = Mock(level={"commitment": "finalized"})
    c.account.return_value = (
        None if marker_data is None else dict(owner=str(payout.VERIFIER), data=marker_data)
    )
    tx = dict(
        slot=473300000,
        meta=dict(err=None),
        transaction=dict(
            signatures=[SIG],
            message=dict(
                accountKeys=[str(payout.VERIFIER)] + [str(key)] * 7,
                instructions=[dict(programIdIndex=0, accounts=list(range(1, 8)),
                                   data=base58.b58encode(bytes([tag]) + b"proof").decode())],
            ),
        ),
    )
    c.call.side_effect = lambda method, *a: (
        [dict(signature=SIG, err=None)] if method == "getSignaturesForAddress" else tx
    )
    return c


class RefundKindTest(unittest.TestCase):
    red = dict(burn_tx_id=BURN, withdrawal_packet_hash=PACKET)

    def test_a_cluster_not_set_to_finalized_is_refused(self):
        c = cluster(b"\2", 10)
        c.level = {"commitment": "confirmed"}
        with self.assertRaisesRegex(ValueError, "finalized"):
            payout.refund_kind(c, self.red)
        c.account.assert_not_called()

    def test_refund_signatures_come_from_the_chain_report(self):
        import exit_rules

        report = dict(covered_bucket_ids=[exit_rules.BUCKET], min_withdrawal_atoms=300000,
                      decision_signatures=dict(open=6, close=5, paid=5, min_withdrawal=5, retire=6))
        rules = exit_rules.of_route(report)
        # A chain that does not report refunds cannot take one.
        with self.assertRaisesRegex(ValueError, "refund"):
            exit_rules.signatures(rules, "exit-refund")
        report["decision_signatures"]["refund"] = 5
        self.assertEqual(exit_rules.signatures(exit_rules.of_route(report), "exit-refund"), 5)
        with self.assertRaisesRegex(ValueError, "refund"):
            exit_rules.signatures(exit_rules.first_release(), "exit-refund")

    def test_a_cancelled_marker_gives_the_refund_decision(self):
        kind = payout.refund_kind(cluster(b"\2", 10), self.red)
        self.assertEqual(kind, f"solana_exit_refund:{PACKET}:473300000:{bytes(range(64)).hex()}")

    def test_no_refund_without_a_cancelled_marker(self):
        for data in (None, b"\1"):
            with self.assertRaisesRegex(ValueError, "refund refused"):
                payout.refund_kind(cluster(data, 10), self.red)
        # A cancelled marker whose history shows no cancel transaction is refused too.
        with self.assertRaisesRegex(ValueError, "refund refused"):
            payout.refund_kind(cluster(b"\2", 2), self.red)


if __name__ == "__main__":
    unittest.main()
