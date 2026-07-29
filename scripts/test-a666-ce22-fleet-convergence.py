#!/usr/bin/env python3
"""Regression tests for transient six-validator convergence lag."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch


SCRIPTS = Path(__file__).resolve().parent


def load_module(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / filename)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {filename}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class FleetConvergenceTests(unittest.TestCase):
    def test_preflight_retries_transient_divergence(self) -> None:
        module = load_module("ce22_finality", "a666-ce22-finality-op.py")
        converged = [{"block_height": 449}]
        with (
            patch.object(
                module,
                "fleet_status",
                side_effect=[
                    RuntimeError("ce22 fleet is not 6/6 on one parent"),
                    converged,
                ],
            ),
            patch.object(module.time, "sleep"),
        ):
            self.assertIs(
                module.wait_for_fleet_status([1, 2, 3, 4, 5, 6], 1.0, 1.0),
                converged,
            )

    def test_preflight_timeout_preserves_last_error(self) -> None:
        module = load_module("ce22_finality_timeout", "a666-ce22-finality-op.py")
        with patch.object(
            module,
            "fleet_status",
            side_effect=RuntimeError("still divergent"),
        ):
            with self.assertRaisesRegex(RuntimeError, "still divergent"):
                module.wait_for_fleet_status(
                    [1, 2, 3, 4, 5, 6],
                    1.0,
                    0.0,
                )

    def test_postflight_retries_wrong_height_and_divergence(self) -> None:
        module = load_module(
            "ce22_batch",
            "a666-ce22-remote-finality-batch.py",
        )

        class Rpc:
            def __init__(self) -> None:
                self.calls = 0

            def fleet_status(self, _ports, _timeout):
                self.calls += 1
                if self.calls == 1:
                    raise RuntimeError("temporarily divergent")
                if self.calls == 2:
                    return [{"block_height": 448}]
                return [{"block_height": 449}]

        rpc = Rpc()
        with patch.object(module.time, "sleep"):
            rows = module.wait_for_height(
                rpc,
                [1, 2, 3, 4, 5, 6],
                1.0,
                1.0,
                449,
            )
        self.assertEqual(rows[0]["block_height"], 449)
        self.assertEqual(rpc.calls, 3)


if __name__ == "__main__":
    unittest.main()
