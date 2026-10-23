#!/usr/bin/env bash
# account-tx index on a scratch copy of the signed 1131 import with a given binary. Local only.
set -uo pipefail
BIN=$1; OUT=$2
R=$HOME/.cache/release-exchange-readiness-20261023-r2; D=$OUT/data; mkdir -p $OUT/out
cp -a $R/canary-1131/signed-import $D
T=pf6395ef7f43ffaa74ce178d320b9bbbfd45532b38; F=pfcd4cc8c354bd340fe18386e6f875ff68f215a95c
G1=219306e62688976275b0c98499a4f43e2d69ced8e2d2ee12c35ca486896ed2f4fbc92fcfa5c997e1614d69ac20ad39d9
G2=c9acb9745bd610893cbbd0288916d098a2af9bf35a08a5002e44c33ed944391b7025fadd1f007c0a70e8520274f0b917
step() { local name=$1; shift; local t0=$(date +%s.%N); "$@" > $OUT/out/$name.json 2> $OUT/out/$name.stderr; local rc=$?; local t1=$(date +%s.%N); printf '%s rc=%s seconds=%.2f\n' "$name" "$rc" "$(echo "$t1 - $t0" | bc)"; }
step status-before $BIN account-tx-index-status --data-dir $D
step scan-testing $BIN account-tx --data-dir $D --address $T --limit 50
step index-build $BIN account-tx-index-build --data-dir $D
step status-after $BIN account-tx-index-status --data-dir $D
step indexed-testing $BIN account-tx --data-dir $D --address $T --limit 50
step indexed-faucet $BIN account-tx --data-dir $D --address $F --limit 50
step tx-219306e6 $BIN rpc --method tx --data-dir $D --tx-id $G1
step tx-c9acb974 $BIN rpc --method tx --data-dir $D --tx-id $G2
step index-build-again $BIN account-tx-index-build --data-dir $D
echo done
