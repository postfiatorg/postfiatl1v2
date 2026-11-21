#!/usr/bin/env python3
"""Archive an acknowledged legacy delivery only when all six finalized tips witness it."""
import json
import pathlib
import re
import subprocess
import time

OUTBOX = pathlib.Path("/var/lib/postfiat/validator-2/certified-send-outbox")
EXPECTED_PEERS = {f"validator-{n}" for n in range(6)}


def sweep() -> None:
    for job_file in OUTBOX.glob("*/job.json"):
        entry = job_file.parent
        job = json.loads(job_file.read_bytes())
        error = job.get("last_error") or ""
        if job["completed"] or "storage_duplicate_record" not in error:
            continue
        if job["source"] != OUTBOX.parent.name or job["job_id"] != entry.name:
            continue
        batch_id = json.loads((entry / "batch.json").read_bytes())["batch_id"]
        detail = (f"transport batch rejected by `{job['target']}` for `batch`: transport batch service "
                  f"apply failed: storage_duplicate_record: ordered batch `{batch_id}` already exists")
        if not re.fullmatch(r"[0-9a-f]{96}", batch_id) or detail not in error:
            continue
        unit = "postfiat-" + job["source"] + ".service"
        pid = subprocess.check_output(["systemctl", "show", "-p", "MainPID", "--value", unit], text=True).strip()
        proc = pathlib.Path("/proc") / pid
        args = (proc / "cmdline").read_bytes().decode().split("\0")
        topology = args[args.index("--topology") + 1]
        output = subprocess.check_output([
            "runuser", "-u", "postfiat", "--", str((proc / "exe").resolve(strict=True)),
            "transport-certified-send-outbox-witness", "--data-dir", str(OUTBOX.parent),
            "--topology", topology, "--job-file", str(job_file),
        ], text=True, timeout=210)
        witnesses = json.loads(output)
        if len(witnesses) != 6 or {s["node_id"] for s in witnesses} != EXPECTED_PEERS:
            continue
        if not all((s["chain_id"], s["block_height"], s["block_tip_hash"], s["state_root"]) ==
                   (job["chain_id"], job["block_height"], job["block_hash"], job["expected_state_root"])
                   for s in witnesses):
            continue
        if json.loads(job_file.read_bytes()) != job:
            continue
        archive = OUTBOX.parent / "sol-live" / ("confirmed-delivery-" + entry.name)
        archive.parent.mkdir(exist_ok=True)
        if archive.exists():
            continue
        entry.rename(archive)
        (archive / "six-way-witness.json").write_text(output)
        print(time.strftime("%FT%TZ", time.gmtime()), "archived", str(archive), flush=True)


if __name__ == "__main__":
    while True:
        try:
            sweep()
        except (FileNotFoundError, ValueError, KeyError, OSError, subprocess.SubprocessError) as exc:
            print("watchdog:", exc, flush=True)
        time.sleep(10)
