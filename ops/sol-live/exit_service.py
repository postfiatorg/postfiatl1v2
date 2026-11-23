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

from collections import deque
from exit_payout import (CheckpointAdvanced, PAID_NO_HISTORY, ASSET, BUCKET, cluster,
                         finish_payment, process, readiness, rpc, signature, validate_burn)

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
        self.worker_ok = True
        with closing(self.connect()) as db:
            db.execute("""CREATE TABLE IF NOT EXISTS jobs (
                burn_tx_id TEXT PRIMARY KEY, stage TEXT NOT NULL, detail TEXT, error TEXT,
                amount_atoms TEXT, recipient TEXT, burn_height INTEGER, solana_tx TEXT)""")

            columns={r[1] for r in db.execute("PRAGMA table_info(jobs)")}
            for name in ("due","retries"):
                if name not in columns:
                    db.execute(f"ALTER TABLE jobs ADD COLUMN {name} INTEGER NOT NULL DEFAULT 0")
            db.execute("""CREATE TABLE IF NOT EXISTS attempts (
                burn TEXT PRIMARY KEY, signature TEXT NOT NULL, expiry INTEGER NOT NULL)""")
            db.commit()

    def probe(self):
        with closing(self.connect()) as db:
            db.execute("SELECT burn_tx_id FROM jobs LIMIT 1").fetchone()

    def attempt(self,burn):
        with closing(self.connect()) as db:
            row=db.execute("SELECT signature,expiry FROM attempts WHERE burn=?",(burn,)).fetchone()
            return tuple(row) if row else None

    def record_attempt(self,burn,sig,height):
        with closing(self.connect()) as db, db:
            db.execute("INSERT OR REPLACE INTO attempts VALUES(?,?,?)",(burn,sig,height))
            db.execute("UPDATE jobs SET stage='paying' WHERE burn_tx_id=?",(burn,))

    def next_due(self,burn):
        with closing(self.connect()) as db:
            return db.execute("SELECT due FROM jobs WHERE burn_tx_id=?",(burn,)).fetchone()[0]

    def defer(self,burn,progress=False):
        with closing(self.connect()) as db, db:
            db.execute("BEGIN IMMEDIATE")
            retries=db.execute("SELECT retries FROM jobs WHERE burn_tx_id=?",(burn,)).fetchone()[0]
            delay=1 if progress else min(300,5 * 2**min(retries,6))
            db.execute("""UPDATE jobs SET stage='queued', retries=?,due=?,error=NULL,detail=?
                          WHERE burn_tx_id=? AND stage!='paid'""",
                       (0 if progress else retries+1,int(time.time())+delay,
                        "Checkpoint advanced." if progress else "Retry scheduled.",burn))

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
        with closing(self.connect()) as db, db:
            db.execute("BEGIN IMMEDIATE")
            row = db.execute(
                "SELECT stage FROM jobs WHERE burn_tx_id=?", (burn,)
            ).fetchone()
            if row is None or row[0] == "failed":
                active = db.execute(
                    "SELECT count(*) FROM jobs WHERE stage NOT IN ('paid','failed')"
                ).fetchone()[0]
                if active >= 32 and not rebuilding:
                    raise OverflowError("withdrawal queue is full")
                db.execute(
                    """INSERT INTO jobs(burn_tx_id,stage) VALUES(?,'queued')
                    ON CONFLICT(burn_tx_id) DO UPDATE SET stage='queued', error=NULL, detail=NULL""",
                    (burn,),
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
                "SELECT burn_tx_id FROM jobs WHERE stage NOT IN ('paid','failed') AND due<=? ORDER BY due,rowid LIMIT 1", (int(time.time()),)
            ).fetchone()
            return row[0] if row else None


def public_row(row):
    value = {k:row[k] for k in ("burn_tx_id","stage","detail","error","amount_atoms","recipient","burn_height","solana_tx")}
    # Older journals may contain raw exception strings.
    if value["error"]:
        value["error"]="Withdrawal processing failed."
    if value["detail"] not in (None,"Retry scheduled.","Checkpoint advanced.","Finalized marker. Signature unavailable."):
        value["detail"]=None
    return value


def chain_payment(burn,known=None):
    return signature(cluster(),burn,known)


def checked_row(journal,burn):
    row=journal.get(burn)
    if row and row["stage"]=="paid":
        result=chain_payment(burn,row["solana_tx"])
        if result is None:
            journal.update(burn,"queued",solana_tx=None,detail=None,error=None)
        else:
            finish_payment(lambda stage,**kw:journal.update(burn,stage,**kw),result)
        return journal.get(burn)
    return row


def redemptions():
    return rpc(0,"vault_bridge_status",asset_id=ASSET)["redemptions"]


def restore_redemption(journal,row):
    burn=row["burn_tx_id"]
    owner=validate_burn(row,burn)
    result=chain_payment(burn)
    journal.enqueue(burn,rebuilding=True)
    journal.update(burn,"queued",burn_height=row["created_at_height"],recipient=str(owner),
                   amount_atoms=str(row["amount_atoms"]),solana_tx=None,detail=None,error=None)
    if result:
        finish_payment(lambda stage,**kw:journal.update(burn,stage,**kw),result)


