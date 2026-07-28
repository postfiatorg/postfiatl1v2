#!/usr/bin/env python3
"""Burn exact mainnet wA666 for a proof-gated return to PFTL."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import sys
from typing import Any

from web3 import Web3


ROOT = Path(__file__).resolve().parents[1]
STAKEHUB = Path("/home/postfiat/repos/StakeHub")
RPC = "https://ethereum-rpc.publicnode.com"
CHAIN_ID = 1
OWNER = Web3.to_checksum_address("0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0")
CONTROLLER = Web3.to_checksum_address("0x9A0262C0572fb4DB08765408eB225E207F40c3d9")
TOKEN = Web3.to_checksum_address("0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5")
PFTL_RECIPIENT = "pfab9b9228942e5c529633a13aa271d5297bec6353"
A666 = (
    "521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452"
    "da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c"
)


def artifact(source: str, contract: str) -> dict[str, Any]:
    path = ROOT / f"crates/ethereum-contracts/out/{source}/{contract}.json"
    return json.loads(path.read_text())


def normalize_tx_hash(value: str) -> str:
    return value if value.startswith("0x") else f"0x{value}"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--transaction-hash")
    parser.add_argument("--amount-atoms", type=int, default=1_000_000)
    parser.add_argument("--return-nonce", required=True)
    args = parser.parse_args()
    if args.execute and args.transaction_hash:
        raise RuntimeError("--execute and --transaction-hash are mutually exclusive")
    nonce = bytes.fromhex(args.return_nonce.removeprefix("0x"))
    if len(nonce) != 32 or nonce == bytes(32):
        raise RuntimeError("--return-nonce must be one nonzero bytes32")
    if args.amount_atoms <= 0:
        raise RuntimeError("--amount-atoms must be positive")

    web3 = Web3(Web3.HTTPProvider(RPC, request_kwargs={"timeout": 120}))
    if not web3.is_connected() or int(web3.eth.chain_id) != CHAIN_ID:
        raise RuntimeError("Ethereum mainnet RPC unavailable or wrong chain")
    controller = web3.eth.contract(
        address=CONTROLLER,
        abi=artifact(
            "PFTLUniswapPrimaryMarketV2.sol", "PFTLUniswapPrimaryMarketV2"
        )["abi"],
    )
    token = web3.eth.contract(
        address=TOKEN,
        abi=artifact(
            "PFTLUniswapHandoffController.sol", "WrappedVenueNAVCoin"
        )["abi"],
    )
    call = controller.functions.burnForPftlReturn(
        args.amount_atoms,
        PFTL_RECIPIENT,
        bytes.fromhex(A666),
        nonce,
    )
    def state_at(block_identifier: int | str = "latest") -> dict[str, Any]:
        return {
            "recipient_balance_atoms": int(
                token.functions.balanceOf(OWNER).call(
                    block_identifier=block_identifier
                )
            ),
            "token_total_supply": int(
                token.functions.totalSupply().call(block_identifier=block_identifier)
            ),
            "total_return_burned_atoms": int(
                controller.functions.totalReturnBurnedAtoms().call(
                    block_identifier=block_identifier
                )
            ),
            "nonce_consumed": bool(
                controller.functions.consumedReturnNonce(nonce).call(
                    block_identifier=block_identifier
                )
            ),
        }

    pre = state_at()
    gas_estimate = None
    if not args.transaction_hash:
        gas_estimate = int(
            web3.eth.estimate_gas(
                {
                    "from": OWNER,
                    "to": CONTROLLER,
                    "data": call._encode_transaction_data(),
                    "value": 0,
                }
            )
        )
    report: dict[str, Any] = {
        "schema": "postfiat-a666-mainnet-return-burn-v1",
        "phase": "prepared",
        "chain_id": CHAIN_ID,
        "rpc": RPC,
        "controller": CONTROLLER,
        "wrapped_token": TOKEN,
        "ethereum_sender": OWNER,
        "pftl_recipient": PFTL_RECIPIENT,
        "native_nav_asset_id": A666,
        "amount_atoms": args.amount_atoms,
        "return_nonce": nonce.hex(),
        "gas_estimate": gas_estimate,
        "pre_state": pre,
    }
    if not args.execute and not args.transaction_hash:
        print(json.dumps(report, indent=2, sort_keys=True))
        return
    if args.transaction_hash:
        transaction_hash = normalize_tx_hash(args.transaction_hash)
    else:
        if pre["nonce_consumed"]:
            raise RuntimeError("return nonce is already consumed")
        if pre["recipient_balance_atoms"] < args.amount_atoms:
            raise RuntimeError("insufficient wA666 balance")

        sys.path.insert(0, str(STAKEHUB))
        from stakehub.agentd import call as agent_call

        response = agent_call(
            {
                "op": "evm_contract_tx",
                "to": CONTROLLER,
                "data": call._encode_transaction_data(),
                "rpc_url": RPC,
                "chain_id": CHAIN_ID,
                "label": "burn proof-minted wA666 for PFTL return",
                "value_wei": 0,
                "gas_usd": 10,
            },
            timeout=1200.0,
        )
        if not response or not response.get("ok"):
            raise RuntimeError(f"StakeHub rejected return burn: {response}")
        transaction_hash = normalize_tx_hash(response["tx"])
    receipt = web3.eth.get_transaction_receipt(transaction_hash)
    if int(receipt.status) != 1:
        raise RuntimeError(f"return burn reverted: {transaction_hash}")
    if args.transaction_hash:
        pre = state_at(int(receipt.blockNumber) - 1)
        report["pre_state"] = pre
    events = controller.events.ReturnBurned().process_receipt(
        receipt, errors=__import__("web3").logs.DISCARD
    )
    if len(events) != 1:
        raise RuntimeError(f"expected one ReturnBurned event, got {len(events)}")
    event = events[0]["args"]
    block_log_index = int(events[0]["logIndex"])
    receipt_log_index = next(
        (
            index
            for index, receipt_log in enumerate(receipt["logs"])
            if int(receipt_log["logIndex"]) == block_log_index
        ),
        None,
    )
    if receipt_log_index is None:
        raise RuntimeError("ReturnBurned event is absent from its transaction receipt logs")
    block_number = int(receipt.blockNumber)
    canonical_preimage = Web3().codec.encode(
        [
            "string",
            "uint256",
            "address",
            "address",
            "bytes",
            "address",
            "string",
            "uint256",
            "bytes32",
            "uint256",
        ],
        [
            "postfiat.pftl_uniswap.return_burn.v1",
            CHAIN_ID,
            CONTROLLER,
            TOKEN,
            bytes.fromhex(A666),
            OWNER,
            PFTL_RECIPIENT,
            args.amount_atoms,
            nonce,
            block_number,
        ],
    )
    expected_burn_id = Web3.keccak(canonical_preimage)
    if (
        event["returnBurnId"] != expected_burn_id
        or Web3.to_checksum_address(event["ethereumSender"]) != OWNER
        or event["returnNonce"] != nonce
        or event["pftlRecipient"] != PFTL_RECIPIENT
        or int(event["amountAtoms"]) != args.amount_atoms
    ):
        raise RuntimeError("ReturnBurned event does not match canonical burn preimage")
    post = state_at(block_number)
    if (
        pre["recipient_balance_atoms"] - post["recipient_balance_atoms"]
        != args.amount_atoms
        or pre["token_total_supply"] - post["token_total_supply"]
        != args.amount_atoms
        or post["total_return_burned_atoms"] - pre["total_return_burned_atoms"]
        != args.amount_atoms
        or not post["nonce_consumed"]
    ):
        raise RuntimeError("return burn post-state mismatch")
    report.update(
        {
            "phase": "burned",
            "return_burn_id": expected_burn_id.hex(),
            "transaction": {
                "tx": transaction_hash,
                "block_number": block_number,
                "block_hash": receipt.blockHash.hex(),
                "transaction_index": int(receipt.transactionIndex),
                "gas_used": int(receipt.gasUsed),
                "status": int(receipt.status),
            },
            # Ethereum receipt-trie proofs index the log within this receipt,
            # not within the containing block.
            "event_log_index": receipt_log_index,
            "block_log_index": block_log_index,
            "receipt_sha256": hashlib.sha256(
                web3.to_json(receipt).encode()
            ).hexdigest(),
            "post_state": post,
        }
    )
    print(json.dumps(report, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
