#!/usr/bin/env bash
# Sheet §7 for one host: agreement, stop the RPC unit, build and check the v2 index, start, rejoin, account_tx, agreement.
set -uo pipefail
V="$1"; N="${V#validator-}"
declare -A H=([validator-0]=64.176.220.75 [validator-1]=95.179.184.122 [validator-2]=66.42.48.39 [validator-3]=149.28.63.106 [validator-4]=95.179.179.206 [validator-5]=45.32.110.170)
HOST=${H[$V]}; PORT=$((27650 + N))
D=~/.postfiat/deployments/exchange-readiness-20261023; S=$D/stage; E=$D/evidence/account-tx-index
PACKET=deployments/exchange-readiness-20261023
ALL=validator-1,validator-0,validator-2,validator-3,validator-4,validator-5
P=/opt/postfiat/releases/exchange-readiness-20261023/postfiat-node
SSH="ssh -o BatchMode=yes -o ConnectTimeout=15 root@$HOST"
cd ~/repos/postfiatl1v2-rel20261023
ts() { date -u +%FT%TZ; }
rpc() { local params="${2:-}"; [ -n "$params" ] || params="{}"; python3 - "$PORT" "$1" "$params" <<'PY'
import json, socket, sys
port, method, params = int(sys.argv[1]), sys.argv[2], json.loads(sys.argv[3])
req = {"version": "postfiat-local-rpc-v1", "id": f"index-{method}", "method": method, "params": params}
with socket.create_connection(("127.0.0.1", port), timeout=120) as s:
    f = s.makefile("rwb"); f.write((json.dumps(req) + "\n").encode()); f.flush()
    print(f.readline(64 << 20).decode().strip())
PY
}
echo "== $V $(ts) (a) agreement before"
python3 $PACKET/observe-fleet.py --stage "$S" --applied "$ALL" --output "$E/observe-before-$V.json" | tail -1 || { echo "STOP: observe-before failed"; exit 1; }
$SSH df -B1 --output=avail / | tail -1 > "$E/disk-before-$V.txt"; echo "disk avail $(cat $E/disk-before-$V.txt)"
rpc account_tx_index_status > "$E/index-status-before-$V.json" || { echo "STOP: index status read failed"; exit 1; }
python3 -c 'import json,sys; r=json.load(open(sys.argv[1]))["result"]; print("index before", r["index_present"], r["index_usable"], r["reason"])' "$E/index-status-before-$V.json"
echo "== $V $(ts) (b) stop the RPC unit"
STOP=$(ts); $SSH systemctl stop postfiat-$V-rpc.service; ST=$($SSH systemctl is-active postfiat-$V-rpc.service postfiat-$V.service | tr '\n' ' ')
echo "stopped at $STOP: rpc,validator = $ST"
case "$ST" in "inactive active "*) ;; *) echo "STOP: unexpected unit states after stop: $ST"; $SSH systemctl start postfiat-$V-rpc.service; exit 1;; esac
echo "== $V $(ts) (c) build"
BSTART=$(date +%s.%N)
$SSH "runuser -u postfiat -- $P account-tx-index-build --data-dir /var/lib/postfiat/$V" > "$E/index-build-$V.log" 2>&1; BRC=$?
BEND=$(date +%s.%N); echo "build rc=$BRC seconds=$(echo "$BEND - $BSTART" | bc)" | tee "$E/index-build-$V.time"
$SSH "runuser -u postfiat -- $P account-tx-index-status --data-dir /var/lib/postfiat/$V" > "$E/index-status-$V.json" 2> "$E/index-status-$V.stderr"; SRC=$?
echo "== $V $(ts) (d) start the RPC unit"
START=$(ts); $SSH systemctl start postfiat-$V-rpc.service; SRC2=$?
echo "started at $START (rc=$SRC2)"
[ $BRC -eq 0 ] || { echo "STOP: build failed rc=$BRC (RPC unit restarted; reads fall back to the archive scan)"; tail -c 1500 "$E/index-build-$V.log"; exit 1; }
python3 - "$E/index-status-$V.json" "$E/index-build-$V.log" <<'PY' || { echo "STOP: index not usable"; exit 1; }
import json, sys
st = json.load(open(sys.argv[1])); b = json.load(open(sys.argv[2]))
ok = st["index_usable"] and st["disk_index_usable"] and b.get("index_usable") and b.get("disk_index_usable")
print(("PASS" if ok else "FAIL"), "index", {k: b.get(k) for k in ("indexed_from_height", "indexed_to_height", "indexed_block_count", "indexed_row_count", "account_count", "disk_account_shard_count")}, "status", st["index_usable"], st["disk_index_usable"], st["reason"])
sys.exit(0 if ok else 1)
PY
DEADLINE=$(( $(date +%s) + 300 )); REJOIN=""
while [ $(date +%s) -lt $DEADLINE ]; do
  A=$($SSH systemctl is-active postfiat-$V-rpc.service postfiat-$V.service | tr '\n' ' ')
  R=$($SSH "python3 -c 'import json,os,sys; p=\"/var/lib/postfiat/$V/readiness/rpc.ready.json\"; d=json.load(open(p)); print(d.get(\"ready\"), d.get(\"degraded\"), int(os.path.getmtime(p)))'" 2>/dev/null)
  if [ "$A" = "active active " ] && rpc status > "$E/status-after-start-$V.json" 2>/dev/null && python3 -c 'import json,sys; r=json.load(open(sys.argv[1])); sys.exit(0 if r.get("ok") else 1)' "$E/status-after-start-$V.json" && [ "${R%% *}" = "True" ]; then REJOIN=$(ts); break; fi
  sleep 5
done
[ -n "$REJOIN" ] || { echo "STOP: $V did not rejoin within five minutes (units: $A, readiness: $R)"; exit 1; }
echo "rejoined at $REJOIN; units $A; readiness $R"
rpc account_tx '{"address": "pf6395ef7f43ffaa74ce178d320b9bbbfd45532b38", "limit": 100}' > "$E/account-tx-testing-$V.json"
python3 - "$E/account-tx-testing-$V.json" <<'PY' || { echo "STOP: account_tx check failed"; exit 1; }
import json, sys
r = json.load(open(sys.argv[1])); res = r.get("result") or {}
grants = [x["block_height"] for x in res.get("rows", []) if 1138 <= x["block_height"] <= 1143 and x["from"].startswith("pfcd4cc8") and x["to"].startswith("pf6395ef")]
ok = r.get("ok") and res.get("index_used") is True and grants == list(range(1138, 1144))
print(("PASS" if ok else "FAIL"), "account_tx testing index_used", res.get("index_used"), "rows", res.get("row_count"), "truncated", res.get("truncated"), "grants", grants, r.get("error"))
sys.exit(0 if ok else 1)
PY
echo "== $V $(ts) (f) agreement after"
python3 $PACKET/observe-fleet.py --stage "$S" --applied "$ALL" --output "$E/observe-after-$V.json" | tail -1 || { echo "STOP: observe-after failed"; exit 1; }
$SSH df -B1 --output=avail / | tail -1 > "$E/disk-after-$V.txt"
printf '{"validator_id":"%s","rpc_stopped":"%s","rpc_started":"%s","rejoined":"%s","build":"%s"}\n' "$V" "$STOP" "$START" "$REJOIN" "$(cat $E/index-build-$V.time)" > "$E/times-$V.json"
echo "== $V PASS $(ts)"
