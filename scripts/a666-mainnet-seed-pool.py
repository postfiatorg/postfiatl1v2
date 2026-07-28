#!/usr/bin/env python3
"""Migrate legacy A651, seed the initialized wA666/USDC v4 pool, and smoke-swap."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import sys
import tempfile
import time
from typing import Any

from eth_abi import decode, encode
from eth_utils import function_signature_to_4byte_selector
from web3 import Web3


ROOT = Path(__file__).resolve().parents[1]
STAKEHUB = Path("/home/postfiat/repos/StakeHub")
RPC = "https://ethereum-rpc.publicnode.com"
OWNER = Web3.to_checksum_address("0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0")
USDC = Web3.to_checksum_address("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
A651 = Web3.to_checksum_address("0x1e55EDa7ce0788E8b624456C4d401A33bD83b62e")
WA666 = Web3.to_checksum_address("0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5")
MIGRATION = Web3.to_checksum_address("0xFfBBae0eb8450D10F8C0D26C2952b089D56e517c")
PERMIT2 = Web3.to_checksum_address("0x000000000022D473030F116dDEE9F6B43aC78BA3")
POSITION_MANAGER = Web3.to_checksum_address("0xbD216513d74C8cf14cf4747E6AaA6420FF64ee9e")
UNIVERSAL_ROUTER = Web3.to_checksum_address("0x66a9893cC07D91D95644AEDD05D03f95E1dBA8Af")
STATE_VIEW = Web3.to_checksum_address("0x7fFE42C4a5DEeA5b0feC41C94C136Cf115597227")
POOL_ID = bytes.fromhex("c5f1e4b5bb07c0718eddcc3d102dc751b8953ec25bb05cdc14d95419d4d16e98")
POOL_KEY = (USDC, WA666, 500, 10, "0x0000000000000000000000000000000000000000")
SEED_ATOMS = 3_000 * 10**6
SMOKE_INPUT_ATOMS = 10 * 10**6
SMOKE_MIN_OUTPUT_ATOMS = 7_000_000
A651_TO_BURN = 382_333_668_078_301_459_218
STATE_PATH = ROOT / "deployments/a666-mainnet-20260727/ethereum/pool-seed-state.json"
EXPECTED_CODE_HASHES = {
    PERMIT2: "0xc67d1657868aa5146eaf24fb879fb1fdec3d2d493b3683a61c9c2f4fb2851131",
    POSITION_MANAGER: "0x77e36c08b19959a30dde46dec9abe6208e371ff2f56884a56fe1e1a53615528b",
    UNIVERSAL_ROUTER: "0x6a5f46971b50c6e1b7eef97902311444e479d734e4f80ad88367783cf373fe7f",
    STATE_VIEW: "0xd7947778589cf4aac9a092a4451292a2056380941635ab7006d3c691d8dfd878",
}

ERC20_ABI = [
    {"type": "function", "name": "balanceOf", "stateMutability": "view", "inputs": [{"name": "", "type": "address"}], "outputs": [{"name": "", "type": "uint256"}]},
    {"type": "function", "name": "totalSupply", "stateMutability": "view", "inputs": [], "outputs": [{"name": "", "type": "uint256"}]},
    {"type": "function", "name": "allowance", "stateMutability": "view", "inputs": [{"name": "", "type": "address"}, {"name": "", "type": "address"}], "outputs": [{"name": "", "type": "uint256"}]},
    {"type": "function", "name": "approve", "stateMutability": "nonpayable", "inputs": [{"name": "spender", "type": "address"}, {"name": "amount", "type": "uint256"}], "outputs": [{"name": "", "type": "bool"}]},
]
PERMIT2_ABI = [
    {"type": "function", "name": "approve", "stateMutability": "nonpayable", "inputs": [{"name": "token", "type": "address"}, {"name": "spender", "type": "address"}, {"name": "amount", "type": "uint160"}, {"name": "expiration", "type": "uint48"}], "outputs": []},
    {"type": "function", "name": "allowance", "stateMutability": "view", "inputs": [{"name": "user", "type": "address"}, {"name": "token", "type": "address"}, {"name": "spender", "type": "address"}], "outputs": [{"name": "amount", "type": "uint160"}, {"name": "expiration", "type": "uint48"}, {"name": "nonce", "type": "uint48"}]},
]
POSITION_MANAGER_ABI = [
    {"type": "function", "name": "modifyLiquidities", "stateMutability": "payable", "inputs": [{"name": "unlockData", "type": "bytes"}, {"name": "deadline", "type": "uint256"}], "outputs": []},
    {"type": "function", "name": "balanceOf", "stateMutability": "view", "inputs": [{"name": "owner", "type": "address"}], "outputs": [{"name": "", "type": "uint256"}]},
]
STATE_VIEW_ABI = [
    {"type": "function", "name": "getLiquidity", "stateMutability": "view", "inputs": [{"name": "poolId", "type": "bytes32"}], "outputs": [{"name": "", "type": "uint128"}]},
    {"type": "function", "name": "getSlot0", "stateMutability": "view", "inputs": [{"name": "poolId", "type": "bytes32"}], "outputs": [{"name": "sqrtPriceX96", "type": "uint160"}, {"name": "tick", "type": "int24"}, {"name": "protocolFee", "type": "uint24"}, {"name": "lpFee", "type": "uint24"}]},
]
MIGRATION_ABI = [
    {"type": "function", "name": "quote", "stateMutability": "view", "inputs": [{"name": "a651Amount", "type": "uint256"}], "outputs": [{"name": "", "type": "uint256"}]},
    {"type": "function", "name": "migrate", "stateMutability": "nonpayable", "inputs": [{"name": "a651Amount", "type": "uint256"}, {"name": "recipient", "type": "address"}], "outputs": [{"name": "", "type": "uint256"}]},
    {"type": "function", "name": "remainingA666Reserve", "stateMutability": "view", "inputs": [], "outputs": [{"name": "", "type": "uint256"}]},
    {"type": "function", "name": "totalA651Burned", "stateMutability": "view", "inputs": [], "outputs": [{"name": "", "type": "uint256"}]},
    {"type": "function", "name": "totalA666Released", "stateMutability": "view", "inputs": [], "outputs": [{"name": "", "type": "uint256"}]},
]


def atomic_write(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(fd, "w") as handle:
            json.dump(value, handle, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(name, path)
    finally:
        Path(name).unlink(missing_ok=True)


def send(web3: Web3, to: str, data: str, label: str) -> dict[str, Any]:
    gas_estimate = int(web3.eth.estimate_gas({"from": OWNER, "to": to, "data": data, "value": 0}))
    sys.path.insert(0, str(STAKEHUB))
    from stakehub.agentd import call
    response = call({"op": "evm_contract_tx", "to": to, "data": data, "rpc_url": RPC, "chain_id": 1, "label": label, "value_wei": 0, "gas_usd": 0}, timeout=1200)
    if not response or not response.get("ok"):
        raise RuntimeError(f"{label} rejected: {response}")
    tx = response["tx"] if response["tx"].startswith("0x") else f"0x{response['tx']}"
    receipt = web3.eth.get_transaction_receipt(tx)
    if int(receipt.status) != 1:
        raise RuntimeError(f"{label} reverted: {tx}")
    return {"label": label, "tx": tx, "block_number": int(receipt.blockNumber), "gas_estimate": gas_estimate, "gas_used": int(receipt.gasUsed)}


def execute_calldata(commands: bytes, inputs: list[bytes], deadline: int) -> str:
    selector = function_signature_to_4byte_selector("execute(bytes,bytes[],uint256)")
    return Web3.to_hex(selector + encode(["bytes", "bytes[]", "uint256"], [commands, inputs, deadline]))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--cleanup-only", action="store_true")
    parser.add_argument("--state-file", type=Path, default=STATE_PATH)
    args = parser.parse_args()
    web3 = Web3(Web3.HTTPProvider(RPC, request_kwargs={"timeout": 120}))
    if not web3.is_connected() or int(web3.eth.chain_id) != 1:
        raise RuntimeError("Ethereum mainnet RPC unavailable or wrong chain")
    for address, expected in EXPECTED_CODE_HASHES.items():
        actual = Web3.to_hex(Web3.keccak(bytes(web3.eth.get_code(address))))
        if actual != expected:
            raise RuntimeError(f"runtime code hash drift at {address}: {actual}")

    usdc = web3.eth.contract(address=USDC, abi=ERC20_ABI)
    a651 = web3.eth.contract(address=A651, abi=ERC20_ABI)
    wa666 = web3.eth.contract(address=WA666, abi=ERC20_ABI)
    migration = web3.eth.contract(address=MIGRATION, abi=MIGRATION_ABI)
    permit2 = web3.eth.contract(address=PERMIT2, abi=PERMIT2_ABI)
    positions = web3.eth.contract(address=POSITION_MANAGER, abi=POSITION_MANAGER_ABI)
    state_view = web3.eth.contract(address=STATE_VIEW, abi=STATE_VIEW_ABI)
    if args.cleanup_only:
        prior = json.loads(args.state_file.read_text()) if args.state_file.exists() else {}
        cleanup_transactions = prior.get("transactions", [])
        if int(permit2.functions.allowance(OWNER, USDC, UNIVERSAL_ROUTER).call()[0]) != 0:
            cleanup_transactions.append(send(web3, PERMIT2, permit2.functions.approve(USDC, UNIVERSAL_ROUTER, 0, 0)._encode_transaction_data(), "revoke unused Universal Router Permit2 allowance"))
        if int(usdc.functions.allowance(OWNER, PERMIT2).call()) != 0:
            cleanup_transactions.append(send(web3, USDC, usdc.functions.approve(PERMIT2, 0)._encode_transaction_data(), "revoke unused smoke-swap ERC20 allowance"))
        swap_signature = Web3.keccak(text="Swap(bytes32,address,int128,int128,uint160,uint128,int24,uint24)")
        logs = web3.eth.get_logs({
            "fromBlock": 25_627_679,
            "toBlock": "latest",
            "address": Web3.to_checksum_address("0x000000000004444c5dc75cB358380D2e3dE08A90"),
            "topics": [swap_signature, Web3.to_hex(POOL_ID)],
        })
        swaps = []
        for log in logs:
            amount0, amount1, sqrt_price_x96, liquidity, tick, fee = decode(
                ["int128", "int128", "uint160", "uint128", "int24", "uint24"],
                bytes(log["data"]),
            )
            swaps.append({
                "block_number": int(log["blockNumber"]),
                "tx": log["transactionHash"].hex(),
                "sender": Web3.to_checksum_address("0x" + log["topics"][2].hex()[-40:]),
                "amount0_usdc_atoms": amount0,
                "amount1_wa666_atoms": amount1,
                "sqrt_price_x96_after": sqrt_price_x96,
                "liquidity_after": liquidity,
                "tick_after": tick,
                "fee_pips": fee,
            })
        prior.update({
            "phase": "seeded-external-trading-observed",
            "transactions": cleanup_transactions,
            "external_swaps": swaps,
            "post_state": {
                "pool_liquidity": int(state_view.functions.getLiquidity(POOL_ID).call()),
                "pool_slot0": list(state_view.functions.getSlot0(POOL_ID).call()),
                "wa666_total_supply_atoms": int(wa666.functions.totalSupply().call()),
                "migration_reserve_atoms": int(migration.functions.remainingA666Reserve().call()),
                "owner_usdc_atoms": int(usdc.functions.balanceOf(OWNER).call()),
                "owner_wa666_atoms": int(wa666.functions.balanceOf(OWNER).call()),
                "position_manager_usdc_permit2_atoms": int(permit2.functions.allowance(OWNER, USDC, POSITION_MANAGER).call()[0]),
                "position_manager_wa666_permit2_atoms": int(permit2.functions.allowance(OWNER, WA666, POSITION_MANAGER).call()[0]),
                "router_usdc_permit2_atoms": int(permit2.functions.allowance(OWNER, USDC, UNIVERSAL_ROUTER).call()[0]),
                "usdc_erc20_permit2_atoms": int(usdc.functions.allowance(OWNER, PERMIT2).call()),
                "wa666_erc20_permit2_atoms": int(wa666.functions.allowance(OWNER, PERMIT2).call()),
            },
        })
        atomic_write(args.state_file, prior)
        print(json.dumps(prior, indent=2, sort_keys=True))
        return
    if int(migration.functions.quote(A651_TO_BURN).call()) != SEED_ATOMS:
        raise RuntimeError("migration quote does not equal exact 3,000 wA666")

    def snapshot() -> dict[str, Any]:
        return {
            "a651_owner_atoms": int(a651.functions.balanceOf(OWNER).call()),
            "usdc_owner_atoms": int(usdc.functions.balanceOf(OWNER).call()),
            "wa666_owner_atoms": int(wa666.functions.balanceOf(OWNER).call()),
            "wa666_total_supply_atoms": int(wa666.functions.totalSupply().call()),
            "migration_reserve_atoms": int(migration.functions.remainingA666Reserve().call()),
            "migration_a651_burned_atoms": int(migration.functions.totalA651Burned().call()),
            "migration_a666_released_atoms": int(migration.functions.totalA666Released().call()),
            "pool_liquidity": int(state_view.functions.getLiquidity(POOL_ID).call()),
            "pool_slot0": list(state_view.functions.getSlot0(POOL_ID).call()),
            "position_nft_balance": int(positions.functions.balanceOf(OWNER).call()),
        }

    previous_transactions: list[dict[str, Any]] = []
    if args.state_file.exists():
        previous_transactions = json.loads(args.state_file.read_text()).get("transactions", [])
    state: dict[str, Any] = {"schema": "postfiat-a666-mainnet-pool-seed-v1", "phase": "prepared", "seed_atoms_each": SEED_ATOMS, "a651_burn_atoms": A651_TO_BURN, "smoke_input_usdc_atoms": SMOKE_INPUT_ATOMS, "pre_state": snapshot(), "transactions": previous_transactions}
    atomic_write(args.state_file, state)
    if not args.execute:
        print(json.dumps(state, indent=2, sort_keys=True))
        return

    txs = state["transactions"]
    if state["pre_state"]["migration_a666_released_atoms"] < SEED_ATOMS:
        txs.append(send(web3, MIGRATION, migration.functions.migrate(A651_TO_BURN, OWNER)._encode_transaction_data(), "retire A651 for exactly 3000 wA666"))
        atomic_write(args.state_file, state)

    if state["pre_state"]["pool_liquidity"] == 0:
        expiry = int(time.time()) + 3600
        for token_contract, token_address, symbol in ((usdc, USDC, "USDC"), (wa666, WA666, "wA666")):
            txs.append(send(web3, token_address, token_contract.functions.approve(PERMIT2, SEED_ATOMS)._encode_transaction_data(), f"approve Permit2 for A666 pool {symbol}"))
            atomic_write(args.state_file, state)
            txs.append(send(web3, PERMIT2, permit2.functions.approve(token_address, POSITION_MANAGER, SEED_ATOMS, expiry)._encode_transaction_data(), f"approve PositionManager through Permit2 for {symbol}"))
            atomic_write(args.state_file, state)

        mint_position = encode(
            ["(address,address,uint24,int24,address)", "int24", "int24", "uint256", "uint128", "uint128", "address", "bytes"],
            [POOL_KEY, -887270, 887270, SEED_ATOMS, SEED_ATOMS, SEED_ATOMS, OWNER, b""],
        )
        settle_pair = encode(["address", "address"], [USDC, WA666])
        unlock_data = encode(["bytes", "bytes[]"], [bytes([0x02, 0x0D]), [mint_position, settle_pair]])
        deadline = int(time.time()) + 600
        txs.append(send(web3, POSITION_MANAGER, positions.functions.modifyLiquidities(unlock_data, deadline)._encode_transaction_data(), "seed A666 Uniswap v4 pool with 3000 USDC and 3000 wA666"))
        atomic_write(args.state_file, state)

        for token_contract, token_address, symbol in ((usdc, USDC, "USDC"), (wa666, WA666, "wA666")):
            txs.append(send(web3, PERMIT2, permit2.functions.approve(token_address, POSITION_MANAGER, 0, 0)._encode_transaction_data(), f"revoke PositionManager Permit2 allowance for {symbol}"))
            atomic_write(args.state_file, state)
            txs.append(send(web3, token_address, token_contract.functions.approve(PERMIT2, 0)._encode_transaction_data(), f"revoke Permit2 ERC20 allowance for {symbol}"))
            atomic_write(args.state_file, state)

    before_smoke = snapshot()
    smoke_expiry = int(time.time()) + 1200
    if int(usdc.functions.allowance(OWNER, PERMIT2).call()) < SMOKE_INPUT_ATOMS:
        txs.append(send(web3, USDC, usdc.functions.approve(PERMIT2, SMOKE_INPUT_ATOMS)._encode_transaction_data(), "approve smoke-swap USDC to Permit2"))
        atomic_write(args.state_file, state)
    if int(permit2.functions.allowance(OWNER, USDC, UNIVERSAL_ROUTER).call()[0]) < SMOKE_INPUT_ATOMS:
        txs.append(send(web3, PERMIT2, permit2.functions.approve(USDC, UNIVERSAL_ROUTER, SMOKE_INPUT_ATOMS, smoke_expiry)._encode_transaction_data(), "approve smoke-swap USDC to Universal Router"))
        atomic_write(args.state_file, state)
    swap_params = encode(
        ["((address,address,uint24,int24,address),bool,uint128,uint128,bytes)"],
        [(POOL_KEY, True, SMOKE_INPUT_ATOMS, SMOKE_MIN_OUTPUT_ATOMS, b"")],
    )
    settle_all = encode(["address", "uint256"], [USDC, SMOKE_INPUT_ATOMS])
    take_all = encode(["address", "uint256"], [WA666, SMOKE_MIN_OUTPUT_ATOMS])
    v4_input = encode(["bytes", "bytes[]"], [bytes([0x06, 0x0C, 0x0F]), [swap_params, settle_all, take_all]])
    router_data = execute_calldata(bytes([0x10]), [v4_input], int(time.time()) + 600)
    txs.append(send(web3, UNIVERSAL_ROUTER, router_data, "smoke swap 10 USDC to wA666 through live v4 pool"))
    txs.append(send(web3, PERMIT2, permit2.functions.approve(USDC, UNIVERSAL_ROUTER, 0, 0)._encode_transaction_data(), "revoke Universal Router Permit2 allowance"))
    txs.append(send(web3, USDC, usdc.functions.approve(PERMIT2, 0)._encode_transaction_data(), "revoke smoke-swap ERC20 allowance"))
    post = snapshot()
    smoke = {
        "usdc_spent_atoms": before_smoke["usdc_owner_atoms"] - post["usdc_owner_atoms"],
        "wa666_received_atoms": post["wa666_owner_atoms"] - before_smoke["wa666_owner_atoms"],
        "supply_delta_atoms": post["wa666_total_supply_atoms"] - before_smoke["wa666_total_supply_atoms"],
    }
    if post["pool_liquidity"] <= 0 or smoke["usdc_spent_atoms"] != SMOKE_INPUT_ATOMS or smoke["wa666_received_atoms"] < SMOKE_MIN_OUTPUT_ATOMS or smoke["supply_delta_atoms"] != 0:
        raise RuntimeError(f"pool seed/smoke invariant failed: post={post}, smoke={smoke}")
    for token, spender in ((USDC, POSITION_MANAGER), (WA666, POSITION_MANAGER), (USDC, UNIVERSAL_ROUTER)):
        if int(permit2.functions.allowance(OWNER, token, spender).call()[0]) != 0:
            raise RuntimeError("residual Permit2 allowance")
    state.update({"phase": "seeded-and-smoke-verified", "pre_smoke_state": before_smoke, "post_state": post, "smoke": smoke})
    atomic_write(args.state_file, state)
    print(json.dumps(state, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
