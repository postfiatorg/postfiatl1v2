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
LANES = ("legacy-jsonl", "bounded-jsonl", "selected-indexed")


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def _campaign(root: Path, *, label: str = "height-50-window-1") -> Path:
    lanes: dict[str, object] = {}
    for lane in LANES:
        normalized = root / "raw" / lane / "normalized.json"
        resources = root / "raw" / lane / "resources.json"
        _write_json(normalized, {"lane": lane, "kind": "normalized"})
        _write_json(resources, {"lane": lane, "kind": "resources"})
        lanes[lane] = {
            "rows": [
                {
                    "height": 50,
                    "windows": [
                        {
                            "label": label,
                            "normalized_report": normalized.relative_to(root).as_posix(),
                            "normalized_report_sha256": _sha256(normalized),
                            "resource_samples": resources.relative_to(root).as_posix(),
                            "resource_samples_sha256": _sha256(resources),
                        }
                    ],
                }
            ]
        }
    report = {
        "schema": PACKAGER.ARTIFACT_SCHEMAS["performance"],
        "status": "PASS",
        "campaign_mode": "release-qualification",
        "evidence_eligible": True,
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
            for lane in LANES:
                window = copied["lanes"][lane]["rows"][0]["windows"][0]
                normalized = packet / window["normalized_report"]
                resources = packet / window["resource_samples"]
                self.assertTrue(normalized.is_file())
                self.assertTrue(resources.is_file())
                self.assertEqual(_sha256(normalized), window["normalized_report_sha256"])
                self.assertEqual(_sha256(resources), window["resource_samples_sha256"])

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
