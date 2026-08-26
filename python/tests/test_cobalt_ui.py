from __future__ import annotations

import http.client
import json
import tempfile
import threading
import unittest
from pathlib import Path
from unittest import mock

from postfiat_rpc import cobalt, cobalt_ui


def example_report(spec: cobalt.ExampleSpec) -> dict:
    if spec.command == "trust-graph":
        return {
            "cobalt_mode": "non_uniform",
            "active_graph": "G1",
            "g1_trust_view_count": 7,
            "g1_activation_height": 30,
            "g1_non_identical_trust_views": True,
            "trust_graph_root": "ab" * 48,
        }
    return {
        "ok": True,
        "status": "passed",
        "scenario_hash": "cd" * 48,
        "scenarios": [{"name": "bounded", "ok": True}],
    }


def shadow_report(path: Path) -> dict:
    return {
        "node_id": path.name,
        "authority_mode": "shadow-advisory",
        "live_authority": False,
        "controls_block_consensus": False,
        "transport_healthy": True,
        "catch_up_status": "current",
        "contiguous_sequence": 0,
        "protocol_decision_count": 0,
        "certificate_signer_count": 0,
        "governance_digest": "ef" * 48,
        "accepted_messages": 3,
        "queue_depth": 0,
        "boot_count": 1,
        "peer_count": 4,
    }


