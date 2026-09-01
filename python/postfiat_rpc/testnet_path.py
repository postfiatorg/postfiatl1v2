"""Testnet-path status CLI.

Reads the committed repository documents that define the public-testnet path
and prints their current gate states:

- Gate Zero Z1/Z2/Z3 and phase A-E checkbox items from
  ``docs/plans/active/l1v2-public-testnet-path-milestone.md``;
- the G0-G7 gate-table rows from
  ``docs/plans/active/storage-scaling-milestone.md``;
- the milestone registry listing in ``docs/plans/README.md``; and
- the head of ``docs/status/chain-state-current.md``.

The tool is deterministic and offline: it parses the committed markdown only.
Any state it cannot parse is reported as UNKNOWN together with the file and
line it looked at. It never invents state.

Run it from the repository root:

    PYTHONPATH=python python3 -m postfiat_rpc.testnet_path
    PYTHONPATH=python python3 -m postfiat_rpc.testnet_path --json
    PYTHONPATH=python python3 -m postfiat_rpc.testnet_path --markdown
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

MILESTONE_PATH = "docs/plans/active/l1v2-public-testnet-path-milestone.md"
STORAGE_MILESTONE_PATH = "docs/plans/active/storage-scaling-milestone.md"
PLANS_README_PATH = "docs/plans/README.md"
CHAIN_STATE_PATH = "docs/status/chain-state-current.md"
INPUT_PATHS = (
    MILESTONE_PATH,
    STORAGE_MILESTONE_PATH,
    PLANS_README_PATH,
    CHAIN_STATE_PATH,
)

GATE_ZERO_LABELS = ("Z1", "Z2", "Z3")
PHASE_LETTERS = ("A", "B", "C", "D", "E")
STORAGE_GATES = tuple(f"G{index}" for index in range(8))

DONE = "DONE"
OPEN = "OPEN"
UNKNOWN = "UNKNOWN"

CHECKBOX_RE = re.compile(r"^- \[(?P<mark>[^\]])\] (?P<label>[A-Z][0-9]+)\s+(?P<text>.*)$")
PHASE_HEADING_RE = re.compile(r"^## (?P<phase>[A-E]) — (?P<title>.+)$")
GATE_ROW_RE = re.compile(
    r"^\|\s*(?P<gate>G[0-7])\s+—\s+(?P<name>[^|]+?)\s*\|\s*(?P<state>[^|]+?)\s*\|"
)
LINK_RE = re.compile(r"\[([^\]]*)\]\([^)]*\)")


@dataclass
class Item:
    """One checkbox item from the testnet-path milestone."""

    label: str
    state: str
    summary: str
    file: str
    line: int


@dataclass
class Phase:
    phase: str
    title: str
    file: str
    line: int
    items: list[Item] = field(default_factory=list)


@dataclass
class GateRow:
    gate: str
    name: str
    state: str
    file: str
    line: int


def clean_markdown(text: str) -> str:
    """Strip links, emphasis, and backticks; collapse whitespace."""
    text = LINK_RE.sub(r"\1", text)
    text = text.replace("**", "").replace("`", "")
    text = re.sub(r"(?<![\w])\*|\*(?![\w])", "", text)
    return re.sub(r"\s+", " ", text).strip()


def summarize(text: str, limit: int = 110) -> str:
    cleaned = clean_markdown(text)
    if len(cleaned) <= limit:
        return cleaned
    cut = cleaned.rfind(" ", 0, limit)
    if cut <= 0:
        cut = limit
    return cleaned[:cut].rstrip(" ,;:") + "…"


def _checkbox_state(mark: str) -> str:
    if mark in ("x", "X"):
        return DONE
    if mark == " ":
        return OPEN
    return UNKNOWN


def parse_milestone(text: str, path: str) -> dict[str, Any]:
    """Parse Gate Zero and phase A-E checkbox items from the milestone."""
    gate_zero_line = 0
    gate_zero_items: list[Item] = []
    phases: dict[str, Phase] = {}
    section: str | None = None  # "Z" for Gate Zero, else a phase letter
    current_item: Item | None = None
    current_text = ""
    for line_number, line in enumerate(text.splitlines(), start=1):
        if line.startswith("## "):
            current_item = None
            phase_match = PHASE_HEADING_RE.match(line)
            if line.startswith("## Gate Zero"):
                section = "Z"
                gate_zero_line = line_number
            elif phase_match:
                section = phase_match.group("phase")
                phases[section] = Phase(
                    phase=section,
                    title=clean_markdown(phase_match.group("title")),
                    file=path,
                    line=line_number,
                )
            else:
                section = None
            continue
        if section is None:
            continue
        checkbox = CHECKBOX_RE.match(line)
        if checkbox:
            item = Item(
                label=checkbox.group("label"),
                state=_checkbox_state(checkbox.group("mark")),
                summary=summarize(checkbox.group("text")),
                file=path,
                line=line_number,
            )
            if item.state == UNKNOWN:
                item.summary = (
                    f"unparseable checkbox marker [{checkbox.group('mark')}]"
                )
                current_item = None
            else:
                current_item = item
                current_text = checkbox.group("text")
            target = gate_zero_items if section == "Z" else phases[section].items
            target.append(item)
            continue
        stripped = line.strip()
        if current_item is not None and stripped and line[:1].isspace():
            # Indented continuation of the previous checkbox item.
            current_text += " " + stripped
            current_item.summary = summarize(current_text)
        else:
            current_item = None

    ordered_gate_zero: list[Item] = []
    by_label = {item.label: item for item in gate_zero_items}
    for label in GATE_ZERO_LABELS:
        if label in by_label:
            ordered_gate_zero.append(by_label[label])
        else:
            ordered_gate_zero.append(
                Item(
                    label=label,
                    state=UNKNOWN,
                    summary="not found in the Gate Zero section",
                    file=path,
                    line=gate_zero_line if gate_zero_line else 1,
                )
            )

    ordered_phases: list[Phase] = []
    for letter in PHASE_LETTERS:
        if letter in phases:
            phase = phases[letter]
            if not phase.items:
                phase.items.append(
                    Item(
                        label=f"{letter}?",
                        state=UNKNOWN,
                        summary="no checkbox items found in this phase section",
                        file=path,
                        line=phase.line,
                    )
                )
            ordered_phases.append(phase)
        else:
            ordered_phases.append(
                Phase(
                    phase=letter,
                    title="section not found",
                    file=path,
                    line=1,
                    items=[
                        Item(
                            label=f"{letter}?",
                            state=UNKNOWN,
                            summary=f"no '## {letter} —' section found",
                            file=path,
                            line=1,
                        )
                    ],
                )
            )
    return {"gate_zero": ordered_gate_zero, "phases": ordered_phases}


def parse_gate_table(text: str, path: str) -> list[GateRow]:
    """Parse the G0-G7 rows of the storage milestone's gate-status table."""
    lines = text.splitlines()
    header_line = 0
    rows: dict[str, GateRow] = {}
    in_table = False
    for line_number, line in enumerate(lines, start=1):
        stripped = line.strip()
        if stripped.startswith("| Gate |") and "Current state" in stripped:
            header_line = line_number
            in_table = True
            continue
        if not in_table:
            continue
        if not stripped.startswith("|"):
            in_table = False
            continue
        row = GATE_ROW_RE.match(stripped)
        if row and row.group("gate") not in rows:
            rows[row.group("gate")] = GateRow(
                gate=row.group("gate"),
                name=clean_markdown(row.group("name")),
                state=clean_markdown(row.group("state")),
                file=path,
                line=line_number,
            )

    ordered: list[GateRow] = []
    for gate in STORAGE_GATES:
        if gate in rows:
            ordered.append(rows[gate])
        else:
            ordered.append(
                GateRow(
                    gate=gate,
                    name="",
                    state=UNKNOWN,
                    file=path,
                    line=header_line if header_line else 1,
                )
            )
    return ordered


