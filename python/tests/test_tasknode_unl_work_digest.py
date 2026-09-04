"""Focused tests for signed Task Node UNL work-digest reconciliation."""

from __future__ import annotations

import copy
import hashlib
import json
import unittest
from pathlib import Path

from eth_keys import keys

from postfiat_rpc import tasknode_unl_schema as schema
from postfiat_rpc.tasknode_unl_work_digest import (
    EVIDENCE_LIMITATIONS,
    WorkDigestVerificationResult,
    sign_work_digest,
    verify_work_digest,
)

FIXTURE_DIR = (
    Path(__file__).resolve().parent / "fixtures" / "tasknode_unl"
)


def _load(name: str) -> dict:
    return json.loads(
        (FIXTURE_DIR / name).read_text(encoding="utf-8")
    )


class ThrowawayWorkDigestSigner:
    """Deterministic in-test key derived only from a TEST-ONLY label."""

    algorithm_id = schema.WORK_DIGEST_SIGNATURE_ALGORITHM

    def __init__(self, label: str = "work-digest-publisher") -> None:
        scalar = hashlib.sha256(f"TEST-ONLY:{label}".encode()).digest()
        self._test_only_key = keys.PrivateKey(scalar)
        self.public_key_hex = (
            self._test_only_key.public_key.to_compressed_bytes().hex()
        )

    def sign_digest(self, digest: bytes) -> bytes:
        return self._test_only_key.sign_msg_hash(digest).to_bytes()


def _body() -> dict:
    return copy.deepcopy(_load("work-digests.json")["cases"][0]["body"])


def _ledger() -> dict:
    return copy.deepcopy(_load("ledger-pointers.json"))


def _publishing_keys() -> dict:
    return copy.deepcopy(_load("publishing-keys.json"))


def _signed_case(
    *,
    body: dict | None = None,
    signer: ThrowawayWorkDigestSigner | None = None,
) -> tuple[dict, dict, dict]:
    selected_body = body or _body()
    selected_signer = signer or ThrowawayWorkDigestSigner()
    envelope = sign_work_digest(selected_body, selected_signer)
    ledger = _ledger()
    ledger["anchor"]["anchored_digest_hash"] = envelope["digest_hash"]
    return envelope, ledger, _publishing_keys()


def _verify(envelope: dict, ledger: dict, publishing_keys: dict):
    body = envelope["body"]
    return verify_work_digest(
        envelope,
        ledger,
        publishing_keys,
        expected_account_id=body["account_id"],
        bound_wallet_address=body["bound_wallet_address"],
    )


def _failure_codes(result: WorkDigestVerificationResult) -> set[str]:
    return {failure.code for failure in result.failures}


class WorkDigestRoundTripTests(unittest.TestCase):
    def test_clean_digest_verifies_and_reconciles_exact_inputs(self) -> None:
        envelope, ledger, publishing_keys = _signed_case()

        result = _verify(envelope, ledger, publishing_keys)

        self.assertEqual(result.status, "verified")
        self.assertEqual(result.failures, ())
        self.assertEqual(
            result.reconciled_inputs,
            envelope["body"]["score_inputs"],
        )
        self.assertEqual(result.omitted_pointer_hashes, ())
        self.assertEqual(
            result.to_dict()["evidence_limitations"],
            list(EVIDENCE_LIMITATIONS),
        )

    def test_same_inputs_have_byte_identical_output(self) -> None:
        envelope, ledger, publishing_keys = _signed_case()

        first = _verify(envelope, ledger, publishing_keys)
        second = _verify(
            json.loads(json.dumps(envelope, sort_keys=False)),
            json.loads(json.dumps(ledger, sort_keys=False)),
            json.loads(json.dumps(publishing_keys, sort_keys=False)),
        )

        self.assertEqual(first.canonical_bytes(), second.canonical_bytes())


