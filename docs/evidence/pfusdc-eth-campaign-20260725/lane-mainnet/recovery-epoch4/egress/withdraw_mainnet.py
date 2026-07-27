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
PROOF_DIR = HERE / "proof"
RESULT = HERE / "withdrawal-result.json"
RPC = "https://ethereum-rpc.publicnode.com"
CHAIN_ID = 1
AMOUNT = 25_000_000
WALLET = Web3.to_checksum_address("0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0")
RECIPIENT = Web3.to_checksum_address("0xE568f9bBc54101DD0FAD10b37116A1E40b8ae8cC")
VAULT_ADDRESS = Web3.to_checksum_address("0x8583409ddbac984ec195dfa06a21103d92403c1e")
VERIFIER_ADDRESS = Web3.to_checksum_address("0xa77d5af456ef212303e31727b6ca4888cd771e2c")
TOKEN_ADDRESS = Web3.to_checksum_address("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
PROGRAM_VKEY = "0x007a22f1b8a47814a027ee0af8086a6f5f6ae4af0530dc7ffb2acac2da617834"


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
    vault_artifact = json.loads(
        (
            REPO
            / "crates/ethereum-contracts/out/ERC20BridgeVaultV2.sol/ERC20BridgeVaultV2.json"
        ).read_text()
    )
    verifier_artifact = json.loads(
        (
            REPO
            / "crates/ethereum-contracts/out/PFTLFinalityVerifierV1.sol/PFTLFinalityVerifierV1.json"
        ).read_text()
    )
    proof_report = json.loads((PROOF_DIR / "proof-report.json").read_text())
    if proof_report.get("proof_mode") != "groth16" or proof_report.get("program_vkey") != PROGRAM_VKEY:
        raise RuntimeError(f"egress proof mode/vkey mismatch: {proof_report}")
    public_values = (PROOF_DIR / "public-values.bin").read_bytes()
    proof = (PROOF_DIR / "proof-calldata.bin").read_bytes()

    w3 = Web3(Web3.HTTPProvider(RPC, request_kwargs={"timeout": 60}))
    if w3.eth.chain_id != CHAIN_ID:
        raise RuntimeError(f"wrong chain: {w3.eth.chain_id}")
    vault = w3.eth.contract(address=VAULT_ADDRESS, abi=vault_artifact["abi"])
    verifier = w3.eth.contract(address=VERIFIER_ADDRESS, abi=verifier_artifact["abi"])
    token = w3.eth.contract(
        address=TOKEN_ADDRESS,
        abi=[
            {
                "type": "function",
                "name": "balanceOf",
                "stateMutability": "view",
                "inputs": [{"name": "account", "type": "address"}],
                "outputs": [{"name": "", "type": "uint256"}],
            }
        ],
    )
    if Web3.to_hex(verifier.functions.programVKey().call()).lower() != PROGRAM_VKEY:
        raise RuntimeError("deployed finality verifier vkey changed")
    decoded = verifier.functions.decodePublicValues(public_values).call()
    burn_commitment = Web3.to_hex(decoded[14])
    withdrawal_commitment = Web3.to_hex(decoded[15])
    proof_nullifier = Web3.to_hex(decoded[26])
    if int(decoded[16]) != AMOUNT or Web3.to_checksum_address(decoded[17]) != RECIPIENT:
        raise RuntimeError("decoded payout does not match the 25 USDC recipient")
    if (
        int(decoded[19]) != CHAIN_ID
        or Web3.to_checksum_address(decoded[20]) != VAULT_ADDRESS
        or Web3.to_checksum_address(decoded[22]) != TOKEN_ADDRESS
    ):
        raise RuntimeError("decoded egress route does not match Ethereum mainnet")

    recipient_before = int(token.functions.balanceOf(RECIPIENT).call())
    vault_before = int(token.functions.balanceOf(VAULT_ADDRESS).call())
    if recipient_before != 0 or vault_before != AMOUNT:
        raise RuntimeError(
            f"unexpected pre-withdraw balances: recipient={recipient_before}, vault={vault_before}"
        )
    if vault.functions.consumedWithdrawalIdCommitment(withdrawal_commitment).call():
        raise RuntimeError("withdrawal commitment is already consumed")
    if vault.functions.consumedBurnTxIdCommitment(burn_commitment).call():
        raise RuntimeError("burn commitment is already consumed")
    if verifier.functions.consumedProofNullifier(proof_nullifier).call():
        raise RuntimeError("proof nullifier is already consumed")
    function = vault.functions.withdrawWithProof(public_values, proof)
    function.call({"from": WALLET})
    gas_estimate = int(function.estimate_gas({"from": WALLET}))

    status = call({"op": "status"})
    if not status or not status.get("ok") or not status.get("unlocked"):
        raise RuntimeError("StakeHub is unavailable or locked")
    session_id = "pfusdc-mainnet-epoch4-egress-20260726"
    call({"op": "close_launch_session", "session_id": session_id})
    opened = call(
        {
            "op": "open_launch_session",
            "session_id": session_id,
            "chain_id": CHAIN_ID,
            "allowlist": [
                WALLET,
                RECIPIENT,
                VAULT_ADDRESS,
                VERIFIER_ADDRESS,
                TOKEN_ADDRESS,
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
            "close_after_action": "withdraw_pfusdc_mainnet",
            "ttl_seconds": 1800,
        }
    )
    if not opened or not opened.get("ok"):
        raise RuntimeError(f"StakeHub launch session rejected: {opened}")
    sent = call(
        {
            "op": "evm_contract_tx",
            "to": VAULT_ADDRESS,
            "data": function._encode_transaction_data(),
            "rpc_url": RPC,
            "chain_id": CHAIN_ID,
            "label": "pfUSDC mainnet 25 USDC proof-native withdrawal",
            "session_id": session_id,
            "session_action": "withdraw_pfusdc_mainnet",
            "value_wei": 0,
            "gas_usd": 0,
        },
        timeout=1800.0,
    )
    tx_hash = (sent or {}).get("tx_hash") or (sent or {}).get("tx")
    if not sent or not sent.get("ok") or not tx_hash:
        raise RuntimeError(f"StakeHub withdrawal failed: {sent}")
    receipt = w3.eth.wait_for_transaction_receipt(tx_hash, timeout=600)
    if receipt.status != 1:
        raise RuntimeError("withdrawal transaction reverted")

    recipient_after = int(token.functions.balanceOf(RECIPIENT).call())
    vault_after = int(token.functions.balanceOf(VAULT_ADDRESS).call())
    if recipient_after - recipient_before != AMOUNT or vault_before - vault_after != AMOUNT:
        raise RuntimeError("withdrawal did not produce exact 25 USDC deltas")
    if vault_after != 0 or recipient_after != AMOUNT:
        raise RuntimeError("terminal vault/recipient balances are not exact")
    if not vault.functions.consumedWithdrawalIdCommitment(withdrawal_commitment).call():
        raise RuntimeError("withdrawal commitment was not consumed")
    if not vault.functions.consumedBurnTxIdCommitment(burn_commitment).call():
        raise RuntimeError("burn commitment was not consumed")
    if not verifier.functions.consumedProofNullifier(proof_nullifier).call():
        raise RuntimeError("proof nullifier was not consumed")
    replay_rejected = False
    try:
        function.call({"from": WALLET})
    except Exception:
        replay_rejected = True
    if not replay_rejected:
        raise RuntimeError("withdrawal proof replay was accepted")

    gas_cost_wei = int(receipt.gasUsed) * int(receipt.effectiveGasPrice)
    result = {
        "schema": "postfiat-pfusdc-mainnet-withdrawal-v1",
        "chain_id": CHAIN_ID,
        "vault": VAULT_ADDRESS,
        "verifier": VERIFIER_ADDRESS,
        "token": TOKEN_ADDRESS,
        "sender": WALLET,
        "recipient": RECIPIENT,
        "amount_atoms": AMOUNT,
        "withdrawal_tx": tx_hash,
        "receipt_block_number": int(receipt.blockNumber),
        "receipt_block_hash": Web3.to_hex(receipt.blockHash),
        "gas_estimate": gas_estimate,
        "gas_used": int(receipt.gasUsed),
        "effective_gas_price": int(receipt.effectiveGasPrice),
        "gas_cost_wei": gas_cost_wei,
        "recipient_balance_before": recipient_before,
        "recipient_balance_after": recipient_after,
        "vault_balance_before": vault_before,
        "vault_balance_after": vault_after,
        "withdrawal_id_commitment": withdrawal_commitment,
        "burn_tx_id_commitment": burn_commitment,
        "proof_nullifier": proof_nullifier,
        "withdrawal_consumed": True,
        "burn_consumed": True,
        "proof_nullifier_consumed": True,
        "replay_rejected": True,
        "proof_report": proof_report,
    }
    write_json(RESULT, result)
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
