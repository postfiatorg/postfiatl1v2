from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

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
        "governance_digest": "ef" * 48,
        "accepted_messages": 3,
        "queue_depth": 0,
        "boot_count": 1,
        "peer_count": 4,
    }


class CobaltUiTests(unittest.TestCase):
    def make_collector(self, root: Path) -> cobalt_ui.SnapshotCollector:
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

    def test_snapshot_uses_real_surface_shapes_and_holds_without_handoff(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            snapshot = self.make_collector(Path(directory)).collect()

        self.assertTrue(snapshot["read_only"])
        self.assertTrue(snapshot["trust"]["ok"])
        self.assertEqual(snapshot["proposals"]["authority_label"], "Foundation registry")
        self.assertEqual(snapshot["shadow"]["node_count"], 4)
        self.assertTrue(snapshot["shadow"]["converged"])
        self.assertEqual(snapshot["activation"]["status"], "HOLD")
        self.assertFalse(snapshot["activation"]["ready"])

    def test_recorded_handoff_moves_activation_to_active(self) -> None:
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

        self.assertEqual(snapshot["activation"]["status"], "ACTIVE")
        self.assertTrue(snapshot["activation"]["ready"])
        self.assertEqual(snapshot["proposals"]["transition_count"], 1)

    def test_mixed_shadow_digests_fail_convergence(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            collector = self.make_collector(Path(directory))

            def split_report(path: Path) -> dict:
                report = shadow_report(path)
                report["governance_digest"] = path.name
                return report

            collector.shadow_runner = split_report
            snapshot = collector.collect()

        self.assertFalse(snapshot["shadow"]["converged"])
        self.assertFalse(snapshot["shadow"]["ok"])

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
