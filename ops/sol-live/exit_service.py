"""Loopback Task Node withdrawal queue backed by a durable SQLite journal."""

import argparse
from contextlib import closing, ExitStack
import json
import os
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import re
from urllib.parse import parse_qs, urlsplit
import sqlite3
import threading
import time

from collections import deque, OrderedDict
from exit_payout import (
    CheckpointAdvanced,
    DestinationNotWallet,
    UnknownBurn,
    PAID_NO_HISTORY,
    prune_work,
    ASSET,
    cluster,
    covered_buckets,
    destination_refusal,
    finish_payment,
    holdings,
    ACCOUNT,
    process,
    readiness,
    rpc,
    rpc_any,
    signature,
    validate_burn,
    NOT_WALLET,
)

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
        self.path = Path(path)
        pointer = self.path.with_name("jobs.current")
        if self.path.name == "jobs.sqlite" and pointer.exists():
            name = json.loads(pointer.read_text())
            if not isinstance(name, str) or not re.fullmatch(
                r"jobs-recovered-[0-9]+\.sqlite", name
            ):
                raise ValueError("Invalid journal generation. Rebuild required.")
            self.path = self.path.with_name(name)
            if not self.path.is_file():
                raise ValueError("Missing journal generation. Rebuild required.")
        self.worker_ok = True
        self.last_prune = 0
        self.last_buffer_job = ""
        self.marker_cache = OrderedDict()
        self.marker_lock = threading.Lock()
        self.refresh_slot = threading.BoundedSemaphore(1)
        self.refresh_rate = Admissions()
        with closing(self.connect()) as db:
            db.execute("""CREATE TABLE IF NOT EXISTS jobs (
                burn_tx_id TEXT PRIMARY KEY, stage TEXT NOT NULL, detail TEXT, error TEXT,
                amount_atoms TEXT, recipient TEXT, burn_height INTEGER, solana_tx TEXT)""")

            columns = {r[1] for r in db.execute("PRAGMA table_info(jobs)")}
            for name in ("due", "retries"):
                if name not in columns:
                    db.execute(
                        f"ALTER TABLE jobs ADD COLUMN {name} INTEGER NOT NULL DEFAULT 0"
                    )
            db.execute("""CREATE TABLE IF NOT EXISTS attempts (
                burn TEXT PRIMARY KEY, signature TEXT NOT NULL, expiry INTEGER NOT NULL)""")
            db.execute("CREATE TABLE IF NOT EXISTS recovery (expiry INTEGER NOT NULL)")
            db.commit()

    def probe(self):
        with closing(self.connect()) as db:
            db.execute("BEGIN IMMEDIATE")
            db.execute("SELECT stage FROM jobs LIMIT 1").fetchone()
            db.rollback()

    def has_capacity(self):
        with closing(self.connect()) as db:
            total, active = db.execute(
                "SELECT count(*), count(*) FILTER (WHERE stage NOT IN ('paid','failed')) FROM jobs"
            ).fetchone()
        return total < 1280 and active < 32

    def attempt(self, burn):
        with closing(self.connect()) as db:
            row = db.execute(
                "SELECT signature,expiry FROM attempts WHERE burn=?", (burn,)
            ).fetchone()
            return tuple(row) if row else None

    def record_attempt(self, burn, sig, height):
        with closing(self.connect()) as db, db:
            db.execute(
                "INSERT OR REPLACE INTO attempts VALUES(?,?,?)", (burn, sig, height)
            )
            db.execute("UPDATE jobs SET stage='paying' WHERE burn_tx_id=?", (burn,))

    def waiting_for_expiry(self):
        with closing(self.connect()) as db, db:
            row = db.execute("SELECT expiry FROM recovery LIMIT 1").fetchone()
            if row is None:
                return False
            c = cluster()
            if c.call("getBlockHeight", c.level) <= row[0]:
                return True
            db.execute("DELETE FROM recovery")
            return False

    def next_due(self, burn):
        with closing(self.connect()) as db:
            return db.execute(
                "SELECT due FROM jobs WHERE burn_tx_id=?", (burn,)
            ).fetchone()[0]

    def defer(self, burn, progress=False):
        with closing(self.connect()) as db, db:
            db.execute("BEGIN IMMEDIATE")
            retries = db.execute(
                "SELECT retries FROM jobs WHERE burn_tx_id=?", (burn,)
            ).fetchone()[0]
            delay = 1 if progress else min(300, 5 * 2 ** min(retries, 6))
            db.execute(
                """UPDATE jobs SET stage='queued', retries=?,due=?,error=NULL,detail=?
                          WHERE burn_tx_id=? AND stage!='paid'""",
                (
                    0 if progress else retries + 1,
                    int(time.time()) + delay,
                    "Checkpoint advanced." if progress else "Retry scheduled.",
                    burn,
                ),
            )

    def connect(self):
        db = sqlite3.connect(self.path, timeout=10)
        db.row_factory = sqlite3.Row
        return db

    def get(self, burn):
        with closing(self.connect()) as db:
            row = db.execute(
                "SELECT * FROM jobs WHERE burn_tx_id=?", (burn,)
            ).fetchone()
            return public_row(row) if row else None

    def enqueue(self, burn, *, rebuilding=False):
        if not BURN.fullmatch(burn):
            raise ValueError("Invalid burn ID.")
        with closing(self.connect()) as db, db:
            db.execute("BEGIN IMMEDIATE")
            row = db.execute(
                "SELECT stage FROM jobs WHERE burn_tx_id=?", (burn,)
            ).fetchone()
            if row is None or row[0] == "failed":
                active = db.execute(
                    "SELECT count(*) FROM jobs WHERE stage NOT IN ('paid','failed')"
                ).fetchone()[0]
                total = db.execute("SELECT count(*) FROM jobs").fetchone()[0]
                if not rebuilding and (active >= 32 or (row is None and total >= 1280)):
                    raise OverflowError("withdrawal queue is full")
                db.execute(
                    """INSERT INTO jobs(burn_tx_id,stage,due) VALUES(?,'queued',?)
                    ON CONFLICT(burn_tx_id) DO UPDATE SET stage='queued', error=NULL, detail=NULL, due=excluded.due""",
                    (burn, int(time.time())),
                )
            return public_row(
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
                "SELECT burn_tx_id FROM jobs WHERE stage NOT IN ('paid','failed') AND due<=? ORDER BY due,rowid LIMIT 1",
                (int(time.time()),),
            ).fetchone()
            return row[0] if row else None


