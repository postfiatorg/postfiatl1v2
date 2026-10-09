"""Python client wrappers for the archive-window and verifier public reads."""

from __future__ import annotations

import re
import unittest
from pathlib import Path
from typing import Any
from unittest import mock

from postfiat_rpc import PostFiatRpcClient, RpcProtocolError
from postfiat_rpc.client import MAX_ARCHIVE_WINDOW_BLOCKS


def _client() -> PostFiatRpcClient:
    return PostFiatRpcClient("127.0.0.1:1234")


class ArchiveWindowWrapperTests(unittest.TestCase):
    def test_sends_the_node_parameter_names_and_returns_the_bundle(self) -> None:
        client = _client()
        bundle = {"schema": "postfiat-history-archive-window-v1", "from_height": 10, "to_height": 12}
        with mock.patch.object(client, "_call", return_value=bundle) as call:
            self.assertEqual(client.archive_window(10, 12), bundle)
        call.assert_called_once_with("archive_window", {"from_height": 10, "to_height": 12})

    def test_passes_an_archive_uri_only_when_given(self) -> None:
        client = _client()
        with mock.patch.object(client, "_call", return_value={}) as call:
            client.archive_window(5, 5, archive_uri="ipfs://bafy")
        call.assert_called_once_with(
            "archive_window",
            {"from_height": 5, "to_height": 5, "archive_uri": "ipfs://bafy"},
        )

    def test_rejects_bad_ranges_before_sending_anything(self) -> None:
        client = _client()
        with mock.patch.object(client, "_call") as call:
            with self.assertRaisesRegex(ValueError, "to_height must be >= from_height"):
                client.archive_window(12, 10)
            with self.assertRaisesRegex(ValueError, "from_height must be non-negative"):
                client.archive_window(-1, 10)
            with self.assertRaisesRegex(ValueError, "to_height must be an integer"):
                client.archive_window(1, True)  # type: ignore[arg-type]
            with self.assertRaisesRegex(ValueError, "must not exceed"):
                client.archive_window(0, MAX_ARCHIVE_WINDOW_BLOCKS)
            with self.assertRaisesRegex(ValueError, "archive_uri must not be empty"):
                client.archive_window(0, 1, archive_uri="")
        call.assert_not_called()

    def test_accepts_a_window_exactly_at_the_node_bound(self) -> None:
        client = _client()
        with mock.patch.object(client, "_call", return_value={}) as call:
            client.archive_window(100, 100 + MAX_ARCHIVE_WINDOW_BLOCKS - 1)
        call.assert_called_once()

    def test_requires_an_object_result(self) -> None:
        client = _client()
        with mock.patch.object(client, "_call", return_value=[]):
            with self.assertRaisesRegex(RpcProtocolError, "archive_window result must be an object"):
                client.archive_window(0, 0)


class VerifierWrapperTests(unittest.TestCase):
    def test_each_verifier_calls_its_method_with_no_parameters(self) -> None:
        client = _client()
        report: dict[str, Any] = {"ok": True}
        for name in ("verify_blocks", "verify_state", "verify_bridge", "verify_mempool", "verify_shielded"):
            with self.subTest(method=name):
                with mock.patch.object(client, "_call", return_value=report) as call:
                    self.assertEqual(getattr(client, name)(), report)
                call.assert_called_once_with(name, {})

    def test_requires_an_object_report(self) -> None:
        client = _client()
        with mock.patch.object(client, "_call", return_value="ok"):
            with self.assertRaisesRegex(RpcProtocolError, "verify_state result must be an object"):
                client.verify_state()


