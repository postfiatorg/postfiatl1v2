from __future__ import annotations

import argparse
import json
import os
import runpy
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "testnet-monitor-snapshot"


class MonitorSnapshotTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.module = runpy.run_path(str(SCRIPT))

    def args(self, **overrides: object) -> argparse.Namespace:
        values = {
            "critical_height_lag": 1,
            "warn_height_lag": 0,
            "warn_rpc_p95_ms": 1000.0,
            "critical_rpc_p95_ms": 5000.0,
            "warn_mempool_pending": 100,
            "critical_mempool_pending": 1000,
            "warn_recent_rejected_receipts": 0,
            "warn_clock_skew_ms": 1000,
            "critical_clock_skew_ms": 5000,
            "warn_certificate_participation_ppm": 800000,
            "warn_disk_available_ppm": 150000,
            "critical_disk_available_ppm": 50000,
            "warn_proof_verify_ms": 5000.0,
            "critical_proof_verify_ms": 15000.0,
            "critical_proof_metric_stale_ms": 300000,
            "warn_rpc_active_connection_ppm": 750000,
            "critical_rpc_active_connection_ppm": 950000,
            "include_account_tx_history": False,
            "account_tx_history_require_row": False,
        }
        values.update(overrides)
        return argparse.Namespace(**values)

    def monitor(self, **overrides: object) -> dict[str, object]:
        monitor: dict[str, object] = {
            "ok": True,
            "block_height": 10,
            "validator_count": 1,
            "rpc_latency": {"p95_ms": 10.0},
            "write_posture": "read_only",
            "observed_unix_ms": 1000,
            "consensus": {
                "recent_certificate_window_blocks": 10,
                "local_recent_certificate_participation_ppm": 1_000_000,
            },
            "mempool": {"pending_count": 0},
            "recent_receipts": {
                "sample_count": 1,
                "accepted_count": 1,
                "rejected_count": 0,
                "unknown_count": 0,
            },
            "account_tx_index": {"effective_usable": True, "disk_usable": True},
            "account_canary": {"enabled": False},
            "storage": {
                "filesystem_total_bytes": 1_000_000,
                "filesystem_available_bytes": 500_000,
                "filesystem_available_ppm": 500_000,
            },
            "proofs": {
                "last_verify_micros": 1000,
                "last_observed_unix_ms": 1000,
            },
            "rpc": {
                "active_connections": 1,
                "active_connection_limit": 64,
                "active_connection_utilization_ppm": 15625,
                "peak_active_connections": 1,
                "accepted_connection_count": 10,
            },
        }
        monitor.update(overrides)
        return monitor

    def checks(self, monitor: dict[str, object], **args: object) -> dict[str, object]:
        return self.module["monitor_checks"](
            [monitor],
            {
                "doctor_ok": True,
                "checks": {"chain_consistent": True, "registry_root_consistent": True},
            },
            {"passed": None, "endpoints": []},
            self.args(**args),
        )

    def test_operational_thresholds_fail_closed(self) -> None:
        self.assertEqual(self.checks(self.monitor())["status"], "ok")
        self.assertIn(
            "rpc_latency_p95_critical",
            self.checks(self.monitor(rpc_latency={"p95_ms": 5001.0}))["criticals"],
        )
        self.assertIn(
            "mempool_pending_critical",
            self.checks(self.monitor(mempool={"pending_count": 1001}))["criticals"],
        )
        self.assertIn(
            "disk_available_critical",
            self.checks(
                self.monitor(
                    storage={
                        "filesystem_total_bytes": 1_000_000,
                        "filesystem_available_bytes": 49_999,
                        "filesystem_available_ppm": 49_999,
                    }
                )
            )["criticals"],
        )
        self.assertIn(
            "proof_verify_latency_critical",
            self.checks(
                self.monitor(
                    proofs={
                        "last_verify_micros": 15_000_001,
                        "last_observed_unix_ms": 1000,
                    }
                )
            )["criticals"],
        )
        self.assertIn(
            "rpc_active_connections_critical",
            self.checks(
                self.monitor(
                    rpc={
                        "active_connections": 64,
                        "active_connection_limit": 64,
                        "active_connection_utilization_ppm": 1_000_000,
                        "peak_active_connections": 64,
                        "accepted_connection_count": 100,
                    }
                )
            )["criticals"],
        )
        self.assertIn(
            "recent_rejected_receipts_warn",
            self.checks(
                self.monitor(
                    recent_receipts={
                        "sample_count": 1,
                        "accepted_count": 0,
                        "rejected_count": 1,
                        "unknown_count": 0,
                    }
                )
            )["warnings"],
        )
        self.assertIn(
            "recent_receipt_semantics_unknown",
            self.checks(
                self.monitor(
                    recent_receipts={
                        "sample_count": 1,
                        "accepted_count": 0,
                        "rejected_count": 0,
                        "unknown_count": 1,
                    }
                )
            )["criticals"],
        )
        self.assertIn(
            "local_certificate_participation_warn",
            self.checks(
                self.monitor(
                    consensus={
                        "recent_certificate_window_blocks": 10,
                        "local_recent_certificate_participation_ppm": 799999,
                    }
                )
            )["warnings"],
        )
        clock_report = self.module["monitor_checks"](
            [self.monitor(observed_unix_ms=1000), self.monitor(observed_unix_ms=6001)],
            {
                "doctor_ok": True,
                "checks": {"chain_consistent": True, "registry_root_consistent": True},
            },
            {"passed": None, "endpoints": []},
            self.args(),
        )
        self.assertIn("validator_clock_skew_critical", clock_report["criticals"])

    def test_endpoint_monitor_reads_metrics_from_their_actual_sections(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.module["endpoint_monitor"].__globals__["ROOT"] = root
            metrics_path = root / "metrics.json"
            receipts_path = root / "receipts.json"
            mempool_path = root / "mempool.json"
            metrics_path.write_text(
                json.dumps(
                    {
                        "result": {
                            "observed_unix_ms": 123456,
                            "consensus": {
                                "block_certificate_count": 7,
                                "block_certificate_vote_count": 35,
                                "recent_certificate_window_blocks": 7,
                                "recent_certificate_vote_count": 35,
                                "local_recent_certificate_vote_count": 6,
                                "local_recent_certificate_participation_ppm": 857142,
                            },
                            "ordering": {
                                "block_height": 7,
                                "ordered_batch_count": 7,
                                "archived_batch_count": 6,
                            },
                            "execution": {"receipt_count": 9},
                            "storage": {
                                "replicated_state_file_count": 17,
                                "filesystem_total_bytes": 1000,
                                "filesystem_available_bytes": 250,
                                "filesystem_available_ppm": 250000,
                            },
                            "proofs": {
                                "last_verify_micros": 123456,
                                "last_observed_unix_ms": 123000,
                            },
                            "rpc": {
                                "active_connections": 48,
                                "active_connection_limit": 64,
                                "active_connection_utilization_ppm": 750000,
                                "peak_active_connections": 63,
                                "accepted_connection_count": 100,
                            },
                            "mempool": {"pending": 3},
                        }
                    }
                ),
                encoding="utf-8",
            )
            receipts_path.write_text(
                json.dumps(
                    {
                        "result": [
                            {"accepted": True, "code": "accepted"},
                            {"accepted": False, "code": "bad_sequence"},
                        ]
                    }
                ),
                encoding="utf-8",
            )
            mempool_path.write_text(json.dumps({"result": {"pending": []}}), encoding="utf-8")
            endpoint = {
                "summary": {
                    "ok": True,
                    "write_posture": "read_only",
                    "validator_count": 1,
                },
                "methods": [
                    {"method": "metrics", "response_file": "metrics.json", "latency_ms": 1},
                    {"method": "receipts", "response_file": "receipts.json", "latency_ms": 1},
                    {"method": "mempool_status", "response_file": "mempool.json", "latency_ms": 1},
                ],
            }
            report = self.module["endpoint_monitor"](endpoint)
            self.assertEqual(report["storage"]["block_height"], 7)
            self.assertEqual(report["storage"]["ordered_batch_count"], 7)
            self.assertEqual(report["storage"]["archived_batch_count"], 6)
            self.assertEqual(report["storage"]["receipt_count"], 9)
            self.assertEqual(report["storage"]["replicated_state_file_count"], 17)
            self.assertEqual(report["storage"]["filesystem_available_ppm"], 250000)
            self.assertEqual(report["proofs"]["last_verify_micros"], 123456)
            self.assertEqual(report["rpc"]["active_connections"], 48)
            self.assertEqual(
                report["rpc"]["active_connection_utilization_ppm"], 750000
            )
            self.assertEqual(report["mempool"]["pending_count"], 3)
            self.assertEqual(report["observed_unix_ms"], 123456)
            self.assertEqual(report["consensus"]["block_certificate_count"], 7)
            self.assertEqual(
                report["consensus"]["local_recent_certificate_participation_ppm"],
                857142,
            )
            self.assertEqual(report["recent_receipts"]["accepted_count"], 1)
            self.assertEqual(report["recent_receipts"]["rejected_count"], 1)

    def test_alert_event_spool_is_private_durable_and_contains_semantics(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            spool = Path(temporary) / "alerts"
            report = {
                "schema": "postfiat-testnet-monitor-snapshot-v1",
                "generated_utc": "2026-07-16T08:30:00Z",
                "checks": {
                    "status": "critical",
                    "warnings": ["recent_rejected_receipts_warn"],
                    "criticals": ["chain_inconsistent"],
                    "monitor_ok": False,
                },
                "monitor_ok": False,
            }
            first = self.module["write_alert_event"](report, spool)
            second = self.module["write_alert_event"](report, spool)

            self.assertEqual(first, second)
            self.assertEqual(first.parent, spool)
            self.assertEqual(os.stat(first).st_mode & 0o777, 0o600)
            events = list(spool.glob("*.json"))
            self.assertEqual(events, [first])
            event = json.loads(first.read_text(encoding="utf-8"))
            self.assertEqual(event["schema"], "postfiat.monitor-alert.v1")
            self.assertEqual(event["severity"], "critical")
            self.assertEqual(event["event_id"], first.stem)
            self.assertEqual(event["criticals"], ["chain_inconsistent"])
            self.assertEqual(
                event["warnings"], ["recent_rejected_receipts_warn"]
            )
            self.assertEqual(event["incident"]["severity"], "SEV-1")
            self.assertEqual(event["incident"]["acknowledge_within_minutes"], 5)
            self.assertEqual(event["incident"]["public_update_within_minutes"], 30)
            self.assertEqual(
                event["incident"]["runbook"],
                "docs/runbooks/incident-response.md",
            )

    def test_alert_event_spool_rejects_symlink_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            destination = root / "unexpected-destination"
            destination.mkdir()
            spool = root / "alerts"
            spool.symlink_to(destination, target_is_directory=True)
            report = {
                "schema": "postfiat-testnet-monitor-snapshot-v1",
                "generated_utc": "2026-07-16T08:31:00Z",
                "checks": {
                    "status": "warning",
                    "warnings": ["proof_latency_unavailable"],
                    "criticals": [],
                },
                "monitor_ok": True,
            }
            with self.assertRaisesRegex(RuntimeError, "private directory"):
                self.module["write_alert_event"](report, spool)
            self.assertEqual(list(destination.iterdir()), [])


if __name__ == "__main__":
    unittest.main()
