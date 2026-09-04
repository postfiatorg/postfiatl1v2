"""Focused tests for the offline Task Node UNL validator binding CLI."""

from __future__ import annotations

import contextlib
import hashlib
import io
import json
import tempfile
import unittest
from datetime import datetime, timezone
from pathlib import Path

from eth_keys import keys

from postfiat_rpc import tasknode_unl_schema as schema
from postfiat_rpc.tasknode_unl import build_parser, main
from postfiat_rpc.tasknode_unl_binding import (
    BindingChallenge,
    BindingLedgerRecord,
    ReattachmentEvidence,
    SignatureEnvelope,
    ValidatorKeyRotation,
    binding_evidence_fields,
    binding_memo_artifact,
    create_bind_memo,
    create_revoke_memo,
    prepare_bind_challenge,
    prepare_revoke_challenge,
    replay_bindings,
    sign_challenge,
    verified_record_document,
    verify_binding_record,
    wallet_address_from_public_key,
)

FIXTURE_PATH = (
    Path(__file__).resolve().parent
    / "fixtures"
    / "tasknode_unl"
    / "bindings.json"
)
UTC = timezone.utc


def _fixtures() -> dict:
    return json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))


def _time(value: str) -> datetime:
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


class ThrowawayTestSigner:
    """In-test secp256k1 signer; generated deterministically from a test label."""

    algorithm_id = schema.BINDING_SIGNATURE_ALGORITHM

    def __init__(self, label: str) -> None:
        scalar = hashlib.sha256(f"TEST-ONLY:{label}".encode()).digest()
        self._test_only_key = keys.PrivateKey(scalar)
        self.public_key_hex = (
            self._test_only_key.public_key.to_compressed_bytes().hex()
        )

    def sign_digest(self, digest: bytes) -> bytes:
        return self._test_only_key.sign_msg_hash(digest).to_bytes()


def _tx(name: str) -> str:
    return _fixtures()["transaction_hashes"][name]


def _nonce(name: str) -> str:
    return _fixtures()["nonces"][name]


def _validator(name: str = "primary") -> str:
    return _fixtures()["validator_ids"][name]


def _bind_challenge(
    validator_signer: ThrowawayTestSigner,
    wallet_signer: ThrowawayTestSigner,
    *,
    nonce: str = "bind_one",
    validator_id: str | None = None,
    previous_wallet_address: str | None = None,
) -> BindingChallenge:
    wallet_address = wallet_address_from_public_key(
        wallet_signer.public_key_hex
    )
    return prepare_bind_challenge(
        validator_id=validator_id or _validator(),
        validator_public_key_hex=validator_signer.public_key_hex,
        wallet_address=wallet_address,
        wallet_public_key_hex=wallet_signer.public_key_hex,
        nonce_hex=_nonce(nonce),
        previous_wallet_address=previous_wallet_address,
    )


def _bind_record(
    validator_signer: ThrowawayTestSigner,
    wallet_signer: ThrowawayTestSigner,
    *,
    tx_name: str = "bind_one",
    nonce: str = "bind_one",
    ledger_index: int = 1,
    close_time: str = "2026-01-01T00:00:00Z",
    validator_id: str | None = None,
    previous_wallet_address: str | None = None,
) -> BindingLedgerRecord:
    challenge = _bind_challenge(
        validator_signer,
        wallet_signer,
        nonce=nonce,
        validator_id=validator_id,
        previous_wallet_address=previous_wallet_address,
    )
    validator_signature = sign_challenge(
        challenge,
        role="validator",
        signer=validator_signer,
    )
    wallet_signature = sign_challenge(
        challenge,
        role="wallet",
        signer=wallet_signer,
    )
    memo = create_bind_memo(
        challenge,
        validator_signature,
        wallet_signature,
    )
    return BindingLedgerRecord(
        tx_hash=_tx(tx_name),
        ledger_index=ledger_index,
        transaction_index=0,
        close_time=_time(close_time),
        sender_wallet_address=challenge.wallet_address,
        challenge=challenge,
        memo=memo,
    )


def _revoke_record(
    active_record: BindingLedgerRecord,
    signer: ThrowawayTestSigner,
    *,
    role: str,
    ledger_index: int = 2,
    sender_wallet_address: str | None = None,
) -> BindingLedgerRecord:
    active = verify_binding_record(active_record)
    challenge = prepare_revoke_challenge(
        active,
        nonce_hex=_nonce("revoke"),
    )
    signature = sign_challenge(
        challenge,
        role=role,
        signer=signer,
    )
    memo = create_revoke_memo(challenge, signature)
    return BindingLedgerRecord(
        tx_hash=_tx("revoke"),
        ledger_index=ledger_index,
        transaction_index=0,
        close_time=datetime(2026, 1, 2, tzinfo=UTC),
        sender_wallet_address=(
            sender_wallet_address or challenge.wallet_address
        ),
        challenge=challenge,
        memo=memo,
    )