class ParameterizedReadWrapperTests(unittest.TestCase):
    def test_shield_scan_sends_owner_and_requires_a_list(self) -> None:
        client = _client()
        notes = [{"note_id": "n1"}]
        with mock.patch.object(client, "_call", return_value=notes) as call:
            self.assertEqual(client.shield_scan("owner-1"), notes)
        call.assert_called_once_with("shield_scan", {"owner": "owner-1"})
        with mock.patch.object(client, "_call", return_value={}):
            with self.assertRaisesRegex(RpcProtocolError, "shield_scan result must be a list"):
                client.shield_scan("owner-1")

    def test_single_argument_reads_send_the_node_parameter_name(self) -> None:
        client = _client()
        for method, kwargs, params in (
            ("shield_disclose", {"note_id": "note-1"}, {"note_id": "note-1"}),
            ("vault_bridge_route", {"asset_id": "asset-1"}, {"asset_id": "asset-1"}),
            ("market_ops_status", {"asset_id": "asset-1"}, {"asset_id": "asset-1"}),
            ("market_ops_status", {"asset_id": "asset-1", "epoch": 7}, {"asset_id": "asset-1", "epoch": 7}),
        ):
            with self.subTest(method=method, kwargs=kwargs):
                with mock.patch.object(client, "_call", return_value={"ok": True}) as call:
                    self.assertEqual(getattr(client, method)(**kwargs), {"ok": True})
                call.assert_called_once_with(method, params)

    def test_asset_orchard_action_status_sends_the_four_indexed_elements(self) -> None:
        client = _client()
        with mock.patch.object(client, "_call", return_value={"pool_id": "p"}) as call:
            client.asset_orchard_action_status(("nf1", "nf2"), ["oc1", "oc2"])
        call.assert_called_once_with(
            "asset_orchard_action_status",
            {
                "nullifier_1": "nf1",
                "nullifier_2": "nf2",
                "output_commitment_1": "oc1",
                "output_commitment_2": "oc2",
            },
        )

    def test_bad_arguments_fail_before_any_request(self) -> None:
        client = _client()
        with mock.patch.object(client, "_call") as call:
            with self.assertRaisesRegex(ValueError, "owner must be a non-empty string"):
                client.shield_scan("")
            with self.assertRaisesRegex(ValueError, "note_id must be a non-empty string"):
                client.shield_disclose(None)  # type: ignore[arg-type]
            with self.assertRaisesRegex(ValueError, "epoch must be a non-negative integer"):
                client.market_ops_status("asset-1", epoch=-1)
            with self.assertRaisesRegex(ValueError, "epoch must be a non-negative integer"):
                client.market_ops_status("asset-1", epoch=True)  # type: ignore[arg-type]
            with self.assertRaisesRegex(ValueError, "nullifiers must hold exactly two values"):
                client.asset_orchard_action_status(("only-one",), ("oc1", "oc2"))
            with self.assertRaisesRegex(ValueError, "output_commitment_2 must be a non-empty string"):
                client.asset_orchard_action_status(("nf1", "nf2"), ("oc1", ""))
        call.assert_not_called()

    def test_object_reads_reject_non_object_results(self) -> None:
        client = _client()
        with mock.patch.object(client, "_call", return_value=[]):
            with self.assertRaisesRegex(RpcProtocolError, "vault_bridge_route result must be an object"):
                client.vault_bridge_route("asset-1")


