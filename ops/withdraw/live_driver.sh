#!/usr/bin/env bash
# Relay finished checkpoint proofs to Ethereum, then prove and send the withdrawal. Operator key by path only.
set -euo pipefail
E=/home/postfiat/repos/b23-coord/evidence/withdraw; L=$E/live-burn-1; PY=/home/postfiat/repos/postfiatl1v2-live/.venv-ops/bin/python
R="ssh -o BatchMode=yes -o ConnectTimeout=20 -p 20628 root@ssh8.vast.ai"; W=/workspace/b23-withdraw-20261116
cd /home/postfiat/repos/postfiatl1v2-withdraw/ops/withdraw
for S in 1136-1201 1201-1266 1266-1331; do
  until $R "test -s $W/checkpoints/$S/proof/proof-report.json" 2>/dev/null; do sleep 20; done
  mkdir -p $E/live-checkpoints/$S
  for f in proof-report.json public-values.bin proof-calldata.bin; do scp -q -P 20628 root@ssh8.vast.ai:$W/checkpoints/$S/proof/$f $E/live-checkpoints/$S/; done
  echo "SEND $S $(date -u +%T)"; PYTHONDONTWRITEBYTECODE=1 $PY eth_send.py checkpoint $E/live-checkpoints/$S $E/live-checkpoints/$S/eth-result.json
done
echo "CHECKPOINTS DONE $(date -u +%T)"
# stop the remote loop before the unneeded fifth segment, then prove the withdrawal under the shared lock
$R 'for p in $(pgrep -f "[p]rove-checkpoints.sh"); do kill $p; done; sleep 1; for p in $(pgrep -x proof-host); do kill $p; done; true' || true
scp -q -P 20628 $L/exit-witness.json root@ssh8.vast.ai:$W/live-exit-witness.json
test "$(date -u +%H%M)" -lt 1805 || { echo "STOP: daily prover window; run withdrawal proof after the daily reading"; exit 75; }
$R "cd $W && (exec 9>/workspace/b23-gpu/prover.lock; flock -w 600 9; SP1_PROVER=cuda RAYON_NUM_THREADS=4 timeout 1500 ./proof-host withdrawal egress.elf live-exit-witness.json live-withdrawal-proof prove > live-withdrawal-prove.log 2>&1); tail -1 live-withdrawal-prove.log"
mkdir -p $L/proof; for f in proof-report.json public-values.bin proof-calldata.bin; do scp -q -P 20628 root@ssh8.vast.ai:$W/live-withdrawal-proof/$f $L/proof/; done
cmp $L/proof/public-values.bin $L/exit-native/native-public-values.bin
echo "WITHDRAW SEND $(date -u +%T)"; PYTHONDONTWRITEBYTECODE=1 $PY eth_send.py withdraw $L/proof $L/eth-withdrawal.json
echo "ALL DONE $(date -u +%T)"
