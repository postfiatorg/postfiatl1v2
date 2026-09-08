"""Pure verifier for the Task Node UNL V2 versioned evidence contract.

The verifier consumes caller-supplied JSON-shaped values and explicit root
commitments.  It performs no file, clock, network, transaction, or credential
access.  Funding is retained only as audit evidence.  Admission relations need
the exact consent signatures specified by the locked V2 amendment.
"""

from __future__ import annotations

import hashlib
from dataclasses import dataclass
from datetime import datetime, timedelta
from typing import Any, Mapping, Protocol, Sequence

from eth_keys import keys
from eth_keys.constants import SECPK1_N

from .tasknode_unl_schema import (
    TaskNodeUnlError,
    canonical_json_bytes,
    format_utc_timestamp,
    require_closed_keys,
    require_int,
)
from .tasknode_unl_v2_schema import (
    BILATERAL_ACTIONS,
    CONTROL_DECLARATION_ACTIONS,
    CONTROL_EVENT_AUTHORITY,
    CONTROL_EVENT_KINDS,
    EVIDENCE_LIMITATIONS,
    FUNDING_OBSERVATION_KINDS,
    MAX_CANONICAL_DOCUMENT_BYTES,
    MAX_CONTROL_MEMBERS,
    MAX_IDENTIFIER_BYTES,
    MAX_RECORDS,
    MAX_WINDOW_INDEX,
    RELATION_KINDS,
    SCORE_EVIDENCE_KINDS,
    SECP256K1_PUBLIC_KEY_BYTES,
    SECP256K1_SIGNATURE_BYTES,
    SHA256_BYTES,
    V2_BILATERAL_RECORD_SCHEMA,
    V2_CONTROL_DECLARE_DOMAIN,
    V2_CONTROL_DECLARATION_SCHEMA,
    V2_CONTROL_DISSOLVE_DOMAIN,
    V2_CONTROL_EVENT_SCHEMA,
    V2_CONTROL_REGISTRY_SCHEMA,
    V2_COWORK_DOMAIN,
    V2_EVIDENCE_INPUT_SCHEMA,
    V2_EVIDENCE_RESULT_SCHEMA,
    V2_EVIDENCE_SNAPSHOT_SCHEMA,
    V2_FUNDING_OBSERVATION_SCHEMA,
    V2_INPUT_ROOT_DOMAIN,
    V2_POLICY_ID,
    V2_POLICY_ROOT_DOMAIN,
    V2_POLICY_SCHEMA,
    V2_REGISTRY_ROOT_DOMAIN,
    V2_SCORE_EVIDENCE_SCHEMA,
    V2_SIGNATURE_ALGORITHM,
    V2_SIGNATURE_SCHEMA,
    V2_VERSION,
    V2_VOUCH_DOMAIN,
    V2_WINDOW_DAYS,
    EvaluationWindow,
    RecordRejection,
    ValidationFailure,
    require_array,
    require_bounded_identifier,
    require_canonical_timestamp,
    require_document_bound,
    require_lower_hex,
    require_v2_object,
    v2_header,
)


class EvidenceSignerAdapter(Protocol):
    """Custody boundary: sign one digest without exposing private material."""

    algorithm_id: str
    public_key_hex: str

    def sign_digest(self, digest: bytes) -> bytes:
        """Return a detached recoverable signature for a 32-byte digest."""


@dataclass(frozen=True)
class ControlEpoch:
    """Current public binding epoch for one account.

    ``epoch`` is exactly ``binding_event_digest``; there is no inferred identity
    or account-sale state.
    """

    chain_id: str
    genesis_hash: str
    account_id: str
    event_kind: str
    binding_event_digest: str
    event_at: datetime
    public_key_hex: str
    authorization_source: str
    incumbent: bool

    @property
    def epoch(self) -> str:
        return self.binding_event_digest

    def to_dict(self) -> dict[str, Any]:
        return {
            **v2_header(V2_CONTROL_EVENT_SCHEMA),
            "policy_id": V2_POLICY_ID,
            "chain_id": self.chain_id,
            "genesis_hash": self.genesis_hash,
            "account_id": self.account_id,
            "event_kind": self.event_kind,
            "binding_event_digest": self.binding_event_digest,
            "event_at": format_utc_timestamp(self.event_at),
            "public_key_hex": self.public_key_hex,
            "authorization_source": self.authorization_source,
            "incumbent": self.incumbent,
        }


@dataclass(frozen=True, order=True)
class ActiveRelation:
    relation_kind: str
    source_account: str
    target_account: str
    statement_digest: str
    credit_id: str
    effective_window: int
    expiry_window: int
    record_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "relation_kind": self.relation_kind,
            "source_account": self.source_account,
            "target_account": self.target_account,
            "statement_digest": self.statement_digest,
            "credit_id": self.credit_id,
            "effective_window": self.effective_window,
            "expiry_window": self.expiry_window,
            "record_digest": self.record_digest,
        }


@dataclass(frozen=True, order=True)
class ActiveControlDeclaration:
    statement_digest: str
    members: tuple[str, ...]
    effective_window: int
    expiry_window: int
    record_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "statement_digest": self.statement_digest,
            "members": list(self.members),
            "effective_window": self.effective_window,
            "expiry_window": self.expiry_window,
            "record_digest": self.record_digest,
        }


@dataclass(frozen=True, order=True)
class ContinuityAssessment:
    account_id: str
    control_epoch: str
    status: str
    incumbent: bool
    retain_incumbent: bool
    reasons: tuple[str, ...]
    fresh_score_evidence: int
    renewed_vouches: int
    post_epoch_cowork: int

    def to_dict(self) -> dict[str, Any]:
        return {
            "account_id": self.account_id,
            "control_epoch": self.control_epoch,
            "status": self.status,
            "incumbent": self.incumbent,
            "retain_incumbent": self.retain_incumbent,
            "reasons": list(self.reasons),
            "fresh_score_evidence": self.fresh_score_evidence,
            "renewed_vouches": self.renewed_vouches,
            "post_epoch_cowork": self.post_epoch_cowork,
        }


