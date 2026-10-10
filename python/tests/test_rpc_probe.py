from __future__ import annotations

import json
import socket
from threading import Thread
import time
import unittest
from unittest import mock

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


STATUS_REPLY = json.dumps(
    {
        "version": RPC_VERSION,
        "id": "rpc-probe",
        "ok": True,
        "result": {"block_height": 7, "block_tip_hash": "0123456789abcdef0123"},
        "error": None,
        "events": [],
    }
).encode() + b"\n"

V6 = (socket.AF_INET6, socket.SOCK_STREAM, 6, "", ("::1", 27650, 0, 0))
V4 = (socket.AF_INET, socket.SOCK_STREAM, 6, "", ("127.0.0.1", 27650))


class _ScriptedSocket:
    """A socket double whose connect() outcome is chosen per address."""

    def __init__(self, log: dict, outcomes: dict):
        self._log = log
        self._outcomes = outcomes
        self._sent = bytearray()

    def __enter__(self):
        return self

    def __exit__(self, *_exc):
        self.close()
        return False

    def settimeout(self, timeout):
        self._log.setdefault("timeouts", []).append(timeout)
        if timeout is not None and timeout <= 0:
            raise ValueError("Timeout value out of range")

    def connect(self, address):
        self._log.setdefault("attempts", []).append(address)
        outcome = self._outcomes.get(address)
        if isinstance(outcome, BaseException):
            raise outcome

    def close(self):
        self._log["closed"] = self._log.get("closed", 0) + 1

    def sendall(self, wire):
        self._sent.extend(wire)
        self._log.setdefault("sent", []).append(json.loads(wire))

    def shutdown(self, _how):
        pass

    def recv(self, _size):
        return STATUS_REPLY


class RpcProbeResolvedAddressTests(unittest.TestCase):
    """The host may resolve to several addresses; the probe must not report
    the endpoint down because the first one refuses."""

    def _probe(self, candidates, outcomes, timeout=2.0):
        log: dict = {}
        with (
            mock.patch("postfiat_rpc.rpc_probe.socket.getaddrinfo", return_value=candidates),
            mock.patch(
                "postfiat_rpc.rpc_probe.socket.socket",
                side_effect=lambda *_a, **_k: _ScriptedSocket(log, outcomes),
            ),
        ):
            return rpc_probe(Endpoint("localhost", 27650), timeout), log

    def test_second_address_is_tried_when_the_first_refuses(self) -> None:
        result, log = self._probe(
            [V6, V4], {V6[4]: ConnectionRefusedError("[Errno 111] Connection refused")}
        )
        self.assertEqual(log["attempts"], [V6[4], V4[4]])
        self.assertEqual(len(log["sent"]), 1, "exactly one status request")
        self.assertEqual(log["sent"][0]["method"], "status")
        self.assertEqual(result.height, 7)
        self.assertEqual(result.tip_prefix, "0123456789ab")
        # the refused socket plus the successful one, closed by the context manager
        self.assertEqual(log["closed"], 2)

    def test_all_addresses_refusing_reports_the_last_refusal(self) -> None:
        with self.assertRaisesRegex(RpcProbeError, r"^connect refused: \[Errno 111\] second$"):
            self._probe(
                [V6, V4],
                {
                    V6[4]: ConnectionRefusedError("[Errno 111] first"),
                    V4[4]: ConnectionRefusedError("[Errno 111] second"),
                },
            )

    def test_every_failed_socket_is_closed(self) -> None:
        log: dict = {}
        outcomes = {V6[4]: ConnectionRefusedError("a"), V4[4]: ConnectionRefusedError("b")}
        with (
            mock.patch("postfiat_rpc.rpc_probe.socket.getaddrinfo", return_value=[V6, V4]),
            mock.patch(
                "postfiat_rpc.rpc_probe.socket.socket",
                side_effect=lambda *_a, **_k: _ScriptedSocket(log, outcomes),
            ),
        ):
            with self.assertRaises(RpcProbeError):
                rpc_probe(Endpoint("localhost", 27650), 2.0)
        self.assertEqual(log["attempts"], [V6[4], V4[4]])
        self.assertEqual(log["closed"], 2)

    def test_timeout_on_the_last_address_is_reported_as_connect_timeout(self) -> None:
        with self.assertRaisesRegex(RpcProbeError, "^connect timeout$"):
            self._probe([V6, V4], {V6[4]: ConnectionRefusedError("x"), V4[4]: socket.timeout()})

    def test_timeout_budget_is_shared_across_addresses(self) -> None:
        # monotonic(): started, remaining before attempt 1, remaining before attempt 2
        clock = iter([100.0, 100.0, 102.5])
        log: dict = {}
        outcomes = {V6[4]: ConnectionRefusedError("first refused")}
        with (
            mock.patch("postfiat_rpc.rpc_probe.time.monotonic", side_effect=lambda: next(clock)),
            mock.patch("postfiat_rpc.rpc_probe.socket.getaddrinfo", return_value=[V6, V4]),
            mock.patch(
                "postfiat_rpc.rpc_probe.socket.socket",
                side_effect=lambda *_a, **_k: _ScriptedSocket(log, outcomes),
            ),
        ):
            with self.assertRaisesRegex(RpcProbeError, "^connect timeout$"):
                rpc_probe(Endpoint("localhost", 27650), 2.0)
        # the budget was spent on the first attempt; the second address is never dialled
        self.assertEqual(log["attempts"], [V6[4]])
        self.assertEqual(log["timeouts"], [2.0])

    def test_resolver_order_is_preserved(self) -> None:
        _result, log = self._probe([V4, V6], {})
        self.assertEqual(log["attempts"], [V4[4]])

    def test_invalid_timeout_closes_the_socket_and_propagates(self) -> None:
        log: dict = {}
        with (
            mock.patch("postfiat_rpc.rpc_probe.time.monotonic", side_effect=[100.0, 100.0]),
            mock.patch("postfiat_rpc.rpc_probe.socket.getaddrinfo", return_value=[V4]),
            mock.patch(
                "postfiat_rpc.rpc_probe.socket.socket",
                side_effect=lambda *_a, **_k: _ScriptedSocket(log, {}),
            ),
            mock.patch.object(_ScriptedSocket, "settimeout", side_effect=ValueError("bad timeout")),
        ):
            with self.assertRaisesRegex(ValueError, "bad timeout"):
                rpc_probe(Endpoint("localhost", 27650), 2.0)
        self.assertEqual(log["closed"], 1)

    def test_empty_resolution_is_a_refusal(self) -> None:
        with mock.patch("postfiat_rpc.rpc_probe.socket.getaddrinfo", return_value=[]):
            with self.assertRaisesRegex(RpcProbeError, "address resolution failed: no addresses"):
                rpc_probe(Endpoint("localhost", 27650), 2.0)
