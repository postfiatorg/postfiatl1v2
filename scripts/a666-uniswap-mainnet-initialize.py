#!/usr/bin/env python3
"""Initialize the reviewed, hookless wA666/USDC Uniswap v4 pool on mainnet.

This is intentionally an initialize-only transaction. It does not mint wA666,
move USDC, approve tokens, or create an LP position. The pool remains empty
until proof-exported wA666 is available for canonical seeding.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import sys
import tempfile
from typing import Any

from eth_abi import encode
from web3 import Web3


ROOT = Path(__file__).resolve().parents[1]
STAKEHUB = Path("/home/postfiat/repos/StakeHub")
RPC = "https://ethereum-rpc.publicnode.com"
CHAIN_ID = 1
EXPECTED_NONCE = 176
Q96 = 79_228_162_514_264_337_593_543_950_336

DEPLOYER = Web3.to_checksum_address("0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0")
USDC = Web3.to_checksum_address("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
TOKEN = Web3.to_checksum_address("0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5")
CONTROLLER = Web3.to_checksum_address("0x9A0262C0572fb4DB08765408eB225E207F40c3d9")
POOL_MANAGER = Web3.to_checksum_address("0x000000000004444c5dc75cB358380D2e3dE08A90")
STATE_VIEW = Web3.to_checksum_address("0x7fFE42C4a5DEeA5b0feC41C94C136Cf115597227")

POOL_MANAGER_CODE_HASH = "0x785f1014552b7ce7d5fb7d0c970ca60edee94fd00425d7ca21609acac7ce1293"
STATE_VIEW_CODE_HASH = "0xd7947778589cf4aac9a092a4451292a2056380941635ab7006d3c691d8dfd878"
USDC_CODE_HASH = "0xd80d4b7c890cb9d6a4893e6b52bc34b56b25335cb13716e0d1d31383e6b41505"
TOKEN_CODE_HASH = "0x671ee905050e2969995a8c6db8b05e4c2f30bd690eeff55093c03f9722be66b0"
CONTROLLER_CODE_HASH = "0x4c62b7d8b3a7928fd9667445f8fd68b3336ba0ec9a8f3e59b463b684fe6ceaaf"

FEE = 500
TICK_SPACING = 10
HOOKS = Web3.to_checksum_address("0x0000000000000000000000000000000000000000")
POOL_ID = "0xc5f1e4b5bb07c0718eddcc3d102dc751b8953ec25bb05cdc14d95419d4d16e98"
STATE_PATH = ROOT / "deployments/a666-mainnet-20260727/ethereum/pool-state.json"

POOL_MANAGER_ABI = [
    {
        "type": "function",
        "name": "initialize",
        "stateMutability": "nonpayable",
        "inputs": [
            {
                "name": "key",
                "type": "tuple",
                "components": [
                    {"name": "currency0", "type": "address"},
                    {"name": "currency1", "type": "address"},
                    {"name": "fee", "type": "uint24"},
                    {"name": "tickSpacing", "type": "int24"},
                    {"name": "hooks", "type": "address"},
                ],
            },
            {"name": "sqrtPriceX96", "type": "uint160"},
        ],
        "outputs": [{"name": "tick", "type": "int24"}],
    }
]
STATE_VIEW_ABI = [
    {
        "type": "function",
        "name": "getSlot0",
        "stateMutability": "view",
        "inputs": [{"name": "poolId", "type": "bytes32"}],
        "outputs": [
            {"name": "sqrtPriceX96", "type": "uint160"},
            {"name": "tick", "type": "int24"},
            {"name": "protocolFee", "type": "uint24"},
            {"name": "lpFee", "type": "uint24"},
        ],
    },
    {
        "type": "function",
        "name": "getLiquidity",
        "stateMutability": "view",
        "inputs": [{"name": "poolId", "type": "bytes32"}],
        "outputs": [{"name": "liquidity", "type": "uint128"}],
    },
]
TOKEN_ABI = [
    {
        "type": "function",
        "name": "totalSupply",
        "stateMutability": "view",
        "inputs": [],
        "outputs": [{"name": "", "type": "uint256"}],
    }
]
CONTROLLER_ABI = [
    {
        "type": "function",
        "name": "mintPaused",
        "stateMutability": "view",
        "inputs": [],
        "outputs": [{"name": "", "type": "bool"}],
    }
]


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


def code_hash(web3: Web3, address: str) -> str:
    return Web3.to_hex(Web3.keccak(bytes(web3.eth.get_code(address)))).lower()


def require_code_hash(web3: Web3, address: str, expected: str, label: str) -> None:
    actual = code_hash(web3, address)
    if actual != expected:
        raise RuntimeError(f"{label} runtime hash mismatch: {actual} != {expected}")


def normalize_tx_hash(value: str) -> str:
    return value if value.startswith("0x") else f"0x{value}"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--state-file", type=Path, default=STATE_PATH)
    args = parser.parse_args()

    web3 = Web3(Web3.HTTPProvider(RPC, request_kwargs={"timeout": 60}))
    if not web3.is_connected() or int(web3.eth.chain_id) != CHAIN_ID:
        raise RuntimeError("Ethereum mainnet RPC unavailable or wrong chain")
    latest_nonce = int(web3.eth.get_transaction_count(DEPLOYER, "latest"))
    pending_nonce = int(web3.eth.get_transaction_count(DEPLOYER, "pending"))
    if latest_nonce != EXPECTED_NONCE or pending_nonce != latest_nonce:
        raise RuntimeError(
            f"deployer nonce drift/pending transaction: latest={latest_nonce} pending={pending_nonce}"
        )

    expected_hashes = {
        POOL_MANAGER: (POOL_MANAGER_CODE_HASH, "PoolManager"),
        STATE_VIEW: (STATE_VIEW_CODE_HASH, "StateView"),
        USDC: (USDC_CODE_HASH, "USDC"),
        TOKEN: (TOKEN_CODE_HASH, "wA666"),
        CONTROLLER: (CONTROLLER_CODE_HASH, "A666 controller"),
    }
    for address, (expected, label) in expected_hashes.items():
        require_code_hash(web3, address, expected, label)

    currencies = sorted((USDC, TOKEN), key=lambda value: int(value, 16))
    key = (currencies[0], currencies[1], FEE, TICK_SPACING, HOOKS)
    computed_pool_id = Web3.to_hex(
        Web3.keccak(
            encode(
                ["address", "address", "uint24", "int24", "address"],
                list(key),
            )
        )
    )
    if computed_pool_id != POOL_ID:
        raise RuntimeError(f"pool ID drift: {computed_pool_id} != {POOL_ID}")

    pool_manager = web3.eth.contract(address=POOL_MANAGER, abi=POOL_MANAGER_ABI)
    state_view = web3.eth.contract(address=STATE_VIEW, abi=STATE_VIEW_ABI)
    token = web3.eth.contract(address=TOKEN, abi=TOKEN_ABI)
    controller = web3.eth.contract(address=CONTROLLER, abi=CONTROLLER_ABI)
    slot0_before = tuple(state_view.functions.getSlot0(POOL_ID).call())
    liquidity_before = int(state_view.functions.getLiquidity(POOL_ID).call())
    token_supply_before = int(token.functions.totalSupply().call())
    mint_paused_before = bool(controller.functions.mintPaused().call())
    if slot0_before != (0, 0, 0, 0) or liquidity_before != 0:
        raise RuntimeError(
            f"pool is already initialized or nonempty: slot0={slot0_before}, "
            f"liquidity={liquidity_before}"
        )
    if token_supply_before != 0 or not mint_paused_before:
        raise RuntimeError(
            f"unsafe pre-state: token_supply={token_supply_before}, "
            f"mint_paused={mint_paused_before}"
        )

    calldata = pool_manager.functions.initialize(key, Q96)._encode_transaction_data()
    gas_estimate = int(
        web3.eth.estimate_gas(
            {
                "from": DEPLOYER,
                "to": POOL_MANAGER,
                "data": calldata,
                "value": 0,
            }
        )
    )

    sys.path.insert(0, str(STAKEHUB))
    from stakehub.agentd import call

    status = call({"op": "status"})
    if not status or not status.get("ok") or not status.get("unlocked"):
        raise RuntimeError("StakeHub agent is unavailable or locked")

    state: dict[str, Any] = {
        "schema": "postfiat-a666-uniswap-mainnet-pool-v1",
        "phase": "prepared",
        "chain_id": CHAIN_ID,
        "rpc": RPC,
        "deployer": DEPLOYER,
        "nonce": EXPECTED_NONCE,
        "block_number_preflight": int(web3.eth.block_number),
        "balance_wei_before": int(web3.eth.get_balance(DEPLOYER)),
        "pool_key": {
            "currency0": key[0],
            "currency1": key[1],
            "fee": FEE,
            "tick_spacing": TICK_SPACING,
            "hooks": HOOKS,
        },
        "pool_id": POOL_ID,
        "initial_sqrt_price_x96": Q96,
        "initial_nav_usd_1e8": 100_000_000,
        "pool_manager": POOL_MANAGER,
        "state_view": STATE_VIEW,
        "wrapped_token": TOKEN,
        "usdc": USDC,
        "calldata": calldata,
        "gas_estimate": gas_estimate,
        "pre_state": {
            "slot0": list(slot0_before),
            "liquidity": liquidity_before,
            "wrapped_total_supply": token_supply_before,
            "controller_mint_paused": mint_paused_before,
        },
        "agent": {
            "unlocked": True,
            "spent_today_usd_before": status.get("spent_today_usd"),
        },
    }
    atomic_write_json(args.state_file, state)
    if not args.execute:
        print(json.dumps(state, indent=2, sort_keys=True))
        return

    # PoolManager is on the passphrase-gated global whitelist. The launch
    # session path is intentionally reserved for transactions that deploy at
    # least one reviewed bytecode artifact, so this call uses the agent's
    # allowlisted, session-less contract-call authorization path.
    response = call(
        {
            "op": "evm_contract_tx",
            "to": POOL_MANAGER,
            "data": calldata,
            "rpc_url": RPC,
            "chain_id": CHAIN_ID,
            "label": "initialize reviewed A666 Uniswap v4 pool",
            "session_action": "initialize_a666_uniswap_pool",
            "value_wei": 0,
            "gas_usd": 0,
        },
        timeout=1200.0,
    )
    if not response or not response.get("ok"):
        raise RuntimeError(f"pool initialization failed: {response}")
    transaction_hash = normalize_tx_hash(response["tx"])
    receipt = web3.eth.get_transaction_receipt(transaction_hash)
    if int(receipt.status) != 1:
        raise RuntimeError("pool initialization reverted")

    slot0_after = tuple(state_view.functions.getSlot0(POOL_ID).call())
    liquidity_after = int(state_view.functions.getLiquidity(POOL_ID).call())
    token_supply_after = int(token.functions.totalSupply().call())
    mint_paused_after = bool(controller.functions.mintPaused().call())
    if slot0_after != (Q96, 0, 0, FEE):
        raise RuntimeError(f"unexpected initialized slot0: {slot0_after}")
    if liquidity_after != 0:
        raise RuntimeError(f"initialize-only pool unexpectedly has liquidity: {liquidity_after}")
    if token_supply_after != 0 or not mint_paused_after:
        raise RuntimeError(
            f"supply/controller drift: token_supply={token_supply_after}, "
            f"mint_paused={mint_paused_after}"
        )

    state.update(
        {
            "phase": "initialized-empty",
            "transaction": {
                "tx": transaction_hash,
                "block_number": int(receipt.blockNumber),
                "gas_used": int(receipt.gasUsed),
                "status": int(receipt.status),
            },
            "post_state": {
                "slot0": list(slot0_after),
                "liquidity": liquidity_after,
                "wrapped_total_supply": token_supply_after,
                "controller_mint_paused": mint_paused_after,
            },
            "end_nonce": int(web3.eth.get_transaction_count(DEPLOYER, "latest")),
            "balance_wei_after": int(web3.eth.get_balance(DEPLOYER)),
            "block_number_verified": int(web3.eth.block_number),
        }
    )
    atomic_write_json(args.state_file, state)
    print(json.dumps(state, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
