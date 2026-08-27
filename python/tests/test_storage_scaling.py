from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from postfiat_rpc.storage_scaling import (
    ARTIFACT_SCHEMAS,
    MANIFEST_FILE,
    MATERIAL_STAGE_PATHS,
    REQUIRED_TAMPER_CASES,
    StorageScalingVerificationError,
    serve_verified_packet,
    verify_packet,
)


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _write_checksums(packet: Path) -> None:
    files = sorted(
        path
        for path in packet.rglob("*")
        if path.is_file() and path.name != "SHA256SUMS.txt"
    )
    (packet / "SHA256SUMS.txt").write_text(
        "".join(
            f"{_sha256(path)}  {path.relative_to(packet).as_posix()}\n"
            for path in files
        ),
        encoding="utf-8",
    )


def _reference(packet: Path, path: Path) -> dict[str, str]:
    return {
        "path": path.relative_to(packet).as_posix(),
        "sha256": _sha256(path),
    }


def _passing_packet(packet: Path) -> None:
    packet.mkdir()
    digest96 = "a" * 96
    revision = "b" * 40
    binary = packet / "bin" / "postfiat-node"
    binary.parent.mkdir()
    binary.write_bytes(b"release-binary-identity")
    binaries = [{"path": "bin/postfiat-node", "sha256": _sha256(binary)}]

    replay_receipts = []
    for height, source_kind in (
        (915, "quarantine_archive"),
        (924, "authenticated_history"),
    ):
        path = packet / "replay" / f"height-{height}.json"
        _write_json(
            path,
            {
                "schema": "postfiat-storage-replay-receipt-v1",
                "source_height": height,
                "source_kind": source_kind,
                "block_count": height,
                "commitment_mode": "legacy_below_storage_activation",
                "exact_replay": True,
                "full_replay_passed": True,
                "logical_rebuild_identical": True,
                "canonical_export_identical": True,
                "tip_hash": digest96,
                "state_root": digest96,
                "ordered_history_accumulator": digest96,
                "chain_id": "postfiat-wan-devnet-2",
                "genesis_hash": (
                    "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad255"
                    "21aff3ed334da07e150a7233a3e90a9"
                ),
                "source_revision": revision,
                "node_binary_sha256": _sha256(binary),
            },
        )
        replay_receipts.append(_reference(packet, path))

    rows = []
    for index, height in enumerate([50, 100, 500, 1000, 5000]):
        consensus = 100.0 + index
        wallet = 105.0 + index
        windows = []
        for window_index in range(5):
            raw_path = (
                packet
                / "performance"
                / f"height-{height}-window-{window_index + 1}.json"
            )
            _write_json(
                raw_path,
                {
                    "schema": "postfiat-real-transaction-latency-benchmark-v1",
                    "status": "passed",
                    "iterations": [
                        {
                            "round_ok": True,
                            "receipt_accepted": True,
                            "finality_confirmed": True,
                            "consensus_round_ms": consensus,
                            "wallet_to_finality_ms": wallet,
                            "round_timings": {
                                "proposal_ms": 10.0,
                                "verification_ms": 10.0,
                                "vote_requests_ms": 10.0,
                                "local_vote_ms": 10.0,
                                "certificate_ms": 10.0,
                                "local_apply_ms": 10.0,
                                "certified_sends_ms": 10.0,
                                "post_apply_status_ms": 10.0,
                                "local_commit_publish_ms": 10.0,
                                "local_apply_breakdown": {
                                    "write_commit_ms": 10.0,
                                },
                            },
                        }
                        for _ in range(50)
                    ],
                },
            )
            windows.append(
                {
                    "rounds": 50,
                    "validators_converged": 6,
                    "literal_receipts_exact": True,
                    "zero_full_history_reads": True,
                    "bounded_index_pages": True,
                    "constant_accumulator_work": True,
                    "storage": {
                        "committed_write_transactions": 300,
                        "fsync_count": 300,
                        "full_history_scans": 0,
                        "full_history_records_read": 0,
                        "full_history_bytes_read": 0,
                    },
                    "resources": {
                        "cpu_ticks": 10,
                        "peak_rss_kib": 20,
                        "disk_growth_bytes": 30,
                        "bytes_read": 40,
                        "bytes_written": 50,
                        "page_reads": 6,
                        "page_writes": 7,
                        "fsync_count": 300,
                        "fsync_micros": 8,
                    },
                    "normalized_report": raw_path.relative_to(packet).as_posix(),
                    "normalized_report_sha256": _sha256(raw_path),
                }
            )
        rows.append(
            {
                "height": height,
                "windows": windows,
                "aggregate": {
                    "consensus_round_ms": {"p95": consensus},
                    "wallet_to_finality_ms": {"p95": wallet},
                },
            }
        )

    tamper_cases = []
    for name in sorted(REQUIRED_TAMPER_CASES):
        receipt_path = packet / "tamper" / f"{name}.json"
        receipt = {
            "schema": "postfiat-storage-tamper-receipt-v1",
            "name": name,
            "passed": True,
            "reason_code": f"rejected_{name}",
            "no_partial_mutation": True,
            "terminal_state": "rejected_voting_blocked",
            "source_revision": revision,
        }
        _write_json(receipt_path, receipt)
        tamper_cases.append(
            {
                **{key: value for key, value in receipt.items() if key != "schema"},
                "receipt": _reference(packet, receipt_path),
            }
        )

    height_relationship_stages = {
        stage: {
            "slope_ms_per_height": 0.0,
            "intercept_ms": 10.0,
            "residual_rmse_ms": 0.0,
            "r_squared": 0.0,
            "sample_kind": "per_window_p95",
            "sample_count": 25,
            "height_50_window_p95_median_ms": 10.0,
            "predicted_delta_50_to_5000_ms": 0.0,
            "material_threshold_ms": 1.0,
            "relative_materiality": 0.10,
            "residual_sigmas": 2.0,
            "material_positive_linear_relationship": False,
        }
        for stage in MATERIAL_STAGE_PATHS
    }

    reports = {
        "source": {
            "schema": ARTIFACT_SCHEMAS["source"],
            "git_revision": revision,
            "spec_sha3_384": digest96,
            "binaries": binaries,
            "clean_checkout": True,
            "build_profile": "release",
        },
        "replay": {
            "schema": ARTIFACT_SCHEMAS["replay"],
            "quarantine_archive_blocks": 915,
            "authenticated_history_height": 924,
            "exact_pre_activation_replay": True,
            "full_replay_passed": True,
            "logical_rebuild_identical": True,
            "canonical_export_identical": True,
            "tip_hash": digest96,
            "state_root": digest96,
            "ordered_history_accumulator": digest96,
            "receipts": replay_receipts,
            "source_revision": revision,
            "node_binary_sha256": _sha256(binary),
        },
        "performance": {
            "schema": ARTIFACT_SCHEMAS["performance"],
            "status": "PASS",
            "campaign_mode": "release-qualification",
            "evidence_eligible": True,
            "source_revision": revision,
            "node_binary_sha256": _sha256(binary),
            "validator_count": 6,
            "windows_per_height": 5,
            "rounds_per_window": 50,
            "legacy_height_50_baseline": {
                "consensus_round_ms": 100.0,
                "wallet_to_finality_ms": 105.0,
            },
            "rows": rows,
            "height_relationship_model": {
                "schema": "postfiat-storage-height-relationship-model-v1",
                "sample_kind": "per_window_p95",
                "relative_materiality": 0.10,
                "residual_sigmas": 2.0,
                "stages": height_relationship_stages,
            },
            "no_positive_linear_height_relationship": True,
        },
        "tamper": {
            "schema": ARTIFACT_SCHEMAS["tamper"],
            "source_revision": revision,
            "cases": tamper_cases,
        },
        "migration": {
            "schema": ARTIFACT_SCHEMAS["migration"],
            "source_revision": revision,
            "node_binary_sha256": _sha256(binary),
            "source_height": 924,
            "chain_id": "postfiat-wan-devnet-2",
            "genesis_hash": (
                "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad255"
                "21aff3ed334da07e150a7233a3e90a9"
            ),
            "exact_existing_chain_rehearsal": True,
            "clone_count": 6,
            "offline_rebuild": True,
            "second_logical_scan": True,
            "generation_pointer_published": True,
            "pre_activation_restart": True,
            "activation": True,
            "pre_activation_cancellation": True,
            "catch_up": True,
            "pre_activation_rollback": True,
            "post_activation_forward_recovery": True,
            "all_six_converged": True,
            "mixed_version_refused": True,
            "backup_verified": True,
            "identities": {
                "source_tip": digest96,
                "source_state_root": digest96,
                "packet_root": digest96,
                "activation_id": digest96,
            },
        },
        "redaction": {
            "schema": ARTIFACT_SCHEMAS["redaction"],
            "passed": True,
            "allowed_nonlocal_ip_files": [],
        },
    }
    artifacts = {}
    for label, report in reports.items():
        path = packet / "artifacts" / f"{label}.json"
        _write_json(path, report)
        artifacts[label] = _reference(packet, path)

    manifest = {
        "schema": "postfiat-storage-scaling-evidence-packet-v1",
        "status": "PASS",
        "captured_at": "2026-08-26T00:00:00Z",
        "source": {
            "git_revision": revision,
            "spec_sha3_384": digest96,
            "binaries": binaries,
        },
        "state_distinction": {
            label: {
                "exact_identifier": f"{label}-{digest96}",
                "observed_at": "2026-08-26T00:00:00Z",
                "freshness": "recorded evidence",
                **({"live_probe": False} if label == "live" else {}),
            }
            for label in ("live", "deployed", "repository")
        },
        "artifacts": artifacts,
    }
    _write_json(packet / MANIFEST_FILE, manifest)
    _write_checksums(packet)


