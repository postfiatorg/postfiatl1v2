#!/usr/bin/env python3
"""Execute and verify the fresh 25 USDC epoch-4 Ethereum deposit.

The StakeHub agent signs both transactions. This process handles only public
calldata, transaction hashes, receipts, event fields, and on-chain readback.
"""
from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import sys
import time
from datetime import UTC, datetime
from pathlib import Path

from eth_abi import encode
from web3 import Web3


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[5]
BUILDER = (
    REPO
    / "docs/evidence/pfusdc-eth-campaign-20260725/lane-c/rev6-execution/06-h315-claim"
    / "h315-evm-deposit.py"
)
MANIFEST = HERE.parent / "deploy" / "manifest.postdeploy-enriched.json"
EXPECTED_MANIFEST_SHA256 = "6f6763b4031fac5f3e8f952752499e21cbb2f9d737b4c025855d957057dbc908"

CHAIN_ID = 1
RPC = "https://ethereum-rpc.publicnode.com"
VAULT = "0x8583409ddbac984ec195dfa06a21103d92403c1e"
VERIFIER = "0xa77d5af456ef212303e31727b6ca4888cd771e2c"
USDC = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
WALLET = "0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0"
RECIPIENT = "pf23d8831301aa1cce6fdd7bf4a2db2aead1619ba8"
AMOUNT_ATOMS = 25_000_000
ROUTE_BINDING = "f062a7b19a33ab9674457b0fd7b8c98a42f43c240806e3a25ef035da3fa643f3"
EXPECTED_VAULT_RUNTIME_KECCAK = "c6dbb722c23bfc841624bb909fcb54d84a65a2ea6ece96e2a28bf61d5dea6d05"

USDC_ABI = [
    {
        "name": "approve",
        "type": "function",
        "stateMutability": "nonpayable",
        "inputs": [
            {"name": "spender", "type": "address"},
            {"name": "value", "type": "uint256"},
        ],
        "outputs": [{"name": "", "type": "bool"}],
    },
    {
        "name": "balanceOf",
        "type": "function",
        "stateMutability": "view",
        "inputs": [{"name": "account", "type": "address"}],
        "outputs": [{"name": "", "type": "uint256"}],
    },
    {
        "name": "allowance",
        "type": "function",
        "stateMutability": "view",
        "inputs": [
            {"name": "owner", "type": "address"},
            {"name": "spender", "type": "address"},
        ],
        "outputs": [{"name": "", "type": "uint256"}],
    },
]

VAULT_ABI = [
    {
        "anonymous": False,
        "name": "ERC20BridgeDepositedV2",
        "type": "event",
        "inputs": [
            {"indexed": True, "name": "depositId", "type": "bytes32"},
            {"indexed": True, "name": "depositor", "type": "address"},
            {"indexed": True, "name": "pftlRecipientHash", "type": "bytes32"},
            {"indexed": False, "name": "pftlRecipient", "type": "string"},
            {"indexed": False, "name": "amount", "type": "uint256"},
            {"indexed": False, "name": "nonce", "type": "bytes32"},
            {"indexed": False, "name": "routeBinding", "type": "bytes32"},
            {"indexed": False, "name": "sourceChainId", "type": "uint256"},
            {"indexed": False, "name": "vault", "type": "address"},
            {"indexed": False, "name": "token", "type": "address"},
        ],
    },
    {
        "name": "depositRecords",
        "type": "function",
        "stateMutability": "view",
        "inputs": [{"name": "", "type": "bytes32"}],
        "outputs": [
            {"name": "depositor", "type": "address"},
            {"name": "amount", "type": "uint96"},
            {"name": "recipientHash", "type": "bytes32"},
            {"name": "routeBinding", "type": "bytes32"},
            {"name": "nonce", "type": "bytes32"},
        ],
    },
    {
        "name": "totalObligations",
        "type": "function",
        "stateMutability": "view",
        "inputs": [],
        "outputs": [{"name": "", "type": "uint256"}],
    },
    {
        "name": "paused",
        "type": "function",
        "stateMutability": "view",
        "inputs": [],
        "outputs": [{"name": "", "type": "bool"}],
    },
    {
        "name": "finalityVerifier",
        "type": "function",
        "stateMutability": "view",
        "inputs": [],
        "outputs": [{"name": "", "type": "address"}],
    },
]


