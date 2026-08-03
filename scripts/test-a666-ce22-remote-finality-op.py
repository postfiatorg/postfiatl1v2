#!/usr/bin/env python3
"""Regression tests for exact signed-release selection in remote finality."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest


PROGRAM = Path(__file__).with_name("a666-ce22-remote-finality-op.py")


def load_program():
    spec = importlib.util.spec_from_file_location("a666_remote_finality", PROGRAM)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import {PROGRAM}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class RemoteFinalityReleaseTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.program = load_program()

    def test_accepts_matching_signed_release_paths(self) -> None:
        release_id = self.program.validated_release_id(
            "/opt/postfiat/releases/a666-public-reserve-abc123/postfiat-node",
            "/etc/postfiat/releases/a666-public-reserve-abc123/topology.json",
        )
        self.assertEqual(release_id, "a666-public-reserve-abc123")

    def test_rejects_mismatched_release_ids(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "different release IDs"):
            self.program.validated_release_id(
                "/opt/postfiat/releases/release-a/postfiat-node",
                "/etc/postfiat/releases/release-b/topology.json",
            )

    def test_rejects_relative_binary_path(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "absolute signed-release paths"):
            self.program.validated_release_id(
                "target/release/postfiat-node",
                "/etc/postfiat/releases/release-a/topology.json",
            )

    def test_rejects_non_release_topology_path(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "absolute signed-release paths"):
            self.program.validated_release_id(
                "/opt/postfiat/releases/release-a/postfiat-node",
                "/etc/postfiat/topology.json",
            )


if __name__ == "__main__":
    unittest.main()
