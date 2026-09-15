"""Single-connection, read-only status round-trip probe for a TCP RPC endpoint.

Run with ``PYTHONPATH=python python3 -m postfiat_rpc.rpc_probe --endpoint 127.0.0.1:27650``.
"""

from __future__ import annotations

import argparse
import json
import socket
import sys
import time
from dataclasses import dataclass

from .client import Endpoint, RPC_VERSION


class RpcProbeError(RuntimeError):
    """A status round trip failed."""


@dataclass(frozen=True)
class RpcProbeResult:
    height: int
    tip_prefix: str
    round_trip_ms: int


def rpc_probe(endpoint: Endpoint, timeout_seconds: float = 5.0) -> RpcProbeResult:
    """Send one status request over one TCP connection, with no retry."""
    if timeout_seconds <= 0:
        raise ValueError("timeout_seconds must be positive")
    started = time.monotonic()
    deadline = started + timeout_seconds
    try:
        family, socktype, proto, _, address = socket.getaddrinfo(
            endpoint.host, endpoint.port, type=socket.SOCK_STREAM
        )[0]
    except (OSError, IndexError) as error:
        raise RpcProbeError(f"connect refused: address resolution failed: {error}") from error
    remaining = deadline - time.monotonic()
    if remaining <= 0:
        raise RpcProbeError("connect timeout")
    with socket.socket(family, socktype, proto) as stream:
        stream.settimeout(remaining)
        try:
            stream.connect(address)
        except socket.timeout as error:
            raise RpcProbeError("connect timeout") from error
        except OSError as error:
            raise RpcProbeError(f"connect refused: {error}") from error
        request = {"version": RPC_VERSION, "id": "rpc-probe", "method": "status", "params": {}}
        wire = json.dumps(request, separators=(",", ":")).encode() + b"\n"
        try:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise RpcProbeError("no response within timeout")
            stream.settimeout(remaining)
            stream.sendall(wire)
            stream.shutdown(socket.SHUT_WR)
        except socket.timeout as error:
            raise RpcProbeError("no response within timeout") from error
        except OSError as error:
            raise RpcProbeError(f"send failed: {error}") from error
        response_bytes = bytearray()
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise RpcProbeError("no response within timeout")
            stream.settimeout(remaining)
            try:
                chunk = stream.recv(4096)
            except socket.timeout as error:
                raise RpcProbeError("no response within timeout") from error
            except OSError as error:
                raise RpcProbeError(f"read failed: {error}") from error
            if not chunk:
                if not response_bytes:
                    raise RpcProbeError("no response: peer closed")
                raise RpcProbeError("malformed response: missing newline")
            line, newline, _rest = chunk.partition(b"\n")
            response_bytes.extend(line)
            if len(response_bytes) > 1_048_576:
                raise RpcProbeError("malformed response: response too large")
            if newline:
                break
    try:
        response = json.loads(response_bytes.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise RpcProbeError(f"malformed response: {error}") from error
    if not isinstance(response, dict):
        raise RpcProbeError("malformed response: envelope must be an object")
    if response.get("version") != RPC_VERSION or response.get("id") != "rpc-probe":
        raise RpcProbeError("malformed response: id or version mismatch")
    if type(response.get("ok")) is not bool:
        raise RpcProbeError("malformed response: missing boolean ok")
    if not response["ok"]:
        error = response.get("error")
        if not isinstance(error, dict) or not isinstance(error.get("code"), str) or not error["code"]:
            raise RpcProbeError("malformed response: missing error code")
        raise RpcProbeError(f"ok:false: {error['code']}")
    if response.get("error") is not None:
        raise RpcProbeError("malformed response: unexpected error")
    if not isinstance(response.get("events"), list):
        raise RpcProbeError("malformed response: missing events")
    result = response.get("result")
    if not isinstance(result, dict):
        raise RpcProbeError("malformed response: status result must be an object")
    height = result.get("block_height")
    tip = result.get("block_tip_hash")
    if type(height) is not int or height < 0:
        raise RpcProbeError("malformed response: missing block_height")
    if not isinstance(tip, str) or not tip:
        raise RpcProbeError("malformed response: missing block_tip_hash")
    if time.monotonic() > deadline:
        raise RpcProbeError("no response within timeout")
    return RpcProbeResult(height, tip[:12], int((time.monotonic() - started) * 1000))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--endpoint", required=True, help="TCP endpoint as host:port")
    parser.add_argument("--timeout-seconds", type=float, default=5.0)
    args = parser.parse_args(argv)
    try:
        endpoint = Endpoint.parse(args.endpoint)
        probe = rpc_probe(endpoint, args.timeout_seconds)
    except (ValueError, RpcProbeError) as error:
        print(str(error).replace("\n", " "), file=sys.stderr)
        return 1
    print(f"height={probe.height} tip={probe.tip_prefix} round_trip_ms={probe.round_trip_ms}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
