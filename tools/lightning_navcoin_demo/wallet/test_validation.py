from __future__ import annotations

import copy
import hashlib
import unittest

from tools.lightning_navcoin_demo.wallet.validation import (
    TimelockPolicy,
    ValidationError,
    decode_preimage_sha256_condition,
    decode_preimage_sha256_fulfillment,
    escrow_condition_hash,
    validate_invoice_against_quote,
    validate_pftl_lock_views,
    verify_fulfillment,
)


PREIMAGE = bytes(range(32))
PAYMENT_HASH = hashlib.sha256(PREIMAGE).hexdigest()
CONDITION = f"a0258020{PAYMENT_HASH}810120"
FULFILLMENT = f"a0228020{PREIMAGE.hex()}"
OWNER = "pf" + "11" * 20
RECIPIENT = "pf" + "22" * 20
GENESIS = "33" * 48
TIP = "44" * 48
ROOT = "55" * 48


def quote() -> dict:
    return {
        "schema": "postfiat.lightning_submarine_quote.v1",
        "swap_id": "66" * 32,
        "quote_expires_unix": 1_700_000_600,
        "direction": "lightning_to_pftl",
        "payment_hash": PAYMENT_HASH,
        "lightning_network": "regtest",
        "invoice": "lnbcrt1synthetic",
        "invoice_payee": "02" + "77" * 32,
        "invoice_amount_msat": 20_000_000,
        "invoice_expiry_unix": 1_700_000_900,
        "min_final_cltv_delta": 144,
        "max_total_cltv_delta": 288,
        "pftl_chain_id": "postfiat-lightning-navcoin-demo",
        "pftl_genesis_hash": GENESIS,
        "pftl_asset_id": "88" * 48,
        "pftl_amount_atoms": 2_000_000,
        "pftl_owner": OWNER,
        "pftl_owner_sequence": 7,
        "pftl_recipient": RECIPIENT,
        "expected_escrow_id": "99" * 48,
        "condition": CONDITION,
        "finish_after": 0,
        "cancel_after": 500,
        "latest_lightning_start_unix": 1_700_000_300,
        "rate_numerator": 1,
        "rate_denominator": 10,
        "coordinator_fee_atoms": 10,
        "nav_epoch": 0,
        "nav_reserve_packet_hash": "",
        "custody_class": "NON_CUSTODIAL_HASHLOCK",
        "atomicity_class": "CONDITIONAL_HTLC",
        "timeout_clock_class": "OFFCHAIN_CROSS_LEDGER_POLICY",
        "asset_control_class": "NON_FREEZABLE_TEST",
    }


def decoded_invoice() -> dict:
    return {
        "destination": "02" + "77" * 32,
        "payment_hash": PAYMENT_HASH,
        "num_satoshis": "20000",
        "timestamp": "1700000000",
        "expiry": "900",
        "cltv_expiry": "144",
        "features": {
            "9": {"name": "tlv-onion", "is_required": False},
            "15": {"name": "payment-addr", "is_required": False},
            "17": {"name": "multi-path-payments", "is_required": False},
        },
    }


def validator_view(index: int = 0) -> dict:
    q = quote()
    return {
        "node_id": f"validator-{index}",
        "status": {
            "chain_id": q["pftl_chain_id"],
            "genesis_hash": GENESIS,
            "validator_count": 6,
            "block_height": 100,
            "block_tip_hash": TIP,
            "state_root": ROOT,
        },
        "escrow": {
            "found": True,
            "escrow": {
                "escrow_id": q["expected_escrow_id"],
                "owner": OWNER,
                "recipient": RECIPIENT,
                "asset_id": q["pftl_asset_id"],
                "amount": q["pftl_amount_atoms"],
                "condition_hash": escrow_condition_hash(CONDITION),
                "finish_after": 0,
                "cancel_after": 500,
                "state": "open",
            },
        },
        "asset": {
            "asset": {
                "asset_id": q["pftl_asset_id"],
                "requires_authorization": False,
                "freeze_enabled": False,
                "clawback_enabled": False,
            },
        },
    }


