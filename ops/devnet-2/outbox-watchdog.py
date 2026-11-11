#!/usr/bin/env python3
"""Clear certified-send jobs that can never complete.

A delivery whose every attempt returns `storage_duplicate_record: ... already
exists` has already been applied at the target: the acknowledgement was lost,
not the data. The retry loop treats that as a failure, so the job stays pending
forever and the node then refuses to propose any new block -- one lost ack
silently takes a validator out of block production.

This moves only those jobs aside, keeping a full copy. A job failing for any
other reason is left alone, so a real delivery problem still surfaces.

Proper fix belongs in the node: treat a duplicate record as an idempotent
success and write the ack with already_applied=true, which the ack schema
already supports.
"""
import json
import pathlib
import shutil
import time

OUTBOX = pathlib.Path("/var/lib/postfiat/validator-2/certified-send-outbox")
BACKUP = pathlib.Path("/root/outbox-watchdog-cleared")
MIN_ATTEMPTS = 1   # the outer counter is 1 while the inner transport loop runs 17 times


def sweep() -> None:
    for entry in sorted(OUTBOX.iterdir()):
        if not entry.is_dir() or entry.name == "completed":
            continue
        job_file = entry / "job.json"
        if not job_file.exists():
            continue
        try:
            job = json.loads(job_file.read_text())
        except Exception:
            continue
        if job.get("completed"):
            continue
        error = job.get("last_error") or ""
        if "storage_duplicate_record" not in error or "already exists" not in error:
            continue
        if int(job.get("attempt_count") or 0) < MIN_ATTEMPTS:
            continue
        stamp = int(time.time())
        shutil.copytree(entry, BACKUP / f"copy-{stamp}-{entry.name}", dirs_exist_ok=True)
        shutil.move(str(entry), str(BACKUP / f"cleared-{stamp}-{entry.name}"))
        print("{} cleared duplicate-stuck job height={} target={} attempts={}".format(
            time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            job.get("block_height"), job.get("target"), job.get("attempt_count")),
            flush=True)


def main() -> None:
    BACKUP.mkdir(parents=True, exist_ok=True)
    while True:
        try:
            sweep()
        except Exception as exc:
            print("watchdog error: {}".format(exc), flush=True)
        time.sleep(10)


if __name__ == "__main__":
    main()
