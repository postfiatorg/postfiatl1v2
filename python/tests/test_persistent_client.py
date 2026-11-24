from __future__ import annotations

import json
import socket
from threading import Thread
import unittest
from unittest import mock

from postfiat_rpc.client import RPC_VERSION, RpcError, RpcProtocolError
from postfiat_rpc.persistent_client import PersistentPostFiatRpcClient
from postfiat_rpc.transparent_swap_latency import CertifiedParent, TransparentSwapFleetSession
from postfiat_rpc.transparent_swap_latency import (
    FleetPropagationDivergence,
    FleetPropagationTimeout,
    PhaseTimeline,
    wait_for_certified_parent_six,
    wait_for_certified_parent_six_independent,
)


class PersistentClientTests(unittest.TestCase):
    def test_two_calls_reuse_one_connection(self) -> None:
        listener = socket.socket()
        listener.bind(("127.0.0.1", 0))
        listener.listen()
        accepted = []

        def serve() -> None:
            stream, _address = listener.accept()
            accepted.append(True)
            with stream, stream.makefile("rwb", buffering=0) as wire:
                for _index in range(2):
                    request = json.loads(wire.readline())
                    wire.write(
                        json.dumps(
                            {
                                "version": "postfiat-local-rpc-v1",
                                "id": request["id"],
                                "ok": True,
                                "result": {"method": request["method"]},
                            }
                        ).encode()
                        + b"\n"
                    )
            listener.close()

        server = Thread(target=serve, daemon=True)
        server.start()
        client = PersistentPostFiatRpcClient(
            f"127.0.0.1:{listener.getsockname()[1]}", timeout_seconds=2
        )
        try:
            self.assertEqual(client.server_info(), {"method": "server_info"})
            self.assertEqual(client.status(), {"method": "status"})
        finally:
            client.close()
        server.join(timeout=2)
        self.assertFalse(server.is_alive())
        self.assertEqual(len(accepted), 1)


def envelope(request: dict, **overrides: object) -> bytes:
    response = {
        "version": RPC_VERSION,
        "id": request["id"],
        "ok": True,
        "result": {"method": request["method"], "params": request["params"]},
    }
    response.update(overrides)
    return json.dumps(response).encode() + b"\n"


def read_request(wire, requests: list[dict]) -> dict | None:
    line = wire.readline()
    if not line:
        return None
    request = json.loads(line)
    requests.append(request)
    return request


def drain(wire, requests: list[dict]) -> None:
    while read_request(wire, requests) is not None:
        pass


def echo_once(wire, requests: list[dict]) -> None:
    wire.write(envelope(read_request(wire, requests)))
    drain(wire, requests)


def start_peer(handlers: list) -> tuple[int, Thread, list[list[dict]]]:
    """Accept one connection per handler, in order, recording each request."""
    listener = socket.socket()
    listener.bind(("127.0.0.1", 0))
    listener.listen()
    port = listener.getsockname()[1]
    seen: list[list[dict]] = []

    def serve() -> None:
        try:
            for handler in handlers:
                stream, _address = listener.accept()
                requests: list[dict] = []
                seen.append(requests)
                with stream, stream.makefile("rwb", buffering=0) as wire:
                    handler(wire, requests)
        finally:
            listener.close()

    server = Thread(target=serve, daemon=True)
    server.start()
    return port, server, seen


class ScriptedStream:
    """Stand-in socket whose recv replays scripted chunks or raises."""

    def __init__(self, replies: list) -> None:
        self.replies = list(replies)
        self.sent: list[dict] = []
        self.closed = False

    def settimeout(self, _timeout: float) -> None:
        pass

    def sendall(self, data: bytes) -> None:
        self.sent.append(json.loads(data))

    def recv(self, _size: int) -> bytes:
        reply = self.replies.pop(0)
        if isinstance(reply, BaseException):
            raise reply
        return reply(self.sent[-1])

    def close(self) -> None:
        self.closed = True


