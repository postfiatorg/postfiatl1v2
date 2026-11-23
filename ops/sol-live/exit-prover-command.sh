#!/bin/bash
# Forced command for the dedicated credential on the prover.
set -euo pipefail
umask 077
case "${SSH_ORIGINAL_COMMAND:-}" in
    pf-sol-exit-check) cd /workspace/b23-withdraw-20261116; test -x proof-host && test -s egress.elf; exit;;
    pf-sol-exit-checkpoint) kind=checkpoint;;
    pf-sol-exit-withdrawal) kind=withdrawal;;
    *) exit 64;;
esac
cd /workspace/b23-withdraw-20261116
exec 9>/workspace/b23-gpu/prover.lock
flock -E 75 -n 9
job=jobs/sol-exit-worker
mkdir -p jobs
rm -rf -- "$job"
mkdir "$job"
trap 'rm -rf -- "$job"' EXIT
# Accept at most the worker's 24 MiB witness limit.
head -c 25165825 > "$job/witness.json"
test "$(wc -c < "$job/witness.json")" -le 25165824
SP1_PROVER=cuda RAYON_NUM_THREADS=4 timeout 1500 ./proof-host "$kind" egress.elf "$job/witness.json" "$job/proof" prove > "$job/prove.log" 2>&1
for file in public-values.bin proof-report.json proof-calldata.bin; do
    test "$(wc -c < "$job/proof/$file")" -le 8192
    base64 -w0 "$job/proof/$file"
    echo
done
