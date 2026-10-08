from __future__ import annotations

import json
import unittest
from typing import Any
from unittest import mock

from postfiat_rpc import PostFiatRpcClient, RpcError

ADDRESS = "pf-account"


def _chain(transfers_per_block: list[list[str]]) -> list[dict[str, Any]]:
    """Blocks at heights 1..n; each inner list holds the recipients of its transfers."""
    return [
        {
            "header": {"height": height, "batch_kind": "transparent", "batch_id": f"b{height}"},
            "receipt_ids": [f"tx{height}-{index}" for index in range(len(recipients))],
            "recipients": recipients,
        }
        for height, recipients in enumerate(transfers_per_block, start=1)
    ]


class AccountTxFallbackScanTests(unittest.TestCase):
    def _scan(
        self,
        chain: list[dict[str, Any]],
        *,
        limit: int,
        from_height: int | None = 1,
        to_height: int | None = None,
    ):
        client = PostFiatRpcClient("127.0.0.1:1234")

        def blocks(*, from_height: int | None = None, limit: int | None = None):
            if from_height is None:
                return chain[-limit:]
            return [b for b in chain if b["header"]["height"] >= from_height][:limit]

        def batch_archive(*, batch_kind: str, batch_id: str, limit: int):
            block = next(b for b in chain if b["header"]["batch_id"] == batch_id)
            payload = {
                "transactions": [
                    {"unsigned": {"from": ADDRESS, "to": to, "amount": 1}}
                    for to in block["recipients"]
                ]
            }
            return [{"payload_json": json.dumps(payload)}]

        refused = RpcError("account_tx", {"code": "rpc_method_not_allowed", "message": "off"})
        with (
            mock.patch.object(client, "_call", side_effect=refused),
            mock.patch.object(client, "blocks", side_effect=blocks),
            mock.patch.object(client, "batch_archive", side_effect=batch_archive),
            mock.patch.object(client, "receipts", return_value=[]),
        ):
            scan = client.account_tx(
                ADDRESS, from_height=from_height, to_height=to_height, limit=limit
            )
        self.assertFalse(scan.index_used)
        return scan

    @staticmethod
    def _ids(scan) -> list[str]:
        return [row.tx_id for row in scan.rows]

    def test_exactly_full_page_is_not_truncated(self) -> None:
        scan = self._scan(_chain([["a", "b"]]), limit=2)
        self.assertEqual(self._ids(scan), ["tx1-0", "tx1-1"])
        self.assertFalse(scan.truncated)

    def test_exactly_full_block_window_is_not_truncated(self) -> None:
        scan = self._scan(_chain([["a"], ["b"]]), limit=2)
        self.assertEqual(self._ids(scan), ["tx1-0", "tx2-0"])
        self.assertFalse(scan.truncated)

    def test_one_more_row_is_truncated(self) -> None:
        scan = self._scan(_chain([["a", "b", "c"]]), limit=2)
        self.assertEqual(self._ids(scan), ["tx1-0", "tx1-1"])
        self.assertTrue(scan.truncated)

    def test_without_start_height_returns_the_newest_rows(self) -> None:
        scan = self._scan(_chain([["a", "b", "c"]]), limit=2, from_height=None)
        self.assertEqual(self._ids(scan), ["tx1-1", "tx1-2"])
        self.assertTrue(scan.truncated)

    def test_unscanned_blocks_in_range_mark_the_result_possibly_incomplete(self) -> None:
        # Same answer as the server scan: blocks past the window are not read.
        scan = self._scan(_chain([["a"], [], []]), limit=2)
        self.assertEqual(self._ids(scan), ["tx1-0"])
        self.assertTrue(scan.truncated)
        bounded = self._scan(_chain([["a"], [], []]), limit=2, to_height=2)
        self.assertFalse(bounded.truncated)

    def test_end_height_below_tip_without_start_height_reads_that_range(self) -> None:
        # The server reads the newest `limit` blocks at or below `to_height`.
        chain = _chain([["a"], ["b"], ["c"], ["d"]])
        whole = self._scan(chain, limit=3, from_height=None, to_height=3)
        self.assertEqual(self._ids(whole), ["tx1-0", "tx2-0", "tx3-0"])
        self.assertFalse(whole.truncated)
        newest = self._scan(chain, limit=2, from_height=None, to_height=3)
        self.assertEqual(self._ids(newest), ["tx2-0", "tx3-0"])
        self.assertTrue(newest.truncated)

    def test_start_and_end_height_read_the_exact_window(self) -> None:
        chain = _chain([["a"], ["b"], ["c"], ["d"]])
        scan = self._scan(chain, limit=3, from_height=2, to_height=3)
        self.assertEqual(self._ids(scan), ["tx2-0", "tx3-0"])
        self.assertFalse(scan.truncated)


