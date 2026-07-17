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
from typing import Any

MAINNET_INFO_URL = "https://api.hyperliquid.xyz/info"
TESTNET_INFO_URL = "https://api.hyperliquid-testnet.xyz/info"
OBSERVATION_DOMAIN = b"postfiat.nav_observation.hyperliquid.v1"
SOURCE_CLASS_MAINNET = "hyperliquid"
SOURCE_CLASS_TESTNET = "hyperliquid-testnet"


def _post_info(payload: dict[str, Any], info_url: str, timeout: float = 15.0) -> Any:
    request = urllib.request.Request(
        info_url,
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return json.load(response)


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
    margin = (perp_state or {}).get("marginSummary", {}) or {}
    positions = []
    for entry in (perp_state or {}).get("assetPositions", []) or []:
        position = entry.get("position", {}) or {}
        positions.append(
            {
                "coin": str(position.get("coin", "")),
                "szi": str(position.get("szi", "0")),
                "entry_px": str(position.get("entryPx") or "0"),
                "position_value": str(position.get("positionValue", "0")),
                "unrealized_pnl": str(position.get("unrealizedPnl", "0")),
                "margin_used": str(position.get("marginUsed", "0")),
            }
        )
    positions.sort(key=lambda item: item["coin"])

    balances = []
    for entry in (spot_state or {}).get("balances", []) or []:
        balances.append(
            {
                "coin": str(entry.get("coin", "")),
                "total": str(entry.get("total", "0")),
                "hold": str(entry.get("hold", "0")),
            }
        )
    balances.sort(key=lambda item: item["coin"])

    observation = {
        "schema": "nav-observation-hyperliquid-v1",
        "source_class": source_class,
        "address": address.lower(),
        "captured_at_unix": int(captured_at_unix if captured_at_unix is not None else time.time()),
        "perp": {
            "account_value": str(margin.get("accountValue", "0")),
            "total_ntl_pos": str(margin.get("totalNtlPos", "0")),
            "total_margin_used": str(margin.get("totalMarginUsed", "0")),
            "withdrawable": str((perp_state or {}).get("withdrawable", "0")),
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
