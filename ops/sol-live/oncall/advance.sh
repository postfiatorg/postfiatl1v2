#!/bin/bash
# Advance the idle chain to the second release height with small faucet payments.
cd ~/var/sol-exit/sol-exit-r2; export PYTHONPATH=$PWD/ops/sol-live
tip(){ python3 -c 'from live import fleet; print(fleet()["block_height"])'; }
while :; do
  t=$(tip) || { sleep 20; continue; }
  [ "$t" -ge 1890 ] && { echo "REACHED $t $(date -u +%T)"; break; }
  timeout 300 pft faucet b23-bob --asset PFT --amount 1000 2>&1 | grep -E "^height|verified|error" | tr '\n' ' '; echo " tip_before=$t $(date -u +%T)"
done
