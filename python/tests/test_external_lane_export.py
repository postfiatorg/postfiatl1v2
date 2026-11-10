from __future__ import annotations

import unittest
from pathlib import Path
from typing import Any
from unittest import mock

from postfiat_rpc import PostFiatRpcClient, external_lane_export, external_lane_export_refusal
from postfiat_rpc.client import EXTERNAL_LANE_EXPORT_BASE_PAYOUT_NOTE, RpcError
from postfiat_rpc.wallet import TransparentWallet

LOCK = "0x" + "1a" * 20
TOKEN = "0x" + "1b" * 20
RECIPIENT = "0x" + "2c" * 20


def _row(**overrides: Any) -> dict[str, Any]:
    row: dict[str, Any] = {
        "lane_id": "base-erc20",
        "config": {"policy": {"exports_enabled": True, "lock_contract_address": LOCK, "token_address": TOKEN}},
        "release_cap": {"active_cap_bps": 100},
        "state": {"released_atoms": 1_000, "exported_atoms": 400, "next_export_nonce": 0},
        "export_count": 0,
        "exports": [],
        "next_export_cursor": None,
    }
    row.update(overrides)
    return row


def _packet(nonce: int, tx_id: str) -> dict[str, Any]:
    return {
        "packet": {
            "lane_id": "base-erc20",
            "export_nonce": nonce,
            "sender": "pf-holder",
            "base_recipient": RECIPIENT,
            "amount_atoms": 100 + nonce,
            "exported_at_height": 10 + nonce,
            "tx_id": tx_id,
        },
        "packet_digest": "0x" + "d1" * 48,
        "withdrawal_id_commitment": "0x" + "e1" * 48,
        "burn_tx_commitment": "0x" + "f1" * 48,
    }


def _wallet() -> TransparentWallet:
    return TransparentWallet(
        chain_id="postfiat-local",
        account_index=0,
        address="pf-holder",
        public_key_hex="00",
        key_file=Path("wallet.key.json"),
        backup_file=Path("wallet.backup.json"),
        key_report={},
    )


class ExternalLaneExportRefusalTests(unittest.TestCase):
    def test_each_refusal_before_signing_has_its_code(self) -> None:
        cases = [
            (None, 1, RECIPIENT, "external_lane_not_configured"),
            (_row(config={"policy": {"lock_contract_address": LOCK}}), 1, RECIPIENT, "external_lane_export_not_supported"),
            (_row(release_cap={"active_cap_bps": 0}), 1, RECIPIENT, "external_lane_export_halted"),
            (_row(), 0, RECIPIENT, "external_lane_export_amount_zero"),
            (_row(), 601, RECIPIENT, "external_lane_export_above_released"),
            (_row(), 1, "0x12", "external_lane_export_recipient_invalid"),
            (_row(), 1, "0x" + "2C" * 20, "external_lane_export_recipient_invalid"),
            (_row(), 1, "0x" + "00" * 20, "external_lane_export_recipient_invalid"),
            (_row(), 1, LOCK, "external_lane_export_recipient_is_lock"),
            (_row(), 1, TOKEN, "external_lane_export_recipient_is_token"),
        ]
        for row, amount, recipient, code in cases:
            with self.subTest(code=code, recipient=recipient):
                refusal = external_lane_export_refusal(row, amount_atoms=amount, base_recipient=recipient)
                self.assertIsNotNone(refusal)
                self.assertEqual(refusal[0], code)  # type: ignore[index]
        self.assertIsNone(external_lane_export_refusal(_row(), amount_atoms=600, base_recipient=RECIPIENT))


