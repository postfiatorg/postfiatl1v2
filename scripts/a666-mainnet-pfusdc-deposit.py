#!/usr/bin/env python3
"""Submit one exact buyer-funded pfUSDC deposit through StakeHub.

Approval and deposit intentionally use distinct launch sessions. StakeHub
accounts the approved USDC amount against the approval session, so combining
both actions can exhaust a session before the deposit is submitted.
"""

from __future__ import annotations

import argparse
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


REPO = Path(__file__).resolve().parents[1]
BASE = (
    REPO
    / "docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/"
    "recovery-epoch4/deposit/execute_epoch4_deposit.py"
)
MANIFEST = (
    REPO
    / "docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/"
    "recovery-epoch4/deploy/manifest.postdeploy-enriched.json"
)


def load_base() -> object:
    spec = importlib.util.spec_from_file_location("audited_epoch4_deposit", BASE)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load audited deposit sender: {BASE}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--amount-atoms", type=int, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--deployment-manifest",
        type=Path,
        help="Frozen post-deployment manifest for a replacement vault lane",
    )
    parser.add_argument(
        "--expected-manifest-sha256",
        help="Required content digest when --deployment-manifest is used",
    )
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
        "--approval-only",
        action="store_true",
        help="Set the exact allowance without submitting the value-moving deposit",
    )
    mode.add_argument(
        "--require-preapproved",
        action="store_true",
        help="Fail unless the exact allowance already exists; never approve inline",
    )
    return parser.parse_args()


def apply_deployment_manifest(
    base: object,
    manifest_path: Path,
    expected_digest: str | None,
) -> str:
    if not expected_digest:
        raise RuntimeError(
            "--expected-manifest-sha256 is required with --deployment-manifest"
        )
    digest = hashlib.sha256(manifest_path.read_bytes()).hexdigest()
    if digest != expected_digest:
        raise RuntimeError(
            f"deployment manifest digest {digest} != frozen digest {expected_digest}"
        )
    manifest = json.loads(manifest_path.read_text())
    route = manifest["route"]
    network = manifest["network"]
    profile = route["route_profile"]
    if route["status"] not in {"generated-not-deployed", "deployed"}:
        raise RuntimeError("deployment manifest has an unsupported route status")
    if route["route_id"] != "ethereum-mainnet-usdc-v1":
        raise RuntimeError("deployment manifest is not the Ethereum mainnet USDC route")
    if int(route["route_epoch"]) < 4:
        raise RuntimeError("replacement deployment manifest regresses the route epoch")
    if profile["vault_address"] != route["vault_address"]:
        raise RuntimeError("route profile and deployment disagree on the vault")
    if profile["vault_runtime_code_hash"] != route["vault_runtime_code_hash"]:
        raise RuntimeError("route profile and deployment disagree on vault runtime")
    if profile["token_address"] != network["token"]["address"]:
        raise RuntimeError("route profile and network disagree on the source token")
    if int(profile["source_chain_id"]) != int(network["source_chain_id"]):
        raise RuntimeError("route profile and network disagree on source chain")

    base.MANIFEST = manifest_path
    base.EXPECTED_MANIFEST_SHA256 = digest
    base.CHAIN_ID = int(network["source_chain_id"])
    base.RPC = network["execution_rpc_default"]
    base.VAULT = route["vault_address"]
    base.VERIFIER = route["verifier_address"]
    base.USDC = network["token"]["address"]
    base.ROUTE_BINDING = route["route_binding"]
    base.EXPECTED_VAULT_RUNTIME_KECCAK = route[
        "vault_runtime_code_hash"
    ].removeprefix("0x")
    return digest


