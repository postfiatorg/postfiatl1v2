"""Pure public-edge extractors for the Task Node UNL trust graph.

All inputs are explicit JSON-shaped ledger snapshots. The module performs no
file, clock, database, credential-store, or network access. It emits the
`TrustEdge` type consumed directly by the step-2 trust walk and retains
canonical source provenance alongside each edge.
"""

from __future__ import annotations

import hashlib
from dataclasses import dataclass
from datetime import datetime, timedelta
from itertools import combinations
from typing import Any, Mapping, Protocol, Sequence

from eth_keys import keys
from eth_keys.constants import SECPK1_N

from .tasknode_unl_binding import wallet_address_from_public_key
from .tasknode_unl_schema import (
    ACCOUNTABILITY_WINDOW_DAYS,
    COWORK_POINTER_INPUT_SCHEMA,
    EDGE_EXTRACTION_RESULT_SCHEMA,
    FUNDING_EXCLUSION_SCHEMA,
    FUNDING_TRANSFER_INPUT_SCHEMA,
    SHADOW_MODE,
    TASKNODE_POINTER_SCHEMA,
    VOUCH_LEDGER_INPUT_SCHEMA,
    VOUCH_MEMO_ENVELOPE_SCHEMA,
    VOUCH_SIGNATURE_ALGORITHM,
    VOUCH_STATEMENT_DOMAIN,
    VOUCH_STATEMENT_SCHEMA,
    TaskNodeUnlError,
    canonical_json_bytes,
    format_utc_timestamp,
    parse_utc_timestamp,
    require_closed_keys,
    require_identifier,
    require_int,
)
from .tasknode_unl_trust_graph import TrustEdge

_SHA256_BYTES = 32
_PUBLIC_KEY_BYTES = 33
_SIGNATURE_BYTES = 65
_MAX_IDENTIFIER_BYTES = 128
_MAX_RECORDS = 32_768
_MAX_ACCOUNTS = 16_384
_MAX_LEDGER_INDEX = (1 << 63) - 1
_VOUCH_CLAIM = "knows_operator"
_PUBLIC_VISIBILITY = "public"
_FORBIDDEN_VISIBILITIES = ("private", "encrypted")
_WORK_UNIT_KINDS = ("hive_project", "team_grant")
_PARTICIPATION_STATUSES = ("completed", "shared", "incomplete")
_EXCLUSION_CATEGORIES = ("exchange", "foundation_distribution")


class VouchSignerAdapter(Protocol):
    """Custody boundary for a caller-supplied vouch signer."""

    algorithm_id: str
    public_key_hex: str

    def sign_digest(self, digest: bytes) -> bytes:
        """Sign one 32-byte digest without exposing private material."""


@dataclass(frozen=True, order=True)
class EdgeProvenance:
    """Canonical public evidence retained for one emitted graph edge."""

    kind: str
    source: str
    target: str
    evidence_id: str
    source_record_ids: tuple[str, ...]
    qualification_reasons: tuple[str, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "kind": self.kind,
            "source": self.source,
            "target": self.target,
            "evidence_id": self.evidence_id,
            "source_record_ids": list(self.source_record_ids),
            "qualification_reasons": list(self.qualification_reasons),
        }


@dataclass(frozen=True)
class EdgeExtractionResult:
    """All-or-nothing edge extraction output for the later shadow runner."""

    status: str
    hold_reasons: tuple[str, ...]
    edges: tuple[TrustEdge, ...]
    provenance: tuple[EdgeProvenance, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": EDGE_EXTRACTION_RESULT_SCHEMA,
            "mode": SHADOW_MODE,
            "status": self.status,
            "hold_reasons": list(self.hold_reasons),
            "edges": [
                {
                    "source": edge.source,
                    "target": edge.target,
                    "kind": edge.kind,
                    "evidence_id": edge.evidence_id,
                }
                for edge in self.edges
            ],
            "provenance": [
                item.to_dict() for item in self.provenance
            ],
        }

    def canonical_bytes(self) -> bytes:
        return canonical_json_bytes(self.to_dict())


@dataclass(frozen=True)
class ExtractedEdges:
    """Canonical edges and provenance from one public source."""

    edges: tuple[TrustEdge, ...]
    provenance: tuple[EdgeProvenance, ...]


@dataclass(frozen=True)
class _Window:
    start: datetime
    end: datetime


@dataclass(frozen=True)
class _Transfer:
    tx_hash: str
    ledger_index: int
    transaction_index: int
    close_time: datetime
    source_wallet: str
    target_wallet: str
    value_units: int

    @property
    def position(self) -> tuple[int, int, str]:
        return self.ledger_index, self.transaction_index, self.tx_hash