@dataclass(frozen=True)
class EvidenceSnapshotResult:
    """Deterministic result with global holds separated from record isolation."""

    status: str
    failures: tuple[ValidationFailure, ...]
    record_rejections: tuple[RecordRejection, ...]
    hold_accounts: tuple[str, ...]
    commitments: Mapping[str, str] | None
    window: EvaluationWindow | None
    audit_funding_observations: tuple[Mapping[str, Any], ...]
    historical_score_evidence: tuple[Mapping[str, Any], ...]
    active_vouches: tuple[ActiveRelation, ...]
    active_cowork: tuple[ActiveRelation, ...]
    active_control_declarations: tuple[ActiveControlDeclaration, ...]
    continuity: tuple[ContinuityAssessment, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            **v2_header(V2_EVIDENCE_RESULT_SCHEMA),
            "policy_id": V2_POLICY_ID,
            "status": self.status,
            "failures": [item.to_dict() for item in self.failures],
            "record_rejections": [
                item.to_dict() for item in self.record_rejections
            ],
            "hold_accounts": list(self.hold_accounts),
            "commitments": (
                dict(self.commitments) if self.commitments is not None else None
            ),
            "window": self.window.to_dict() if self.window is not None else None,
            "audit_funding_observations": [
                dict(item) for item in self.audit_funding_observations
            ],
            "historical_score_evidence": [
                dict(item) for item in self.historical_score_evidence
            ],
            "active_vouches": [item.to_dict() for item in self.active_vouches],
            "active_cowork": [item.to_dict() for item in self.active_cowork],
            "active_control_declarations": [
                item.to_dict() for item in self.active_control_declarations
            ],
            "continuity": [item.to_dict() for item in self.continuity],
            "evidence_limitations": list(EVIDENCE_LIMITATIONS),
        }

    def canonical_bytes(self) -> bytes:
        return canonical_json_bytes(self.to_dict())


@dataclass(frozen=True)
class _BilateralContext:
    row: Mapping[str, Any]
    chain_id: str
    genesis_hash: str
    action: str
    relation_kind: str
    source_account: str
    target_account: str
    source_epoch: str
    target_epoch: str
    statement_digest: str
    work_completed_at: datetime | None
    acknowledged_at: datetime
    finalized_window: int
    effective_window: int
    expiry_window: int


@dataclass(frozen=True)
class _DeclarationContext:
    row: Mapping[str, Any]
    chain_id: str
    genesis_hash: str
    action: str
    statement_digest: str
    members: tuple[tuple[str, str], ...]
    acknowledged_at: datetime
    finalized_window: int
    effective_window: int
    expiry_window: int


@dataclass(frozen=True)
class _ScoreContext:
    row: Mapping[str, Any]
    account_id: str
    control_epoch: str
    evidence_kind: str
    evidence_digest: str
    evidence_at: datetime


def _failure(error: TaskNodeUnlError, fallback: str) -> ValidationFailure:
    return ValidationFailure(
        field=error.detail or fallback,
        code=error.code,
        detail="",
    )


def _hold(
    failures: Sequence[ValidationFailure],
) -> EvidenceSnapshotResult:
    return EvidenceSnapshotResult(
        status="hold",
        failures=tuple(sorted(set(failures))),
        record_rejections=(),
        hold_accounts=(),
        commitments=None,
        window=None,
        audit_funding_observations=(),
        historical_score_evidence=(),
        active_vouches=(),
        active_cowork=(),
        active_control_declarations=(),
        continuity=(),
    )


def _domain_hash(domain: str, value: object) -> str:
    require_document_bound(value, "canonical_document")
    return hashlib.sha256(
        domain.encode("ascii") + b"\x00" + canonical_json_bytes(value)
    ).hexdigest()


def evidence_contract_policy() -> dict[str, Any]:
    """Return the frozen, root-committed section-A policy document."""

    return {
        **v2_header(V2_POLICY_SCHEMA),
        "policy_id": V2_POLICY_ID,
        "window_days": V2_WINDOW_DAYS,
        "signature_algorithm": V2_SIGNATURE_ALGORITHM,
        "signature_canonicalization": "recoverable-low-s",
        "control_event_kinds": list(CONTROL_EVENT_KINDS),
        "control_event_authority": {
            key: CONTROL_EVENT_AUTHORITY[key]
            for key in sorted(CONTROL_EVENT_AUTHORITY)
        },
        "relation_kinds": list(RELATION_KINDS),
        "funding_observation_kinds": list(FUNDING_OBSERVATION_KINDS),
        "score_evidence_kinds": list(SCORE_EVIDENCE_KINDS),
        "bilateral_actions": list(BILATERAL_ACTIONS),
        "control_declaration_actions": list(CONTROL_DECLARATION_ACTIONS),
        "record_schemas": {
            "control_event": V2_CONTROL_EVENT_SCHEMA,
            "score_evidence": V2_SCORE_EVIDENCE_SCHEMA,
            "funding_observation": V2_FUNDING_OBSERVATION_SCHEMA,
            "bilateral_record": V2_BILATERAL_RECORD_SCHEMA,
            "control_declaration": V2_CONTROL_DECLARATION_SCHEMA,
            "signature": V2_SIGNATURE_SCHEMA,
        },
        "domains": {
            "policy_root": V2_POLICY_ROOT_DOMAIN,
            "input_root": V2_INPUT_ROOT_DOMAIN,
            "registry_root": V2_REGISTRY_ROOT_DOMAIN,
            "vouch": V2_VOUCH_DOMAIN,
            "cowork": V2_COWORK_DOMAIN,
            "control_declaration": V2_CONTROL_DECLARE_DOMAIN,
            "control_dissolution": V2_CONTROL_DISSOLVE_DOMAIN,
        },
        "bounds": {
            "sha256_bytes": SHA256_BYTES,
            "secp256k1_public_key_bytes": SECP256K1_PUBLIC_KEY_BYTES,
            "secp256k1_signature_bytes": SECP256K1_SIGNATURE_BYTES,
            "max_identifier_bytes": MAX_IDENTIFIER_BYTES,
            "max_records_per_class": MAX_RECORDS,
            "max_control_members": MAX_CONTROL_MEMBERS,
            "max_window_index": MAX_WINDOW_INDEX,
            "max_canonical_document_bytes": MAX_CANONICAL_DOCUMENT_BYTES,
        },
        "boundary_rules": {
            "relation_update_delay_windows": 1,
            "relation_revocation_delay_windows": 1,
            "control_dissolution_delay_windows": 2,
        },
        "acknowledgement_bindings": [
            "chain_id",
            "genesis_hash",
            "policy_id",
            "relation_kind_or_members",
            "statement_digest",
            "account_ids",
            "control_epochs",
            "ledger_finality_digest",
            "effective_window",
            "expiry_window",
        ],
        "evidence_limitations": list(EVIDENCE_LIMITATIONS),
    }


def policy_root() -> str:
    return _domain_hash(V2_POLICY_ROOT_DOMAIN, evidence_contract_policy())


def evidence_input_root(evidence: object) -> str:
    return _domain_hash(V2_INPUT_ROOT_DOMAIN, evidence)


def control_registry_root(registry: object) -> str:
    return _domain_hash(V2_REGISTRY_ROOT_DOMAIN, registry)