if __name__ == "__main__":
    unittest.main()


OTHER = "pf-other"
ISSUER = "pf-issuer"
ASSET_ID = "a" * 96


def _swap(leg_0_owner: str, leg_0_recipient: str, leg_1_owner: str, leg_1_recipient: str) -> dict:
    def leg(owner: str, recipient: str, amount: int, sequence: int) -> dict:
        return {
            "owner": owner,
            "recipient": recipient,
            "issuer": ISSUER,
            "asset_id": ASSET_ID,
            "amount": amount,
            "sequence": sequence,
            "fee": 1,
        }

    return {
        "unsigned": {
            "leg_0": leg(leg_0_owner, leg_0_recipient, 5, 7),
            "leg_1": leg(leg_1_owner, leg_1_recipient, 9, 3),
        }
    }


def _escrow_create(owner: str) -> dict:
    return {
        "unsigned": {
            "transaction_kind": "escrow",
            "fee": 1,
            "sequence": 11,
            "operation": {"operation": "escrow_create", "owner": owner, "recipient": OTHER, "amount": 4},
        }
    }


def _nft_transfer(sender: str) -> dict:
    return {
        "unsigned": {
            "transaction_kind": "nft",
            "fee": 1,
            "sequence": 12,
            "operation": {"operation": "nft_transfer", "from": sender, "to": OTHER, "issuer": ISSUER, "nft_id": "n" * 96},
        }
    }


def _offer_create(owner: str) -> dict:
    return {
        "unsigned": {
            "transaction_kind": "offer",
            "fee": 1,
            "sequence": 13,
            "operation": {
                "operation": "offer_create",
                "owner": owner,
                "taker_gets_amount": 2,
                "taker_gets_asset_id": "PFT",
                "taker_pays_asset_id": ASSET_ID,
            },
        }
    }


