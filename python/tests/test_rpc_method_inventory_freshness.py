"""The committed RPC method inventory must match what the generator derives
from the current source, so docs/runbooks/rpc-method-inventory.{json,md}
cannot silently lag the node's method surface (they lagged by eleven methods
between 2026-07-17 and 2026-10-09, and by fifteen python_client flags once the
detector learned to read black-wrapped calls)."""

from __future__ import annotations

import importlib.machinery
import importlib.util
import json
import re
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
GENERATOR = REPO / "scripts" / "testnet-rpc-method-inventory"
INVENTORY_JSON = REPO / "docs" / "runbooks" / "rpc-method-inventory.json"
INVENTORY_MD = REPO / "docs" / "runbooks" / "rpc-method-inventory.md"
METHOD_COVERAGE = REPO / "docs" / "rpc" / "method-coverage.md"


def _load_generator():
    loader = importlib.machinery.SourceFileLoader("testnet_rpc_method_inventory", str(GENERATOR))
    spec = importlib.util.spec_from_loader(loader.name, loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


def _methods(inventory: dict) -> dict[str, dict]:
    return {row["method"]: row for row in inventory["methods"]}


class RpcMethodInventoryFreshnessTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.committed = json.loads(INVENTORY_JSON.read_text(encoding="utf-8"))
        cls.generated = _load_generator().build_inventory()

    def test_committed_inventory_lists_exactly_the_methods_the_source_defines(self) -> None:
        committed = set(_methods(self.committed))
        generated = set(_methods(self.generated))
        self.assertEqual(
            sorted(generated - committed),
            [],
            "methods in the source but missing from the committed inventory; "
            "regenerate with scripts/testnet-rpc-method-inventory "
            "--output docs/runbooks/rpc-method-inventory.json "
            "--markdown docs/runbooks/rpc-method-inventory.md",
        )
        self.assertEqual(sorted(committed - generated), [], "stale methods in the committed inventory")

    def test_committed_rows_and_counts_match_the_generator(self) -> None:
        committed = _methods(self.committed)
        generated = _methods(self.generated)
        for name, row in generated.items():
            with self.subTest(method=name):
                # The whole derived row (posture, every gate flag, python_client,
                # sdk_constant, notes), so a detector or posture change cannot
                # leave the committed inventory stale in any column.
                self.assertEqual(committed[name], row)
        self.assertEqual(self.committed["counts"], self.generated["counts"])
        self.assertTrue(all(self.committed["checks"].values()), self.committed["checks"])

    def test_every_documented_public_read_is_a_read_only_public_method(self) -> None:
        rows = re.findall(r"^\| `([a-z_]+)` \| public read \|", METHOD_COVERAGE.read_text(encoding="utf-8"), re.M)
        self.assertGreaterEqual(len(rows), 21, rows)
        committed = _methods(self.committed)
        wrong = {name: committed.get(name, {}).get("posture") for name in rows if committed.get(name, {}).get("posture") != "read_only_public"}
        self.assertEqual(wrong, {})

    def test_markdown_report_matches_the_committed_json_summary(self) -> None:
        markdown = INVENTORY_MD.read_text(encoding="utf-8")
        counts = self.committed["counts"]
        self.assertIn(f"- Total methods observed: {counts['total_methods']}", markdown)
        self.assertIn(f"- Read-only public methods: {counts['read_only_public']}", markdown)
        self.assertIn(f"Date: {self.committed['generated_utc'][:10]}", markdown)


if __name__ == "__main__":
    unittest.main()
