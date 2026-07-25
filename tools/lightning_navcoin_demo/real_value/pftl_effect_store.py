"""Durable effect-key journal for signer-isolated PFTL submissions."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import sqlite3
import threading
import time
from typing import Any, Mapping


EFFECT_KEY = re.compile(r"^[A-Za-z0-9._:-]{1,192}$")
HEX_32 = re.compile(r"^[0-9a-f]{64}$")
HEX_48 = re.compile(r"^[0-9a-f]{96}$")
STATUSES = frozenset(
    {"PLANNED", "SIGNED", "SUBMITTING", "SUCCEEDED", "REJECTED"}
)
_SECRET_MARKERS = (
    "preimage",
    "fulfillment",
    "private_key",
    "mnemonic",
    "seed",
    "macaroon",
    "wallet_password",
)
_EFFECT_COLUMNS = (
    "effect_key, kind, request_sha256, signer_address, signer_sequence, "
    "escrow_id, status, signed_artifact_path, signed_artifact_sha256, "
    "tx_id, evidence_json, created_ns, updated_ns"
)
_EFFECT_SCHEMA = """
    CREATE TABLE IF NOT EXISTS pftl_effects (
        effect_key TEXT PRIMARY KEY,
        kind TEXT NOT NULL,
        request_sha256 TEXT NOT NULL,
        signer_address TEXT NOT NULL,
        signer_sequence INTEGER,
        escrow_id TEXT NOT NULL,
        status TEXT NOT NULL,
        signed_artifact_path TEXT,
        signed_artifact_sha256 TEXT,
        tx_id TEXT,
        evidence_json TEXT,
        created_ns INTEGER NOT NULL,
        updated_ns INTEGER NOT NULL,
        CHECK(status IN ('PLANNED','SIGNED','SUBMITTING','SUCCEEDED','REJECTED'))
    )
