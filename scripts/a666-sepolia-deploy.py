#!/usr/bin/env python3
"""Persistent CONTROLLED a666 bridge-stack deployment on Ethereum Sepolia.

Signing stays inside the already-unlocked StakeHub agent. The script accepts no
private key. It predicts every CREATE address, derives the authoritative PFTL
route digest with postfiat-node before deployment, checkpoints every broadcast,
and separates pool initialization from canonical export-backed liquidity.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any

import rlp
from eth_abi import encode
from web3 import Web3


ROOT = Path(__file__).resolve().parents[1]
CONTRACTS = ROOT / "crates/ethereum-contracts"
NODE = ROOT / "target/release/postfiat-node"
STAKEHUB = Path(__file__).resolve().parents[2] / "StakeHub"
STATE = ROOT / "docs/evidence/a666-uniswap-bridge-build-20260723/sepolia-persistent/state.json"
RPC = "https://ethereum-sepolia-rpc.publicnode.com"
CHAIN_ID = 11_155_111
OWNER = Web3.to_checksum_address("0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0")
USDC = Web3.to_checksum_address("0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238")
POOL_MANAGER = Web3.to_checksum_address("0xE03A1074c86CFeDd5C142C4F04F1a1536e203543")
POSITION_MANAGER = Web3.to_checksum_address("0x429ba70129df741B2Ca2a85BC3A2a3328e5c09b4")
UNIVERSAL_ROUTER = Web3.to_checksum_address("0x3A9D48AB9751398BbFa63ad67599Bb04e4BdF98b")
STATE_VIEW = Web3.to_checksum_address("0xe1dd9c3fa50edb962e442f60dfbc432e24537e4c")
PERMIT2 = Web3.to_checksum_address("0x000000000022D473030F116dDEE9F6B43aC78BA3")

NATIVE_A666 = "300bf48a63a94770b6e67817f88cd1abf77e7f592a061e15682d7fd9973260af4c2e631e32df3c2c402b7d2fe272a293"
PFUSDC = "02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b"
PRICING_PACKET = "b57dc2b7b39b2a1449bfc810f0a90d279ae2f5d3ce19652c9a966a89ede84d92fd0b5433a9d9a10e030654701a09ed70"
TRUST_CLASS = Web3.keccak(text="CONTROLLED")
Q96 = 79_228_162_514_264_337_593_543_950_336

DEPLOYS = [
    ("wrapped_navcoin", "PFTLUniswapHandoffController.sol", "WrappedVenueNAVCoin"),
    ("receipt_verifier", "PFTLUniswapHandoffController.sol", "ControlledPFTLReceiptVerifier"),
    ("replay_registry", "PFTLUniswapHandoffController.sol", "PacketReplayRegistry"),
    ("v4_router", "PFTLUniswapV4PoolHarness.sol", "PFTLUniswapV4ExactInputRouter"),
    ("launch_helper", "PFTLUniswapV4PoolHarness.sol", "PFTLUniswapV4LaunchHelper"),
    ("settlement_adapter", "PFTLUniswapHandoffController.sol", "UniswapSettlementAdapter"),
    ("bridge_controller", "PFTLUniswapHandoffController.sol", "PFTLUniswapHandoffController"),
]


class DeploymentError(RuntimeError):
    pass


def artifact(source: str, name: str) -> dict[str, Any]:
    path = CONTRACTS / "out" / source / f"{name}.json"
    if not path.is_file():
        raise DeploymentError(f"missing artifact: {path}")
    value = json.loads(path.read_text())
    if not value.get("bytecode", {}).get("object"):
        raise DeploymentError(f"artifact has no bytecode: {path}")
    return value


def contract(web3: Web3, source: str, name: str, address: str | None = None):
    value = artifact(source, name)
    return web3.eth.contract(
        address=Web3.to_checksum_address(address) if address else None,
        abi=value["abi"],
        bytecode=value["bytecode"]["object"],
    )


def predict_create(owner: str, nonce: int) -> str:
    raw = Web3.keccak(rlp.encode([bytes.fromhex(owner[2:]), nonce]))[-20:]
    return Web3.to_checksum_address(raw)


def pool_id(wrapped: str) -> str:
    currency0, currency1 = sorted(
        [Web3.to_checksum_address(wrapped), USDC], key=lambda value: int(value, 16)
    )
    encoded = encode(
        ["address", "address", "uint24", "int24", "address"],
        [currency0, currency1, 500, 10, "0x0000000000000000000000000000000000000000"],
    )
    return Web3.to_hex(Web3.keccak(encoded))


def route_config(addresses: dict[str, str], pool: str) -> dict[str, Any]:
    return {
        "schema": "postfiat-pftl-uniswap-route-config-v1",
        "route_id": "pftl-a666-ce22-sepolia-wA666-usdc-controlled-20260723-v1",
        "route_family": "primary_pftl_mint",
        "native_nav_asset_id": NATIVE_A666,
        "settlement_asset_id": PFUSDC,
        "wrapped_navcoin_token": addresses["wrapped_navcoin"],
        "handoff_controller": addresses["bridge_controller"],
        "settlement_adapter": addresses["settlement_adapter"],
        "verifier_mode": "controlled-attestation-v1",
        "route_trust_class": "CONTROLLED",
        "uniswap_pool_id_or_path": pool,
        "router": addresses["v4_router"],
        "failure_behavior": "refund_unconsumed_pftl_packet",
        "route_supply_cap_atoms": 10_000_000,
        "packet_notional_cap_atoms": 1_000_000,
        "seed_nav_epoch": 1,
        "seed_usdc_atoms": 100_000,
        "seed_wrapped_navcoin_atoms": 100_000,
        "lp_recipient": OWNER,
        "lp_custody_policy": "controlled_sepolia_canonical_export_lp",
    }


def derive_route_digest(config: dict[str, Any]) -> str:
    with tempfile.TemporaryDirectory(prefix="a666-route-digest-") as temporary:
        temporary_path = Path(temporary)
        config_path = temporary_path / "route.json"
        config_path.write_text(json.dumps(config, sort_keys=True))
        result = subprocess.run(
            [
                str(NODE),
                "navcoin-bridge-route-init",
                "--data-dir",
                str(temporary_path / "state"),
                "--config-file",
                str(config_path),
                "--ethereum-chain-id",
                str(CHAIN_ID),
                "--latest-finalized-nav-epoch",
                "1",
                "--return-finality-blocks",
                "64",
            ],
            check=True,
            capture_output=True,
            text=True,
        )
        return str(json.loads(result.stdout)["route_config_digest"])


def build_plan(web3: Web3) -> dict[str, Any]:
    nonce = web3.eth.get_transaction_count(OWNER, "pending")
    addresses = {
        key: predict_create(OWNER, nonce + offset)
        for offset, (key, _source, _name) in enumerate(DEPLOYS)
    }
    pool = pool_id(addresses["wrapped_navcoin"])
    config = route_config(addresses, pool)
    return {
        "schema": "postfiat-a666-sepolia-deployment-state-v1",
        "trust_class": "CONTROLLED",
        "chain_id": CHAIN_ID,
        "rpc": RPC,
        "owner": OWNER,
        "start_nonce": nonce,
        "addresses": addresses,
        "official_uniswap": {
            "pool_manager": POOL_MANAGER,
            "position_manager": POSITION_MANAGER,
            "universal_router": UNIVERSAL_ROUTER,
            "state_view": STATE_VIEW,
            "permit2": PERMIT2,
        },
        "usdc": USDC,
        "pool_id": pool,
        "route_path_hash": Web3.to_hex(
            Web3.keccak(text=f"a666/USDC:v4:500:10:{pool}:CONTROLLED")
        ),
        "route_config": config,
        "route_config_digest": derive_route_digest(config),
        "deployments": {},
        "setup_transactions": {},
        "pool_initialized": False,
    }


def load_state() -> dict[str, Any] | None:
    if not STATE.is_file():
        return None
    return json.loads(STATE.read_text())


def save_state(value: dict[str, Any]) -> None:
    STATE.parent.mkdir(parents=True, exist_ok=True)
    temporary = STATE.with_suffix(".tmp")
    temporary.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    temporary.replace(STATE)


def require_code(web3: Web3, address: str, label: str) -> None:
    for _attempt in range(20):
        if web3.eth.get_code(Web3.to_checksum_address(address)):
            return
        time.sleep(2)
    raise DeploymentError(f"{label} has no code: {address}")


def preflight(web3: Web3, state: dict[str, Any]) -> dict[str, Any]:
    if not web3.is_connected() or web3.eth.chain_id != CHAIN_ID:
        raise DeploymentError("wrong or unavailable Sepolia RPC")
    if not NODE.is_file():
        raise DeploymentError(f"missing reconciled postfiat-node: {NODE}")
    for label, address in {
        "PoolManager": POOL_MANAGER,
        "PositionManager": POSITION_MANAGER,
        "UniversalRouter": UNIVERSAL_ROUTER,
        "StateView": STATE_VIEW,
        "Permit2": PERMIT2,
        "Circle test-USDC": USDC,
    }.items():
        require_code(web3, address, label)
    for key, source, name in DEPLOYS:
        value = artifact(source, name)
        if not bytes.fromhex(value["bytecode"]["object"].removeprefix("0x")):
            raise DeploymentError(f"empty creation bytecode: {key}")
    if state["chain_id"] != CHAIN_ID or state["owner"].lower() != OWNER.lower():
        raise DeploymentError("persisted deployment state targets a different chain or owner")
    if derive_route_digest(state["route_config"]) != state["route_config_digest"]:
        raise DeploymentError("persisted route digest does not reproduce")
    return {
        "chain_id": CHAIN_ID,
        "owner": OWNER,
        "owner_nonce_pending": web3.eth.get_transaction_count(OWNER, "pending"),
        "owner_eth_wei": web3.eth.get_balance(OWNER),
        "owner_test_usdc_atoms": int(
            web3.eth.contract(
                address=USDC,
                abi=[
                    {
                        "type": "function",
                        "name": "balanceOf",
                        "stateMutability": "view",
                        "inputs": [{"name": "account", "type": "address"}],
                        "outputs": [{"name": "", "type": "uint256"}],
                    }
                ],
            ).functions.balanceOf(OWNER).call()
        ),
        "route_config_digest": state["route_config_digest"],
        "pool_id": state["pool_id"],
        "addresses": state["addresses"],
    }


def expected_deploys(state: dict[str, Any], web3: Web3) -> list[dict[str, Any]]:
    rows = []
    for key, source, name in DEPLOYS:
        if web3.eth.get_code(Web3.to_checksum_address(state["addresses"][key])):
            continue
        value = artifact(source, name)
        bytecode = bytes.fromhex(value["bytecode"]["object"].removeprefix("0x"))
        rows.append(
            {
                "label": f"deploy_{key}",
                "bytecode_hash": Web3.to_hex(Web3.keccak(bytecode)),
                "bytecode_len": len(bytecode),
            }
        )
    return rows


def constructor_args(key: str, state: dict[str, Any]) -> tuple[Any, ...]:
    address = state["addresses"]
    if key == "wrapped_navcoin":
        return ("Wrapped a666 CONTROLLED", "wA666", 6, OWNER)
    if key == "receipt_verifier":
        return (OWNER, TRUST_CLASS)
    if key == "replay_registry":
        return (OWNER,)
    if key == "v4_router":
        return (POOL_MANAGER,)
    if key == "launch_helper":
        return (OWNER, POOL_MANAGER, POSITION_MANAGER, PERMIT2)
    if key == "settlement_adapter":
        return (
            address["v4_router"],
            address["wrapped_navcoin"],
            USDC,
            bytes.fromhex(state["pool_id"][2:]),
            bytes.fromhex(state["route_path_hash"][2:]),
            OWNER,
        )
    if key == "bridge_controller":
        config = (
            OWNER,
            CHAIN_ID,
            bytes.fromhex(state["route_config_digest"]),
            TRUST_CLASS,
            bytes.fromhex(PFUSDC),
            bytes.fromhex(NATIVE_A666),
            bytes.fromhex(PRICING_PACKET),
            1,
            bytes.fromhex(state["pool_id"][2:]),
            10_000_000,
            1_000_000,
            address["replay_registry"],
        )
        return (
            address["wrapped_navcoin"],
            address["settlement_adapter"],
            address["receipt_verifier"],
            config,
        )
    raise DeploymentError(f"unknown deployment key: {key}")


def agent_call(request: dict[str, Any], timeout: float = 900) -> dict[str, Any]:
    sys.path.insert(0, str(STAKEHUB))
    from stakehub.agentd import call

    response = call(request, None, timeout)
    if not response or not response.get("ok"):
        raise DeploymentError(str((response or {}).get("error", "StakeHub agent unavailable")))
    return response


def send_call(
    target: Any,
    function: Any,
    state: dict[str, Any],
    label: str,
) -> dict[str, Any]:
    result = agent_call(
        {
            "op": "evm_contract_tx",
            "to": target.address,
            "data": function._encode_transaction_data(),
            "rpc_url": RPC,
            "chain_id": CHAIN_ID,
            "label": f"a666 Sepolia CONTROLLED {label}",
            "session_id": state["route_config_digest"],
            "session_action": label,
            "gas_usd": 0,
        }
    )
    state["setup_transactions"][label] = result["tx"]
    save_state(state)
    return result


def deploy(web3: Web3, state: dict[str, Any]) -> dict[str, Any]:
    missing = expected_deploys(state, web3)
    if missing:
        agent_call({"op": "close_launch_session"})
        agent_call(
            {
                "op": "open_launch_session",
                "session_id": state["route_config_digest"],
                "chain_id": CHAIN_ID,
                "allowlist": [
                    OWNER,
                    USDC,
                    POOL_MANAGER,
                    POSITION_MANAGER,
                    UNIVERSAL_ROUTER,
                    STATE_VIEW,
                    PERMIT2,
                    *state["addresses"].values(),
                ],
                "expected_deploys": missing,
                "usdc_address": USDC,
                "usdc_budget": 0,
                "ttl_seconds": 3600,
            }
        )
    else:
        agent_call({"op": "close_launch_session"})
        # Calls still need a bounded session; use an already-deployed artifact as
        # an intentionally unused expected deployment.
        value = artifact("PFTLUniswapHandoffController.sol", "PacketReplayRegistry")
        bytecode = bytes.fromhex(value["bytecode"]["object"].removeprefix("0x"))
        agent_call(
            {
                "op": "open_launch_session",
                "session_id": state["route_config_digest"],
                "chain_id": CHAIN_ID,
                "allowlist": [
                    OWNER,
                    USDC,
                    POOL_MANAGER,
                    POSITION_MANAGER,
                    UNIVERSAL_ROUTER,
                    STATE_VIEW,
                    PERMIT2,
                    *state["addresses"].values(),
                ],
                "expected_deploys": [
                    {
                        "label": "unused_resume_guard",
                        "bytecode_hash": Web3.to_hex(Web3.keccak(bytecode)),
                        "bytecode_len": len(bytecode),
                    }
                ],
                "usdc_address": USDC,
                "usdc_budget": 0,
                "ttl_seconds": 3600,
            }
        )

    try:
        for key, source, name in DEPLOYS:
            address = Web3.to_checksum_address(state["addresses"][key])
            if web3.eth.get_code(address):
                require_code(web3, address, key)
                continue
            expected_nonce = state["start_nonce"] + [row[0] for row in DEPLOYS].index(key)
            actual_nonce = web3.eth.get_transaction_count(OWNER, "pending")
            if actual_nonce != expected_nonce:
                raise DeploymentError(
                    f"nonce drift before {key}: got {actual_nonce}, expected {expected_nonce}"
                )
            factory = contract(web3, source, name)
            data = factory.constructor(*constructor_args(key, state)).data_in_transaction
            result = agent_call(
                {
                    "op": "evm_contract_tx",
                    "to": None,
                    "data": data,
                    "rpc_url": RPC,
                    "chain_id": CHAIN_ID,
                    "label": f"a666 Sepolia CONTROLLED deploy {key}",
                    "session_id": state["route_config_digest"],
                    "session_action": f"deploy_{key}",
                    "gas_usd": 0,
                }
            )
            if Web3.to_checksum_address(result["contract_address"]) != address:
                raise DeploymentError(f"{key} deployed at unexpected address")
            require_code(web3, address, key)
            state["deployments"][key] = result["tx"]
            save_state(state)

        address = state["addresses"]
        wrapped = contract(
            web3,
            "PFTLUniswapHandoffController.sol",
            "WrappedVenueNAVCoin",
            address["wrapped_navcoin"],
        )
        adapter = contract(
            web3,
            "PFTLUniswapHandoffController.sol",
            "UniswapSettlementAdapter",
            address["settlement_adapter"],
        )
        replay = contract(
            web3,
            "PFTLUniswapHandoffController.sol",
            "PacketReplayRegistry",
            address["replay_registry"],
        )
        helper = contract(
            web3,
            "PFTLUniswapV4PoolHarness.sol",
            "PFTLUniswapV4LaunchHelper",
            address["launch_helper"],
        )
        controller_address = address["bridge_controller"]

        if wrapped.functions.controller().call() == "0x0000000000000000000000000000000000000000":
            send_call(wrapped, wrapped.functions.setController(controller_address), state, "wrapped_set_controller")
        if not wrapped.functions.controller_locked().call():
            send_call(wrapped, wrapped.functions.lockController(), state, "wrapped_lock_controller")
        if adapter.functions.controller().call() == "0x0000000000000000000000000000000000000000":
            send_call(adapter, adapter.functions.setController(controller_address), state, "adapter_set_controller")
        if not adapter.functions.controller_locked().call():
            send_call(adapter, adapter.functions.lockController(), state, "adapter_lock_controller")
        if not replay.functions.authorized_controller(controller_address).call():
            send_call(
                replay,
                replay.functions.setControllerAuthorization(controller_address, True),
                state,
                "replay_authorize_controller",
            )

        if not state.get("pool_initialized"):
            send_call(
                helper,
                helper.functions.initializePool(address["wrapped_navcoin"], USDC, 100_000, 100_000),
                state,
                "initialize_uniswap_v4_pool",
            )
            state["pool_initialized"] = True
            state["pool_sqrt_price_x96"] = Q96
            save_state(state)
    finally:
        agent_call({"op": "close_launch_session", "session_id": state["route_config_digest"]})
    return readback(web3, state)


def readback(web3: Web3, state: dict[str, Any]) -> dict[str, Any]:
    address = state["addresses"]
    for key, value in address.items():
        require_code(web3, value, key)
    wrapped = contract(
        web3,
        "PFTLUniswapHandoffController.sol",
        "WrappedVenueNAVCoin",
        address["wrapped_navcoin"],
    )
    verifier = contract(
        web3,
        "PFTLUniswapHandoffController.sol",
        "ControlledPFTLReceiptVerifier",
        address["receipt_verifier"],
    )
    replay = contract(
        web3,
        "PFTLUniswapHandoffController.sol",
        "PacketReplayRegistry",
        address["replay_registry"],
    )
    adapter = contract(
        web3,
        "PFTLUniswapHandoffController.sol",
        "UniswapSettlementAdapter",
        address["settlement_adapter"],
    )
    controller = contract(
        web3,
        "PFTLUniswapHandoffController.sol",
        "PFTLUniswapHandoffController",
        address["bridge_controller"],
    )
    state_view = web3.eth.contract(
        address=STATE_VIEW,
        abi=[
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
            }
        ],
    )
    slot0 = state_view.functions.getSlot0(bytes.fromhex(state["pool_id"][2:])).call()
    result = {
        "schema": "postfiat-a666-sepolia-readback-v1",
        "trust_class": "CONTROLLED",
        "chain_id": web3.eth.chain_id,
        "block_number": web3.eth.block_number,
        "addresses": address,
        "route_config_digest": state["route_config_digest"],
        "pool_id": state["pool_id"],
        "pool_slot0": {
            "sqrt_price_x96": int(slot0[0]),
            "tick": int(slot0[1]),
            "protocol_fee": int(slot0[2]),
            "lp_fee": int(slot0[3]),
        },
        "wrapped": {
            "name": wrapped.functions.name().call(),
            "symbol": wrapped.functions.symbol().call(),
            "decimals": wrapped.functions.decimals().call(),
            "owner": wrapped.functions.owner().call(),
            "controller": wrapped.functions.controller().call(),
            "controller_locked": wrapped.functions.controller_locked().call(),
            "total_supply": wrapped.functions.totalSupply().call(),
        },
        "verifier": {
            "owner": verifier.functions.owner().call(),
            "route_trust_class": Web3.to_hex(verifier.functions.route_trust_class().call()),
        },
        "replay_controller_authorized": replay.functions.authorized_controller(
            address["bridge_controller"]
        ).call(),
        "adapter": {
            "controller": adapter.functions.controller().call(),
            "controller_locked": adapter.functions.controller_locked().call(),
            "pool_id": Web3.to_hex(adapter.functions.uniswap_pool_id().call()),
            "token_in": adapter.functions.token_in().call(),
            "token_out": adapter.functions.token_out().call(),
        },
        "controller": {
            "owner": controller.functions.owner().call(),
            "route_trust_class": Web3.to_hex(controller.functions.route_trust_class().call()),
            "verifier_trust_class": Web3.to_hex(controller.functions.verifierTrustClass().call()),
            "route_config_digest": Web3.to_hex(controller.functions.route_config_digest().call()),
            "pool_id": Web3.to_hex(controller.functions.uniswap_pool_id().call()),
            "paused": controller.functions.paused().call(),
            "outstanding_minted_atoms": controller.functions.outstanding_minted_atoms().call(),
        },
    }
    if result["pool_slot0"]["sqrt_price_x96"] == 0:
        raise DeploymentError("persistent pool is not initialized")
    if result["wrapped"]["total_supply"] != 0:
        raise DeploymentError("pre-export wrapped supply must remain zero")
    if result["verifier"]["route_trust_class"] != Web3.to_hex(TRUST_CLASS):
        raise DeploymentError("verifier is not CONTROLLED")
    if result["controller"]["route_config_digest"] != "0x" + state["route_config_digest"]:
        raise DeploymentError("controller route digest mismatch")
    state["readback"] = result
    save_state(state)
    return result


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("action", choices=("preflight", "deploy", "readback"))
    args = parser.parse_args()
    web3 = Web3(Web3.HTTPProvider(RPC, request_kwargs={"timeout": 30}))
    state = load_state() or build_plan(web3)
    if args.action == "preflight":
        print(json.dumps(preflight(web3, state), indent=2, sort_keys=True))
        return
    if not STATE.exists():
        save_state(state)
    preflight(web3, state)
    result = deploy(web3, state) if args.action == "deploy" else readback(web3, state)
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
