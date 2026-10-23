"""Read-only RPC reads through the six tunnels after the rollout. Writes nothing on any host."""
import json, socket, sys
from datetime import datetime, timezone
def rpc(port, method, params=None):
    req = {"version": "postfiat-local-rpc-v1", "id": f"after-{method}", "method": method, "params": params or {}}
    with socket.create_connection(("127.0.0.1", port), timeout=120) as s:
        f = s.makefile("rwb"); f.write((json.dumps(req) + "\n").encode()); f.flush()
        return json.loads(f.readline(64 << 20))
out = {"observed_at": datetime.now(timezone.utc).isoformat(), "hosts": {}}
for i in range(6):
    port = 27650 + i; v = f"validator-{i}"
    st = rpc(port, "status")["result"]
    blocks = rpc(port, "blocks", {"from_height": 1138, "limit": 10})["result"]
    idx = rpc(port, "account_tx_index_status")
    ir = idx.get("result") or {}
    out["hosts"][v] = {
        "height": st["block_height"], "tip": st["block_tip_hash"], "state_root": st["state_root"],
        "build_git_revision": st.get("build_git_revision"), "deployment_manifest_verified": st.get("deployment_manifest_verified"),
        "deployment_manifest_sha256": st.get("deployment_manifest_sha256"), "binary_sha256": st["deployment_runtime_artifacts"]["binary_sha256"],
        "blocks_since_1138": [(b["header"]["height"], b["header"]["batch_kind"], b["header"]["proposer"]) for b in blocks],
        "account_tx_index_status": {k: ir.get(k) for k in ("index_present", "index_usable", "reason", "disk_index_present", "disk_index_usable", "disk_index_reason")} if idx.get("ok") else {"error": idx.get("error")},
    }
at = rpc(27651, "account_tx", {"address": "pf6395ef7f43ffaa74ce178d320b9bbbfd45532b38", "limit": 10})
r = at.get("result") or {}
out["account_tx_validator_1_testing"] = {"ok": at.get("ok"), "error": at.get("error"), "index_used": r.get("index_used"), "row_count": r.get("row_count"), "truncated": r.get("truncated"),
    "rows": [(x["block_height"], x["transaction_kind"], x["from"][:8], x["to"][:8], x["amount"], x.get("receipt_code")) for x in r.get("rows", [])]}
json.dump(out, sys.stdout, indent=1)
