#!/usr/bin/env python3
"""Withdraw the latency-gate proof using the audited epoch-4 sender."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
BASE = (
    REPO
    / "docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/"
    "recovery-epoch4/egress/withdraw_mainnet.py"
)


def main() -> None:
    spec = importlib.util.spec_from_file_location("audited_epoch4_withdrawal_sender", BASE)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load audited withdrawal sender: {BASE}")
    sender = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = sender
    spec.loader.exec_module(sender)
    sender.HERE = HERE
    sender.PROOF_DIR = HERE / "proof"
    sender.RESULT = HERE / "withdrawal-result.json"
    sender.AMOUNT = 1_000_000
    sender.main()


if __name__ == "__main__":
    main()
