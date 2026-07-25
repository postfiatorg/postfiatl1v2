"""Durable, conservative ≤$5/run and ≤$20 lifetime real-value budget."""

from __future__ import annotations

from contextlib import contextmanager
import json
import os
from pathlib import Path
import sqlite3
import stat
import threading
import time
from typing import Any, Iterator, Mapping

from .authorization import ValueAuthorization, verify_value_authorization
from .policy import (
    ExecutionMode,
    MainnetQuoteView,
    RealValuePolicy,
    RealValuePolicyError,
)


BUDGET_SCHEMA_VERSION = 1


class BudgetError(RealValuePolicyError):
    """A budget mutation is invalid, unsafe, or exceeds a dust cap."""


def _encode_evidence(value: Mapping[str, Any], name: str) -> bytes:
    try:
        encoded = json.dumps(
            dict(value),
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        ).encode("ascii")
    except (TypeError, UnicodeEncodeError) as error:
        raise BudgetError(f"{name} is not canonical ASCII JSON") from error
    if len(encoded) > 16 * 1024:
        raise BudgetError(f"{name} is oversized")
    forbidden = ("preimage", "secret", "macaroon", "seed", "private_key")
    if any(marker.encode() in encoded.lower() for marker in forbidden):
        raise BudgetError(f"{name} contains secret-bearing fields")
    return encoded


