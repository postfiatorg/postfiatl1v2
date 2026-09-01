from __future__ import annotations

import unittest

from postfiat_rpc.testnet_path import (
    parse_chain_state_head,
    parse_gate_table,
    parse_milestone,
    parse_plans_readme,
)

MILESTONE_SNIPPET = """\
# L1v2 Public Testnet Path

## Gate Zero — operator preconditions (recorded 2026-08-30)

- [ ] Z1 **Cobalt is live**: Cobalt-ratified transitions are the sole
      registry-change path on the running devnet
- [ ] Z2 **AI-governance decision made and live**: the C2 shadow evaluation
- [ ] Z3 **NAVCoin swaps work end to end**: full round trips

## A — Finish offline storage qualification (G3 → G5)

- [x] G4 scaling PASS at candidate `d0ae79f3`
- [ ] A1 *(operator, external)* Re-supply the height-915 quarantine archive

## B — Pre-deployment rehearsal and deployment decision (G6)

- [x] B1 *(operator)* Authorized and captured six copies
- [ ] B2 Run the six-clone migration rehearsal

## C — Validator story

- [ ] C1 Complete Dynamic UNL governance verification

## D — Public-testnet eligibility gates

- [ ] D1 Inventory the non-storage release gates

## E — Mandate deliverables

- [ ] E1 Python CLI: `testnet-path` status tool
"""

GATE_TABLE_SNIPPET = """\
## Gated to-do list

| Gate | Current state | Work allowed now | Budget and advance rule |
| --- | --- | --- | --- |
| G0 — campaign control | **PASS** | None. | Reopen only if resume changes. |
| G1 — candidate freeze | **PASS FOR CLOSED FAILED LINEAGE** | Preserve. | New freeze. |
| G2 — safety | **EAGER-INDEX LOCAL PASS / PACKET BINDING PENDING** | Preserve. | Open. |
| G3 — exact replay | **HEIGHT 915 INPUT OPEN** | Preserve. | Do not claim. |
| G4 — scaling | **HISTORICAL PASS FOR `d0ae79f3` ONLY** | Preserve. | No successor. |
| G5 — offline packet | **BLOCKED BY HEIGHT 915** | Do not package. | Needs input. |
| G6 — six-clone rehearsal | **OLD RUNNER PASS INVALIDATED** | Preserve. | Repair. |
| G7 — Dynamic UNL handoff | **DIRECTION RECORDED / IMPLEMENTATION DEFERRED** | Preserve. | Later. |
"""


class MilestoneParseTest(unittest.TestCase):
    def test_checked_and_unchecked_items(self) -> None:
        parsed = parse_milestone(MILESTONE_SNIPPET, "milestone.md")
        gate_zero = {item.label: item for item in parsed["gate_zero"]}
        self.assertEqual(
            [item.label for item in parsed["gate_zero"]], ["Z1", "Z2", "Z3"]
        )
        self.assertEqual(gate_zero["Z1"].state, "OPEN")
        self.assertIn("Cobalt is live", gate_zero["Z1"].summary)
        self.assertEqual(gate_zero["Z1"].line, 5)

        phases = {phase.phase: phase for phase in parsed["phases"]}
        self.assertEqual(sorted(phases), ["A", "B", "C", "D", "E"])
        phase_a = {item.label: item for item in phases["A"].items}
        self.assertEqual(phase_a["G4"].state, "DONE")
        self.assertEqual(phase_a["A1"].state, "OPEN")
        self.assertEqual(phases["B"].items[0].label, "B1")
        self.assertEqual(phases["B"].items[0].state, "DONE")
        self.assertEqual(phases["E"].items[0].state, "OPEN")

    def test_unknown_for_malformed_marker_and_missing_items(self) -> None:
        snippet = (
            "## Gate Zero — operator preconditions\n"
            "\n"
            "- [?] Z1 **Cobalt is live**: malformed marker\n"
            "- [ ] Z2 **AI-governance decision made and live**\n"
        )
        parsed = parse_milestone(snippet, "milestone.md")
        gate_zero = {item.label: item for item in parsed["gate_zero"]}
        self.assertEqual(gate_zero["Z1"].state, "UNKNOWN")
        self.assertEqual(gate_zero["Z1"].file, "milestone.md")
        self.assertEqual(gate_zero["Z1"].line, 3)
        self.assertEqual(gate_zero["Z2"].state, "OPEN")
        # Z3 is absent entirely: UNKNOWN, pointing at the section heading.
        self.assertEqual(gate_zero["Z3"].state, "UNKNOWN")
        self.assertEqual(gate_zero["Z3"].line, 1)
        # No phase sections at all: every phase reports UNKNOWN.
        for phase in parsed["phases"]:
            self.assertEqual(phase.items[0].state, "UNKNOWN")
            self.assertEqual(phase.items[0].file, "milestone.md")


class GateTableParseTest(unittest.TestCase):
    def test_gate_table_rows(self) -> None:
        rows = {row.gate: row for row in parse_gate_table(GATE_TABLE_SNIPPET, "storage.md")}
        self.assertEqual(sorted(rows), [f"G{i}" for i in range(8)])
        self.assertEqual(rows["G0"].state, "PASS")
        self.assertEqual(rows["G0"].name, "campaign control")
        self.assertEqual(rows["G0"].line, 5)
        self.assertEqual(
            rows["G2"].state, "EAGER-INDEX LOCAL PASS / PACKET BINDING PENDING"
        )
        self.assertEqual(rows["G4"].state, "HISTORICAL PASS FOR d0ae79f3 ONLY")

    def test_missing_rows_are_unknown(self) -> None:
        truncated = "\n".join(GATE_TABLE_SNIPPET.splitlines()[:7]) + "\n"
        rows = {row.gate: row for row in parse_gate_table(truncated, "storage.md")}
        self.assertEqual(rows["G0"].state, "PASS")
        self.assertEqual(rows["G7"].state, "UNKNOWN")
        self.assertEqual(rows["G7"].file, "storage.md")
        self.assertEqual(rows["G7"].line, 3)  # points at the table header line

    def test_no_table_at_all(self) -> None:
        rows = parse_gate_table("no table here\n", "storage.md")
        self.assertTrue(all(row.state == "UNKNOWN" for row in rows))
        self.assertTrue(all(row.line == 1 for row in rows))


class RegistryAndChainStateTest(unittest.TestCase):
    def test_registry_listing(self) -> None:
        readme = (
            "# Plans\n\n## Active\n\n"
            "- [L1v2 Public Testnet Path](active/l1v2-public-testnet-path-milestone.md)\n"
        )
        registry = parse_plans_readme(readme, "README.md")
        self.assertEqual(registry["state"], "LISTED ACTIVE")
        self.assertEqual(registry["line"], 5)
        missing = parse_plans_readme("# Plans\n", "README.md")
        self.assertEqual(missing["state"], "UNKNOWN")

    def test_chain_state_head(self) -> None:
        head = (
            "# PostFiat L1 Current State\n\n"
            "Updated: `2026-08-31T04:30:00Z`\n\n"
            '!!! success "2026-08-31: storage DEPLOYED at height 931"\n'
        )
        chain = parse_chain_state_head(head, "chain.md")
        self.assertEqual(chain["updated"], "2026-08-31T04:30:00Z")
        self.assertEqual(chain["banner"], "2026-08-31: storage DEPLOYED at height 931")
        empty = parse_chain_state_head("", "chain.md")
        self.assertEqual(empty["updated"], "UNKNOWN")
        self.assertEqual(empty["banner"], "UNKNOWN")


if __name__ == "__main__":
    unittest.main()
