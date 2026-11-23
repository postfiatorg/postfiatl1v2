import contextlib
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch

HERE = Path(__file__).resolve().parent
BOOK = (HERE / "RUNBOOK-LIVE.md").read_text()


class FifthReview(unittest.TestCase):
    def test_release_gate_counts_every_discovered_review_case(self):
        from exit_review_gate import run_gate
        names = [f"tests::solana_exit::exit_review{v}_case{i}" for v, n in [("",9),("4",8),("5",1)] for i in range(n)]
        listing = "".join(name+": test\n" for name in names)
        with tempfile.TemporaryDirectory() as d:
            def execute(args):
                return listing if "--list" in args else "test result: ok. 1 passed; 0 failed; 0 ignored;"
            self.assertEqual(run_gate(execute, Path(d)), len(names))
            with self.assertRaises(ValueError):
                run_gate(lambda args: listing if "--list" in args else "test result: ok. 0 passed; 0 failed;", Path(d))
            with self.assertRaises(ValueError):
                run_gate(lambda args: listing.replace("exit_review4_", "removed_") if "--list" in args else execute(args), Path(d))