class PersistentClientResponseBindingTests(unittest.TestCase):
    def assert_closed_then_reconnects(self, client, streams, *, request_id=None) -> None:
        self.assertIsNone(client._stream)
        self.assertTrue(streams[0].closed)
        response = client.call_response(
            "account_info", {"address": "next"}, request_id=request_id
        )
        self.assertEqual(
            response["result"], {"method": "account_info", "params": {"address": "next"}}
        )
        self.assertIs(client._stream, streams[1])

    def scripted_client(self, *streams: ScriptedStream):
        client = PersistentPostFiatRpcClient("127.0.0.1:9", timeout_seconds=2)
        patcher = mock.patch.object(socket, "create_connection", side_effect=list(streams))
        patcher.start()
        self.addCleanup(patcher.stop)
        self.addCleanup(client.close)
        return client

    def test_duplicated_response_line_closes_and_next_call_gets_own_result(self) -> None:
        def stray_duplicate(wire, requests):
            first = read_request(wire, requests)
            wire.write(envelope(first))
            read_request(wire, requests)
            wire.write(envelope(first))
            drain(wire, requests)

        port, server, seen = start_peer([stray_duplicate, echo_once])
        client = PersistentPostFiatRpcClient(f"127.0.0.1:{port}", timeout_seconds=2)
        try:
            self.assertEqual(
                client.call_response("account_info", {"address": "A"})["result"],
                {"method": "account_info", "params": {"address": "A"}},
            )
            with self.assertRaisesRegex(RpcProtocolError, "response id mismatch"):
                client.call_response("account_info", {"address": "B"})
            self.assertIsNone(client._stream)
            self.assertEqual(
                client.call_response("account_info", {"address": "C"})["result"],
                {"method": "account_info", "params": {"address": "C"}},
            )
        finally:
            client.close()
        server.join(timeout=2)
        self.assertFalse(server.is_alive())
        self.assertEqual([len(requests) for requests in seen], [2, 1])
        self.assertEqual(seen[1][0]["params"], {"address": "C"})

    def test_foreign_response_id_closes_and_next_call_reconnects(self) -> None:
        def foreign_id(wire, requests):
            wire.write(envelope(read_request(wire, requests), id="other"))
            drain(wire, requests)

        port, server, seen = start_peer([foreign_id, echo_once])
        client = PersistentPostFiatRpcClient(f"127.0.0.1:{port}", timeout_seconds=2)
        try:
            with self.assertRaisesRegex(RpcProtocolError, "response id mismatch"):
                client.call_response("account_info", {"address": "A"})
            self.assertIsNone(client._stream)
            self.assertEqual(
                client.call_response("account_info", {"address": "B"})["result"],
                {"method": "account_info", "params": {"address": "B"}},
            )
        finally:
            client.close()
        server.join(timeout=2)
        self.assertFalse(server.is_alive())
        self.assertEqual([len(requests) for requests in seen], [1, 1])

    def test_reused_request_id_is_refused_before_send_and_closes(self) -> None:
        port, server, seen = start_peer([echo_once, echo_once])
        client = PersistentPostFiatRpcClient(f"127.0.0.1:{port}", timeout_seconds=2)
        try:
            client.call_response("account_info", {"address": "A"}, request_id="fixed")
            with self.assertRaisesRegex(
                RpcProtocolError, "request id reused on this persistent session"
            ):
                client.call_response("ledger_status", {}, request_id="fixed")
            self.assertIsNone(client._stream)
            self.assertEqual(
                client.call_response("account_info", {"address": "C"}, request_id="fixed")[
                    "result"
                ],
                {"method": "account_info", "params": {"address": "C"}},
            )
        finally:
            client.close()
        server.join(timeout=2)
        self.assertFalse(server.is_alive())
        self.assertEqual(
            [[request["id"] for request in requests] for requests in seen],
            [["fixed"], ["fixed"]],
        )
        self.assertEqual(seen[0][0]["params"], {"address": "A"})

    def test_caller_id_matching_an_earlier_auto_id_is_refused(self) -> None:
        first, second = ScriptedStream([envelope]), ScriptedStream([envelope])
        client = self.scripted_client(first, second)
        auto_id = client.call_response("server_info")["id"]
        with self.assertRaisesRegex(RpcProtocolError, "request id reused"):
            client.call_response("status", request_id=auto_id)
        self.assertEqual(len(first.sent), 1)
        self.assert_closed_then_reconnects(client, [first, second], request_id=auto_id)

    def test_keyboard_interrupt_during_recv_closes_session(self) -> None:
        first, second = ScriptedStream([KeyboardInterrupt()]), ScriptedStream([envelope])
        client = self.scripted_client(first, second)
        with self.assertRaises(KeyboardInterrupt):
            client.call_response("server_info")
        self.assert_closed_then_reconnects(client, [first, second])

    def test_response_version_mismatch_closes_session(self) -> None:
        first = ScriptedStream([lambda request: envelope(request, version="v0")])
        second = ScriptedStream([envelope])
        client = self.scripted_client(first, second)
        with self.assertRaisesRegex(RpcProtocolError, "response version mismatch"):
            client.call_response("server_info")
        self.assert_closed_then_reconnects(client, [first, second])

    def test_response_id_with_another_json_type_is_a_mismatch(self) -> None:
        first = ScriptedStream([lambda request: envelope(request, id=1)])
        second = ScriptedStream([envelope])
        client = self.scripted_client(first, second)
        with self.assertRaisesRegex(RpcProtocolError, "response id mismatch"):
            client.call_response("server_info", request_id="1")
        self.assert_closed_then_reconnects(client, [first, second], request_id="1")

    def test_a_node_error_under_its_placeholder_id_is_reported_and_closes_the_session(self) -> None:
        # The node may send more than one placeholder reply for a line it could not read whole
        # (F224-8), so the session is not trusted to pair the next request.
        refusal = {"code": "rpc_protocol_error", "message": "rpc request id of 300 bytes exceeds the 256-byte limit"}
        first = ScriptedStream([lambda request: envelope(request, id="remote-1", ok=False, result=None, error=refusal)])
        second = ScriptedStream([envelope])
        client = self.scripted_client(first, second)
        with self.assertRaisesRegex(RpcError, "rpc_protocol_error"):
            client.call_response("server_info", request_id="x" * 300)
        self.assert_closed_then_reconnects(client, [first, second])

    def test_bytes_beyond_one_response_close_session(self) -> None:
        first = ScriptedStream([lambda request: envelope(request) + b'{"stray"'])
        second = ScriptedStream([envelope])
        client = self.scripted_client(first, second)
        with self.assertRaisesRegex(
            RpcProtocolError, "persistent RPC peer sent data beyond one response"
        ):
            client.call_response("server_info")
        self.assert_closed_then_reconnects(client, [first, second])

    def test_unparseable_response_closes_session(self) -> None:
        for raw in (b"not json\n", b"[]\n", b"\xff\n", b'{"id": ' + b"9" * 5000 + b"}\n"):
            with self.subTest(raw=raw[:16]):
                first = ScriptedStream([lambda _request, raw=raw: raw])
                second = ScriptedStream([envelope])
                client = self.scripted_client(first, second)
                with self.assertRaises(RpcProtocolError):
                    client.call_response("server_info")
                self.assert_closed_then_reconnects(client, [first, second])

    def test_error_envelope_bound_to_request_keeps_session(self) -> None:
        def rejected(request):
            return json.dumps(
                {
                    "version": RPC_VERSION,
                    "id": request["id"],
                    "ok": False,
                    "error": {"code": "not_found", "message": "missing"},
                }
            ).encode() + b"\n"

        stream = ScriptedStream([rejected, envelope])
        client = self.scripted_client(stream)
        with self.assertRaisesRegex(RpcError, "not_found"):
            client.call_response("account_info", {"address": "A"})
        self.assertIs(client._stream, stream)
        self.assertEqual(
            client.call_response("account_info", {"address": "B"})["result"]["params"],
            {"address": "B"},
        )
        self.assertIs(client._stream, stream)
        self.assertFalse(stream.closed)


