#!/usr/bin/env python3
"""Advance the ownerless Ethereum PFTL verifier with one SP1 checkpoint proof."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
import time
from typing import Any

from web3 import Web3


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "python"))
from postfiat_ops.constrained_signer import submit_evm_transaction

RPC = "https://ethereum-rpc.publicnode.com"
CHAIN_ID = 1
ROUTE_ID = "pftl-a666-ethereum-wA666-usdc-v1"
ROUTE_CONFIG_DIGEST = "12ed00ca87e29554ce4b978da1710fffc0830767e84e62f08df257f727db953efdd89bcf6ea99f5634d6e5ea8aca2933"
SIGNER_SOCKET = Path(os.environ.get("POSTFIAT_SIGNER_SOCKET", "/run/postfiat/a666-signer.sock"))
MAXIMUM_FEE_WEI = int(os.environ.get("POSTFIAT_SIGNER_MAXIMUM_FEE_WEI", "10000000000000000"))
MUTATION_NOT_AFTER_EPOCH = int(os.environ.get("POSTFIAT_MUTATION_NOT_AFTER_EPOCH", "0"))
OWNER = Web3.to_checksum_address("0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0")
VERIFIER = Web3.to_checksum_address("0xb79FF97EcC11574a8A78d0b5a9D7C8c2A94bF96A")
PROGRAM_VKEY = "0x004e44aca326861252ee5ff7863b1174635b727759b75d46b28bb28d4a7b34f9"


def atomic_write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    temporary = Path(name)
    try:
        with os.fdopen(descriptor, "w") as handle:
            json.dump(value, handle, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def artifact() -> dict[str, Any]:
    path = ROOT / (
        "crates/ethereum-contracts/out/PFTLReceiptFinalityVerifierV1.sol/"
        "PFTLReceiptFinalityVerifierV1.json"
    )
    return json.loads(path.read_text())


def normalize_tx_hash(value: str) -> str:
    return value if value.startswith("0x") else f"0x{value}"


def enforce_mutation_deadline() -> None:
    if MUTATION_NOT_AFTER_EPOCH and int(time.time()) >= MUTATION_NOT_AFTER_EPOCH:
        raise RuntimeError(
            "checkpoint advance mutation deadline margin reached at epoch "
            f"{MUTATION_NOT_AFTER_EPOCH}"
        )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--proof-dir", type=Path, required=True)
    parser.add_argument("--prior-block-id", required=True)
    parser.add_argument("--target-block-id", required=True)
    parser.add_argument("--prior-height", type=int, required=True)
    parser.add_argument("--target-height", type=int, required=True)
    parser.add_argument("--state-file", type=Path, required=True)
    args = parser.parse_args()

    if args.prior_height <= 0 or args.target_height <= args.prior_height:
        raise RuntimeError("checkpoint heights are not strictly increasing")
    if args.target_height - args.prior_height > 65:
        raise RuntimeError("checkpoint proof exceeds the 64-ancestry-step bound")
    for label, value in (
        ("prior", args.prior_block_id),
        ("target", args.target_block_id),
    ):
        if len(value) != 96 or any(char not in "0123456789abcdef" for char in value):
            raise RuntimeError(f"{label} checkpoint block id is malformed")

    public_values = (args.proof_dir / "public-values.bin").read_bytes()
    proof = (args.proof_dir / "proof-calldata.bin").read_bytes()
    report = json.loads((args.proof_dir / "proof-report.json").read_text())
    if len(public_values) != 256 or not proof:
        raise RuntimeError("missing or malformed checkpoint proof artifacts")
    if (
        report.get("program_vkey") != PROGRAM_VKEY
        or report.get("proof_mode") != "groth16"
        or report.get("prover_backend") != "cuda"
        or report.get("public_values_bytes") != 256
    ):
        raise RuntimeError("checkpoint proof report does not match the deployed verifier")

    web3 = Web3(Web3.HTTPProvider(RPC, request_kwargs={"timeout": 120}))
    if not web3.is_connected() or int(web3.eth.chain_id) != CHAIN_ID:
        raise RuntimeError("Ethereum mainnet RPC unavailable or wrong chain")
    verifier = web3.eth.contract(address=VERIFIER, abi=artifact()["abi"])
    if Web3.to_hex(verifier.functions.programVKey().call()).lower() != PROGRAM_VKEY:
        raise RuntimeError("deployed verifier program vkey changed")

    prior_commitment = Web3.keccak(bytes.fromhex(args.prior_block_id))
    target_commitment = Web3.keccak(bytes.fromhex(args.target_block_id))
    pre_height = int(verifier.functions.latestFinalizedHeight().call())
    pre_commitment = verifier.functions.latestCheckpointCommitment().call()
    if pre_height != args.prior_height or pre_commitment != prior_commitment:
        raise RuntimeError("deployed verifier is not at the requested prior checkpoint")

    function = verifier.functions.advanceCheckpoint(public_values, proof)
    function.call({"from": OWNER})
    gas_estimate = int(function.estimate_gas({"from": OWNER}))
    state: dict[str, Any] = {
        "schema": "postfiat.a666.pftl_checkpoint_advance.v1",
        "phase": "prepared",
        "chain_id": CHAIN_ID,
        "verifier": VERIFIER,
        "program_vkey": PROGRAM_VKEY,
        "prior_height": args.prior_height,
        "prior_block_id": args.prior_block_id,
        "prior_checkpoint_commitment": Web3.to_hex(prior_commitment),
        "target_height": args.target_height,
        "target_block_id": args.target_block_id,
        "target_checkpoint_commitment": Web3.to_hex(target_commitment),
        "gas_estimate": gas_estimate,
    }
    atomic_write_json(args.state_file, state)
    if not args.execute:
        print(json.dumps(state, indent=2, sort_keys=True))
        return

    calldata = function._encode_transaction_data()
    enforce_mutation_deadline()
    response = submit_evm_transaction(
        SIGNER_SOCKET,
        chain_id=CHAIN_ID,
        transaction_kind="a666_checkpoint_advance",
        target_contract=VERIFIER.lower(),
        calldata=calldata,
        native_value_wei=0,
        maximum_fee_wei=MAXIMUM_FEE_WEI,
        route_id=ROUTE_ID,
        route_config_digest=ROUTE_CONFIG_DIGEST,
        label=f"advance A666 PFTL checkpoint {args.prior_height}->{args.target_height}",
        idempotency_key="a666-checkpoint-" + hashlib.sha256(
            f"{ROUTE_CONFIG_DIGEST}:{VERIFIER.lower()}:{calldata.lower()}".encode()
        ).hexdigest(),
        timeout=1200.0,
    )
    raw_tx = response.get("transaction_hash")
    if not raw_tx:
        raise RuntimeError("checkpoint transaction response omitted its hash")
    transaction_hash = normalize_tx_hash(raw_tx)
    receipt = web3.eth.wait_for_transaction_receipt(transaction_hash, timeout=600)
    if int(receipt.status) != 1:
        raise RuntimeError(f"checkpoint transaction reverted: {transaction_hash}")
    if (
        int(verifier.functions.latestFinalizedHeight().call()) != args.target_height
        or verifier.functions.latestCheckpointCommitment().call() != target_commitment
    ):
        raise RuntimeError("checkpoint terminal state mismatch")
    state.update(
        {
            "phase": "checkpoint-advanced",
            "transaction_hash": transaction_hash,
            "receipt_block_number": int(receipt.blockNumber),
            "gas_used": int(receipt.gasUsed),
            "effective_gas_price": int(receipt.effectiveGasPrice),
        }
    )
    atomic_write_json(args.state_file, state)
    print(json.dumps(state, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
