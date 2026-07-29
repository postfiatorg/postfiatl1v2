#!/usr/bin/env python3

from __future__ import annotations

import argparse
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock


SCRIPT = Path(__file__).with_name("a666-resident-rounds.py")
SPEC = importlib.util.spec_from_file_location("a666_resident_rounds", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"cannot import {SCRIPT}")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class FakeRpc:
    @staticmethod
    def wait_for_fleet_status(_ports, _rpc_timeout, _convergence_timeout):
        return [
            {
                "block_height": 466,
                "state_root": "11" * 48,
                "node_id": f"validator-{index}",
            }
            for index in range(6)
        ]


class ResidentRoundTests(unittest.TestCase):
    def test_remote_paths_are_height_and_proposer_scoped(self) -> None:
        paths = MODULE.remote_paths("a666-opt-pass4", "validator-3", 470)
        self.assertEqual("/var/lib/postfiat/validator-3", paths["data_dir"])
        self.assertEqual(
            "/var/lib/postfiat/validator-3/a666-resident-rounds/"
            "a666-opt-pass4/h470",
            paths["root"],
        )
        self.assertNotEqual(paths["batch_dir"], paths["isolated_outbox"])

    def test_select_entry_fails_closed_on_kind_or_height_ambiguity(self) -> None:
        manifest = {
            "entries": [
                {"height": 467, "batch_kind": "transparent"},
                {"height": 468, "batch_kind": "shielded"},
            ]
        }
        selected = MODULE.select_entry(manifest, 468, "shielded")
        self.assertEqual(468, selected["height"])
        with self.assertRaises(RuntimeError):
            MODULE.select_entry(manifest, 468, "transparent")
        manifest["entries"].append({"height": 468, "batch_kind": "shielded"})
        with self.assertRaises(RuntimeError):
            MODULE.select_entry(manifest, 468, "shielded")

    def test_wait_ready_requires_both_shielded_verifiers_warm(self) -> None:
        entry = {
            "height": 468,
            "proposer": "validator-0",
            "host": "192.0.2.1",
            "remote": MODULE.remote_paths(
                "a666-opt-pass4", "validator-0", 468
            ),
        }
        ready = {
            "schema": MODULE.READY_SCHEMA,
            "node_id": "validator-0",
            "start_height": 468,
            "max_rounds": 1,
            "shielded_verifier_prewarm": {
                "requested": True,
                "asset_orchard_swap_verifier_warm": True,
                "asset_orchard_private_egress_verifier_warm": True,
            },
        }
        completed = mock.Mock(stdout=json.dumps(ready))
        with mock.patch.object(MODULE, "ssh", return_value=completed):
            self.assertEqual(ready, MODULE.wait_ready(entry, 10.0))
        ready["shielded_verifier_prewarm"][
            "asset_orchard_private_egress_verifier_warm"
        ] = False
        completed = mock.Mock(stdout=json.dumps(ready))
        with (
            mock.patch.object(MODULE, "ssh", return_value=completed),
            self.assertRaises(RuntimeError),
        ):
            MODULE.wait_ready(entry, 10.0)

    def test_start_worker_creates_writable_root_before_launch(self) -> None:
        entry = {
            "height": 468,
            "batch_kind": "shielded",
            "proposer": "validator-0",
            "host": "192.0.2.1",
            "remote": MODULE.remote_paths(
                "a666-opt-pass4", "validator-0", 468
            ),
        }
        with mock.patch.object(MODULE, "ssh") as remote:
            MODULE.start_worker(
                entry,
                "/opt/postfiat/postfiat-node",
                "/etc/postfiat/topology.json",
            )
        self.assertEqual(2, remote.call_count)
        prepare = remote.call_args_list[0].args[1]
        root = entry["remote"]["root"]
        self.assertIn(f"install -d -o postfiat -g postfiat -m 700 {root}", prepare)
        launch = remote.call_args_list[1].args[1]
        self.assertIn("unshare --mount --propagation private", launch)
        self.assertIn("POSTFIAT_PREWARM_SHIELDED_VERIFIER=1", launch)
        self.assertIn(
            "POSTFIAT_PREWARM_ASSET_ORCHARD_PRIVATE_EGRESS_VERIFIER=1",
            launch,
        )
        self.assertIn("POSTFIAT_CERTIFIED_BATCH_LOOP_READY_FILE=", launch)

    def test_start_freezes_one_deterministic_worker_per_planned_height(self) -> None:
        hosts = {f"validator-{index}": f"192.0.2.{index + 1}" for index in range(6)}
        plan = "transparent,transparent,shielded,shielded,shielded,transparent"
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "manifest.json"
            args = argparse.Namespace(
                workflow_id="a666-opt-pass4",
                start_height=466,
                plan=plan,
                output=output,
                proposer_hosts_file=Path("unused-hosts.json"),
                remote_binary="/opt/postfiat/postfiat-node",
                remote_topology="/etc/postfiat/topology.json",
                ports="28650,28651,28652,28653,28654,28655",
                timeout_seconds=45.0,
                ready_timeout=180.0,
            )

            def proposer(_hosts, _binary, height):
                return f"validator-{(height - 467) % 6}"

            def readiness(entry, _timeout):
                return {
                    "schema": MODULE.READY_SCHEMA,
                    "node_id": entry["proposer"],
                    "start_height": entry["height"],
                    "max_rounds": 1,
                }

            with (
                mock.patch.object(MODULE, "load_hosts", return_value=hosts),
                mock.patch.object(
                    MODULE, "load_rpc_helpers", return_value=FakeRpc()
                ),
                mock.patch.object(MODULE, "proposer_for_height", side_effect=proposer),
                mock.patch.object(MODULE, "start_worker") as start_worker,
                mock.patch.object(MODULE, "wait_ready", side_effect=readiness),
            ):
                with contextlib.redirect_stdout(io.StringIO()):
                    MODULE.command_start(args)

            manifest = json.loads(output.read_text())
            self.assertEqual(MODULE.MANIFEST_SCHEMA, manifest["schema"])
            self.assertEqual(466, manifest["start_height"])
            self.assertEqual(
                [467, 468, 469, 470, 471, 472],
                [entry["height"] for entry in manifest["entries"]],
            )
            self.assertEqual(plan.split(","), [
                entry["batch_kind"] for entry in manifest["entries"]
            ])
            self.assertEqual(6, start_worker.call_count)


if __name__ == "__main__":
    unittest.main()
