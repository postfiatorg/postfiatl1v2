#!/usr/bin/env python3
"""Create deterministic, redaction-gated archives for semantic-close worktrees."""

from __future__ import annotations

import gzip
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
from typing import Any


ARCHIVE_ROOT = Path(
    "/home/postfiat/repos/_archives/a666-nested-semantic-close-20260808"
)
PRIVATE_REMOTE = "postfiatorg/postfiatl1v2-private-archive-20260717"
SOURCES = (
    {
        "name": "pftl1v2-pr2-debug-pool-gate",
        "branch": "fix/20260725-2-debug-pool-gate",
        "head": "f722a31e3d60727b0410f91e54a81bf3b105052d",
        "disposition": "semantic-close-canonical-debug-gates",
    },
    {
        "name": "pftl1v2-pr3-rpc-transport-auth",
        "branch": "fix/20260725-3-rpc-transport-auth",
        "head": "1a9f426c78a21b708698ba87302e00e759e7299b",
        "disposition": "semantic-close-canonical-transport-auth",
    },
    {
        "name": "pftl1v2-pr6-orchard-vk-panics",
        "branch": "fix/20260725-6-orchard-vk-panics",
        "head": "a00e7dadaf1a9cb30009ab226b982b19b52c28b7",
        "disposition": "semantic-close-canonical-orchard-vk-panic-policy",
    },
    {
        "name": "pftl1v2-pr9-ambient-backlog",
        "branch": "fix/20260725-9-ambient-backlog",
        "head": "a417c65ccb78900e9ab9fdd9923345b356bded0f",
        "disposition": "canonical-supersedes-ambient-ledger-statuses",
    },
)


def run(
    command: list[str], *, cwd: Path, text: bool = True
) -> subprocess.CompletedProcess[Any]:
    return subprocess.run(
        command,
        cwd=cwd,
        capture_output=True,
        text=text,
        check=False,
    )


def git_text(worktree: Path, *args: str) -> str:
    result = run(["git", *args], cwd=worktree)
    if result.returncode:
        raise RuntimeError(f"git operation failed with exit class {result.returncode}")
    return result.stdout.strip()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def atomic_json(path: Path, value: Any) -> None:
    if path.exists():
        raise RuntimeError(f"refusing to overwrite existing archive metadata: {path.name}")
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    os.replace(temporary, path)


def tree_manifest(worktree: Path) -> list[dict[str, Any]]:
    result = run(
        ["git", "ls-tree", "-r", "-l", "-z", "HEAD"],
        cwd=worktree,
        text=False,
    )
    if result.returncode:
        raise RuntimeError(
            f"tree manifest failed with exit class {result.returncode}"
        )
    entries: list[dict[str, Any]] = []
    for record in result.stdout.split(b"\0"):
        if not record:
            continue
        metadata, raw_path = record.split(b"\t", 1)
        mode, object_type, object_id, raw_size = metadata.split(b" ", 3)
        if object_type != b"blob":
            continue
        entries.append(
            {
                "location": raw_path.decode("utf-8", errors="surrogateescape"),
                "mode": mode.decode("ascii"),
                "blob_sha1": object_id.decode("ascii"),
                "bytes": int(raw_size),
            }
        )
    return entries


def remote_head(worktree: Path, branch: str) -> str:
    result = run(
        ["git", "ls-remote", "--heads", "origin", f"refs/heads/{branch}"],
        cwd=worktree,
    )
    if result.returncode:
        raise RuntimeError(
            f"remote-head verification failed with exit class {result.returncode}"
        )
    fields = result.stdout.strip().split()
    if len(fields) != 2:
        raise RuntimeError("remote branch is absent or ambiguous")
    return fields[0]