class RemainingReadWrapperTests(unittest.TestCase):
    def test_single_argument_reads_send_the_node_parameter_name(self) -> None:
        client = _client()
        for method, kwargs, params in (
            ("nav_reserve_proof_status", {"asset_id": "asset-1"}, {"asset_id": "asset-1"}),
            ("vault_bridge_status", {"asset_id": "asset-1"}, {"asset_id": "asset-1"}),
            ("fx_fix_info", {"fix_packet_hash": "ab" * 48}, {"fix_packet_hash": "ab" * 48}),
            ("fx_fix_reservation_info", {"reservation_id": "res-1"}, {"reservation_id": "res-1"}),
            ("yolo_target_receipt", {"registration_id": "reg-1"}, {"registration_id": "reg-1"}),
            (
                "pfusdc_egress_witness",
                {"withdrawal_id": "wd-1"},
                {"withdrawal_id": "wd-1"},
            ),
            (
                "pfusdc_egress_witness",
                {"withdrawal_id": "wd-1", "prior_checkpoint": "cd" * 48},
                {"withdrawal_id": "wd-1", "prior_checkpoint": "cd" * 48},
            ),
        ):
            with self.subTest(method=method, kwargs=kwargs):
                with mock.patch.object(client, "_call", return_value={"ok": True}) as call:
                    self.assertEqual(getattr(client, method)(**kwargs), {"ok": True})
                call.assert_called_once_with(method, params)

    def test_fx_fix_list_sends_filters_and_the_presence_flag_only_when_set(self) -> None:
        client = _client()
        with mock.patch.object(client, "_call", return_value={"fixes": []}) as call:
            client.fx_fix_list()
        call.assert_called_once_with("fx_fix_list", {})
        with mock.patch.object(client, "_call", return_value={"fixes": []}) as call:
            client.fx_fix_list(base_asset_id="A", quote_asset_id="B", active_only=True, limit=7)
        call.assert_called_once_with(
            "fx_fix_list",
            {"limit": 7, "base_asset_id": "A", "quote_asset_id": "B", "active_only": True},
        )
        with mock.patch.object(client, "_call", return_value={"fixes": []}) as call:
            client.fx_fix_list(active_only=False, limit=10_000)
        # active_only=False is not sent (the node flag is presence-only); limit is bounded.
        call.assert_called_once_with("fx_fix_list", {"limit": 512})

    def test_quote_and_preflight_send_integer_amounts_with_the_node_names(self) -> None:
        client = _client()
        with mock.patch.object(client, "_call", return_value={"quote": 1}) as call:
            client.fx_fix_quote("ab" * 48, 1_000_000)
        call.assert_called_once_with(
            "fx_fix_quote", {"fix_packet_hash": "ab" * 48, "base_atoms": 1_000_000}
        )
        with mock.patch.object(client, "_call", return_value={"ok": True}) as call:
            client.pfusdc_ingress_preflight(
                "pfusdc", recipient="pf-recipient", depositor="0xdepositor", amount_atoms=0
            )
        call.assert_called_once_with(
            "pfusdc_ingress_preflight",
            {
                "asset_id": "pfusdc",
                "recipient": "pf-recipient",
                "depositor": "0xdepositor",
                "amount_atoms": 0,
            },
        )

    def test_bad_arguments_fail_before_any_request(self) -> None:
        client = _client()
        with mock.patch.object(client, "_call") as call:
            with self.assertRaisesRegex(ValueError, "asset_id must be a non-empty string"):
                client.nav_reserve_proof_status("")
            with self.assertRaisesRegex(ValueError, "base_atoms must be a non-negative integer"):
                client.fx_fix_quote("ab" * 48, -1)
            with self.assertRaisesRegex(ValueError, "base_atoms must be a non-negative integer"):
                client.fx_fix_quote("ab" * 48, True)  # type: ignore[arg-type]
            with self.assertRaisesRegex(ValueError, "amount_atoms must be a non-negative integer"):
                client.pfusdc_ingress_preflight("a", recipient="r", depositor="d", amount_atoms=1.5)  # type: ignore[arg-type]
            with self.assertRaisesRegex(ValueError, "depositor must be a non-empty string"):
                client.pfusdc_ingress_preflight("a", recipient="r", depositor="", amount_atoms=1)
            with self.assertRaisesRegex(ValueError, "prior_checkpoint must be a non-empty string"):
                client.pfusdc_egress_witness("wd-1", prior_checkpoint="")
            with self.assertRaisesRegex(ValueError, "base_asset_id must be a non-empty string"):
                client.fx_fix_list(base_asset_id="")
            with self.assertRaisesRegex(ValueError, "limit must be positive"):
                client.fx_fix_list(limit=0)
        call.assert_not_called()

    def test_object_reads_reject_non_object_results(self) -> None:
        client = _client()
        with mock.patch.object(client, "_call", return_value=[]):
            with self.assertRaisesRegex(RpcProtocolError, "fx_fix_list result must be an object"):
                client.fx_fix_list()


class DocumentedPublicReadCoverageTests(unittest.TestCase):
    """Every method docs/rpc/method-coverage.md marks `public read` must have a
    client wrapper of the same name, so the Python client cannot drift from
    the documented public surface without this test noticing."""

    def test_every_documented_public_read_has_a_client_wrapper(self) -> None:
        coverage = Path(__file__).resolve().parents[2] / "docs" / "rpc" / "method-coverage.md"
        rows = re.findall(r"^\| `([a-z_]+)` \| public read \|", coverage.read_text(encoding="utf-8"), re.M)
        self.assertGreaterEqual(len(rows), 21, rows)
        missing = sorted(name for name in rows if not callable(getattr(PostFiatRpcClient, name, None)))
        self.assertEqual(missing, [], f"public reads without a client wrapper: {missing}")


if __name__ == "__main__":
    unittest.main()
