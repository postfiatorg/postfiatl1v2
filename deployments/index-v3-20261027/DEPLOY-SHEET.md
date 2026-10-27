# Deploy sheet — index-v3-20261027

The release-day sequence (2026-10-27), run from the signing workstation. It follows the
[exchange-readiness-20261023 sheet](https://github.com/postfiatorg/postfiatl1v2/blob/release/exchange-readiness-20261023/deployments/exchange-readiness-20261023/DEPLOY-SHEET.md),
the [rollout runbook](../../docs/runbooks/safe-validator-rollout.md) and the
[account_tx index runbook](../../docs/runbooks/account-tx-index.md). Keys are used by path only; no key
material is printed, copied to a host or committed.

**Status: §0–§4 done on 2026-10-27** (before state at 1153, [checks](observed/before-checks.json); signed canary backup `03e5c61f…` and current-head replay PASS, [record](observed/canary-backup.json)); §5–§6 done (all six on `b5cf3ce4…` at 1159, [rollout record](observed/rollout-record.json), [after checks](observed/after-checks.json)). Cut and locally qualified on 2026-10-27 ([packet](qualification/README.md)).
Every live section (§0, §1, §3–§9) is covered by the operator's go of the day; each step marked **GO**
needs that go. This release has no governance step: the three activation heights stay as set on
2026-10-23 (active from 1153).

```bash
D=~/.postfiat/deployments/index-v3-20261027   # local, not in Git; stage prepared 2026-10-27
S=$D/stage; E=$D/evidence; R=index-v3-20261027
C=$S/rootfs/etc/postfiat/releases/$R; U=$S/rootfs/etc/systemd/system
B=$S/rootfs/opt/postfiat/releases/$R/postfiat-node
PACKET=deployments/index-v3-20261027
K=~/.postfiat/deployments/cobalt-activation-8694b99d/snapshot-keys
VULTR=~/.postfiat/deployments/cobalt-activation-8694b99d/vultr-api-key
Q=~/.cache/release-index-v3-20261027   # build trees: source-1 at d728f895, target-1/release/postfiat-node
```

## 0. Go/no-go — covered by the operator's go of the day

- [x] **GO 0.** No other rollout or session is active on the fleet; `apt-daily-upgrade` is not running;
  CI on `d728f895` is green in docs-build (40335966553), rust-ci with the full workspace test job
  (40335966336) and product-security-ci (40335966335). 2026-10-27 10:41Z: `origin/main` at `77725bef` (the cut record); no other rollout; the two other tmux sessions on this server are read-only reviews; `apt-daily-upgrade` inactive on all six.

## 1. Before state (read-only) — covered by the operator's go of the day

```bash
python3 $PACKET/observe-fleet.py --output $PACKET/observed/before.json
```

Expected: six validators on `exchange-readiness-20261023` (`4085ac70…`, build `6b1a0585`, manifest
`210544a8…`), height 1153, tip `db8b87b6…`, root `7141956e…` (or a later height with six-way agreement),
mempools empty. Record on each host, read-only, the index status: v2 usable or stale, not v3
(`postfiat-node account-tx-index-status` with the deployed executable).

- [x] **GO 1.** Before state matches. Otherwise stop. 2026-10-27 10:42Z: six on `exchange-readiness-20261023` (`4085ac70…`, build `6b1a0585`, manifest `210544a8…`, signature gate PASS) at 1153 / `db8b87b6…` / `7141956e…`, mempools 0 ([before.json](observed/before.json)); index v2 usable to 1153 on all six ([before-index-status/](observed/before-index-status/)).

## 2. Stage and local preflight (local)

The stage and the signed manifest were prepared on 2026-10-27 (manifest `a61e290a…`, publisher `pfc531e0…`,
valid 1793095187–1824631487 per `$E/manifest-validity.txt`). Recheck them:

```bash
sha256sum $B $C/deployment-manifest.json   # b5cf3ce4…, a61e290a…
python3 $PACKET/local-preflight.py --stage $S --require-signed > $E/local-preflight-signed.json
```

Expected: `PASS_SIGNED_LOCAL_INPUTS_ONLY`, 33 generated files, four circuit artifacts, signed gate for all six.

## 3. Fleet preflight (read-only on the fleet, writes local rollout state) — covered by the operator's go of the day

```bash
install -m 0644 $PACKET/inventory.txt $D/inventory.txt
scripts/postfiat-safe-rollout preflight --stage-report $S/stage-report.json \
  --inventory-file $D/inventory.txt --vultr-api-key-file $VULTR \
  --state-file $E/rollout-state.json --canary-validator-id validator-1 \
  --rpc-tunnel-base-port 27650 > $E/preflight.json 2> $E/preflight.stderr
```

Expected: six-way agreement, six signer rosters valid, 0 deletions, order 1, 0, 2, 3, 4, 5. Read the
three activation heights from the §4 import (`read-import-state.py`): 1153 each.

- [x] **GO 3.** Preflight passes. 2026-10-27 10:44Z: stage `b5cf3ce4…`/`a61e290a…`, local preflight `PASS_SIGNED_LOCAL_INPUTS_ONLY`; fleet preflight verified, six-way agreement at 1153, six rosters valid (one registry root), 0 deletions, order 1, 0, 2, 3, 4, 5. Activation heights from the §4 import: 1153, 1153, 1153; both routes `authority_epoch` 1.

## 4. Fresh signed canary backup from validator-1 and its exact replay — covered by the operator's go of the day

- [x] **GO 4.** Take the backup (writes one export on validator-1; check its free disk first). 2026-10-27 10:45–10:53Z: manifest `03e5c61f…` at 1153; re-import, checkpoint and full replay verified with the new executable; harness PASS, receipt `d3d06a9a…`; validator-1 free disk 16.61 GB → 16.29 GB.

```bash
scripts/postfiat-safe-rollout backup --state-file $E/rollout-state.json \
  --evidence-dir $E/pre-rollout-backup \
  --snapshot-publisher-key-file $K/snapshot-publisher.private.json \
  --snapshot-publisher-public-key-file $K/snapshot-publisher.public.json > $E/backup.json 2> $E/backup.stderr
X=~/.cache/deploy-index-v3-20261027/backup-reverify
$B snapshot-import-signed-finalized-checkpoint --data-dir $X --snapshot-dir $E/pre-rollout-backup/backup-signed \
  --trusted-publisher-key-file $K/snapshot-publisher.public.json --node-id validator-1
$B verify-finalized-checkpoint --data-dir $X     # verified: true, the before-state height, tip and root
python3 $PACKET/read-import-state.py $X > $E/import-state.json   # three heights 1153 (ATTENTION is expected: the script checks for unset heights)
```

Then the current-head exact replay with the new executable, on a scratch copy of the new export, from the
clean build tree of `d728f895`:

```bash
read -r HT TIP ROOT < <(python3 -c 'import json,sys; b=json.load(open(sys.argv[1]))["backup"]; print(b["height"], b["tip"], b["state_root"])' $E/rollout-state.json)
Y=~/.cache/deploy-index-v3-20261027/replay-$HT; mkdir -p $Y
cp -a $E/pre-rollout-backup/backup-signed $Y/backup-signed
(cd $Q/source-1 && python3 benchmarks/storage-scaling/run_replay_evidence.py \
  --node-bin $Q/target-1/release/postfiat-node \
  --signed-archive-dir $Y/backup-signed --trusted-publisher-file $K/snapshot-publisher.public.json \
  --expected-height $HT --expected-tip $TIP --expected-root $ROOT \
  --output-dir $Y/out --expected-source-revision d728f895ee6c6d9910c6e0c0f3b9d92f30e0f3f2)
```

Expected: PASS at the before-state height, tip and root; archive tree hash unchanged. A mismatch is a STOP.
Record the backup manifest hash and the receipt hash in `RELEASE-ID.txt`.

## 5. One validator at a time — covered by the operator's go of the day

Order: validator-1 (canary), then 0, 2, 3, 4, 5. Before the first grant, point StakeHub's `~/.pft/config.toml`
`local_node_binary` at this release; point `runtime_binary` and `topology_file` at it only after the last
validator is applied. The grant recipient is the `testing` wallet
`pf6395ef7f43ffaa74ce178d320b9bbbfd45532b38`.

- [x] **GO 5.N** (one go per validator: validator-1, validator-0, validator-2, validator-3, validator-4, validator-5). 2026-10-27 11:05:29–11:34:43Z: all six applied in order; one 1-PFT grant to `testing` after each, at 1154–1159 (proposers validator-2, 3, 4, 5, 0, 1), each certified at view 0 with quorum 5 of 6 and the same block, tip and root on all six; no rollback ([rollout record](observed/rollout-record.json)).

```bash
scripts/postfiat-safe-rollout apply-next --state-file $E/rollout-state.json > $E/apply-N.json
(cd ~/repos/StakeHub && .venv/bin/pft faucet testing --asset PFT --amount 1000000) > $E/grant-after-<validator>.out
python3 $PACKET/observe-fleet.py --stage $S --applied validator-1[,validator-0,...] \
  --output $PACKET/observed/after-<validator>.json
```

Each grant must certify at the same height with the same tip and root on all six. On divergence or a missed
certification: stop and use [Rollback](#rollback-one-validator-emergency-only) on that validator. After the
upgrade each applied host reports its v2 index unsupported (`account_tx_index_status`) and answers
`account_tx` from the archive scan (`index_used: false`) until §7; this is expected.

## 6. After checks — covered by the operator's go of the day

```bash
python3 $PACKET/observe-fleet.py --stage $S \
  --applied validator-1,validator-0,validator-2,validator-3,validator-4,validator-5 \
  --output $PACKET/observed/after.json
python3 $PACKET/demo-preflight.py
```

Expected: all 12 validator and RPC processes on `b5cf3ce4…`; `deployment_manifest_verified=true` and
`build_git_revision` `d728f895` on every host; RPC units with `--max-requests 100000` and `RestartSec=1`.

- [x] **GO 6.** After checks pass; proceed to §7. 2026-10-27 11:35–11:37Z: after state 1159 / `ae91ad6d…` / `60ddbee3…` on six; demo preflight READY (60 PASS); all six on `b5cf3ce4…`, manifest verified, build `d728f895`; RPC units keep `--max-requests 100000` and `RestartSec=1`, both env files `POSTFIAT_DEPLOYMENT_VERIFIED_RECORD`; no governance block since 1153 (activation heights stay 1153, both routes `authority_epoch` 1); swap service committee epoch 2 on six; the v2 index reported unsupported on six, as expected before §7 ([after checks](observed/after-checks.json)).

## 7. account_tx index build, one host at a time — covered by the operator's go of the day

The upgrade leaves each host's v2 index unread ([runbook](../../docs/runbooks/account-tx-index.md#upgrade-backfill)).
The build removes the v2 files and writes v3; from then on every commit extends it. Produce no block during a
host's build (no grant): a commit that lands mid-build leaves the index one block behind, and the build must
be run again. Stop the host's RPC unit (the only reader) during the build; the validator unit keeps running.
Run validator-1 first; continue only if it reports both indexes usable at schema v3.

```bash
N=1   # then 0, 2, 3, 4, 5
ssh root@HOST
  N=1; P=/opt/postfiat/releases/index-v3-20261027/postfiat-node
  systemctl stop postfiat-validator-$N-rpc.service
  time runuser -u postfiat -- $P account-tx-index-build --data-dir /var/lib/postfiat/validator-$N
  runuser -u postfiat -- $P account-tx-index-status --data-dir /var/lib/postfiat/validator-$N
  systemctl start postfiat-validator-$N-rpc.service
```

Expected: `index_usable: true`, `disk_index_usable: true`, `account_tx_index_meta.json` schema
`postfiat-account-tx-disk-index-v3`, range 1 to the tip, `disk_index_lag_blocks: 0`. Time: see the packet's
index check (work server, signed 1137 import: 6.8 s for 1,137 blocks, 6.0 s per 1,000 blocks; the v2 build of the deployed executable took 4.8 s), so a few seconds per host at about 1,160
blocks; allow a minute on the smaller validator hosts. Then one grant: every host's `disk_index_lag_blocks`
stays 0 (the index follows the commit).

- [ ] **GO 7.N** (one go per host: validator-1, validator-0, validator-2, validator-3, validator-4, validator-5).
- [ ] validator-1 reported both indexes usable at v3 before the others ran.
- [ ] Six-way agreement before and after every host (`observe-fleet.py`).

## 8. Vote-record check — covered by the operator's go of the day

After the first height finalized by a proposer running the new executable (any §5 grant after that host was
applied), read on that proposer, read-only:

```bash
ssh root@PROPOSER ls /var/lib/postfiat/validator-N/block-vote-records/
ssh root@PROPOSER python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); print(sorted(v["validator_id"] for v in d["votes"]))' \
  /var/lib/postfiat/validator-N/block-vote-records/height-H.json
```

Expected: `height-H.json` exists and names the six validators with an outcome each (the file format is in the
[runbook](../../docs/runbooks/signed-validator-snapshot-recovery.md#vote-outcome-records-sct-08-item-4); adapt the
field names if the file differs). A missing file is recorded, not a rollback reason: the record is not read by
consensus.

- [ ] **GO 8.** The record exists on the proposer and names the six validators.

## 9. After state — covered by the operator's go of the day

```bash
python3 $PACKET/observe-fleet.py --stage $S \
  --applied validator-1,validator-0,validator-2,validator-3,validator-4,validator-5 \
  --output $PACKET/observed/after-final.json
python3 $PACKET/demo-preflight.py
```

Expected: six on `b5cf3ce4…` with six-way agreement; the three activation heights 1153; v3 index usable on
all six. Record the result in `RELEASE-ID.txt`, the plan and [Current State](../../docs/status/chain-state-current.md).

## Rollback (one validator, emergency only)

`safe-rollout` has no rollback subcommand and makes no per-host data anchor. Each host keeps the
`exchange-readiness-20261023` executable (`4085ac70…`) and release directory; its units are in the local
2026-10-23 stage. [rollback-one.sh](rollback-one.sh) stops the validator, checks the 2026-10-23 executable
against the data in place at the given identity, reinstalls the 2026-10-23 units, verifies the signed
2026-10-23 manifest (`210544a8…`) and restarts:

```bash
scp ~/.postfiat/deployments/exchange-readiness-20261023/stage/rootfs/etc/systemd/system/postfiat-validator-N{,-rpc}.service \
  root@HOST:/etc/postfiat/releases/exchange-readiness-20261023/
ssh root@HOST bash -s -- validator-N HEIGHT TIP ROOT < $PACKET/rollback-one.sh
```

Rehearsed locally: the 2026-10-23 executable verifies and replays the data this build imported and ran six
services on ([packet](qualification/README.md)).

**After the index build (§7):** the deployed executable still serves `account_tx`, but not from the index. It ignores the v3 files and answers from its bounded archive scan (`index_used: false`, the newest 100 blocks, rows equal to the v2 index rows of those blocks for 20 of 20 addresses). Its `account-tx-index-status` (CLI and the `account_tx_index_status` RPC) fails with `missing field accounts` on the v3 aggregate. The required step after rolling a host back: stop its RPC unit, run the deployed executable's `account-tx-index-build` (4.3 s on the 1137 import; it rewrites a usable v2 index, and `account_tx` then answers from it byte-identically to before the upgrade, 20 of 20), and start the RPC unit. The v3 per-account directories stay beside the v2 files and are not read. Moving the host forward again needs this release's `account-tx-index-build` once more ([index check](qualification/index-check/index-v3-check.json)). Consensus is unaffected either way: the index is an operator cache.

If the executable cannot verify the data in place, the script stops before installing anything; the fallback
is the signed validator-1 backup from §4.
