from __future__ import annotations

import hashlib
import unittest

from tools.xrpl_navcoin_demo.protocol import (
    CrossLedgerHashlock,
    ProtocolEncodingError,
    SecretPreimage,
    extract_xrpl_finish_preimage,
    verify_pair,
)


class CrossLedgerProtocolTests(unittest.TestCase):
    def setUp(self) -> None:
        self.hashlock = CrossLedgerHashlock(
            SecretPreimage(bytes(range(32)))
        )

    def test_same_condition_bytes_with_ledger_specific_case(self) -> None:
        public = self.hashlock.public_values()
        self.assertEqual(
            public["payment_hash"],
            hashlib.sha256(bytes(range(32))).hexdigest(),
        )
        self.assertEqual(public["pftl_condition"].upper(), public["xrpl_condition"])
        self.assertEqual(
            self.hashlock.pftl_fulfillment().upper(),
            self.hashlock.xrpl_fulfillment(),
        )
        self.assertTrue(
            verify_pair(
                public["pftl_condition"],
                self.hashlock.pftl_fulfillment(),
                xrpl=False,
            )
        )
        self.assertTrue(
            verify_pair(
                public["xrpl_condition"],
                self.hashlock.xrpl_fulfillment(),
                xrpl=True,
            )
        )

    def test_wrong_preimage_is_false_on_both_ledgers(self) -> None:
        wrong = CrossLedgerHashlock(SecretPreimage(bytes([0xFF]) * 32))
        public = self.hashlock.public_values()
        self.assertFalse(
            verify_pair(
                public["pftl_condition"],
                wrong.pftl_fulfillment(),
                xrpl=False,
            )
        )
        self.assertFalse(
            verify_pair(
                public["xrpl_condition"],
                wrong.xrpl_fulfillment(),
                xrpl=True,
            )
        )

    def test_casing_is_not_silently_normalized(self) -> None:
        public = self.hashlock.public_values()
        with self.assertRaises(ProtocolEncodingError):
            verify_pair(
                public["xrpl_condition"].lower(),
                self.hashlock.xrpl_fulfillment(),
                xrpl=True,
            )
        with self.assertRaises(ProtocolEncodingError):
            verify_pair(
                public["pftl_condition"].upper(),
                self.hashlock.pftl_fulfillment(),
                xrpl=False,
            )

    def test_public_xrpl_finish_reveals_authenticated_preimage(self) -> None:
        public = self.hashlock.public_values()
        extracted = extract_xrpl_finish_preimage(
            {
                "TransactionType": "EscrowFinish",
                "Condition": public["xrpl_condition"],
                "Fulfillment": self.hashlock.xrpl_fulfillment(),
            },
            expected_condition=public["xrpl_condition"],
        )
        self.assertEqual(extracted.reveal_for_protocol(), bytes(range(32)))

    def test_secret_representations_are_redacted(self) -> None:
        secret_hex = self.hashlock.secret.protocol_hex()
        self.assertNotIn(secret_hex, repr(self.hashlock.secret))
        self.assertNotIn(secret_hex, repr(self.hashlock))


if __name__ == "__main__":
    unittest.main()
