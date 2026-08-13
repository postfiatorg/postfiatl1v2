#!/usr/bin/env python3
"""Unit tests for fail-closed Ethereum payout recovery validation."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import sys
import unittest


SCRIPT = Path(__file__).with_name("a666-mainnet-pfusdc-withdraw.py")
SPEC = importlib.util.spec_from_file_location("a666_mainnet_pfusdc_withdraw", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"cannot load {SCRIPT}")
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class RecoveryEventValidationTest(unittest.TestCase):
    WITHDRAWAL = "0x" + "11" * 32
    BURN = "0x" + "22" * 32
    RECIPIENT = "0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0"

    def event(self) -> dict:
        return {
            "args": {
                "withdrawalIdCommitment": bytes.fromhex(self.WITHDRAWAL[2:]),
                "burnTxIdCommitment": bytes.fromhex(self.BURN[2:]),
                "recipient": self.RECIPIENT,
                "amount": 481_552,
            }
        }

    def validate(self, event: dict) -> None:
        MODULE.validate_recovery_event(
            event,
            withdrawal_commitment=self.WITHDRAWAL,
            burn_commitment=self.BURN,
            recipient=self.RECIPIENT,
            amount_atoms=481_552,
        )

    def test_accepts_exact_bound_event(self) -> None:
        self.validate(self.event())

    def test_rejects_wrong_recipient(self) -> None:
        event = self.event()
        event["args"]["recipient"] = "0x0000000000000000000000000000000000000001"
        with self.assertRaisesRegex(RuntimeError, "wrong recipient"):
            self.validate(event)

    def test_rejects_wrong_amount(self) -> None:
        event = self.event()
        event["args"]["amount"] = 481_551
        with self.assertRaisesRegex(RuntimeError, "wrong amount"):
            self.validate(event)

    def test_rejects_wrong_commitment(self) -> None:
        event = self.event()
        event["args"]["burnTxIdCommitment"] = bytes.fromhex("33" * 32)
        with self.assertRaisesRegex(RuntimeError, "wrong burn commitment"):
            self.validate(event)


if __name__ == "__main__":
    unittest.main()
