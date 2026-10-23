# Deploy sheet — exchange-readiness-20261023

The release-day sequence (2026-10-23), run from the signing workstation. It follows the
[store-handle-20261021 sheet](https://github.com/postfiatorg/postfiatl1v2/blob/release/store-handle-20261021/deployments/store-handle-20261021/DEPLOY-SHEET.md),
the [rollout runbook](../../docs/runbooks/safe-validator-rollout.md), the
[account_tx index runbook](../../docs/runbooks/account-tx-index.md) and the review's
[activation facts](../../docs/review/exchange-readiness-merge-review-20261023.md#activation-steps-later-today-after-all-six-run-the-new-binary).
Keys are used by path only; no key material is printed, copied to a host or committed.

**Status: §0–§7 done on 2026-10-23** (deployed at 1143; account_tx v2 index on all six) (before state at 1137, [checks](observed/before-checks.json); signed canary backup `3257115c…` and current-head replay PASS, [record](observed/canary-backup.json)); §5 not started. Cut and locally qualified on 2026-10-23, re-qualified the same day with the account history index fix (`6b1a0585`, [packet](qualification/README.md)).
Every live section (§0, §1, §3–§9) is covered by the operator's go of the day; each step marked
**GO** needs that go. §8 (governance activation) runs only after §6 and §7 are clean on all six.

```bash
D=~/.postfiat/deployments/exchange-readiness-20261023   # local, not in Git; stage prepared 2026-10-23
S=$D/stage; E=$D/evidence; R=exchange-readiness-20261023
C=$S/rootfs/etc/postfiat/releases/$R; U=$S/rootfs/etc/systemd/system
B=$S/rootfs/opt/postfiat/releases/$R/postfiat-node
PACKET=deployments/exchange-readiness-20261023
K=~/.postfiat/deployments/cobalt-activation-8694b99d/snapshot-keys
VULTR=~/.postfiat/deployments/cobalt-activation-8694b99d/vultr-api-key
```

## 0. Go/no-go — covered by the operator's go of the day

- [x] **GO 0.** No other rollout or session is active on the fleet (the storage migration rehearsal under `~/.cache/g3-migration-rehearsal-20261023/` is local only); `apt-daily-upgrade` is not running; CI on `d9260e93` (main) has no failed workflow (`gh run list --commit d9260e93…`; the release merge `6b1a0585` adds no other code).

## 1. Before state (read-only) — covered by the operator's go of the day

```bash
python3 $PACKET/observe-fleet.py --output $PACKET/observed/before.json
```

Expected: six validators on `store-handle-20261021` (`eea96ba6…`, build `86706699`, manifest
`8ce1da30…`), height 1137, tip `18a4dc86…`, root `1277c76c…` (or a later height with six-way
agreement), mempools empty. The three activation heights and both routes' `authority_epoch` are
read from the §4 canary import (no extra host read).

- [x] **GO 1.** Before state matches. Otherwise stop. 2026-10-23 06:31Z: six on `store-handle-20261021` at 1137 / `18a4dc86…` / `1277c76c…`, mempools 0 ([before.json](observed/before.json)).

## 2. Stage and local preflight (local)

The stage and the signed manifest were prepared on 2026-10-23 and re-made for `6b1a0585` (manifest `210544a8…`,
publisher `pfc531e0…`, valid 1792734699–1824270999; the first stage is kept as `stage-6604c93e/`). Recheck them:

```bash
sha256sum $B $C/deployment-manifest.json   # 4085ac70…, 210544a8…
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

Expected: six-way agreement, six signer rosters valid, 0 deletions, order 1, 0, 2, 3, 4, 5.

## 4. Fresh signed canary backup from validator-1 and its exact replay — covered by the operator's go of the day

- [x] **GO 4.** Take the backup (writes one export on validator-1; check its free disk first). 2026-10-23 06:39–06:48Z: manifest `3257115c…` at 1137; re-import, checkpoint and full replay verified; harness PASS, receipt `18114495…`; validator-1 free disk 17.19 GB → 16.87 GB.

```bash
scripts/postfiat-safe-rollout backup --state-file $E/rollout-state.json \
  --evidence-dir $E/pre-rollout-backup \
  --snapshot-publisher-key-file $K/snapshot-publisher.private.json \
  --snapshot-publisher-public-key-file $K/snapshot-publisher.public.json > $E/backup.json 2> $E/backup.stderr
X=~/.cache/deploy-exchange-readiness-20261023/backup-reverify
$B snapshot-import-signed-finalized-checkpoint --data-dir $X --snapshot-dir $E/pre-rollout-backup/backup-signed \
  --trusted-publisher-key-file $K/snapshot-publisher.public.json --node-id validator-1
$B verify-finalized-checkpoint --data-dir $X     # verified: true, the before-state height, tip and root
python3 $PACKET/read-import-state.py $X > $E/import-state.json   # three heights null, both routes authority_epoch 1, PASS
```

Then the current-head exact replay the other lane asks for, on a scratch copy of the new
export, from a clean checkout of `6b1a0585` (the build tree `~/.cache/release-exchange-readiness-20261023-r2/source-1`):

```bash
H=$(python3 -c 'import json,sys; b=json.load(open(sys.argv[1]))["backup"]; print(b["height"], b["tip"], b["state_root"])' $E/rollout-state.json)
read -r HT TIP ROOT <<< "$H"
Y=~/.cache/deploy-exchange-readiness-20261023/replay-$HT; mkdir -p $Y
cp -a $E/pre-rollout-backup/backup-signed $Y/backup-signed
(cd ~/.cache/release-exchange-readiness-20261023-r2/source-1 && python3 benchmarks/storage-scaling/run_replay_evidence.py \
  --node-bin ~/.cache/release-exchange-readiness-20261023-r2/target-1/release/postfiat-node \
  --signed-archive-dir $Y/backup-signed --trusted-publisher-file $K/snapshot-publisher.public.json \
  --expected-height $HT --expected-tip $TIP --expected-root $ROOT \
  --output-dir $Y/out --expected-source-revision 6b1a05854c0b3a0ac0dec583fa09ff380d95d952)
```

Expected: PASS at the before-state height, tip and root (signed import, checkpoint, full replay);
archive tree hash unchanged. A mismatch is a STOP. Record the backup manifest hash and the
receipt hash in `RELEASE-ID.txt`.

## 5. One validator at a time — covered by the operator's go of the day

Order: validator-1 (canary), then 0, 2, 3, 4, 5. Before the first grant, point StakeHub's
`~/.pft/config.toml` `local_node_binary` at this release. Point `runtime_binary` and
`topology_file` at it only after the last validator is applied: the faucet runs
`runtime_binary status` on all six hosts, and the new path exists only on applied hosts.

The grant recipient is the `testing` wallet `pf6395ef7f43ffaa74ce178d320b9bbbfd45532b38`
(faucet `pfcd4cc8…`). It passes the new admission rule (`pf` + 40 lowercase hex; review
[table](../../docs/review/exchange-readiness-merge-review-20261023.md#r3-the-admission-change-and-the-devnets-addresses);
the new build admitted it locally, [admission check](qualification/admission-result.json)).

For each validator, only after the previous check passed:

- [x] **GO 5.N** (one go per validator: validator-1, validator-0, validator-2, validator-3, validator-4, validator-5). 2026-10-23 07:10:57–07:39:43Z: all six applied in order; grants at 1138–1143 (proposers validator-4, 5, 0, 1, 2, 3), each certified at view 0 with quorum 5 of 6 and the same block, tip and root on all six; no rollback ([rollout record](observed/rollout-record.json)).

```bash
scripts/postfiat-safe-rollout apply-next --state-file $E/rollout-state.json > $E/apply-N.json
(cd ~/repos/StakeHub && .venv/bin/pft faucet testing --asset PFT --amount 1000000) > $E/grant-after-<validator>.out
python3 $PACKET/observe-fleet.py --stage $S --applied validator-1[,validator-0,...] \
  --output $PACKET/observed/after-<validator>.json
```

Each grant must certify at the same height with the same tip and root on all six (`blocks` RPC
on all six). On divergence or a missed certification: stop and use
[Rollback](#rollback-one-validator-emergency-only) on that validator. Submit no `account_set`,
`issued_payment_v2` or governance record during §5: the old binary rejects them (review XR-04).

## 6. After checks — covered by the operator's go of the day

```bash
python3 $PACKET/observe-fleet.py --stage $S \
  --applied validator-1,validator-0,validator-2,validator-3,validator-4,validator-5 \
  --output $PACKET/observed/after.json
python3 $PACKET/demo-preflight.py
```

Expected: all 12 validator and RPC processes on `4085ac70…`; `deployment_manifest_verified=true`
and `build_git_revision` `6b1a0585` on every host; RPC units with `--max-requests 100000` and
`RestartSec=1`.

- [x] **GO 6.** After checks pass; proceed to §7. 2026-10-23 07:40–07:55Z: after state 1143 / `7fef75aa…` / `8c3a32fd…` on six; demo preflight READY (60 PASS); 12 processes on `4085ac70…`; manifest verified, build `6b1a0585`; RPC unit flags kept ([after checks](observed/after-checks.json)).

## 7. account_tx index build, one host at a time — covered by the operator's go of the day

The build is expected to succeed. The first cut's executable stopped on devnet history with
`receipt log contains duplicate tx_id 219306e6…` (a retried governance amendment keeps its id, so the
transactional receipt log repeats it at heights 64/66, and `c9acb974…` at 940/941/945); `d9260e93`
accepts exactly those governance repeats. On the signed 1131 import the new executable builds a usable
v2 index in 4.1 s (958 rows, 255 accounts), and `account_tx` answers for the `testing` wallet and the
faucet with the grant rows ([index check](qualification/requal-6b1a0585/index-fix.json)). Run validator-1
first; continue with the others only if it reports both indexes usable.

The runbook needs no stopped unit (ordered commit never touches the cache). Conservative here:
stop the host's RPC unit (the only reader) during the build; the validator unit keeps running.

```bash
N=1   # then 0, 2, 3, 4, 5
ssh root@HOST
  N=1; P=/opt/postfiat/releases/exchange-readiness-20261023/postfiat-node
  systemctl stop postfiat-validator-$N-rpc.service
  time runuser -u postfiat -- $P account-tx-index-build --data-dir /var/lib/postfiat/validator-$N
  runuser -u postfiat -- $P account-tx-index-status --data-dir /var/lib/postfiat/validator-$N
  systemctl start postfiat-validator-$N-rpc.service
```

Expected: `index_usable: true`, `disk_index_usable: true`, schema `postfiat-account-tx-index-v2`,
indexed range 1 to the tip. Time: 3.6 s per 1,000 blocks on the work server (4.1 s for 1131 blocks),
so about 5 s per host at 1,137+ blocks; allow a minute on the smaller validator hosts.

- [x] **GO 7.N** (one go per host: validator-1, validator-0, validator-2, validator-3, validator-4, validator-5).
- [x] validator-1 reported both indexes usable before the others ran.
- [x] Six-way agreement at 1143 / `7fef75aa…` / `8c3a32fd…`, mempools 0, before and after every host (`observe-fleet.py`).

**Result (2026-10-23, 08:02–08:23Z, height 1143 throughout, no block produced).** All six built a usable v2
index (schema `postfiat-account-tx-index-v2`, range 1–1143: 1,105 indexed blocks, 970 rows, 255 accounts and
shards). Only the host's RPC unit was stopped, 11–31 s per host; the validator unit stayed active. After each
start the RPC was ready again within 15 s and `account_tx` for `testing` answered from the index
(`index_used: true`) with the grants at 1138–1143. Records: [account-tx-index/](observed/account-tx-index/).

| Host | Index before | Index after | Range | Blocks indexed | Rows | Build (s, incl. ssh) | RPC stopped | RPC started | Ready again | `account_tx` `testing` (index) |
|---|---|---|---|---|---|---|---|---|---|---|
| validator-1 | absent | v2 usable (index True, disk True) | 1–1143 | 1105 | 970 | 6.9 | 08:04:06 | 08:04:17 | 08:04:26 | 58 rows, grants 1138–1143 |
| validator-0 | absent | v2 usable (index True, disk True) | 1–1143 | 1105 | 970 | 8.4 | 08:07:21 | 08:07:36 | 08:07:41 | 58 rows, grants 1138–1143 |
| validator-2 | absent | v2 usable (index True, disk True) | 1–1143 | 1105 | 970 | 14.8 | 08:10:37 | 08:11:08 | 08:11:23 | 58 rows, grants 1138–1143 |
| validator-3 | absent | v2 usable (index True, disk True) | 1–1143 | 1105 | 970 | 8.8 | 08:14:25 | 08:14:40 | 08:14:46 | 58 rows, grants 1138–1143 |
| validator-4 | absent | v2 usable (index True, disk True) | 1–1143 | 1105 | 970 | 7.4 | 08:17:41 | 08:17:52 | 08:18:02 | 58 rows, grants 1138–1143 |
| validator-5 | absent | v2 usable (index True, disk True) | 1–1143 | 1105 | 970 | 11.5 | 08:20:57 | 08:21:19 | 08:21:28 | 58 rows, grants 1138–1143 |

The build time includes the ssh round trip. The index is current to 1143. Ordered commit does not extend it, so after
the next block `account_tx` falls back to the archive scan until `account-tx-index-build` runs again (runbook).

## 8. Governance activation of the three heights — covered by the operator's go of the day, only after §6 and §7 are clean on all six

Kinds `recipient_format_activation_height`, `rejected_tx_fee_activation_height`,
`require_memo_activation_height`: one amendment each, one governance batch each. The value is the
first enforced height and must be above the block that applies it. Signers: the six validators,
each with its validator key file (`governance-authorization-sign` checks the current registry key;
quorum and support from the amendment, `--support` defaults to all six). The issuer does not sign.

Height rule: read the tip T just before signing; `--proposal-slot` the next slot, `--expires-at-height`
a few blocks later, `--height` about T+10 for all three (the last governance block must stay below
it). The recipient-format height cannot be postponed (earliest unpaused wins); the other two can.
Submit recipient-format last. Reach the height with faucet grants, then probe once per rule.

```bash
for KIND in rejected-tx-fee require-memo recipient-format; do
  $B ratify-$KIND-activation-height --data-dir PATH --validators validator-0,validator-1,validator-2,validator-3,validator-4,validator-5 \
    --height H --amendment-file $E/$KIND.unsigned.json
  # each validator N: $B governance-authorization-sign --data-dir PATH --amendment-file $E/$KIND.unsigned.json \
  #   --validator validator-N --validator-key-file PATH --proposal-slot S --expires-at-height X --authorization-file $E/$KIND.N.json
  $B governance-amendment-assemble --data-dir PATH --amendment-file $E/$KIND.unsigned.json \
    --authorization-files $E/$KIND.0.json,…,$E/$KIND.5.json --proposal-slot S --output $E/$KIND.signed.json
  $B governance-batch --data-dir PATH --amendment-file $E/$KIND.signed.json --batch-file $E/$KIND.batch.json
  $B transport-peer-certified-batch-round --data-dir PATH --topology PATH --batch-kind governance \
    --batch-file $E/$KIND.batch.json --key-file PATH --artifact-dir PATH
done
```

- [ ] **GO 8.N** (one go per record).

## 9. After state — covered by the operator's go of the day

```bash
python3 $PACKET/observe-fleet.py --stage $S \
  --applied validator-1,validator-0,validator-2,validator-3,validator-4,validator-5 \
  --output $PACKET/observed/after-final.json
python3 $PACKET/demo-preflight.py
```

Expected: six on `4085ac70…` with six-way agreement; the three heights as recorded in §8 (or
unset if §8 did not run). Record the result in `RELEASE-ID.txt` and the plan.

## 8b. View change at 1144, 2026-10-23 — covered by the operator's go of the day

**Why.** The first governance round for `rejected_tx_fee_activation_height` (batch `3c2a18d7…`,
amendment `e4745af7…`, H = 1153, slot 1144, signatures valid to 1154) stalled at 08:40Z. All six hold a
durable prepare and precommit for the same block at 1144 view 0: locked block `cee96283…`, payload
`e2902657…`. The proposer, validator-4, timed out reading validator-5's precommit reply
([first stop](observed/activation/STOP-20261023.json)). The same-round retry at 08:56Z was refused
locally with `consensus v2 prepare vote would violate durable round monotonicity`
([second stop](observed/activation/STOP-20261023-retry-1.json)). No command assembles the existing
commit-phase votes, so the block moves to view 1. Read from the deployed source `6b1a0585`:

- **(a) Timeout votes and certificate.**
  - Each validator signs a timeout vote for 1144 view 0 on its own host with
    `block-timeout-vote --height 1144 --view 0 --high-qc consensus-v2-signed-high-qc --skip-block-log-verify`
    (`crates/node/src/main_parts/cli_dispatch_parts/group_04.rs:2188-2220`; the high-QC string is the RPC
    path's, `finality_view_recovery.rs:2`).
  - Full-history replay is skipped because this chain's replay stops at block 1011, the same reason the
    RPC path skips it (`finality_view_recovery.rs:165-178`).
  - The v2 vote carries the host's durable `high_qc`, the view-0 prepare QC for `cee96283…`
    (`crates/node/src/consensus_v2_store.rs:346-381`). It is allowed because the host's highest timeout
    round is unset and no round above 1144/0 exists (`crates/ordering_fast/src/consensus_v2.rs:751-796`,
    `:1105-1123`).
  - `block-timeout-certificate --height 1144 --view 0 --vote-files … --skip-block-log-verify` assembles the
    certificate (`group_04.rs:2221-2253`). It needs the BFT quorum of the six: **5**
    (`consensus_artifacts.rs:1024-1026`; v2 `consensus_v2.rs:619-623`). The certificate's high QC is the
    highest of the votes' QCs (`consensus_v2.rs:629`, `:1374-1390`): here the 1144/0 prepare QC.
- **(b) View-1 proposer and proposal.**
  - The leader is `((height % n) + (view % n)) % n` (`crates/ordering_fast/src/lib.rs:355-364`), so
    1144 view 1 is **validator-5**. `block-proposer --height 1144 --view 1` agrees.
  - The view-1 proposal's `valid_qc` is the certificate's high QC (`crates/node/src/consensus_v2_finality.rs:56`).
    `verify_consensus_v2_proposal` (`consensus_v2.rs:432-461`) requires a timeout certificate for view 0,
    `valid_qc` equal to that certificate's high QC, and that QC to be a prior prepare QC **for the
    proposal's own block** (`:451-461`). Any other block is refused when the proposer signs
    (`consensus_v2_finality.rs:113`) and by every voter.
  - The v2 block id is built from height, parent, payload hash, state root and the roots, not the view
    (`consensus_v2_finality.rs:58-89`). So view 1 must re-propose the same governance batch, giving the
    same block `cee96283…` and payload `e2902657…`.
  - The locked votes are not discarded: a different or empty block cannot pass.
- **(c) Existing locks at view 1.**
  - A view-1 prepare vote needs `highest_prepare_round < 1144/1`, and the proposal's block must equal
    the lock (`consensus_v2.rs:810-836`); here it does.
  - A view-1 precommit needs `highest_precommit_round < 1144/1` and must not conflict with the lock
    (`:869-893`); 1144/1 is newer than the 1144/0 lock on the same block.
  - All six can therefore vote again for the locked block at view 1.
- **(d) Sequence** (state read on all six before and after every step; `--timeout-ms 60000
  --send-retries 3 --retry-backoff-ms 2000` on the round):
  1. State read: all six at 1143 / `7fef75aa…`, safety records at 1144/0 locked on `cee96283…`.
  2. On each host, validator-0 to validator-5 one at a time: `block-timeout-vote` (as `postfiat`, own key
     file by path) → `/var/lib/postfiat/governance-activation-20261023/timeout-1144-v0-validator-N.json`,
     copied to this server.
  3. On validator-5: `block-timeout-certificate` over the six votes, then `block-timeout-verify`.
  4. On validator-5: the same batch file (sha256 `46772a02…`), then
     `transport-peer-certified-batch-round --batch-kind governance --require-local-proposer --height 1144
     --view 1 --timeout-certificate-file … --timeout-ms 60000 --send-retries 3 --retry-backoff-ms 2000`.
  5. State read: all six at 1144 view 1, same tip and root, batch `3c2a18d7…`, payload `e2902657…`,
     receipt accepted for `e4745af7…`, archived value 1153, mempools 0.
  - **Stop** on any command error, a host not at 1144 within two minutes of the others, any disagreement,
    or a 1144 payload other than the governance batch.
- **(e) Slot and expiry.** The governance authorization is checked against the block height, not the view:
  `proposal_slot` is the next height (`mempool_proposals.rs:3233-3249`, `governance.rs:2589-2599`), with
  `proposal_slot == height` and `expires_at_height >= height` (`consensus_artifacts.rs:2888-2889`). Slot
  1144 with expiry 1154 still fits a view-1 commit at 1144.

## Rollback (one validator, emergency only)

`safe-rollout` has no rollback subcommand and makes no per-host data anchor. Each host keeps the
`store-handle-20261021` executable (`eea96ba6…`) and release directory; its units are in the local
2026-10-21 stage. [rollback-one.sh](rollback-one.sh) stops the validator, checks the 2026-10-21
executable against the data in place at the given identity, reinstalls the 2026-10-21 units,
verifies the signed 2026-10-21 manifest (`8ce1da30…`) and restarts:

```bash
scp ~/.postfiat/deployments/store-handle-20261021/stage/rootfs/etc/systemd/system/postfiat-validator-N{,-rpc}.service \
  root@HOST:/etc/postfiat/releases/store-handle-20261021/
ssh root@HOST bash -s -- validator-N HEIGHT TIP ROOT < $PACKET/rollback-one.sh
```

Rehearsed locally: the 2026-10-21 executable verifies and replays the data this build imported
and ran six services on ([packet](qualification/README.md)).

- **After the index build (§7):** rollback is safe for consensus: the old executable still reads the
  data dir (`status`, `verify-state` pass) and ignores the v2 index (`unsupported account_tx index
  schema`). It can rebuild a v1 index on a chain without repeated receipt ids (rehearsed on a fixture),
  but not on this devnet's history: its own scan and index build stop on the repeated governance id, as
  before this release. So after a rollback, `account_tx` on that host fails as it does today.
- **After §8:** do not roll back any validator once an activation record is committed: the old
  executable rejects the new amendment kinds and cannot apply that block (review XR-04).

If the executable cannot verify the data in place, the script stops before installing anything;
the fallback is the signed validator-1 backup from §4.
