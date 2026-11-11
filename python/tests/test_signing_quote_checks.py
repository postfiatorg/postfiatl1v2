"""Every signing helper refuses a quote that is not for its request before the
signer runs or anything is submitted."""

from __future__ import annotations

import copy
import tempfile
import unittest
from pathlib import Path
from typing import Any, Callable
from unittest import mock


from postfiat_rpc import latency as latency_module
from postfiat_rpc import wallet as wallet_module
from postfiat_rpc.wallet import (
    ASSET_TRANSACTION_DEFAULT_MAX_FEE_ATOMS,
    TRANSFER_DEFAULT_MAX_FEE_ATOMS,
    QuoteRefusedError,
    TransparentWallet,
    default_max_fee_atoms,
)

GENESIS = "ab" * 48


def _wallet(address: str = "pf-holder") -> TransparentWallet:
    return TransparentWallet(
        chain_id="postfiat-local",
        account_index=0,
        address=address,
        public_key_hex="00",
        key_file=Path("/nonexistent/wallet.key.json"),
        backup_file=Path("wallet.backup.json"),
        key_report={"genesis_hash": GENESIS},
    )


def _operation_quote(source: str, operation: dict[str, Any], sequence: int = 7, fee: int = 22) -> dict[str, Any]:
    return {
        "result": {
            "transaction_kind": operation["operation"],
            "chain_id": "postfiat-local",
            "genesis_hash": GENESIS,
            "protocol_version": 1,
            "source": source,
            "operation": copy.deepcopy(operation),
            "sequence": sequence,
            "minimum_fee": fee,
        }
    }


def _transfer_quote(kind: str, to: str, amount: int, sequence: int = 7, fee: int = 22) -> dict[str, Any]:
    return {
        "result": {
            "transaction_kind": kind,
            "chain_id": "postfiat-local",
            "genesis_hash": GENESIS,
            "protocol_version": 1,
            "from": "pf-holder",
            "to": to,
            "amount": amount,
            "sequence": sequence,
            "minimum_fee": fee,
        }
    }


def _changed(quote: dict[str, Any], path: tuple[str, ...], value: Any) -> dict[str, Any]:
    quote = copy.deepcopy(quote)
    target = quote["result"]
    for key in path[:-1]:
        target = target[key]
    target[path[-1]] = value
    return quote


# family -> (helper, client quote method, client submit method, operation, field that names the recipient/asset)
FAMILIES: dict[str, tuple[Callable[..., Any], str, str, dict[str, Any]]] = {
    "asset": (
        wallet_module.submit_asset_transaction,
        "asset_fee_quote_response",
        "mempool_submit_signed_asset_transaction",
        {"operation": "issued_payment", "source": "pf-holder", "destination": "pf-dest", "asset_id": "aa" * 48, "amount": 5},
    ),
    "escrow": (
        wallet_module.submit_escrow_transaction,
        "escrow_fee_quote_response",
        "mempool_submit_signed_escrow_transaction",
        {"operation": "escrow_create", "owner": "pf-holder", "recipient": "pf-dest", "asset_id": "PFT", "amount": 5},
    ),
    "nft": (
        wallet_module.submit_nft_transaction,
        "nft_fee_quote_response",
        "mempool_submit_signed_nft_transaction",
        {"operation": "nft_transfer", "owner": "pf-holder", "nft_id": "cc" * 48, "destination": "pf-dest"},
    ),
    "offer": (
        wallet_module.submit_offer_transaction,
        "offer_fee_quote_response",
        "mempool_submit_signed_offer_transaction",
        {
            "operation": "offer_create",
            "owner": "pf-holder",
            "taker_gets_asset_id": "PFT",
            "taker_gets_amount": 5,
            "taker_pays_asset_id": "dd" * 48,
            "taker_pays_amount": 6,
            "expiration_height": 0,
        },
    ),
}

RECIPIENT_FIELD = {"asset": "destination", "escrow": "recipient", "nft": "destination", "offer": "taker_pays_asset_id"}
ASSET_FIELD = {"asset": "asset_id", "escrow": "asset_id", "nft": "nft_id", "offer": "taker_gets_asset_id"}
AMOUNT_FIELD = {"asset": "amount", "escrow": "amount", "nft": None, "offer": "taker_gets_amount"}


