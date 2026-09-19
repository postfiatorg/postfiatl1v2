"""Offline checks for the snapshot gate used by the transparent-only smoke script."""

from __future__ import annotations

import json
import re
import subprocess
import unittest
from pathlib import Path


SMOKE = Path(__file__).resolve().parents[2] / "scripts" / "testnet-monitor-snapshot-smoke"


class MonitorSnapshotSmokeHealthTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        script = SMOKE.read_text(encoding="utf-8")
        match = re.search(
            r'jq -e \\\n\s+\'([^\']+)\' \\\n\s+"\$SNAPSHOT_REPORT"',
            script,
        )
        if match is None:
            raise AssertionError("snapshot gate not found in smoke script")
        cls.snapshot_gate = match.group(1)

    def smoke_accepts(self, *, status: str, warnings: list[str],
                      criticals: list[str] | None = None, monitor_ok: bool = True) -> bool:
        snapshot = {
            "schema": "postfiat-testnet-monitor-snapshot-v1",
            "monitor_ok": monitor_ok,
            "account_canary_enabled": True,
            "checks": {
                "status": status,
                "warnings": warnings,
                "criticals": [] if criticals is None else criticals,
                "height_lag": 0,
                "online_endpoint_count": 1,
            },
        }
        result = subprocess.run(
            ["jq", "-e", self.snapshot_gate],
            input=json.dumps(snapshot),
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertIn(result.returncode, (0, 1), result.stderr)
        return result.returncode == 0

    def test_only_expected_no_proof_warning_is_tolerated(self) -> None:
        cases = (
            ("healthy", "ok", [], [], True, True),
            ("transparent_only", "warning", ["proof_latency_unavailable"], [], True, True),
            ("other_warning", "warning", ["height_lag_warn"], [], True, False),
            ("combined_warnings", "warning", ["proof_latency_unavailable", "height_lag_warn"], [], True, False),
            ("critical", "critical", ["proof_latency_unavailable"], ["chain_inconsistent"], False, False),
            ("unhealthy_monitor", "warning", ["proof_latency_unavailable"], [], False, False),
            ("inconsistent_ok", "ok", ["height_lag_warn"], [], True, False),
        )
        for name, status, warnings, criticals, monitor_ok, expected in cases:
            with self.subTest(name=name):
                self.assertEqual(
                    self.smoke_accepts(status=status, warnings=warnings,
                                       criticals=criticals, monitor_ok=monitor_ok),
                    expected,
                )


if __name__ == "__main__":
    unittest.main()
