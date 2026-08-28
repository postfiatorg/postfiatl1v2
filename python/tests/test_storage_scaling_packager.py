from __future__ import annotations

import hashlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from typing import Any

REPO = Path(__file__).resolve().parents[2]
PACKAGER_PATH = REPO / "benchmarks" / "storage-scaling" / "package_packet.py"


def _load_packager() -> Any:
    spec = importlib.util.spec_from_file_location(
        "postfiat_storage_scaling_packager_tests",
        PACKAGER_PATH,
    )
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load storage-scaling packet assembler")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


PACKAGER = _load_packager()
LANE_HEIGHTS = {
    "selected-indexed": [50, 5000],
    "legacy-jsonl": [50],
}


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def _campaign(root: Path, *, label: str = "height-50-window-1") -> Path:
    corpora: dict[int, tuple[Path, str]] = {}
    for height in (50, 5000):
        corpus = root / "corpora" / f"height-{height}.json"
        _write_json(
            corpus,
            {
                "schema": "postfiat-tx-latency-signed-transfer-corpus-v1",
                "transfers": [{"height": height}],
            },
        )
        corpora[height] = (corpus, _sha256(corpus))

    lanes: dict[str, object] = {}
    for lane, heights in LANE_HEIGHTS.items():
        rows = []
        for height in heights:
            normalized = root / "raw" / lane / f"height-{height}.normalized.json"
            resources = root / "raw" / lane / f"height-{height}.resources.json"
            _write_json(normalized, {"lane": lane, "height": height, "kind": "normalized"})
            _write_json(resources, {"lane": lane, "height": height, "kind": "resources"})
            vote_lock = (
                root / "raw" / lane / f"height-{height}.vote-lock-work.json"
            )
            _write_json(
                vote_lock,
                {
                    "schema": "postfiat-storage-vote-lock-work-gate-v1",
                    "passed": True,
                },
            )
            window_label = (
                label
                if lane == "selected-indexed" and height == 50
                else f"height-{height}-window-1"
            )
            rows.append(
                {
                    "height": height,
                    "windows": [
                        {
                            "label": window_label,
                            "signed_transfer_corpus": corpora[height][0]
                            .relative_to(root)
                            .as_posix(),
                            "signed_transfer_corpus_sha256": corpora[height][1],
                            "normalized_report": normalized.relative_to(root).as_posix(),
                            "normalized_report_sha256": _sha256(normalized),
                            "resource_samples": resources.relative_to(root).as_posix(),
                            "resource_samples_sha256": _sha256(resources),
                            "vote_lock_work_receipt": (
                                vote_lock.relative_to(root).as_posix()
                            ),
                            "vote_lock_work_receipt_sha256": _sha256(vote_lock),
                        }
                    ],
                }
            )
        lanes[lane] = {"rows": rows}

    report = {
        "schema": PACKAGER.ARTIFACT_SCHEMAS["performance"],
        "status": "PASS",
        "campaign_mode": "release-qualification",
        "evidence_eligible": True,
        "materials_by_height": [
            {
                "height": height,
                "snapshot": (
                    f"canonical/snapshots/height-{height}.snapshot"
                    if height == 50
                    else None
                ),
                "snapshot_sha256": f"{height:064x}" if height == 50 else None,
                "prepared_fleet": f"prepared-fleets/height-{height}",
                "prepared_fleet_sha256": f"{height + 1:064x}",
                "corpus_source_mode": (
                    "authenticated-portable-snapshot-import"
                    if height == 50
                    else "disposable-canonical-prepared-fleet-clone"
                ),
                "corpus_source_prepared_fleet_sha256": (
                    None if height == 50 else f"{height + 1:064x}"
                ),
                "corpus_scratch_before_sha256": (
                    None if height == 50 else f"{height + 1:064x}"
                ),
                "corpus_scratch_after_sha256": (
                    None if height == 50 else f"{height + 3:064x}"
                ),
                "corpus_scratch_mutated": None if height == 50 else True,
                "corpus_scratch_discarded": None if height == 50 else True,
                "corpus_scratch_restored_sha256": (
                    None if height == 50 else f"{height + 1:064x}"
                ),
                "signed_transfer_corpus": corpus.relative_to(root).as_posix(),
                "signed_transfer_corpus_sha256": digest,
                "transfer_count": 50,
                "first_sequence": 1,
                "last_sequence": 50,
            }
            for height, (corpus, digest) in corpora.items()
        ],
        "lanes": lanes,
    }
    path = root / "campaign-report.json"
    _write_json(path, report)
    return path