def public_row(row):
    value = {
        k: row[k]
        for k in (
            "burn_tx_id",
            "stage",
            "detail",
            "error",
            "amount_atoms",
            "recipient",
            "burn_height",
            "solana_tx",
        )
    }
    # Older journals may contain raw exception strings. A refused destination
    # keeps its fixed, named reason so the holder learns why.
    if value["error"] and not value["error"].startswith(NOT_WALLET):
        value["error"] = "Withdrawal processing failed."
    if value["detail"] not in (
        None,
        "Retry scheduled.",
        "Checkpoint advanced.",
        "Finalized marker. Signature unavailable.",
    ):
        value["detail"] = None
    return value


def chain_payment(burn, known=None, *, history_limit=100):
    return signature(cluster(), burn, known, history_limit=history_limit)


def checked_row(journal, burn):
    row = journal.get(burn)
    if not row or row["stage"] != "paid":
        return row
    with journal.marker_lock:
        cached = journal.marker_cache.get(burn)
        if cached and time.monotonic() < cached[0]:
            return cached[1]
    if not journal.refresh_slot.acquire(blocking=False):
        if cached:
            return cached[1]
        raise RuntimeError("Settlement refresh busy.")
    try:
        # Another handler may have finished between the first lookup and acquisition.
        with journal.marker_lock:
            cached = journal.marker_cache.get(burn)
            if cached and time.monotonic() < cached[0]:
                return cached[1]
        if not journal.refresh_rate.take():
            if cached:
                return cached[1]
            raise RuntimeError("Settlement refresh limit reached.")
        known = row["solana_tx"] or (journal.attempt(burn) or (None,))[0]
        result = chain_payment(burn, known, history_limit=4)
        if result is None:
            journal.update(burn, "queued", solana_tx=None, detail=None, error=None)
            return journal.get(burn)
        if result is PAID_NO_HISTORY and known:
            verified = dict(
                row, solana_tx=None, detail="Finalized marker. Signature unavailable."
            )
        else:
            finish_payment(
                lambda stage, **kw: journal.update(burn, stage, **kw), result
            )
            verified = journal.get(burn)
        expiry = time.monotonic() + 300 if result is PAID_NO_HISTORY else float("inf")
        with journal.marker_lock:
            journal.marker_cache[burn] = (expiry, verified)
            journal.marker_cache.move_to_end(burn)
            if len(journal.marker_cache) > 1280:
                journal.marker_cache.popitem(last=False)
        return verified
    finally:
        journal.refresh_slot.release()