class StorageScalingVerifierTests(unittest.TestCase):
    def packet_dir(self, temporary: str) -> Path:
        return Path(temporary) / "packet"

    def test_storage_scaling_packet_verifies_offline(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)
            verified = verify_packet(packet)
            self.assertIs(verified.report["verified"], True)
            self.assertIs(verified.report["offline"], True)
            self.assertIs(verified.report["live_probe_performed"], False)
            self.assertEqual(
                verified.report["tamper_case_count"], len(REQUIRED_TAMPER_CASES)
            )

    def test_storage_scaling_packet_rejects_checksum_tamper(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)
            manifest = packet / MANIFEST_FILE
            manifest.write_text(
                manifest.read_text(encoding="utf-8") + " ", encoding="utf-8"
            )
            with self.assertRaisesRegex(
                StorageScalingVerificationError, "checksum mismatch"
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_nonpassing_claim(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)
            manifest_path = packet / MANIFEST_FILE
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["status"] = "PUBLIC TESTNET BLOCKED"
            _write_json(manifest_path, manifest)
            _write_checksums(packet)
            with self.assertRaisesRegex(
                StorageScalingVerificationError, "status is not PASS"
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_raw_window_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)
            raw = packet / "performance" / "height-50-window-1.json"
            report = json.loads(raw.read_text(encoding="utf-8"))
            report["iterations"][0]["round_ok"] = False
            _write_json(raw, report)
            performance_path = packet / "artifacts" / "performance.json"
            performance = json.loads(performance_path.read_text(encoding="utf-8"))
            performance["rows"][0]["windows"][0][
                "normalized_report_sha256"
            ] = _sha256(raw)
            _write_json(performance_path, performance)
            manifest_path = packet / MANIFEST_FILE
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["artifacts"]["performance"]["sha256"] = _sha256(
                performance_path
            )
            _write_json(manifest_path, manifest)
            _write_checksums(packet)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "performance iteration round_ok",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_manifest_only_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)
            manifest_path = packet / MANIFEST_FILE
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest.pop("artifacts")
            _write_json(manifest_path, manifest)
            _write_checksums(packet)
            with self.assertRaisesRegex(
                StorageScalingVerificationError, "artifacts"
            ):
                verify_packet(packet)

    def test_storage_scaling_browser_refuses_non_loopback_bind(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)
            verified = verify_packet(packet)
            with self.assertRaisesRegex(StorageScalingVerificationError, "loopback"):
                serve_verified_packet(verified, "0.0.0.0", 0)


if __name__ == "__main__":
    unittest.main()
