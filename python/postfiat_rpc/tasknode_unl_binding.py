"""Offline validator-key to Task Node wallet binding primitives.

The module is pure apart from caller-controlled file handling in the companion
CLI. It performs no network access, transaction construction, transaction
submission, clock reads, random generation, or credential-store access.
Private keys are never accepted: custody-preserving signer adapters receive
only a 32-byte digest and return a detached signature.
"""

from __future__ import annotations

import hashlib
from dataclasses import dataclass
from datetime import datetime, timedelta
from typing import Any, Protocol, Sequence

from eth_keys import keys
from eth_keys.constants import SECPK1_N

from .tasknode_unl_schema import (
    BINDING_CHALLENGE_DOMAIN,
    BINDING_CHALLENGE_SCHEMA,
    BINDING_EVALUATION_WINDOW_DAYS,
    BINDING_LEDGER_RECORD_SCHEMA,
    BINDING_MEMO_ARTIFACT_SCHEMA,
    BINDING_MEMO_SCHEMA,
    BINDING_REPLAY_INPUT_SCHEMA,
    BINDING_REPLAY_RESULT_SCHEMA,
    BINDING_SIGNATURE_ALGORITHM,
    BINDING_SIGNATURE_SCHEMA,
    BINDING_VERIFICATION_RESULT_SCHEMA,
    PFT_LEDGER_MEMO_MAX_BYTES,
    SHADOW_MODE,
    TASKNODE_BINDING_EVIDENCE_FIELDS,
    TaskNodeUnlError,
    canonical_json_bytes,
    format_utc_timestamp,
    parse_utc_timestamp,
    require_closed_keys,
    require_identifier,
    require_int,
)

_BIND_ACTION = "bind"
_REVOKE_ACTION = "revoke"
_ACTIONS = (_BIND_ACTION, _REVOKE_ACTION)
_SIGNER_ROLES = ("validator", "wallet")
_SECP256K1_PUBLIC_KEY_BYTES = 33
_SECP256K1_SIGNATURE_BYTES = 65
_SHA256_BYTES = 32
_MAX_IDENTIFIER_BYTES = 96
_MAX_VALIDATOR_ID_BYTES = 48
_MAX_WALLET_ADDRESS_BYTES = 80
_MAX_RECORDS = 4_096
_MAX_ROTATIONS = 1_024
_MAX_REATTACHMENTS = 1_024
_MAX_LEDGER_INDEX = (1 << 63) - 1
_RIPPLE_BASE58_ALPHABET = (
    "rpshnaf39wBUDNEGHJKLM4PQRST7VWXYZ2bcdeCg65jkm8oFqi1tuvAxyz"
)


class SignerAdapter(Protocol):
    """Custody boundary for an external or in-memory signing implementation."""

    algorithm_id: str
    public_key_hex: str

    def sign_digest(self, digest: bytes) -> bytes:
        """Sign exactly one 32-byte digest without exposing private material."""


def _require_bounded_identifier(
    value: object,
    field: str,
    *,
    maximum_bytes: int = _MAX_IDENTIFIER_BYTES,
) -> str:
    checked = require_identifier(value, field)
    if len(checked.encode("utf-8")) > maximum_bytes:
        raise TaskNodeUnlError("identifier_too_long", field)
    return checked


def _require_lower_hex(
    value: object,
    field: str,
    *,
    byte_length: int,
) -> str:
    if not isinstance(value, str) or len(value) != byte_length * 2:
        raise TaskNodeUnlError("invalid_hex_length", field)
    if value != value.lower():
        raise TaskNodeUnlError("non_canonical_hex", field)
    try:
        bytes.fromhex(value)
    except ValueError as exc:
        raise TaskNodeUnlError("invalid_hex", field) from exc
    return value


def _require_position(
    ledger_index: object,
    transaction_index: object,
) -> tuple[int, int]:
    ledger = require_int(ledger_index, "ledger_index", minimum=1)
    transaction = require_int(transaction_index, "transaction_index", minimum=0)
    if ledger > _MAX_LEDGER_INDEX or transaction > _MAX_LEDGER_INDEX:
        raise TaskNodeUnlError("ledger_position_out_of_range")
    return ledger, transaction


def _require_aware_timestamp(value: datetime, field: str) -> datetime:
    if value.tzinfo is None or value.utcoffset() is None:
        raise TaskNodeUnlError("timestamp_missing_timezone", field)
    return value


def _public_key(public_key_hex: object, field: str) -> keys.PublicKey:
    checked = _require_lower_hex(
        public_key_hex,
        field,
        byte_length=_SECP256K1_PUBLIC_KEY_BYTES,
    )
    raw = bytes.fromhex(checked)
    if raw[0] not in (2, 3):
        raise TaskNodeUnlError("invalid_compressed_public_key", field)
    try:
        public_key = keys.PublicKey.from_compressed_bytes(raw)
    except Exception as exc:
        raise TaskNodeUnlError("invalid_secp256k1_public_key", field) from exc
    if public_key.to_compressed_bytes() != raw:
        raise TaskNodeUnlError("non_canonical_public_key", field)
    return public_key


def _base58_encode(value: bytes) -> str:
    number = int.from_bytes(value, "big")
    encoded = ""
    while number:
        number, remainder = divmod(number, 58)
        encoded = _RIPPLE_BASE58_ALPHABET[remainder] + encoded
    leading_zeroes = len(value) - len(value.lstrip(b"\x00"))
    return _RIPPLE_BASE58_ALPHABET[0] * leading_zeroes + encoded


def wallet_address_from_public_key(public_key_hex: str) -> str:
    """Derive an XRPL/PFT Ledger classic address from a compressed public key."""

    public_key = _public_key(public_key_hex, "wallet_public_key_hex")
    compressed = public_key.to_compressed_bytes()
    sha256 = hashlib.sha256(compressed).digest()
    try:
        account_id = hashlib.new("ripemd160", sha256).digest()
    except ValueError as exc:
        raise TaskNodeUnlError("ripemd160_unavailable") from exc
    payload = b"\x00" + account_id
    checksum = hashlib.sha256(hashlib.sha256(payload).digest()).digest()[:4]
    return _base58_encode(payload + checksum)


