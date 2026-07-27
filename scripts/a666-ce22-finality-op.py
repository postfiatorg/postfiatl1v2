#!/usr/bin/env python3
"""Submit one signed asset operation through ce22's quorum-finality RPC.

The script fails closed unless all six loopback-tunnel endpoints agree on the
parent height, block hash, state root, chain domain, and empty mempool. It signs
with the operation's declared key file through the deployed postfiat-node
binary, then tries only the endpoint selected by the finality service as the
current proposer (following a typed wrong-proposer response when necessary).
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import time
from typing import Any

RPC_VERSION = "postfiat-local-rpc-v1"
EXPECTED_CHAIN_ID = "postfiat-wan-devnet-2"
EXPECTED_GENESIS_HASH = (
    "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad255"
    "21aff3ed334da07e150a7233a3e90a9"
)
EXPECTED_VALIDATORS = 6
RESPONSE_CAP = 16 * 1024 * 1024


def rpc_call(port: int, request: dict[str, Any], timeout: float) -> dict[str, Any]:
    wire = json.dumps(request, separators=(",", ":")).encode() + b"\n"
    chunks: list[bytes] = []
    received = 0
    with socket.create_connection(("127.0.0.1", port), timeout=timeout) as stream:
        stream.settimeout(timeout)
        stream.sendall(wire)
        stream.shutdown(socket.SHUT_WR)
        while True:
            chunk = stream.recv(65536)
            if not chunk:
                break
            received += len(chunk)
            if received > RESPONSE_CAP:
                raise RuntimeError(f"RPC response from port {port} exceeded byte cap")
            chunks.append(chunk)
    return json.loads(b"".join(chunks))


def request(request_id: str, method: str, params: dict[str, Any]) -> dict[str, Any]:
    return {
        "version": RPC_VERSION,
        "id": request_id,
        "method": method,
        "params": params,
    }


def fleet_status(ports: list[int], timeout: float) -> list[dict[str, Any]]:
    statuses = []
    for port in ports:
        response = rpc_call(port, request(f"status-{port}", "status", {}), timeout)
        if response.get("ok") is not True:
            raise RuntimeError(f"status failed on port {port}: {response.get('error')}")
        result = response["result"]
        if result.get("status") != "running":
            raise RuntimeError(f"validator on port {port} is not running")
        if result.get("chain_id") != EXPECTED_CHAIN_ID:
            raise RuntimeError(f"chain mismatch on port {port}")
        if result.get("genesis_hash") != EXPECTED_GENESIS_HASH:
            raise RuntimeError(f"genesis mismatch on port {port}")
        if result.get("validator_count") != EXPECTED_VALIDATORS:
            raise RuntimeError(f"validator-count mismatch on port {port}")
        statuses.append({"port": port, **result})
    domains = {
        (row["block_height"], row["block_tip_hash"], row["state_root"])
        for row in statuses
    }
    if len(statuses) != EXPECTED_VALIDATORS or len(domains) != 1:
        raise RuntimeError("ce22 fleet is not 6/6 on one parent")
    if len({row["node_id"] for row in statuses}) != EXPECTED_VALIDATORS:
        raise RuntimeError("ce22 status endpoints do not identify six unique validators")
    if any(row["mempool_pending"] != 0 for row in statuses):
        raise RuntimeError("ce22 mempool is not empty")
    return statuses


def write_json(path: Path, value: Any, mode: int = 0o600) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    os.chmod(path, mode)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--ops-file", type=Path, required=True)
    parser.add_argument("--artifact-dir", type=Path, required=True)
    parser.add_argument("--node-bin", type=Path, required=True)
    parser.add_argument(
        "--ports",
        default="28650,28651,28652,28653,28654,28655",
    )
    parser.add_argument("--timeout-seconds", type=float, default=45.0)
    parser.add_argument("--postflight-seconds", type=float, default=30.0)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    ports = [int(value) for value in args.ports.split(",")]
    if len(ports) != EXPECTED_VALIDATORS:
        raise RuntimeError("exactly six RPC endpoints are required")
    if args.artifact_dir.exists():
        raise RuntimeError(f"artifact directory already exists: {args.artifact_dir}")
    args.artifact_dir.mkdir(parents=True, mode=0o700)

    payload = json.loads(args.ops_file.read_text())
    operations = payload.get("operations")
    if not isinstance(operations, list) or len(operations) != 1:
        raise RuntimeError("ops file must contain exactly one operation")
    item = operations[0]
    label = item["label"]
    source = item["source"]
    key_file = Path(item["key_file"])
    operation = item["operation"]
    if not key_file.is_file():
        raise RuntimeError(f"declared signing key does not exist: {key_file}")

    pre = fleet_status(ports, args.timeout_seconds)
    write_json(
        args.artifact_dir / "preflight-fleet.json",
        {
            "schema": "postfiat-a666-ce22-finality-preflight-v1",
            "label": label,
            "validator_count": len(pre),
            "height": pre[0]["block_height"],
            "block_tip_hash": pre[0]["block_tip_hash"],
            "state_root": pre[0]["state_root"],
            "mempool_pending": 0,
            "nodes": [
                {
                    "node_id": row["node_id"],
                    "port": row["port"],
                    "height": row["block_height"],
                    "state_root": row["state_root"],
                }
                for row in pre
            ],
        },
        0o644,
    )

    quote_request = request(
        f"{label}-quote",
        "asset_fee_quote",
        {
            "source": source,
            "operation_json": json.dumps(operation, separators=(",", ":")),
        },
    )
    quote_response = rpc_call(ports[0], quote_request, args.timeout_seconds)
    write_json(args.artifact_dir / "quote.request.json", quote_request, 0o644)
    write_json(args.artifact_dir / "quote.response.json", quote_response, 0o644)
    if quote_response.get("ok") is not True:
        raise RuntimeError(f"asset fee quote failed: {quote_response.get('error')}")
    quote_result_file = args.artifact_dir / "quote.result.json"
    write_json(quote_result_file, quote_response["result"], 0o644)

    signed_raw = subprocess.run(
        [
            str(args.node_bin),
            "wallet-sign-asset-transaction",
            "--key-file",
            str(key_file),
            "--quote-file",
            str(quote_result_file),
        ],
        check=True,
        text=True,
        capture_output=True,
    ).stdout
    signed = json.loads(signed_raw)
    signed_file = args.artifact_dir / "signed.json"
    write_json(signed_file, signed)

    parent = pre[0]
    finality_request = request(
        f"{label}-finality",
        "mempool_submit_signed_asset_transaction_finality",
        {
            "signed_asset_transaction_json": json.dumps(
                signed, separators=(",", ":")
            ),
            "proxy_required_current_height": parent["block_height"],
            "proxy_required_parent_hash": parent["block_tip_hash"],
            "proxy_required_state_root": parent["state_root"],
            "proxy_readiness_timeout_ms": int(args.timeout_seconds * 1000),
        },
    )
    write_json(args.artifact_dir / "finality.request.json", finality_request)

    responses: list[dict[str, Any]] = []
    finality_response: dict[str, Any] | None = None
    candidate_ports = list(ports)
    while candidate_ports:
        port = candidate_ports.pop(0)
        response = rpc_call(port, finality_request, args.timeout_seconds)
        responses.append({"port": port, "response": response})
        if response.get("ok") is True:
            finality_response = response
            break
        error = response.get("error") or {}
        if error.get("code") != "rpc_finality_wrong_proposer":
            raise RuntimeError(
                f"finality submit failed on port {port}: {error}"
            )
        message = str(error.get("message", ""))
        selected = next(
            (
                row["port"]
                for row in pre
                if f"`{row['node_id']}`" in message
            ),
            None,
        )
        if selected is not None and selected in candidate_ports:
            candidate_ports.remove(selected)
            candidate_ports.insert(0, selected)
    write_json(args.artifact_dir / "finality.responses.json", responses, 0o644)
    if finality_response is None:
        raise RuntimeError("no ce22 proposer accepted the finality request")

    result = finality_response["result"]
    if result.get("round_ok") is not True:
        raise RuntimeError("finality response did not report round_ok=true")
    finality = result.get("finality") or {}
    receipt = finality.get("receipt") or {}
    if finality.get("confirmed") is not True or receipt.get("accepted") is not True:
        raise RuntimeError("finality response did not contain an accepted confirmed receipt")

    deadline = time.monotonic() + args.postflight_seconds
    post: list[dict[str, Any]] | None = None
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            candidate = fleet_status(ports, args.timeout_seconds)
            if candidate[0]["block_height"] == parent["block_height"] + 1:
                post = candidate
                break
        except Exception as error:  # fleet convergence is expected to be brief
            last_error = error
        time.sleep(0.25)
    if post is None:
        raise RuntimeError(f"fleet did not converge after finality: {last_error}")

    summary = {
        "schema": "postfiat-a666-ce22-finality-operation-v1",
        "label": label,
        "source": source,
        "transaction_kind": signed["unsigned"]["transaction_kind"],
        "tx_id": result["tx_id"],
        "accepted": True,
        "confirmed": True,
        "round_ok": True,
        "validator_count": len(post),
        "start_height": parent["block_height"],
        "end_height": post[0]["block_height"],
        "start_state_root": parent["state_root"],
        "end_state_root": post[0]["state_root"],
        "end_block_tip_hash": post[0]["block_tip_hash"],
        "end_mempool_pending": 0,
        "trust_class": "CONTROLLED",
    }
    write_json(args.artifact_dir / "summary.json", summary, 0o644)
    print(json.dumps(summary, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