class CobaltUiTests(unittest.TestCase):
    def make_collector(self, root: Path) -> cobalt_ui.SnapshotCollector:
        repository = cobalt.repository_root(Path(__file__))
        node_dir = root / "node"
        shadow_root = root / "shadow"
        node_dir.mkdir()
        shadow_root.mkdir()
        for index in range(4):
            validator = shadow_root / f"validator-{index}"
            validator.mkdir()
            (validator / "state.json").write_text("{}", encoding="utf-8")
        (node_dir / "governance.json").write_text(
            json.dumps(
                {
                    "active_validator_count": 4,
                    "authority_mode": 0,
                    "validator_registry_updates": [],
                    "cobalt_authority_transitions": [],
                    "amendments": [],
                }
            ),
            encoding="utf-8",
        )
        return cobalt_ui.SnapshotCollector(
            cobalt_ui.CollectorOptions(
                root=root,
                node_data_dir=node_dir,
                shadow_root=shadow_root,
                benchmark_packet=repository
                / "benchmarks/cobalt-rippled-liveness/packet",
                handoff_packet=repository
                / "benchmarks/cobalt-handoff-rehearsal/packet",
                benchmark_manifest_sha256=cobalt.DEFAULT_BENCHMARK_PACKET_SHA256,
                handoff_manifest_sha256=cobalt.DEFAULT_HANDOFF_PACKET_SHA256,
                handoff_verifier_sha256=cobalt.DEFAULT_HANDOFF_VERIFIER_SHA256,
                cargo="cargo",
                target=None,
                timeout_seconds=1,
            ),
            example_runner=example_report,
            shadow_runner=shadow_report,
            node_runner=lambda: {
                "chain_id": "cobalt-ui-test",
                "block_height": 0,
                "state_root": "01" * 48,
                "node_id": "validator-0",
            },
        )

    def test_snapshot_separates_ready_evidence_from_foundation_authority(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            snapshot = self.make_collector(Path(directory)).collect()

        self.assertTrue(snapshot["read_only"])
        self.assertTrue(snapshot["trust"]["ok"])
        self.assertEqual(snapshot["proposals"]["authority_label"], "Foundation registry")
        self.assertEqual(snapshot["shadow_health"]["node_count"], 4)
        self.assertTrue(snapshot["shadow_health"]["converged"])
        self.assertEqual(snapshot["rehearsal_readiness"]["status"], "GO")
        self.assertTrue(snapshot["rehearsal_readiness"]["ready"])
        self.assertFalse(snapshot["rehearsal_readiness"]["activation_performed"])
        self.assertTrue(snapshot["actual_authority"]["foundation_active"])
        self.assertFalse(snapshot["actual_authority"]["cobalt_active"])
        self.assertEqual(snapshot["actual_authority"]["block_finality"], "consensus-v2")
        self.assertEqual(snapshot["scenario"]["case_count"], 80)

    def test_recorded_handoff_changes_actual_authority_not_readiness(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            collector = self.make_collector(Path(directory))
            governance_path = collector.options.node_data_dir / "governance.json"
            governance = json.loads(governance_path.read_text(encoding="utf-8"))
            governance["authority_mode"] = 1
            governance["cobalt_authority_transitions"] = [
                {
                    "transition_id": "12" * 48,
                    "from_authority_mode": 0,
                    "to_authority_mode": 1,
                    "amendment_sequence": 1,
                    "activation_height": 30,
                }
            ]
            governance_path.write_text(json.dumps(governance), encoding="utf-8")
            snapshot = collector.collect()

        self.assertEqual(snapshot["rehearsal_readiness"]["status"], "ACTIVATED")
        self.assertTrue(snapshot["rehearsal_readiness"]["ready"])
        self.assertTrue(snapshot["rehearsal_readiness"]["activation_performed"])
        self.assertTrue(snapshot["actual_authority"]["cobalt_active"])
        self.assertFalse(snapshot["actual_authority"]["foundation_active"])
        self.assertEqual(snapshot["proposals"]["transition_count"], 1)

    def test_activation_status_collector_renders_live_authority(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            status_path = Path(directory) / "activation-status.json"
            status_path.write_text(
                json.dumps(
                    {
                        "schema": cobalt.CLI_SCHEMA,
                        "command": "live-status",
                        "ok": True,
                        "status": "ACTIVATED",
                        "terminal_decision": "ACTIVATE",
                        "authority": {"mode": "cobalt-validator-trust"},
                        "block_finality": "consensus-v2",
                        "node": {
                            "node_id": "validator-0",
                            "chain_id": "cobalt-ui-test",
                            "height": 919,
                            "state_root": "01" * 48,
                        },
                        "trust_graph_root": "02" * 48,
                        "latest_transition": {
                            "transition_id": "03" * 48,
                            "transition_kind": "activate_cobalt",
                            "activation_height": 916,
                        },
                        "latest_registry_update": {
                            "update_id": "04" * 48,
                            "operation": "rotate_key",
                            "subject_node_id": "validator-5",
                            "activation_height": 917,
                        },
                        "transition_history": [{"transition_id": "03" * 48}],
                        "verifier": {
                            "verified": True,
                            "authority_mode": 1,
                            "cobalt_mode": "non_uniform",
                            "active_validator_count": 6,
                            "validator_registry_update_count": 1,
                            "amendment_count": 19,
                        },
                        "sidecars": [
                            {
                                "node_id": "validator-0",
                                "transport_healthy": True,
                                "catch_up_status": "current",
                                "controls_block_consensus": False,
                                "state_hash": "05" * 48,
                            }
                        ],
                        "checks": [{"key": "authority_mode", "label": "active", "ok": True}],
                    }
                ),
                encoding="utf-8",
            )
            snapshot = cobalt_ui.ActivationStatusCollector(status_path).collect()

        self.assertEqual(snapshot["rehearsal_readiness"]["status"], "ACTIVATED")
        self.assertTrue(snapshot["rehearsal_readiness"]["activation_performed"])
        self.assertTrue(snapshot["actual_authority"]["cobalt_active"])
        self.assertEqual(snapshot["actual_authority"]["block_finality"], "consensus-v2")
        self.assertEqual(snapshot["proposals"]["node_status"]["block_height"], 919)

    @mock.patch("postfiat_rpc.cobalt_ui.cobalt.adversarial_result")
    def test_adversarial_packet_collector_renders_read_only_gate(
        self, adversarial_result: mock.Mock
    ) -> None:
        adversarial_result.return_value = {
            "status": "KEEP_ACTIVE",
            "ok": True,
            "campaign_complete": True,
            "scope": "protocol capability",
            "block_finality": "consensus-v2",
            "checks": [{"key": "final_gate_keep_active", "ok": True}],
            "experiments": {
                f"E{index}": {"status": "passed", "summary": "passed"}
                for index in range(1, 7)
            },
            "rejected_cases": [
                {
                    "experiment": "E5",
                    "name": "stolen_key_rotation",
                    "rejected": True,
                    "durable_state_unchanged": True,
                    "reason": "quorum missing",
                }
            ],
            "claims": {
                "protocol_capability_only": True,
                "operator_decentralization_proven": False,
                "proposal_origin": "Foundation-administered validators",
            },
            "live_authority": {
                "chain_id": "fixture-chain",
                "height": 922,
                "state_root": "11" * 48,
                "registry_root": "22" * 48,
                "trust_graph_root": "33" * 48,
                "trust_model": "uniform full overlap",
                "trust_graph_profile": "six validator canonical views",
                "trust_view_count": 6,
                "non_identical_trust_views": False,
                "validator_count": 6,
                "all_six_converged": True,
                "block_finality": "consensus-v2",
                "fleet": [
                    {"node_id": f"validator-{index}"} for index in range(6)
                ],
                "authority_transitions": [
                    {
                        "kind": "return_to_cobalt",
                        "transition_id": "44" * 48,
                        "proposal_identity": "validator-3",
                        "authorization_identities": [
                            f"validator-{index}" for index in range(5)
                        ],
                        "height": 921,
                        "accepted": True,
                    }
                ],
                "legitimate_rotation": {
                    "update_id": "55" * 48,
                    "subject_node_id": "validator-5",
                    "proposal_identity": "validator-4",
                    "authorization_identities": [
                        f"validator-{index}" for index in range(5)
                    ],
                    "height": 922,
                    "accepted": True,
                },
            },
        }

        snapshot = cobalt_ui.AdversarialPacketCollector(
            Path("/evidence/packet"), "aa" * 32
        ).collect()

        self.assertTrue(snapshot["read_only"])
        self.assertEqual(snapshot["adversarial"]["gate"], "KEEP_ACTIVE")
        self.assertEqual(snapshot["adversarial"]["experiment_pass_count"], 6)
        self.assertEqual(snapshot["adversarial"]["rejected_case_count"], 1)
        self.assertEqual(snapshot["scenario"]["mode"], "adversarial")
        self.assertEqual(snapshot["scenario"]["rejected_count"], 1)
        self.assertEqual(snapshot["scenario"]["mutation_count"], 0)
        self.assertEqual(snapshot["trust"]["mode"], "uniform full overlap")
        self.assertFalse(snapshot["trust"]["non_identical_views"])
        self.assertFalse(
            snapshot["adversarial"]["operator_decentralization_proven"]
        )
        self.assertTrue(snapshot["actual_authority"]["cobalt_active"])

    def test_mixed_shadow_digests_fail_convergence(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            collector = self.make_collector(Path(directory))

            def split_report(path: Path) -> dict:
                report = shadow_report(path)
                report["governance_digest"] = path.name
                return report

            collector.shadow_runner = split_report
            snapshot = collector.collect()

        self.assertFalse(snapshot["shadow_health"]["converged"])
        self.assertFalse(snapshot["shadow_health"]["ok"])
        self.assertEqual(snapshot["rehearsal_readiness"]["status"], "GO")

    def test_http_surface_allows_reads_and_rejects_mutation(self) -> None:
        class FixedCache:
            def get(self, *, force: bool = False) -> dict:
                return {"schema": cobalt_ui.UI_SCHEMA, "read_only": True, "force": force}

        server = cobalt_ui.CobaltUiServer(("127.0.0.1", 0), cobalt_ui.CobaltUiHandler)
        server.cache = FixedCache()  # type: ignore[assignment]
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            connection = http.client.HTTPConnection(*server.server_address, timeout=2)
            connection.request("GET", "/api/snapshot?refresh=1")
            response = connection.getresponse()
            payload = json.loads(response.read())
            self.assertEqual(response.status, 200)
            self.assertTrue(payload["read_only"])
            self.assertTrue(payload["force"])
            self.assertIn("default-src 'self'", response.getheader("content-security-policy"))
            connection.close()

            connection = http.client.HTTPConnection(*server.server_address, timeout=2)
            connection.request("POST", "/api/snapshot", body=b"{}")
            response = connection.getresponse()
            response.read()
            self.assertEqual(response.status, 405)
            connection.close()
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=2)

    def test_governance_reader_rejects_oversized_state(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "governance.json"
            path.write_text("{}", encoding="utf-8")
            with self.assertRaisesRegex(cobalt.CobaltCliError, "read limit"):
                cobalt_ui.read_bounded_bytes(path, limit=1)

    def test_governance_reader_accepts_valid_node_integrity_frame(self) -> None:
        payload = b'{"authority_mode":0,"amendments":[]}'
        raw = payload + b"\npftmac1:" + (b"a" * 96) + b"\n"

        decoded = cobalt_ui.decode_node_json(Path("governance.json"), raw)

        self.assertEqual(decoded["authority_mode"], 0)


if __name__ == "__main__":
    unittest.main()
