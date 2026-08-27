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
    "performance": "postfiat-storage-scaling-six-validator-campaign-v1",
    "tamper": "postfiat-storage-scaling-tamper-matrix-v1",
    "migration": "postfiat-storage-scaling-six-clone-migration-v1",
    "redaction": "postfiat-storage-scaling-redaction-v1",
}
SENSITIVE = re.compile(
    r"private[-_ ]?key|secret|password|mnemonic|spending[-_ ]?key|"
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
    rows = report.get("rows")
    if not isinstance(rows, list):
        raise ValueError("performance report omitted rows")
    for row in rows:
        if not isinstance(row, dict) or not isinstance(row.get("windows"), list):
            raise ValueError("performance row is malformed")
        for window in row["windows"]:
            if not isinstance(window, dict):
                raise ValueError("performance window is malformed")
            raw = window.get("normalized_report")
            expected = window.get("normalized_report_sha256")
            if not isinstance(raw, str) or not isinstance(expected, str):
                raise ValueError("performance window omitted its normalized report")
            raw_source = (source.parent / raw).resolve()
            if sha256(raw_source) != expected:
                raise ValueError("performance normalized report digest mismatch")
            label = str(window.get("label", raw_source.stem))
            destination = packet / "performance" / "windows" / f"{label}.json"
            copy_file(raw_source, destination)
            window["normalized_report"] = destination.relative_to(packet).as_posix()
            window["normalized_report_sha256"] = sha256(destination)
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
    parser.add_argument("--rollback-node-bin", type=Path, required=True)
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

    packet = args.output_dir.resolve()
    if packet.exists():
        raise ValueError(f"refusing to overwrite packet: {packet}")
    validate_capture_time(args.captured_at)
    if args.source_revision != git_revision() or len(args.source_revision) != 40:
        raise ValueError("source revision does not match HEAD")
    if not git_clean():
        raise ValueError("packet assembly requires a clean checkout")
    node_bin = args.node_bin.resolve()
    rollback_node_bin = args.rollback_node_bin.resolve()
    if not node_bin.is_file() or node_bin.parent.name != "release":
        raise ValueError("--node-bin must identify a target/release binary")
    if (
        not rollback_node_bin.is_file()
        or rollback_node_bin.is_symlink()
        or rollback_node_bin.parent.name != "release"
        or rollback_node_bin == node_bin
        or sha256(rollback_node_bin) == sha256(node_bin)
    ):
        raise ValueError(
            "--rollback-node-bin must identify a distinct regular target/release binary"
        )

    performance = read_json(args.performance_report.resolve())
    binary_digest = sha256(node_bin)
    if (
        performance.get("source_revision") != args.source_revision
        or performance.get("node_binary_sha256") != binary_digest
    ):
        raise ValueError("performance report source or binary identity mismatch")

    packet.mkdir(parents=True)
    binary_destination = packet / "bin" / "postfiat-node"
    copy_file(node_bin, binary_destination)
    rollback_binary_destination = packet / "bin" / "postfiat-node-rollback"
    copy_file(rollback_node_bin, rollback_binary_destination)
    binaries = [
        {
            "path": binary_destination.relative_to(packet).as_posix(),
            "sha256": sha256(binary_destination),
        },
        {
            "path": rollback_binary_destination.relative_to(packet).as_posix(),
            "sha256": sha256(rollback_binary_destination),
        },
    ]
    source_report = {
        "schema": ARTIFACT_SCHEMAS["source"],
        "git_revision": args.source_revision,
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
