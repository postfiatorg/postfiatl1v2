#!/usr/bin/env python3
import json
import os
import sys
import tempfile
from pathlib import Path

from web3 import Web3

sys.path.insert(0, "/home/postfiat/repos/StakeHub")
from stakehub.agentd import call


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[5]
PROOF_DIR = HERE / "h316-h325-proof"
RESULT = HERE / "h316-h325-advance.json"
RPC = "https://ethereum-rpc.publicnode.com"
CHAIN_ID = 1
WALLET = Web3.to_checksum_address("0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0")
VERIFIER_ADDRESS = Web3.to_checksum_address("0xa77d5af456ef212303e31727b6ca4888cd771e2c")
VAULT_ADDRESS = Web3.to_checksum_address("0x8583409ddbac984ec195dfa06a21103d92403c1e")
TOKEN_ADDRESS = Web3.to_checksum_address("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
RECIPIENT = Web3.to_checksum_address("0xE568f9bBc54101DD0FAD10b37116A1E40b8ae8cC")
PROGRAM_VKEY = "0x007a22f1b8a47814a027ee0af8086a6f5f6ae4af0530dc7ffb2acac2da617834"
PRIOR_HEIGHT = 316
PRIOR_BLOCK_ID = "f7abc6a0a4a18a261c36a28bbaf0631ec77f9dd7dfe53545b8e4ffff40c67f9238a2b37e923bc81393e5cef84c56fd0c"
TARGET_HEIGHT = 325
TARGET_BLOCK_ID = "5157de5cc9a0cbb780be7ed063f8c09f372c2fb4ec9de6cf3d71345e38852ea9ab3ab4f515d46ce3ff4b066bd72178cd"


def write_json(path: Path, value: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    temporary = Path(name)
    try:
        with os.fdopen(fd, "w") as handle:
            json.dump(value, handle, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def main() -> None:
    artifact = json.loads(
        (
            REPO
            / "crates/ethereum-contracts/out/PFTLFinalityVerifierV1.sol/PFTLFinalityVerifierV1.json"
        ).read_text()
    )
    report = json.loads((PROOF_DIR / "proof-report.json").read_text())
    if report.get("proof_mode") != "groth16" or report.get("program_vkey") != PROGRAM_VKEY:
        raise RuntimeError(f"checkpoint proof mode/vkey mismatch: {report}")
    public_values = (PROOF_DIR / "public-values.bin").read_bytes()
    proof = (PROOF_DIR / "proof-calldata.bin").read_bytes()

    w3 = Web3(Web3.HTTPProvider(RPC, request_kwargs={"timeout": 60}))
    if w3.eth.chain_id != CHAIN_ID:
        raise RuntimeError(f"wrong chain: {w3.eth.chain_id}")
    verifier = w3.eth.contract(address=VERIFIER_ADDRESS, abi=artifact["abi"])
    if Web3.to_hex(verifier.functions.programVKey().call()).lower() != PROGRAM_VKEY:
        raise RuntimeError("deployed checkpoint verifier vkey changed")
    prior_commitment = Web3.keccak(bytes.fromhex(PRIOR_BLOCK_ID))
    target_commitment = Web3.keccak(bytes.fromhex(TARGET_BLOCK_ID))
    if (
        int(verifier.functions.latestFinalizedHeight().call()) != PRIOR_HEIGHT
        or verifier.functions.latestCheckpointCommitment().call() != prior_commitment
    ):
        raise RuntimeError("deployed verifier is not at the expected height-316 checkpoint")

    function = verifier.functions.advanceCheckpoint(public_values, proof)
    function.call({"from": WALLET})
    gas_estimate = int(function.estimate_gas({"from": WALLET}))
    status = call({"op": "status"})
    if not status or not status.get("ok") or not status.get("unlocked"):
        raise RuntimeError("StakeHub is unavailable or locked")
    session_id = "pfusdc-mainnet-epoch4-checkpoint-h325-20260726"
    call({"op": "close_launch_session", "session_id": session_id})
    opened = call(
        {
            "op": "open_launch_session",
            "session_id": session_id,
            "chain_id": CHAIN_ID,
            "allowlist": [
                WALLET,
                VERIFIER_ADDRESS,
                VAULT_ADDRESS,
                TOKEN_ADDRESS,
                RECIPIENT,
            ],
            "expected_deploys": [
                {
                    "label": "pfusdc_noop",
                    "bytecode_hash": "0x" + "00" * 32,
                    "bytecode_len": 1,
                }
            ],
            "usdc_address": TOKEN_ADDRESS,
            "usdc_budget": 0,
            "close_after_action": "advance_checkpoint_h325",
            "ttl_seconds": 1800,
        }
    )
    if not opened or not opened.get("ok"):
        raise RuntimeError(f"StakeHub launch session rejected: {opened}")
    sent = call(
        {
            "op": "evm_contract_tx",
            "to": VERIFIER_ADDRESS,
            "data": function._encode_transaction_data(),
            "rpc_url": RPC,
            "chain_id": CHAIN_ID,
            "label": "pfUSDC mainnet checkpoint 316->325",
            "session_id": session_id,
            "session_action": "advance_checkpoint_h325",
            "value_wei": 0,
            "gas_usd": 0,
        },
        timeout=1800.0,
    )
    tx_hash = (sent or {}).get("tx_hash") or (sent or {}).get("tx")
    if not sent or not sent.get("ok") or not tx_hash:
        raise RuntimeError(f"StakeHub checkpoint transaction failed: {sent}")
    receipt = w3.eth.wait_for_transaction_receipt(tx_hash, timeout=600)
    if receipt.status != 1:
        raise RuntimeError("checkpoint transaction reverted")
    if (
        int(verifier.functions.latestFinalizedHeight().call()) != TARGET_HEIGHT
        or verifier.functions.latestCheckpointCommitment().call() != target_commitment
    ):
        raise RuntimeError("checkpoint terminal state mismatch")
    gas_cost_wei = int(receipt.gasUsed) * int(receipt.effectiveGasPrice)
    result = {
        "schema": "postfiat-pfusdc-mainnet-checkpoint-advance-v1",
        "chain_id": CHAIN_ID,
        "verifier": VERIFIER_ADDRESS,
        "program_vkey": PROGRAM_VKEY,
        "prior_height": PRIOR_HEIGHT,
        "prior_block_id": PRIOR_BLOCK_ID,
        "prior_checkpoint_commitment": Web3.to_hex(prior_commitment),
        "target_height": TARGET_HEIGHT,
        "target_block_id": TARGET_BLOCK_ID,
        "target_checkpoint_commitment": Web3.to_hex(target_commitment),
        "tx_hash": tx_hash,
        "receipt_block_number": int(receipt.blockNumber),
        "receipt_block_hash": Web3.to_hex(receipt.blockHash),
        "gas_estimate": gas_estimate,
        "gas_used": int(receipt.gasUsed),
        "effective_gas_price": int(receipt.effectiveGasPrice),
        "gas_cost_wei": gas_cost_wei,
        "proof_report": report,
    }
    write_json(RESULT, result)
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
