#!/usr/bin/env bash
# Run on the dedicated four-GPU host from its speed2 installation.
set -euo pipefail
cd /workspace/sol-ingress/speed-i2
export RAYON_NUM_THREADS=32
# Measured on a 19 MB mainnet block: 8 core workers per GPU cut leaves 36 s -> 28 s and the
# final proof 39 s -> 29 s; the final proof is still verified before it is returned.
export SP1_WORKER_NUM_CORE_WORKERS=8 SP1_WORKER_CORE_BUFFER_SIZE=8 SP1_WORKER_VERIFY_INTERMEDIATES=false
export SP1_GROTH16_CIRCUIT_PATH="$PWD/circuit-install"
# The SP1 installer uses this marker to recognize a completed artifact install.
(cd circuit-install/v6.1.0 && sha256sum -c ../../circuits.sha256 && touch .complete)
# Clear what a dead prover leaves behind, but only while no running prover holds prover.lock:
# its socket, which serve.py refuses to start over, and its proving processes, which outlive
# serve.py and keep the GPU memory, so the next prover runs out of memory on its first proof.
flock -n prover.lock bash -c 'pkill -9 -f "^$PWD/host-live serve "; pkill -9 -x sp1-gpu-server; rm -f prover.sock' || true
exec python3 serve.py --lock prover.lock --prime prime.bin ./host-live elf "$PWD/prover.sock" 4
