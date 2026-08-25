from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from postfiat_rpc import cobalt


def report_for(spec: cobalt.ExampleSpec) -> dict:
    if spec.command == "trust-graph":
        return {
            "schema": "postfiat-cobalt-current-trust-graph-root-v1",
            "cobalt_mode": "non_uniform",
            "active_graph": "G1",
            "g1_trust_view_count": 7,
            "g1_activation_height": 30,
            "trust_graph_root": "ab" * 48,
        }
    return {
        "schema": f"fixture-{spec.command}-v1",
        "ok": True,
        "status": "passed",
        "scenarios": [{"name": "fixture", "ok": True, "reason": "accepted"}],
    }


class CobaltCliTests(unittest.TestCase):
    def test_protocol_replay_is_explicitly_feature_gated(self) -> None:
        command = cobalt.cargo_command(
            cobalt.EXAMPLES["protocol-replay"],
            cargo="/tools/cargo",
            target="x86_64-unknown-linux-musl",
        )

        self.assertEqual(command[0], "/tools/cargo")
        self.assertIn("--locked", command)
        self.assertEqual(command[-2:], ["--features", "cobalt-unsafe-simulation"])
        self.assertIn("x86_64-unknown-linux-musl", command)

    def test_result_is_advisory_and_never_claims_live_authority(self) -> None:
        spec = cobalt.EXAMPLES["transition-witness"]

        result = cobalt.result_envelope(spec, report_for(spec))

        self.assertTrue(result["ok"])
        self.assertEqual(result["authority"]["mode"], "advisory")
        self.assertFalse(result["authority"]["live"])
        self.assertFalse(result["authority"]["controls_block_consensus"])
        self.assertFalse(result["authority"]["writes_validator_registry"])

    def test_shadow_readiness_runs_all_three_real_example_boundaries(self) -> None:
        seen: list[str] = []

        def runner(spec: cobalt.ExampleSpec) -> dict:
            seen.append(spec.example)
            return report_for(spec)

        result = cobalt.execute("shadow-readiness", runner)

        self.assertTrue(result["ok"])
        self.assertEqual(result["summary"]["passed"], 3)
        self.assertEqual(result["summary"]["total"], 3)
        self.assertFalse(result["summary"]["live_authority"])
        self.assertEqual(seen, [spec.example for spec in cobalt.EXAMPLES.values()])

    def test_human_output_states_scope_and_summarizes_scenarios(self) -> None:
        spec = cobalt.EXAMPLES["protocol-replay"]
        result = cobalt.result_envelope(spec, report_for(spec))

        rendered = cobalt.render_human(result)

        self.assertIn("Status: PASS", rendered)
        self.assertIn("Authority: advisory only", rendered)
        self.assertIn("Scenarios: 1", rendered)
        self.assertIn("[PASS] fixture", rendered)

    def test_shadow_status_requires_advisory_scope_and_healthy_transport(self) -> None:
        report = {
            "authority_mode": "shadow-advisory",
            "live_authority": False,
            "controls_block_consensus": False,
            "transport_healthy": True,
            "signer_private_key_loaded": True,
        }

        result = cobalt.shadow_result("shadow-service-status", report)

        self.assertTrue(result["ok"])
        self.assertFalse(result["authority"]["live"])
        report["live_authority"] = True
        self.assertFalse(cobalt.shadow_result("shadow-service-status", report)["ok"])

    def test_shadow_drill_human_output_includes_signer_transport_and_faults(self) -> None:
        result = cobalt.shadow_result(
            "shadow-service-drill",
            {
                "ok": True,
                "status": "passed",
                "validator_count": 4,
                "active_contributor_count": 3,
                "common_randomness_hash": "ab" * 48,
                "converged_governance_digest": "cd" * 48,
                "checks": {
                    "bounded_transport": True,
                    "restart_recovered_queue": True,
                },
            },
        )

        rendered = cobalt.render_human(result)

        self.assertIn("Validators: 4", rendered)
        self.assertIn("[PASS] bounded transport", rendered)
        self.assertIn("[PASS] restart recovered queue", rendered)
        self.assertIn("Live authority: no", rendered)

    def test_runtime_probe_and_fleet_require_consistent_non_authoritative_state(self) -> None:
        probe = {
            "node_id": "validator-0",
            "peer_health": "healthy",
            "queue_health": "healthy",
            "replay_posture": "consistent",
            "catch_up_status": "current",
            "registry_root": "aa" * 48,
            "trust_graph_root": "bb" * 48,
            "live_authority": False,
            "controls_block_consensus": False,
        }

        self.assertTrue(cobalt.runtime_result("probe", probe)["ok"])
        fleet = cobalt.fleet_result(
            ["127.0.0.1:9700", "127.0.0.1:9701"], [probe, dict(probe)]
        )
        self.assertTrue(fleet["ok"])
        self.assertTrue(fleet["summary"]["consistent_roots"])
        changed = dict(probe, trust_graph_root="cc" * 48)
        self.assertFalse(
            cobalt.fleet_result(
                ["127.0.0.1:9700", "127.0.0.1:9701"], [probe, changed]
            )["ok"]
        )

    def test_runtime_snapshot_and_replay_human_output_are_plain_english(self) -> None:
        snapshot = cobalt.runtime_result(
            "snapshot",
            {
                "identity": {"node_id": "validator-0"},
                "authority_mode": "shadow-advisory",
                "live_authority": False,
                "controls_block_consensus": False,
                "registry_root": "aa" * 48,
                "trust_graph_root": "bb" * 48,
                "protocol_high_watermark": 4,
                "protocol_decisions": {"4": {}},
                "state_hash": "cc" * 48,
            },
        )
        replay = cobalt.runtime_result(
            "replay", [{"round": 4, "ratification_id": "dd" * 48}]
        )

        self.assertIn("Protocol high-water mark: 4", cobalt.render_human(snapshot))
        self.assertIn("round 4", cobalt.render_human(replay))

    def test_scenario_command_authenticates_matched_packet(self) -> None:
        root = cobalt.repository_root(Path(__file__))

        result = cobalt.scenario_result(
            root / "benchmarks/cobalt-rippled-liveness/packet"
        )

        self.assertTrue(result["ok"])
        self.assertEqual(result["summary"]["case_count"], 80)
        self.assertEqual(result["summary"]["cobalt_passed"], 80)
        self.assertEqual(result["summary"]["rippled_passed"], 80)
        self.assertEqual(result["summary"]["cobalt_conflicting_decisions"], 0)
        self.assertTrue(result["summary"]["cobalt_replay_equal"])
        self.assertFalse(result["summary"]["unresolved_methodology_exception"])
        rendered = cobalt.render_human(result)
        self.assertIn("Cases: 80", rendered)
        self.assertIn("Methodology exception: none", rendered)

    def test_readiness_is_go_without_claiming_cobalt_is_active(self) -> None:
        root = cobalt.repository_root(Path(__file__))

        result = cobalt.readiness_result(
            root / "benchmarks/cobalt-rippled-liveness/packet",
            root / "benchmarks/cobalt-handoff-rehearsal/packet",
        )

        self.assertTrue(result["ok"])
        self.assertEqual(result["status"], "GO")
        self.assertFalse(result["activation_performed"])
        self.assertEqual(result["actual_authority"]["validator_trust"], "foundation")
        self.assertFalse(result["actual_authority"]["cobalt_active"])
        self.assertEqual(result["actual_authority"]["block_finality"], "consensus-v2")
        self.assertIn("Activation performed by this command: no", cobalt.render_human(result))

    @mock.patch("postfiat_rpc.cobalt.run_bounded_json_command")
    def test_live_status_reports_terminal_activation_without_block_authority(
        self, run_json: mock.Mock
    ) -> None:
        registry_root = "aa" * 48
        trust_root = "bb" * 48
        update_id = "cc" * 48
        with tempfile.TemporaryDirectory() as directory:
            data_dir = Path(directory)
            (data_dir / "governance.json").write_text(
                json.dumps(
                    {
                        "authority_mode": 1,
                        "cobalt_authority_transitions": [
                            {
                                "transition_id": "dd" * 48,
                                "to_authority_mode": 1,
                                "activation_height": 916,
                            }
                        ],
                        "validator_registry_updates": [
                            {
                                "update_id": update_id,
                                "activation_height": 917,
                                "new_registry_root": registry_root,
                                "new_trust_graph_root": trust_root,
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            def command_result(command: list[str], **_kwargs: object) -> dict:
                if "cobalt-shadow" in command[0]:
                    return {
                        "transport_healthy": True,
                        "catch_up_status": "current",
                        "registry_root": registry_root,
                        "trust_graph_root": trust_root,
                        "controls_block_consensus": False,
                    }
                if command[1] == "status":
                    return {
                        "node_id": "validator-0",
                        "chain_id": "live-test",
                        "block_height": 919,
                        "block_tip_hash": "ee" * 48,
                        "state_root": "ff" * 48,
                        "validator_count": 6,
                    }
                if command[1] == "verify-governance":
                    return {
                        "verified": True,
                        "cobalt_mode": "non_uniform",
                        "trust_graph_root": trust_root,
                        "active_validator_count": 6,
                        "latest_validator_registry_update_id": update_id,
                    }
                return {
                    "transport_healthy": True,
                    "catch_up_status": "current",
                    "registry_root": registry_root,
                    "trust_graph_root": trust_root,
                    "controls_block_consensus": False,
                }

            run_json.side_effect = command_result
            result = cobalt.live_status_result(
                data_dir,
                node_bin=Path("/opt/postfiat/postfiat-node"),
                shadow_bin=Path("/opt/postfiat/postfiat-cobalt-shadow"),
                shadow_data_dirs=[Path("/var/lib/postfiat-cobalt-shadow")],
                timeout_seconds=5,
            )

        self.assertTrue(result["ok"])
        self.assertEqual(result["status"], "ACTIVATED")
        self.assertEqual(result["terminal_decision"], "ACTIVATE")
        self.assertTrue(result["authority"]["writes_validator_registry"])
        self.assertFalse(result["authority"]["controls_block_consensus"])
        self.assertIn("Terminal decision: ACTIVATE", cobalt.render_human(result))

    def test_packet_authentication_rejects_tampering(self) -> None:
        root = cobalt.repository_root(Path(__file__))
        source = root / "benchmarks/cobalt-rippled-liveness/packet"
        with tempfile.TemporaryDirectory() as directory:
            packet = Path(directory) / "packet"
            shutil.copytree(source, packet)
            (packet / "cobalt-report.json").write_text("{}\n", encoding="utf-8")

            with self.assertRaisesRegex(cobalt.CobaltCliError, "checksum mismatch"):
                cobalt.scenario_result(packet)

    def test_packet_authentication_rejects_manifest_path_traversal(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            packet = Path(directory)
            manifest = b"0" * 64 + b"  ../outside.json\n"
            (packet / "SHA256SUMS").write_bytes(manifest)
            expected = hashlib.sha256(manifest).hexdigest()

            with self.assertRaisesRegex(cobalt.CobaltCliError, "malformed entry"):
                cobalt.verify_packet(
                    packet,
                    expected_manifest_sha256=expected,
                    expected_verifier_schema="test-v1",
                    required_files=set(),
                    required_checks=set(),
                )

    @mock.patch("postfiat_rpc.cobalt.subprocess.run")
    def test_shadow_runner_invokes_dedicated_binary_with_data_dir(
        self, run: mock.Mock
    ) -> None:
        run.return_value = subprocess.CompletedProcess(
            args=["cargo"],
            returncode=0,
            stdout=json.dumps({"ok": True, "status": "passed"}).encode(),
            stderr=b"",
        )

        report = cobalt.run_shadow_service(
            "drill",
            root=Path("/repo"),
            data_dir=Path("/state"),
            cargo="cargo",
            target=None,
            timeout_seconds=10,
        )

        self.assertTrue(report["ok"])
        command = run.call_args.args[0]
        self.assertIn("postfiat-cobalt-shadow", command)
        self.assertEqual(command[-3:], ["drill", "--data-dir", "/state"])

    @mock.patch("postfiat_rpc.cobalt.subprocess.run")
    def test_example_runner_rejects_non_json_output(self, run: mock.Mock) -> None:
        run.return_value = subprocess.CompletedProcess(
            args=["cargo"], returncode=0, stdout=b"not json", stderr=b""
        )

        with self.assertRaisesRegex(cobalt.CobaltCliError, "valid JSON"):
            cobalt.run_example(
                cobalt.EXAMPLES["trust-graph"],
                root=Path("/repo"),
                cargo="cargo",
                target=None,
                timeout_seconds=1,
            )

    @mock.patch("postfiat_rpc.cobalt.subprocess.run")
    def test_example_runner_uses_bounded_nonshell_subprocess(self, run: mock.Mock) -> None:
        payload = report_for(cobalt.EXAMPLES["trust-graph"])
        run.return_value = subprocess.CompletedProcess(
            args=["cargo"], returncode=0, stdout=json.dumps(payload).encode(), stderr=b""
        )

        result = cobalt.run_example(
            cobalt.EXAMPLES["trust-graph"],
            root=Path("/repo"),
            cargo="cargo",
            target=None,
            timeout_seconds=9,
        )

        self.assertEqual(result, payload)
        _args, kwargs = run.call_args
        self.assertEqual(kwargs["timeout"], 9)
        self.assertNotIn("shell", kwargs)
        self.assertFalse(kwargs["check"])


if __name__ == "__main__":
    unittest.main()