def _require_identifier(
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


def _require_bool(value: object, field: str) -> bool:
    if not isinstance(value, bool):
        raise TaskNodeUnlError("invalid_boolean", field)
    return value


def _require_array(
    value: object,
    field: str,
    *,
    maximum: int = _MAX_RECORDS,
) -> list[Any]:
    if not isinstance(value, list):
        raise TaskNodeUnlError("invalid_array", field)
    if len(value) > maximum:
        raise TaskNodeUnlError("array_too_large", field)
    return value


def _canonical_timestamp(value: object, field: str) -> datetime:
    parsed = parse_utc_timestamp(value, field)
    if format_utc_timestamp(parsed) != value:
        raise TaskNodeUnlError("non_canonical_timestamp", field)
    return parsed


def _optional_timestamp(value: object, field: str) -> datetime | None:
    return None if value is None else _canonical_timestamp(value, field)


def _parse_window(value: object, field: str) -> _Window:
    row = require_closed_keys(
        value,
        required=("start", "end", "days"),
        field=field,
    )
    start = _canonical_timestamp(row["start"], f"{field}.start")
    end = _canonical_timestamp(row["end"], f"{field}.end")
    days = require_int(row["days"], f"{field}.days", minimum=1)
    if days != ACCOUNTABILITY_WINDOW_DAYS:
        raise TaskNodeUnlError("window_days_mismatch", f"{field}.days")
    if end - start != timedelta(days=ACCOUNTABILITY_WINDOW_DAYS):
        raise TaskNodeUnlError("window_bounds_mismatch", field)
    return _Window(start=start, end=end)


def _parse_document_header(
    document: object,
    *,
    schema: str,
    field: str,
    required: Sequence[str],
) -> Mapping[str, Any]:
    row = require_closed_keys(
        document,
        required=("schema", "mode", "window", *required),
        field=field,
    )
    if row["schema"] != schema:
        raise TaskNodeUnlError("unknown_schema", f"{field}.schema")
    if row["mode"] != SHADOW_MODE:
        raise TaskNodeUnlError("mode_mismatch", f"{field}.mode")
    return row


def _public_key(public_key_hex: object, field: str) -> keys.PublicKey:
    checked = _require_lower_hex(
        public_key_hex,
        field,
        byte_length=_PUBLIC_KEY_BYTES,
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


def _verify_signature(
    public_key_hex: object,
    signature_hex: object,
    digest: bytes,
    field: str,
) -> None:
    public_key = _public_key(public_key_hex, f"{field}.public_key_hex")
    checked = _require_lower_hex(
        signature_hex,
        f"{field}.signature_hex",
        byte_length=_SIGNATURE_BYTES,
    )
    try:
        signature = keys.Signature(bytes.fromhex(checked))
    except Exception as exc:
        raise TaskNodeUnlError(
            "invalid_signature_encoding", f"{field}.signature_hex"
        ) from exc
    if signature.s > SECPK1_N // 2:
        raise TaskNodeUnlError(
            "non_canonical_signature", f"{field}.signature_hex"
        )
    if not public_key.verify_msg_hash(digest, signature):
        raise TaskNodeUnlError(
            "signature_verification_failed", f"{field}.signature_hex"
        )
    try:
        recovered = signature.recover_public_key_from_msg_hash(digest)
    except Exception as exc:
        raise TaskNodeUnlError(
            "signature_recovery_failed", f"{field}.signature_hex"
        ) from exc
    if recovered != public_key:
        raise TaskNodeUnlError(
            "signature_recovery_mismatch", f"{field}.signature_hex"
        )


def _parse_vouch_statement(
    value: object,
    field: str = "vouch_statement",
) -> tuple[Mapping[str, Any], datetime]:
    row = require_closed_keys(
        value,
        required=(
            "schema",
            "mode",
            "source_account",
            "target_account",
            "source_wallet_address",
            "issued_at",
            "claim",
            "nonce_hex",
        ),
        field=field,
    )
    if row["schema"] != VOUCH_STATEMENT_SCHEMA:
        raise TaskNodeUnlError("unknown_schema", f"{field}.schema")
    if row["mode"] != SHADOW_MODE:
        raise TaskNodeUnlError("mode_mismatch", f"{field}.mode")
    source = _require_identifier(
        row["source_account"], f"{field}.source_account"
    )
    target = _require_identifier(
        row["target_account"], f"{field}.target_account"
    )
    if source == target:
        raise TaskNodeUnlError("self_edge", field)
    _require_identifier(
        row["source_wallet_address"],
        f"{field}.source_wallet_address",
    )
    issued_at = _canonical_timestamp(
        row["issued_at"], f"{field}.issued_at"
    )
    if row["claim"] != _VOUCH_CLAIM:
        raise TaskNodeUnlError("unknown_vouch_claim", f"{field}.claim")
    _require_lower_hex(
        row["nonce_hex"],
        f"{field}.nonce_hex",
        byte_length=_SHA256_BYTES,
    )
    return row, issued_at


def vouch_statement_hash(statement: object) -> bytes:
    """Return the domain-separated digest of one canonical vouch statement."""

    row, _issued_at = _parse_vouch_statement(statement)
    signing_bytes = (
        VOUCH_STATEMENT_DOMAIN.encode("ascii")
        + b"\x00"
        + canonical_json_bytes(row)
    )
    return hashlib.sha256(signing_bytes).digest()


def sign_vouch_statement(
    statement: object,
    signer: VouchSignerAdapter,
) -> dict[str, Any]:
    """Sign a vouch locally through a custody-preserving signer adapter."""

    row, _issued_at = _parse_vouch_statement(statement)
    if signer.algorithm_id != VOUCH_SIGNATURE_ALGORITHM:
        raise TaskNodeUnlError(
            "unknown_signature_algorithm", "signer.algorithm_id"
        )
    if (
        wallet_address_from_public_key(signer.public_key_hex)
        != row["source_wallet_address"]
    ):
        raise TaskNodeUnlError(
            "signer_wallet_mismatch", "statement.source_wallet_address"
        )
    digest = vouch_statement_hash(row)
    signature = signer.sign_digest(digest)
    if not isinstance(signature, bytes):
        raise TaskNodeUnlError(
            "signer_returned_non_bytes", "signer.sign_digest"
        )
    envelope = {
        "schema": VOUCH_MEMO_ENVELOPE_SCHEMA,
        "mode": SHADOW_MODE,
        "digest_hash": digest.hex(),
        "signature_algorithm": signer.algorithm_id,
        "public_key_hex": signer.public_key_hex,
        "signature_hex": signature.hex(),
        "statement": row,
    }
    _verify_signature(
        signer.public_key_hex,
        envelope["signature_hex"],
        digest,
        "vouch_memo",
    )
    return envelope


def _verified_vouch_record(
    value: object,
    index: int,
    window: _Window,
    wallet_by_account: Mapping[str, str],
) -> tuple[
    TrustEdge,
    EdgeProvenance,
    tuple[str, int],
    tuple[int, int],
]:
    field = f"vouch_ledger.memos[{index}]"
    row = require_closed_keys(
        value,
        required=(
            "tx_hash",
            "ledger_index",
            "transaction_index",
            "memo_index",
            "close_time",
            "sender_wallet_address",
            "visibility",
            "memo",
        ),
        field=field,
    )
    visibility = row["visibility"]
    if visibility in _FORBIDDEN_VISIBILITIES:
        raise TaskNodeUnlError("private_vouch_forbidden", f"{field}.visibility")
    if visibility != _PUBLIC_VISIBILITY:
        raise TaskNodeUnlError("unknown_visibility", f"{field}.visibility")
    tx_hash = _require_lower_hex(
        row["tx_hash"], f"{field}.tx_hash", byte_length=_SHA256_BYTES
    )
    ledger_index = require_int(
        row["ledger_index"], f"{field}.ledger_index", minimum=1
    )
    transaction_index = require_int(
        row["transaction_index"],
        f"{field}.transaction_index",
        minimum=0,
    )
    memo_index = require_int(
        row["memo_index"], f"{field}.memo_index", minimum=0
    )
    if (
        ledger_index > _MAX_LEDGER_INDEX
        or transaction_index > _MAX_LEDGER_INDEX
        or memo_index > _MAX_LEDGER_INDEX
    ):
        raise TaskNodeUnlError("ledger_position_out_of_range", field)
    close_time = _canonical_timestamp(
        row["close_time"], f"{field}.close_time"
    )
    if not window.start <= close_time <= window.end:
        raise TaskNodeUnlError("record_outside_window", f"{field}.close_time")
    sender = _require_identifier(
        row["sender_wallet_address"],
        f"{field}.sender_wallet_address",
    )

    memo = require_closed_keys(
        row["memo"],
        required=(
            "schema",
            "mode",
            "digest_hash",
            "signature_algorithm",
            "public_key_hex",
            "signature_hex",
            "statement",
        ),
        field=f"{field}.memo",
    )
    if memo["schema"] != VOUCH_MEMO_ENVELOPE_SCHEMA:
        raise TaskNodeUnlError("unknown_schema", f"{field}.memo.schema")
    if memo["mode"] != SHADOW_MODE:
        raise TaskNodeUnlError("mode_mismatch", f"{field}.memo.mode")
    if memo["signature_algorithm"] != VOUCH_SIGNATURE_ALGORITHM:
        raise TaskNodeUnlError(
            "unknown_signature_algorithm",
            f"{field}.memo.signature_algorithm",
        )
    statement, issued_at = _parse_vouch_statement(
        memo["statement"], f"{field}.memo.statement"
    )
    if issued_at > close_time:
        raise TaskNodeUnlError(
            "vouch_issued_after_ledger_close",
            f"{field}.memo.statement.issued_at",
        )
    if statement["source_wallet_address"] != sender:
        raise TaskNodeUnlError(
            "vouch_sender_mismatch", f"{field}.sender_wallet_address"
        )
    source_wallet = wallet_by_account.get(statement["source_account"])
    if source_wallet is None:
        raise TaskNodeUnlError(
            "vouch_source_binding_missing",
            f"{field}.memo.statement.source_account",
        )
    if source_wallet != sender:
        raise TaskNodeUnlError(
            "vouch_source_binding_mismatch",
            f"{field}.memo.statement.source_account",
        )
    public_key_hex = _require_lower_hex(
        memo["public_key_hex"],
        f"{field}.memo.public_key_hex",
        byte_length=_PUBLIC_KEY_BYTES,
    )
    if wallet_address_from_public_key(public_key_hex) != sender:
        raise TaskNodeUnlError(
            "vouch_public_key_wallet_mismatch",
            f"{field}.memo.public_key_hex",
        )
    digest = vouch_statement_hash(statement)
    claimed_digest = _require_lower_hex(
        memo["digest_hash"],
        f"{field}.memo.digest_hash",
        byte_length=_SHA256_BYTES,
    )
    if claimed_digest != digest.hex():
        raise TaskNodeUnlError(
            "vouch_digest_mismatch", f"{field}.memo.digest_hash"
        )
    _verify_signature(
        public_key_hex,
        memo["signature_hex"],
        digest,
        f"{field}.memo",
    )

    evidence_id = f"vouch:{tx_hash}:{memo_index}"
    edge = TrustEdge(
        source=statement["source_account"],
        target=statement["target_account"],
        kind="vouch",
        evidence_id=evidence_id,
    )
    provenance = EdgeProvenance(
        kind=edge.kind,
        source=edge.source,
        target=edge.target,
        evidence_id=evidence_id,
        source_record_ids=(f"{tx_hash}:{memo_index}",),
        qualification_reasons=("signed_public_ledger_vouch",),
    )
    return (
        edge,
        provenance,
        (tx_hash, memo_index),
        (ledger_index, transaction_index),
    )


def _canonical_extracted(
    edges: Sequence[TrustEdge],
    provenance: Sequence[EdgeProvenance],
) -> ExtractedEdges:
    return ExtractedEdges(
        edges=tuple(
            sorted(
                set(edges),
                key=lambda edge: (
                    edge.kind,
                    edge.source,
                    edge.target,
                    edge.evidence_id,
                ),
            )
        ),
        provenance=tuple(sorted(set(provenance))),
    )


def extract_vouch_edges(
    document: object,
    *,
    wallet_accounts: object,
) -> ExtractedEdges:
    """Verify public ledger vouches and emit directed weight-one edge facts."""

    row = _parse_document_header(
        document,
        schema=VOUCH_LEDGER_INPUT_SCHEMA,
        field="vouch_ledger",
        required=("memos",),
    )
    window = _parse_window(row["window"], "vouch_ledger.window")
    values = _require_array(row["memos"], "vouch_ledger.memos")
    _account_by_wallet, wallet_by_account = _wallet_accounts(
        wallet_accounts
    )
    records: dict[tuple[str, int], bytes] = {}
    tx_hash_by_position: dict[tuple[int, int], str] = {}
    position_by_tx_hash: dict[str, tuple[int, int]] = {}
    edges: list[TrustEdge] = []
    provenance: list[EdgeProvenance] = []
    for index, value in enumerate(values):
        edge, source, record_id, position = _verified_vouch_record(
            value, index, window, wallet_by_account
        )
        encoded = canonical_json_bytes(value)
        previous = records.get(record_id)
        if previous is not None and previous != encoded:
            raise TaskNodeUnlError(
                "conflicting_vouch_record",
                f"{record_id[0]}:{record_id[1]}",
            )
        records[record_id] = encoded
        previous_hash = tx_hash_by_position.get(position)
        if previous_hash is not None and previous_hash != record_id[0]:
            raise TaskNodeUnlError(
                "conflicting_ledger_position",
                f"{position[0]}:{position[1]}",
            )
        tx_hash_by_position[position] = record_id[0]
        previous_position = position_by_tx_hash.get(record_id[0])
        if previous_position is not None and previous_position != position:
            raise TaskNodeUnlError(
                "conflicting_ledger_position", record_id[0]
            )
        position_by_tx_hash[record_id[0]] = position
        edges.append(edge)
        provenance.append(source)
    return _canonical_extracted(edges, provenance)


def _cowork_record(
    value: object,
    index: int,
    window: _Window,
    wallet_by_account: Mapping[str, str],
) -> tuple[str, str, str, str, str, tuple[int, int]]:
    field = f"cowork_pointers.pointers[{index}]"
    row = require_closed_keys(
        value,
        required=(
            "pointer_hash",
            "pointer_schema",
            "account_id",
            "sender_wallet_address",
            "ledger_index",
            "transaction_index",
            "work_unit_kind",
            "work_unit_id",
            "participation_status",
            "close_time",
        ),
        field=field,
    )
    pointer_hash = _require_lower_hex(
        row["pointer_hash"],
        f"{field}.pointer_hash",
        byte_length=_SHA256_BYTES,
    )
    if row["pointer_schema"] != TASKNODE_POINTER_SCHEMA:
        raise TaskNodeUnlError(
            "unknown_pointer_schema", f"{field}.pointer_schema"
        )
    account_id = _require_identifier(
        row["account_id"], f"{field}.account_id"
    )
    sender = _require_identifier(
        row["sender_wallet_address"], f"{field}.sender_wallet_address"
    )
    expected_wallet = wallet_by_account.get(account_id)
    if expected_wallet is None:
        raise TaskNodeUnlError(
            "cowork_account_binding_missing", f"{field}.account_id"
        )
    if expected_wallet != sender:
        raise TaskNodeUnlError(
            "cowork_account_binding_mismatch", f"{field}.account_id"
        )
    ledger_index = require_int(
        row["ledger_index"], f"{field}.ledger_index", minimum=1
    )
    transaction_index = require_int(
        row["transaction_index"],
        f"{field}.transaction_index",
        minimum=0,
    )
    if (
        ledger_index > _MAX_LEDGER_INDEX
        or transaction_index > _MAX_LEDGER_INDEX
    ):
        raise TaskNodeUnlError("ledger_position_out_of_range", field)
    kind = row["work_unit_kind"]
    if kind not in _WORK_UNIT_KINDS:
        raise TaskNodeUnlError(
            "unknown_work_unit_kind", f"{field}.work_unit_kind"
        )
    unit_id = _require_identifier(
        row["work_unit_id"], f"{field}.work_unit_id"
    )
    status = row["participation_status"]
    if status not in _PARTICIPATION_STATUSES:
        raise TaskNodeUnlError(
            "unknown_participation_status",
            f"{field}.participation_status",
        )
    close_time = _canonical_timestamp(
        row["close_time"], f"{field}.close_time"
    )
    if not window.start <= close_time <= window.end:
        raise TaskNodeUnlError("record_outside_window", f"{field}.close_time")
    return (
        pointer_hash,
        account_id,
        kind,
        unit_id,
        status,
        (ledger_index, transaction_index),
    )


def extract_cowork_edges(
    document: object,
    *,
    wallet_accounts: object,
) -> ExtractedEdges:
    """Emit one undirected edge fact per distinct qualifying shared unit."""

    row = _parse_document_header(
        document,
        schema=COWORK_POINTER_INPUT_SCHEMA,
        field="cowork_pointers",
        required=("pointers",),
    )
    window = _parse_window(row["window"], "cowork_pointers.window")
    values = _require_array(row["pointers"], "cowork_pointers.pointers")
    _account_by_wallet, wallet_by_account = _wallet_accounts(
        wallet_accounts
    )
    records: dict[str, bytes] = {}
    pointer_by_position: dict[tuple[int, int], str] = {}
    units: dict[tuple[str, str], dict[str, set[str]]] = {}
    for index, value in enumerate(values):
        (
            pointer_hash,
            account,
            kind,
            unit_id,
            status,
            position,
        ) = _cowork_record(value, index, window, wallet_by_account)
        encoded = canonical_json_bytes(value)
        previous = records.get(pointer_hash)
        if previous is not None and previous != encoded:
            raise TaskNodeUnlError(
                "conflicting_pointer_record", pointer_hash
            )
        records[pointer_hash] = encoded
        previous_hash = pointer_by_position.get(position)
        if previous_hash is not None and previous_hash != pointer_hash:
            raise TaskNodeUnlError(
                "conflicting_ledger_position",
                f"{position[0]}:{position[1]}",
            )
        pointer_by_position[position] = pointer_hash
        qualifies = (
            (kind == "hive_project" and status == "completed")
            or (kind == "team_grant" and status == "shared")
        )
        if qualifies:
            units.setdefault((kind, unit_id), {}).setdefault(
                account, set()
            ).add(pointer_hash)

    edges: list[TrustEdge] = []
    provenance: list[EdgeProvenance] = []
    for (kind, unit_id), participants in sorted(units.items()):
        evidence_id = f"cowork:{kind}:{unit_id}"
        for source, target in combinations(sorted(participants), 2):
            edge = TrustEdge(source, target, "cowork", evidence_id)
            record_ids = tuple(
                sorted(participants[source] | participants[target])
            )
            edges.append(edge)
            provenance.append(
                EdgeProvenance(
                    kind=edge.kind,
                    source=source,
                    target=target,
                    evidence_id=evidence_id,
                    source_record_ids=record_ids,
                    qualification_reasons=(
                        (
                            "shared_completed_hive_project"
                            if kind == "hive_project"
                            else "shared_team_grant"
                        ),
                    ),
                )
            )
    return _canonical_extracted(edges, provenance)


def _parse_exclusions(
    document: object,
    window: _Window,
) -> frozenset[str]:
    if document is None:
        raise TaskNodeUnlError(
            "missing_funding_exclusion_list", "funding_exclusions"
        )
    row = require_closed_keys(
        document,
        required=(
            "schema",
            "mode",
            "version",
            "publication_tx_hash",
            "published_at",
            "valid_from",
            "valid_until",
            "addresses",
        ),
        field="funding_exclusions",
    )
    if row["schema"] != FUNDING_EXCLUSION_SCHEMA:
        raise TaskNodeUnlError(
            "unknown_schema", "funding_exclusions.schema"
        )
    if row["mode"] != SHADOW_MODE:
        raise TaskNodeUnlError(
            "mode_mismatch", "funding_exclusions.mode"
        )
    _require_identifier(row["version"], "funding_exclusions.version")
    _require_lower_hex(
        row["publication_tx_hash"],
        "funding_exclusions.publication_tx_hash",
        byte_length=_SHA256_BYTES,
    )
    published_at = _canonical_timestamp(
        row["published_at"], "funding_exclusions.published_at"
    )
    valid_from = _canonical_timestamp(
        row["valid_from"], "funding_exclusions.valid_from"
    )
    valid_until = _optional_timestamp(
        row["valid_until"], "funding_exclusions.valid_until"
    )
    if published_at > window.end:
        raise TaskNodeUnlError(
            "exclusion_list_published_after_window_end",
            "funding_exclusions.published_at",
        )
    if valid_from > window.start or (
        valid_until is not None and valid_until < window.end
    ):
        raise TaskNodeUnlError(
            "stale_funding_exclusion_list", "funding_exclusions"
        )
    if valid_until is not None and valid_until <= valid_from:
        raise TaskNodeUnlError(
            "invalid_exclusion_interval", "funding_exclusions"
        )

    values = _require_array(
        row["addresses"], "funding_exclusions.addresses"
    )
    categories_by_address: dict[str, str] = {}
    for index, value in enumerate(values):
        field = f"funding_exclusions.addresses[{index}]"
        item = require_closed_keys(
            value,
            required=("address", "category"),
            field=field,
        )
        address = _require_identifier(
            item["address"], f"{field}.address"
        )
        category = item["category"]
        if category not in _EXCLUSION_CATEGORIES:
            raise TaskNodeUnlError(
                "unknown_exclusion_category", f"{field}.category"
            )
        previous = categories_by_address.get(address)
        if previous is not None and previous != category:
            raise TaskNodeUnlError(
                "conflicting_exclusion_record", address
            )
        categories_by_address[address] = category
    return frozenset(categories_by_address)


def _wallet_accounts(
    value: object,
) -> tuple[dict[str, str], dict[str, str]]:
    values = _require_array(
        value, "funding_transfers.wallet_accounts", maximum=_MAX_ACCOUNTS
    )
    account_by_wallet: dict[str, str] = {}
    wallet_by_account: dict[str, str] = {}
    for index, item_value in enumerate(values):
        field = f"funding_transfers.wallet_accounts[{index}]"
        item = require_closed_keys(
            item_value,
            required=("wallet_address", "account_id"),
            field=field,
        )
        wallet = _require_identifier(
            item["wallet_address"], f"{field}.wallet_address"
        )
        account = _require_identifier(
            item["account_id"], f"{field}.account_id"
        )
        previous_account = account_by_wallet.get(wallet)
        if previous_account is not None and previous_account != account:
            raise TaskNodeUnlError(
                "wallet_maps_multiple_accounts", f"{field}.wallet_address"
            )
        previous_wallet = wallet_by_account.get(account)
        if previous_wallet is not None and previous_wallet != wallet:
            raise TaskNodeUnlError(
                "account_maps_multiple_wallets", f"{field}.account_id"
            )
        account_by_wallet[wallet] = account
        wallet_by_account[account] = wallet
    return account_by_wallet, wallet_by_account


def funding_wallet_account_maps(
    document: object,
) -> tuple[dict[str, str], dict[str, str]]:
    """Return the validated one-to-one wallet/account map for edge evidence."""

    row = _parse_document_header(
        document,
        schema=FUNDING_TRANSFER_INPUT_SCHEMA,
        field="funding_transfers",
        required=(
            "value_asset",
            "history_complete_from_ledger_genesis",
            "window_complete",
            "wallet_accounts",
            "transfers",
        ),
    )
    return _wallet_accounts(row["wallet_accounts"])


def _transfer_record(
    value: object,
    index: int,
    *,
    value_asset: str,
    window_end: datetime,
) -> _Transfer:
    field = f"funding_transfers.transfers[{index}]"
    row = require_closed_keys(
        value,
        required=(
            "tx_hash",
            "ledger_index",
            "transaction_index",
            "close_time",
            "source_wallet_address",
            "target_wallet_address",
            "asset",
            "value_units",
        ),
        field=field,
    )
    tx_hash = _require_lower_hex(
        row["tx_hash"], f"{field}.tx_hash", byte_length=_SHA256_BYTES
    )
    ledger_index = require_int(
        row["ledger_index"], f"{field}.ledger_index", minimum=1
    )
    transaction_index = require_int(
        row["transaction_index"],
        f"{field}.transaction_index",
        minimum=0,
    )
    if (
        ledger_index > _MAX_LEDGER_INDEX
        or transaction_index > _MAX_LEDGER_INDEX
    ):
        raise TaskNodeUnlError("ledger_position_out_of_range", field)
    close_time = _canonical_timestamp(
        row["close_time"], f"{field}.close_time"
    )
    if close_time > window_end:
        raise TaskNodeUnlError(
            "transfer_after_window_end", f"{field}.close_time"
        )
    source = _require_identifier(
        row["source_wallet_address"],
        f"{field}.source_wallet_address",
    )
    target = _require_identifier(
        row["target_wallet_address"],
        f"{field}.target_wallet_address",
    )
    if source == target:
        raise TaskNodeUnlError("self_funding_transfer", field)
    if row["asset"] != value_asset:
        raise TaskNodeUnlError("funding_asset_mismatch", f"{field}.asset")
    value_units = require_int(
        row["value_units"], f"{field}.value_units", minimum=1
    )
    return _Transfer(
        tx_hash=tx_hash,
        ledger_index=ledger_index,
        transaction_index=transaction_index,
        close_time=close_time,
        source_wallet=source,
        target_wallet=target,
        value_units=value_units,
    )


def _funding_evidence_id(
    pair: tuple[str, str],
    reasons: Sequence[str],
    record_ids: Sequence[str],
) -> str:
    document = {
        "kind": "funding",
        "pair": list(pair),
        "qualification_reasons": list(sorted(reasons)),
        "source_record_ids": list(sorted(record_ids)),
    }
    return "funding:" + hashlib.sha256(
        canonical_json_bytes(document)
    ).hexdigest()


def extract_funding_edges(
    document: object,
    exclusion_document: object,
) -> ExtractedEdges:
    """Extract first-funder and strict-majority funding relations."""

    row = _parse_document_header(
        document,
        schema=FUNDING_TRANSFER_INPUT_SCHEMA,
        field="funding_transfers",
        required=(
            "value_asset",
            "history_complete_from_ledger_genesis",
            "window_complete",
            "wallet_accounts",
            "transfers",
        ),
    )
    window = _parse_window(row["window"], "funding_transfers.window")
    exclusions = _parse_exclusions(exclusion_document, window)
    value_asset = _require_identifier(
        row["value_asset"], "funding_transfers.value_asset"
    )
    if not _require_bool(
        row["history_complete_from_ledger_genesis"],
        "funding_transfers.history_complete_from_ledger_genesis",
    ):
        raise TaskNodeUnlError(
            "incomplete_funding_history",
            "funding_transfers.history_complete_from_ledger_genesis",
        )
    if not _require_bool(
        row["window_complete"], "funding_transfers.window_complete"
    ):
        raise TaskNodeUnlError(
            "incomplete_funding_window",
            "funding_transfers.window_complete",
        )
    account_by_wallet, _wallet_by_account = _wallet_accounts(
        row["wallet_accounts"]
    )

    values = _require_array(
        row["transfers"], "funding_transfers.transfers"
    )
    records: dict[str, bytes] = {}
    tx_hash_by_position: dict[tuple[int, int], str] = {}
    transfers: list[_Transfer] = []
    for index, value in enumerate(values):
        transfer = _transfer_record(
            value,
            index,
            value_asset=value_asset,
            window_end=window.end,
        )
        encoded = canonical_json_bytes(value)
        previous = records.get(transfer.tx_hash)
        if previous is not None and previous != encoded:
            raise TaskNodeUnlError(
                "conflicting_transfer_record", transfer.tx_hash
            )
        records[transfer.tx_hash] = encoded
        ledger_position = (
            transfer.ledger_index,
            transfer.transaction_index,
        )
        previous_hash = tx_hash_by_position.get(ledger_position)
        if previous_hash is not None and previous_hash != transfer.tx_hash:
            raise TaskNodeUnlError(
                "conflicting_ledger_position",
                f"{ledger_position[0]}:{ledger_position[1]}",
            )
        tx_hash_by_position[ledger_position] = transfer.tx_hash
        transfers.append(transfer)
    transfers = sorted(set(transfers), key=lambda item: item.position)

    reasons_by_pair: dict[tuple[str, str], set[str]] = {}
    records_by_pair: dict[tuple[str, str], set[str]] = {}

    first_inbound: dict[str, _Transfer] = {}
    for transfer in transfers:
        first_inbound.setdefault(transfer.target_wallet, transfer)
    for target, transfer in sorted(first_inbound.items()):
        source = transfer.source_wallet
        if source in exclusions or target in exclusions:
            continue
        if source not in account_by_wallet or target not in account_by_wallet:
            continue
        pair = tuple(
            sorted((account_by_wallet[source], account_by_wallet[target]))
        )
        reasons_by_pair.setdefault(pair, set()).add(
            f"first_funder:{source}->{target}"
        )
        records_by_pair.setdefault(pair, set()).add(transfer.tx_hash)

    inbound_totals: dict[str, int] = {}
    inbound_by_source: dict[tuple[str, str], int] = {}
    inbound_records: dict[tuple[str, str], set[str]] = {}
    all_inbound_records: dict[str, set[str]] = {}
    for transfer in transfers:
        if not window.start <= transfer.close_time <= window.end:
            continue
        inbound_totals[transfer.target_wallet] = (
            inbound_totals.get(transfer.target_wallet, 0)
            + transfer.value_units
        )
        key = (transfer.source_wallet, transfer.target_wallet)
        inbound_by_source[key] = (
            inbound_by_source.get(key, 0) + transfer.value_units
        )
        inbound_records.setdefault(key, set()).add(transfer.tx_hash)
        all_inbound_records.setdefault(
            transfer.target_wallet, set()
        ).add(transfer.tx_hash)

    for (source, target), contribution in sorted(
        inbound_by_source.items()
    ):
        if contribution * 2 <= inbound_totals[target]:
            continue
        if source in exclusions or target in exclusions:
            continue
        if source not in account_by_wallet or target not in account_by_wallet:
            continue
        pair = tuple(
            sorted((account_by_wallet[source], account_by_wallet[target]))
        )
        reasons_by_pair.setdefault(pair, set()).add(
            f"majority_inflow:{source}->{target}"
        )
        records_by_pair.setdefault(pair, set()).update(
            all_inbound_records[target]
        )

    edges: list[TrustEdge] = []
    provenance: list[EdgeProvenance] = []
    for pair, reasons in sorted(reasons_by_pair.items()):
        record_ids = tuple(sorted(records_by_pair[pair]))
        reason_ids = tuple(sorted(reasons))
        evidence_id = _funding_evidence_id(
            pair, reason_ids, record_ids
        )
        edge = TrustEdge(pair[0], pair[1], "funding", evidence_id)
        edges.append(edge)
        provenance.append(
            EdgeProvenance(
                kind=edge.kind,
                source=edge.source,
                target=edge.target,
                evidence_id=evidence_id,
                source_record_ids=record_ids,
                qualification_reasons=reason_ids,
            )
        )
    return _canonical_extracted(edges, provenance)


def extract_public_edges(
    *,
    vouch_ledger: object,
    cowork_pointers: object,
    funding_transfers: object,
    funding_exclusions: object,
    expected_window_end: datetime,
) -> EdgeExtractionResult:
    """Extract all public sources atomically, holding on any invalid input."""

    try:
        vouch_row = _parse_document_header(
            vouch_ledger,
            schema=VOUCH_LEDGER_INPUT_SCHEMA,
            field="vouch_ledger",
            required=("memos",),
        )
        cowork_row = _parse_document_header(
            cowork_pointers,
            schema=COWORK_POINTER_INPUT_SCHEMA,
            field="cowork_pointers",
            required=("pointers",),
        )
        funding_row = _parse_document_header(
            funding_transfers,
            schema=FUNDING_TRANSFER_INPUT_SCHEMA,
            field="funding_transfers",
            required=(
                "value_asset",
                "history_complete_from_ledger_genesis",
                "window_complete",
                "wallet_accounts",
                "transfers",
            ),
        )
        windows = (
            _parse_window(vouch_row["window"], "vouch_ledger.window"),
            _parse_window(cowork_row["window"], "cowork_pointers.window"),
            _parse_window(
                funding_row["window"], "funding_transfers.window"
            ),
        )
        if len(set(windows)) != 1:
            raise TaskNodeUnlError(
                "edge_window_mismatch", "edge_sources.window"
            )
        if windows[0].end != expected_window_end:
            raise TaskNodeUnlError(
                "edge_window_mismatch", "policy_evidence.evaluation_end"
            )
        extracted = (
            extract_vouch_edges(
                vouch_ledger,
                wallet_accounts=funding_row["wallet_accounts"],
            ),
            extract_cowork_edges(
                cowork_pointers,
                wallet_accounts=funding_row["wallet_accounts"],
            ),
            extract_funding_edges(
                funding_transfers, funding_exclusions
            ),
        )
    except TaskNodeUnlError as error:
        reason = error.code
        if error.detail:
            reason = f"{reason}:{error.detail}"
        return EdgeExtractionResult(
            status="hold",
            hold_reasons=(reason,),
            edges=(),
            provenance=(),
        )

    edges = [
        edge
        for source in extracted
        for edge in source.edges
    ]
    provenance = [
        item
        for source in extracted
        for item in source.provenance
    ]
    canonical = _canonical_extracted(edges, provenance)
    return EdgeExtractionResult(
        status="extracted",
        hold_reasons=(),
        edges=canonical.edges,
        provenance=canonical.provenance,
    )
