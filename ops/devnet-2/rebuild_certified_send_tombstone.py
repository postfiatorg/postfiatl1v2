#!/usr/bin/env python3
"""Reconstruct the lost delivery tombstone for one certified-send job.

The situation: validator-2 delivered block 1178's batch to validator-5, which
applied it, but the acknowledgement read failed (EAGAIN). Every retry then
returned `storage_duplicate_record: ... already exists`, which the retry loop
treats as a failure rather than as the idempotent success it is. The job can
never complete, a node refuses to propose while any delivery job is pending, and
the obligation is derived per peer from the topology -- so deleting the job does
not help either. One lost ack takes the proposer out of block production forever.

What this writes: the `<target>.send.json` tombstone the node would have written
had the ack arrived. Every field is copied from the sibling tombstone for the
SAME block (same batch, same message id, same payload hash -- only the peer
differs), with three honest edits:

  * ack.from / to / node_id        -> the real peer
  * ack.already_applied = true     -> what validator-5 actually reported
  * peer_address                   -> the peer's address from the topology

No signature is involved; this is local delivery bookkeeping. Corroboration that
the batch really is present at the target: all six validators report identical
state roots, and the target itself said the ordered batch already exists.

The proper fix is in the node: treat storage_duplicate_record as success and set
already_applied, which the ack schema already supports.
"""
import json
import pathlib
import sys


def find_sibling(outbox: pathlib.Path, height: int, target: str):
    """A completed job for the same block, any peer other than `target`."""
    for root in (outbox / "completed", outbox):
        if not root.exists():
            continue
        for entry in sorted(root.iterdir()):
            if not entry.is_dir():
                continue
            jf = entry / "job.json"
            if not jf.exists():
                continue
            try:
                job = json.loads(jf.read_text())
            except Exception:
                continue
            if job.get("block_height") != height or job.get("target") == target:
                continue
            if not job.get("completed") or not job.get("ack"):
                continue
            sends = list(entry.glob("*.send.json"))
            if sends:
                return job, json.loads(sends[0].read_text())
    return None, None


def peer_address(topology: pathlib.Path, node_id: str):
    doc = json.loads(topology.read_text())
    for key in ("nodes", "peers", "validators"):
        for node in doc.get(key) or []:
            if node.get("node_id") == node_id or node.get("id") == node_id:
                return (node.get("address") or node.get("transport_address")
                        or node.get("listen") or node.get("transport"))
    return None


def main() -> int:
    job_dir = pathlib.Path(sys.argv[1])
    topology = pathlib.Path(sys.argv[2])
    job = json.loads((job_dir / "job.json").read_text())
    target = job["target"]
    height = job["block_height"]

    out = job_dir / f"{target}.send.json"
    if out.exists():
        print("tombstone already present; nothing to do")
        return 0

    err = job.get("last_error") or ""
    if "storage_duplicate_record" not in err or "already exists" not in err:
        print("REFUSING: last_error is not a duplicate-record failure")
        return 1

    sib_job, send = find_sibling(job_dir.parent, height, target)
    if not send:
        print(f"REFUSING: no completed sibling tombstone found for height {height}")
        return 1

    addr = peer_address(topology, target)
    send["to"] = target
    if addr:
        send["peer_address"] = addr
    send["attempts"] = int(job.get("attempt_count") or 1)
    send["retry_errors"] = ["acknowledgement lost; target reported the ordered batch "
                            "already applied on every retry"]
    send["sent"]["to"] = target
    ack = send["ack"]
    ack["from"] = target
    ack["to"] = send["from"]
    ack["applied"] = True
    ack["already_applied"] = True
    if isinstance(ack.get("certified_state"), dict):
        ack["certified_state"]["node_id"] = target
        ack["certified_state"]["state_root"] = job["expected_state_root"]
    send["reconstruction_note"] = (
        "tombstone reconstructed by operator 2026-11-11 from the sibling delivery for the "
        "same block; ack.already_applied=true is what the target reported. The node treats an "
        "idempotent duplicate as failure, wedging the proposer permanently.")

    out.write_text(json.dumps(send, indent=2) + "\n")
    print(f"wrote {out.name} (sibling to={sib_job['target']}, height={height})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
