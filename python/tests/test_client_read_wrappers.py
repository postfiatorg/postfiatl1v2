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

    def test_verify_bridge_returns_the_report_on_a_node_with_no_bridge_transfer(self) -> None:
        # Issue #160: with no transfer `latest_transfer_id` is empty; the node's
        # event names the constant subject "bridge" and the report is unchanged.
        client = _client()
        report: dict[str, Any] = {
            "verified": True,
            "domain_count": 0,
            "transfer_count": 0,
            "attestation_count": 0,
            "replay_cache_count": 0,
            "inbound_used": 0,
            "outbound_used": 0,
            "latest_transfer_id": "",
        }

        def answer(request: dict[str, Any]) -> dict[str, Any]:
            self.assertEqual((request["method"], request["params"]), ("verify_bridge", {}))
            return {
                "version": request["version"],
                "id": request["id"],
                "ok": True,
                "result": report,
                "error": None,
                "events": [
                    {"event_type": "verify_bridge", "subject": "bridge", "message": "bridge state verified"}
                ],
            }

        with mock.patch.object(client, "_send", side_effect=answer) as send:
            self.assertEqual(client.verify_bridge(), report)
        send.assert_called_once()

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

    def test_asset_orchard_pair_reads_send_full_ids_and_the_inclusion_height(self) -> None:
        client = _client()
        for method in ("asset_orchard_pricing_claim", "asset_orchard_private_egress_binding"):
            with self.subTest(method=method):
                with mock.patch.object(client, "_call", return_value={"status": "ok"}) as call:
                    getattr(client, method)("aa" * 48, "bb" * 48, 701)
                call.assert_called_once_with(
                    method,
                    {"base_asset_id": "aa" * 48, "quote_asset_id": "bb" * 48, "inclusion_height": 701},
                )
                with mock.patch.object(client, "_call") as call:
                    with self.assertRaisesRegex(ValueError, "inclusion_height must be a non-negative integer"):
                        getattr(client, method)("aa" * 48, "bb" * 48, -1)
                    with self.assertRaisesRegex(ValueError, "quote_asset_id must be a non-empty string"):
                        getattr(client, method)("aa" * 48, "", 701)
                call.assert_not_called()

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

    def test_external_lane_release_coverage_separates_pruned_from_stored(self) -> None:
        row = {
            "lane_id": "base-erc20",
            "release_count": 120,
            "pruned_release_count": 100,
            "release_history_root": "0x" + "ab" * 48,
        }
        self.assertEqual(
            PostFiatRpcClient.external_lane_release_coverage(row),
            {
                "lane_id": "base-erc20",
                "release_count": 120,
                "pruned_release_count": 100,
                "stored_release_count": 20,
                "release_history_root": "0x" + "ab" * 48,
            },
        )
        unpruned = PostFiatRpcClient.external_lane_release_coverage({"release_count": 3})
        self.assertEqual((unpruned["pruned_release_count"], unpruned["stored_release_count"]), (0, 3))
        with self.assertRaises(RpcProtocolError):
            PostFiatRpcClient.external_lane_release_coverage(
                {"release_count": 1, "pruned_release_count": 2}
            )
        with self.assertRaises(RpcProtocolError):
            PostFiatRpcClient.external_lane_release_coverage({"pruned_release_count": 0})

    def test_external_lane_info_sends_the_release_page_parameters(self) -> None:
        client = _client()
        with mock.patch.object(client, "_call", return_value={"lanes": []}) as call:
            client.external_lane_info()
        call.assert_called_once_with("external_lane_info", {})
        with mock.patch.object(client, "_call", return_value={"lanes": []}) as call:
            client.external_lane_info("base-erc20", release_cursor=40, release_limit=20)
        call.assert_called_once_with(
            "external_lane_info",
            {"lane_id": "base-erc20", "release_cursor": 40, "release_limit": 20},
        )
        with self.assertRaisesRegex(ValueError, "release_cursor must be a non-negative integer"):
            client.external_lane_info(release_cursor=-1)

    def test_external_lane_info_bounds_the_release_limit_like_the_other_list_reads(self) -> None:
        client = _client()
        with mock.patch.object(client, "_call", return_value={"lanes": []}) as call:
            with self.assertRaisesRegex(ValueError, "release_limit must be positive"):
                client.external_lane_info(release_limit=0)
            with self.assertRaisesRegex(ValueError, "release_limit must be a non-negative integer"):
                client.external_lane_info(release_limit=True)  # type: ignore[arg-type]
        call.assert_not_called()
        for asked, sent in ((1, 1), (100, 100), (101, 100), (500, 100)):
            with mock.patch.object(client, "_call", return_value={"lanes": []}) as call:
                client.external_lane_info(release_limit=asked)
            call.assert_called_once_with("external_lane_info", {"release_limit": sent})

    def test_external_lane_info_bounds_the_export_limit_like_the_release_limit(self) -> None:
        client = _client()
        with mock.patch.object(client, "_call", return_value={"lanes": []}) as call:
            with self.assertRaisesRegex(ValueError, "export_limit must be positive"):
                client.external_lane_info(export_limit=0)
            with self.assertRaisesRegex(ValueError, "export_limit must be a non-negative integer"):
                client.external_lane_info(export_limit=True)  # type: ignore[arg-type]
        call.assert_not_called()
        for asked, sent in ((1, 1), (100, 100), (101, 100), (500, 100)):
            with mock.patch.object(client, "_call", return_value={"lanes": []}) as call:
                client.external_lane_info(export_limit=asked)
            call.assert_called_once_with("external_lane_info", {"export_limit": sent})

    def test_external_lane_info_sends_the_export_page_parameters(self) -> None:
        client = _client()
        with mock.patch.object(client, "_call", return_value={"lanes": []}) as call:
            client.external_lane_info("base-erc20", export_cursor=4, export_limit=2)
        call.assert_called_once_with(
            "external_lane_info",
            {"lane_id": "base-erc20", "export_cursor": 4, "export_limit": 2},
        )
        with self.assertRaisesRegex(ValueError, "export_cursor must be a non-negative integer"):
            client.external_lane_info(export_cursor=-1)

    def test_external_lane_export_coverage_reads_the_export_fields(self) -> None:
        row = {
            "lane_id": "base-erc20",
            "state": {"exported_atoms": 900, "next_export_nonce": 5},
            "export_count": 5,
            "pruned_export_count": 2,
            "export_history_root": "0x" + "cd" * 48,
            "exports": [],
            "next_export_cursor": None,
        }
        self.assertEqual(
            PostFiatRpcClient.external_lane_export_coverage(row),
            {
                "lane_id": "base-erc20",
                "export_count": 5,
                "pruned_export_count": 2,
                "stored_export_count": 3,
                "exported_atoms": 900,
                "export_history_root": "0x" + "cd" * 48,
            },
        )
        # A row from a node before exports reads as no export.
        before = PostFiatRpcClient.external_lane_export_coverage({"lane_id": "base-erc20", "state": {}})
        self.assertEqual((before["export_count"], before["exported_atoms"]), (0, 0))
        with self.assertRaises(RpcProtocolError):
            PostFiatRpcClient.external_lane_export_coverage(
                {"state": {"next_export_nonce": 2}, "export_count": 1}
            )
        with self.assertRaises(RpcProtocolError):
            PostFiatRpcClient.external_lane_export_coverage(
                {"state": {"next_export_nonce": 1}, "export_count": 1, "pruned_export_count": 2}
            )

    def test_external_lane_info_returns_the_governed_cap_and_committee(self) -> None:
        client = _client()
        release_cap = {
            "active_cap_bps": 100,
            "active": {"amendment_id": "ab" * 48, "cap_bps": 100, "effective_height": 4},
            "pending": {"amendment_id": "cd" * 48, "cap_bps": 200, "effective_height": 36},
        }
        committee = {
            "committee_epoch": 2,
            "committee_root": "0x" + "ef" * 48,
            "active": {"amendment_id": "12" * 48, "committee_epoch": 2, "binding": "34" * 48, "effective_height": 9},
        }
        lane = {
            "lane_id": "base-erc20",
            "window_cap_atoms": 3_000_000,
            "release_cap": release_cap,
            "committee": committee,
        }
        with mock.patch.object(client, "_call", return_value={"lanes": [lane]}):
            report = client.external_lane_info("base-erc20")
        self.assertEqual(report["lanes"][0]["release_cap"], release_cap)
        self.assertEqual(report["lanes"][0]["committee"], committee)

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

    def test_private_receipt_and_market_reads_send_the_token_and_filters(self) -> None:
        client = _client()
        with mock.patch.object(client, "_call", return_value={"status": "ok"}) as call:
            client.private_pool_receipts("t", cursor="c", limit=10, address="pfa", session_scope="s")
        call.assert_called_once_with(
            "private_pool_receipts",
            {"snapshot_token": "t", "cursor": "c", "limit": 10, "address": "pfa", "session_scope": "s"},
        )
        with mock.patch.object(client, "_call", return_value={"status": "ok"}) as call:
            client.private_pool_market("t", "A", "B", inclusion_height=9)
        call.assert_called_once_with(
            "private_pool_market",
            {"snapshot_token": "t", "base_asset_id": "A", "quote_asset_id": "B", "inclusion_height": 9},
        )
        with mock.patch.object(client, "_call") as call:
            with self.assertRaisesRegex(ValueError, "limit must be a positive integer"):
                client.private_pool_receipts("t", limit=0)
            with self.assertRaisesRegex(ValueError, "quote_asset_id must be a non-empty string"):
                client.private_pool_market("t", "A", "")
        call.assert_not_called()

    def test_owned_objects_passes_the_cursor_and_flag_only_when_given(self) -> None:
        client = _client()
        with mock.patch.object(client, "_call", return_value={}) as call:
            client.owned_objects("ab")
        call.assert_called_once_with("owned_objects", {"owner_public_key_hex": "ab"})
        with mock.patch.object(client, "_call", return_value={}) as call:
            client.owned_objects("ab", limit=128, cursor="ooc1.x", omit_owner_keys=True)
        call.assert_called_once_with(
            "owned_objects",
            {"owner_public_key_hex": "ab", "limit": 128, "cursor": "ooc1.x", "omit_owner_keys": True},
        )

    def test_owned_objects_all_follows_cursors_and_restarts_on_a_new_view(self) -> None:
        client = _client()

        def page(view, ids, next_cursor, height=5):
            return {
                "height": height,
                "state_root": "root",
                "view_digest": view,
                "objects": [{"id": i, "value": 10} for i in ids],
                "object_count": len(ids),
                "total_value": 30,
                "truncated": next_cursor is not None,
                "cursor": None,
                "next_cursor": next_cursor,
            }

        # The reviewer's sequence: same height and root, object 1 replaced by 4.
        pages = [
            page("v1", ["1"], "c1"),
            page("v2", ["2", "3", "4"], None),  # the view changed: restart
            page("v2", ["2"], "c2"),
            page("v2", ["3", "4"], None),
        ]
        with mock.patch.object(client, "_call", side_effect=pages) as call:
            result = client.owned_objects_all("ab", page_limit=1)
        self.assertEqual([o["id"] for o in result["objects"]], ["2", "3", "4"])
        self.assertEqual(sum(o["value"] for o in result["objects"]), result["total_value"])
        self.assertEqual(call.call_args_list[1].args[1]["cursor"], "c1")
        self.assertNotIn("cursor", call.call_args_list[2].args[1])
        # The aggregate is not a page: no continuation fields, one view.
        self.assertEqual(
            (result["view_digest"], result["object_count"], result["truncated"]), ("v2", 3, False)
        )
        self.assertIsNone(result["next_cursor"])
        self.assertIsNone(result["cursor"])
        self.assertEqual(result["pages"], 2)

    def test_owned_objects_revision_is_typed_and_optional(self) -> None:
        from postfiat_rpc import OwnedObjectsRevision, RpcClientError
        from postfiat_rpc.client import owned_objects_revision

        page = {"height": 7, "view_digest": "v", "node_id": "validator-0",
                "inventory_revision": {"finalized_height": 7, "fastpay_sequence": 2}}
        node, revision = owned_objects_revision(page)
        self.assertEqual((node, revision), ("validator-0", OwnedObjectsRevision(7, 2)))
        self.assertLess(OwnedObjectsRevision(7, 2), OwnedObjectsRevision(8, 0))
        self.assertIsNone(owned_objects_revision({"height": 7, "view_digest": "v"}))
        for bad in ({"finalized_height": 6, "fastpay_sequence": 2},
                    {"finalized_height": 7, "fastpay_sequence": -1},
                    {"finalized_height": 7, "fastpay_sequence": True}, 3):
            with self.assertRaises(RpcClientError):
                owned_objects_revision({**page, "inventory_revision": bad})
        with self.assertRaises(RpcClientError):
            owned_objects_revision({**page, "node_id": " "})
        # owned_objects_all checks every page and keeps the first page's revision.
        client = _client()
        last = {**page, "objects": [], "object_count": 0, "truncated": False, "next_cursor": None}
        with mock.patch.object(client, "_call", return_value=last):
            result = client.owned_objects_all("ab")
        self.assertEqual(result["inventory_revision"], page["inventory_revision"])
        broken = {**last, "inventory_revision": {"finalized_height": 1}}
        with mock.patch.object(client, "_call", return_value=broken):
            with self.assertRaises(RpcClientError):
                client.owned_objects_all("ab")

    def test_owned_objects_all_restarts_on_a_view_changed_refusal_within_the_bound(self) -> None:
        from postfiat_rpc.client import OWNED_OBJECTS_ALL_MAX_RESTARTS, RpcError

        client = _client()
        first = {"view_digest": "v", "objects": [{"id": "1"}], "object_count": 1,
                 "truncated": True, "next_cursor": "c", "height": 1, "state_root": "r"}
        last = {**first, "objects": [{"id": "2"}], "truncated": False, "next_cursor": None}
        changed = RpcError("owned_objects", {"code": "owned_objects_view_changed", "message": "moved"})
        with mock.patch.object(client, "_call", side_effect=[first, changed, first, last]):
            result = client.owned_objects_all("ab")
        self.assertEqual([o["id"] for o in result["objects"]], ["1", "2"])
        busy = [first, changed] * (OWNED_OBJECTS_ALL_MAX_RESTARTS + 1)
        with mock.patch.object(client, "_call", side_effect=busy) as call:
            with self.assertRaisesRegex(Exception, "kept changing"):
                client.owned_objects_all("ab")
        self.assertEqual(call.call_count, 2 * (OWNED_OBJECTS_ALL_MAX_RESTARTS + 1))
        other = RpcError("owned_objects", {"code": "owned_objects_cursor_malformed", "message": "x"})
        with mock.patch.object(client, "_call", side_effect=[first, other]):
            with self.assertRaises(RpcError):
                client.owned_objects_all("ab")

    def test_owned_objects_all_refuses_a_truncated_answer_from_a_node_without_paging(self) -> None:
        client = _client()
        legacy = {"objects": [{"id": "1"}], "object_count": 1, "truncated": True, "total_value": 20}
        with mock.patch.object(client, "_call", return_value=legacy):
            with self.assertRaisesRegex(Exception, "too old for paging"):
                client.owned_objects_all("ab")
        no_view = {**legacy, "next_cursor": "c", "height": 1, "state_root": "r"}
        with mock.patch.object(client, "_call", return_value=no_view):
            with self.assertRaisesRegex(Exception, "too old for paging"):
                client.owned_objects_all("ab")

    def test_fastpay_recovery_committee_sends_only_the_optional_epoch(self) -> None:
        client = _client()
        with mock.patch.object(client, "_call", return_value={"active": True}) as call:
            client.fastpay_recovery_committee()
        call.assert_called_once_with("fastpay_recovery_committee", {})
        with mock.patch.object(client, "_call", return_value={"active": False}) as call:
            client.fastpay_recovery_committee(epoch=1)
        call.assert_called_once_with("fastpay_recovery_committee", {"epoch": 1})
        with mock.patch.object(client, "_call") as call:
            for bad in (-1, True, "2"):
                with self.assertRaisesRegex(ValueError, "epoch must be a non-negative integer"):
                    client.fastpay_recovery_committee(epoch=bad)  # type: ignore[arg-type]
        call.assert_not_called()
        with mock.patch.object(client, "_call", return_value=[]):
            with self.assertRaisesRegex(RpcProtocolError, "must be an object"):
                client.fastpay_recovery_committee()

    def test_private_readiness_sends_only_the_optional_pool_id(self) -> None:
        client = _client()
        with mock.patch.object(client, "_call", return_value={"ready": False}) as call:
            client.private_readiness()
        call.assert_called_once_with("private_readiness", {})
        with mock.patch.object(client, "_call", return_value={"ready": False}) as call:
            client.private_readiness(pool_id="asset-orchard-v1")
        call.assert_called_once_with("private_readiness", {"pool_id": "asset-orchard-v1"})
        with mock.patch.object(client, "_call") as call:
            with self.assertRaisesRegex(ValueError, "pool_id must be a non-empty string"):
                client.private_readiness(pool_id="")
        call.assert_not_called()

    def test_private_pool_reads_send_the_token_section_cursor_and_limit(self) -> None:
        client = _client()
        with mock.patch.object(client, "_call", return_value={"snapshot_token": "t"}) as call:
            client.private_pool_snapshot("asset-orchard-v1", address="pfa", inclusion_height=7)
        call.assert_called_once_with(
            "private_pool_snapshot",
            {"pool_id": "asset-orchard-v1", "address": "pfa", "inclusion_height": 7},
        )
        with mock.patch.object(client, "_call", return_value={"status": "ok"}) as call:
            client.private_pool_page("t", "commitments", cursor="c", limit=100)
        call.assert_called_once_with(
            "private_pool_page",
            {"snapshot_token": "t", "section": "commitments", "cursor": "c", "limit": 100},
        )
        with mock.patch.object(client, "_call", return_value={"status": "ok"}) as call:
            client.private_account_lines("t", "pfa")
        call.assert_called_once_with("private_account_lines", {"snapshot_token": "t", "address": "pfa"})
        with mock.patch.object(client, "_call") as call:
            with self.assertRaisesRegex(ValueError, "must be given together"):
                client.private_pool_snapshot("p", base_asset_id="A")
            with self.assertRaisesRegex(ValueError, "limit must be a positive integer"):
                client.private_pool_page("t", "outputs", limit=0)
            with self.assertRaisesRegex(ValueError, "snapshot_token must be a non-empty string"):
                client.private_account_lines("", "pfa")
        call.assert_not_called()

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