class ExternalLaneExportTests(unittest.TestCase):
    def test_export_builds_the_operation_and_goes_through_the_asset_signing_helper(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        with (
            mock.patch.object(client, "external_lane_info", return_value={"lanes": [_row()]}) as info,
            mock.patch("postfiat_rpc.wallet.submit_asset_transaction", return_value="result") as submit,
        ):
            result = external_lane_export(
                client, wallet=_wallet(), lane_id="base-erc20", amount_atoms=600, base_recipient=RECIPIENT
            )
        self.assertEqual(result, "result")
        info.assert_called_once_with("base-erc20", release_limit=1, export_limit=1)
        self.assertEqual(
            submit.call_args.kwargs["operation"],
            {
                "operation": "external_lane_export_v1",
                "sender": "pf-holder",
                "lane_id": "base-erc20",
                "amount_atoms": 600,
                "base_recipient": RECIPIENT,
            },
        )

    def test_a_refused_export_signs_and_submits_nothing(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        with (
            mock.patch.object(client, "external_lane_info", return_value={"lanes": [_row()]}),
            mock.patch("postfiat_rpc.wallet.submit_asset_transaction") as submit,
        ):
            with self.assertRaisesRegex(ValueError, "refused before signing: external_lane_export_recipient_is_token"):
                external_lane_export(
                    client, wallet=_wallet(), lane_id="base-erc20", amount_atoms=1, base_recipient=TOKEN
                )
            with self.assertRaisesRegex(ValueError, "amount_atoms must be an integer"):
                external_lane_export(
                    client, wallet=_wallet(), lane_id="base-erc20", amount_atoms=True, base_recipient=RECIPIENT
                )
        submit.assert_not_called()


class ExternalLaneExportStatusTests(unittest.TestCase):
    def test_status_by_nonce_reads_one_page_and_reports_finality(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        row = _row(export_count=2, state={"exported_atoms": 201, "next_export_nonce": 2}, exports=[_packet(1, "aa")])
        finality = {"confirmed": True, "block": {"header": {"height": 11}}}
        with (
            mock.patch.object(client, "external_lane_info", return_value={"current_height": 12, "lanes": [row]}) as info,
            mock.patch.object(client, "tx", return_value=finality),
        ):
            status = client.external_lane_export_status("base-erc20", 1)
        info.assert_called_once_with("base-erc20", export_cursor=2, export_limit=100)
        self.assertEqual((status["export_nonce"], status["amount_atoms"], status["tx_id"]), (1, 101, "aa"))
        self.assertEqual(status["packet_digest"], "0x" + "d1" * 48)
        self.assertTrue(status["block_final"])
        self.assertEqual((status["lane_exported_atoms"], status["lane_export_count"]), (201, 2))
        self.assertEqual(status["base_payout"], EXTERNAL_LANE_EXPORT_BASE_PAYOUT_NOTE)

    def test_status_by_tx_id_pages_older_exports_and_unconfirmed_is_not_final(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        state = {"exported_atoms": 201, "next_export_nonce": 2}
        pages = [
            {"lanes": [_row(export_count=2, state=state, exports=[_packet(1, "bb")], next_export_cursor=1)]},
            {"lanes": [_row(export_count=2, state=state, exports=[_packet(0, "aa")])]},
        ]
        with (
            mock.patch.object(client, "external_lane_info", side_effect=pages) as info,
            mock.patch.object(client, "tx", side_effect=RpcError("tx", {"code": "transaction_not_found", "message": "not found"})),
        ):
            status = client.external_lane_export_status("base-erc20", "aa")
        self.assertEqual(info.call_args_list[1].kwargs["export_cursor"], 1)
        self.assertEqual(status["export_nonce"], 0)
        self.assertFalse(status["block_final"])

    def test_status_of_an_unknown_lane_or_export_raises(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        with mock.patch.object(client, "external_lane_info", return_value={"lanes": []}):
            with self.assertRaisesRegex(LookupError, "external_lane_not_configured"):
                client.external_lane_export_status("base-erc21", 0)
        with mock.patch.object(client, "external_lane_info", return_value={"lanes": [_row()]}):
            with self.assertRaisesRegex(LookupError, "no stored export"):
                client.external_lane_export_status("base-erc20", "cc")


if __name__ == "__main__":
    unittest.main()
