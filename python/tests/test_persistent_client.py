from __future__ import annotations

import json
import socket
from threading import Thread
import unittest

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
