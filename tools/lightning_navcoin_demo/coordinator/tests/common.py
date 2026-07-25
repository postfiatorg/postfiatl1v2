from __future__ import annotations

import hashlib

from tools.lightning_navcoin_demo.coordinator.protocol import (
    SecretPreimage,
    encode_condition,
    payment_hash,
)
from tools.lightning_navcoin_demo.coordinator.signing import (
    Ed25519Signer,
    sign_quote,
)


TEST_SIGNING_SEED = bytes.fromhex("1f" * 32)


def secret_for(index: int = 0) -> SecretPreimage:
    return SecretPreimage(hashlib.sha256(f"test-secret-{index}".encode()).digest())


def quote_for(
    index: int = 0,
    *,
    amount_atoms: int = 200_000,
    direction: str = "lightning_to_pftl",
    now_unix: int | None = None,
) -> dict[str, object]:
    secret = secret_for(index)
    digest = payment_hash(secret)
    quote_expires_unix = 1_700_000_300 if now_unix is None else now_unix + 300
    latest_lightning_start_unix = (
        1_700_000_600 if now_unix is None else now_unix + 600
    )
    invoice_expiry_unix = 1_700_000_900 if now_unix is None else now_unix + 900
    return {
        "schema": "postfiat.lightning_submarine_quote.v1",
        "swap_id": hashlib.sha256(f"swap-{index}".encode()).hexdigest(),
        "quote_expires_unix": quote_expires_unix,
        "direction": direction,
        "payment_hash": digest.hex(),
        "lightning_network": "regtest",
        "invoice": f"lnbcrt-fixed-test-invoice-{index}",
        "invoice_payee": "02" + "11" * 32,
        "invoice_amount_msat": amount_atoms * 10,
        "invoice_expiry_unix": invoice_expiry_unix,
        "min_final_cltv_delta": 144,
        "max_total_cltv_delta": 288,
        "pftl_chain_id": "pftl-local-six",
        "pftl_genesis_hash": "22" * 48,
        "pftl_asset_id": "33" * 48,
        "pftl_amount_atoms": amount_atoms,
        "pftl_owner": "pf" + "44" * 20,
        "pftl_owner_sequence": index + 1,
        "pftl_recipient": "pf" + "55" * 20,
        "expected_escrow_id": hashlib.sha256(
            f"escrow-{index}".encode()
        ).hexdigest()
        + hashlib.sha256(f"escrow-suffix-{index}".encode()).hexdigest()[:32],
        "condition": encode_condition(digest),
        "finish_after": 0,
        "cancel_after": 500,
        "latest_lightning_start_unix": latest_lightning_start_unix,
        "rate_numerator": 100,
        "rate_denominator": 1,
        "coordinator_fee_atoms": 10,
        "nav_epoch": 0,
        "nav_reserve_packet_hash": "",
        "custody_class": "NON_CUSTODIAL_HASHLOCK",
        "atomicity_class": "CONDITIONAL_HTLC",
        "timeout_clock_class": "OFFCHAIN_CROSS_LEDGER_POLICY",
        "asset_control_class": "NON_FREEZABLE_TEST",
    }


def envelope_for(
    index: int = 0,
    *,
    amount_atoms: int = 200_000,
    direction: str = "lightning_to_pftl",
    now_unix: int | None = None,
) -> dict[str, object]:
    return sign_quote(
        quote_for(
            index,
            amount_atoms=amount_atoms,
            direction=direction,
            now_unix=now_unix,
        ),
        Ed25519Signer.from_private_bytes(TEST_SIGNING_SEED),
    )
