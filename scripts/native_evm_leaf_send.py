#!/usr/bin/env python3
"""Bounded native ETH transfer leaf for the campaign driver."""
from __future__ import annotations

import argparse
import json
import time
from pathlib import Path
from typing import Any
from urllib.request import Request, urlopen

try:
    from . import native_agentd_leaf
except ImportError:
    import native_agentd_leaf


def _rpc(url: str, method: str, params: list[Any]) -> Any:
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    request = Request(url, data=body, headers={"content-type": "application/json"})
    with urlopen(request, timeout=15) as response:
        payload = json.loads(response.read().decode())
    if payload.get("error"):
        raise RuntimeError(str(payload["error"]))
    return payload.get("result")


def _hex_int(value: Any) -> int:
    return int(value, 16) if isinstance(value, str) and value.startswith("0x") else int(value)


def _wait_receipt(rpc_url: str, tx_hash: str) -> dict[str, Any]:
    for _ in range(120):
        receipt = _rpc(rpc_url, "eth_getTransactionReceipt", [tx_hash])
        if receipt:
            return receipt
        time.sleep(1)
    raise RuntimeError("transaction receipt timeout")


def send(args: argparse.Namespace) -> dict[str, Any]:
    if int(args.chain_id) != 1:
        raise ValueError("chain-id must be 1")
    if not (args.recipient.startswith("0x") and len(args.recipient) == 42):
        raise ValueError("recipient must be an EVM address")
    try:
        int(args.recipient[2:], 16)
    except ValueError as exc:
        raise ValueError("recipient must be an EVM address") from exc
    amount = int(args.amount_wei)
    if amount < 0:
        raise ValueError("amount must be non-negative")
    gas = _hex_int(_rpc(args.rpc_url, "eth_estimateGas", [{"to": args.recipient, "value": hex(amount)}]))
    gas_price = _hex_int(_rpc(args.rpc_url, "eth_gasPrice", []))
    estimated_fee = gas * gas_price
    if estimated_fee > int(args.max_fee_wei):
        raise RuntimeError("estimated fee exceeds max-fee-wei")
    if args.expected_recipient_balance_wei is not None:
        recipient_balance = _hex_int(
            _rpc(args.rpc_url, "eth_getBalance", [args.recipient, "latest"])
        )
        if recipient_balance != int(args.expected_recipient_balance_wei):
            raise RuntimeError(
                "recipient balance changed before broadcast: "
                f"expected {args.expected_recipient_balance_wei}, got {recipient_balance}"
            )
    tx_hash = native_agentd_leaf.evm_send(args.stakehub_home, "ethereum", args.recipient, amount, args.label)
    receipt = _wait_receipt(args.rpc_url, tx_hash)
    status = _hex_int(receipt.get("status", 0))
    if status != 1:
        raise RuntimeError("Ethereum transfer reverted")
    tx = _rpc(args.rpc_url, "eth_getTransactionByHash", [tx_hash]) or {}
    report = {
        "tx_hash": tx_hash,
        "status": status,
        "block_number": _hex_int(receipt.get("blockNumber", 0)),
        "from": tx.get("from"),
        "to": tx.get("to", args.recipient),
        "value_wei": _hex_int(tx.get("value", amount)),
        "gas_used": _hex_int(receipt.get("gasUsed", gas)),
        "effective_gas_price": _hex_int(receipt.get("effectiveGasPrice", gas_price)),
    }
    Path(args.report).parent.mkdir(parents=True, exist_ok=True)
    Path(args.report).write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stakehub-home", required=True)
    parser.add_argument("--chain-id", required=True, type=int)
    parser.add_argument("--rpc-url", required=True)
    parser.add_argument("--recipient", required=True)
    parser.add_argument("--amount-wei", required=True, type=int)
    parser.add_argument("--max-fee-wei", required=True, type=int)
    parser.add_argument("--expected-recipient-balance-wei", type=int)
    parser.add_argument("--label", required=True)
    parser.add_argument("--report", required=True)
    args = parser.parse_args(argv)
    try:
        report = send(args)
    except (OSError, RuntimeError, ValueError) as exc:
        print(f"STOP-no-retry: {exc}")
        return 2
    print(json.dumps(report, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