def redemptions():
    """Finalized redemptions and the custody buckets the exit descriptor covers."""
    s = rpc_any("vault_bridge_status", asset_id=ASSET)
    return s["redemptions"], covered_buckets(s["buckets"])


def restore_redemption(journal, row, covered):
    burn = row["burn_tx_id"]
    owner = validate_burn(row, burn, covered)
    result = chain_payment(burn)
    journal.enqueue(burn, rebuilding=True)
    journal.update(
        burn,
        "queued",
        burn_height=row["created_at_height"],
        recipient=str(owner),
        amount_atoms=str(row["amount_atoms"]),
        solana_tx=None,
        detail=None,
        error=None,
    )
    if result:
        finish_payment(lambda stage, **kw: journal.update(burn, stage, **kw), result)


def rebuild(state):
    import tempfile

    from live import save

    generation = "jobs-recovered-" + str(time.time_ns()) + ".sqlite"
    with tempfile.TemporaryDirectory(dir=state) as tmp:
        path = Path(tmp) / "recovered.sqlite"
        journal = Journal(path)
        rows, covered = redemptions()
        for row in rows:
            if row["bucket_id"] in covered:
                restore_redemption(journal, row, covered)
        with closing(journal.connect()) as db, db:
            if journal.pending() is not None:
                c = cluster()
                expiry = c.call("getLatestBlockhash", c.level)["value"][
                    "lastValidBlockHeight"
                ]
                db.execute("INSERT INTO recovery VALUES(?)", (expiry,))
            if db.execute("PRAGMA integrity_check").fetchone()[0] != "ok":
                raise ValueError("Recovery integrity check failed.")
        # SQLite sidecars belong to a specific database. Keep every old generation
        # intact and select a separately fsynced database with a single atomic rename.
        os.rename(path, state / generation)
        directory_fd = os.open(state, os.O_DIRECTORY)
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
        save(state / "jobs.current", generation)
        fd = os.open(state, os.O_DIRECTORY)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)


def process_next(journal, directory):
    burn = journal.pending()
    if burn is not None and not BURN.fullmatch(burn):
        raise ValueError("Invalid journal entry. Rebuild required.")
    if burn is None or journal.waiting_for_expiry():
        return False
    try:
        process(
            burn,
            directory / burn,
            lambda stage, **kw: journal.update(burn, stage, **kw),
            attempt=journal.attempt(burn),
            before_send=lambda sig, h: journal.record_attempt(burn, sig, h),
        )
    except UnknownBurn:
        journal.update(burn, "failed", error="Unknown finalized burn.")
    except DestinationNotWallet as refused:
        # Named and not retried: the holder must see why; a refund is still open.
        journal.update(burn, "failed", error=str(refused))
    except CheckpointAdvanced:
        journal.defer(burn, progress=True)
    except Exception:
        journal.defer(burn)
    return True


