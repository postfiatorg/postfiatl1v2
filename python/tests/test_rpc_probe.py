from __future__ import annotations

import json
import socket
from threading import Thread
import time
import unittest

from postfiat_rpc.client import Endpoint, RPC_VERSION
from postfiat_rpc.rpc_probe import RpcProbeError, main, rpc_probe


class RpcProbeTests(unittest.TestCase):
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
