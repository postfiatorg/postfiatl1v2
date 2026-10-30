#!/usr/bin/env python3
"""Read-only section 7 check: the proposer's block-vote-records file for HEIGHT exists, names the six validators,
and shows no false no_answer (a validator whose accepting vote is in the block's certificate).
Usage: vote_record_check.py HEIGHT OUTPUT. Writes nothing on any host."""
import json, socket, subprocess, sys, hashlib
from datetime import datetime, timezone

HOSTS = ["64.176.220.75", "95.179.184.122", "66.42.48.39", "149.28.63.106", "95.179.179.206", "45.32.110.170"]
height, output = int(sys.argv[1]), sys.argv[2]


def rpc(port, method, params):
    with socket.create_connection(("127.0.0.1", port), timeout=60) as s:
        f = s.makefile("rwb")
        f.write((json.dumps({"version": "postfiat-local-rpc-v1", "id": "vr", "method": method, "params": params}) + "\n").encode()); f.flush()
        r = json.loads(f.readline(64 << 20))
    assert r.get("ok") is True, r.get("error")
    return r["result"]


header = rpc(27650, "blocks", {"from_height": height, "limit": 1})[0]["header"]
proposer = header["proposer"]; index = int(proposer.split("-")[1])
status = rpc(27650 + index, "status", {})
path = f"/var/lib/postfiat/{proposer}/block-vote-records/height-{height:020d}.json"
read = subprocess.run(["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=15", f"root@{HOSTS[index]}", "cat", path],
                      capture_output=True, timeout=120)
record = json.loads(read.stdout) if read.returncode == 0 else None
voters = sorted(v["validator"] for v in header["certificate"]["votes"] if v.get("accept") is True)
outcomes = {e["validator"]: e["outcome"] for e in record["entries"]} if record else {}
checks = {
    "proposer_on_new_build": str(status["build_git_revision"]).startswith("ae70d66d"),
    "record_exists": record is not None,
    "names_six_validators": sorted(outcomes) == [f"validator-{i}" for i in range(6)],
    "matches_block": bool(record) and record["block_height"] == height and record["block_hash"] == header["block_hash"]
                     and record["certificate_id"] == header["certificate_id"] and record["proposer"] == proposer,
    "no_false_no_answer": bool(record) and all(outcomes.get(v) == "accepted" for v in voters),
}
report = {"checked_at": datetime.now(timezone.utc).isoformat(), "height": height, "proposer": proposer, "file": path,
          "file_sha256": hashlib.sha256(read.stdout).hexdigest() if record else None,
          "certificate_accepting_voters": voters, "outcomes": outcomes, "record": record, "checks": checks, "pass": all(checks.values())}
with open(output, "x") as f:
    json.dump(report, f, indent=1); f.write("\n")
print(("PASS" if report["pass"] else "FAIL"), height, proposer, outcomes, checks)
sys.exit(0 if report["pass"] else 1)
