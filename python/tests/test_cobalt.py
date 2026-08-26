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


def write_adversarial_packet(packet: Path, root: Path) -> str:
    packet.mkdir(parents=True)
    live_cases = sorted(cobalt.ADVERSARIAL_LIVE_CASES)
    validators = list(cobalt.ADVERSARIAL_VALIDATORS)
    authorizers = validators[:5]
    observed_at = "2026-08-26T04:00:00Z"
    tip_hash = "11" * 48
    state_root = "aa" * 48
    registry_root = "bb" * 48
    trust_graph_root = "cc" * 48
    ratification_anchor_id = "22" * 48

    experiment_pins = []
    experiment_rows = {}
    for experiment, relative in cobalt.ADVERSARIAL_EXPERIMENT_PACKET_PATHS.items():
        source = root / relative
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_text(f"fixture {experiment}\n", encoding="ascii")
        source_hash = hashlib.sha256(source.read_bytes()).hexdigest()
        experiment_pins.append(
            {
                "experiment": experiment,
                "path": relative,
                "sha256sums_sha256": source_hash,
            }
        )
        experiment_rows[experiment] = {
            "status": "passed",
            "summary": "fixture",
            "sha256sums_sha256": source_hash,
        }

    publication_documents = []
    for relative in sorted(cobalt.ADVERSARIAL_PUBLICATION_PATHS):
        source = root / relative
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_text(f"fixture publication {relative}\n", encoding="utf-8")
        publication_documents.append(
            {
                "path": relative,
                "sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
            }
        )

    transitions = [
        {
            "kind": "forward_rollback_to_foundation",
            "accepted": True,
            "height": 920,
            "transition_id": "dd" * 48,
            "proposal_identity": "validator-2",
            "authorization_identities": authorizers,
            "receipt_accepted": True,
            "finality_confirmed": True,
            "all_six_converged": True,
        },
        {
            "kind": "return_to_cobalt",
            "accepted": True,
            "height": 921,
            "transition_id": "ee" * 48,
            "proposal_identity": "validator-3",
            "authorization_identities": authorizers,
            "receipt_accepted": True,
            "finality_confirmed": True,
            "all_six_converged": True,
        },
    ]
    rotation = {
        "accepted": True,
        "stale_key_rejected": True,
        "height": 922,
        "update_id": "ff" * 48,
        "subject_node_id": "validator-5",
        "proposal_identity": "validator-4",
        "authorization_identities": authorizers,
        "receipt_accepted": True,
        "finality_confirmed": True,
        "all_six_converged": True,
        "previous_public_key_sha256": "33" * 32,
        "new_public_key_sha256": "44" * 32,
        "ratification_anchor_sequence": 2,
        "ratification_anchor_id": ratification_anchor_id,
    }
    finality_receipts = [
        {
            "height": height,
            "block_hash": f"{height % 256:02x}" * 48,
            "state_root": state_root,
            "receipt_accepted": True,
            "finality_confirmed": True,
            "all_six_converged": True,
        }
        for height in (920, 921, 922)
    ]
    fleet = [
        {
            "node_id": validator,
            "height": 922,
            "tip_hash": tip_hash,
            "state_root": state_root,
            "registry_root": registry_root,
            "trust_graph_root": trust_graph_root,
            "authority_mode": "cobalt-validator-trust",
            "ratification_anchor_sequence": 2,
            "ratification_anchor_id": ratification_anchor_id,
            "validator_service_active": True,
            "rpc_service_active": True,
            "shadow_service_active": True,
        }
        for validator in validators
    ]
    rejected_cases = []
    for index, name in enumerate(live_cases):
        row = {
            "experiment": "E5",
            "name": name,
            "rejected": True,
            "durable_state_unchanged": True,
            "reason": "fixture rejection",
            "verifier_node_id": validators[index % len(validators)],
            "observed_height": 922,
            "evidence_sha256": f"{index + 1:02x}" * 32,
        }
        if name == "stolen_key_rotation":
            row.update(
                {
                    "signature_count": 1,
                    "decision_certificate_present": True,
                    "stolen_validator": "validator-5",
                    "attempted_subject": "validator-5",
                }
            )
        rejected_cases.append(row)

    browser_snapshot = {
        "schema": cobalt.ADVERSARIAL_BROWSER_SNAPSHOT_SCHEMA,
        "collected_at": observed_at,
        "read_only": True,
        "actual_authority": {
            "cobalt_active": True,
            "controls_block_consensus": False,
            "block_finality": "consensus-v2",
        },
        "adversarial": {
            "gate": "KEEP_ACTIVE",
            "campaign_complete": True,
            "experiment_pass_count": 6,
            "rejected_case_count": len(live_cases),
        },
    }
    cli_output = (
        "Final gate: KEEP_ACTIVE\n"
        "Campaign complete: yes\n"
        + "\n".join(live_cases)
        + "\n"
    )
    objects = {
        "adversarial-status.json": {
            "gate": "KEEP_ACTIVE",
            "campaign_complete": True,
            "final_release_gate": "passed",
            "scope": "protocol capability",
            "proposal_origin": "Foundation-administered validators",
            "protocol_capability_only": True,
            "operator_decentralization_proven": False,
            "cobalt_scope": "validator-registry ratification",
            "trust_selection_is_separate": True,
        },
        "browser-snapshot.json": browser_snapshot,
        "experiments.json": {"experiments": experiment_rows},
        "interfaces.json": {
            "cli": {
                "passed": True,
                "exit_code": 0,
                "command": "python -m postfiat_rpc.cobalt adversarial",
                "output_sha256": hashlib.sha256(cli_output.encode("utf-8")).hexdigest(),
            },
            "browser": {
                "passed": True,
                "read_only": True,
                "snapshot_get_http_status": 200,
                "snapshot_get_path": "/api/snapshot",
                "snapshot_body_sha256": hashlib.sha256(
                    (json.dumps(browser_snapshot, indent=2, sort_keys=True) + "\n").encode()
                ).hexdigest(),
                "mutation_probe_method": "POST",
                "mutation_probe_path": "/api/snapshot",
                "mutation_probe_http_status": 405,
            },
        },
        "live-authority.json": {
            "chain_id": "postfiat-wan-devnet-2",
            "observed_at": observed_at,
            "height": 922,
            "tip_hash": tip_hash,
            "state_root": state_root,
            "registry_root": registry_root,
            "trust_graph_root": trust_graph_root,
            "ratification_anchor_sequence": 2,
            "ratification_anchor_id": ratification_anchor_id,
            "trust_model": "uniform full overlap",
            "trust_graph_profile": "six validator canonical views",
            "trust_view_count": 6,
            "non_identical_trust_views": False,
            "validator_count": 6,
            "all_six_converged": True,
            "authority_mode": "cobalt-validator-trust",
            "block_finality": "consensus-v2",
            "cobalt_controls_block_consensus": False,
            "fleet": fleet,
            "authority_transitions": transitions,
            "legitimate_rotation": rotation,
            "finality_receipts": finality_receipts,
        },
        "publication.json": {
            "published": True,
            "published_at": observed_at,
            "operator_boundary_explicit": True,
            "documents": publication_documents,
            "article": {
                "url": "https://postfiat.org/blog/cobalt-further-evaluation/",
                "http_status": 200,
                "content_sha256": "55" * 32,
                "cobalt_active_since_height_916": True,
                "authority_off_claim_absent": True,
            },
            "results": {
                "path": "docs/governance/cobalt-adversarial-verification-results.md",
                "published": True,
                "public_url": "https://postfiat.org/docs/cobalt-adversarial-results/",
            },
        },
        "rejected-cases.json": {"cases": rejected_cases},
        "source-pins.json": {"experiment_packets": experiment_pins},
        "verifier.json": {
            "schema": cobalt.ADVERSARIAL_PACKET_SCHEMA,
            "result": "passed",
            "checks": {
                name: True for name in cobalt.ADVERSARIAL_REQUIRED_CHECKS
            },
        },
    }
    for name, value in objects.items():
        (packet / name).write_text(
            json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
    (packet / "README.md").write_text("# Fixture\n", encoding="utf-8")
    (packet / "cli-output.txt").write_text(cli_output, encoding="utf-8")
    (packet / "verify_packet.py").write_text("print('fixture')\n", encoding="utf-8")
    lines = [
        f"{hashlib.sha256((packet / name).read_bytes()).hexdigest()}  {name}"
        for name in sorted(cobalt.ADVERSARIAL_REQUIRED_FILES)
    ]
    manifest = ("\n".join(lines) + "\n").encode("ascii")
    (packet / "SHA256SUMS.txt").write_bytes(manifest)
    return hashlib.sha256(manifest).hexdigest()


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

    def test_adversarial_command_authenticates_and_renders_complete_campaign(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "source"
            packet = Path(directory) / "packet"
            packet_root = write_adversarial_packet(packet, root)

            with mock.patch.object(cobalt, "repository_root", return_value=root):
                result = cobalt.adversarial_result(
                    packet, expected_manifest_sha256=packet_root
                )

        self.assertTrue(result["ok"])
        self.assertEqual(result["status"], "KEEP_ACTIVE")
        self.assertEqual(len(result["rejected_cases"]), 9)
        rendered = cobalt.render_human(result)
        self.assertIn("Final gate: KEEP_ACTIVE", rendered)
        self.assertIn("stolen_key_rotation: fixture rejection", rendered)

    def test_adversarial_command_rejects_rehashed_semantic_tampering(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "source"
            packet = Path(directory) / "packet"
            write_adversarial_packet(packet, root)
            status_path = packet / "adversarial-status.json"
            status = json.loads(status_path.read_text(encoding="utf-8"))
            status["gate"] = "ROLLED_BACK"
            status_path.write_text(
                json.dumps(status, indent=2, sort_keys=True) + "\n", encoding="utf-8"
            )
            lines = [
                f"{hashlib.sha256((packet / name).read_bytes()).hexdigest()}  {name}"
                for name in sorted(cobalt.ADVERSARIAL_REQUIRED_FILES)
            ]
            manifest = ("\n".join(lines) + "\n").encode("ascii")
            (packet / "SHA256SUMS.txt").write_bytes(manifest)
            packet_root = hashlib.sha256(manifest).hexdigest()

            with mock.patch.object(cobalt, "repository_root", return_value=root):
                with self.assertRaisesRegex(cobalt.CobaltCliError, "semantic verifier"):
                    cobalt.adversarial_result(
                        packet, expected_manifest_sha256=packet_root
                    )

    def test_packet_reader_rejects_symlinks(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target.json"
            target.write_text("{}\n", encoding="utf-8")
            link = root / "packet.json"
            link.symlink_to(target)

            with self.assertRaisesRegex(cobalt.CobaltCliError, "symlink|cannot open"):
                cobalt.read_packet_bytes(link)

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
