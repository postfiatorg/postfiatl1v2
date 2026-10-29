"""Read the ERC-20 PFT token's state on Base.

Reads the fixed-supply token (crates/pft-token-contracts/src/PFTToken.sol)
over Ethereum JSON-RPC: ``eth_chainId``, ``eth_blockNumber``, ``eth_getCode``
and ``eth_call`` for name(), symbol(), decimals(), totalSupply() and
balanceOf(treasury), all pinned to one block. The token, treasury and chain id
come from a deployment record
(``crates/pft-token-contracts/deployments/pft-token-base-<chain id>/deployment.json``)
or from ``--token``/``--treasury`` on Base (8453) or Base Sepolia (84532).

The command only reads; it never signs or sends a transaction. It reports the
deployed code's size, not its hash (keccak-256 is not in the standard library).

    PYTHONPATH=python python3 -m postfiat_rpc.pft_token \\
        --record crates/pft-token-contracts/deployments/pft-token-base-84532/deployment.json [--table]
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Sequence
from urllib.parse import urlsplit

STATE_SCHEMA = "postfiat.pft_token_state.v1"
DEFAULT_RPC_URL = "https://sepolia.base.org"
BASE_CHAIN_IDS = (8453, 84532)
TIMEOUT_SECONDS = 20.0
MAX_RESPONSE_BYTES = 1024 * 1024
MAX_RECORD_BYTES = 64 * 1024
ADDRESS = re.compile(r"^0x[0-9a-fA-F]{40}$")
HEX = re.compile(r"^0x[0-9a-fA-F]*$")
SELECTORS = {
    "name": "06fdde03",
    "symbol": "95d89b41",
    "decimals": "313ce567",
    "totalSupply": "18160ddd",
    "balanceOf": "70a08231",
}
TABLE_FIELDS = (
    "chain_id",
    "rpc_url",
    "token",
    "name",
    "symbol",
    "decimals",
    "total_supply_pft",
    "total_supply_units",
    "treasury",
    "treasury_balance_pft",
    "treasury_balance_units",
    "treasury_holds_whole_supply",
    "code_size_bytes",
    "block_number",
    "read_at",
)


class PftTokenError(ValueError):
    """Bad input, or an RPC request that failed or returned an error (exit 1)."""


class PftTokenRefused(PftTokenError):
    """A chain-id mismatch, a token without code, or a malformed answer (exit 2)."""


def utc_iso() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def redact_rpc_url(url: str) -> str:
    """The RPC URL up to its host; any path, query or credentials become ``/…``."""

    parts = urlsplit(url)
    host = parts.hostname or ""
    if ":" in host:
        host = f"[{host}]"
    if parts.port is not None:
        host = f"{host}:{parts.port}"
    hidden = parts.path not in ("", "/") or parts.query or parts.fragment or "@" in parts.netloc
    return f"{parts.scheme}://{host}" + ("/…" if hidden else "")


def address(value: Any, field: str) -> str:
    if not isinstance(value, str) or not ADDRESS.match(value):
        raise PftTokenError(f"{field} is not a 0x-prefixed 20-byte address")
    return value


def units_to_decimal(units: int, decimals: int) -> str:
    """``units / 10**decimals`` as an exact decimal string: (10**17, 6) -> "100000000000"."""

    whole, fraction = divmod(units, 10**decimals)
    if not fraction:
        return str(whole)
    return f"{whole}.{str(fraction).rjust(decimals, '0').rstrip('0')}"


def load_record(path: str | Path) -> dict[str, Any]:
    record_path = Path(path).expanduser()
    try:
        if record_path.stat().st_size > MAX_RECORD_BYTES:
            raise PftTokenError(f"`{record_path}` is larger than {MAX_RECORD_BYTES} bytes")
        record = json.loads(record_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise PftTokenError(f"`{record_path}`: {error}") from error
    if not isinstance(record, dict):
        raise PftTokenError(f"`{record_path}` is not a JSON object")
    chain_id = record.get("chain_id")
    if not isinstance(chain_id, int) or isinstance(chain_id, bool) or chain_id <= 0:
        raise PftTokenError(f"`{record_path}` has no positive integer chain_id")
    return {
        "chain_id": chain_id,
        "token": address(record.get("token"), f"`{record_path}` token"),
        "treasury": address(record.get("treasury"), f"`{record_path}` treasury"),
    }


class JsonRpc:
    """A minimal JSON-RPC 2.0 client over ``urllib``."""

    def __init__(self, url: str, timeout: float = TIMEOUT_SECONDS) -> None:
        try:
            parts = urlsplit(url)
            valid = parts.scheme in ("http", "https") and bool(parts.hostname) and parts.port != 0
        except ValueError:
            valid = False
        if not valid:
            raise PftTokenError("--rpc-url must be an http(s) URL")
        self.url = url
        self.timeout = timeout
        self._next_id = 0

    def call(self, method: str, params: list[Any]) -> Any:
        self._next_id += 1
        body = json.dumps({"jsonrpc": "2.0", "id": self._next_id, "method": method, "params": params}).encode()
        request = urllib.request.Request(
            self.url,
            data=body,
            headers={"content-type": "application/json", "user-agent": "postfiat-rpc-pft-token"},
            method="POST",
        )
        try:
            with urllib.request.urlopen(request, timeout=self.timeout) as response:
                raw = response.read(MAX_RESPONSE_BYTES + 1)
        except urllib.error.HTTPError as error:
            raise PftTokenError(f"{method}: HTTP {error.code}") from None
        except (urllib.error.URLError, OSError, ValueError) as error:
            raise PftTokenError(f"{method}: the RPC request failed: {getattr(error, 'reason', error)}") from None
        if len(raw) > MAX_RESPONSE_BYTES:
            raise PftTokenRefused(f"{method}: the answer is larger than {MAX_RESPONSE_BYTES} bytes")
        try:
            answer = json.loads(raw)
        except (UnicodeDecodeError, json.JSONDecodeError):
            raise PftTokenRefused(f"{method}: the answer is not JSON") from None
        if not isinstance(answer, dict) or answer.get("id") != self._next_id:
            raise PftTokenRefused(f"{method}: the answer is not a JSON-RPC response to this request")
        if answer.get("error") is not None:
            error = answer["error"]
            message = error.get("message") if isinstance(error, dict) else error
            raise PftTokenError(f"{method}: RPC error: {str(message)[:200]}")
        if "result" not in answer:
            raise PftTokenRefused(f"{method}: the answer has no result")
        return answer["result"]


def hex_bytes(value: Any, what: str) -> bytes:
    if not isinstance(value, str) or not HEX.match(value) or len(value) % 2:
        raise PftTokenRefused(f"{what}: the answer is not 0x-prefixed hex bytes")
    return bytes.fromhex(value[2:])


def hex_quantity(value: Any, what: str) -> int:
    if not isinstance(value, str) or not re.fullmatch(r"0x[0-9a-fA-F]{1,64}", value):
        raise PftTokenRefused(f"{what}: the answer is not a hex quantity")
    return int(value, 16)


def abi_uint(data: bytes, what: str, bits: int = 256) -> int:
    if len(data) != 32:
        raise PftTokenRefused(f"{what}: expected one 32-byte word, got {len(data)} bytes")
    value = int.from_bytes(data, "big")
    if value >= 1 << bits:
        raise PftTokenRefused(f"{what}: the value does not fit in uint{bits}")
    return value


def abi_string(data: bytes, what: str) -> str:
    if len(data) < 64 or len(data) % 32:
        raise PftTokenRefused(f"{what}: not an ABI-encoded string")
    offset = int.from_bytes(data[:32], "big")
    if offset % 32 or offset + 32 > len(data):
        raise PftTokenRefused(f"{what}: the string offset is out of range")
    length = int.from_bytes(data[offset : offset + 32], "big")
    if offset + 32 + length > len(data):
        raise PftTokenRefused(f"{what}: the string length is out of range")
    try:
        return data[offset + 32 : offset + 32 + length].decode("utf-8")
    except UnicodeDecodeError:
        raise PftTokenRefused(f"{what}: the string is not UTF-8") from None


def read_token_state(
    rpc_url: str = DEFAULT_RPC_URL,
    *,
    record: str | Path | None = None,
    token: str | None = None,
    treasury: str | None = None,
    timeout: float = TIMEOUT_SECONDS,
) -> dict[str, Any]:
    """The token's state as one JSON object.

    Raises :class:`PftTokenRefused` for a chain-id mismatch, a token without
    code or a malformed answer, and :class:`PftTokenError` otherwise.
    """

    if (record is None) == (token is None):
        raise PftTokenError("give exactly one of --record or --token")
    if record is not None:
        loaded = load_record(record)
        token, expected_chain_ids = loaded["token"], (loaded["chain_id"],)
        treasury = address(treasury, "--treasury") if treasury else loaded["treasury"]
    else:
        token = address(token, "--token")
        if not treasury:
            raise PftTokenError("--treasury is required with --token")
        treasury = address(treasury, "--treasury")
        expected_chain_ids = BASE_CHAIN_IDS

    rpc = JsonRpc(rpc_url, timeout)
    chain_id = hex_quantity(rpc.call("eth_chainId", []), "eth_chainId")
    if chain_id not in expected_chain_ids:
        expected = " or ".join(str(value) for value in expected_chain_ids)
        raise PftTokenRefused(f"the RPC endpoint is on chain {chain_id}, expected {expected}")
    block_number = hex_quantity(rpc.call("eth_blockNumber", []), "eth_blockNumber")
    block = hex(block_number)

    code = hex_bytes(rpc.call("eth_getCode", [token, block]), "eth_getCode")
    if not code:
        raise PftTokenRefused(f"{token} has no code on chain {chain_id} at block {block_number}")

    def call(function: str, argument: str = "") -> bytes:
        data = "0x" + SELECTORS[function] + argument
        return hex_bytes(rpc.call("eth_call", [{"to": token, "data": data}, block]), f"{function}()")

    name = abi_string(call("name"), "name()")
    symbol = abi_string(call("symbol"), "symbol()")
    decimals = abi_uint(call("decimals"), "decimals()", bits=8)
    total_supply = abi_uint(call("totalSupply"), "totalSupply()")
    balance = abi_uint(call("balanceOf", treasury[2:].lower().rjust(64, "0")), "balanceOf(treasury)")
    return {
        "schema": STATE_SCHEMA,
        "chain_id": chain_id,
        "rpc_url": redact_rpc_url(rpc_url),
        "record": str(record) if record is not None else None,
        "token": token,
        "treasury": treasury,
        "name": name,
        "symbol": symbol,
        "decimals": decimals,
        "total_supply_units": str(total_supply),
        "total_supply_pft": units_to_decimal(total_supply, decimals),
        "treasury_balance_units": str(balance),
        "treasury_balance_pft": units_to_decimal(balance, decimals),
        "treasury_holds_whole_supply": balance == total_supply,
        "code_size_bytes": len(code),
        "block_number": block_number,
        "read_at": utc_iso(),
    }


def render_table(state: dict[str, Any]) -> str:
    def cell(value: Any) -> str:
        return json.dumps(value) if isinstance(value, bool) else str(value)

    width = max(len(field) for field in TABLE_FIELDS)
    return "".join(f"{field.ljust(width)}  {cell(state[field])}\n" for field in TABLE_FIELDS)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="python -m postfiat_rpc.pft_token",
        description="Read the ERC-20 PFT token's state on Base over JSON-RPC (read only).",
    )
    parser.add_argument("--rpc-url", default=DEFAULT_RPC_URL, help=f"JSON-RPC endpoint (default {DEFAULT_RPC_URL})")
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--record", metavar="PATH", help="a deployment.json record: token, treasury and chain id")
    source.add_argument("--token", metavar="ADDRESS", help="the token address on Base (8453) or Base Sepolia (84532)")
    parser.add_argument("--treasury", metavar="ADDRESS", help="the treasury address (default from the record)")
    parser.add_argument("--timeout", type=float, default=TIMEOUT_SECONDS, help="seconds per RPC request")
    parser.add_argument("--table", action="store_true", help="print aligned lines instead of JSON")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        state = read_token_state(
            args.rpc_url, record=args.record, token=args.token, treasury=args.treasury, timeout=args.timeout
        )
    except PftTokenRefused as error:
        print(f"pft-token-refused: {error}", file=sys.stderr)
        return 2
    except PftTokenError as error:
        print(f"pft-token-failed: {error}", file=sys.stderr)
        return 1
    if args.table:
        print(render_table(state), end="")
    else:
        print(json.dumps(state, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
