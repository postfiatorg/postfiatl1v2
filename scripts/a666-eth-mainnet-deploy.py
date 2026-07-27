#!/usr/bin/env python3
"""Deploy the paused, proof-gated A666 Ethereum stack through StakeHub agentd.

The script is intentionally specific to the reviewed 2026-07-27 deployment
tuple. It refuses nonce drift, artifact drift, runtime-code drift, a locked
agent, pending transactions, or any pre-existing code at a predicted address.
No private key is read or written by this process.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
from typing import Any

from eth_abi import encode
from eth_utils import keccak, to_checksum_address
import rlp
from web3 import Web3


ROOT = Path(__file__).resolve().parents[1]
STAKEHUB = Path("/home/postfiat/repos/StakeHub")
RPC = "https://ethereum-rpc.publicnode.com"
CHAIN_ID = 1
DEPLOYER = to_checksum_address("0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0")
EXPECTED_START_NONCE = 169
STATE_PATH = ROOT / "deployments/a666-mainnet-20260727/ethereum/deployment-state.json"

SP1_VERIFIER = to_checksum_address("0x397a5f7f3dbd538f23de225b51f532c34448da9b")
SP1_VERIFIER_CODE_HASH = "0x11612dc6695484a4dfcac5ae1a7bcb093f891eb93d8aa9ddeca7f24a1f3b7d57"
SP1_PROGRAM_VKEY = bytes.fromhex(
    "004e44aca326861252ee5ff7863b1174635b727759b75d46b28bb28d4a7b34f9"
)
SP1_ELF_SHA256 = "495e46273337ce4ff035177825a605cc389ad82c05ead11d4874e349ba22cc3a"

USDC = to_checksum_address("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
USDC_CODE_HASH = "0xd80d4b7c890cb9d6a4893e6b52bc34b56b25335cb13716e0d1d31383e6b41505"
A651 = to_checksum_address("0x1e55EDa7ce0788E8b624456C4d401A33bD83b62e")
A651_SUPPLY_CONTROLLER = to_checksum_address(
    "0x74f4A27Acd503B3aABE955659BFEda33082e3340"
)

PFTL_CHAIN_ID = "postfiat-wan-devnet-2"
PFTL_GENESIS_HASH = (
    "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad255"
    "21aff3ed334da07e150a7233a3e90a9"
)
PFTL_PROTOCOL_VERSION = 1
PFTL_INITIAL_HEIGHT = 344
PFTL_INITIAL_BLOCK_ID = (
    "283a24c12098da85a3387cd92f7be0a70cc63a142be1c3b4628bc15053f5dc9b"
    "862130ce9c9a17c1e9e1e2f78e166d35"
)
ROUTE_ID = "pftl-a666-ethereum-wA666-usdc-v1"
A666_ASSET_ID = (
    "521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b6"
    "2d20e18555642bec32174498cbee5e2c"
)
PFUSDC_ASSET_ID = (
    "02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c2"
    "33f6830bd5221fe2717fb6a1a7005d7b"
)
A666_OPENING_ATOMS = 31_386_197_455
A651_TOTAL_ATOMS = 4_000 * 10**18

EXPECTED_ADDRESSES = {
    "token": to_checksum_address("0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5"),
    "verifier": to_checksum_address("0xb79FF97EcC11574a8A78d0b5a9D7C8c2A94bF96A"),
    "controller": to_checksum_address("0x9A0262C0572fb4DB08765408eB225E207F40c3d9"),
    "migration": to_checksum_address("0xFfBBae0eb8450D10F8C0D26C2952b089D56e517c"),
}
EXPECTED_BASE_CODE = {
    "token": (
        "0x3901e7f7f942beb7e60e78fdb55924c714bdfa5edc6a04ab5b8681fd1bbd3160",
        3991,
    ),
    "verifier": (
        "0x2273b7dab8f01cf325974c631267777d6217e1bc3503d5fe1027f64854b35b72",
        8554,
    ),
    "controller": (
        "0xe582206785a79cc5c23cfc685dd6bddcbadb379f2aeb6933819e2932a442e31a",
        9568,
    ),
    "migration": (
        "0x63d354ad5b1de9db3f665ae51af18b3664aee10e9bf0d93b389760ca266edeae",
        2669,
    ),
}
EXPECTED_INIT_CODE_HASHES = {
    "token": "0x03f6c94359d3bee77008d961a901ec5a2f5379122ae8a52bf14da587f4336a9f",
    "verifier": "0xce280010da6f1fa02d921d3057b956d94d9abbe4125eb5b7aab22b54f9ff5bda",
    "controller": "0x307a127f23a3cbed4b2c2d3b28b8ba972151c17a021b567c71acf3ccf06df8c0",
    "migration": "0xacceafb4893f1e6c3a36c6c14c93f56b3cca0066a3be6b07e8f93609093439f7",
}
EXPECTED_RUNTIME_CODE_HASHES = {
    "token": "0x671ee905050e2969995a8c6db8b05e4c2f30bd690eeff55093c03f9722be66b0",
    "verifier": "0xe7d8647046de37e0ce2981b0ebaab0c5f56ab3c215a10b4c1ca2d1f36e39cb6d",
    "controller": "0x4c62b7d8b3a7928fd9667445f8fd68b3336ba0ec9a8f3e59b463b684fe6ceaaf",
    "migration": "0x51788f6f8024084fde219255be9804dcc34af014b38e4813d30cd5da41c62ed0",
}

ARTIFACTS = {
    "token": ROOT
    / "crates/ethereum-contracts/out/PFTLUniswapHandoffController.sol/WrappedVenueNAVCoin.json",
    "verifier": ROOT
    / "crates/ethereum-contracts/out/PFTLReceiptFinalityVerifierV1.sol/PFTLReceiptFinalityVerifierV1.json",
    "controller": ROOT
    / "crates/ethereum-contracts/out/PFTLUniswapPrimaryMarketV2.sol/PFTLUniswapPrimaryMarketV2.json",
    "migration": ROOT
    / "crates/ethereum-contracts/out/A651ToA666MigrationV1.sol/A651ToA666MigrationV1.json",
}


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


def hex_hash(value: bytes) -> str:
    return Web3.to_hex(Web3.keccak(value)).lower()


def create_address(sender: str, nonce: int) -> str:
    encoded = rlp.encode([bytes.fromhex(sender[2:]), nonce])
    return to_checksum_address(keccak(encoded)[-20:])


def load_artifacts() -> dict[str, dict[str, Any]]:
    loaded = {name: json.loads(path.read_text()) for name, path in ARTIFACTS.items()}
    for name, artifact in loaded.items():
        bytecode = bytes.fromhex(artifact["bytecode"]["object"].removeprefix("0x"))
        expected_hash, expected_len = EXPECTED_BASE_CODE[name]
        if hex_hash(bytecode) != expected_hash or len(bytecode) != expected_len:
            raise RuntimeError(f"{name} creation bytecode drift")
    return loaded


def deployment_tuple(
    web3: Web3, artifacts: dict[str, dict[str, Any]]
) -> tuple[dict[str, str], dict[str, str], str]:
    addresses = {
        name: create_address(DEPLOYER, EXPECTED_START_NONCE + offset)
        for offset, name in enumerate(("token", "verifier", "controller", "migration"))
    }
    if addresses != EXPECTED_ADDRESSES:
        raise RuntimeError("predicted CREATE address drift")

    currencies = sorted((addresses["token"], USDC), key=lambda value: int(value, 16))
    pool_id = Web3.to_hex(
        Web3.keccak(
            encode(
                ["address", "address", "uint24", "int24", "address"],
                [currencies[0], currencies[1], 500, 10, "0x" + "00" * 20],
            )
        )
    )
    if pool_id != "0xc5f1e4b5bb07c0718eddcc3d102dc751b8953ec25bb05cdc14d95419d4d16e98":
        raise RuntimeError("pool ID drift")

    token = web3.eth.contract(
        abi=artifacts["token"]["abi"], bytecode=artifacts["token"]["bytecode"]["object"]
    )
    token_data = token.constructor("Post Fiat a666", "wA666", 6, DEPLOYER).data_in_transaction

    verifier_config = (
        SP1_VERIFIER,
        SP1_PROGRAM_VKEY,
        Web3.keccak(text=PFTL_CHAIN_ID),
        Web3.keccak(bytes.fromhex(PFTL_GENESIS_HASH)),
        PFTL_PROTOCOL_VERSION,
        Web3.keccak(text=ROUTE_ID),
        Web3.keccak(bytes.fromhex(A666_ASSET_ID)),
        Web3.keccak(bytes.fromhex(PFUSDC_ASSET_ID)),
        CHAIN_ID,
        addresses["controller"],
        addresses["token"],
        bytes.fromhex(EXPECTED_RUNTIME_CODE_HASHES["token"].removeprefix("0x")),
        4096,
        1120,
        Web3.keccak(bytes.fromhex(PFTL_INITIAL_BLOCK_ID)),
        PFTL_INITIAL_HEIGHT,
    )
    verifier = web3.eth.contract(
        abi=artifacts["verifier"]["abi"],
        bytecode=artifacts["verifier"]["bytecode"]["object"],
    )
    verifier_data = verifier.constructor(verifier_config).data_in_transaction

    controller_config = (
        CHAIN_ID,
        bytes.fromhex(PFUSDC_ASSET_ID),
        bytes.fromhex(A666_ASSET_ID),
        bytes.fromhex(pool_id.removeprefix("0x")),
        2_000_000 * 10**6,
        250_000 * 10**6,
        DEPLOYER,
    )
    controller = web3.eth.contract(
        abi=artifacts["controller"]["abi"],
        bytecode=artifacts["controller"]["bytecode"]["object"],
    )
    controller_data = controller.constructor(
        addresses["token"], addresses["verifier"], controller_config
    ).data_in_transaction

    migration = web3.eth.contract(
        abi=artifacts["migration"]["abi"],
        bytecode=artifacts["migration"]["bytecode"]["object"],
    )
    migration_data = migration.constructor(
        A651_SUPPLY_CONTROLLER,
        A651,
        addresses["token"],
        A666_OPENING_ATOMS,
        A651_TOTAL_ATOMS,
    ).data_in_transaction
    init_codes = {
        "token": token_data,
        "verifier": verifier_data,
        "controller": controller_data,
        "migration": migration_data,
    }
    for name, data in init_codes.items():
        if hex_hash(bytes.fromhex(data.removeprefix("0x"))) != EXPECTED_INIT_CODE_HASHES[name]:
            raise RuntimeError(f"{name} init code drift")
    return addresses, init_codes, pool_id


def assert_code_hash(web3: Web3, address: str, expected: str, label: str) -> None:
    actual = hex_hash(bytes(web3.eth.get_code(address)))
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
    if latest_nonce != EXPECTED_START_NONCE or pending_nonce != latest_nonce:
        raise RuntimeError(
            f"deployer nonce drift/pending transaction: latest={latest_nonce} pending={pending_nonce}"
        )
    if int(web3.eth.get_balance(DEPLOYER)) < 10**16:
        raise RuntimeError("deployer ETH balance is below 0.01 ETH")
    assert_code_hash(web3, SP1_VERIFIER, SP1_VERIFIER_CODE_HASH, "SP1 verifier")
    assert_code_hash(web3, USDC, USDC_CODE_HASH, "USDC")

    elf = ROOT / "programs/pftl-uniswap-receipt/elf/pftl-uniswap-receipt-program"
    if hashlib.sha256(elf.read_bytes()).hexdigest() != SP1_ELF_SHA256:
        raise RuntimeError("SP1 guest ELF hash drift")

    artifacts = load_artifacts()
    addresses, init_codes, pool_id = deployment_tuple(web3, artifacts)
    for name, address in addresses.items():
        if web3.eth.get_code(address):
            raise RuntimeError(f"{name} predicted address already has code: {address}")

    sys.path.insert(0, str(STAKEHUB))
    from stakehub.agentd import call

    status = call({"op": "status"})
    if not status or not status.get("ok") or not status.get("unlocked"):
        raise RuntimeError("StakeHub agent is unavailable or locked")

    state: dict[str, Any] = {
        "schema": "postfiat-a666-ethereum-mainnet-deployment-v1",
        "phase": "prepared",
        "chain_id": CHAIN_ID,
        "rpc": RPC,
        "deployer": DEPLOYER,
        "start_nonce": EXPECTED_START_NONCE,
        "balance_wei_before": int(web3.eth.get_balance(DEPLOYER)),
        "block_number_preflight": int(web3.eth.block_number),
        "addresses": addresses,
        "pool_id": pool_id,
        "sp1": {
            "verifier": SP1_VERIFIER,
            "verifier_runtime_code_hash": SP1_VERIFIER_CODE_HASH,
            "program_vkey": Web3.to_hex(SP1_PROGRAM_VKEY),
            "elf_sha256": SP1_ELF_SHA256,
        },
        "pftl": {
            "chain_id": PFTL_CHAIN_ID,
            "genesis_hash": PFTL_GENESIS_HASH,
            "protocol_version": PFTL_PROTOCOL_VERSION,
            "initial_finalized_height": PFTL_INITIAL_HEIGHT,
            "initial_block_id": PFTL_INITIAL_BLOCK_ID,
            "initial_checkpoint_commitment": Web3.to_hex(
                Web3.keccak(bytes.fromhex(PFTL_INITIAL_BLOCK_ID))
            ),
            "route_id": ROUTE_ID,
            "a666_asset_id": A666_ASSET_ID,
            "pfusdc_asset_id": PFUSDC_ASSET_ID,
        },
        "migration": {
            "a651_token": A651,
            "a651_supply_controller": A651_SUPPLY_CONTROLLER,
            "a666_numerator_atoms": A666_OPENING_ATOMS,
            "a651_denominator_atoms": A651_TOTAL_ATOMS,
        },
        "creation_bytecode": {
            name: {
                "keccak256": EXPECTED_BASE_CODE[name][0],
                "length": EXPECTED_BASE_CODE[name][1],
                "init_code_keccak256": EXPECTED_INIT_CODE_HASHES[name],
                "runtime_code_keccak256": EXPECTED_RUNTIME_CODE_HASHES[name],
            }
            for name in addresses
        },
        "agent": {
            "unlocked": True,
            "spent_today_usd_before": status.get("spent_today_usd"),
        },
        "transactions": [],
    }
    atomic_write_json(args.state_file, state)
    if not args.execute:
        print(json.dumps(state, indent=2, sort_keys=True))
        return

    session_id = "a666-ethereum-mainnet-20260727"
    call({"op": "close_launch_session", "session_id": session_id})
    opened = call(
        {
            "op": "open_launch_session",
            "session_id": session_id,
            "chain_id": CHAIN_ID,
            "allowlist": [
                DEPLOYER,
                SP1_VERIFIER,
                USDC,
                A651,
                A651_SUPPLY_CONTROLLER,
                *addresses.values(),
            ],
            "expected_deploys": [
                {
                    "label": f"deploy_a666_{name}",
                    "bytecode_hash": EXPECTED_BASE_CODE[name][0],
                    "bytecode_len": EXPECTED_BASE_CODE[name][1],
                }
                for name in ("token", "verifier", "controller", "migration")
            ],
            "usdc_address": USDC,
            "usdc_budget": 0,
            "close_after_action": "authorize_a651_migration",
            "ttl_seconds": 1800,
        }
    )
    if not opened or not opened.get("ok"):
        raise RuntimeError(f"StakeHub launch session rejected: {opened}")

    for name in ("token", "verifier", "controller", "migration"):
        response = call(
            {
                "op": "evm_contract_tx",
                "to": None,
                "data": init_codes[name],
                "rpc_url": RPC,
                "chain_id": CHAIN_ID,
                "label": f"deploy A666 {name}",
                "session_id": session_id,
                "session_action": f"deploy_a666_{name}",
                "value_wei": 0,
                "gas_usd": 0,
            },
            timeout=1200.0,
        )
        if not response or not response.get("ok"):
            raise RuntimeError(f"{name} deployment failed: {response}")
        address = to_checksum_address(response["contract_address"])
        if address != addresses[name]:
            raise RuntimeError(f"{name} deployed at unexpected address {address}")
        transaction_hash = normalize_tx_hash(response["tx"])
        receipt = web3.eth.get_transaction_receipt(transaction_hash)
        if int(receipt.status) != 1:
            raise RuntimeError(f"{name} deployment reverted")
        assert_code_hash(web3, address, EXPECTED_RUNTIME_CODE_HASHES[name], name)
        state["transactions"].append(
            {
                "action": f"deploy_a666_{name}",
                "tx": transaction_hash,
                "block_number": int(receipt.blockNumber),
                "gas_used": int(receipt.gasUsed),
                "contract_address": address,
            }
        )
        atomic_write_json(args.state_file, state)

    token = web3.eth.contract(address=addresses["token"], abi=artifacts["token"]["abi"])
    old_controller_abi = [
        {
            "type": "function",
            "name": "setPrimaryController",
            "stateMutability": "nonpayable",
            "inputs": [
                {"name": "controller", "type": "address"},
                {"name": "allowed", "type": "bool"},
            ],
            "outputs": [],
        },
        {
            "type": "function",
            "name": "primaryControllers",
            "stateMutability": "view",
            "inputs": [{"name": "", "type": "address"}],
            "outputs": [{"name": "", "type": "bool"}],
        },
    ]
    old_controller = web3.eth.contract(
        address=A651_SUPPLY_CONTROLLER, abi=old_controller_abi
    )
    calls = [
        (
            "set_a666_controller",
            addresses["token"],
            token.functions.setController(addresses["controller"])._encode_transaction_data(),
        ),
        (
            "lock_a666_controller",
            addresses["token"],
            token.functions.lockController()._encode_transaction_data(),
        ),
        (
            "authorize_a651_migration",
            A651_SUPPLY_CONTROLLER,
            old_controller.functions.setPrimaryController(
                addresses["migration"], True
            )._encode_transaction_data(),
        ),
    ]
    for action, target, data in calls:
        response = call(
            {
                "op": "evm_contract_tx",
                "to": target,
                "data": data,
                "rpc_url": RPC,
                "chain_id": CHAIN_ID,
                "label": action.replace("_", " "),
                "session_id": session_id,
                "session_action": action,
                "value_wei": 0,
                "gas_usd": 0,
            },
            timeout=1200.0,
        )
        if not response or not response.get("ok"):
            raise RuntimeError(f"{action} failed: {response}")
        transaction_hash = normalize_tx_hash(response["tx"])
        receipt = web3.eth.get_transaction_receipt(transaction_hash)
        if int(receipt.status) != 1:
            raise RuntimeError(f"{action} reverted")
        state["transactions"].append(
            {
                "action": action,
                "tx": transaction_hash,
                "block_number": int(receipt.blockNumber),
                "gas_used": int(receipt.gasUsed),
                "target": target,
            }
        )
        atomic_write_json(args.state_file, state)

    verifier = web3.eth.contract(
        address=addresses["verifier"], abi=artifacts["verifier"]["abi"]
    )
    controller = web3.eth.contract(
        address=addresses["controller"], abi=artifacts["controller"]["abi"]
    )
    migration = web3.eth.contract(
        address=addresses["migration"], abi=artifacts["migration"]["abi"]
    )
    for name, address in addresses.items():
        assert_code_hash(web3, address, EXPECTED_RUNTIME_CODE_HASHES[name], name)
    readback = {
        "token_name": token.functions.name().call(),
        "token_symbol": token.functions.symbol().call(),
        "token_decimals": token.functions.decimals().call(),
        "token_owner": token.functions.owner().call(),
        "token_controller": token.functions.controller().call(),
        "token_controller_locked": token.functions.controller_locked().call(),
        "token_total_supply": token.functions.totalSupply().call(),
        "verifier_program_vkey": Web3.to_hex(verifier.functions.programVKey().call()),
        "verifier_initial_checkpoint": Web3.to_hex(
            verifier.functions.latestCheckpointCommitment().call()
        ),
        "verifier_initial_height": verifier.functions.latestFinalizedHeight().call(),
        "controller_mint_paused": controller.functions.mintPaused().call(),
        "controller_route_supply_cap_atoms": controller.functions.routeSupplyCapAtoms().call(),
        "controller_packet_notional_cap_atoms": controller.functions.packetNotionalCapAtoms().call(),
        "controller_outstanding_atoms": controller.functions.outstandingMintedAtoms().call(),
        "migration_a666_numerator_atoms": migration.functions.a666NumeratorAtoms().call(),
        "migration_a651_denominator_atoms": migration.functions.a651DenominatorAtoms().call(),
        "migration_remaining_a666_atoms": migration.functions.remainingA666Reserve().call(),
        "migration_authorized_on_a651": old_controller.functions.primaryControllers(
            addresses["migration"]
        ).call(),
    }
    expected_readback = {
        "token_name": "Post Fiat a666",
        "token_symbol": "wA666",
        "token_decimals": 6,
        "token_owner": DEPLOYER,
        "token_controller": addresses["controller"],
        "token_controller_locked": True,
        "token_total_supply": 0,
        "verifier_program_vkey": Web3.to_hex(SP1_PROGRAM_VKEY),
        "verifier_initial_checkpoint": Web3.to_hex(
            Web3.keccak(bytes.fromhex(PFTL_INITIAL_BLOCK_ID))
        ),
        "verifier_initial_height": PFTL_INITIAL_HEIGHT,
        "controller_mint_paused": True,
        "controller_route_supply_cap_atoms": 2_000_000 * 10**6,
        "controller_packet_notional_cap_atoms": 250_000 * 10**6,
        "controller_outstanding_atoms": 0,
        "migration_a666_numerator_atoms": A666_OPENING_ATOMS,
        "migration_a651_denominator_atoms": A651_TOTAL_ATOMS,
        "migration_remaining_a666_atoms": 0,
        "migration_authorized_on_a651": True,
    }
    if readback != expected_readback:
        raise RuntimeError(f"deployment readback mismatch: {readback}")
    state.update(
        {
            "phase": "verified",
            "readback": readback,
            "end_nonce": int(web3.eth.get_transaction_count(DEPLOYER, "latest")),
            "balance_wei_after": int(web3.eth.get_balance(DEPLOYER)),
            "block_number_verified": int(web3.eth.block_number),
        }
    )
    atomic_write_json(args.state_file, state)
    print(json.dumps(state, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