class FakePersistentClient:
    def __init__(self, info: dict) -> None:
        self.info = info
        self.closed = False
        self.responses: list[dict] = []
        self.calls: list[tuple[str, dict, str | None]] = []

    def server_info(self) -> dict:
        if isinstance(self.info, list):
            if len(self.info) > 1:
                return self.info.pop(0)
            return self.info[0]
        return self.info

    def close(self) -> None:
        self.closed = True

    def call_response(
        self, method: str, params: dict, *, request_id: str | None = None
    ) -> dict:
        self.calls.append((method, params, request_id))
        return self.responses.pop(0)


def fleet_info(node: str, *, root: str = "root") -> dict:
    return {
        "node_id": node,
        "chain_id": "chain",
        "genesis_hash": "genesis",
        "protocol_version": 1,
        "validators": {"active_count": 6},
        "ledger": {"height": 946, "hash": "block", "state_root": root},
        "mempool": {"pending": 0},
    }


class FleetSessionTests(unittest.TestCase):
    @staticmethod
    def direct_session() -> tuple[TransparentSwapFleetSession, dict[str, FakePersistentClient]]:
        endpoints = {f"validator-{index}": f"endpoint-{index}" for index in range(6)}
        clients = {node: FakePersistentClient(fleet_info(node)) for node in endpoints}
        return (
            TransparentSwapFleetSession(
                endpoints=endpoints,
                clients=clients,  # type: ignore[arg-type]
                initial_info={node: client.info for node, client in clients.items()},
            ),
            clients,
        )

    def test_resolve_binds_identity_domain_point_and_proposer_rotation(self) -> None:
        endpoints = {f"validator-{index}": f"endpoint-{index}" for index in range(6)}
        clients = {endpoint: FakePersistentClient(fleet_info(node)) for node, endpoint in endpoints.items()}

        with TransparentSwapFleetSession.resolve(
            endpoints, client_factory=lambda endpoint: clients[endpoint]  # type: ignore[arg-type]
        ) as session:
            self.assertEqual(session.initial_parent, CertifiedParent(946, "block", "root"))
            self.assertEqual(session.proposer_for_parent(session.initial_parent), "validator-5")
            self.assertEqual(session.proposer_for_height(948), "validator-0")
        self.assertTrue(all(client.closed for client in clients.values()))

    def test_resolve_rejects_one_endpoint_identity_mismatch_and_closes_all(self) -> None:
        endpoints = {f"validator-{index}": f"endpoint-{index}" for index in range(6)}
        clients = {endpoint: FakePersistentClient(fleet_info(node)) for node, endpoint in endpoints.items()}
        clients["endpoint-3"].info["node_id"] = "validator-retired"

        with self.assertRaisesRegex(RuntimeError, "identity mismatch"):
            TransparentSwapFleetSession.resolve(
                endpoints, client_factory=lambda endpoint: clients[endpoint]  # type: ignore[arg-type]
            )
        self.assertTrue(all(client.closed for client in clients.values()))

    def test_resolve_rejects_root_mismatch(self) -> None:
        endpoints = {f"validator-{index}": f"endpoint-{index}" for index in range(6)}
        clients = {endpoint: FakePersistentClient(fleet_info(node)) for node, endpoint in endpoints.items()}
        clients["endpoint-4"].info["ledger"]["state_root"] = "fork"

        with self.assertRaisesRegex(RuntimeError, "ledger mismatch"):
            TransparentSwapFleetSession.resolve(
                endpoints, client_factory=lambda endpoint: clients[endpoint]  # type: ignore[arg-type]
            )

    def test_submit_uses_cached_proposer_and_requires_exact_parent_extension(self) -> None:
        session, clients = self.direct_session()
        clients["validator-5"].responses.append(
            {
                "result": {
                    "finality": {
                        "block": {
                            "header": {
                                "height": 947,
                                "parent_hash": "block",
                                "block_hash": "next-block",
                                "state_root": "next-root",
                                "proposer": "validator-5",
                            }
                        }
                    }
                }
            }
        )

        _result, proposer, next_parent = session.submit_asset_finality(
            {"signed": True}, session.initial_parent, request_id="submit-1"
        )

        self.assertEqual(proposer, "validator-5")
        self.assertEqual(next_parent, CertifiedParent(947, "next-block", "next-root"))
        method, params, request_id = clients["validator-5"].calls[0]
        self.assertEqual(method, "mempool_submit_signed_asset_transaction_finality")
        self.assertEqual(request_id, "submit-1")
        self.assertEqual(params["proxy_required_current_height"], 946)
        self.assertEqual(params["proxy_required_parent_hash"], "block")
        self.assertEqual(params["proxy_required_state_root"], "root")
        self.assertEqual(json.loads(params["signed_asset_transaction_json"]), {"signed": True})
        self.assertTrue(all(not client.calls for node, client in clients.items() if node != "validator-5"))

    def test_submit_rejects_finality_for_wrong_parent(self) -> None:
        session, clients = self.direct_session()
        clients["validator-5"].responses.append(
            {
                "result": {
                    "finality": {
                        "block": {
                            "header": {
                                "height": 947,
                                "parent_hash": "fork",
                                "block_hash": "next-block",
                                "state_root": "next-root",
                                "proposer": "validator-5",
                            }
                        }
                    }
                }
            }
        )

        with self.assertRaisesRegex(RuntimeError, "cached parent hash"):
            session.submit_asset_finality(
                {"signed": True}, session.initial_parent, request_id="submit-red"
            )


