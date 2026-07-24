from __future__ import annotations

import copy
import hashlib
import json
import unittest

from tools.lightning_navcoin_demo.coordinator.quote import (
    QuoteValidationError,
    canonical_quote_bytes,
    parse_canonical_quote,
    validate_quote,
)
from tools.lightning_navcoin_demo.coordinator.signing import (
    Ed25519Signer,
    QuoteSignatureError,
    encode_signed_quote,
    parse_signed_quote,
    sign_quote,
    verify_signed_quote,
)
from tools.lightning_navcoin_demo.coordinator.tests.common import (
    TEST_SIGNING_SEED,
    envelope_for,
    quote_for,
)


PINNED_CANONICAL_QUOTE_SHA256 = (
    "9c47a00d0995133cf640e36d20e6696ce15cdef0974a0e3eff7bf09369801805"
)
PINNED_PUBLIC_KEY_B64URL = "QwRr_kCSs-lJlOraFdzCDYqqB7ZY_TlU644O-4vcpd4"
PINNED_KEY_ID = "51863ba52db419f6b772d580d18173576f381334426e7680cb3b78eeee106dc0"
PINNED_SIGNATURE_B64URL = (
    "1-VC8Z4j58rP_M0UFhzANlx6VAYpRg-MAt_P1ol2ybaO-7pTmtX48xQeJao8eOduQ"
    "7yTcvwgz_-tsGnboKSrCw"
)
PINNED_ENVELOPE_SHA256 = (
    "5e1b295306d7f25533a1db17c8fa1270f81c36fcae4a4e490d6b2cb37081e173"
)


class CanonicalQuoteTests(unittest.TestCase):
    def test_canonical_quote_hash_is_pinned(self) -> None:
        encoded = canonical_quote_bytes(quote_for())
        self.assertEqual(
            hashlib.sha256(encoded).hexdigest(), PINNED_CANONICAL_QUOTE_SHA256
        )
        self.assertEqual(parse_canonical_quote(encoded), validate_quote(quote_for()))

    def test_pretty_or_duplicate_json_is_not_canonical(self) -> None:
        quote = quote_for()
        pretty = json.dumps(quote, sort_keys=True, indent=2).encode("ascii")
        with self.assertRaises(QuoteValidationError):
            parse_canonical_quote(pretty)
        canonical = canonical_quote_bytes(quote).decode("ascii")
        duplicate = canonical.replace(
            '{"asset_control_class":',
            '{"schema":"postfiat.lightning_submarine_quote.v1",'
            '"asset_control_class":',
            1,
        ).encode("ascii")
        with self.assertRaises(QuoteValidationError):
            parse_canonical_quote(duplicate)

    def test_unknown_missing_wrong_type_and_inconsistent_fields_reject(self) -> None:
        mutations = []
        unknown = quote_for()
        unknown["new_semantics"] = True
        mutations.append(unknown)
        missing = quote_for()
        del missing["condition"]
        mutations.append(missing)
        boolean_amount = quote_for()
        boolean_amount["pftl_amount_atoms"] = True
        mutations.append(boolean_amount)
        wrong_condition = quote_for()
        wrong_condition["condition"] = str(wrong_condition["condition"]).replace(
            "a0", "a1", 1
        )
        mutations.append(wrong_condition)
        empty_window = quote_for()
        empty_window["finish_after"] = empty_window["cancel_after"]
        mutations.append(empty_window)
        bad_time_order = quote_for()
        bad_time_order["latest_lightning_start_unix"] = 1_800_000_000
        mutations.append(bad_time_order)
        for quote in mutations:
            with self.subTest(quote=quote):
                with self.assertRaises(QuoteValidationError):
                    validate_quote(quote)

    def test_chain_identifiers_addresses_and_payee_are_canonical(self) -> None:
        mutations = {
            "pftl_genesis_hash": "22" * 47,
            "pftl_asset_id": "AA" * 48,
            "expected_escrow_id": "66" * 49,
            "pftl_owner": "PF" + "44" * 20,
            "pftl_recipient": "pf" + "55" * 19,
            "invoice_payee": "04" + "11" * 32,
        }
        for field, value in mutations.items():
            quote = quote_for()
            quote[field] = value
            with self.subTest(field=field):
                with self.assertRaises(QuoteValidationError):
                    validate_quote(quote)


class QuoteSignatureTests(unittest.TestCase):
    def setUp(self) -> None:
        self.signer = Ed25519Signer.from_private_bytes(TEST_SIGNING_SEED)
        self.envelope = sign_quote(quote_for(), self.signer)

    def test_ed25519_vector_and_envelope_are_pinned(self) -> None:
        self.assertEqual(self.envelope["public_key"], PINNED_PUBLIC_KEY_B64URL)
        self.assertEqual(self.envelope["key_id"], PINNED_KEY_ID)
        self.assertEqual(self.envelope["signature"], PINNED_SIGNATURE_B64URL)
        encoded = encode_signed_quote(self.envelope)
        self.assertEqual(
            hashlib.sha256(encoded).hexdigest(), PINNED_ENVELOPE_SHA256
        )
        self.assertEqual(
            parse_signed_quote(
                encoded, expected_public_key=self.signer.public_key_bytes()
            )["quote"],
            validate_quote(quote_for()),
        )

    def test_every_quote_field_is_covered_by_signature(self) -> None:
        for field, original in self.envelope["quote"].items():
            tampered = copy.deepcopy(self.envelope)
            if type(original) is int:
                tampered["quote"][field] = original + 1
            else:
                tampered["quote"][field] = f"{original}x"
            with self.subTest(field=field):
                with self.assertRaises(QuoteSignatureError):
                    verify_signed_quote(tampered)

    def test_expected_public_key_is_enforced(self) -> None:
        with self.assertRaises(QuoteSignatureError):
            verify_signed_quote(self.envelope, expected_public_key=bytes(32))

    def test_envelope_field_injection_and_noncanonical_encoding_reject(self) -> None:
        injected = dict(self.envelope)
        injected["unsigned_note"] = "ignored?"
        with self.assertRaises(QuoteSignatureError):
            verify_signed_quote(injected)
        pretty = json.dumps(self.envelope, sort_keys=True, indent=2).encode("ascii")
        with self.assertRaises(QuoteSignatureError):
            parse_signed_quote(pretty)

    def test_signature_and_public_key_encoding_are_strict(self) -> None:
        for field in ("signature", "public_key"):
            malformed = copy.deepcopy(self.envelope)
            malformed[field] = str(malformed[field]) + "="
            with self.subTest(field=field):
                with self.assertRaises(QuoteSignatureError):
                    verify_signed_quote(malformed)