class BindingRoundTripTests(unittest.TestCase):
    def setUp(self) -> None:
        self.validator = ThrowawayTestSigner("validator-alpha")
        self.wallet = ThrowawayTestSigner("wallet-alpha")

    def test_valid_bind_sign_countersign_verify_and_fields(self) -> None:
        record = _bind_record(self.validator, self.wallet)
        event = verify_binding_record(record)
        evidence = binding_evidence_fields(event)
        artifact = binding_memo_artifact(record.memo)

        self.assertEqual(event.action, "bind")
        self.assertEqual(
            tuple(evidence),
            schema.TASKNODE_BINDING_EVIDENCE_FIELDS,
        )
        self.assertEqual(
            evidence[
                "validator.identity.tasknode_binding.wallet_address"
            ],
            record.sender_wallet_address,
        )
        self.assertEqual(
            evidence["validator.identity.tasknode_binding.tx_hash"],
            _tx("bind_one"),
        )
        self.assertEqual(
            evidence[
                "validator.identity.tasknode_binding.challenge_digest"
            ],
            record.challenge.digest_hex(),
        )
        self.assertLessEqual(
            artifact["memo_bytes"],
            schema.PFT_LEDGER_MEMO_MAX_BYTES,
        )
        self.assertFalse(artifact["submission_supported"])
        self.assertTrue(
            record.challenge.signing_bytes().startswith(
                schema.BINDING_CHALLENGE_DOMAIN.encode() + b"\x00"
            )
        )
        self.assertEqual(
            verified_record_document(event)["mode"],
            "SHADOW_ONLY",
        )

    def test_wrong_key_wallet_countersign_is_rejected(self) -> None:
        challenge = _bind_challenge(self.validator, self.wallet)
        validator_signature = sign_challenge(
            challenge,
            role="validator",
            signer=self.validator,
        )
        wrong_wallet = ThrowawayTestSigner("wrong-wallet")
        wrong_signature = wrong_wallet.sign_digest(challenge.digest())
        forged_envelope = SignatureEnvelope(
            role="wallet",
            algorithm=schema.BINDING_SIGNATURE_ALGORITHM,
            public_key_hex=self.wallet.public_key_hex,
            challenge_digest=challenge.digest_hex(),
            signature_hex=wrong_signature.hex(),
        )
        with self.assertRaisesRegex(
            schema.TaskNodeUnlError,
            "signature_verification_failed",
        ):
            create_bind_memo(
                challenge,
                validator_signature,
                forged_envelope,
            )

    def test_wallet_address_must_match_countersigning_key(self) -> None:
        other = ThrowawayTestSigner("other-wallet")
        with self.assertRaisesRegex(
            schema.TaskNodeUnlError,
            "wallet_address_public_key_mismatch",
        ):
            prepare_bind_challenge(
                validator_id=_validator(),
                validator_public_key_hex=self.validator.public_key_hex,
                wallet_address=wallet_address_from_public_key(
                    other.public_key_hex
                ),
                wallet_public_key_hex=self.wallet.public_key_hex,
                nonce_hex=_nonce("bind_one"),
            )

    def test_bind_memo_must_be_sent_by_the_bound_wallet(self) -> None:
        record = _bind_record(self.validator, self.wallet)
        relay = ThrowawayTestSigner("bind-relay")
        wrong_sender = BindingLedgerRecord(
            tx_hash=record.tx_hash,
            ledger_index=record.ledger_index,
            transaction_index=record.transaction_index,
            close_time=record.close_time,
            sender_wallet_address=wallet_address_from_public_key(
                relay.public_key_hex
            ),
            challenge=record.challenge,
            memo=record.memo,
        )
        with self.assertRaisesRegex(
            schema.TaskNodeUnlError,
            "binding_memo_wrong_sender",
        ):
            verify_binding_record(wrong_sender)

    def test_same_inputs_produce_byte_identical_artifacts(self) -> None:
        first = _bind_record(self.validator, self.wallet)
        second = _bind_record(self.validator, self.wallet)
        self.assertEqual(first.challenge.digest(), second.challenge.digest())
        self.assertEqual(first.memo.payload_bytes(), second.memo.payload_bytes())
        self.assertEqual(
            json.dumps(first.to_dict(), sort_keys=True),
            json.dumps(second.to_dict(), sort_keys=True),
        )