class FakeAdaptiveClock:
    def __init__(self) -> None:
        self.now = 0.0
        self.waits: list[float] = []

    def clock(self) -> float:
        return self.now

    def wait(self, seconds: float) -> None:
        self.waits.append(seconds)
        self.now += seconds


class FleetPropagationWaitTests(unittest.TestCase):
    @staticmethod
    def session_with_v5_rows(v5_rows: list[dict]) -> TransparentSwapFleetSession:
        endpoints = {f"validator-{index}": f"endpoint-{index}" for index in range(6)}
        clients = {
            node: FakePersistentClient(v5_rows if node == "validator-5" else fleet_info(node))
            for node in endpoints
        }
        return TransparentSwapFleetSession(
            endpoints=endpoints,
            clients=clients,  # type: ignore[arg-type]
            initial_info={node: fleet_info(node) for node in endpoints},
        )

    def test_adaptive_wait_reaches_exact_six_validator_certified_point(self) -> None:
        lagging = fleet_info("validator-5")
        lagging["ledger"] = {"height": 945, "hash": "old", "state_root": "old-root"}
        clock = FakeAdaptiveClock()
        session = self.session_with_v5_rows([lagging, fleet_info("validator-5")])
        timeline = PhaseTimeline()

        report = wait_for_certified_parent_six(
            session,
            CertifiedParent(946, "block", "root"),
            timeline,
            clock=clock.clock,
            wait=clock.wait,
        )

        self.assertEqual(report.rounds, 2)
        self.assertEqual(report.adaptive_waits_ms, (10.0,))
        self.assertEqual(clock.waits, [0.01])
        self.assertEqual(len(report.validators), 6)
        self.assertIn(
            "verify.six_certified_parent_wait",
            {record.name for record in timeline.records},
        )

    def test_target_height_with_wrong_root_fails_without_waiting(self) -> None:
        fork = fleet_info("validator-5", root="fork")
        clock = FakeAdaptiveClock()
        session = self.session_with_v5_rows([fork])

        with self.assertRaisesRegex(FleetPropagationDivergence, "conflicts"):
            wait_for_certified_parent_six(
                session,
                CertifiedParent(946, "block", "root"),
                PhaseTimeline(),
                clock=clock.clock,
                wait=clock.wait,
            )
        self.assertEqual(clock.waits, [])

    def test_malformed_height_fails_without_waiting(self) -> None:
        malformed = fleet_info("validator-5")
        malformed["ledger"]["height"] = "946"
        clock = FakeAdaptiveClock()
        session = self.session_with_v5_rows([malformed])

        with self.assertRaisesRegex(FleetPropagationDivergence, "invalid ledger height"):
            wait_for_certified_parent_six(
                session,
                CertifiedParent(946, "block", "root"),
                PhaseTimeline(),
                clock=clock.clock,
                wait=clock.wait,
            )
        self.assertEqual(clock.waits, [])

    def test_timeout_uses_increasing_bounded_waits(self) -> None:
        lagging = fleet_info("validator-5")
        lagging["ledger"] = {"height": 945, "hash": "old", "state_root": "old-root"}
        clock = FakeAdaptiveClock()
        session = self.session_with_v5_rows([lagging])

        with self.assertRaisesRegex(FleetPropagationTimeout, "timed out"):
            wait_for_certified_parent_six(
                session,
                CertifiedParent(946, "block", "root"),
                PhaseTimeline(),
                timeout_seconds=0.035,
                initial_wait_seconds=0.01,
                max_wait_seconds=0.1,
                clock=clock.clock,
                wait=clock.wait,
            )
        self.assertEqual(clock.waits, [0.01, 0.02, 0.0050000000000000044])

    def test_independent_watchers_stop_querying_converged_validators(self) -> None:
        lagging = fleet_info("validator-5")
        lagging["ledger"] = {"height": 945, "hash": "old", "state_root": "old-root"}
        session = self.session_with_v5_rows([lagging, fleet_info("validator-5")])

        report = wait_for_certified_parent_six_independent(
            session,
            CertifiedParent(946, "block", "root"),
            PhaseTimeline(),
            initial_wait_seconds=0.0001,
            max_wait_seconds=0.0002,
        )

        self.assertEqual(report.validators["validator-5"]["attempts"], 2)
        self.assertTrue(
            all(
                report.validators[f"validator-{index}"]["attempts"] == 1
                for index in range(5)
            )
        )

    def test_independent_watcher_conflict_fails_closed(self) -> None:
        session = self.session_with_v5_rows([fleet_info("validator-5", root="fork")])

        with self.assertRaisesRegex(FleetPropagationDivergence, "conflicts"):
            wait_for_certified_parent_six_independent(
                session,
                CertifiedParent(946, "block", "root"),
                PhaseTimeline(),
                initial_wait_seconds=0.0001,
                max_wait_seconds=0.0002,
            )


if __name__ == "__main__":
    unittest.main()