@dataclass(frozen=True)
class BindingChallenge:
    action: str
    validator_id: str
    validator_public_key_hex: str
    wallet_address: str
    wallet_public_key_hex: str
    nonce_hex: str
    binding_tx_hash: str | None = None
    previous_wallet_address: str | None = None

    def validate(self) -> None:
        if self.action not in _ACTIONS:
            raise TaskNodeUnlError("unknown_binding_action", self.action)
        _require_bounded_identifier(
            self.validator_id,
            "validator_id",
            maximum_bytes=_MAX_VALIDATOR_ID_BYTES,
        )
        _public_key(
            self.validator_public_key_hex,
            "validator_public_key_hex",
        )
        wallet_address = _require_bounded_identifier(
            self.wallet_address,
            "wallet_address",
            maximum_bytes=_MAX_WALLET_ADDRESS_BYTES,
        )
        _public_key(self.wallet_public_key_hex, "wallet_public_key_hex")
        if wallet_address_from_public_key(self.wallet_public_key_hex) != wallet_address:
            raise TaskNodeUnlError("wallet_address_public_key_mismatch")
        _require_lower_hex(
            self.nonce_hex,
            "nonce_hex",
            byte_length=_SHA256_BYTES,
        )
        if self.action == _BIND_ACTION:
            if self.binding_tx_hash is not None:
                raise TaskNodeUnlError("bind_has_binding_tx_hash")
            if self.previous_wallet_address is not None:
                _require_bounded_identifier(
                    self.previous_wallet_address,
                    "previous_wallet_address",
                    maximum_bytes=_MAX_WALLET_ADDRESS_BYTES,
                )
                if self.previous_wallet_address == self.wallet_address:
                    raise TaskNodeUnlError("reattachment_wallet_unchanged")
        else:
            if self.binding_tx_hash is None:
                raise TaskNodeUnlError("revoke_missing_binding_tx_hash")
            _require_lower_hex(
                self.binding_tx_hash,
                "binding_tx_hash",
                byte_length=_SHA256_BYTES,
            )
            if self.previous_wallet_address is not None:
                raise TaskNodeUnlError("revoke_has_previous_wallet_address")
        if len(canonical_json_bytes(self.to_dict())) > 2_048:
            raise TaskNodeUnlError("binding_challenge_too_large")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": BINDING_CHALLENGE_SCHEMA,
            "mode": SHADOW_MODE,
            "signature_algorithm": BINDING_SIGNATURE_ALGORITHM,
            "action": self.action,
            "validator_id": self.validator_id,
            "validator_public_key_hex": self.validator_public_key_hex,
            "wallet_address": self.wallet_address,
            "wallet_public_key_hex": self.wallet_public_key_hex,
            "nonce_hex": self.nonce_hex,
            "binding_tx_hash": self.binding_tx_hash,
            "previous_wallet_address": self.previous_wallet_address,
        }

    def signing_bytes(self) -> bytes:
        self.validate()
        return (
            BINDING_CHALLENGE_DOMAIN.encode("ascii")
            + b"\x00"
            + canonical_json_bytes(self.to_dict())
        )

    def digest(self) -> bytes:
        return hashlib.sha256(self.signing_bytes()).digest()

    def digest_hex(self) -> str:
        return self.digest().hex()


@dataclass(frozen=True)
class SignatureEnvelope:
    role: str
    algorithm: str
    public_key_hex: str
    challenge_digest: str
    signature_hex: str

    def to_dict(self) -> dict[str, str]:
        return {
            "schema": BINDING_SIGNATURE_SCHEMA,
            "mode": SHADOW_MODE,
            "role": self.role,
            "algorithm": self.algorithm,
            "public_key_hex": self.public_key_hex,
            "challenge_digest": self.challenge_digest,
            "signature_hex": self.signature_hex,
        }


def _expected_public_key(challenge: BindingChallenge, role: str) -> str:
    if role == "validator":
        return challenge.validator_public_key_hex
    if role == "wallet":
        return challenge.wallet_public_key_hex
    raise TaskNodeUnlError("unknown_signer_role", role)


def verify_signature_envelope(
    challenge: BindingChallenge,
    envelope: SignatureEnvelope,
    *,
    expected_role: str,
) -> None:
    """Verify one detached signature against the exact challenge digest."""

    challenge.validate()
    if expected_role not in _SIGNER_ROLES:
        raise TaskNodeUnlError("unknown_signer_role", expected_role)
    if envelope.role != expected_role:
        raise TaskNodeUnlError("signature_role_mismatch", expected_role)
    if envelope.algorithm != BINDING_SIGNATURE_ALGORITHM:
        raise TaskNodeUnlError("unknown_signature_algorithm", envelope.algorithm)
    expected_key = _expected_public_key(challenge, expected_role)
    if envelope.public_key_hex != expected_key:
        raise TaskNodeUnlError("signature_public_key_mismatch", expected_role)
    if envelope.challenge_digest != challenge.digest_hex():
        raise TaskNodeUnlError("signature_challenge_mismatch", expected_role)
    public_key = _public_key(envelope.public_key_hex, "public_key_hex")
    signature_hex = _require_lower_hex(
        envelope.signature_hex,
        "signature_hex",
        byte_length=_SECP256K1_SIGNATURE_BYTES,
    )
    try:
        signature = keys.Signature(bytes.fromhex(signature_hex))
    except Exception as exc:
        raise TaskNodeUnlError("invalid_signature_encoding", expected_role) from exc
    digest = challenge.digest()
    if signature.s > SECPK1_N // 2:
        raise TaskNodeUnlError("non_canonical_signature", expected_role)
    if not public_key.verify_msg_hash(digest, signature):
        raise TaskNodeUnlError("signature_verification_failed", expected_role)
    try:
        recovered = signature.recover_public_key_from_msg_hash(digest)
    except Exception as exc:
        raise TaskNodeUnlError(
            "signature_recovery_failed",
            expected_role,
        ) from exc
    if recovered != public_key:
        raise TaskNodeUnlError("signature_recovery_mismatch", expected_role)


def sign_challenge(
    challenge: BindingChallenge,
    *,
    role: str,
    signer: SignerAdapter,
) -> SignatureEnvelope:
    """Obtain and self-verify a signature through a custody-preserving adapter."""

    if role not in _SIGNER_ROLES:
        raise TaskNodeUnlError("unknown_signer_role", role)
    if signer.algorithm_id != BINDING_SIGNATURE_ALGORITHM:
        raise TaskNodeUnlError(
            "unknown_signature_algorithm",
            signer.algorithm_id,
        )
    expected_key = _expected_public_key(challenge, role)
    if signer.public_key_hex != expected_key:
        raise TaskNodeUnlError("signer_public_key_mismatch", role)
    signature = signer.sign_digest(challenge.digest())
    if not isinstance(signature, bytes):
        raise TaskNodeUnlError("signer_returned_non_bytes", role)
    envelope = SignatureEnvelope(
        role=role,
        algorithm=signer.algorithm_id,
        public_key_hex=signer.public_key_hex,
        challenge_digest=challenge.digest_hex(),
        signature_hex=signature.hex(),
    )
    verify_signature_envelope(challenge, envelope, expected_role=role)
    return envelope


