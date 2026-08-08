#!/usr/bin/env python3
"""Generate secret-safe C0 manifests for nested _worktree_holding children.

The scanner reads candidate content locally but emits only sanitized locations,
line numbers, and finding classes. It never emits a matched value or matching
line. Git stderr is also never copied into a manifest or terminal output.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
from typing import Any


HOLDING = Path("/home/postfiat/repos/_worktree_holding")
CURRENT_CANONICAL_REPO = Path(
    "/home/postfiat/repos/pftl-main-integration-20260807"
)
CURRENT_CANONICAL = "52e51bc290eb8d6416e78d31bab6315de5729af6"
CURRENT_CANONICAL_LABEL = "postfiatorg/postfiatl1v2 main"
SAME_HISTORY_REF = "origin/open-source-productionization-20260716"
ARCHIVE_REMOTE_LABEL = "postfiatorg/postfiatl1v2-private-archive-20260717"
MAX_CONTENT_SCAN_BYTES = 5 * 1024 * 1024

PR_METADATA = {
    "pftl1v2-pr1-cobalt-sig-verify": {
        "number": 18,
        "merged_at": "2026-07-26T17:28:08Z",
    },
    "pftl1v2-pr10a-dead-script-refs": {
        "number": 15,
        "merged_at": "2026-07-26T17:23:52Z",
    },
    "pftl1v2-pr10b-docs-content-gaps": {
        "number": 17,
        "merged_at": "2026-07-26T17:23:57Z",
    },
    "pftl1v2-pr2-debug-pool-gate": {
        "number": 16,
        "merged_at": "2026-07-26T17:28:22Z",
    },
    "pftl1v2-pr3-rpc-transport-auth": {
        "number": 22,
        "merged_at": "2026-07-26T17:28:08Z",
    },
    "pftl1v2-pr4-dos-hardening": {
        "number": 20,
        "merged_at": "2026-07-26T17:25:19Z",
    },
    "pftl1v2-pr5-storage-integrity": {
        "number": 21,
        "merged_at": "2026-07-26T17:28:02Z",
    },
    "pftl1v2-pr6-orchard-vk-panics": {
        "number": 19,
        "merged_at": "2026-07-26T17:29:58Z",
    },
    "pftl1v2-pr9-ambient-backlog": {
        "number": 14,
        "merged_at": "2026-07-26T17:19:29Z",
    },
}

VALUE_PATTERNS: tuple[tuple[str, re.Pattern[bytes]], ...] = (
    (
        "private-key-block",
        re.compile(
            rb"-----BEGIN (?:RSA |EC |DSA |OPENSSH |PGP )?PRIVATE KEY-----",
            re.IGNORECASE,
        ),
    ),
    (
        "cloud-access-key-identifier",
        re.compile(rb"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b"),
    ),
    (
        "bearer-token-material",
        re.compile(rb"\bBearer\s+[A-Za-z0-9._~+/=-]{12,}", re.IGNORECASE),
    ),
    (
        "credential-bearing-uri",
        re.compile(rb"[a-z][a-z0-9+.-]*://[^\s/:@]{2,}:[^\s/@]{2,}@", re.IGNORECASE),
    ),
    (
        "private-key-reference-or-assignment",
        re.compile(
            rb"(?i)\b(?:private[_-]?key|secret[_-]?key)\b\s*(?::|=)",
        ),
    ),
    (
        "api-token-reference-or-assignment",
        re.compile(
            rb"(?i)\b(?:api[_-]?key|api[_-]?token|access[_-]?token|auth[_-]?token|jupyter[_-]?token)\b\s*(?::|=)",
        ),
    ),
    (
        "password-passphrase-reference-or-assignment",
        re.compile(rb"(?i)\b(?:password|passphrase)\b\s*(?::|=)"),
    ),
    (
        "mnemonic-seed-reference-or-assignment",
        re.compile(rb"(?i)\b(?:mnemonic|seed[_-]?phrase)\b\s*(?::|=)"),
    ),
)

PATH_CLASS_PATTERNS: tuple[tuple[str, re.Pattern[str]], ...] = (
    (
        "credential-adjacent-filename",
        re.compile(
            r"(?i)(?:^|[._-])(?:credential|secret|token|passphrase|password|mnemonic|private[_-]?key)(?:[._-]|$)"
        ),
    ),
    (
        "environment-file",
        re.compile(r"(?i)(?:^|/)\.env(?:\.|$)"),
    ),
)

TOKENLIKE_NAME = re.compile(
    r"(?i)(?:sk-[A-Za-z0-9_-]{16,}|gh[pousr]_[A-Za-z0-9]{16,}|"
    r"(?:AKIA|ASIA)[A-Z0-9]{16}|[A-Fa-f0-9]{64,})"
)


class GitFailure(RuntimeError):
    pass


def git(
    worktree: Path,
    args: list[str],
    *,
    alternate_objects: Path | None = None,
    allow_failure: bool = False,
) -> str | None:
    env = os.environ.copy()
    if alternate_objects is not None:
        env["GIT_ALTERNATE_OBJECT_DIRECTORIES"] = str(alternate_objects)
    result = subprocess.run(
        ["git", "-C", str(worktree), *args],
        capture_output=True,
        text=True,
        check=False,
        env=env,
    )
    if result.returncode != 0:
        if allow_failure:
            return None
        raise GitFailure(f"git command failed with exit class {result.returncode}")
    return result.stdout.rstrip("\n")


def safe_location(relative: str) -> str:
    parts = []
    for part in Path(relative).parts:
        if TOKENLIKE_NAME.search(part):
            digest = hashlib.sha256(part.encode()).hexdigest()[:12]
            parts.append(f"<redacted-tokenlike-name-sha256-{digest}>")
        else:
            parts.append(part)
    return Path(*parts).as_posix()


def safe_subject(subject: str) -> tuple[str | None, str | None]:
    if TOKENLIKE_NAME.search(subject):
        return None, "secret-adjacent-commit-subject"
    return subject, None


def disk_bytes(root: Path) -> int:
    total = 0
    for path in root.rglob("*"):
        try:
            if path.is_symlink():
                total += path.lstat().st_size
            elif path.is_file():
                total += path.stat().st_size
        except OSError:
            continue
    return total


def human_bytes(value: int) -> str:
    number = float(value)
    for unit in ("B", "KiB", "MiB", "GiB", "TiB"):
        if number < 1024 or unit == "TiB":
            return f"{number:.1f}{unit}" if unit != "B" else f"{int(number)}B"
        number /= 1024
    raise AssertionError("unreachable")


def parse_status(worktree: Path) -> tuple[list[dict[str, str]], list[str]]:
    output = git(
        worktree,
        ["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )
    assert output is not None
    dirty: list[dict[str, str]] = []
    untracked: list[str] = []
    for record in output.split("\0"):
        if not record:
            continue
        status = record[:2]
        raw_path = record[3:]
        if status == "??":
            untracked.append(raw_path)
        else:
            dirty.append({"status": status, "location": safe_location(raw_path)})
    return dirty, untracked


def candidate_git_files(worktree: Path) -> list[str]:
    output = git(
        worktree,
        ["ls-files", "-z", "--cached", "--others", "--exclude-standard"],
    )
    assert output is not None
    return [item for item in output.split("\0") if item]


def candidate_plain_files(root: Path) -> list[str]:
    files = []
    for path in root.rglob("*"):
        if path.is_file() and not path.is_symlink():
            files.append(path.relative_to(root).as_posix())
    return sorted(files)


def scan_secret_adjacent(
    root: Path, relative_files: list[str]
) -> tuple[list[dict[str, Any]], dict[str, int]]:
    findings: set[tuple[str, int | None, str]] = set()
    coverage = {
        "candidate_files": len(relative_files),
        "content_scanned": 0,
        "large_files_location_only": 0,
        "unreadable_files_location_only": 0,
    }
    for relative in relative_files:
        safe = safe_location(relative)
        for finding_class, pattern in PATH_CLASS_PATTERNS:
            if pattern.search(relative):
                findings.add((safe, None, finding_class))
        path = root / relative
        try:
            size = path.stat().st_size
        except OSError:
            findings.add((safe, None, "unreadable-file"))
            coverage["unreadable_files_location_only"] += 1
            continue
        if size > MAX_CONTENT_SCAN_BYTES:
            findings.add((safe, None, "large-file-not-content-scanned"))
            coverage["large_files_location_only"] += 1
            continue
        try:
            data = path.read_bytes()
        except OSError:
            findings.add((safe, None, "unreadable-file"))
            coverage["unreadable_files_location_only"] += 1
            continue
        coverage["content_scanned"] += 1
        for finding_class, pattern in VALUE_PATTERNS:
            for match in pattern.finditer(data):
                line = data.count(b"\n", 0, match.start()) + 1
                findings.add((safe, line, finding_class))
    rendered = [
        {
            "location": location,
            **({"line": line} if line is not None else {}),
            "class": finding_class,
        }
        for location, line, finding_class in sorted(
            findings, key=lambda item: (item[0], item[1] or 0, item[2])
        )
    ]
    return rendered, coverage


def classify_untracked(
    untracked: list[str], secret_findings: list[dict[str, Any]]
) -> dict[str, int]:
    secret_locations = {entry["location"] for entry in secret_findings}
    counts = {
        "evidence": 0,
        "scratch_or_generated": 0,
        "secret_adjacent": 0,
        "source_or_other": 0,
    }
    for relative in untracked:
        safe = safe_location(relative)
        lowered = relative.lower()
        if safe in secret_locations:
            counts["secret_adjacent"] += 1
        elif any(
            marker in lowered
            for marker in (
                "target/",
                "cache/",
                ".cache/",
                "tmp/",
                ".tmp",
                "__pycache__/",
            )
        ):
            counts["scratch_or_generated"] += 1
        elif any(
            marker in lowered
            for marker in (
                "evidence",
                "artifact",
                "report",
                "transcript",
                "fixture",
                "score",
            )
        ) or Path(lowered).suffix in {".json", ".jsonl", ".md", ".log", ".txt", ".csv"}:
            counts["evidence"] += 1
        else:
            counts["source_or_other"] += 1
    return counts


def parse_cherry(output: str | None) -> dict[str, Any]:
    if output is None:
        return {"status": "no-common-ancestor", "commits": []}
    commits = []
    for line in output.splitlines():
        sign, commit, subject = line.split(" ", 2)
        safe_text, finding_class = safe_subject(subject)
        entry: dict[str, Any] = {"sign": sign, "commit": commit}
        if safe_text is not None:
            entry["subject"] = safe_text
        if finding_class is not None:
            entry["subject_class"] = finding_class
        commits.append(entry)
    return {
        "status": "ok",
        "minus_patch_equivalent": sum(item["sign"] == "-" for item in commits),
        "plus_unique": sum(item["sign"] == "+" for item in commits),
        "commits": commits,
    }


def ahead_behind(
    worktree: Path,
    left: str,
    right: str,
    *,
    alternate_objects: Path | None = None,
) -> tuple[int, int]:
    output = git(
        worktree,
        ["rev-list", "--left-right", "--count", f"{left}...{right}"],
        alternate_objects=alternate_objects,
    )
    assert output is not None
    behind, ahead = output.split()
    return int(behind), int(ahead)


def git_manifest(worktree: Path) -> dict[str, Any]:
    name = worktree.name
    dirty, untracked = parse_status(worktree)
    candidates = candidate_git_files(worktree)
    findings, coverage = scan_secret_adjacent(worktree, candidates)
    upstream = git(
        worktree, ["rev-parse", "--abbrev-ref", "@{upstream}"], allow_failure=True
    )
    branch = git(worktree, ["branch", "--show-current"]) or ""
    head = git(worktree, ["rev-parse", "HEAD"])
    archive_main_behind, archive_main_ahead = ahead_behind(
        worktree, "origin/main", "HEAD"
    )
    same_behind, same_ahead = ahead_behind(worktree, SAME_HISTORY_REF, "HEAD")
    alternate = CURRENT_CANONICAL_REPO / ".git" / "objects"
    try:
        current_behind, current_ahead = ahead_behind(
            worktree,
            CURRENT_CANONICAL,
            "HEAD",
            alternate_objects=alternate,
        )
    except GitFailure:
        current_behind, current_ahead = None, None
    common = git(
        worktree,
        ["merge-base", CURRENT_CANONICAL, "HEAD"],
        alternate_objects=alternate,
        allow_failure=True,
    )
    current_cherry = git(
        worktree,
        ["cherry", "-v", CURRENT_CANONICAL, "HEAD"],
        alternate_objects=alternate,
        allow_failure=True,
    )
    same_cherry = git(
        worktree,
        ["cherry", "-v", SAME_HISTORY_REF, "HEAD"],
    )
    metadata = PR_METADATA[name]
    size = disk_bytes(worktree)
    return {
        "schema": "postfiat.track-c.worktree-manifest.v2",
        "worktree": str(worktree),
        "branch": branch,
        "head": head,
        "upstream": upstream or "",
        "remote": ARCHIVE_REMOTE_LABEL,
        "dirty_tracked": len(dirty),
        "untracked": len(untracked),
        "disk": human_bytes(size),
        "ahead_origin_main": str(archive_main_ahead),
        "ahead_origin_master": "",
        "ahead_behind": {
            "current_canonical": {
                "label": CURRENT_CANONICAL_LABEL,
                "commit": CURRENT_CANONICAL,
                "common_ancestor": common,
                "raw_behind": current_behind,
                "raw_ahead": current_ahead,
                "interpretation": (
                    "unrelated rewritten histories; raw counts are recorded but "
                    "are not integration evidence"
                    if common is None
                    else "shared history"
                ),
            },
            "same_history_integration_ref": {
                "ref": SAME_HISTORY_REF,
                "behind": same_behind,
                "ahead": same_ahead,
            },
            "archive_origin_main": {
                "ref": "origin/main",
                "behind": archive_main_behind,
                "ahead": archive_main_ahead,
            },
        },
        "git_cherry_v": {
            "current_canonical": parse_cherry(current_cherry),
            "same_history_integration_ref": {
                "ref": SAME_HISTORY_REF,
                **parse_cherry(same_cherry),
            },
        },
        "dirty_tracked_files": dirty,
        "untracked_classification_counts": classify_untracked(
            untracked, findings
        ),
        "secret_adjacent_methodology": (
            "The scanner reads local content internally and emits only sanitized "
            "locations, line numbers, and finding classes. It never emits a "
            "matched value or matching line, and captured Git stderr is never "
            "copied to output; this inventory pass therefore cannot itself "
            "surface a secret value or trigger a secret-output STOP."
        ),
        "secret_adjacent_findings": findings,
        "secret_scan_coverage": coverage,
        "disk_footprint_bytes": size,
        "pull_request": {
            "repository": ARCHIVE_REMOTE_LABEL,
            "number": metadata["number"],
            "state": "MERGED",
            "merged_at": metadata["merged_at"],
            "url": (
                "https://github.com/postfiatorg/"
                "postfiatl1v2-private-archive-20260717/pull/"
                f"{metadata['number']}"
            ),
        },
        "proposed_disposition": {
            "class": "archive",
            "terminal_state": "retire-clean-after-canonical-equivalence",
            "reason": (
                "Worktree is clean, branch is fully pushed, and its archive PR "
                "is merged, but today's canonical PFTL main has unrelated "
                "rewritten history. Preserve the upstream branch, merged PR, "
                "and this manifest until semantic equivalence against canonical "
                "main is demonstrated; no dirty or untracked cargo exists."
            ),
        },
    }


def plain_manifest(root: Path) -> dict[str, Any]:
    relative_files = candidate_plain_files(root)
    findings, coverage = scan_secret_adjacent(root, relative_files)
    size = disk_bytes(root)
    counts = classify_untracked(relative_files, findings)
    return {
        "schema": "postfiat.track-c.worktree-manifest.v2",
        "worktree": str(root),
        "branch": "",
        "head": "",
        "upstream": "",
        "remote": "",
        "dirty_tracked": 0,
        "untracked": len(relative_files),
        "disk": human_bytes(size),
        "ahead_origin_main": "",
        "ahead_origin_master": "",
        "ahead_behind": {
            "current_canonical": {
                "status": "not-git",
                "label": CURRENT_CANONICAL_LABEL,
                "commit": CURRENT_CANONICAL,
            }
        },
        "git_cherry_v": {
            "current_canonical": {
                "status": "not-git",
                "commits": [],
            }
        },
        "dirty_tracked_files": [],
        "untracked_classification_counts": counts,
        "secret_adjacent_methodology": (
            "The scanner reads local content internally and emits only sanitized "
            "locations, line numbers, and finding classes. It never emits a "
            "matched value or matching line; this inventory pass therefore "
            "cannot itself surface a secret value or trigger a secret-output STOP."
        ),
        "secret_adjacent_findings": findings,
        "secret_scan_coverage": coverage,
        "disk_footprint_bytes": size,
        "proposed_disposition": {
            "class": "archive",
            "reason": (
                "Non-Git scoring evidence has no upstream preservation. Keep a "
                "location/class-only redaction manifest plus content hashes in "
                "the retirement archive before removing the local directory."
            ),
        },
    }


def write_manifest(output_dir: Path, name: str, manifest: dict[str, Any]) -> None:
    output_dir.mkdir(parents=True, exist_ok=True)
    path = output_dir / f"nested-{name}.json"
    temporary = path.with_suffix(".json.tmp")
    temporary.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    os.replace(temporary, path)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--output-dir",
        action="append",
        type=Path,
        required=True,
        help="May be repeated to persist the exact manifest set in two locations.",
    )
    args = parser.parse_args()
    children = sorted(path for path in HOLDING.iterdir() if path.is_dir())
    expected = set(PR_METADATA) | {"postfiatl1v2-20260529-score-artifacts"}
    actual = {path.name for path in children}
    if actual != expected:
        raise SystemExit(
            "nested child-name set changed; refusing incomplete or overbroad inventory"
        )
    manifests: dict[str, dict[str, Any]] = {}
    for child in children:
        if child.name in PR_METADATA:
            manifests[child.name] = git_manifest(child)
        else:
            manifests[child.name] = plain_manifest(child)
    for output_dir in args.output_dir:
        for name, manifest in manifests.items():
            write_manifest(output_dir, name, manifest)
    summary = {
        "manifests_written_per_output": len(manifests),
        "output_directories": [str(path) for path in args.output_dir],
        "git_worktrees": sum(name in PR_METADATA for name in manifests),
        "non_git_children": sum(name not in PR_METADATA for name in manifests),
        "secret_output_policy": "locations-and-classes-only",
    }
    print(json.dumps(summary, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