class ReplayRuleTests(unittest.TestCase):
    def setUp(self) -> None:
        self.validator = ThrowawayTestSigner("validator-alpha")
        self.wallet = ThrowawayTestSigner("wallet-alpha")
        self.evaluation_end = datetime(2026, 1, 10, tzinfo=UTC)

    def test_later_bind_from_same_wallet_supersedes_prior_memo(self) -> None:
        first = _bind_record(self.validator, self.wallet)
        second = _bind_record(
            self.validator,
            self.wallet,
            tx_name="bind_two",
            nonce="bind_two",
            ledger_index=2,
            close_time="2026-01-02T00:00:00Z",
        )
        result = replay_bindings(
            (second, first),
            (),
            (),
            evaluation_end=self.evaluation_end,
        )

        self.assertEqual(result.status, "ready")
        self.assertEqual(len(result.active_bindings), 1)
        self.assertEqual(result.active_bindings[0].tx_hash, _tx("bind_two"))
        self.assertEqual(
            result.decisions[-1].reason,
            "superseded_prior_binding",
        )
        self.assertEqual(
            result.canonical_bytes(),
            replay_bindings(
                (first, second),
                (),
                (),
                evaluation_end=self.evaluation_end,
            ).canonical_bytes(),
        )

    def test_revoke_is_accepted_from_each_bound_key(self) -> None:
        bind = _bind_record(self.validator, self.wallet)
        relay = ThrowawayTestSigner("revoke-relay")
        relay_address = wallet_address_from_public_key(relay.public_key_hex)
        for role, signer in (
            ("validator", self.validator),
            ("wallet", self.wallet),
        ):
            with self.subTest(role=role):
                revoke = _revoke_record(
                    bind,
                    signer,
                    role=role,
                    sender_wallet_address=(
                        relay_address if role == "validator" else None
                    ),
                )
                result = replay_bindings(
                    (bind, revoke),
                    (),
                    (),
                    evaluation_end=self.evaluation_end,
                )
                self.assertEqual(result.active_bindings, ())
                self.assertEqual(
                    result.decisions[-1].reason,
                    f"revoked_by_{role}",
                )
                expected_freeze = (
                    ((bind.sender_wallet_address, 2),)
                    if role == "validator"
                    else ()
                )
                self.assertEqual(
                    result.frozen_work_history,
                    expected_freeze,
                )

    def test_second_validator_for_one_wallet_flags_shared_control(self) -> None:
        first = _bind_record(self.validator, self.wallet)
        other_validator = ThrowawayTestSigner("validator-beta")
        second = _bind_record(
            other_validator,
            self.wallet,
            tx_name="bind_two",
            nonce="bind_two",
            ledger_index=2,
            close_time="2026-01-02T00:00:00Z",
            validator_id=_validator("secondary"),
        )
        result = replay_bindings(
            (first, second),
            (),
            (),
            evaluation_end=self.evaluation_end,
        )

        self.assertEqual(len(result.active_bindings), 1)
        self.assertEqual(
            result.active_bindings[0].validator_id,
            _validator(),
        )
        self.assertEqual(
            result.shared_control_evidence,
            (
                (
                    first.sender_wallet_address,
                    (_validator(), _validator("secondary")),
                ),
            ),
        )
        self.assertEqual(result.decisions[-1].outcome, "shared_control")

    def test_rotation_without_timely_rebind_holds_after_one_window(self) -> None:
        old_bind = _bind_record(self.validator, self.wallet)
        replacement = ThrowawayTestSigner("validator-rotated")
        rotation = ValidatorKeyRotation(
            validator_id=_validator(),
            previous_public_key_hex=self.validator.public_key_hex,
            new_public_key_hex=replacement.public_key_hex,
            ledger_index=2,
            transaction_index=0,
            rotated_at=_time(_fixtures()["times"]["rotation"]),
        )
        result = replay_bindings(
            (old_bind,),
            (rotation,),
            (),
            evaluation_end=_time(
                _fixtures()["times"]["expired_evaluation"]
            ),
        )
        self.assertEqual(result.status, "hold")
        self.assertIn(
            f"rotation_rebind_expired:{_validator()}",
            result.hold_reasons,
        )
        self.assertEqual(result.pending_rotation_rebind, ())

    def test_rotation_rebound_in_window_supersedes_old_key(self) -> None:
        old_bind = _bind_record(self.validator, self.wallet)
        replacement = ThrowawayTestSigner("validator-rotated")
        rotation = ValidatorKeyRotation(
            validator_id=_validator(),
            previous_public_key_hex=self.validator.public_key_hex,
            new_public_key_hex=replacement.public_key_hex,
            ledger_index=2,
            transaction_index=0,
            rotated_at=_time(_fixtures()["times"]["rotation"]),
        )
        new_bind = _bind_record(
            replacement,
            self.wallet,
            tx_name="bind_two",
            nonce="bind_two",
            ledger_index=3,
            close_time=_fixtures()["times"]["bind_two"],
        )
        result = replay_bindings(
            (new_bind, old_bind),
            (rotation,),
            (),
            evaluation_end=_time(
                _fixtures()["times"]["timely_evaluation"]
            ),
        )
        self.assertEqual(result.status, "ready")
        self.assertEqual(result.pending_rotation_rebind, ())
        self.assertEqual(
            result.active_bindings[0].validator_public_key_hex,
            replacement.public_key_hex,
        )

    def test_rotation_inside_window_is_pending_not_expired(self) -> None:
        old_bind = _bind_record(self.validator, self.wallet)
        replacement = ThrowawayTestSigner("validator-rotated")
        rotation = ValidatorKeyRotation(
            validator_id=_validator(),
            previous_public_key_hex=self.validator.public_key_hex,
            new_public_key_hex=replacement.public_key_hex,
            ledger_index=2,
            transaction_index=0,
            rotated_at=_time(_fixtures()["times"]["rotation"]),
        )
        result = replay_bindings(
            (old_bind,),
            (rotation,),
            (),
            evaluation_end=_time(
                _fixtures()["times"]["timely_evaluation"]
            ),
        )
        self.assertEqual(result.status, "ready")
        self.assertEqual(
            result.pending_rotation_rebind,
            (_validator(),),
        )

    def test_validator_revoke_freezes_history_and_requires_two_cowork_vouches(
        self,
    ) -> None:
        old_bind = _bind_record(self.validator, self.wallet)
        revoke = _revoke_record(
            old_bind,
            self.validator,
            role="validator",
        )
        new_wallet = ThrowawayTestSigner("replacement-wallet")
        reattach = _bind_record(
            self.validator,
            new_wallet,
            tx_name="reattach",
            nonce="reattach",
            ledger_index=3,
            close_time="2026-01-03T00:00:00Z",
            previous_wallet_address=old_bind.sender_wallet_address,
        )

        missing = replay_bindings(
            (old_bind, revoke, reattach),
            (),
            (),
            evaluation_end=self.evaluation_end,
        )
        self.assertEqual(missing.status, "hold")
        self.assertIn(
            f"reattachment_vouches_missing:{_validator()}",
            missing.hold_reasons,
        )

        evidence = ReattachmentEvidence(
            binding_tx_hash=_tx("reattach"),
            frozen_wallet_address=old_bind.sender_wallet_address,
            cowork_accounts=("account-a", "account-b", "account-c"),
            valid_vouch_accounts=("account-b", "account-c", "outsider"),
        )
        accepted = replay_bindings(
            (reattach, revoke, old_bind),
            (),
            (evidence,),
            evaluation_end=self.evaluation_end,
        )
        self.assertEqual(accepted.status, "ready")
        self.assertEqual(
            accepted.active_bindings[0].wallet_address,
            reattach.sender_wallet_address,
        )
        self.assertEqual(
            accepted.frozen_work_history,
            ((old_bind.sender_wallet_address, 2),),
        )


