"""The read-only private-path page renders the node's reads and nothing else."""

from __future__ import annotations

import http.client
import json
import re
import threading
import unittest
from unittest import mock

from postfiat_rpc import PostFiatRpcClient, private_path_ui
from postfiat_rpc.client import RpcError, RpcProtocolError

BASE = "aa" * 48
QUOTE = "bb" * 48
INTENT = "cd" * 48

READINESS = {
    "schema": "postfiat.private_readiness.v1",
    "chain_id": "postfiat-wan-devnet-2",
    "finalized_height": 1161,
    "swap_verifier": {"loaded": True, "matches_pin": True, "error": None},
    "atomic_shielded_batches": {"effective_height": None, "active_at_next_height": False},
    "a666_pfusdc_route": {"route_id": "a666-pfusdc-route", "base_asset_id": BASE, "quote_asset_id": QUOTE},
    "admission_switch": {"state": "unknown", "looked_for": ["--allow-private-action-admission"]},
    "ready": False,
    "reasons": ["shielded_atomic_batch_activation_absent", "asset_orchard_pricing_enforcement_activation_absent"],
}
SNAPSHOT = {
    "schema": "postfiat.private_pool_snapshot.v1",
    "snapshot_token": "pps1:asset-orchard-v1:1161:a4d5:f3b0",
    "finalized_height": 1161,
    "pool_id": "asset-orchard-v1",
    "pool_present": True,
    "counts": {"commitments": 3, "nullifiers": 1, "accepted_anchors": 2, "outputs": 3, "assets": 2},
    "page_limits": {"items": 512, "outputs": 64, "output_ciphertext_bytes": 1048576, "assets": 16},
    "completeness": {"empty_pool_before_ingress": False, "history_complete": True, "outputs_complete": True,
                     "anchors_from_retained_prefixes": True, "complete": True},
}
PRICING_STALE = {
    "schema": "postfiat.asset_orchard_pricing_claim.v1",
    "inclusion_height": 1162,
    "status": "stale",
    "refusal": {"code": "asset_orchard_pricing_nav_stale", "reason": "NAV epoch is live through height 1100"},
    "base_asset_id": BASE,
    "quote_asset_id": QUOTE,
    "pricing_claim": None,
}
EGRESS_UNGOVERNED = {
    "schema": "postfiat.asset_orchard_private_egress_binding.v1",
    "status": "ungoverned",
    "refusal": {"code": "asset_orchard_private_egress_policy_ungoverned", "reason": "no governed egress policy"},
    "bindings": [{"asset_id": BASE, "governed": False, "policy_id": None, "disclosure_hash": None}],
}


def fake_client(**overrides: object) -> mock.Mock:
    client = mock.create_autospec(PostFiatRpcClient, instance=True)
    client.private_readiness.return_value = READINESS
    client.private_pool_snapshot.return_value = SNAPSHOT
    client.asset_orchard_pricing_claim.return_value = PRICING_STALE
    client.asset_orchard_private_egress_binding.return_value = EGRESS_UNGOVERNED
    for name, value in overrides.items():
        target = getattr(client, name)
        if isinstance(value, Exception):
            target.side_effect = value
        else:
            target.return_value = value
    return client


def sections(state: dict) -> dict[str, dict]:
    return {section["title"]: section for section in state["view"]["sections"]}


