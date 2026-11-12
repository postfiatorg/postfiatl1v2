#!/bin/bash
# Roll ONE more validator, then prove six-node agreement with a test transfer.
# Usage: ops/b23/rollout_one.sh N HOST [fix-outbox-ownership]
set -uo pipefail
N=$1; H=$2; FIX=${3:-}
cd /home/postfiat/repos/postfiatl1v2-b23-ops
E=${EVID:-/home/postfiat/repos/b23-coord/evidence/ops/release1}; export PYTHONDONTWRITEBYTECODE=1
if [ "$FIX" = "fix-outbox-ownership" ]; then
  echo "=== v$N: root-owned outbox entries before fix"
  ssh -o BatchMode=yes root@$H "cd /var/lib/postfiat/validator-$N && find certified-send-outbox -user root | tee /root/b23-release1-outbox-root-owned-v$N.txt | wc -l && chown -R postfiat:postfiat certified-send-outbox && echo after=\$(find certified-send-outbox -user root | wc -l)" | tee $E/outbox-ownership-fix-v$N.txt
fi
echo "=== v$N: deploy"
bash ops/b23/deploy_host.sh $N $H || { echo "DEPLOY FAILED v$N"; exit 1; }
echo "=== v$N: fleet check after restart"
python3 -u ops/b23/fleet_check.py $E/fleet-v$N-up.json || { echo "NOT CONVERGED after v$N restart"; exit 2; }
echo "=== v$N: test transfer"
# rpc-serve finality path (NOT pft faucet: its ce22.runtime_binary is the stale 2026-10-27 index-v3 build)
python3 -u ops/b23/submit_transfer.py ${REL:-rel}-test-v$N-$(date +%s) pfab9b9228942e5c529633a13aa271d5297bec6353 /home/postfiat/tmp/navswap-ce22-venue-rebuild-20260719/private/holder-key.json pfee68d0a5a89e8f554bcd942d2f38659f45bfab1b 100000 > $E/test-transfer-v$N.out 2>&1
cat $E/test-transfer-v$N.out | tail -2
HT=$(python3 -c "import json,sys;l=[x for x in open('$E/test-transfer-v$N.out') if x.startswith('{\"label\"')];print(json.loads(l[-1])['height'] if l else '')")
[ -n "$HT" ] || { echo "TEST TRANSFER FAILED v$N"; exit 3; }
echo "height: $HT"
python3 -u ops/b23/check_block.py $E/check-v$N-$HT.json $HT pfee68d0a5a89e8f554bcd942d2f38659f45bfab1b || { echo "DISAGREE at $HT after v$N"; exit 4; }
python3 -u ops/b23/fleet_check.py $E/fleet-v$N-after-tx.json || exit 5
echo "=== v$N: DONE"
