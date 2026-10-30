# Deploy sheet — vote-records-20261030

The release-day sequence, run from the signing workstation. It follows the
[index-v3-20261027 sheet](https://github.com/postfiatorg/postfiatl1v2/blob/release/index-v3-20261027/deployments/index-v3-20261027/DEPLOY-SHEET.md)
and the [rollout runbook](../../docs/runbooks/safe-validator-rollout.md). Keys are used by path only; no key
material is printed, copied to a host or committed.

**Status: deployed on all six validators on 2026-10-30** (applied 10:31–11:14Z at 1162–1167; after state 1168 /
`a20e406e…` / `39771b74…`; [rollout record](observed/rollout-record.json), [packet](qualification/README.md)). Every live section (§0, §1, §3–§8) needs the operator's go of the day; each step marked **GO**
needs that go. This release has **no governance step and no index build**: the three activation heights stay
1153, the account_tx index stays v3, and the W-1 activation kinds stay absent.

```bash
D=~/.postfiat/deployments/vote-records-20261030   # local, not in Git; stage prepared 2026-10-30
S=$D/stage; E=$D/evidence; R=vote-records-20261030
C=$S/rootfs/etc/postfiat/releases/$R; U=$S/rootfs/etc/systemd/system
B=$S/rootfs/opt/postfiat/releases/$R/postfiat-node
PACKET=deployments/vote-records-20261030
K=~/.postfiat/deployments/cobalt-activation-8694b99d/snapshot-keys
VULTR=~/.postfiat/deployments/cobalt-activation-8694b99d/vultr-api-key
Q=~/.cache/release-vote-records-20261030   # build trees: source-1 at ae70d66d, target-1/release/postfiat-node
```

## 0. Go/no-go

- [x] **GO 0.** No other rollout or session is active on the fleet; `apt-daily-upgrade` is not running. CI on
  `ae70d66d` is green in all three workflows: docs-build `41032153113` (success at the cut), rust-ci
  `41032153158` with the full workspace test job and product-security-ci `41032153060` (both in progress at
  the cut). The rollout does not start before all three are green.
  2026-10-30: no other rollout or session on the fleet (10:06Z); `apt-daily-upgrade` inactive on six; all three
  workflows green, read by the operator at 10:30Z (docs-build `41032153113`, rust-ci `41032153158`,
  product-security-ci `41032153060`).

## 1. Before state (read-only)

```bash
python3 $PACKET/observe-fleet.py --output $PACKET/observed/before.json
```

Expected: six validators on `index-v3-20261027` (`b5cf3ce4…`, build `d728f895`, manifest `a61e290a…`),
height 1160, tip `f65e6fac…`, root `f06608d4…` (or a later height with six-way agreement; whether a block came
since 2026-10-27 is read here), mempools empty. Read-only on each host: `account-tx-index-status` with the
deployed executable reports v3 usable, lag 0; the vote-record state: `ls block-vote-records/` on each host
(record which proposers have files; VR-01 makes some `no_answer` entries false).

- [x] **GO 1.** Before state matches. Otherwise stop. 10:15Z: six on `b5cf3ce4…` at 1161 / `a4d5bd82…` / `f3b09178…`
  (one block since 1160), mempools 0; v3 index usable to 1161, lag 0, on six; vote records: validator-2 1160
  (validator-0 `no_answer`), validator-3 1161 (six `accepted`), none on 0, 1, 4, 5 ([before](observed/before.json),
  [reads](observed/before-reads.json)).

## 2. Stage and local preflight (local)

The stage and the signed manifest were prepared on 2026-10-30 (manifest `ee1df5b3…`, publisher `pfc531e0…`,
valid 1793353262–1824889562 per `$E/manifest-validity.txt`). Recheck them:

```bash
sha256sum $B $C/deployment-manifest.json   # 87cfe797…, ee1df5b3…
python3 $PACKET/local-preflight.py --stage $S --require-signed > $E/local-preflight-signed.json
```

Expected: `PASS_SIGNED_LOCAL_INPUTS_ONLY`, 33 generated files, four circuit artifacts, signed gate for all six.

## 3. Fleet preflight (read-only on the fleet, writes local rollout state)

```bash
install -m 0644 $PACKET/inventory.txt $D/inventory.txt
scripts/postfiat-safe-rollout preflight --stage-report $S/stage-report.json \
  --inventory-file $D/inventory.txt --vultr-api-key-file $VULTR \
  --state-file $E/rollout-state.json --canary-validator-id validator-1 \
  --rpc-tunnel-base-port 27650 > $E/preflight.json 2> $E/preflight.stderr
```

Expected: six-way agreement, six signer rosters valid, 0 deletions, order 1, 0, 2, 3, 4, 5. From the §4
import (`read-import-state.py`): the three activation heights 1153 each, both W-1 kinds `null`
(`w1_inert: true`), both routes `authority_epoch` 1.

- [x] **GO 3.** Preflight passes and the heights are as expected. 10:16Z: six-way agreement at 1161, six rosters
  valid, 0 deletions, order 1, 0, 2, 3, 4, 5; import 1153 ×3, both routes epoch 1, W-1 kinds `null`. Local
  preflight (§2): `PASS_SIGNED_LOCAL_INPUTS_ONLY`.

## 4. Fresh signed canary backup from validator-1 and its exact replay

- [x] **GO 4.** Take the backup (writes one export on validator-1; check its free disk first). 10:17–10:24Z:
  validator-1 free 16.09 GB before, 15.77 GB after; the backup also stages the candidate executable there
  (`.vote-records-20261030-validator-1.candidate-postfiat-node`, 65,901,656 bytes, the file the apply uses);
  manifest `9b68d2a1…` at 1161; re-import, checkpoint and full replay PASS; head replay PASS, receipt
  `8d61e40c…` ([record](observed/canary-backup.json)).

```bash
scripts/postfiat-safe-rollout backup --state-file $E/rollout-state.json \
  --evidence-dir $E/pre-rollout-backup \
  --snapshot-publisher-key-file $K/snapshot-publisher.private.json \
  --snapshot-publisher-public-key-file $K/snapshot-publisher.public.json > $E/backup.json 2> $E/backup.stderr
X=~/.cache/deploy-vote-records-20261030/backup-reverify
$B snapshot-import-signed-finalized-checkpoint --data-dir $X --snapshot-dir $E/pre-rollout-backup/backup-signed \
  --trusted-publisher-key-file $K/snapshot-publisher.public.json --node-id validator-1
$B verify-finalized-checkpoint --data-dir $X     # verified: true, the before-state height, tip and root
python3 $PACKET/read-import-state.py $X > $E/import-state.json   # 1153 x3, W-1 kinds null
```

Then the exact replay with the new executable through the harness, on a scratch copy of the new export, from
the clean build tree of `ae70d66d`:

```bash
read -r HT TIP ROOT < <(python3 -c 'import json,sys; b=json.load(open(sys.argv[1]))["backup"]; print(b["height"], b["tip"], b["state_root"])' $E/rollout-state.json)
Y=~/.cache/deploy-vote-records-20261030/replay-$HT; mkdir -p $Y
cp -a $E/pre-rollout-backup/backup-signed $Y/backup-signed
(cd $Q/source-1 && python3 benchmarks/storage-scaling/run_replay_evidence.py \
  --node-bin $Q/target-1/release/postfiat-node \
  --signed-archive-dir $Y/backup-signed --trusted-publisher-file $K/snapshot-publisher.public.json \
  --expected-height $HT --expected-tip $TIP --expected-root $ROOT \
  --output-dir $Y/out --expected-source-revision ae70d66d8a5083cc1a33aaf4a3e17f6901f7bc34)
```

Expected: PASS at the before-state height, tip and root; archive tree hash unchanged. A mismatch is a STOP.
Record the backup manifest hash and the receipt hash in `RELEASE-ID.txt`.

## 5. One validator at a time

Order: validator-1 (canary), then 0, 2, 3, 4, 5. Before the first grant, point StakeHub's `~/.pft/config.toml`
`local_node_binary` at this release; point `runtime_binary` and `topology_file` at it only after the last
validator is applied. The grant recipient is the `testing` wallet `pf6395ef7f43ffaa74ce178d320b9bbbfd45532b38`.

- [x] **GO 5.N** (one go per validator: validator-1, validator-0, validator-2, validator-3, validator-4, validator-5).
  2026-10-30 10:31–11:16Z: all six applied in order; one 1-PFT grant to `testing` after each, at 1162–1167
  (proposers validator-4, 5, 0, 1, 2, 3), each certified at view 0 with 5 of 6 votes and the same block, tip and
  root on all six; no rollback. validator-2's first try (10:50Z) failed before any change: SSH to `66.42.48.39`
  hung and exited 255. From its second try (11:03Z) on, apply-next, the grants, grant checks and observations
  ran through a multiplexed SSH wrapper with keepalives (transport only; [rollout record](observed/rollout-record.json)).

```bash
scripts/postfiat-safe-rollout apply-next --state-file $E/rollout-state.json > $E/apply-N.json
(cd ~/repos/StakeHub && .venv/bin/pft faucet testing --asset PFT --amount 1000000) > $E/grant-after-<validator>.out
python3 $PACKET/observe-fleet.py --stage $S --applied validator-1[,validator-0,...] \
  --output $PACKET/observed/after-<validator>.json
```

Each grant must certify at the same height with the same tip and root on all six. On divergence or a missed
certification: stop and use [Rollback](#rollback-one-validator-emergency-only) on that validator.

## 6. After checks

```bash
python3 $PACKET/observe-fleet.py --stage $S \
  --applied validator-1,validator-0,validator-2,validator-3,validator-4,validator-5 \
  --output $PACKET/observed/after.json
python3 $PACKET/demo-preflight.py
```

Expected: all 12 validator and RPC processes on `87cfe797…`; `deployment_manifest_verified=true` and
`build_git_revision` `ae70d66d` on every host; RPC units with `--max-requests 100000` and `RestartSec=1`, no
`--allow-private-action-admission`. Through each host's RPC tunnel: `private_action_submit` answers
`rpc_method_not_allowed`; `private_readiness` (`pool_id` `asset-orchard-v1`) answers `ready: false` with the
reasons `asset_orchard_pricing_enforcement_activation_absent` and `a666_pfusdc_route_absent` and the admission
switch `off`. On each host after its first commit with the new executable:
`/var/lib/postfiat/validator-N/private_pool_snapshot_points.json` exists.

- [x] **GO 6.** After checks pass; proceed to §7. 11:16–11:18Z at 1167: all six on `87cfe797…`, manifest verified,
  build `ae70d66d`; RPC units `--max-requests 100000`, `RestartSec=1`, no `--allow-private-action-admission`;
  `private_action_submit` `rpc_method_not_allowed` and `private_readiness` not ready (both reasons, switch `off`)
  on six; snapshot points file present on six; v3 index usable, lag 0; only transparent blocks since 1161
  (activation heights stay 1153, routes epoch 1); swap service epoch 2; demo preflight READY, 60 PASS
  ([after checks](observed/after-checks.json)).

## 7. Vote-record check

After the first height finalized by a proposer running the new executable (any §5 grant after that host was
applied), read on that proposer, read-only:

```bash
ssh root@PROPOSER ls /var/lib/postfiat/validator-N/block-vote-records/
ssh root@PROPOSER python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); print({e["validator"]: e["outcome"] for e in d["entries"]})' \
  /var/lib/postfiat/validator-N/block-vote-records/height-HHHHHHHHHHHHHHHHHHHH.json
```

The file name pads the height to 20 digits. Expected: the file exists, names the six validators with an
outcome each, and shows no false `no_answer` (the VR-01 symptom): a validator whose event log shows it accepted
the vote request is `accepted` (`answered_after_quorum` true when it came after the quorum), a refusal after the
deadline stays `refused`, an answer after the deadline is `late_after_deadline`. A missing file or a wrong entry
is recorded, not a rollback reason: consensus never reads the record.

- [x] **GO 7.** The record exists on the proposer, names the six validators and has no false `no_answer`.
  1168 (grant after StakeHub's `runtime_binary` moved to this release, 11:16Z): validator-4's
  `height-00000000000000001168.json` names six, all `accepted` (validator-2 `answered_after_quorum`), the block
  certified on six ([record](observed/vote-record-1168.json)). The rounds for 1162–1167 ran the previous
  executable (`runtime_binary`), so their records are not this release's.

## 8. After state

```bash
python3 $PACKET/observe-fleet.py --stage $S \
  --applied validator-1,validator-0,validator-2,validator-3,validator-4,validator-5 \
  --output $PACKET/observed/after-final.json
python3 $PACKET/demo-preflight.py
```

Expected: six on `87cfe797…` with six-way agreement; the three activation heights 1153; W-1 kinds absent; v3
index usable on all six. Record the result in `RELEASE-ID.txt`, the plan and
[Current State](../../docs/status/chain-state-current.md).

- [x] 2026-10-30 11:21Z: six on `87cfe797…` at 1168 / `a20e406e…` / `39771b74…`, mempools 0; demo preflight
  READY, 60 PASS ([after state](observed/after-final.json)). StakeHub `~/.pft/config.toml`: `local_node_binary`,
  `runtime_binary` and `topology_file` on this release.

## Rollback (one validator, emergency only)

`safe-rollout` has no rollback subcommand and makes no per-host data anchor. Each host keeps the
`index-v3-20261027` executable (`b5cf3ce4…`) and release directory; its units are in the local 2026-10-27
stage. [rollback-one.sh](rollback-one.sh) stops the validator, checks the 2026-10-27 executable against the
data in place at the given identity, reinstalls the 2026-10-27 units, verifies the signed 2026-10-27 manifest
(`a61e290a…`) and restarts:

```bash
scp ~/.postfiat/deployments/index-v3-20261027/stage/rootfs/etc/systemd/system/postfiat-validator-N{,-rpc}.service \
  root@HOST:/etc/postfiat/releases/index-v3-20261027/
ssh root@HOST bash -s -- validator-N HEIGHT TIP ROOT < $PACKET/rollback-one.sh
```

**After the new executable wrote `private_pool_snapshot_points.json` and fixed vote records:**
the old executable still serves; no extra step is required. Rehearsed on a
four-validator loopback devnet ([packet](qualification/README.md#rotation-and-rollback-rehearsal)): after the new
executable committed height 1 (each applying validator wrote `private_pool_snapshot_points.json`; the proposer
wrote `block-vote-records/height-…01.json` with the fixed entries), the `index-v3-20261027` executable read those
data directories (`status`; `verify-state` at the new executable's root `e4058cb7…`), then served two of the
four validators in round 2 and applied block 2 with the same root as the new executable (`76895f4c…`); afterwards
both executables verify the checkpoint and replay all three directories at that root. The old executable ignores
the unknown file (it neither reads nor rewrites it) and does not read vote records (as proposer it writes its own,
with the VR-01 defect). Leave `private_pool_snapshot_points.json` and `block-vote-records/` in place. Moving the
host forward again needs nothing either: at its next commit the new executable finds no point for the previous
height and computes the point from the full state. Also rehearsed: the old executable's checkpoint and full replay
of the new executable's signed 1153 import, and of the six 1021 copies the new build ran six services on, PASS.

If the executable cannot verify the data in place, the script stops before installing anything; the fallback
is the signed validator-1 backup from §4.
