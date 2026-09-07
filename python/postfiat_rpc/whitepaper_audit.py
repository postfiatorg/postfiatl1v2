"""Read and verify the pinned whitepaper claim inventory; no network or signing."""

from __future__ import annotations

import argparse
from collections import Counter
from functools import lru_cache
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys

REPOSITORY = Path(__file__).resolve().parents[2]
INVENTORY = "docs/architecture/whitepaper-claims.json"
MATRIX = "docs/architecture/whitepaper-alignment.md"
STATUSES = {
    "implemented-and-tested", "implemented-with-insufficient-verification",
    "partial", "planned", "divergent", "unresolved",
}
PLANES = {"source", "observed-deployment", "research-target", "unresolved"}
ROLES = {"code", "test", "documentation", "receipt"}
SCHEMA = "postfiat.whitepaper-audit.v1"


class AuditError(ValueError):
    """An inventory cannot be verified against its declared baseline."""


def _text(value: object, label: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise AuditError(f"{label}: expected nonempty text")
    return value


def _keys(value: object, expected: set[str], label: str) -> dict:
    if not isinstance(value, dict) or set(value) != expected:
        raise AuditError(f"{label}: expected fields {sorted(expected)}")
    return value


def _path(value: object) -> str:
    path = _text(value, "path")
    parsed = PurePosixPath(path)
    if parsed.is_absolute() or ".." in parsed.parts or "\\" in path or str(parsed) != path:
        raise AuditError(f"invalid repository-relative path: {path}")
    return path


@lru_cache(maxsize=512)
def git_bytes(root: Path, commit: str, path: str) -> bytes:
    result = subprocess.run(
        ["git", "-C", str(root), "show", f"{commit}:{_path(path)}"],
        capture_output=True, check=False,
    )
    if result.returncode:
        raise AuditError(f"baseline file unavailable: {commit}:{path}")
    return result.stdout


def sections(paper: str) -> dict[str, tuple[str, int]]:
    found = {}
    fenced = False
    for number, line in enumerate(paper.splitlines(), 1):
        if line.startswith("```"):
            fenced = not fenced
        if fenced:
            continue
        match = re.match(r"^#{2,3} (\d+(?:\.\d+)?)[.]?\s+(.+)$", line)
        if match:
            found[match[1]] = (line.lstrip("# "), number)
        elif line in {"## Abstract", "## Appendix A: Evidence Register", "## References"}:
            key = {"## Abstract": "Abstract", "## References": "References"}.get(line, "Appendix A")
            found[key] = (line.lstrip("# "), number)
    return found


def validate(document: object, root: Path = REPOSITORY) -> dict:
    data = _keys(document, {"schema", "baseline", "claims"}, "inventory")
    if data["schema"] != SCHEMA:
        raise AuditError("unknown inventory schema")
    baseline = _keys(data["baseline"], {"commit", "whitepaper", "sha256", "version"}, "baseline")
    commit = baseline["commit"]
    if not isinstance(commit, str) or not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise AuditError("baseline.commit: expected full lowercase commit hash")
    _text(baseline["version"], "baseline.version")
    original = git_bytes(root, commit, _path(baseline["whitepaper"]))
    if hashlib.sha256(original).hexdigest() != baseline["sha256"]:
        raise AuditError("original whitepaper SHA-256 mismatch")
    paper = original.decode("utf-8")
    headings = sections(paper)
    rows = data["claims"]
    if not isinstance(rows, list) or not rows:
        raise AuditError("claims: expected nonempty list")
    covered: set[str] = set()
    ids: set[str] = set()
    for row in rows:
        row = _keys(row, {"id", "sections", "locator", "claim", "status", "plane", "finding", "evidence", "followup"}, "claim")
        identifier = _text(row["id"], "claim.id")
        if not re.fullmatch(r"WP-\d{2}", identifier) or identifier in ids:
            raise AuditError(f"invalid or duplicate claim id: {identifier}")
        ids.add(identifier)
        refs = row["sections"]
        if not isinstance(refs, list) or not refs or any(not isinstance(s, str) for s in refs):
            raise AuditError(f"{identifier}: invalid section list")
        if len(set(refs)) != len(refs) or any(s not in headings for s in refs):
            raise AuditError(f"{identifier}: duplicate or unknown section")
        covered.update(refs)
        locator = _text(row["locator"], "locator")
        if paper.count(locator) != 1:
            raise AuditError(f"{identifier}: whitepaper locator must match exactly once")
        line = paper[:paper.index(locator)].count("\n") + 1
        containing = max((key for key, (_, start) in headings.items() if start <= line), key=lambda key: headings[key][1])
        if containing not in refs:
            raise AuditError(f"{identifier}: locator is outside declared sections")
        for key in ("claim", "finding", "followup"):
            _text(row[key], f"{identifier}.{key}")
        if not isinstance(row["status"], str) or not isinstance(row["plane"], str) or row["status"] not in STATUSES or row["plane"] not in PLANES:
            raise AuditError(f"{identifier}: unknown status or evidence plane")
        evidence = row["evidence"]
        if not isinstance(evidence, list) or not evidence:
            raise AuditError(f"{identifier}: evidence references required")
        for reference in evidence:
            reference = _keys(reference, {"path", "anchor", "role"}, f"{identifier}.evidence")
            source = git_bytes(root, commit, _path(reference["path"])).decode("utf-8")
            if _text(reference["anchor"], "anchor") not in source:
                raise AuditError(f"{identifier}: anchor missing in {reference['path']}: {reference['anchor']}")
            if not isinstance(reference["role"], str) or reference["role"] not in ROLES:
                raise AuditError(f"{identifier}: unknown evidence role")
        if row["status"] == "implemented-and-tested" and not any(e["role"] == "test" for e in evidence):
            raise AuditError(f"{identifier}: implemented-and-tested requires a test reference")
    missing = set(headings) - covered
    if missing:
        raise AuditError(f"uncovered whitepaper sections: {sorted(missing)}")
    for number in range(1, 9):
        if not any(row["locator"].startswith(f"**[E{number}]") for row in rows):
            raise AuditError(f"uncovered evidence-register entry: E{number}")
    return data


def load_inventory(path: Path, root: Path = REPOSITORY) -> dict:
    try:
        return validate(json.loads(path.read_text(encoding="utf-8")), root)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise AuditError(f"cannot read inventory: {error}") from error


def _cell(value: str) -> str:
    return value.replace("|", "&#124;").replace("\n", " ")


def render_markdown(data: dict, root: Path = REPOSITORY) -> str:
    baseline = data["baseline"]
    commit = baseline["commit"]
    paper = git_bytes(root, commit, baseline["whitepaper"]).decode("utf-8")
    base = f"https://github.com/postfiatorg/postfiatl1v2/blob/{commit}/"
    output = [
        "# Whitepaper-to-code alignment", "",
        "This inventory records findings against the **original audit baseline**, including discrepancies corrected by the accompanying documentation patch. It is not a list of current unfixed defects. Read the [overview and section guide](whitepaper-overview.md), [gap backlog](whitepaper-gaps.md), and [validation record](whitepaper-validation.md) for resolutions and tests actually run.", "",
        f"Baseline: `{commit}`. Original whitepaper SHA-256: `{baseline['sha256']}`.", "",
        "**Status:** implemented-and-tested means relevant retained tests exist for the stated narrower behavior; it does not mean all cited tests were rerun. Implemented-with-insufficient-verification means source exists but the broader verification is incomplete. Partial, planned, divergent and unresolved are deliberate limits, not passing scores. Evidence planes separate source, observed deployment, research targets and unresolved evidence.", "",
        "Generated from [whitepaper-claims.json](whitepaper-claims.json) by `PYTHONPATH=python python3 -m postfiat_rpc.whitepaper_audit --markdown`. `--check` checks references, coverage and rendering, not protocol truth.", "",
    ]
    last_group = None
    for row in data["claims"]:
        group = row["sections"][0].split(".")[0]
        if group != last_group:
            output += ["", f"## {group}", "", "| Claim and original location | Finding | Evidence at the pinned commit | Follow-up |", "| --- | --- | --- | --- |"]
            last_group = group
        line = paper[:paper.index(row["locator"])].count("\n") + 1
        location = f"[{_cell(', '.join(row['sections']))}]({base}{baseline['whitepaper']}#L{line})"
        references = []
        for reference in row["evidence"]:
            source = git_bytes(root, commit, reference["path"]).decode("utf-8")
            number = source[:source.index(reference["anchor"])].count("\n") + 1
            label = f"{reference['role']}: {PurePosixPath(reference['path']).name}:{number}"
            references.append(f"[{_cell(label)}]({base}{reference['path']}#L{number})")
        output.append(f"| **{row['id']}** · {location}<br>{_cell(row['claim'])} | **{row['status']}** · {row['plane']}<br>{_cell(row['finding'])} | {'<br>'.join(references)} | {_cell(row['followup'])} |")
    return "\n".join(output) + "\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inventory", type=Path, default=REPOSITORY / INVENTORY)
    parser.add_argument("--section", help="Exact section (for example 6.4); 6 includes its subsections")
    formats = parser.add_mutually_exclusive_group()
    formats.add_argument("--json", action="store_true", help="Validated inventory or filtered claims")
    formats.add_argument("--markdown", action="store_true", help="Render the complete MkDocs table")
    formats.add_argument("--check", action="store_true", help="Verify pins, anchors, coverage and checked-in table")
    args = parser.parse_args(argv)
    try:
        if args.section and (args.markdown or args.check):
            raise AuditError("--section cannot be combined with --markdown or --check")
        data = load_inventory(args.inventory)
        rows = data["claims"]
        if args.section:
            rows = [row for row in rows if any(s == args.section or s.startswith(args.section + ".") for s in row["sections"])]
            if not rows:
                raise AuditError(f"unknown section: {args.section}")
        if args.check:
            if (REPOSITORY / MATRIX).read_text(encoding="utf-8") != render_markdown(data):
                raise AuditError("rendered alignment table is stale; regenerate with --markdown")
            current = (REPOSITORY / data["baseline"]["whitepaper"]).read_bytes()
            if current != (REPOSITORY / "docs/assets/raw/whitepaper.md.txt").read_bytes():
                raise AuditError("downloadable whitepaper differs from canonical source")
            print(f"PASS: {len(rows)} claims; all original sections and E1–E8 covered; pinned references, table and download verified. This is an inventory check, not a protocol proof.")
        elif args.markdown:
            print(render_markdown(data), end="")
        elif args.json:
            print(json.dumps({**data, "claims": rows}, indent=2, ensure_ascii=False))
        else:
            print(f"PostFiat whitepaper audit · {data['baseline']['commit']}")
            print("Evidence at a pinned revision; retained tests are not all rerun. See whitepaper-validation.md.")
            print(" | ".join(f"{status}: {count}" for status, count in sorted(Counter(r["status"] for r in rows).items())))
            for row in rows:
                print(f"\n{row['id']} §{', '.join(row['sections'])} [{row['status']}; {row['plane']}] {row['claim']}\n  {row['finding']}\n  Follow-up: {row['followup']}")
                for reference in row["evidence"]:
                    print(f"  {reference['role']}: {reference['path']} :: {reference['anchor']}")
        return 0
    except (AuditError, OSError, UnicodeError) as error:
        print(f"whitepaper audit: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