@dataclass(frozen=True)
class BindingMemo:
    action: str
    validator_id: str
    wallet_address: str
    challenge_digest: str
    validator_signature: str | None
    wallet_signature: str | None

    def to_payload(self) -> dict[str, Any]:
        return {
            "s": BINDING_MEMO_SCHEMA,
            "m": SHADOW_MODE,
            "o": self.action,
            "v": self.validator_id,
            "w": self.wallet_address,
            "d": self.challenge_digest,
            "vs": self.validator_signature,
            "ws": self.wallet_signature,
        }

    def payload_bytes(self) -> bytes:
        encoded = canonical_json_bytes(self.to_payload())
        if len(encoded) > PFT_LEDGER_MEMO_MAX_BYTES:
            raise TaskNodeUnlError(
                "binding_memo_too_large",
                str(len(encoded)),
            )
        return encoded


def _validate_memo_shape(memo: BindingMemo) -> None:
    if memo.action not in _ACTIONS:
        raise TaskNodeUnlError("unknown_binding_action", memo.action)
    _require_bounded_identifier(
        memo.validator_id,
        "memo.validator_id",
        maximum_bytes=_MAX_VALIDATOR_ID_BYTES,
    )
    _require_bounded_identifier(
        memo.wallet_address,
        "memo.wallet_address",
        maximum_bytes=_MAX_WALLET_ADDRESS_BYTES,
    )
    _require_lower_hex(
        memo.challenge_digest,
        "memo.challenge_digest",
        byte_length=_SHA256_BYTES,
    )
    signatures = (
        ("validator", memo.validator_signature),
        ("wallet", memo.wallet_signature),
    )
    present = 0
    for role, signature in signatures:
        if signature is not None:
            present += 1
            _require_lower_hex(
                signature,
                f"memo.{role}_signature",
                byte_length=_SECP256K1_SIGNATURE_BYTES,
            )
    required_count = 2 if memo.action == _BIND_ACTION else 1
    if present != required_count:
        raise TaskNodeUnlError("binding_memo_signature_count")
    memo.payload_bytes()


def create_bind_memo(
    challenge: BindingChallenge,
    validator_signature: SignatureEnvelope,
    wallet_signature: SignatureEnvelope,
) -> BindingMemo:
    """Verify both signatures and build a bounded bind memo payload."""

    if challenge.action != _BIND_ACTION:
        raise TaskNodeUnlError("challenge_action_mismatch", _BIND_ACTION)
    verify_signature_envelope(
        challenge,
        validator_signature,
        expected_role="validator",
    )
    verify_signature_envelope(
        challenge,
        wallet_signature,
        expected_role="wallet",
    )
    memo = BindingMemo(
        action=_BIND_ACTION,
        validator_id=challenge.validator_id,
        wallet_address=challenge.wallet_address,
        challenge_digest=challenge.digest_hex(),
        validator_signature=validator_signature.signature_hex,
        wallet_signature=wallet_signature.signature_hex,
    )
    _validate_memo_shape(memo)
    return memo


def create_revoke_memo(
    challenge: BindingChallenge,
    signature: SignatureEnvelope,
) -> BindingMemo:
    """Verify either bound key's signature and build a bounded revoke memo."""

    if challenge.action != _REVOKE_ACTION:
        raise TaskNodeUnlError("challenge_action_mismatch", _REVOKE_ACTION)
    if signature.role not in _SIGNER_ROLES:
        raise TaskNodeUnlError("unknown_signer_role", signature.role)
    verify_signature_envelope(
        challenge,
        signature,
        expected_role=signature.role,
    )
    memo = BindingMemo(
        action=_REVOKE_ACTION,
        validator_id=challenge.validator_id,
        wallet_address=challenge.wallet_address,
        challenge_digest=challenge.digest_hex(),
        validator_signature=(
            signature.signature_hex if signature.role == "validator" else None
        ),
        wallet_signature=(
            signature.signature_hex if signature.role == "wallet" else None
        ),
    )
    _validate_memo_shape(memo)
    return memo


def binding_memo_artifact(memo: BindingMemo) -> dict[str, Any]:
    """Return the local-only artifact containing the PFT Ledger memo payload."""

    _validate_memo_shape(memo)
    payload = memo.to_payload()
    encoded = memo.payload_bytes()
    return {
        "schema": BINDING_MEMO_ARTIFACT_SCHEMA,
        "mode": SHADOW_MODE,
        "memo_payload": payload,
        "memo_hex": encoded.hex(),
        "memo_bytes": len(encoded),
        "submission_supported": False,
    }


@dataclass(frozen=True)
class BindingLedgerRecord:
    tx_hash: str
    ledger_index: int
    transaction_index: int
    close_time: datetime
    sender_wallet_address: str
    challenge: BindingChallenge
    memo: BindingMemo

    @property
    def position(self) -> tuple[int, int]:
        return self.ledger_index, self.transaction_index

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": BINDING_LEDGER_RECORD_SCHEMA,
            "tx_hash": self.tx_hash,
            "ledger_index": self.ledger_index,
            "transaction_index": self.transaction_index,
            "close_time": format_utc_timestamp(self.close_time),
            "sender_wallet_address": self.sender_wallet_address,
            "challenge": self.challenge.to_dict(),
            "memo": self.memo.to_payload(),
        }


@dataclass(frozen=True)
class VerifiedBindingEvent:
    action: str
    validator_id: str
    validator_public_key_hex: str
    wallet_address: str
    wallet_public_key_hex: str
    challenge_digest: str
    validator_signature: str | None
    wallet_signature: str | None
    tx_hash: str
    ledger_index: int
    transaction_index: int
    close_time: datetime
    binding_tx_hash: str | None
    previous_wallet_address: str | None

    @property
    def position(self) -> tuple[int, int]:
        return self.ledger_index, self.transaction_index

    @property
    def revoke_role(self) -> str | None:
        if self.action != _REVOKE_ACTION:
            return None
        return "validator" if self.validator_signature is not None else "wallet"


