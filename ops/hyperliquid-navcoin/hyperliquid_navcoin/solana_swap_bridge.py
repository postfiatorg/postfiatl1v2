"""Solana NAVCoin swap readiness gate and deterministic dust-swap rehearsal bridge."""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from typing import Any

from .attestor_verifier import (
    AttestorQuorumReport,
    NonIssuerAttestation,
    RegisteredAttestor,
    verify_non_issuer_attestor_quorum,
)
from .hypercore_verifier import (
    HyperCorePolicy,
    HyperCoreVerificationResult,
    compute_nav_per_unit,
    nav_reserve_packet_id,
    verify_hypercore_snapshot,
)

REHEARSAL_SCHEMA_V1 = "postfiat.hyperliquid_solana_nav_rehearsal.v1"
DEFAULT_MAX_OBSERVER_LAG_SLOTS = 150
DEFAULT_EPOCH_BOUNDARY_CLOSE_SLOTS = 3_344  # ~12 min matching ops/sol-live/mainnet-health.py


@dataclass(frozen=True)
class SolanaClusterReadiness:
    """Solana bridge and cluster readiness snapshot (mirrors ops/sol-live/mainnet-health.py)."""

    route_id: str
    anchored_epoch: int
    current_epoch: int
    finalized_slot: int
    observer_slot: int
    epoch_end_slot: int
    route_profile_hash: str
    expected_profile_hash: str
    prover_healthy: bool
    relay_ready: bool
    solana_navcoin_mint: str
    solana_usdc_mint: str
    health_age_seconds: int = 0


@dataclass(frozen=True)
class SwapReadinessConfig:
    chain_id: str
    asset_id: str
    asset_code: str
    asset_issuer: str
    asset_precision: int
    circulating_supply: int
    proof_profile: str
    epoch: int
    min_attestations: int = 2
    max_snapshot_age_seconds: int = 300
    max_attestation_age_seconds: int = 300
    max_observer_lag_slots: int = DEFAULT_MAX_OBSERVER_LAG_SLOTS
    epoch_boundary_close_slots: int = DEFAULT_EPOCH_BOUNDARY_CLOSE_SLOTS
    max_health_age_seconds: int = 120
    max_spread_bps: int = 100
    max_dust_usdc_atoms: int = 10_000_000  # 10.00 USDC cap for dust-swap rehearsals
    min_swap_usdc_atoms: int = 100_000     # 0.10 USDC floor


@dataclass(frozen=True)
class DustSwapRequest:
    """Deterministic Solana dust-swap rehearsal intent (pfUSDC <-> Hyperliquid NAVCoin)."""

    side: str  # "buy_navcoin" (USDC -> NAVCoin) | "sell_navcoin" (NAVCoin -> USDC)
    usdc_atoms: int | None = None
    navcoin_atoms: int | None = None
    spread_bps: int = 25
    solana_trader: str = ""
    pftl_recipient: str = ""


