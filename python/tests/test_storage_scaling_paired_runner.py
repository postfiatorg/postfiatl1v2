from __future__ import annotations

import importlib.util
import hashlib
import hmac
import json
import os
import stat
import tempfile
import threading
import time
import unittest
from pathlib import Path
from types import SimpleNamespace
from typing import Any
from unittest import mock

REPO = Path(__file__).resolve().parents[2]
PAIRED_PATH = REPO / "benchmarks" / "storage-scaling" / "run_paired_campaign.py"


def _load_paired() -> Any:
    spec = importlib.util.spec_from_file_location(
        "postfiat_storage_scaling_paired_runner_tests",
        PAIRED_PATH,
    )
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load paired storage campaign runner")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


PAIRED = _load_paired()


def _bind_synthetic_generation_pointers(
    prepared_fleet: Path,
    working_fleet: Path,
) -> None:
    for index in range(PAIRED.BASE.VALIDATORS):
        pointer = {
            "schema": "postfiat-transactional-generation-pointer-v1",
            "generation": "generation-00000001",
            "database_directory": str(
                (
                    working_fleet
                    / f"validator-{index}"
                    / "transactional-snapshot-generation-v1"
                ).resolve()
            ),
            "database_file": "postfiat-state-v1.redb",
            "migration_packet_root": "1" * 96,
        }
        path = (
            prepared_fleet / f"validator-{index}" / "transactional_generation.json"
        )
        path.write_text(
            json.dumps(pointer, indent=2, sort_keys=True)
            + "\n"
            + f"pftmac1:{'2' * 96}\n",
            encoding="utf-8",
        )


def _write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def _vote_lock_report(
    round_overrides: list[dict[str, dict[str, object]]],
) -> dict[str, object]:
    iterations = []
    for overrides in round_overrides:
        targets = []
        for index in range(PAIRED.BASE.VALIDATORS - 1):
            node_id = f"validator-{index}"
            timing: dict[str, object] = {
                "vote_lock_files_examined": 3,
                "vote_lock_bytes_decoded": 4_096,
                "vote_lock_migration_performed": False,
            }
            timing.update(overrides.get(node_id, {}))
            targets.append(
                {
                    "target": node_id,
                    "result": "ok",
                    "vote_request_breakdown": {
                        "remote_handling": {
                            "block_vote_breakdown": timing,
                        }
                    },
                }
            )
        iterations.append(
            {
                "round_timings": {
                    "vote_request_targets": targets,
                }
            }
        )
    return {"iterations": iterations}


def _vote_lock_report_with_targets(
    rounds: list[tuple[list[str], dict[str, dict[str, object]]]],
) -> dict[str, object]:
    iterations = []
    for target_ids, overrides in rounds:
        targets = []
        for node_id in target_ids:
            timing: dict[str, object] = {
                "vote_lock_files_examined": 3,
                "vote_lock_bytes_decoded": 4_096,
                "vote_lock_migration_performed": False,
            }
            timing.update(overrides.get(node_id, {}))
            targets.append(
                {
                    "target": node_id,
                    "result": "ok",
                    "vote_request_breakdown": {
                        "remote_handling": {"block_vote_breakdown": timing}
                    },
                }
            )
        iterations.append({"round_timings": {"vote_request_targets": targets}})
    return {"iterations": iterations}


def _certified_send_report(
    rounds: list[tuple[str, dict[str, object]]],
) -> dict[str, object]:
    iterations = []
    for node_id, overrides in rounds:
        timings: dict[str, object] = {
            "outbox_resume_node_id": node_id,
            "outbox_tombstones_validated": 0,
            "outbox_files_read": 0,
            "outbox_bytes_hashed": 0,
            "outbox_index_files_read": 1,
            "outbox_index_bytes_read": 128,
            "outbox_completed_entries_enumerated": 0,
            "outbox_jobs_compacted": 0,
            "outbox_jobs_pruned": 0,
            "outbox_index_migration_performed": False,
        }
        timings.update(overrides)
        iterations.append(
            {
                "source_node": "validator-0",
                "round_timings": timings,
            }
        )
    return {"iterations": iterations}


def _round_coverage_report(residuals_ms: list[float]) -> dict[str, object]:
    iterations = []
    for residual_ms in residuals_ms:
        timings: dict[str, object] = {
            field: 10.0 for field in PAIRED.BASE.ROUND_COVERAGE_STAGE_FIELDS
        }
        timings["shielded_verifier_prewarm"] = {"total_ms": 5.0}
        timings["total_ms"] = (
            5.0
            + 10.0 * len(PAIRED.BASE.ROUND_COVERAGE_STAGE_FIELDS)
            + residual_ms
        )
        iterations.append(
            {"source_node": "validator-0", "round_timings": timings}
        )
    return {"iterations": iterations}


def _synthetic_fleet(root: Path, height: int) -> tuple[Path, str]:
    fleet = root / "prepared-fleets" / f"canonical-height-{height}"
    for index in range(PAIRED.BASE.VALIDATORS):
        validator = fleet / f"validator-{index}"
        validator.mkdir(parents=True)
        (validator / "state.bin").write_bytes(f"{height}:{index}".encode())
    return fleet, PAIRED.BASE.directory_digest(fleet)


