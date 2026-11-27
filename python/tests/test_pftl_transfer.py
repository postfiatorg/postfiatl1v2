from __future__ import annotations

import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import pytest

import postfiat_rpc.pftl_transfer as cli
from postfiat_rpc.wallet import FaucetPftResult, SendPftResult, TransparentWallet

RECIPIENT = "pfb385030ad80e5f7a59ec93b71f11055e9ccb9230"
RECIPIENT_CHECKSUMMED = "pfb385030Ad80e5F7a59EC93b71f11055E9cCb9230"


class PftlTransferCliTests(unittest.TestCase):
    def test_transfer_report_preserves_large_atom_amount_as_decimal_string(self) -> None:
        import json

        for atoms, expected in [
            (1, "0.000001"),
            (1_250_000, "1.250000"),
            (2**64 - 1, "18446744073709.551615"),
        ]:
            with self.subTest(atoms=atoms):
                report = json.loads(json.dumps(cli.transfer_report("send", atoms, {"tx_id": "tx"})))
                self.assertEqual(report["amount_atoms"], atoms)
                self.assertEqual(report["amount_pft"], expected)
                self.assertEqual(cli.pft_to_atoms(report["amount_pft"]), atoms)

    def test_verified_send_refuses_incomplete_trust_before_submission(self) -> None:
        args = cli.build_parser().parse_args([
            "send", "--endpoint", "127.0.0.1:1234", "--wallet-dir", "wallet",
            "--to", RECIPIENT, "--amount-atoms", "1", "--poll-finality",
            "--verification-archive", "archive", "--trusted-genesis-hash", "ab" * 48,
        ])
        with mock.patch.object(cli, "load_wallet") as load, self.assertRaises(ValueError):
            cli.run_send(args)
        load.assert_not_called()

    def test_historical_committee_pins_require_full_verification_settings(self) -> None:
        args = cli.build_parser().parse_args([
            "send", "--endpoint", "127.0.0.1:1234", "--wallet-dir", "wallet",
            "--to", RECIPIENT, "--amount-atoms", "1", "--poll-finality",
            "--historical-committee-pins", "recovery.json",
        ])
        with mock.patch.object(cli, "load_wallet") as load, self.assertRaises(ValueError):
            cli.run_send(args)
        load.assert_not_called()

    def test_pft_to_atoms_is_exact_at_six_decimals(self) -> None:
        self.assertEqual(cli.pft_to_atoms("20"), 20_000_000)
        self.assertEqual(cli.pft_to_atoms("0.000001"), 1)
        with self.assertRaises(ValueError):
            cli.pft_to_atoms("0")
        with self.assertRaises(ValueError):
            cli.pft_to_atoms("1.0000001")

    def test_pft_to_atoms_accepts_finite_positive_amounts(self) -> None:
        self.assertEqual(cli.pft_to_atoms("1"), 1_000_000)
        self.assertEqual(cli.pft_to_atoms("0.000001"), 1)
        self.assertEqual(cli.pft_to_atoms("12.5"), 12_500_000)
        self.assertEqual(cli.pft_to_atoms(" 3 "), 3_000_000)

    def test_pft_to_atoms_refuses_extra_decimals_beyond_28_significant_digits(self) -> None:
        for value in [
            "1." + "0" * 27 + "1",
            "1." + "0" * 35 + "1",
            "12345678901234567890123.4567891",
            "9999999999999999999999.9999991",
        ]:
            with self.subTest(value=value):
                with self.assertRaisesRegex(ValueError, "^amount has more than 6 decimal places$"):
                    cli.pft_to_atoms(value)

    def test_pft_to_atoms_keeps_trailing_zero_spellings(self) -> None:
        for value in ["1.000000000000000000000000000000", "1." + "0" * 40]:
            with self.subTest(value=value):
                self.assertEqual(cli.pft_to_atoms(value), 1_000_000)

    def test_pft_to_atoms_scales_long_integers_exactly(self) -> None:
        for value in ["123456789012345678901234567891", "1" * 60]:
            with self.subTest(value=value):
                self.assertEqual(cli.pft_to_atoms(value), int(value) * 10**6)

    def test_pft_to_atoms_existing_results_are_unchanged(self) -> None:
        for value, atoms in [
            ("1", 1_000_000),
            ("1.000001", 1_000_001),
            ("1e3", 1_000_000_000),
            ("1E-6", 1),
            (" 7 ", 7_000_000),
            ("18446744073709.551615", 2**64 - 1),
            ("9" * 22 + ".999999", int("9" * 28)),
        ]:
            with self.subTest(value=value):
                self.assertEqual(cli.pft_to_atoms(value), atoms)
        for value, message in [
            ("1.0000001", "amount has more than 6 decimal places"),
            ("18446744073709.5516151", "amount has more than 6 decimal places"),
            ("0", "amount must be positive"),
            ("-1", "amount must be positive"),
            ("abc", "amount must be a decimal PFTL value"),
            ("NaN", "amount must be a finite decimal PFTL value"),
            ("Infinity", "amount must be a finite decimal PFTL value"),
            ("1e400000000", "amount exponent is out of decimal range"),
            ("1e-400000000", "amount is below PFTL precision"),
        ]:
            with self.subTest(value=value):
                with self.assertRaisesRegex(ValueError, "^" + message + "$"):
                    cli.pft_to_atoms(value)

    def test_send_cli_reports_nan_amount_as_value_error(self) -> None:
        stdout = io.StringIO()
        with (
            mock.patch.object(cli, "PostFiatRpcClient") as client_ctor,
            mock.patch.object(cli, "load_wallet") as load,
            mock.patch.object(cli, "send_pft") as send,
            contextlib.redirect_stdout(stdout),
        ):
            exit_code = cli.main(
                [
                    "send",
                    "--to",
                    RECIPIENT,
                    "--amount",
                    "NaN",
                    "--endpoint",
                    "rpc.invalid:27650",
                    "--wallet-dir",
                    "/nonexistent",
                ]
            )

        self.assertEqual(exit_code, 1)
        report = json.loads(stdout.getvalue())
        self.assertFalse(report["ok"])
        self.assertTrue(report["error"].startswith("ValueError: amount"), report["error"])
        client_ctor.assert_not_called()
        load.assert_not_called()
        send.assert_not_called()

    def test_faucet_requires_explicit_apply_acknowledgement(self) -> None:
        args = cli.build_parser().parse_args(["faucet", "--to", RECIPIENT])
        with mock.patch.object(cli, "request_faucet_pft") as request:
            with self.assertRaisesRegex(ValueError, "--allow-faucet-apply"):
                cli.run_faucet(args)
        request.assert_not_called()

    def test_faucet_action_delegates_to_wallet_helper(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            args = cli.build_parser().parse_args(
                [
                    "faucet",
                    "--allow-faucet-apply",
                    "--to",
                    RECIPIENT,
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
        self.assertEqual(kwargs["to_address"], RECIPIENT)
        self.assertEqual(kwargs["amount"], 20_000_000)

    def test_cli_rejects_a_malformed_recipient_before_any_side_effect(self) -> None:
        for to, reason in [
            ("pfrecipient", "wrong_length"),
            ("pf" + "g" * 40, "non_hex"),
            ("PF" + RECIPIENT[2:], "uppercase_prefix"),
            ("pfB385030Ad80e5F7a59EC93b71f11055E9cCb9230", "bad_checksum"),
        ]:
            with self.subTest(to=to):
                stdout = io.StringIO()
                with (
                    mock.patch.object(cli, "PostFiatRpcClient") as client_ctor,
                    mock.patch.object(cli, "load_wallet") as load,
                    mock.patch.object(cli, "send_pft") as send,
                    contextlib.redirect_stdout(stdout),
                ):
                    exit_code = cli.main(
                        [
                            "send",
                            "--to",
                            to,
                            "--endpoint",
                            "rpc.invalid:27650",
                            "--wallet-dir",
                            "/nonexistent",
                        ]
                    )
                self.assertEqual(exit_code, 1)
                report = json.loads(stdout.getvalue())
                self.assertFalse(report["ok"])
                self.assertIn(f"--to is not a valid PFTL address ({reason})", report["error"])
                client_ctor.assert_not_called()
                load.assert_not_called()
                send.assert_not_called()

    def test_address_command_prints_canonical_and_checksummed_forms(self) -> None:
        for given in [RECIPIENT, RECIPIENT_CHECKSUMMED, f"  {RECIPIENT_CHECKSUMMED}\n"]:
            with self.subTest(given=given):
                stdout = io.StringIO()
                with contextlib.redirect_stdout(stdout):
                    exit_code = cli.main(["address", given])
                self.assertEqual(exit_code, 0)
                report = json.loads(stdout.getvalue())
                self.assertEqual(report["canonical"], RECIPIENT)
                self.assertEqual(report["checksummed"], RECIPIENT_CHECKSUMMED)

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
                    RECIPIENT_CHECKSUMMED,
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
        self.assertEqual(kwargs["to_address"], RECIPIENT)
        self.assertEqual(kwargs["amount"], 1_250_000)


def _send_result(**overrides: object) -> SendPftResult:
    fields: dict[str, object] = {
        "tx_id": "tx-poll",
        "quote_response": {"ok": True},
        "signed_transfer": {"ok": True},
        "submit_result": {"ok": True},
        "finalized_batch_file": None,
        "receipts_by_validator": (),
        "submit_mode": "submit_and_poll",
    }
    fields.update(overrides)
    return SendPftResult(**fields)  # type: ignore[arg-type]


TIMED_OUT = {"pending": True, "finalized": False, "finality_timeout": True}
FINALIZED = {
    "pending": False,
    "finalized": True,
    "finality_timeout": False,
    "finality_receipt": {"tx_id": "tx-poll", "accepted": True},
}


def _run_send_cli(
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    result: SendPftResult,
    *,
    poll: bool,
) -> tuple[int, str, str]:
    argv = [
        "send",
        "--to",
        RECIPIENT,
        "--amount",
        "1",
        "--endpoint",
        "127.0.0.1:27650",
        "--wallet-dir",
        str(tmp_path),
    ]
    if poll:
        argv.append("--poll-finality")
    with (
        mock.patch.object(cli, "PostFiatRpcClient"),
        mock.patch.object(cli, "load_wallet"),
        mock.patch.object(cli, "send_pft", return_value=result) as send,
        mock.patch.object(cli, "send_pft_and_poll_finality", return_value=result) as polled,
    ):
        exit_code = cli.main(argv)
    (polled if poll else send).assert_called_once()
    (send if poll else polled).assert_not_called()
    captured = capsys.readouterr()
    return exit_code, captured.out, captured.err


def _main_stdout(result: SendPftResult) -> str:
    # What main prints for the same result: the unchanged transfer_report, ok true.
    return json.dumps(cli.transfer_report("send", 1_000_000, result), indent=2, sort_keys=True) + "\n"


def test_poll_finality_timeout_exits_zero_and_warns_once_on_stderr(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    result = _send_result(**TIMED_OUT)
    exit_code, out, err = _run_send_cli(tmp_path, capsys, result, poll=True)

    assert exit_code == 0
    assert out == _main_stdout(result)
    report = json.loads(out)
    assert report["ok"] is True
    assert report["result"]["finality_timeout"] is True
    assert "error" not in report
    lines = err.splitlines()
    assert len(lines) == 1, err
    assert err.endswith("\n")
    assert "finality not observed" in lines[0]
    assert "tx-poll" in lines[0]
    assert "do not resubmit" in lines[0]


@pytest.mark.parametrize(
    "fields",
    [
        {"pending": True, "finalized": False, "finality_timeout": False},
        {"pending": False, "finalized": True, "finality_timeout": True},
    ],
    ids=["not-finalized-no-timeout-flag", "finalized-but-timeout-flag"],
)
def test_poll_finality_not_proven_warns_once(
    tmp_path: Path, capsys: pytest.CaptureFixture[str], fields: dict[str, bool]
) -> None:
    result = _send_result(**fields)
    exit_code, out, err = _run_send_cli(tmp_path, capsys, result, poll=True)

    assert exit_code == 0
    assert out == _main_stdout(result)
    assert len(err.splitlines()) == 1
    assert "finality not observed" in err


def test_poll_finality_without_tx_id_exits_zero_and_warns_once(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    result = _send_result(tx_id=None, submit_mode="rpc_submit", pending=True)
    exit_code, out, err = _run_send_cli(tmp_path, capsys, result, poll=True)

    assert exit_code == 0
    assert out == _main_stdout(result)
    lines = err.splitlines()
    assert len(lines) == 1, err
    assert "finality not polled" in lines[0]
    assert "do not resubmit" in lines[0]


def test_poll_finality_verifier_failure_keeps_tx_id_exits_zero_and_warns_once(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    error = "WalletCommandError: payment finality verification failed: archive unreadable"
    result = _send_result(pending=True, confirmation="pending", verification_error=error)
    exit_code, out, err = _run_send_cli(tmp_path, capsys, result, poll=True)

    assert exit_code == 0
    assert out == _main_stdout(result)
    report = json.loads(out)
    assert report["ok"] is True
    assert report["tx_id"] == "tx-poll"
    assert report["result"]["confirmation"] == "pending"
    assert report["result"]["verification_error"] == error
    lines = err.splitlines()
    assert len(lines) == 1, err
    assert "local verifier failed" in lines[0]
    assert "archive unreadable" in lines[0]
    assert "tx-poll" in lines[0]
    assert "do not resubmit" in lines[0]


def test_poll_finality_warning_escapes_tx_id_control_characters(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    result = _send_result(tx_id="tx\nsecond-line", **TIMED_OUT)
    exit_code, out, err = _run_send_cli(tmp_path, capsys, result, poll=True)

    assert exit_code == 0
    assert out == _main_stdout(result)
    assert len(err.splitlines()) == 1, err
    assert "tx\\nsecond-line" in err


def test_poll_finality_finalized_exits_zero_with_empty_stderr(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    result = _send_result(**FINALIZED)
    exit_code, out, err = _run_send_cli(tmp_path, capsys, result, poll=True)

    assert exit_code == 0
    assert out == _main_stdout(result)
    assert err == ""


def test_plain_send_is_unchanged_and_never_warns(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    result = _send_result(submit_mode="rpc_submit", pending=True)
    exit_code, out, err = _run_send_cli(tmp_path, capsys, result, poll=False)

    assert exit_code == 0
    assert out == _main_stdout(result)
    assert json.loads(out)["ok"] is True
    assert err == ""


def test_poll_finality_warning_survives_closed_stderr(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    result = _send_result(**TIMED_OUT)
    closed = io.StringIO()
    closed.close()
    with mock.patch.object(cli.sys, "stderr", closed):
        exit_code, out, _ = _run_send_cli(tmp_path, capsys, result, poll=True)

    assert exit_code == 0
    assert out == _main_stdout(result)


@pytest.mark.parametrize(
    "value",
    [
        "NaN",
        "sNaN",
        "-NaN",
        "Infinity",
        "-Infinity",
        "inf",
        "1e400000000",
        "",
        "abc",
        "0",
        "-1",
        "0.0000001",
    ],
)
def test_pft_to_atoms_refuses_non_finite_and_invalid_amounts(value: str) -> None:
    with pytest.raises(ValueError, match="amount"):
        cli.pft_to_atoms(value)


if __name__ == "__main__":
    unittest.main()
