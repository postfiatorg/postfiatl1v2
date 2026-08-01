"""Client for the provider-neutral PostFiat constrained signer protocol."""

from __future__ import annotations

import json
import socket
from pathlib import Path
from typing import Any


REQUEST_SCHEMA = "postfiat.constrained_signer.request.v1"
RESPONSE_SCHEMA = "postfiat.constrained_signer.response.v1"
MAX_MESSAGE_BYTES = 256 * 1024


class ConstrainedSignerError(RuntimeError):
    """A signer request was rejected or could not be completed."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


def call_signer(
    socket_path: str | Path,
    request: dict[str, Any],
    *,
    timeout: float = 1200.0,
) -> dict[str, Any]:
    """Send one bounded JSON request over a local Unix-domain socket."""

    payload = dict(request)
    payload.setdefault("schema", REQUEST_SCHEMA)
    encoded = json.dumps(payload, separators=(",", ":"), sort_keys=True).encode() + b"\n"
    if len(encoded) > MAX_MESSAGE_BYTES:
        raise ConstrainedSignerError("signer_request_too_large", "signer request exceeds 256 KiB")

    received = bytearray()
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
        client.settimeout(timeout)
        client.connect(str(socket_path))
        client.sendall(encoded)
        while b"\n" not in received:
            chunk = client.recv(min(65536, MAX_MESSAGE_BYTES + 1 - len(received)))
            if not chunk:
                break
            received.extend(chunk)
            if len(received) > MAX_MESSAGE_BYTES:
                raise ConstrainedSignerError(
                    "signer_response_too_large", "signer response exceeds 256 KiB"
                )

    line = bytes(received).split(b"\n", 1)[0]
    try:
        response = json.loads(line)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ConstrainedSignerError(
            "signer_response_invalid", "signer returned malformed JSON"
        ) from error
    if not isinstance(response, dict) or response.get("schema") != RESPONSE_SCHEMA:
        raise ConstrainedSignerError(
            "signer_response_schema_invalid", "signer returned the wrong response schema"
        )
    if response.get("ok") is not True:
        raise ConstrainedSignerError(
            str(response.get("code") or "signer_rejected"),
            str(response.get("message") or "constrained signer rejected the request"),
        )
    return response


def signer_status(socket_path: str | Path, *, timeout: float = 5.0) -> dict[str, Any]:
    return call_signer(socket_path, {"op": "status"}, timeout=timeout)


def submit_evm_transaction(
    socket_path: str | Path,
    *,
    chain_id: int,
    transaction_kind: str,
    target_contract: str,
    calldata: str,
    native_value_wei: int,
    maximum_fee_wei: int,
    route_id: str,
    route_config_digest: str,
    label: str,
    idempotency_key: str,
    timeout: float = 1200.0,
) -> dict[str, Any]:
    return call_signer(
        socket_path,
        {
            "op": "submit_evm_transaction",
            "chain_id": chain_id,
            "transaction_kind": transaction_kind,
            "target_contract": target_contract,
            "calldata": calldata,
            "native_value_wei": native_value_wei,
            "maximum_fee_wei": maximum_fee_wei,
            "route_id": route_id,
            "route_config_digest": route_config_digest,
            "label": label,
            "idempotency_key": idempotency_key,
        },
        timeout=timeout,
    )
