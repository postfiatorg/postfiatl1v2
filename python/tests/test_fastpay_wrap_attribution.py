"""wrap names the FastPay object its own signed deposit creates, derived the
way the node derives it, not by comparing two changing inventories."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from typing import Any
from unittest import mock

from postfiat_rpc import wallet as wallet_module
from postfiat_rpc.client import PostFiatRpcClient
from postfiat_rpc.wallet import TransparentWallet, WalletCommandError, owned_deposit_object_id, wrap_fastpay

VECTOR = Path(__file__).resolve().parents[2] / "crates/execution/tests/fixtures/owned_deposit_object_v1.json"
KEY = "ab" * 1952
CAPABILITIES = {
    "fastpay_bridge_enabled": True,
    "fastpay_bridge_mode": "proxy_broadcast_devnet",
    "fastpay_owned_apply_broadcast_enabled": True,
    "chain_id": "postfiat-local",
    "genesis_hash": "cd" * 48,
    "protocol_version": 3,
    "block_height": 20,
}


def _wallet() -> TransparentWallet:
    return TransparentWallet(
        chain_id="postfiat-local",
        account_index=0,
        address="pf-owner",
        public_key_hex=KEY,
        key_file=Path("wallet.key.json"),
        backup_file=Path("wallet.backup.json"),
        key_report={},
    )


def _object(ident: str) -> dict[str, Any]:
    return {"id": ident, "version": 1, "owner_pubkey_hex": KEY, "value": 10, "asset": "PFT"}


def _snapshot(objects: list[dict[str, Any]]) -> dict[str, Any]:
    return {"objects": objects, "total_value": 10 * len(objects), "truncated": False, "view_digest": f"v{len(objects)}"}


class SharedVectorTests(unittest.TestCase):
    def test_python_derivation_matches_the_rust_vector(self) -> None:
        vector = json.loads(VECTOR.read_text(encoding="utf-8"))
        self.assertEqual(owned_deposit_object_id(vector["deposit"]), vector["object_id"])
        changed = json.loads(json.dumps(vector["deposit"]))
        changed["nonce"][0] ^= 1
        self.assertNotEqual(owned_deposit_object_id(changed), vector["object_id"])


class WrapAttributionTests(unittest.TestCase):
    def _wrap(self, after: list[str], *, tamper: bool = False) -> tuple[Any, list[str], mock.Mock]:
        """Run wrap; `after` lists the objects of the after-read, where "own"
        stands for the object the signed deposit creates."""

        signed_ids: list[str] = []

        def fake_run(args, *, json_output, cwd=wallet_module.REPO_ROOT):
            del cwd, json_output
            command = list(args)
            deposit = json.loads(Path(command[command.index("--deposit-file") + 1]).read_text(encoding="utf-8"))
            if tamper:
                deposit["amount_atoms"] += 1
            signed_ids.append(owned_deposit_object_id(deposit))
            transaction = {
                "operation": {
                    "kind": "owned_deposit",
                    "signed": {"deposit": deposit, "algorithm_id": "ml-dsa-65", "signature": [0]},
                }
            }
            Path(command[command.index("--output") + 1]).write_text(json.dumps(transaction), encoding="utf-8")

        def owned_objects_all(*_args, **_kwargs):
            return _snapshot([_object(signed_ids[0] if name == "own" else name) for name in after])

        client = PostFiatRpcClient("127.0.0.1:1234")
        submit = mock.Mock(return_value={"tx_id": "deposit-1"})
        with (
            tempfile.TemporaryDirectory() as tmp,
            mock.patch.object(client, "server_capabilities", return_value=CAPABILITIES),
            mock.patch.object(client, "account", return_value={"account": {"sequence": 7, "balance": 100}}),
            mock.patch.object(client, "owned_objects_all", side_effect=owned_objects_all),
            mock.patch.object(wallet_module, "_run", side_effect=fake_run),
            mock.patch.object(client, "mempool_submit_fastlane_primary_finality", submit),
            mock.patch.object(
                client,
                "receipts",
                return_value=[{"tx_id": "deposit-1", "accepted": True, "code": "owned_deposit_applied"}],
            ),
        ):
            try:
                result: Any = wrap_fastpay(client, wallet=_wallet(), amount=10, work_dir=tmp, check_capabilities=False)
            except WalletCommandError as error:
                result = error
        return result, signed_ids, submit

    def test_a_concurrent_same_value_object_is_not_taken_for_the_wrap(self) -> None:
        result, signed_ids, _ = self._wrap(["incoming", "own"])
        self.assertNotIsInstance(result, WalletCommandError)
        self.assertEqual(result.object_id, signed_ids[0])
        self.assertTrue(result.result["object_in_snapshot"])

    def test_an_already_spent_wrap_object_still_reports_the_accepted_deposit(self) -> None:
        result, signed_ids, _ = self._wrap(["incoming"])
        self.assertNotIsInstance(result, WalletCommandError)
        self.assertEqual(result.object_id, signed_ids[0])
        self.assertNotEqual(result.object_id, "incoming")
        self.assertEqual(result.result["receipt"]["code"], "owned_deposit_applied")
        self.assertFalse(result.result["object_in_snapshot"])

    def test_a_signed_deposit_other_than_the_requested_one_is_not_submitted(self) -> None:
        result, _, submit = self._wrap(["own"], tamper=True)
        self.assertIsInstance(result, WalletCommandError)
        self.assertIn("not the requested deposit", str(result))
        submit.assert_not_called()


if __name__ == "__main__":
    unittest.main()