class AccountTxFallbackSwapRowTests(unittest.TestCase):
    """The archive scan (``account_tx_rows_for_transparent_block``) numbers a
    block's receipt ids transfers, payments_v2, asset, atomic swap, escrow,
    NFT, offer. The fallback must emit swap rows and keep every later kind on
    the same ``transaction_index`` and ``tx_id`` as the archive scan."""

    def _rows(self, payload: dict, receipt_count: int):
        client = PostFiatRpcClient("127.0.0.1:1234")
        block = {
            "header": {"height": 1, "batch_kind": "transparent", "batch_id": "b1"},
            "receipt_ids": [f"tx-{index}" for index in range(receipt_count)],
        }
        refused = RpcError("account_tx", {"code": "rpc_method_not_allowed", "message": "off"})
        with (
            mock.patch.object(client, "_call", side_effect=refused),
            mock.patch.object(client, "blocks", return_value=[block]),
            mock.patch.object(
                client, "batch_archive", return_value=[{"payload_json": json.dumps(payload)}]
            ),
            mock.patch.object(client, "receipts", return_value=[]),
        ):
            scan = client.account_tx(ADDRESS, from_height=1, limit=10)
        self.assertFalse(scan.index_used)
        return [
            (row.transaction_index, row.tx_id, row.transaction_kind, row.tx_role)
            for row in scan.rows
        ]

    def test_swap_rows_and_later_kinds_match_the_archive_scan_numbering(self):
        payload = {
            "transactions": [{"unsigned": {"from": ADDRESS, "to": OTHER, "amount": 1}}],
            "atomic_swap_transactions": [_swap(ADDRESS, OTHER, OTHER, ADDRESS)],
            "escrow_transactions": [_escrow_create(ADDRESS)],
            "nft_transactions": [_nft_transfer(ADDRESS)],
            "offer_transactions": [_offer_create(ADDRESS)],
        }
        # receipt ids: 0 transfer, 1 swap, 2 escrow, 3 nft, 4 offer
        self.assertEqual(
            self._rows(payload, receipt_count=5),
            [
                (0, "tx-0", "transparent_transfer", None),
                (1, "tx-1", "atomic_swap", "leg_0"),
                (1, "tx-1", "atomic_swap", "leg_1"),
                (2, "tx-2", "escrow", None),
                (3, "tx-3", "nft", None),
                (4, "tx-4", "offer", "offer_taker"),
            ],
        )

    def test_swap_row_fields_come_from_the_leg(self):
        client = PostFiatRpcClient("127.0.0.1:1234")
        block = {
            "header": {"height": 1, "batch_kind": "transparent", "batch_id": "b1"},
            "receipt_ids": ["tx-0"],
        }
        payload = {"transactions": [], "atomic_swap_transactions": [_swap(ADDRESS, OTHER, OTHER, OTHER)]}
        refused = RpcError("account_tx", {"code": "rpc_method_not_allowed", "message": "off"})
        with (
            mock.patch.object(client, "_call", side_effect=refused),
            mock.patch.object(client, "blocks", return_value=[block]),
            mock.patch.object(
                client, "batch_archive", return_value=[{"payload_json": json.dumps(payload)}]
            ),
            mock.patch.object(
                client, "receipts", return_value=[{"tx_id": "tx-0", "accepted": True, "code": "ok"}]
            ),
        ):
            scan = client.account_tx(ADDRESS, from_height=1, limit=10)
        # Only leg_0 involves the account.
        self.assertEqual(len(scan.rows), 1)
        row = scan.rows[0]
        self.assertEqual(row.transaction_kind, "atomic_swap")
        self.assertEqual(row.tx_role, "leg_0")
        self.assertEqual((row.sender, row.recipient), (ADDRESS, OTHER))
        self.assertEqual((row.amount, row.fee, row.sequence), (5, 1, 7))
        self.assertEqual((row.asset_id, row.issuer), (ASSET_ID, ISSUER))
        self.assertEqual((row.accepted, row.receipt_code), (True, "ok"))
        self.assertEqual(row.block_height, 1)
        self.assertEqual((row.batch_kind, row.batch_id), ("transparent", "b1"))

    def test_swaps_the_account_is_not_part_of_still_shift_later_rows(self):
        payload = {
            "transactions": [],
            "atomic_swap_transactions": [
                _swap(OTHER, OTHER, OTHER, OTHER),
                _swap(OTHER, OTHER, OTHER, OTHER),
            ],
            "escrow_transactions": [_escrow_create(ADDRESS)],
            "offer_transactions": [_offer_create(ADDRESS)],
        }
        # receipt ids: 0 swap, 1 swap, 2 escrow, 3 offer
        self.assertEqual(
            self._rows(payload, receipt_count=4),
            [(2, "tx-2", "escrow", None), (3, "tx-3", "offer", "offer_taker")],
        )

    def test_blocks_without_swaps_are_unchanged(self):
        payload = {
            "transactions": [{"unsigned": {"from": OTHER, "to": ADDRESS, "amount": 1}}],
            "escrow_transactions": [_escrow_create(ADDRESS)],
        }
        self.assertEqual(
            self._rows(payload, receipt_count=2),
            [(0, "tx-0", "transparent_transfer", None), (1, "tx-1", "escrow", None)],
        )