def _synthetic_prepared_campaign(root: Path) -> dict[str, Any]:
    source_revision = "a" * 40
    runner_revision = "b" * 40
    private = root / "shared" / "private"
    (private / "seed").mkdir(parents=True)
    validator_records = [
        {
            "node_id": f"validator-{index}",
            "algorithm_id": "ML-DSA-65",
            "public_key_hex": bytes([index + 1]).hex(),
        }
        for index in range(PAIRED.BASE.VALIDATORS)
    ]
    _write_json(
        private / "seed" / "validator_keys.json",
        {"validators": validator_records},
    )
    (private / "wallet.key.json").write_text(
        "synthetic private wallet material\n", encoding="utf-8"
    )
    topology = root / "shared" / "topology.json"
    _write_json(topology, {"validators": 6})
    height_one = root / "shared" / "snapshots" / "height-1.snapshot"
    height_one.mkdir(parents=True)
    (height_one / "state.bin").write_bytes(b"height one")
    height_50_snapshot = root / "canonical" / "snapshots" / "height-50.snapshot"
    height_50_snapshot.mkdir(parents=True)
    (height_50_snapshot / "state.bin").write_bytes(b"height fifty")
    fleet_50, fleet_50_digest = _synthetic_fleet(root, 50)
    fleet_5000, fleet_5000_digest = _synthetic_fleet(root, 5000)

    corpora: dict[int, tuple[Path, str]] = {}
    for height in (50, 5000):
        corpus = root / "corpora" / f"height-{height}.json"
        transfers = [
            {
                "unsigned": {
                    "from": "pf-test-wallet",
                    "to": "pf-test-recipient",
                    "amount": 10,
                    "sequence": height + offset,
                },
                "algorithm_id": "ML-DSA-65",
                "public_key_hex": "01",
                "signature_hex": "02",
            }
            for offset in range(2)
        ]
        _write_json(
            corpus,
            {
                "schema": "postfiat-tx-latency-signed-transfer-corpus-v1",
                "transfers": transfers,
            },
        )
        corpora[height] = (corpus, PAIRED.sha256(corpus))

    completed: dict[str, dict[str, Any]] = {}
    for start, final, fleet_digest in (
        (1, 50, fleet_50_digest),
        (50, 5000, fleet_5000_digest),
    ):
        label = f"advance-{start}-to-{final}"
        rounds = final - start
        counters = {
            "committed_write_transactions": rounds * PAIRED.BASE.VALIDATORS,
            "page_reads": rounds * 12,
            "page_writes": rounds * 6,
            "full_history_scans": 0,
            "full_history_records_read": 0,
            "full_history_bytes_read": 0,
        }
        final_fleet = [
            {
                "node_id": f"validator-{index}",
                "height": final,
                "tip": f"{final:096x}",
                "state_root": f"{final + 1:096x}",
            }
            for index in range(PAIRED.BASE.VALIDATORS)
        ]
        result = {
            "label": label,
            "starting_height": start,
            "final_height": final,
            "rounds": rounds,
            "validators_converged": PAIRED.BASE.VALIDATORS,
            "literal_receipts_exact": True,
            "backend_work_gate_pass": True,
            "zero_full_history_reads": True,
            "batch_builder_binary_sha256": "c" * 64,
            "batch_builder_build": {
                "git_revision": runner_revision[:8],
                "profile": "release",
            },
            "storage": {**counters, "transactional": dict(counters)},
            "final_tip": f"{final:096x}",
            "final_state_root": f"{final + 1:096x}",
            "final_fleet": final_fleet,
            "result_prepared_fleet_sha256": fleet_digest,
            "normalized_report": f"normalized/{label}.report.json",
        }
        report = root / "canonical" / result["normalized_report"]
        _write_json(report, {"label": label, "status": "passed"})
        result["normalized_report_sha256"] = PAIRED.sha256(report)
        receipt = root / "canonical" / "receipts" / f"{label}.json"
        _write_json(receipt, result)
        completed[f"canonical/{label}"] = {
            "kind": "advance",
            "runner_root": "canonical",
            "result": result,
            "receipt": receipt.relative_to(root).as_posix(),
            "receipt_sha256": PAIRED.sha256(receipt),
            "elapsed_seconds": float(rounds),
        }

    identities = [
        {
            "node_id": f"validator-{index}",
            "algorithm_id": "ML-DSA-65",
            "public_key_sha256": hashlib.sha256(bytes([index + 1])).hexdigest(),
        }
        for index in range(PAIRED.BASE.VALIDATORS)
    ]
    checkpoint = {
        "schema": PAIRED.CHECKPOINT_SCHEMA,
        "campaign_schema": PAIRED.SCHEMA,
        "status": "FAILED",
        "started_at": "2026-08-28T00:00:00Z",
        "elapsed_wall_seconds": 1234.5,
        "source_revision": source_revision,
        "runner_source_revision": runner_revision,
        "node_binary_sha256": "d" * 64,
        "node_binary_build": {
            "git_revision": source_revision[:8],
            "profile": "release",
        },
        "batch_builder_binary_sha256": "c" * 64,
        "runner_bindings": {
            "spec_sha3_384": "e" * 96,
            "paired_runner_sha256": "f" * 64,
            "selected_runner_sha256": "1" * 64,
            "shared_runner_sha256": "2" * 64,
            "vote_lock_work_gate_schema": PAIRED.BASE.VOTE_LOCK_WORK_GATE_SCHEMA,
            "certified_send_work_gate_schema": PAIRED.BASE.CERTIFIED_SEND_WORK_GATE_SCHEMA,
            "round_coverage_gate_schema": PAIRED.BASE.ROUND_COVERAGE_GATE_SCHEMA,
        },
        "public_inputs": {
            "validator_public_identities": identities,
            "topology_sha256": PAIRED.sha256(topology),
            "height_1_snapshot_sha256": PAIRED.BASE.directory_digest(height_one),
        },
        "private_paths": {"topology": topology.relative_to(root).as_posix()},
        "completed_units": completed,
        "height_materials": {
            "50": {
                "height": 50,
                "snapshot": height_50_snapshot.relative_to(root).as_posix(),
                "snapshot_sha256": PAIRED.BASE.directory_digest(height_50_snapshot),
                "prepared_fleet": fleet_50.relative_to(root).as_posix(),
                "prepared_fleet_sha256": fleet_50_digest,
                "signed_transfer_corpus": corpora[50][0].relative_to(root).as_posix(),
                "signed_transfer_corpus_sha256": corpora[50][1],
                "transfer_count": 2,
                "first_sequence": 50,
                "last_sequence": 51,
                "corpus_source_mode": "authenticated-portable-snapshot-import",
                "corpus_source_prepared_fleet_sha256": None,
                "corpus_scratch_before_sha256": None,
                "corpus_scratch_after_sha256": None,
                "corpus_scratch_mutated": None,
                "corpus_scratch_discarded": None,
                "corpus_scratch_restored_sha256": None,
            },
            "5000": {
                "height": 5000,
                "snapshot": None,
                "snapshot_sha256": None,
                "prepared_fleet": fleet_5000.relative_to(root).as_posix(),
                "prepared_fleet_sha256": fleet_5000_digest,
                "signed_transfer_corpus": corpora[5000][0].relative_to(root).as_posix(),
                "signed_transfer_corpus_sha256": corpora[5000][1],
                "transfer_count": 2,
                "first_sequence": 5000,
                "last_sequence": 5001,
                "corpus_source_mode": "disposable-canonical-prepared-fleet-clone",
                "corpus_source_prepared_fleet_sha256": fleet_5000_digest,
                "corpus_scratch_before_sha256": fleet_5000_digest,
                "corpus_scratch_after_sha256": "3" * 64,
                "corpus_scratch_mutated": True,
                "corpus_scratch_discarded": True,
                "corpus_scratch_restored_sha256": fleet_5000_digest,
            },
        },
    }
    _write_json(root / "campaign-checkpoint.json", checkpoint)
    return checkpoint


def _bind_synthetic_campaign_binaries(
    root: Path,
    checkpoint: dict[str, Any],
    node_bin: Path,
    batch_builder_bin: Path,
) -> None:
    checkpoint["node_binary_sha256"] = PAIRED.sha256(node_bin)
    checkpoint["batch_builder_binary_sha256"] = PAIRED.sha256(batch_builder_bin)
    for record in checkpoint["completed_units"].values():
        result = record["result"]
        result["batch_builder_binary_sha256"] = PAIRED.sha256(batch_builder_bin)
        receipt = root / record["receipt"]
        _write_json(receipt, result)
        record["receipt_sha256"] = PAIRED.sha256(receipt)
    _write_json(root / "campaign-checkpoint.json", checkpoint)


