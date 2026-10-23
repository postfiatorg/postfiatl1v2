# Read-only. Runs on each host as: python3 - <validator> <release>
import json, os, subprocess, sys
from datetime import datetime, timezone
v, release = sys.argv[1:3]
def sh(*c): return subprocess.run(c, capture_output=True, text=True, timeout=60).stdout
out = {}
unit = sh("systemctl", "cat", f"postfiat-{v}-rpc.service")
exec_line = next((l for l in unit.splitlines() if l.startswith("ExecStart=")), "")
out["rpc_unit"] = {"exec_release_binary": exec_line.startswith(f"ExecStart=/opt/postfiat/releases/{release}/postfiat-node "),
                   "max_requests_100000": "--max-requests 100000" in exec_line,
                   "restart_always": "Restart=always" in unit.splitlines(),
                   "restart_sec_1": "RestartSec=1" in unit.splitlines()}
logs = {}
for name in ("transport-validator-events.ndjson", "rpc-events.ndjson"):
    st = os.stat(f"/var/log/postfiat/{v}/{name}")
    logs[name] = {"bytes": st.st_size, "mtime": datetime.fromtimestamp(int(st.st_mtime), timezone.utc).isoformat()}
by_height, opens = {}, {}
with open(f"/var/log/postfiat/{v}/transport-validator-events.ndjson", "rb") as f:
    f.seek(max(0, os.path.getsize(f.name) - 8 * 1024 * 1024))
    for line in f.read().splitlines():
        if b'"kind":"batch"' not in line: continue
        try: e = json.loads(line)
        except ValueError: continue
        a = e.get("batch_ack") or {}
        h = (a.get("certified_state") or {}).get("block_height")
        if e.get("outcome") == "accepted" and a.get("applied") and h and h >= 1138:
            by_height[str(h)] = by_height.get(str(h), 0) + 1
            opens[str(h)] = (a.get("storage_work") or {}).get("database_opens")
logs["transport_accepted_batch_events_by_grant_height"] = by_height
logs["storage_work_database_opens_by_height"] = opens
out["event_logs"] = logs
df = sh("df", "-B1", "--output=avail", "/").split()
out["disk_root_avail_bytes"] = int(df[1])
helptext = subprocess.run([f"/opt/postfiat/releases/{release}/postfiat-node", "--help"], capture_output=True, text=True, timeout=30)
out["help_lists_account_tx_index_build"] = "account-tx-index-build" in (helptext.stdout + helptext.stderr)
print(json.dumps(out))
