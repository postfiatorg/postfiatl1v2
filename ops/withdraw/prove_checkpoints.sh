#!/usr/bin/env bash
# Run on the existing Vast instance only. Each proof owns the shared lock.
# The date-bound cutoff leaves the 17:55 UTC daily job its prover window.
set -euo pipefail
cd /workspace/b23-withdraw-20261116
for segment in 1071-1136 1136-1201 1201-1266 1266-1331 1331-1370; do
  test "$(date -u +%Y%m%d)" = 20261116
  if test "$(date -u +%H%M)" -ge 1735; then
    echo "STOP: preserve daily prover window before starting $segment"
    exit 75
  fi
  if test -s "checkpoints/$segment/proof/proof-report.json"; then continue; fi
  (
    exec 9>/workspace/b23-gpu/prover.lock
    flock -n 9
    echo "START $segment $(date -u +%FT%TZ)"
    SP1_PROVER=cuda RAYON_NUM_THREADS=4 timeout 1200 ./proof-host checkpoint \
      egress.elf "checkpoints/$segment/witness.json" "checkpoints/$segment/proof" prove \
      > "checkpoints/$segment/prove.log" 2>&1
    echo "PASS $segment $(date -u +%FT%TZ)"
  )
done