def main() -> None:
    args = parse_args()
    if args.amount_atoms <= 0:
        raise RuntimeError("--amount-atoms must be positive")
    if args.output.exists():
        raise RuntimeError(f"refusing to overwrite evidence: {args.output}")

    base = load_base()
    if args.deployment_manifest:
        manifest_digest = apply_deployment_manifest(
            base,
            args.deployment_manifest,
            args.expected_manifest_sha256,
        )
    else:
        if args.expected_manifest_sha256:
            raise RuntimeError(
                "--expected-manifest-sha256 requires --deployment-manifest"
            )
        manifest_digest = hashlib.sha256(MANIFEST.read_bytes()).hexdigest()
        if manifest_digest != base.EXPECTED_MANIFEST_SHA256:
            raise RuntimeError("authorized pfUSDC deployment manifest changed")

    sys.path.insert(0, str(REPO.parent / "StakeHub"))
    from stakehub.agentd import call

    w3 = Web3(Web3.HTTPProvider(base.RPC, request_kwargs={"timeout": 60}))
    if not w3.is_connected() or w3.eth.chain_id != base.CHAIN_ID:
        raise RuntimeError("Ethereum mainnet RPC is unavailable or on the wrong chain")

    vault_address = Web3.to_checksum_address(base.VAULT)
    verifier_address = Web3.to_checksum_address(base.VERIFIER)
    usdc_address = Web3.to_checksum_address(base.USDC)
    wallet_address = Web3.to_checksum_address(base.WALLET)
    if Web3.keccak(w3.eth.get_code(vault_address)).hex().removeprefix("0x") != (
        base.EXPECTED_VAULT_RUNTIME_KECCAK
    ):
        raise RuntimeError("pfUSDC vault runtime hash differs from the authorized deployment")

    usdc = w3.eth.contract(address=usdc_address, abi=base.USDC_ABI)
    vault = w3.eth.contract(address=vault_address, abi=base.VAULT_ABI)
    if vault.functions.paused().call():
        raise RuntimeError("pfUSDC vault is paused")
    if vault.functions.finalityVerifier().call() != verifier_address:
        raise RuntimeError("pfUSDC vault finality verifier differs from the governed verifier")

    amount = args.amount_atoms
    before = {
        "wallet_usdc_atoms": usdc.functions.balanceOf(wallet_address).call(),
        "vault_usdc_atoms": usdc.functions.balanceOf(vault_address).call(),
        "allowance_atoms": usdc.functions.allowance(wallet_address, vault_address).call(),
        "total_obligations_atoms": vault.functions.totalObligations().call(),
        "wallet_eth_wei": w3.eth.get_balance(wallet_address),
        "confirmed_nonce": w3.eth.get_transaction_count(wallet_address, "latest"),
        "pending_nonce": w3.eth.get_transaction_count(wallet_address, "pending"),
    }
    if before["wallet_usdc_atoms"] < amount:
        raise RuntimeError("StakeHub wallet has insufficient USDC")
    if before["pending_nonce"] != before["confirmed_nonce"]:
        raise RuntimeError("StakeHub wallet has a pending Ethereum transaction")

    builder = base.load_builder()
    nonce = os.urandom(32)
    route_binding = bytes.fromhex(base.ROUTE_BINDING)
    deposit_calldata = builder.build_deposit_v2_calldata(
        w3.codec, amount, base.RECIPIENT, nonce, route_binding
    )
    builder.assert_deposit_v2_calldata(
        deposit_calldata, amount, base.RECIPIENT, nonce, route_binding
    )
    recipient_hash = Web3.keccak(text=base.RECIPIENT)
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
                base.CHAIN_ID,
                vault_address,
                usdc_address,
                wallet_address,
                amount,
                recipient_hash,
                nonce,
                route_binding,
            ],
        )
    )
    approve_calldata = usdc.encode_abi("approve", args=[vault_address, amount])

    evidence: dict[str, object] = {
        "schema": (
            "postfiat.a666.pfusdc_buyer_preapproval.v1"
            if args.approval_only
            else "postfiat.a666.pfusdc_buyer_deposit.v1"
        ),
        "verdict": "PENDING",
        "started_unix": int(time.time()),
        "manifest_sha256": manifest_digest,
        "chain_id": base.CHAIN_ID,
        "vault": base.VAULT,
        "verifier": base.VERIFIER,
        "usdc": base.USDC,
        "depositor": base.WALLET,
        "pftl_recipient": base.RECIPIENT,
        "amount_atoms": amount,
        "nonce": Web3.to_hex(nonce),
        "route_binding": "0x" + base.ROUTE_BINDING,
        "recipient_hash": Web3.to_hex(recipient_hash),
        "predicted_deposit_id": Web3.to_hex(predicted_deposit_id),
        "deposit_calldata": deposit_calldata,
        "deposit_calldata_sha256": hashlib.sha256(
            bytes.fromhex(deposit_calldata[2:])
        ).hexdigest(),
        "pre_state": before,
    }
    active_session: str | None = None

    def close_session() -> None:
        nonlocal active_session
        if active_session is None:
            return
        response = call(
            {"op": "close_launch_session", "session_id": active_session}, timeout=30
        )
        evidence.setdefault("closed_sessions", []).append(
            {
                "session_id": active_session,
                "response": base.public_agent_response(response or {}),
            }
        )
        active_session = None

    def open_session(action: str, calldata: str) -> str:
        nonlocal active_session
        session = (
            f"a666-pfusdc-{action}-"
            f"{datetime.now(UTC).strftime('%Y%m%dT%H%M%S%fZ')}"
        )
        response = call(
            {
                "op": "open_launch_session",
                "session_id": session,
                "chain_id": base.CHAIN_ID,
                "allowlist": [base.WALLET, base.USDC, base.VAULT],
                "expected_deploys": [
                    {
                        "label": f"a666-pfusdc-{action}-calldata",
                        "bytecode_hash": Web3.to_hex(Web3.keccak(hexstr=calldata)),
                        "bytecode_len": len(bytes.fromhex(calldata[2:])),
                    }
                ],
                "usdc_address": base.USDC,
                "usdc_budget": amount,
                "close_after_action": action,
                "ttl_seconds": 1800,
            },
            timeout=60,
        )
        if not response or not response.get("ok"):
            raise RuntimeError(f"StakeHub {action} session failed to open: {response}")
        active_session = session
        evidence.setdefault("opened_sessions", []).append(
            {
                "session_id": session,
                "action": action,
                "response": base.public_agent_response(response),
            }
        )
        return session

    def send(action: str, to: str, calldata: str) -> tuple[str, object]:
        session = open_session(action, calldata)
        response = call(
            {
                "op": "evm_contract_tx",
                "to": to,
                "data": calldata,
                "rpc_url": base.RPC,
                "chain_id": base.CHAIN_ID,
                "session_id": session,
                "session_action": action,
                "label": f"A666 pfUSDC {action} {amount} atoms",
                "gas_usd": 10,
            },
            timeout=1200,
        )
        if not response or not response.get("ok"):
            raise RuntimeError(f"StakeHub {action} transaction failed: {response}")
        transaction_hash = base.tx_hash(response)
        receipt = base.wait_receipt(w3, transaction_hash)
        if receipt.status != 1:
            raise RuntimeError(f"{action} transaction reverted: {transaction_hash}")
        evidence[action] = {
            "response": base.public_agent_response(response),
            "tx_hash": transaction_hash,
            "block_number": receipt.blockNumber,
            "block_hash": receipt.blockHash.hex(),
            "gas_used": receipt.gasUsed,
            "effective_gas_price_wei": receipt.effectiveGasPrice,
        }
        close_session()
        return transaction_hash, receipt

    try:
        if args.approval_only:
            if before["allowance_atoms"] != amount:
                send("approve", base.USDC, approve_calldata)
            allowance = usdc.functions.allowance(wallet_address, vault_address).call()
            if allowance != amount:
                raise RuntimeError("USDC approval did not produce the exact allowance")
            evidence["post_state"] = {
                **before,
                "allowance_atoms": allowance,
                "wallet_eth_wei": w3.eth.get_balance(wallet_address),
                "confirmed_nonce": w3.eth.get_transaction_count(
                    wallet_address, "latest"
                ),
                "pending_nonce": w3.eth.get_transaction_count(
                    wallet_address, "pending"
                ),
            }
            evidence["completed_unix"] = int(time.time())
            evidence["elapsed_seconds"] = (
                evidence["completed_unix"] - evidence["started_unix"]
            )
            evidence["verdict"] = "PASS"
            print(f"A666_PFUSDC_PREAPPROVAL: PASS amount_atoms={amount}")
            return
        if args.require_preapproved and before["allowance_atoms"] != amount:
            raise RuntimeError(
                "exact USDC allowance is not preapproved; inline approval is disabled"
            )
        if before["allowance_atoms"] != amount:
            send("approve", base.USDC, approve_calldata)
            allowance = usdc.functions.allowance(wallet_address, vault_address).call()
            if allowance != amount:
                raise RuntimeError("USDC approval did not produce the exact allowance")

        _, receipt = send("deposit", base.VAULT, deposit_calldata)
        logs = vault.events.ERC20BridgeDepositedV2().process_receipt(receipt)
        if len(logs) != 1:
            raise RuntimeError(f"expected one deposit event, observed {len(logs)}")
        event = logs[0]["args"]
        expected_event = {
            "depositId": predicted_deposit_id,
            "depositor": wallet_address,
            "pftlRecipientHash": recipient_hash,
            "pftlRecipient": base.RECIPIENT,
            "amount": amount,
            "nonce": nonce,
            "routeBinding": route_binding,
            "sourceChainId": base.CHAIN_ID,
            "vault": vault_address,
            "token": usdc_address,
        }
        for field, expected in expected_event.items():
            if event[field] != expected:
                raise RuntimeError(f"deposit event field {field} differs from intent")
        record = vault.functions.depositRecords(predicted_deposit_id).call()
        if tuple(record) != (
            wallet_address,
            amount,
            recipient_hash,
            route_binding,
            nonce,
        ):
            raise RuntimeError("on-chain deposit record differs from intent")

        after = {
            "wallet_usdc_atoms": usdc.functions.balanceOf(wallet_address).call(),
            "vault_usdc_atoms": usdc.functions.balanceOf(vault_address).call(),
            "allowance_atoms": usdc.functions.allowance(wallet_address, vault_address).call(),
            "total_obligations_atoms": vault.functions.totalObligations().call(),
            "wallet_eth_wei": w3.eth.get_balance(wallet_address),
            "confirmed_nonce": w3.eth.get_transaction_count(wallet_address, "latest"),
            "pending_nonce": w3.eth.get_transaction_count(wallet_address, "pending"),
        }
        if before["wallet_usdc_atoms"] - after["wallet_usdc_atoms"] != amount:
            raise RuntimeError("wallet USDC delta differs from the deposit amount")
        if after["vault_usdc_atoms"] - before["vault_usdc_atoms"] != amount:
            raise RuntimeError("vault USDC delta differs from the deposit amount")
        if after["total_obligations_atoms"] - before["total_obligations_atoms"] != amount:
            raise RuntimeError("vault obligation delta differs from the deposit amount")

        evidence["event"] = {
            "deposit_id": Web3.to_hex(event["depositId"]),
            "depositor": event["depositor"],
            "pftl_recipient": event["pftlRecipient"],
            "amount_atoms": event["amount"],
            "nonce": Web3.to_hex(event["nonce"]),
            "route_binding": Web3.to_hex(event["routeBinding"]),
        }
        evidence["post_state"] = after
        evidence["completed_unix"] = int(time.time())
        evidence["elapsed_seconds"] = (
            evidence["completed_unix"] - evidence["started_unix"]
        )
        evidence["verdict"] = "PASS"
    except Exception as error:
        evidence["error"] = str(error)
        raise
    finally:
        close_session()
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n")

    print(
        f"A666_PFUSDC_DEPOSIT: PASS tx={evidence['deposit']['tx_hash']} "
        f"deposit_id={evidence['event']['deposit_id']}"
    )


if __name__ == "__main__":
    main()
