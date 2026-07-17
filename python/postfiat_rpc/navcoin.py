"""NAVCOIN calculation and native operation builders.

The functions here are deterministic helpers for building reserve packets and
native NAV operation JSON. Submission still goes through
``postfiat_rpc.submit_asset_transaction`` so signing remains in the Rust SDK.
"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from typing import Any


VALUATION_UNIT = "usd_1e6"
DEFAULT_PROOF_PROFILE = "local-nitro-placeholder-v0"

NAV_PROFILE_VERIFIER_LEDGER_TRANSPARENT = "ledger-transparent"
NAV_PROFILE_VERIFIER_PLACEHOLDER = "placeholder"


@dataclass(frozen=True)
class NavInputs:
    issuer: str
    ap_account: str
    asset_code: str
    epoch: int
    circulating_supply: int
    mint_amount: int
    redeem_amount: int
    cash_micro_usd: int
    broker_positions_micro_usd: int
    liabilities_micro_usd: int
    pending_redemptions_micro_usd: int
    proof_profile: str = DEFAULT_PROOF_PROFILE
    reserve_accounts: tuple[str, ...] = ()


def sha384_hex(data: bytes) -> str:
    return hashlib.sha384(data).hexdigest()


def canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")


def derived_asset_id(issuer: str, code: str, version: int = 1) -> str:
    return sha384_hex(f"postfiat.example.issued_asset:{issuer}:{code}:{version}".encode())


def nav_proof_profile_id(
    verifier_kind: str,
    source_class: str = "ledger",
    max_snapshot_age_blocks: int = 0,
    challenge_window_blocks: int = 0,
    max_epoch_gap_blocks: int = 0,
    settle_deadline_blocks: int = 0,
    min_challenge_bond: int = 0,
    min_attestations: int = 0,
    tolerance_bp: int = 0,
    valuation_policy_hash: str = "",
    sp1_program_vkey: str = "",
    sp1_proof_encoding: str = "",
    max_proof_bytes: int = 0,
    max_public_values_bytes: int = 0,
) -> str:
    """Mirror of the Rust content-addressed profile id (SHA3-384 over the
    domain-tagged canonical preimage)."""
    preimage = (
        f"verifier_kind={verifier_kind}\n"
        f"source_class={source_class}\n"
        f"max_snapshot_age_blocks={max_snapshot_age_blocks}\n"
        f"challenge_window_blocks={challenge_window_blocks}\n"
        f"max_epoch_gap_blocks={max_epoch_gap_blocks}\n"
        f"settle_deadline_blocks={settle_deadline_blocks}\n"
        f"min_challenge_bond={min_challenge_bond}\n"
        f"min_attestations={min_attestations}\n"
        f"tolerance_bp={tolerance_bp}\n"
        f"valuation_policy_hash={valuation_policy_hash}\n"
        f"sp1_program_vkey={sp1_program_vkey}\n"
        f"sp1_proof_encoding={sp1_proof_encoding}\n"
        f"max_proof_bytes={max_proof_bytes}\n"
        f"max_public_values_bytes={max_public_values_bytes}\n"
    )
    hasher = hashlib.sha3_384()
    hasher.update(b"postfiat.nav_proof_profile_id.v1")
    hasher.update(b"\x00")
    hasher.update(preimage.encode())
    return hasher.hexdigest()


def build_profile_register_operation(
    registrant: str,
    verifier_kind: str,
    source_class: str = "ledger",
    max_snapshot_age_blocks: int = 0,
    challenge_window_blocks: int = 0,
    max_epoch_gap_blocks: int = 0,
    settle_deadline_blocks: int = 0,
    min_challenge_bond: int = 0,
    min_attestations: int = 0,
    tolerance_bp: int = 0,
    valuation_policy_hash: str = "",
    sp1_program_vkey: str = "",
    sp1_proof_encoding: str = "",
    max_proof_bytes: int = 0,
    max_public_values_bytes: int = 0,
) -> dict[str, Any]:
    return {
        "operation": "nav_profile_register",
        "registrant": registrant,
        "verifier_kind": verifier_kind,
        "source_class": source_class,
        "max_snapshot_age_blocks": max_snapshot_age_blocks,
        "challenge_window_blocks": challenge_window_blocks,
        "max_epoch_gap_blocks": max_epoch_gap_blocks,
        "settle_deadline_blocks": settle_deadline_blocks,
        "min_challenge_bond": min_challenge_bond,
        "min_attestations": min_attestations,
        "tolerance_bp": tolerance_bp,
        "valuation_policy_hash": valuation_policy_hash,
        "sp1_program_vkey": sp1_program_vkey,
        "sp1_proof_encoding": sp1_proof_encoding,
        "max_proof_bytes": max_proof_bytes,
        "max_public_values_bytes": max_public_values_bytes,
    }


def build_attestor_register_operation(
    attestor: str,
    domain: str,
    bond: int = 0,
) -> dict[str, Any]:
    return {
        "operation": "nav_attestor_register",
        "attestor": attestor,
        "domain": domain,
        "bond": bond,
    }


def build_reserve_attest_operation(
    attestor: str,
    asset_id: str,
    epoch: int,
    reserve_packet_hash: str,
    passed: bool,
    observation_root: str,
) -> dict[str, Any]:
    return {
        "operation": "nav_reserve_attest",
        "attestor": attestor,
        "asset_id": asset_id,
        "epoch": epoch,
        "reserve_packet_hash": reserve_packet_hash,
        "pass": passed,
        "observation_root": observation_root,
    }


def build_redeem_settle_operation(
    issuer: str,
    asset_id: str,
    redemption_id: str,
    settlement_receipt_hash: str,
) -> dict[str, Any]:
    return {
        "operation": "nav_redeem_settle",
        "issuer": issuer,
        "asset_id": asset_id,
        "redemption_id": redemption_id,
        "settlement_receipt_hash": settlement_receipt_hash,
    }


def nav_reserve_packet_id(asset_id: str, epoch: int, reserve_packet_hash: str) -> str:
    return sha384_hex(
        f"postfiat.nav_reserve_packet_id.v1:{asset_id}:{epoch}:{reserve_packet_hash}".encode()
    )


def calculate_nav(inputs: NavInputs) -> dict[str, int]:
    gross_assets = inputs.cash_micro_usd + inputs.broker_positions_micro_usd
    gross_liabilities = inputs.liabilities_micro_usd + inputs.pending_redemptions_micro_usd
    verified_net_assets = gross_assets - gross_liabilities
    if verified_net_assets <= 0:
        raise ValueError("verified net assets must be positive")
    if inputs.circulating_supply <= 0:
        raise ValueError("circulating supply must be positive")
    nav_per_unit = verified_net_assets // inputs.circulating_supply
    over_collateralization_remainder = verified_net_assets - (
        inputs.circulating_supply * nav_per_unit
    )
    return {
        "gross_assets_micro_usd": gross_assets,
        "gross_liabilities_micro_usd": gross_liabilities,
        "verified_net_assets_micro_usd": verified_net_assets,
        "nav_per_unit_micro_usd": nav_per_unit,
        "over_collateralization_remainder_micro_usd": over_collateralization_remainder,
    }


def build_packet_and_operations(inputs: NavInputs) -> dict[str, Any]:
    if inputs.mint_amount <= 0:
        raise ValueError("mint amount must be positive")
    if inputs.redeem_amount <= 0:
        raise ValueError("redeem amount must be positive")
    if inputs.mint_amount > inputs.circulating_supply:
        raise ValueError("mint amount cannot exceed finalized circulating supply cap")
    if inputs.redeem_amount > inputs.mint_amount:
        raise ValueError("redeem amount cannot exceed minted amount in this example")

    nav = calculate_nav(inputs)
    asset_id = derived_asset_id(inputs.issuer, inputs.asset_code)
    reserve_packet = {
        "schema": "postfiat-navcoin-reserve-packet-example-v1",
        "asset_code": inputs.asset_code,
        "asset_id": asset_id,
        "epoch": inputs.epoch,
        "valuation_unit": VALUATION_UNIT,
        "proof_profile": inputs.proof_profile,
        "sources": {
            "cash_micro_usd": inputs.cash_micro_usd,
            "broker_positions_micro_usd": inputs.broker_positions_micro_usd,
            "liabilities_micro_usd": inputs.liabilities_micro_usd,
            "pending_redemptions_micro_usd": inputs.pending_redemptions_micro_usd,
        },
        "circulating_supply": inputs.circulating_supply,
        "nav_per_unit": nav["nav_per_unit_micro_usd"],
        "verified_net_assets": nav["verified_net_assets_micro_usd"],
        "invariant": {
            "verified_net_assets_gte_supply_times_nav": (
                nav["verified_net_assets_micro_usd"]
                >= inputs.circulating_supply * nav["nav_per_unit_micro_usd"]
            ),
            "over_collateralization_remainder": nav[
                "over_collateralization_remainder_micro_usd"
            ],
        },
    }
    reserve_packet_hash = sha384_hex(canonical_json_bytes(reserve_packet))
    source_root = sha384_hex(canonical_json_bytes(reserve_packet["sources"]))
    attestor_root = sha384_hex(
        canonical_json_bytes(
            {
                "attestor_group": "example-controlled-attestor",
                "packet_hash": reserve_packet_hash,
                "proof_profile": inputs.proof_profile,
            }
        )
    )

    operations = {
        "asset_create": {
            "operation": "asset_create",
            "issuer": inputs.issuer,
            "code": inputs.asset_code,
            "version": 1,
            "precision": 6,
            "display_name": "NAVCOIN Example",
            "max_supply": inputs.circulating_supply,
            "requires_authorization": False,
            "freeze_enabled": True,
            "clawback_enabled": False,
        },
        "nav_asset_register": {
            "operation": "nav_asset_register",
            "issuer": inputs.issuer,
            "asset_id": asset_id,
            "reserve_operator": inputs.issuer,
            "proof_profile": inputs.proof_profile,
            "valuation_unit": VALUATION_UNIT,
            "redemption_account": inputs.issuer,
        },
        "ap_trustline": {
            "operation": "trust_set",
            "account": inputs.ap_account,
            "issuer": inputs.issuer,
            "asset_id": asset_id,
            "limit": inputs.circulating_supply,
            "authorized": False,
            "frozen": False,
            "reserve_paid": 10,
        },
        "nav_reserve_submit": {
            "operation": "nav_reserve_submit",
            "issuer": inputs.issuer,
            "submitter": inputs.issuer,
            "asset_id": asset_id,
            "epoch": inputs.epoch,
            "nav_per_unit": nav["nav_per_unit_micro_usd"],
            "circulating_supply": inputs.circulating_supply,
            "verified_net_assets": nav["verified_net_assets_micro_usd"],
            "proof_profile": inputs.proof_profile,
            "source_root": source_root,
            "attestor_root": attestor_root,
            "reserve_packet_hash": reserve_packet_hash,
            "reserve_accounts": list(inputs.reserve_accounts),
        },
        "nav_epoch_finalize": {
            "operation": "nav_epoch_finalize",
            "issuer": inputs.issuer,
            "asset_id": asset_id,
            "epoch": inputs.epoch,
            "reserve_packet_hash": reserve_packet_hash,
        },
        "nav_mint_at_nav": {
            "operation": "nav_mint_at_nav",
            "issuer": inputs.issuer,
            "to": inputs.ap_account,
            "asset_id": asset_id,
            "amount": inputs.mint_amount,
            "epoch": inputs.epoch,
            "reserve_packet_hash": reserve_packet_hash,
        },
        "nav_redeem_at_nav": {
            "operation": "nav_redeem_at_nav",
            "owner": inputs.ap_account,
            "issuer": inputs.issuer,
            "asset_id": asset_id,
            "amount": inputs.redeem_amount,
            "epoch": inputs.epoch,
            "reserve_packet_hash": reserve_packet_hash,
        },
    }

    return {
        "calculation": nav,
        "asset_id": asset_id,
        "reserve_packet_hash": reserve_packet_hash,
        "reserve_packet_id": nav_reserve_packet_id(asset_id, inputs.epoch, reserve_packet_hash),
        "source_root": source_root,
        "attestor_root": attestor_root,
        "reserve_packet": reserve_packet,
        "operations": operations,
        "post_redeem_supply": inputs.mint_amount - inputs.redeem_amount,
        "redemption_claim_micro_usd": inputs.redeem_amount * nav["nav_per_unit_micro_usd"],
    }
