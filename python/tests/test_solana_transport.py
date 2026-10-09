"""Transport bounds for the Solana NAV observation adapter."""

from __future__ import annotations

import io
import json
import unittest
from unittest import mock

from postfiat_rpc import solana


class _Response(io.BytesIO):
    """A urlopen-style response: a readable body usable as a context manager."""

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()
        return False


def _urlopen_returning(payload: bytes):
    return mock.patch(
        "postfiat_rpc.solana.urllib.request.urlopen",
        return_value=_Response(payload),
    )


class SolanaRpcTransportTests(unittest.TestCase):
    def test_parses_a_bounded_json_object_and_returns_result(self) -> None:
        body = json.dumps({"jsonrpc": "2.0", "id": 1, "result": {"value": 42}}).encode()
        with _urlopen_returning(body) as urlopen:
            self.assertEqual(solana.fetch_balance_lamports("Addr1", "http://rpc.test"), 42)
        request = urlopen.call_args.args[0]
        self.assertEqual(request.full_url, "http://rpc.test")
        self.assertEqual(json.loads(request.data)["method"], "getBalance")

    def test_accepts_a_body_exactly_at_the_bound(self) -> None:
        body = json.dumps({"jsonrpc": "2.0", "id": 1, "result": {"value": 7}}).encode()
        # JSON tolerates trailing whitespace, so pad to exactly the bound.
        body = body + b" " * (solana.MAX_RPC_RESPONSE_BYTES - len(body))
        self.assertEqual(len(body), solana.MAX_RPC_RESPONSE_BYTES)
        with _urlopen_returning(body):
            self.assertEqual(solana._rpc("getBalance", ["Addr1"], "http://rpc.test")["value"], 7)

    def test_rejects_a_body_over_the_bound_before_parsing(self) -> None:
        # One byte over: the adapter must stop reading at the bound + 1 and
        # refuse, never buffering or parsing the rest.
        oversized = _Response(b"{" + b" " * solana.MAX_RPC_RESPONSE_BYTES + b"}")
        with mock.patch("postfiat_rpc.solana.urllib.request.urlopen", return_value=oversized):
            with mock.patch("postfiat_rpc.solana.json.loads", wraps=json.loads) as loads:
                with self.assertRaisesRegex(ValueError, "exceeded the byte limit"):
                    solana._rpc("getBalance", ["Addr1"], "http://rpc.test")
        loads.assert_not_called()

    def test_reads_at_most_the_bound_plus_one_byte(self) -> None:
        response = _Response(json.dumps({"result": {"value": 1}}).encode())
        with mock.patch.object(response, "read", wraps=response.read) as read:
            with mock.patch("postfiat_rpc.solana.urllib.request.urlopen", return_value=response):
                solana._rpc("getBalance", ["Addr1"], "http://rpc.test")
        read.assert_called_once_with(solana.MAX_RPC_RESPONSE_BYTES + 1)

    def test_rejects_a_non_object_body(self) -> None:
        with _urlopen_returning(b"[1, 2, 3]"):
            with self.assertRaisesRegex(ValueError, "must be a JSON object"):
                solana._rpc("getBalance", ["Addr1"], "http://rpc.test")

    def test_surfaces_an_rpc_error_object(self) -> None:
        body = json.dumps({"jsonrpc": "2.0", "id": 1, "error": {"code": -32602, "message": "bad"}}).encode()
        with _urlopen_returning(body):
            with self.assertRaisesRegex(RuntimeError, "solana rpc error"):
                solana._rpc("getBalance", ["Addr1"], "http://rpc.test")


if __name__ == "__main__":
    unittest.main()
