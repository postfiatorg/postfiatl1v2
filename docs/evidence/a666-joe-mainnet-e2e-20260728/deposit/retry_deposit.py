#!/usr/bin/env python3
"""Submit only the vault deposit after the approval/deposit nonce race."""

from __future__ import annotations

import importlib.util
import json
import sys
from datetime import UTC, datetime
from pathlib import Path

from web3 import Web3


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
BASE = (
    REPO
    / "docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/"
    "recovery-epoch4/deposit/execute_epoch4_deposit.py"
)


def main() -> None:
    spec = importlib.util.spec_from_file_location("epoch4_deposit", BASE)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load audited deposit sender: {BASE}")
    base = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = base
    spec.loader.exec_module(base)
    sys.path.insert(0, str(REPO.parent / "StakeHub"))
    from stakehub.agentd import call

    original = json.loads((HERE / "deposit-result.json").read_text())
    if original["verdict"] != "PENDING" or "approve" not in original:
        raise RuntimeError("expected a successful approval and pending deposit")

    w3 = Web3(Web3.HTTPProvider(base.RPC, request_kwargs={"timeout": 60}))
    wallet = Web3.to_checksum_address(base.WALLET)
    usdc_address = Web3.to_checksum_address(base.USDC)
    vault_address = Web3.to_checksum_address(base.VAULT)
    usdc = w3.eth.contract(address=usdc_address, abi=base.USDC_ABI)
    vault = w3.eth.contract(address=vault_address, abi=base.VAULT_ABI)
    amount = int(original["amount_atoms"])
    calldata = original["deposit_calldata"]
    predicted_deposit_id = original["predicted_deposit_id"]

    before = {
        "wallet_usdc_atoms": usdc.functions.balanceOf(wallet).call(),
        "vault_usdc_atoms": usdc.functions.balanceOf(vault_address).call(),
        "allowance_atoms": usdc.functions.allowance(wallet, vault_address).call(),
        "total_obligations_atoms": vault.functions.totalObligations().call(),
        "wallet_eth_wei": w3.eth.get_balance(wallet),
        "pending_nonce": w3.eth.get_transaction_count(wallet, "pending"),
    }
    if before["allowance_atoms"] != amount:
        raise RuntimeError("vault allowance is not the exact approved amount")
    if before["wallet_usdc_atoms"] < amount:
        raise RuntimeError("wallet has insufficient USDC")

    session_id = f"pfusdc-joe-deposit-retry-{datetime.now(UTC).strftime('%Y%m%dT%H%M%SZ')}"
    opened = call(
        {
            "op": "open_launch_session",
            "session_id": session_id,
            "chain_id": base.CHAIN_ID,
            "allowlist": [base.WALLET, base.USDC, base.VAULT],
            "expected_deploys": [
                {
                    "label": "joe-deposit-calldata-frozen",
                    "bytecode_hash": Web3.to_hex(Web3.keccak(hexstr=calldata)),
                    "bytecode_len": len(bytes.fromhex(calldata[2:])),
                }
            ],
            "usdc_address": base.USDC,
            "usdc_budget": amount,
            "close_after_action": "deposit",
            "ttl_seconds": 1800,
        },
        timeout=60,
    )
    if not opened or not opened.get("ok"):
        raise RuntimeError(f"open launch session failed: {opened}")

    evidence: dict[str, object] = {
        "schema": "postfiat.pfusdc.joe_deposit_retry.v1",
        "verdict": "PENDING",
        "session_id": session_id,
        "original_result": str(HERE / "deposit-result.json"),
        "predicted_deposit_id": predicted_deposit_id,
        "amount_atoms": amount,
        "pre_state": before,
        "agent_open_session": base.public_agent_response(opened),
    }
    try:
        response = call(
            {
                "op": "evm_contract_tx",
                "to": base.VAULT,
                "data": calldata,
                "rpc_url": base.RPC,
                "chain_id": base.CHAIN_ID,
                "session_id": session_id,
                "session_action": "deposit",
                "label": "Joe deposit 100.5 USDC",
                "gas_usd": 10,
            },
            timeout=1200,
        )
        if not response or not response.get("ok"):
            raise RuntimeError(f"agent deposit failed: {response}")
        transaction_hash = base.tx_hash(response)
        receipt = base.wait_receipt(w3, transaction_hash)
        if receipt.status != 1:
            raise RuntimeError(f"deposit reverted: {transaction_hash}")
        logs = vault.events.ERC20BridgeDepositedV2().process_receipt(receipt)
        if len(logs) != 1:
            raise RuntimeError(f"expected one deposit event, observed {len(logs)}")
        event = logs[0]["args"]
        if Web3.to_hex(event["depositId"]).lower() != predicted_deposit_id.lower():
            raise RuntimeError("deposit id differs from frozen prediction")
        if event["amount"] != amount or event["pftlRecipient"] != original["pftl_recipient"]:
            raise RuntimeError("deposit event differs from frozen intent")

        record = vault.functions.depositRecords(bytes.fromhex(predicted_deposit_id[2:])).call()
        after = {
            "wallet_usdc_atoms": usdc.functions.balanceOf(wallet).call(),
            "vault_usdc_atoms": usdc.functions.balanceOf(vault_address).call(),
            "allowance_atoms": usdc.functions.allowance(wallet, vault_address).call(),
            "total_obligations_atoms": vault.functions.totalObligations().call(),
            "wallet_eth_wei": w3.eth.get_balance(wallet),
            "pending_nonce": w3.eth.get_transaction_count(wallet, "pending"),
        }
        if before["wallet_usdc_atoms"] - after["wallet_usdc_atoms"] != amount:
            raise RuntimeError("wallet USDC delta differs from the deposit amount")
        if after["vault_usdc_atoms"] - before["vault_usdc_atoms"] != amount:
            raise RuntimeError("vault USDC delta differs from the deposit amount")
        if after["total_obligations_atoms"] - before["total_obligations_atoms"] != amount:
            raise RuntimeError("vault obligation delta differs from the deposit amount")

        block = w3.eth.get_block(receipt.blockNumber)
        evidence.update(
            {
                "verdict": "PASS",
                "deposit": {
                    "response": base.public_agent_response(response),
                    "tx_hash": transaction_hash,
                    "block_number": receipt.blockNumber,
                    "block_hash": receipt.blockHash.hex(),
                    "block_timestamp": block.timestamp,
                    "gas_used": receipt.gasUsed,
                    "effective_gas_price_wei": receipt.effectiveGasPrice,
                },
                "event": {
                    "deposit_id": Web3.to_hex(event["depositId"]),
                    "pftl_recipient": event["pftlRecipient"],
                    "amount_atoms": event["amount"],
                    "nonce": Web3.to_hex(event["nonce"]),
                    "route_binding": Web3.to_hex(event["routeBinding"]),
                },
                "deposit_record": {
                    "depositor": record[0],
                    "amount_atoms": record[1],
                    "recipient_hash": Web3.to_hex(record[2]),
                    "route_binding": Web3.to_hex(record[3]),
                    "nonce": Web3.to_hex(record[4]),
                },
                "post_state": after,
            }
        )
    finally:
        closed = call(
            {"op": "close_launch_session", "session_id": session_id}, timeout=30
        )
        evidence["agent_close_session"] = base.public_agent_response(closed or {})
        (HERE / "deposit-retry-result.json").write_text(
            json.dumps(evidence, indent=2, sort_keys=True) + "\n"
        )

    print(
        f"JOE_DEPOSIT: PASS tx={evidence['deposit']['tx_hash']} "
        f"deposit_id={evidence['event']['deposit_id']}"
    )


if __name__ == "__main__":
    main()
