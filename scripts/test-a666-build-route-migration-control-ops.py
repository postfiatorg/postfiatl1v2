#!/usr/bin/env python3
"""Regression tests for A666 public-successor migration controls."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


PROGRAM = Path(__file__).with_name("a666-build-route-migration-control-ops.py")
ASSET_ID = (
    "521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b6"
    "2d20e18555642bec32174498cbee5e2c"
)


def route(*, paused: bool = False) -> dict[str, object]:
    return {
        "route_id": "pftl-a666-ethereum-wA666-usdc-v1",
        "native_nav_asset_id": ASSET_ID,
        "live_value_enabled": True,
        "paused": paused,
        "active_reservation_count": 0,
        "export_entitlement_count": 0,
        "route_epoch": 6,
        "policy_epoch": 6,
        "pricing_nav_epoch": 6,
    }


class MigrationControlTests(unittest.TestCase):
    def run_builder(self, status: dict[str, object]) -> tuple[subprocess.CompletedProcess[str], Path]:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        route_path = root / "route.json"
        key_path = root / "reserve-key.json"
        output = root / "output"
        route_path.write_text(json.dumps(status), encoding="utf-8")
        key_path.write_text("{}\n", encoding="utf-8")
        result = subprocess.run(
            [
                sys.executable,
                str(PROGRAM),
                "--route-status",
                str(route_path),
                "--reserve-key-file",
                str(key_path),
                "--output-dir",
                str(output),
            ],
            text=True,
            capture_output=True,
            check=False,
        )
        return result, output

    def test_unpaused_route_gets_pre_pause_resume_and_emergency_pause(self) -> None:
        result, output = self.run_builder(route())
        self.assertEqual(result.returncode, 0, result.stderr)
        manifest = json.loads((output / "migration-control-manifest.json").read_text())
        self.assertEqual(
            set(manifest["operations"]),
            {
                "pause_before_migration",
                "resume_after_verification",
                "emergency_pause_after_resume",
            },
        )
        pause = json.loads((output / "01-pause-before-migration.ops.json").read_text())
        resume = json.loads((output / "90-resume-after-verification.ops.json").read_text())
        emergency = json.loads(
            (output / "99-emergency-pause-after-resume.ops.json").read_text()
        )
        self.assertIs(pause["operations"][0]["operation"]["paused"], True)
        self.assertIs(resume["operations"][0]["operation"]["paused"], False)
        self.assertIs(emergency["operations"][0]["operation"]["paused"], True)

    def test_already_paused_route_does_not_emit_noop_pre_pause(self) -> None:
        result, output = self.run_builder(route(paused=True))
        self.assertEqual(result.returncode, 0, result.stderr)
        manifest = json.loads((output / "migration-control-manifest.json").read_text())
        self.assertNotIn("pause_before_migration", manifest["operations"])
        self.assertFalse((output / "01-pause-before-migration.ops.json").exists())

    def test_active_order_state_is_rejected(self) -> None:
        status = route()
        status["active_reservation_count"] = 1
        result, _ = self.run_builder(status)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("zero active reservations", result.stderr)

    def test_wrong_native_asset_is_rejected(self) -> None:
        status = route()
        status["native_nav_asset_id"] = "00" * 48
        result, _ = self.run_builder(status)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("native asset is not A666", result.stderr)

    def test_stream_input_hash_binds_the_validated_bytes(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        key_path = root / "reserve-key.json"
        key_path.write_text("{}\n", encoding="utf-8")
        output = root / "output"
        raw = json.dumps(route(), sort_keys=True).encode()
        read_fd, write_fd = os.pipe()
        try:
            os.write(write_fd, raw)
            os.close(write_fd)
            write_fd = -1
            result = subprocess.run(
                [
                    sys.executable,
                    str(PROGRAM),
                    "--route-status",
                    f"/dev/fd/{read_fd}",
                    "--reserve-key-file",
                    str(key_path),
                    "--output-dir",
                    str(output),
                ],
                text=True,
                capture_output=True,
                check=False,
                pass_fds=(read_fd,),
            )
        finally:
            os.close(read_fd)
            if write_fd >= 0:
                os.close(write_fd)
        self.assertEqual(result.returncode, 0, result.stderr)
        manifest = json.loads((output / "migration-control-manifest.json").read_text())
        self.assertEqual(manifest["route_status_sha256"], hashlib.sha256(raw).hexdigest())


if __name__ == "__main__":
    unittest.main()
