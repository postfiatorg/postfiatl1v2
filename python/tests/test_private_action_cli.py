"""Private action admission: client wrappers and the CLI (W-1, S3)."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest import mock

import json

from postfiat_rpc import PostFiatRpcClient, __main__ as cli
from postfiat_rpc.client import Endpoint, private_action_hash, private_action_intent_id

ACTION = '{"kind":"shielded_swap_v1","swap_json":"{}"}'
GENESIS = "ab" * 48
# Same intent vector as crates/node/src/tests/private_action_admission_tests.rs.
INTENT = "118a9e46af58af181b70f2549388b66da940fedc76730dadee4d79c7a373b58169f04e077e649e9b9d610f7c5f588451"
ACTION_HASH = private_action_hash(ACTION)
VECTORS = Path(__file__).resolve().parents[2] / "crates/node/tests/fixtures/private-actions-v1/vectors.json"


def _client() -> PostFiatRpcClient:
    return PostFiatRpcClient(Endpoint(host="127.0.0.1", port=1))


class PrivateActionTests(unittest.TestCase):
    def test_intent_and_action_hash_match_the_node_vectors(self) -> None:
        self.assertEqual(private_action_intent_id("postfiat-wan-devnet-2", GENESIS, 1, ACTION), INTENT)
        self.assertNotEqual(private_action_intent_id("postfiat-wan-devnet-2", GENESIS, 2, ACTION), INTENT)

    def test_intent_and_action_hash_match_the_contract_fixtures(self) -> None:
        vectors = json.loads(VECTORS.read_text(encoding="utf-8"))
        domain = vectors["domain"]
        for vector in vectors["positive"]:
            with self.subTest(kind=vector["kind"]):
                self.assertEqual(
                    private_action_intent_id(
                        domain["chain_id"], domain["genesis_hash"], domain["protocol_version"], vector["action_json"]
                    ),
                    vector["intent_id"],
                )
                self.assertEqual(private_action_hash(vector["action_json"]), vector["action_hash"])

    def test_client_sends_flat_params_and_refuses_bad_arguments(self) -> None:
        client = _client()
        params = {
            "chain_id": "postfiat-wan-devnet-2",
            "genesis_hash": GENESIS,
            "protocol_version": 1,
            "pool_id": "asset-orchard-v1",
            "session_scope": "native:install",
            "intent_id": INTENT,
            "action_json": ACTION,
            "action_hash": ACTION_HASH,
        }
        with mock.patch.object(client, "_call", return_value={"record": {"state": "queued"}}) as call:
            client.private_action_submit(**params)
        call.assert_called_once_with("private_action_submit", params)
        with mock.patch.object(client, "_call", return_value={"state": "pending_unknown"}) as call:
            client.private_action_receipt(INTENT)
        call.assert_called_once_with("private_action_receipt", {"intent_id": INTENT})
        with mock.patch.object(client, "_call") as call:
            with self.assertRaisesRegex(ValueError, "session_scope must be a non-empty string"):
                client.private_action_submit(**{**params, "session_scope": ""})
            with self.assertRaisesRegex(ValueError, "protocol_version must be a non-negative integer"):
                client.private_action_submit(**{**params, "protocol_version": -1})
        call.assert_not_called()

    def test_cli_recomputes_the_intent_from_the_action_file(self) -> None:
        client = mock.create_autospec(PostFiatRpcClient, instance=True)
        client.private_action_submit.return_value = {"record": {"state": "queued"}}
        with tempfile.TemporaryDirectory() as root:
            action_file = Path(root) / "action.json"
            action_file.write_text(ACTION, encoding="utf-8")
            base = [
                "--endpoint", "local=127.0.0.1:1", "--method", "private_action_submit",
                "--chain-id", "postfiat-wan-devnet-2", "--genesis-hash", GENESIS,
                "--protocol-version", "1", "--session-scope", "native:install",
                "--action-file", str(action_file),
            ]
            args = cli.build_parser().parse_args(base)
            result = cli.call_method(client, args)
            client.private_action_submit.assert_called_once_with(
                chain_id="postfiat-wan-devnet-2",
                genesis_hash=GENESIS,
                protocol_version=1,
                pool_id="asset-orchard-v1",
                session_scope="native:install",
                intent_id=INTENT,
                action_json=ACTION,
                action_hash=ACTION_HASH,
            )
            self.assertEqual(cli.method_checks("private_action_submit", result, args), {"private_action_reserved": True})
            client.reset_mock()
            wrong = cli.build_parser().parse_args([*base, "--intent-id", "00" * 48])
            with self.assertRaises(SystemExit):
                cli.call_method(client, wrong)
            client.private_action_submit.assert_not_called()
            missing = cli.build_parser().parse_args(base[:4] + base[6:])
            with self.assertRaises(SystemExit) as caught:
                cli.call_method(client, missing)
            self.assertIn("--chain-id is required for private_action_submit", str(caught.exception))

    def test_cli_receipt_is_read_only_and_checks_the_state(self) -> None:
        client = mock.create_autospec(PostFiatRpcClient, instance=True)
        client.private_action_receipt.return_value = {"state": "pending_unknown", "record": None}
        args = cli.build_parser().parse_args(
            ["--endpoint", "local=127.0.0.1:1", "--method", "private_action_receipt", "--intent-id", INTENT]
        )
        result = cli.call_method(client, args)
        client.private_action_receipt.assert_called_once_with(INTENT)
        self.assertEqual(cli.method_checks("private_action_receipt", result, args), {"private_action_receipt_state": True})
        self.assertEqual(
            cli.method_checks("private_action_receipt", {"state": "committed"}, args),
            {"private_action_receipt_state": False},
        )


if __name__ == "__main__":
    unittest.main()
