#!/usr/bin/env python3
"""Prepare or revoke the wallet's exact Uniswap Permit2 allowance chain."""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
from pathlib import Path

from web3 import Web3


sys.path.insert(
    0,
    os.environ.get("A666_STAKEHUB_REPO", "/home/postfiat/repos/StakeHub-repeat-demo"),
)
from stakehub.agentd import call as agentd_call  # noqa: E402


RPC = os.environ.get("A666_ETHEREUM_RPC", "https://ethereum-rpc.publicnode.com")
WALLET = Web3.to_checksum_address("0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0")
USDC = Web3.to_checksum_address("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
WA666 = Web3.to_checksum_address("0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5")
PERMIT2 = Web3.to_checksum_address("0x000000000022D473030F116dDEE9F6B43aC78BA3")
ROUTER = Web3.to_checksum_address("0x66a9893cC07D91D95644AEDD05D03f95E1dBA8Af")
MAX_UINT256 = (1 << 256) - 1
MAX_UINT160 = (1 << 160) - 1
ERC20_ABI = [
    {
        "type": "function",
        "name": "approve",
        "stateMutability": "nonpayable",
        "inputs": [
            {"name": "spender", "type": "address"},
            {"name": "amount", "type": "uint256"},
        ],
        "outputs": [{"name": "", "type": "bool"}],
    },
    {
        "type": "function",
        "name": "allowance",
        "stateMutability": "view",
        "inputs": [
            {"name": "owner", "type": "address"},
            {"name": "spender", "type": "address"},
        ],
        "outputs": [{"name": "", "type": "uint256"}],
    },
]
PERMIT2_ABI = [
    {
        "type": "function",
        "name": "approve",
        "stateMutability": "nonpayable",
        "inputs": [
            {"name": "token", "type": "address"},
            {"name": "spender", "type": "address"},
            {"name": "amount", "type": "uint160"},
            {"name": "expiration", "type": "uint48"},
        ],
        "outputs": [],
    },
    {
        "type": "function",
        "name": "allowance",
        "stateMutability": "view",
        "inputs": [
            {"name": "user", "type": "address"},
            {"name": "token", "type": "address"},
            {"name": "spender", "type": "address"},
        ],
        "outputs": [
            {"name": "amount", "type": "uint160"},
            {"name": "expiration", "type": "uint48"},
            {"name": "nonce", "type": "uint48"},
        ],
    },
]


def tx_hash(response: dict[str, object]) -> str:
    value = str(response["tx"])
    return value if value.startswith("0x") else "0x" + value


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=["prepare", "revoke"])
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--ttl-seconds", type=int, default=86_400)
    args = parser.parse_args()
    if args.output.exists():
        raise RuntimeError(f"refusing to overwrite {args.output}")
    if args.mode == "prepare" and args.ttl_seconds < 3_600:
        raise RuntimeError("allowance TTL must be at least one hour")

    web3 = Web3(Web3.HTTPProvider(RPC, request_kwargs={"timeout": 120}))
    if not web3.is_connected() or int(web3.eth.chain_id) != 1:
        raise RuntimeError("Ethereum mainnet RPC unavailable or wrong chain")
    permit2 = web3.eth.contract(address=PERMIT2, abi=PERMIT2_ABI)
    tokens = {
        "usdc": web3.eth.contract(address=USDC, abi=ERC20_ABI),
        "wa666": web3.eth.contract(address=WA666, abi=ERC20_ABI),
    }

    def state() -> dict[str, object]:
        result: dict[str, object] = {}
        for name, token in tokens.items():
            permit_allowance = permit2.functions.allowance(
                WALLET, token.address, ROUTER
            ).call()
            result[name] = {
                "erc20_to_permit2": int(
                    token.functions.allowance(WALLET, PERMIT2).call()
                ),
                "permit2_to_router": {
                    "amount": int(permit_allowance[0]),
                    "expiration": int(permit_allowance[1]),
                    "nonce": int(permit_allowance[2]),
                },
            }
        return result

    transactions: list[dict[str, object]] = []

    def send(target: str, calldata: str, label: str) -> None:
        response = agentd_call(
            {
                "op": "evm_contract_tx",
                "to": target,
                "data": calldata,
                "rpc_url": RPC,
                "chain_id": 1,
                "label": label,
                "value_wei": 0,
                "gas_usd": 0,
            },
            timeout=1200,
        )
        if not response or response.get("ok") is not True:
            raise RuntimeError(f"agent rejected {label}: {response}")
        transaction_hash = tx_hash(response)
        receipt = web3.eth.get_transaction_receipt(transaction_hash)
        if int(receipt.status) != 1:
            raise RuntimeError(f"{label} reverted: {transaction_hash}")
        transactions.append(
            {
                "label": label,
                "tx": transaction_hash,
                "block_number": int(receipt.blockNumber),
                "gas_used": int(receipt.gasUsed),
            }
        )

    pre = state()
    erc_amount = MAX_UINT256 if args.mode == "prepare" else 0
    permit_amount = MAX_UINT160 if args.mode == "prepare" else 0
    expiration = int(time.time()) + args.ttl_seconds if args.mode == "prepare" else 0
    for name, token in tokens.items():
        current = state()[name]
        if current["erc20_to_permit2"] != erc_amount:
            send(
                token.address,
                token.encode_abi("approve", args=[PERMIT2, erc_amount]),
                f"{args.mode} {name} ERC20 allowance to Permit2",
            )
        current = state()[name]["permit2_to_router"]
        permit_satisfied = current["amount"] == permit_amount
        if args.mode == "prepare":
            permit_satisfied = permit_satisfied and current["expiration"] > int(time.time()) + 3_600
        if not permit_satisfied:
            send(
                PERMIT2,
                permit2.encode_abi(
                    "approve", args=[token.address, ROUTER, permit_amount, expiration]
                ),
                f"{args.mode} {name} Permit2 allowance to Universal Router",
            )
    post = state()
    for name in tokens:
        if post[name]["erc20_to_permit2"] != erc_amount:
            raise RuntimeError(f"{name} ERC20 allowance postcondition failed")
        permit_state = post[name]["permit2_to_router"]
        if permit_state["amount"] != permit_amount:
            raise RuntimeError(f"{name} Permit2 amount postcondition failed")
        if args.mode == "prepare" and permit_state["expiration"] <= int(time.time()) + 3_600:
            raise RuntimeError(f"{name} Permit2 expiration postcondition failed")

    report = {
        "schema": "postfiat.a666.uniswap_allowance_chain.v1",
        "verdict": "PASS",
        "mode": args.mode,
        "wallet": WALLET,
        "permit2": PERMIT2,
        "universal_router": ROUTER,
        "pre_state": pre,
        "post_state": post,
        "transactions": transactions,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps(report, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
