#!/usr/bin/env python3
"""Production Uniswap V4 swap calldata builder for the wA666/USDC pool.

Builds exact Universal Router execute() calldata for wA666↔USDC swaps through
pool 0xc5f1…16e98. Dry-run simulates via eth_call; --execute refuses unless
--packet-sha256 is present and simulation succeeded. Never hardcodes min-out=0.

Signing path (when --execute): stakehub.agentd evm_contract_tx — the same
sanctioned mechanism as a666-mainnet-seed-pool.py.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import time
from pathlib import Path
from typing import Any

from eth_abi import encode
from eth_utils import function_signature_to_4byte_selector
from web3 import Web3


# ---------------------------------------------------------------------------
# Binding constants
# ---------------------------------------------------------------------------

USDC = Web3.to_checksum_address("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
WA666 = Web3.to_checksum_address("0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5")
POOL_MANAGER = Web3.to_checksum_address("0x000000000004444c5dc75cB358380D2e3dE08A90")
UNIVERSAL_ROUTER = Web3.to_checksum_address("0x66a9893cC07D91D95644AEDD05D03f95E1dBA8Af")
STATE_VIEW = Web3.to_checksum_address("0x7fFE42C4a5DEeA5b0feC41C94C136Cf115597227")
PERMIT2 = Web3.to_checksum_address("0x000000000022D473030F116dDEE9F6B43aC78BA3")
WALLET = Web3.to_checksum_address("0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0")
ZERO_ADDR = "0x0000000000000000000000000000000000000000"

POOL_ID = bytes.fromhex("c5f1e4b5bb07c0718eddcc3d102dc751b8953ec25bb05cdc14d95419d4d16e98")
POOL_KEY = (USDC, WA666, 500, 10, ZERO_ADDR)  # currency0=USDC, currency1=wA666

Q96 = 2**96

ERC20_ABI = [
    {"type": "function", "name": "balanceOf", "stateMutability": "view", "inputs": [{"name": "", "type": "address"}], "outputs": [{"name": "", "type": "uint256"}]},
    {"type": "function", "name": "approve", "stateMutability": "nonpayable", "inputs": [{"name": "spender", "type": "address"}, {"name": "amount", "type": "uint256"}], "outputs": [{"name": "", "type": "bool"}]},
]
PERMIT2_ABI = [
    {"type": "function", "name": "approve", "stateMutability": "nonpayable", "inputs": [{"name": "token", "type": "address"}, {"name": "spender", "type": "address"}, {"name": "amount", "type": "uint160"}, {"name": "expiration", "type": "uint48"}], "outputs": []},
    {"type": "function", "name": "allowance", "stateMutability": "view", "inputs": [{"name": "user", "type": "address"}, {"name": "token", "type": "address"}, {"name": "spender", "type": "address"}], "outputs": [{"name": "amount", "type": "uint160"}, {"name": "expiration", "type": "uint48"}, {"name": "nonce", "type": "uint48"}]},
]
STATE_VIEW_ABI = [
    {"type": "function", "name": "getSlot0", "stateMutability": "view", "inputs": [{"name": "poolId", "type": "bytes32"}], "outputs": [{"name": "sqrtPriceX96", "type": "uint160"}, {"name": "tick", "type": "int24"}, {"name": "protocolFee", "type": "uint24"}, {"name": "lpFee", "type": "uint24"}]},
    {"type": "function", "name": "getLiquidity", "stateMutability": "view", "inputs": [{"name": "poolId", "type": "bytes32"}], "outputs": [{"name": "", "type": "uint128"}]},
]


# ---------------------------------------------------------------------------
# Calldata construction (provenance: a666-mainnet-seed-pool.py lines 214-264)
# ---------------------------------------------------------------------------

def execute_calldata(commands: bytes, inputs: list[bytes], deadline: int) -> str:
    selector = function_signature_to_4byte_selector("execute(bytes,bytes[],uint256)")
    return Web3.to_hex(selector + encode(["bytes", "bytes[]", "uint256"], [commands, inputs, deadline]))


def build_swap_calldata(
    direction: str,
    amount_in_atoms: int,
    min_out_atoms: int,
    deadline_epoch: int,
) -> str:
    """Build Universal Router V4_SWAP calldata.

    Actions: 0x06=SWAP_EXACT_IN_SINGLE, 0x0C=SETTLE_ALL, 0x0F=TAKE_ALL
    Command: 0x10=V4_SWAP
    """
    if direction == "usdc-to-wa666":
        zero_for_one = True   # selling currency0 (USDC) for currency1 (wA666)
        token_in = USDC
        token_out = WA666
    elif direction == "wa666-to-usdc":
        zero_for_one = False  # selling currency1 (wA666) for currency0 (USDC)
        token_in = WA666
        token_out = USDC
    else:
        raise ValueError(f"unknown direction: {direction}")

    swap_params = encode(
        ["((address,address,uint24,int24,address),bool,uint128,uint128,bytes)"],
        [(POOL_KEY, zero_for_one, amount_in_atoms, min_out_atoms, b"")],
    )
    settle_all = encode(["address", "uint128"], [token_in, amount_in_atoms])
    take_all = encode(["address", "uint128"], [token_out, min_out_atoms])
    v4_input = encode(["bytes", "bytes[]"], [bytes([0x06, 0x0C, 0x0F]), [swap_params, settle_all, take_all]])
    return execute_calldata(bytes([0x10]), [v4_input], deadline_epoch)


# ---------------------------------------------------------------------------
# Quote from StateView
# ---------------------------------------------------------------------------

def read_pool_state(web3: Web3) -> dict[str, int]:
    sv = web3.eth.contract(address=STATE_VIEW, abi=STATE_VIEW_ABI)
    slot0 = sv.functions.getSlot0(POOL_ID).call()
    liq = sv.functions.getLiquidity(POOL_ID).call()
    return {"sqrtPriceX96": slot0[0], "tick": slot0[1], "liquidity": liq}


def fair_value_out(direction: str, amount_in: int, sqrt_price_x96: int) -> int:
    """Estimate output atoms from sqrtPriceX96 (ignores liquidity depth / tick)."""
    if direction == "usdc-to-wa666":
        # price = (sqrtP/2^96)^2 = USDC per wA666; wA666_out = USDC_in / price
        price = (sqrt_price_x96 * sqrt_price_x96) // Q96  # USDC atoms per wA666 atom
        if price == 0:
            return 0
        return (amount_in * Q96 * Q96) // (sqrt_price_x96 * sqrt_price_x96)
    else:  # wa666-to-usdc
        return (amount_in * sqrt_price_x96 * sqrt_price_x96) // (Q96 * Q96)


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> int:
    parser = argparse.ArgumentParser(description="wA666/USDC Uniswap V4 swap calldata builder")
    parser.add_argument("--direction", required=True, choices=["wa666-to-usdc", "usdc-to-wa666"])
    parser.add_argument("--amount-in-atoms", type=int, required=True)
    parser.add_argument("--min-out-atoms", type=int, required=True)
    parser.add_argument("--deadline-epoch", type=int, required=True)
    parser.add_argument("--rpc-url", default="https://ethereum-rpc.publicnode.com")
    parser.add_argument("--packet-sha256", default=None, help="required for --execute")
    parser.add_argument("--quote-from-stateview", action="store_true")
    parser.add_argument("--execute", action="store_true", help="broadcast via agentd (REFUSES without --packet-sha256)")
    args = parser.parse_args()

    calldata = build_swap_calldata(args.direction, args.amount_in_atoms, args.min_out_atoms, args.deadline_epoch)
    calldata_hash = hashlib.sha256(bytes.fromhex(calldata[2:])).hexdigest()

    web3 = Web3(Web3.HTTPProvider(args.rpc_url, request_kwargs={"timeout": 120}))
    if not web3.is_connected():
        print(json.dumps({"error": "RPC unavailable"}))
        return 1

    block_number = web3.eth.block_number

    # Quote from StateView if requested
    fair_out = None
    if args.quote_from_stateview:
        pool = read_pool_state(web3)
        fair_out = fair_value_out(args.direction, args.amount_in_atoms, pool["sqrtPriceX96"])
        if fair_out and args.min_out_atoms > fair_out:
            print(json.dumps({
                "error": "min_out exceeds fair value (impossible fill)",
                "min_out_atoms": args.min_out_atoms,
                "fair_value_estimate": fair_out,
                "sqrtPriceX96": pool["sqrtPriceX96"],
            }))
            return 1

    # eth_call simulation
    sim_result = {"status": "pending"}
    try:
        web3.eth.call({"from": WALLET, "to": UNIVERSAL_ROUTER, "data": calldata, "value": 0})
        sim_result = {"status": "success"}
    except Exception as exc:
        sim_result = {"status": "revert", "error": str(exc)[:300]}

    output = {
        "direction": args.direction,
        "amount_in_atoms": args.amount_in_atoms,
        "min_out_atoms": args.min_out_atoms,
        "deadline_epoch": args.deadline_epoch,
        "rpc_url": args.rpc_url,
        "block_number": block_number,
        "to": UNIVERSAL_ROUTER,
        "calldata": calldata,
        "calldata_sha256": calldata_hash,
        "simulation": sim_result,
        "fair_value_estimate": fair_out,
        "packet_sha256": args.packet_sha256,
    }

    if args.execute:
        if not args.packet_sha256:
            print(json.dumps({"error": "--execute requires --packet-sha256"}))
            return 1
        if sim_result["status"] != "success":
            print(json.dumps({"error": "--execute refused: simulation did not succeed", "simulation": sim_result}))
            return 1
        # Sign via agentd evm_contract_tx (same as a666-mainnet-seed-pool.py send())
        sys.path.insert(0, "/home/postfiat/repos/StakeHub")
        from stakehub.agentd import call as agentd_call
        resp = agentd_call({
            "op": "evm_contract_tx",
            "to": UNIVERSAL_ROUTER,
            "data": calldata,
            "rpc_url": args.rpc_url,
            "chain_id": 1,
            "label": f"uniswap-v4-swap-{args.direction}-{args.packet_sha256[:16]}",
            "value_wei": 0,
            "gas_usd": 0,
        }, timeout=1200)
        if not resp or not resp.get("ok"):
            print(json.dumps({"error": "agentd rejected tx", "response": resp}))
            return 1
        tx_hash = resp["tx"] if resp["tx"].startswith("0x") else f"0x{resp['tx']}"
        receipt = web3.eth.get_transaction_receipt(tx_hash)
        output["tx_hash"] = tx_hash
        output["tx_status"] = int(receipt.status)
        output["gas_used"] = int(receipt.gasUsed)

    print(json.dumps(output, indent=2, sort_keys=True))
    return 0 if sim_result["status"] == "success" else 1


if __name__ == "__main__":
    sys.exit(main())
