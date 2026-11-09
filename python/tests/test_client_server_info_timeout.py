"""`server_info` gets a longer read timeout than the client default."""
from __future__ import annotations

import unittest
from unittest import mock

from postfiat_rpc import PostFiatRpcClient
from postfiat_rpc import client as client_module


class _Stop(Exception):
    pass


class ServerInfoTimeoutTests(unittest.TestCase):
    def _timeout_used(self, client: PostFiatRpcClient, method: str) -> float:
        with mock.patch.object(
            client_module.socket, "create_connection", side_effect=_Stop
        ) as create:
            with self.assertRaises(_Stop):
                getattr(client, method)()
        return create.call_args.kwargs["timeout"]

    def test_server_info_uses_the_longer_timeout(self):
        client = PostFiatRpcClient("127.0.0.1:1234")
        self.assertEqual(client_module.SERVER_INFO_TIMEOUT_SECONDS, 30.0)
        self.assertEqual(self._timeout_used(client, "server_info"), 30.0)
        self.assertEqual(
            self._timeout_used(client, "status"), client_module.DEFAULT_TIMEOUT_SECONDS
        )

    def test_a_longer_client_timeout_is_kept(self):
        client = PostFiatRpcClient("127.0.0.1:1234", timeout_seconds=60)
        self.assertEqual(self._timeout_used(client, "server_info"), 60)


class WebSocketServerInfoTimeoutTests(unittest.TestCase):
    def _recv_timeout(self, client, method: str) -> float:
        timeouts = []

        class _FakeWebSocket:
            def send(self, wire):
                pass

            def recv(self, timeout=None):
                timeouts.append(timeout)
                raise _Stop

        with mock.patch.object(client, "_connect_websocket", return_value=_FakeWebSocket()):
            with self.assertRaises(Exception):
                getattr(client, method)()
        self.assertEqual(len(timeouts), 1)
        return timeouts[0]

    def test_websocket_server_info_uses_the_longer_timeout(self):
        client = client_module.PostFiatWebSocketRpcClient("ws://127.0.0.1:1234")
        self.assertEqual(self._recv_timeout(client, "server_info"), 30.0)
        client = client_module.PostFiatWebSocketRpcClient("ws://127.0.0.1:1234")
        self.assertEqual(
            self._recv_timeout(client, "status"), client_module.DEFAULT_TIMEOUT_SECONDS
        )


if __name__ == "__main__":
    unittest.main()
