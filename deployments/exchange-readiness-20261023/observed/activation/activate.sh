#!/usr/bin/env bash
# One activation amendment: ratify (validator-1), sign on each of the six hosts with its own key file (by path),
# assemble + batch + certified governance round on the proposer of SLOT, then the six-host state read.
# v2 (2026-10-23 after the 1144 view change): the round uses the live-fleet flags of scripts/pfusdc-private-send-live
# (--quorum-early-full-propagation --local-apply-before-certified-send): without them the round sends the certified
# block to its own validator service while it holds the store lease (transport_protocol.rs:910-918,
# transport_runtime.rs:2838), which resets that connection (the 1144 view-1 self-delivery failure).
set -uo pipefail
K="$1"; SLOT="$2"; H="$3"; PROP="$4"
A=~/.postfiat/deployments/exchange-readiness-20261023/evidence/activation; L=$A/$K; mkdir -p $L
declare -A IP=([validator-0]=64.176.220.75 [validator-1]=95.179.184.122 [validator-2]=66.42.48.39 [validator-3]=149.28.63.106 [validator-4]=95.179.179.206 [validator-5]=45.32.110.170)
ALL=validator-0,validator-1,validator-2,validator-3,validator-4,validator-5
P=/opt/postfiat/releases/exchange-readiness-20261023/postfiat-node
W=/var/lib/postfiat/governance-activation-20261023
TOPO=/etc/postfiat/releases/exchange-readiness-20261023/topology.json
X=$((SLOT + 10))
ssh_() { ssh -o BatchMode=yes -o ConnectTimeout=15 root@${IP[$1]} "${@:2}"; }
ts() { date -u +%FT%TZ; }
echo "== $K $(ts) slot=$SLOT H=$H expires=$X proposer=$PROP"
python3 - "$SLOT" <<'PY' || { echo "STOP: tip is not SLOT-1 on all six"; exit 1; }
import json, socket, sys
slot = int(sys.argv[1]); seen = set()
for i in range(6):
    with socket.create_connection(("127.0.0.1", 27650 + i), timeout=60) as s:
        f = s.makefile("rwb"); f.write((json.dumps({"version": "postfiat-local-rpc-v1", "id": "pre", "method": "status", "params": {}}) + "\n").encode()); f.flush()
        r = json.loads(f.readline(1 << 24))["result"]; seen.add((r["block_height"], r["block_tip_hash"], r["mempool_pending"]))
print("tip", seen); sys.exit(0 if len(seen) == 1 and next(iter(seen))[0] == slot - 1 and next(iter(seen))[2] == 0 else 1)
PY
PR=$(ssh_ validator-1 "runuser -u postfiat -- $P block-proposer --data-dir /var/lib/postfiat/validator-1 --height $SLOT" | python3 -c 'import json,sys; print(json.load(sys.stdin)["proposer"])')
[ "$PR" = "$PROP" ] || { echo "STOP: proposer of $SLOT is $PR, expected $PROP"; exit 1; }
for v in validator-0 validator-1 validator-2 validator-3 validator-4 validator-5; do ssh_ $v "install -d -o postfiat -g postfiat -m 0700 $W"; done
echo "-- ratify on validator-1"
ssh_ validator-1 "runuser -u postfiat -- $P ratify-$K-activation-height --data-dir /var/lib/postfiat/validator-1 --validators $ALL --height $H --amendment-file $W/$K.unsigned.json" > $L/ratify.stdout 2> $L/ratify.stderr || { echo "STOP: ratify failed"; cat $L/ratify.stderr | head -5; exit 1; }
scp -q root@${IP[validator-1]}:$W/$K.unsigned.json $L/$K.unsigned.json
AID=$(python3 -c 'import json,sys; a=json.load(open(sys.argv[1])); print(a["amendment_id"]); print(a["kind"], a["value"], a["quorum"], a["support"], a["paused"], a["activation_height"], a["veto_until_height"], file=sys.stderr)' $L/$K.unsigned.json)
echo "amendment $AID"
USHA=$(sha256sum $L/$K.unsigned.json | cut -d' ' -f1)
echo "-- sign on each host"
for v in validator-0 validator-1 validator-2 validator-3 validator-4 validator-5; do
  [ "$v" = validator-1 ] || { scp -q $L/$K.unsigned.json root@${IP[$v]}:$W/$K.unsigned.json && ssh_ $v "chown postfiat:postfiat $W/$K.unsigned.json && chmod 600 $W/$K.unsigned.json"; }
  ssh_ $v "test \"\$(sha256sum $W/$K.unsigned.json | cut -d' ' -f1)\" = $USHA && runuser -u postfiat -- $P governance-authorization-sign --data-dir /var/lib/postfiat/$v --amendment-file $W/$K.unsigned.json --validator $v --validator-key-file /var/lib/postfiat/$v/validator_keys.json --proposal-slot $SLOT --expires-at-height $X --authorization-file $W/$K.authorization-$v.json" > $L/sign-$v.stdout 2> $L/sign-$v.stderr || { echo "STOP: sign failed on $v"; head -5 $L/sign-$v.stderr; exit 1; }
  scp -q root@${IP[$v]}:$W/$K.authorization-$v.json $L/$K.authorization-$v.json || { echo "STOP: fetch authorization $v"; exit 1; }
  echo "signed $v"