def parse_plans_readme(text: str, path: str) -> dict[str, Any]:
    """Locate the testnet-path milestone in the plans registry."""
    section = None
    for line_number, line in enumerate(text.splitlines(), start=1):
        if line.startswith("## "):
            section = line[3:].strip()
            continue
        if "l1v2-public-testnet-path-milestone.md" in line:
            state = (
                f"LISTED {section.upper()}" if section in ("Active", "Completed") else UNKNOWN
            )
            return {"state": state, "file": path, "line": line_number}
    return {"state": UNKNOWN, "file": path, "line": 1}


def parse_chain_state_head(text: str, path: str) -> dict[str, Any]:
    """Extract the update stamp and lead banner from chain-state-current.md."""
    updated = UNKNOWN
    updated_line = 1
    banner = UNKNOWN
    banner_line = 1
    for line_number, line in enumerate(text.splitlines(), start=1):
        if updated == UNKNOWN:
            match = re.match(r"^Updated: `([^`]+)`", line)
            if match:
                updated = match.group(1)
                updated_line = line_number
        if banner == UNKNOWN:
            match = re.match(r'^!!! \w+ "([^"]+)"', line)
            if match:
                banner = match.group(1)
                banner_line = line_number
        if updated != UNKNOWN and banner != UNKNOWN:
            break
    return {
        "updated": updated,
        "updated_line": updated_line,
        "banner": banner,
        "banner_line": banner_line,
        "file": path,
    }