class StorageScalingPackagerTests(unittest.TestCase):
    def test_copy_performance_copies_each_bound_resource_stream(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            campaign = root / "campaign"
            campaign.mkdir()
            source = _campaign(campaign)
            packet = root / "packet"

            destination = PACKAGER.copy_performance(packet, source)

            copied = json.loads(destination.read_text(encoding="utf-8"))
            self.assertEqual(
                copied["rows"],
                copied["lanes"]["selected-indexed"]["rows"],
            )
            corpora = {
                entry["height"]: entry
                for entry in copied["materials_by_height"]
            }
            self.assertEqual(set(corpora), {50, 5000})
            for entry in corpora.values():
                self.assertNotIn("prepared_fleet", entry)
                self.assertRegex(entry["prepared_fleet_sha256"], r"^[0-9a-f]{64}$")
                corpus = packet / entry["signed_transfer_corpus"]
                self.assertTrue(corpus.is_file())
                self.assertEqual(
                    _sha256(corpus),
                    entry["signed_transfer_corpus_sha256"],
                )
            for lane, heights in LANE_HEIGHTS.items():
                for row in copied["lanes"][lane]["rows"]:
                    height = row["height"]
                    self.assertIn(height, heights)
                    window = row["windows"][0]
                    normalized = packet / window["normalized_report"]
                    resources = packet / window["resource_samples"]
                    vote_lock = packet / window["vote_lock_work_receipt"]
                    self.assertTrue(normalized.is_file())
                    self.assertTrue(resources.is_file())
                    self.assertTrue(vote_lock.is_file())
                    self.assertEqual(
                        _sha256(normalized),
                        window["normalized_report_sha256"],
                    )
                    self.assertEqual(
                        _sha256(resources),
                        window["resource_samples_sha256"],
                    )
                    self.assertEqual(
                        _sha256(vote_lock),
                        window["vote_lock_work_receipt_sha256"],
                    )
                    self.assertEqual(
                        window["signed_transfer_corpus"],
                        corpora[height]["signed_transfer_corpus"],
                    )
                    self.assertEqual(
                        window["signed_transfer_corpus_sha256"],
                        corpora[height]["signed_transfer_corpus_sha256"],
                    )

    def test_copy_performance_rejects_noncanonical_destination_label(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            campaign = root / "campaign"
            campaign.mkdir()
            source = _campaign(campaign, label="../escape")
            packet = root / "packet"

            with self.assertRaisesRegex(ValueError, "label is not canonical"):
                PACKAGER.copy_performance(packet, source)
            self.assertFalse((root / "escape.json").exists())

    def test_copy_performance_includes_prepared_input_build_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            campaign = root / "campaign"
            campaign.mkdir()
            source = _campaign(campaign)
            report = json.loads(source.read_text(encoding="utf-8"))
            advance_receipt = campaign / "prepared-input" / "advance-receipt.json"
            advance_report = campaign / "prepared-input" / "advance-report.json"
            _write_json(
                advance_receipt,
                {
                    "kind": "receipt",
                    "signed_transfer_corpus": "/host-local/corpus.json",
                },
            )
            _write_json(advance_report, {"kind": "report"})
            manifest = {
                "schema": PACKAGER.PREPARED_INPUT_MANIFEST_SCHEMA,
                "candidate": {"source_revision": "a" * 40},
                "batch_builder": {"binary_sha256": "b" * 64},
                "runner": {"source_revision": "c" * 40},
                "build": {"final_height": 5000},
                "advances": [
                    {
                        "unit_id": "canonical/advance-1-to-5000",
                        "receipt": {"path": "unused", "sha256": _sha256(advance_receipt)},
                        "report": {"path": "unused", "sha256": _sha256(advance_report)},
                    }
                ],
            }
            manifest_path = campaign / "prepared-input" / "manifest.json"
            _write_json(manifest_path, manifest)
            report.update(
                {
                    "input_mode": "prepared-input-manifest",
                    "prepared_input_manifest": manifest_path.relative_to(
                        campaign
                    ).as_posix(),
                    "prepared_input_manifest_sha256": _sha256(manifest_path),
                    "prepared_input_build": {
                        key: manifest[key]
                        for key in ("candidate", "batch_builder", "runner", "build")
                    },
                    "prepared_input_import": {
                        "advances": [
                            {
                                "unit_id": "canonical/advance-1-to-5000",
                                "receipt": advance_receipt.relative_to(
                                    campaign
                                ).as_posix(),
                                "receipt_sha256": _sha256(advance_receipt),
                                "report": advance_report.relative_to(
                                    campaign
                                ).as_posix(),
                                "report_sha256": _sha256(advance_report),
                            }
                        ]
                    },
                }
            )
            _write_json(source, report)
            packet = root / "packet"

            destination = PACKAGER.copy_performance(packet, source)

            copied = json.loads(destination.read_text(encoding="utf-8"))
            copied_manifest = packet / copied["prepared_input_manifest"]
            self.assertEqual(_sha256(copied_manifest), _sha256(manifest_path))
            imported = copied["prepared_input_import"]["advances"][0]
            self.assertTrue((packet / imported["receipt"]).is_file())
            self.assertTrue((packet / imported["report"]).is_file())
            self.assertEqual(
                imported["source_receipt_sha256"],
                manifest["advances"][0]["receipt"]["sha256"],
            )
            copied_receipt = json.loads(
                (packet / imported["receipt"]).read_text(encoding="utf-8")
            )
            self.assertEqual(
                copied_receipt["signed_transfer_corpus"],
                "$SIGNED_TRANSFER_CORPUS",
            )

    def test_copy_performance_rejects_high_height_portable_snapshot(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            campaign = root / "campaign"
            campaign.mkdir()
            source = _campaign(campaign)
            report = json.loads(source.read_text(encoding="utf-8"))
            high = report["materials_by_height"][1]
            high["snapshot"] = "canonical/snapshots/height-5000.snapshot"
            high["snapshot_sha256"] = "f" * 64
            _write_json(source, report)

            with self.assertRaisesRegex(ValueError, "prepared corpus binding"):
                PACKAGER.copy_performance(root / "packet", source)

    def test_copy_performance_rejects_scratch_not_cloned_from_source(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            campaign = root / "campaign"
            campaign.mkdir()
            source = _campaign(campaign)
            report = json.loads(source.read_text(encoding="utf-8"))
            report["materials_by_height"][1][
                "corpus_scratch_before_sha256"
            ] = "f" * 64
            _write_json(source, report)

            with self.assertRaisesRegex(ValueError, "prepared corpus binding"):
                PACKAGER.copy_performance(root / "packet", source)

    def test_resolve_campaign_file_rejects_parent_escape(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            campaign = root / "campaign"
            campaign.mkdir()
            outside = root / "outside.json"
            _write_json(outside, {"outside": True})

            with self.assertRaisesRegex(ValueError, "path is unsafe"):
                PACKAGER.resolve_campaign_file(
                    campaign,
                    "../outside.json",
                    _sha256(outside),
                    "test artifact",
                )


if __name__ == "__main__":
    unittest.main()
