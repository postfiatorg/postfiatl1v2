#!/usr/bin/env bash
# account-tx index v2 on a scratch copy of the signed 1131 import, then the rollback binary on the same data. Local only.
set -uo pipefail
R=$HOME/.cache/release-exchange-readiness-20261023; NEW=$R/binaries/candidate-1; OLD=$R/binaries/rollback
I=$R/index-check; mkdir -p $I/out; D=$I/data
cp -a $R/canary-1131/signed-import $D
T=pf6395ef7f43ffaa74ce178d320b9bbbfd45532b38
step() { local name=$1; shift; local t0=$(date +%s.%N); "$@" > $I/out/$name.json 2> $I/out/$name.stderr; local rc=$?; local t1=$(date +%s.%N); printf '%s rc=%s seconds=%.2f\n' "$name" "$rc" "$(echo "$t1 - $t0" | bc)"; }
step new-index-status-before $NEW account-tx-index-status --data-dir $D
step new-index-build $NEW account-tx-index-build --data-dir $D
step new-index-status-after $NEW account-tx-index-status --data-dir $D
step new-account-tx-testing $NEW account-tx --data-dir $D --address $T --limit 20
step new-verify-state-after-index $NEW verify-state --data-dir $D
step old-index-status-on-v2 $OLD account-tx-index-status --data-dir $D
step old-account-tx-on-v2 $OLD account-tx --data-dir $D --address $T --limit 20
step old-status-on-v2 $OLD status --data-dir $D
step old-checkpoint-on-v2 $OLD verify-finalized-checkpoint --data-dir $D
step old-verify-state-on-v2 $OLD verify-state --data-dir $D
step old-index-build-over-v2 $OLD account-tx-index-build --data-dir $D
step old-index-status-after-rebuild $OLD account-tx-index-status --data-dir $D
step new-index-status-on-v1 $NEW account-tx-index-status --data-dir $D
step new-account-tx-on-v1 $NEW account-tx --data-dir $D --address $T --limit 20
echo done
