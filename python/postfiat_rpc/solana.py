"""Solana source adapter for NAV multi-fetch observation.

Solana account state is public and address-indexed via JSON-RPC: balances
(getBalance), parsed stake accounts (getAccountInfo jsonParsed), and SPL
token balances (getTokenAccountBalance). Like Hyperliquid's info endpoint,
this makes Solana legs multi-fetch friendly: N observers query the same
addresses with no credentials and attest.

Normalization mirrors the Hyperliquid adapter: values kept as exact
integers (lamports) or verbatim strings, deterministic ordering, capture
timestamp excluded from the comparable view, domain-tagged SHA3-384
observation roots.
"""

from __future__ import annotations

import hashlib
import json
import time
import urllib.request
from typing import Any

MAINNET_RPC_URL = "https://api.mainnet-beta.solana.com"
OBSERVATION_DOMAIN = b"postfiat.nav_observation.solana.v1"
SOURCE_CLASS_MAINNET = "solana"
LAMPORTS_PER_SOL = 1_000_000_000


def _rpc(method: str, params: list[Any], rpc_url: str, timeout: float = 15.0) -> Any:
    payload = {"jsonrpc": "2.0", "id": 1, "method": method, "params": params}
    request = urllib.request.Request(
        rpc_url,
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=timeout) as response:
        body = json.load(response)
    if "error" in body and body["error"]:
        raise RuntimeError(f"solana rpc error: {body['error']}")
    return body.get("result")


def fetch_balance_lamports(address: str, rpc_url: str = MAINNET_RPC_URL) -> int:
    result = _rpc("getBalance", [address], rpc_url)
    return int(result["value"])


def fetch_account_parsed(address: str, rpc_url: str = MAINNET_RPC_URL) -> Any:
    result = _rpc(
        "getAccountInfo", [address, {"encoding": "jsonParsed"}], rpc_url
    )
    return result.get("value")


def stake_summary(parsed_account: Any) -> dict[str, Any]:
    """Extract delegation facts from a parsed stake account. Returns zeros
    for non-stake accounts so plain balance accounts normalize uniformly."""
    summary = {"delegated_lamports": 0, "rent_exempt_reserve_lamports": 0, "voter": ""}
    if not parsed_account:
        return summary
    data = parsed_account.get("data", {})
    parsed = data.get("parsed", {}) if isinstance(data, dict) else {}
    if parsed.get("type") != "delegated":
        info = parsed.get("info", {}) if isinstance(parsed, dict) else {}
    else:
        info = parsed.get("info", {})
    stake = (info or {}).get("stake") or {}
    delegation = stake.get("delegation") or {}
    meta = (info or {}).get("meta") or {}
    if delegation:
        summary["delegated_lamports"] = int(delegation.get("stake", 0))
        summary["voter"] = str(delegation.get("voter", ""))
    if meta:
        summary["rent_exempt_reserve_lamports"] = int(meta.get("rentExemptReserve", 0))
    return summary


def normalize_observation(
    address: str,
    balance_lamports: int,
    stake: dict[str, Any] | None = None,
    source_class: str = SOURCE_CLASS_MAINNET,
    captured_at_unix: int | None = None,
) -> dict[str, Any]:
    stake = stake or {"delegated_lamports": 0, "rent_exempt_reserve_lamports": 0, "voter": ""}
    return {
        "schema": "nav-observation-solana-v1",
        "source_class": source_class,
        "address": address,
        "captured_at_unix": int(captured_at_unix if captured_at_unix is not None else time.time()),
        "balance_lamports": int(balance_lamports),
        "stake": {
            "delegated_lamports": int(stake.get("delegated_lamports", 0)),
            "rent_exempt_reserve_lamports": int(stake.get("rent_exempt_reserve_lamports", 0)),
            "voter": str(stake.get("voter", "")),
        },
    }


def comparable_view(observation: dict[str, Any]) -> dict[str, Any]:
    view = json.loads(json.dumps(observation, sort_keys=True))
    view.pop("captured_at_unix", None)
    return view


def observation_root(observation: dict[str, Any]) -> str:
    canonical = json.dumps(
        comparable_view(observation), sort_keys=True, separators=(",", ":")
    ).encode("utf-8")
    hasher = hashlib.sha3_384()
    hasher.update(OBSERVATION_DOMAIN)
    hasher.update(b"\x00")
    hasher.update(canonical)
    return hasher.hexdigest()


def observe(address: str, rpc_url: str = MAINNET_RPC_URL) -> dict[str, Any]:
    balance = fetch_balance_lamports(address, rpc_url)
    parsed = fetch_account_parsed(address, rpc_url)
    stake = stake_summary(parsed)
    observation = normalize_observation(address, balance, stake)
    return {"observation": observation, "observation_root": observation_root(observation)}
