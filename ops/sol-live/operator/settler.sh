#!/bin/bash
# Settles every finalized Solana payout on the L1 so the next withdrawal is not blocked on custody accounting.
cd /home/postfiat/var/sol-exit/sol-exit-r1
export PYTHONPATH=$PWD/ops/sol-live EXIT_ROLL_PLAN=$HOME/var/sol-exit/sol-exit-r1-timing.json EXIT_WORK=$HOME/var/sol-exit
while :; do
  for burn in $(python3 - <<'P' 2>/dev/null
from exit_payout import *
c=cluster()
for r in rpc(3,"vault_bridge_status",asset_id=ASSET)["redemptions"]:
    if r["bucket_id"]==BUCKET and not r.get("settlement_receipt_hash") and signature(c,r["burn_tx_id"],None): print(r["burn_tx_id"])
P
); do echo "$(date -u +%T) settling $burn"; python3 ~/var/sol-exit/live/settle.py 71dbc9b3 "$burn" 2>&1 | tail -3; done
  sleep 90
done