def prune_terminal(journal, directory, retain=1024):
    with closing(journal.connect()) as db:
        rows = db.execute(
            """SELECT burn_tx_id,stage FROM jobs WHERE stage IN ('paid','failed')
            ORDER BY rowid DESC LIMIT 8 OFFSET ?""",
            (retain,),
        ).fetchall()
    burns = {r["burn_tx_id"]: r["stage"] for r in rows}
    # Rotate through retained work even when a different due job needs its rent.
    paths = sorted(
        p
        for p in directory.glob("[0-9a-f]" * 96)
        if p.name not in burns and not p.is_symlink() and p.is_dir()
    )
    if paths:
        path = next((p for p in paths if p.name > journal.last_buffer_job), paths[0])
        journal.last_buffer_job = path.name
        row = journal.get(path.name)
        if row and row["stage"] == "paid":
            if chain_payment(path.name, row["solana_tx"]):
                prune_work(cluster(), path, reclaim=True)
        elif row and any(path.glob("*.key")):
            from exit_payout import reclaim_completed

            reclaim_completed(cluster(), path)
    for burn, stage in burns.items():
        if not BURN.fullmatch(burn):
            raise ValueError("Invalid journal entry. Rebuild required.")
        result = chain_payment(burn)
        if result is None and stage == "paid":
            journal.update(burn, "queued", solana_tx=None, detail=None, error=None)
            continue
        if not result and any(r["burn_tx_id"] == burn for r in redemptions()):
            continue
        path = directory / burn
        if path.exists():
            prune_work(cluster(), path, reclaim=True)
            if path.exists():
                continue
        with closing(journal.connect()) as db, db:
            db.execute("DELETE FROM attempts WHERE burn=?", (burn,))
            db.execute("DELETE FROM jobs WHERE burn_tx_id=? AND stage=?", (burn, stage))


def worker(journal, directory, wake):
    while True:
        try:
            working = process_next(journal, directory)
            if time.monotonic() - journal.last_prune > 60:
                journal.last_prune = time.monotonic()
                prune_terminal(journal, directory)
            journal.worker_ok = True
        except Exception:
            journal.worker_ok = False
            working = False
        if not working:
            wake.wait(2)
            wake.clear()


class Admissions:
    def __init__(self, limit=12):
        self.limit = limit
        self.times = deque()
        self.lock = threading.Lock()

    def take(self):
        with self.lock:
            now = time.monotonic()
            while self.times and self.times[0] <= now - 60:
                self.times.popleft()
            if len(self.times) >= self.limit:
                return False
            self.times.append(now)
            return True


def local_healthy(server):
    if not server.thread.is_alive() or not server.journal.worker_ok:
        return False
    try:
        server.journal.probe()
    except (sqlite3.Error, OSError):
        return False
    return True


def service_readiness(server, *, fresh=False):
    if not local_healthy(server):
        return dict(
            ok=True,
            ready=False,
            payout_ready=False,
            message="Withdrawal worker or journal unavailable.",
        )
    if not server.journal.has_capacity():
        return dict(
            ok=True, ready=False, payout_ready=False, message="Withdrawal queue full."
        )
    with server.ready_lock:
        if fresh or time.monotonic() >= server.valid_until:
            server.ready = readiness()
            server.valid_until = time.monotonic() + 5
        return server.ready


# Account readiness: strict query, a short per-account cache and a small bound on
# concurrent reads. When the bound is hit the wallet gets 429 and retries.
ACCOUNT_CACHE = OrderedDict()
ACCOUNT_CACHE_SECONDS = 5
ACCOUNT_READERS = 2
ACCOUNT_SLOTS = threading.BoundedSemaphore(ACCOUNT_READERS)
ACCOUNT_LOCK = threading.Lock()
DESTINATION = re.compile("[1-9A-HJ-NP-Za-km-z]{32,44}")


