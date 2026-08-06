#!/usr/bin/env python3
"""Minimal authenticated Unix-socket custody leaf.

Wire contract mirrored from the daemon implementation: request JSON plus LF,
response JSON plus LF, with status/evm_send/evm_contract_tx handlers (agentd.py
lines 704-714, 1153-1210, and 1458-1526).  This module has no state authority.
"""
from __future__ import annotations

import json
import os
import socket
from pathlib import Path
from typing import Any


def _socket(home: str | os.PathLike[str]) -> Path:
    return Path(home) / "agent.sock"


def _request(home: str | os.PathLike[str], request: dict[str, Any]) -> dict[str, Any]:
    path = _socket(home)
    if not path.exists():
        raise RuntimeError(f"agent socket missing: {path}")
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
            sock.settimeout(30.0)
            sock.connect(str(path))
            sock.sendall(json.dumps(request, separators=(",", ":")).encode() + b"\n")
            chunks: list[bytes] = []
            while True:
                chunk = sock.recv(65536)
                if not chunk:
                    break
                chunks.append(chunk)
                if b"\n" in chunk:
                    break
    except OSError as exc:
        raise RuntimeError(f"agent socket request failed: {exc}") from exc
    raw = b"".join(chunks).split(b"\n", 1)[0]
    try:
        response = json.loads(raw.decode())
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise RuntimeError("agent response is not JSON") from exc
    if not isinstance(response, dict) or response.get("ok") is not True:
        detail = response.get("error", "unknown agent error") if isinstance(response, dict) else "malformed response"
        raise RuntimeError(str(detail))
    return response


def session_status(home: str | os.PathLike[str]) -> dict[str, Any]:
    return _request(home, {"op": "status"})


def evm_send(home: str | os.PathLike[str], chain: str, dest: str, wei: int, label: str) -> str:
    response = _request(home, {"op": "evm_send", "chain": chain, "asset": "eth", "dest": dest, "wei": int(wei), "label": label})
    tx = response.get("tx")
    if not isinstance(tx, str) or not tx:
        raise RuntimeError("agent response omitted tx hash")
    return tx


def evm_contract_tx(home: str | os.PathLike[str], chain: int | str, to: str, data: str, value_wei: int, label: str) -> str:
    rpc_url = os.environ.get("EVM_RPC_URL")
    if not rpc_url:
        raise RuntimeError("EVM_RPC_URL is required for contract signing")
    response = _request(home, {"op": "evm_contract_tx", "to": to, "data": data, "rpc_url": rpc_url, "chain_id": int(chain), "label": label, "value_wei": int(value_wei), "gas_usd": 0})
    tx = response.get("tx")
    if not isinstance(tx, str) or not tx:
        raise RuntimeError("agent response omitted tx hash")
    return tx
