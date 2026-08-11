#!/usr/bin/env python3
"""Regression tests for fail-closed remote runtime identity checks."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest


PROGRAM = Path(__file__).with_name("a666_remote_runtime.py")


def load_program():
    spec = importlib.util.spec_from_file_location("a666_remote_runtime", PROGRAM)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import {PROGRAM}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class RemoteRuntimeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.program = load_program()

    def rows(self):
        rows = []
        for index in range(6):
            for suffix in ("", "-rpc"):
                rows.append(
                    {
                        "node_id": f"validator-{index}",
                        "service": f"postfiat-validator-{index}{suffix}.service",
                        "pid": 1000 + index,
                        "expected_binary": "/opt/postfiat/releases/r1/postfiat-node",
                        "expected_binary_sha256": "a" * 64,
                        "active_binary": "/opt/postfiat/releases/r1/postfiat-node",
                        "active_binary_sha256": "a" * 64,
                        "topology_sha256": "b" * 64,
                        "topology_argument_present": True,
                    }
                )
        return rows

    def test_accepts_uniform_matching_fleet(self) -> None:
        self.program.validate_runtime_rows(self.rows())

    def test_rejects_active_binary_path_mismatch(self) -> None:
        rows = self.rows()
        rows[3]["active_binary"] = "/opt/postfiat/releases/old/postfiat-node"
        with self.assertRaisesRegex(RuntimeError, "runs .* not"):
            self.program.validate_runtime_rows(rows)

    def test_rejects_active_binary_hash_mismatch(self) -> None:
        rows = self.rows()
        rows[4]["active_binary_sha256"] = "c" * 64
        with self.assertRaisesRegex(RuntimeError, "binary SHA-256 mismatch"):
            self.program.validate_runtime_rows(rows)

    def test_rejects_missing_topology_argument(self) -> None:
        rows = self.rows()
        rows[0]["topology_argument_present"] = False
        with self.assertRaisesRegex(RuntimeError, "requested topology path"):
            self.program.validate_runtime_rows(rows)

    def test_rejects_cross_host_topology_drift(self) -> None:
        rows = self.rows()
        rows[-1]["topology_sha256"] = "c" * 64
        with self.assertRaisesRegex(RuntimeError, "one identical topology"):
            self.program.validate_runtime_rows(rows)

    def test_parse_rejects_malformed_probe(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "malformed output"):
            self.program.parse_runtime_probe("too\\tfew\\tfields")


if __name__ == "__main__":
    unittest.main()
