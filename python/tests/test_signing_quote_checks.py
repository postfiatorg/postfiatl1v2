"""Every signing helper refuses a quote that is not for its request before the
signer runs or anything is submitted."""

from __future__ import annotations

import copy
import importlib.util
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


# family -> (helper, client quote method, client submit method, operation).
# Operations are serialized as the Rust types serialize them
# (crates/types/src/transactions_mempool_receipts.rs): defaulted fields with
# `skip_serializing_if` are omitted; `AuthenticFixtureTests` checks each
# against the generated schema.
FAMILIES: dict[str, tuple[Callable[..., Any], str, str, dict[str, Any]]] = {
    "asset": (
        wallet_module.submit_asset_transaction,
        "asset_fee_quote_response",
        "mempool_submit_signed_asset_transaction",
        {"operation": "issued_payment", "from": "pf-holder", "to": "pf-dest", "issuer": "pf-issuer", "asset_id": "aa" * 48, "amount": 5},
    ),
    "escrow": (
        wallet_module.submit_escrow_transaction,
        "escrow_fee_quote_response",
        "mempool_submit_signed_escrow_transaction",
        {"operation": "escrow_create", "owner": "pf-holder", "recipient": "pf-dest", "asset_id": "PFT", "amount": 5, "finish_after": 100},
    ),
    "nft": (
        wallet_module.submit_nft_transaction,
        "nft_fee_quote_response",
        "mempool_submit_signed_nft_transaction",
        {"operation": "nft_transfer", "nft_id": "cc" * 48, "from": "pf-holder", "to": "pf-dest"},
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
        },
    ),
}

RECIPIENT_FIELD = {"asset": "to", "escrow": "recipient", "nft": "to", "offer": "taker_pays_asset_id"}
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


SCHEMAS = wallet_module.OPERATION_SCHEMAS


def _check(operation: dict[str, Any], quoted: dict[str, Any]) -> None:
    quote = _operation_quote("pf-holder", operation)
    quote["result"]["operation"] = quoted
    wallet_module.check_signing_quote(
        quote, wallet=_wallet(), kind=operation["operation"], sequence=7, max_fee_atoms=200, operation=operation
    )


def _without(operation: dict[str, Any], *keys: str) -> dict[str, Any]:
    return {key: value for key, value in operation.items() if key not in keys}


class AuthenticFixtureTests(unittest.TestCase):
    def test_fixtures_use_the_rust_field_names_and_every_required_field(self) -> None:
        for family, (_, _, _, operation) in FAMILIES.items():
            with self.subTest(family=family):
                fields = SCHEMAS["structs"][SCHEMAS["operations"][operation["operation"]]]
                self.assertLessEqual(set(operation) - {"operation"}, set(fields))
                required = {key for key, rule in fields.items() if rule == "required"}
                self.assertLessEqual(required, set(operation))

    def test_the_committed_schema_matches_the_rust_types(self) -> None:
        script = Path(__file__).resolve().parents[2] / "scripts/generate_operation_schemas.py"
        spec = importlib.util.spec_from_file_location("generate_operation_schemas", script)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        self.assertEqual(module.build(), SCHEMAS, "run scripts/generate_operation_schemas.py --write")