class PrivatePathStateTests(unittest.TestCase):
    def test_reads_every_section_from_one_validator_at_the_next_height(self) -> None:
        client = fake_client()
        state = private_path_ui.read_private_path_state(client)
        client.private_readiness.assert_called_once_with(pool_id="asset-orchard-v1")
        client.private_pool_snapshot.assert_called_once_with(
            "asset-orchard-v1", base_asset_id=BASE, quote_asset_id=QUOTE
        )
        client.asset_orchard_pricing_claim.assert_called_once_with(BASE, QUOTE, 1162)
        client.asset_orchard_private_egress_binding.assert_called_once_with(BASE, QUOTE, 1162)
        self.assertEqual((state["next_height"], state["base_asset_id"]), (1162, BASE))
        # Read-only: no submit, sign or other mutating call reaches the client.
        called = {call[0].split(".")[0] for call in client.method_calls}
        self.assertEqual(
            called,
            {"private_readiness", "private_pool_snapshot", "asset_orchard_pricing_claim",
             "asset_orchard_private_egress_binding"},
        )

    def test_not_ready_reasons_and_every_readiness_field_are_shown(self) -> None:
        readiness = sections(private_path_ui.read_private_path_state(fake_client()))["Readiness"]
        self.assertEqual(readiness["status"], "not_ready")
        self.assertEqual(readiness["verdict"], "Not ready for private actions")
        self.assertEqual(readiness["reasons"], READINESS["reasons"])
        labels = {label for label, _ in readiness["rows"]}
        for label in ("chain_id", "finalized_height", "swap_verifier.loaded",
                      "atomic_shielded_batches.effective_height", "a666_pfusdc_route.route_id",
                      "admission_switch.state", "admission_switch.looked_for[0]"):
            self.assertIn(label, labels)
        ready = sections(private_path_ui.read_private_path_state(
            fake_client(private_readiness={**READINESS, "ready": True, "reasons": []})
        ))["Readiness"]
        self.assertEqual((ready["status"], ready["reasons"]), ("ok", []))

    def test_snapshot_counts_pricing_and_egress_render_from_fixtures(self) -> None:
        view = sections(private_path_ui.read_private_path_state(fake_client()))
        snapshot = dict(view["Pool snapshot"]["rows"])
        self.assertEqual(view["Pool snapshot"]["status"], "ok")
        self.assertEqual((snapshot["counts.commitments"], snapshot["counts.nullifiers"]), ("3", "1"))
        self.assertEqual(snapshot["snapshot_token"], SNAPSHOT["snapshot_token"])
        pricing = view["Pricing claim"]
        self.assertEqual(pricing["status"], "refused")
        self.assertEqual(pricing["reasons"], ["asset_orchard_pricing_nav_stale"])
        self.assertIn("stale", pricing["verdict"])
        egress = view["Egress binding"]
        self.assertEqual((egress["status"], egress["reasons"]),
                         ("ungoverned", ["asset_orchard_private_egress_policy_ungoverned"]))
        governed = sections(private_path_ui.read_private_path_state(fake_client(
            asset_orchard_private_egress_binding={
                "status": "ok", "refusal": None,
                "bindings": [{"asset_id": BASE, "governed": True, "policy_id": "a666.private-egress.v1",
                              "disclosure_hash": "ef" * 48}],
            }
        )))["Egress binding"]
        self.assertEqual(governed["status"], "ok")
        self.assertIn(["bindings[0].policy_id", "a666.private-egress.v1"], governed["rows"])

    def test_a_node_refusal_is_shown_as_a_refusal_and_a_transport_error_as_a_failure(self) -> None:
        refused = RpcError("private_pool_snapshot", {"code": "rpc_error", "message": "state did not match one committed chain tip"})
        view = sections(private_path_ui.read_private_path_state(fake_client(private_pool_snapshot=refused)))
        snapshot = view["Pool snapshot"]
        self.assertEqual(snapshot["status"], "refused")
        self.assertTrue(snapshot["verdict"].startswith("Refused: "))
        self.assertEqual(snapshot["reasons"], ["rpc_error"])
        failed = sections(private_path_ui.read_private_path_state(
            fake_client(private_readiness=RpcProtocolError("connection reset"))
        ))
        self.assertEqual(failed["Readiness"]["status"], "failed")
        self.assertTrue(failed["Readiness"]["verdict"].startswith("Failed: "))

    def test_without_a_governed_pair_pricing_and_egress_are_refused_not_guessed(self) -> None:
        client = fake_client(private_readiness={**READINESS, "a666_pfusdc_route": None})
        view = sections(private_path_ui.read_private_path_state(client))
        client.asset_orchard_pricing_claim.assert_not_called()
        for title in ("Pricing claim", "Egress binding"):
            self.assertEqual((view[title]["status"], view[title]["reasons"]), ("refused", ["pair_unknown"]))

    def test_receipt_view_states(self) -> None:
        pending = private_path_ui.build_receipt_view(INTENT, {"result": {"state": "pending_unknown", "record": None}})
        self.assertEqual(pending["status"], "pending")
        rejected = private_path_ui.build_receipt_view(INTENT, {"result": {
            "state": "rejected",
            "record": {"batch_json": "x" * 300, "action_json": "{}", "outcome": {"code": "invalid_proof"}},
        }})
        self.assertEqual((rejected["status"], rejected["reasons"]), ("refused", ["invalid_proof"]))
        self.assertIn(["record.batch_json", "<300 bytes>"], rejected["rows"])
        accepted = private_path_ui.build_receipt_view(INTENT, {"result": {"state": "accepted", "record": {}}})
        self.assertEqual(accepted["status"], "ok")


class PrivatePathUiServerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.server = private_path_ui.PrivatePathUiServer(("127.0.0.1", 0), private_path_ui.PrivatePathUiHandler)
        self.client = fake_client()
        self.client.private_action_receipt.return_value = {"state": "pending_unknown", "record": None}
        self.server.reader = lambda: private_path_ui.read_private_path_state(self.client)
        self.server.receipt_reader = lambda intent_id: private_path_ui._read(
            lambda: self.client.private_action_receipt(intent_id)
        )
        thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        thread.start()

        def stop() -> None:
            self.server.shutdown()
            self.server.server_close()
            thread.join(timeout=2)

        self.addCleanup(stop)

    def request(self, method: str, path: str) -> tuple[int, dict[str, str], bytes]:
        connection = http.client.HTTPConnection(*self.server.server_address, timeout=2)
        try:
            connection.request(method, path, body=b"{}" if method in ("POST", "PUT") else None)
            response = connection.getresponse()
            return response.status, {k.lower(): v for k, v in response.getheaders()}, response.read()
        finally:
            connection.close()

    def test_page_and_assets(self) -> None:
        status, headers, body = self.request("GET", "/")
        self.assertEqual(status, 200)
        self.assertTrue(headers["content-type"].startswith("text/html"))
        self.assertIn("default-src 'self'", headers["content-security-policy"])
        html = body.decode()
        for element in ('id="sections"', 'id="refresh"', 'id="intent-id"', 'id="receipt"', 'src="/app.js"'):
            self.assertIn(element, html)
        for path, kind in (("/app.js", "text/javascript"), ("/styles.css", "text/css")):
            status, headers, _ = self.request("GET", path)
            self.assertEqual((status, headers["content-type"].split(";")[0]), (200, kind))

    def test_api_state_returns_the_reads_view_and_fetched_at(self) -> None:
        status, headers, body = self.request("GET", "/api/state")
        self.assertEqual((status, headers["cache-control"]), (200, "no-store"))
        payload = json.loads(body)
        self.assertRegex(payload.pop("fetched_at"), r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$")
        self.assertEqual(payload["schema"], private_path_ui.STATE_SCHEMA)
        self.assertEqual(payload["readiness"]["result"], READINESS)
        self.assertEqual(
            [section["title"] for section in payload["view"]["sections"]],
            ["Readiness", "Pool snapshot", "Pricing claim", "Egress binding"],
        )

    def test_api_receipt_reads_one_intent_and_refuses_a_malformed_id(self) -> None:
        status, _, body = self.request("GET", f"/api/receipt?intent_id={INTENT}")
        payload = json.loads(body)
        self.assertEqual((status, payload["view"]["status"]), (200, "pending"))
        self.client.private_action_receipt.assert_called_once_with(INTENT)
        for query in ("", "?intent_id=CD", f"?intent_id={INTENT}&intent_id={INTENT}", "?intent_id=" + "zz" * 48):
            with self.subTest(query=query):
                status, _, body = self.request("GET", f"/api/receipt{query}")
                self.assertEqual((status, json.loads(body)["refused"]), (400, True))
        self.assertEqual(self.client.private_action_receipt.call_count, 1)

    def test_unknown_path_is_404(self) -> None:
        for path in ("/api/submit", "/index.html/../private_path_ui.py", "/api/private/v1/submit"):
            with self.subTest(path=path):
                self.assertEqual(self.request("GET", path)[0], 404)

    def test_every_non_get_method_is_405(self) -> None:
        for method in ("POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS", "SUBMIT"):
            with self.subTest(method=method):
                status, headers, _ = self.request(method, "/api/state")
                self.assertEqual((status, headers.get("allow")), (405, "GET"))

    def test_assets_hold_no_secrets_or_external_urls(self) -> None:
        for name in ("index.html", "app.js", "styles.css"):
            with self.subTest(asset=name):
                text = (private_path_ui.ASSET_DIR / name).read_text(encoding="utf-8")
                self.assertEqual(re.findall(r"(?:https?:)?//[^\s\"'`<>)]+", text), [])
                self.assertIsNone(re.search(r"api[-_]?key|secret|token=|bearer|password", text, re.IGNORECASE))
                self.assertNotIn("@import", text)
                self.assertNotIn("<script>", text)
                self.assertNotIn("innerHTML", text)
                self.assertNotIn("method:", text, "the page only issues GET requests")


if __name__ == "__main__":
    unittest.main()