def newest_input_commit(repo_root: Path) -> dict[str, str] | None:
    """Newest commit touching any input file, from local git history only."""
    try:
        output = subprocess.run(
            ["git", "-C", str(repo_root), "log", "-1", "--format=%h %H %cs", "--"]
            + list(INPUT_PATHS),
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return None
    parts = output.split()
    if len(parts) != 3:
        return None
    return {"short": parts[0], "hash": parts[1], "date": parts[2]}


def _read(repo_root: Path, relative: str) -> str | None:
    path = repo_root / relative
    try:
        return path.read_text(encoding="utf-8")
    except OSError:
        return None


def collect_status(repo_root: Path) -> dict[str, Any]:
    milestone_text = _read(repo_root, MILESTONE_PATH)
    if milestone_text is None:
        milestone = parse_milestone("", MILESTONE_PATH)
    else:
        milestone = parse_milestone(milestone_text, MILESTONE_PATH)

    storage_text = _read(repo_root, STORAGE_MILESTONE_PATH)
    storage_gates = parse_gate_table(storage_text or "", STORAGE_MILESTONE_PATH)

    readme_text = _read(repo_root, PLANS_README_PATH)
    registry = parse_plans_readme(readme_text or "", PLANS_README_PATH)

    chain_text = _read(repo_root, CHAIN_STATE_PATH)
    chain_state = parse_chain_state_head(chain_text or "", CHAIN_STATE_PATH)

    return {
        "generated_by": "python -m postfiat_rpc.testnet_path",
        "sources": list(INPUT_PATHS),
        "newest_input_commit": newest_input_commit(repo_root),
        "gate_zero": [asdict(item) for item in milestone["gate_zero"]],
        "phases": [asdict(phase) for phase in milestone["phases"]],
        "storage_gates": [asdict(row) for row in storage_gates],
        "plans_registry": registry,
        "chain_state": chain_state,
    }


def _mark(state: str) -> str:
    return {DONE: "[x]", OPEN: "[ ]"}.get(state, "[?]")


def _item_line(item: dict[str, Any]) -> str:
    location = f" ({item['file']}:{item['line']})" if item["state"] == UNKNOWN else ""
    return (
        f"  {_mark(item['state'])} {item['label']:<3} {item['state']:<7} "
        f"{item['summary']}{location}"
    )


def render_plain(status: dict[str, Any]) -> str:
    lines = ["L1v2 public testnet path — status"]
    commit = status["newest_input_commit"]
    if commit:
        lines.append(
            f"Inputs last committed in {commit['short']} ({commit['date']})."
        )
    else:
        lines.append("Inputs last committed in UNKNOWN (git history unavailable).")
    lines.append("")
    lines.append("Gate Zero — operator preconditions (block community-facing work):")
    for item in status["gate_zero"]:
        lines.append(_item_line(item))
    for phase in status["phases"]:
        lines.append("")
        lines.append(f"Phase {phase['phase']} — {phase['title']}:")
        for item in phase["items"]:
            lines.append(_item_line(item))
    lines.append("")
    lines.append(f"Storage milestone gates ({STORAGE_MILESTONE_PATH}):")
    for row in status["storage_gates"]:
        location = f" ({row['file']}:{row['line']})" if row["state"] == UNKNOWN else ""
        name = f" — {row['name']}" if row["name"] else ""
        lines.append(f"  {row['gate']}{name}: {row['state']}{location}")
    lines.append("")
    registry = status["plans_registry"]
    location = (
        f" ({registry['file']}:{registry['line']})"
        if registry["state"] == UNKNOWN
        else ""
    )
    lines.append(
        f"Plans registry: testnet-path milestone {registry['state']}{location}"
    )
    chain = status["chain_state"]
    lines.append(
        f"Chain state head: updated {chain['updated']} — {chain['banner']} "
        f"({chain['file']}:{chain['updated_line']})"
    )
    return "\n".join(lines) + "\n"


def _table_cell(text: str) -> str:
    return text.replace("|", "\\|")


def render_markdown(status: dict[str, Any]) -> str:
    commit = status["newest_input_commit"]
    if commit:
        commit_line = (
            f"Generated at input commit `{commit['short']}` ({commit['date']}), the "
            "newest commit touching the input documents at generation time."
        )
    else:
        commit_line = (
            "Generated at input commit UNKNOWN (git history unavailable at "
            "generation time)."
        )
    lines = [
        "# Testnet Path Status",
        "",
        "!!! note \"Generated page — do not edit by hand\"",
        "",
        "    This page is generated by the testnet-path status CLI. Regenerate it",
        "    from the repository root with:",
        "",
        "    ```",
        "    PYTHONPATH=python python3 -m postfiat_rpc.testnet_path --markdown \\",
        "        > docs/status/testnet-path.md",
        "    ```",
        "",
        commit_line,
        "",
        "Inputs: the [testnet-path milestone](../plans/active/l1v2-public-testnet-path-milestone.md),",
        "the [storage scaling milestone](../plans/active/storage-scaling-milestone.md) gate table,",
        "the [plans registry](../plans/README.md), and",
        "[chain-state-current](chain-state-current.md).",
        "",
        "## Gate Zero — operator preconditions",
        "",
        "No community-facing step may start until all three close.",
        "",
        "| Gate | State | What it means |",
        "| --- | --- | --- |",
    ]
    for item in status["gate_zero"]:
        summary = _table_cell(item["summary"])
        if item["state"] == UNKNOWN:
            summary += f" ({item['file']}:{item['line']})"
        lines.append(f"| {item['label']} | {item['state']} | {summary} |")
    lines += ["", "## Phases A–E", ""]
    for phase in status["phases"]:
        lines += [
            f"### {phase['phase']} — {_table_cell(phase['title'])}",
            "",
            "| Item | State | What it means |",
            "| --- | --- | --- |",
        ]
        for item in phase["items"]:
            summary = _table_cell(item["summary"])
            if item["state"] == UNKNOWN:
                summary += f" ({item['file']}:{item['line']})"
            lines.append(f"| {item['label']} | {item['state']} | {summary} |")
        lines.append("")
    lines += [
        "## Storage milestone gates (G0–G7)",
        "",
        "Row states from the storage scaling milestone's gate-status table.",
        "",
        "| Gate | State |",
        "| --- | --- |",
    ]
    for row in status["storage_gates"]:
        state = _table_cell(row["state"])
        if row["state"] == UNKNOWN:
            state += f" ({row['file']}:{row['line']})"
        name = f" — {_table_cell(row['name'])}" if row["name"] else ""
        lines.append(f"| {row['gate']}{name} | {state} |")
    registry = status["plans_registry"]
    chain = status["chain_state"]
    lines += [
        "",
        "## Context",
        "",
        f"- Plans registry: the testnet-path milestone is {registry['state']}.",
        f"- Chain state head: updated {chain['updated']} — {_table_cell(chain['banner'])}.",
        "",
    ]
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="python -m postfiat_rpc.testnet_path",
        description="Print the current public-testnet-path gate states parsed "
        "from the committed repository documents.",
    )
    output = parser.add_mutually_exclusive_group()
    output.add_argument("--json", action="store_true", help="machine-readable output")
    output.add_argument(
        "--markdown",
        action="store_true",
        help="emit the docs/status/testnet-path.md page",
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=Path(__file__).resolve().parents[2],
        help="repository root (default: derived from this module's location)",
    )
    args = parser.parse_args(argv)

    status = collect_status(args.repo_root)
    if args.json:
        sys.stdout.write(json.dumps(status, indent=2, ensure_ascii=False) + "\n")
    elif args.markdown:
        sys.stdout.write(render_markdown(status))
    else:
        sys.stdout.write(render_plain(status))
    return 0


if __name__ == "__main__":
    sys.exit(main())
