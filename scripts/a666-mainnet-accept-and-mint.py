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
import time
from typing import Any

from web3 import Web3


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "python"))
RPC = "https://ethereum-rpc.publicnode.com"
CHAIN_ID = 1
ROUTE_ID = "pftl-a666-ethereum-wA666-usdc-v1"
ROUTE_CONFIG_DIGEST = "12ed00ca87e29554ce4b978da1710fffc0830767e84e62f08df257f727db953efdd89bcf6ea99f5634d6e5ea8aca2933"
MAXIMUM_FEE_WEI = int(os.environ.get("POSTFIAT_SIGNER_MAXIMUM_FEE_WEI", "10000000000000000"))
MUTATION_NOT_AFTER_EPOCH = int(os.environ.get("POSTFIAT_MUTATION_NOT_AFTER_EPOCH", "0"))
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


def enforce_mutation_deadline(label: str) -> None:
    if MUTATION_NOT_AFTER_EPOCH and int(time.time()) >= MUTATION_NOT_AFTER_EPOCH:
        raise RuntimeError(
            f"{label}: mutation deadline margin reached at epoch "
            f"{MUTATION_NOT_AFTER_EPOCH}"
        )


def send(call: Any, web3: Web3, label: str) -> dict[str, Any]:
    calldata = call._encode_transaction_data()
    gas_estimate = int(
        web3.eth.estimate_gas(
            {"from": OWNER, "to": call.address, "data": calldata, "value": 0}
        )
    )
    idempotency_key = "a666-" + hashlib.sha256(
        f"{ROUTE_CONFIG_DIGEST}:{call.address.lower()}:{calldata.lower()}".encode()
    ).hexdigest()
    enforce_mutation_deadline(label)
    stakehub_repo = Path(
        os.environ.get("A666_STAKEHUB_REPO", "/home/postfiat/repos/StakeHub-master-e6")
    )
    sys.path.insert(0, str(stakehub_repo))
    from stakehub.agentd import call as agentd_call

    response = agentd_call(
        {
            "op": "evm_contract_tx",
            "to": call.address,
            "data": calldata,
            "rpc_url": RPC,
            "chain_id": CHAIN_ID,
            "label": f"{label}-{idempotency_key[5:21]}",
            "value_wei": 0,
            "gas_usd": 0,
        },
        timeout=1200,
    )
    if not response or response.get("ok") is not True:
        raise RuntimeError(f"StakeHub agent rejected {label}: {response}")
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
    parser.add_argument("--packet", type=Path)
    parser.add_argument("--receipt-witness", type=Path)
    parser.add_argument("--proof-dir", type=Path, default=PROOF_DIR)
    parser.add_argument("--state-file", type=Path, default=STATE_PATH)
    parser.add_argument("--expected-finalized-height", type=int, default=348)
    args = parser.parse_args()

    receipt_witness_sha256 = None
    if args.receipt_witness is not None:
        receipt_witness_bytes = args.receipt_witness.read_bytes()
        receipt_witness = json.loads(receipt_witness_bytes)
        packet = receipt_witness["mint_packet"]
        receipt = receipt_witness["receipt"]
        block_header = receipt_witness["block"]["header"]
        zero_hash48 = "00" * 48
        if (
            packet["source_receipt_hash"] == zero_hash48
            or packet["source_receipt_root"] == zero_hash48
            or packet["source_receipt_hash"] != receipt["receipt_hash"]
            or packet["source_receipt_root"]
            != block_header["pftl_uniswap_receipt_root"]
            or packet["source_packet_hash"] != receipt["packet_hash"]
            or int(packet["mint_amount_atoms"]) != int(receipt["amount_atoms"])
        ):
            raise RuntimeError(
                "receipt witness does not contain one internally consistent finalized mint packet"
            )
        receipt_witness_sha256 = hashlib.sha256(receipt_witness_bytes).hexdigest()
    else:
        if args.execute:
            raise RuntimeError(
                "--execute requires --receipt-witness; refusing a pre-export packet template"
            )
        packet_path = args.packet or PACKET_PATH
        packet = json.loads(packet_path.read_text())
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
    recipient = Web3.to_checksum_address(packet["ethereum_recipient"])

    receipt_commitment = verifier.functions.receiptCommitment(
        mint_packet[4], mint_packet[3], mint_packet[0], bytes.fromhex(packet_digest[2:])
    ).call()
    prepared_state: dict[str, Any] = {
        "schema": "postfiat-a666-proof-gated-mint-mainnet-v1",
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
        "receipt_witness_sha256": receipt_witness_sha256,
        "proof": {
            "public_values_bytes": len(public_values),
            "public_values_sha256": hashlib.sha256(public_values).hexdigest(),
            "proof_bytes": len(proof),
            "proof_sha256": hashlib.sha256(proof).hexdigest(),
        },
        "pre_state": {
            "ethereum_block_number": int(web3.eth.block_number),
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
            "recipient_balance_atoms": int(token.functions.balanceOf(recipient).call()),
            "migration_reserve_atoms": int(
                migration.functions.remainingA666Reserve().call()
            ),
        },
        "transactions": [],
    }
    if args.state_file.exists():
        state = json.loads(args.state_file.read_text())
        if not (
            state.get("schema") == prepared_state["schema"]
            and str(state.get("packet_digest", "")).lower() == packet_digest.lower()
            and state.get("receipt_commitment") == prepared_state["receipt_commitment"]
            and state.get("receipt_witness_sha256") == receipt_witness_sha256
            and state.get("proof") == prepared_state["proof"]
        ):
            raise RuntimeError("existing mint state does not match this proof and packet")
    else:
        state = prepared_state
        atomic_write_json(args.state_file, state)
    if not args.execute:
        print(json.dumps(state, indent=2, sort_keys=True))
        return

    if state["pre_state"]["mint_paused"]:
        raise RuntimeError(
            "A666 mint controller is paused; refusing to mutate governed pause state"
        )
    if not bool(verifier.functions.acceptedReceiptCommitment(receipt_commitment).call()):
        state["transactions"].append(
            send(
                verifier.functions.verifyAndAccept(public_values, proof),
                web3,
                "accept finalized A666 export proof",
            )
        )
        atomic_write_json(args.state_file, state)
    if not bool(verifier.functions.acceptedReceiptCommitment(receipt_commitment).call()):
        raise RuntimeError(
            "accepted SP1 proof does not authorize the selected finalized mint packet"
        )
    if bool(controller.functions.mintPaused().call()):
        raise RuntimeError(
            "A666 mint controller became paused during execution; refusing to unpause"
        )
    if not bool(
        controller.functions.consumedPacket(bytes.fromhex(packet_digest[2:])).call()
    ):
        state["transactions"].append(
            send(
                controller.functions.consumeMintOnly(mint_packet),
                web3,
                "consume finalized A666 mint packet",
            )
        )
        atomic_write_json(args.state_file, state)

    if not any(
        item.get("label") == "consume finalized A666 mint packet"
        for item in state["transactions"]
    ):
        event_signature = Web3.keccak(
            text="PacketConsumed(bytes32,bytes32,bytes32,address,uint256)"
        ).hex()
        logs = web3.eth.get_logs(
            {
                "fromBlock": int(state["pre_state"].get("ethereum_block_number", 0)),
                "toBlock": "latest",
                "address": CONTROLLER,
                "topics": [event_signature, packet_digest],
            }
        )
        if len(logs) != 1:
            raise RuntimeError("could not uniquely recover the finalized mint transaction")
        recovered = web3.eth.get_transaction_receipt(logs[0]["transactionHash"])
        state["transactions"].append(
            {
                "label": "consume finalized A666 mint packet",
                "tx": Web3.to_hex(recovered.transactionHash),
                "block_number": int(recovered.blockNumber),
                "gas_estimate": None,
                "gas_used": int(recovered.gasUsed),
                "status": int(recovered.status),
                "recovered_after_restart": True,
            }
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
        "recipient_balance_atoms": int(token.functions.balanceOf(recipient).call()),
        "migration_reserve_atoms": int(migration.functions.remainingA666Reserve().call()),
    }
    expected_atoms = int(packet["mint_amount_atoms"])
    pre_state = state["pre_state"]
    if (
        not post_state["receipt_accepted"]
        or post_state["latest_finalized_height"] < args.expected_finalized_height
        or post_state["mint_paused"]
        or not post_state["packet_consumed"]
        or post_state["total_minted_atoms"] - pre_state["total_minted_atoms"]
        != expected_atoms
        or post_state["token_total_supply"] - pre_state["token_total_supply"]
        != expected_atoms
        or post_state["recipient_balance_atoms"] - pre_state["recipient_balance_atoms"]
        != expected_atoms
        or post_state["migration_reserve_atoms"] != pre_state["migration_reserve_atoms"]
    ):
        raise RuntimeError(f"A666 mint post-state mismatch: {post_state}")
    state.update({"phase": "minted-to-recipient", "post_state": post_state})
    atomic_write_json(args.state_file, state)
    print(json.dumps(state, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
