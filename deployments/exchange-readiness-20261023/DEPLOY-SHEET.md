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
  # on the proposer of slot S (block-proposer --height S); see §8b for why the last two flags are required
  $B transport-peer-certified-batch-round --data-dir PATH --topology PATH --batch-kind governance \
    --batch-file $E/$KIND.batch.json --key-file PATH --artifact-dir PATH --require-local-proposer --height S \
    --quorum-early-full-propagation --local-apply-before-certified-send \
    --timeout-ms 60000 --send-retries 3 --retry-backoff-ms 2000
done
```

- [x] **GO 8.N** (one go per record), 2026-10-23 08:38–10:34Z. Tip T = 1143, so H = 1153 for all three.
  Each record was ratified on validator-1 and signed on each host with its own key file by path; the
  amendment needs quorum 6 with support 6 and has six signed authorizations. Evidence is in
  [observed/activation/](observed/activation/).

| Kind | Amendment | Batch | Height / view / proposer | Block / root | Certificate (votes) | H | Read on six |
|---|---|---|---|---|---|---|---|
| `rejected_tx_fee_activation_height` | `e4745af7…` | `3c2a18d7…` | 1144 / **1** / validator-5 (view change, §8b) | `cee96283…` / `34ede112…` | `c26c7a99…` (6 of 6) | 1153 | identical, receipt accepted |
| `require_memo_activation_height` | `cdf4c13c…` | `ad5be4b6…` | 1145 / 0 / validator-5 | `2dc1aff3…` / `d3e6fce5…` | `60b5afef…` (5: v1–v5) | 1153 | identical, receipt accepted |
| `recipient_format_activation_height` (last, irreversible) | `a3771fed…` | `cb15f8f2…` | 1146 / 0 / validator-0 | `085d7e3b…` / `c38e5d3b…` | `365f275e…` (5: v0, v1, v3–v5) | 1153 | identical, receipt accepted |

The reads in `state-after-three-batches.json` cover all six hosts: the three blocks, the archived
amendments with value 1153, and receipts accepted.

- [x] **Grants to H.** Seven faucet grants of 1 PFT, faucet `pfcd4cc8…` → testing `pf6395ef…`, both
  canonical addresses, 10:14–10:28Z. Each was certified identically on six, view 0, with mempools 0
  (`grants/grant-N.check.json`):

| Height | Proposer | Block / root | Voters | tx |
|---|---|---|---|---|
| 1147 | validator-1 | `519299d8…` / `1e32b366…` | 5 | `64c7fd8c…` |
| 1148 | validator-2 | `20b2854e…` / `bdb082ac…` | 5 | `d8fc2dbe…` |
| 1149 | validator-3 | `0d17981a…` / `89d73bbe…` | 5 | `f80e71d7…` |
| 1150 | validator-4 | `4853941b…` / `7ed7c54f…` | 5 | `6f8f4b27…` |
| 1151 | validator-5 | `c5eb82f1…` / `e74b9e55…` | 6 | `f919fc50…` |
| 1152 | validator-0 | `88dcf352…` / `6f35a713…` | 5 | `a7cf0689…` |
| 1153 | validator-1 | `db8b87b6…` / `7141956e…` | 5 | `42e11d07…` (first block under the three rules; fee 32, accepted) |

- [x] **Probes at tip 1153** ([probes.json](observed/activation/probes/probes.json)):
  - **Fee claim:** a read-only confirmation that the rule is active. No `tec_*` transaction was sent.
    Mempool admission dry-runs execution and refuses any transfer whose receipt would not be accepted
    (`crates/node/src/mempool_proposals.rs:249-263`), so such a transaction cannot reach a block from
    the testing wallet.
  - **Require-memo:** read-only. No account carries the flag. Blocks 1138–1153 hold only transparent
    transfers and the three governance batches.
  - **Recipient format:** a read-only admission probe through validator-1's RPC, with nothing submitted.
    The 41-character `pf6395ef…532b3` is refused with `invalid_recipient_address`. The canonical
    `pf6395ef…532b38` passes the address check and then fails only on the probe's deliberate protocol
    error.
- [x] **After state** ([after-activation.json](observed/after-activation.json)):
  - all six on `4085ac70…` at 1153 / `db8b87b6…` / `7141956e…`, six-way agreement, mempools 0;
  - `demo-preflight.py` READY.
- [x] **validator-1 index rebuild.** The release's index does not follow commits, so §7 was run again on
  validator-1 only:
  - RPC unit stopped 10:32:17–10:32:28Z (11 s); the build took 6.5 s;
  - v2 index usable for 1–1153 (1112 blocks, 977 rows, 255 accounts);
  - `account_tx` for the testing wallet is served from the index and shows all 13 grants (1138–1143
    and 1147–1153);
  - six-way agreement before and after.
  Evidence: `observed/activation/account-tx-index-validator-1/`. The other five hosts' indexes stay at
  1143 and fall back to the archive scan for later blocks until the next release, whose index follows
  commits (main `0ac2bc9b`).

**Rollback after §8.** No validator may return to `store-handle-20261021` (`eea96ba6…`), because it
rejects the new amendment kinds (review XR-04). The fee-claim and require-memo rules can be postponed
by a later amendment of the same kind with a far height, since the latest amendment wins. The
recipient-format rule cannot be undone: the earliest unpaused amendment wins, so it holds from 1153.

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

**Part 2 result.** Six timeout votes 09:28:33–09:29:47Z; timeout certificate `7e2541c4…` (v2 `319dca74…`,
6 of 6 votes, quorum 5, verified) on validator-5; the view-1 round on validator-5 09:31:34–09:34:07Z built
block certificate `c26c7a99…` with six accepting votes, and validators 0–4 committed 1144 view 1
(`cee96283…` / root `34ede112…`, batch `3c2a18d7…`). The round then exited 1: its delivery to validator-5's
own service was reset four times, so validator-5 stayed at 1143
([third stop](observed/activation/view-change-1144/STOP-20261023-view-change.json)).

**Validator-5 catch-up, 09:46:27–09:46:32Z.** From validator-4: `transport-batch-send --to validator-5
--batch-kind governance` with the same batch file and the view-1 block certificate (`--timeout-ms 60000
--send-retries 3 --retry-backoff-ms 2000`): one attempt, applied, one accepted receipt. All six at
1144 / `cee96283…` / `34ede112…`, mempools 0, `rejected_tx_fee_activation_height = 1153` archived on six
([state read](observed/activation/catch-up-validator-5/state-after-catch-up.json)). The fallback
`rpc-catch-up-certified-delta` was not needed.

**Cause of the reset (not a host fault).** Without `--local-apply-before-certified-send`, the round adds the
proposer itself to the certified-send targets when its topology address is not loopback
(`crates/node/src/transport_protocol.rs:910-918`). The round holds the store lease for its whole run
(`crates/node/src/transport_runtime.rs:2838`). The local validator service opens the store before it
reads a request (`transport_runtime.rs:1130`), so it fails and drops the connection unread. The client
sees `Connection reset by peer`, and the service logs no event: none of the four attempts appears in
validator-5's event log. The live-fleet scripts pass `--quorum-early-full-propagation
--local-apply-before-certified-send` (`scripts/pfusdc-private-send-live:158-172`,
`scripts/a666-remote-sync-batch-round.py:53-55`). That applies the block in the round's own process and
sends only to the five peers. The 1145 and 1146 rounds use those flags.

**Health of validator-5 vs validator-4 (read-only, 09:50–09:56Z).**

| Reading | validator-5 | validator-4 |
|---|---|---|
| Load (1/5/15 min, 2 CPUs) | 0.87 / 0.65 / 0.65 | 0.24 / 0.09 / 0.02 |
| CPU pressure some avg10/avg60 | 1.36 / 1.38 % | 0.02 / 0.21 % |
| Memory available, swap used | 2.55 GB of 4.1 GB, 68 MB | 2.60 GB of 4.1 GB, 68 MB |
| Memory pressure | 0 | 0 |
| Disk free | 25.8 GB (67 % used) | 22.7 GB (71 % used) |
| Validator unit | active since 07:36:56, NRestarts 0, 107 MB, 1 thread, 5 fds | active since 07:31:06, NRestarts 0, 86 MB, 1 thread, 5 fds |
| RPC unit | active since 08:21:22 (§7), NRestarts 0, 794 MB (peak 1.47 GB) | active since 08:17:54, NRestarts 0, 721 MB (peak 1.41 GB) |
| Unit limits | none (MemoryMax, CPUQuota unset; TasksMax 1024; NOFILE 65536) | same |
| RPC CPU over 10 s | 56 % of one CPU | 32 % (fleet range 32–56 %; v2 52 %) |
| Transport | 1 listener 10.77.0.6:26660 | 1 listener 10.77.0.5:26658 |
| Journal, last 2 h | validator 0 lines; RPC 6 lines, 0 error/reset/timeout; kernel clean | same |
| Transport events, last 400 | 395 accepted; 5 refused votes (the duplicate-vote refusals of the view change) | 400 accepted |
| 28701/28702/28703, cobalt-shadow | active, NRestarts 0 | active, NRestarts 0 |

Verdict: **clean**. There is no sustained load, no memory pressure, no unit near a limit and no reset in
the journal. The RPC units use a third to a half of one CPU on all six hosts, so this is a fleet-wide
pattern rather than something specific to validator-5. The 09:34Z reset is the self-send described
above. The 08:40Z read timeout is validator-4's then-default 5 s read timeout expiring while waiting for
validator-5's vote reply; all later rounds use 60 s.
Evidence: `observed/activation/health-validator-5/`.

**After the health check.** `require_memo_activation_height` landed at 1145 view 0 with proposer
validator-5. The round ran 10:02:01–10:03:11Z with the two flags above: no send failures, certificate
5 of 6, and validator-0's vote was still outstanding at quorum. `recipient_format_activation_height`
landed at 1146 view 0 with proposer validator-0, 10:07:03–10:07:53Z. After each round, all six read the
same height, tip and root. A first launch of the require-memo script at 09:58Z was cut off by its
launching shell during signing, before any authorization existed. The re-run produced the same
amendment id `cdf4c13c…` ([note](observed/activation/require-memo-interrupted-0958-NOTE.txt)).

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