def _canonical_reserve_packet_hash(
    *,
    asset_id: str,
    epoch: int,
    verified_net_assets_usd_e6: int,
    circulating_supply: int,
    nav_per_unit_usd_e6: int,
    observation_root: str,
    snapshot_commitment: str,
) -> str:
    canonical = json.dumps(
        {
            "schema": "postfiat.hyperliquid_nav.reserve_packet.v1",
            "asset_id": asset_id,
            "epoch": epoch,
            "valuation_unit": "usd_1e6",
            "verified_net_assets": verified_net_assets_usd_e6,
            "circulating_supply": circulating_supply,
            "nav_per_unit": nav_per_unit_usd_e6,
            "observation_root": observation_root,
            "snapshot_commitment": snapshot_commitment.lower(),
        },
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    hasher = hashlib.sha3_384()
    hasher.update(b"postfiat.hyperliquid_nav.reserve_packet_hash.v1")
    hasher.update(b"\x00")
    hasher.update(canonical)
    return hasher.hexdigest()


def evaluate_solana_navcoin_readiness(
    *,
    config: SwapReadinessConfig,
    hypercore_result: HyperCoreVerificationResult,
    quorum_report: AttestorQuorumReport,
    solana: SolanaClusterReadiness,
) -> dict[str, Any]:
    """Fail-closed readiness gate combining HyperCore proof, non-issuer quorum, and Solana health."""
    blockers: list[str] = []

    if hypercore_result.verified_net_assets_usd_e6 <= 0:
        blockers.append("non_positive_verified_net_assets")
    if not quorum_report.quorum_met:
        blockers.append(
            f"attestor_quorum_not_met:{quorum_report.valid_non_issuer_count}/{quorum_report.required_quorum}"
        )
    if solana.current_epoch != solana.anchored_epoch:
        blockers.append(
            f"solana_epoch_mismatch:current={solana.current_epoch}:anchored={solana.anchored_epoch}"
        )
    if solana.finalized_slot >= solana.epoch_end_slot - config.epoch_boundary_close_slots:
        blockers.append(f"solana_epoch_boundary_closed:end_slot={solana.epoch_end_slot}")
    if solana.finalized_slot - solana.observer_slot > config.max_observer_lag_slots:
        blockers.append(
            f"solana_observer_lag:{solana.finalized_slot - solana.observer_slot}>{config.max_observer_lag_slots}"
        )
    if solana.route_profile_hash != solana.expected_profile_hash:
        blockers.append("solana_route_profile_hash_mismatch")
    if solana.health_age_seconds > config.max_health_age_seconds:
        blockers.append(
            f"solana_health_stale:{solana.health_age_seconds}s>{config.max_health_age_seconds}s"
        )
    if not solana.prover_healthy:
        blockers.append("solana_prover_unhealthy")
    if not solana.relay_ready:
        blockers.append("solana_relay_not_ready")

    return {
        "ready": len(blockers) == 0,
        "route_id": solana.route_id,
        "blockers": blockers,
        "hypercore_commitment": hypercore_result.commitment,
        "valid_non_issuer_attestors": list(quorum_report.valid_attestors),
        "solana_finalized_slot": solana.finalized_slot,
        "solana_observer_slot": solana.observer_slot,
        "solana_epoch": solana.current_epoch,
    }


def rehearse_dust_swap(
    *,
    config: SwapReadinessConfig,
    policy: HyperCorePolicy,
    snapshot_receipt: dict[str, Any],
    registered_attestors: list[RegisteredAttestor],
    attestations: list[NonIssuerAttestation],
    solana: SolanaClusterReadiness,
    swap: DustSwapRequest,
    now_unix: int,
) -> dict[str, Any]:
    """Run full deterministic Hyperliquid NAVCoin reserve-to-Solana dust-swap rehearsal."""
    # 1. Verify HyperCoreReserveReader snapshot.
    hc = verify_hypercore_snapshot(
        snapshot_receipt,
        policy,
        now_unix=now_unix,
        max_snapshot_age_seconds=config.max_snapshot_age_seconds,
    )

    # 2. Compute universal Pillar 7 / C4 exact floor NAV per unit & packet hash.
    nav_per_unit = compute_nav_per_unit(
        hc.verified_net_assets_usd_e6,
        config.circulating_supply,
        config.asset_precision,
    )
    reserve_packet_hash = _canonical_reserve_packet_hash(
        asset_id=config.asset_id,
        epoch=config.epoch,
        verified_net_assets_usd_e6=hc.verified_net_assets_usd_e6,
        circulating_supply=config.circulating_supply,
        nav_per_unit_usd_e6=nav_per_unit,
        observation_root=hc.observation_root,
        snapshot_commitment=hc.commitment,
    )
    packet_id = nav_reserve_packet_id(config.asset_id, config.epoch, reserve_packet_hash)

    # 3. Verify non-issuer attestor signatures and quorum.
    quorum = verify_non_issuer_attestor_quorum(
        asset_issuer=config.asset_issuer,
        asset_id=config.asset_id,
        epoch=config.epoch,
        reserve_packet_hash=reserve_packet_hash,
        expected_observation_root=hc.observation_root,
        expected_snapshot_commitment=hc.commitment,
        expected_net_assets_usd_e6=hc.verified_net_assets_usd_e6,
        registered_attestors=registered_attestors,
        attestations=attestations,
        min_attestations=config.min_attestations,
        now_unix=now_unix,
        max_attestation_age_seconds=config.max_attestation_age_seconds,
    )

    # 4. Evaluate Solana swap readiness gate fail-closed.
    readiness = evaluate_solana_navcoin_readiness(
        config=config,
        hypercore_result=hc,
        quorum_report=quorum,
        solana=solana,
    )
    if not readiness["ready"]:
        raise ValueError(f"swap_readiness_blocked:{','.join(readiness['blockers'])}")

    # 5. Validate and price deterministic dust-swap quote.
    if swap.spread_bps < 0 or swap.spread_bps > config.max_spread_bps:
        raise ValueError(f"swap_spread_exceeds_cap:{swap.spread_bps}>{config.max_spread_bps}")
    unit_scale = 10**config.asset_precision

    if swap.side == "buy_navcoin":
        if swap.usdc_atoms is None or swap.usdc_atoms <= 0:
            raise ValueError("buy_navcoin requires positive usdc_atoms")
        if swap.usdc_atoms < config.min_swap_usdc_atoms:
            raise ValueError(f"swap_below_minimum:{swap.usdc_atoms}<{config.min_swap_usdc_atoms}")
        if swap.usdc_atoms > config.max_dust_usdc_atoms:
            raise ValueError(f"dust_swap_cap_exceeded:{swap.usdc_atoms}>{config.max_dust_usdc_atoms}")
        effective_nav_price = (nav_per_unit * (10_000 + swap.spread_bps) + 9_999) // 10_000
        navcoin_out = (swap.usdc_atoms * unit_scale) // effective_nav_price
        if navcoin_out <= 0:
            raise ValueError("dust_swap_output_zero")
        quote = {
            "side": "buy_navcoin",
            "input_mint": solana.solana_usdc_mint,
            "output_mint": solana.solana_navcoin_mint,
            "usdc_in_atoms": swap.usdc_atoms,
            "navcoin_out_atoms": navcoin_out,
            "nav_per_unit_usd_e6": nav_per_unit,
            "effective_price_usd_e6": effective_nav_price,
            "spread_bps": swap.spread_bps,
        }
    elif swap.side == "sell_navcoin":
        if swap.navcoin_atoms is None or swap.navcoin_atoms <= 0:
            raise ValueError("sell_navcoin requires positive navcoin_atoms")
        gross_usdc = (swap.navcoin_atoms * nav_per_unit) // unit_scale
        if gross_usdc < config.min_swap_usdc_atoms:
            raise ValueError(f"swap_below_minimum:{gross_usdc}<{config.min_swap_usdc_atoms}")
        if gross_usdc > config.max_dust_usdc_atoms:
            raise ValueError(f"dust_swap_cap_exceeded:{gross_usdc}>{config.max_dust_usdc_atoms}")
        usdc_out = (gross_usdc * (10_000 - swap.spread_bps)) // 10_000
        if usdc_out <= 0:
            raise ValueError("dust_swap_output_zero")
        quote = {
            "side": "sell_navcoin",
            "input_mint": solana.solana_navcoin_mint,
            "output_mint": solana.solana_usdc_mint,
            "navcoin_in_atoms": swap.navcoin_atoms,
            "usdc_out_atoms": usdc_out,
            "nav_per_unit_usd_e6": nav_per_unit,
            "spread_bps": swap.spread_bps,
        }
    else:
        raise ValueError(f"unsupported swap side: {swap.side}")

    # 6. Build L1 reserve submission & finalization operations for auditability.
    l1_operations = {
        "nav_reserve_submit": {
            "operation": "nav_reserve_submit",
            "issuer": config.asset_issuer,
            "submitter": config.asset_issuer,
            "asset_id": config.asset_id,
            "epoch": config.epoch,
            "nav_per_unit": nav_per_unit,
            "circulating_supply": config.circulating_supply,
            "verified_net_assets": hc.verified_net_assets_usd_e6,
            "proof_profile": config.proof_profile,
            "source_root": hc.observation_root,
            "attestor_root": quorum.attestor_root,
            "reserve_packet_hash": reserve_packet_hash,
        },
        "nav_reserve_attest": list(quorum.l1_attest_operations),
        "nav_epoch_finalize": {
            "operation": "nav_epoch_finalize",
            "issuer": config.asset_issuer,
            "asset_id": config.asset_id,
            "epoch": config.epoch,
            "reserve_packet_hash": reserve_packet_hash,
        },
    }

    return {
        "schema": REHEARSAL_SCHEMA_V1,
        "chain_id": config.chain_id,
        "asset_id": config.asset_id,
        "asset_code": config.asset_code,
        "epoch": config.epoch,
        "reserve_packet_id": packet_id,
        "reserve_packet_hash": reserve_packet_hash,
        "nav_per_unit_usd_e6": nav_per_unit,
        "hypercore_verification": hc.to_dict(),
        "attestor_quorum": quorum.to_dict(),
        "solana_readiness": readiness,
        "dust_swap_plan": {
            "mode": "rehearsal",
            "solana_trader": swap.solana_trader,
            "pftl_recipient": swap.pftl_recipient,
            "quote": quote,
        },
        "l1_operations": l1_operations,
    }