"""


class EffectStoreError(RuntimeError):
    """The durable effect journal is invalid, conflicted, or corrupt."""


def _canonical_hash(value: Mapping[str, Any]) -> str:
    try:
        encoded = json.dumps(
            value,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        ).encode("ascii")
    except (TypeError, ValueError, UnicodeError) as error:
        raise EffectStoreError("effect request is not canonical JSON") from error
    if len(encoded) > 64 * 1024:
        raise EffectStoreError("effect request exceeds the size bound")
    return hashlib.sha256(encoded).hexdigest()


def _effect_key(value: Any) -> str:
    if type(value) is not str or EFFECT_KEY.fullmatch(value) is None:
        raise EffectStoreError("effect_key is not canonical bounded ASCII")
    return value


def _assert_secret_free(value: Any, path: str = "$") -> None:
    if isinstance(value, Mapping):
        for key, child in value.items():
            lowered = str(key).lower()
            if any(marker in lowered for marker in _SECRET_MARKERS):
                raise EffectStoreError(
                    f"secret-bearing evidence field is forbidden: {path}.{key}"
                )
            _assert_secret_free(child, f"{path}.{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _assert_secret_free(child, f"{path}[{index}]")


class PftlEffectStore:
    """SQLite/WAL journal that never persists an operation or signer secret."""

    def __init__(self, path: str | Path, *, clock_ns=time.time_ns) -> None:
        self.path = Path(path)
        self.path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        os.chmod(self.path.parent, 0o700)
        self._clock_ns = clock_ns
        self._lock = threading.RLock()
        with self._connect() as connection:
            existing = connection.execute(
                "SELECT sql FROM sqlite_master "
                "WHERE type = 'table' AND name = 'pftl_effects'"
            ).fetchone()
            if existing is None:
                connection.execute(_EFFECT_SCHEMA)
            elif "'REJECTED'" not in str(existing["sql"]):
                # Local-journal-only schema migration. It preserves every
                # immutable request/artifact row and merely adds a distinct
                # terminal state for literal consensus rejection.
                connection.execute("BEGIN IMMEDIATE")
                try:
                    connection.execute(
                        "ALTER TABLE pftl_effects RENAME TO pftl_effects_v1"
                    )
                    connection.execute(_EFFECT_SCHEMA)
                    connection.execute(
                        f"INSERT INTO pftl_effects ({_EFFECT_COLUMNS}) "
                        f"SELECT {_EFFECT_COLUMNS} FROM pftl_effects_v1"
                    )
                    connection.execute("DROP TABLE pftl_effects_v1")
                    connection.commit()
                except Exception:
                    connection.rollback()
                    raise
        os.chmod(self.path, 0o600)

    def _connect(self) -> sqlite3.Connection:
        connection = sqlite3.connect(
            self.path,
            timeout=10,
            isolation_level=None,
        )
        connection.row_factory = sqlite3.Row
        connection.execute("PRAGMA journal_mode=WAL")
        connection.execute("PRAGMA synchronous=FULL")
        connection.execute("PRAGMA foreign_keys=ON")
        connection.execute("PRAGMA busy_timeout=10000")
        return connection

    @staticmethod
    def _row(row: sqlite3.Row) -> dict[str, Any]:
        result = dict(row)
        evidence_json = result.pop("evidence_json")
        result["evidence"] = (
            None if evidence_json is None else json.loads(evidence_json)
        )
        return result

    def begin(
        self,
        *,
        effect_key: str,
        kind: str,
        request: Mapping[str, Any],
        signer_address: str,
        escrow_id: str,
        signer_sequence: int | None = None,
    ) -> dict[str, Any]:
        effect_key = _effect_key(effect_key)
        if (
            type(kind) is not str
            or not kind
            or len(kind) > 64
            or not kind.isascii()
        ):
            raise EffectStoreError("effect kind is invalid")
        if type(signer_address) is not str or not signer_address.startswith("pf"):
            raise EffectStoreError("effect signer address is invalid")
        if type(escrow_id) is not str or HEX_48.fullmatch(escrow_id) is None:
            raise EffectStoreError("effect escrow_id is invalid")
        if signer_sequence is not None and (
            type(signer_sequence) is not int
            or signer_sequence < 1
            or signer_sequence > (1 << 63) - 1
        ):
            raise EffectStoreError("effect signer sequence is invalid")
        request_sha256 = _canonical_hash(request)
        now = self._clock_ns()
        with self._lock, self._connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            existing = connection.execute(
                "SELECT * FROM pftl_effects WHERE effect_key = ?",
                (effect_key,),
            ).fetchone()
            if existing is not None:
                if (
                    existing["kind"] != kind
                    or existing["request_sha256"] != request_sha256
                    or existing["signer_address"] != signer_address
                    or existing["escrow_id"] != escrow_id
                    or (
                        signer_sequence is not None
                        and existing["signer_sequence"] not in (None, signer_sequence)
                    )
                ):
                    connection.rollback()
                    raise EffectStoreError(
                        "effect_key was reused for a different PFTL request"
                    )
                if existing["signer_sequence"] is None and signer_sequence is not None:
                    cursor = connection.execute(
                        """
                        UPDATE pftl_effects
                        SET signer_sequence = ?, updated_ns = ?
                        WHERE effect_key = ? AND signer_sequence IS NULL
                        """,
                        (signer_sequence, now, effect_key),
                    )
                    if cursor.rowcount != 1:
                        connection.rollback()
                        raise EffectStoreError("effect sequence compare-and-set failed")
                connection.commit()
                row = connection.execute(
                    "SELECT * FROM pftl_effects WHERE effect_key = ?",
                    (effect_key,),
                ).fetchone()
                if row is None:
                    raise EffectStoreError("effect disappeared after idempotent begin")
                return self._row(row)
            connection.execute(
                """
                INSERT INTO pftl_effects(
                    effect_key, kind, request_sha256, signer_address,
                    signer_sequence, escrow_id, status, created_ns, updated_ns
                ) VALUES(?, ?, ?, ?, ?, ?, 'PLANNED', ?, ?)
                """,
                (
                    effect_key,
                    kind,
                    request_sha256,
                    signer_address,
                    signer_sequence,
                    escrow_id,
                    now,
                    now,
                ),
            )
            connection.commit()
            row = connection.execute(
                "SELECT * FROM pftl_effects WHERE effect_key = ?",
                (effect_key,),
            ).fetchone()
            if row is None:
                raise EffectStoreError("new effect was not persisted")
            return self._row(row)

    def reserve_sequence(self, effect_key: str, sequence: int) -> dict[str, Any]:
        effect_key = _effect_key(effect_key)
        if type(sequence) is not int or sequence < 1 or sequence > (1 << 63) - 1:
            raise EffectStoreError("signer sequence is invalid")
        now = self._clock_ns()
        with self._lock, self._connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            row = connection.execute(
                "SELECT * FROM pftl_effects WHERE effect_key = ?",
                (effect_key,),
            ).fetchone()
            if row is None:
                connection.rollback()
                raise EffectStoreError("cannot reserve sequence for unknown effect")
            if row["signer_sequence"] not in (None, sequence):
                connection.rollback()
                raise EffectStoreError("effect already reserved a different sequence")
            if row["signer_sequence"] is None:
                connection.execute(
                    """
                    UPDATE pftl_effects
                    SET signer_sequence = ?, updated_ns = ?
                    WHERE effect_key = ? AND signer_sequence IS NULL
                    """,
                    (sequence, now, effect_key),
                )
            connection.commit()
            updated = connection.execute(
                "SELECT * FROM pftl_effects WHERE effect_key = ?",
                (effect_key,),
            ).fetchone()
            if updated is None:
                raise EffectStoreError("effect disappeared after sequence reservation")
            return self._row(updated)

    def mark_signed(
        self,
        effect_key: str,
        *,
        signed_artifact_path: Path,
        signed_artifact_sha256: str,
    ) -> dict[str, Any]:
        effect_key = _effect_key(effect_key)
        if not signed_artifact_path.is_absolute():
            raise EffectStoreError("signed artifact path must be absolute")
        if HEX_32.fullmatch(signed_artifact_sha256) is None:
            raise EffectStoreError("signed artifact SHA-256 is invalid")
        now = self._clock_ns()
        with self._lock, self._connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            row = connection.execute(
                "SELECT * FROM pftl_effects WHERE effect_key = ?",
                (effect_key,),
            ).fetchone()
            if row is None or row["signer_sequence"] is None:
                connection.rollback()
                raise EffectStoreError("effect is absent or has no reserved sequence")
            if row["status"] in {"SUCCEEDED", "REJECTED"}:
                connection.commit()
                return self._row(row)
            if row["status"] not in {"PLANNED", "SIGNED"}:
                connection.rollback()
                raise EffectStoreError(
                    "cannot replace an artifact after submission became uncertain"
                )
            if row["signed_artifact_sha256"] not in (
                None,
                signed_artifact_sha256,
            ):
                connection.rollback()
                raise EffectStoreError("effect signed artifact changed")
            connection.execute(
                """
                UPDATE pftl_effects
                SET status = 'SIGNED', signed_artifact_path = ?,
                    signed_artifact_sha256 = ?, updated_ns = ?
                WHERE effect_key = ?
                """,
                (
                    str(signed_artifact_path),
                    signed_artifact_sha256,
                    now,
                    effect_key,
                ),
            )
            connection.commit()
            updated = connection.execute(
                "SELECT * FROM pftl_effects WHERE effect_key = ?",
                (effect_key,),
            ).fetchone()
            if updated is None:
                raise EffectStoreError("effect disappeared after signing")
            return self._row(updated)

    def mark_submitting(self, effect_key: str) -> dict[str, Any]:
        effect_key = _effect_key(effect_key)
        now = self._clock_ns()
        with self._lock, self._connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            row = connection.execute(
                "SELECT * FROM pftl_effects WHERE effect_key = ?",
                (effect_key,),
            ).fetchone()
            if row is None or row["status"] not in {"SIGNED", "SUBMITTING"}:
                connection.rollback()
                raise EffectStoreError("effect is not signed for submission")
            if row["status"] == "SIGNED":
                connection.execute(
                    """
                    UPDATE pftl_effects
                    SET status = 'SUBMITTING', updated_ns = ?
                    WHERE effect_key = ? AND status = 'SIGNED'
                    """,
                    (now, effect_key),
                )
            connection.commit()
            updated = connection.execute(
                "SELECT * FROM pftl_effects WHERE effect_key = ?",
                (effect_key,),
            ).fetchone()
            if updated is None:
                raise EffectStoreError("effect disappeared before submission")
            return self._row(updated)

    def mark_succeeded(
        self,
        effect_key: str,
        *,
        tx_id: str,
        evidence: Mapping[str, Any],
    ) -> dict[str, Any]:
        effect_key = _effect_key(effect_key)
        if HEX_48.fullmatch(tx_id) is None:
            raise EffectStoreError("effect tx_id is invalid")
        if (
            evidence.get("accepted") is not True
            or evidence.get("code") != "accepted"
            or evidence.get("mutation_free") is True
        ):
            raise EffectStoreError(
                "only a literal accepted receipt can mark an effect succeeded"
            )
        _assert_secret_free(evidence)
        evidence_json = json.dumps(
            evidence,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        )
        now = self._clock_ns()
        with self._lock, self._connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            row = connection.execute(
                "SELECT * FROM pftl_effects WHERE effect_key = ?",
                (effect_key,),
            ).fetchone()
            if row is None:
                connection.rollback()
                raise EffectStoreError("cannot complete unknown effect")
            if row["status"] == "SUCCEEDED":
                if row["tx_id"] != tx_id or row["evidence_json"] != evidence_json:
                    connection.rollback()
                    raise EffectStoreError("succeeded effect evidence changed")
                connection.commit()
                return self._row(row)
            if row["status"] not in {"SIGNED", "SUBMITTING"}:
                connection.rollback()
                raise EffectStoreError(
                    "effect is not signed/submitting for accepted completion"
                )
            connection.execute(
                """
                UPDATE pftl_effects
                SET status = 'SUCCEEDED', tx_id = ?, evidence_json = ?, updated_ns = ?
                WHERE effect_key = ?
                """,
                (tx_id, evidence_json, now, effect_key),
            )
            connection.commit()
            updated = connection.execute(
                "SELECT * FROM pftl_effects WHERE effect_key = ?",
                (effect_key,),
            ).fetchone()
            if updated is None:
                raise EffectStoreError("effect disappeared after completion")
            return self._row(updated)

    def mark_rejected(
        self,
        effect_key: str,
        *,
        tx_id: str,
        evidence: Mapping[str, Any],
    ) -> dict[str, Any]:
        """Persist a zero-effect consensus rejection as a distinct terminal."""

        effect_key = _effect_key(effect_key)
        if HEX_48.fullmatch(tx_id) is None:
            raise EffectStoreError("effect tx_id is invalid")
        if (
            evidence.get("accepted") is not False
            or evidence.get("mutation_free") is not True
            or type(evidence.get("code")) is not str
            or not evidence["code"]
            or evidence["code"] == "accepted"
        ):
            raise EffectStoreError(
                "rejected effect lacks literal mutation-free rejection evidence"
            )
        _assert_secret_free(evidence)
        evidence_json = json.dumps(
            evidence,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        )
        now = self._clock_ns()
        with self._lock, self._connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            row = connection.execute(
                "SELECT * FROM pftl_effects WHERE effect_key = ?",
                (effect_key,),
            ).fetchone()
            if row is None:
                connection.rollback()
                raise EffectStoreError("cannot reject unknown effect")
            if row["status"] == "REJECTED":
                if row["tx_id"] != tx_id or row["evidence_json"] != evidence_json:
                    connection.rollback()
                    raise EffectStoreError("rejected effect evidence changed")
                connection.commit()
                return self._row(row)
            if row["status"] not in {"SIGNED", "SUBMITTING"}:
                connection.rollback()
                raise EffectStoreError(
                    "effect is not signed/submitting for rejected completion"
                )
            connection.execute(
                """
                UPDATE pftl_effects
                SET status = 'REJECTED', tx_id = ?, evidence_json = ?, updated_ns = ?
                WHERE effect_key = ?
                """,
                (tx_id, evidence_json, now, effect_key),
            )
            connection.commit()
            updated = connection.execute(
                "SELECT * FROM pftl_effects WHERE effect_key = ?",
                (effect_key,),
            ).fetchone()
            if updated is None:
                raise EffectStoreError("effect disappeared after rejection")
            return self._row(updated)

    def get(self, effect_key: str) -> dict[str, Any] | None:
        effect_key = _effect_key(effect_key)
        with self._lock, self._connect() as connection:
            row = connection.execute(
                "SELECT * FROM pftl_effects WHERE effect_key = ?",
                (effect_key,),
            ).fetchone()
            return None if row is None else self._row(row)