def fail(message: str) -> None:
    raise RuntimeError(message)


def load_builder() -> object:
    spec = importlib.util.spec_from_file_location("epoch4_strict_deposit_builder", BUILDER)
    if spec is None or spec.loader is None:
        fail(f"cannot load strict builder {BUILDER}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def wait_receipt(w3: Web3, tx_hash: str) -> object:
    for _ in range(120):
        try:
            receipt = w3.eth.get_transaction_receipt(tx_hash)
        except Exception:
            receipt = None
        if receipt is not None:
            return receipt
        time.sleep(5)
    fail(f"receipt timeout for {tx_hash}")


def public_agent_response(response: dict[str, object]) -> dict[str, object]:
    allowed = {"ok", "tx", "tx_hash", "transaction_hash", "gas_used", "charged_usd", "value_wei"}
    return {key: value for key, value in response.items() if key in allowed}


def tx_hash(response: dict[str, object]) -> str:
    value = response.get("tx") or response.get("tx_hash") or response.get("transaction_hash")
    if not isinstance(value, str) or not value:
        fail(f"agent response omitted transaction hash: {sorted(response)}")
    return value if value.startswith("0x") else "0x" + value


def main() -> None:
    manifest_digest = hashlib.sha256(MANIFEST.read_bytes()).hexdigest()
    if manifest_digest != EXPECTED_MANIFEST_SHA256:
        fail(f"postdeploy manifest digest {manifest_digest} != authorized digest")

    sys.path.insert(0, str(REPO.parent / "StakeHub"))
    from stakehub.agentd import call

    w3 = Web3(Web3.HTTPProvider(RPC, request_kwargs={"timeout": 30}))
    if not w3.is_connected() or w3.eth.chain_id != CHAIN_ID:
        fail("Ethereum mainnet RPC is unavailable or returned the wrong chain")

    vault_address = Web3.to_checksum_address(VAULT)
    verifier_address = Web3.to_checksum_address(VERIFIER)
    usdc_address = Web3.to_checksum_address(USDC)
    wallet_address = Web3.to_checksum_address(WALLET)
    code_hash = Web3.keccak(w3.eth.get_code(vault_address)).hex()
    if code_hash.removeprefix("0x") != EXPECTED_VAULT_RUNTIME_KECCAK:
        fail(f"vault runtime hash {code_hash} != deployed manifest")

    usdc = w3.eth.contract(address=usdc_address, abi=USDC_ABI)
    vault = w3.eth.contract(address=vault_address, abi=VAULT_ABI)
    if vault.functions.paused().call():
        fail("vault is paused")
    if vault.functions.finalityVerifier().call() != verifier_address:
        fail("vault finality verifier differs from the authorized epoch-4 verifier")

    before = {
        "wallet_usdc_atoms": usdc.functions.balanceOf(wallet_address).call(),
        "vault_usdc_atoms": usdc.functions.balanceOf(vault_address).call(),
        "allowance_atoms": usdc.functions.allowance(wallet_address, vault_address).call(),
        "total_obligations_atoms": vault.functions.totalObligations().call(),
        "wallet_eth_wei": w3.eth.get_balance(wallet_address),
    }
    if before["wallet_usdc_atoms"] < AMOUNT_ATOMS:
        fail("wallet has insufficient USDC")

    builder = load_builder()
    nonce = os.urandom(32)
    route_binding = bytes.fromhex(ROUTE_BINDING)
    calldata = builder.build_deposit_v2_calldata(
        w3.codec, AMOUNT_ATOMS, RECIPIENT, nonce, route_binding
    )
    builder.assert_deposit_v2_calldata(
        calldata, AMOUNT_ATOMS, RECIPIENT, nonce, route_binding
    )
    recipient_hash = Web3.keccak(text=RECIPIENT)
    predicted_deposit_id = Web3.keccak(
        encode(
            [
                "string",
                "uint256",
                "address",
                "address",
                "address",
                "uint256",
                "bytes32",
                "bytes32",
                "bytes32",
            ],
            [
                "postfiat.erc20_bridge.deposit.v2",
                CHAIN_ID,
                vault_address,
                usdc_address,
                wallet_address,
                AMOUNT_ATOMS,
                recipient_hash,
                nonce,
                route_binding,
            ],
        )
    )
    approve_calldata = usdc.encode_abi("approve", args=[vault_address, AMOUNT_ATOMS])

    session_id = f"pfusdc-mainnet-epoch4-deposit-{datetime.now(UTC).strftime('%Y%m%dT%H%M%SZ')}"
    open_response = call(
        {
            "op": "open_launch_session",
            "session_id": session_id,
            "chain_id": CHAIN_ID,
            "allowlist": [WALLET, USDC, VAULT],
            "expected_deploys": [
                {
                    "label": "epoch4-approve-calldata-frozen",
                    "bytecode_hash": Web3.to_hex(Web3.keccak(hexstr=approve_calldata)),
                    "bytecode_len": len(bytes.fromhex(approve_calldata[2:])),
                },
                {
                    "label": "epoch4-deposit-calldata-frozen",
                    "bytecode_hash": Web3.to_hex(Web3.keccak(hexstr=calldata)),
                    "bytecode_len": len(bytes.fromhex(calldata[2:])),
                },
            ],
            "usdc_address": USDC,
            "usdc_budget": AMOUNT_ATOMS,
            "close_after_action": "deposit",
            "ttl_seconds": 1800,
        },
        timeout=60.0,
    )
    if not open_response or not open_response.get("ok"):
        fail(f"open launch session failed: {open_response}")

    evidence: dict[str, object] = {
        "schema": "postfiat.pfusdc.mainnet_epoch4_deposit.v1",
        "verdict": "PENDING",
        "manifest_sha256": manifest_digest,
        "session_id": session_id,
        "chain_id": CHAIN_ID,
        "vault": VAULT,
        "verifier": VERIFIER,
        "usdc": USDC,
        "depositor": WALLET,
        "pftl_recipient": RECIPIENT,
        "amount_atoms": AMOUNT_ATOMS,
        "nonce": Web3.to_hex(nonce),
        "route_binding": "0x" + ROUTE_BINDING,
        "recipient_hash": Web3.to_hex(recipient_hash),
        "predicted_deposit_id": Web3.to_hex(predicted_deposit_id),
        "deposit_calldata": calldata,
        "deposit_calldata_sha256": hashlib.sha256(bytes.fromhex(calldata[2:])).hexdigest(),
        "pre_state": before,
        "agent_open_session": public_agent_response(open_response),
    }

    try:
        approve_response = call(
            {
                "op": "evm_contract_tx",
                "to": USDC,
                "data": approve_calldata,
                "rpc_url": RPC,
                "chain_id": CHAIN_ID,
                "session_id": session_id,
                "session_action": "approve",
                "label": "epoch4 approve 25 USDC",
                "gas_usd": 10,
            },
            timeout=1200.0,
        )
        if not approve_response or not approve_response.get("ok"):
            fail(f"agent approve failed: {approve_response}")
        approve_hash = tx_hash(approve_response)
        approve_receipt = wait_receipt(w3, approve_hash)
        if approve_receipt.status != 1:
            fail(f"approve reverted: {approve_hash}")
        evidence["approve"] = {
            "response": public_agent_response(approve_response),
            "tx_hash": approve_hash,
            "block_number": approve_receipt.blockNumber,
            "gas_used": approve_receipt.gasUsed,
            "effective_gas_price_wei": approve_receipt.effectiveGasPrice,
        }

        deposit_response = call(
            {
                "op": "evm_contract_tx",
                "to": VAULT,
                "data": calldata,
                "rpc_url": RPC,
                "chain_id": CHAIN_ID,
                "session_id": session_id,
                "session_action": "deposit",
                "label": "epoch4 deposit 25 USDC",
                "gas_usd": 10,
            },
            timeout=1200.0,
        )
        if not deposit_response or not deposit_response.get("ok"):
            fail(f"agent deposit failed: {deposit_response}")
        deposit_hash = tx_hash(deposit_response)
        receipt = wait_receipt(w3, deposit_hash)
        if receipt.status != 1:
            fail(f"deposit reverted: {deposit_hash}")

        logs = vault.events.ERC20BridgeDepositedV2().process_receipt(receipt)
        if len(logs) != 1:
            fail(f"expected one deposit event, observed {len(logs)}")
        args = logs[0]["args"]
        expected_event = {
            "depositId": predicted_deposit_id,
            "depositor": wallet_address,
            "pftlRecipientHash": recipient_hash,
            "pftlRecipient": RECIPIENT,
            "amount": AMOUNT_ATOMS,
            "nonce": nonce,
            "routeBinding": route_binding,
            "sourceChainId": CHAIN_ID,
            "vault": vault_address,
            "token": usdc_address,
        }
        for field, expected in expected_event.items():
            if args[field] != expected:
                fail(f"deposit event {field} {args[field]!r} != {expected!r}")

        record = vault.functions.depositRecords(predicted_deposit_id).call()
        expected_record = (wallet_address, AMOUNT_ATOMS, recipient_hash, route_binding, nonce)
        if tuple(record) != expected_record:
            fail(f"deposit record {record!r} != expected consumer vector")

        after = {
            "wallet_usdc_atoms": usdc.functions.balanceOf(wallet_address).call(),
            "vault_usdc_atoms": usdc.functions.balanceOf(vault_address).call(),
            "allowance_atoms": usdc.functions.allowance(wallet_address, vault_address).call(),
            "total_obligations_atoms": vault.functions.totalObligations().call(),
            "wallet_eth_wei": w3.eth.get_balance(wallet_address),
        }
        if before["wallet_usdc_atoms"] - after["wallet_usdc_atoms"] != AMOUNT_ATOMS:
            fail("wallet USDC delta is not exactly 25 USDC")
        if after["vault_usdc_atoms"] - before["vault_usdc_atoms"] != AMOUNT_ATOMS:
            fail("vault USDC delta is not exactly 25 USDC")
        if after["total_obligations_atoms"] - before["total_obligations_atoms"] != AMOUNT_ATOMS:
            fail("vault obligation delta is not exactly 25 USDC")

        evidence["deposit"] = {
            "response": public_agent_response(deposit_response),
            "tx_hash": deposit_hash,
            "block_number": receipt.blockNumber,
            "block_hash": receipt.blockHash.hex(),
            "transaction_index": receipt.transactionIndex,
            "log_index": logs[0]["logIndex"],
            "gas_used": receipt.gasUsed,
            "effective_gas_price_wei": receipt.effectiveGasPrice,
        }
        evidence["event"] = {
            "deposit_id": Web3.to_hex(args["depositId"]),
            "depositor": args["depositor"],
            "recipient_hash": Web3.to_hex(args["pftlRecipientHash"]),
            "pftl_recipient": args["pftlRecipient"],
            "amount_atoms": args["amount"],
            "nonce": Web3.to_hex(args["nonce"]),
            "route_binding": Web3.to_hex(args["routeBinding"]),
            "source_chain_id": args["sourceChainId"],
            "vault": args["vault"],
            "token": args["token"],
        }
        evidence["deposit_record"] = {
            "depositor": record[0],
            "amount_atoms": record[1],
            "recipient_hash": Web3.to_hex(record[2]),
            "route_binding": Web3.to_hex(record[3]),
            "nonce": Web3.to_hex(record[4]),
        }
        evidence["post_state"] = after
        evidence["verdict"] = "PASS"
    finally:
        close_response = call(
            {"op": "close_launch_session", "session_id": session_id}, timeout=30.0
        )
        evidence["agent_close_session"] = public_agent_response(close_response or {})
        HERE.mkdir(parents=True, exist_ok=True)
        (HERE / "deposit-result.json").write_text(
            json.dumps(evidence, indent=2, sort_keys=True) + "\n"
        )

    print(
        "EPOCH4_DEPOSIT: PASS "
        f"tx={evidence['deposit']['tx_hash']} "
        f"deposit_id={evidence['event']['deposit_id']}"
    )


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"EPOCH4_DEPOSIT: FAIL {error}", file=sys.stderr)
        sys.exit(1)
