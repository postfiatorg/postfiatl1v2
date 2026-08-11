#!/usr/bin/env python3
"""Execute the frozen $10 epoch-6 successor deposit through unlocked agentd.

This is a parameter-only adapter over the previously audited epoch-4 sender.
It fixes every consumer-visible input, including the nonce, and normalizes the
successful report to the ingress claim runner's evidence schema.
"""

from __future__ import annotations

import importlib.util
import json
import os
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BASE = (
    ROOT
    / "docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/"
    "recovery-epoch4/deposit/execute_epoch4_deposit.py"
)
HERE = (
    ROOT
    / "docs/evidence/a666-egress-lane-redeploy-20260809/epoch6-successor/"
    "roundtrip/deposit"
)
MANIFEST = (
    ROOT
    / "docs/evidence/a666-egress-lane-redeploy-20260809/epoch6-successor/"
    "deploy/manifest.postdeploy-enriched.json"
)
NONCE = bytes.fromhex(
    "28fda3f9d462a9a1d68e50259c6b2a07a2782de3a1e1ed9e1f9ea125862d3187"
)


def main() -> None:
    output = HERE / "deposit-result.json"
    if output.exists():
        raise RuntimeError(f"refusing to overwrite deposit evidence: {output}")

    spec = importlib.util.spec_from_file_location("audited_epoch4_deposit_sender", BASE)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load audited deposit sender: {BASE}")
    sender = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = sender
    spec.loader.exec_module(sender)

    sender.HERE = HERE
    sender.MANIFEST = MANIFEST
    sender.EXPECTED_MANIFEST_SHA256 = (
        "b213a7462ba5495977a6795376dada0a56d48ec58991b1821f2cb2463e7532be"
    )
    sender.CHAIN_ID = 1
    sender.RPC = "https://ethereum-rpc.publicnode.com"
    sender.VAULT = "0x4939a45caa85Da31Fb26D7DBe6477B45F7f08688"
    sender.VERIFIER = "0xA53926F0F7453ad9f8dCa592A076991eC627838C"
    sender.USDC = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
    sender.WALLET = "0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0"
    sender.RECIPIENT = "pfab9b9228942e5c529633a13aa271d5297bec6353"
    sender.AMOUNT_ATOMS = 10_000_000
    sender.ROUTE_BINDING = (
        "0a8b4b4184ca85b6b3f4e54d8bd581c747da3984df13b85d3610a57613d7228c"
    )
    sender.EXPECTED_VAULT_RUNTIME_KECCAK = (
        "c6dbb722c23bfc841624bb909fcb54d84a65a2ea6ece96e2a28bf61d5dea6d05"
    )

    original_urandom = os.urandom
    try:
        sender.os.urandom = lambda length: NONCE if length == 32 else original_urandom(length)
        sender.main()
    finally:
        sender.os.urandom = original_urandom

    report = json.loads(output.read_text(encoding="utf-8"))
    expected = {
        "verdict": "PASS",
        "manifest_sha256": sender.EXPECTED_MANIFEST_SHA256,
        "chain_id": sender.CHAIN_ID,
        "vault": sender.VAULT,
        "verifier": sender.VERIFIER,
        "usdc": sender.USDC,
        "depositor": sender.WALLET,
        "pftl_recipient": sender.RECIPIENT,
        "amount_atoms": sender.AMOUNT_ATOMS,
        "nonce": "0x" + NONCE.hex(),
        "route_binding": "0x" + sender.ROUTE_BINDING,
    }
    for field, value in expected.items():
        actual = report.get(field)
        if isinstance(value, str) and value.startswith("0x"):
            matches = isinstance(actual, str) and actual.lower() == value.lower()
        else:
            matches = actual == value
        if not matches:
            raise RuntimeError(f"deposit report {field} {actual!r} != {value!r}")
    report["source_schema"] = report["schema"]
    report["schema"] = "postfiat.a666.pfusdc_buyer_deposit.v1"
    report["workflow_id"] = "a666-epoch6-successor-10usdc-20260810"
    output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