def rebuild(state):
    import tempfile
    target=state/"jobs.sqlite"
    with tempfile.TemporaryDirectory(dir=state) as tmp:
        path=Path(tmp)/"recovered.sqlite"
        journal=Journal(path)
        for row in redemptions():
            if row["bucket_id"]==BUCKET:
                restore_redemption(journal,row)
        with closing(journal.connect()) as db:
            if db.execute("PRAGMA integrity_check").fetchone()[0]!="ok":
                raise ValueError("Recovery integrity check failed.")
        if target.exists():
            # Keep the original inode for offline recovery of any in-flight signatures.
            if any(Path(str(target)+ext).exists() for ext in ("-wal","-journal","-shm")):
                raise ValueError("SQLite sidecar present. Preserve the database set together before rebuilding.")
            os.link(target,state/("jobs.sqlite.damaged-"+str(time.time_ns())))
        os.replace(path,target)
        fd=os.open(state,os.O_DIRECTORY)
        try: os.fsync(fd)
        finally: os.close(fd)


def process_next(journal, directory):
    burn = journal.pending()
    if burn is None:
        return False
    try:
        process(burn,directory/burn,lambda stage,**kw:journal.update(burn,stage,**kw),
                attempt=journal.attempt(burn),before_send=lambda sig,h:journal.record_attempt(burn,sig,h))
    except CheckpointAdvanced:
        journal.defer(burn,progress=True)
    except Exception:
        journal.defer(burn)
    return True


def worker(journal, directory, wake):
    while True:
        try:
            working=process_next(journal,directory)
            journal.worker_ok=True
        except Exception:
            journal.worker_ok=False
            working=False
        if not working:
            wake.wait(2)
            wake.clear()


class Admissions:
    def __init__(self,limit=12):
        self.limit=limit
        self.times=deque()
        self.lock=threading.Lock()

    def take(self):
        with self.lock:
            now=time.monotonic()
            while self.times and self.times[0]<=now-60:
                self.times.popleft()
            if len(self.times)>=self.limit:
                return False
            self.times.append(now)
            return True


def local_healthy(server):
    if not server.thread.is_alive() or not server.journal.worker_ok:
        return False
    try:
        server.journal.probe()
    except (sqlite3.Error,OSError):
        return False
    return True


def service_readiness(server):
    if not local_healthy(server):
        return dict(ok=True,ready=False,payout_ready=False,message="Withdrawal worker or journal unavailable.")
    with server.ready_lock:
        if time.monotonic() >= server.valid_until:
            server.ready=readiness()
            server.valid_until=time.monotonic()+5
        return server.ready


class WithdrawalServer(ThreadingHTTPServer):
    request_queue_size=16
    def __init__(self,address,handler):
        self.slots=threading.BoundedSemaphore(8)
        super().__init__(address,handler)

    def process_request(self,request,address):
        if not self.slots.acquire(blocking=False):
            self.shutdown_request(request)
            return
        try:
            super().process_request(request,address)
        except BaseException:
            self.slots.release()
            raise

    def process_request_thread(self,request,address):
        try:
            super().process_request_thread(request,address)
        finally:
            self.slots.release()

    def start_worker(self):
        self.thread=threading.Thread(target=worker,args=(self.journal,self.state,self.wake),daemon=True)
        self.thread.start()

    def service_actions(self):
        if not self.thread.is_alive():
            self.start_worker()


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
            return self.reply(200,service_readiness(self.server))
        prefix = BASE + "/"
        if not self.path.startswith(prefix):
            return self.reply(404, dict(error="unknown path"))
        burn = self.path[len(prefix) :]
        if not BURN.fullmatch(burn):
            return self.reply(400, dict(error="invalid burn_tx_id"))
        try:
            row = checked_row(self.server.journal,burn)
        except Exception:
            return self.reply(503,dict(error="Settlement status unavailable."))
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
            row=checked_row(self.server.journal,burn)
            if row and row["stage"]!="failed":
                return self.reply(200 if row["stage"]=="paid" else 202,row)
            if not self.server.admissions.take():
                return self.reply(429,dict(error="Withdrawal admission rate exceeded."))
            if not service_readiness(self.server).get("payout_ready"):
                return self.reply(503,dict(error="Withdrawal worker unavailable."))
            red=next((r for r in redemptions() if r["burn_tx_id"]==burn and r["bucket_id"]==BUCKET),None)
            if red is None:
                return self.reply(404,dict(error="Unknown finalized burn."))
            validate_burn(red,burn)
            row=self.server.journal.enqueue(burn)
        except OverflowError:
            return self.reply(429,dict(error="Withdrawal queue full."))
        except Exception:
            return self.reply(503,dict(error="Withdrawal admission unavailable."))
        self.server.wake.set()
        self.reply(200 if row["stage"] == "paid" else 202, row)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--port", type=int, default=18089)
    p.add_argument(
        "--state", type=Path, default=Path.home() / ".local/state/postfiat/sol-exit"
    )
    p.add_argument("--rebuild",action="store_true",help="recover journal from L1 and finalized Solana markers")
    args = p.parse_args()
    os.umask(0o077)
    args.state.mkdir(parents=True, exist_ok=True)
    import fcntl

    with (args.state/"worker.lock").open("a") as lock:
        fcntl.flock(lock,fcntl.LOCK_EX | fcntl.LOCK_NB)
        if args.rebuild:
            rebuild(args.state)
            return
        with WithdrawalServer(("127.0.0.1",args.port),Handler) as server:
            server.state=args.state
            server.journal=Journal(args.state/"jobs.sqlite")
            server.ready_lock=threading.Lock()
            server.valid_until=0
            server.admissions=Admissions()
            server.wake=threading.Event()
            server.start_worker()
            server.serve_forever()


if __name__ == "__main__":
    main()