def account_reply(server, path):
    """(HTTP code, body) for /readiness?account=…[&destination=…]."""
    url = urlsplit(path)
    try:
        query = parse_qs(url.query, strict_parsing=True)
    except ValueError:
        query = None
    if (
        not query
        or not set(query) <= {"account", "destination"}
        or any(len(v) != 1 for v in query.values())
        or ACCOUNT.fullmatch(query.get("account", [""])[0]) is None
        or ("destination" in query and DESTINATION.fullmatch(query["destination"][0]) is None)
    ):
        return 400, dict(error="Invalid account readiness query.")
    key = (query["account"][0], query.get("destination", [None])[0])
    now = time.monotonic()
    with ACCOUNT_LOCK:
        hit = ACCOUNT_CACHE.get(key)
        if hit and hit[0] > now:
            return 200, hit[1]
    if not ACCOUNT_SLOTS.acquire(blocking=False):
        return 429, dict(error="Account readiness busy; retry in a few seconds.")
    try:
        value = account_readiness(server, path)
    finally:
        ACCOUNT_SLOTS.release()
    with ACCOUNT_LOCK:
        ACCOUNT_CACHE[key] = (time.monotonic() + ACCOUNT_CACHE_SECONDS, value)
        while len(ACCOUNT_CACHE) > 1024:
            ACCOUNT_CACHE.popitem(last=False)
    return 200, value


def account_readiness(server, path):
    """Readiness plus the Solana-source series an account may offer, for the wallet."""
    url = urlsplit(path)
    if url.path != BASE + "/readiness" or not url.query:
        return None
    value = dict(service_readiness(server))
    query = parse_qs(url.query)
    account = query.get("account", [""])[0]
    try:
        series = holdings(
            account,
            value.get("buckets"),
            minimum=int(value.get("min_withdrawal_atoms") or 1),
        )
    except Exception:
        return dict(
            value, ready=False, series=[], message="Account holdings unavailable."
        )
    value = dict(value, account=account, series=series)
    if "destination" in query:
        # The same owner check the payout service runs, before the wallet builds a burn.
        try:
            from solders.pubkey import Pubkey

            refusal = destination_refusal(
                destination_cluster(), Pubkey.from_string(query["destination"][0])
            )
        except Exception:
            refusal = NOT_WALLET + " (destination could not be checked)"
        value["destination_refusal"] = refusal
        if refusal is not None:
            value.update(ready=False, message=refusal)
    return value


DESTINATION_CLUSTER = None


def destination_cluster():
    """One shared cluster for wallet destination checks. Each read is one attempt with
    a 5 s timeout and no 429 backoff, so a check holds its account slot only briefly."""
    global DESTINATION_CLUSTER
    if DESTINATION_CLUSTER is None:
        from exit_setup import ExitCluster, FEE_PAYER, require

        c = ExitCluster(FEE_PAYER)

        def call(method, *params):
            body = dict(jsonrpc="2.0", id=1, method=method, params=params)
            response = c.http.post(c.rpc, json=body, timeout=5)
            require(response.ok, f"{method} HTTP {response.status_code}")
            msg = response.json()
            require("error" not in msg, f"{method}: {msg.get('error')}")
            return msg["result"]

        c.call = call
        DESTINATION_CLUSTER = c
    return DESTINATION_CLUSTER


