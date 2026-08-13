#!/usr/bin/env python3
"""Withdraw an exact proof-native pfUSDC redemption from a frozen vault lane."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import sys
from pathlib import Path
from typing import Any

from web3 import Web3
from web3.logs import DISCARD


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
        "--ethereum-rpc",
        default="https://ethereum-rpc.publicnode.com",
    )
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
    parser.add_argument(
        "--stakehub-home",
        type=Path,
        default=Path("~/.stakehub").expanduser(),
    )
    return parser.parse_args()


def validate_recovery_event(
    event: dict[str, Any],
    *,
    withdrawal_commitment: str,
    burn_commitment: str,
    recipient: str,
    amount_atoms: int,
) -> None:
    args = event.get("args") or {}
    if Web3.to_hex(args.get("withdrawalIdCommitment", b"")).lower() != withdrawal_commitment.lower():
        raise RuntimeError("recovered payout has the wrong withdrawal commitment")
    if Web3.to_hex(args.get("burnTxIdCommitment", b"")).lower() != burn_commitment.lower():
        raise RuntimeError("recovered payout has the wrong burn commitment")
    try:
        event_recipient = Web3.to_checksum_address(args.get("recipient"))
    except (TypeError, ValueError) as failure:
        raise RuntimeError("recovered payout has an invalid recipient") from failure
    if event_recipient != Web3.to_checksum_address(recipient):
        raise RuntimeError("recovered payout has the wrong recipient")
    if int(args.get("amount", -1)) != amount_atoms:
        raise RuntimeError("recovered payout has the wrong amount")


def find_withdrawal_logs(
    w3: Web3,
    vault: Any,
    withdrawal_commitment: str,
    burn_commitment: str,
    start_block: int,
    journal_path: Path,
) -> list[Any]:
    event = vault.events.ProofNativeWithdrawal()
    event_topic = Web3.keccak(text="ProofNativeWithdrawal(bytes32,bytes32,bytes32,address,uint256)").hex()
    query = {
        "address": vault.address,
        "topics": [event_topic, withdrawal_commitment, burn_commitment],
    }
    if journal_path.is_file():
        candidates: list[str] = []
        for line in journal_path.read_text().splitlines():
            try:
                row = json.loads(line)
            except json.JSONDecodeError:
                continue
            if (
                row.get("type") == "agent_evm_contract_tx_broadcast"
                and str(row.get("to", "")).lower() == vault.address.lower()
                and int(row.get("chain_id") or 0) == 1
                and row.get("session_action") == "withdraw_pfusdc_mainnet"
                and isinstance(row.get("tx"), str)
            ):
                candidates.append(row["tx"])
        matches: list[Any] = []
        for tx_hash in reversed(list(dict.fromkeys(candidates))):
            try:
                receipt = w3.eth.get_transaction_receipt(tx_hash)
                if int(receipt.status) != 1 or int(receipt.blockNumber) < start_block:
                    continue
                for decoded in event.process_receipt(receipt, errors=DISCARD):
                    args = decoded.get("args") or {}
                    if (
                        Web3.to_hex(args.get("withdrawalIdCommitment", b"")).lower() == withdrawal_commitment.lower()
                        and Web3.to_hex(args.get("burnTxIdCommitment", b"")).lower() == burn_commitment.lower()
                    ):
                        matches.append(decoded)
            except Exception:
                continue
        if matches:
            return matches
    try:
        return [event.process_log(log) for log in w3.eth.get_logs({**query, "fromBlock": start_block, "toBlock": "latest"})]
    except (ValueError, OSError):
        latest = int(w3.eth.block_number)
        matches: list[Any] = []
        chunk = 2_000
        while latest >= start_block:
            first = max(start_block, latest - chunk + 1)
            logs = w3.eth.get_logs({**query, "fromBlock": first, "toBlock": latest})
            matches.extend(event.process_log(log) for log in logs)
            if matches:
                break
            latest = first - 1
        return matches


def recover_completed_withdrawal(sender: Any) -> bool:
    vault_artifact = json.loads(
        (sender.REPO / "crates/ethereum-contracts/out/ERC20BridgeVaultV2.sol/ERC20BridgeVaultV2.json").read_text()
    )
    verifier_artifact = json.loads(
        (sender.REPO / "crates/ethereum-contracts/out/PFTLFinalityVerifierV1.sol/PFTLFinalityVerifierV1.json").read_text()
    )
    proof_report = json.loads((sender.PROOF_DIR / "proof-report.json").read_text())
    if proof_report.get("proof_mode") != "groth16" or proof_report.get("program_vkey") != sender.PROGRAM_VKEY:
        raise RuntimeError("egress proof mode or program key does not match the active route")
    public_values = (sender.PROOF_DIR / "public-values.bin").read_bytes()
    proof = (sender.PROOF_DIR / "proof-calldata.bin").read_bytes()
    w3 = Web3(Web3.HTTPProvider(sender.RPC, request_kwargs={"timeout": 60}))
    if w3.eth.chain_id != sender.CHAIN_ID:
        raise RuntimeError(f"wrong chain: {w3.eth.chain_id}")
    vault = w3.eth.contract(address=sender.VAULT_ADDRESS, abi=vault_artifact["abi"])
    verifier = w3.eth.contract(address=sender.VERIFIER_ADDRESS, abi=verifier_artifact["abi"])
    token = w3.eth.contract(
        address=sender.TOKEN_ADDRESS,
        abi=[{
            "type": "function",
            "name": "balanceOf",
            "stateMutability": "view",
            "inputs": [{"name": "account", "type": "address"}],
            "outputs": [{"name": "", "type": "uint256"}],
        }],
    )
    if Web3.to_hex(verifier.functions.programVKey().call()).lower() != sender.PROGRAM_VKEY:
        raise RuntimeError("deployed finality verifier program key changed")
    decoded = verifier.functions.decodePublicValues(public_values).call()
    burn_commitment = Web3.to_hex(decoded[14])
    withdrawal_commitment = Web3.to_hex(decoded[15])
    proof_nullifier = Web3.to_hex(decoded[26])
    if int(decoded[16]) != sender.AMOUNT or Web3.to_checksum_address(decoded[17]) != sender.RECIPIENT:
        raise RuntimeError("decoded payout does not match the requested amount and recipient")
    if (
        int(decoded[19]) != sender.CHAIN_ID
        or Web3.to_checksum_address(decoded[20]) != sender.VAULT_ADDRESS
        or Web3.to_checksum_address(decoded[22]) != sender.TOKEN_ADDRESS
    ):
        raise RuntimeError("decoded egress route does not match Ethereum mainnet")

    consumed = (
        bool(vault.functions.consumedWithdrawalIdCommitment(withdrawal_commitment).call()),
        bool(vault.functions.consumedBurnTxIdCommitment(burn_commitment).call()),
        bool(verifier.functions.consumedProofNullifier(proof_nullifier).call()),
    )
    if consumed == (False, False, False):
        return False
    if consumed != (True, True, True):
        raise RuntimeError("withdrawal commitments are in an inconsistent partially consumed state")

    deployment_receipt = w3.eth.get_transaction_receipt(sender.DEPLOYMENT_TX)
    if (
        int(deployment_receipt.status) != 1
        or Web3.to_checksum_address(deployment_receipt.contractAddress) != sender.VAULT_ADDRESS
    ):
        raise RuntimeError("vault deployment transaction does not bind the active vault")
    events = find_withdrawal_logs(
        w3,
        vault,
        withdrawal_commitment,
        burn_commitment,
        int(deployment_receipt.blockNumber),
        sender.STAKEHUB_JOURNAL,
    )
    if len(events) != 1:
        raise RuntimeError(f"expected one matching on-chain payout event, found {len(events)}")
    recovered = events[0]
    validate_recovery_event(
        recovered,
        withdrawal_commitment=withdrawal_commitment,
        burn_commitment=burn_commitment,
        recipient=sender.RECIPIENT,
        amount_atoms=sender.AMOUNT,
    )
    receipt = w3.eth.get_transaction_receipt(recovered["transactionHash"])
    if int(receipt.status) != 1:
        raise RuntimeError("recovered payout transaction reverted")
    block_number = int(receipt.blockNumber)
    if block_number <= 0:
        raise RuntimeError("recovered payout has an invalid block number")
    recipient_before = int(token.functions.balanceOf(sender.RECIPIENT).call(block_identifier=block_number - 1))
    recipient_after = int(token.functions.balanceOf(sender.RECIPIENT).call(block_identifier=block_number))
    vault_before = int(token.functions.balanceOf(sender.VAULT_ADDRESS).call(block_identifier=block_number - 1))
    vault_after = int(token.functions.balanceOf(sender.VAULT_ADDRESS).call(block_identifier=block_number))
    function = vault.functions.withdrawWithProof(public_values, proof)
    replay_rejected = False
    try:
        function.call({"from": sender.WALLET})
    except Exception:
        replay_rejected = True
    if not replay_rejected:
        raise RuntimeError("withdrawal proof replay was accepted")
    transaction_hash = Web3.to_hex(receipt.transactionHash)
    result = {
        "schema": "postfiat-pfusdc-mainnet-withdrawal-v1",
        "chain_id": sender.CHAIN_ID,
        "vault": sender.VAULT_ADDRESS,
        "verifier": sender.VERIFIER_ADDRESS,
        "token": sender.TOKEN_ADDRESS,
        "sender": sender.WALLET,
        "recipient": sender.RECIPIENT,
        "amount_atoms": sender.AMOUNT,
        "withdrawal_tx": transaction_hash,
        "receipt_block_number": block_number,
        "receipt_block_hash": Web3.to_hex(receipt.blockHash),
        "gas_estimate": None,
        "gas_used": int(receipt.gasUsed),
        "effective_gas_price": int(receipt.effectiveGasPrice),
        "gas_cost_wei": int(receipt.gasUsed) * int(receipt.effectiveGasPrice),
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
        "recovered_from_chain": True,
        "recovery_event_validated": True,
        "proof_report": proof_report,
    }
    sender.write_json(sender.RESULT, result)
    print(json.dumps(result, indent=2, sort_keys=True))
    return True


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
    sender.RPC = args.ethereum_rpc
    sender.STAKEHUB_JOURNAL = args.stakehub_home.expanduser().resolve() / "journal.jsonl"
    sender.AMOUNT = args.amount_atoms
    sender.RECIPIENT = Web3.to_checksum_address(args.recipient)
    sender.VAULT_ADDRESS = Web3.to_checksum_address(route["vault_address"])
    deployments = [
        row for row in manifest.get("deployments", [])
        if str(row.get("contract_address", "")).lower() == sender.VAULT_ADDRESS.lower()
        and row.get("ok") is True
        and isinstance(row.get("tx"), str)
    ]
    if len(deployments) != 1:
        raise RuntimeError("deployment manifest does not bind one successful vault deployment")
    sender.DEPLOYMENT_TX = deployments[0]["tx"]
    sender.VERIFIER_ADDRESS = Web3.to_checksum_address(route["verifier_address"])
    sender.TOKEN_ADDRESS = Web3.to_checksum_address(network["token"]["address"])
    sender.PROGRAM_VKEY = programs["egress"]["program_vkey"]
    if not recover_completed_withdrawal(sender):
        sender.main()


if __name__ == "__main__":
    main()
