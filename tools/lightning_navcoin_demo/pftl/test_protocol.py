from __future__ import annotations

import unittest

from .harness import (
    FinalizedEffect,
    HarnessError,
    _assert_secret_free_public,
    _safe_root,
)
from .protocol import (
    ProtocolEncodingError,
    canonical_vector,
    decode_condition,
    decode_fulfillment,
    fulfillment_satisfies,
)


class ProtocolTests(unittest.TestCase):
    def test_canonical_round_trip_and_shared_payment_hash(self) -> None:
        vector = canonical_vector(bytes(range(32)))
        self.assertEqual(len(vector["payment_hash"]), 64)
        self.assertEqual(len(vector["condition"]), 78)
        self.assertEqual(len(vector["fulfillment"]), 72)
        self.assertEqual(decode_condition(vector["condition"]).hex(), vector["payment_hash"])
        self.assertEqual(decode_fulfillment(vector["fulfillment"]), bytes(range(32)))
        self.assertTrue(
            fulfillment_satisfies(vector["condition"], vector["fulfillment"])
        )

    def test_wrong_preimage_is_false(self) -> None:
        condition = canonical_vector(bytes(range(32)))["condition"]
        fulfillment = canonical_vector(bytes([0xFF]) * 32)["fulfillment"]
        self.assertFalse(fulfillment_satisfies(condition, fulfillment))

    def test_noncanonical_and_malformed_encodings_reject(self) -> None:
        vector = canonical_vector(bytes(range(32)))
        for condition in (
            vector["condition"].upper(),
            vector["condition"][:-2],
            "a1258020" + vector["payment_hash"] + "810120",
        ):
            with self.assertRaises(ProtocolEncodingError):
                decode_condition(condition)
        for fulfillment in (
            vector["fulfillment"].upper(),
            vector["fulfillment"][:-2],
            "a0238020" + vector["preimage_hex"],
        ):
            with self.assertRaises(ProtocolEncodingError):
                decode_fulfillment(fulfillment)

    def test_secret_free_effect_shape(self) -> None:
        effect = FinalizedEffect(
            accepted=True,
            reason="accepted",
            tx_id="tx",
            finalized_height=1,
            state_root="root",
            block_tip_hash="tip",
            agreeing_validator_count=6,
            validator_count=6,
            receipt_count=6,
            certificate_id="certificate",
            effect_key="swap:pftl-finish",
            escrow_id="escrow",
        ).to_dict()
        encoded = repr(effect).lower()
        self.assertNotIn("preimage", encoded)
        self.assertNotIn("fulfillment", encoded)

    def test_public_finality_shape_rejects_secret_field_names(self) -> None:
        _assert_secret_free_public(
            {
                "certificate": {"votes": [{"signature_hex": "00"}]},
                "validator_registry": [{"public_key_hex": "11"}],
            }
        )
        for field in ("preimage", "fulfillment", "private_key_hex", "macaroon"):
            with self.assertRaises(HarnessError):
                _assert_secret_free_public({field: "not-public"})

    def test_broad_roots_are_refused(self) -> None:
        for path in ("/", str(__import__("pathlib").Path.home())):
            with self.assertRaises(HarnessError):
                _safe_root(path)


if __name__ == "__main__":
    unittest.main()