class RealValueBudget:
    """One-writer SQLite ledger; spent ceilings are never released."""

    def __init__(self, path: str | Path, policy: RealValuePolicy) -> None:
        self.path = Path(path)
        if str(self.path) == ":memory:":
            raise ValueError("real-value budget requires a durable file")
        self.path.parent.mkdir(parents=True, mode=0o700, exist_ok=True)
        parent = self.path.parent.stat()
        if (
            parent.st_uid != os.geteuid()
            or parent.st_mode & (stat.S_IRWXG | stat.S_IRWXO)
        ):
            raise BudgetError(
                "budget directory must be coordinator-owned and mode 0700"
            )
        if self.path.exists():
            metadata = self.path.lstat()
            if (
                not stat.S_ISREG(metadata.st_mode)
                or stat.S_ISLNK(metadata.st_mode)
                or metadata.st_uid != os.geteuid()
                or metadata.st_mode & (stat.S_IRWXG | stat.S_IRWXO)
            ):
                raise BudgetError(
                    "budget database must be a coordinator-owned mode-0600 file"
                )
        else:
            descriptor = os.open(
                self.path,
                os.O_CREAT | os.O_EXCL | os.O_WRONLY | os.O_CLOEXEC,
                0o600,
            )
            os.close(descriptor)
        self.policy = policy
        self._lock = threading.RLock()
        self._connection = sqlite3.connect(
            str(self.path),
            timeout=30.0,
            isolation_level=None,
            check_same_thread=False,
        )
        self._connection.row_factory = sqlite3.Row
        self._connection.execute("PRAGMA foreign_keys = ON")
        self._connection.execute("PRAGMA busy_timeout = 30000")
        if str(self._connection.execute("PRAGMA journal_mode = WAL").fetchone()[0]).lower() != "wal":
            self._connection.close()
            raise BudgetError("SQLite WAL mode is required")
        self._connection.execute("PRAGMA synchronous = FULL")
        self._initialize()
        self._secure_sqlite_files()

    def _secure_sqlite_files(self) -> None:
        for candidate in (
            self.path,
            Path(f"{self.path}-wal"),
            Path(f"{self.path}-shm"),
        ):
            if not candidate.exists():
                continue
            metadata = candidate.lstat()
            if (
                not stat.S_ISREG(metadata.st_mode)
                or stat.S_ISLNK(metadata.st_mode)
                or metadata.st_uid != os.geteuid()
            ):
                raise BudgetError("budget SQLite artifact failed ownership checks")
            os.chmod(candidate, 0o600)

    def close(self) -> None:
        with self._lock:
            if self._connection is not None:
                self._connection.execute("PRAGMA wal_checkpoint(FULL)")
                self._connection.close()
                self._connection = None  # type: ignore[assignment]

    def __enter__(self) -> "RealValueBudget":
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

    def _initialize(self) -> None:
        with self._lock:
            version = self._connection.execute("PRAGMA user_version").fetchone()[0]
            if version not in (0, BUDGET_SCHEMA_VERSION):
                raise BudgetError(f"unsupported budget schema version {version}")
            self._connection.executescript(
                """
                BEGIN IMMEDIATE;
                CREATE TABLE IF NOT EXISTS budget (
                    singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
                    policy_id TEXT NOT NULL,
                    reserved_usd_e8 INTEGER NOT NULL CHECK(reserved_usd_e8 >= 0),
                    spent_usd_e8 INTEGER NOT NULL CHECK(spent_usd_e8 >= 0),
                    reserved_count INTEGER NOT NULL CHECK(reserved_count >= 0),
                    spent_count INTEGER NOT NULL CHECK(spent_count >= 0)
                );
                CREATE TABLE IF NOT EXISTS authorizations (
                    authorization_id TEXT PRIMARY KEY,
                    policy_id TEXT NOT NULL,
                    category TEXT NOT NULL,
                    quote_sha256 TEXT NOT NULL UNIQUE,
                    swap_id TEXT NOT NULL UNIQUE,
                    direction TEXT NOT NULL,
                    principal_msat INTEGER NOT NULL CHECK(principal_msat >= 0),
                    max_fee_msat INTEGER NOT NULL CHECK(max_fee_msat >= 0),
                    max_all_in_usd_e8 INTEGER NOT NULL CHECK(max_all_in_usd_e8 > 0),
                    expires_unix INTEGER NOT NULL,
                    envelope_sha256 TEXT NOT NULL,
                    state TEXT NOT NULL CHECK(state IN ('RESERVED', 'SPENT', 'RELEASED')),
                    created_unix INTEGER NOT NULL,
                    updated_unix INTEGER NOT NULL,
                    terminal_evidence_json BLOB
                );
                PRAGMA user_version = 1;
                COMMIT;
                """
            )
            row = self._connection.execute(
                "SELECT policy_id FROM budget WHERE singleton = 1"
            ).fetchone()
            if row is None:
                self._connection.execute(
                    """
                    INSERT INTO budget(
                        singleton, policy_id, reserved_usd_e8, spent_usd_e8,
                        reserved_count, spent_count
                    ) VALUES(1, ?, 0, 0, 0, 0)
                    """,
                    (self.policy.policy_id,),
                )
            elif row["policy_id"] != self.policy.policy_id:
                raise BudgetError("budget file is bound to a different policy")
            if self._connection.execute("PRAGMA quick_check").fetchone()[0] != "ok":
                raise BudgetError("SQLite budget quick_check failed")

    @staticmethod
    def _row(row: sqlite3.Row) -> dict[str, Any]:
        return {
            "authorization_id": row["authorization_id"],
            "policy_id": row["policy_id"],
            "category": row["category"],
            "quote_sha256": row["quote_sha256"],
            "swap_id": row["swap_id"],
            "direction": row["direction"],
            "principal_msat": int(row["principal_msat"]),
            "max_fee_msat": int(row["max_fee_msat"]),
            "max_all_in_usd_e8": int(row["max_all_in_usd_e8"]),
            "expires_unix": int(row["expires_unix"]),
            "state": row["state"],
            "created_unix": int(row["created_unix"]),
            "updated_unix": int(row["updated_unix"]),
        }

    def _capacity_check(
        self, reserved: int, spent: int, requested: int
    ) -> None:
        if requested > self.policy.max_per_run_usd_e8:
            raise BudgetError("authorization exceeds per-run cap")
        if reserved + spent + requested > self.policy.max_lifetime_usd_e8:
            raise BudgetError("authorization exceeds remaining lifetime budget")

    def preview(self, authorization: ValueAuthorization) -> dict[str, Any]:
        with self._lock:
            budget = self._connection.execute(
                "SELECT * FROM budget WHERE singleton = 1"
            ).fetchone()
        if budget is None:
            raise BudgetError("budget singleton is absent")
        reserved = int(budget["reserved_usd_e8"])
        spent = int(budget["spent_usd_e8"])
        self._capacity_check(reserved, spent, authorization.max_all_in_usd_e8)
        return {
            "would_fit": True,
            "mutated": False,
            "mode": self.policy.mode.value,
            "requested_usd_e8": authorization.max_all_in_usd_e8,
            "reserved_usd_e8": reserved,
            "spent_usd_e8": spent,
            "remaining_after_usd_e8": (
                self.policy.max_lifetime_usd_e8
                - reserved
                - spent
                - authorization.max_all_in_usd_e8
            ),
        }

    def reserve(
        self,
        authorization_envelope: Mapping[str, Any],
        *,
        quote: MainnetQuoteView | None = None,
        now_unix: int | None = None,
    ) -> dict[str, Any]:
        """Consume a signed permit into RESERVED before any remote side effect."""

        if self.policy.mode is not ExecutionMode.ARMED:
            raise BudgetError("real-value policy is DRY_RUN; reservation is disabled")
        now = int(time.time()) if now_unix is None else now_unix
        authorization = verify_value_authorization(
            authorization_envelope,
            self.policy,
            quote=quote,
            now_unix=now,
        )
        with self._transaction() as connection:
            existing = connection.execute(
                "SELECT * FROM authorizations WHERE authorization_id = ?",
                (authorization.authorization_id,),
            ).fetchone()
            if existing is not None:
                if existing["envelope_sha256"] != authorization.envelope_sha256:
                    raise BudgetError("authorization id was reused with different content")
                return self._row(existing)
            collision = connection.execute(
                """
                SELECT authorization_id FROM authorizations
                WHERE quote_sha256 = ? OR swap_id = ?
                """,
                (authorization.quote_sha256, authorization.swap_id),
            ).fetchone()
            if collision is not None:
                raise BudgetError("quote or swap already has another authorization")
            budget = connection.execute(
                "SELECT * FROM budget WHERE singleton = 1"
            ).fetchone()
            if budget is None:
                raise BudgetError("budget singleton is absent")
            reserved = int(budget["reserved_usd_e8"])
            spent = int(budget["spent_usd_e8"])
            self._capacity_check(
                reserved, spent, authorization.max_all_in_usd_e8
            )
            connection.execute(
                """
                INSERT INTO authorizations(
                    authorization_id, policy_id, category, quote_sha256,
                    swap_id, direction, principal_msat, max_fee_msat,
                    max_all_in_usd_e8, expires_unix, envelope_sha256,
                    state, created_unix, updated_unix
                ) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'RESERVED', ?, ?)
                """,
                (
                    authorization.authorization_id,
                    authorization.policy_id,
                    authorization.category,
                    authorization.quote_sha256,
                    authorization.swap_id,
                    authorization.direction,
                    authorization.principal_msat,
                    authorization.max_fee_msat,
                    authorization.max_all_in_usd_e8,
                    authorization.expires_unix,
                    authorization.envelope_sha256,
                    now,
                    now,
                ),
            )
            connection.execute(
                """
                UPDATE budget
                SET reserved_usd_e8 = reserved_usd_e8 + ?,
                    reserved_count = reserved_count + 1
                WHERE singleton = 1
                """,
                (authorization.max_all_in_usd_e8,),
            )
            row = connection.execute(
                "SELECT * FROM authorizations WHERE authorization_id = ?",
                (authorization.authorization_id,),
            ).fetchone()
            if row is None:
                raise BudgetError("reserved authorization disappeared")
            return self._row(row)

    def mark_spent(
        self,
        authorization_id: str,
        *,
        terminal_evidence: Mapping[str, Any],
        now_unix: int | None = None,
    ) -> dict[str, Any]:
        """Conservatively charge the authorized ceiling, never the lower actual."""

        now = int(time.time()) if now_unix is None else now_unix
        encoded = _encode_evidence(terminal_evidence, "terminal evidence")
        with self._transaction() as connection:
            row = connection.execute(
                "SELECT * FROM authorizations WHERE authorization_id = ?",
                (authorization_id,),
            ).fetchone()
            if row is None:
                raise BudgetError("unknown authorization")
            if row["state"] == "SPENT":
                if bytes(row["terminal_evidence_json"] or b"") != encoded:
                    raise BudgetError("spent authorization evidence changed")
                return self._row(row)
            if row["state"] != "RESERVED":
                raise BudgetError("only a reserved authorization can be spent")
            ceiling = int(row["max_all_in_usd_e8"])
            connection.execute(
                """
                UPDATE authorizations
                SET state = 'SPENT', updated_unix = ?, terminal_evidence_json = ?
                WHERE authorization_id = ?
                """,
                (now, encoded, authorization_id),
            )
            cursor = connection.execute(
                """
                UPDATE budget
                SET reserved_usd_e8 = reserved_usd_e8 - ?,
                    spent_usd_e8 = spent_usd_e8 + ?,
                    reserved_count = reserved_count - 1,
                    spent_count = spent_count + 1
                WHERE singleton = 1 AND reserved_usd_e8 >= ?
                    AND reserved_count >= 1
                """,
                (ceiling, ceiling, ceiling),
            )
            if cursor.rowcount != 1:
                raise BudgetError("budget spend would underflow")
            updated = connection.execute(
                "SELECT * FROM authorizations WHERE authorization_id = ?",
                (authorization_id,),
            ).fetchone()
            if updated is None:
                raise BudgetError("spent authorization disappeared")
            return self._row(updated)

    def release_unspent(
        self,
        authorization_id: str,
        *,
        no_value_evidence: Mapping[str, Any],
        now_unix: int | None = None,
    ) -> dict[str, Any]:
        """Release only with secret-free evidence that no value side effect occurred."""

        now = int(time.time()) if now_unix is None else now_unix
        if no_value_evidence.get("value_moved") is not False:
            raise BudgetError("release requires literal value_moved=false")
        encoded = _encode_evidence(no_value_evidence, "no-value evidence")
        with self._transaction() as connection:
            row = connection.execute(
                "SELECT * FROM authorizations WHERE authorization_id = ?",
                (authorization_id,),
            ).fetchone()
            if row is None:
                raise BudgetError("unknown authorization")
            if row["state"] == "RELEASED":
                if bytes(row["terminal_evidence_json"] or b"") != encoded:
                    raise BudgetError("released authorization evidence changed")
                return self._row(row)
            if row["state"] != "RESERVED":
                raise BudgetError("spent authorization cannot be released")
            ceiling = int(row["max_all_in_usd_e8"])
            connection.execute(
                """
                UPDATE authorizations
                SET state = 'RELEASED', updated_unix = ?, terminal_evidence_json = ?
                WHERE authorization_id = ?
                """,
                (now, encoded, authorization_id),
            )
            cursor = connection.execute(
                """
                UPDATE budget
                SET reserved_usd_e8 = reserved_usd_e8 - ?,
                    reserved_count = reserved_count - 1
                WHERE singleton = 1 AND reserved_usd_e8 >= ?
                    AND reserved_count >= 1
                """,
                (ceiling, ceiling),
            )
            if cursor.rowcount != 1:
                raise BudgetError("budget release would underflow")
            updated = connection.execute(
                "SELECT * FROM authorizations WHERE authorization_id = ?",
                (authorization_id,),
            ).fetchone()
            if updated is None:
                raise BudgetError("released authorization disappeared")
            return self._row(updated)

    def summary(self) -> dict[str, Any]:
        with self._lock:
            row = self._connection.execute(
                "SELECT * FROM budget WHERE singleton = 1"
            ).fetchone()
            accounting = self._connection.execute(
                """
                SELECT state, COUNT(*) AS item_count,
                    COALESCE(SUM(max_all_in_usd_e8), 0) AS total_usd_e8
                FROM authorizations
                GROUP BY state
                """
            ).fetchall()
        if row is None:
            raise BudgetError("budget singleton is absent")
        reserved = int(row["reserved_usd_e8"])
        spent = int(row["spent_usd_e8"])
        by_state = {
            item["state"]: (
                int(item["item_count"]),
                int(item["total_usd_e8"]),
            )
            for item in accounting
        }
        if by_state.get("RESERVED", (0, 0)) != (
            int(row["reserved_count"]),
            reserved,
        ):
            raise BudgetError("reserved budget accounting is inconsistent")
        if by_state.get("SPENT", (0, 0)) != (
            int(row["spent_count"]),
            spent,
        ):
            raise BudgetError("spent budget accounting is inconsistent")
        if reserved + spent > self.policy.max_lifetime_usd_e8:
            raise BudgetError("budget accounting exceeds lifetime cap")
        return {
            "schema": "postfiat.lightning_real_value_budget.v1",
            "policy_id": self.policy.policy_id,
            "mode": self.policy.mode.value,
            "per_run_cap_usd_e8": self.policy.max_per_run_usd_e8,
            "lifetime_cap_usd_e8": self.policy.max_lifetime_usd_e8,
            "reserved_usd_e8": reserved,
            "spent_usd_e8": spent,
            "remaining_usd_e8": self.policy.max_lifetime_usd_e8 - reserved - spent,
            "reserved_count": int(row["reserved_count"]),
            "spent_count": int(row["spent_count"]),
        }

    def authorization_for_swap(self, swap_id: str) -> dict[str, Any] | None:
        if type(swap_id) is not str or not swap_id:
            raise BudgetError("swap_id is required")
        with self._lock:
            row = self._connection.execute(
                "SELECT * FROM authorizations WHERE swap_id = ?",
                (swap_id,),
            ).fetchone()
        return None if row is None else self._row(row)

    def reserved_swap_ids(self, *, limit: int = 256) -> tuple[str, ...]:
        """Return bounded durable work whose value authorization is still open."""

        if type(limit) is not int or limit < 1 or limit > 4096:
            raise BudgetError("reserved authorization scan limit is invalid")
        with self._lock:
            rows = self._connection.execute(
                """
                SELECT swap_id FROM authorizations
                WHERE state = 'RESERVED'
                ORDER BY created_unix, authorization_id
                LIMIT ?
                """,
                (limit,),
            ).fetchall()
        return tuple(str(row["swap_id"]) for row in rows)
