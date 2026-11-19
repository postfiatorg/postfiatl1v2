#!/usr/bin/env python3
"""Check both binaries against the recorded status of six distinct source validators."""
import argparse
import concurrent.futures
import json
from pathlib import Path
import subprocess


def cli(binary, store, command):
    return json.loads(subprocess.check_output([binary, command, "--data-dir", store]))


def check(row, args):
    store = args.stores / row["node_id"]
    statuses = []
    proofs = []
    for binary, revision in [(args.r5, "ac91a1f4"), (args.r6, args.revision)]:
        status = cli(binary, store, "status")
        observed = {k: status[k] for k in row}
        if observed != {**row, "build_git_revision": revision}:
            raise ValueError(f"{store.name} status mismatch: {observed}")
        del status["build_git_revision"]
        statuses.append(status)
        proofs.append(cli(binary, store, "verify-finalized-checkpoint"))
    p = proofs[0]
    if statuses[0] != statuses[1] or proofs[0] != proofs[1]:
        raise ValueError(f"{store.name} release reports differ")
    if (p["verified"], p["checkpoint_height"], p["checkpoint_block_hash"], p["checkpoint_state_root"],
        p["validator_count"], p["quorum"], p["genesis_hash"]) != (
            True, row["block_height"], row["block_tip_hash"], row["state_root"], 6, 5, row["genesis_hash"]):
        raise ValueError(f"{store.name} checkpoint failed")
    return dict(node=store.name, r5="ac91a1f4", r6=args.revision, height=p["checkpoint_height"],
                tip=p["checkpoint_block_hash"], root=p["checkpoint_state_root"], certificate=p["certificate_id"],
                quorum=p["quorum"], full_status_except_build_equal=True, full_checkpoint_equal=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ["r5", "r6", "stores", "fleet"]:
        parser.add_argument(arg, type=Path)
    parser.add_argument("revision")
    args = parser.parse_args()
    fleet = json.loads(args.fleet.read_text())
    if len(fleet) != 6 or {r["node_id"] for r in fleet} != {f"validator-{n}" for n in range(6)}:
        raise ValueError("expected six distinct validators")
    if len({(r["genesis_hash"], r["block_height"], r["block_tip_hash"], r["state_root"]) for r in fleet}) != 1:
        raise ValueError("source validators disagree")
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
        reports = list(pool.map(lambda row: check(row, args), fleet))
    if len({r["certificate"] for r in reports}) != 1:
        raise ValueError("restored certificates disagree")
    print(json.dumps(reports, indent=2))


if __name__ == "__main__":
    main()
