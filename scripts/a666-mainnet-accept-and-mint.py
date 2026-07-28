#!/usr/bin/env python3
"""Accept the finalized A666 SP1 receipt and mint its exact Ethereum packet."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
from typing import Any

from web3 import Web3


ROOT = Path(__file__).resolve().parents[1]
STAKEHUB = Path("/home/postfiat/repos/StakeHub")
RPC = "https://ethereum-rpc.publicnode.com"
CHAIN_ID = 1
OWNER = Web3.to_checksum_address("0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0")
VERIFIER = Web3.to_checksum_address("0xb79FF97EcC11574a8A78d0b5a9D7C8c2A94bF96A")
CONTROLLER = Web3.to_checksum_address("0x9A0262C0572fb4DB08765408eB225E207F40c3d9")
TOKEN = Web3.to_checksum_address("0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5")
MIGRATION = Web3.to_checksum_address("0xFfBBae0eb8450D10F8C0D26C2952b089D56e517c")
PACKET_PATH = ROOT / "deployments/a666-mainnet-20260727/13-opening-export-proof/resolved-mint-packet.json"
PROOF_DIR = ROOT / "deployments/a666-mainnet-20260727/13-opening-export-proof/groth16-deployed-vkey"
STATE_PATH = ROOT / "deployments/a666-mainnet-20260727/ethereum/opening-mint-state.json"


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


def artifact(source: str, contract: str) -> dict[str, Any]:
    path = ROOT / f"crates/ethereum-contracts/out/{source}/{contract}.json"
    return json.loads(path.read_text())


def pftl_bytes(value: str) -> bytes:
    decoded = bytes.fromhex(value.removeprefix("0x"))
    if len(decoded) != 48:
        raise RuntimeError("PFTL commitment must be exactly 48 bytes")
    return decoded


def normalize_tx_hash(value: str) -> str:
    return value if value.startswith("0x") else f"0x{value}"


def send(call: Any, web3: Web3, label: str) -> dict[str, Any]:
    calldata = call._encode_transaction_data()
    gas_estimate = int(
        web3.eth.estimate_gas(
            {"from": OWNER, "to": call.address, "data": calldata, "value": 0}
        )
    )
    sys.path.insert(0, str(STAKEHUB))
    from stakehub.agentd import call as agent_call

    response = agent_call(
        {
            "op": "evm_contract_tx",
            "to": call.address,
            "data": calldata,
            "rpc_url": RPC,
            "chain_id": CHAIN_ID,
            "label": label,
            "value_wei": 0,
            "gas_usd": 0,
        },
        timeout=1200.0,
    )
    if not response or not response.get("ok"):
        raise RuntimeError(f"{label} rejected: {response}")
    transaction_hash = normalize_tx_hash(response["tx"])
    receipt = web3.eth.get_transaction_receipt(transaction_hash)
    if int(receipt.status) != 1:
        raise RuntimeError(f"{label} reverted: {transaction_hash}")
    return {
        "label": label,
        "tx": transaction_hash,
        "block_number": int(receipt.blockNumber),
        "gas_estimate": gas_estimate,
        "gas_used": int(receipt.gasUsed),
        "status": int(receipt.status),
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--packet", type=Path, default=PACKET_PATH)
    parser.add_argument("--proof-dir", type=Path, default=PROOF_DIR)
    parser.add_argument("--state-file", type=Path, default=STATE_PATH)
    args = parser.parse_args()

    packet = json.loads(args.packet.read_text())
    public_values = (args.proof_dir / "public-values.bin").read_bytes()
    proof = (args.proof_dir / "proof-calldata.bin").read_bytes()
    if len(public_values) != 1120 or not proof:
        raise RuntimeError("missing or malformed Groth16 artifacts")
    if int(packet["deadline_seconds"]) <= int(__import__("time").time()):
        raise RuntimeError("mint packet deadline has expired")

    web3 = Web3(Web3.HTTPProvider(RPC, request_kwargs={"timeout": 120}))
    if not web3.is_connected() or int(web3.eth.chain_id) != CHAIN_ID:
        raise RuntimeError("Ethereum mainnet RPC unavailable or wrong chain")

    verifier = web3.eth.contract(
        address=VERIFIER,
        abi=artifact(
            "PFTLReceiptFinalityVerifierV1.sol", "PFTLReceiptFinalityVerifierV1"
        )["abi"],
    )
    controller = web3.eth.contract(
        address=CONTROLLER,
        abi=artifact("PFTLUniswapPrimaryMarketV2.sol", "PFTLUniswapPrimaryMarketV2")[
            "abi"
        ],
    )
    token = web3.eth.contract(
        address=TOKEN,
        abi=artifact("PFTLUniswapHandoffController.sol", "WrappedVenueNAVCoin")[
            "abi"
        ],
    )
    migration = web3.eth.contract(
        address=MIGRATION,
        abi=artifact("A651ToA666MigrationV1.sol", "A651ToA666MigrationV1")["abi"],
    )

    mint_packet = (
        pftl_bytes(packet["route_config_digest"]),
        pftl_bytes(packet["source_packet_hash"]),
        pftl_bytes(packet["reservation_id"]),
        pftl_bytes(packet["source_receipt_hash"]),
        pftl_bytes(packet["source_receipt_root"]),
        pftl_bytes(packet["settlement_asset_id"]),
        pftl_bytes(packet["native_nav_asset_id"]),
        pftl_bytes(packet["pricing_reserve_packet_hash"]),
        bytes.fromhex(packet["policy_hash_commitment"].removeprefix("0x")),
        int(packet["route_epoch"]),
        int(packet["pricing_nav_epoch"]),
        int(packet["deadline_seconds"]),
        bytes.fromhex(packet["nonce"].removeprefix("0x")),
        int(packet["destination_chain_id"]),
        Web3.to_checksum_address(packet["destination_controller"]),
        Web3.to_checksum_address(packet["wrapped_token"]),
        Web3.to_checksum_address(packet["ethereum_recipient"]),
        int(packet["mint_amount_atoms"]),
        int(packet["settlement_value_atoms"]),
    )
    packet_digest = Web3.to_hex(controller.functions.packetDigest(mint_packet).call())
    expected_digest = "0x3f4a57859cd56bd2978d709aa5671f0651cff3ad72fd1272c6abee6f9bc48798"
    if packet_digest.lower() != expected_digest:
        raise RuntimeError(f"packet digest drift: {packet_digest}")

    receipt_commitment = verifier.functions.receiptCommitment(
        mint_packet[4], mint_packet[3], mint_packet[0], bytes.fromhex(packet_digest[2:])
    ).call()
    state: dict[str, Any] = {
        "schema": "postfiat-a666-opening-mint-mainnet-v1",
        "phase": "prepared",
        "chain_id": CHAIN_ID,
        "rpc": RPC,
        "addresses": {
            "owner": OWNER,
            "verifier": VERIFIER,
            "controller": CONTROLLER,
            "token": TOKEN,
            "migration": MIGRATION,
        },
        "packet_digest": packet_digest,
        "receipt_commitment": Web3.to_hex(receipt_commitment),
        "proof": {
            "public_values_bytes": len(public_values),
            "public_values_sha256": hashlib.sha256(public_values).hexdigest(),
            "proof_bytes": len(proof),
            "proof_sha256": hashlib.sha256(proof).hexdigest(),
        },
        "pre_state": {
            "receipt_accepted": bool(
                verifier.functions.acceptedReceiptCommitment(receipt_commitment).call()
            ),
            "latest_finalized_height": int(
                verifier.functions.latestFinalizedHeight().call()
            ),
            "mint_paused": bool(controller.functions.mintPaused().call()),
            "packet_consumed": bool(
                controller.functions.consumedPacket(bytes.fromhex(packet_digest[2:])).call()
            ),
            "total_minted_atoms": int(controller.functions.totalMintedAtoms().call()),
            "token_total_supply": int(token.functions.totalSupply().call()),
            "migration_reserve_atoms": int(
                migration.functions.remainingA666Reserve().call()
            ),
        },
        "transactions": [],
    }
    atomic_write_json(args.state_file, state)
    if not args.execute:
        print(json.dumps(state, indent=2, sort_keys=True))
        return

    if not state["pre_state"]["receipt_accepted"]:
        state["transactions"].append(
            send(
                verifier.functions.verifyAndAccept(public_values, proof),
                web3,
                "accept finalized A666 opening export proof",
            )
        )
        atomic_write_json(args.state_file, state)
    if bool(controller.functions.mintPaused().call()):
        state["transactions"].append(
            send(
                controller.functions.setMintPaused(False),
                web3,
                "unpause proof-gated A666 mint controller",
            )
        )
        atomic_write_json(args.state_file, state)
    if not bool(
        controller.functions.consumedPacket(bytes.fromhex(packet_digest[2:])).call()
    ):
        state["transactions"].append(
            send(
                controller.functions.consumeMintOnly(mint_packet),
                web3,
                "consume finalized A666 opening mint packet",
            )
        )
        atomic_write_json(args.state_file, state)

    post_state = {
        "receipt_accepted": bool(
            verifier.functions.acceptedReceiptCommitment(receipt_commitment).call()
        ),
        "latest_finalized_height": int(verifier.functions.latestFinalizedHeight().call()),
        "mint_paused": bool(controller.functions.mintPaused().call()),
        "packet_consumed": bool(
            controller.functions.consumedPacket(bytes.fromhex(packet_digest[2:])).call()
        ),
        "total_minted_atoms": int(controller.functions.totalMintedAtoms().call()),
        "token_total_supply": int(token.functions.totalSupply().call()),
        "migration_reserve_atoms": int(migration.functions.remainingA666Reserve().call()),
    }
    expected_atoms = int(packet["mint_amount_atoms"])
    if (
        not post_state["receipt_accepted"]
        or post_state["latest_finalized_height"] != 348
        or post_state["mint_paused"]
        or not post_state["packet_consumed"]
        or post_state["total_minted_atoms"] != expected_atoms
        or post_state["token_total_supply"] != expected_atoms
        or post_state["migration_reserve_atoms"] != expected_atoms
    ):
        raise RuntimeError(f"opening mint post-state mismatch: {post_state}")
    state.update({"phase": "minted-to-migration", "post_state": post_state})
    atomic_write_json(args.state_file, state)
    print(json.dumps(state, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