def verify_binding_record(record: BindingLedgerRecord) -> VerifiedBindingEvent:
    """Verify sender, challenge, memo, and every present signature locally."""

    tx_hash = _require_lower_hex(
        record.tx_hash,
        "tx_hash",
        byte_length=_SHA256_BYTES,
    )
    ledger_index, transaction_index = _require_position(
        record.ledger_index,
        record.transaction_index,
    )
    close_time = _require_aware_timestamp(record.close_time, "close_time")
    sender = _require_bounded_identifier(
        record.sender_wallet_address,
        "sender_wallet_address",
        maximum_bytes=_MAX_WALLET_ADDRESS_BYTES,
    )
    challenge = record.challenge
    challenge.validate()
    memo = record.memo
    _validate_memo_shape(memo)
    if challenge.action == _BIND_ACTION and sender != challenge.wallet_address:
        raise TaskNodeUnlError("binding_memo_wrong_sender")
    if (
        memo.action != challenge.action
        or memo.validator_id != challenge.validator_id
        or memo.wallet_address != challenge.wallet_address
        or memo.challenge_digest != challenge.digest_hex()
    ):
        raise TaskNodeUnlError("memo_challenge_mismatch")

    digest = challenge.digest_hex()
    if challenge.action == _BIND_ACTION:
        if (
            memo.validator_signature is None
            or memo.wallet_signature is None
        ):
            raise TaskNodeUnlError("binding_memo_signature_count")
        validator_envelope = SignatureEnvelope(
            role="validator",
            algorithm=BINDING_SIGNATURE_ALGORITHM,
            public_key_hex=challenge.validator_public_key_hex,
            challenge_digest=digest,
            signature_hex=memo.validator_signature,
        )
        wallet_envelope = SignatureEnvelope(
            role="wallet",
            algorithm=BINDING_SIGNATURE_ALGORITHM,
            public_key_hex=challenge.wallet_public_key_hex,
            challenge_digest=digest,
            signature_hex=memo.wallet_signature,
        )
        verify_signature_envelope(
            challenge,
            validator_envelope,
            expected_role="validator",
        )
        verify_signature_envelope(
            challenge,
            wallet_envelope,
            expected_role="wallet",
        )
    else:
        role = "validator" if memo.validator_signature is not None else "wallet"
        signature_hex = (
            memo.validator_signature
            if role == "validator"
            else memo.wallet_signature
        )
        if signature_hex is None:
            raise TaskNodeUnlError("binding_memo_signature_count")
        envelope = SignatureEnvelope(
            role=role,
            algorithm=BINDING_SIGNATURE_ALGORITHM,
            public_key_hex=_expected_public_key(challenge, role),
            challenge_digest=digest,
            signature_hex=signature_hex,
        )
        verify_signature_envelope(challenge, envelope, expected_role=role)

    return VerifiedBindingEvent(
        action=challenge.action,
        validator_id=challenge.validator_id,
        validator_public_key_hex=challenge.validator_public_key_hex,
        wallet_address=challenge.wallet_address,
        wallet_public_key_hex=challenge.wallet_public_key_hex,
        challenge_digest=digest,
        validator_signature=memo.validator_signature,
        wallet_signature=memo.wallet_signature,
        tx_hash=tx_hash,
        ledger_index=ledger_index,
        transaction_index=transaction_index,
        close_time=close_time,
        binding_tx_hash=challenge.binding_tx_hash,
        previous_wallet_address=challenge.previous_wallet_address,
    )


def binding_evidence_fields(event: VerifiedBindingEvent) -> dict[str, str]:
    """Project a verified bind event into the five proposal evidence fields."""

    if event.action != _BIND_ACTION:
        raise TaskNodeUnlError("binding_evidence_requires_bind")
    if (
        event.validator_signature is None
        or event.wallet_signature is None
    ):
        raise TaskNodeUnlError("binding_memo_signature_count")
    return {
        TASKNODE_BINDING_EVIDENCE_FIELDS[0]: event.wallet_address,
        TASKNODE_BINDING_EVIDENCE_FIELDS[1]: event.tx_hash,
        TASKNODE_BINDING_EVIDENCE_FIELDS[2]: event.challenge_digest,
        TASKNODE_BINDING_EVIDENCE_FIELDS[3]: event.validator_signature,
        TASKNODE_BINDING_EVIDENCE_FIELDS[4]: event.wallet_signature,
    }


def verified_record_document(event: VerifiedBindingEvent) -> dict[str, Any]:
    result: dict[str, Any] = {
        "schema": BINDING_VERIFICATION_RESULT_SCHEMA,
        "mode": SHADOW_MODE,
        "verified": True,
        "action": event.action,
        "validator_id": event.validator_id,
        "wallet_address": event.wallet_address,
        "tx_hash": event.tx_hash,
        "ledger_index": event.ledger_index,
        "transaction_index": event.transaction_index,
    }
    if event.action == _BIND_ACTION:
        result["evidence_fields"] = binding_evidence_fields(event)
    else:
        result["revoked_binding_tx_hash"] = event.binding_tx_hash
        result["revoke_role"] = event.revoke_role
    return result


@dataclass(frozen=True)
class ValidatorKeyRotation:
    validator_id: str
    previous_public_key_hex: str
    new_public_key_hex: str
    ledger_index: int
    transaction_index: int
    rotated_at: datetime

    @property
    def position(self) -> tuple[int, int]:
        return self.ledger_index, self.transaction_index


@dataclass(frozen=True)
class ReattachmentEvidence:
    binding_tx_hash: str
    frozen_wallet_address: str
    cowork_accounts: tuple[str, ...]
    valid_vouch_accounts: tuple[str, ...]

    def qualifying_vouchers(self) -> tuple[str, ...]:
        return tuple(
            sorted(set(self.cowork_accounts) & set(self.valid_vouch_accounts))
        )


@dataclass(frozen=True)
class ReplayDecision:
    tx_hash: str
    action: str
    outcome: str
    reason: str

    def to_dict(self) -> dict[str, str]:
        return {
            "tx_hash": self.tx_hash,
            "action": self.action,
            "outcome": self.outcome,
            "reason": self.reason,
        }


@dataclass(frozen=True)
class ActiveBinding:
    validator_id: str
    validator_public_key_hex: str
    wallet_address: str
    wallet_public_key_hex: str
    tx_hash: str
    ledger_index: int
    transaction_index: int
    close_time: datetime
    evidence_fields: tuple[tuple[str, str], ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "validator_id": self.validator_id,
            "validator_public_key_hex": self.validator_public_key_hex,
            "wallet_address": self.wallet_address,
            "wallet_public_key_hex": self.wallet_public_key_hex,
            "tx_hash": self.tx_hash,
            "ledger_index": self.ledger_index,
            "transaction_index": self.transaction_index,
            "close_time": format_utc_timestamp(self.close_time),
            "evidence_fields": dict(self.evidence_fields),
        }


