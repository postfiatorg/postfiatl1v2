from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest import mock

import postfiat_rpc.pftl_transfer as cli
from postfiat_rpc.wallet import FaucetPftResult, SendPftResult, TransparentWallet


class PftlTransferCliTests(unittest.TestCase):
    def test_pft_to_atoms_is_exact_at_six_decimals(self) -> None:
        self.assertEqual(cli.pft_to_atoms("20"), 20_000_000)
        self.assertEqual(cli.pft_to_atoms("0.000001"), 1)
        with self.assertRaises(ValueError):
            cli.pft_to_atoms("0")
        with self.assertRaises(ValueError):
            cli.pft_to_atoms("1.0000001")

    def test_faucet_action_delegates_to_wallet_helper(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            args = cli.build_parser().parse_args(
                [
                    "faucet",
                    "--to",
                    "pfrecipient",
                    "--amount",
                    "20",
                    "--work-dir",
                    str(root / "work"),
                ]
            )
            result = FaucetPftResult(
                tx_id="tx1",
                batch_file=root / "faucet.batch.json",
                batch={"ok": True},
                receipts_by_validator=(),
            )
            with (
                mock.patch.object(cli, "default_data_dir", return_value=root / "node0"),
                mock.patch.object(cli, "default_validator_data_dirs", return_value=[root / "node0"]),
                mock.patch.object(cli, "request_faucet_pft", return_value=result) as request,
            ):
                report = cli.run_faucet(args)

        self.assertTrue(report["ok"])
        self.assertEqual(report["amount_atoms"], 20_000_000)
        self.assertEqual(report["tx_id"], "tx1")
        kwargs = request.call_args.kwargs
        self.assertEqual(kwargs["to_address"], "pfrecipient")
        self.assertEqual(kwargs["amount"], 20_000_000)

    def test_send_action_loads_wallet_and_delegates_to_send_pft(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            wallet = TransparentWallet(
                chain_id="postfiat-local",
                account_index=0,
                address="pfsender",
                public_key_hex="00",
                key_file=root / "transparent-0.key.json",
                backup_file=root / "transparent-0.backup.json",
                key_report={},
            )
            result = SendPftResult(
                tx_id="tx2",
                quote_response={"ok": True},
                signed_transfer={"ok": True},
                submit_result={"ok": True},
                finalized_batch_file=None,
                receipts_by_validator=(),
            )
            args = cli.build_parser().parse_args(
                [
                    "send",
                    "--to",
                    "pfrecipient",
                    "--amount",
                    "1.25",
                    "--endpoint",
                    "127.0.0.1:27650",
                    "--wallet-dir",
                    str(root),
                ]
            )
            client = object()
            with (
                mock.patch.object(cli, "PostFiatRpcClient", return_value=client) as client_ctor,
                mock.patch.object(cli, "load_wallet", return_value=wallet) as load,
                mock.patch.object(cli, "send_pft", return_value=result) as send,
            ):
                report = cli.run_send(args)

        self.assertTrue(report["ok"])
        self.assertEqual(report["amount_atoms"], 1_250_000)
        self.assertEqual(report["tx_id"], "tx2")
        client_ctor.assert_called_once_with("127.0.0.1:27650", timeout_seconds=8.0)
        self.assertEqual(load.call_args.kwargs["wallet_dir"], str(root))
        kwargs = send.call_args.kwargs
        self.assertIs(kwargs["wallet"], wallet)
        self.assertEqual(kwargs["to_address"], "pfrecipient")
        self.assertEqual(kwargs["amount"], 1_250_000)


if __name__ == "__main__":
    unittest.main()