def create_archive(source: dict[str, str]) -> dict[str, Any]:
    name = source["name"]
    worktree = Path("/home/postfiat/repos/_worktree_holding") / name
    expected_head = source["head"]
    if git_text(worktree, "status", "--porcelain=v1", "--untracked-files=all"):
        raise RuntimeError(f"source worktree is not clean: {name}")
    if git_text(worktree, "branch", "--show-current") != source["branch"]:
        raise RuntimeError(f"source branch mismatch: {name}")
    if git_text(worktree, "rev-parse", "HEAD") != expected_head:
        raise RuntimeError(f"source head mismatch: {name}")
    if git_text(worktree, "rev-parse", "@{upstream}") != expected_head:
        raise RuntimeError(f"source upstream mismatch: {name}")
    if remote_head(worktree, source["branch"]) != expected_head:
        raise RuntimeError(f"live remote head mismatch: {name}")

    scanner = worktree / "scripts/public-secret-scan"
    scan = run([str(scanner)], cwd=worktree)
    if scan.returncode or "public secret scan passed mode=tracked-tree" not in scan.stdout:
        raise RuntimeError(
            f"redaction scan failed with finding metadata withheld: {name}"
        )

    entries = tree_manifest(worktree)
    entries_bytes = json.dumps(
        entries, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")
    archive = ARCHIVE_ROOT / f"{name}-{expected_head[:12]}.tar.gz"
    manifest_path = ARCHIVE_ROOT / f"{name}-{expected_head[:12]}.manifest.json"
    if archive.exists() or manifest_path.exists():
        raise RuntimeError(f"refusing to overwrite existing archive: {name}")
    raw_tar = archive.with_suffix(".tar.tmp")
    compressed = archive.with_suffix(archive.suffix + ".tmp")
    with raw_tar.open("wb") as output:
        result = subprocess.run(
            [
                "git",
                "archive",
                "--format=tar",
                f"--prefix={name}/",
                "HEAD",
            ],
            cwd=worktree,
            stdout=output,
            stderr=subprocess.PIPE,
            check=False,
        )
    if result.returncode:
        raw_tar.unlink(missing_ok=True)
        raise RuntimeError(
            f"git archive failed with exit class {result.returncode}: {name}"
        )
    with raw_tar.open("rb") as source_tar, compressed.open("wb") as raw_output:
        with gzip.GzipFile(
            filename="",
            mode="wb",
            fileobj=raw_output,
            compresslevel=9,
            mtime=0,
        ) as gzip_output:
            shutil.copyfileobj(source_tar, gzip_output, length=1024 * 1024)
    archive_data = {
        "file": archive.name,
        "bytes": compressed.stat().st_size,
        "sha256": sha256_file(compressed),
        "uncompressed_tar_bytes": raw_tar.stat().st_size,
        "uncompressed_tar_sha256": sha256_file(raw_tar),
    }
    os.replace(compressed, archive)
    raw_tar.unlink()
    manifest = {
        "schema": "postfiat.track-c.semantic-close-archive.v1",
        "access_class": "restricted-engineering-evidence",
        "source": {
            "repository": PRIVATE_REMOTE,
            "worktree": str(worktree),
            "branch": source["branch"],
            "head": expected_head,
            "upstream_head": expected_head,
            "live_remote_head": expected_head,
            "clean": True,
        },
        "disposition": source["disposition"],
        "redaction_scan": {
            "status": "passed",
            "mode": "tracked-tree",
            "finding_count": 0,
            "scanner_location": "scripts/public-secret-scan",
            "scanner_sha256": sha256_file(scanner),
            "output_policy": "rule-location-class-only-on-failure-never-values",
        },
        "tree": {
            "entry_count": len(entries),
            "content_bytes": sum(item["bytes"] for item in entries),
            "entry_manifest_sha256": hashlib.sha256(entries_bytes).hexdigest(),
            "entries": entries,
        },
        "archive": archive_data,
        "retirement_authorized": False,
        "deletion_performed": False,
    }
    atomic_json(manifest_path, manifest)
    return {
        "name": name,
        "branch": source["branch"],
        "head": expected_head,
        "archive": archive_data,
        "manifest": manifest_path.name,
        "manifest_sha256": sha256_file(manifest_path),
        "redaction_scan": "passed",
        "remote_head_verified": True,
        "tree_entry_count": len(entries),
    }


def main() -> int:
    if ARCHIVE_ROOT.exists() and any(ARCHIVE_ROOT.iterdir()):
        raise SystemExit("archive root exists and is non-empty; refusing overwrite")
    ARCHIVE_ROOT.mkdir(parents=True, exist_ok=True)
    results = [create_archive(source) for source in SOURCES]
    index = {
        "schema": "postfiat.track-c.semantic-close-archive-index.v1",
        "access_class": "restricted-engineering-evidence",
        "archive_root": str(ARCHIVE_ROOT),
        "archives": results,
        "archive_count": len(results),
        "all_redaction_scans_passed": all(
            item["redaction_scan"] == "passed" for item in results
        ),
        "all_remote_heads_verified": all(
            item["remote_head_verified"] for item in results
        ),
        "retirement_authorized": False,
        "deletion_performed": False,
    }
    index_path = ARCHIVE_ROOT / "INDEX.json"
    atomic_json(index_path, index)
    print(
        json.dumps(
            {
                "archive_count": len(results),
                "all_redaction_scans_passed": True,
                "all_remote_heads_verified": True,
                "index_sha256": sha256_file(index_path),
                "total_archive_bytes": sum(
                    item["archive"]["bytes"] for item in results
                ),
                "output_policy": "names-counts-hashes-status-only",
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
