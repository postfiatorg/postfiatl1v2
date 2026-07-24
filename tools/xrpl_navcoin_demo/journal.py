"""Durable idempotency journal for cross-ledger side effects."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import sqlite3
from typing import Any, Callable


class IdempotencyConflict(RuntimeError):
    pass


def _canonical(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False)


class EffectJournal:
    def __init__(self, path: Path) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        self.connection = sqlite3.connect(path, isolation_level=None)
        self.connection.execute("PRAGMA journal_mode=WAL")
        self.connection.execute("PRAGMA synchronous=FULL")
        self.connection.execute(
            """
            CREATE TABLE IF NOT EXISTS effects(
              effect_key TEXT PRIMARY KEY,
              request_hash TEXT NOT NULL,
              status TEXT NOT NULL CHECK(status IN ('INTENT','FINAL')),
              result_json TEXT
            )
            """
        )

    def close(self) -> None:
        self.connection.execute("PRAGMA wal_checkpoint(FULL)")
        self.connection.close()

    def execute(
        self, effect_key: str, request: Any, operation: Callable[[], Any]
    ) -> tuple[Any, bool]:
        encoded = _canonical(request)
        digest = hashlib.sha256(encoded.encode()).hexdigest()
        self.connection.execute("BEGIN IMMEDIATE")
        try:
            row = self.connection.execute(
                "SELECT request_hash,status,result_json FROM effects WHERE effect_key=?",
                (effect_key,),
            ).fetchone()
            if row:
                if row[0] != digest:
                    raise IdempotencyConflict("effect key reused with another request")
                if row[1] == "FINAL":
                    self.connection.execute("COMMIT")
                    return json.loads(row[2]), True
                raise RuntimeError("durable intent requires ledger reconciliation")
            self.connection.execute(
                "INSERT INTO effects VALUES(?,?,?,NULL)",
                (effect_key, digest, "INTENT"),
            )
            self.connection.execute("COMMIT")
        except BaseException:
            self.connection.execute("ROLLBACK")
            raise

        result = operation()
        result_json = _canonical(result)
        self.connection.execute("BEGIN IMMEDIATE")
        try:
            self.connection.execute(
                "UPDATE effects SET status='FINAL',result_json=? WHERE effect_key=?",
                (result_json, effect_key),
            )
            self.connection.execute("COMMIT")
        except BaseException:
            self.connection.execute("ROLLBACK")
            raise
        return result, False

