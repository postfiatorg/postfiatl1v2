#!/usr/bin/env bash
# Run ONLY on the already authorized Vast 53579074 host. All inputs are public.
# This proves an archived synthetic committee receipt, NOT a new Schwab export.
set -euo pipefail
cd /workspace/b23-pool-phase2
exec 9>>/workspace/b23-gpu/prover.lock
flock -w 60 9
test "$(/root/.sp1/bin/sp1-gpu-server --version)" = '6.3.1'
test ! -e fixture-proof
date -u +%FT%TZ > started-utc.txt
sha256sum pftl-uniswap-prover receipt-v2.elf fixture-witness.json > input-sha256.txt
export SP1_PROVER=cuda RAYON_NUM_THREADS=4
./pftl-uniswap-prover program-info --elf receipt-v2.elf --output program-info.json
timeout 1800 ./pftl-uniswap-prover receipt --witness fixture-witness.json \
  --elf receipt-v2.elf --proof-program-version 2 --require-prover cuda --prove \
  --output-dir fixture-proof
date -u +%FT%TZ > finished-utc.txt
