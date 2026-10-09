"""The `python -m postfiat_rpc` CLI reaches every documented public read."""

from __future__ import annotations

import re
import unittest
from pathlib import Path
from unittest import mock

from postfiat_rpc import PostFiatRpcClient, __main__ as cli


def _args(method: str, **overrides):
    argv = ["--endpoint", "local=127.0.0.1:1234", "--method", method]
    for key, value in overrides.items():
        flag = f"--{key.replace('_', '-')}"
        if value is True:
            argv.append(flag)
        else:
            argv.extend([flag, str(value)])
    return cli.build_parser().parse_args(argv)


class CliPublicReadDispatchTests(unittest.TestCase):
    def test_every_documented_public_read_is_a_cli_method_choice(self) -> None:
        coverage = Path(__file__).resolve().parents[2] / "docs" / "rpc" / "method-coverage.md"
        rows = re.findall(r"^\| `([a-z_]+)` \| public read \|", coverage.read_text(encoding="utf-8"), re.M)
        self.assertGreaterEqual(len(rows), 21, rows)
        missing = sorted(name for name in rows if name not in cli.CLI_METHODS)
        self.assertEqual(missing, [], f"public reads not reachable from the CLI: {missing}")
        # Every CLI method must be a client method of the same name (dispatch
        # cannot route to something that does not exist).
        self.assertEqual(
            sorted(name for name in cli.CLI_METHODS if not callable(getattr(PostFiatRpcClient, name, None))),
            [],
        )

    def test_dispatch_routes_each_public_read_with_the_parsed_arguments(self) -> None:
        client = mock.create_autospec(PostFiatRpcClient, instance=True)
        cases = [
            ("archive_window", {"from_height": 10, "to_height": 12}, "archive_window", (10, 12), {"archive_uri": None}),
            (
                "archive_window",
                {"from_height": 10, "to_height": 12, "archive_uri": "ipfs://x"},
                "archive_window",
                (10, 12),
                {"archive_uri": "ipfs://x"},
            ),
            ("verify_blocks", {}, "verify_blocks", (), {}),
            ("verify_shielded", {}, "verify_shielded", (), {}),
            ("shield_scan", {"owner": "pf-owner"}, "shield_scan", ("pf-owner",), {}),
            ("shield_disclose", {"note_id": "note-1"}, "shield_disclose", ("note-1",), {}),
            ("vault_bridge_route", {"asset_id": "a"}, "vault_bridge_route", ("a",), {}),
            ("vault_bridge_status", {"asset_id": "a"}, "vault_bridge_status", ("a",), {}),
            ("nav_reserve_proof_status", {"asset_id": "a"}, "nav_reserve_proof_status", ("a",), {}),
            ("market_ops_status", {"asset_id": "a"}, "market_ops_status", ("a",), {"epoch": None}),
            ("market_ops_status", {"asset_id": "a", "epoch": 7}, "market_ops_status", ("a",), {"epoch": 7}),
            (
                "asset_orchard_action_status",
                {"nullifier_1": "n1", "nullifier_2": "n2", "output_commitment_1": "c1", "output_commitment_2": "c2"},
                "asset_orchard_action_status",
                (("n1", "n2"), ("c1", "c2")),
                {},
            ),
            (
                "fx_fix_list",
                {},
                "fx_fix_list",
                (),
                {"base_asset_id": None, "quote_asset_id": None, "active_only": False, "limit": None},
            ),
            (
                "fx_fix_list",
                {"base_asset_id": "A", "quote_asset_id": "B", "active_only": True, "limit": 5},
                "fx_fix_list",
                (),
                {"base_asset_id": "A", "quote_asset_id": "B", "active_only": True, "limit": 5},
            ),
            ("fx_fix_info", {"fix_packet_hash": "ab" * 48}, "fx_fix_info", ("ab" * 48,), {}),
            ("fx_fix_reservation_info", {"reservation_id": "r"}, "fx_fix_reservation_info", ("r",), {}),
            ("fx_fix_quote", {"fix_packet_hash": "ab" * 48, "base_atoms": 5}, "fx_fix_quote", ("ab" * 48, 5), {}),
            (
                "pfusdc_ingress_preflight",
                {"asset_id": "a", "recipient": "r", "depositor": "d", "amount_atoms": 9},
                "pfusdc_ingress_preflight",
                ("a",),
                {"recipient": "r", "depositor": "d", "amount_atoms": 9},
            ),
            ("pfusdc_egress_witness", {"withdrawal_id": "w"}, "pfusdc_egress_witness", ("w",), {"prior_checkpoint": None}),
            (
                "pfusdc_egress_witness",
                {"withdrawal_id": "w", "prior_checkpoint": "cp"},
                "pfusdc_egress_witness",
                ("w",),
                {"prior_checkpoint": "cp"},
            ),
            ("yolo_target_receipt", {"registration_id": "reg"}, "yolo_target_receipt", ("reg",), {}),
        ]
        for method, argv, target, expected_args, expected_kwargs in cases:
            with self.subTest(method=method, argv=argv):
                client.reset_mock()
                getattr(client, target).return_value = {"ok": True}
                self.assertEqual(cli.call_method(client, _args(method, **argv)), {"ok": True})
                getattr(client, target).assert_called_once_with(*expected_args, **expected_kwargs)

    def test_missing_required_arguments_exit_before_any_call(self) -> None:
        client = mock.create_autospec(PostFiatRpcClient, instance=True)
        for method, argv, flag in (
            ("archive_window", {"from_height": 1}, "--to-height"),
            ("shield_scan", {}, "--owner"),
            ("fx_fix_quote", {"fix_packet_hash": "ab" * 48}, "--base-atoms"),
            ("pfusdc_ingress_preflight", {"asset_id": "a", "recipient": "r", "depositor": "d"}, "--amount-atoms"),
            ("asset_orchard_action_status", {"nullifier_1": "n1", "nullifier_2": "n2", "output_commitment_1": "c1"}, "--output-commitment-2"),
        ):
            with self.subTest(method=method):
                with self.assertRaises(SystemExit) as caught:
                    cli.call_method(client, _args(method, **argv))
                self.assertIn(f"{flag} is required for {method}", str(caught.exception))
        self.assertEqual(client.method_calls, [])

    def test_method_checks_require_the_documented_result_shape(self) -> None:
        args = _args("verify_state")
        self.assertEqual(cli.method_checks("verify_state", {"ok": True}, args), {"verify_state_is_object": True})
        self.assertEqual(cli.method_checks("verify_state", [], args), {"verify_state_is_object": False})
        self.assertEqual(cli.method_checks("shield_scan", [], _args("shield_scan", owner="o")), {"shield_scan_is_list": True})
        self.assertEqual(cli.method_checks("fx_fix_list", "nope", _args("fx_fix_list")), {"fx_fix_list_is_object": False})


if __name__ == "__main__":
    unittest.main()