class StorageScalingPairedRunnerTests(unittest.TestCase):
    def test_vote_lock_gate_accepts_bounded_work_and_first_round_migration(
        self,
    ) -> None:
        report = _vote_lock_report(
            [
                {
                    "validator-0": {
                        "vote_lock_files_examined": 4_999,
                        "vote_lock_bytes_decoded": 21_000_000,
                        "vote_lock_migration_performed": True,
                    }
                },
                {},
            ]
        )

        gate = PAIRED.BASE.vote_lock_work_from_report(report)

        self.assertTrue(gate["passed"])
        self.assertEqual(gate["reason_codes"], [])
        validator = gate["validators"]["validator-0"]
        self.assertEqual(validator["migration_rounds"], [1])
        self.assertEqual(validator["votes_observed"], 2)

    def test_vote_lock_gate_rejects_migration_twice(self) -> None:
        migration = {
            "validator-0": {
                "vote_lock_files_examined": 4_999,
                "vote_lock_bytes_decoded": 21_000_000,
                "vote_lock_migration_performed": True,
            }
        }
        report = _vote_lock_report([migration, migration])

        gate = PAIRED.BASE.vote_lock_work_from_report(report)

        self.assertFalse(gate["passed"])
        self.assertIn(
            PAIRED.BASE.VOTE_LOCK_REASON_MIGRATION_REPEATED,
            gate["reason_codes"],
        )

    def test_vote_lock_gate_rejects_late_migration(self) -> None:
        report = _vote_lock_report(
            [
                {},
                {
                    "validator-0": {
                        "vote_lock_files_examined": 4_999,
                        "vote_lock_bytes_decoded": 21_000_000,
                        "vote_lock_migration_performed": True,
                    }
                },
            ]
        )

        gate = PAIRED.BASE.vote_lock_work_from_report(report)

        self.assertFalse(gate["passed"])
        self.assertEqual(
            gate["reason_codes"],
            [PAIRED.BASE.VOTE_LOCK_REASON_MIGRATION_LATE],
        )

    def test_vote_lock_gate_rejects_oversized_bytes(self) -> None:
        report = _vote_lock_report(
            [
                {
                    "validator-0": {
                        "vote_lock_bytes_decoded": 4_097,
                    }
                }
            ]
        )

        gate = PAIRED.BASE.vote_lock_work_from_report(report)

        self.assertFalse(gate["passed"])
        self.assertEqual(
            gate["reason_codes"],
            [PAIRED.BASE.VOTE_LOCK_REASON_BYTES_EXCEEDED],
        )

    def test_vote_lock_gate_defaults_legacy_absent_fields_to_zero_false(
        self,
    ) -> None:
        report = _vote_lock_report([{}])
        for target in report["iterations"][0]["round_timings"][
            "vote_request_targets"
        ]:
            target["vote_request_breakdown"]["remote_handling"][
                "block_vote_breakdown"
            ] = {}

        gate = PAIRED.BASE.vote_lock_work_from_report(report)

        self.assertTrue(gate["passed"])
        self.assertEqual(gate["reason_codes"], [])
        self.assertTrue(
            all(
                validator["max_files_examined"] == 0
                and validator["max_bytes_decoded"] == 0
                and validator["migration_rounds"] == []
                for validator in gate["validators"].values()
            )
        )

    def test_vote_lock_gate_allows_each_validators_first_use_after_restore(
        self,
    ) -> None:
        report = _vote_lock_report_with_targets(
            [
                ([f"validator-{index}" for index in range(5)], {}),
                (
                    [f"validator-{index}" for index in range(1, 6)],
                    {
                        "validator-5": {
                            "vote_lock_files_examined": 4_999,
                            "vote_lock_bytes_decoded": 21_000_000,
                            "vote_lock_migration_performed": True,
                        }
                    },
                ),
            ]
        )

        gate = PAIRED.BASE.vote_lock_work_from_report(report)

        self.assertTrue(gate["passed"])
        self.assertEqual(gate["validators"]["validator-5"]["migration_rounds"], [2])
        self.assertEqual(gate["validators"]["validator-5"]["votes_observed"], 1)

    def test_certified_send_gate_allows_first_use_in_a_later_global_round(
        self,
    ) -> None:
        report = _certified_send_report(
            [
                ("validator-0", {}),
                (
                    "validator-5",
                    {
                        "outbox_tombstones_validated": 1_024,
                        "outbox_files_read": 3_072,
                        "outbox_bytes_hashed": 1_024,
                        "outbox_index_files_read": 0,
                        "outbox_index_bytes_read": 0,
                        "outbox_completed_entries_enumerated": 1_024,
                        "outbox_index_migration_performed": True,
                    },
                ),
            ]
        )

        gate = PAIRED.BASE.certified_send_work_from_report(report)

        self.assertTrue(gate["passed"])
        self.assertEqual(gate["validators"]["validator-5"]["migration_rounds"], [2])
        self.assertEqual(gate["validators"]["validator-5"]["resumes_observed"], 1)

    def test_certified_send_gate_rejects_repeated_and_late_migration(self) -> None:
        migration = {
            "outbox_tombstones_validated": 1,
            "outbox_files_read": 3,
            "outbox_bytes_hashed": 1,
            "outbox_index_files_read": 0,
            "outbox_index_bytes_read": 0,
            "outbox_completed_entries_enumerated": 1,
            "outbox_index_migration_performed": True,
        }
        report = _certified_send_report(
            [("validator-0", migration), ("validator-0", migration)]
        )

        gate = PAIRED.BASE.certified_send_work_from_report(report)

        self.assertFalse(gate["passed"])
        self.assertIn(
            PAIRED.BASE.CERTIFIED_SEND_REASON_MIGRATION_REPEATED,
            gate["reason_codes"],
        )
        self.assertIn(
            PAIRED.BASE.CERTIFIED_SEND_REASON_MIGRATION_LATE,
            gate["reason_codes"],
        )

    def test_certified_send_gate_rejects_untouched_tombstone_validation(
        self,
    ) -> None:
        report = _certified_send_report(
            [
                (
                    "validator-0",
                    {
                        "outbox_tombstones_validated": 1,
                        "outbox_files_read": 3,
                        "outbox_bytes_hashed": 1,
                    },
                )
            ]
        )

        gate = PAIRED.BASE.certified_send_work_from_report(report)

        self.assertFalse(gate["passed"])
        self.assertEqual(
            gate["reason_codes"],
            [PAIRED.BASE.CERTIFIED_SEND_REASON_UNTOUCHED_VALIDATION],
        )

    def test_certified_send_gate_rejects_unbounded_index_work(self) -> None:
        report = _certified_send_report(
            [
                (
                    "validator-0",
                    {
                        "outbox_index_files_read": 2,
                        "outbox_index_bytes_read": 128,
                    },
                )
            ]
        )

        gate = PAIRED.BASE.certified_send_work_from_report(report)

        self.assertFalse(gate["passed"])
        self.assertEqual(
            gate["reason_codes"],
            [PAIRED.BASE.CERTIFIED_SEND_REASON_INDEX_WORK_EXCEEDED],
        )

    def test_certified_send_gate_rejects_nonmigration_directory_enumeration(
        self,
    ) -> None:
        report = _certified_send_report(
            [
                (
                    "validator-0",
                    {"outbox_completed_entries_enumerated": 1},
                )
            ]
        )

        gate = PAIRED.BASE.certified_send_work_from_report(report)

        self.assertFalse(gate["passed"])
        self.assertEqual(
            gate["reason_codes"],
            [PAIRED.BASE.CERTIFIED_SEND_REASON_INDEX_WORK_EXCEEDED],
        )

    def test_certified_send_gate_defaults_absent_legacy_counters(self) -> None:
        report = {
            "iterations": [
                {
                    "round_timings": {
                        "outbox_resume_node_id": "validator-0",
                    }
                }
            ]
        }

        gate = PAIRED.BASE.certified_send_work_from_report(report)

        self.assertTrue(gate["passed"])
        self.assertEqual(gate["validators"]["validator-0"]["resumes_observed"], 1)

    def test_round_coverage_gate_accepts_small_residual(self) -> None:
        gate = PAIRED.BASE.round_coverage_from_report(
            _round_coverage_report([20.0])
        )

        self.assertTrue(gate["passed"])
        self.assertEqual(gate["max_residual_ms"], 20.0)

    def test_round_coverage_gate_rejects_hidden_outbox_scale_work(self) -> None:
        gate = PAIRED.BASE.round_coverage_from_report(
            _round_coverage_report([1_200.0])
        )

        self.assertFalse(gate["passed"])
        self.assertEqual(
            gate["reason_codes"],
            [PAIRED.BASE.ROUND_COVERAGE_REASON_RESIDUAL_EXCEEDED],
        )

    def test_release_configuration_is_the_time_budgeted_matrix(self) -> None:
        configuration = PAIRED.campaign_configuration(False)

        self.assertEqual(
            configuration["lane_height_matrix"],
            [
                {"lane": "selected-indexed", "height": 50},
                {"lane": "selected-indexed", "height": 5000},
                {"lane": "legacy-jsonl", "height": 50},
            ],
        )
        self.assertEqual(configuration["windows_per_height"], 5)
        self.assertEqual(configuration["rounds_per_window"], 50)
        self.assertEqual(configuration["advance_chunk_rounds"], 1500)
        self.assertEqual(
            configuration["node_preparation_mode"],
            "byte-verified-prepared-fleet-clone",
        )
        self.assertEqual(
            configuration["advance_execution_mode"],
            "persistent-peer-certified-batch-loop",
        )
        self.assertEqual(configuration["max_wall_seconds"], 4 * 60 * 60)

    def test_development_matrix_exercises_snapshot_free_selected_advance(self) -> None:
        configuration = PAIRED.campaign_configuration(True)

        self.assertEqual(
            configuration["lane_height_matrix"],
            [
                {"lane": "selected-indexed", "height": 2},
                {"lane": "selected-indexed", "height": 3},
                {"lane": "legacy-jsonl", "height": 2},
            ],
        )

    def test_export_prepared_input_manifest_binds_failed_campaign_build(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "campaign"
            root.mkdir()
            _synthetic_prepared_campaign(root)
            manifest_path = Path(temporary) / "prepared-input.json"

            manifest = PAIRED.export_prepared_input_manifest(root, manifest_path)

            self.assertEqual(manifest["schema"], PAIRED.PREPARED_INPUT_MANIFEST_SCHEMA)
            self.assertEqual(manifest["build"]["final_height"], 5000)
            self.assertEqual(
                [advance["starting_height"] for advance in manifest["advances"]],
                [1, 50],
            )
            self.assertEqual(
                manifest["build"]["counters"]["committed_write_transactions"],
                (49 + 4950) * PAIRED.BASE.VALIDATORS,
            )
            self.assertEqual(
                [material["height"] for material in manifest["materials"]],
                [50, 5000],
            )
            encoded = manifest_path.read_text(encoding="utf-8")
            self.assertNotIn("synthetic private validator material", encoded)
            self.assertNotIn("synthetic private wallet material", encoded)
            self.assertEqual(json.loads(encoded), manifest)

    def test_derive_prepared_input_manifest_separates_build_and_measurement(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_root = Path(temporary)
            campaign = temporary_root / "campaign"
            campaign.mkdir()
            checkpoint = _synthetic_prepared_campaign(campaign)
            original_node = temporary_root / "original-node"
            original_node.write_bytes(b"original candidate")
            original_builder = temporary_root / "original-builder"
            original_builder.write_bytes(b"original builder")
            _bind_synthetic_campaign_binaries(
                campaign,
                checkpoint,
                original_node,
                original_builder,
            )
            source_manifest_path = temporary_root / "prepared-input.json"
            source_manifest = PAIRED.export_prepared_input_manifest(
                campaign,
                source_manifest_path,
            )
            node_bin = temporary_root / "corrected-postfiat-node"
            node_bin.write_bytes(b"corrected candidate")
            batch_builder_bin = temporary_root / "corrected-batch-builder"
            batch_builder_bin.write_bytes(b"corrected builder")
            corrected_revision = "8" * 40
            runner_revision = "9" * 40
            derived_path = temporary_root / "corrected-prepared-input.json"
            candidate_build_manifest_path = temporary_root / "g1-candidate.json"
            _write_json(
                candidate_build_manifest_path,
                {
                    "schema": "postfiat.storage.corrected_g1_candidate.v1",
                    "status": "PASS",
                    "candidate": {
                        "source_revision": corrected_revision,
                        "binary_sha256": PAIRED.sha256(node_bin),
                        "embedded_build_git_revision": corrected_revision[:8],
                        "embedded_build_profile": "release",
                    },
                },
            )

            derived = PAIRED.derive_prepared_input_manifest(
                source_manifest_path,
                derived_path,
                candidate_build_manifest_path=candidate_build_manifest_path,
                node_bin=node_bin,
                batch_builder_bin=batch_builder_bin,
                expected_source_revision=corrected_revision,
                runner_source_revision=runner_revision,
            )

            self.assertEqual(
                derived["candidate"]["node_binary_sha256"],
                PAIRED.sha256(node_bin),
            )
            self.assertEqual(
                derived["candidate"]["candidate_build_manifest_sha256"],
                PAIRED.sha256(candidate_build_manifest_path),
            )
            self.assertEqual(
                derived["batch_builder"]["binary_sha256"],
                PAIRED.sha256(batch_builder_bin),
            )
            self.assertEqual(derived["runner"]["source_revision"], runner_revision)
            self.assertEqual(
                derived["prepared_by"]["candidate"],
                source_manifest["candidate"],
            )
            self.assertEqual(
                derived["prepared_by"]["batch_builder"],
                source_manifest["batch_builder"],
            )
            self.assertEqual(
                derived["prepared_by"]["source_manifest_sha256"],
                PAIRED.sha256(source_manifest_path),
            )
            self.assertEqual(
                [
                    material["prepared_fleet"]["sha256"]
                    for material in derived["materials"]
                ],
                [
                    material["prepared_fleet"]["sha256"]
                    for material in source_manifest["materials"]
                ],
            )
            PAIRED.verify_prepared_input_sources(derived_path, derived)

            tampered = json.loads(json.dumps(derived))
            tampered["prepared_by"]["batch_builder"]["binary_sha256"] = derived[
                "batch_builder"
            ]["binary_sha256"]
            with self.assertRaisesRegex(ValueError, "receipt differs"):
                PAIRED.verify_prepared_input_sources(derived_path, tampered)

    def test_export_prepared_input_manifest_rejects_noncontiguous_advances(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "campaign"
            root.mkdir()
            checkpoint = _synthetic_prepared_campaign(root)
            checkpoint["completed_units"]["canonical/advance-50-to-5000"]["result"][
                "starting_height"
            ] = 51
            _write_json(root / "campaign-checkpoint.json", checkpoint)

            with self.assertRaisesRegex(ValueError, "not contiguous"):
                PAIRED.export_prepared_input_manifest(
                    root, Path(temporary) / "prepared-input.json"
                )

    def test_export_prepared_input_manifest_rejects_full_history_work(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "campaign"
            root.mkdir()
            checkpoint = _synthetic_prepared_campaign(root)
            record = checkpoint["completed_units"]["canonical/advance-50-to-5000"]
            result = record["result"]
            result["storage"]["full_history_scans"] = 1
            result["storage"]["transactional"]["full_history_scans"] = 1
            receipt = root / record["receipt"]
            _write_json(receipt, result)
            record["receipt_sha256"] = PAIRED.sha256(receipt)
            _write_json(root / "campaign-checkpoint.json", checkpoint)

            with self.assertRaisesRegex(ValueError, "full-history work"):
                PAIRED.export_prepared_input_manifest(
                    root, Path(temporary) / "prepared-input.json"
                )

    def test_export_prepared_input_manifest_rejects_incomplete_top_material(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "campaign"
            root.mkdir()
            checkpoint = _synthetic_prepared_campaign(root)
            checkpoint["height_materials"].pop("5000")
            _write_json(root / "campaign-checkpoint.json", checkpoint)

            with self.assertRaisesRegex(ValueError, "material is incomplete"):
                PAIRED.export_prepared_input_manifest(
                    root, Path(temporary) / "prepared-input.json"
                )

    def test_prepared_input_import_starts_fresh_measurement_checkpoint(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_root = Path(temporary)
            campaign = temporary_root / "campaign"
            campaign.mkdir()
            checkpoint = _synthetic_prepared_campaign(campaign)
            release = temporary_root / "target" / "release"
            release.mkdir(parents=True)
            node_bin = release / "postfiat-node"
            node_bin.write_bytes(b"candidate node")
            batch_builder_bin = release / "postfiat-storage-corpus-batches"
            batch_builder_bin.write_bytes(b"batch builder")
            _bind_synthetic_campaign_binaries(
                campaign, checkpoint, node_bin, batch_builder_bin
            )
            manifest_path = temporary_root / "prepared-input.json"
            manifest = PAIRED.export_prepared_input_manifest(
                campaign, manifest_path
            )
            output = temporary_root / "measurement"
            output.mkdir()

            with mock.patch.object(
                PAIRED,
                "host_description",
                return_value={"synthetic": True},
            ):
                imported = PAIRED.initialize_prepared_campaign(
                    output,
                    manifest_path=manifest_path,
                    node_bin=node_bin,
                    batch_builder_bin=batch_builder_bin,
                    expected_source_revision=manifest["candidate"][
                        "source_revision"
                    ],
                    runner_source_revision="9" * 40,
                    configuration=PAIRED.campaign_configuration(False),
                )

            self.assertEqual(imported["status"], "RUNNING")
            self.assertEqual(imported["elapsed_wall_seconds"], 0.0)
            self.assertEqual(imported["input_mode"], "prepared-input-manifest")
            self.assertEqual(imported["current_height"], 5000)
            self.assertEqual(set(imported["height_materials"]), {"50", "5000"})
            self.assertEqual(
                imported["prepared_input_build"]["build"]["elapsed_seconds"],
                manifest["build"]["elapsed_seconds"],
            )
            self.assertEqual(
                imported["prepared_input_import"]["prepared_fleets"][1][
                    "source_sha256"
                ],
                imported["prepared_input_import"]["prepared_fleets"][1][
                    "destination_sha256"
                ],
            )
            state = PAIRED.CampaignState(output, imported, stop_after_units=None)
            PAIRED.validate_prepared_campaign_binding(state)
            imported_manifest = output / imported["prepared_input_manifest"]
            imported_manifest.write_text(
                imported_manifest.read_text(encoding="utf-8") + " ",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "manifest changed"):
                PAIRED.validate_prepared_campaign_binding(state)

    def test_prepared_input_import_rejects_binary_digest_change(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_root = Path(temporary)
            campaign = temporary_root / "campaign"
            campaign.mkdir()
            _synthetic_prepared_campaign(campaign)
            manifest_path = temporary_root / "prepared-input.json"
            PAIRED.export_prepared_input_manifest(campaign, manifest_path)
            release = temporary_root / "target" / "release"
            release.mkdir(parents=True)
            node_bin = release / "postfiat-node"
            node_bin.write_bytes(b"different candidate")
            batch_builder_bin = release / "postfiat-storage-corpus-batches"
            batch_builder_bin.write_bytes(b"different helper")
            output = temporary_root / "measurement"
            output.mkdir()

            with self.assertRaisesRegex(ValueError, "node binary digest differs"):
                PAIRED.initialize_prepared_campaign(
                    output,
                    manifest_path=manifest_path,
                    node_bin=node_bin,
                    batch_builder_bin=batch_builder_bin,
                    expected_source_revision="a" * 40,
                    runner_source_revision="9" * 40,
                    configuration=PAIRED.campaign_configuration(False),
                )

    def test_prepared_generation_pointer_rebase_preserves_authentication(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            nodes = Path(temporary) / "nodes"
            domain = b"postfiat.storage.state-file.v1:state file"
            for index in range(PAIRED.BASE.VALIDATORS):
                validator = nodes / f"validator-{index}"
                database = validator / "transactional-snapshot-generation-v1"
                database.mkdir(parents=True)
                (database / "postfiat-state-v1.redb").write_bytes(bytes([index]))
                key = bytes([index + 1]) * 48
                key_path = validator / ".integrity.key"
                key_path.write_bytes(key)
                key_path.chmod(0o600)
                pointer = {
                    "schema": "postfiat-transactional-generation-pointer-v1",
                    "generation": "generation-00000001",
                    "database_directory": (
                        f"/old/campaign/validator-{index}/"
                        "transactional-snapshot-generation-v1"
                    ),
                    "database_file": "postfiat-state-v1.redb",
                    "migration_packet_root": "a" * 96,
                }
                body = json.dumps(pointer, indent=2).encode("utf-8")
                tag = hmac.new(
                    key, domain + b"\x00" + body, hashlib.sha3_384
                ).hexdigest()
                (validator / "transactional_generation.json").write_bytes(
                    body + b"\npftmac1:" + tag.encode("ascii") + b"\n"
                )

            PAIRED.BASE.rebase_prepared_generation_pointers(nodes)

            for index in range(PAIRED.BASE.VALIDATORS):
                validator = nodes / f"validator-{index}"
                raw = (validator / "transactional_generation.json").read_bytes()
                body, trailer = raw.rstrip().rsplit(b"\n", 1)
                pointer = json.loads(body)
                self.assertEqual(
                    Path(pointer["database_directory"]),
                    (
                        validator / "transactional-snapshot-generation-v1"
                    ).resolve(),
                )
                key = (validator / ".integrity.key").read_bytes()
                self.assertEqual(
                    trailer.decode(),
                    "pftmac1:"
                    + hmac.new(
                        key, domain + b"\x00" + body, hashlib.sha3_384
                    ).hexdigest(),
                )

    def test_data_dir_corpus_creation_uses_supplied_node_without_snapshot(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "validator-0"
            source.mkdir()
            (source / "state.bin").write_bytes(b"stopped prepared state")
            output = root / "corpus.json"
            logs = root / "logs"
            logs.mkdir()
            before = PAIRED.BASE.directory_digest(source)

            def fake_run(command: list[str], **_kwargs: Any) -> Any:
                self.assertIn("tx-latency-corpus-create", command)
                self.assertNotIn("snapshot-import", command)
                self.assertEqual(
                    Path(command[command.index("--data-dir") + 1]), source
                )
                corpus = {
                    "schema": "postfiat-tx-latency-signed-transfer-corpus-v1",
                    "transfers": [{"sequence": 1}, {"sequence": 2}],
                }
                output.write_text(
                    json.dumps(corpus, sort_keys=True) + "\n", encoding="utf-8"
                )
                return SimpleNamespace(
                    stdout=json.dumps(
                        {
                            "transfer_count": 2,
                            "sha256": PAIRED.BASE.digest(output),
                        }
                    )
                )

            with mock.patch.object(PAIRED.BASE.SHARED, "run", side_effect=fake_run):
                report = PAIRED.BASE.create_signed_transfer_corpus(
                    node_bin=Path("postfiat-node"),
                    source_data_dir=source,
                    wallet_key=Path("wallet.json"),
                    wallet_address="wallet",
                    recipient="recipient",
                    count=2,
                    output_file=output,
                    logs=logs,
                    label="prepared",
                )

            self.assertEqual(report["transfer_count"], 2)
            self.assertEqual(PAIRED.BASE.directory_digest(source), before)
            self.assertFalse((root / ".prepared.corpus-node").exists())

    def test_corpus_creation_requires_one_source_kind(self) -> None:
        arguments = {
            "node_bin": Path("postfiat-node"),
            "wallet_key": Path("wallet.json"),
            "wallet_address": "wallet",
            "recipient": "recipient",
            "count": 1,
            "output_file": Path("corpus.json"),
            "logs": Path("logs"),
            "label": "invalid",
        }
        with self.assertRaisesRegex(ValueError, "exactly one source"):
            PAIRED.BASE.create_signed_transfer_corpus(**arguments)
        with self.assertRaisesRegex(ValueError, "exactly one source"):
            PAIRED.BASE.create_signed_transfer_corpus(
                **arguments,
                source_snapshot=Path("snapshot"),
                source_data_dir=Path("node"),
            )

    def test_prepared_corpus_generation_binds_and_discards_mutated_scratch(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "prepared"
            for index in range(PAIRED.BASE.VALIDATORS):
                validator = source / f"validator-{index}"
                validator.mkdir(parents=True)
                (validator / "state.bin").write_bytes(bytes([index]))
            working = root / "canonical" / "nodes"
            _bind_synthetic_generation_pointers(source, working)
            source_sha256 = PAIRED.BASE.directory_digest(source)
            output = root / "corpus.json"

            def create_corpus(**arguments: Any) -> dict[str, Any]:
                data_dir = arguments["source_data_dir"]
                self.assertEqual(data_dir, working / "validator-0")
                (data_dir / "state.bin").write_bytes(b"mutated scratch")
                output.write_text("{}\n", encoding="utf-8")
                return {
                    "transfer_count": 2,
                    "first_sequence": 7,
                    "last_sequence": 8,
                }

            with mock.patch.object(
                PAIRED.BASE,
                "create_signed_transfer_corpus",
                side_effect=create_corpus,
            ):
                report, provenance = PAIRED.create_corpus_from_prepared_fleet(
                    node_bin=Path("postfiat-node"),
                    prepared_fleet=source,
                    prepared_fleet_sha256=source_sha256,
                    working_fleet=working,
                    wallet_key=Path("wallet.json"),
                    wallet_address="wallet",
                    recipient="recipient",
                    count=2,
                    expected_first_sequence=7,
                    output_file=output,
                    logs=root / "logs",
                    label="height-7",
                )

            self.assertEqual(report["last_sequence"], 8)
            self.assertEqual(PAIRED.BASE.directory_digest(source), source_sha256)
            self.assertEqual(
                provenance["source_prepared_fleet_sha256"], source_sha256
            )
            self.assertEqual(
                provenance["mode"],
                "disposable-canonical-prepared-fleet-clone",
            )
            self.assertEqual(provenance["scratch_before_sha256"], source_sha256)
            self.assertEqual(
                provenance["scratch_restored_sha256"],
                source_sha256,
            )
            self.assertNotEqual(
                provenance["scratch_after_sha256"],
                provenance["scratch_before_sha256"],
            )
            self.assertTrue(provenance["scratch_mutated"])
            self.assertTrue(provenance["scratch_discarded"])
            self.assertTrue(working.is_dir())
            self.assertEqual(
                PAIRED.BASE.directory_digest(working),
                source_sha256,
            )

    def test_prepared_corpus_generation_rejects_wrong_sequence_before_discard(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "prepared"
            for index in range(PAIRED.BASE.VALIDATORS):
                validator = source / f"validator-{index}"
                validator.mkdir(parents=True)
                (validator / "state.bin").write_bytes(bytes([index]))
            working = root / "canonical" / "nodes"
            _bind_synthetic_generation_pointers(source, working)
            source_sha256 = PAIRED.BASE.directory_digest(source)

            with mock.patch.object(
                PAIRED.BASE,
                "create_signed_transfer_corpus",
                return_value={
                    "transfer_count": 1,
                    "first_sequence": 99,
                    "last_sequence": 99,
                },
            ):
                with self.assertRaisesRegex(RuntimeError, "sequence differs"):
                    PAIRED.create_corpus_from_prepared_fleet(
                        node_bin=Path("postfiat-node"),
                        prepared_fleet=source,
                        prepared_fleet_sha256=source_sha256,
                        working_fleet=working,
                        wallet_key=Path("wallet.json"),
                        wallet_address="wallet",
                        recipient="recipient",
                        count=1,
                        expected_first_sequence=7,
                        output_file=root / "corpus.json",
                        logs=root / "logs",
                        label="height-7",
                    )

            self.assertEqual(PAIRED.BASE.directory_digest(source), source_sha256)
            self.assertTrue(working.is_dir())
            self.assertEqual(
                PAIRED.BASE.directory_digest(working),
                source_sha256,
            )

    def test_prepared_corpus_generation_rejects_noncanonical_pointer(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "prepared"
            for index in range(PAIRED.BASE.VALIDATORS):
                validator = source / f"validator-{index}"
                validator.mkdir(parents=True)
                (validator / "state.bin").write_bytes(bytes([index]))
            _bind_synthetic_generation_pointers(source, root / "wrong-nodes")
            source_sha256 = PAIRED.BASE.directory_digest(source)
            with mock.patch.object(
                PAIRED.BASE,
                "create_signed_transfer_corpus",
            ) as create_corpus:
                with self.assertRaisesRegex(
                    RuntimeError,
                    "does not bind the canonical working fleet",
                ):
                    PAIRED.create_corpus_from_prepared_fleet(
                        node_bin=Path("postfiat-node"),
                        prepared_fleet=source,
                        prepared_fleet_sha256=source_sha256,
                        working_fleet=root / "canonical" / "nodes",
                        wallet_key=Path("wallet.json"),
                        wallet_address="wallet",
                        recipient="recipient",
                        count=1,
                        expected_first_sequence=7,
                        output_file=root / "corpus.json",
                        logs=root / "logs",
                        label="height-7",
                    )
            create_corpus.assert_not_called()

    def test_prepared_fleet_clone_is_byte_verified(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            destination = root / "destination"
            for index in range(PAIRED.BASE.VALIDATORS):
                validator = source / f"validator-{index}"
                validator.mkdir(parents=True)
                (validator / "state.bin").write_bytes(bytes([index, 0, 255]))
            expected = PAIRED.BASE.directory_digest(source)

            observed = PAIRED.BASE.clone_prepared_fleet(
                source,
                destination,
                expected,
            )

            self.assertEqual(observed, expected)
            self.assertEqual(PAIRED.BASE.directory_digest(destination), expected)
            (source / "validator-0" / "state.bin").write_bytes(b"changed")
            with self.assertRaisesRegex(RuntimeError, "expected digest"):
                PAIRED.BASE.clone_prepared_fleet(
                    source,
                    destination,
                    expected,
                )

    def test_prepared_fleet_clone_incrementally_restores_exact_content(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            destination = root / "destination"
            for index in range(PAIRED.BASE.VALIDATORS):
                validator = source / f"validator-{index}"
                validator.mkdir(parents=True)
                (validator / "state.bin").write_bytes(bytes([index, 0, 255]))
            expected = PAIRED.BASE.directory_digest(source)
            PAIRED.BASE.clone_prepared_fleet(source, destination, expected)

            unchanged = destination / "validator-2" / "state.bin"
            unchanged_inode = unchanged.stat().st_ino
            (destination / "validator-0" / "state.bin").write_bytes(b"mutated")
            (destination / "validator-1" / "state.bin").unlink()
            (destination / "validator-3" / "extra.bin").write_bytes(b"extra")

            observed = PAIRED.BASE.clone_prepared_fleet(
                source,
                destination,
                expected,
            )

            self.assertEqual(observed, expected)
            self.assertEqual(PAIRED.BASE.directory_digest(destination), expected)
            self.assertEqual(unchanged.stat().st_ino, unchanged_inode)
            self.assertFalse((destination / "validator-3" / "extra.bin").exists())

    def test_prepared_fleet_clone_falls_back_on_same_metadata_tamper(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            destination = root / "destination"
            for index in range(PAIRED.BASE.VALIDATORS):
                validator = source / f"validator-{index}"
                validator.mkdir(parents=True)
                (validator / "state.bin").write_bytes(bytes([index, 0, 255]))
            expected = PAIRED.BASE.directory_digest(source)
            PAIRED.BASE.clone_prepared_fleet(source, destination, expected)

            source_file = source / "validator-0" / "state.bin"
            destination_file = destination / "validator-0" / "state.bin"
            destination_file.write_bytes(b"bad")
            source_metadata = source_file.stat()
            os.utime(
                destination_file,
                ns=(source_metadata.st_atime_ns, source_metadata.st_mtime_ns),
            )
            copytree = PAIRED.BASE.shutil.copytree
            with mock.patch.object(
                PAIRED.BASE.shutil,
                "copytree",
                wraps=copytree,
            ) as copytree_mock:
                observed = PAIRED.BASE.clone_prepared_fleet(
                    source,
                    destination,
                    expected,
                )

            self.assertEqual(observed, expected)
            self.assertEqual(destination_file.read_bytes(), source_file.read_bytes())
            self.assertGreaterEqual(copytree_mock.call_count, 1)

    def test_resource_sampler_is_ready_before_measurement(self) -> None:
        stop_event = threading.Event()
        samples: list[dict[str, Any]] = []
        include_disk_calls: list[bool] = []

        def fake_resource_sample(
            _pids: list[int],
            _nodes: Path,
            *,
            include_disk: bool,
        ) -> dict[str, Any]:
            include_disk_calls.append(include_disk)
            time.sleep(0.05)
            return {
                "monotonic_ns": time.monotonic_ns(),
                "host_cpu_ticks": 0,
                "host_memory": {"total_kib": 1, "available_kib": 1},
                "network": {"received": 0, "transmitted": 0},
                "node_disk_bytes": 0 if include_disk else None,
                "processes": {},
            }

        started = time.monotonic()
        with mock.patch.object(
            PAIRED.BASE.SHARED,
            "resource_sample",
            side_effect=fake_resource_sample,
        ):
            sample_thread = PAIRED.BASE.SHARED.start_resource_sampler(
                stop_event,
                lambda: [],
                Path("."),
                samples,
            )
            elapsed = time.monotonic() - started
            self.assertGreaterEqual(elapsed, 0.04)
            self.assertEqual(len(samples), 1)
            stop_event.set()
            sample_thread.join()

        self.assertEqual(include_disk_calls, [True, True])
        self.assertEqual(len(samples), 2)

    def test_atomic_checkpoint_replaces_with_private_file(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "campaign-checkpoint.json"

            PAIRED.atomic_write_checkpoint(path, {"generation": 1})
            PAIRED.atomic_write_checkpoint(path, {"generation": 2})

            self.assertEqual(
                json.loads(path.read_text(encoding="utf-8")),
                {"generation": 2},
            )
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
            self.assertFalse(path.with_name(f".{path.name}.tmp").exists())

    def test_completed_unit_persists_exact_timing(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            checkpoint = {
                "configuration": {"max_wall_seconds": 900},
                "elapsed_wall_seconds": 0.0,
                "completed_units": {"material/height-50": {"kind": "material"}},
                "current_unit": None,
                "status": "RUNNING",
            }
            state = PAIRED.CampaignState(root, checkpoint, stop_after_units=None)

            state.begin_unit("material/height-50")
            state.finish_unit()

            completed = state.value["completed_units"]["material/height-50"]
            self.assertRegex(completed["started_at"], r"Z$")
            self.assertRegex(completed["finished_at"], r"Z$")
            self.assertGreaterEqual(completed["elapsed_seconds"], 0.0)
            self.assertIsNone(state.value["current_unit"])

    def test_failed_unit_and_last_stop_persist_message_and_timing(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            checkpoint = {
                "configuration": {"max_wall_seconds": 900},
                "elapsed_wall_seconds": 0.0,
                "completed_units": {},
                "failed_units": [],
                "current_unit": None,
                "status": "RUNNING",
            }
            state = PAIRED.CampaignState(root, checkpoint, stop_after_units=None)
            state.begin_unit("selected-indexed/height-50-window-1")

            state.mark_interrupted(RuntimeError("resource sampler missed process"))

            failed = state.value["failed_units"][0]
            self.assertEqual(
                failed["error_message"], "resource sampler missed process"
            )
            self.assertRegex(failed["started_at"], r"Z$")
            self.assertRegex(failed["finished_at"], r"Z$")
            self.assertGreaterEqual(failed["elapsed_seconds"], 0.0)
            self.assertEqual(
                state.value["last_stop"],
                {
                    "at": state.value["last_stop"]["at"],
                    "type": "RuntimeError",
                    "message": "resource sampler missed process",
                },
            )

    def test_quarantine_preserves_partial_unit_instead_of_overwriting(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            runner = root / "lanes" / "selected-indexed"
            partial = runner / "height-2-window-1"
            partial.mkdir(parents=True)
            (partial / "report.json").write_text("{}\n", encoding="utf-8")
            nodes = runner / "nodes"
            nodes.mkdir()
            (nodes / "partial").write_text("partial\n", encoding="utf-8")
            corpus = root / "corpora" / "partial.json"
            corpus.parent.mkdir()
            corpus.write_text("{}\n", encoding="utf-8")
            prepared_fleet = root / "prepared-fleets" / "height-2"
            prepared_fleet.mkdir(parents=True)
            (prepared_fleet / "partial").write_text("partial\n", encoding="utf-8")
            checkpoint = {
                "configuration": {"max_wall_seconds": 900},
                "elapsed_wall_seconds": 0.0,
                "current_unit": {
                    "unit_id": "selected-indexed/height-2-window-1",
                    "runner_root": "lanes/selected-indexed",
                    "label": "height-2-window-1",
                    "owned_corpus": "corpora/partial.json",
                    "owned_prepared_fleet": "prepared-fleets/height-2",
                },
                "interrupted_units": [],
                "status": "INTERRUPTED",
            }
            state = PAIRED.CampaignState(
                root,
                checkpoint,
                stop_after_units=None,
            )

            PAIRED.quarantine_current_unit(state)

            self.assertFalse(partial.exists())
            self.assertFalse(nodes.exists())
            self.assertFalse(corpus.exists())
            self.assertFalse(prepared_fleet.exists())
            self.assertIsNone(state.value["current_unit"])
            self.assertEqual(state.value["status"], "RUNNING")
            discarded = state.value["interrupted_units"][0]
            self.assertRegex(discarded["started_at"], r"Z$")
            self.assertRegex(discarded["finished_at"], r"Z$")
            self.assertGreaterEqual(discarded["elapsed_seconds"], 0.0)
            quarantined = list((root / "interrupted").iterdir())
            self.assertEqual(len(quarantined), 1)
            self.assertTrue(
                (quarantined[0] / "height-2-window-1" / "report.json").is_file()
            )
            self.assertTrue((quarantined[0] / "nodes" / "partial").is_file())
            self.assertTrue(
                (quarantined[0] / "corpora" / "partial.json").is_file()
            )
            self.assertTrue(
                (
                    quarantined[0]
                    / "prepared-fleets"
                    / "height-2"
                    / "partial"
                ).is_file()
            )

    def test_complete_checkpoint_is_not_resumable(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            checkpoint = {
                "status": "COMPLETE",
                "elapsed_wall_seconds": 0.0,
            }
            state = PAIRED.CampaignState(
                root,
                checkpoint,
                stop_after_units=None,
            )

            with self.assertRaisesRegex(ValueError, "not resumable"):
                PAIRED.validate_checkpoint(
                    state,
                    expected_source_revision="0" * 40,
                    node_bin=Path("/does/not/matter"),
                    batch_builder_bin=Path("/does/not/matter-either"),
                    configuration={},
                )

    def test_persistent_advance_iteration_binds_receipt_and_six_states(self) -> None:
        digest96 = "a" * 96
        states = [
            {
                "node_id": f"validator-{index}",
                "block_height": 2,
                "block_tip_hash": digest96,
                "state_root": "b" * 96,
            }
            for index in range(6)
        ]
        batch = {
            "corpus_index": 0,
            "batch_file": "round-000001.batch.json",
            "batch_id": "c" * 96,
            "tx_id": "d" * 96,
            "signed_transfer_sha256": "e" * 64,
        }
        round_report = {
            "schema": "postfiat-transport-peer-certified-batch-round-v1",
            "round_ok": True,
            "from": "validator-0",
            "batch_file": "/tmp/round-000001.batch.json",
            "proposal_proposer": "validator-2",
            "proposal_signed": True,
            "proposal_signature_signer": "validator-2",
            "require_local_proposer": False,
            "require_signed_proposal": True,
            "allow_peer_failures": False,
            "local_apply_before_certified_send": True,
            "certified_sends_deferred": False,
            "all_vote_requests_verified": True,
            "all_sends_verified": True,
            "local_receipt_count": 1,
            "local_accepted_count": 1,
            "local_rejected_count": 0,
            "vote_request_failures": [],
            "send_failures": [],
            "unresolved_vote_targets": [],
            "skipped_certified_send_targets": [],
            "certification": {
                "certificate_id": "f" * 96,
                "block_height": 2,
                "vote_count": 6,
            },
            "timings": {
                "client_visible_finality_ms": 10.0,
                "total_ms": 11.0,
                "certified_sends_ms": 2.0,
                "local_apply_ms": 1.0,
                "local_apply_breakdown": {
                    "write_commit_ms": 0.5,
                    "write_commit_breakdown": {
                        "refresh_account_tx_index_ms": 0.0,
                    },
                },
            },
            "local_hot_finality": [
                {
                    "tx_id": batch["tx_id"],
                    "confirmed": True,
                    "receipt": {"tx_id": batch["tx_id"], "accepted": True},
                    "block": {
                        "header": {
                            "height": 2,
                            "batch_id": batch["batch_id"],
                            "certificate_id": "f" * 96,
                            "block_hash": digest96,
                        }
                    },
                }
            ],
            "local_state": states[0],
            "sends": [
                {
                    "verified": True,
                    "ack": {
                        "applied": True,
                        "receipt_count": 1,
                        "accepted_count": 1,
                        "rejected_count": 0,
                        "certified_state": state,
                    },
                }
                for state in states[1:]
            ],
        }

        iteration = PAIRED.BASE.persistent_advance_iteration(
            round_report,
            batch,
            iteration=1,
            block_height=2,
        )

        self.assertEqual(iteration["tx_id"], batch["tx_id"])
        self.assertEqual(iteration["source_node"], "validator-2")
        self.assertTrue(iteration["receipt_accepted"])
        round_report["sends"][0]["ack"]["accepted_count"] = 0
        with self.assertRaisesRegex(RuntimeError, "certified send differs"):
            PAIRED.BASE.persistent_advance_iteration(
                round_report,
                batch,
                iteration=1,
                block_height=2,
            )

    def test_targeted_height_model_accepts_two_height_profile(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            rows = []
            for height, value in ((50, 10.0), (5000, 10.5)):
                label = f"height-{height}-window-1"
                normalized = root / f"{label}.json"
                normalized.write_text(
                    json.dumps(
                        {
                            "iterations": [
                                {
                                    "round_timings": {
                                        "proposal_ms": value,
                                        "verification_ms": value,
                                        "vote_requests_ms": value,
                                        "local_vote_ms": value,
                                        "certificate_ms": value,
                                        "local_apply_ms": value,
                                        "certified_sends_ms": value,
                                        "post_apply_status_ms": value,
                                        "local_commit_publish_ms": value,
                                        "local_apply_breakdown": {
                                            "write_commit_ms": value,
                                        },
                                    }
                                }
                            ]
                        }
                    )
                    + "\n",
                    encoding="utf-8",
                )
                rows.append(
                    {
                        "height": height,
                        "windows": [
                            {
                                "label": label,
                                "normalized_report": normalized.name,
                            }
                        ],
                    }
                )

            models = PAIRED.BASE.height_relationship_models(
                rows,
                root,
                expected_heights=[50, 5000],
                rounds_per_window=1,
            )

            self.assertEqual(set(models), set(PAIRED.BASE.MATERIAL_STAGE_PATHS))
            self.assertTrue(
                all(model["sample_count"] == 2 for model in models.values())
            )


if __name__ == "__main__":
    unittest.main()
