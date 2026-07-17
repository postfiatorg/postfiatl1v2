from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
import unittest
from pathlib import Path, PurePosixPath
from unittest.mock import patch

from postfiat_ops.safe_rollout import (
    CopyEntry,
    InventoryEntry,
    SafetyError,
    STATE_SCHEMA,
    apply_next,
    build_diff,
    copy_entries,
    create_backup,
    fleet_convergence,
    next_validator,
    parse_inventory,
    preflight,
    reject_unsafe_cli_tokens,
    rollout_order,
    validate_copy_entries,
    validate_diff,
    verify_remote_committee_rosters,
)


def completed(argv: list[str], stdout: str = "") -> subprocess.CompletedProcess[str]:
    return subprocess.CompletedProcess(argv, 0, stdout, "")


class FakeRunner:
    def __init__(self, remote_hash_output: str = "") -> None:
        self.calls: list[tuple[list[str], str | None]] = []
        self.remote_hash_output = remote_hash_output

    def run(self, argv, *, input_text=None, capture=True):
        args = list(argv)
        self.calls.append((args, input_text))
        if args[0] == "scp" and "-r" in args:
            Path(args[-1]).mkdir(parents=True, exist_ok=False)
        if "snapshot-export-signed" in args:
            destination = Path(args[args.index("--snapshot-dir") + 1])
            destination.mkdir(parents=True, exist_ok=False)
            (destination / "snapshot.signed-manifest.json").write_text(
                '{"schema":"test"}\n', encoding="utf-8"
            )
        if "snapshot-import-signed" in args:
            destination = Path(args[args.index("--data-dir") + 1])
            destination.mkdir(parents=True, exist_ok=False)
            (destination / "chain_tip.json").write_text(
                '{"height":600,"block_hash":"tip-a","state_root":"root-a"}\n',
                encoding="utf-8",
            )
        if "verify-state" in args:
            return completed(args, '{"verified":true,"state_root":"root-a"}\n')
        if args[0] == "ssh" and input_text and " validate-local-keys " in input_text:
            validator_id = f"validator-{int(args[3].rsplit('.', 1)[-1]) - 1}"
            return completed(
                args,
                json.dumps(
                    {
                        "schema": "postfiat-local-key-validation-v1",
                        "node_id": validator_id,
                        "validator_keys_valid": True,
                        "validator_key_permissions_valid": True,
                        "validator_key_count": 6,
                        "required_validator_count": 6,
                    }
                ),
            )
        if args[0] == "ssh" and input_text and " status --data-dir " in input_text:
            return completed(
                args,
                json.dumps(
                    {
                        "block_height": 600,
                        "block_tip_hash": "tip-a",
                        "state_root": "root-a",
                        "mempool_pending": 0,
                    }
                ),
            )
        if args[0] == "ssh" and input_text and "sha256sum" in input_text:
            return completed(args, self.remote_hash_output)
        return completed(args)


class SafeRolloutTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.release_id = "release-safe-1"
        self.stage = self.root / "stage"
        self.rootfs = self.stage / "rootfs"
        self.stage.mkdir()
        validators = []
        for index in range(6):
            validator = f"validator-{index}"
            validators.append({"validator_id": validator})
            targets = [
                f"/etc/postfiat/releases/{self.release_id}/{validator}.bindings.json",
                f"/etc/postfiat/releases/{self.release_id}/{validator}.rpc.env",
                f"/etc/postfiat/releases/{self.release_id}/{validator}.transport.env",
                f"/etc/systemd/system/postfiat-{validator}.service",
                f"/etc/systemd/system/postfiat-{validator}-rpc.service",
            ]
            for target in targets:
                self._write_target(target, f"{validator}:{target}\n")
            self._write_target(
                f"/etc/postfiat/releases/{self.release_id}/{validator}.bindings.json",
                json.dumps(
                    {
                        "schema": "postfiat.deployment_validator_bindings.v1",
                        "validators": [
                            {
                                "validator_id": validator,
                                "services": [
                                    {
                                        "service_id": "rpc",
                                        "service_unit_file": f"/etc/systemd/system/postfiat-{validator}-rpc.service",
                                        "environment_file": f"/etc/postfiat/releases/{self.release_id}/{validator}.rpc.env",
                                    },
                                    {
                                        "service_id": "transport",
                                        "service_unit_file": f"/etc/systemd/system/postfiat-{validator}.service",
                                        "environment_file": f"/etc/postfiat/releases/{self.release_id}/{validator}.transport.env",
                                    },
                                ],
                            }
                        ],
                    }
                ),
            )
        for name in [
            "deployment-manifest.json",
            "deployment.public.json",
            "topology.json",
            "swap.metadata.json",
            "private-egress.metadata.json",
        ]:
            self._write_target(f"/etc/postfiat/releases/{self.release_id}/{name}", name)
        self._write_target(
            f"/opt/postfiat/releases/{self.release_id}/postfiat-node", "binary"
        )
        self.stage_report = self.stage / "stage-report.json"
        self.stage_report.write_text(
            json.dumps(
                {
                    "schema": "postfiat.deployment_validator_unit_stage.v1",
                    "release_id": self.release_id,
                    "rootfs_dir": str(self.rootfs),
                    "validators": validators,
                }
            ),
            encoding="utf-8",
        )
        self.inventory = self.root / "fleet.txt"
        self.inventory.write_text(
            "\n".join(
                f"validator-{index} 192.0.2.{index + 1} p2p={26650 + index * 2} "
                f"rpc={27650 + index} region=ewr "
                f"vultr_instance=00000000-0000-0000-0000-00000000000{index}"
                for index in range(6)
            )
            + "\n",
            encoding="utf-8",
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _write_target(self, target: str, value: str) -> None:
        path = self.rootfs.joinpath(*PurePosixPath(target).parts[1:])
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(value, encoding="utf-8")

    def test_rejects_every_delete_shape_and_root_destination(self) -> None:
        for token in ["--delete", "--delete-delay", "--delete-excluded", "host:/", "/"]:
            with self.subTest(token=token), self.assertRaises(SafetyError):
                reject_unsafe_cli_tokens(["preflight", token])

    def test_rejects_operator_destination_and_rsync_passthrough(self) -> None:
        for token in ["--destination=/tmp", "--target-root=/", "--rsync-arg=-a"]:
            with self.subTest(token=token), self.assertRaises(SafetyError):
                reject_unsafe_cli_tokens([token])

    def test_stage_derives_only_release_and_named_unit_targets(self) -> None:
        release, entries = copy_entries(self.stage_report, "validator-1")
        self.assertEqual(self.release_id, release)
        self.assertEqual(11, len(entries))
        self.assertIn(
            PurePosixPath("/etc/systemd/system/postfiat-validator-1.service"),
            {entry.target for entry in entries},
        )

    def test_non_allowlisted_target_is_a_hard_error(self) -> None:
        source = self.root / "payload"
        source.write_text("payload", encoding="utf-8")
        with self.assertRaises(SafetyError):
            validate_copy_entries(
                [CopyEntry(source, PurePosixPath("/etc/passwd"))],
                self.release_id,
                "validator-1",
            )

    def test_diff_reports_exact_create_update_unchanged_without_deletes(self) -> None:
        _, entries = copy_entries(self.stage_report, "validator-1")
        remote = {
            str(entries[0].target): None,
            str(entries[1].target): "wrong",
            str(entries[2].target): __import__("hashlib").sha256(
                entries[2].source.read_bytes()
            ).hexdigest(),
        }
        rows = build_diff(entries, remote)
        self.assertEqual("create", rows[0]["action"])
        self.assertEqual("update", rows[1]["action"])
        self.assertEqual("unchanged", rows[2]["action"])
        self.assertNotIn("delete", {row["action"] for row in rows})

    def test_diff_rejects_delete_and_paths_outside_plan(self) -> None:
        with self.assertRaises(SafetyError):
            validate_diff([{"action": "delete", "path": "/etc/passwd"}], {"/etc/passwd"})
        with self.assertRaises(SafetyError):
            validate_diff([{"action": "create", "path": "/etc/passwd"}], {"/allowed"})

    def test_inventory_requires_all_six_unique_instance_bindings(self) -> None:
        rows = parse_inventory(self.inventory)
        self.assertEqual(6, len(rows))
        broken = self.root / "broken.txt"
        broken.write_text(self.inventory.read_text(encoding="utf-8").replace(
            "validator-5", "validator-4", 1
        ), encoding="utf-8")
        with self.assertRaises(SafetyError):
            parse_inventory(broken)

    def test_state_machine_requires_preflight_backup_and_canary_prefix(self) -> None:
        state = {
            "schema": STATE_SCHEMA,
            "order": rollout_order("validator-1"),
            "applied": [],
            "preflight": {"verified": True},
            "backup": {"verified": False},
        }
        with self.assertRaises(SafetyError):
            next_validator(state)
        state["backup"]["verified"] = True
        self.assertEqual("validator-1", next_validator(state))
        state["applied"] = ["validator-0"]
        with self.assertRaises(SafetyError):
            next_validator(state)

    @patch("postfiat_ops.safe_rollout.time.sleep")
    @patch("postfiat_ops.safe_rollout._fleet_convergence_once")
    def test_fleet_convergence_retries_only_transient_reachability(
        self, mock_once, mock_sleep
    ) -> None:
        expected = {"verified": True, "validator_count": 6}
        mock_once.side_effect = [ConnectionRefusedError(), expected]
        self.assertEqual(expected, fleet_convergence([]))
        mock_sleep.assert_called_once_with(1.0)

        mock_once.reset_mock()
        mock_sleep.reset_mock()
        mock_once.side_effect = SafetyError("fleet ledger divergence")
        with self.assertRaisesRegex(SafetyError, "ledger divergence"):
            fleet_convergence([])
        mock_sleep.assert_not_called()

    def test_preflight_rejects_one_node_with_incomplete_committee_roster(self) -> None:
        class IncompleteRosterRunner(FakeRunner):
            def run(self, argv, *, input_text=None, capture=True):
                result = super().run(argv, input_text=input_text, capture=capture)
                args = list(argv)
                if (
                    args[0] == "ssh"
                    and input_text
                    and " validate-local-keys " in input_text
                    and args[3] == "root@192.0.2.1"
                ):
                    report = json.loads(result.stdout)
                    report["validator_key_count"] = 1
                    return completed(args, json.dumps(report))
                return result

        inventory = parse_inventory(self.inventory)
        with self.assertRaisesRegex(
            SafetyError, "incomplete or invalid committee roster on validator-0"
        ):
            verify_remote_committee_rosters(
                IncompleteRosterRunner(), inventory, "root"
            )

        reports = verify_remote_committee_rosters(FakeRunner(), inventory, "root")
        self.assertEqual(6, len(reports))
        self.assertTrue(all(report["validator_key_count"] == 6 for report in reports))

    @patch("postfiat_ops.safe_rollout.query_vultr_inventory")
    @patch("postfiat_ops.safe_rollout.fleet_convergence")
    @patch("postfiat_ops.safe_rollout.remote_hashes")
    def test_preflight_is_read_only_and_writes_zero_deletion_plan(
        self, mock_remote_hashes, mock_convergence, mock_cloud
    ) -> None:
        mock_remote_hashes.return_value = {}
        mock_convergence.return_value = {"verified": True, "validator_count": 6}
        mock_cloud.return_value = [
            {"validator_id": f"validator-{index}"} for index in range(6)
        ]
        state_file = self.root / "rollout-state.json"
        args = argparse.Namespace(
            state_file=state_file,
            inventory_file=self.inventory,
            vultr_api_key_file=self.root / "api-key",
            stage_report=self.stage_report,
            canary_validator_id="validator-1",
            ssh_user="root",
        )
        state = preflight(args, FakeRunner())
        self.assertTrue(state["preflight"]["verified"])
        self.assertEqual(0, state["preflight"]["deletion_count"])
        self.assertEqual("validator-1", state["order"][0])
        self.assertEqual([], state["applied"])

    def test_backup_is_mandatory_signed_and_verified_before_apply(self) -> None:
        state_file = self.root / "rollout-state.json"
        state_file.write_text(
            json.dumps(
                {
                    "schema": STATE_SCHEMA,
                    "release_id": self.release_id,
                    "stage_report": str(self.stage_report),
                    "stage_report_sha256": __import__("hashlib").sha256(
                        self.stage_report.read_bytes()
                    ).hexdigest(),
                    "inventory_file": str(self.inventory),
                    "inventory_file_sha256": __import__("hashlib").sha256(
                        self.inventory.read_bytes()
                    ).hexdigest(),
                    "ssh_user": "root",
                    "canary_validator_id": "validator-1",
                    "order": rollout_order("validator-1"),
                    "applied": [],
                    "preflight": {"verified": True},
                    "backup": {"verified": False},
                }
            ),
            encoding="utf-8",
        )
        publisher_private = self.root / "publisher.private.json"
        publisher_public = self.root / "publisher.public.json"
        publisher_private.write_text("private", encoding="utf-8")
        publisher_public.write_text("public", encoding="utf-8")
        args = argparse.Namespace(
            state_file=state_file,
            evidence_dir=self.root / "evidence",
            snapshot_publisher_key_file=publisher_private,
            snapshot_publisher_public_key_file=publisher_public,
        )
        runner = FakeRunner()
        state = create_backup(args, runner)
        self.assertTrue(state["backup"]["verified"])
        self.assertEqual("root-a", state["backup"]["state_root"])
        flattened = [argument for call, _ in runner.calls for argument in call]
        self.assertIn("snapshot-export-signed", flattened)
        self.assertIn("snapshot-import-signed", flattened)
        remote_scripts = [
            script for call, script in runner.calls if call[0] == "ssh" and script
        ]
        self.assertEqual(1, len(remote_scripts))
        self.assertIn("systemctl show --property=MainPID", remote_scripts[0])
        self.assertIn('readlink -f "/proc/$active_pid/exe"', remote_scripts[0])
        self.assertIn(
            "mkdir /var/lib/postfiat/pre-rollout-snapshots/.release-safe-1-validator-1.lock",
            remote_scripts[0],
        )
        self.assertIn(
            "trap 'rmdir /var/lib/postfiat/pre-rollout-snapshots/.release-safe-1-validator-1.lock' EXIT",
            remote_scripts[0],
        )
        self.assertNotIn(
            f"/opt/postfiat/releases/{self.release_id}/postfiat-node",
            remote_scripts[0],
        )

    @patch("postfiat_ops.safe_rollout.fleet_convergence")
    @patch("postfiat_ops.safe_rollout.remote_hashes")
    def test_apply_next_selects_canary_and_never_invokes_rsync(
        self, mock_hashes, mock_convergence
    ) -> None:
        mock_hashes.return_value = {}
        mock_convergence.return_value = {"verified": True, "validator_count": 6}
        state_file = self.root / "rollout-state.json"
        state_file.write_text(
            json.dumps(
                {
                    "schema": STATE_SCHEMA,
                    "release_id": self.release_id,
                    "stage_report": str(self.stage_report),
                    "stage_report_sha256": __import__("hashlib").sha256(
                        self.stage_report.read_bytes()
                    ).hexdigest(),
                    "inventory_file": str(self.inventory),
                    "inventory_file_sha256": __import__("hashlib").sha256(
                        self.inventory.read_bytes()
                    ).hexdigest(),
                    "ssh_user": "root",
                    "canary_validator_id": "validator-1",
                    "order": rollout_order("validator-1"),
                    "applied": [],
                    "preflight": {"verified": True},
                    "backup": {"verified": True},
                }
            ),
            encoding="utf-8",
        )
        runner = FakeRunner()
        state = apply_next(argparse.Namespace(state_file=state_file), runner)
        self.assertEqual(["validator-1"], state["applied"])
        self.assertEqual(
            6,
            len(
                state["apply_reports"]["validator-1"]
                ["pre_mutation_committee_rosters"]
            ),
        )
        command_words = [word for call, _ in runner.calls for word in call]
        self.assertNotIn("rsync", command_words)
        self.assertFalse(any(word.startswith("--delete") for word in command_words))
        scp_destinations = [
            call[-1] for call, _ in runner.calls if call and call[0] == "scp" and "-r" not in call
        ]
        self.assertTrue(scp_destinations)
        self.assertTrue(
            all(destination.endswith(".incoming-safe-rollout") for destination in scp_destinations)
        )
        promotion_scripts = [
            script for call, script in runner.calls if call and call[0] == "ssh" and script and "mv -T" in script
        ]
        self.assertEqual(1, len(promotion_scripts))


if __name__ == "__main__":
    unittest.main()
