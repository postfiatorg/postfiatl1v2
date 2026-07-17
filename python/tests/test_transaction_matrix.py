from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest import mock

from postfiat_rpc import RpcError
from postfiat_rpc.transaction_matrix import (
    disabled_probe,
    evidence_complete,
    latest_fleet_repair_dir,
    summarize_matrix,
    write_report,
)


class TransactionMatrixTests(unittest.TestCase):
    def test_latest_fleet_repair_dir_picks_latest_named_run(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "20260628T010000Z-fleet-repair").mkdir()
            (root / "20260628T020000Z-fleet-repair").mkdir()
            (root / "other").mkdir()

            self.assertEqual(
                latest_fleet_repair_dir(root),
                root / "20260628T020000Z-fleet-repair",
            )

    def test_evidence_complete_requires_all_paths(self) -> None:
        self.assertTrue(evidence_complete([
            {"path": "a", "exists": True},
            {"path": "b", "exists": True},
        ]))
        self.assertFalse(evidence_complete([
            {"path": "a", "exists": True},
            {"path": "b", "exists": False},
        ]))

    def test_disabled_probe_classifies_method_not_allowed(self) -> None:
        error = RpcError(
            "mempool_submit_signed_asset_transaction",
            {"code": "rpc_method_not_allowed", "message": "disabled"},
        )

        probe = disabled_probe("asset_submit", mock.Mock(side_effect=error))

        self.assertFalse(probe["ok"])
        self.assertEqual(probe["classification"], "disabled")

    def test_disabled_probe_classifies_validation_rejection_separately(self) -> None:
        error = RpcError(
            "mempool_submit_signed_asset_transaction",
            {"code": "asset_transaction_rejected", "message": "bad signature"},
        )

        probe = disabled_probe("asset_submit", mock.Mock(side_effect=error))

        self.assertFalse(probe["ok"])
        self.assertEqual(probe["classification"], "rejected")

    def test_summarize_matrix_lists_unproven_categories(self) -> None:
        summary = summarize_matrix([
            {"category": "Native", "status": "accepted_partial", "operations": []},
            {
                "category": "Asset",
                "status": "disabled_current_wallet_endpoint",
                "operations": [{"status": "unproven_live"}],
            },
            {
                "category": "Bridge",
                "status": "read_only_only_on_wallet_endpoint",
                "operations": [],
            },
        ])

        self.assertEqual(summary["accepted_categories"], ["Native"])
        self.assertEqual(summary["disabled_or_read_only_categories"], ["Asset", "Bridge"])
        self.assertEqual(summary["unproven_categories"], ["Asset"])
        self.assertEqual(summary["open_work_categories"], ["Asset", "Bridge"])

    def test_write_report_writes_json_and_markdown(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / "matrix.json"
            markdown = Path(tmp) / "matrix.md"
            report = {
                "created_at": "now",
                "proxy_url": "ws://example",
                "evidence_root": "reports/run",
                "categories": [{"category": "Native", "status": "accepted", "notes": []}],
                "summary": {"unproven_categories": []},
            }

            write_report(report, output, markdown)

            self.assertTrue(output.exists())
            self.assertIn("Transaction Permutation Matrix", markdown.read_text())


if __name__ == "__main__":
    unittest.main()
