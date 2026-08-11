#!/usr/bin/env python3
"""Convert a verified nav-roundtrip deposit report into ingress-runner evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

from web3 import Web3
from web3.logs import DISCARD


ROOT = Path(__file__).resolve().parents[1]
VAULT_ARTIFACT = (
    ROOT
    / "crates/ethereum-contracts/out/ERC20BridgeVaultL1.sol/"
    "ERC20BridgeVaultL1.json"
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--evm-report", type=Path, required=True)
    parser.add_argument("--deployment-manifest", type=Path, required=True)
    parser.add_argument("--expected-manifest-sha256", required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args()


def h(value: object) -> str:
    return Web3.to_hex(value)


def main() -> None:
    args = parse_args()
    if args.output.exists():
        raise RuntimeError(f"refusing to overwrite evidence: {args.output}")
    manifest_bytes = args.deployment_manifest.read_bytes()
    manifest_sha256 = hashlib.sha256(manifest_bytes).hexdigest()
    if manifest_sha256 != args.expected_manifest_sha256:
        raise RuntimeError("deployment manifest digest differs from the frozen digest")
    manifest = json.loads(manifest_bytes)
    report = json.loads(args.evm_report.read_text(encoding="utf-8"))
    if report.get("schema") != "postfiat-nav-roundtrip-evm-deposit-report-v1":
        raise RuntimeError("unexpected nav-roundtrip deposit report schema")
    if report.get("delta_ok") is not True or report.get("failure_reasons") != []:
        raise RuntimeError("nav-roundtrip deposit report is not a clean pass")

    network = manifest["network"]
    route = manifest["route"]
    profile = route["route_profile"]
    expected = {
        "source_chain_id": int(network["source_chain_id"]),
        "vault_address": route["vault_address"],
        "usdc_address": network["token"]["address"],
    }
    if int(report["source_chain_id"]) != expected["source_chain_id"]:
        raise RuntimeError("deposit report chain does not match deployment")
    for field in ("vault_address", "usdc_address"):
        if report[field].lower() != expected[field].lower():
            raise RuntimeError(f"deposit report {field} does not match deployment")
    if profile["vault_address"].lower() != expected["vault_address"].lower():
        raise RuntimeError("route profile vault does not match deployment")

    rpc = network["execution_rpc_default"]
    w3 = Web3(Web3.HTTPProvider(rpc, request_kwargs={"timeout": 60}))
    if not w3.is_connected() or w3.eth.chain_id != expected["source_chain_id"]:
        raise RuntimeError("source RPC is unavailable or on the wrong chain")
    receipt = w3.eth.get_transaction_receipt(report["deposit_tx"])
    if receipt.status != 1:
        raise RuntimeError("deposit receipt is not successful")
    artifact = json.loads(VAULT_ARTIFACT.read_text(encoding="utf-8"))
    vault = w3.eth.contract(
        address=Web3.to_checksum_address(expected["vault_address"]),
        abi=artifact["abi"],
    )
    events = vault.events.ERC20BridgeDepositedV2().process_receipt(
        receipt, errors=DISCARD
    )
    if len(events) != 1:
        raise RuntimeError(f"expected one deposit event, found {len(events)}")
    event = events[0]["args"]

    amount = int(report["amount_atoms"])
    recipient = report["pftl_recipient"]
    nonce = report["nonce"].lower()
    route_binding = "0x" + route["route_binding"].removeprefix("0x").lower()
    checks = (
        int(event["amount"]) == amount,
        event["pftlRecipient"] == recipient,
        h(event["nonce"]).lower() == nonce,
        h(event["routeBinding"]).lower() == route_binding,
        int(event["sourceChainId"]) == expected["source_chain_id"],
        event["vault"].lower() == expected["vault_address"].lower(),
        event["token"].lower() == expected["usdc_address"].lower(),
        event["depositor"].lower() == report["stakehub_wallet"].lower(),
    )
    if not all(checks):
        raise RuntimeError("deposit event does not exactly match the frozen run inputs")

    output = {
        "schema": "postfiat.a666.pfusdc_buyer_deposit.v1",
        "verdict": "PASS",
        "manifest_sha256": manifest_sha256,
        "chain_id": expected["source_chain_id"],
        "vault": expected["vault_address"],
        "verifier": route["verifier_address"],
        "usdc": expected["usdc_address"],
        "depositor": report["stakehub_wallet"],
        "pftl_recipient": recipient,
        "amount_atoms": amount,
        "nonce": nonce,
        "route_binding": route_binding,
        "deposit": {
            "tx_hash": report["deposit_tx"],
            "block_number": int(receipt.blockNumber),
            "block_hash": h(receipt.blockHash),
            "transaction_index": int(receipt.transactionIndex),
            "log_index": int(events[0]["logIndex"]),
            "gas_used": int(receipt.gasUsed),
            "effective_gas_price_wei": int(receipt.effectiveGasPrice),
        },
        "event": {
            "deposit_id": h(event["depositId"]),
            "depositor": event["depositor"],
            "pftl_recipient": event["pftlRecipient"],
            "amount_atoms": int(event["amount"]),
            "nonce": h(event["nonce"]),
            "route_binding": h(event["routeBinding"]),
            "source_chain_id": int(event["sourceChainId"]),
            "vault": event["vault"],
            "token": event["token"],
        },
        "pre_state": {
            "wallet_usdc_atoms": int(report["wallet_usdc_before_atoms"]),
            "vault_usdc_atoms": int(report["vault_usdc_before_atoms"]),
            "allowance_atoms": int(report["allowance_before_atoms"]),
        },
        "post_state": {
            "wallet_usdc_atoms": int(report["wallet_usdc_after_atoms"]),
            "vault_usdc_atoms": int(report["vault_usdc_after_atoms"]),
        },
        "source_report": {
            "path": str(args.evm_report),
            "sha256": hashlib.sha256(args.evm_report.read_bytes()).hexdigest(),
        },
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, indent=2, sort_keys=True) + "\n")
    print(json.dumps(output, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
