from __future__ import annotations

import unittest
from unittest import mock

from postfiat_rpc.client import PostFiatRpcClient


def _row(index, tx_id, kind, sender, recipient, amount, role, fill_index=None, offer_id=None):
    return {
        "tx_id": tx_id,
        "block_height": 7,
        "batch_kind": "transparent",
        "batch_id": "batch-7",
        "transaction_index": index,
        "transaction_kind": kind,
        "from": sender,
        "to": recipient,
        "amount": amount,
        "fee": 0,
        "sequence": 0,
        "offer_id": offer_id,
        "tx_role": role,
        "fill_index": fill_index,
        "accepted": True,
        "receipt_code": "ok",
    }


class AccountTxHistoryRowTests(unittest.TestCase):
    def test_history_keeps_every_swap_leg_and_offer_fill(self) -> None:
        # account_tx lists one row per atomic swap leg the account owns or
        # receives and one row per offer fill, all with the same tx_id and
        # transaction_index (docs/rpc/account-history.md).
        rows = [
            _row(0, "swap-tx", "atomic_swap", "pf-a", "pf-b", 100, "leg_0"),
            _row(0, "swap-tx", "atomic_swap", "pf-b", "pf-a", 250, "leg_1"),
            _row(1, "offer-tx", "offer_create", "pf-a", "pf-a", 0, "create", offer_id="o-taker"),
            _row(1, "offer-tx", "offer_create", "pf-b", "pf-a", 40, "maker", 0, "o-m1"),
            _row(1, "offer-tx", "offer_create", "pf-b", "pf-a", 60, "maker", 1, "o-m2"),
        ]
        result = {
            "address": "pf-a",
            "from_height": 7,
            "to_height": 7,
            "scan_limit": 512,
            "row_count": len(rows),
            "truncated": False,
            "index_used": True,
            "scanned_block_count": 1,
            "archive_lookup_count": 0,
            "rows": rows,
        }
        client = PostFiatRpcClient("127.0.0.1:1234")
        with mock.patch.object(client, "_call", return_value=result):
            history = client.account_tx_history("pf-a", from_height=7, to_height=7)

        self.assertTrue(history.complete)
        self.assertEqual(history.windows[0]["row_count"], 5)
        self.assertEqual(
            [(row.tx_id, row.tx_role, row.fill_index, row.amount) for row in history.rows],
            [
                ("swap-tx", "leg_0", None, 100),
                ("swap-tx", "leg_1", None, 250),
                ("offer-tx", "create", None, 0),
                ("offer-tx", "maker", 0, 40),
                ("offer-tx", "maker", 1, 60),
            ],
        )


if __name__ == "__main__":
    unittest.main()
