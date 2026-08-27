from __future__ import annotations

import importlib.util
import json
import stat
import tempfile
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


class StorageScalingPairedRunnerTests(unittest.TestCase):
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
            source_sha256 = PAIRED.BASE.directory_digest(source)
            working = root / "canonical" / "nodes"
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
            self.assertEqual(provenance["scratch_before_sha256"], source_sha256)
            self.assertNotEqual(
                provenance["scratch_after_sha256"],
                provenance["scratch_before_sha256"],
            )
            self.assertTrue(provenance["scratch_mutated"])
            self.assertTrue(provenance["scratch_discarded"])
            self.assertFalse(working.exists())

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
            source_sha256 = PAIRED.BASE.directory_digest(source)
            working = root / "canonical" / "nodes"

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
            with self.assertRaisesRegex(ValueError, "digest changed"):
                PAIRED.BASE.clone_prepared_fleet(
                    source,
                    destination,
                    expected,
                )

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
                    configuration={},
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
