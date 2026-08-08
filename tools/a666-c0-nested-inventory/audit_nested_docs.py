#!/usr/bin/env python3
"""Secret-safe read-only equivalence audit for nested documentation branches."""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
import survey_nested as inventory  # noqa: E402


CURRENT_REPO = inventory.CURRENT_CANONICAL_REPO
CURRENT_COMMIT = inventory.CURRENT_CANONICAL
PR9 = inventory.HOLDING / "pftl1v2-pr9-ambient-backlog"
LEDGERS = (
    "docs/status/ambient-finding-disposition-ledger/p0-001-100.json",
    "docs/status/ambient-finding-disposition-ledger/p0-101-198.json",
)
SCRIPT_REF_RE = re.compile(rb"scripts/[A-Za-z0-9_./-]+")
RPC_ARM_PREFIX_RE = re.compile(rb"^\s{8}\"[A-Za-z0-9_.:-]+\"")
RPC_WIRE_NAME_RE = re.compile(rb"\"([A-Za-z][A-Za-z0-9_.:-]+)\"")


def run_bytes(command: list[str], *, allow_failure: bool = False) -> bytes | None:
    result = subprocess.run(command, capture_output=True, check=False)
    if result.returncode != 0:
        if allow_failure:
            return None
        raise RuntimeError(f"captured command failed with exit class {result.returncode}")
    return result.stdout


def git_bytes(repo: Path, args: list[str], *, allow_failure: bool = False) -> bytes | None:
    return run_bytes(
        ["git", "-C", str(repo), *args],
        allow_failure=allow_failure,
    )


def blob(repo: Path, ref: str, location: str) -> bytes | None:
    return git_bytes(repo, ["show", f"{ref}:{location}"], allow_failure=True)


def tree_paths(prefix: str | None = None) -> set[str]:
    args = ["ls-tree", "-r", "--name-only", "-z", CURRENT_COMMIT]
    if prefix is not None:
        args.extend(["--", prefix])
    output = git_bytes(CURRENT_REPO, args)
    assert output is not None
    return {
        item
        for item in output.decode(
            "utf-8", errors="surrogateescape"
        ).split("\0")
        if item
    }


def ledger_audit() -> dict[str, Any]:
    results = []
    all_old_ids: set[str] = set()
    all_current_ids: set[str] = set()
    missing_ids: set[str] = set()
    changed_status_ids: set[str] = set()
    for location in LEDGERS:
        old_data = blob(PR9, "HEAD", location)
        current_data = blob(CURRENT_REPO, CURRENT_COMMIT, location)
        if old_data is None or current_data is None:
            results.append(
                {
                    "location": inventory.safe_location(location),
                    "old_present": old_data is not None,
                    "current_present": current_data is not None,
                }
            )
            continue
        old_json = json.loads(old_data)
        current_json = json.loads(current_data)
        old_findings = {
            str(item["id"]): str(item.get("status", ""))
            for item in old_json.get("findings", [])
        }
        current_findings = {
            str(item["id"]): str(item.get("status", ""))
            for item in current_json.get("findings", [])
        }
        old_ids = set(old_findings)
        current_ids = set(current_findings)
        local_missing = old_ids - current_ids
        local_changed = {
            finding_id
            for finding_id in old_ids & current_ids
            if old_findings[finding_id] != current_findings[finding_id]
        }
        transitions = Counter(
            f"{old_findings[finding_id]}->{current_findings[finding_id]}"
            for finding_id in local_changed
        )
        all_old_ids.update(old_ids)
        all_current_ids.update(current_ids)
        missing_ids.update(local_missing)
        changed_status_ids.update(local_changed)
        results.append(
            {
                "location": inventory.safe_location(location),
                "old_finding_count": len(old_findings),
                "current_finding_count": len(current_findings),
                "old_status_counts": dict(sorted(Counter(old_findings.values()).items())),
                "current_status_counts": dict(
                    sorted(Counter(current_findings.values()).items())
                ),
                "old_ids_missing_current": sorted(local_missing),
                "status_changed_ids": sorted(local_changed),
                "status_transition_counts": dict(sorted(transitions.items())),
            }
        )
    return {
        "methodology": (
            "Only ledger locations, finding ids, and status-class counts are "
            "emitted. Rationales and all other field values are never emitted."
        ),
        "ledgers": results,
        "old_id_count": len(all_old_ids),
        "current_id_count": len(all_current_ids),
        "old_ids_missing_current": sorted(missing_ids),
        "status_changed_ids": sorted(changed_status_ids),
        "semantic_objective_present": not missing_ids,
        "manual_status_review_required": bool(changed_status_ids),
    }


