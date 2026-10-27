#!/usr/bin/env bash
# Sheet section 7 for one host: stop only its RPC unit, build the account_tx index with the release executable,
# read its status, start the RPC unit, then check through the tunnel that the index is v3, usable to the tip and
# answers account_tx. Usage: index_step.sh N HOST. No block may be produced while this runs.
set -uo pipefail
N="$1"; HOST="$2"
OUT=$HOME/repos/postfiatl1v2-rel20261027/deployments/index-v3-20261027/observed/account-tx-index
mkdir -p "$OUT"
PORT=$((27650 + N))
TESTING=pf6395ef7f43ffaa74ce178d320b9bbbfd45532b38
ssh -o BatchMode=yes -o ConnectTimeout=15 "root@$HOST" bash -s -- "$N" > "$OUT/index-step-validator-$N.log" 2>&1 <<'REMOTE'
set -euo pipefail
N="$1"; P=/opt/postfiat/releases/index-v3-20261027/postfiat-node; DATA=/var/lib/postfiat/validator-$N
test "$(sha256sum "$P" | cut -d' ' -f1)" = b5cf3ce4bc046e6338beca6d5ead6fd0343426ddb01ac2f461c31e2775f9c8eb
echo "before $(runuser -u postfiat -- "$P" account-tx-index-status --data-dir "$DATA" | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["index_usable"], d["reason"], d["disk_index_usable"], d["disk_index_reason"])')"
echo "rpc_stop $(date -u +%T)"; systemctl stop "postfiat-validator-$N-rpc.service"
t0=$(date +%s.%N)
runuser -u postfiat -- "$P" account-tx-index-build --data-dir "$DATA" > "/tmp/index-build-validator-$N.json"
t1=$(date +%s.%N)
echo "build_seconds $(python3 -c "print(round($t1 - $t0, 3))")"
cat "/tmp/index-build-validator-$N.json"; rm -f "/tmp/index-build-validator-$N.json"
runuser -u postfiat -- "$P" account-tx-index-status --data-dir "$DATA"
python3 -c "import json; print('meta_schema', json.load(open('$DATA/account_tx_index_meta.json'))['schema'], 'aggregate_schema', json.load(open('$DATA/account_tx_index.json'))['schema'])"
systemctl start "postfiat-validator-$N-rpc.service"; echo "rpc_start $(date -u +%T)"
systemctl is-active "postfiat-validator-$N.service" "postfiat-validator-$N-rpc.service"
REMOTE
rc=$?
echo "remote rc=$rc"; test $rc -eq 0 || { tail -20 "$OUT/index-step-validator-$N.log"; exit 1; }
python3 - "$PORT" "$TESTING" "$OUT" "$N" <<'PY'
import json, socket, sys, time
port, testing, out, n = int(sys.argv[1]), sys.argv[2], sys.argv[3], sys.argv[4]
def rpc(method, params=None):
    req = {"version": "postfiat-local-rpc-v1", "id": f"index-{method}", "method": method, "params": params or {}}
    with socket.create_connection(("127.0.0.1", port), timeout=60) as s:
        f = s.makefile("rwb"); f.write((json.dumps(req) + "\n").encode()); f.flush()
        r = json.loads(f.readline(64 << 20))
    if r.get("ok") is not True: raise RuntimeError(f"{method}: {r.get('error')}")
    return r["result"]
start = time.time(); last = None
while time.time() - start < 120:
    try:
        st = rpc("account_tx_index_status"); break
    except Exception as e:
        last = e; time.sleep(2)
else:
    raise SystemExit(f"FAIL rpc not ready on {port}: {last}")
ready = round(time.time() - start, 1)
tx = rpc("account_tx", {"address": testing, "limit": 100})
status = rpc("status")
rec = {"validator_id": f"validator-{n}", "rpc_ready_after_start_seconds": ready, "index_status": st,
       "account_tx_testing": {k: tx.get(k) for k in ("index_used", "row_count", "truncated", "scanned_block_count", "archive_lookup_count")},
       "testing_last_heights": [r["block_height"] for r in tx["rows"][-6:]], "status_height": status["block_height"], "status_tip": status["block_tip_hash"]}
json.dump(rec, open(f"{out}/after-index-validator-{n}.json", "w"), indent=1)
ok = (st["index_usable"] and st["disk_index_usable"] and st.get("disk_index_lag_blocks") == 0 and st.get("index_lag_blocks") == 0
      and st["disk_indexed_to_height"] == status["block_height"] and tx["index_used"] is True and tx["archive_lookup_count"] == 0)
print(("PASS" if ok else "FAIL"), f"validator-{n}", "v3 usable", st["index_usable"], st["disk_index_usable"], "to", st["disk_indexed_to_height"],
      "lag", st.get("index_lag_blocks"), st.get("disk_index_lag_blocks"), "testing rows", tx["row_count"], "index_used", tx["index_used"], "rpc ready", ready, "s")
sys.exit(0 if ok else 1)
PY
