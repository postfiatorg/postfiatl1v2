#!/usr/bin/env python3
"""Withdraw an exact proof-native pfUSDC redemption from a frozen vault lane."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import sys
from pathlib import Path

from web3 import Web3


REPO = Path(__file__).resolve().parents[1]
BASE = (
    REPO
    / "docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/"
    "recovery-epoch4/egress/withdraw_mainnet.py"
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--proof-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--deployment-manifest", type=Path, required=True)
    parser.add_argument("--expected-manifest-sha256", required=True)
    parser.add_argument("--amount-atoms", type=int, required=True)
    parser.add_argument("--recipient", required=True)
    parser.add_argument(
        "--stakehub-repo",
        type=Path,
        default=Path("/home/postfiat/repos/StakeHub-master-e6"),
    )
    parser.add_argument(
        "--contract-artifact-root",
        type=Path,
        default=REPO,
        help="repository containing the compiled epoch-bound contract ABIs",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.amount_atoms <= 0:
        raise RuntimeError("--amount-atoms must be positive")
    if args.output.exists():
        raise RuntimeError(f"refusing to overwrite evidence: {args.output}")
    manifest_bytes = args.deployment_manifest.read_bytes()
    manifest_digest = hashlib.sha256(manifest_bytes).hexdigest()
    if manifest_digest != args.expected_manifest_sha256:
        raise RuntimeError(
            f"deployment manifest digest {manifest_digest} does not match frozen digest"
        )
    manifest = json.loads(manifest_bytes)
    route = manifest["route"]
    network = manifest["network"]
    programs = manifest["programs"]
    if route["route_id"] != "ethereum-mainnet-usdc-v1":
        raise RuntimeError("deployment manifest is not the Ethereum mainnet USDC route")
    if int(route["route_epoch"]) < 5:
        raise RuntimeError("deployment manifest predates the epoch-5 recovery lane")
    if int(network["source_chain_id"]) != 1:
        raise RuntimeError("deployment manifest is not for Ethereum mainnet")
    stakehub_package = args.stakehub_repo / "stakehub" / "agentd.py"
    if not stakehub_package.is_file():
        raise RuntimeError(f"StakeHub agent module is missing: {stakehub_package}")
    sys.path.insert(0, str(args.stakehub_repo))

    spec = importlib.util.spec_from_file_location(
        "audited_pfusdc_mainnet_withdrawal_sender", BASE
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load audited withdrawal sender: {BASE}")
    sender = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = sender
    spec.loader.exec_module(sender)

    sender.HERE = args.output.parent
    sender.REPO = args.contract_artifact_root.resolve()
    sender.PROOF_DIR = args.proof_dir
    sender.RESULT = args.output
    sender.AMOUNT = args.amount_atoms
    sender.RECIPIENT = Web3.to_checksum_address(args.recipient)
    sender.VAULT_ADDRESS = Web3.to_checksum_address(route["vault_address"])
    sender.VERIFIER_ADDRESS = Web3.to_checksum_address(route["verifier_address"])
    sender.TOKEN_ADDRESS = Web3.to_checksum_address(network["token"]["address"])
    sender.PROGRAM_VKEY = programs["egress"]["program_vkey"]
    sender.main()


if __name__ == "__main__":
    main()
