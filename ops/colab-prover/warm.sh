#!/bin/bash
# Runs ON the Colab VM: downloads the circuit files by proving once, then starts the warm deposit prover.
set -euo pipefail
W=/workspace/b23-withdraw-20261116; D=/workspace/sol-ingress/speed-i2; H=/root/dep-home
cd $W
( exec 9>>/workspace/b23-gpu/prover.lock; flock -w 2400 9
  SP1_PROVER=cuda RAYON_NUM_THREADS=$(nproc) timeout 2400 ./proof-host withdrawal egress.elf /content/bundle/warm-witness.json jobs/warm-proof prove > jobs/warm.log 2>&1 )
grep -o "verified real Groth16 proof in [0-9]* ms" jobs/warm.log
mkdir -p $H/.sp1 && ln -sfn /root/.sp1/circuits $H/.sp1/circuits
if [ "${1:-}" = deposits ]; then
  cd $D; rm -f prover.sock
  HOME=$H RAYON_NUM_THREADS=$(( $(nproc) > 32 ? 32 : $(nproc) )) SP1_WORKER_NUM_CORE_WORKERS=8 SP1_WORKER_CORE_BUFFER_SIZE=8 SP1_WORKER_VERIFY_INTERMEDIATES=false \
    SP1_GROTH16_CIRCUIT_PATH=/root/.sp1/circuits/groth16 nohup setsid python3 serve.py --lock prover.lock --prime prime.bin ./host-live elf $D/prover.sock 1 > serve.log 2>&1 < /dev/null &
  sleep 8; until [ -S prover.sock ] || ! pgrep -f "serve.py --lock" >/dev/null; do sleep 3; done
  [ -S prover.sock ] && grep -o '"ready_seconds":[0-9.]*' serve.log || { tail -3 serve.log; exit 1; }
fi
echo WARM
