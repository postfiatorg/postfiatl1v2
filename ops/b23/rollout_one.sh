#!/bin/bash
# Roll ONE more validator, then prove six-node agreement with a test transfer.
# Usage: ops/b23/rollout_one.sh N HOST [fix-outbox-ownership]
set -uo pipefail
N=$1; H=$2; FIX=${3:-}
cd /home/postfiat/repos/postfiatl1v2-b23-ops
E=/home/postfiat/repos/b23-coord/evidence/ops/release1; export PYTHONDONTWRITEBYTECODE=1
if [ "$FIX" = "fix-outbox-ownership" ]; then
  echo "=== v$N: root-owned outbox entries before fix"
  ssh -o BatchMode=yes root@$H "cd /var/lib/postfiat/validator-$N && find certified-send-outbox -user root | tee /root/b23-release1-outbox-root-owned-v$N.txt | wc -l && chown -R postfiat:postfiat certified-send-outbox && echo after=\$(find certified-send-outbox -user root | wc -l)" | tee $E/outbox-ownership-fix-v$N.txt
fi
echo "=== v$N: deploy"
bash ops/b23/deploy_host.sh $N $H || { echo "DEPLOY FAILED v$N"; exit 1; }
echo "=== v$N: fleet check after restart"
python3 -u ops/b23/fleet_check.py $E/fleet-v$N-up.json || { echo "NOT CONVERGED after v$N restart"; exit 2; }
echo "=== v$N: test transfer"
(cd /home/postfiat/repos/StakeHub && timeout 400 .venv/bin/pft faucet pfee68d0a5a89e8f554bcd942d2f38659f45bfab1b --asset PFT --amount 100000) > $E/test-transfer-v$N.out 2>&1
grep -E "ok:|height|proposer|verified" $E/test-transfer-v$N.out
HT=$(awk '/^height:/{print $2}' $E/test-transfer-v$N.out)
[ -n "$HT" ] || { echo "TEST TRANSFER FAILED v$N"; cat $E/test-transfer-v$N.out | tail -5; exit 3; }
python3 -u ops/b23/check_block.py $E/check-v$N-$HT.json $HT pfee68d0a5a89e8f554bcd942d2f38659f45bfab1b || { echo "DISAGREE at $HT after v$N"; exit 4; }
python3 -u ops/b23/fleet_check.py $E/fleet-v$N-after-tx.json || exit 5
echo "=== v$N: DONE"
