"""Valuation policy for basis-trade NAV assets (nSOL / nETH pattern).

A basis NAV asset holds a long staked position in asset X plus a short X
perp on Hyperliquid (with USDC margin), so NAV floats with the carry
(staking yield + funding) while price exposure nets toward zero.

The policy answers three questions deterministically:
1. VALUATION — every leg is marked at the venue where the hedge settles
   (Hyperliquid's mid for X), so the hedge nets exactly in the NAV math:
   marking the legs from different oracles would inject phantom basis
   into a portfolio whose purpose is having none. The mark source is
   part of this policy and therefore part of the content-addressed
   policy hash; external observers are free to recompute NAV with their
   own oracle, and a real discrepancy is what the bonded challenge path
   exists for.
2. INVARIANTS — hedge ratio within a declared band, margin ratio above
   a declared floor. Violations are FAIL verdicts even when the value
   matches: a basis product that is no longer hedged is misrepresented
   even if today's NAV is accurate.
3. IDENTITY — the policy parameters hash to a valuation_policy_hash that
   is registered on the proof profile, so what observers enforce is
   itself a registered, immutable fact.
"""

from __future__ import annotations

import hashlib
import json
from decimal import Decimal
from typing import Any

POLICY_SCHEMA = "nav-basis-policy-v1"
POLICY_DOMAIN = b"postfiat.nav_valuation_policy.v1"
LAMPORTS_PER_SOL = Decimal(1_000_000_000)


def policy_descriptor(
    asset_symbol: str,
    solana_accounts: list[str],
    hyperliquid_accounts: list[str],
    mark_source: str = "hyperliquid-mid",
    hedge_band_bp: int = 500,
    min_margin_ratio_bp: int = 20_000,
    stake_haircut_bp: int = 0,
) -> dict[str, Any]:
    return {
        "schema": POLICY_SCHEMA,
        "asset_symbol": asset_symbol,
        "legs": {
            "solana_accounts": sorted(solana_accounts),
            "hyperliquid_accounts": sorted(hyperliquid_accounts),
        },
        "mark_source": mark_source,
        "hedge_band_bp": hedge_band_bp,
        "min_margin_ratio_bp": min_margin_ratio_bp,
        "stake_haircut_bp": stake_haircut_bp,
    }


def policy_hash(descriptor: dict[str, Any]) -> str:
    canonical = json.dumps(descriptor, sort_keys=True, separators=(",", ":")).encode("utf-8")
    hasher = hashlib.sha3_384()
    hasher.update(POLICY_DOMAIN)
    hasher.update(b"\x00")
    hasher.update(canonical)
    return hasher.hexdigest()


def _position_for(hl_observation: dict[str, Any], symbol: str) -> dict[str, Any] | None:
    for position in hl_observation["perp"]["positions"]:
        if position["coin"] == symbol:
            return position
    return None


def evaluate(
    descriptor: dict[str, Any],
    solana_observations: list[dict[str, Any]],
    hyperliquid_observation: dict[str, Any],
    mark: Decimal,
) -> dict[str, Any]:
    """Compute portfolio NAV and strategy invariants from normalized
    observations. Returns values plus a verdict; everything is exact
    Decimal arithmetic over the verbatim observation strings."""
    symbol = descriptor["asset_symbol"]
    haircut = Decimal(1) - Decimal(descriptor["stake_haircut_bp"]) / Decimal(10_000)

    staked_lamports = sum(
        Decimal(obs["balance_lamports"]) + Decimal(obs["stake"]["delegated_lamports"])
        for obs in solana_observations
    )
    staked_qty = staked_lamports / LAMPORTS_PER_SOL
    staked_value = staked_qty * mark * haircut

    hl_equity = Decimal(hyperliquid_observation["perp"]["account_value"])
    nav = staked_value + hl_equity

    position = _position_for(hyperliquid_observation, symbol)
    short_qty = -Decimal(position["szi"]) if position else Decimal(0)

    hedge_gap_bp = (
        abs(staked_qty - short_qty) / staked_qty * Decimal(10_000)
        if staked_qty > 0
        else Decimal(10_000)
    )
    margin_used = Decimal(hyperliquid_observation["perp"]["total_margin_used"])
    margin_ratio_bp = (
        hl_equity / margin_used * Decimal(10_000) if margin_used > 0 else Decimal(10_000_000)
    )

    hedged = hedge_gap_bp <= Decimal(descriptor["hedge_band_bp"])
    margined = margin_ratio_bp >= Decimal(descriptor["min_margin_ratio_bp"])

    return {
        "schema": "nav-basis-evaluation-v1",
        "policy_hash": policy_hash(descriptor),
        "mark": str(mark),
        "staked_qty": str(staked_qty),
        "staked_value": str(staked_value),
        "hyperliquid_equity": str(hl_equity),
        "nav": str(nav),
        "short_qty": str(short_qty),
        "hedge_gap_bp": str(round(hedge_gap_bp, 4)),
        "margin_ratio_bp": str(round(margin_ratio_bp, 2)),
        "invariants": {
            "hedge_within_band": hedged,
            "margin_above_floor": margined,
        },
        "strategy_pass": bool(hedged and margined),
    }