@dataclass(frozen=True)
class BindingReplayResult:
    status: str
    hold_reasons: tuple[str, ...]
    active_bindings: tuple[ActiveBinding, ...]
    shared_control_evidence: tuple[tuple[str, tuple[str, ...]], ...]
    frozen_work_history: tuple[tuple[str, int], ...]
    pending_rotation_rebind: tuple[str, ...]
    decisions: tuple[ReplayDecision, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": BINDING_REPLAY_RESULT_SCHEMA,
            "mode": SHADOW_MODE,
            "status": self.status,
            "hold_reasons": list(self.hold_reasons),
            "active_bindings": [
                binding.to_dict() for binding in self.active_bindings
            ],
            "shared_control_evidence": [
                {
                    "wallet_address": wallet,
                    "validator_ids": list(validators),
                }
                for wallet, validators in self.shared_control_evidence
            ],
            "frozen_work_history": [
                {
                    "wallet_address": wallet,
                    "frozen_at_ledger_index": ledger_index,
                }
                for wallet, ledger_index in self.frozen_work_history
            ],
            "pending_rotation_rebind": list(self.pending_rotation_rebind),
            "decisions": [decision.to_dict() for decision in self.decisions],
        }

    def canonical_bytes(self) -> bytes:
        return canonical_json_bytes(self.to_dict())


def _validated_rotations(
    rotations: Sequence[ValidatorKeyRotation],
    evaluation_end: datetime,
) -> tuple[ValidatorKeyRotation, ...]:
    ordered = sorted(
        rotations,
        key=lambda row: (
            row.ledger_index,
            row.transaction_index,
            row.validator_id,
        ),
    )
    seen_positions: set[tuple[int, int]] = set()
    current_keys: dict[str, str] = {}
    for rotation in ordered:
        _require_bounded_identifier(
            rotation.validator_id,
            "rotation.validator_id",
            maximum_bytes=_MAX_VALIDATOR_ID_BYTES,
        )
        previous = _public_key(
            rotation.previous_public_key_hex,
            "rotation.previous_public_key_hex",
        ).to_compressed_bytes().hex()
        new = _public_key(
            rotation.new_public_key_hex,
            "rotation.new_public_key_hex",
        ).to_compressed_bytes().hex()
        if previous == new:
            raise TaskNodeUnlError("rotation_key_unchanged", rotation.validator_id)
        position = _require_position(
            rotation.ledger_index,
            rotation.transaction_index,
        )
        if position in seen_positions:
            raise TaskNodeUnlError("duplicate_ledger_position")
        seen_positions.add(position)
        rotated_at = _require_aware_timestamp(
            rotation.rotated_at,
            "rotation.rotated_at",
        )
        if rotated_at > evaluation_end:
            raise TaskNodeUnlError("future_rotation", rotation.validator_id)
        expected_previous = current_keys.get(rotation.validator_id)
        if expected_previous is not None and expected_previous != previous:
            raise TaskNodeUnlError(
                "rotation_key_chain_mismatch",
                rotation.validator_id,
            )
        current_keys[rotation.validator_id] = new
    return tuple(ordered)


def _validated_identifiers(
    values: Sequence[str],
    field: str,
) -> tuple[str, ...]:
    checked = tuple(
        _require_bounded_identifier(value, field) for value in values
    )
    if len(set(checked)) != len(checked):
        raise TaskNodeUnlError("duplicate_identifier", field)
    return tuple(sorted(checked))


def _validated_reattachments(
    rows: Sequence[ReattachmentEvidence],
) -> dict[str, ReattachmentEvidence]:
    result: dict[str, ReattachmentEvidence] = {}
    for row in rows:
        tx_hash = _require_lower_hex(
            row.binding_tx_hash,
            "reattachment.binding_tx_hash",
            byte_length=_SHA256_BYTES,
        )
        if tx_hash in result:
            raise TaskNodeUnlError("duplicate_reattachment_evidence", tx_hash)
        wallet = _require_bounded_identifier(
            row.frozen_wallet_address,
            "reattachment.frozen_wallet_address",
            maximum_bytes=_MAX_WALLET_ADDRESS_BYTES,
        )
        result[tx_hash] = ReattachmentEvidence(
            binding_tx_hash=tx_hash,
            frozen_wallet_address=wallet,
            cowork_accounts=_validated_identifiers(
                row.cowork_accounts,
                "reattachment.cowork_accounts",
            ),
            valid_vouch_accounts=_validated_identifiers(
                row.valid_vouch_accounts,
                "reattachment.valid_vouch_accounts",
            ),
        )
    return result


def _expected_validator_key(
    validator_id: str,
    position: tuple[int, int],
    rotations: Sequence[ValidatorKeyRotation],
) -> str | None:
    relevant = [
        rotation
        for rotation in rotations
        if rotation.validator_id == validator_id
    ]
    if not relevant:
        return None
    current = relevant[0].previous_public_key_hex
    for rotation in relevant:
        if rotation.position <= position:
            current = rotation.new_public_key_hex
        else:
            break
    return current


