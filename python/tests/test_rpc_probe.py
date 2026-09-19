from __future__ import annotations

import json
import socket
from threading import Thread
import time
import unittest
from unittest.mock import patch

from postfiat_rpc.client import Endpoint, RPC_VERSION
from postfiat_rpc.rpc_probe import RpcProbeError, main, rpc_probe


class RpcProbeTests(unittest.TestCase):
    def test_probe_tries_next_resolved_address_when_first_connect_refuses(self) -> None:
        addresses = [
            (socket.AF_INET6, socket.SOCK_STREAM, 6, "", ("::1", 27650, 0, 0)),
            (socket.AF_INET, socket.SOCK_STREAM, 6, "", ("127.0.0.1", 27650)),
        ]
        response = json.dumps({
            "version": RPC_VERSION, "id": "rpc-probe", "ok": True,
            "result": {"block_height": 42, "block_tip_hash": "abcdef0123456789"},
            "error": None, "events": [],
        }).encode() + b"\n"
        attempts = []
        sent = []

        class FakeSocket:
            def __enter__(self):
                return self

            def __exit__(self, *_args):
                return False

            def settimeout(self, _timeout):
                pass

            def close(self):
                pass

            def connect(self, address):
                attempts.append(address)
                if address == addresses[0][4]:
                    raise ConnectionRefusedError("first address refused")

            def sendall(self, wire):
                sent.append(json.loads(wire))

            def shutdown(self, _how):
                pass

            def recv(self, _size):
                return response

        with patch("postfiat_rpc.rpc_probe.socket.getaddrinfo", return_value=addresses), patch(
            "postfiat_rpc.rpc_probe.socket.socket", side_effect=lambda *_args: FakeSocket()
        ):
            result = rpc_probe(Endpoint("localhost", 27650), 2)

        self.assertEqual(attempts, [addresses[0][4], addresses[1][4]])
        self.assertEqual(len(sent), 1)
        self.assertEqual(sent[0]["method"], "status")
        self.assertEqual(result.height, 42)

    def test_probe_reports_refusal_after_all_resolved_addresses_fail(self) -> None:
        addresses = [
            (socket.AF_INET6, socket.SOCK_STREAM, 6, "", ("::1", 27650, 0, 0)),
            (socket.AF_INET, socket.SOCK_STREAM, 6, "", ("127.0.0.1", 27650)),
        ]
        attempts = []
        closed = []

        class RefusingSocket:
            def settimeout(self, _timeout):
                pass

            def connect(self, address):
                attempts.append(address)
                raise ConnectionRefusedError("synthetic refusal")

            def close(self):
                closed.append(True)

        with patch("postfiat_rpc.rpc_probe.socket.getaddrinfo", return_value=addresses), patch(
            "postfiat_rpc.rpc_probe.socket.socket", side_effect=lambda *_args: RefusingSocket()
        ):
            with self.assertRaisesRegex(RpcProbeError, "connect refused: synthetic refusal"):
                rpc_probe(Endpoint("localhost", 27650), 2)

        self.assertEqual(attempts, [addresses[0][4], addresses[1][4]])
        self.assertEqual(len(closed), 2)

    def test_invalid_timeout_closes_connect_attempt(self) -> None:
        closed = []

        class RejectingSocket:
            def settimeout(self, _timeout):
                raise ValueError("invalid timeout")

            def close(self):
                closed.append(True)

        addresses = [(socket.AF_INET, socket.SOCK_STREAM, 6, "", ("127.0.0.1", 27650))]
        with patch("postfiat_rpc.rpc_probe.socket.getaddrinfo", return_value=addresses), patch(
            "postfiat_rpc.rpc_probe.socket.socket", return_value=RejectingSocket()
        ):
            with self.assertRaisesRegex(ValueError, "invalid timeout"):
                rpc_probe(Endpoint("localhost", 27650), 2)

        self.assertEqual(closed, [True])

    def test_status_round_trip_uses_one_connection_and_shuts_down_write_side(self) -> None:
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            listener.listen(1)
            port = listener.getsockname()[1]
            requests = []

            def serve() -> None:
                with listener.accept()[0] as stream:
                    stream.settimeout(2)
                    with stream.makefile("rb") as wire:
                        requests.append(json.loads(wire.readline()))
                        self.assertEqual(wire.readline(), b"")
                    response = {
                        "version": RPC_VERSION, "id": requests[0]["id"], "ok": True,
                        "result": {"block_height": 42, "block_tip_hash": "abcdef0123456789"},
                        "error": None, "events": [],
                    }
                    stream.sendall(json.dumps(response).encode() + b"\n")

            server = Thread(target=serve)
            server.start()
            result = rpc_probe(Endpoint("127.0.0.1", port), 2)
            server.join(timeout=2)
            self.assertFalse(server.is_alive())
            self.assertEqual(requests[0]["method"], "status")
            self.assertEqual(requests[0]["params"], {})
            self.assertEqual(result.height, 42)
            self.assertEqual(result.tip_prefix, "abcdef012345")
            self.assertLess(result.round_trip_ms, 2000)

    def test_accepted_peer_with_no_reply_exits_with_timeout(self) -> None:
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            listener.listen(1)
            port = listener.getsockname()[1]

            def serve() -> None:
                with listener.accept()[0] as stream:
                    stream.settimeout(2)
                    with stream.makefile("rb") as wire:
                        request = json.loads(wire.readline())
                    self.assertEqual(request["method"], "status")
                    time.sleep(0.25)

            server = Thread(target=serve)
            server.start()
            with self.assertRaisesRegex(RpcProbeError, "no response within timeout"):
                rpc_probe(Endpoint("127.0.0.1", port), 0.08)
            server.join(timeout=2)
            self.assertFalse(server.is_alive())

    def test_cli_reports_refused_connection_with_nonzero_exit(self) -> None:
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            port = listener.getsockname()[1]
        self.assertEqual(main(["--endpoint", f"127.0.0.1:{port}", "--timeout-seconds", "0.1"]), 1)


if __name__ == "__main__":
    unittest.main()
