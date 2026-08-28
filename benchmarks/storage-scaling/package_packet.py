#!/usr/bin/env python3
"""Assemble and self-verify a redaction-safe storage-scaling evidence packet."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
from datetime import datetime
from pathlib import Path, PurePosixPath
from typing import Any

REPO = Path(__file__).resolve().parents[2]
PYTHON_ROOT = REPO / "python"
SPEC = REPO / "docs" / "architecture" / "storage-scaling-fix-spec.md"
ARTIFACT_SCHEMAS = {
    "source": "postfiat-storage-source-identity-v1",
    "replay": "postfiat-storage-scaling-replay-v1",
    "performance": "postfiat-storage-scaling-time-budgeted-six-validator-campaign-v3",
    "tamper": "postfiat-storage-scaling-tamper-matrix-v1",
    "migration": "postfiat-storage-scaling-six-clone-migration-v1",
    "redaction": "postfiat-storage-scaling-redaction-v1",
}
SENSITIVE = re.compile(
    r"private[-_ ]?key(?![A-Za-z0-9_])|secret|password|mnemonic|spending[-_ ]?key|"
    r"full[-_ ]?viewing[-_ ]?key|master[-_ ]?seed|rseed|ssh[-_ ]?cred",
    re.IGNORECASE,
)
LOCAL_PATH = re.compile(r"(?:/home/|/Users/|[A-Za-z]:\\\\Users\\\\)")
NONLOCAL_IPV4 = re.compile(
    r"(?<![0-9.])(?!127\.0\.0\.1\b)(?:[0-9]{1,3}\.){3}[0-9]{1,3}(?![0-9.])"
)


def read_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_bytes())
    if not isinstance(value, dict):
        raise ValueError(f"expected JSON object: {path}")
    return value


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def reference(packet: Path, path: Path) -> dict[str, str]:
    return {
        "path": path.relative_to(packet).as_posix(),
        "sha256": sha256(path),
    }


def resolve_report_reference(report_path: Path, value: Any) -> Path:
    if not isinstance(value, dict):
        raise ValueError(f"report reference must be an object in {report_path}")
    raw = value.get("path")
    expected = value.get("sha256")
    if not isinstance(raw, str) or not isinstance(expected, str):
        raise ValueError(f"report reference is incomplete in {report_path}")
    pure = PurePosixPath(raw)
    if pure.is_absolute() or any(part in {"", ".", ".."} for part in pure.parts):
        raise ValueError(f"report reference is unsafe in {report_path}: {raw}")
    source = report_path.parent.joinpath(*pure.parts).resolve()
    if sha256(source) != expected:
        raise ValueError(f"report reference digest mismatch: {source}")
    return source


def copy_file(source: Path, destination: Path) -> None:
    if source.is_symlink() or not source.is_file():
        raise ValueError(f"source is not a regular file: {source}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    if destination.exists():
        raise ValueError(f"refusing to overwrite packet entry: {destination}")
    shutil.copyfile(source, destination)


def resolve_campaign_file(
    campaign_root: Path,
    raw: Any,
    expected_sha256: Any,
    label: str,
) -> Path:
    if not isinstance(raw, str) or not isinstance(expected_sha256, str):
        raise ValueError(f"{label} omitted its bound file")
    relative = PurePosixPath(raw)
    if relative.is_absolute() or any(
        part in {"", ".", ".."} for part in relative.parts
    ):
        raise ValueError(f"{label} path is unsafe")
    candidate = campaign_root.joinpath(*relative.parts)
    if candidate.is_symlink():
        raise ValueError(f"{label} must not be a symlink")
    source = candidate.resolve()
    if not source.is_relative_to(campaign_root):
        raise ValueError(f"{label} escaped campaign root")
    if not source.is_file():
        raise ValueError(f"{label} is not a regular file")
    if sha256(source) != expected_sha256:
        raise ValueError(f"{label} digest mismatch")
    return source


def copy_replay(packet: Path, source: Path) -> Path:
    report = read_json(source)
    if report.get("schema") != ARTIFACT_SCHEMAS["replay"]:
        raise ValueError("replay report schema mismatch")
    rewritten = []
    receipts = report.get("receipts")
    if not isinstance(receipts, list):
        raise ValueError("replay report omitted receipts")
    for value in receipts:
        receipt_source = resolve_report_reference(source, value)
        receipt = read_json(receipt_source)
        height = receipt.get("source_height")
        destination = packet / "replay" / f"height-{height}.json"
        copy_file(receipt_source, destination)
        rewritten.append(reference(packet, destination))
    report["receipts"] = rewritten
    destination = packet / "artifacts" / "replay.json"
    write_json(destination, report)
    return destination


def copy_performance(packet: Path, source: Path) -> Path:
    report = read_json(source)
    if (
        report.get("schema") != ARTIFACT_SCHEMAS["performance"]
        or report.get("status") != "PASS"
        or report.get("campaign_mode") != "release-qualification"
        or report.get("evidence_eligible") is not True
    ):
        raise ValueError("performance report is not an evidence-eligible PASS")
    campaign_root = source.parent.resolve()
    materials = report.get("materials_by_height")
    required_heights = [50, 5000]
    if (
        not isinstance(materials, list)
        or [entry.get("height") if isinstance(entry, dict) else None for entry in materials]
        != required_heights
    ):
        raise ValueError("performance report omitted the closed height-material set")
    corpora_by_height: dict[int, tuple[str, str]] = {}
    for entry in materials:
        if not isinstance(entry, dict):
            raise ValueError("performance height-material entry is malformed")
        height = int(entry["height"])
        if re.fullmatch(
            r"[0-9a-f]{64}",
            str(entry.get("prepared_fleet_sha256", "")),
        ) is None:
            raise ValueError(
                f"performance height-{height} prepared fleet digest is invalid"
            )
        if height == 50:
            if (
                not isinstance(entry.get("snapshot"), str)
                or re.fullmatch(
                    r"[0-9a-f]{64}", str(entry.get("snapshot_sha256", ""))
                )
                is None
                or entry.get("corpus_source_mode")
                != "authenticated-portable-snapshot-import"
                or entry.get("corpus_source_prepared_fleet_sha256") is not None
                or any(
                    entry.get(field) is not None
                    for field in (
                        "corpus_scratch_before_sha256",
                        "corpus_scratch_after_sha256",
                        "corpus_scratch_mutated",
                        "corpus_scratch_discarded",
                    )
                )
            ):
                raise ValueError("performance height-50 snapshot binding is invalid")
        elif (
            entry.get("snapshot") is not None
            or entry.get("snapshot_sha256") is not None
            or entry.get("corpus_source_mode")
            != "disposable-canonical-prepared-fleet-clone"
            or entry.get("corpus_source_prepared_fleet_sha256")
            != entry.get("prepared_fleet_sha256")
            or re.fullmatch(
                r"[0-9a-f]{64}",
                str(entry.get("corpus_scratch_before_sha256", "")),
            )
            is None
            or re.fullmatch(
                r"[0-9a-f]{64}",
                str(entry.get("corpus_scratch_after_sha256", "")),
            )
            is None
            or entry.get("corpus_scratch_before_sha256")
            != entry.get("prepared_fleet_sha256")
            or entry.get("corpus_scratch_mutated")
            is not (
                entry.get("corpus_scratch_before_sha256")
                != entry.get("corpus_scratch_after_sha256")
            )
            or entry.get("corpus_scratch_discarded") is not True
        ):
            raise ValueError(
                f"performance height-{height} prepared corpus binding is invalid"
            )
        entry.pop("prepared_fleet", None)
        corpus_source = resolve_campaign_file(
            campaign_root,
            entry.get("signed_transfer_corpus"),
            entry.get("signed_transfer_corpus_sha256"),
            f"performance height-{height} signed transfer corpus",
        )
        corpus_destination = packet / "performance" / "corpora" / f"height-{height}.json"
        copy_file(corpus_source, corpus_destination)
        corpus_path = corpus_destination.relative_to(packet).as_posix()
        corpus_digest = sha256(corpus_destination)
        entry["signed_transfer_corpus"] = corpus_path
        entry["signed_transfer_corpus_sha256"] = corpus_digest
        corpora_by_height[height] = (corpus_path, corpus_digest)

    lanes = report.get("lanes")
    required_lane_heights = {
        "selected-indexed": [50, 5000],
        "legacy-jsonl": [50],
    }
    if not isinstance(lanes, dict) or set(lanes) != set(required_lane_heights):
        raise ValueError("performance report omitted the time-budgeted lane set")
    for lane_name in ("selected-indexed", "legacy-jsonl"):
        lane = lanes[lane_name]
        if not isinstance(lane, dict) or not isinstance(lane.get("rows"), list):
            raise ValueError(f"performance lane {lane_name} omitted rows")
        if [
            row.get("height") if isinstance(row, dict) else None
            for row in lane["rows"]
        ] != required_lane_heights[lane_name]:
            raise ValueError("performance lane heights differ from the profile")
        for row in lane["rows"]:
            if not isinstance(row, dict) or not isinstance(row.get("windows"), list):
                raise ValueError("performance row is malformed")
            height = row.get("height")
            for window_index, window in enumerate(row["windows"], start=1):
                if not isinstance(window, dict):
                    raise ValueError("performance window is malformed")
                label = f"height-{height}-window-{window_index}"
                if window.get("label") != label:
                    raise ValueError("performance window label is not canonical")
                expected_corpus = corpora_by_height[int(height)]
                if window.get("signed_transfer_corpus_sha256") != expected_corpus[1]:
                    raise ValueError(
                        "performance window used a different signed transfer corpus"
                    )
                window["signed_transfer_corpus"] = expected_corpus[0]
                window["signed_transfer_corpus_sha256"] = expected_corpus[1]
                raw_source = resolve_campaign_file(
                    campaign_root,
                    window.get("normalized_report"),
                    window.get("normalized_report_sha256"),
                    "performance normalized report",
                )
                resource_source = resolve_campaign_file(
                    campaign_root,
                    window.get("resource_samples"),
                    window.get("resource_samples_sha256"),
                    "performance resource samples",
                )
                destination = (
                    packet / "performance" / "windows" / lane_name / f"{label}.json"
                )
                copy_file(raw_source, destination)
                window["normalized_report"] = destination.relative_to(packet).as_posix()
                window["normalized_report_sha256"] = sha256(destination)
                resource_destination = (
                    packet / "performance" / "resources" / lane_name / f"{label}.json"
                )
                copy_file(resource_source, resource_destination)
                window["resource_samples"] = (
                    resource_destination.relative_to(packet).as_posix()
                )
                window["resource_samples_sha256"] = sha256(resource_destination)
    report["rows"] = lanes["selected-indexed"]["rows"]
    destination = packet / "artifacts" / "performance.json"
    write_json(destination, report)
    return destination


def copy_tamper(packet: Path, source: Path) -> Path:
    report = read_json(source)
    if report.get("schema") != ARTIFACT_SCHEMAS["tamper"]:
        raise ValueError("tamper report schema mismatch")
    cases = report.get("cases")
    if not isinstance(cases, list):
        raise ValueError("tamper report omitted cases")
    for case in cases:
        if not isinstance(case, dict):
            raise ValueError("tamper case is malformed")
        receipt_source = resolve_report_reference(source, case.get("receipt"))
        name = str(case.get("name", ""))
        destination = packet / "tamper" / f"{name}.json"
        receipt = read_json(receipt_source)
        test_receipts = receipt.get("test_receipts")
        if not isinstance(test_receipts, list) or not test_receipts:
            raise ValueError("tamper receipt omitted executable test evidence")
        for test_receipt in test_receipts:
            if not isinstance(test_receipt, dict):
                raise ValueError("tamper test receipt is malformed")
            for evidence_key in ("report", "manifest"):
                if evidence_key not in test_receipt:
                    continue
                evidence_source = resolve_report_reference(
                    source,
                    test_receipt[evidence_key],
                )
                evidence_destination = (
                    packet / "tamper" / "evidence" / evidence_source.name
                )
                if evidence_destination.exists():
                    if sha256(evidence_destination) != sha256(evidence_source):
                        raise ValueError("tamper evidence destination conflicts")
                else:
                    copy_file(evidence_source, evidence_destination)
                test_receipt[evidence_key] = reference(packet, evidence_destination)
        write_json(destination, receipt)
        case["receipt"] = reference(packet, destination)
    destination = packet / "artifacts" / "tamper.json"
    write_json(destination, report)
    return destination


def copy_direct(
    packet: Path,
    source: Path,
    label: str,
) -> Path:
    report = read_json(source)
    if report.get("schema") != ARTIFACT_SCHEMAS[label]:
        raise ValueError(f"{label} report schema mismatch")
    destination = packet / "artifacts" / f"{label}.json"
    write_json(destination, report)
    return destination


def git_revision() -> str:
    return subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=REPO,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout.strip()


def git_clean() -> bool:
    return (
        subprocess.run(
            ["git", "status", "--porcelain"],
            cwd=REPO,
            check=True,
            stdout=subprocess.PIPE,
            text=True,
        ).stdout
        == ""
    )


def validate_capture_time(value: str) -> None:
    if not value.endswith("Z"):
        raise ValueError("--captured-at must be UTC and end with Z")
    datetime.fromisoformat(value[:-1] + "+00:00")


def scan_redaction(packet: Path, allowed_nonlocal: set[str]) -> None:
    for path in packet.rglob("*"):
        if not path.is_file() or path.stat().st_size > 16 * 1024 * 1024:
            continue
        relative = path.relative_to(packet).as_posix()
        text = path.read_text(encoding="utf-8", errors="ignore")
        if SENSITIVE.search(text):
            raise ValueError(f"sensitive marker found in packet file: {relative}")
        if LOCAL_PATH.search(text):
            raise ValueError(f"host-local path found in packet file: {relative}")
        if relative not in allowed_nonlocal and NONLOCAL_IPV4.search(text):
            raise ValueError(f"non-loopback IPv4 found in packet file: {relative}")


def write_checksums(packet: Path) -> None:
    files = sorted(
        path
        for path in packet.rglob("*")
        if path.is_file() and path.name != "SHA256SUMS.txt"
    )
    (packet / "SHA256SUMS.txt").write_text(
        "".join(
            f"{sha256(path)}  {path.relative_to(packet).as_posix()}\n"
            for path in files
        ),
        encoding="utf-8",
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--source-revision", required=True)
    parser.add_argument("--captured-at", required=True)
    parser.add_argument("--node-bin", type=Path, required=True)
    parser.add_argument("--batch-builder-bin", type=Path, required=True)
    parser.add_argument("--rollback-node-bin", type=Path, required=True)
    parser.add_argument("--incompatible-node-bin", type=Path, required=True)
    parser.add_argument("--state-distinction", type=Path, required=True)
    parser.add_argument("--replay-report", type=Path, required=True)
    parser.add_argument("--performance-report", type=Path, required=True)
    parser.add_argument("--tamper-report", type=Path, required=True)
    parser.add_argument("--migration-report", type=Path, required=True)
    parser.add_argument(
        "--allow-nonlocal-ip-file",
        action="append",
        default=[],
    )
    args = parser.parse_args()

    raw_packet = args.output_dir.expanduser()
    raw_node_bin = args.node_bin.expanduser()
    raw_batch_builder_bin = args.batch_builder_bin.expanduser()
    raw_rollback_node_bin = args.rollback_node_bin.expanduser()
    raw_incompatible_node_bin = args.incompatible_node_bin.expanduser()
    if (
        raw_packet.is_symlink()
        or raw_node_bin.is_symlink()
        or raw_batch_builder_bin.is_symlink()
        or raw_rollback_node_bin.is_symlink()
        or raw_incompatible_node_bin.is_symlink()
    ):
        raise ValueError("packet output and release binary paths must not be symlinks")
    packet = raw_packet.resolve()
    if packet.exists():
        raise ValueError(f"refusing to overwrite packet: {packet}")
    validate_capture_time(args.captured_at)
    if re.fullmatch(r"[0-9a-f]{40}", args.source_revision) is None:
        raise ValueError("candidate source revision is invalid")
    assembly_revision = git_revision()
    if not git_clean():
        raise ValueError("packet assembly requires a clean checkout")
    node_bin = raw_node_bin.resolve()
    batch_builder_bin = raw_batch_builder_bin.resolve()
    rollback_node_bin = raw_rollback_node_bin.resolve()
    incompatible_node_bin = raw_incompatible_node_bin.resolve()
    if not node_bin.is_file() or node_bin.parent.name != "release":
        raise ValueError("--node-bin must identify a regular target/release binary")
    if (
        not batch_builder_bin.is_file()
        or batch_builder_bin.parent.name != "release"
        or batch_builder_bin.name != "postfiat-storage-corpus-batches"
    ):
        raise ValueError(
            "--batch-builder-bin must identify the regular release corpus batch builder"
        )
    if (
        not rollback_node_bin.is_file()
        or rollback_node_bin.parent.name != "release"
        or rollback_node_bin == node_bin
        or sha256(rollback_node_bin) == sha256(node_bin)
    ):
        raise ValueError(
            "--rollback-node-bin must identify a distinct regular target/release binary"
        )
    if (
        not incompatible_node_bin.is_file()
        or incompatible_node_bin.parent.name != "release"
        or incompatible_node_bin in {node_bin, rollback_node_bin}
        or sha256(incompatible_node_bin)
        in {sha256(node_bin), sha256(rollback_node_bin)}
    ):
        raise ValueError(
            "--incompatible-node-bin must identify a third regular target/release binary"
        )
    performance = read_json(args.performance_report.resolve())
    migration = read_json(args.migration_report.resolve())
    binary_digest = sha256(node_bin)
    batch_builder_digest = sha256(batch_builder_bin)
    incompatible_binary_digest = sha256(incompatible_node_bin)
    performance_lanes = performance.get("lanes")
    if (
        performance.get("source_revision") != args.source_revision
        or performance.get("node_binary_sha256") != binary_digest
        or performance.get("batch_builder_binary_sha256")
        != batch_builder_digest
        or not isinstance(performance_lanes, dict)
        or set(performance_lanes) != {
            "selected-indexed",
            "legacy-jsonl",
        }
        or any(
            not isinstance(lane, dict)
            or lane.get("source_revision") != args.source_revision
            or lane.get("node_binary_sha256") != binary_digest
            for lane in performance_lanes.values()
        )
    ):
        raise ValueError("performance report source or binary identity mismatch")
    incompatible = migration.get("incompatible_binary")
    if (
        migration.get("status") != "PASS"
        or migration.get("evidence_eligible") is not True
        or migration.get("source_worktree_clean") is not True
        or migration.get("source_revision") != args.source_revision
        or migration.get("node_binary_sha256") != binary_digest
        or not isinstance(incompatible, dict)
        or incompatible.get("sha256") != incompatible_binary_digest
    ):
        raise ValueError("migration report is not an evidence-eligible binary-bound PASS")

    packet.mkdir(parents=True)
    binary_destination = packet / "bin" / "postfiat-node"
    copy_file(node_bin, binary_destination)
    batch_builder_destination = packet / "bin" / "postfiat-storage-corpus-batches"
    copy_file(batch_builder_bin, batch_builder_destination)
    rollback_binary_destination = packet / "bin" / "postfiat-node-rollback"
    copy_file(rollback_node_bin, rollback_binary_destination)
    incompatible_binary_destination = packet / "bin" / "postfiat-node-incompatible"
    copy_file(incompatible_node_bin, incompatible_binary_destination)
    binaries = [
        {
            "path": binary_destination.relative_to(packet).as_posix(),
            "sha256": sha256(binary_destination),
        },
        {
            "path": batch_builder_destination.relative_to(packet).as_posix(),
            "sha256": sha256(batch_builder_destination),
        },
        {
            "path": rollback_binary_destination.relative_to(packet).as_posix(),
            "sha256": sha256(rollback_binary_destination),
        },
        {
            "path": incompatible_binary_destination.relative_to(packet).as_posix(),
            "sha256": sha256(incompatible_binary_destination),
        },
    ]
    source_report = {
        "schema": ARTIFACT_SCHEMAS["source"],
        "git_revision": args.source_revision,
        "assembly_revision": assembly_revision,
        "spec_sha3_384": hashlib.sha3_384(SPEC.read_bytes()).hexdigest(),
        "binaries": binaries,
        "clean_checkout": True,
        "build_profile": "release",
    }
    source_path = packet / "artifacts" / "source.json"
    write_json(source_path, source_report)

    artifact_paths = {
        "source": source_path,
        "replay": copy_replay(packet, args.replay_report.resolve()),
        "performance": copy_performance(packet, args.performance_report.resolve()),
        "tamper": copy_tamper(packet, args.tamper_report.resolve()),
        "migration": copy_direct(
            packet,
            args.migration_report.resolve(),
            "migration",
        ),
    }
    allowed_nonlocal = set(args.allow_nonlocal_ip_file)
    redaction_path = packet / "artifacts" / "redaction.json"
    write_json(
        redaction_path,
        {
            "schema": ARTIFACT_SCHEMAS["redaction"],
            "passed": True,
            "allowed_nonlocal_ip_files": sorted(allowed_nonlocal),
        },
    )
    artifact_paths["redaction"] = redaction_path

    state_distinction = read_json(args.state_distinction.resolve())
    manifest = {
        "schema": "postfiat-storage-scaling-evidence-packet-v1",
        "status": "PASS",
        "captured_at": args.captured_at,
        "source": {
            "git_revision": args.source_revision,
            "assembly_revision": assembly_revision,
            "spec_sha3_384": source_report["spec_sha3_384"],
            "binaries": binaries,
        },
        "state_distinction": state_distinction,
        "artifacts": {
            label: reference(packet, path)
            for label, path in artifact_paths.items()
        },
    }
    write_json(packet / "storage-scaling-packet.json", manifest)
    scan_redaction(packet, allowed_nonlocal)
    write_checksums(packet)

    sys.path.insert(0, str(PYTHON_ROOT))
    from postfiat_rpc.storage_scaling import verify_packet

    verified = verify_packet(packet)
    print(json.dumps(verified.report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
