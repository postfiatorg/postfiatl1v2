#!/usr/bin/env python3
"""Crash the fixture worker on both sides of signing, then verify credit and replay."""

import argparse
import fcntl
import json
import os
import signal
import sqlite3
import subprocess
import sys
import time
from pathlib import Path
from unittest.mock import patch

from jobs import Relay
from storage import digest, load


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("config", type=Path)
    args = p.parse_args()
    cfg = load(args.config)
    if cfg["ledger"] != "stub":
        p.error(
            "the destructive process-interruption test requires the local fixture stub"
        )
    relay = Relay(cfg)
    sig = (Path(cfg["capture_root"]) / "signature").read_text().strip()
    job, _ = relay.create("deposit", sig)
    cmd = [
        sys.executable,
        Path(__file__).with_name("relay.py"),
        "--config",
        args.config,
        "deposit",
        sig,
    ]

    def count(txid):
        with sqlite3.connect(relay.ledger.db) as db:
            return [
                db.execute("SELECT COUNT(*) FROM tx WHERE id=?", (txid,)).fetchone()[0],
                db.execute(
                    "SELECT COUNT(*) FROM credits WHERE txid=?", (txid,)
                ).fetchone()[0],
            ]

    if not (job / "signed.json").exists():
        # Hold the sequence lock. SIGKILL cannot race with signing or submission.
        with (relay.outbox / "submitter.lock").open("a") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            proc = subprocess.Popen(cmd)
            try:
                while "proof_done" not in load(job / "job.json")["timings"]:
                    if proc.poll() is not None:
                        raise RuntimeError(
                            "proof worker exited: " + str(proc.returncode)
                        )
                    time.sleep(0.1)
            finally:
                proc.kill()
                proc.wait()
            assert (
                proc.returncode == -signal.SIGKILL
                and not (job / "signed.json").exists()
            )
            print("SIGKILL after proof checkpoint, signed=false", flush=True)
        # Kill after the committed write, before the relay sees a submission response.
        child = os.fork()
        if child == 0:
            submit = relay.ledger.submit

            def lost_ack(signed, out):
                submit(signed, out)
                os.kill(os.getpid(), signal.SIGKILL)

            relay.ledger.submit = lost_ack
            try:
                relay.run(job)
            finally:
                os._exit(2)
        _, status = os.waitpid(child, 0)
        assert os.waitstatus_to_exitcode(status) == -signal.SIGKILL
        print("SIGKILL after ledger commit, before submission checkpoint", flush=True)
    signed = (job / "signed.json").read_bytes()
    tid = load(job / "job.json")["tx_id"]
    before = count(tid)
    # The ledger has the commit but the worker lacks its acknowledgement. A new
    # worker must query the existing receipt, without calling either operation.
    with (
        patch.object(
            relay.ledger,
            "submit",
            side_effect=AssertionError("unexpected resubmission"),
        ),
        patch.object(
            relay.ledger, "sign", side_effect=AssertionError("unexpected re-signing")
        ),
    ):
        result = relay.run(job)
    replay = json.loads(subprocess.check_output(cmd))
    assert before == count(tid) and before[0] == 1 and before[1] > 0
    assert (
        result["status"] == replay["status"] == "accepted"
        and replay["idempotent_replay"]
    )
    assert (
        signed == (job / "signed.json").read_bytes()
        and result["tx_id"] == replay["tx_id"]
    )
    with sqlite3.connect(relay.ledger.db) as db:
        rows = db.execute(
            "SELECT recipient,atoms FROM credits WHERE txid=?", (result["tx_id"],)
        ).fetchall()
    from source import journal

    proven = journal((job / "public.bin").read_bytes())["receipts"]
    assert sorted(rows) == sorted(
        (r["pftl_recipient"], r["amount_atoms"]) for r in proven
    )
    print(
        json.dumps(
            {
                "signature": sig,
                "slot": load(job / "source.json")["slot"],
                "job_id": result["job_id"],
                "ledger": "stub",
                "backend": result["prover"],
                "tx_id": result["tx_id"],
                "signed_sha256": digest(signed),
                "proof_bytes": (job / "proof.bin").stat().st_size,
                "credited_atoms": str(sum(int(amount) for _, amount in rows)),
                "additional_submissions": 0,
                "timings": result["timings"],
                "seconds": relay.summary()["deposit"],
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
