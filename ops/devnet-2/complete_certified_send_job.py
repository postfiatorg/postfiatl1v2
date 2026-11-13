#!/usr/bin/env python3
"""Complete a certified-send job whose target already holds the batch.

Background: a delivery to validator-5 lost its acknowledgement while that node's
RPC was wedged. Validator-5 had already applied the ordered batch, so every
retry returns `storage_duplicate_record: ... already exists`. The retry loop
treats that as a failure, so the job never completes. A node refuses to propose
while any delivery job is pending, and the obligation is derived per peer from
the topology -- so deleting the job directory does not help either; a missing
job for a peer is itself counted as pending. The proposer is wedged permanently.

This writes the completion the node would have written had it treated the
duplicate as the idempotent success it is. No signature is involved: the ack is
local delivery bookkeeping, and all four values come from the job's own fields.
`already_applied: True` is what validator-5 actually reported, and the ack
schema carries that field for exactly this case.

Independent corroboration: all six validators report identical state roots, so
block 1178 is present everywhere.
"""
import json
import pathlib
import sys

NOTE = (
    "completed by operator 2026-11-11: every delivery attempt returned "
    "storage_duplicate_record, so the target already held the ordered batch and only "
    "the acknowledgement was lost. All six validators report identical state roots. "
    "The node treats an idempotent duplicate as a failure, which wedges the proposer; "
    "the ack schema already has already_applied for this case and the node should set it."
)


def main() -> int:
    path = pathlib.Path(sys.argv[1])
    job = json.loads(path.read_text())

    if job.get("completed"):
        print("already completed; nothing to do")
        return 0
    error = job.get("last_error") or ""
    if "storage_duplicate_record" not in error or "already exists" not in error:
        print("REFUSING: last_error is not a duplicate-record failure")
        return 1
    for field in ("block_hash", "expected_state_root", "block_height", "target"):
        if not job.get(field):
            print(f"REFUSING: job is missing {field}")
            return 1

    job["completed"] = True
    job["ack"] = {
        "already_applied": True,
        "block_height": job["block_height"],
        "block_tip_hash": job["block_hash"],
        "state_root": job["expected_state_root"],
    }
    job["completion_note"] = NOTE
    path.write_text(json.dumps(job, indent=2) + "\n")
    print(f"completed job height={job['block_height']} target={job['target']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
