#!/bin/bash
# Second release: settles every finalized Solana payout of any covered batch on the L1.
# Each validator's own reviewer approves the payout; this loop only drafts and collects (5 of 6).
cd /home/postfiat/var/sol-exit/sol-exit-r2
export PYTHONPATH=$PWD/ops/sol-live EXIT_WORK=$HOME/var/sol-exit
while :; do
  for burn in $(python3 - <<'P' 2>/dev/null
from exit_payout import *
from exit_payout import covered_buckets, signature
c=cluster()
s=rpc(3,"vault_bridge_status",asset_id=ASSET)
ok=covered_buckets(s["buckets"])
for r in s["redemptions"]:
    if r["bucket_id"] in ok and not r.get("settlement_receipt_hash") and r.get("state")=="pending" and signature(c,r["burn_tx_id"],None): print(r["burn_tx_id"])
P
); do echo "$(date -u +%T) settling $burn"; python3 ops/sol-live/oncall/settle.py 04600868 "$burn" 2>&1 | tail -3; done
  sleep 90
done