class DefaultFieldTests(unittest.TestCase):
    TRUST = {"operation": "trust_set", "account": "pf-holder", "issuer": "pf-issuer", "asset_id": "aa" * 48, "limit": 5}

    def test_typed_serde_defaults_equal_absence(self) -> None:
        # Rust serializes trust_set's defaulted flags; the request may omit them.
        _check(self.TRUST, {**self.TRUST, "authorized": False, "frozen": False, "reserve_paid": 0})
        _check({**self.TRUST, "authorized": False, "reserve_paid": 0}, self.TRUST)
        escrow = FAMILIES["escrow"][3]
        _check({**escrow, "condition": "", "cancel_after": 0}, escrow)
        for field, value in (("authorized", True), ("reserve_paid", 50), ("frozen", True), ("extra", "x")):
            with self.subTest(field=field):
                with self.assertRaises(QuoteRefusedError) as raised:
                    _check(self.TRUST, {**self.TRUST, field: value})
                self.assertEqual(raised.exception.code, "quote_mismatch")

    def test_a_wrong_typed_default_is_not_absence(self) -> None:
        for field, value in (("authorized", None), ("authorized", 0), ("reserve_paid", False), ("reserve_paid", None)):
            with self.subTest(field=field, value=value):
                with self.assertRaises(QuoteRefusedError):
                    _check({**self.TRUST, field: value}, self.TRUST)

    def test_a_missing_required_field_is_not_filled_by_a_default(self) -> None:
        with self.assertRaises(QuoteRefusedError):
            _check(self.TRUST, _without(self.TRUST, "limit"))
        with self.assertRaises(QuoteRefusedError):
            _check(_without(self.TRUST, "limit"), {**self.TRUST, "limit": 0})
        halt = {"operation": "nav_halt", "issuer": "pf-holder", "asset_id": "aa" * 48, "reason": "r"}
        with self.assertRaises(QuoteRefusedError):
            _check(halt, {**halt, "halted": False})

    # The review's "optional scalar fields where zero/empty is NOT absence".
    OPTION_NOT_ABSENT = [
        ("asset_create", "max_supply", 0),
        ("vault_bridge_nav_subscription_allocate", "consume_supply_owner", ""),
        ("vault_bridge_nav_subscription_allocate", "consume_supply_allocation_id", ""),
        ("vault_bridge_nav_subscription_allocate", "nav_recipient", ""),
        ("vault_bridge_nav_subscription_allocate", "subscription_id", ""),
        ("pftl_uniswap_order_reserve", "settlement_source_asset_id", ""),
        ("pftl_uniswap_primary_redeem", "settlement_source_asset_id", ""),
        ("pftl_uniswap_export_debit", "reservation_id", ""),
        ("pftl_uniswap_export_debit", "settlement_value_atoms", 0),
        ("pftl_uniswap_export_debit", "ethereum_packet_digest", ""),
        ("pftl_uniswap_export_debit", "ethereum_packet_schema_version", 0),
    ]

    def test_an_option_field_is_absent_only_as_null(self) -> None:
        for kind, field, value in self.OPTION_NOT_ABSENT:
            operation = {"operation": kind, "issuer": "pf-holder"}
            with self.subTest(operation=kind, field=field):
                _check({**operation, field: None}, operation)
                with self.assertRaises(QuoteRefusedError) as raised:
                    _check({**operation, field: value}, operation)
                self.assertIn(f"operation.{field}", str(raised.exception))
                with self.assertRaises(QuoteRefusedError):
                    _check(operation, {**operation, field: value})
        packet = {"operation": "fx_fix_register_v1", "operator": "pf-holder", "packet": {"packet_hash": "aa" * 48}}
        with self.assertRaises(QuoteRefusedError):
            _check({**packet, "packet": {**packet["packet"], "previous_fix_hash": ""}}, packet)

    def test_an_invalid_zero_cap_request_is_not_signed_as_an_uncapped_asset(self) -> None:
        create = {"operation": "asset_create", "issuer": "pf-holder", "code": "USD", "version": 1, "precision": 6}
        quote = _operation_quote("pf-holder", create)
        client = mock.Mock()
        client.asset_fee_quote_response.return_value = quote
        with tempfile.TemporaryDirectory() as tmp, mock.patch.object(wallet_module, "_run") as signer, mock.patch.object(
            wallet_module, "_run_json"
        ) as signer_json:
            with self.assertRaises(QuoteRefusedError) as raised:
                wallet_module.submit_asset_transaction(
                    client, wallet=_wallet(), operation={**create, "max_supply": 0}, work_dir=tmp, sequence=7
                )
        self.assertIn("operation.max_supply", str(raised.exception))
        signer.assert_not_called()
        signer_json.assert_not_called()


class EmptyVectorTests(unittest.TestCase):
    PAYMENT = {"operation": "issued_payment_v2", "from": "pf-holder", "to": "pf-dest", "issuer": "pf-issuer", "asset_id": "aa" * 48, "amount": 5}
    MEMO = {"memo_type": "", "memo_format": "", "memo_data": "6869"}

    def test_an_empty_default_vector_equals_absence(self) -> None:
        _check(self.PAYMENT, {**self.PAYMENT, "memos": []})
        _check({**self.PAYMENT, "memos": []}, self.PAYMENT)
        reserve = {"operation": "nav_reserve_submit", "issuer": "pf-holder", "reserve_accounts": []}
        _check({**reserve, "sp1_proof_bytes": [], "sp1_public_values": []}, reserve)
        settle = {"operation": "vault_bridge_redeem_settle", "asset_id": "aa" * 48}
        _check({**settle, "withdrawal_observations": []}, settle)
        propose = {"operation": "vault_bridge_deposit_propose", "proposer": "pf-holder"}
        _check(propose, {**propose, "source_proof_bytes": [], "source_public_values": []})

    def test_nested_defaults_follow_the_nested_type(self) -> None:
        _check({**self.PAYMENT, "memos": [{"memo_data": "6869"}]}, {**self.PAYMENT, "memos": [self.MEMO]})
        with self.assertRaises(QuoteRefusedError):
            _check({**self.PAYMENT, "memos": [{"memo_data": "6869", "memo_type": None}]}, {**self.PAYMENT, "memos": [self.MEMO]})

    def test_a_changed_or_reordered_list_is_refused(self) -> None:
        other = {**self.MEMO, "memo_data": "6a6a"}
        cases = [
            ([self.MEMO], []),
            ([], [self.MEMO]),
            ([self.MEMO], [other]),
            ([self.MEMO, other], [other, self.MEMO]),
        ]
        for requested, quoted in cases:
            with self.subTest(requested=requested, quoted=quoted):
                with self.assertRaises(QuoteRefusedError):
                    _check({**self.PAYMENT, "memos": requested}, {**self.PAYMENT, "memos": quoted})
        with self.assertRaises(QuoteRefusedError):
            _check(self.PAYMENT, {**self.PAYMENT, "memos": None})

    def test_an_empty_optional_vector_is_not_absence(self) -> None:
        advance = {"operation": "pftl_uniswap_route_epoch_advance", "operator": "pf-holder"}
        with self.assertRaises(QuoteRefusedError):
            _check({**advance, "settlement_source_asset_ids": []}, advance)
        _check({**advance, "settlement_source_asset_ids": None}, advance)


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