class LedgerReconciliationTests(unittest.TestCase):
    def test_pointer_missing_from_ledger_holds_with_named_field(self) -> None:
        envelope, ledger, publishing_keys = _signed_case()
        missing_hash = envelope["body"]["pointers"][1]["pointer_hash"]
        ledger["pointers"] = [
            pointer
            for pointer in ledger["pointers"]
            if pointer["pointer_hash"] != missing_hash
        ]

        result = _verify(envelope, ledger, publishing_keys)

        self.assertEqual(result.status, "hold")
        self.assertIn("pointer_missing_from_ledger", _failure_codes(result))
        self.assertTrue(
            any(missing_hash in failure.field for failure in result.failures)
        )
        self.assertIsNone(result.reconciled_inputs)

    def test_pointer_emitted_by_different_wallet_holds(self) -> None:
        envelope, ledger, publishing_keys = _signed_case()
        ledger["pointers"][0]["sender_wallet_address"] = (
            "rTESTONLYDifferentWallet"
        )

        result = _verify(envelope, ledger, publishing_keys)

        self.assertEqual(result.status, "hold")
        self.assertIn("pointer_wrong_sender", _failure_codes(result))
        self.assertTrue(
            any(
                failure.field.endswith(".sender_wallet_address")
                for failure in result.failures
            )
        )

    def test_on_ledger_pointer_omission_is_reported(self) -> None:
        envelope, ledger, publishing_keys = _signed_case()
        omitted_hash = "4444" * 16
        ledger["pointers"].append(
            {
                "pointer_hash": omitted_hash,
                "pointer_schema": "pf.ptr/v4",
                "sender_wallet_address": envelope["body"][
                    "bound_wallet_address"
                ],
                "account_id": envelope["body"]["account_id"],
                "ledger_index": 1004,
                "transaction_index": 0,
                "close_time": "2026-06-04T12:00:00Z",
            }
        )

        result = _verify(envelope, ledger, publishing_keys)

        self.assertEqual(result.status, "hold")
        self.assertIn("eligible_pointer_omitted", _failure_codes(result))
        self.assertEqual(result.omitted_pointer_hashes, (omitted_hash,))
        self.assertTrue(
            any(omitted_hash in failure.field for failure in result.failures)
        )

    def test_incomplete_frozen_view_never_passes_with_a_warning(self) -> None:
        envelope, ledger, publishing_keys = _signed_case()
        ledger["complete_for_account_window"] = False

        result = _verify(envelope, ledger, publishing_keys)

        self.assertEqual(result.status, "hold")
        self.assertIn("incomplete_frozen_view", _failure_codes(result))
        self.assertIsNone(result.reconciled_inputs)


class SignatureAndInputTests(unittest.TestCase):
    def test_wrong_publishing_key_signature_holds(self) -> None:
        envelope, ledger, publishing_keys = _signed_case(
            signer=ThrowawayWorkDigestSigner("wrong-publisher")
        )

        result = _verify(envelope, ledger, publishing_keys)

        self.assertEqual(result.status, "hold")
        self.assertTrue(
            _failure_codes(result)
            & {
                "signature_verification_failed",
                "signature_recovery_mismatch",
            }
        )
        self.assertTrue(
            all(failure.field for failure in result.failures)
        )
        self.assertIsNone(result.reconciled_inputs)

    def test_tampered_signature_holds(self) -> None:
        envelope, ledger, publishing_keys = _signed_case()
        envelope["signature_hex"] = "00" * 65

        result = _verify(envelope, ledger, publishing_keys)

        self.assertEqual(result.status, "hold")
        self.assertTrue(
            _failure_codes(result)
            & {
                "signature_verification_failed",
                "signature_recovery_failed",
                "signature_recovery_mismatch",
            }
        )

    def test_non_reconciling_score_input_holds_and_names_input(self) -> None:
        body = _body()
        body["score_inputs"]["verification_passes"] = 1
        envelope, ledger, publishing_keys = _signed_case(body=body)

        result = _verify(envelope, ledger, publishing_keys)

        self.assertEqual(result.status, "hold")
        self.assertIn("score_input_mismatch", _failure_codes(result))
        self.assertIn(
            "body.score_inputs.verification_passes",
            {failure.field for failure in result.failures},
        )
        self.assertIsNone(result.reconciled_inputs)

    def test_unknown_recorded_outcome_is_a_field_named_hold(self) -> None:
        envelope, ledger, publishing_keys = _signed_case()
        envelope["body"]["pointers"][0]["outcome"][
            "verification_outcome"
        ] = "maybe"

        result = _verify(envelope, ledger, publishing_keys)

        self.assertEqual(result.status, "hold")
        self.assertIn(
            "unknown_verification_outcome", _failure_codes(result)
        )
        self.assertIn(
            "body.pointers[0].outcome.verification_outcome",
            {failure.field for failure in result.failures},
        )

    def test_anchor_digest_mismatch_holds(self) -> None:
        envelope, ledger, publishing_keys = _signed_case()
        ledger["anchor"]["anchored_digest_hash"] = "ff" * 32

        result = _verify(envelope, ledger, publishing_keys)

        self.assertEqual(result.status, "hold")
        self.assertIn("anchor_digest_mismatch", _failure_codes(result))
        self.assertIsNone(result.reconciled_inputs)


if __name__ == "__main__":
    unittest.main()
