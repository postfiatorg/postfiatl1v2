"""Python client wrappers for the archive-window and verifier public reads."""

from __future__ import annotations

import unittest
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


if __name__ == "__main__":
    unittest.main()
