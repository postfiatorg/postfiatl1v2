from __future__ import annotations

import json
import stat
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import postfiat_rpc.wallet as wallet_module
from postfiat_rpc import PostFiatRpcClient, PostFiatWebSocketRpcClient
from postfiat_rpc.wallet import (
    OrchardWallet,
    TransparentWallet,
    WalletCommandError,
    _first_certified_tx_id,
    _escrow_id,
    _first_receipt_tx_id,
    _issued_asset_id,
    _nft_id,
    _offer_id,
    _write_json,
    _work_dir,
    authorize_trustline,
    authorize_asset_trustline,
    build_atomic_settlement_template,
    build_atomic_swap_template,
    burn_nft,
    burn_non_fungible_token,
    cancel_escrow,
    cancel_offer,
    cancel_pft_escrow,
    clawback_token,
    clawback_issued_asset,
    create_asset_trustline,
    create_escrow,
    create_issued_asset,
    create_issued_asset_escrow,
    create_offer,
    create_pft_escrow,
    execute_atomic_settlement,
    freeze_asset_trustline,
    freeze_trustline,
    finish_escrow,
    finish_pft_escrow,
    load_wallet,
    mint_non_fungible_token,
    mint_token,
    mint_nft,
    place_offer,
    request_faucet_pft,
    revoke_trustline_authorization,
    revoke_asset_trustline_authorization,
    send_issued_asset,
    send_payment,
    send_fastpay,
    send_pft,
    send_pft_and_poll_finality,
    send_token,
    set_asset_trustline_control,
    set_trustline,
    submit_escrow_transaction,
    transfer_non_fungible_token,
    transfer_nft,
    unfreeze_trustline,
    unfreeze_asset_trustline,
    unwrap_fastpay,
    wrap_fastpay,
)


