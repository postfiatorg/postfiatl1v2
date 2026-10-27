#!/usr/bin/env python3
"""Read-only after checks for index-v3-20261027 (sheet section 6), through the RPC tunnels and read-only SSH.
Usage: after_checks.py OUTPUT. Writes nothing on any host."""
import json, socket, subprocess, sys
from datetime import datetime, timezone

NEW = "b5cf3ce4bc046e6338beca6d5ead6fd0343426ddb01ac2f461c31e2775f9c8eb"
HOSTS = ["64.176.220.75", "95.179.184.122", "66.42.48.39", "149.28.63.106", "95.179.179.206", "45.32.110.170"]
REMOTE = r'''
import json, subprocess, sys
v, release = sys.argv[1:3]
def sh(*c): return subprocess.run(c, capture_output=True, text=True, timeout=60).stdout
unit = sh("systemctl", "cat", f"postfiat-{v}-rpc.service")
exec_line = next((l for l in unit.splitlines() if l.startswith("ExecStart=")), "")
env = open(f"/etc/postfiat/releases/{release}/{v}.rpc.env").read() + open(f"/etc/postfiat/releases/{release}/{v}.transport.env").read()
df = sh("df", "-B1", "--output=avail", "/").split()
print(json.dumps({"rpc_unit": {"exec_release_binary": exec_line.startswith(f"ExecStart=/opt/postfiat/releases/{release}/postfiat-node "),
                               "max_requests_100000": "--max-requests 100000" in exec_line,
                               "restart_sec_1": "RestartSec=1" in unit.splitlines()},
                  "verified_record_env": env.count("POSTFIAT_DEPLOYMENT_VERIFIED_RECORD="),
                  "active": {u: sh("systemctl", "is-active", u).strip() for u in (f"postfiat-{v}.service", f"postfiat-{v}-rpc.service")},
                  "disk_root_avail_bytes": int(df[1])}))
'''


def rpc(port, method, params=None):
    req = {"version": "postfiat-local-rpc-v1", "id": f"after-{method}", "method": method, "params": params or {}}
    with socket.create_connection(("127.0.0.1", port), timeout=120) as s:
        f = s.makefile("rwb"); f.write((json.dumps(req) + "\n").encode()); f.flush()
        r = json.loads(f.readline(64 << 20))
    if r.get("ok") is not True:
        raise RuntimeError(f"{method} on {port}: {r.get('error')}")
    return r["result"]


out, hosts = {"observed_at": datetime.now(timezone.utc).isoformat()}, {}
for i, host in enumerate(HOSTS):
    v, port = f"validator-{i}", 27650 + i
    st = rpc(port, "status")
    blocks = rpc(port, "blocks", {"from_height": 1154, "limit": 64})
    caps = rpc(port, "fastswap_capabilities")
    idx = rpc(port, "account_tx_index_status")
    remote = json.loads(subprocess.run(["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=15", f"root@{host}", "python3", "-", v, "index-v3-20261027"],
                                       input=REMOTE, capture_output=True, text=True, check=True, timeout=120).stdout)
    hosts[v] = {"height": st["block_height"], "tip": st["block_tip_hash"], "state_root": st["state_root"], "mempool_pending": st["mempool_pending"],
                "binary_sha256": st["deployment_runtime_artifacts"]["binary_sha256"], "build_git_revision": st["build_git_revision"],
                "deployment_manifest_verified": st.get("deployment_manifest_verified"),
                "blocks_since_1153": [(b["header"]["height"], b["header"]["batch_kind"], b["header"]["proposer"]) for b in blocks],
                "fastswap_capabilities": caps, "account_tx_index_status": {k: idx.get(k) for k in ("index_present", "index_usable", "reason", "disk_index_present", "disk_index_usable", "disk_index_reason")},
                "remote": remote}
ident = {(h["height"], h["tip"], h["state_root"]) for h in hosts.values()}
kinds = {k for h in hosts.values() for (_, k, _) in h["blocks_since_1153"]}


def epoch(caps):
    text = json.dumps(caps)
    for key in ("committee_epoch", "epoch"):
        if f'"{key}": 2' in text:
            return 2
    return None


checks = {
    "six_way_agreement": len(ident) == 1,
    "mempools_empty": all(h["mempool_pending"] == 0 for h in hosts.values()),
    "all_six_on_new_executable": all(h["binary_sha256"] == NEW for h in hosts.values()),
    "manifest_verified_on_six": all(h["deployment_manifest_verified"] is True for h in hosts.values()),
    "build_d728f895_on_six": all(str(h["build_git_revision"]).startswith("d728f895") for h in hosts.values()),
    "rpc_units_flags": all(all(h["remote"]["rpc_unit"].values()) for h in hosts.values()),
    "verified_record_env_on_six": all(h["remote"]["verified_record_env"] == 2 for h in hosts.values()),
    "units_active": all(all(s == "active" for s in h["remote"]["active"].values()) for h in hosts.values()),
    "no_governance_block_since_1153": kinds <= {"transparent"},
    "swap_service_committee_epoch_2": all(epoch(h["fastswap_capabilities"]) == 2 for h in hosts.values()),
    "v2_index_reported_unsupported": all(h["account_tx_index_status"]["index_usable"] is False and "unsupported" in str(h["account_tx_index_status"]["reason"]) for h in hosts.values()),
}
out.update(checks=checks, pass_=all(checks.values()), hosts=hosts,
           activation_heights_note="1153, 1153, 1153 at the section 4 import (1153); every block since 1153 is a transparent faucet grant, so no amendment changed them",
           routes_note="both routes authority_epoch 1 at the section 4 import; no governance or route-policy block since")
with open(sys.argv[1], "x") as f:
    json.dump(out, f, indent=1); f.write("\n")
r0 = hosts["validator-0"]
print(("PASS" if out["pass_"] else "FAIL"), f"height={r0['height']} tip={r0['tip'][:12]} root={r0['state_root'][:12]}", checks)
sys.exit(0 if out["pass_"] else 1)