class WithdrawalServer(ThreadingHTTPServer):
    request_queue_size = 16

    def __init__(self, address, handler):
        self.slots = threading.BoundedSemaphore(8)
        super().__init__(address, handler)

    def process_request(self, request, address):
        if not self.slots.acquire(blocking=False):
            try:
                request.settimeout(0.1)
                request.sendall(
                    b'HTTP/1.1 503 Service Unavailable\r\nConnection: close\r\nContent-Type: application/json\r\n\r\n{"error":"Withdrawal service busy."}'
                )
            except OSError:
                pass
            self.shutdown_request(request)
            return
        try:
            super().process_request(request, address)
        except BaseException:
            self.slots.release()
            raise

    def process_request_thread(self, request, address):
        try:
            super().process_request_thread(request, address)
        finally:
            self.slots.release()

    def start_worker(self):
        self.thread = threading.Thread(
            target=worker, args=(self.journal, self.state, self.wake), daemon=True
        )
        self.thread.start()

    def service_actions(self):
        if not self.thread.is_alive():
            self.start_worker()


class Handler(BaseHTTPRequestHandler):
    def log_request(self, code="-", size="-"):
        # User-controlled request targets never enter the service log.
        pass

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
        if self.path == BASE + "/readiness/qualify":
            return self.reply(200, service_readiness(self.server, fresh=True))
        if self.path == BASE + "/readiness":
            return self.reply(200, service_readiness(self.server))
        if urlsplit(self.path).path == BASE + "/readiness":
            return self.reply(*account_reply(self.server, self.path))
        prefix = BASE + "/"
        if not self.path.startswith(prefix):
            return self.reply(404, dict(error="unknown path"))
        burn = self.path[len(prefix) :]
        if not BURN.fullmatch(burn):
            return self.reply(400, dict(error="invalid burn_tx_id"))
        try:
            row = checked_row(self.server.journal, burn)
        except Exception:
            return self.reply(503, dict(error="Settlement status unavailable."))
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
            burn = parse_request(self.rfile.read(int(size)))
        except (UnicodeError, ValueError):
            return self.reply(400, dict(error="Invalid burn request."))
        try:
            row = checked_row(self.server.journal, burn)
            if row and row["stage"] != "failed":
                return self.reply(200 if row["stage"] == "paid" else 202, row)
            if not self.server.admissions.take():
                return self.reply(
                    429, dict(error="Withdrawal admission rate exceeded.")
                )
            rows, covered = redemptions()
            red = next(
                (r for r in rows if r["burn_tx_id"] == burn and r["bucket_id"] in covered),
                None,
            )
            if red is None:
                return self.reply(404, dict(error="Unknown finalized burn."))
            validate_burn(red, burn, covered)
            row = self.server.journal.enqueue(burn)
        except OverflowError:
            return self.reply(429, dict(error="Withdrawal queue full."))
        except Exception:
            return self.reply(503, dict(error="Withdrawal admission unavailable."))
        self.server.wake.set()
        self.reply(200 if row["stage"] == "paid" else 202, row)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--qualification-socket", action="store_true",
                   help="serve authenticated open qualification in /run/pf-sol-exit")
    p.add_argument("--port", type=int, default=18089)
    p.add_argument(
        "--state", type=Path, default=Path.home() / ".local/state/postfiat/sol-exit"
    )
    p.add_argument(
        "--rebuild",
        action="store_true",
        help="recover journal from L1 and finalized Solana markers",
    )
    args = p.parse_args()
    os.umask(0o077)
    args.state.mkdir(parents=True, exist_ok=True)
    import fcntl

    with (args.state / "worker.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        if args.rebuild:
            rebuild(args.state)
            return
        with WithdrawalServer(("127.0.0.1", args.port), Handler) as server:
            server.state = args.state
            server.journal = Journal(args.state / "jobs.sqlite")
            server.ready_lock = threading.Lock()
            server.valid_until = 0
            server.admissions = Admissions()
            server.wake = threading.Event()
            server.start_worker()
            with ExitStack() as listeners:
                if args.qualification_socket:
                    import exit_qualification_socket

                    listeners.enter_context(exit_qualification_socket.serve(
                        exit_qualification_socket.SOCKET, lambda: service_readiness(server, fresh=True)
                    ))
                server.serve_forever()


if __name__ == "__main__":
    main()
