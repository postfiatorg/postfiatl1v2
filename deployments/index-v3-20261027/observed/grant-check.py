#!/usr/bin/env python3
"""Read-only: the grant's block is certified identically on all six (blocks + status RPC)."""
import json, socket, sys, time
from datetime import datetime, timezone

grant_file, output = sys.argv[1], sys.argv[2]
base = int(sys.argv[3]) if len(sys.argv) > 3 else 27650
grant = json.load(open(grant_file))
height, tx = int(grant["height"]), grant["tx_id"]


def rpc(port, method, params):
    with socket.create_connection(("127.0.0.1", port), timeout=30) as sock:
        stream = sock.makefile("rwb")
        stream.write((json.dumps({"version": "postfiat-local-rpc-v1", "id": f"grant-check-{method}",
                                  "method": method, "params": params}) + "\n").encode())
        stream.flush()
        reply = json.loads(stream.readline(64 << 20))
    if reply.get("ok") is not True:
        raise RuntimeError(f"{method} on {port}: {reply.get('error')}")
    return reply["result"]


rows = []
for index in range(6):
    port = base + index
    for _ in range(30):
        status = rpc(port, "status", {})
        if status["block_height"] >= height:
            break
        time.sleep(2)
    block = rpc(port, "blocks", {"from_height": height, "limit": 1})[0]
    header, cert = block["header"], block["header"]["certificate"]
    rows.append({
        "validator_id": f"validator-{index}", "height": header["height"], "block_hash": header["block_hash"],
        "state_root": header["state_root"], "proposer": header["proposer"], "view": header["view"],
        "certificate_id": header["certificate_id"], "quorum": cert["quorum"],
        "certificate_validators": len(cert["validators"]),
        "voters": sorted(v["validator"] for v in cert["votes"] if v.get("accept") is True),
        "receipt_ids": block["receipt_ids"], "status_height": status["block_height"],
        "status_tip": status["block_tip_hash"], "status_root": status["state_root"],
        "mempool_pending": status["mempool_pending"],
    })
first = rows[0]
same = lambda k: all(r[k] == first[k] for r in rows)
checks = {
    "identical_block_on_6": all(r["height"] == height for r in rows) and all(same(k) for k in ("block_hash", "state_root", "certificate_id", "proposer", "view")),
    "quorum_5_of_6": all(len(r["voters"]) >= 5 and r["quorum"] == 5 and r["certificate_validators"] == 6 for r in rows),
    "tip_is_grant_block_on_6": all(r["status_height"] == height and r["status_tip"] == r["block_hash"] and r["status_root"] == r["state_root"] for r in rows),
    "mempools_empty": all(r["mempool_pending"] == 0 for r in rows),
    "receipt_verified_6_of_6": all(tx in r["receipt_ids"] for r in rows) and grant.get("verified_6_of_6") is True,
    "proposer_matches_receipt": first["proposer"] == grant["proposer"],
}
report = {"checked_at": datetime.now(timezone.utc).isoformat(), "height": height, "tx_id": tx,
          "block_hash": first["block_hash"], "state_root": first["state_root"], "proposer": first["proposer"],
          "view": first["view"], "certificate_id": first["certificate_id"], "voters": first["voters"],
          "checks": checks, "pass": all(checks.values()), "validators": rows}
with open(output, "x") as handle:
    json.dump(report, handle, indent=1)
    handle.write("\n")
print(("PASS" if report["pass"] else "FAIL"), f"height={height} proposer={first['proposer']} view={first['view']}",
      f"block={first['block_hash'][:12]} root={first['state_root'][:12]} voters={','.join(first['voters'])}", checks)
sys.exit(0 if report["pass"] else 1)
