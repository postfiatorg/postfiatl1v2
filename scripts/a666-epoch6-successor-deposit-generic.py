#!/usr/bin/env python3
"""Execute one nonce-unique deposit into the frozen epoch-6 successor vault."""

from __future__ import annotations

import argparse
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
MANIFEST = (
    ROOT
    / "docs/evidence/a666-egress-lane-redeploy-20260809/epoch6-successor/"
    "deploy/manifest.postdeploy-enriched.json"
)
MANIFEST_SHA256 = "b213a7462ba5495977a6795376dada0a56d48ec58991b1821f2cb2463e7532be"
VAULT = "0x4939a45caa85Da31Fb26D7DBe6477B45F7f08688"
VERIFIER = "0xA53926F0F7453ad9f8dCa592A076991eC627838C"
USDC = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
WALLET = "0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0"
RECIPIENT = "pfab9b9228942e5c529633a13aa271d5297bec6353"
ROUTE_BINDING = "0a8b4b4184ca85b6b3f4e54d8bd581c747da3984df13b85d3610a57613d7228c"
VAULT_RUNTIME_KECCAK = "c6dbb722c23bfc841624bb909fcb54d84a65a2ea6ece96e2a28bf61d5dea6d05"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--amount-atoms", type=int, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--workflow-id", required=True)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.amount_atoms <= 0:
        raise RuntimeError("--amount-atoms must be positive")
    if not args.workflow_id or len(args.workflow_id) > 64:
        raise RuntimeError("--workflow-id must contain at most 64 characters")
    output = args.output_dir / "deposit-result.json"
    if args.output_dir.exists():
        raise RuntimeError(f"refusing to overwrite deposit evidence: {args.output_dir}")
    args.output_dir.mkdir(parents=True, mode=0o700)

    spec = importlib.util.spec_from_file_location("audited_epoch4_deposit_sender", BASE)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load audited deposit sender: {BASE}")
    sender = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = sender
    spec.loader.exec_module(sender)

    sender.HERE = args.output_dir
    sender.BUILDER = Path(
        os.environ.get("A666_DEPOSIT_BUILDER", str(sender.BUILDER))
    )
    sender.MANIFEST = MANIFEST
    sender.EXPECTED_MANIFEST_SHA256 = MANIFEST_SHA256
    sender.CHAIN_ID = 1
    sender.RPC = os.environ.get(
        "A666_ETHEREUM_RPC", "https://ethereum-rpc.publicnode.com"
    )
    sender.VAULT = VAULT
    sender.VERIFIER = VERIFIER
    sender.USDC = USDC
    sender.WALLET = WALLET
    sender.RECIPIENT = RECIPIENT
    sender.AMOUNT_ATOMS = args.amount_atoms
    sender.ROUTE_BINDING = ROUTE_BINDING
    sender.EXPECTED_VAULT_RUNTIME_KECCAK = VAULT_RUNTIME_KECCAK
    sender.main()

    report = json.loads(output.read_text(encoding="utf-8"))
    expected = {
        "verdict": "PASS",
        "manifest_sha256": MANIFEST_SHA256,
        "chain_id": 1,
        "vault": VAULT,
        "verifier": VERIFIER,
        "usdc": USDC,
        "depositor": WALLET,
        "pftl_recipient": RECIPIENT,
        "amount_atoms": args.amount_atoms,
        "route_binding": "0x" + ROUTE_BINDING,
    }
    for field, value in expected.items():
        actual = report.get(field)
        if isinstance(value, str) and value.startswith("0x"):
            matches = isinstance(actual, str) and actual.lower() == value.lower()
        else:
            matches = actual == value
        if not matches:
            raise RuntimeError(f"deposit report {field} {actual!r} != {value!r}")
    nonce = report.get("nonce")
    if not isinstance(nonce, str) or len(nonce.removeprefix("0x")) != 64:
        raise RuntimeError("deposit report has no canonical nonce")
    report["source_schema"] = report["schema"]
    report["schema"] = "postfiat.a666.pfusdc_buyer_deposit.v1"
    report["workflow_id"] = args.workflow_id
    output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
