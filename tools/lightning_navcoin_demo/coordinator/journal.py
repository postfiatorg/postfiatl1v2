"""Crash-safe SQLite journal for the synthetic swap coordinator.

The journal makes internal intent durable before an external side effect is
attempted.  It cannot make a remote Lightning/PFTL call exactly-once; adapters
must submit the durable ``effect_key`` as their own idempotency key and safely
query/replay after a crash.
"""

from __future__ import annotations

from contextlib import contextmanager
from dataclasses import dataclass
from enum import Enum
import hashlib
import hmac
import json
import os
from pathlib import Path
import sqlite3
import stat
import threading
import time
from typing import Any, Callable, Iterator, Mapping

from .protocol import SecretPreimage
from .signing import encode_signed_quote, parse_signed_quote, verify_signed_quote


SCHEMA_VERSION = 1
MAX_PUBLIC_JSON_BYTES = 64 * 1024
MAX_COLLECTION_ITEMS = 256
MAX_JSON_DEPTH = 10
MAX_TEXT_BYTES = 16 * 1024
MAX_SQLITE_INTEGER = (1 << 63) - 1
FORBIDDEN_PUBLIC_KEYS = frozenset(
    {
        "s",
        "secret",
        "preimage",
        "r_preimage",
        "payment_preimage",
        "fulfillment",
    }
)


class JournalError(RuntimeError):
    """Base coordinator journal error."""


class InvalidTransition(JournalError):
    """The requested state edge is absent from the protocol graph."""


class IdempotencyConflict(JournalError):
    """An idempotency key was reused for a different request."""


class ExposureLimitExceeded(JournalError):
    """Admission would exceed a principal or aggregate exposure cap."""


class JournalCorruption(JournalError):
    """Persisted state violates an internal journal invariant."""


class SecretMaterialRejected(JournalError):
    """Secret material was offered to a public log/evidence field."""


class SwapState(str, Enum):
    QUOTED = "QUOTED"
    QUOTE_EXPIRED = "QUOTE_EXPIRED"
    ABORTED_NO_VALUE = "ABORTED_NO_VALUE"
    PFTL_LOCK_SUBMITTED = "PFTL_LOCK_SUBMITTED"
    LOCK_FAILED = "LOCK_FAILED"
    PFTL_LOCK_FINAL = "PFTL_LOCK_FINAL"
    LN_IN_FLIGHT = "LN_IN_FLIGHT"
    LN_SETTLED = "LN_SETTLED"
    PFTL_FINISH_FINAL = "PFTL_FINISH_FINAL"
    REFUND_ELIGIBLE = "REFUND_ELIGIBLE"
    PFTL_CANCEL_FINAL = "PFTL_CANCEL_FINAL"


TERMINAL_STATES = frozenset(
    {
        SwapState.QUOTE_EXPIRED,
        SwapState.ABORTED_NO_VALUE,
        SwapState.LOCK_FAILED,
        SwapState.PFTL_FINISH_FINAL,
        SwapState.PFTL_CANCEL_FINAL,
    }
)
LEGAL_TRANSITIONS: dict[SwapState, frozenset[SwapState]] = {
    SwapState.QUOTED: frozenset(
        {
            SwapState.QUOTE_EXPIRED,
            SwapState.ABORTED_NO_VALUE,
            SwapState.PFTL_LOCK_SUBMITTED,
        }
    ),
    SwapState.QUOTE_EXPIRED: frozenset(),
    SwapState.ABORTED_NO_VALUE: frozenset(),
    SwapState.PFTL_LOCK_SUBMITTED: frozenset(
        {
            SwapState.ABORTED_NO_VALUE,
            SwapState.LOCK_FAILED,
            SwapState.PFTL_LOCK_FINAL,
        }
    ),
    SwapState.LOCK_FAILED: frozenset(),
    SwapState.PFTL_LOCK_FINAL: frozenset(
        {SwapState.LN_IN_FLIGHT, SwapState.REFUND_ELIGIBLE}
    ),
    # A route can fail after an outgoing payment was attempted but before it
    # settled.  Refund is forbidden once LN_SETTLED is durable.
    SwapState.LN_IN_FLIGHT: frozenset(
        {SwapState.LN_SETTLED, SwapState.REFUND_ELIGIBLE}
    ),
    SwapState.LN_SETTLED: frozenset({SwapState.PFTL_FINISH_FINAL}),
    SwapState.REFUND_ELIGIBLE: frozenset({SwapState.PFTL_CANCEL_FINAL}),
    SwapState.PFTL_FINISH_FINAL: frozenset(),
    SwapState.PFTL_CANCEL_FINAL: frozenset(),
}


@dataclass(frozen=True)
class ExposureLimits:
    per_principal_atoms: int
    aggregate_atoms: int

    def __post_init__(self) -> None:
        for field, value in (
            ("per_principal_atoms", self.per_principal_atoms),
            ("aggregate_atoms", self.aggregate_atoms),
        ):
            if type(value) is not int or value <= 0 or value > MAX_SQLITE_INTEGER:
                raise ValueError(f"{field} must be a positive uint63")
        if self.per_principal_atoms > self.aggregate_atoms:
            raise ValueError("per-principal cap exceeds aggregate cap")


@dataclass(frozen=True)
class SideEffectSpec:
    effect_key: str
    kind: str
    payload: Mapping[str, Any]

    def __post_init__(self) -> None:
        _bounded_identifier(self.effect_key, "effect_key")
        _bounded_identifier(self.kind, "side-effect kind")


@dataclass(frozen=True, repr=False)
class QuoteIntent:
    request_id: str
    created: bool
    invoice_preimage: SecretPreimage | None
    completed_swap_id: str | None

    def __repr__(self) -> str:
        return (
            "QuoteIntent("
            f"request_id={self.request_id!r}, created={self.created!r}, "
            "invoice_preimage=<redacted>, "
            f"completed_swap_id={self.completed_swap_id!r})"
        )


def _bounded_identifier(value: Any, field: str, *, maximum: int = 256) -> str:
    if (
        type(value) is not str
        or not value
        or len(value) > maximum
        or not value.isascii()
        or any(ord(character) < 0x21 or ord(character) > 0x7E for character in value)
    ):
        raise ValueError(f"{field} must be printable non-whitespace ASCII")
    return value