class SecondAttemptTests(unittest.TestCase):
    def test_latency_rechecks_the_quote_of_its_bad_sequence_retry(self) -> None:
        for memo, kind in ((False, "transparent_transfer"), (True, "payment_v2")):
            for name, path, value, code in TransferQuoteTests.CASES[:2] + TransferQuoteTests.CASES[5:6]:
                with self.subTest(memo=memo, case=name):
                    client = mock.Mock()
                    client.transfer_fee_quote_response.side_effect = [
                        RuntimeError("bad_sequence: stale account sequence"),
                        _changed(_transfer_quote(kind, "pf-dest", 5), path, value),
                    ]
                    signer = mock.Mock(side_effect=AssertionError("signer called"))
                    with tempfile.TemporaryDirectory() as tmp, mock.patch.object(
                        latency_module, "_run", signer
                    ), mock.patch.object(latency_module.time, "sleep"):
                        with self.assertRaises(QuoteRefusedError) as raised:
                            latency_module.run_native_or_payment(
                                client=client, wallet_a=_wallet(), wallet_b=_wallet("pf-dest"),
                                amount=5, work_dir=Path(tmp), memo=memo,
                            )
                    self.assertEqual(raised.exception.code, code)
                    self.assertEqual(client.transfer_fee_quote_response.call_count, 2)
                    signer.assert_not_called()


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

    INVALID_CEILINGS = (float("nan"), -1, 132.0, True, 2**64)

    def test_latency_refuses_an_invalid_ceiling_before_the_quote(self) -> None:
        for ceiling in self.INVALID_CEILINGS:
            for memo in (False, True):
                with self.subTest(ceiling=ceiling, memo=memo):
                    client = mock.Mock()
                    client.transfer_fee_quote_response.return_value = _transfer_quote(
                        "payment_v2" if memo else "transparent_transfer", "pf-dest", 5, fee=2**64 - 1
                    )
                    signer = mock.Mock(side_effect=AssertionError("signer called"))
                    with tempfile.TemporaryDirectory() as tmp, mock.patch.object(latency_module, "_run", signer):
                        with self.assertRaisesRegex(ValueError, "max_fee_atoms"):
                            latency_module.run_native_or_payment(
                                client=client,
                                wallet_a=_wallet(),
                                wallet_b=_wallet("pf-dest"),
                                amount=5,
                                work_dir=Path(tmp),
                                memo=memo,
                                max_fee_atoms=ceiling,
                            )
                    client.transfer_fee_quote_response.assert_not_called()
                    signer.assert_not_called()

    def test_latency_explicit_ceiling_is_applied(self) -> None:
        client = mock.Mock()
        client.transfer_fee_quote_response.return_value = _transfer_quote("transparent_transfer", "pf-dest", 5, fee=50)
        with tempfile.TemporaryDirectory() as tmp, mock.patch.object(latency_module, "_run") as signer:
            with self.assertRaises(QuoteRefusedError) as raised:
                latency_module.run_native_or_payment(
                    client=client, wallet_a=_wallet(), wallet_b=_wallet("pf-dest"), amount=5,
                    work_dir=Path(tmp), memo=False, max_fee_atoms=49,
                )
        self.assertEqual(raised.exception.code, "quote_fee_above_ceiling")
        signer.assert_not_called()

    def test_check_signing_quote_refuses_an_invalid_ceiling(self) -> None:
        quote = _transfer_quote("transparent_transfer", "pf-dest", 5, fee=2**64 - 1)
        for ceiling in self.INVALID_CEILINGS:
            with self.subTest(ceiling=ceiling):
                with self.assertRaisesRegex(ValueError, "max_fee_atoms"):
                    wallet_module.check_signing_quote(
                        quote, wallet=_wallet(), kind="transparent_transfer", sequence=7,
                        max_fee_atoms=ceiling, to_address="pf-dest", amount=5,
                    )

    def test_offer_create_without_a_ceiling_is_refused_before_the_quote(self) -> None:
        client = mock.Mock()
        with self.assertRaisesRegex(ValueError, "required for offer_create"):
            wallet_module.submit_offer_transaction(client, wallet=_wallet(), operation=FAMILIES["offer"][3])
        client.offer_fee_quote_response.assert_not_called()


if __name__ == "__main__":
    unittest.main()
