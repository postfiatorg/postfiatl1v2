#!/usr/bin/env python3
"""Fail-closed raw RPC finality submitter; stdlib only."""
from __future__ import annotations

import argparse
import json
import socket
import sys
from pathlib import Path

RPC_VERSION = "postfiat-local-rpc-v1"
MAX_RESPONSE_BYTES = 8 * 1024 * 1024


class StopError(RuntimeError):
    pass


def _request(sock: socket.socket, payload: dict, timeout_seconds: float) -> dict:
    sock.settimeout(timeout_seconds)
    sock.sendall((json.dumps(payload, separators=(",", ":")) + "\n").encode("utf-8"))
    chunks = bytearray()
    while len(chunks) < MAX_RESPONSE_BYTES:
        chunk = sock.recv(65536)
        if not chunk:
            raise StopError("RPC socket closed before response")
        chunks.extend(chunk)
        if b"\n" in chunk:
            break
    line = bytes(chunks).split(b"\n", 1)[0]
    try:
        response = json.loads(line.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise StopError("RPC response was not valid JSON") from exc
    if not isinstance(response, dict):
        raise StopError("RPC response must be an object")
    return response


def _load_signed_transaction(path: Path) -> tuple[dict, str]:
    try:
        value = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        raise StopError(f"signed transaction file malformed: {path}") from exc
    if not isinstance(value, dict):
        raise StopError("signed transaction must be a JSON object")
    required = ("unsigned", "algorithm_id", "public_key_hex", "signature_hex")
    missing = [name for name in required if name not in value or value[name] in (None, "")]
    if missing:
        raise StopError("signed transaction missing required key(s): " + ",".join(missing))
    try:
        encoded = json.dumps(value, separators=(",", ":"))
    except (TypeError, ValueError) as exc:
        raise StopError("signed transaction is not JSON-serializable") from exc
    return value, encoded


def _result(response: dict, label: str) -> dict:
    if response.get("version") != RPC_VERSION:
        raise StopError(f"{label} response protocol version mismatch")
    if response.get("ok") is not True:
        raise StopError(f"{label} RPC response was not ok")
    result = response.get("result")
    if not isinstance(result, dict):
        raise StopError(f"{label} response missing result object")
    return result


def submit(args: argparse.Namespace) -> dict:
    _, signed_json = _load_signed_transaction(Path(args.signed_tx_file))
    timeout_seconds = max(0.001, args.readiness_timeout_ms / 1000.0)
    status_request = {
        "version": RPC_VERSION,
        "id": f"{args.id}-status",
        "method": "status",
        "params": {},
    }
    # StatusReport serializes block_height, block_tip_hash, and state_root;
    # see postfiatl1v2 crates/types/src/core_chain.rs:450-480 and the RPC
    # status dispatch in crates/node/src/rpc_cli.rs:1033-1052.
    finality_request = None
    with socket.create_connection((args.socket_host, args.socket_port), timeout=timeout_seconds) as sock:
        status = _result(_request(sock, status_request, timeout_seconds), "status")
        required = ("block_height", "block_tip_hash", "state_root")
        missing = [name for name in required if name not in status or status[name] in (None, "")]
        if missing:
            raise StopError("status missing finality pin field(s): " + ",".join(missing))
        if not isinstance(status["block_height"], int) or isinstance(status["block_height"], bool):
            raise StopError("status block_height must be an integer")
        finality_request = {
            "version": RPC_VERSION,
            "id": args.id,
            "method": "mempool_submit_signed_asset_transaction_finality",
            "params": {
                "proxy_readiness_timeout_ms": args.readiness_timeout_ms,
                "proxy_required_current_height": status["block_height"],
                "proxy_required_parent_hash": status["block_tip_hash"],
                "proxy_required_state_root": status["state_root"],
                "signed_asset_transaction_json": signed_json,
            },
        }
        response = _request(sock, finality_request, timeout_seconds)
    result = _result(response, "finality")
    finality = result.get("finality")
    if not isinstance(finality, dict):
        raise StopError("finality response missing finality object")
    receipt = finality.get("receipt")
    if not isinstance(receipt, dict):
        raise StopError("finality response missing receipt object")
    if finality.get("confirmed") is not True:
        raise StopError("finality.confirmed was not true")
    if receipt.get("accepted") is not True:
        raise StopError("finality.receipt.accepted was not true")
    if result.get("round_ok") is not True:
        raise StopError("finality round_ok was not true")
    tx_id = result.get("tx_id")
    header = finality.get("block")
    header = header.get("header") if isinstance(header, dict) else None
    end_height = header.get("height") if isinstance(header, dict) else None
    if not isinstance(tx_id, str) or not tx_id or not isinstance(end_height, int):
        raise StopError("finality response missing tx_id or block height")
    return {"response": response, "tx_id": tx_id, "end_height": end_height, "round_ok": result["round_ok"]}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--signed-tx-file", required=True)
    parser.add_argument("--id", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--socket-host", default="127.0.0.1")
    parser.add_argument("--socket-port", type=int, default=39660)
    parser.add_argument("--readiness-timeout-ms", type=int, default=45000)
    args = parser.parse_args(argv)
    try:
        if args.readiness_timeout_ms <= 0:
            raise StopError("readiness-timeout-ms must be positive")
        result = submit(args)
        output = Path(args.output)
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(result["response"], indent=2, sort_keys=True) + "\n")
        print(f"{args.id} {result['tx_id']} {result['end_height']} {str(result['round_ok']).lower()}")
        return 0
    except (OSError, StopError, ValueError, TypeError) as exc:
        print(f"STOP-no-retry: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