def dead_script_reference_audit(paths: set[str]) -> dict[str, Any]:
    output = git_bytes(
        CURRENT_REPO,
        [
            "grep",
            "-n",
            "-o",
            "-E",
            r"scripts/[A-Za-z0-9_./-]+",
            CURRENT_COMMIT,
            "--",
            "*.md",
            "*.txt",
        ],
        allow_failure=True,
    )
    findings: set[tuple[str, str]] = set()
    if output is not None:
        prefix = f"{CURRENT_COMMIT}:"
        for raw_line in output.decode(
            "utf-8", errors="surrogateescape"
        ).splitlines():
            line = raw_line[len(prefix) :] if raw_line.startswith(prefix) else raw_line
            pieces = line.split(":", 2)
            if len(pieces) != 3:
                continue
            location, _, reference = pieces
            reference = reference.rstrip("./")
            if reference not in paths:
                findings.add(
                    (
                        inventory.safe_location(location),
                        inventory.safe_location(reference),
                    )
                )
    active = [
        {"location": location, "missing_script": reference}
        for location, reference in sorted(findings)
        if not location.startswith("docs/archive/")
    ]
    archived = [
        {"location": location, "missing_script": reference}
        for location, reference in sorted(findings)
        if location.startswith("docs/archive/")
    ]
    return {
        "methodology": (
            "Git grep is captured internally. Output contains only sanitized "
            "documentation locations and referenced script paths whose target "
            "is absent from the canonical tree; no matching prose line is emitted."
        ),
        "unique_missing_location_reference_pairs": len(findings),
        "active_docs_missing_pairs": len(active),
        "archived_docs_missing_pairs": len(archived),
        "affected_active_document_count": len(
            {item["location"] for item in active}
        ),
        "unique_missing_script_count": len(
            {item["missing_script"] for item in active}
        ),
        "active_docs": active,
        "archived_docs": archived,
        "semantic_objective_present": not active,
    }


def rpc_coverage_audit(paths: set[str]) -> dict[str, Any]:
    rpc_source = blob(
        CURRENT_REPO,
        CURRENT_COMMIT,
        "crates/node/src/rpc_dispatch.rs",
    )
    assert rpc_source is not None
    methods = sorted(
        {
            match.decode("ascii")
            for line in rpc_source.splitlines()
            if RPC_ARM_PREFIX_RE.search(line) and b"=>" in line
            for match in RPC_WIRE_NAME_RE.findall(line.split(b"=>", 1)[0])
        }
    )
    rpc_docs_paths = sorted(
        path
        for path in paths
        if path.startswith("docs/rpc/")
        and Path(path).suffix.lower() in {".md", ".txt"}
    )
    docs_blob = bytearray()
    for location in rpc_docs_paths:
        data = blob(CURRENT_REPO, CURRENT_COMMIT, location)
        if data is not None and len(data) <= inventory.MAX_CONTENT_SCAN_BYTES:
            docs_blob.extend(data)
            docs_blob.extend(b"\n")
    method_results = []
    for wire_name in methods:
        encoded = wire_name.encode()
        documented = encoded in docs_blob
        method_results.append(
            {
                "wire_method": wire_name,
                "documented_under_docs_rpc": documented,
            }
        )
    missing = [
        item["wire_method"]
        for item in method_results
        if not item["documented_under_docs_rpc"]
    ]
    coverage_path = "docs/rpc/method-coverage.md"
    return {
        "methodology": (
            "RPC constant identifiers and protocol method names are extracted "
            "internally from canonical source; docs/rpc content is searched "
            "internally. Output contains identifiers, protocol method names, "
            "booleans, counts, and sanitized paths only; no source or prose line "
            "is emitted."
        ),
        "archived_coverage_path": coverage_path,
        "archived_coverage_path_present": coverage_path in paths,
        "rpc_docs_paths": [inventory.safe_location(path) for path in rpc_docs_paths],
        "dispatch_wire_method_count": len(methods),
        "documented_method_count": sum(
            item["documented_under_docs_rpc"] for item in method_results
        ),
        "undocumented_method_count": len(missing),
        "undocumented_wire_methods": missing,
        "methods": method_results,
        "semantic_objective_present": coverage_path in paths or not missing,
    }


def atomic_write(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    os.replace(temporary, path)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    paths = tree_paths()
    output = {
        "schema": "postfiat.track-c.nested-docs-equivalence.v1",
        "canonical_commit": CURRENT_COMMIT,
        "methodology": (
            "All Git blobs and grep results are captured internally. Output "
            "contains sanitized locations, finding ids, status classes, script "
            "paths, RPC identifiers/method names, counts, hashes, and booleans "
            "only. No source/prose line, rationale, matched secret value, or Git "
            "stderr is emitted."
        ),
        "pr9_ambient_ledgers": ledger_audit(),
        "pr10a_dead_script_references": dead_script_reference_audit(paths),
        "pr10b_rpc_coverage": rpc_coverage_audit(paths),
    }
    atomic_write(args.output, output)
    print(
        json.dumps(
            {
                "pr9_objective_present": output["pr9_ambient_ledgers"][
                    "semantic_objective_present"
                ],
                "pr10a_objective_present": output[
                    "pr10a_dead_script_references"
                ]["semantic_objective_present"],
                "pr10b_objective_present": output["pr10b_rpc_coverage"][
                    "semantic_objective_present"
                ],
                "output_policy": "locations-ids-classes-paths-counts-only",
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
