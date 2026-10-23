#!/usr/bin/env bash
# Exact replays through benchmarks/storage-scaling/run_replay_evidence.py with the new build (local only).
set -uo pipefail
R=$HOME/.cache/release-exchange-readiness-20261023-r2
while [ ! -s $R/node-builds.json ]; do sleep 5; done
python3 -c 'import json,sys; assert json.load(open(sys.argv[1]))["result"]=="PASS"' $R/node-builds.json || { echo "builds did not reproduce"; exit 1; }
NODE=$R/target-1/release/postfiat-node; SRCREV=6b1a05854c0b3a0ac0dec583fa09ff380d95d952
H=$R/source-1/benchmarks/storage-scaling
TREE='import sys; sys.path.insert(0, sys.argv[1]); import importlib.util as u; s=u.spec_from_file_location("h", sys.argv[1]+"/run_replay_evidence.py"); m=u.module_from_spec(s); s.loader.exec_module(m); print(m.tree_sha256(__import__("pathlib").Path(sys.argv[2])), sys.argv[2])'
export PYTHONDONTWRITEBYTECODE=1
# 1. height 924, legacy-source mode
SRC=$HOME/.postfiat/deployments/storage-rollout-10dd9f20/g6-rehearsal-20260830-v3/backups/validator-0
MAN=$HOME/.postfiat/deployments/storage-rollout-10dd9f20/g6-rehearsal-20260830-v3/generations/initial-migration/validator-0/storage-migration-manifest.json
B=$R/replay-924; mkdir -p $B/scratch $B/tmp
read -r TIP ROOT < <(python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); print(d["tip_hash"], d["state_root"])' "$HOME/.cache/g3-924-replay-20261022/receipts/height-924-harness/receipts/height-924.json")
echo "== $(date -u +%FT%TZ) 924 start tip=${TIP:0:8} root=${ROOT:0:8}"
python3 -c "$TREE" "$H" "$SRC" | tee $B/source-tree-before.txt
cp -a "$SRC" $B/scratch/validator-0
python3 -c "$TREE" "$H" $B/scratch/validator-0 | tee $B/source-tree-after-copy.txt
(cd $R/source-1 && TMPDIR=$B/tmp /usr/bin/time -v python3 benchmarks/storage-scaling/run_replay_evidence.py \
  --node-bin $NODE --legacy-source-dir $B/scratch/validator-0 --recorded-manifest $MAN \
  --expected-height 924 --expected-tip $TIP --expected-root $ROOT \
  --output-dir $B/out --expected-source-revision $SRCREV) > $B/harness.log 2>&1; rc=$?
echo "== $(date -u +%FT%TZ) 924 harness exit $rc"
python3 -c "$TREE" "$H" "$SRC" | tee $B/source-tree-after.txt
rm -rf $B/scratch $B/tmp
# 2. current head: the newest signed export on this server (1131, 2026-10-21 canary), signed-archive mode
E=$HOME/.postfiat/deployments/store-handle-20261021/evidence
C=$R/replay-1131; mkdir -p $C/scratch $C/tmp
read -r CTIP CROOT < <(python3 -c 'import json,sys; d=json.load(open(sys.argv[1]))["backup"]; print(d["tip"], d["state_root"])' $E/rollout-state.json)
echo "== $(date -u +%FT%TZ) 1131 start tip=${CTIP:0:8} root=${CROOT:0:8}"
python3 -c "$TREE" "$H" $E/pre-rollout-backup/backup-signed | tee $C/archive-tree-before.txt
cp -a $E/pre-rollout-backup/backup-signed $C/scratch/backup-signed
(cd $R/source-1 && TMPDIR=$C/tmp /usr/bin/time -v python3 benchmarks/storage-scaling/run_replay_evidence.py \
  --node-bin $NODE --signed-archive-dir $C/scratch/backup-signed \
  --trusted-publisher-file $HOME/.postfiat/deployments/cobalt-activation-8694b99d/snapshot-keys/snapshot-publisher.public.json \
  --expected-height 1131 --expected-tip $CTIP --expected-root $CROOT \
  --output-dir $C/out --expected-source-revision $SRCREV) > $C/harness.log 2>&1; rc=$?
echo "== $(date -u +%FT%TZ) 1131 harness exit $rc"
python3 -c "$TREE" "$H" $E/pre-rollout-backup/backup-signed | tee $C/archive-tree-after.txt
rm -rf $C/scratch $C/tmp
echo "== $(date -u +%FT%TZ) replays done"