class OfflineCliTests(unittest.TestCase):
    def setUp(self) -> None:
        self.validator = ThrowawayTestSigner("cli-validator")
        self.wallet = ThrowawayTestSigner("cli-wallet")

    def test_cli_prepares_and_finalizes_without_private_key_arguments(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            challenge_path = root / "challenge.json"
            validator_signature_path = root / "validator-signature.json"
            wallet_signature_path = root / "wallet-signature.json"
            memo_path = root / "memo.json"
            record_path = root / "record.json"
            verification_path = root / "verification.json"

            rc = main(
                [
                    "prepare-bind",
                    "--validator-id",
                    _validator(),
                    "--validator-public-key-hex",
                    self.validator.public_key_hex,
                    "--wallet-address",
                    wallet_address_from_public_key(
                        self.wallet.public_key_hex
                    ),
                    "--wallet-public-key-hex",
                    self.wallet.public_key_hex,
                    "--nonce-hex",
                    _nonce("bind_one"),
                    "--output",
                    str(challenge_path),
                ]
            )
            self.assertEqual(rc, 0)
            challenge = BindingChallenge(
                action="bind",
                validator_id=_validator(),
                validator_public_key_hex=self.validator.public_key_hex,
                wallet_address=wallet_address_from_public_key(
                    self.wallet.public_key_hex
                ),
                wallet_public_key_hex=self.wallet.public_key_hex,
                nonce_hex=_nonce("bind_one"),
            )
            self.assertEqual(
                json.loads(challenge_path.read_text()),
                challenge.to_dict(),
            )
            validator_signature_path.write_text(
                json.dumps(
                    sign_challenge(
                        challenge,
                        role="validator",
                        signer=self.validator,
                    ).to_dict()
                ),
                encoding="utf-8",
            )
            wallet_signature_path.write_text(
                json.dumps(
                    sign_challenge(
                        challenge,
                        role="wallet",
                        signer=self.wallet,
                    ).to_dict()
                ),
                encoding="utf-8",
            )

            rc = main(
                [
                    "finalize-bind",
                    "--challenge",
                    str(challenge_path),
                    "--validator-signature",
                    str(validator_signature_path),
                    "--wallet-signature",
                    str(wallet_signature_path),
                    "--output",
                    str(memo_path),
                ]
            )
            self.assertEqual(rc, 0)
            artifact = json.loads(memo_path.read_text())
            self.assertEqual(artifact["mode"], "SHADOW_ONLY")
            self.assertFalse(artifact["submission_supported"])
            self.assertLessEqual(
                artifact["memo_bytes"],
                schema.PFT_LEDGER_MEMO_MAX_BYTES,
            )

            record_path.write_text(
                json.dumps(
                    {
                        "schema": schema.BINDING_LEDGER_RECORD_SCHEMA,
                        "tx_hash": _tx("bind_one"),
                        "ledger_index": 1,
                        "transaction_index": 0,
                        "close_time": "2026-01-01T00:00:00Z",
                        "sender_wallet_address": challenge.wallet_address,
                        "challenge": challenge.to_dict(),
                        "memo": artifact["memo_payload"],
                    }
                ),
                encoding="utf-8",
            )
            rc = main(
                [
                    "verify-record",
                    "--record",
                    str(record_path),
                    "--output",
                    str(verification_path),
                ]
            )
            self.assertEqual(rc, 0)
            verification = json.loads(verification_path.read_text())
            self.assertTrue(verification["verified"])
            self.assertEqual(
                tuple(verification["evidence_fields"]),
                tuple(sorted(schema.TASKNODE_BINDING_EVIDENCE_FIELDS)),
            )

        parser = build_parser()
        with contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit):
                parser.parse_args(
                    [
                        "prepare-bind",
                        "--private-key",
                        "TEST-ONLY",
                    ]
                )

    def test_cli_has_no_submit_or_live_command(self) -> None:
        parser = build_parser()
        subparser_action = next(
            action
            for action in parser._actions
            if isinstance(action, __import__("argparse")._SubParsersAction)
        )
        self.assertEqual(
            set(subparser_action.choices),
            {
                "prepare-bind",
                "finalize-bind",
                "prepare-revoke",
                "finalize-revoke",
                "verify-record",
                "replay",
                "shadow-derive",
                "derive",
            },
        )
        self.assertNotIn("submit", subparser_action.choices)
        self.assertNotIn("send", subparser_action.choices)

    def test_binding_modules_import_no_network_or_subprocess_clients(self) -> None:
        root = Path(__file__).resolve().parents[1] / "postfiat_rpc"
        source = "\n".join(
            (root / name).read_text(encoding="utf-8")
            for name in ("tasknode_unl.py", "tasknode_unl_binding.py")
        )
        for forbidden in (
            "import requests",
            "import urllib",
            "import socket",
            "import http.client",
            "import aiohttp",
            "import subprocess",
        ):
            with self.subTest(forbidden=forbidden):
                self.assertNotIn(forbidden, source)

    def test_fixture_is_explicitly_test_only_and_has_no_keys(self) -> None:
        fixture = _fixtures()
        self.assertIn("TEST ONLY", fixture["notice"])
        serialized = json.dumps(fixture).lower()
        self.assertNotIn("private_key", serialized)
        self.assertNotIn("seed", serialized)


if __name__ == "__main__":
    unittest.main()
