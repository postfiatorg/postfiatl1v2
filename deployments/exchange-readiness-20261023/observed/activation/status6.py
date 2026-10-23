#!/usr/bin/env python3
"""Read-only: status of all six through the tunnels. Usage: status6.py OUTPUT."""
import json, socket, sys
from datetime import datetime, timezone
rows = {}
for i in range(6):
    req = {"version": "postfiat-local-rpc-v1", "id": "status6", "method": "status", "params": {}}
    with socket.create_connection(("127.0.0.1", 27650 + i), timeout=60) as s:
        f = s.makefile("rwb"); f.write((json.dumps(req) + "\n").encode()); f.flush(); r = json.loads(f.readline(1 << 24))["result"]
    rows[f"validator-{i}"] = {"height": r["block_height"], "tip": r["block_tip_hash"], "state_root": r["state_root"], "mempool_pending": r["mempool_pending"], "binary_sha256": r["deployment_runtime_artifacts"]["binary_sha256"]}
out = {"observed_at": datetime.now(timezone.utc).isoformat(), "hosts": rows, "six_way": len({(r["height"], r["tip"], r["state_root"]) for r in rows.values()}) == 1}
with open(sys.argv[1], "x") as f: json.dump(out, f, indent=1); f.write("\n")
for v, r in rows.items(): print(v, r["height"], r["tip"][:12], r["state_root"][:12], r["mempool_pending"])
print("six_way", out["six_way"])
