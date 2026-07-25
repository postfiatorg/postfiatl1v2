from __future__ import annotations

import hashlib

from ...coordinator.protocol import encode_condition, payment_hash, SecretPreimage
from ...coordinator.signing import Ed25519Signer, sign_quote
from ..authorization import sign_value_authorization
from ..policy import (
    ATOMICITY_CLAIM,
    POLICY_SCHEMA,
    PRICE_SCHEMA,
    RealValuePolicy,
    PriceObservation,
)


QUOTE_SIGNER = Ed25519Signer.from_private_bytes(bytes.fromhex("71" * 32))
AUTH_SIGNER = Ed25519Signer.from_private_bytes(bytes.fromhex("72" * 32))


def policy_mapping(*, mode: str = "DRY_RUN") -> dict[str, object]:
    return {
        "schema": POLICY_SCHEMA,
        "policy_id": hashlib.sha256(b"real-value-test-policy").hexdigest(),
        "mode": mode,
        "lightning_network": "bitcoin",
        "expected_lnd_pubkey": "02" + "11" * 32,
        "quote_signer_public_key_hex": QUOTE_SIGNER.public_key_bytes().hex(),
        "authorization_public_key_hex": AUTH_SIGNER.public_key_bytes().hex(),
        "pftl_chain_id": "pftl-lightning-nav-mainnet-demo",
        "pftl_genesis_hash": "22" * 48,
        "pftl_build_git_revision": "ae3c53c9",
        "pftl_asset_id": "33" * 48,
        "pftl_asset_precision": 6,
        "pftl_rpc_endpoints": [f"tcp://127.0.0.1:{31000 + i}" for i in range(6)],
        "pftl_nav_epoch": 7,
        "pftl_nav_reserve_packet_hash": "44" * 48,
        "pftl_nav_valuation_unit": "USD_PER_WHOLE_ASSET_UNIT",
        "pftl_nav_valuation_scale": 100_000_000,
        "pftl_nav_per_unit_usd_e8": 100_000_000,
        "coordinator_pftl_address": "pf" + "55" * 20,
        "pftl_user_address": "pf" + "77" * 20,
        "trust_class": "CONTROLLED",
        "atomicity_claim": ATOMICITY_CLAIM,
        "require_non_freezable": True,
        "max_per_run_usd_e8": 5 * 100_000_000,
        "max_lifetime_usd_e8": 20 * 100_000_000,
        "max_fee_msat": 10_000,
        "max_price_age_seconds": 60,
        "max_quote_lifetime_seconds": 120,
        "minimum_pftl_validators": 6,
    }


def policy(*, mode: str = "DRY_RUN") -> RealValuePolicy:
    return RealValuePolicy.from_mapping(policy_mapping(mode=mode))


def price(*, observed_at_unix: int = 1_800_000_000) -> PriceObservation:
    return PriceObservation.from_mapping(
        {
            "schema": PRICE_SCHEMA,
            "btc_usd_e8": 100_000 * 100_000_000,
            "observed_at_unix": observed_at_unix,
            "source": "operator-reviewed-test-price",
        }
    )


def quote_mapping(
    *,
    direction: str = "lightning_to_pftl",
    invoice_payee: str = "02" + "11" * 32,
) -> dict[str, object]:
    secret = SecretPreimage(bytes.fromhex("66" * 32))
    digest = payment_hash(secret)
    return {
        "schema": "postfiat.lightning_submarine_quote.v1",
        "swap_id": hashlib.sha256(
            f"mainnet-test-{direction}-{invoice_payee}".encode()
        ).hexdigest(),
        "quote_expires_unix": 1_800_000_060,
        "direction": direction,
        "payment_hash": digest.hex(),
        "lightning_network": "bitcoin",
        "invoice": "lnbc1testfixedinvoice",
        "invoice_payee": invoice_payee,
        "invoice_amount_msat": 100_000,
        "invoice_expiry_unix": 1_800_000_900,
        "min_final_cltv_delta": 144,
        "max_total_cltv_delta": 288,
        "pftl_chain_id": "pftl-lightning-nav-mainnet-demo",
        "pftl_genesis_hash": "22" * 48,
        "pftl_asset_id": "33" * 48,
        "pftl_amount_atoms": 1_000,
        "pftl_owner": "pf" + "55" * 20,
        "pftl_owner_sequence": 8,
        "pftl_recipient": "pf" + "77" * 20,
        "expected_escrow_id": "88" * 48,
        "condition": encode_condition(digest),
        "finish_after": 0,
        "cancel_after": 900,
        "latest_lightning_start_unix": 1_800_000_050,
        "rate_numerator": 1,
        "rate_denominator": 100,
        "coordinator_fee_atoms": 0,
        "nav_epoch": 7,
        "nav_reserve_packet_hash": "44" * 48,
        "custody_class": "NON_CUSTODIAL_HASHLOCK",
        "atomicity_class": "CONDITIONAL_HTLC",
        "timeout_clock_class": "OFFCHAIN_CROSS_LEDGER_POLICY",
        "asset_control_class": "CONTROLLED_ISSUED_ASSET",
    }


def signed_quote(**kwargs: object) -> dict[str, object]:
    return sign_quote(quote_mapping(**kwargs), QUOTE_SIGNER)


def authorization_for(view: object, route: RealValuePolicy) -> dict[str, object]:
    authorization = {
        "schema": "postfiat.lightning_value_authorization.v1",
        "authorization_id": hashlib.sha256(
            f"auth:{view.swap_id}".encode()
        ).hexdigest(),
        "policy_id": route.policy_id,
        "category": "SWAP",
        "quote_sha256": view.quote_sha256,
        "swap_id": view.swap_id,
        "direction": view.direction,
        "principal_msat": view.invoice_amount_msat,
        "max_fee_msat": route.max_fee_msat,
        "max_all_in_usd_e8": view.maximum_all_in_usd_e8,
        "expires_unix": 1_800_000_100,
        "authorized_by": "nazgul",
    }
    return sign_value_authorization(authorization, AUTH_SIGNER)