def seal_evidence_snapshot(
    evidence: object,
    control_registry: object,
) -> dict[str, Any]:
    """Bind policy, input, and registry roots into one closed snapshot."""

    return {
        **v2_header(V2_EVIDENCE_SNAPSHOT_SCHEMA),
        "commitments": {
            "policy_root": policy_root(),
            "input_root": evidence_input_root(evidence),
            "registry_root": control_registry_root(control_registry),
        },
        "evidence": evidence,
    }


def _public_key(public_key_hex: object, field: str) -> keys.PublicKey:
    checked = require_lower_hex(
        public_key_hex,
        field,
        byte_length=SECP256K1_PUBLIC_KEY_BYTES,
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


def _parse_control_event(value: object, field: str) -> ControlEpoch:
    row = require_v2_object(
        value,
        schema=V2_CONTROL_EVENT_SCHEMA,
        required=(
            "policy_id",
            "chain_id",
            "genesis_hash",
            "account_id",
            "event_kind",
            "binding_event_digest",
            "event_at",
            "public_key_hex",
            "authorization_source",
            "incumbent",
        ),
        field=field,
    )
    if row["policy_id"] != V2_POLICY_ID:
        raise TaskNodeUnlError("policy_id_mismatch", f"{field}.policy_id")
    chain_id = require_bounded_identifier(row["chain_id"], f"{field}.chain_id")
    genesis_hash = require_lower_hex(
        row["genesis_hash"], f"{field}.genesis_hash"
    )
    account = require_bounded_identifier(row["account_id"], f"{field}.account_id")
    kind = row["event_kind"]
    if kind not in CONTROL_EVENT_KINDS:
        raise TaskNodeUnlError("unauthorized_control_event_kind", f"{field}.event_kind")
    source = row["authorization_source"]
    if source != CONTROL_EVENT_AUTHORITY[kind]:
        raise TaskNodeUnlError(
            "control_event_authority_mismatch",
            f"{field}.authorization_source",
        )
    if not isinstance(row["incumbent"], bool):
        raise TaskNodeUnlError("invalid_boolean", f"{field}.incumbent")
    _public_key(row["public_key_hex"], f"{field}.public_key_hex")
    return ControlEpoch(
        chain_id=chain_id,
        genesis_hash=genesis_hash,
        account_id=account,
        event_kind=kind,
        binding_event_digest=require_lower_hex(
            row["binding_event_digest"], f"{field}.binding_event_digest"
        ),
        event_at=require_canonical_timestamp(row["event_at"], f"{field}.event_at"),
        public_key_hex=str(row["public_key_hex"]),
        authorization_source=str(source),
        incumbent=row["incumbent"],
    )


def control_epoch_from_event(
    value: object,
    authorized_event_accounts: Mapping[str, str],
) -> ControlEpoch:
    """Accept an epoch reset only when the binding/recovery digest owns it.

    The authority map is supplied by the existing binding/recovery verifier.
    Accusations, routine signatures, and unsolicited transfers are not valid
    event kinds and cannot enter this function as accepted epochs.
    """

    epoch = _parse_control_event(value, "control_event")
    owner = authorized_event_accounts.get(epoch.binding_event_digest)
    if owner != epoch.account_id:
        raise TaskNodeUnlError(
            "control_event_owner_mismatch", "control_event.account_id"
        )
    return epoch


def _parse_registry(
    value: object,
) -> tuple[str, str, tuple[ControlEpoch, ...]]:
    row = require_v2_object(
        value,
        schema=V2_CONTROL_REGISTRY_SCHEMA,
        required=("policy_id", "chain_id", "genesis_hash", "entries"),
        field="control_registry",
    )
    if row["policy_id"] != V2_POLICY_ID:
        raise TaskNodeUnlError(
            "policy_id_mismatch", "control_registry.policy_id"
        )
    chain_id = require_bounded_identifier(
        row["chain_id"], "control_registry.chain_id"
    )
    genesis_hash = require_lower_hex(
        row["genesis_hash"], "control_registry.genesis_hash"
    )
    entries = require_array(row["entries"], "control_registry.entries")
    parsed = tuple(
        _parse_control_event(item, f"control_registry.entries[{index}]")
        for index, item in enumerate(entries)
    )
    accounts: dict[str, ControlEpoch] = {}
    digests: dict[str, str] = {}
    for entry in parsed:
        if entry.chain_id != chain_id:
            raise TaskNodeUnlError(
                "chain_id_mismatch", "control_registry.entries"
            )
        if entry.genesis_hash != genesis_hash:
            raise TaskNodeUnlError(
                "genesis_hash_mismatch", "control_registry.entries"
            )
        if entry.account_id in accounts:
            raise TaskNodeUnlError(
                "duplicate_registry_account", "control_registry.entries"
            )
        prior = digests.get(entry.epoch)
        if prior is not None and prior != entry.account_id:
            raise TaskNodeUnlError(
                "control_epoch_account_conflict", "control_registry.entries"
            )
        accounts[entry.account_id] = entry
        digests[entry.epoch] = entry.account_id
    return chain_id, genesis_hash, tuple(
        accounts[key] for key in sorted(accounts)
    )


def _bounded_window(value: object, field: str) -> int:
    result = require_int(value, field, minimum=0)
    if result > MAX_WINDOW_INDEX:
        raise TaskNodeUnlError("window_index_out_of_range", field)
    return result


def _parse_score(value: object, index: int) -> _ScoreContext:
    field = f"evidence.score_evidence[{index}]"
    row = require_v2_object(
        value,
        schema=V2_SCORE_EVIDENCE_SCHEMA,
        required=(
            "account_id",
            "control_epoch",
            "evidence_kind",
            "evidence_digest",
            "evidence_at",
        ),
        field=field,
    )
    if row["evidence_kind"] not in SCORE_EVIDENCE_KINDS:
        raise TaskNodeUnlError(
            "unknown_score_evidence_kind", f"{field}.evidence_kind"
        )
    return _ScoreContext(
        row=row,
        account_id=require_bounded_identifier(row["account_id"], f"{field}.account_id"),
        control_epoch=require_lower_hex(row["control_epoch"], f"{field}.control_epoch"),
        evidence_kind=str(row["evidence_kind"]),
        evidence_digest=require_lower_hex(
            row["evidence_digest"], f"{field}.evidence_digest"
        ),
        evidence_at=require_canonical_timestamp(
            row["evidence_at"], f"{field}.evidence_at"
        ),
    )


def _parse_funding(value: object, index: int) -> Mapping[str, Any]:
    field = f"evidence.funding_observations[{index}]"
    row = require_v2_object(
        value,
        schema=V2_FUNDING_OBSERVATION_SCHEMA,
        required=(
            "observation_digest",
            "observation_kind",
            "source_account",
            "target_account",
            "observed_at",
            "value_units",
            "source_record_digests",
        ),
        field=field,
    )
    require_lower_hex(row["observation_digest"], f"{field}.observation_digest")
    if row["observation_kind"] not in FUNDING_OBSERVATION_KINDS:
        raise TaskNodeUnlError(
            "unknown_funding_observation_kind",
            f"{field}.observation_kind",
        )
    source = require_bounded_identifier(
        row["source_account"], f"{field}.source_account"
    )
    target = require_bounded_identifier(
        row["target_account"], f"{field}.target_account"
    )
    if source == target:
        raise TaskNodeUnlError("self_funding_observation", f"{field}.target_account")
    require_canonical_timestamp(row["observed_at"], f"{field}.observed_at")
    require_int(row["value_units"], f"{field}.value_units", minimum=0)
    source_digests = require_array(
        row["source_record_digests"],
        f"{field}.source_record_digests",
        maximum=128,
    )
    checked_digests = [
        require_lower_hex(
            digest, f"{field}.source_record_digests[{index}]"
        )
        for index, digest in enumerate(source_digests)
    ]
    if not checked_digests:
        raise TaskNodeUnlError(
            "funding_provenance_missing", f"{field}.source_record_digests"
        )
    if checked_digests != sorted(set(checked_digests)):
        raise TaskNodeUnlError(
            "non_canonical_funding_provenance",
            f"{field}.source_record_digests",
        )
    return row


def _parse_bilateral_unsigned(value: object, field: str) -> _BilateralContext:
    row = require_v2_object(
        value,
        schema=V2_BILATERAL_RECORD_SCHEMA,
        required=(
            "policy_id",
            "chain_id",
            "genesis_hash",
            "action",
            "relation_kind",
            "source_account",
            "target_account",
            "source_epoch",
            "target_epoch",
            "statement_digest",
            "work_completed_at",
            "acknowledged_at",
            "ledger_finality_digest",
            "finalized_window",
            "effective_window",
            "expiry_window",
            "signatures",
        ),
        field=field,
    )
    if row["policy_id"] != V2_POLICY_ID:
        raise TaskNodeUnlError("policy_id_mismatch", f"{field}.policy_id")
    chain_id = require_bounded_identifier(
        row["chain_id"], f"{field}.chain_id"
    )
    genesis_hash = require_lower_hex(
        row["genesis_hash"], f"{field}.genesis_hash"
    )
    require_lower_hex(
        row["ledger_finality_digest"],
        f"{field}.ledger_finality_digest",
    )
    action = row["action"]
    if action not in BILATERAL_ACTIONS:
        raise TaskNodeUnlError("unknown_bilateral_action", f"{field}.action")
    kind = row["relation_kind"]
    if kind not in RELATION_KINDS:
        raise TaskNodeUnlError("unknown_relation_kind", f"{field}.relation_kind")
    source = require_bounded_identifier(
        row["source_account"], f"{field}.source_account"
    )
    target = require_bounded_identifier(
        row["target_account"], f"{field}.target_account"
    )
    if source == target:
        raise TaskNodeUnlError("self_relation", f"{field}.target_account")
    work_completed_at = (
        None
        if row["work_completed_at"] is None
        else require_canonical_timestamp(
            row["work_completed_at"], f"{field}.work_completed_at"
        )
    )
    if kind == "cowork" and work_completed_at is None:
        raise TaskNodeUnlError(
            "cowork_completion_missing", f"{field}.work_completed_at"
        )
    if kind == "vouch" and work_completed_at is not None:
        raise TaskNodeUnlError(
            "vouch_has_work_completion", f"{field}.work_completed_at"
        )
    finalized = _bounded_window(row["finalized_window"], f"{field}.finalized_window")
    effective = _bounded_window(row["effective_window"], f"{field}.effective_window")
    expiry = _bounded_window(row["expiry_window"], f"{field}.expiry_window")
    if effective != finalized + 1:
        raise TaskNodeUnlError(
            "boundary_update_mismatch", f"{field}.effective_window"
        )
    if expiry < effective:
        raise TaskNodeUnlError("expiry_before_effective", f"{field}.expiry_window")
    require_array(row["signatures"], f"{field}.signatures", maximum=2)
    return _BilateralContext(
        row=row,
        chain_id=chain_id,
        genesis_hash=genesis_hash,
        action=str(action),
        relation_kind=str(kind),
        source_account=source,
        target_account=target,
        source_epoch=require_lower_hex(row["source_epoch"], f"{field}.source_epoch"),
        target_epoch=require_lower_hex(row["target_epoch"], f"{field}.target_epoch"),
        statement_digest=require_lower_hex(
            row["statement_digest"], f"{field}.statement_digest"
        ),
        work_completed_at=work_completed_at,
        acknowledged_at=require_canonical_timestamp(
            row["acknowledged_at"], f"{field}.acknowledged_at"
        ),
        finalized_window=finalized,
        effective_window=effective,
        expiry_window=expiry,
    )


def _member_rows(
    value: object,
    field: str,
) -> tuple[tuple[str, str], ...]:
    rows = require_array(value, field, maximum=MAX_CONTROL_MEMBERS)
    if len(rows) < 2:
        raise TaskNodeUnlError("control_group_too_small", field)
    members: list[tuple[str, str]] = []
    for index, item in enumerate(rows):
        member_field = f"{field}[{index}]"
        row = require_closed_keys(
            item,
            required=("account_id", "control_epoch"),
            field=member_field,
        )
        members.append(
            (
                require_bounded_identifier(
                    row["account_id"], f"{member_field}.account_id"
                ),
                require_lower_hex(
                    row["control_epoch"], f"{member_field}.control_epoch"
                ),
            )
        )
    if members != sorted(members):
        raise TaskNodeUnlError("non_canonical_member_order", field)
    if len({account for account, _epoch in members}) != len(members):
        raise TaskNodeUnlError("duplicate_control_member", field)
    return tuple(members)


def _parse_declaration_unsigned(value: object, field: str) -> _DeclarationContext:
    row = require_v2_object(
        value,
        schema=V2_CONTROL_DECLARATION_SCHEMA,
        required=(
            "policy_id",
            "chain_id",
            "genesis_hash",
            "action",
            "statement_digest",
            "members",
            "acknowledged_at",
            "ledger_finality_digest",
            "finalized_window",
            "effective_window",
            "expiry_window",
            "signatures",
        ),
        field=field,
    )
    if row["policy_id"] != V2_POLICY_ID:
        raise TaskNodeUnlError("policy_id_mismatch", f"{field}.policy_id")
    chain_id = require_bounded_identifier(
        row["chain_id"], f"{field}.chain_id"
    )
    genesis_hash = require_lower_hex(
        row["genesis_hash"], f"{field}.genesis_hash"
    )
    require_lower_hex(
        row["ledger_finality_digest"],
        f"{field}.ledger_finality_digest",
    )
    action = row["action"]
    if action not in CONTROL_DECLARATION_ACTIONS:
        raise TaskNodeUnlError(
            "unknown_control_declaration_action", f"{field}.action"
        )
    finalized = _bounded_window(row["finalized_window"], f"{field}.finalized_window")
    effective = _bounded_window(row["effective_window"], f"{field}.effective_window")
    expiry = _bounded_window(row["expiry_window"], f"{field}.expiry_window")
    expected_delay = 1 if action == "declare" else 2
    if effective != finalized + expected_delay:
        code = (
            "boundary_update_mismatch"
            if action == "declare"
            else "control_dissolution_delay_mismatch"
        )
        raise TaskNodeUnlError(code, f"{field}.effective_window")
    if expiry < effective:
        raise TaskNodeUnlError("expiry_before_effective", f"{field}.expiry_window")
    signatures = require_array(
        row["signatures"], f"{field}.signatures", maximum=MAX_CONTROL_MEMBERS
    )
    return _DeclarationContext(
        row=row,
        chain_id=chain_id,
        genesis_hash=genesis_hash,
        action=str(action),
        statement_digest=require_lower_hex(
            row["statement_digest"], f"{field}.statement_digest"
        ),
        members=_member_rows(row["members"], f"{field}.members"),
        acknowledged_at=require_canonical_timestamp(
            row["acknowledged_at"], f"{field}.acknowledged_at"
        ),
        finalized_window=finalized,
        effective_window=effective,
        expiry_window=expiry,
    )


def _unsigned_record(row: Mapping[str, Any]) -> dict[str, Any]:
    return {key: row[key] for key in sorted(row) if key != "signatures"}


def bilateral_statement_digest(value: object) -> bytes:
    context = _parse_bilateral_unsigned(value, "bilateral_record")
    domain = V2_VOUCH_DOMAIN if context.relation_kind == "vouch" else V2_COWORK_DOMAIN
    return bytes.fromhex(_domain_hash(domain, _unsigned_record(context.row)))


def control_declaration_statement_digest(value: object) -> bytes:
    context = _parse_declaration_unsigned(value, "control_declaration")
    domain = (
        V2_CONTROL_DECLARE_DOMAIN
        if context.action == "declare"
        else V2_CONTROL_DISSOLVE_DOMAIN
    )
    return bytes.fromhex(_domain_hash(domain, _unsigned_record(context.row)))


def _signature_document(
    *,
    account_id: str,
    control_epoch: str,
    signer: EvidenceSignerAdapter,
    digest: bytes,
) -> dict[str, Any]:
    if signer.algorithm_id != V2_SIGNATURE_ALGORITHM:
        raise TaskNodeUnlError("unknown_signature_algorithm", "signer.algorithm_id")
    _public_key(signer.public_key_hex, "signer.public_key_hex")
    signature = signer.sign_digest(digest)
    if not isinstance(signature, bytes):
        raise TaskNodeUnlError("signer_returned_non_bytes", "signer.sign_digest")
    require_lower_hex(
        signature.hex(),
        "signature.signature_hex",
        byte_length=SECP256K1_SIGNATURE_BYTES,
    )
    document = {
        **v2_header(V2_SIGNATURE_SCHEMA),
        "account_id": require_bounded_identifier(account_id, "signature.account_id"),
        "control_epoch": require_lower_hex(
            control_epoch, "signature.control_epoch"
        ),
        "algorithm": signer.algorithm_id,
        "public_key_hex": signer.public_key_hex,
        "signed_digest": digest.hex(),
        "signature_hex": signature.hex(),
    }
    _verify_raw_signature(
        signer.public_key_hex,
        document["signature_hex"],
        digest,
        "signature.signature_hex",
    )
    return document


def sign_bilateral_record(
    value: object,
    *,
    account_id: str,
    signer: EvidenceSignerAdapter,
) -> dict[str, Any]:
    context = _parse_bilateral_unsigned(value, "bilateral_record")
    if account_id not in (context.source_account, context.target_account):
        raise TaskNodeUnlError("signer_not_relation_endpoint", "account_id")
    epoch = (
        context.source_epoch
        if account_id == context.source_account
        else context.target_epoch
    )
    return _signature_document(
        account_id=account_id,
        control_epoch=epoch,
        signer=signer,
        digest=bilateral_statement_digest(value),
    )


def sign_control_declaration(
    value: object,
    *,
    account_id: str,
    signer: EvidenceSignerAdapter,
) -> dict[str, Any]:
    context = _parse_declaration_unsigned(value, "control_declaration")
    epochs = dict(context.members)
    if account_id not in epochs:
        raise TaskNodeUnlError("signer_not_control_member", "account_id")
    return _signature_document(
        account_id=account_id,
        control_epoch=epochs[account_id],
        signer=signer,
        digest=control_declaration_statement_digest(value),
    )


def _verify_raw_signature(
    public_key_hex: object,
    signature_hex: object,
    digest: bytes,
    field: str,
) -> None:
    public_key = _public_key(
        public_key_hex, field.replace("signature_hex", "public_key_hex")
    )
    checked = require_lower_hex(
        signature_hex, field, byte_length=SECP256K1_SIGNATURE_BYTES
    )
    try:
        signature = keys.Signature(bytes.fromhex(checked))
    except Exception as exc:
        raise TaskNodeUnlError("invalid_signature_encoding", field) from exc
    if signature.s > SECPK1_N // 2:
        raise TaskNodeUnlError("non_canonical_signature", field)
    if not public_key.verify_msg_hash(digest, signature):
        raise TaskNodeUnlError("signature_verification_failed", field)
    try:
        recovered = signature.recover_public_key_from_msg_hash(digest)
    except Exception as exc:
        raise TaskNodeUnlError("signature_recovery_failed", field) from exc
    if recovered != public_key:
        raise TaskNodeUnlError("signature_recovery_mismatch", field)


def _parse_signature(
    value: object,
    field: str,
    *,
    expected_digest: bytes,
    registry: Mapping[str, ControlEpoch],
) -> str:
    row = require_v2_object(
        value,
        schema=V2_SIGNATURE_SCHEMA,
        required=(
            "account_id",
            "control_epoch",
            "algorithm",
            "public_key_hex",
            "signed_digest",
            "signature_hex",
        ),
        field=field,
    )
    account = require_bounded_identifier(row["account_id"], f"{field}.account_id")
    binding = registry.get(account)
    if binding is None:
        raise TaskNodeUnlError("signature_account_unbound", f"{field}.account_id")
    epoch = require_lower_hex(row["control_epoch"], f"{field}.control_epoch")
    if epoch != binding.epoch:
        raise TaskNodeUnlError("signature_epoch_mismatch", f"{field}.control_epoch")
    if row["algorithm"] != V2_SIGNATURE_ALGORITHM:
        raise TaskNodeUnlError("unknown_signature_algorithm", f"{field}.algorithm")
    if row["public_key_hex"] != binding.public_key_hex:
        raise TaskNodeUnlError(
            "signature_public_key_mismatch", f"{field}.public_key_hex"
        )
    signed = require_lower_hex(row["signed_digest"], f"{field}.signed_digest")
    if signed != expected_digest.hex():
        raise TaskNodeUnlError("signature_statement_mismatch", f"{field}.signed_digest")
    _verify_raw_signature(
        row["public_key_hex"],
        row["signature_hex"],
        expected_digest,
        f"{field}.signature_hex",
    )
    return account


def _parse_bilateral(
    value: object,
    index: int,
    registry: Mapping[str, ControlEpoch],
    *,
    chain_id: str,
    genesis_hash: str,
) -> _BilateralContext:
    field = f"evidence.bilateral_records[{index}]"
    context = _parse_bilateral_unsigned(value, field)
    if context.chain_id != chain_id:
        raise TaskNodeUnlError("chain_id_mismatch", f"{field}.chain_id")
    if context.genesis_hash != genesis_hash:
        raise TaskNodeUnlError(
            "genesis_hash_mismatch", f"{field}.genesis_hash"
        )
    source_binding = registry.get(context.source_account)
    target_binding = registry.get(context.target_account)
    if source_binding is None:
        raise TaskNodeUnlError("relation_account_unbound", f"{field}.source_account")
    if target_binding is None:
        raise TaskNodeUnlError("relation_account_unbound", f"{field}.target_account")
    if context.source_epoch != source_binding.epoch:
        raise TaskNodeUnlError("relation_epoch_mismatch", f"{field}.source_epoch")
    if context.target_epoch != target_binding.epoch:
        raise TaskNodeUnlError("relation_epoch_mismatch", f"{field}.target_epoch")
    newest_event = max(source_binding.event_at, target_binding.event_at)
    if context.acknowledged_at < newest_event:
        raise TaskNodeUnlError(
            "acknowledgement_before_epoch", f"{field}.acknowledged_at"
        )
    if (
        context.work_completed_at is not None
        and context.work_completed_at < newest_event
    ):
        raise TaskNodeUnlError("cowork_before_epoch", f"{field}.work_completed_at")
    digest = bilateral_statement_digest(value)
    signatures = require_array(
        context.row["signatures"], f"{field}.signatures", maximum=2
    )
    signers = tuple(
        _parse_signature(
            signature,
            f"{field}.signatures[{signature_index}]",
            expected_digest=digest,
            registry=registry,
        )
        for signature_index, signature in enumerate(signatures)
    )
    if tuple(sorted(signers)) != signers:
        raise TaskNodeUnlError("non_canonical_signature_order", f"{field}.signatures")
    if len(set(signers)) != len(signers):
        raise TaskNodeUnlError("duplicate_signature", f"{field}.signatures")
    endpoints = {context.source_account, context.target_account}
    if context.action in ("grant", "renew"):
        if set(signers) != endpoints:
            raise TaskNodeUnlError("bilateral_consent_missing", f"{field}.signatures")
    elif len(signers) != 1 or signers[0] not in endpoints:
        raise TaskNodeUnlError("revocation_signer_invalid", f"{field}.signatures")
    return context


def _parse_declaration(
    value: object,
    index: int,
    registry: Mapping[str, ControlEpoch],
    *,
    chain_id: str,
    genesis_hash: str,
) -> _DeclarationContext:
    field = f"evidence.control_declarations[{index}]"
    context = _parse_declaration_unsigned(value, field)
    if context.chain_id != chain_id:
        raise TaskNodeUnlError("chain_id_mismatch", f"{field}.chain_id")
    if context.genesis_hash != genesis_hash:
        raise TaskNodeUnlError(
            "genesis_hash_mismatch", f"{field}.genesis_hash"
        )
    for member_index, (account, epoch) in enumerate(context.members):
        binding = registry.get(account)
        member_field = f"{field}.members[{member_index}]"
        if binding is None:
            raise TaskNodeUnlError(
                "control_member_unbound", f"{member_field}.account_id"
            )
        if epoch != binding.epoch:
            raise TaskNodeUnlError(
                "control_epoch_mismatch", f"{member_field}.control_epoch"
            )
        if context.acknowledged_at < binding.event_at:
            raise TaskNodeUnlError(
                "acknowledgement_before_epoch", f"{field}.acknowledged_at"
            )
    digest = control_declaration_statement_digest(value)
    signatures = require_array(
        context.row["signatures"], f"{field}.signatures", maximum=MAX_CONTROL_MEMBERS
    )
    signers = tuple(
        _parse_signature(
            signature,
            f"{field}.signatures[{signature_index}]",
            expected_digest=digest,
            registry=registry,
        )
        for signature_index, signature in enumerate(signatures)
    )
    if tuple(sorted(signers)) != signers:
        raise TaskNodeUnlError("non_canonical_signature_order", f"{field}.signatures")
    if len(set(signers)) != len(signers):
        raise TaskNodeUnlError("duplicate_signature", f"{field}.signatures")
    if set(signers) != {account for account, _epoch in context.members}:
        raise TaskNodeUnlError("all_member_consent_missing", f"{field}.signatures")
    return context


def _claimed_accounts(value: object, record_class: str) -> tuple[str, ...]:
    if not isinstance(value, Mapping):
        return ()
    claimed: list[str] = []
    if record_class == "bilateral":
        candidates = (value.get("source_account"), value.get("target_account"))
    elif record_class == "control_declaration":
        members = value.get("members")
        candidates = (
            tuple(
                item.get("account_id")
                for item in members
                if isinstance(item, Mapping)
            )
            if isinstance(members, list)
            else ()
        )
    else:
        candidates = (value.get("account_id"),)
    for candidate in candidates:
        if isinstance(candidate, str) and candidate and candidate == candidate.strip():
            claimed.append(candidate)
    return tuple(sorted(set(claimed)))


def _rejection(
    record_class: str,
    index: int,
    error: TaskNodeUnlError,
    value: object,
) -> RecordRejection:
    return RecordRejection(
        record_class=record_class,
        index=index,
        field=error.detail or f"evidence.{record_class}[{index}]",
        code=error.code,
        accounts=_claimed_accounts(value, record_class),
    )


def _relation_key(context: _BilateralContext) -> tuple[str, ...]:
    if context.relation_kind == "cowork":
        endpoints = tuple(sorted((context.source_account, context.target_account)))
    else:
        endpoints = (context.source_account, context.target_account)
    return (context.relation_kind, *endpoints, context.statement_digest)


def _active_relations(
    records: Sequence[_BilateralContext],
    window_index: int,
) -> tuple[ActiveRelation, ...]:
    states: dict[tuple[str, ...], ActiveRelation] = {}
    ordered = sorted(
        records,
        key=lambda item: (
            item.effective_window,
            item.action,
            _domain_hash("postfiat/tasknode-unl-v2/record/v2", item.row),
        ),
    )
    for item in ordered:
        if item.effective_window > window_index:
            continue
        key = _relation_key(item)
        if item.action == "revoke":
            states.pop(key, None)
            continue
        if item.expiry_window < window_index:
            states.pop(key, None)
            continue
        record_digest = _domain_hash(
            "postfiat/tasknode-unl-v2/record/v2", item.row
        )
        credit_id = _domain_hash(
            "postfiat/tasknode-unl-v2/relation-credit/v2",
            {
                "relation_kind": item.relation_kind,
                "endpoints": list(key[1:-1]),
                "statement_digest": item.statement_digest,
            },
        )
        states[key] = ActiveRelation(
            relation_kind=item.relation_kind,
            source_account=item.source_account,
            target_account=item.target_account,
            statement_digest=item.statement_digest,
            credit_id=credit_id,
            effective_window=item.effective_window,
            expiry_window=item.expiry_window,
            record_digest=record_digest,
        )
    return tuple(sorted(states.values()))


def _active_declarations(
    records: Sequence[_DeclarationContext],
    window_index: int,
) -> tuple[ActiveControlDeclaration, ...]:
    states: dict[str, ActiveControlDeclaration] = {}
    expected_members: dict[str, tuple[str, ...]] = {}
    ordered = sorted(
        records,
        key=lambda item: (
            item.effective_window,
            item.action,
            _domain_hash("postfiat/tasknode-unl-v2/record/v2", item.row),
        ),
    )
    for item in ordered:
        members = tuple(account for account, _epoch in item.members)
        prior_members = expected_members.get(item.statement_digest)
        if prior_members is not None and prior_members != members:
            continue
        expected_members[item.statement_digest] = members
        if item.effective_window > window_index:
            continue
        if item.action == "dissolve":
            states.pop(item.statement_digest, None)
            continue
        if item.expiry_window < window_index:
            states.pop(item.statement_digest, None)
            continue
        states[item.statement_digest] = ActiveControlDeclaration(
            statement_digest=item.statement_digest,
            members=members,
            effective_window=item.effective_window,
            expiry_window=item.expiry_window,
            record_digest=_domain_hash(
                "postfiat/tasknode-unl-v2/record/v2", item.row
            ),
        )
    return tuple(sorted(states.values()))


def assess_fresh_window(
    epoch: ControlEpoch,
    *,
    window: EvaluationWindow,
    score_evidence: Sequence[_ScoreContext],
    active_relations: Sequence[ActiveRelation],
) -> ContinuityAssessment:
    """Assess only V2 evidence freshness; policy scoring remains a later stage."""

    scores = tuple(
        item
        for item in score_evidence
        if item.account_id == epoch.account_id
        and item.control_epoch == epoch.epoch
        and item.evidence_at >= epoch.event_at
    )
    vouches = tuple(
        item
        for item in active_relations
        if item.relation_kind == "vouch"
        and epoch.account_id in (item.source_account, item.target_account)
    )
    cowork = tuple(
        item
        for item in active_relations
        if item.relation_kind == "cowork"
        and epoch.account_id in (item.source_account, item.target_account)
    )
    reasons: list[str] = []
    if window.end < epoch.event_at + timedelta(days=V2_WINDOW_DAYS):
        reasons.append("fresh_window_incomplete")
    score_kinds = {item.evidence_kind for item in scores}
    reasons.extend(
        f"post_epoch_score_evidence_missing:{kind}"
        for kind in SCORE_EVIDENCE_KINDS
        if kind not in score_kinds
    )
    status = "HOLD_CONTINUITY" if reasons else "READY"
    return ContinuityAssessment(
        account_id=epoch.account_id,
        control_epoch=epoch.epoch,
        status=status,
        incumbent=epoch.incumbent,
        retain_incumbent=epoch.incumbent,
        reasons=tuple(reasons),
        fresh_score_evidence=len(scores),
        renewed_vouches=len(vouches),
        post_epoch_cowork=len(cowork),
    )


def _parse_evidence_header(
    value: object,
) -> tuple[Mapping[str, Any], EvaluationWindow, str, str]:
    row = require_v2_object(
        value,
        schema=V2_EVIDENCE_INPUT_SCHEMA,
        required=(
            "policy_id",
            "chain_id",
            "genesis_hash",
            "window",
            "score_evidence",
            "funding_observations",
            "bilateral_records",
            "control_declarations",
        ),
        field="evidence",
    )
    if row["policy_id"] != V2_POLICY_ID:
        raise TaskNodeUnlError("policy_id_mismatch", "evidence.policy_id")
    chain_id = require_bounded_identifier(row["chain_id"], "evidence.chain_id")
    genesis_hash = require_lower_hex(
        row["genesis_hash"], "evidence.genesis_hash"
    )
    require_document_bound(row, "evidence")
    for name in (
        "score_evidence",
        "funding_observations",
        "bilateral_records",
        "control_declarations",
    ):
        require_array(row[name], f"evidence.{name}")
    return (
        row,
        EvaluationWindow.from_dict(row["window"], "evidence.window"),
        chain_id,
        genesis_hash,
    )


def verify_evidence_snapshot(
    snapshot: object,
    control_registry: object,
    *,
    expected_policy_root: str | None = None,
    expected_input_root: str | None = None,
    expected_registry_root: str | None = None,
) -> EvidenceSnapshotResult:
    """Verify commitments, isolate bad records, and derive boundary-active facts."""

    try:
        row = require_v2_object(
            snapshot,
            schema=V2_EVIDENCE_SNAPSHOT_SCHEMA,
            required=("commitments", "evidence"),
            field="snapshot",
        )
        commitments = require_closed_keys(
            row["commitments"],
            required=("policy_root", "input_root", "registry_root"),
            field="snapshot.commitments",
        )
        checked_commitments = {
            name: require_lower_hex(
                commitments[name], f"snapshot.commitments.{name}"
            )
            for name in ("policy_root", "input_root", "registry_root")
        }
        actual = {
            "policy_root": policy_root(),
            "input_root": evidence_input_root(row["evidence"]),
            "registry_root": control_registry_root(control_registry),
        }
        supplied_expected = {
            "policy_root": expected_policy_root,
            "input_root": expected_input_root,
            "registry_root": expected_registry_root,
        }
        for name in ("policy_root", "input_root", "registry_root"):
            if checked_commitments[name] != actual[name]:
                raise TaskNodeUnlError(
                    "commitment_mismatch", f"snapshot.commitments.{name}"
                )
            if supplied_expected[name] is not None:
                expected = require_lower_hex(
                    supplied_expected[name], f"expected_{name}"
                )
                if checked_commitments[name] != expected:
                    raise TaskNodeUnlError(
                        "expected_commitment_mismatch",
                        f"snapshot.commitments.{name}",
                    )
        evidence, window, chain_id, genesis_hash = _parse_evidence_header(
            row["evidence"]
        )
        (
            registry_chain_id,
            registry_genesis_hash,
            registry_entries,
        ) = _parse_registry(control_registry)
        if registry_chain_id != chain_id:
            raise TaskNodeUnlError(
                "chain_id_mismatch", "control_registry.chain_id"
            )
        if registry_genesis_hash != genesis_hash:
            raise TaskNodeUnlError(
                "genesis_hash_mismatch", "control_registry.genesis_hash"
            )
    except TaskNodeUnlError as error:
        return _hold((_failure(error, "snapshot"),))

    registry = {entry.account_id: entry for entry in registry_entries}
    rejections: list[RecordRejection] = []
    holds: set[str] = set()

    scores: list[_ScoreContext] = []
    historical_scores: list[Mapping[str, Any]] = []
    seen_score_digests: set[tuple[str, str, str]] = set()
    for index, value in enumerate(evidence["score_evidence"]):
        try:
            score = _parse_score(value, index)
            binding = registry.get(score.account_id)
            if binding is None:
                raise TaskNodeUnlError(
                    "score_account_unbound",
                    f"evidence.score_evidence[{index}].account_id",
                )
            duplicate_key = (
                score.account_id,
                score.evidence_kind,
                score.evidence_digest,
            )
            if duplicate_key in seen_score_digests:
                continue
            seen_score_digests.add(duplicate_key)
            if (
                score.control_epoch != binding.epoch
                or score.evidence_at < binding.event_at
            ):
                historical_scores.append(score.row)
            else:
                scores.append(score)
        except TaskNodeUnlError as error:
            rejection = _rejection("score_evidence", index, error, value)
            rejections.append(rejection)
            holds.update(rejection.accounts)

    funding: dict[str, Mapping[str, Any]] = {}
    for index, value in enumerate(evidence["funding_observations"]):
        try:
            parsed = _parse_funding(value, index)
            digest = str(parsed["observation_digest"])
            prior = funding.get(digest)
            if prior is not None and prior != parsed:
                raise TaskNodeUnlError(
                    "conflicting_funding_observation",
                    f"evidence.funding_observations[{index}].observation_digest",
                )
            funding[digest] = parsed
        except TaskNodeUnlError as error:
            rejections.append(_rejection("funding_observation", index, error, value))

    bilateral: list[_BilateralContext] = []
    seen_bilateral_records: set[str] = set()
    for index, value in enumerate(evidence["bilateral_records"]):
        try:
            parsed = _parse_bilateral(
                value,
                index,
                registry,
                chain_id=chain_id,
                genesis_hash=genesis_hash,
            )
            digest = _domain_hash("postfiat/tasknode-unl-v2/record/v2", parsed.row)
            if digest in seen_bilateral_records:
                raise TaskNodeUnlError(
                    "replayed_acknowledgement",
                    f"evidence.bilateral_records[{index}]",
                )
            seen_bilateral_records.add(digest)
            bilateral.append(parsed)
        except TaskNodeUnlError as error:
            rejection = _rejection("bilateral", index, error, value)
            rejections.append(rejection)
            holds.update(rejection.accounts)

    declarations: list[_DeclarationContext] = []
    seen_declaration_records: set[str] = set()
    declaration_members: dict[str, tuple[str, ...]] = {}
    for index, value in enumerate(evidence["control_declarations"]):
        try:
            parsed = _parse_declaration(
                value,
                index,
                registry,
                chain_id=chain_id,
                genesis_hash=genesis_hash,
            )
            members = tuple(account for account, _epoch in parsed.members)
            prior = declaration_members.get(parsed.statement_digest)
            if prior is not None and prior != members:
                raise TaskNodeUnlError(
                    "control_declaration_member_mismatch",
                    f"evidence.control_declarations[{index}].members",
                )
            declaration_members[parsed.statement_digest] = members
            digest = _domain_hash("postfiat/tasknode-unl-v2/record/v2", parsed.row)
            if digest in seen_declaration_records:
                raise TaskNodeUnlError(
                    "replayed_acknowledgement",
                    f"evidence.control_declarations[{index}]",
                )
            seen_declaration_records.add(digest)
            declarations.append(parsed)
        except TaskNodeUnlError as error:
            rejection = _rejection("control_declaration", index, error, value)
            rejections.append(rejection)
            holds.update(rejection.accounts)

    active_relations = _active_relations(bilateral, window.index)
    active_declarations = _active_declarations(declarations, window.index)
    continuity = tuple(
        assess_fresh_window(
            entry,
            window=window,
            score_evidence=scores,
            active_relations=active_relations,
        )
        for entry in registry_entries
    )
    holds.update(
        item.account_id for item in continuity if item.status == "HOLD_CONTINUITY"
    )
    vouches = tuple(
        item for item in active_relations if item.relation_kind == "vouch"
    )
    cowork = tuple(
        item for item in active_relations if item.relation_kind == "cowork"
    )
    return EvidenceSnapshotResult(
        status="verified_with_holds" if rejections or holds else "verified",
        failures=(),
        record_rejections=tuple(sorted(rejections)),
        hold_accounts=tuple(sorted(holds)),
        commitments=checked_commitments,
        window=window,
        audit_funding_observations=tuple(funding[key] for key in sorted(funding)),
        historical_score_evidence=tuple(
            sorted(
                historical_scores,
                key=lambda item: canonical_json_bytes(item),
            )
        ),
        active_vouches=vouches,
        active_cowork=cowork,
        active_control_declarations=active_declarations,
        continuity=continuity,
    )