def _normalize_public(value: Any, *, depth: int = 0) -> Any:
    if depth > MAX_JSON_DEPTH:
        raise ValueError("public JSON exceeds maximum depth")
    if value is None or type(value) is bool:
        return value
    if type(value) is int:
        if value < -MAX_SQLITE_INTEGER or value > MAX_SQLITE_INTEGER:
            raise ValueError("public JSON integer is outside int63")
        return value
    if type(value) is str:
        if len(value.encode("utf-8")) > MAX_TEXT_BYTES:
            raise ValueError("public JSON string exceeds size limit")
        return value
    if type(value) is list or type(value) is tuple:
        if len(value) > MAX_COLLECTION_ITEMS:
            raise ValueError("public JSON list exceeds item limit")
        return [_normalize_public(item, depth=depth + 1) for item in value]
    if isinstance(value, Mapping):
        if len(value) > MAX_COLLECTION_ITEMS:
            raise ValueError("public JSON object exceeds item limit")
        normalized: dict[str, Any] = {}
        for key, item in value.items():
            if type(key) is not str or not key or len(key) > 256:
                raise ValueError("public JSON object key is invalid")
            if key.lower() in FORBIDDEN_PUBLIC_KEYS:
                raise SecretMaterialRejected(
                    f"secret-bearing field {key!r} is forbidden in public journal data"
                )
            normalized[key] = _normalize_public(item, depth=depth + 1)
        return normalized
    raise ValueError(f"unsupported public JSON type: {type(value).__name__}")


def _canonical_public_json(value: Any) -> bytes:
    normalized = _normalize_public(value)
    encoded = json.dumps(
        normalized,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
        allow_nan=False,
    ).encode("ascii")
    if len(encoded) > MAX_PUBLIC_JSON_BYTES:
        raise ValueError("public JSON exceeds encoded size limit")
    return encoded


def _request_hash(value: Any) -> str:
    return hashlib.sha256(
        b"postfiat.lightning_coordinator.request.v1\x00"
        + _canonical_public_json(value)
    ).hexdigest()


def redact_for_log(value: Any, *, known_secrets: tuple[SecretPreimage, ...] = ()) -> Any:
    """Best-effort redaction for general structured logs.

    Journal APIs reject explicit secret-bearing fields.  This helper additionally
    replaces known secret hex if it appears inside otherwise free-form text.
    """

    secret_strings: list[str] = []
    for secret in known_secrets:
        raw_hex = secret.protocol_hex()
        secret_strings.extend((raw_hex, raw_hex.upper()))

    def walk(item: Any, depth: int = 0) -> Any:
        if depth > MAX_JSON_DEPTH:
            return "<redacted:depth>"
        if isinstance(item, Mapping):
            output: dict[str, Any] = {}
            for key, child in item.items():
                key_text = str(key)
                if key_text.lower() in FORBIDDEN_PUBLIC_KEYS:
                    output[key_text] = "<redacted>"
                else:
                    output[key_text] = walk(child, depth + 1)
            return output
        if isinstance(item, (list, tuple)):
            return [walk(child, depth + 1) for child in item[:MAX_COLLECTION_ITEMS]]
        if isinstance(item, bytes):
            return "<redacted:bytes>"
        if isinstance(item, SecretPreimage):
            return "<redacted>"
        if isinstance(item, str):
            redacted = item
            for secret_string in secret_strings:
                redacted = redacted.replace(secret_string, "<redacted>")
            return redacted
        return item

    return walk(value)