def replay_bindings(
    records: Sequence[BindingLedgerRecord],
    rotations: Sequence[ValidatorKeyRotation],
    reattachments: Sequence[ReattachmentEvidence],
    *,
    evaluation_end: datetime,
) -> BindingReplayResult:
    """Replay verified memo records and enforce all step-three binding rules."""

    evaluation_end = _require_aware_timestamp(
        evaluation_end,
        "evaluation_end",
    )
    if len(records) > _MAX_RECORDS:
        raise TaskNodeUnlError("too_many_binding_records")
    if len(rotations) > _MAX_ROTATIONS:
        raise TaskNodeUnlError("too_many_rotations")
    if len(reattachments) > _MAX_REATTACHMENTS:
        raise TaskNodeUnlError("too_many_reattachments")

    validated_rotations = _validated_rotations(rotations, evaluation_end)
    verified = [verify_binding_record(record) for record in records]
    verified.sort(key=lambda event: (*event.position, event.tx_hash))
    seen_hashes: set[str] = set()
    seen_positions = {rotation.position for rotation in validated_rotations}
    for event in verified:
        if event.tx_hash in seen_hashes:
            raise TaskNodeUnlError("duplicate_binding_tx_hash", event.tx_hash)
        seen_hashes.add(event.tx_hash)
        if event.position in seen_positions:
            raise TaskNodeUnlError("duplicate_ledger_position")
        seen_positions.add(event.position)
        if event.close_time > evaluation_end:
            raise TaskNodeUnlError("future_binding_record", event.tx_hash)

    timeline = sorted(
        [
            (event.position, event.close_time)
            for event in verified
        ]
        + [
            (rotation.position, rotation.rotated_at)
            for rotation in validated_rotations
        ]
    )
    for previous, current in zip(timeline, timeline[1:]):
        if current[1] < previous[1]:
            raise TaskNodeUnlError("ledger_time_order_mismatch")

    reattachment_by_tx = _validated_reattachments(reattachments)
    record_hashes = {event.tx_hash for event in verified}
    orphaned = sorted(set(reattachment_by_tx) - record_hashes)
    if orphaned:
        raise TaskNodeUnlError(
            "orphan_reattachment_evidence",
            orphaned[0],
        )

    active_by_wallet: dict[str, VerifiedBindingEvent] = {}
    active_by_validator: dict[str, VerifiedBindingEvent] = {}
    wallet_validators: dict[str, set[str]] = {}
    frozen_by_validator: dict[str, tuple[str, int]] = {}
    frozen_by_wallet: dict[str, int] = {}
    reattached_validators: set[str] = set()
    bind_history: list[VerifiedBindingEvent] = []
    decisions: list[ReplayDecision] = []
    hold_reasons: set[str] = set()

    for event in verified:
        if event.action == _BIND_ACTION:
            expected_key = _expected_validator_key(
                event.validator_id,
                event.position,
                validated_rotations,
            )
            if (
                expected_key is not None
                and event.validator_public_key_hex != expected_key
            ):
                reason = f"validator_key_mismatch:{event.validator_id}"
                hold_reasons.add(reason)
                decisions.append(
                    ReplayDecision(
                        event.tx_hash,
                        event.action,
                        "hold",
                        reason,
                    )
                )
                continue

            seen_for_wallet = wallet_validators.setdefault(
                event.wallet_address,
                set(),
            )
            seen_for_wallet.add(event.validator_id)
            if len(seen_for_wallet) > 1:
                decisions.append(
                    ReplayDecision(
                        event.tx_hash,
                        event.action,
                        "shared_control",
                        "wallet_second_validator_attempt",
                    )
                )
                continue

            frozen = frozen_by_validator.get(event.validator_id)
            if frozen is not None and event.validator_id not in reattached_validators:
                frozen_wallet, _frozen_index = frozen
                evidence = reattachment_by_tx.get(event.tx_hash)
                if event.wallet_address == frozen_wallet:
                    reason = f"frozen_wallet_reuse:{event.validator_id}"
                    hold_reasons.add(reason)
                    decisions.append(
                        ReplayDecision(
                            event.tx_hash,
                            event.action,
                            "hold",
                            reason,
                        )
                    )
                    continue
                if (
                    event.previous_wallet_address != frozen_wallet
                    or evidence is None
                    or evidence.frozen_wallet_address != frozen_wallet
                    or len(evidence.qualifying_vouchers()) < 2
                ):
                    reason = (
                        f"reattachment_vouches_missing:{event.validator_id}"
                    )
                    hold_reasons.add(reason)
                    decisions.append(
                        ReplayDecision(
                            event.tx_hash,
                            event.action,
                            "hold",
                            reason,
                        )
                    )
                    continue
                reattached_validators.add(event.validator_id)
            elif event.previous_wallet_address is not None:
                reason = f"unexpected_reattachment_claim:{event.validator_id}"
                hold_reasons.add(reason)
                decisions.append(
                    ReplayDecision(
                        event.tx_hash,
                        event.action,
                        "hold",
                        reason,
                    )
                )
                continue

            wallet_active = active_by_wallet.get(event.wallet_address)
            if (
                wallet_active is not None
                and wallet_active.validator_id != event.validator_id
            ):
                raise TaskNodeUnlError(
                    "one_wallet_multiple_active_validators",
                    event.wallet_address,
                )
            validator_active = active_by_validator.get(event.validator_id)
            if (
                validator_active is not None
                and validator_active.wallet_address != event.wallet_address
            ):
                reason = f"validator_multiple_wallets:{event.validator_id}"
                hold_reasons.add(reason)
                decisions.append(
                    ReplayDecision(
                        event.tx_hash,
                        event.action,
                        "hold",
                        reason,
                    )
                )
                continue

            if wallet_active is not None:
                active_by_validator.pop(wallet_active.validator_id, None)
            active_by_wallet[event.wallet_address] = event
            active_by_validator[event.validator_id] = event
            bind_history.append(event)
            decisions.append(
                ReplayDecision(
                    event.tx_hash,
                    event.action,
                    "accepted",
                    (
                        "superseded_prior_binding"
                        if wallet_active is not None
                        else "binding_activated"
                    ),
                )
            )
            continue

        target = active_by_wallet.get(event.wallet_address)
        if (
            target is None
            or target.validator_id != event.validator_id
            or target.tx_hash != event.binding_tx_hash
            or target.validator_public_key_hex
            != event.validator_public_key_hex
            or target.wallet_public_key_hex != event.wallet_public_key_hex
        ):
            raise TaskNodeUnlError(
                "revoke_target_not_active",
                event.tx_hash,
            )
        active_by_wallet.pop(event.wallet_address)
        active_by_validator.pop(event.validator_id)
        if event.revoke_role == "validator":
            frozen_by_validator[event.validator_id] = (
                event.wallet_address,
                event.ledger_index,
            )
            frozen_by_wallet[event.wallet_address] = event.ledger_index
            reattached_validators.discard(event.validator_id)
        decisions.append(
            ReplayDecision(
                event.tx_hash,
                event.action,
                "accepted",
                f"revoked_by_{event.revoke_role}",
            )
        )

    pending_rotations: set[str] = set()
    for rotation in validated_rotations:
        deadline = rotation.rotated_at + timedelta(
            days=BINDING_EVALUATION_WINDOW_DAYS
        )
        timely = any(
            event.validator_id == rotation.validator_id
            and event.validator_public_key_hex == rotation.new_public_key_hex
            and event.position > rotation.position
            and rotation.rotated_at <= event.close_time <= deadline
            for event in bind_history
        )
        if timely:
            continue
        if evaluation_end >= deadline:
            hold_reasons.add(
                f"rotation_rebind_expired:{rotation.validator_id}"
            )
        else:
            pending_rotations.add(rotation.validator_id)

    shared_control = tuple(
        (
            wallet,
            tuple(sorted(validators)),
        )
        for wallet, validators in sorted(wallet_validators.items())
        if len(validators) > 1
    )
    active_bindings = tuple(
        ActiveBinding(
            validator_id=event.validator_id,
            validator_public_key_hex=event.validator_public_key_hex,
            wallet_address=event.wallet_address,
            wallet_public_key_hex=event.wallet_public_key_hex,
            tx_hash=event.tx_hash,
            ledger_index=event.ledger_index,
            transaction_index=event.transaction_index,
            close_time=event.close_time,
            evidence_fields=tuple(
                binding_evidence_fields(event).items()
            ),
        )
        for event in sorted(
            active_by_validator.values(),
            key=lambda row: row.validator_id,
        )
    )
    return BindingReplayResult(
        status="hold" if hold_reasons else "ready",
        hold_reasons=tuple(sorted(hold_reasons)),
        active_bindings=active_bindings,
        shared_control_evidence=shared_control,
        frozen_work_history=tuple(sorted(frozen_by_wallet.items())),
        pending_rotation_rebind=tuple(sorted(pending_rotations)),
        decisions=tuple(decisions),
    )