class ConditionTests(unittest.TestCase):
    def test_canonical_vectors(self) -> None:
        self.assertEqual(decode_preimage_sha256_condition(CONDITION), PAYMENT_HASH)
        self.assertEqual(decode_preimage_sha256_fulfillment(FULFILLMENT), PREIMAGE)
        self.assertTrue(verify_fulfillment(CONDITION, FULFILLMENT))

    def test_noncanonical_and_wrong_preimage_reject(self) -> None:
        for malformed in (
            CONDITION.upper(),
            "a1258020" + PAYMENT_HASH + "810120",
            "a0258020" + PAYMENT_HASH + "810121",
            CONDITION[:-2],
        ):
            with self.subTest(malformed=malformed):
                with self.assertRaises(ValidationError):
                    decode_preimage_sha256_condition(malformed)
        with self.assertRaises(ValidationError):
            decode_preimage_sha256_fulfillment(FULFILLMENT.upper())
        wrong = "a0228020" + ("ff" * 32)
        self.assertFalse(verify_fulfillment(CONDITION, wrong))


class InvoiceTests(unittest.TestCase):
    def test_invoice_and_quote_validate(self) -> None:
        result = validate_invoice_against_quote(
            quote(),
            decoded_invoice(),
            now_unix=1_700_000_100,
            verify_quote_signature=lambda _: True,
        )
        self.assertEqual(result.payment_hash, PAYMENT_HASH)
        self.assertEqual(result.amount_msat, 20_000_000)

    def test_amp_and_mismatch_reject(self) -> None:
        amp = decoded_invoice()
        amp["features"]["30"] = {"name": "amp", "is_required": False}
        with self.assertRaisesRegex(ValidationError, "AMP"):
            validate_invoice_against_quote(
                quote(), amp, now_unix=1_700_000_100, verify_quote_signature=lambda _: True
            )
        mismatch = decoded_invoice()
        mismatch["payment_hash"] = "00" * 32
        with self.assertRaisesRegex(ValidationError, "payment hash"):
            validate_invoice_against_quote(
                quote(),
                mismatch,
                now_unix=1_700_000_100,
                verify_quote_signature=lambda _: True,
            )

    def test_signature_expiry_and_network_reject(self) -> None:
        with self.assertRaisesRegex(ValidationError, "signature"):
            validate_invoice_against_quote(
                quote(),
                decoded_invoice(),
                now_unix=1_700_000_100,
                verify_quote_signature=lambda _: False,
            )
        mainnet = quote()
        mainnet["lightning_network"] = "bitcoin"
        with self.assertRaisesRegex(ValidationError, "regtest"):
            validate_invoice_against_quote(
                mainnet,
                decoded_invoice(),
                now_unix=1_700_000_100,
                verify_quote_signature=lambda _: True,
            )
        with self.assertRaisesRegex(ValidationError, "quote is expired"):
            validate_invoice_against_quote(
                quote(),
                decoded_invoice(),
                now_unix=1_700_000_700,
                verify_quote_signature=lambda _: True,
            )


class PftlViewTests(unittest.TestCase):
    def test_six_and_five_validator_views_validate(self) -> None:
        policy = TimelockPolicy()
        six = [validator_view(index) for index in range(6)]
        result = validate_pftl_lock_views(quote(), six, policy=policy)
        self.assertEqual(result.available_validators, 6)
        result = validate_pftl_lock_views(quote(), six[:5], policy=policy)
        self.assertEqual(result.available_validators, 5)

    def test_four_divergence_freeze_and_short_window_reject(self) -> None:
        policy = TimelockPolicy()
        views = [validator_view(index) for index in range(6)]
        with self.assertRaisesRegex(ValidationError, "fewer"):
            validate_pftl_lock_views(quote(), views[:4], policy=policy)

        duplicated = copy.deepcopy(views)
        duplicated[-1]["node_id"] = duplicated[0]["node_id"]
        with self.assertRaisesRegex(ValidationError, "distinct"):
            validate_pftl_lock_views(quote(), duplicated, policy=policy)

        divergent = copy.deepcopy(views)
        divergent[-1]["status"]["state_root"] = "aa" * 48
        with self.assertRaisesRegex(ValidationError, "not converged"):
            validate_pftl_lock_views(quote(), divergent, policy=policy)

        freezable = copy.deepcopy(views)
        for view in freezable:
            view["asset"]["asset"]["freeze_enabled"] = True
        with self.assertRaisesRegex(ValidationError, "freeze_enabled"):
            validate_pftl_lock_views(quote(), freezable, policy=policy)

        short = quote()
        short["cancel_after"] = 200
        for view in views:
            view["escrow"]["escrow"]["cancel_after"] = 200
        with self.assertRaisesRegex(ValidationError, "does not outlast"):
            validate_pftl_lock_views(short, views, policy=policy)


if __name__ == "__main__":
    unittest.main()
