#!/usr/bin/env python3
"""Burn exact mainnet wA666 for a proof-gated return to PFTL."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import sys
from typing import Any

from web3 import Web3


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "python"))
ARTIFACT_ROOT = Path(os.environ.get("A666_CONTRACT_ARTIFACT_ROOT", ROOT))

RPC = os.environ.get("A666_ETHEREUM_RPC", "https://ethereum-rpc.publicnode.com")
CHAIN_ID = 1
ROUTE_ID = "pftl-a666-ethereum-wA666-usdc-v1"
ROUTE_CONFIG_DIGEST = "12ed00ca87e29554ce4b978da1710fffc0830767e84e62f08df257f727db953efdd89bcf6ea99f5634d6e5ea8aca2933"
MAXIMUM_FEE_WEI = int(os.environ.get("POSTFIAT_SIGNER_MAXIMUM_FEE_WEI", "10000000000000000"))
DEFAULT_OWNER = Web3.to_checksum_address("0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0")
CONTROLLER = Web3.to_checksum_address("0x9A0262C0572fb4DB08765408eB225E207F40c3d9")
TOKEN = Web3.to_checksum_address("0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5")
DEFAULT_PFTL_RECIPIENT = "pfab9b9228942e5c529633a13aa271d5297bec6353"
A666 = (
    "521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452"
    "da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c"
)


def validate_return_capacity(path: Path, amount_atoms: int) -> dict[str, Any]:
    """Fail closed unless PFTL can import the proposed wrapped-token burn."""
    status = json.loads(path.read_text())
    expected = {
        "route_id": ROUTE_ID,
        "handoff_controller": CONTROLLER.lower(),
        "wrapped_navcoin_token": TOKEN.lower(),
        "native_nav_asset_id": A666,
    }
    for field, value in expected.items():
        actual = status.get(field)
        if field in {"handoff_controller", "wrapped_navcoin_token"} and isinstance(
            actual, str
        ):
            actual = actual.lower()
        if actual != value:
            raise RuntimeError(f"PFTL supply status {field} binding mismatch")
    if status.get("invariant_holds") is not True:
        raise RuntimeError("PFTL supply invariant does not hold")
    if status.get("paused") is not False or status.get("live_value_enabled") is not True:
        raise RuntimeError("PFTL return route is not active")
    ethereum_spendable = status.get("ethereum_spendable_supply_atoms")
    capacity = status.get("available_return_import_atoms", ethereum_spendable)
    if not isinstance(capacity, int) or isinstance(capacity, bool) or capacity < 0:
        raise RuntimeError("PFTL return-import capacity is missing or invalid")
    if (
        not isinstance(ethereum_spendable, int)
        or isinstance(ethereum_spendable, bool)
        or ethereum_spendable < 0
        or capacity != ethereum_spendable
    ):
        raise RuntimeError(
            "PFTL return-import capacity does not match Ethereum spendable supply"
        )
    if capacity < amount_atoms:
        raise RuntimeError(
            "insufficient PFTL return-import capacity: "
            f"requested {amount_atoms} atoms, available {capacity} atoms"
        )
    return status


def artifact(source: str, contract: str) -> dict[str, Any]:
    path = ARTIFACT_ROOT / f"crates/ethereum-contracts/out/{source}/{contract}.json"
    return json.loads(path.read_text())


def normalize_tx_hash(value: str) -> str:
    return value if value.startswith("0x") else f"0x{value}"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--transaction-hash")
    parser.add_argument("--amount-atoms", type=int, default=1_000_000)
    parser.add_argument("--return-nonce", required=True)
    parser.add_argument("--ethereum-sender", default=DEFAULT_OWNER)
    parser.add_argument("--pftl-recipient", default=DEFAULT_PFTL_RECIPIENT)
    parser.add_argument(
        "--pftl-supply-status",
        type=Path,
        help="fresh navcoin-bridge-supply-status JSON required for --execute",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="optional non-overwriting JSON evidence output",
    )
    args = parser.parse_args()
    if args.output is not None and args.output.exists():
        raise RuntimeError(f"refusing to overwrite {args.output}")

    def emit(value: dict[str, Any]) -> None:
        serialized = json.dumps(value, indent=2, sort_keys=True) + "\n"
        if args.output is not None:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(serialized)
        print(serialized, end="")

    if args.execute and args.transaction_hash:
        raise RuntimeError("--execute and --transaction-hash are mutually exclusive")
    nonce = bytes.fromhex(args.return_nonce.removeprefix("0x"))
    if len(nonce) != 32 or nonce == bytes(32):
        raise RuntimeError("--return-nonce must be one nonzero bytes32")
    if args.amount_atoms <= 0:
        raise RuntimeError("--amount-atoms must be positive")
    if args.execute and args.pftl_supply_status is None:
        raise RuntimeError("--execute requires --pftl-supply-status")
    supply_status = (
        validate_return_capacity(args.pftl_supply_status, args.amount_atoms)
        if args.pftl_supply_status is not None
        else None
    )
    owner = Web3.to_checksum_address(args.ethereum_sender)
    pftl_recipient = args.pftl_recipient.strip().lower()
    if not pftl_recipient.startswith("pf") or len(pftl_recipient) != 42:
        raise RuntimeError("--pftl-recipient must be one canonical PFTL account")

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
        pftl_recipient,
        bytes.fromhex(A666),
        nonce,
    )
    def state_at(block_identifier: int | str = "latest") -> dict[str, Any]:
        return {
            "recipient_balance_atoms": int(
                    token.functions.balanceOf(owner).call(
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

    pre = None if args.transaction_hash else state_at()
    gas_estimate = None
    if not args.transaction_hash:
        gas_estimate = int(
            web3.eth.estimate_gas(
                {
                    "from": owner,
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
        "ethereum_sender": owner,
        "pftl_recipient": pftl_recipient,
        "native_nav_asset_id": A666,
        "amount_atoms": args.amount_atoms,
        "return_nonce": nonce.hex(),
        "gas_estimate": gas_estimate,
        "pftl_return_import_capacity_atoms": (
            supply_status.get(
                "available_return_import_atoms",
                supply_status["ethereum_spendable_supply_atoms"],
            )
            if supply_status is not None
            else None
        ),
        "pre_state": pre,
    }
    if not args.execute and not args.transaction_hash:
        emit(report)
        return
    if args.transaction_hash:
        transaction_hash = normalize_tx_hash(args.transaction_hash)
    else:
        if pre["nonce_consumed"]:
            raise RuntimeError("return nonce is already consumed")
        if pre["recipient_balance_atoms"] < args.amount_atoms:
            raise RuntimeError("insufficient wA666 balance")

        calldata = call._encode_transaction_data()
        idempotency_key = "a666-return-" + hashlib.sha256(
            f"{ROUTE_CONFIG_DIGEST}:{CONTROLLER.lower()}:{calldata.lower()}".encode()
        ).hexdigest()
        stakehub_repo = Path(
            os.environ.get(
                "A666_STAKEHUB_REPO", "/home/postfiat/repos/StakeHub-master-e6"
            )
        )
        sys.path.insert(0, str(stakehub_repo))
        from stakehub.agentd import call as agentd_call

        response = agentd_call(
            {
                "op": "evm_contract_tx",
                "to": CONTROLLER,
                "data": calldata,
                "rpc_url": RPC,
                "chain_id": CHAIN_ID,
                "label": f"burn proof-minted wA666 for PFTL return-{idempotency_key[-16:]}",
                "value_wei": 0,
                "gas_usd": 0,
            },
            timeout=1200,
        )
        if not response or response.get("ok") is not True:
            raise RuntimeError(f"StakeHub agent rejected return burn: {response}")
        transaction_hash = normalize_tx_hash(response["tx"])
    receipt = web3.eth.get_transaction_receipt(transaction_hash)
    if int(receipt.status) != 1:
        raise RuntimeError(f"return burn reverted: {transaction_hash}")
    if args.transaction_hash:
        transaction = web3.eth.get_transaction(transaction_hash)
        if (
            Web3.to_checksum_address(transaction["from"]) != owner
            or Web3.to_checksum_address(transaction["to"]) != CONTROLLER
            or transaction["input"].hex().lower()
            != call._encode_transaction_data().removeprefix("0x").lower()
        ):
            raise RuntimeError("return burn transaction calldata binding mismatch")
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
                owner,
                pftl_recipient,
            args.amount_atoms,
            nonce,
            block_number,
        ],
    )
    expected_burn_id = Web3.keccak(canonical_preimage)
    if (
        event["returnBurnId"] != expected_burn_id
        or Web3.to_checksum_address(event["ethereumSender"]) != owner
        or event["returnNonce"] != nonce
        or event["pftlRecipient"] != pftl_recipient
        or int(event["amountAtoms"]) != args.amount_atoms
    ):
        raise RuntimeError("ReturnBurned event does not match canonical burn preimage")
    post = state_at("latest" if args.transaction_hash else block_number)
    if args.transaction_hash:
        if not post["nonce_consumed"] or post["total_return_burned_atoms"] < args.amount_atoms:
            raise RuntimeError("return burn current state mismatch")
    elif (
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
    emit(report)


if __name__ == "__main__":
    main()
