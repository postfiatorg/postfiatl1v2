"""Hyperliquid source adapter for NAV multi-fetch observation.

Hyperliquid account state is public and address-indexed: anyone can POST
to the /info endpoint with a user address and receive perp clearinghouse
state and spot balances — no API key, no signature. That makes it the
reference source class for the multi-fetch-quorum proof profile: N
independent observers fetch the same address, normalize identically,
hash, and attest.

Observers MUST query the master account address; agent wallet addresses
return empty data per the official docs. Subaccounts and vaults are
separate addresses and must be enumerated in the declared perimeter.

Normalization is deliberately conservative and deterministic: values are
kept as the exact decimal strings the API returns (no float round-trips),
lists are sorted by coin, and the observation root is a domain-tagged
SHA3-384 over canonical JSON, mirroring the chain's hashing discipline.
"""

from __future__ import annotations

import hashlib
import json
import time
import urllib.request
from decimal import Decimal, InvalidOperation
from typing import Any

MAINNET_INFO_URL = "https://api.hyperliquid.xyz/info"
TESTNET_INFO_URL = "https://api.hyperliquid-testnet.xyz/info"
OBSERVATION_DOMAIN = b"postfiat.nav_observation.hyperliquid.v1"
SOURCE_CLASS_MAINNET = "hyperliquid"
SOURCE_CLASS_TESTNET = "hyperliquid-testnet"
MAX_INFO_RESPONSE_BYTES = 1_048_576


def _object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be an object")
    return value


def _decimal(value: Any, label: str) -> str:
    if not isinstance(value, str):
        raise ValueError(f"{label} must be an exact decimal string")
    try:
        if not Decimal(value).is_finite():
            raise ValueError(f"{label} must be finite")
    except InvalidOperation as error:
        raise ValueError(f"{label} must be an exact decimal string") from error
    return value


def _post_info(payload: dict[str, Any], info_url: str, timeout: float = 15.0) -> Any:
    request = urllib.request.Request(
        info_url,
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=timeout) as response:
        body = response.read(MAX_INFO_RESPONSE_BYTES + 1)
    if len(body) > MAX_INFO_RESPONSE_BYTES:
        raise ValueError("venue response exceeded the byte limit")
    return json.loads(body)


def fetch_perp_state(address: str, info_url: str = MAINNET_INFO_URL) -> Any:
    return _post_info({"type": "clearinghouseState", "user": address}, info_url)


def fetch_spot_state(address: str, info_url: str = MAINNET_INFO_URL) -> Any:
    return _post_info({"type": "spotClearinghouseState", "user": address}, info_url)


def fetch_all_mids(info_url: str = MAINNET_INFO_URL) -> Any:
    """Venue mid prices keyed by coin symbol (decimal strings)."""
    return _post_info({"type": "allMids"}, info_url)


def normalize_observation(
    address: str,
    perp_state: Any,
    spot_state: Any,
    source_class: str = SOURCE_CLASS_MAINNET,
    captured_at_unix: int | None = None,
) -> dict[str, Any]:
    """Reduce raw API responses to the deterministic fields observers
    compare. Decimal strings are preserved verbatim; ordering is fixed."""
    perp = _object(perp_state, "perp state")
    spot = _object(spot_state, "spot state")
    margin = _object(perp.get("marginSummary"), "margin summary")
    raw_positions = perp.get("assetPositions")
    raw_balances = spot.get("balances")
    if not isinstance(raw_positions, list) or not isinstance(raw_balances, list):
        raise ValueError("venue positions and balances must be arrays")
    positions = []
    for entry in raw_positions:
        position = _object(_object(entry, "position entry").get("position"), "position")
        coin = position.get("coin")
        if not isinstance(coin, str) or not coin:
            raise ValueError("position coin is missing")
        entry_px = position.get("entryPx")
        positions.append(
            {
                "coin": coin,
                "szi": _decimal(position.get("szi"), "position size"),
                "entry_px": (
                    "0" if entry_px is None else _decimal(entry_px, "entry price")
                ),
                "position_value": _decimal(position.get("positionValue"), "position value"),
                "unrealized_pnl": _decimal(position.get("unrealizedPnl"), "unrealized PnL"),
                "margin_used": _decimal(position.get("marginUsed"), "margin used"),
            }
        )
    positions.sort(key=lambda item: item["coin"])

    balances = []
    for entry in raw_balances:
        balance = _object(entry, "balance")
        coin = balance.get("coin")
        if not isinstance(coin, str) or not coin:
            raise ValueError("balance coin is missing")
        balances.append(
            {
                "coin": coin,
                "total": _decimal(balance.get("total"), "balance total"),
                "hold": _decimal(balance.get("hold"), "balance hold"),
            }
        )
    balances.sort(key=lambda item: item["coin"])

    observation = {
        "schema": "nav-observation-hyperliquid-v1",
        "source_class": source_class,
        "address": address.lower(),
        "captured_at_unix": int(captured_at_unix if captured_at_unix is not None else time.time()),
        "perp": {
            "account_value": _decimal(margin.get("accountValue"), "account value"),
            "total_ntl_pos": _decimal(margin.get("totalNtlPos"), "total notional position"),
            "total_margin_used": _decimal(margin.get("totalMarginUsed"), "total margin used"),
            "withdrawable": _decimal(perp.get("withdrawable"), "withdrawable"),
            "positions": positions,
        },
        "spot": {"balances": balances},
    }
    return observation


def comparable_view(observation: dict[str, Any]) -> dict[str, Any]:
    """The slice observers compare across fetches: everything except the
    capture timestamp, which legitimately differs per observer."""
    view = json.loads(json.dumps(observation, sort_keys=True))
    view.pop("captured_at_unix", None)
    return view


def observation_root(observation: dict[str, Any]) -> str:
    """Domain-tagged SHA3-384 over the canonical comparable view."""
    canonical = json.dumps(
        comparable_view(observation), sort_keys=True, separators=(",", ":")
    ).encode("utf-8")
    hasher = hashlib.sha3_384()
    hasher.update(OBSERVATION_DOMAIN)
    hasher.update(b"\x00")
    hasher.update(canonical)
    return hasher.hexdigest()


def observe(
    address: str,
    info_url: str = MAINNET_INFO_URL,
    source_class: str = SOURCE_CLASS_MAINNET,
) -> dict[str, Any]:
    """One full observation: fetch both state surfaces, normalize, hash."""
    perp = fetch_perp_state(address, info_url)
    spot = fetch_spot_state(address, info_url)
    observation = normalize_observation(address, perp, spot, source_class)
    return {
        "observation": observation,
        "observation_root": observation_root(observation),
    }
