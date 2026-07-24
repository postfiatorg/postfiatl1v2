from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path
import unittest

from tools.lightning_navcoin_demo.coordinator.protocol import (
    AmpInvoiceRejected,
    InvoiceBindingError,
    LndInvoiceFacts,
    ProtocolEncodingError,
    SecretPreimage,
    decode_condition,
    decode_fulfillment,
    encode_condition,
    encode_fulfillment,
    payment_hash,
    validate_invoice_binding,
    verify_fulfillment,
)


VECTOR_PATH = Path(__file__).parents[1] / "test_vectors.json"


class ProtocolVectorTests(unittest.TestCase):
    def setUp(self) -> None:
        self.vector = json.loads(VECTOR_PATH.read_text(encoding="utf-8"))["vectors"][0]
        self.secret = SecretPreimage.from_hex(self.vector["secret_hex"])

    def test_pinned_preimage_sha256_vector(self) -> None:
        self.assertEqual(payment_hash(self.secret).hex(), self.vector["payment_hash"])
        self.assertEqual(
            encode_condition(payment_hash(self.secret)), self.vector["condition"]
        )
        self.assertEqual(encode_fulfillment(self.secret), self.vector["fulfillment"])
        self.assertEqual(
            decode_condition(self.vector["condition"]), payment_hash(self.secret)
        )
        self.assertEqual(
            decode_fulfillment(self.vector["fulfillment"]).reveal_for_protocol(),
            self.secret.reveal_for_protocol(),
        )
        self.assertTrue(
            verify_fulfillment(
                self.vector["condition"], self.vector["fulfillment"]
            )
        )

    def test_wrong_preimage_is_false(self) -> None:
        wrong = SecretPreimage(hashlib.sha256(b"wrong").digest())
        self.assertFalse(
            verify_fulfillment(self.vector["condition"], encode_fulfillment(wrong))
        )

    def test_malformed_and_noncanonical_encodings_reject(self) -> None:
        malformed_conditions = [
            self.vector["condition"].upper(),
            self.vector["condition"][:-2],
            "b0" + self.vector["condition"][2:],
            self.vector["condition"][:-2] + "ff",
            self.vector["condition"][:-1] + "g",
        ]
        for encoded in malformed_conditions:
            with self.subTest(encoded=encoded):
                with self.assertRaises(ProtocolEncodingError):
                    decode_condition(encoded)
        malformed_fulfillments = [
            self.vector["fulfillment"].upper(),
            self.vector["fulfillment"][:-2],
            "b0" + self.vector["fulfillment"][2:],
            self.vector["fulfillment"][:-1] + "g",
        ]
        for encoded in malformed_fulfillments:
            with self.subTest(encoded=encoded):
                with self.assertRaises(ProtocolEncodingError):
                    decode_fulfillment(encoded)

    def test_secret_repr_is_redacted(self) -> None:
        self.assertNotIn(self.vector["secret_hex"], repr(self.secret))
        self.assertEqual(str(self.secret), "<redacted>")


class InvoiceBindingTests(unittest.TestCase):
    def setUp(self) -> None:
        self.secret = SecretPreimage(bytes(range(32)))
        self.response = {
            "payment_hash": payment_hash(self.secret).hex(),
            "num_msat": "2000000",
            "destination": "02" + "11" * 32,
            "timestamp": "1700000000",
            "expiry": "900",
            "cltv_expiry": "144",
            "features": {"9": {"name": "tlv-onion"}},
            "is_amp": False,
        }

    def facts(self, response: dict[str, object] | None = None) -> LndInvoiceFacts:
        return LndInvoiceFacts.from_decode_pay_req(
            self.response if response is None else response,
            network="regtest",
        )

    def assert_binding(self, facts: LndInvoiceFacts) -> None:
        validate_invoice_binding(
            facts,
            expected_payment_hash=payment_hash(self.secret),
            expected_amount_msat=2_000_000,
            expected_payee="02" + "11" * 32,
            expected_expiry_unix=1_700_000_900,
            expected_min_final_cltv_delta=144,
            expected_network="regtest",
        )

    def test_cross_checks_lnd_payment_hash_and_fields(self) -> None:
        self.assert_binding(self.facts())

    def test_amp_rejected_by_explicit_flag_legacy_flag_or_feature(self) -> None:
        variants = []
        for field, value in (
            ("is_amp", True),
            ("amp", True),
            ("features", {"30": {"name": "amp"}}),
        ):
            response = copy.deepcopy(self.response)
            response[field] = value
            variants.append(response)
        for response in variants:
            with self.subTest(response=response):
                with self.assertRaises(AmpInvoiceRejected):
                    self.assert_binding(self.facts(response))

    def test_each_invoice_binding_mismatch_rejects(self) -> None:
        facts = self.facts()
        base = {
            "expected_payment_hash": payment_hash(self.secret),
            "expected_amount_msat": 2_000_000,
            "expected_payee": "02" + "11" * 32,
            "expected_expiry_unix": 1_700_000_900,
            "expected_min_final_cltv_delta": 144,
            "expected_network": "regtest",
        }
        mutations = {
            "expected_payment_hash": bytes(32),
            "expected_amount_msat": 2_000_001,
            "expected_payee": "03" + "11" * 32,
            "expected_expiry_unix": 1_700_000_901,
            "expected_min_final_cltv_delta": 145,
            "expected_network": "signet",
        }
        for field, value in mutations.items():
            arguments = dict(base)
            arguments[field] = value
            with self.subTest(field=field):
                with self.assertRaises(InvoiceBindingError):
                    validate_invoice_binding(facts, **arguments)

    def test_noncanonical_payment_hash_rejects(self) -> None:
        response = dict(self.response)
        response["payment_hash"] = str(response["payment_hash"]).upper()
        with self.assertRaises(ProtocolEncodingError):
            self.facts(response)
