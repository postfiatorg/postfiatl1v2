"""Loopback Task Node withdrawal queue backed by a durable SQLite journal."""

import argparse
from contextlib import closing
import json
import os
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import re
import sqlite3
import threading
import time

from exit_payout import process, readiness

BASE = "/solana/withdrawals"
BURN = re.compile("[0-9a-f]{96}")
COLUMNS = {"amount_atoms", "recipient", "burn_height", "solana_tx", "error", "detail"}


def parse_request(data):
    request = json.loads(data)
    if not isinstance(request, dict) or set(request) != {"burn_tx_id"}:
        raise ValueError("expected a burn_tx_id")
    burn = request["burn_tx_id"]
    if not isinstance(burn, str) or not BURN.fullmatch(burn):
        raise ValueError("invalid burn_tx_id")
    return burn


class Journal:
    def __init__(self, path):
        self.path = path
        with closing(self.connect()) as db:
            db.execute("""CREATE TABLE IF NOT EXISTS jobs (
                burn_tx_id TEXT PRIMARY KEY, stage TEXT NOT NULL, detail TEXT, error TEXT,
                amount_atoms TEXT, recipient TEXT, burn_height INTEGER, solana_tx TEXT)""")

    def connect(self):
        db = sqlite3.connect(self.path, timeout=10)
        db.row_factory = sqlite3.Row
        return db

    def get(self, burn):
        with closing(self.connect()) as db:
            row = db.execute(
                "SELECT * FROM jobs WHERE burn_tx_id=?", (burn,)
            ).fetchone()
            return dict(row) if row else None

    def enqueue(self, burn):
        with closing(self.connect()) as db, db:
            db.execute("BEGIN IMMEDIATE")
            row = db.execute(
                "SELECT stage FROM jobs WHERE burn_tx_id=?", (burn,)
            ).fetchone()
            if row is None or row[0] == "failed":
                active = db.execute(
                    "SELECT count(*) FROM jobs WHERE stage NOT IN ('paid','failed')"
                ).fetchone()[0]
                if active >= 32:
                    raise OverflowError("withdrawal queue is full")
                db.execute(
                    """INSERT INTO jobs(burn_tx_id,stage) VALUES(?,'queued')
                    ON CONFLICT(burn_tx_id) DO UPDATE SET stage='queued', error=NULL, detail=NULL""",
                    (burn,),
                )
            return dict(
                db.execute("SELECT * FROM jobs WHERE burn_tx_id=?", (burn,)).fetchone()
            )

    def update(self, burn, stage, **fields):
        if fields.keys() - COLUMNS:
            raise ValueError("unknown job field")
        fields["stage"] = stage
        sets = ", ".join(k + "=?" for k in fields)
        with closing(self.connect()) as db, db:
            db.execute(
                f"UPDATE jobs SET {sets} WHERE burn_tx_id=?", (*fields.values(), burn)
            )

    def pending(self):
        with closing(self.connect()) as db:
            row = db.execute(
                "SELECT burn_tx_id FROM jobs WHERE stage NOT IN ('paid','failed') ORDER BY rowid LIMIT 1"
            ).fetchone()
            return row[0] if row else None


def process_next(journal, directory):
    burn = journal.pending()
    if burn is None:
        return False
    try:
        process(
            burn,
            directory / burn,
            lambda stage, **kw: journal.update(burn, stage, **kw),
        )
    except Exception as error:
        journal.update(burn, "failed", error=str(error)[:400] or type(error).__name__)
    return True


def worker(journal, directory, wake):
    while True:
        if not process_next(journal, directory):
            wake.wait(2)
            wake.clear()


class Handler(BaseHTTPRequestHandler):
    def setup(self):
        super().setup()
        self.connection.settimeout(8)

    def reply(self, code, value):
        data = json.dumps(value).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if self.path == BASE + "/readiness":
            with self.server.ready_lock:
                if time.monotonic() >= self.server.valid_until:
                    self.server.ready = readiness()
                    self.server.valid_until = time.monotonic() + 5
                return self.reply(200, self.server.ready)
        prefix = BASE + "/"
        if not self.path.startswith(prefix):
            return self.reply(404, dict(error="unknown path"))
        burn = self.path[len(prefix) :]
        if not BURN.fullmatch(burn):
            return self.reply(400, dict(error="invalid burn_tx_id"))
        row = self.server.journal.get(burn)
        return (
            self.reply(200, row) if row else self.reply(404, dict(error="unknown job"))
        )

    def do_POST(self):
        if self.path != BASE:
            return self.reply(404, dict(error="unknown path"))
        if self.headers.get_content_type() != "application/json":
            return self.reply(415, dict(error="expected application/json"))
        size = self.headers.get("Content-Length")
        if size is None:
            return self.reply(411, dict(error="length required"))
        if not re.fullmatch(r"[0-9]{1,9}", size):
            return self.reply(400, dict(error="invalid length"))
        if int(size) > 256:
            return self.reply(413, dict(error="body too large"))
        try:
            row = self.server.journal.enqueue(parse_request(self.rfile.read(int(size))))
        except (UnicodeError, ValueError) as error:
            return self.reply(400, dict(error=str(error)))
        except OverflowError as error:
            return self.reply(429, dict(error=str(error)))
        self.server.wake.set()
        self.reply(200 if row["stage"] == "paid" else 202, row)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--port", type=int, default=18089)
    p.add_argument(
        "--state", type=Path, default=Path.home() / ".local/state/postfiat/sol-exit"
    )
    args = p.parse_args()
    os.umask(0o077)
    args.state.mkdir(parents=True, exist_ok=True)
    import fcntl

    with (
        (args.state / "worker.lock").open("a") as lock,
        ThreadingHTTPServer(("127.0.0.1", args.port), Handler) as server,
    ):
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        server.journal = Journal(args.state / "jobs.sqlite")
        server.ready_lock = threading.Lock()
        server.valid_until = 0
        server.wake = threading.Event()
        threading.Thread(
            target=worker, args=(server.journal, args.state, server.wake), daemon=True
        ).start()
        server.serve_forever()


if __name__ == "__main__":
    main()