def binding_challenge_from_dict(value: object) -> BindingChallenge:
    row = require_closed_keys(
        value,
        required=(
            "schema",
            "mode",
            "signature_algorithm",
            "action",
            "validator_id",
            "validator_public_key_hex",
            "wallet_address",
            "wallet_public_key_hex",
            "nonce_hex",
            "binding_tx_hash",
            "previous_wallet_address",
        ),
        field="binding_challenge",
    )
    if row["schema"] != BINDING_CHALLENGE_SCHEMA:
        raise TaskNodeUnlError("unknown_schema", str(row["schema"]))
    if row["mode"] != SHADOW_MODE:
        raise TaskNodeUnlError("binding_not_shadow_only")
    if row["signature_algorithm"] != BINDING_SIGNATURE_ALGORITHM:
        raise TaskNodeUnlError(
            "unknown_signature_algorithm",
            str(row["signature_algorithm"]),
        )
    challenge = BindingChallenge(
        action=_require_bounded_identifier(row["action"], "action"),
        validator_id=_require_bounded_identifier(
            row["validator_id"],
            "validator_id",
            maximum_bytes=_MAX_VALIDATOR_ID_BYTES,
        ),
        validator_public_key_hex=_require_lower_hex(
            row["validator_public_key_hex"],
            "validator_public_key_hex",
            byte_length=_SECP256K1_PUBLIC_KEY_BYTES,
        ),
        wallet_address=_require_bounded_identifier(
            row["wallet_address"],
            "wallet_address",
            maximum_bytes=_MAX_WALLET_ADDRESS_BYTES,
        ),
        wallet_public_key_hex=_require_lower_hex(
            row["wallet_public_key_hex"],
            "wallet_public_key_hex",
            byte_length=_SECP256K1_PUBLIC_KEY_BYTES,
        ),
        nonce_hex=_require_lower_hex(
            row["nonce_hex"],
            "nonce_hex",
            byte_length=_SHA256_BYTES,
        ),
        binding_tx_hash=(
            None
            if row["binding_tx_hash"] is None
            else _require_lower_hex(
                row["binding_tx_hash"],
                "binding_tx_hash",
                byte_length=_SHA256_BYTES,
            )
        ),
        previous_wallet_address=(
            None
            if row["previous_wallet_address"] is None
            else _require_bounded_identifier(
                row["previous_wallet_address"],
                "previous_wallet_address",
                maximum_bytes=_MAX_WALLET_ADDRESS_BYTES,
            )
        ),
    )
    challenge.validate()
    return challenge


def signature_envelope_from_dict(value: object) -> SignatureEnvelope:
    row = require_closed_keys(
        value,
        required=(
            "schema",
            "mode",
            "role",
            "algorithm",
            "public_key_hex",
            "challenge_digest",
            "signature_hex",
        ),
        field="binding_signature",
    )
    if row["schema"] != BINDING_SIGNATURE_SCHEMA:
        raise TaskNodeUnlError("unknown_schema", str(row["schema"]))
    if row["mode"] != SHADOW_MODE:
        raise TaskNodeUnlError("binding_not_shadow_only")
    return SignatureEnvelope(
        role=_require_bounded_identifier(row["role"], "role"),
        algorithm=_require_bounded_identifier(
            row["algorithm"],
            "algorithm",
        ),
        public_key_hex=_require_lower_hex(
            row["public_key_hex"],
            "public_key_hex",
            byte_length=_SECP256K1_PUBLIC_KEY_BYTES,
        ),
        challenge_digest=_require_lower_hex(
            row["challenge_digest"],
            "challenge_digest",
            byte_length=_SHA256_BYTES,
        ),
        signature_hex=_require_lower_hex(
            row["signature_hex"],
            "signature_hex",
            byte_length=_SECP256K1_SIGNATURE_BYTES,
        ),
    )


def binding_memo_from_dict(value: object) -> BindingMemo:
    row = require_closed_keys(
        value,
        required=("s", "m", "o", "v", "w", "d", "vs", "ws"),
        field="binding_memo",
    )
    if row["s"] != BINDING_MEMO_SCHEMA:
        raise TaskNodeUnlError("unknown_schema", str(row["s"]))
    if row["m"] != SHADOW_MODE:
        raise TaskNodeUnlError("binding_not_shadow_only")

    def optional_signature(field: str) -> str | None:
        return (
            None
            if row[field] is None
            else _require_lower_hex(
                row[field],
                f"binding_memo.{field}",
                byte_length=_SECP256K1_SIGNATURE_BYTES,
            )
        )

    memo = BindingMemo(
        action=_require_bounded_identifier(row["o"], "binding_memo.o"),
        validator_id=_require_bounded_identifier(
            row["v"],
            "binding_memo.v",
            maximum_bytes=_MAX_VALIDATOR_ID_BYTES,
        ),
        wallet_address=_require_bounded_identifier(
            row["w"],
            "binding_memo.w",
            maximum_bytes=_MAX_WALLET_ADDRESS_BYTES,
        ),
        challenge_digest=_require_lower_hex(
            row["d"],
            "binding_memo.d",
            byte_length=_SHA256_BYTES,
        ),
        validator_signature=optional_signature("vs"),
        wallet_signature=optional_signature("ws"),
    )
    _validate_memo_shape(memo)
    return memo


