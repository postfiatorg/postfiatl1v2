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
        # Whether the amount fits is the quote's pending-aware dry run's.
        self.assertIsNone(external_lane_export_refusal(_row(), amount_atoms=601, base_recipient=RECIPIENT))


def _quote(**overrides: Any) -> dict[str, Any]:
    result: dict[str, Any] = {"sender_balance": 10_000, "minimum_fee": 22, "account_reserve": 10}
    result.update(overrides)
    return {"ok": True, "result": result}


SIGNED = {"unsigned": {"fee": 22}, "signature_hex": "aa"}


class ExternalLaneExportTests(unittest.TestCase):
    def _export(self, client: PostFiatRpcClient, quote: dict[str, Any], **kwargs: Any) -> tuple[Any, Any, Any, Any]:
        """Runs `external_lane_export` with the node reads and the signer
        subprocess mocked; returns (result or exception, signer, submit, quote)."""
        with (
            mock.patch.object(client, "external_lane_info", return_value={"lanes": [_row()]}),
            mock.patch.object(client, "asset_fee_quote_response", return_value=quote) as quote_call,
            mock.patch.object(client, "mempool_submit_signed_asset_transaction", return_value={"tx_id": "ab"}) as submit,
            mock.patch("postfiat_rpc.wallet._run") as signer,
            mock.patch("postfiat_rpc.wallet._read_json", return_value=SIGNED),
            mock.patch("postfiat_rpc.wallet._write_json"),
            mock.patch("postfiat_rpc.wallet._work_dir", return_value=Path("unused")),
        ):
            try:
                outcome: Any = external_lane_export(
                    client, wallet=_wallet(), lane_id="base-erc20", base_recipient=RECIPIENT, **kwargs
                )
            except ValueError as error:
                outcome = error
        return outcome, signer, submit, quote_call

    def test_export_quotes_signs_and_submits_the_operation(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        result, signer, submit, quote = self._export(client, _quote(), amount_atoms=600)
        self.assertEqual(result.tx_id, "ab")
        self.assertEqual(
            quote.call_args.args[1],
            {
                "operation": "external_lane_export_v1",
                "sender": "pf-holder",
                "lane_id": "base-erc20",
                "amount_atoms": 600,
                "base_recipient": RECIPIENT,
            },
        )
        signer.assert_called_once()
        submit.assert_called_once()
        self.assertEqual(submit.call_args.args[0], SIGNED)

    def test_a_dry_run_returns_the_quote_and_the_signed_transaction_and_never_submits(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        quote = _quote()
        result, signer, submit, _ = self._export(client, quote, amount_atoms=600, dry_run=True)
        signer.assert_called_once()
        submit.assert_not_called()
        self.assertEqual(result.quote_response, quote)
        self.assertEqual(result.signed_asset_transaction, SIGNED)
        self.assertEqual((result.tx_id, result.submit_result), (None, {}))
        # The refusals hold for a dry run too.
        error, signer, _, _ = self._export(client, _quote(sender_balance=0), amount_atoms=600, dry_run=True)
        self.assertRegex(str(error), "insufficient_funds")
        signer.assert_not_called()

    def test_a_balance_short_of_amount_fee_and_reserve_is_refused_before_the_signer_runs(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        # 600 + 22 + 10 = 632: one atom short, then exactly enough.
        error, signer, submit, _ = self._export(client, _quote(sender_balance=631), amount_atoms=600)
        self.assertRegex(str(error), "^refused before signing: insufficient_funds: balance 631 cannot cover")
        signer.assert_not_called()
        submit.assert_not_called()
        result, signer, _, _ = self._export(client, _quote(sender_balance=632), amount_atoms=600)
        self.assertEqual(result.tx_id, "ab")
        signer.assert_called_once()

    def test_the_quotes_pending_aware_export_refusal_is_refused_before_the_signer_runs(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        for code in (
            "external_lane_export_above_released",
            "external_lane_export_nonce_exhausted",
            "external_lane_export_records_full",
        ):
            with self.subTest(code=code):
                quote = _quote(export_refusal={"code": code, "message": "after the pending exports"})
                error, signer, submit, _ = self._export(client, quote, amount_atoms=1)
                self.assertRegex(str(error), f"^refused before signing: {code}: after the pending exports")
                signer.assert_not_called()
                submit.assert_not_called()

    def test_a_pending_release_lets_an_export_above_the_committed_amount_be_signed(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        # Committed released 1,000 minus exported 400 is 600; a pending
        # release makes 1,500 fit, so the quote carries no export_refusal.
        result, signer, submit, _ = self._export(client, _quote(), amount_atoms=1_500)
        self.assertEqual(result.tx_id, "ab")
        signer.assert_called_once()
        submit.assert_called_once()

    def test_a_refused_export_signs_and_submits_nothing(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        with (
            mock.patch.object(client, "external_lane_info", return_value={"lanes": [_row()]}),
            mock.patch.object(client, "asset_fee_quote_response") as quote,
            mock.patch("postfiat_rpc.wallet._run") as signer,
        ):
            with self.assertRaisesRegex(ValueError, "refused before signing: external_lane_export_recipient_is_token"):
                external_lane_export(
                    client, wallet=_wallet(), lane_id="base-erc20", amount_atoms=1, base_recipient=TOKEN
                )
            with self.assertRaisesRegex(ValueError, "amount_atoms must be an integer"):
                external_lane_export(
                    client, wallet=_wallet(), lane_id="base-erc20", amount_atoms=True, base_recipient=RECIPIENT
                )
        quote.assert_not_called()
        signer.assert_not_called()


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
        self.assertTrue(status["in_committed_block"])
        self.assertNotIn("block_log_verified", status)
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
            mock.patch.object(client, "tx", side_effect=RpcError("tx", {"code": "rpc_tx_not_found", "message": "not found"})),
        ):
            status = client.external_lane_export_status("base-erc20", "aa")
        self.assertEqual(info.call_args_list[1].kwargs["export_cursor"], 1)
        self.assertEqual(status["export_nonce"], 0)
        self.assertFalse(status["in_committed_block"])

    def test_status_audit_passes_audit_block_log_and_reports_block_log_verified(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        row = _row(export_count=1, state={"exported_atoms": 100, "next_export_nonce": 1}, exports=[_packet(0, "aa")])
        finality = {"confirmed": True, "block": {"header": {"height": 10}}, "block_log_verified": True}
        with (
            mock.patch.object(client, "external_lane_info", return_value={"lanes": [row]}),
            mock.patch.object(client, "tx", return_value=finality) as tx,
        ):
            status = client.external_lane_export_status("base-erc20", 0, audit=True)
        tx.assert_called_once_with("aa", audit_block_log=True)
        self.assertEqual((status["in_committed_block"], status["block_log_verified"]), (True, True))

    def test_status_with_a_sender_refuses_another_senders_packet(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        row = _row(export_count=1, state={"exported_atoms": 100, "next_export_nonce": 1}, exports=[_packet(0, "aa")])
        finality = {"confirmed": True, "block": {"header": {"height": 10}}}
        with (
            mock.patch.object(client, "external_lane_info", return_value={"lanes": [row]}),
            mock.patch.object(client, "tx", return_value=finality) as tx,
        ):
            self.assertEqual(client.external_lane_export_status("base-erc20", 0, sender="pf-holder")["sender"], "pf-holder")
            with self.assertRaisesRegex(ValueError, "^external_lane_export_other_sender: .*the lane's, not the sender's"):
                client.external_lane_export_status("base-erc20", 0, sender="pf-other")
        tx.assert_called_once()

    def test_status_raises_an_rpc_error_other_than_transaction_not_found(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        row = _row(export_count=1, state={"exported_atoms": 100, "next_export_nonce": 1}, exports=[_packet(0, "aa")])
        with (
            mock.patch.object(client, "external_lane_info", return_value={"lanes": [row]}),
            mock.patch.object(client, "tx", side_effect=RpcError("tx", {"code": "rpc_error", "message": "store unreadable"})),
        ):
            with self.assertRaisesRegex(RpcError, "store unreadable"):
                client.external_lane_export_status("base-erc20", 0)

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