class WalletHelperTests(unittest.TestCase):
    def test_work_dir_creates_supplied_directory(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "wallet-work"
            self.assertEqual(_work_dir(path), path)
            self.assertTrue(path.is_dir())

    def test_default_work_dir_is_private(self) -> None:
        path = _work_dir(None)
        try:
            mode = stat.S_IMODE(path.stat().st_mode)
            self.assertEqual(mode, 0o700)
        finally:
            path.rmdir()

    def test_write_json_creates_private_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "wallet.json"
            _write_json(path, {"secret": "value"})
            self.assertEqual(json.loads(path.read_text(encoding="utf-8")), {"secret": "value"})
            mode = stat.S_IMODE(path.stat().st_mode)
            self.assertEqual(mode, 0o600)

    def test_first_receipt_tx_id_accepts_list_receipts(self) -> None:
        self.assertEqual(_first_receipt_tx_id([[{"tx_id": "abc"}]]), "abc")

    def test_first_receipt_tx_id_accepts_object_receipts(self) -> None:
        self.assertEqual(_first_receipt_tx_id([{"receipts": [{"tx_id": "def"}]}]), "def")

    def test_first_certified_tx_id_accepts_hot_finality_receipts(self) -> None:
        self.assertEqual(
            _first_certified_tx_id({"local_hot_finality": [{"receipt": {"tx_id": "abc"}}]}),
            "abc",
        )

    def test_load_wallet_reads_key_report_and_backup_path(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            wallet_dir = Path(tmp)
            key_file = wallet_dir / "transparent-0.key.json"
            backup_file = wallet_dir / "transparent-0.backup.json"
            key_file.write_text(
                json.dumps({"address": "pfabc", "public_key_hex": "00"}),
                encoding="utf-8",
            )
            backup_file.write_text(
                json.dumps({"chain_id": "postfiat-local"}),
                encoding="utf-8",
            )

            wallet = load_wallet(wallet_dir=wallet_dir, chain_id="postfiat-local")

        self.assertEqual(wallet.address, "pfabc")
        self.assertEqual(wallet.public_key_hex, "00")
        self.assertEqual(wallet.chain_id, "postfiat-local")

    def test_load_wallet_rejects_backup_chain_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            wallet_dir = Path(tmp)
            key_file = wallet_dir / "transparent-0.key.json"
            backup_file = wallet_dir / "transparent-0.backup.json"
            key_file.write_text(
                json.dumps({"address": "pfabc", "public_key_hex": "00"}),
                encoding="utf-8",
            )
            backup_file.write_text(
                json.dumps({"chain_id": "65100"}),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(WalletCommandError, "chain_id mismatch"):
                load_wallet(wallet_dir=wallet_dir, chain_id="postfiat-local")

    def test_request_faucet_pft_can_use_peer_certified_round(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            data_dir = root / "node0"
            data_dir.mkdir()
            topology = root / "topology.json"
            key_file = root / "validator_keys.json"
            proposal_key_file = root / "proposal_keys.json"
            topology.write_text("{}", encoding="utf-8")
            key_file.write_text("{}", encoding="utf-8")
            proposal_key_file.write_text("{}", encoding="utf-8")

            with (
                mock.patch.object(wallet_module, "_node_bin", return_value=["postfiat-node"]),
                mock.patch.object(
                    wallet_module,
                    "_run_json",
                    side_effect=[
                        {"batch": "created"},
                        {
                            "local_hot_finality": [{"receipt": {"tx_id": "tx-certified"}}],
                            "certification": {"vote_count": 6},
                        },
                    ],
                ) as run_json,
                mock.patch.object(wallet_module, "_apply_batch") as apply_batch,
            ):
                result = request_faucet_pft(
                    data_dir=data_dir,
                    to_address="pfrecipient",
                    amount=20_000_000,
                    work_dir=root / "work",
                    certify_topology=topology,
                    certify_key_file=key_file,
                    certify_proposal_key_file=proposal_key_file,
                )

        self.assertEqual(result.tx_id, "tx-certified")
        self.assertIsNotNone(result.certified_round)
        self.assertIsNotNone(result.certified_artifact_dir)
        apply_batch.assert_not_called()
        certify_args = run_json.call_args_list[1].args[0]
        self.assertIn("transport-peer-certified-batch-round", certify_args)
        self.assertIn("--proposal-key-file", certify_args)
        self.assertIn(str(proposal_key_file), certify_args)

    def test_transfer_fee_quote_response_returns_envelope(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        envelope = {
            "version": "postfiat-local-rpc-v1",
            "id": "quote-1",
            "ok": True,
            "result": {"schema": "postfiat-transfer-fee-quote-v1"},
        }
        with mock.patch.object(client, "_send", return_value=envelope):
            response = client.transfer_fee_quote_response(
                "pf-from",
                "pf-to",
                1,
                request_id="quote-1",
            )
        self.assertEqual(response, envelope)

    def test_payment_v2_submit_sends_signed_json_param(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        envelope = {
            "version": "postfiat-local-rpc-v1",
            "id": "submit-1",
            "ok": True,
            "result": {"tx_id": "ab" * 48, "payment": {"unsigned": {}}},
        }
        with mock.patch.object(client, "_send", return_value=envelope) as send:
            result = client.mempool_submit_signed_payment_v2(
                {"unsigned": {"memos": [{"memo_data": "74657374"}]}},
                request_id="submit-1",
            )
        self.assertEqual(result["tx_id"], "ab" * 48)
        request = send.call_args.args[0]
        self.assertEqual(request["method"], "mempool_submit_signed_payment_v2")
        self.assertIn("signed_payment_v2_json", request["params"])

    def test_asset_fee_quote_response_sends_operation_json(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        envelope = {
            "version": "postfiat-local-rpc-v1",
            "id": "asset-quote-1",
            "ok": True,
            "result": {"schema": "postfiat-asset-fee-quote-v1"},
        }
        operation = {
            "operation": "asset_create",
            "issuer": "pf-issuer",
            "code": "USD",
            "version": 1,
            "precision": 2,
        }
        with mock.patch.object(client, "_send", return_value=envelope) as send:
            response = client.asset_fee_quote_response(
                "pf-issuer",
                operation,
                sequence=2,
                request_id="asset-quote-1",
            )
        self.assertEqual(response, envelope)
        request = send.call_args.args[0]
        self.assertEqual(request["method"], "asset_fee_quote")
        self.assertEqual(request["params"]["source"], "pf-issuer")
        self.assertIn("operation_json", request["params"])
        self.assertEqual(request["params"]["sequence"], 2)

    def test_asset_transaction_submit_sends_signed_json_param(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        envelope = {
            "version": "postfiat-local-rpc-v1",
            "id": "asset-submit-1",
            "ok": True,
            "result": {"tx_id": "cd" * 48, "transaction": {"unsigned": {}}},
        }
        with mock.patch.object(client, "_send", return_value=envelope) as send:
            result = client.mempool_submit_signed_asset_transaction(
                {"unsigned": {"transaction_kind": "asset_create"}},
                request_id="asset-submit-1",
            )
        self.assertEqual(result["tx_id"], "cd" * 48)
        request = send.call_args.args[0]
        self.assertEqual(request["method"], "mempool_submit_signed_asset_transaction")
        self.assertIn("signed_asset_transaction_json", request["params"])

    def test_escrow_fee_quote_and_submit_send_json_params(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        quote_envelope = {
            "version": "postfiat-local-rpc-v1",
            "id": "escrow-quote-1",
            "ok": True,
            "result": {"schema": "postfiat-escrow-fee-quote-v1"},
        }
        operation = {
            "operation": "escrow_create",
            "owner": "pf-owner",
            "recipient": "pf-recipient",
            "asset_id": "PFT",
            "amount": 10,
            "condition": "hashlock",
        }
        with mock.patch.object(client, "_send", return_value=quote_envelope) as send:
            response = client.escrow_fee_quote_response(
                "pf-owner",
                operation,
                sequence=3,
                request_id="escrow-quote-1",
            )
        self.assertEqual(response, quote_envelope)
        request = send.call_args.args[0]
        self.assertEqual(request["method"], "escrow_fee_quote")
        self.assertEqual(request["params"]["source"], "pf-owner")
        self.assertIn("operation_json", request["params"])
        self.assertEqual(request["params"]["sequence"], 3)

        submit_envelope = {
            "version": "postfiat-local-rpc-v1",
            "id": "escrow-submit-1",
            "ok": True,
            "result": {"tx_id": "ef" * 48, "transaction": {"unsigned": {}}},
        }
        with mock.patch.object(client, "_send", return_value=submit_envelope) as send:
            result = client.mempool_submit_signed_escrow_transaction(
                {"unsigned": {"transaction_kind": "escrow_create"}},
                request_id="escrow-submit-1",
            )
        self.assertEqual(result["tx_id"], "ef" * 48)
        request = send.call_args.args[0]
        self.assertEqual(request["method"], "mempool_submit_signed_escrow_transaction")
        self.assertIn("signed_escrow_transaction_json", request["params"])

    def test_nft_fee_quote_and_submit_send_json_params(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        quote_envelope = {
            "version": "postfiat-local-rpc-v1",
            "id": "nft-quote-1",
            "ok": True,
            "result": {"schema": "postfiat-nft-fee-quote-v1"},
        }
        operation = {
            "operation": "nft_mint",
            "issuer": "pf-issuer",
            "collection_id": "NFT-003",
            "serial": 1,
            "owner": "pf-owner",
            "metadata_hash": "ab" * 32,
        }
        with mock.patch.object(client, "_send", return_value=quote_envelope) as send:
            response = client.nft_fee_quote_response(
                "pf-issuer",
                operation,
                sequence=4,
                request_id="nft-quote-1",
            )
        self.assertEqual(response, quote_envelope)
        request = send.call_args.args[0]
        self.assertEqual(request["method"], "nft_fee_quote")
        self.assertEqual(request["params"]["source"], "pf-issuer")
        self.assertIn("operation_json", request["params"])
        self.assertEqual(request["params"]["sequence"], 4)

        submit_envelope = {
            "version": "postfiat-local-rpc-v1",
            "id": "nft-submit-1",
            "ok": True,
            "result": {"tx_id": "12" * 48, "transaction": {"unsigned": {}}},
        }
        with mock.patch.object(client, "_send", return_value=submit_envelope) as send:
            result = client.mempool_submit_signed_nft_transaction(
                {"unsigned": {"transaction_kind": "nft_mint"}},
                request_id="nft-submit-1",
            )
        self.assertEqual(result["tx_id"], "12" * 48)
        request = send.call_args.args[0]
        self.assertEqual(request["method"], "mempool_submit_signed_nft_transaction")
        self.assertIn("signed_nft_transaction_json", request["params"])

    def test_offer_fee_quote_and_submit_send_json_params(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        quote_envelope = {
            "version": "postfiat-local-rpc-v1",
            "id": "offer-quote-1",
            "ok": True,
            "result": {"schema": "postfiat-offer-fee-quote-v1"},
        }
        operation = {
            "operation": "offer_create",
            "owner": "pf-owner",
            "taker_gets_asset_id": "PFT",
            "taker_gets_amount": 25,
            "taker_pays_asset_id": "ab" * 48,
            "taker_pays_amount": 10,
            "expiration_height": 50,
        }
        with mock.patch.object(client, "_send", return_value=quote_envelope) as send:
            response = client.offer_fee_quote_response(
                "pf-owner",
                operation,
                sequence=5,
                request_id="offer-quote-1",
            )
        self.assertEqual(response, quote_envelope)
        request = send.call_args.args[0]
        self.assertEqual(request["method"], "offer_fee_quote")
        self.assertEqual(request["params"]["source"], "pf-owner")
        self.assertIn("operation_json", request["params"])
        self.assertEqual(request["params"]["sequence"], 5)

        submit_envelope = {
            "version": "postfiat-local-rpc-v1",
            "id": "offer-submit-1",
            "ok": True,
            "result": {"tx_id": "34" * 48, "transaction": {"unsigned": {}}},
        }
        with mock.patch.object(client, "_send", return_value=submit_envelope) as send:
            result = client.mempool_submit_signed_offer_transaction(
                {"unsigned": {"transaction_kind": "offer_create"}},
                request_id="offer-submit-1",
            )
        self.assertEqual(result["tx_id"], "34" * 48)
        request = send.call_args.args[0]
        self.assertEqual(request["method"], "mempool_submit_signed_offer_transaction")
        self.assertIn("signed_offer_transaction_json", request["params"])

    def test_atomic_settlement_template_sends_swap_params(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        asset_id = "aa" * 48
        def response_for_request(request: dict[str, object]) -> dict[str, object]:
            return {
                "version": "postfiat-local-rpc-v1",
                "id": request["id"],
                "ok": True,
                "result": {
                    "schema": "postfiat-atomic-settlement-template-v1",
                    "settlement_id": "bb" * 48,
                },
            }

        with mock.patch.object(client, "_send", side_effect=response_for_request) as send:
            result = client.atomic_settlement_template(
                left_owner="pf-pft-owner",
                left_recipient="pf-issued-owner",
                left_asset_id="PFT",
                left_amount=100,
                right_owner="pf-issued-owner",
                right_recipient="pf-pft-owner",
                right_asset_id=asset_id,
                right_amount=25,
                condition="shared-secret",
                finish_after=7,
                cancel_after=12,
                left_sequence=2,
            )
        self.assertEqual(result["settlement_id"], "bb" * 48)
        request = send.call_args.args[0]
        self.assertEqual(request["method"], "atomic_settlement_template")
        self.assertEqual(request["params"]["left_asset_id"], "PFT")
        self.assertEqual(request["params"]["right_asset_id"], asset_id)
        self.assertEqual(request["params"]["left_amount"], 100)
        self.assertEqual(request["params"]["right_amount"], 25)
        self.assertEqual(request["params"]["condition"], "shared-secret")
        self.assertEqual(request["params"]["finish_after"], 7)
        self.assertEqual(request["params"]["cancel_after"], 12)
        self.assertEqual(request["params"]["left_sequence"], 2)
        self.assertNotIn("right_sequence", request["params"])

    def test_asset_read_methods_send_bounded_params(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        asset_id = "01" * 48
        result_schema = {"value": "postfiat-asset-info-v1"}

        def response_for_request(request: dict[str, object]) -> dict[str, object]:
            return {
                "version": "postfiat-local-rpc-v1",
                "id": request["id"],
                "ok": True,
                "result": {"schema": result_schema["value"]},
            }

        with mock.patch.object(client, "_send", side_effect=response_for_request) as send:
            result = client.asset_info(asset_id)
        self.assertEqual(result["schema"], "postfiat-asset-info-v1")
        request = send.call_args.args[0]
        self.assertEqual(request["method"], "asset_info")
        self.assertEqual(request["params"], {"asset_id": asset_id})

        result_schema["value"] = "postfiat-account-lines-v1"
        with mock.patch.object(client, "_send", side_effect=response_for_request) as send:
            client.account_lines(
                "pf-holder",
                issuer="pf-issuer",
                asset_id=asset_id,
                limit=5,
            )
        request = send.call_args.args[0]
        self.assertEqual(request["method"], "account_lines")
        self.assertEqual(request["params"]["account"], "pf-holder")
        self.assertEqual(request["params"]["issuer"], "pf-issuer")
        self.assertEqual(request["params"]["asset_id"], asset_id)
        self.assertEqual(request["params"]["limit"], 5)

        result_schema["value"] = "postfiat-account-assets-v1"
        with mock.patch.object(client, "_send", side_effect=response_for_request) as send:
            client.account_assets("pf-holder", asset_id=asset_id, limit=6)
        request = send.call_args.args[0]
        self.assertEqual(request["method"], "account_assets")
        self.assertEqual(request["params"]["account"], "pf-holder")
        self.assertEqual(request["params"]["asset_id"], asset_id)
        self.assertEqual(request["params"]["limit"], 6)

        result_schema["value"] = "postfiat-issuer-assets-v1"
        with mock.patch.object(client, "_send", side_effect=response_for_request) as send:
            client.issuer_assets("pf-issuer", limit=7)
        request = send.call_args.args[0]
        self.assertEqual(request["method"], "issuer_assets")
        self.assertEqual(request["params"], {"issuer": "pf-issuer", "limit": 7})

        escrow_id = "02" * 48
        result_schema["value"] = "postfiat-escrow-info-v1"
        with mock.patch.object(client, "_send", side_effect=response_for_request) as send:
            client.escrow_info(escrow_id)
        request = send.call_args.args[0]
        self.assertEqual(request["method"], "escrow_info")
        self.assertEqual(request["params"], {"escrow_id": escrow_id})

        result_schema["value"] = "postfiat-account-escrows-v1"
        with mock.patch.object(client, "_send", side_effect=response_for_request) as send:
            client.account_escrows(
                "pf-owner",
                role="owner",
                state="open",
                limit=8,
            )
        request = send.call_args.args[0]
        self.assertEqual(request["method"], "account_escrows")
        self.assertEqual(request["params"]["account"], "pf-owner")
        self.assertEqual(request["params"]["role"], "owner")
        self.assertEqual(request["params"]["state"], "open")
        self.assertEqual(request["params"]["limit"], 8)

        nft_id = "03" * 48
        result_schema["value"] = "postfiat-nft-info-v1"
        with mock.patch.object(client, "_send", side_effect=response_for_request) as send:
            client.nft_info(nft_id)
        request = send.call_args.args[0]
        self.assertEqual(request["method"], "nft_info")
        self.assertEqual(request["params"], {"nft_id": nft_id})

        result_schema["value"] = "postfiat-account-nfts-v1"
        with mock.patch.object(client, "_send", side_effect=response_for_request) as send:
            client.account_nfts("pf-owner", include_burned=True, limit=9)
        request = send.call_args.args[0]
        self.assertEqual(request["method"], "account_nfts")
        self.assertEqual(request["params"]["account"], "pf-owner")
        self.assertEqual(request["params"]["include_burned"], True)
        self.assertEqual(request["params"]["limit"], 9)

        result_schema["value"] = "postfiat-issuer-nfts-v1"
        with mock.patch.object(client, "_send", side_effect=response_for_request) as send:
            client.issuer_nfts(
                "pf-issuer",
                collection_id="collection-1",
                include_burned=True,
                limit=10,
            )
        request = send.call_args.args[0]
        self.assertEqual(request["method"], "issuer_nfts")
        self.assertEqual(request["params"]["issuer"], "pf-issuer")
        self.assertEqual(request["params"]["collection_id"], "collection-1")
        self.assertEqual(request["params"]["include_burned"], True)
        self.assertEqual(request["params"]["limit"], 10)

        offer_id = "04" * 48
        result_schema["value"] = "postfiat-offer-info-v1"
        with mock.patch.object(client, "_send", side_effect=response_for_request) as send:
            client.offer_info(offer_id)
        request = send.call_args.args[0]
        self.assertEqual(request["method"], "offer_info")
        self.assertEqual(request["params"], {"offer_id": offer_id})

        result_schema["value"] = "postfiat-account-offers-v1"
        with mock.patch.object(client, "_send", side_effect=response_for_request) as send:
            client.account_offers("pf-owner", state="open", limit=11)
        request = send.call_args.args[0]
        self.assertEqual(request["method"], "account_offers")
        self.assertEqual(request["params"]["account"], "pf-owner")
        self.assertEqual(request["params"]["state"], "open")
        self.assertEqual(request["params"]["limit"], 11)

        result_schema["value"] = "postfiat-book-offers-v1"
        with mock.patch.object(client, "_send", side_effect=response_for_request) as send:
            client.book_offers("PFT", asset_id, limit=12)
        request = send.call_args.args[0]
        self.assertEqual(request["method"], "book_offers")
        self.assertEqual(request["params"]["taker_gets_asset_id"], "PFT")
        self.assertEqual(request["params"]["taker_pays_asset_id"], asset_id)
        self.assertEqual(request["params"]["limit"], 12)

    def test_asset_wallet_helpers_quote_sign_and_submit_operations(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=0,
            address="pf-issuer",
            public_key_hex="00",
            key_file=Path("wallet.key.json"),
            backup_file=Path("wallet.backup.json"),
            key_report={},
        )
        asset_id = _issued_asset_id("postfiat-local", "pf-issuer", "USD", 1)
        quote_calls: list[dict[str, object]] = []
        submitted: list[dict[str, object]] = []

        def quote_response(
            source: str,
            operation: dict[str, object] | str,
            *,
            sequence: int | None = None,
            request_id: str | None = None,
        ) -> dict[str, object]:
            self.assertIsInstance(operation, dict)
            operation_obj = dict(operation) if isinstance(operation, dict) else {}
            if operation_obj.get("operation") == "nft_transfer":
                operation_obj["issuer"] = "pf-owner"
                operation_obj["issuer_transfer_fee"] = 7
            quote_calls.append({"source": source, "operation": operation_obj, "sequence": sequence})
            return {
                "version": "postfiat-local-rpc-v1",
                "id": request_id or "asset-quote",
                "ok": True,
                "result": {
                    "schema": "postfiat-asset-fee-quote-v1",
                    "chain_id": "postfiat-local",
                    "genesis_hash": "01" * 48,
                    "protocol_version": 1,
                    "source": source,
                    "sequence": sequence or len(quote_calls),
                    "minimum_fee": 42,
                    "transaction_kind": operation_obj["operation"],
                    "operation": operation_obj,
                },
            }

        def submit_signed(
            signed_asset_transaction: dict[str, object] | str,
            *,
            request_id: str | None = None,
        ) -> dict[str, object]:
            self.assertIsInstance(signed_asset_transaction, dict)
            submitted.append(dict(signed_asset_transaction))
            return {"tx_id": "ab" * 48, "transaction": signed_asset_transaction}

        def fake_run(
            args: object,
            *,
            json_output: bool,
            cwd: Path = wallet_module.REPO_ROOT,
        ) -> None:
            del cwd
            command = list(args)  # type: ignore[arg-type]
            self.assertFalse(json_output)
            self.assertIn("wallet-sign-asset-transaction", command)
            quote_path = Path(command[command.index("--quote-response") + 1])
            output_path = Path(command[command.index("--output") + 1])
            quote = json.loads(quote_path.read_text(encoding="utf-8"))
            signed = {
                "unsigned": {
                    "transaction_kind": quote["result"]["transaction_kind"],
                    "operation": quote["result"]["operation"],
                }
            }
            output_path.write_text(json.dumps(signed), encoding="utf-8")
            return None

        with tempfile.TemporaryDirectory() as tmp:
            work_dir = Path(tmp)
            with (
                mock.patch.object(client, "asset_fee_quote_response", side_effect=quote_response),
                mock.patch.object(
                    client,
                    "mempool_submit_signed_asset_transaction",
                    side_effect=submit_signed,
                ),
                mock.patch.object(wallet_module, "_run", side_effect=fake_run),
            ):
                create_result = create_issued_asset(
                    client,
                    wallet=wallet,
                    code="USD",
                    precision=2,
                    work_dir=work_dir,
                )
                trust_result = create_asset_trustline(
                    client,
                    wallet=wallet,
                    issuer="pf-bank",
                    asset_id=asset_id,
                    limit=100,
                    work_dir=work_dir,
                    sequence=2,
                )
                payment_result = send_issued_asset(
                    client,
                    wallet=wallet,
                    to_address="pf-holder",
                    issuer="pf-issuer",
                    asset_id=asset_id,
                    amount=40,
                    work_dir=work_dir,
                )
                clawback_result = clawback_issued_asset(
                    client,
                    wallet=wallet,
                    owner="pf-holder",
                    asset_id=asset_id,
                    amount=10,
                    work_dir=work_dir,
                )

        self.assertEqual(create_result.asset_id, asset_id)
        self.assertEqual(trust_result.asset_id, asset_id)
        self.assertEqual(payment_result.asset_id, asset_id)
        self.assertEqual(clawback_result.asset_id, asset_id)
        self.assertEqual([call["source"] for call in quote_calls], ["pf-issuer"] * 4)
        self.assertEqual(
            [call["operation"]["operation"] for call in quote_calls],  # type: ignore[index]
            ["asset_create", "trust_set", "issued_payment", "asset_clawback"],
        )
        self.assertEqual(quote_calls[1]["sequence"], 2)
        self.assertEqual(len(submitted), 4)
        self.assertEqual(
            submitted[0]["unsigned"]["transaction_kind"],  # type: ignore[index]
            "asset_create",
        )
        self.assertEqual(
            submitted[3]["unsigned"]["transaction_kind"],  # type: ignore[index]
            "asset_clawback",
        )

    def test_asset_issuer_control_helpers_preserve_line_terms(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=0,
            address="pf-issuer",
            public_key_hex="00",
            key_file=Path("wallet.key.json"),
            backup_file=Path("wallet.backup.json"),
            key_report={},
        )
        asset_id = _issued_asset_id("postfiat-local", "pf-issuer", "USD", 1)
        asset_id_value = asset_id
        line_state: dict[str, object] = {
            "authorized": False,
            "frozen": False,
            "limit": 100,
            "reserve_paid": 10,
        }
        quote_calls: list[dict[str, object]] = []

        def account_lines(
            account: str,
            *,
            issuer: str | None = None,
            asset_id: str | None = None,
            limit: int | None = None,
        ) -> dict[str, object]:
            self.assertEqual(account, "pf-holder")
            self.assertEqual(issuer, "pf-issuer")
            self.assertEqual(asset_id, asset_id_value)
            self.assertEqual(limit, 2)
            return {
                "lines": [
                    {
                        "account": account,
                        "issuer": issuer,
                        "asset_id": asset_id,
                        "limit": line_state["limit"],
                        "authorized": line_state["authorized"],
                        "frozen": line_state["frozen"],
                        "reserve_paid": line_state["reserve_paid"],
                    }
                ]
            }

        def quote_response(
            source: str,
            operation: dict[str, object] | str,
            *,
            sequence: int | None = None,
            request_id: str | None = None,
        ) -> dict[str, object]:
            del sequence
            self.assertEqual(source, "pf-issuer")
            self.assertIsInstance(operation, dict)
            operation_obj = dict(operation) if isinstance(operation, dict) else {}
            quote_calls.append(operation_obj)
            line_state["authorized"] = operation_obj["authorized"]
            line_state["frozen"] = operation_obj["frozen"]
            return {
                "version": "postfiat-local-rpc-v1",
                "id": request_id or "asset-quote",
                "ok": True,
                "result": {
                    "schema": "postfiat-asset-fee-quote-v1",
                    "chain_id": "postfiat-local",
                    "genesis_hash": "01" * 48,
                    "protocol_version": 1,
                    "source": source,
                    "sequence": len(quote_calls),
                    "minimum_fee": 42,
                    "transaction_kind": operation_obj["operation"],
                    "operation": operation_obj,
                },
            }

        def submit_signed(
            signed_asset_transaction: dict[str, object] | str,
            *,
            request_id: str | None = None,
        ) -> dict[str, object]:
            del request_id
            self.assertIsInstance(signed_asset_transaction, dict)
            return {"tx_id": "ab" * 48, "transaction": signed_asset_transaction}

        def fake_run(
            args: object,
            *,
            json_output: bool,
            cwd: Path = wallet_module.REPO_ROOT,
        ) -> None:
            del cwd
            command = list(args)  # type: ignore[arg-type]
            self.assertFalse(json_output)
            quote_path = Path(command[command.index("--quote-response") + 1])
            output_path = Path(command[command.index("--output") + 1])
            quote = json.loads(quote_path.read_text(encoding="utf-8"))
            signed = {
                "unsigned": {
                    "transaction_kind": quote["result"]["transaction_kind"],
                    "operation": quote["result"]["operation"],
                }
            }
            output_path.write_text(json.dumps(signed), encoding="utf-8")

        with tempfile.TemporaryDirectory() as tmp:
            with (
                mock.patch.object(client, "account_lines", side_effect=account_lines),
                mock.patch.object(client, "asset_fee_quote_response", side_effect=quote_response),
                mock.patch.object(
                    client,
                    "mempool_submit_signed_asset_transaction",
                    side_effect=submit_signed,
                ),
                mock.patch.object(wallet_module, "_run", side_effect=fake_run),
            ):
                authorize_asset_trustline(
                    client,
                    wallet=wallet,
                    account="pf-holder",
                    asset_id=asset_id,
                    work_dir=tmp,
                )
                freeze_asset_trustline(
                    client,
                    wallet=wallet,
                    account="pf-holder",
                    asset_id=asset_id,
                    work_dir=tmp,
                )
                unfreeze_asset_trustline(
                    client,
                    wallet=wallet,
                    account="pf-holder",
                    asset_id=asset_id,
                    work_dir=tmp,
                )
                revoke_asset_trustline_authorization(
                    client,
                    wallet=wallet,
                    account="pf-holder",
                    asset_id=asset_id,
                    work_dir=tmp,
                )
                set_asset_trustline_control(
                    client,
                    wallet=wallet,
                    account="pf-holder",
                    asset_id=asset_id,
                    authorized=True,
                    frozen=True,
                    work_dir=tmp,
                )

        self.assertEqual(
            [(call["authorized"], call["frozen"]) for call in quote_calls],
            [(True, False), (True, True), (True, False), (False, False), (True, True)],
        )
        self.assertTrue(
            all(
                call["operation"] == "trust_set"
                and call["account"] == "pf-holder"
                and call["issuer"] == "pf-issuer"
                and call["asset_id"] == asset_id
                and call["limit"] == 100
                and call["reserve_paid"] == 10
                for call in quote_calls
            )
        )

    def test_escrow_wallet_helpers_quote_sign_and_submit_operations(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        owner_wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=0,
            address="pf-owner",
            public_key_hex="00",
            key_file=Path("owner.key.json"),
            backup_file=Path("owner.backup.json"),
            key_report={},
        )
        recipient_wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=1,
            address="pf-recipient",
            public_key_hex="11",
            key_file=Path("recipient.key.json"),
            backup_file=Path("recipient.backup.json"),
            key_report={},
        )
        quote_calls: list[dict[str, object]] = []
        submitted: list[dict[str, object]] = []
        escrow_id = _escrow_id("postfiat-local", "pf-owner", 9)
        issued_asset_id = "01" * 48
        issued_escrow_id = _escrow_id("postfiat-local", "pf-owner", 10)

        def quote_response(
            source: str,
            operation: dict[str, object] | str,
            *,
            sequence: int | None = None,
            request_id: str | None = None,
        ) -> dict[str, object]:
            self.assertIsInstance(operation, dict)
            operation_obj = dict(operation) if isinstance(operation, dict) else {}
            quote_calls.append({"source": source, "operation": operation_obj, "sequence": sequence})
            quoted_sequence = sequence or (9 + len(quote_calls) - 1)
            return {
                "version": "postfiat-local-rpc-v1",
                "id": request_id or "escrow-quote",
                "ok": True,
                "result": {
                    "schema": "postfiat-escrow-fee-quote-v1",
                    "chain_id": "postfiat-local",
                    "genesis_hash": "01" * 48,
                    "protocol_version": 1,
                    "source": source,
                    "sequence": quoted_sequence,
                    "minimum_fee": 42,
                    "transaction_kind": operation_obj["operation"],
                    "operation": operation_obj,
                },
            }

        def submit_signed(
            signed_escrow_transaction: dict[str, object] | str,
            *,
            request_id: str | None = None,
        ) -> dict[str, object]:
            self.assertIsInstance(signed_escrow_transaction, dict)
            submitted.append(dict(signed_escrow_transaction))
            return {"tx_id": "cd" * 48, "transaction": signed_escrow_transaction}

        def fake_run(
            args: object,
            *,
            json_output: bool,
            cwd: Path = wallet_module.REPO_ROOT,
        ) -> None:
            del cwd
            command = list(args)  # type: ignore[arg-type]
            self.assertFalse(json_output)
            self.assertIn("wallet-sign-escrow-transaction", command)
            quote_path = Path(command[command.index("--quote-response") + 1])
            output_path = Path(command[command.index("--output") + 1])
            quote = json.loads(quote_path.read_text(encoding="utf-8"))
            signed = {
                "unsigned": {
                    "transaction_kind": quote["result"]["transaction_kind"],
                    "operation": quote["result"]["operation"],
                }
            }
            output_path.write_text(json.dumps(signed), encoding="utf-8")
            return None

        with tempfile.TemporaryDirectory() as tmp:
            work_dir = Path(tmp)
            with (
                mock.patch.object(client, "escrow_fee_quote_response", side_effect=quote_response),
                mock.patch.object(
                    client,
                    "mempool_submit_signed_escrow_transaction",
                    side_effect=submit_signed,
                ),
                mock.patch.object(wallet_module, "_run", side_effect=fake_run),
            ):
                create_result = create_pft_escrow(
                    client,
                    wallet=owner_wallet,
                    recipient=recipient_wallet.address,
                    amount=50,
                    condition="hashlock",
                    work_dir=work_dir,
                )
                issued_create_result = create_issued_asset_escrow(
                    client,
                    wallet=owner_wallet,
                    recipient=recipient_wallet.address,
                    asset_id=issued_asset_id,
                    amount=60,
                    condition="issued-hashlock",
                    work_dir=work_dir,
                )
                finish_result = finish_pft_escrow(
                    client,
                    wallet=recipient_wallet,
                    escrow_id=escrow_id,
                    owner=owner_wallet.address,
                    fulfillment="hashlock",
                    work_dir=work_dir,
                    sequence=3,
                )
                cancel_result = cancel_pft_escrow(
                    client,
                    wallet=owner_wallet,
                    escrow_id=escrow_id,
                    work_dir=work_dir,
                    sequence=4,
                )

        self.assertEqual(create_result.escrow_id, escrow_id)
        self.assertEqual(issued_create_result.escrow_id, issued_escrow_id)
        self.assertEqual(finish_result.escrow_id, escrow_id)
        self.assertEqual(cancel_result.escrow_id, escrow_id)
        self.assertEqual(
            [call["operation"]["operation"] for call in quote_calls],  # type: ignore[index]
            ["escrow_create", "escrow_create", "escrow_finish", "escrow_cancel"],
        )
        self.assertEqual(
            [call["source"] for call in quote_calls],
            ["pf-owner", "pf-owner", "pf-recipient", "pf-owner"],
        )
        self.assertEqual(quote_calls[2]["sequence"], 3)
        self.assertEqual(quote_calls[3]["sequence"], 4)
        self.assertEqual(len(submitted), 4)
        self.assertEqual(
            submitted[0]["unsigned"]["transaction_kind"],  # type: ignore[index]
            "escrow_create",
        )
        self.assertEqual(
            submitted[1]["unsigned"]["operation"]["asset_id"],  # type: ignore[index]
            issued_asset_id,
        )

    def test_submit_escrow_transaction_prefers_key_file_signer(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        quote_calls: list[dict[str, object]] = []
        submitted: list[dict[str, object]] = []
        run_json_commands: list[list[str]] = []

        def quote_response(
            source: str,
            operation: dict[str, object] | str,
            *,
            sequence: int | None = None,
            request_id: str | None = None,
        ) -> dict[str, object]:
            self.assertIsInstance(operation, dict)
            operation_obj = dict(operation) if isinstance(operation, dict) else {}
            quote_calls.append({"source": source, "operation": operation_obj, "sequence": sequence})
            return {
                "version": "postfiat-local-rpc-v1",
                "id": request_id or "escrow-quote",
                "ok": True,
                "result": {
                    "schema": "postfiat-escrow-fee-quote-v1",
                    "chain_id": "postfiat-local",
                    "genesis_hash": "01" * 48,
                    "protocol_version": 1,
                    "source": source,
                    "sequence": sequence or 7,
                    "minimum_fee": 42,
                    "transaction_kind": operation_obj["operation"],
                    "operation": operation_obj,
                },
            }

        def fake_run_json(
            args: object,
            *,
            cwd: Path = wallet_module.REPO_ROOT,
        ) -> dict[str, object]:
            del cwd
            command = list(args)  # type: ignore[arg-type]
            run_json_commands.append(command)
            self.assertIn("wallet-sign-escrow-transaction", command)
            self.assertIn("--key-file", command)
            self.assertIn("--quote-file", command)
            return {
                "unsigned": {
                    "transaction_kind": "escrow_create",
                    "operation": "escrow_create",
                    "source": "pf-owner",
                }
            }

        def submit_signed(
            signed_escrow_transaction: dict[str, object] | str,
            *,
            request_id: str | None = None,
        ) -> dict[str, object]:
            self.assertIsInstance(signed_escrow_transaction, dict)
            submitted.append(dict(signed_escrow_transaction))
            return {"tx_id": "cd" * 48, "transaction": signed_escrow_transaction}

        with tempfile.TemporaryDirectory() as tmp:
            work_dir = Path(tmp)
            key_file = work_dir / "owner.key.json"
            key_file.write_text("{}", encoding="utf-8")
            wallet = TransparentWallet(
                chain_id="postfiat-local",
                account_index=0,
                address="pf-owner",
                public_key_hex="00",
                key_file=key_file,
                backup_file=work_dir / "owner.backup.json",
                key_report={},
            )
            with (
                mock.patch.object(client, "escrow_fee_quote_response", side_effect=quote_response),
                mock.patch.object(
                    client,
                    "mempool_submit_signed_escrow_transaction",
                    side_effect=submit_signed,
                ),
                mock.patch.object(wallet_module, "_run_json", side_effect=fake_run_json),
            ):
                result = submit_escrow_transaction(
                    client,
                    wallet=wallet,
                    operation={
                        "operation": "escrow_create",
                        "owner": "pf-owner",
                        "recipient": "pf-recipient",
                        "asset_id": "PFT",
                        "amount": 1,
                        "condition": "secret",
                    },
                    work_dir=work_dir,
                )

        self.assertEqual(result.tx_id, "cd" * 48)
        self.assertEqual(len(run_json_commands), 1)
        self.assertEqual(len(submitted), 1)
        self.assertEqual(quote_calls[0]["source"], "pf-owner")

    def test_nft_wallet_helpers_quote_sign_and_submit_operations(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        owner_wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=0,
            address="pf-owner",
            public_key_hex="00",
            key_file=Path("owner.key.json"),
            backup_file=Path("owner.backup.json"),
            key_report={},
        )
        nft_id = _nft_id("postfiat-local", "pf-owner", "collection-1", 1)
        quote_calls: list[dict[str, object]] = []
        submitted: list[dict[str, object]] = []

        def quote_response(
            source: str,
            operation: dict[str, object] | str,
            *,
            sequence: int | None = None,
            request_id: str | None = None,
        ) -> dict[str, object]:
            self.assertIsInstance(operation, dict)
            operation_obj = dict(operation) if isinstance(operation, dict) else {}
            quote_calls.append({"source": source, "operation": operation_obj, "sequence": sequence})
            return {
                "version": "postfiat-local-rpc-v1",
                "id": request_id or "nft-quote",
                "ok": True,
                "result": {
                    "schema": "postfiat-nft-fee-quote-v1",
                    "transaction_kind": operation_obj["operation"],
                    "chain_id": "postfiat-local",
                    "genesis_hash": "01" * 48,
                    "protocol_version": 1,
                    "source": source,
                    "sequence": sequence or len(quote_calls),
                    "sequence_source": "explicit" if sequence else "ledger_mempool",
                    "sender_balance": 100,
                    "sender_sequence": 0,
                    "mempool_pending_for_sender": 0,
                    "base_nft_fee": 22,
                    "state_expansion_fee": 10,
                    "minimum_fee": 32,
                    "account_reserve": 10,
                    "transfer_fee_byte_quantum": 512,
                    "transfer_fee_per_quantum": 1,
                    "nft_weight_bytes": 11000,
                    "sender_balance_after_fee": 68,
                    "sender_meets_reserve_after_fee": True,
                    "issuer_transfer_fee": (
                        7 if operation_obj["operation"] == "nft_transfer" else 0
                    ),
                    "issuer_transfer_fee_recipient": (
                        "pf-owner" if operation_obj["operation"] == "nft_transfer" else None
                    ),
                    "sender_balance_after_fee_and_issuer_transfer_fee": (
                        61 if operation_obj["operation"] == "nft_transfer" else 68
                    ),
                    "sender_meets_reserve_after_fee_and_issuer_transfer_fee": True,
                    "operation": operation_obj,
                },
            }

        def submit_signed(
            signed_nft_transaction: dict[str, object] | str,
            *,
            request_id: str | None = None,
        ) -> dict[str, object]:
            self.assertIsInstance(signed_nft_transaction, dict)
            submitted.append(dict(signed_nft_transaction))
            return {"tx_id": "cd" * 48, "transaction": signed_nft_transaction}

        def fake_run(
            args: object,
            *,
            json_output: bool,
            cwd: Path = wallet_module.REPO_ROOT,
        ) -> None:
            del cwd
            command = list(args)  # type: ignore[arg-type]
            self.assertFalse(json_output)
            self.assertIn("wallet-sign-nft-transaction", command)
            quote_path = Path(command[command.index("--quote-response") + 1])
            output_path = Path(command[command.index("--output") + 1])
            quote = json.loads(quote_path.read_text(encoding="utf-8"))
            signed = {
                "unsigned": {
                    "transaction_kind": quote["result"]["transaction_kind"],
                    "operation": quote["result"]["operation"],
                }
            }
            output_path.write_text(json.dumps(signed), encoding="utf-8")
            return None

        with tempfile.TemporaryDirectory() as tmp:
            work_dir = Path(tmp)
            with (
                mock.patch.object(client, "nft_fee_quote_response", side_effect=quote_response),
                mock.patch.object(
                    client,
                    "mempool_submit_signed_nft_transaction",
                    side_effect=submit_signed,
                ),
                mock.patch.object(wallet_module, "_run", side_effect=fake_run),
            ):
                mint_result = mint_nft(
                    client,
                    wallet=owner_wallet,
                    collection_id="collection-1",
                    serial=1,
                    metadata_hash="ab" * 32,
                    metadata_uri="ipfs://postfiat-nft",
                    collection_flags=wallet_module.NFT_COLLECTION_FLAG_TRANSFER_LOCKED,
                    issuer_transfer_fee=7,
                    work_dir=work_dir,
                )
                transfer_result = transfer_nft(
                    client,
                    wallet=owner_wallet,
                    nft_id=nft_id,
                    to_address="pf-recipient",
                    work_dir=work_dir,
                    sequence=2,
                )
                burn_result = burn_nft(
                    client,
                    wallet=owner_wallet,
                    nft_id=nft_id,
                    work_dir=work_dir,
                    sequence=3,
                )

        self.assertEqual(mint_result.nft_id, nft_id)
        self.assertEqual(transfer_result.nft_id, nft_id)
        self.assertEqual(burn_result.nft_id, nft_id)
        self.assertEqual([call["source"] for call in quote_calls], ["pf-owner"] * 3)
        self.assertEqual(
            [call["operation"]["operation"] for call in quote_calls],  # type: ignore[index]
            ["nft_mint", "nft_transfer", "nft_burn"],
        )
        self.assertEqual(quote_calls[1]["sequence"], 2)
        self.assertEqual(quote_calls[2]["sequence"], 3)
        self.assertEqual(len(submitted), 3)
        self.assertEqual(
            submitted[0]["unsigned"]["transaction_kind"],  # type: ignore[index]
            "nft_mint",
        )
        self.assertEqual(
            submitted[0]["unsigned"]["operation"]["collection_flags"],  # type: ignore[index]
            wallet_module.NFT_COLLECTION_FLAG_TRANSFER_LOCKED,
        )
        self.assertEqual(
            submitted[1]["unsigned"]["operation"]["to"],  # type: ignore[index]
            "pf-recipient",
        )
        self.assertEqual(
            submitted[2]["unsigned"]["operation"]["owner"],  # type: ignore[index]
            "pf-owner",
        )

    def test_offer_wallet_helpers_quote_sign_and_submit_operations(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        owner_wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=0,
            address="pf-owner",
            public_key_hex="00",
            key_file=Path("owner.key.json"),
            backup_file=Path("owner.backup.json"),
            key_report={},
        )
        asset_id = "01" * 48
        offer_id = _offer_id("postfiat-local", "pf-owner", 1)
        quote_calls: list[dict[str, object]] = []
        submitted: list[dict[str, object]] = []

        def quote_response(
            source: str,
            operation: dict[str, object] | str,
            *,
            sequence: int | None = None,
            request_id: str | None = None,
        ) -> dict[str, object]:
            self.assertIsInstance(operation, dict)
            operation_obj = dict(operation) if isinstance(operation, dict) else {}
            quote_calls.append({"source": source, "operation": operation_obj, "sequence": sequence})
            return {
                "version": "postfiat-local-rpc-v1",
                "id": request_id or "offer-quote",
                "ok": True,
                "result": {
                    "schema": "postfiat-offer-fee-quote-v1",
                    "transaction_kind": operation_obj["operation"],
                    "chain_id": "postfiat-local",
                    "genesis_hash": "01" * 48,
                    "protocol_version": 1,
                    "source": source,
                    "sequence": sequence or len(quote_calls),
                    "sequence_source": "explicit" if sequence else "ledger_mempool",
                    "sender_balance": 100,
                    "sender_sequence": 0,
                    "mempool_pending_for_sender": 0,
                    "base_offer_fee": 22,
                    "match_fee": 0,
                    "state_expansion_fee": 10,
                    "estimated_cross_count": 0,
                    "max_dex_crosses_per_transaction": 64,
                    "will_create_residual_offer": True,
                    "offer_object_reserve": 10,
                    "minimum_fee": 32,
                    "account_reserve": 10,
                    "transfer_fee_byte_quantum": 512,
                    "transfer_fee_per_quantum": 1,
                    "offer_weight_bytes": 11000,
                    "sender_balance_after_fee": 68,
                    "sender_balance_after_fee_and_reserve": 58,
                    "sender_meets_reserve_after_fee": True,
                    "sender_meets_reserve_after_fee_and_reserve": True,
                    "operation": operation_obj,
                },
            }

        def submit_signed(
            signed_offer_transaction: dict[str, object] | str,
            *,
            request_id: str | None = None,
        ) -> dict[str, object]:
            self.assertIsInstance(signed_offer_transaction, dict)
            submitted.append(dict(signed_offer_transaction))
            return {"tx_id": "ef" * 48, "transaction": signed_offer_transaction}

        def fake_run(
            args: object,
            *,
            json_output: bool,
            cwd: Path = wallet_module.REPO_ROOT,
        ) -> None:
            del cwd
            command = list(args)  # type: ignore[arg-type]
            self.assertFalse(json_output)
            self.assertIn("wallet-sign-offer-transaction", command)
            quote_path = Path(command[command.index("--quote-response") + 1])
            output_path = Path(command[command.index("--output") + 1])
            quote = json.loads(quote_path.read_text(encoding="utf-8"))
            signed = {
                "unsigned": {
                    "transaction_kind": quote["result"]["transaction_kind"],
                    "operation": quote["result"]["operation"],
                }
            }
            output_path.write_text(json.dumps(signed), encoding="utf-8")
            return None

        with tempfile.TemporaryDirectory() as tmp:
            work_dir = Path(tmp)
            with (
                mock.patch.object(client, "offer_fee_quote_response", side_effect=quote_response),
                mock.patch.object(
                    client,
                    "mempool_submit_signed_offer_transaction",
                    side_effect=submit_signed,
                ),
                mock.patch.object(wallet_module, "_run", side_effect=fake_run),
            ):
                create_result = create_offer(
                    client,
                    wallet=owner_wallet,
                    taker_gets_asset_id="PFT",
                    taker_gets_amount=25,
                    taker_pays_asset_id=asset_id,
                    taker_pays_amount=10,
                    expiration_height=50,
                    work_dir=work_dir,
                )
                cancel_result = cancel_offer(
                    client,
                    wallet=owner_wallet,
                    offer_id=offer_id,
                    work_dir=work_dir,
                    sequence=2,
                )

        self.assertEqual(create_result.offer_id, offer_id)
        self.assertEqual(cancel_result.offer_id, offer_id)
        self.assertEqual([call["source"] for call in quote_calls], ["pf-owner"] * 2)
        self.assertEqual(
            [call["operation"]["operation"] for call in quote_calls],  # type: ignore[index]
            ["offer_create", "offer_cancel"],
        )
        self.assertEqual(quote_calls[1]["sequence"], 2)
        self.assertEqual(len(submitted), 2)
        self.assertEqual(
            submitted[0]["unsigned"]["transaction_kind"],  # type: ignore[index]
            "offer_create",
        )
        self.assertEqual(
            submitted[1]["unsigned"]["operation"]["offer_id"],  # type: ignore[index]
            offer_id,
        )

    def test_atomic_settlement_wallet_helper_builds_template(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        left_wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=0,
            address="pf-left",
            public_key_hex="00",
            key_file=Path("left.key.json"),
            backup_file=Path("left.backup.json"),
            key_report={},
        )
        right_wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=1,
            address="pf-right",
            public_key_hex="11",
            key_file=Path("right.key.json"),
            backup_file=Path("right.backup.json"),
            key_report={},
        )
        asset_id = "02" * 48
        template = {
            "settlement_id": "03" * 48,
            "left": {
                "escrow_id": "04" * 48,
                "operation": {"operation": "escrow_create", "asset_id": "PFT"},
            },
            "right": {
                "escrow_id": "05" * 48,
                "operation": {"operation": "escrow_create", "asset_id": asset_id},
            },
        }
        with mock.patch.object(
            client,
            "atomic_settlement_template",
            return_value=template,
        ) as call:
            result = build_atomic_settlement_template(
                client,
                left_wallet=left_wallet,
                right_wallet=right_wallet,
                left_asset_id="PFT",
                left_amount=100,
                right_asset_id=asset_id,
                right_amount=25,
                condition="shared-secret",
                finish_after=7,
                cancel_after=12,
                left_sequence=2,
            )
        self.assertEqual(result.settlement_id, "03" * 48)
        self.assertEqual(result.left_escrow_id, "04" * 48)
        self.assertEqual(result.right_escrow_id, "05" * 48)
        self.assertEqual(result.right_operation["asset_id"], asset_id)
        kwargs = call.call_args.kwargs
        self.assertEqual(kwargs["left_owner"], "pf-left")
        self.assertEqual(kwargs["left_recipient"], "pf-right")
        self.assertEqual(kwargs["right_owner"], "pf-right")
        self.assertEqual(kwargs["right_recipient"], "pf-left")
        self.assertEqual(kwargs["cancel_after"], 12)
        self.assertEqual(kwargs["left_sequence"], 2)
        self.assertIsNone(kwargs["right_sequence"])

    def test_atomic_settlement_execution_signs_each_leg_and_finishes_after_creates(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        left_wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=0,
            address="pf-left",
            public_key_hex="00",
            key_file=Path("left.key.json"),
            backup_file=Path("left.backup.json"),
            key_report={},
        )
        right_wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=1,
            address="pf-right",
            public_key_hex="11",
            key_file=Path("right.key.json"),
            backup_file=Path("right.backup.json"),
            key_report={},
        )
        asset_id = "02" * 48
        left_escrow_id = "04" * 48
        right_escrow_id = "05" * 48
        template = {
            "settlement_id": "03" * 48,
            "left": {
                "owner": left_wallet.address,
                "recipient": right_wallet.address,
                "asset_id": "PFT",
                "amount": 100,
                "sequence": 7,
                "escrow_id": left_escrow_id,
                "operation": {
                    "operation": "escrow_create",
                    "owner": left_wallet.address,
                    "recipient": right_wallet.address,
                    "asset_id": "PFT",
                    "amount": 100,
                    "condition": "shared-secret",
                    "finish_after": 0,
                    "cancel_after": 20,
                },
            },
            "right": {
                "owner": right_wallet.address,
                "recipient": left_wallet.address,
                "asset_id": asset_id,
                "amount": 25,
                "sequence": 11,
                "escrow_id": right_escrow_id,
                "operation": {
                    "operation": "escrow_create",
                    "owner": right_wallet.address,
                    "recipient": left_wallet.address,
                    "asset_id": asset_id,
                    "amount": 25,
                    "condition": "shared-secret",
                    "finish_after": 0,
                    "cancel_after": 20,
                },
            },
        }
        order: list[str] = []
        creates: list[dict[str, object]] = []
        finishes: list[dict[str, object]] = []
        waits: list[tuple[str, str]] = []

        def submit(
            _client: PostFiatRpcClient,
            *,
            wallet: TransparentWallet,
            operation: dict[str, object],
            work_dir: str | Path | None = None,
            sequence: int | None = None,
            submit_finality: bool = False,
            finalize_data_dir: str | Path | None = None,
            validator_data_dirs: object = None,
            escrow_id: str | None = None,
        ) -> wallet_module.EscrowTransactionResult:
            del work_dir, submit_finality, finalize_data_dir, validator_data_dirs
            order.append(f"create:{wallet.address}")
            creates.append({
                "wallet": wallet.address,
                "operation": dict(operation),
                "sequence": sequence,
                "escrow_id": escrow_id,
            })
            return wallet_module.EscrowTransactionResult(
                tx_id=f"tx-{len(creates)}",
                operation=dict(operation),
                quote_response={},
                signed_escrow_transaction={"signed_by": wallet.address},
                submit_result={"ok": True, "tx_id": f"tx-{len(creates)}"},
                finalized_batch_file=None,
                receipts_by_validator=(),
                escrow_id=escrow_id,
            )

        def finish(
            _client: PostFiatRpcClient,
            *,
            recipient_wallet: TransparentWallet,
            escrow_id: str,
            owner: str,
            fulfillment: str = "",
            work_dir: str | Path | None = None,
            sequence: int | None = None,
            submit_finality: bool = False,
            finalize_data_dir: str | Path | None = None,
            validator_data_dirs: object = None,
        ) -> wallet_module.EscrowTransactionResult:
            del work_dir, submit_finality, finalize_data_dir, validator_data_dirs
            order.append(f"finish:{escrow_id}")
            finishes.append({
                "recipient": recipient_wallet.address,
                "escrow_id": escrow_id,
                "owner": owner,
                "fulfillment": fulfillment,
                "sequence": sequence,
            })
            return wallet_module.EscrowTransactionResult(
                tx_id=f"finish-{len(finishes)}",
                operation={
                    "operation": "escrow_finish",
                    "escrow_id": escrow_id,
                    "owner": owner,
                    "recipient": recipient_wallet.address,
                    "fulfillment": fulfillment,
                },
                quote_response={},
                signed_escrow_transaction={"signed_by": recipient_wallet.address},
                submit_result={"ok": True},
                finalized_batch_file=None,
                receipts_by_validator=(),
                escrow_id=escrow_id,
            )

        def wait_open(
            _client: PostFiatRpcClient,
            escrow_id: str,
            label: str,
            *,
            timeout_seconds: float = 45.0,
            poll_seconds: float = 1.0,
        ) -> dict[str, object]:
            order.append(f"wait:{escrow_id}")
            waits.append((label, escrow_id))
            return {
                "schema": "postfiat-escrow-info-v1",
                "found": True,
                "escrow_id": escrow_id,
                "escrow": {"state": "open"},
            }

        with (
            mock.patch.object(client, "atomic_settlement_template", return_value=template),
            mock.patch.object(wallet_module, "submit_escrow_transaction", side_effect=submit),
            mock.patch.object(wallet_module, "_wait_for_open_escrow", side_effect=wait_open),
            mock.patch.object(wallet_module, "finish_escrow", side_effect=finish),
        ):
            result = execute_atomic_settlement(
                client,
                left_wallet=left_wallet,
                right_wallet=right_wallet,
                left_asset_id="PFT",
                left_amount=100,
                right_asset_id=asset_id,
                right_amount=25,
                condition="shared-secret",
                cancel_after=20,
                left_finish_sequence=13,
                right_finish_sequence=17,
            )

        self.assertEqual(result.settlement_id, "03" * 48)
        self.assertEqual(result.left_create.escrow_id, left_escrow_id)
        self.assertEqual(result.right_create.escrow_id, right_escrow_id)
        self.assertEqual(
            order,
            [
                "create:pf-left",
                "create:pf-right",
                f"wait:{left_escrow_id}",
                f"wait:{right_escrow_id}",
                f"finish:{left_escrow_id}",
                f"finish:{right_escrow_id}",
            ],
        )
        self.assertEqual(waits, [("left escrow_create", left_escrow_id), ("right escrow_create", right_escrow_id)])
        self.assertEqual(result.left_create_escrow_info["escrow"]["state"], "open")  # type: ignore[index]
        self.assertEqual(result.right_create_escrow_info["escrow"]["state"], "open")  # type: ignore[index]
        self.assertEqual([call["wallet"] for call in creates], ["pf-left", "pf-right"])
        self.assertEqual([call["sequence"] for call in creates], [7, 11])
        self.assertEqual(creates[0]["operation"], template["left"]["operation"])
        self.assertEqual(creates[1]["operation"], template["right"]["operation"])
        self.assertEqual(finishes[0]["recipient"], "pf-right")
        self.assertEqual(finishes[0]["owner"], "pf-left")
        self.assertEqual(finishes[0]["fulfillment"], "shared-secret")
        self.assertEqual(finishes[0]["sequence"], 13)
        self.assertEqual(finishes[1]["recipient"], "pf-left")
        self.assertEqual(finishes[1]["owner"], "pf-right")
        self.assertEqual(finishes[1]["fulfillment"], "shared-secret")
        self.assertEqual(finishes[1]["sequence"], 17)

    def test_atomic_settlement_execution_does_not_reveal_fulfillment_after_rejected_create(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        left_wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=0,
            address="pf-left",
            public_key_hex="00",
            key_file=Path("left.key.json"),
            backup_file=Path("left.backup.json"),
            key_report={},
        )
        right_wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=1,
            address="pf-right",
            public_key_hex="11",
            key_file=Path("right.key.json"),
            backup_file=Path("right.backup.json"),
            key_report={},
        )
        asset_id = "02" * 48
        template = {
            "settlement_id": "03" * 48,
            "left": {
                "sequence": 7,
                "escrow_id": "04" * 48,
                "operation": {"operation": "escrow_create", "asset_id": "PFT"},
            },
            "right": {
                "sequence": 11,
                "escrow_id": "05" * 48,
                "operation": {"operation": "escrow_create", "asset_id": asset_id},
            },
        }

        def submit(
            _client: PostFiatRpcClient,
            *,
            wallet: TransparentWallet,
            operation: dict[str, object],
            escrow_id: str | None = None,
            **_kwargs: object,
        ) -> wallet_module.EscrowTransactionResult:
            ok = wallet.address == "pf-left"
            return wallet_module.EscrowTransactionResult(
                tx_id=None,
                operation=dict(operation),
                quote_response={},
                signed_escrow_transaction={"signed_by": wallet.address},
                submit_result={
                    "ok": ok,
                    "error": None if ok else {"code": "rejected", "message": "right create rejected"},
                },
                finalized_batch_file=None,
                receipts_by_validator=(),
                escrow_id=escrow_id,
            )

        with (
            mock.patch.object(client, "atomic_settlement_template", return_value=template),
            mock.patch.object(wallet_module, "submit_escrow_transaction", side_effect=submit),
            mock.patch.object(wallet_module, "finish_escrow") as finish_call,
        ):
            with self.assertRaisesRegex(ValueError, "right create rejected"):
                execute_atomic_settlement(
                    client,
                    left_wallet=left_wallet,
                    right_wallet=right_wallet,
                    left_asset_id="PFT",
                    left_amount=100,
                    right_asset_id=asset_id,
                    right_amount=25,
                    condition="shared-secret",
                    cancel_after=20,
                )
        finish_call.assert_not_called()

    def test_atomic_settlement_execution_does_not_reveal_fulfillment_before_creates_are_open(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        left_wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=0,
            address="pf-left",
            public_key_hex="00",
            key_file=Path("left.key.json"),
            backup_file=Path("left.backup.json"),
            key_report={},
        )
        right_wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=1,
            address="pf-right",
            public_key_hex="11",
            key_file=Path("right.key.json"),
            backup_file=Path("right.backup.json"),
            key_report={},
        )
        template = {
            "settlement_id": "03" * 48,
            "left": {
                "sequence": 7,
                "escrow_id": "04" * 48,
                "operation": {"operation": "escrow_create", "asset_id": "PFT"},
            },
            "right": {
                "sequence": 11,
                "escrow_id": "05" * 48,
                "operation": {"operation": "escrow_create", "asset_id": "02" * 48},
            },
        }

        def submit(
            _client: PostFiatRpcClient,
            *,
            wallet: TransparentWallet,
            operation: dict[str, object],
            escrow_id: str | None = None,
            **_kwargs: object,
        ) -> wallet_module.EscrowTransactionResult:
            return wallet_module.EscrowTransactionResult(
                tx_id=f"tx-{wallet.address}",
                operation=dict(operation),
                quote_response={},
                signed_escrow_transaction={"signed_by": wallet.address},
                submit_result={"ok": True, "tx_id": f"tx-{wallet.address}"},
                finalized_batch_file=None,
                receipts_by_validator=(),
                escrow_id=escrow_id,
            )

        with (
            mock.patch.object(client, "atomic_settlement_template", return_value=template),
            mock.patch.object(wallet_module, "submit_escrow_transaction", side_effect=submit),
            mock.patch.object(wallet_module, "_wait_for_open_escrow", side_effect=TimeoutError("right create not open")),
            mock.patch.object(wallet_module, "finish_escrow") as finish_call,
        ):
            with self.assertRaisesRegex(TimeoutError, "right create not open"):
                execute_atomic_settlement(
                    client,
                    left_wallet=left_wallet,
                    right_wallet=right_wallet,
                    left_asset_id="PFT",
                    left_amount=100,
                    right_asset_id="02" * 48,
                    right_amount=25,
                    condition="shared-secret",
                    cancel_after=20,
                )
        finish_call.assert_not_called()

    def test_xrpl_style_payment_and_token_helpers_delegate_to_canonical_helpers(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        issuer_wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=0,
            address="pf-issuer",
            public_key_hex="00",
            key_file=Path("issuer.key.json"),
            backup_file=Path("issuer.backup.json"),
            key_report={},
        )
        holder_wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=1,
            address="pf-holder",
            public_key_hex="11",
            key_file=Path("holder.key.json"),
            backup_file=Path("holder.backup.json"),
            key_report={},
        )
        asset_id = "01" * 48
        payment_result = object()
        mint_result = object()
        trustline_result = object()
        token_send_result = object()
        clawback_result = object()

        with mock.patch.object(wallet_module, "send_pft", return_value=payment_result) as call:
            self.assertIs(
                send_payment(
                    client,
                    wallet=holder_wallet,
                    destination="pf-destination",
                    amount=25,
                    memo_type="invoice",
                    memo_format="text/plain",
                    memo_data="INV-1",
                    sequence=2,
                ),
                payment_result,
            )
        kwargs = call.call_args.kwargs
        self.assertEqual(kwargs["wallet"], holder_wallet)
        self.assertEqual(kwargs["to_address"], "pf-destination")
        self.assertEqual(kwargs["amount"], 25)
        self.assertEqual(kwargs["memo_type"], "invoice")
        self.assertEqual(kwargs["memo_format"], "text/plain")
        self.assertEqual(kwargs["memo_data"], "INV-1")
        self.assertEqual(kwargs["sequence"], 2)

        with mock.patch.object(
            wallet_module,
            "create_issued_asset",
            return_value=mint_result,
        ) as call:
            self.assertIs(
                mint_token(
                    client,
                    issuer_wallet=issuer_wallet,
                    currency="USD",
                    precision=2,
                    display_name="US Dollar",
                    max_supply=1_000_000,
                    requires_authorization=True,
                    freeze_enabled=True,
                    clawback_enabled=True,
                ),
                mint_result,
            )
        kwargs = call.call_args.kwargs
        self.assertEqual(kwargs["wallet"], issuer_wallet)
        self.assertEqual(kwargs["code"], "USD")
        self.assertEqual(kwargs["precision"], 2)
        self.assertEqual(kwargs["display_name"], "US Dollar")
        self.assertEqual(kwargs["max_supply"], 1_000_000)
        self.assertTrue(kwargs["requires_authorization"])
        self.assertTrue(kwargs["freeze_enabled"])
        self.assertTrue(kwargs["clawback_enabled"])

        with mock.patch.object(
            wallet_module,
            "create_asset_trustline",
            return_value=trustline_result,
        ) as call:
            self.assertIs(
                set_trustline(
                    client,
                    holder_wallet=holder_wallet,
                    issuer=issuer_wallet.address,
                    asset_id=asset_id,
                    limit=500,
                    reserve_paid=12,
                    sequence=3,
                ),
                trustline_result,
            )
        kwargs = call.call_args.kwargs
        self.assertEqual(kwargs["wallet"], holder_wallet)
        self.assertEqual(kwargs["issuer"], issuer_wallet.address)
        self.assertEqual(kwargs["asset_id"], asset_id)
        self.assertEqual(kwargs["limit"], 500)
        self.assertEqual(kwargs["reserve_paid"], 12)
        self.assertEqual(kwargs["sequence"], 3)

        with mock.patch.object(wallet_module, "send_issued_asset", return_value=token_send_result) as call:
            self.assertIs(
                send_token(
                    client,
                    sender_wallet=issuer_wallet,
                    destination=holder_wallet.address,
                    issuer=issuer_wallet.address,
                    asset_id=asset_id,
                    value=75,
                ),
                token_send_result,
            )
        kwargs = call.call_args.kwargs
        self.assertEqual(kwargs["wallet"], issuer_wallet)
        self.assertEqual(kwargs["to_address"], holder_wallet.address)
        self.assertEqual(kwargs["issuer"], issuer_wallet.address)
        self.assertEqual(kwargs["asset_id"], asset_id)
        self.assertEqual(kwargs["amount"], 75)

        with mock.patch.object(
            wallet_module,
            "clawback_issued_asset",
            return_value=clawback_result,
        ) as call:
            self.assertIs(
                clawback_token(
                    client,
                    issuer_wallet=issuer_wallet,
                    owner=holder_wallet.address,
                    asset_id=asset_id,
                    value=20,
                ),
                clawback_result,
            )
        kwargs = call.call_args.kwargs
        self.assertEqual(kwargs["wallet"], issuer_wallet)
        self.assertEqual(kwargs["owner"], holder_wallet.address)
        self.assertEqual(kwargs["asset_id"], asset_id)
        self.assertEqual(kwargs["amount"], 20)

    def test_xrpl_style_trustline_control_helpers_use_issuer_wallet(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        issuer_wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=0,
            address="pf-issuer",
            public_key_hex="00",
            key_file=Path("issuer.key.json"),
            backup_file=Path("issuer.backup.json"),
            key_report={},
        )
        asset_id = "01" * 48

        for helper, canonical_name in [
            (authorize_trustline, "authorize_asset_trustline"),
            (revoke_trustline_authorization, "revoke_asset_trustline_authorization"),
            (freeze_trustline, "freeze_asset_trustline"),
            (unfreeze_trustline, "unfreeze_asset_trustline"),
        ]:
            expected = object()
            with mock.patch.object(wallet_module, canonical_name, return_value=expected) as call:
                self.assertIs(
                    helper(
                        client,
                        issuer_wallet=issuer_wallet,
                        account="pf-holder",
                        asset_id=asset_id,
                        sequence=4,
                    ),
                    expected,
                )
            kwargs = call.call_args.kwargs
            self.assertEqual(kwargs["wallet"], issuer_wallet)
            self.assertEqual(kwargs["account"], "pf-holder")
            self.assertEqual(kwargs["asset_id"], asset_id)
            self.assertEqual(kwargs["sequence"], 4)

    def test_xrpl_style_escrow_nft_offer_and_swap_helpers_delegate(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        owner_wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=0,
            address="pf-owner",
            public_key_hex="00",
            key_file=Path("owner.key.json"),
            backup_file=Path("owner.backup.json"),
            key_report={},
        )
        right_wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=1,
            address="pf-right",
            public_key_hex="11",
            key_file=Path("right.key.json"),
            backup_file=Path("right.backup.json"),
            key_report={},
        )
        asset_id = "01" * 48
        nft_id = "02" * 48
        escrow_id = "03" * 48

        with mock.patch.object(wallet_module, "create_pft_escrow", return_value="pft") as call:
            self.assertEqual(
                create_escrow(
                    client,
                    owner_wallet=owner_wallet,
                    destination=right_wallet.address,
                    amount=10,
                    condition="hashlock",
                ),
                "pft",
            )
        kwargs = call.call_args.kwargs
        self.assertEqual(kwargs["wallet"], owner_wallet)
        self.assertEqual(kwargs["recipient"], right_wallet.address)
        self.assertEqual(kwargs["amount"], 10)
        self.assertEqual(kwargs["condition"], "hashlock")

        with mock.patch.object(
            wallet_module,
            "create_issued_asset_escrow",
            return_value="issued",
        ) as call:
            self.assertEqual(
                create_escrow(
                    client,
                    owner_wallet=owner_wallet,
                    destination=right_wallet.address,
                    asset_id=asset_id,
                    amount=11,
                    condition="issued-hashlock",
                ),
                "issued",
            )
        kwargs = call.call_args.kwargs
        self.assertEqual(kwargs["asset_id"], asset_id)
        self.assertEqual(kwargs["amount"], 11)
        self.assertEqual(kwargs["recipient"], right_wallet.address)

        with mock.patch.object(wallet_module, "finish_pft_escrow", return_value="finish") as call:
            self.assertEqual(
                finish_escrow(
                    client,
                    recipient_wallet=right_wallet,
                    escrow_id=escrow_id,
                    owner=owner_wallet.address,
                    fulfillment="hashlock",
                ),
                "finish",
            )
        kwargs = call.call_args.kwargs
        self.assertEqual(kwargs["wallet"], right_wallet)
        self.assertEqual(kwargs["escrow_id"], escrow_id)
        self.assertEqual(kwargs["owner"], owner_wallet.address)
        self.assertEqual(kwargs["fulfillment"], "hashlock")

        with mock.patch.object(wallet_module, "cancel_pft_escrow", return_value="cancel") as call:
            self.assertEqual(
                cancel_escrow(client, owner_wallet=owner_wallet, escrow_id=escrow_id),
                "cancel",
            )
        self.assertEqual(call.call_args.kwargs["wallet"], owner_wallet)

        with mock.patch.object(wallet_module, "mint_nft", return_value="mint-nft") as call:
            self.assertEqual(
                mint_non_fungible_token(
                    client,
                    issuer_wallet=owner_wallet,
                    collection_id="collection-1",
                    serial=1,
                    metadata_hash="ab" * 32,
                    owner=right_wallet.address,
                    metadata_uri="ipfs://postfiat-nft",
                    issuer_transfer_fee=7,
                ),
                "mint-nft",
            )
        kwargs = call.call_args.kwargs
        self.assertEqual(kwargs["wallet"], owner_wallet)
        self.assertEqual(kwargs["owner"], right_wallet.address)
        self.assertEqual(kwargs["metadata_uri"], "ipfs://postfiat-nft")
        self.assertEqual(kwargs["issuer_transfer_fee"], 7)

        with mock.patch.object(wallet_module, "transfer_nft", return_value="transfer-nft") as call:
            self.assertEqual(
                transfer_non_fungible_token(
                    client,
                    owner_wallet=owner_wallet,
                    nft_id=nft_id,
                    destination=right_wallet.address,
                    sequence=5,
                ),
                "transfer-nft",
            )
        kwargs = call.call_args.kwargs
        self.assertEqual(kwargs["wallet"], owner_wallet)
        self.assertEqual(kwargs["to_address"], right_wallet.address)
        self.assertEqual(kwargs["sequence"], 5)

        with mock.patch.object(wallet_module, "burn_nft", return_value="burn-nft") as call:
            self.assertEqual(
                burn_non_fungible_token(client, owner_wallet=owner_wallet, nft_id=nft_id),
                "burn-nft",
            )
        self.assertEqual(call.call_args.kwargs["wallet"], owner_wallet)

        with mock.patch.object(wallet_module, "create_offer", return_value="offer") as call:
            self.assertEqual(
                place_offer(
                    client,
                    wallet=owner_wallet,
                    taker_gets_asset_id="PFT",
                    taker_gets_value=25,
                    taker_pays_asset_id=asset_id,
                    taker_pays_value=10,
                    expiration_height=50,
                ),
                "offer",
            )
        kwargs = call.call_args.kwargs
        self.assertEqual(kwargs["taker_gets_amount"], 25)
        self.assertEqual(kwargs["taker_pays_amount"], 10)
        self.assertEqual(kwargs["expiration_height"], 50)

        with mock.patch.object(
            wallet_module,
            "build_atomic_settlement_template",
            return_value="swap",
        ) as call:
            self.assertEqual(
                build_atomic_swap_template(
                    client,
                    left_wallet=owner_wallet,
                    right_wallet=right_wallet,
                    left_asset_id="PFT",
                    left_amount=100,
                    right_asset_id=asset_id,
                    right_amount=25,
                    condition="shared-secret",
                    finish_after=7,
                    cancel_after=12,
                    left_sequence=2,
                ),
                "swap",
            )
        kwargs = call.call_args.kwargs
        self.assertEqual(kwargs["left_wallet"], owner_wallet)
        self.assertEqual(kwargs["right_wallet"], right_wallet)
        self.assertEqual(kwargs["right_asset_id"], asset_id)
        self.assertEqual(kwargs["finish_after"], 7)
        self.assertEqual(kwargs["cancel_after"], 12)
        self.assertEqual(kwargs["left_sequence"], 2)

    def test_asset_wallet_helpers_validate_bounds(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=0,
            address="pf-issuer",
            public_key_hex="00",
            key_file=Path("wallet.key.json"),
            backup_file=Path("wallet.backup.json"),
            key_report={},
        )
        with self.assertRaises(ValueError):
            create_issued_asset(client, wallet=wallet, code="", precision=2)
        with self.assertRaises(ValueError):
            create_issued_asset(client, wallet=wallet, code="USD", precision=19)
        with self.assertRaises(ValueError):
            create_asset_trustline(client, wallet=wallet, issuer="pf-bank", asset_id="zz", limit=1)
        with self.assertRaises(ValueError):
            send_issued_asset(
                client,
                wallet=wallet,
                to_address="pf-holder",
                issuer="pf-issuer",
                asset_id="01" * 48,
                amount=0,
            )
        with self.assertRaises(ValueError):
            clawback_issued_asset(
                client,
                wallet=wallet,
                owner="pf-holder",
                asset_id="PFT",
                amount=1,
            )
        with self.assertRaises(ValueError):
            clawback_issued_asset(
                client,
                wallet=wallet,
                owner="pf-holder",
                asset_id="01" * 48,
                amount=0,
            )
        with self.assertRaises(ValueError):
            create_pft_escrow(
                client,
                wallet=wallet,
                recipient="pf-recipient",
                amount=1,
            )
        with self.assertRaises(ValueError):
            create_pft_escrow(
                client,
                wallet=wallet,
                recipient=wallet.address,
                amount=1,
                condition="hashlock",
            )
        with self.assertRaises(ValueError):
            create_issued_asset_escrow(
                client,
                wallet=wallet,
                recipient="pf-recipient",
                asset_id="PFT",
                amount=1,
                condition="hashlock",
            )
        with self.assertRaises(ValueError):
            finish_pft_escrow(
                client,
                wallet=wallet,
                escrow_id="zz",
                owner="pf-owner",
            )
        with self.assertRaises(ValueError):
            mint_nft(
                client,
                wallet=wallet,
                collection_id="",
                serial=1,
                metadata_hash="ab" * 32,
            )
        with self.assertRaises(ValueError):
            mint_nft(
                client,
                wallet=wallet,
                collection_id="collection-1",
                serial=0,
                metadata_hash="ab" * 32,
            )
        with self.assertRaises(ValueError):
            mint_nft(
                client,
                wallet=wallet,
                collection_id="collection-1",
                serial=1,
                metadata_hash="AB" * 32,
            )
        with self.assertRaises(ValueError):
            transfer_nft(
                client,
                wallet=wallet,
                nft_id="01" * 48,
                to_address=wallet.address,
            )
        with self.assertRaises(ValueError):
            burn_nft(
                client,
                wallet=wallet,
                nft_id="zz",
            )
        with self.assertRaises(ValueError):
            create_offer(
                client,
                wallet=wallet,
                taker_gets_asset_id="PFT",
                taker_gets_amount=1,
                taker_pays_asset_id="PFT",
                taker_pays_amount=1,
            )
        with self.assertRaises(ValueError):
            create_offer(
                client,
                wallet=wallet,
                taker_gets_asset_id="PFT",
                taker_gets_amount=0,
                taker_pays_asset_id="01" * 48,
                taker_pays_amount=1,
            )
        with self.assertRaises(ValueError):
            cancel_offer(client, wallet=wallet, offer_id="zz")

    def test_wallet_dataclasses_hold_paths(self) -> None:
        transparent = TransparentWallet(
            chain_id="chain",
            account_index=0,
            address="pfabc",
            public_key_hex="00",
            key_file=Path("wallet.key.json"),
            backup_file=Path("wallet.backup.json"),
            key_report={},
        )
        orchard = OrchardWallet(
            account_index=0,
            address_raw_hex="11",
            key_file=Path("orchard.key.json"),
            view_key_file=Path("orchard.view-key.json"),
            key_report={},
            view_key_report={},
        )
        self.assertEqual(transparent.address, "pfabc")
        self.assertEqual(orchard.address_raw_hex, "11")

    def test_send_pft_submit_only_mode_does_not_call_apply_batch(self) -> None:
        """WAN/testnet send mode must not use local apply-batch."""
        client = PostFiatRpcClient("127.0.0.1:1234")
        wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=0,
            address="pf-from",
            public_key_hex="00",
            key_file=Path("wallet.key.json"),
            backup_file=Path("wallet.backup.json"),
            key_report={},
        )
        quote_envelope = {
            "version": "postfiat-local-rpc-v1",
            "id": "quote-1",
            "ok": True,
            "result": {
                "schema": "postfiat-transfer-fee-quote-v1",
                "chain_id": "postfiat-local",
                "genesis_hash": "01" * 48,
                "protocol_version": 1,
                "source": "pf-from",
                "sequence": 1,
                "minimum_fee": 32,
            },
        }
        submit_result = {"tx_id": "ab" * 48}

        def fake_run(args, *, json_output, cwd=wallet_module.REPO_ROOT):
            del cwd
            command = list(args)
            self.assertIn("wallet-sign-quote", command)
            output_path = Path(command[command.index("--output") + 1])
            output_path.write_text(json.dumps({"unsigned": {}}), encoding="utf-8")

        with tempfile.TemporaryDirectory() as tmp:
            work_dir = Path(tmp)
            with (
                mock.patch.object(client, "transfer_fee_quote_response", return_value=quote_envelope),
                mock.patch.object(client, "mempool_submit_signed_transfer", return_value=submit_result),
                mock.patch.object(wallet_module, "_run", side_effect=fake_run),
                mock.patch.object(wallet_module, "_apply_batch") as fake_apply,
            ):
                result = send_pft(
                    client,
                    wallet=wallet,
                    to_address="pf-to",
                    amount=1_000_000,
                    work_dir=work_dir,
                )
        self.assertEqual(result.submit_mode, "submit_only")
        self.assertTrue(result.pending)
        self.assertFalse(result.finalized)
        fake_apply.assert_not_called()

    def test_send_pft_local_apply_mode_calls_apply_batch(self) -> None:
        """Local harness mode must call apply-batch only when finalize_data_dir is supplied."""
        client = PostFiatRpcClient("127.0.0.1:1234")
        wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=0,
            address="pf-from",
            public_key_hex="00",
            key_file=Path("wallet.key.json"),
            backup_file=Path("wallet.backup.json"),
            key_report={},
        )
        quote_envelope = {
            "version": "postfiat-local-rpc-v1",
            "id": "quote-1",
            "ok": True,
            "result": {
                "schema": "postfiat-transfer-fee-quote-v1",
                "chain_id": "postfiat-local",
                "genesis_hash": "01" * 48,
                "protocol_version": 1,
                "source": "pf-from",
                "sequence": 1,
                "minimum_fee": 32,
            },
        }
        submit_result = {"tx_id": "ab" * 48}

        def fake_run(args, *, json_output, cwd=wallet_module.REPO_ROOT):
            del cwd
            command = list(args)
            if "wallet-sign-quote" in command:
                output_path = Path(command[command.index("--output") + 1])
                output_path.write_text(json.dumps({"unsigned": {}}), encoding="utf-8")

        with tempfile.TemporaryDirectory() as tmp:
            work_dir = Path(tmp)
            data_dir = Path(tmp) / "validator0"
            data_dir.mkdir()
            with (
                mock.patch.object(client, "transfer_fee_quote_response", return_value=quote_envelope),
                mock.patch.object(client, "mempool_submit_signed_transfer", return_value=submit_result),
                mock.patch.object(wallet_module, "_run", side_effect=fake_run),
                mock.patch.object(wallet_module, "_apply_batch", return_value=[{"tx_id": "ab" * 48}]) as fake_apply,
                mock.patch.object(wallet_module, "_run_json", return_value={"batch_id": "cd" * 48}) as fake_run_json,
            ):
                result = send_pft(
                    client,
                    wallet=wallet,
                    to_address="pf-to",
                    amount=1_000_000,
                    work_dir=work_dir,
                    finalize_data_dir=data_dir,
                    validator_data_dirs=[data_dir],
                )
        self.assertEqual(result.submit_mode, "local_apply")
        self.assertTrue(result.finalized)
        self.assertFalse(result.pending)
        fake_apply.assert_called_once()

    def test_send_pft_and_poll_finality_does_not_use_apply_batch(self) -> None:
        """submit_and_poll mode must not call apply-batch at any point."""
        client = PostFiatRpcClient("127.0.0.1:1234")
        wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=0,
            address="pf-from",
            public_key_hex="00",
            key_file=Path("wallet.key.json"),
            backup_file=Path("wallet.backup.json"),
            key_report={},
        )
        quote_envelope = {
            "version": "postfiat-local-rpc-v1",
            "id": "quote-1",
            "ok": True,
            "result": {
                "schema": "postfiat-transfer-fee-quote-v1",
                "chain_id": "postfiat-local",
                "genesis_hash": "01" * 48,
                "protocol_version": 1,
                "source": "pf-from",
                "sequence": 1,
                "minimum_fee": 32,
            },
        }
        submit_result = {"tx_id": "ab" * 48}
        tx_finalized = {"tx_id": "ab" * 48, "block_height": 10, "certified": True}

        def fake_run(args, *, json_output, cwd=wallet_module.REPO_ROOT):
            del cwd
            command = list(args)
            self.assertIn("wallet-sign-quote", command)
            output_path = Path(command[command.index("--output") + 1])
            output_path.write_text(json.dumps({"unsigned": {}}), encoding="utf-8")

        with tempfile.TemporaryDirectory() as tmp:
            work_dir = Path(tmp)
            with (
                mock.patch.object(client, "transfer_fee_quote_response", return_value=quote_envelope),
                mock.patch.object(client, "mempool_submit_signed_transfer", return_value=submit_result),
                mock.patch.object(client, "tx", return_value=tx_finalized),
                mock.patch.object(client, "receipts", return_value=[]),
                mock.patch.object(wallet_module, "_run", side_effect=fake_run),
                mock.patch.object(wallet_module, "_apply_batch") as fake_apply,
            ):
                result = send_pft_and_poll_finality(
                    client,
                    wallet=wallet,
                    to_address="pf-to",
                    amount=1_000_000,
                    work_dir=work_dir,
                    poll_timeout_seconds=1.0,
                    poll_interval_seconds=0.1,
                    use_finality_submit=False,
                )
        self.assertEqual(result.submit_mode, "submit_and_poll")
        self.assertTrue(result.finalized)
        self.assertFalse(result.pending)
        self.assertFalse(result.finality_timeout)
        self.assertIsNotNone(result.finality_receipt)
        fake_apply.assert_not_called()

    def test_send_pft_and_poll_finality_with_finality_submit_does_not_use_apply_batch(self) -> None:
        """submit_and_poll with finality submit must not call apply-batch."""
        client = PostFiatRpcClient("127.0.0.1:1234")
        wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=0,
            address="pf-from",
            public_key_hex="00",
            key_file=Path("wallet.key.json"),
            backup_file=Path("wallet.backup.json"),
            key_report={},
        )
        quote_envelope = {
            "version": "postfiat-local-rpc-v1",
            "id": "quote-1",
            "ok": True,
            "result": {
                "schema": "postfiat-transfer-fee-quote-v1",
                "chain_id": "postfiat-local",
                "genesis_hash": "01" * 48,
                "protocol_version": 1,
                "source": "pf-from",
                "sequence": 1,
                "minimum_fee": 32,
            },
        }
        tx_id = "ab" * 48
        finality_submit_result = {
            "tx_id": tx_id,
            "finality": {
                "block": {
                    "header": {
                        "height": 10,
                        "certificate": {"quorum": 5},
                    },
                },
                "local_hot_finality": [
                    {"receipt": {"tx_id": tx_id, "accepted": True}},
                ],
            },
        }

        def fake_run(args, *, json_output, cwd=wallet_module.REPO_ROOT):
            del cwd
            command = list(args)
            self.assertIn("wallet-sign-quote", command)
            output_path = Path(command[command.index("--output") + 1])
            output_path.write_text(json.dumps({"unsigned": {}}), encoding="utf-8")

        with tempfile.TemporaryDirectory() as tmp:
            work_dir = Path(tmp)
            with (
                mock.patch.object(client, "transfer_fee_quote_response", return_value=quote_envelope),
                mock.patch.object(client, "mempool_submit_signed_transfer_finality", return_value=finality_submit_result),
                mock.patch.object(wallet_module, "_run", side_effect=fake_run),
                mock.patch.object(wallet_module, "_apply_batch") as fake_apply,
            ):
                result = send_pft_and_poll_finality(
                    client,
                    wallet=wallet,
                    to_address="pf-to",
                    amount=1_000_000,
                    work_dir=work_dir,
                    use_finality_submit=True,
                )
        self.assertEqual(result.submit_mode, "submit_and_poll")
        self.assertTrue(result.finalized)
        self.assertFalse(result.pending)
        self.assertFalse(result.finality_timeout)
        self.assertIsNotNone(result.finality_receipt)
        self.assertEqual(result.finality_receipt.get("accepted"), True)
        fake_apply.assert_not_called()

    def test_send_pft_result_has_send_mode_fields(self) -> None:
        """SendPftResult must expose submit_mode, pending, finalized, finality_receipt, finality_timeout."""
        from postfiat_rpc.wallet import SendPftResult
        result = SendPftResult(
            tx_id="ab" * 48,
            quote_response={},
            signed_transfer={},
            submit_result={},
            finalized_batch_file=None,
            receipts_by_validator=(),
        )
        self.assertEqual(result.submit_mode, "local_apply")
        self.assertFalse(result.pending)
        self.assertFalse(result.finalized)
        self.assertIsNone(result.finality_receipt)
        self.assertFalse(result.finality_timeout)

    def test_server_capabilities_reports_read_only_when_no_submit_flags(self) -> None:
        """server_capabilities must report read_only=True when rpc section lacks submit flags."""
        client = PostFiatRpcClient("127.0.0.1:1234")
        server_info = {"rpc": {"read_only": True, "mempool_submit_enabled": False}}
        status = {"block_height": 100, "mempool_pending": 0, "chain_id": "postfiat-wan-devnet",
                  "validator_count": 6, "last_run_unix": 0}
        with mock.patch.object(client, "_call", side_effect=[server_info, status]):
            caps = client.server_capabilities()
        self.assertTrue(caps["read_only"])
        self.assertFalse(caps["mempool_submit_enabled"])
        self.assertFalse(caps["mempool_submit_finality_enabled"])
        self.assertEqual(caps["block_height"], 100)
        self.assertEqual(caps["chain_id"], "postfiat-wan-devnet")
        self.assertEqual(caps["validator_count"], 6)

    def test_server_capabilities_reports_writable_when_submit_enabled(self) -> None:
        """server_capabilities must report read_only=False when mempool_submit_enabled is True."""
        client = PostFiatRpcClient("127.0.0.1:1234")
        server_info = {"rpc": {"read_only": False, "mempool_submit_enabled": True,
                                "mempool_submit_finality_enabled": False,
                                "max_mempool_submit_per_peer": 100,
                                "max_mempool_submit_total": 1000}}
        status = {"block_height": 200, "mempool_pending": 3, "chain_id": "postfiat-wan-devnet",
                  "validator_count": 6, "last_run_unix": 1700000000}
        with mock.patch.object(client, "_call", side_effect=[server_info, status]):
            caps = client.server_capabilities()
        self.assertFalse(caps["read_only"])
        self.assertTrue(caps["mempool_submit_enabled"])
        self.assertEqual(caps["block_height"], 200)

    def test_server_capabilities_defaults_to_read_only_on_missing_rpc_section(self) -> None:
        """server_capabilities must default to read_only=True if server_info has no rpc section."""
        client = PostFiatRpcClient("127.0.0.1:1234")
        server_info = {"some_other_field": "value"}
        status = {"block_height": 50}
        with mock.patch.object(client, "_call", side_effect=[server_info, status]):
            caps = client.server_capabilities()
        self.assertTrue(caps["read_only"])
        self.assertFalse(caps["mempool_submit_enabled"])
        self.assertEqual(caps["block_height"], 50)


class NavcoinBridgeClientMethodTests(unittest.TestCase):
    """Tests for public PFTL-to-Uniswap NAVCoin bridge read helpers."""

    def test_navcoin_bridge_public_reads_send_expected_methods_and_params(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        route_id = "pftl-a666-ethereum-wA666-usdc-v1"
        packet_hash = "aa" * 48
        responses = [
            {"schema": "postfiat-pftl-uniswap-routes-status-v1", "routes": []},
            {"schema": "postfiat-pftl-uniswap-packet-status-v1", "packet_hash": packet_hash},
            {"schema": "postfiat-pftl-uniswap-claims-status-v1", "exports": [], "returns": []},
            {"schema": "postfiat-pftl-uniswap-supply-status-v1", "route_id": route_id},
            {"schema": "postfiat-navcoin-bridge-receipt-replay-v1", "route_id": route_id},
        ]

        with mock.patch.object(client, "_call", side_effect=responses) as mock_call:
            self.assertEqual(client.navcoin_bridge_routes(), responses[0])
            self.assertEqual(client.navcoin_bridge_packet(route_id, packet_hash), responses[1])
            self.assertEqual(
                client.navcoin_bridge_claims(route_id, limit=25, include_terminal=True),
                responses[2],
            )
            self.assertEqual(client.navcoin_bridge_supply_status(route_id), responses[3])
            self.assertEqual(client.navcoin_bridge_receipt_replay(route_id), responses[4])

        self.assertEqual(
            mock_call.call_args_list,
            [
                mock.call("navcoin_bridge_routes"),
                mock.call(
                    "navcoin_bridge_packet",
                    {"route_id": route_id, "packet_hash": packet_hash},
                ),
                mock.call(
                    "navcoin_bridge_claims",
                    {"route_id": route_id, "limit": 25, "include_terminal": True},
                ),
                mock.call("navcoin_bridge_supply_status", {"route_id": route_id}),
                mock.call("navcoin_bridge_receipt_replay", {"route_id": route_id}),
            ],
        )

    def test_navcoin_bridge_public_reads_validate_required_inputs(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")

        with self.assertRaisesRegex(ValueError, "route_id is required"):
            client.navcoin_bridge_packet("", "aa" * 48)
        with self.assertRaisesRegex(ValueError, "packet_hash is required"):
            client.navcoin_bridge_packet("route", "")
        with self.assertRaisesRegex(ValueError, "route_id is required"):
            client.navcoin_bridge_claims("")
        with self.assertRaisesRegex(ValueError, "limit must be positive"):
            client.navcoin_bridge_claims("route", limit=0)
        with self.assertRaisesRegex(ValueError, "route_id is required"):
            client.navcoin_bridge_supply_status("")
        with self.assertRaisesRegex(ValueError, "route_id is required"):
            client.navcoin_bridge_receipt_replay("")


class FastPayClientMethodTests(unittest.TestCase):
    """Tests for the FastPay owned_objects / owned_sign / owned_apply client methods."""

    def test_owned_objects_sends_correct_params(self) -> None:
        """owned_objects must send owner_public_key_hex, asset, and limit params."""
        client = PostFiatRpcClient("127.0.0.1:1234")
        expected_result = {
            "owner_public_key_hex": "abcd1234",
            "objects": [],
            "total_value": 0,
        }
        with mock.patch.object(client, "_call", return_value=expected_result) as mock_call:
            result = client.owned_objects("abcd1234", asset="PFT", limit=256)
        self.assertEqual(result, expected_result)
        mock_call.assert_called_once_with(
            "owned_objects",
            {"owner_public_key_hex": "abcd1234", "asset": "PFT", "limit": 256},
        )

    def test_owned_objects_minimal_params(self) -> None:
        """owned_objects must work with just the public key (no asset/limit)."""
        client = PostFiatRpcClient("127.0.0.1:1234")
        expected_result = {"objects": [{"id": "obj1", "value": 100, "asset": "PFT"}], "total_value": 100}
        with mock.patch.object(client, "_call", return_value=expected_result) as mock_call:
            result = client.owned_objects("pk123")
        self.assertEqual(result, expected_result)
        mock_call.assert_called_once_with("owned_objects", {"owner_public_key_hex": "pk123"})

    def test_owned_sign_sends_order_and_validator_id(self) -> None:
        """owned_sign must send order_json and validator_id params."""
        client = PostFiatRpcClient("127.0.0.1:1234")
        order_json = '{"inputs":[],"outputs":[],"fee":0,"nonce":1,"memos":[]}'
        vote_result = {"validator_id": "validator-0", "signature_hex": "deadbeef"}
        with mock.patch.object(client, "_call", return_value=vote_result) as mock_call:
            result = client.owned_sign(order_json, "validator-0")
        self.assertEqual(result, vote_result)
        mock_call.assert_called_once_with(
            "owned_sign",
            {"order_json": order_json, "validator_id": "validator-0"},
        )

    def test_owned_apply_sends_cert_json(self) -> None:
        """owned_apply must send cert_json param."""
        client = PostFiatRpcClient("127.0.0.1:1234")
        cert_json = '{"order":{},"owner_pubkey_hex":"pk","owner_signature_hex":"sig","votes":[]}'
        apply_result = "certified owned-transfer applied"
        with mock.patch.object(client, "_call", return_value=apply_result) as mock_call:
            result = client.owned_apply(cert_json)
        self.assertEqual(result, apply_result)
        mock_call.assert_called_once_with("owned_apply", {"cert_json": cert_json})

    def test_recovery_safe_fastpay_v3_rpc_contract(self) -> None:
        """The WAN client must expose every recovery-safe v3 payment boundary."""
        client = PostFiatRpcClient("127.0.0.1:1234")
        order_json = '{"order":{"recovery":{"lock_id":"lock"}}}'
        cert_json = '{"order":{"recovery":{"lock_id":"lock"}},"votes":[]}'
        with mock.patch.object(client, "_call", return_value={"ok": True}) as call:
            client.owned_recovery_capabilities()
            client.owned_sign_v3(order_json, "validator-0")
            client.owned_apply_v3(cert_json)
            client.owned_unwrap_sign_v3(order_json, "validator-0")
            client.owned_unwrap_apply_v3(cert_json)

        self.assertEqual(
            call.call_args_list,
            [
                mock.call("owned_recovery_capabilities"),
                mock.call(
                    "owned_sign_v3",
                    {"order_json": order_json, "validator_id": "validator-0"},
                ),
                mock.call("owned_apply_v3", {"cert_json": cert_json}),
                mock.call(
                    "owned_unwrap_sign_v3",
                    {"order_json": order_json, "validator_id": "validator-0"},
                ),
                mock.call("owned_unwrap_apply_v3", {"cert_json": cert_json}),
            ],
        )

    def test_unsigned_direct_wrap_and_unwrap_are_not_exposed(self) -> None:
        """The Python client must not retain the removed arbitrary-debit RPCs."""
        client = PostFiatRpcClient("127.0.0.1:1234")
        self.assertFalse(hasattr(client, "wrap_owned"))
        self.assertFalse(hasattr(client, "unwrap_owned"))

    def test_signed_fastlane_primary_submit_sends_exact_transaction(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        transaction = {"operation": {"owned_deposit": {"signed": {}}}}
        with mock.patch.object(client, "_call", return_value={"tx_id": "deposit-1"}) as call:
            result = client.mempool_submit_fastlane_primary(transaction)
        self.assertEqual(result, {"tx_id": "deposit-1"})
        call.assert_called_once_with(
            "mempool_submit_fastlane_primary",
            {"fastlane_primary_json": json.dumps(transaction, separators=(",", ":"))},
            request_id=None,
        )

    def test_signed_fastlane_primary_finality_submit_sends_exact_transaction(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        transaction = {"operation": {"owned_deposit": {"signed": {}}}}
        with mock.patch.object(client, "_call", return_value={"tx_id": "deposit-1"}) as call:
            result = client.mempool_submit_fastlane_primary_finality(transaction)
        self.assertEqual(result, {"tx_id": "deposit-1"})
        call.assert_called_once_with(
            "mempool_submit_fastlane_primary_finality",
            {"fastlane_primary_json": json.dumps(transaction, separators=(",", ":"))},
            request_id=None,
        )


class FastPayFlowTests(unittest.TestCase):
    """Tests for the full FastPay owned-transfer flow: sign → collect votes → apply."""

    def _make_order_json(self, input_id="obj1", input_version=1, amount=90, fee=10, nonce=1):
        """Build a valid OwnedTransferOrder JSON."""
        return json.dumps({
            "inputs": [{"id": input_id, "version": input_version}],
            "outputs": [
                {"owner_pubkey_hex": "recipient_pk", "value": amount, "asset": "PFT"},
            ],
            "fee": fee,
            "nonce": nonce,
            "memos": [],
        })

    def _make_vote(self, validator_id, signature_hex):
        return {"validator_id": validator_id, "signature_hex": signature_hex}

    def _wallet(self) -> TransparentWallet:
        return TransparentWallet(
            chain_id="postfiat-local",
            account_index=0,
            address="pf-owner",
            public_key_hex="owner_pk",
            key_file=Path("wallet.key.json"),
            backup_file=Path("wallet.backup.json"),
            key_report={},
        )

    def _fastpay_caps(self) -> dict[str, object]:
        return {
            "fastpay_bridge_enabled": True,
            "fastpay_bridge_mode": "proxy_broadcast_devnet",
            "fastpay_owned_apply_broadcast_enabled": True,
        }

    def _fastpay_recovery_caps(self) -> dict[str, object]:
        return {
            "schema": "postfiat-fastpay-recovery-capabilities-v1",
            "domain": {
                "schema": "postfiat-owned-certificate-domain-v3",
                "chain_id": "postfiat-local",
                "genesis_hash": "aa" * 48,
                "protocol_version": 3,
                "registry_id": "bb" * 48,
            },
            "committee_epoch": 7,
            "current_height": 20,
            "validator_count": 6,
            "quorum": 5,
            "policy": {
                "schema": "postfiat-fastpay-recovery-policy-v1",
                "activation_height": 10,
                "max_validity_blocks": 8,
                "max_recovery_blocks": 12,
            },
        }

    def test_wrap_fastpay_uses_signed_consensus_deposit_and_exact_receipt(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        public_key_hex = "ab" * 1952
        wallet = TransparentWallet(
            chain_id="postfiat-local",
            account_index=0,
            address="pf-owner",
            public_key_hex=public_key_hex,
            key_file=Path("wallet.key.json"),
            backup_file=Path("wallet.backup.json"),
            key_report={},
        )
        capabilities = {
            **self._fastpay_caps(),
            # The hardened public proxy disables the generic mempool submit
            # surface while retaining this signed, consensus-ordered deposit.
            "mempool_submit_enabled": False,
            "chain_id": "postfiat-local",
            "genesis_hash": "cd" * 48,
            "protocol_version": 3,
            "block_height": 20,
        }

        def fake_run(args, *, json_output, cwd=wallet_module.REPO_ROOT):
            del cwd, json_output
            command = list(args)
            self.assertIn("wallet-sign-owned-deposit", command)
            deposit_path = Path(command[command.index("--deposit-file") + 1])
            deposit = json.loads(deposit_path.read_text(encoding="utf-8"))
            self.assertEqual(deposit["source_address"], "pf-owner")
            self.assertEqual(deposit["sequence"], 8)
            self.assertEqual(deposit["amount_atoms"], 10)
            self.assertEqual(deposit["fee_pft"], 1)
            self.assertEqual(deposit["valid_through_height"], 120)
            output_path = Path(command[command.index("--output") + 1])
            output_path.write_text(
                json.dumps({"operation": {"owned_deposit": {"signed": {}}}}),
                encoding="utf-8",
            )

        with tempfile.TemporaryDirectory() as tmp:
            with (
                mock.patch.object(client, "server_capabilities", return_value=capabilities),
                mock.patch.object(
                    client,
                    "account",
                    return_value={"account": {"sequence": 7, "balance": 100}},
                ),
                mock.patch.object(
                    client,
                    "owned_objects",
                    side_effect=[
                        {"objects": [], "total_value": 0},
                        {
                            "objects": [
                                {
                                    "id": "obj1",
                                    "version": 1,
                                    "owner_pubkey_hex": public_key_hex,
                                    "value": 10,
                                    "asset": "PFT",
                                }
                            ],
                            "total_value": 10,
                        },
                    ],
                ) as owned,
                mock.patch.object(wallet_module, "_run", side_effect=fake_run),
                mock.patch.object(
                    client,
                    "mempool_submit_fastlane_primary_finality",
                    return_value={"tx_id": "deposit-1"},
                ) as submit,
                mock.patch.object(
                    client,
                    "receipts",
                    return_value=[
                        {"tx_id": "deposit-1", "accepted": True, "code": "owned_deposit_applied"}
                    ],
                ),
            ):
                result = wrap_fastpay(
                    client,
                    wallet=wallet,
                    amount=10,
                    work_dir=tmp,
                )

        self.assertEqual(owned.call_count, 2)
        submit.assert_called_once()
        self.assertEqual(result.operation, "wrap")
        self.assertEqual(result.object_id, "obj1")
        self.assertEqual(result.result["receipt"]["code"], "owned_deposit_applied")
        self.assertEqual(result.objects_snapshot["total_value"], 10)

    def test_unwrap_fastpay_signs_collects_quorum_and_applies(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        wallet = self._wallet()
        validators = [
            {"node_id": f"validator-{i}", "public_key_hex": f"pk-{i}"}
            for i in range(6)
        ]

        def owned_unwrap_vote(order_json, validator_id):
            envelope = json.loads(order_json)
            order = envelope["order"]
            self.assertEqual(envelope["owner_pubkey_hex"], "owner_pk")
            self.assertEqual(envelope["owner_signature_hex"], "owner_unwrap_sig")
            self.assertEqual(order["to_address"], "pf-owner")
            self.assertEqual(order["amount"], 6)
            self.assertEqual(order["fee"], 1)
            idx = int(str(validator_id).rsplit("-", 1)[1])
            if idx == 5:
                raise RuntimeError("slow validator not needed for quorum")
            return {"validator_id": validator_id, "signature_hex": f"unwrap-sig{idx}"}

        def sign_v3(*, wallet, order, capabilities, work_dir):
            del wallet, capabilities, work_dir
            signed_order = json.loads(json.dumps(order))
            signed_order["recovery"]["lock_id"] = "aa" * 48
            return {
                "owner_pubkey_hex": "owner_pk",
                "owner_signature_hex": "owner_unwrap_sig",
                "order": signed_order,
            }

        with tempfile.TemporaryDirectory() as tmp:
            with (
                mock.patch.object(client, "server_capabilities", return_value=self._fastpay_caps()),
                mock.patch.object(
                    client,
                    "owned_recovery_capabilities",
                    return_value=self._fastpay_recovery_caps(),
                ),
                mock.patch.object(wallet_module, "_sign_fastpay_unwrap_order_v3", side_effect=sign_v3),
                mock.patch.object(
                    wallet_module,
                    "_verify_fastpay_apply_v3",
                    return_value=tuple({"validator_id": f"validator-{i}"} for i in range(5)),
                ),
                mock.patch.object(client, "validators", return_value={"validators": validators}),
                mock.patch.object(
                    client,
                    "owned_objects",
                    return_value={
                        "objects": [
                            {"id": "large", "version": 1, "value": 10, "asset": "PFT"},
                            {"id": "small", "version": 2, "value": 7, "asset": "PFT"},
                        ]
                    },
                ),
                mock.patch.object(client, "owned_unwrap_sign_v3", side_effect=owned_unwrap_vote) as owned_unwrap_sign,
                mock.patch.object(
                    client,
                    "owned_unwrap_apply_v3",
                    return_value={"validators": []},
                ) as owned_unwrap_apply,
            ):
                result = unwrap_fastpay(
                    client,
                    wallet=wallet,
                    amount=6,
                    fee=1,
                    work_dir=tmp,
                )

        self.assertEqual(result.operation, "unwrap")
        self.assertEqual(result.object_id, "small")
        self.assertEqual(result.order["inputs"], [{"id": "small", "version": 2}])
        self.assertEqual(result.order["to_address"], "pf-owner")
        self.assertEqual(result.order["amount"], 6)
        self.assertEqual(result.order["fee"], 1)
        self.assertEqual(len(result.votes), 5)
        self.assertGreaterEqual(owned_unwrap_sign.call_count, 5)
        self.assertLessEqual(owned_unwrap_sign.call_count, 6)
        owned_unwrap_apply.assert_called_once()
        certificate = json.loads(owned_unwrap_apply.call_args.args[0])
        self.assertEqual(certificate["owner_pubkey_hex"], "owner_pk")
        self.assertEqual(certificate["owner_signature_hex"], "owner_unwrap_sig")
        self.assertEqual(len(certificate["votes"]), 5)

    def test_unwrap_fastpay_combines_fragmented_owned_objects(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        wallet = self._wallet()
        validators = [
            {"node_id": f"validator-{i}", "public_key_hex": f"pk-{i}"}
            for i in range(6)
        ]
        expected_inputs = [{"id": f"object-{index}", "version": 1} for index in range(20)]

        def owned_unwrap_vote(order_json, validator_id):
            envelope = json.loads(order_json)
            order = envelope["order"]
            self.assertEqual(envelope["owner_pubkey_hex"], "owner_pk")
            self.assertEqual(envelope["owner_signature_hex"], "owner_unwrap_sig")
            self.assertEqual(order["inputs"], expected_inputs)
            self.assertEqual(order["amount"], 1_950_000)
            return {"validator_id": validator_id, "signature_hex": f"unwrap-sig-{validator_id}"}

        def sign_v3(*, wallet, order, capabilities, work_dir):
            del wallet, capabilities, work_dir
            signed_order = json.loads(json.dumps(order))
            signed_order["recovery"]["lock_id"] = "ab" * 48
            return {
                "owner_pubkey_hex": "owner_pk",
                "owner_signature_hex": "owner_unwrap_sig",
                "order": signed_order,
            }

        with tempfile.TemporaryDirectory() as tmp:
            with (
                mock.patch.object(client, "server_capabilities", return_value=self._fastpay_caps()),
                mock.patch.object(
                    client,
                    "owned_recovery_capabilities",
                    return_value=self._fastpay_recovery_caps(),
                ),
                mock.patch.object(wallet_module, "_sign_fastpay_unwrap_order_v3", side_effect=sign_v3),
                mock.patch.object(
                    wallet_module,
                    "_verify_fastpay_apply_v3",
                    return_value=tuple({"validator_id": f"validator-{i}"} for i in range(5)),
                ),
                mock.patch.object(client, "validators", return_value={"validators": validators}),
                mock.patch.object(
                    client,
                    "owned_objects",
                    return_value={
                        "objects": [
                            {"id": f"object-{index}", "version": 1, "value": 100_000, "asset": "PFT"}
                            for index in range(20)
                        ]
                    },
                ),
                mock.patch.object(client, "owned_unwrap_sign_v3", side_effect=owned_unwrap_vote),
                mock.patch.object(
                    client,
                    "owned_unwrap_apply_v3",
                    return_value={"validators": []},
                ) as owned_unwrap_apply,
            ):
                result = unwrap_fastpay(
                    client,
                    wallet=wallet,
                    amount=1_950_000,
                    fee=0,
                    work_dir=tmp,
                )

        self.assertEqual(result.operation, "unwrap")
        self.assertEqual(result.order["inputs"], expected_inputs)
        owned_unwrap_apply.assert_called_once()
        certificate = json.loads(owned_unwrap_apply.call_args.args[0])
        self.assertEqual(certificate["order"]["amount"], 1_950_000)
        self.assertEqual(len(certificate["order"]["inputs"]), 20)
        self.assertEqual(len(certificate["votes"]), 5)

    def test_unwrap_fastpay_prefers_recovery_safe_v3_and_authenticates_apply_quorum(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        wallet = self._wallet()
        validators = [
            {"node_id": f"validator-{index}", "public_key_hex": f"pk-{index}"}
            for index in range(6)
        ]
        recovery_caps = self._fastpay_recovery_caps()
        lock_id = "dd" * 48

        def sign_v3(*, wallet, order, capabilities, work_dir):
            del work_dir
            self.assertEqual(wallet.public_key_hex, "owner_pk")
            self.assertEqual(capabilities, recovery_caps)
            self.assertEqual(order["to_address"], "pf-owner")
            self.assertEqual(order["amount"], 6)
            signed_order = json.loads(json.dumps(order))
            signed_order["recovery"]["lock_id"] = lock_id
            return {
                "order": signed_order,
                "owner_pubkey_hex": "owner_pk",
                "owner_signature_hex": "owner-v3-unwrap-signature",
            }

        def vote_v3(order_json, validator_id):
            signed = json.loads(order_json)
            self.assertEqual(signed["order"]["recovery"]["lock_id"], lock_id)
            return {"validator_id": validator_id, "signature_hex": f"vote-{validator_id}"}

        apply_result = {
            "validators": [
                {
                    "validator_id": f"validator-{index}",
                    "ok": True,
                    "result": {"validator_id": f"validator-{index}"},
                }
                for index in range(5)
            ]
        }
        authenticated = tuple(row["result"] for row in apply_result["validators"])

        with tempfile.TemporaryDirectory() as tmp:
            with (
                mock.patch.object(client, "server_capabilities", return_value=self._fastpay_caps()),
                mock.patch.object(client, "owned_recovery_capabilities", return_value=recovery_caps),
                mock.patch.object(client, "validators", return_value={"validators": validators}),
                mock.patch.object(
                    client,
                    "owned_objects",
                    return_value={
                        "objects": [
                            {"id": "22" * 32, "version": 1, "value": 10, "asset": "PFT"}
                        ]
                    },
                ),
                mock.patch.object(
                    wallet_module,
                    "_sign_fastpay_unwrap_order_v3",
                    side_effect=sign_v3,
                ),
                mock.patch.object(client, "owned_unwrap_sign_v3", side_effect=vote_v3) as sign_rpc,
                mock.patch.object(
                    client,
                    "owned_unwrap_apply_v3",
                    return_value=apply_result,
                ) as apply_rpc,
                mock.patch.object(
                    wallet_module,
                    "_verify_fastpay_apply_v3",
                    return_value=authenticated,
                ) as verify_apply,
                mock.patch.object(
                    client,
                    "owned_unwrap_sign",
                    side_effect=AssertionError("legacy v2 unwrap signer must not be called"),
                ),
                mock.patch.object(
                    client,
                    "owned_unwrap_apply",
                    side_effect=AssertionError("legacy v2 unwrap apply must not be called"),
                ),
            ):
                result = unwrap_fastpay(
                    client,
                    wallet=wallet,
                    amount=6,
                    fee=1,
                    work_dir=tmp,
                )

        self.assertEqual(result.order["recovery"]["lock_id"], lock_id)
        self.assertEqual(result.result["authenticated_acknowledgements"], list(authenticated))
        self.assertGreaterEqual(sign_rpc.call_count, 5)
        apply_rpc.assert_called_once()
        verify_apply.assert_called_once()

    def test_send_fastpay_signs_collects_quorum_and_applies(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        wallet = self._wallet()
        validators = [
            {"node_id": f"validator-{i}", "public_key_hex": f"pk-{i}"}
            for i in range(6)
        ]
        def owned_vote(order_json, validator_id):
            envelope = json.loads(order_json)
            self.assertEqual(envelope["owner_pubkey_hex"], "owner_pk")
            self.assertEqual(envelope["owner_signature_hex"], "owner_sig")
            self.assertEqual(envelope["order"]["inputs"][0]["id"], "11" * 32)
            idx = int(str(validator_id).rsplit("-", 1)[1])
            if idx == 5:
                raise RuntimeError("slow validator not needed for quorum")
            return {"validator_id": validator_id, "signature_hex": f"sig{idx}"}

        def sign_v3(*, wallet, order, capabilities, work_dir):
            del wallet, capabilities, work_dir
            signed_order = json.loads(json.dumps(order))
            signed_order["recovery"]["lock_id"] = "ac" * 48
            return {
                "owner_pubkey_hex": "owner_pk",
                "owner_signature_hex": "owner_sig",
                "order": signed_order,
            }

        with tempfile.TemporaryDirectory() as tmp:
            with (
                mock.patch.object(client, "server_capabilities", return_value=self._fastpay_caps()),
                mock.patch.object(
                    client,
                    "owned_recovery_capabilities",
                    return_value=self._fastpay_recovery_caps(),
                ),
                mock.patch.object(wallet_module, "_sign_fastpay_order_v3", side_effect=sign_v3),
                mock.patch.object(
                    wallet_module,
                    "_verify_fastpay_apply_v3",
                    return_value=tuple({"validator_id": f"validator-{i}"} for i in range(5)),
                ),
                mock.patch.object(client, "validators", return_value={"validators": validators}),
                mock.patch.object(
                    client,
                    "owned_objects",
                    return_value={"objects": [{"id": "11" * 32, "version": 1, "value": 100, "asset": "PFT"}]},
                ),
                mock.patch.object(client, "owned_sign_v3", side_effect=owned_vote) as owned_sign,
                mock.patch.object(
                    client,
                    "owned_apply_v3",
                    return_value={"validators": []},
                ) as owned_apply,
                mock.patch.object(wallet_module, "_apply_batch") as apply_batch,
            ):
                result = send_fastpay(
                    client,
                    wallet=wallet,
                    recipient_public_key_hex="recipient_pk",
                    amount=50,
                    fee=1,
                    work_dir=tmp,
                )

        self.assertEqual(result.operation, "send")
        self.assertEqual(len(result.votes), 5)
        self.assertEqual(result.order["outputs"][0]["owner_pubkey_hex"], "recipient_pk")
        self.assertEqual(result.order["outputs"][0]["value"], 50)
        self.assertEqual(result.order["outputs"][1]["owner_pubkey_hex"], "owner_pk")
        self.assertEqual(result.order["outputs"][1]["value"], 49)
        self.assertGreaterEqual(owned_sign.call_count, 5)
        self.assertLessEqual(owned_sign.call_count, 6)
        owned_apply.assert_called_once()
        apply_batch.assert_not_called()

    def test_send_fastpay_prefers_recovery_safe_v3_and_authenticates_apply_quorum(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        wallet = self._wallet()
        validators = [
            {"node_id": f"validator-{index}", "public_key_hex": f"pk-{index}"}
            for index in range(6)
        ]
        recovery_caps = self._fastpay_recovery_caps()
        lock_id = "cc" * 48

        def sign_v3(*, wallet, order, capabilities, work_dir):
            del work_dir
            self.assertEqual(wallet.public_key_hex, "owner_pk")
            self.assertEqual(capabilities, recovery_caps)
            self.assertEqual(order["recovery"]["committee_epoch"], 7)
            self.assertEqual(order["recovery"]["valid_from_height"], 20)
            self.assertEqual(order["recovery"]["expires_at_height"], 28)
            self.assertEqual(order["recovery"]["recovery_closes_at_height"], 40)
            signed_order = json.loads(json.dumps(order))
            signed_order["recovery"]["lock_id"] = lock_id
            return {
                "order": signed_order,
                "owner_pubkey_hex": "owner_pk",
                "owner_signature_hex": "owner-v3-signature",
            }

        def vote_v3(order_json, validator_id):
            signed = json.loads(order_json)
            self.assertEqual(signed["order"]["recovery"]["lock_id"], lock_id)
            return {"validator_id": validator_id, "signature_hex": f"vote-{validator_id}"}

        apply_result = {
            "validators": [
                {
                    "validator_id": f"validator-{index}",
                    "ok": True,
                    "result": {"validator_id": f"validator-{index}"},
                }
                for index in range(5)
            ]
        }
        authenticated = tuple(row["result"] for row in apply_result["validators"])

        with tempfile.TemporaryDirectory() as tmp:
            with (
                mock.patch.object(client, "server_capabilities", return_value=self._fastpay_caps()),
                mock.patch.object(client, "owned_recovery_capabilities", return_value=recovery_caps),
                mock.patch.object(client, "validators", return_value={"validators": validators}),
                mock.patch.object(
                    client,
                    "owned_objects",
                    return_value={
                        "objects": [
                            {"id": "11" * 32, "version": 1, "value": 100, "asset": "PFT"}
                        ]
                    },
                ),
                mock.patch.object(wallet_module, "_sign_fastpay_order_v3", create=True, side_effect=sign_v3),
                mock.patch.object(client, "owned_sign_v3", side_effect=vote_v3) as owned_sign_v3,
                mock.patch.object(client, "owned_apply_v3", return_value=apply_result) as owned_apply_v3,
                mock.patch.object(
                    wallet_module,
                    "_verify_fastpay_apply_v3",
                    create=True,
                    return_value=authenticated,
                ) as verify_apply,
                mock.patch.object(
                    client,
                    "owned_sign",
                    side_effect=AssertionError("legacy v2 signer must not be called"),
                ),
                mock.patch.object(
                    client,
                    "owned_apply",
                    side_effect=AssertionError("legacy v2 apply must not be called"),
                ),
            ):
                result = send_fastpay(
                    client,
                    wallet=wallet,
                    recipient_public_key_hex="recipient_pk",
                    amount=50,
                    fee=1,
                    work_dir=tmp,
                )

        self.assertEqual(len(result.votes), 5)
        self.assertEqual(result.order["recovery"]["lock_id"], lock_id)
        self.assertEqual(result.result["authenticated_acknowledgements"], list(authenticated))
        self.assertGreaterEqual(owned_sign_v3.call_count, 5)
        owned_apply_v3.assert_called_once()
        verify_apply.assert_called_once()

    def test_fastpay_helpers_reject_raw_single_validator_rpc(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        wallet = self._wallet()
        with mock.patch.object(
            client,
            "server_capabilities",
            return_value={"fastpay_bridge_enabled": False},
        ):
            with self.assertRaisesRegex(WalletCommandError, "broadcast RPC endpoint"):
                wrap_fastpay(client, wallet=wallet, amount=10)

    def test_full_fastpay_flow_signs_collects_votes_and_applies(self) -> None:
        """The full FastPay flow: sign order → get votes from validators → assemble cert → apply."""
        client = PostFiatRpcClient("127.0.0.1:1234")

        order_json = json.dumps({
            "order": json.loads(self._make_order_json()),
            "owner_pubkey_hex": "owner_pk",
            "owner_signature_hex": "owner_sig",
        })
        votes = [
            self._make_vote("validator-0", "sig0"),
            self._make_vote("validator-1", "sig1"),
            self._make_vote("validator-2", "sig2"),
        ]
        cert_json = json.dumps({
            "order": json.loads(order_json)["order"],
            "owner_pubkey_hex": "owner_pk",
            "owner_signature_hex": "owner_sig",
            "votes": votes,
        })

        # Mock: owned_sign returns votes for each validator, owned_apply returns success
        with mock.patch.object(client, "_call", side_effect=[
            votes[0],  # owned_sign validator-0
            votes[1],  # owned_sign validator-1
            votes[2],  # owned_sign validator-2
            "certified owned-transfer applied (quorum 3 of 6)",  # owned_apply
        ]) as mock_call:
            # Step 1: Collect votes from validators
            collected_votes = []
            for vid in ["validator-0", "validator-1", "validator-2"]:
                vote = client.owned_sign(order_json, vid)
                collected_votes.append(vote)

            # Step 2: Assemble certificate
            self.assertEqual(len(collected_votes), 3)
            cert = {
                "order": json.loads(order_json)["order"],
                "owner_pubkey_hex": "owner_pk",
                "owner_signature_hex": "owner_sig",
                "votes": collected_votes,
            }

            # Step 3: Apply the certificate
            apply_result = client.owned_apply(json.dumps(cert))

        self.assertEqual(len(collected_votes), 3)
        self.assertEqual(apply_result, "certified owned-transfer applied (quorum 3 of 6)")
        # Verify 4 RPC calls were made (3 owned_sign + 1 owned_apply)
        self.assertEqual(mock_call.call_count, 4)
        # Check the methods called
        calls = mock_call.call_args_list
        self.assertEqual(calls[0][0][0], "owned_sign")
        self.assertEqual(calls[3][0][0], "owned_apply")

    def test_fastpay_flow_handles_validator_sign_failures(self) -> None:
        """FastPay flow should continue collecting votes even if some validators fail."""
        client = PostFiatRpcClient("127.0.0.1:1234")

        order_json = json.dumps({
            "order": json.loads(self._make_order_json()),
            "owner_pubkey_hex": "owner_pk",
            "owner_signature_hex": "owner_sig",
        })

        # Mock: validator-0 succeeds, validator-1 raises, validator-2 succeeds
        def mock_call_side_effect(method, params):
            if method == "owned_sign":
                vid = params["validator_id"]
                if vid == "validator-1":
                    raise Exception("validator unavailable")
                return self._make_vote(vid, f"sig_{vid}")
            if method == "owned_apply":
                return "applied with 2 votes"
            raise ValueError(f"unexpected method: {method}")

        with mock.patch.object(client, "_call", side_effect=mock_call_side_effect):
            collected_votes = []
            for vid in ["validator-0", "validator-1", "validator-2"]:
                try:
                    vote = client.owned_sign(order_json, vid)
                    collected_votes.append(vote)
                except Exception:
                    pass  # Skip failed validators

            self.assertEqual(len(collected_votes), 2)
            cert = {
                "order": json.loads(order_json)["order"],
                "owner_pubkey_hex": "owner_pk",
                "owner_signature_hex": "owner_sig",
                "votes": collected_votes,
            }
            result = client.owned_apply(json.dumps(cert))

        self.assertEqual(len(collected_votes), 2)
        self.assertEqual(result, "applied with 2 votes")

    def test_fastpay_vote_collection_reuses_independent_websocket_workers(self) -> None:
        """Parallel votes reuse one independent socket per validator across payments."""

        class SharedWebSocketClient:
            url = "ws://127.0.0.1:8080"
            timeout_seconds = 7
            response_byte_cap = 12345

            def __init__(self):
                self.workers = {}
                self.worker_requests = []

            def persistent_worker(self, key):
                self.worker_requests.append(key)
                if key not in self.workers:
                    self.workers[key] = VoteClient()
                return self.workers[key]

            def owned_sign(self, _order_json, _validator_id):
                raise AssertionError("shared WebSocket client must not be used for vote RPCs")

        class VoteClient:
            def __init__(self):
                self.calls = []

            def owned_sign(self, _order_json, validator_id):
                self.calls.append(validator_id)
                return {"validator_id": validator_id, "signature_hex": f"sig-{validator_id}"}

        validators = [{"validator_id": f"validator-{idx}"} for idx in range(3)]
        client = SharedWebSocketClient()
        for nonce in (1, 2):
            votes = wallet_module._collect_fastpay_votes(
                client,
                {
                    "order": {"inputs": [], "outputs": [], "fee": 0, "nonce": nonce, "memos": []},
                    "owner_pubkey_hex": "owner_pk",
                    "owner_signature_hex": "owner_sig",
                },
                validators,
                quorum=3,
            )
            self.assertEqual(len(votes), 3)

        self.assertEqual(len(client.workers), 3)
        self.assertEqual(len(client.worker_requests), 6)
        for worker in client.workers.values():
            self.assertEqual(len(worker.calls), 2)

    def test_websocket_fastpay_session_cache_and_worker_lifecycle(self) -> None:
        client = PostFiatWebSocketRpcClient("ws://127.0.0.1:8080")
        loads = []

        first = client.session_get_or_load("fastpay.validators", lambda: loads.append(1) or {"v": 1})
        second = client.session_get_or_load("fastpay.validators", lambda: loads.append(2) or {"v": 2})
        self.assertIs(first, second)
        self.assertEqual(loads, [1])

        worker = client.persistent_worker("fastpay-owned-sign:validator-0")
        self.assertIs(worker, client.persistent_worker("fastpay-owned-sign:validator-0"))
        with mock.patch.object(worker, "close") as worker_close:
            client.close()
        worker_close.assert_called_once_with()
        self.assertEqual(client._worker_clients, {})

        client.clear_session_cache()
        refreshed = client.session_get_or_load(
            "fastpay.validators", lambda: loads.append(3) or {"v": 3}
        )
        self.assertEqual(refreshed, {"v": 3})
        self.assertEqual(loads, [1, 3])

    def test_websocket_proxy_auth_and_origin_propagate_to_persistent_workers(self) -> None:
        client = PostFiatWebSocketRpcClient(
            "ws://127.0.0.1:8080",
            origin="https://wallet.example.test",
            proxy_auth_token="test-session-token",
        )

        request = client._request("owned_sign", {"order_json": "{}"})
        self.assertEqual(request["proxy_auth_token"], "test-session-token")

        worker = client.persistent_worker("fastpay-owned-sign:validator-0")
        self.assertEqual(worker.origin, "https://wallet.example.test")
        self.assertEqual(worker.proxy_auth_token, "test-session-token")

        with mock.patch("websockets.sync.client.connect") as connect:
            connect.return_value = mock.Mock()
            client._connect_websocket()
        connect.assert_called_once_with(
            "ws://127.0.0.1:8080",
            origin="https://wallet.example.test",
            open_timeout=client.timeout_seconds,
            close_timeout=client.timeout_seconds,
            max_size=client.response_byte_cap,
            proxy=None,
        )

    def test_owned_transfer_order_is_value_conserving(self) -> None:
        """An owned-transfer order must conserve value: outputs + fee == inputs."""
        order = {
            "inputs": [{"id": "obj1", "version": 1}],
            "outputs": [
                {"owner_pubkey_hex": "recipient_pk", "value": 90, "asset": "PFT"},
                {"owner_pubkey_hex": "owner_pk", "value": 9, "asset": "PFT"},
            ],
            "fee": 1,
            "nonce": 1,
            "memos": [],
        }
        # Value conservation: 90 + 9 + 1 = 100 (input value)
        total_output = sum(o["value"] for o in order["outputs"])
        self.assertEqual(total_output + order["fee"], 100)

    def test_owned_transfer_certificate_structure(self) -> None:
        """A certificate must contain order, owner_pubkey_hex, owner_signature_hex, and votes."""
        order = json.loads(self._make_order_json())
        cert = {
            "order": order,
            "owner_pubkey_hex": "owner_pk_hex",
            "owner_signature_hex": "owner_sig_hex",
            "votes": [
                {"validator_id": "v0", "signature_hex": "sig0"},
                {"validator_id": "v1", "signature_hex": "sig1"},
            ],
        }
        self.assertIn("order", cert)
        self.assertIn("owner_pubkey_hex", cert)
        self.assertIn("owner_signature_hex", cert)
        self.assertIn("votes", cert)
        self.assertEqual(len(cert["votes"]), 2)
        self.assertEqual(cert["votes"][0]["validator_id"], "v0")


if __name__ == "__main__":
    unittest.main()