def binding_ledger_record_from_dict(value: object) -> BindingLedgerRecord:
    row = require_closed_keys(
        value,
        required=(
            "schema",
            "tx_hash",
            "ledger_index",
            "transaction_index",
            "close_time",
            "sender_wallet_address",
            "challenge",
            "memo",
        ),
        field="binding_record",
    )
    if row["schema"] != BINDING_LEDGER_RECORD_SCHEMA:
        raise TaskNodeUnlError("unknown_schema", str(row["schema"]))
    ledger_index, transaction_index = _require_position(
        row["ledger_index"],
        row["transaction_index"],
    )
    return BindingLedgerRecord(
        tx_hash=_require_lower_hex(
            row["tx_hash"],
            "tx_hash",
            byte_length=_SHA256_BYTES,
        ),
        ledger_index=ledger_index,
        transaction_index=transaction_index,
        close_time=parse_utc_timestamp(row["close_time"], "close_time"),
        sender_wallet_address=_require_bounded_identifier(
            row["sender_wallet_address"],
            "sender_wallet_address",
            maximum_bytes=_MAX_WALLET_ADDRESS_BYTES,
        ),
        challenge=binding_challenge_from_dict(row["challenge"]),
        memo=binding_memo_from_dict(row["memo"]),
    )


def _rotation_from_dict(value: object, index: int) -> ValidatorKeyRotation:
    field = f"rotations[{index}]"
    row = require_closed_keys(
        value,
        required=(
            "validator_id",
            "previous_public_key_hex",
            "new_public_key_hex",
            "ledger_index",
            "transaction_index",
            "rotated_at",
        ),
        field=field,
    )
    ledger_index, transaction_index = _require_position(
        row["ledger_index"],
        row["transaction_index"],
    )
    return ValidatorKeyRotation(
        validator_id=_require_bounded_identifier(
            row["validator_id"],
            f"{field}.validator_id",
            maximum_bytes=_MAX_VALIDATOR_ID_BYTES,
        ),
        previous_public_key_hex=_require_lower_hex(
            row["previous_public_key_hex"],
            f"{field}.previous_public_key_hex",
            byte_length=_SECP256K1_PUBLIC_KEY_BYTES,
        ),
        new_public_key_hex=_require_lower_hex(
            row["new_public_key_hex"],
            f"{field}.new_public_key_hex",
            byte_length=_SECP256K1_PUBLIC_KEY_BYTES,
        ),
        ledger_index=ledger_index,
        transaction_index=transaction_index,
        rotated_at=parse_utc_timestamp(
            row["rotated_at"],
            f"{field}.rotated_at",
        ),
    )


def _string_array(value: object, field: str) -> tuple[str, ...]:
    if not isinstance(value, list):
        raise TaskNodeUnlError("invalid_array", field)
    if len(value) > _MAX_RECORDS:
        raise TaskNodeUnlError("array_too_large", field)
    return tuple(
        _require_bounded_identifier(item, f"{field}[{index}]")
        for index, item in enumerate(value)
    )


def _reattachment_from_dict(
    value: object,
    index: int,
) -> ReattachmentEvidence:
    field = f"reattachments[{index}]"
    row = require_closed_keys(
        value,
        required=(
            "binding_tx_hash",
            "frozen_wallet_address",
            "cowork_accounts",
            "valid_vouch_accounts",
        ),
        field=field,
    )
    return ReattachmentEvidence(
        binding_tx_hash=_require_lower_hex(
            row["binding_tx_hash"],
            f"{field}.binding_tx_hash",
            byte_length=_SHA256_BYTES,
        ),
        frozen_wallet_address=_require_bounded_identifier(
            row["frozen_wallet_address"],
            f"{field}.frozen_wallet_address",
            maximum_bytes=_MAX_WALLET_ADDRESS_BYTES,
        ),
        cowork_accounts=_string_array(
            row["cowork_accounts"],
            f"{field}.cowork_accounts",
        ),
        valid_vouch_accounts=_string_array(
            row["valid_vouch_accounts"],
            f"{field}.valid_vouch_accounts",
        ),
    )


def replay_bindings_document(value: object) -> BindingReplayResult:
    row = require_closed_keys(
        value,
        required=(
            "schema",
            "mode",
            "evaluation_end",
            "records",
            "rotations",
            "reattachments",
        ),
        field="binding_replay",
    )
    if row["schema"] != BINDING_REPLAY_INPUT_SCHEMA:
        raise TaskNodeUnlError("unknown_schema", str(row["schema"]))
    if row["mode"] != SHADOW_MODE:
        raise TaskNodeUnlError("binding_not_shadow_only")
    records = row["records"]
    rotations = row["rotations"]
    reattachments = row["reattachments"]
    for value_, field, maximum in (
        (records, "records", _MAX_RECORDS),
        (rotations, "rotations", _MAX_ROTATIONS),
        (reattachments, "reattachments", _MAX_REATTACHMENTS),
    ):
        if not isinstance(value_, list):
            raise TaskNodeUnlError("invalid_array", field)
        if len(value_) > maximum:
            raise TaskNodeUnlError("array_too_large", field)
    return replay_bindings(
        tuple(binding_ledger_record_from_dict(item) for item in records),
        tuple(
            _rotation_from_dict(item, index)
            for index, item in enumerate(rotations)
        ),
        tuple(
            _reattachment_from_dict(item, index)
            for index, item in enumerate(reattachments)
        ),
        evaluation_end=parse_utc_timestamp(
            row["evaluation_end"],
            "evaluation_end",
        ),
    )


def prepare_bind_challenge(
    *,
    validator_id: str,
    validator_public_key_hex: str,
    wallet_address: str,
    wallet_public_key_hex: str,
    nonce_hex: str,
    previous_wallet_address: str | None = None,
) -> BindingChallenge:
    challenge = BindingChallenge(
        action=_BIND_ACTION,
        validator_id=validator_id,
        validator_public_key_hex=validator_public_key_hex,
        wallet_address=wallet_address,
        wallet_public_key_hex=wallet_public_key_hex,
        nonce_hex=nonce_hex,
        previous_wallet_address=previous_wallet_address,
    )
    challenge.validate()
    return challenge


def prepare_revoke_challenge(
    active_binding: VerifiedBindingEvent,
    *,
    nonce_hex: str,
) -> BindingChallenge:
    if active_binding.action != _BIND_ACTION:
        raise TaskNodeUnlError("revoke_requires_binding_event")
    challenge = BindingChallenge(
        action=_REVOKE_ACTION,
        validator_id=active_binding.validator_id,
        validator_public_key_hex=active_binding.validator_public_key_hex,
        wallet_address=active_binding.wallet_address,
        wallet_public_key_hex=active_binding.wallet_public_key_hex,
        nonce_hex=nonce_hex,
        binding_tx_hash=active_binding.tx_hash,
    )
    challenge.validate()
    return challenge