done
echo "-- assemble, batch and round on $PROP"
for v in validator-0 validator-1 validator-2 validator-3 validator-4 validator-5; do
  [ "$v" = "$PROP" ] || scp -q $L/$K.authorization-$v.json root@${IP[$PROP]}:$W/$K.authorization-$v.json
done
[ "$PROP" = validator-1 ] || true
ssh_ $PROP "chown postfiat:postfiat $W/$K.* && chmod 600 $W/$K.*"
AUTH=$(for v in validator-0 validator-1 validator-2 validator-3 validator-4 validator-5; do printf '%s,' "$W/$K.authorization-$v.json"; done); AUTH=${AUTH%,}
ssh_ $PROP "runuser -u postfiat -- $P governance-amendment-assemble --data-dir /var/lib/postfiat/$PROP --amendment-file $W/$K.unsigned.json --authorization-files $AUTH --proposal-slot $SLOT --output $W/$K.signed.json" > $L/assemble.stdout 2> $L/assemble.stderr || { echo "STOP: assemble refused"; head -5 $L/assemble.stderr; exit 1; }
ssh_ $PROP "runuser -u postfiat -- $P governance-batch --data-dir /var/lib/postfiat/$PROP --amendment-file $W/$K.signed.json --batch-file $W/$K.batch.json" > $L/batch.stdout 2> $L/batch.stderr || { echo "STOP: governance-batch refused"; head -5 $L/batch.stderr; exit 1; }
scp -q root@${IP[$PROP]}:$W/$K.signed.json $L/$K.signed.json; scp -q root@${IP[$PROP]}:$W/$K.batch.json $L/$K.batch.json
echo "batch $(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["batch_id"])' $L/$K.batch.json) sha256 $(sha256sum $L/$K.batch.json | cut -c1-16)"
echo "-- round $(ts)"
ssh_ $PROP "runuser -u postfiat -- $P transport-peer-certified-batch-round --data-dir /var/lib/postfiat/$PROP --topology $TOPO --batch-kind governance --batch-file $W/$K.batch.json --key-file /var/lib/postfiat/$PROP/validator_keys.json --require-local-proposer --height $SLOT --view 0 --quorum-early-full-propagation --local-apply-before-certified-send --timeout-ms 60000 --send-retries 3 --retry-backoff-ms 2000 --artifact-dir $W/$K-round" > $L/round.json 2> $L/round.stderr; RC=$?
echo "round rc=$RC $(ts)"; [ $RC -eq 0 ] || { echo "STOP: round failed"; tail -c 1500 $L/round.stderr; exit 1; }
scp -q -r root@${IP[$PROP]}:$W/$K-round $L/round-artifacts 2>/dev/null
sleep 5
python3 $A/state-read.py $L/state-after.json $SLOT:$AID || { echo "STOP: state read after $K failed"; exit 1; }
echo "== $K PASS $(ts)"