def _operation_cases(family: str, operation: dict[str, Any], quote: dict[str, Any]) -> list[tuple[str, dict[str, Any], str]]:
    cases = [
        ("recipient", _changed(quote, ("operation", RECIPIENT_FIELD[family]), "pf-attacker"), "quote_mismatch"),
        ("asset", _changed(quote, ("operation", ASSET_FIELD[family]), "ee" * 48), "quote_mismatch"),
        ("kind", _changed(quote, ("transaction_kind",), "trust_set"), "quote_mismatch"),
        ("operation kind", _changed(quote, ("operation", "operation"), "something_else"), "quote_mismatch"),
        ("extra field", _changed(quote, ("operation", "extra"), 1), "quote_mismatch"),
        ("source", _changed(quote, ("source",), "pf-other"), "quote_mismatch"),
        ("fee", _changed(quote, ("minimum_fee",), 10**9), "quote_fee_above_ceiling"),
        ("fee type", _changed(quote, ("minimum_fee",), "22"), "quote_invalid"),
        ("sequence", _changed(quote, ("sequence",), 8), "quote_mismatch"),
        ("chain", _changed(quote, ("chain_id",), "postfiat-other"), "quote_mismatch"),
        ("genesis", _changed(quote, ("genesis_hash",), "cd" * 48), "quote_mismatch"),
    ]
    if AMOUNT_FIELD[family]:
        cases.append(("amount", _changed(quote, ("operation", AMOUNT_FIELD[family]), 5_000), "quote_mismatch"))
        cases.append(("amount as float", _changed(quote, ("operation", AMOUNT_FIELD[family]), 5.0), "quote_mismatch"))
    return cases


class OperationHelperQuoteTests(unittest.TestCase):
    def _run(self, family: str, quote: dict[str, Any]) -> tuple[Any, mock.Mock, mock.Mock, mock.Mock]:
        helper, quote_method, submit_method, operation = FAMILIES[family]
        client = mock.Mock()
        submit = mock.Mock(return_value={"tx_id": "ff" * 48})
        signer = mock.Mock(side_effect=AssertionError("signer called"))
        signer_json = mock.Mock(side_effect=AssertionError("signer called"))
        with tempfile.TemporaryDirectory() as tmp, mock.patch.object(
            client, quote_method, return_value=quote
        ), mock.patch.object(client, submit_method, submit), mock.patch.object(
            wallet_module, "_run", signer
        ), mock.patch.object(wallet_module, "_run_json", signer_json):
            try:
                result: Any = helper(
                    client,
                    wallet=_wallet(),
                    operation=copy.deepcopy(operation),
                    work_dir=tmp,
                    sequence=7,
                    max_fee_atoms=200,
                )
            except (QuoteRefusedError, AssertionError) as error:
                result = error
        return result, signer, signer_json, submit

    def test_each_changed_quote_is_refused_before_the_signer_and_submit(self) -> None:
        for family, (_, _, _, operation) in FAMILIES.items():
            quote = _operation_quote("pf-holder", operation)
            for name, changed, code in _operation_cases(family, operation, quote):
                with self.subTest(family=family, case=name):
                    result, signer, signer_json, submit = self._run(family, changed)
                    self.assertIsInstance(result, QuoteRefusedError)
                    self.assertEqual(result.code, code)
                    signer.assert_not_called()
                    signer_json.assert_not_called()
                    submit.assert_not_called()

    def test_matching_quote_reaches_the_signer(self) -> None:
        for family, (_, _, _, operation) in FAMILIES.items():
            with self.subTest(family=family):
                result, signer, signer_json, submit = self._run(family, _operation_quote("pf-holder", operation))
                self.assertNotIsInstance(result, QuoteRefusedError)
                self.assertEqual(signer.call_count + signer_json.call_count, 1)
                submit.assert_not_called()


