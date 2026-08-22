from __future__ import annotations

import json
import subprocess
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