class CoordinatorJournal:
    """One-writer-at-a-time durable swap journal."""

    def __init__(
        self,
        path: str | Path,
        limits: ExposureLimits,
        *,
        clock_ns: Callable[[], int] = time.time_ns,
    ) -> None:
        self.path = Path(path)
        if str(self.path) == ":memory:":
            raise ValueError("durable journal requires a filesystem path")
        self.path.parent.mkdir(parents=True, exist_ok=True)
        if self.path.parent.is_symlink() or not self.path.parent.is_dir():
            raise JournalError("journal parent must be a real directory")
        self.path.parent.chmod(0o700)
        if self.path.is_symlink():
            raise JournalError("journal database may not be a symbolic link")
        self.limits = limits
        self._clock_ns = clock_ns
        self._lock = threading.RLock()
        self._connection = sqlite3.connect(
            str(self.path),
            timeout=30.0,
            isolation_level=None,
            # Every access is serialized by ``self._lock``. This permits the
            # service to hand work between worker threads without exposing one
            # connection to concurrent SQLite calls.
            check_same_thread=False,
        )
        self._connection.row_factory = sqlite3.Row
        self._connection.execute("PRAGMA foreign_keys = ON")
        self._connection.execute("PRAGMA busy_timeout = 30000")
        mode = self._connection.execute("PRAGMA journal_mode = WAL").fetchone()[0]
        if str(mode).lower() != "wal":
            self._connection.close()
            raise JournalError("SQLite WAL mode is required")
        self._connection.execute("PRAGMA synchronous = FULL")
        self._secure_storage_permissions()
        self._initialize_schema()
        self._secure_storage_permissions()

    def _secure_storage_permissions(self) -> None:
        """Keep the secret-bearing database and SQLite sidecars owner-only."""

        for path in (
            self.path,
            Path(f"{self.path}-wal"),
            Path(f"{self.path}-shm"),
        ):
            try:
                metadata = os.lstat(path)
            except FileNotFoundError:
                continue
            if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
                raise JournalError(
                    f"journal storage path is not a regular file: {path.name}"
                )
            os.chmod(path, 0o600, follow_symlinks=False)

    def close(self) -> None:
        with self._lock:
            if self._connection is not None:
                self._secure_storage_permissions()
                self._connection.execute("PRAGMA wal_checkpoint(FULL)")
                self._connection.close()
                self._connection = None  # type: ignore[assignment]

    def __enter__(self) -> "CoordinatorJournal":
        return self

    def __exit__(self, *_exc: object) -> None:
        self.close()

    @contextmanager
    def _transaction(self) -> Iterator[sqlite3.Connection]:
        with self._lock:
            connection = self._connection
            connection.execute("BEGIN IMMEDIATE")
            try:
                yield connection
                connection.execute("COMMIT")
            except BaseException:
                connection.execute("ROLLBACK")
                raise
            finally:
                self._secure_storage_permissions()

    def _initialize_schema(self) -> None:
        with self._lock:
            connection = self._connection
            version = connection.execute("PRAGMA user_version").fetchone()[0]
            if version not in (0, SCHEMA_VERSION):
                raise JournalError(
                    f"unsupported coordinator journal schema version {version}"
                )
            try:
                connection.executescript(
                    """
                BEGIN IMMEDIATE;
                CREATE TABLE IF NOT EXISTS swaps (
                    swap_id TEXT PRIMARY KEY,
                    payment_hash TEXT NOT NULL UNIQUE,
                    principal TEXT NOT NULL,
                    direction TEXT NOT NULL,
                    asset_id TEXT NOT NULL,
                    exposure_atoms INTEGER NOT NULL
                        CHECK(exposure_atoms > 0),
                    state TEXT NOT NULL,
                    state_version INTEGER NOT NULL CHECK(state_version >= 0),
                    exposure_released INTEGER NOT NULL DEFAULT 0
                        CHECK(exposure_released IN (0, 1)),
                    signed_quote BLOB NOT NULL,
                    created_ns INTEGER NOT NULL,
                    updated_ns INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS events (
                    event_key TEXT PRIMARY KEY,
                    swap_id TEXT NOT NULL REFERENCES swaps(swap_id),
                    event_ordinal INTEGER NOT NULL,
                    from_state TEXT,
                    to_state TEXT NOT NULL,
                    request_hash TEXT NOT NULL,
                    evidence_json BLOB NOT NULL,
                    created_ns INTEGER NOT NULL,
                    UNIQUE(swap_id, event_ordinal)
                );
                CREATE TABLE IF NOT EXISTS side_effects (
                    effect_key TEXT PRIMARY KEY,
                    swap_id TEXT NOT NULL REFERENCES swaps(swap_id),
                    kind TEXT NOT NULL,
                    request_hash TEXT NOT NULL,
                    payload_json BLOB NOT NULL,
                    status TEXT NOT NULL
                        CHECK(status IN ('PENDING', 'SUCCEEDED', 'FAILED_TERMINAL')),
                    attempt_count INTEGER NOT NULL DEFAULT 0
                        CHECK(attempt_count >= 0),
                    created_ns INTEGER NOT NULL,
                    updated_ns INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS side_effect_attempts (
                    attempt_key TEXT PRIMARY KEY,
                    effect_key TEXT NOT NULL REFERENCES side_effects(effect_key),
                    request_hash TEXT NOT NULL,
                    outcome TEXT NOT NULL
                        CHECK(outcome IN ('SUCCEEDED', 'RETRYABLE_FAILURE', 'TERMINAL_FAILURE')),
                    result_json BLOB NOT NULL,
                    created_ns INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS side_effect_checkpoints (
                    checkpoint_key TEXT PRIMARY KEY,
                    effect_key TEXT NOT NULL REFERENCES side_effects(effect_key),
                    request_hash TEXT NOT NULL,
                    evidence_json BLOB NOT NULL,
                    created_ns INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS principal_exposure (
                    principal TEXT PRIMARY KEY,
                    active_atoms INTEGER NOT NULL CHECK(active_atoms >= 0),
                    active_swaps INTEGER NOT NULL CHECK(active_swaps >= 0)
                );
                CREATE TABLE IF NOT EXISTS aggregate_exposure (
                    singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
                    active_atoms INTEGER NOT NULL CHECK(active_atoms >= 0),
                    active_swaps INTEGER NOT NULL CHECK(active_swaps >= 0)
                );
                INSERT OR IGNORE INTO aggregate_exposure(
                    singleton, active_atoms, active_swaps
                ) VALUES(1, 0, 0);
                CREATE TABLE IF NOT EXISTS secrets (
                    swap_id TEXT NOT NULL REFERENCES swaps(swap_id),
                    secret_name TEXT NOT NULL,
                    secret_value BLOB NOT NULL,
                    created_ns INTEGER NOT NULL,
                    PRIMARY KEY(swap_id, secret_name)
                );
                CREATE TABLE IF NOT EXISTS quote_intents (
                    request_id TEXT PRIMARY KEY,
                    request_hash TEXT NOT NULL,
                    request_json BLOB NOT NULL,
                    invoice_preimage BLOB,
                    completed_swap_id TEXT REFERENCES swaps(swap_id),
                    created_ns INTEGER NOT NULL,
                    updated_ns INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS swaps_state_idx ON swaps(state);
                CREATE INDEX IF NOT EXISTS events_swap_idx
                    ON events(swap_id, event_ordinal);
                CREATE INDEX IF NOT EXISTS side_effects_status_idx
                    ON side_effects(status, created_ns);
                PRAGMA user_version = 1;
                COMMIT;
                """
                )
            except BaseException:
                # ``executescript`` owns this explicit transaction. Roll back
                # only when it has not already done so for a parse error.
                if connection.in_transaction:
                    connection.execute("ROLLBACK")
                raise
            quick_check = connection.execute("PRAGMA quick_check").fetchone()[0]
            if quick_check != "ok":
                raise JournalCorruption(f"SQLite quick_check failed: {quick_check}")

    def reserve_quote_intent(
        self,
        request_id: str,
        request: Mapping[str, Any],
        *,
        create_invoice_preimage: bool,
    ) -> QuoteIntent:
        """Persist request identity and any invoice secret before calling LND."""

        request_id = _bounded_identifier(request_id, "request_id")
        request_json = _canonical_public_json(request)
        request_hash = _request_hash(request)
        now = self._clock_ns()
        with self._transaction() as connection:
            existing = connection.execute(
                "SELECT * FROM quote_intents WHERE request_id = ?",
                (request_id,),
            ).fetchone()
            if existing is not None:
                if (
                    existing["request_hash"] != request_hash
                    or bytes(existing["request_json"]) != request_json
                ):
                    raise IdempotencyConflict(
                        "quote request id was reused with different content"
                    )
                raw = existing["invoice_preimage"]
                if create_invoice_preimage != (raw is not None):
                    raise IdempotencyConflict(
                        "quote request direction changed secret requirements"
                    )
                return QuoteIntent(
                    request_id=request_id,
                    created=False,
                    invoice_preimage=(
                        None if raw is None else SecretPreimage(bytes(raw))
                    ),
                    completed_swap_id=existing["completed_swap_id"],
                )
            secret = (
                SecretPreimage.generate() if create_invoice_preimage else None
            )
            connection.execute(
                """
                INSERT INTO quote_intents(
                    request_id, request_hash, request_json, invoice_preimage,
                    completed_swap_id, created_ns, updated_ns
                ) VALUES(?, ?, ?, ?, NULL, ?, ?)
                """,
                (
                    request_id,
                    request_hash,
                    request_json,
                    (
                        None
                        if secret is None
                        else secret.reveal_for_protocol()
                    ),
                    now,
                    now,
                ),
            )
            return QuoteIntent(
                request_id=request_id,
                created=True,
                invoice_preimage=secret,
                completed_swap_id=None,
            )

    def complete_quote_intent(self, request_id: str, swap_id: str) -> None:
        request_id = _bounded_identifier(request_id, "request_id")
        swap_id = _bounded_identifier(swap_id, "swap_id")
        if request_id != swap_id:
            raise IdempotencyConflict("quote intent must complete to its request id")
        now = self._clock_ns()
        with self._transaction() as connection:
            intent = connection.execute(
                "SELECT completed_swap_id FROM quote_intents WHERE request_id = ?",
                (request_id,),
            ).fetchone()
            if intent is None:
                raise JournalError("quote intent is absent")
            if intent["completed_swap_id"] not in (None, swap_id):
                raise IdempotencyConflict(
                    "quote intent was completed by another swap"
                )
            swap = connection.execute(
                "SELECT swap_id FROM swaps WHERE swap_id = ?",
                (swap_id,),
            ).fetchone()
            if swap is None:
                raise JournalError("cannot complete quote intent before swap admission")
            connection.execute(
                """
                UPDATE quote_intents
                SET completed_swap_id = ?, updated_ns = ?
                WHERE request_id = ?
                """,
                (swap_id, now, request_id),
            )

    def create_swap(
        self,
        principal: str,
        signed_quote: Mapping[str, Any],
        *,
        expected_public_key: bytes | None = None,
        secret: SecretPreimage | None = None,
        secret_name: str = "invoice_preimage",
    ) -> dict[str, Any]:
        """Admit a quote and reserve exposure atomically.

        Repeating the byte-identical signed quote for the same principal is
        idempotent.  ``secret`` is stored in a separate, non-exported table in
        the same transaction; this is synthetic-demo storage, not production
        key sealing.
        """

        principal = _bounded_identifier(principal, "principal", maximum=128)
        quote = verify_signed_quote(
            signed_quote, expected_public_key=expected_public_key
        )
        signed_quote_bytes = encode_signed_quote(signed_quote)
        swap_id = quote["swap_id"]
        exposure = quote["pftl_amount_atoms"]
        if exposure > self.limits.per_principal_atoms:
            raise ExposureLimitExceeded("swap exceeds per-principal exposure cap")
        event_key = f"quote:{swap_id}"
        event_request_hash = _request_hash(
            {
                "principal": principal,
                "signed_quote_sha256": hashlib.sha256(signed_quote_bytes).hexdigest(),
                "to_state": SwapState.QUOTED.value,
            }
        )
        now = self._clock_ns()
        with self._transaction() as connection:
            existing = connection.execute(
                "SELECT * FROM swaps WHERE swap_id = ?", (swap_id,)
            ).fetchone()
            if existing is not None:
                if (
                    existing["principal"] != principal
                    or bytes(existing["signed_quote"]) != signed_quote_bytes
                ):
                    raise IdempotencyConflict(
                        "swap_id was reused for a different quote or principal"
                    )
                event = connection.execute(
                    "SELECT request_hash FROM events WHERE event_key = ?",
                    (event_key,),
                ).fetchone()
                if event is None or event["request_hash"] != event_request_hash:
                    raise JournalCorruption("idempotent quote event is inconsistent")
                if secret is not None:
                    self._store_secret_tx(
                        connection, swap_id, secret_name, secret, now=now
                    )
                return self._swap_row(existing)
            if quote["quote_expires_unix"] * 1_000_000_000 <= now:
                raise JournalError("cannot admit an expired quote")

            principal_row = connection.execute(
                "SELECT active_atoms, active_swaps FROM principal_exposure "
                "WHERE principal = ?",
                (principal,),
            ).fetchone()
            principal_atoms = (
                int(principal_row["active_atoms"]) if principal_row else 0
            )
            aggregate_row = connection.execute(
                "SELECT active_atoms, active_swaps FROM aggregate_exposure "
                "WHERE singleton = 1"
            ).fetchone()
            if aggregate_row is None:
                raise JournalCorruption("aggregate exposure row is absent")
            aggregate_atoms = int(aggregate_row["active_atoms"])
            if principal_atoms + exposure > self.limits.per_principal_atoms:
                raise ExposureLimitExceeded("per-principal exposure cap exceeded")
            if aggregate_atoms + exposure > self.limits.aggregate_atoms:
                raise ExposureLimitExceeded("aggregate exposure cap exceeded")

            try:
                connection.execute(
                    """
                    INSERT INTO swaps(
                        swap_id, payment_hash, principal, direction, asset_id,
                        exposure_atoms, state, state_version, exposure_released,
                        signed_quote, created_ns, updated_ns
                    ) VALUES(?, ?, ?, ?, ?, ?, ?, 0, 0, ?, ?, ?)
                    """,
                    (
                        swap_id,
                        quote["payment_hash"],
                        principal,
                        quote["direction"],
                        quote["pftl_asset_id"],
                        exposure,
                        SwapState.QUOTED.value,
                        signed_quote_bytes,
                        now,
                        now,
                    ),
                )
            except sqlite3.IntegrityError as error:
                if "payment_hash" in str(error):
                    raise IdempotencyConflict(
                        "payment hash was already admitted under another swap"
                    ) from error
                raise
            connection.execute(
                """
                INSERT INTO events(
                    event_key, swap_id, event_ordinal, from_state, to_state,
                    request_hash, evidence_json, created_ns
                ) VALUES(?, ?, 0, NULL, ?, ?, ?, ?)
                """,
                (
                    event_key,
                    swap_id,
                    SwapState.QUOTED.value,
                    event_request_hash,
                    _canonical_public_json({"admitted": True}),
                    now,
                ),
            )
            connection.execute(
                """
                INSERT INTO principal_exposure(principal, active_atoms, active_swaps)
                VALUES(?, ?, 1)
                ON CONFLICT(principal) DO UPDATE SET
                    active_atoms = active_atoms + excluded.active_atoms,
                    active_swaps = active_swaps + 1
                """,
                (principal, exposure),
            )
            connection.execute(
                """
                UPDATE aggregate_exposure
                SET active_atoms = active_atoms + ?, active_swaps = active_swaps + 1
                WHERE singleton = 1
                """,
                (exposure,),
            )
            if secret is not None:
                self._store_secret_tx(
                    connection, swap_id, secret_name, secret, now=now
                )
            inserted = connection.execute(
                "SELECT * FROM swaps WHERE swap_id = ?", (swap_id,)
            ).fetchone()
            if inserted is None:
                raise JournalCorruption("newly admitted swap is absent")
            return self._swap_row(inserted)

    def advance(
        self,
        swap_id: str,
        target: SwapState | str,
        event_key: str,
        *,
        evidence: Mapping[str, Any] | None = None,
        side_effect: SideEffectSpec | None = None,
        secret_write: tuple[str, SecretPreimage] | None = None,
    ) -> dict[str, Any]:
        """Apply one legal state edge with an optional durable side-effect intent."""

        swap_id = _bounded_identifier(swap_id, "swap_id")
        event_key = _bounded_identifier(event_key, "event_key")
        try:
            target_state = target if isinstance(target, SwapState) else SwapState(target)
        except ValueError as error:
            raise InvalidTransition(f"unknown target state: {target}") from error
        evidence_value = {} if evidence is None else evidence
        evidence_json = _canonical_public_json(evidence_value)
        request_value: dict[str, Any] = {
            "swap_id": swap_id,
            "target": target_state.value,
            "evidence_sha256": hashlib.sha256(evidence_json).hexdigest(),
        }
        if side_effect is not None:
            payload_json = _canonical_public_json(side_effect.payload)
            request_value["side_effect"] = {
                "effect_key": side_effect.effect_key,
                "kind": side_effect.kind,
                "payload_sha256": hashlib.sha256(payload_json).hexdigest(),
            }
        if secret_write is not None:
            secret_name, secret = secret_write
            _bounded_identifier(secret_name, "secret_name")
            if not isinstance(secret, SecretPreimage):
                raise ValueError("secret_write requires SecretPreimage")
            request_value["secret_commitment"] = hashlib.sha256(
                secret.reveal_for_protocol()
            ).hexdigest()
        event_request_hash = _request_hash(request_value)
        now = self._clock_ns()

        with self._transaction() as connection:
            existing_event = connection.execute(
                "SELECT request_hash FROM events WHERE event_key = ?", (event_key,)
            ).fetchone()
            if existing_event is not None:
                if existing_event["request_hash"] != event_request_hash:
                    raise IdempotencyConflict(
                        "event key was reused for a different transition request"
                    )
                row = connection.execute(
                    "SELECT * FROM swaps WHERE swap_id = ?", (swap_id,)
                ).fetchone()
                if row is None:
                    raise JournalCorruption("event refers to a missing swap")
                if secret_write is not None:
                    self._store_secret_tx(
                        connection,
                        swap_id,
                        secret_write[0],
                        secret_write[1],
                        now=now,
                    )
                return self._swap_row(row)

            row = connection.execute(
                "SELECT * FROM swaps WHERE swap_id = ?", (swap_id,)
            ).fetchone()
            if row is None:
                raise JournalError("unknown swap")
            current = SwapState(row["state"])
            if target_state not in LEGAL_TRANSITIONS[current]:
                raise InvalidTransition(
                    f"illegal transition {current.value} -> {target_state.value}"
                )
            self._assert_no_known_secret_tx(
                connection, swap_id, evidence_json
            )
            if side_effect is not None:
                self._assert_no_known_secret_tx(
                    connection, swap_id, payload_json
                )
            if secret_write is not None:
                secret_hex = (
                    secret_write[1].protocol_hex().encode("ascii")
                )
                if secret_hex in evidence_json.lower() or (
                    side_effect is not None and secret_hex in payload_json.lower()
                ):
                    raise SecretMaterialRejected(
                        "new preimage appeared in public journal data"
                    )
            next_version = int(row["state_version"]) + 1
            if side_effect is not None:
                self._insert_side_effect_tx(
                    connection,
                    swap_id,
                    side_effect,
                    payload_json=payload_json,
                    now=now,
                )
            if secret_write is not None:
                self._store_secret_tx(
                    connection,
                    swap_id,
                    secret_write[0],
                    secret_write[1],
                    now=now,
                )
            connection.execute(
                """
                INSERT INTO events(
                    event_key, swap_id, event_ordinal, from_state, to_state,
                    request_hash, evidence_json, created_ns
                ) VALUES(?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    event_key,
                    swap_id,
                    next_version,
                    current.value,
                    target_state.value,
                    event_request_hash,
                    evidence_json,
                    now,
                ),
            )
            cursor = connection.execute(
                """
                UPDATE swaps
                SET state = ?, state_version = ?, updated_ns = ?
                WHERE swap_id = ? AND state = ? AND state_version = ?
                """,
                (
                    target_state.value,
                    next_version,
                    now,
                    swap_id,
                    current.value,
                    row["state_version"],
                ),
            )
            if cursor.rowcount != 1:
                raise JournalCorruption("swap compare-and-set failed")
            if target_state in TERMINAL_STATES:
                self._release_exposure_tx(connection, row)
            updated = connection.execute(
                "SELECT * FROM swaps WHERE swap_id = ?", (swap_id,)
            ).fetchone()
            if updated is None:
                raise JournalCorruption("advanced swap is absent")
            return self._swap_row(updated)

    def abort_unattempted_side_effect(
        self,
        swap_id: str,
        effect_key: str,
        event_key: str,
        *,
        evidence: Mapping[str, Any],
    ) -> dict[str, Any]:
        """Atomically abort a never-attempted PFTL create and release exposure.

        This narrow operation exists so an expired, unauthorized quote cannot
        consume admission exposure forever. It cannot classify a transport
        ambiguity: the side effect must still be PENDING with exactly zero
        attempts, and no external call is made by this method.
        """

        swap_id = _bounded_identifier(swap_id, "swap_id")
        effect_key = _bounded_identifier(effect_key, "effect_key")
        event_key = _bounded_identifier(event_key, "event_key")
        evidence_json = _canonical_public_json(evidence)
        request_hash = _request_hash(
            {
                "swap_id": swap_id,
                "effect_key": effect_key,
                "target": SwapState.ABORTED_NO_VALUE.value,
                "evidence_sha256": hashlib.sha256(evidence_json).hexdigest(),
            }
        )
        now = self._clock_ns()
        with self._transaction() as connection:
            prior = connection.execute(
                "SELECT request_hash FROM events WHERE event_key = ?",
                (event_key,),
            ).fetchone()
            if prior is not None:
                if prior["request_hash"] != request_hash:
                    raise IdempotencyConflict(
                        "abort event key was reused with different evidence"
                    )
                row = connection.execute(
                    "SELECT * FROM swaps WHERE swap_id = ?", (swap_id,)
                ).fetchone()
                if row is None:
                    raise JournalCorruption("abort event refers to missing swap")
                return self._swap_row(row)

            row = connection.execute(
                "SELECT * FROM swaps WHERE swap_id = ?", (swap_id,)
            ).fetchone()
            if row is None:
                raise JournalError("unknown swap")
            if SwapState(row["state"]) is not SwapState.PFTL_LOCK_SUBMITTED:
                raise InvalidTransition(
                    "only a submitted, unattempted lock intent can be aborted"
                )
            effect = connection.execute(
                "SELECT * FROM side_effects WHERE effect_key = ?",
                (effect_key,),
            ).fetchone()
            if (
                effect is None
                or effect["swap_id"] != swap_id
                or effect["kind"] != "PFTL_ESCROW_CREATE"
            ):
                raise JournalCorruption("PFTL create intent is absent or mismatched")
            if effect["status"] != "PENDING" or int(effect["attempt_count"]) != 0:
                raise InvalidTransition(
                    "PFTL create may not be aborted after any submission attempt"
                )
            self._assert_no_known_secret_tx(
                connection, swap_id, evidence_json
            )
            next_version = int(row["state_version"]) + 1
            connection.execute(
                """
                UPDATE side_effects
                SET status = 'FAILED_TERMINAL', updated_ns = ?
                WHERE effect_key = ? AND status = 'PENDING' AND attempt_count = 0
                """,
                (now, effect_key),
            )
            connection.execute(
                """
                INSERT INTO events(
                    event_key, swap_id, event_ordinal, from_state, to_state,
                    request_hash, evidence_json, created_ns
                ) VALUES(?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    event_key,
                    swap_id,
                    next_version,
                    SwapState.PFTL_LOCK_SUBMITTED.value,
                    SwapState.ABORTED_NO_VALUE.value,
                    request_hash,
                    evidence_json,
                    now,
                ),
            )
            cursor = connection.execute(
                """
                UPDATE swaps
                SET state = ?, state_version = ?, updated_ns = ?
                WHERE swap_id = ? AND state = ? AND state_version = ?
                """,
                (
                    SwapState.ABORTED_NO_VALUE.value,
                    next_version,
                    now,
                    swap_id,
                    SwapState.PFTL_LOCK_SUBMITTED.value,
                    row["state_version"],
                ),
            )
            if cursor.rowcount != 1:
                raise JournalCorruption("abort compare-and-set failed")
            self._release_exposure_tx(connection, row)
            updated = connection.execute(
                "SELECT * FROM swaps WHERE swap_id = ?", (swap_id,)
            ).fetchone()
            if updated is None:
                raise JournalCorruption("aborted swap is absent")
            return self._swap_row(updated)

    def _insert_side_effect_tx(
        self,
        connection: sqlite3.Connection,
        swap_id: str,
        spec: SideEffectSpec,
        *,
        payload_json: bytes,
        now: int,
    ) -> None:
        side_effect_request_hash = _request_hash(
            {
                "swap_id": swap_id,
                "kind": spec.kind,
                "payload_sha256": hashlib.sha256(payload_json).hexdigest(),
            }
        )
        existing = connection.execute(
            "SELECT swap_id, request_hash FROM side_effects WHERE effect_key = ?",
            (spec.effect_key,),
        ).fetchone()
        if existing is not None:
            if (
                existing["swap_id"] != swap_id
                or existing["request_hash"] != side_effect_request_hash
            ):
                raise IdempotencyConflict(
                    "side-effect key was reused for a different request"
                )
            return
        connection.execute(
            """
            INSERT INTO side_effects(
                effect_key, swap_id, kind, request_hash, payload_json,
                status, attempt_count, created_ns, updated_ns
            ) VALUES(?, ?, ?, ?, ?, 'PENDING', 0, ?, ?)
            """,
            (
                spec.effect_key,
                swap_id,
                spec.kind,
                side_effect_request_hash,
                payload_json,
                now,
                now,
            ),
        )

    def record_side_effect_attempt(
        self,
        effect_key: str,
        attempt_key: str,
        outcome: str,
        *,
        result: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Record a retry outcome idempotently.

        ``RETRYABLE_FAILURE`` leaves the effect pending.  A crash after a remote
        success but before this call is handled by resubmitting/querying with
        the same durable ``effect_key``.
        """

        effect_key = _bounded_identifier(effect_key, "effect_key")
        attempt_key = _bounded_identifier(attempt_key, "attempt_key")
        if outcome not in {"SUCCEEDED", "RETRYABLE_FAILURE", "TERMINAL_FAILURE"}:
            raise ValueError("unsupported side-effect outcome")
        result_json = _canonical_public_json({} if result is None else result)
        attempt_request_hash = _request_hash(
            {
                "effect_key": effect_key,
                "outcome": outcome,
                "result_sha256": hashlib.sha256(result_json).hexdigest(),
            }
        )
        now = self._clock_ns()
        with self._transaction() as connection:
            prior_attempt = connection.execute(
                "SELECT request_hash FROM side_effect_attempts WHERE attempt_key = ?",
                (attempt_key,),
            ).fetchone()
            if prior_attempt is not None:
                if prior_attempt["request_hash"] != attempt_request_hash:
                    raise IdempotencyConflict(
                        "attempt key was reused for a different result"
                    )
                effect = connection.execute(
                    "SELECT * FROM side_effects WHERE effect_key = ?", (effect_key,)
                ).fetchone()
                if effect is None:
                    raise JournalCorruption("attempt refers to missing side effect")
                return self._side_effect_row(effect)

            effect = connection.execute(
                "SELECT * FROM side_effects WHERE effect_key = ?", (effect_key,)
            ).fetchone()
            if effect is None:
                raise JournalError("unknown side effect")
            self._assert_no_known_secret_tx(
                connection, effect["swap_id"], result_json
            )
            if effect["status"] != "PENDING":
                raise InvalidTransition("side effect is already terminal")
            connection.execute(
                """
                INSERT INTO side_effect_attempts(
                    attempt_key, effect_key, request_hash, outcome,
                    result_json, created_ns
                ) VALUES(?, ?, ?, ?, ?, ?)
                """,
                (
                    attempt_key,
                    effect_key,
                    attempt_request_hash,
                    outcome,
                    result_json,
                    now,
                ),
            )
            new_status = {
                "SUCCEEDED": "SUCCEEDED",
                "RETRYABLE_FAILURE": "PENDING",
                "TERMINAL_FAILURE": "FAILED_TERMINAL",
            }[outcome]
            connection.execute(
                """
                UPDATE side_effects
                SET status = ?, attempt_count = attempt_count + 1, updated_ns = ?
                WHERE effect_key = ?
                """,
                (new_status, now, effect_key),
            )
            updated = connection.execute(
                "SELECT * FROM side_effects WHERE effect_key = ?", (effect_key,)
            ).fetchone()
            if updated is None:
                raise JournalCorruption("updated side effect is absent")
            return self._side_effect_row(updated)

    def record_side_effect_checkpoint(
        self,
        effect_key: str,
        checkpoint_key: str,
        *,
        evidence: Mapping[str, Any],
    ) -> dict[str, Any]:
        """Persist one immutable pre-effect observation.

        This is used when an exact post-effect delta must be measured against a
        just-in-time consensus snapshot.  Reusing a key with different evidence
        fails closed instead of silently moving the baseline.
        """

        effect_key = _bounded_identifier(effect_key, "effect_key")
        checkpoint_key = _bounded_identifier(checkpoint_key, "checkpoint_key")
        evidence_json = _canonical_public_json(evidence)
        request_hash = _request_hash(
            {
                "effect_key": effect_key,
                "evidence_sha256": hashlib.sha256(evidence_json).hexdigest(),
            }
        )
        now = self._clock_ns()
        with self._transaction() as connection:
            effect = connection.execute(
                "SELECT effect_key, swap_id FROM side_effects WHERE effect_key = ?",
                (effect_key,),
            ).fetchone()
            if effect is None:
                raise JournalError("unknown side effect")
            self._assert_no_known_secret_tx(
                connection, effect["swap_id"], evidence_json
            )
            existing = connection.execute(
                """
                SELECT effect_key, request_hash, evidence_json, created_ns
                FROM side_effect_checkpoints WHERE checkpoint_key = ?
                """,
                (checkpoint_key,),
            ).fetchone()
            if existing is not None:
                if (
                    existing["effect_key"] != effect_key
                    or existing["request_hash"] != request_hash
                ):
                    raise IdempotencyConflict(
                        "checkpoint key was reused for different evidence"
                    )
                return {
                    "checkpoint_key": checkpoint_key,
                    "effect_key": effect_key,
                    "evidence": json.loads(
                        bytes(existing["evidence_json"]).decode("ascii")
                    ),
                    "created_ns": int(existing["created_ns"]),
                }
            connection.execute(
                """
                INSERT INTO side_effect_checkpoints(
                    checkpoint_key, effect_key, request_hash,
                    evidence_json, created_ns
                ) VALUES(?, ?, ?, ?, ?)
                """,
                (
                    checkpoint_key,
                    effect_key,
                    request_hash,
                    evidence_json,
                    now,
                ),
            )
            return {
                "checkpoint_key": checkpoint_key,
                "effect_key": effect_key,
                "evidence": dict(evidence),
                "created_ns": now,
            }

    def side_effect_checkpoint(self, checkpoint_key: str) -> dict[str, Any] | None:
        checkpoint_key = _bounded_identifier(checkpoint_key, "checkpoint_key")
        with self._lock:
            row = self._connection.execute(
                """
                SELECT checkpoint_key, effect_key, evidence_json, created_ns
                FROM side_effect_checkpoints WHERE checkpoint_key = ?
                """,
                (checkpoint_key,),
            ).fetchone()
        if row is None:
            return None
        return {
            "checkpoint_key": row["checkpoint_key"],
            "effect_key": row["effect_key"],
            "evidence": json.loads(bytes(row["evidence_json"]).decode("ascii")),
            "created_ns": int(row["created_ns"]),
        }

    @staticmethod
    def _assert_no_known_secret_tx(
        connection: sqlite3.Connection, swap_id: str, public_json: bytes
    ) -> None:
        lowered = public_json.lower()
        rows = connection.execute(
            "SELECT secret_value FROM secrets WHERE swap_id = ?", (swap_id,)
        ).fetchall()
        for row in rows:
            secret_hex = bytes(row["secret_value"]).hex().encode("ascii")
            if secret_hex in lowered:
                raise SecretMaterialRejected(
                    "known preimage appeared in public journal data"
                )

    def store_secret(
        self,
        swap_id: str,
        secret_name: str,
        secret: SecretPreimage,
    ) -> None:
        swap_id = _bounded_identifier(swap_id, "swap_id")
        secret_name = _bounded_identifier(secret_name, "secret_name")
        with self._transaction() as connection:
            self._store_secret_tx(
                connection,
                swap_id,
                secret_name,
                secret,
                now=self._clock_ns(),
            )

    def _store_secret_tx(
        self,
        connection: sqlite3.Connection,
        swap_id: str,
        secret_name: str,
        secret: SecretPreimage,
        *,
        now: int,
    ) -> None:
        secret_name = _bounded_identifier(secret_name, "secret_name")
        raw = secret.reveal_for_protocol()
        existing = connection.execute(
            "SELECT secret_value FROM secrets WHERE swap_id = ? AND secret_name = ?",
            (swap_id, secret_name),
        ).fetchone()
        if existing is not None:
            if not hmac.compare_digest(bytes(existing["secret_value"]), raw):
                raise IdempotencyConflict(
                    "secret name was reused with different material"
                )
            return
        try:
            connection.execute(
                """
                INSERT INTO secrets(swap_id, secret_name, secret_value, created_ns)
                VALUES(?, ?, ?, ?)
                """,
                (swap_id, secret_name, raw, now),
            )
        except sqlite3.IntegrityError as error:
            raise JournalError("cannot store a secret for an unknown swap") from error

    def load_secret(self, swap_id: str, secret_name: str) -> SecretPreimage:
        swap_id = _bounded_identifier(swap_id, "swap_id")
        secret_name = _bounded_identifier(secret_name, "secret_name")
        with self._lock:
            row = self._connection.execute(
                "SELECT secret_value FROM secrets WHERE swap_id = ? AND secret_name = ?",
                (swap_id, secret_name),
            ).fetchone()
        if row is None:
            raise JournalError("secret not found")
        return SecretPreimage(bytes(row["secret_value"]))

    def _release_exposure_tx(
        self, connection: sqlite3.Connection, swap_row: sqlite3.Row
    ) -> None:
        if int(swap_row["exposure_released"]) != 0:
            raise JournalCorruption("swap exposure was already released")
        principal = swap_row["principal"]
        exposure = int(swap_row["exposure_atoms"])
        principal_row = connection.execute(
            "SELECT active_atoms, active_swaps FROM principal_exposure "
            "WHERE principal = ?",
            (principal,),
        ).fetchone()
        aggregate_row = connection.execute(
            "SELECT active_atoms, active_swaps FROM aggregate_exposure "
            "WHERE singleton = 1"
        ).fetchone()
        if principal_row is None or aggregate_row is None:
            raise JournalCorruption("exposure accumulator is absent")
        if (
            int(principal_row["active_atoms"]) < exposure
            or int(principal_row["active_swaps"]) < 1
            or int(aggregate_row["active_atoms"]) < exposure
            or int(aggregate_row["active_swaps"]) < 1
        ):
            raise JournalCorruption("exposure accumulator would underflow")
        connection.execute(
            """
            UPDATE principal_exposure
            SET active_atoms = active_atoms - ?, active_swaps = active_swaps - 1
            WHERE principal = ?
            """,
            (exposure, principal),
        )
        connection.execute(
            """
            UPDATE aggregate_exposure
            SET active_atoms = active_atoms - ?, active_swaps = active_swaps - 1
            WHERE singleton = 1
            """,
            (exposure,),
        )
        connection.execute(
            "UPDATE swaps SET exposure_released = 1 WHERE swap_id = ?",
            (swap_row["swap_id"],),
        )

    def get_swap(self, swap_id: str) -> dict[str, Any]:
        swap_id = _bounded_identifier(swap_id, "swap_id")
        with self._lock:
            row = self._connection.execute(
                "SELECT * FROM swaps WHERE swap_id = ?", (swap_id,)
            ).fetchone()
        if row is None:
            raise JournalError("unknown swap")
        return self._swap_row(row)

    def side_effect(self, swap_id: str, kind: str) -> dict[str, Any] | None:
        """Read the unique side effect of ``kind`` for one swap."""

        swap_id = _bounded_identifier(swap_id, "swap_id")
        kind = _bounded_identifier(kind, "kind", maximum=64)
        with self._lock:
            rows = self._connection.execute(
                """
                SELECT * FROM side_effects
                WHERE swap_id = ? AND kind = ?
                ORDER BY created_ns, effect_key
                LIMIT 2
                """,
                (swap_id, kind),
            ).fetchall()
        if len(rows) > 1:
            raise JournalCorruption("duplicate side-effect kind for swap")
        return None if not rows else self._side_effect_row(rows[0])

    def recoverable_swaps(self) -> list[dict[str, Any]]:
        terminal_values = tuple(state.value for state in TERMINAL_STATES)
        placeholders = ", ".join("?" for _ in terminal_values)
        with self._lock:
            rows = self._connection.execute(
                f"""
                SELECT * FROM swaps
                WHERE state NOT IN ({placeholders})
                ORDER BY created_ns, swap_id
                """,
                terminal_values,
            ).fetchall()
        return [self._swap_row(row) for row in rows]

    def pending_side_effects(self) -> list[dict[str, Any]]:
        with self._lock:
            rows = self._connection.execute(
                """
                SELECT * FROM side_effects
                WHERE status = 'PENDING'
                ORDER BY created_ns, effect_key
                """
            ).fetchall()
        return [self._side_effect_row(row) for row in rows]

    def exposure(self, principal: str | None = None) -> dict[str, int]:
        with self._lock:
            if principal is None:
                row = self._connection.execute(
                    "SELECT active_atoms, active_swaps FROM aggregate_exposure "
                    "WHERE singleton = 1"
                ).fetchone()
            else:
                principal = _bounded_identifier(principal, "principal", maximum=128)
                row = self._connection.execute(
                    "SELECT active_atoms, active_swaps FROM principal_exposure "
                    "WHERE principal = ?",
                    (principal,),
                ).fetchone()
        if row is None:
            return {"active_atoms": 0, "active_swaps": 0}
        return {
            "active_atoms": int(row["active_atoms"]),
            "active_swaps": int(row["active_swaps"]),
        }

    def events(self, swap_id: str) -> list[dict[str, Any]]:
        swap_id = _bounded_identifier(swap_id, "swap_id")
        with self._lock:
            rows = self._connection.execute(
                "SELECT * FROM events WHERE swap_id = ? ORDER BY event_ordinal",
                (swap_id,),
            ).fetchall()
        return [
            {
                "event_key": row["event_key"],
                "swap_id": row["swap_id"],
                "event_ordinal": int(row["event_ordinal"]),
                "from_state": row["from_state"],
                "to_state": row["to_state"],
                "request_hash": row["request_hash"],
                "evidence": json.loads(bytes(row["evidence_json"]).decode("ascii")),
                "created_ns": int(row["created_ns"]),
            }
            for row in rows
        ]

    def export_public_audit(self) -> dict[str, Any]:
        """Return a stable audit snapshot which intentionally excludes secrets."""

        with self._lock:
            swap_rows = self._connection.execute(
                "SELECT * FROM swaps ORDER BY created_ns, swap_id"
            ).fetchall()
            event_rows = self._connection.execute(
                "SELECT swap_id FROM swaps ORDER BY created_ns, swap_id"
            ).fetchall()
            side_effect_rows = self._connection.execute(
                "SELECT * FROM side_effects ORDER BY created_ns, effect_key"
            ).fetchall()
        return {
            "schema": "postfiat.lightning_coordinator.audit.v1",
            "exposure": self.exposure(),
            "swaps": [self._swap_row(row) for row in swap_rows],
            "events": [
                event
                for row in event_rows
                for event in self.events(row["swap_id"])
            ],
            "side_effects": [
                self._side_effect_row(row) for row in side_effect_rows
            ],
        }

    @staticmethod
    def _swap_row(row: sqlite3.Row) -> dict[str, Any]:
        envelope = parse_signed_quote(bytes(row["signed_quote"]))
        return {
            "swap_id": row["swap_id"],
            "payment_hash": row["payment_hash"],
            "principal": row["principal"],
            "direction": row["direction"],
            "asset_id": row["asset_id"],
            "exposure_atoms": int(row["exposure_atoms"]),
            "state": row["state"],
            "state_version": int(row["state_version"]),
            "exposure_released": bool(row["exposure_released"]),
            "signed_quote": envelope,
            "created_ns": int(row["created_ns"]),
            "updated_ns": int(row["updated_ns"]),
        }

    @staticmethod
    def _side_effect_row(row: sqlite3.Row) -> dict[str, Any]:
        return {
            "effect_key": row["effect_key"],
            "swap_id": row["swap_id"],
            "kind": row["kind"],
            "payload": json.loads(bytes(row["payload_json"]).decode("ascii")),
            "status": row["status"],
            "attempt_count": int(row["attempt_count"]),
            "created_ns": int(row["created_ns"]),
            "updated_ns": int(row["updated_ns"]),
        }
