#!/usr/bin/env bash
# Faucet grants (1 PFT, faucet pfcd4cc8... -> testing pf6395ef...) until the tip >= TARGET; each certified identically on six.
set -uo pipefail
TARGET=$1; G=~/.postfiat/deployments/exchange-readiness-20261023/evidence/activation/grants
E=~/.postfiat/deployments/exchange-readiness-20261023/evidence
tip() { python3 -c 'import json,socket; s=socket.create_connection(("127.0.0.1",27651),timeout=60); f=s.makefile("rwb"); f.write((json.dumps({"version":"postfiat-local-rpc-v1","id":"t","method":"status","params":{}})+"\n").encode()); f.flush(); print(json.loads(f.readline(1<<24))["result"]["block_height"])'; }
n=0
while [ "$(tip)" -lt "$TARGET" ]; do
  n=$((n+1)); echo "-- grant $n $(date -u +%FT%TZ) tip $(tip)"
  (cd ~/repos/StakeHub && .venv/bin/pft --json faucet testing --asset PFT --amount 1000000) > $G/grant-$n.out 2> $G/grant-$n.err || { echo "STOP: grant $n failed"; tail -5 $G/grant-$n.err; exit 1; }
  python3 $E/grant-check.py $G/grant-$n.out $G/grant-$n.check.json || { echo "STOP: grant $n not certified identically on six"; exit 1; }
done
echo "== grants done: $n, tip $(tip) $(date -u +%FT%TZ)"