class TransferQuoteTests(unittest.TestCase):
    CASES = [
        ("recipient", ("to",), "pf-attacker", "quote_mismatch"),
        ("amount", ("amount",), 5_000, "quote_mismatch"),
        ("amount type", ("amount",), "5", "quote_invalid"),
        ("from", ("from",), "pf-other", "quote_mismatch"),
        ("kind", ("transaction_kind",), "asset_create", "quote_mismatch"),
        ("fee", ("minimum_fee",), 10**9, "quote_fee_above_ceiling"),
        ("sequence", ("sequence",), 8, "quote_mismatch"),
        ("chain", ("chain_id",), "postfiat-other", "quote_mismatch"),
        ("genesis", ("genesis_hash",), "cd" * 48, "quote_mismatch"),
    ]

    def _send(self, quote: dict[str, Any], memo: bool) -> tuple[Any, mock.Mock, mock.Mock]:
        client = mock.Mock()
        signer = mock.Mock(side_effect=AssertionError("signer called"))
        submit = mock.Mock(return_value={"tx_id": "ff" * 48})
        with tempfile.TemporaryDirectory() as tmp, mock.patch.object(
            client, "transfer_fee_quote_response", return_value=quote
        ), mock.patch.object(wallet_module, "_run", signer), mock.patch.object(
            client, "mempool_submit_signed_transfer", submit
        ), mock.patch.object(client, "mempool_submit_signed_payment_v2", submit):
            try:
                result: Any = wallet_module.send_pft(
                    client,
                    wallet=_wallet(),
                    to_address="pf-dest",
                    amount=5,
                    work_dir=tmp,
                    sequence=7,
                    memo_data="6869" if memo else None,
                )
            except QuoteRefusedError as error:
                result = error
            except AssertionError as error:
                result = error
        return result, signer, submit

    def test_send_pft_refuses_each_changed_quote(self) -> None:
        for memo, kind in ((False, "transparent_transfer"), (True, "payment_v2")):
            quote = _transfer_quote(kind, "pf-dest", 5)
            for name, path, value, code in self.CASES:
                with self.subTest(memo=memo, case=name):
                    result, signer, submit = self._send(_changed(quote, path, value), memo)
                    self.assertIsInstance(result, QuoteRefusedError)
                    self.assertEqual(result.code, code)
                    signer.assert_not_called()
                    submit.assert_not_called()
            with self.subTest(memo=memo, case="matching"):
                result, signer, submit = self._send(quote, memo)
                self.assertEqual(signer.call_count, 1)

    def test_latency_payment_refuses_a_changed_quote(self) -> None:
        for memo, kind in ((False, "transparent_transfer"), (True, "payment_v2")):
            for name, path, value, code in self.CASES[:2] + self.CASES[5:6]:
                with self.subTest(memo=memo, case=name):
                    client = mock.Mock()
                    quote = _changed(_transfer_quote(kind, "pf-dest", 5), path, value)
                    client.transfer_fee_quote_response.return_value = quote
                    signer = mock.Mock(side_effect=AssertionError("signer called"))
                    with tempfile.TemporaryDirectory() as tmp, mock.patch.object(latency_module, "_run", signer):
                        with self.assertRaises(QuoteRefusedError) as raised:
                            latency_module.run_native_or_payment(
                                client=client,
                                wallet_a=_wallet(),
                                wallet_b=_wallet("pf-dest"),
                                amount=5,
                                work_dir=Path(tmp),
                                memo=memo,
                            )
                    self.assertEqual(raised.exception.code, code)
                    signer.assert_not_called()
                    client.mempool_submit_signed_transfer_finality.assert_not_called()
                    client.mempool_submit_signed_payment_v2_finality.assert_not_called()


class FeeCeilingTests(unittest.TestCase):
    def test_defaults_follow_the_fee_rule(self) -> None:
        self.assertEqual(TRANSFER_DEFAULT_MAX_FEE_ATOMS, 4 * (23 + 10))
        self.assertEqual(ASSET_TRANSACTION_DEFAULT_MAX_FEE_ATOMS, 4 * (23 + 30))
        self.assertEqual(default_max_fee_atoms("transfer"), 132)
        self.assertEqual(default_max_fee_atoms("escrow", {"operation": "escrow_cancel"}), 132 + 4)
        trust = {"operation": "trust_set", "reserve_paid": 1_000}
        self.assertEqual(default_max_fee_atoms("asset", trust), 212 + 4 * 970 + 4)
        self.assertEqual(default_max_fee_atoms("offer", {"operation": "offer_cancel"}), 92 + 4)
        with self.assertRaisesRegex(ValueError, "required for offer_create"):
            default_max_fee_atoms("offer", {"operation": "offer_create"})

    def test_offer_create_without_a_ceiling_is_refused_before_the_quote(self) -> None:
        client = mock.Mock()
        with self.assertRaisesRegex(ValueError, "required for offer_create"):
            wallet_module.submit_offer_transaction(client, wallet=_wallet(), operation=FAMILIES["offer"][3])
        client.offer_fee_quote_response.assert_not_called()


if __name__ == "__main__":
    unittest.main()
