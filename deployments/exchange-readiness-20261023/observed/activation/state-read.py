#!/usr/bin/env python3
"""Read-only state read on all six through the tunnels: status, the governance blocks given, their receipts and
archived amendments. Usage: state-read.py OUTPUT [HEIGHT:AMENDMENT_ID ...]. Writes nothing on any host."""
import json, socket, sys
from datetime import datetime, timezone
def rpc(port, method, params=None):
    req = {"version": "postfiat-local-rpc-v1", "id": f"activation-{method}", "method": method, "params": params or {}}
    with socket.create_connection(("127.0.0.1", port), timeout=120) as s:
        f = s.makefile("rwb"); f.write((json.dumps(req) + "\n").encode()); f.flush()
        r = json.loads(f.readline(64 << 20))
    if r.get("ok") is not True: raise RuntimeError(f"{method} on {port}: {r.get('error')}")
    return r["result"]
out_path = sys.argv[1]; targets = [t.split(":", 1) for t in sys.argv[2:]]
hosts = {}
for i in range(6):
    port = 27650 + i; v = f"validator-{i}"
    st = rpc(port, "status"); mp = rpc(port, "mempool_status")
    row = {"height": st["block_height"], "tip": st["block_tip_hash"], "state_root": st["state_root"], "mempool_pending": st["mempool_pending"],
           "build_git_revision": st["build_git_revision"], "binary_sha256": st["deployment_runtime_artifacts"]["binary_sha256"], "governance": {}}
    for height, amendment_id in targets:
        b = rpc(port, "blocks", {"from_height": int(height), "limit": 1})[0]; h = b["header"]; c = h["certificate"]
        arch = rpc(port, "batch_archive", {"batch_kind": h["batch_kind"], "batch_id": h["batch_id"], "limit": 1})
        amendments = json.loads(arch[0]["payload_json"]).get("amendments", []) if arch else []
        receipt = rpc(port, "tx", {"tx_id": amendment_id})["receipt"]
        row["governance"][height] = {
            "height": h["height"], "batch_kind": h["batch_kind"], "batch_id": h["batch_id"], "block_hash": h["block_hash"], "state_root": h["state_root"],
            "proposer": h["proposer"], "view": h["view"], "certificate_id": h["certificate_id"], "quorum": c["quorum"],
            "accepting_votes": sorted(x["validator"] for x in c["votes"] if x.get("accept") is True), "receipt_ids": b["receipt_ids"],
            "amendments": [{"amendment_id": a["amendment_id"], "kind": a["kind"], "value": a["value"], "quorum": a.get("quorum"), "support": len(a.get("support", [])),
                            "signed_authorizations": len(a.get("signed_authorizations", [])), "paused": a.get("paused", False)} for a in amendments],
            "receipt": receipt}
    hosts[v] = row
ident = {(r["height"], r["tip"], r["state_root"]) for r in hosts.values()}
gov_same = all(json.dumps(r["governance"], sort_keys=True) == json.dumps(hosts["validator-0"]["governance"], sort_keys=True) for r in hosts.values())
checks = {"six_way_agreement": len(ident) == 1, "mempools_empty": all(r["mempool_pending"] == 0 for r in hosts.values()),
          "governance_identical_on_6": gov_same,
          "uniform_binary": len({r["binary_sha256"] for r in hosts.values()}) == 1}
for height, amendment_id in targets:
    g = hosts["validator-0"]["governance"][height]
    checks[f"{height}_governance_block_quorum_receipt_accepted"] = (g["batch_kind"] == "governance" and len(g["accepting_votes"]) >= g["quorum"] == 5
        and g["receipt"]["accepted"] is True and amendment_id in g["receipt_ids"] and len(g["amendments"]) == 1 and g["amendments"][0]["amendment_id"] == amendment_id)
out = {"observed_at": datetime.now(timezone.utc).isoformat(), "checks": checks, "pass": all(checks.values()), "hosts": hosts}
with open(out_path, "x") as f: json.dump(out, f, indent=1); f.write("\n")
r0 = hosts["validator-0"]
print(("PASS" if out["pass"] else "FAIL"), f"height={r0['height']} tip={r0['tip'][:12]} root={r0['state_root'][:12]}", checks,
      {h: (g["amendments"][0]["kind"], g["amendments"][0]["value"], g["receipt"]["code"]) for h, g in r0["governance"].items() if g["amendments"]})
sys.exit(0 if out["pass"] else 1)
