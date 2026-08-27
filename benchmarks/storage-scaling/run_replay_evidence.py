#!/usr/bin/env python3
"""Produce exact height-915 and height-924 storage replay receipts offline.

The supplied data directories are treated as immutable sources. Each replay is
performed on a disposable copy, rebuilt into the selected transactional store,
and independently verified against the authenticated legacy source. No network
or fleet endpoint is contacted.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any

REPO = Path(__file__).resolve().parents[2]
REQUIRED_SOURCES = {
    915: "quarantine_archive",
    924: "authenticated_history",
}
CONTROLLED_CHAIN_ID = "postfiat-wan-devnet-2"
CONTROLLED_GENESIS_HASH = (
    "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad255"
    "21aff3ed334da07e150a7233a3e90a9"
)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def validate_regular_tree(root: Path, label: str) -> None:
    if not root.is_dir() or root.is_symlink():
        raise ValueError(f"{label} is not a regular directory")
    for path in root.rglob("*"):
        if path.is_symlink() or not (path.is_dir() or path.is_file()):
            raise ValueError(f"{label} contains a symlink or special entry: {path}")


def tree_sha256(root: Path) -> str:
    digest = hashlib.sha256()
    for path in sorted(root.rglob("*"), key=lambda item: item.relative_to(root).as_posix()):
        relative = path.relative_to(root).as_posix().encode("utf-8")
        if path.is_dir():
            digest.update(b"D")
            digest.update(len(relative).to_bytes(8, "big"))
            digest.update(relative)
            continue
        digest.update(b"F")
        digest.update(len(relative).to_bytes(8, "big"))
        digest.update(relative)
        digest.update(path.stat().st_size.to_bytes(8, "big"))
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
    return digest.hexdigest()


def migration_packet_root(manifest: dict[str, Any]) -> str:
    canonical = dict(manifest)
    canonical["migration_packet_root"] = ""
    encoded = json.dumps(
        canonical,
        ensure_ascii=False,
        separators=(",", ":"),
    ).encode("utf-8")
    digest = hashlib.sha3_384()
    digest.update(b"postfiat.storage_migration.packet.v1")
    digest.update(b"\x00")
    digest.update(encoded)
    return digest.hexdigest()


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def read_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_bytes())
    if not isinstance(value, dict):
        raise ValueError(f"expected a JSON object: {path.name}")
    return value


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


def run_json(command: list[str]) -> dict[str, Any]:
    completed = subprocess.run(
        command,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        value = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(
            f"{Path(command[0]).name} {command[1]} did not emit JSON"
        ) from error
    if not isinstance(value, dict):
        raise RuntimeError(f"{Path(command[0]).name} {command[1]} emitted non-object JSON")
    return value


def run_checked(command: list[str]) -> None:
    subprocess.run(
        command,
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
    )


def replay_source(
    *,
    node_bin: Path,
    source: Path,
    height: int,
    source_kind: str,
    scratch: Path,
    receipt_path: Path,
    revision: str,
    source_tree_digest: str,
) -> dict[str, str]:
    clone = scratch / f"source-{height}"
    rebuild = scratch / f"transactional-{height}"
    shutil.copytree(source, clone, copy_function=shutil.copy2)

    status = run_json(
        [
            str(node_bin),
            "status",
            "--data-dir",
            str(clone),
            "--expect-height",
            str(height),
        ]
    )
    if status.get("block_height") != height:
        raise RuntimeError(f"{source_kind} source is not at exact height {height}")
    if status.get("chain_id") != CONTROLLED_CHAIN_ID:
        raise RuntimeError(f"{source_kind} source is not the controlled chain")
    if status.get("genesis_hash") != CONTROLLED_GENESIS_HASH:
        raise RuntimeError(f"{source_kind} source is not the controlled genesis")
    storage = status.get("storage")
    if not isinstance(storage, dict):
        raise RuntimeError(f"{source_kind} source omitted storage status")
    if storage.get("commitment_version") != "postfiat.replicated_state.v1":
        raise RuntimeError(f"{source_kind} source is not below storage activation")

    for command in (
        "verify-state",
        "verify-blocks",
        "verify-governance",
        "verify-bridge",
        "verify-mempool",
        "verify-shielded",
    ):
        run_checked([str(node_bin), command, "--data-dir", str(clone)])

    tip_hash = str(status.get("block_tip_hash", ""))
    state_root = str(status.get("state_root", ""))
    if len(tip_hash) != 96 or len(state_root) != 96:
        raise RuntimeError(f"{source_kind} source identities are malformed")

    rebuilt = run_json(
        [
            str(node_bin),
            "storage-rebuild-transactional",
            "--data-dir",
            str(clone),
            "--output-dir",
            str(rebuild),
            "--expected-tip",
            tip_hash,
            "--expected-state-root",
            state_root,
            "--offline-confirmed",
        ]
    )
    verified = run_json(
        [
            str(node_bin),
            "storage-rebuild-transactional",
            "--data-dir",
            str(clone),
            "--output-dir",
            str(rebuild),
            "--expected-tip",
            tip_hash,
            "--expected-state-root",
            state_root,
            "--verify-only",
            "--offline-confirmed",
        ]
    )
    if (
        rebuilt.get("schema") != "postfiat-storage-migration-report-v1"
        or rebuilt.get("published") is not True
        or verified.get("schema") != "postfiat-storage-migration-report-v1"
        or verified.get("verify_only") is not True
        or verified.get("published") is not False
        or rebuilt.get("migration_packet_root")
        != verified.get("migration_packet_root")
    ):
        raise RuntimeError(f"{source_kind} transactional rebuild did not verify")

    manifest_path = rebuild / "storage-migration-manifest.json"
    checksum_path = rebuild / "storage-migration-manifest.sha3-384"
    manifest = read_json(manifest_path)
    if (
        manifest.get("schema") != "postfiat-storage-migration-manifest-v1"
        or manifest.get("chain_id") != CONTROLLED_CHAIN_ID
        or manifest.get("genesis_hash") != CONTROLLED_GENESIS_HASH
        or manifest.get("block_count") != height
        or manifest.get("source_tip", {}).get("height") != height
        or manifest.get("source_tip", {}).get("block_hash") != tip_hash
        or manifest.get("source_tip", {}).get("state_root") != state_root
    ):
        raise RuntimeError(f"{source_kind} migration manifest is not exact")
    packet_root = migration_packet_root(manifest)
    checksum = checksum_path.read_text(encoding="utf-8")
    expected_checksum = (
        f"{packet_root}  storage-migration-manifest.json\n"
    )
    if (
        packet_root != manifest.get("migration_packet_root")
        or packet_root != rebuilt.get("migration_packet_root")
        or packet_root != verified.get("migration_packet_root")
        or checksum != expected_checksum
    ):
        raise RuntimeError(f"{source_kind} migration manifest packet root failed")

    ordered_history = str(manifest.get("ordered_history_accumulator", ""))
    if len(ordered_history) != 96:
        raise RuntimeError(f"{source_kind} ordered-history accumulator is malformed")
    logical_report = manifest.get("logical_store_report")
    if not isinstance(logical_report, dict):
        raise RuntimeError(f"{source_kind} logical integrity report is missing")

    receipt: dict[str, Any] = {
        "schema": "postfiat-storage-replay-receipt-v1",
        "source_height": height,
        "source_kind": source_kind,
        "block_count": height,
        "commitment_mode": "legacy_below_storage_activation",
        "exact_replay": True,
        "full_replay_passed": True,
        "logical_rebuild_identical": True,
        "canonical_export_identical": True,
        "tip_hash": tip_hash,
        "state_root": state_root,
        "ordered_history_accumulator": ordered_history,
        "chain_id": status.get("chain_id"),
        "genesis_hash": status.get("genesis_hash"),
        "protocol_version": status.get("protocol_version"),
        "receipt_count": manifest.get("receipt_count"),
        "archive_count": manifest.get("archive_count"),
        "ordered_batch_count": manifest.get("ordered_batch_count"),
        "blocks_root": manifest.get("blocks_root"),
        "receipts_root": manifest.get("receipts_root"),
        "archive_root": manifest.get("archive_root"),
        "ordered_batches_root": manifest.get("ordered_batches_root"),
        "current_state_root": manifest.get("current_state_root"),
        "migration_packet_root": packet_root,
        "migration_manifest_sha256": sha256(manifest_path),
        "migration_manifest_sha3_384": hashlib.sha3_384(
            manifest_path.read_bytes()
        ).hexdigest(),
        "source_tree_sha256": source_tree_digest,
        "logical_store_report": logical_report,
        "source_revision": revision,
        "node_binary_sha256": sha256(node_bin),
        "offline": True,
        "network_contacted": False,
    }
    write_json(receipt_path, receipt)
    return {
        "path": receipt_path.relative_to(receipt_path.parent.parent).as_posix(),
        "sha256": sha256(receipt_path),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--node-bin", type=Path, required=True)
    parser.add_argument("--quarantine-dir", type=Path, required=True)
    parser.add_argument("--authenticated-history-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--expected-source-revision", required=True)
    args = parser.parse_args()

    raw_node_bin = args.node_bin.expanduser()
    raw_output = args.output_dir.expanduser()
    raw_sources = {
        915: args.quarantine_dir.expanduser(),
        924: args.authenticated_history_dir.expanduser(),
    }
    if raw_node_bin.is_symlink() or raw_output.is_symlink():
        raise ValueError("binary and output paths must not be symlinks")
    for height, source in raw_sources.items():
        if source.is_symlink():
            raise ValueError(f"height-{height} source path must not be a symlink")

    node_bin = raw_node_bin.resolve()
    output = raw_output.resolve()
    sources = {height: path.resolve() for height, path in raw_sources.items()}
    if not node_bin.is_file() or node_bin.is_symlink() or node_bin.parent.name != "release":
        raise ValueError("--node-bin must identify a regular target/release binary")
    if output.exists():
        raise ValueError(f"refusing to overwrite output directory: {output}")
    revision = git_revision()
    if revision != args.expected_source_revision:
        raise ValueError("HEAD does not match --expected-source-revision")
    if not git_clean():
        raise ValueError("replay evidence requires a clean checkout")

    source_digests: dict[int, str] = {}
    for height, source in sources.items():
        validate_regular_tree(source, f"height-{height} source")
        if (
            output == source
            or output.is_relative_to(source)
            or source.is_relative_to(output)
        ):
            raise ValueError(
                f"height-{height} source and output directory must be disjoint"
            )
        source_digests[height] = tree_sha256(source)

    output.mkdir(parents=True)
    receipts_dir = output / "receipts"
    receipts_dir.mkdir()

    references: list[dict[str, str]] = []
    receipts: dict[int, dict[str, Any]] = {}
    with tempfile.TemporaryDirectory(prefix="postfiat-storage-replay-") as temporary:
        scratch = Path(temporary)
        for height, source_kind in REQUIRED_SOURCES.items():
            receipt_path = receipts_dir / f"height-{height}.json"
            references.append(
                replay_source(
                    node_bin=node_bin,
                    source=sources[height],
                    height=height,
                    source_kind=source_kind,
                    scratch=scratch,
                    receipt_path=receipt_path,
                    revision=revision,
                    source_tree_digest=source_digests[height],
                )
            )
            receipts[height] = read_json(receipt_path)
            if tree_sha256(sources[height]) != source_digests[height]:
                raise RuntimeError(
                    f"height-{height} immutable source changed during replay"
                )

    authenticated = receipts[924]
    report = {
        "schema": "postfiat-storage-scaling-replay-v1",
        "quarantine_archive_blocks": 915,
        "authenticated_history_height": 924,
        "exact_pre_activation_replay": True,
        "full_replay_passed": True,
        "logical_rebuild_identical": True,
        "canonical_export_identical": True,
        "tip_hash": authenticated["tip_hash"],
        "state_root": authenticated["state_root"],
        "ordered_history_accumulator": authenticated[
            "ordered_history_accumulator"
        ],
        "receipts": references,
        "source_revision": revision,
        "node_binary_sha256": sha256(node_bin),
        "offline": True,
        "network_contacted": False,
    }
    report_path = output / "replay-report.json"
    write_json(report_path, report)
    print(f"storage-scaling-replay=PASS")
    print(f"report={report_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
