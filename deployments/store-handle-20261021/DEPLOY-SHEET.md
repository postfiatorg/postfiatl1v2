# Deploy sheet — store-handle-20261021

The release-day sequence (2026-10-21), run from the signing workstation. It follows the
[fastlane-cap-20261014 sheet](https://github.com/postfiatorg/postfiatl1v2/blob/release/fastlane-cap-20261014/deployments/fastlane-cap-20261014/DEPLOY-SHEET.md),
the [rollout runbook](../../docs/runbooks/safe-validator-rollout.md) and the
[FastSwap committee migration runbook](../../docs/runbooks/fastpay-committee-recovery.md#moving-the-fastswap-store-to-the-activated-committee-epoch).
Keys are used by path only; no key material is printed, copied to a host or committed.

**Status: §0–§8 done on 2026-10-21** (deployed at height 1137, [rollout record](observed/rollout-record.json);
FastSwap committee epoch 2 on all six, [§7](#7-fastswap-committee-migration-one-host-at-a-time--covered-by-the-operators-go-of-the-day)). Cut and locally qualified on 2026-10-21 ([packet](qualification/README.md)).
Every live section (§0, §1, §3–§8) is covered by the operator's go of the day; each step marked
**GO** needs that go. No store conversion, no FastPay committee record and no chain transaction
are part of this release.

```bash
D=~/.postfiat/deployments/store-handle-20261021   # local, not in Git; stage prepared 2026-10-21
S=$D/stage; E=$D/evidence; R=store-handle-20261021
C=$S/rootfs/etc/postfiat/releases/$R; U=$S/rootfs/etc/systemd/system
B=$S/rootfs/opt/postfiat/releases/$R/postfiat-node
PACKET=deployments/store-handle-20261021
K=~/.postfiat/deployments/cobalt-activation-8694b99d/snapshot-keys
VULTR=~/.postfiat/deployments/cobalt-activation-8694b99d/vultr-api-key
```

## 0. Go/no-go — covered by the operator's go of the day

- [x] **GO 0.** No other rollout or session is active on the fleet; `apt-daily-upgrade` is not running; CI on `86706699` has no failed workflow (`gh run list --commit 86706699f646405743385beabf609c6771a518a3`).

## 1. Before state (read-only) — covered by the operator's go of the day

```bash
python3 $PACKET/observe-fleet.py --output $PACKET/observed/before.json
```

Expected: six validators on `fastlane-cap-20261014` (`b1b3dbb1…`, build
`8bed9b90`, manifest `4f379c57…`), height 1131, tip `9b054f09…`, root `a12e7c5f…`
(or a later height with six-way agreement), mempools empty.

- [x] **GO 1.** Before state matches. Otherwise stop.

## 2. Stage and local preflight (local)

The stage and the signed manifest were prepared on 2026-10-21 (manifest
`8ce1da30…`, publisher `pfc531e0…`, valid 1792578425–1824114725). Recheck them:

```bash
sha256sum $B $C/deployment-manifest.json   # eea96ba6…, 8ce1da30…
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

## 4. Fresh signed canary backup from validator-1 — covered by the operator's go of the day

- [x] **GO 4.** Take the backup (writes one export on validator-1; check its free disk first).

```bash
scripts/postfiat-safe-rollout backup --state-file $E/rollout-state.json \
  --evidence-dir $E/pre-rollout-backup \
  --snapshot-publisher-key-file $K/snapshot-publisher.private.json \
  --snapshot-publisher-public-key-file $K/snapshot-publisher.public.json > $E/backup.json 2> $E/backup.stderr
X=~/.cache/deploy-store-handle-20261021/backup-reverify
$B snapshot-import-signed-finalized-checkpoint --data-dir $X --snapshot-dir $E/pre-rollout-backup/backup-signed \
  --trusted-publisher-key-file $K/snapshot-publisher.public.json --node-id validator-1
$B verify-finalized-checkpoint --data-dir $X     # verified: true, the before-state height, tip and root
$B verify-state --data-dir $X                    # full replay, same root
```

Record the backup manifest hash in `RELEASE-ID.txt`.

## 5. One validator at a time — covered by the operator's go of the day

Order: validator-1 (canary), then 0, 2, 3, 4, 5. Before the first grant, point
StakeHub's `~/.pft/config.toml` `local_node_binary` at this release. Point
`runtime_binary` and `topology_file` at it only after the last validator is applied: the
faucet runs `runtime_binary status` on all six hosts, and the new path exists
only on applied hosts (done this way on 2026-10-09 and 2026-10-15).

For each validator, only after the previous check passed:

- [x] **GO 5.N** (one go per validator: validator-1, validator-0, validator-2, validator-3, validator-4, validator-5).

```bash
scripts/postfiat-safe-rollout apply-next --state-file $E/rollout-state.json > $E/apply-N.json
(cd ~/repos/StakeHub && .venv/bin/pft faucet testing --asset PFT --amount 1000000) > $E/grant-after-<validator>.out
python3 $PACKET/observe-fleet.py --stage $S --applied validator-1[,validator-0,...] \
  --output $PACKET/observed/after-<validator>.json
```

Each grant must certify at the same height with the same tip and root on all
six (`blocks` RPC on all six). On divergence or a missed certification: stop
and use [Rollback](#rollback-one-validator-emergency-only) on that validator.

## 6. After checks — covered by the operator's go of the day

```bash
python3 $PACKET/observe-fleet.py --stage $S \
  --applied validator-1,validator-0,validator-2,validator-3,validator-4,validator-5 \
  --output $PACKET/observed/after.json
python3 $PACKET/demo-preflight.py
```

Expected: all 12 validator and RPC processes on `eea96ba6…`;
`deployment_manifest_verified=true` on every host; RPC units with
`--max-requests 100000` and `RestartSec=1`.

- [x] **GO 6.** After checks pass; proceed to §7.

## 7. FastSwap committee migration, one host at a time — covered by the operator's go of the day

Exactly as the [runbook section](../../docs/runbooks/fastpay-committee-recovery.md#moving-the-fastswap-store-to-the-activated-committee-epoch).
Order: validator-5 first, then 0, 1, 2, 3, 4. Only files under `fastswap-v1/` change; no
transaction is sent. The inputs are the 2026-10-15 files in
[observed/bridge-activation/](observed/bridge-activation/):
`7.2-activate-committee.fastlane-primary.json` (sha256 `35acb2ff…`, committee epoch 2, root
`72836827…`) and `7.1-anchor-checkpoint.fastlane-primary.json` (sha256 `4497c495…`, anchor `355fe608…`).

- [x] **GO 7.N** (one go per host: validator-5, validator-0, validator-1, validator-2, validator-3, validator-4).

```bash
N=5   # then 0, 1, 2, 3, 4
ssh root@HOST install -d -o postfiat -g postfiat -m 700 /var/lib/postfiat/fastswap-migration-20261021
scp $PACKET/observed/bridge-activation/7.{1-anchor-checkpoint,2-activate-committee}.fastlane-primary.json \
  root@HOST:/var/lib/postfiat/fastswap-migration-20261021/
ssh root@HOST
  N=5; I=/var/lib/postfiat/fastswap-migration-20261021   # on the host
  chown postfiat:postfiat $I/7.*.fastlane-primary.json && sha256sum $I/7.*.fastlane-primary.json   # 4497c495…, 35acb2ff…
  systemctl stop postfiat-validator-$N-rpc.service postfiat-validator-$N.service
  systemctl is-active postfiat-validator-$N-rpc.service postfiat-validator-$N.service   # inactive, inactive
  install -d -o postfiat -g postfiat -m 700 /var/lib/postfiat/fastswap-v1-epoch2-backup-validator-$N
  M="runuser -u postfiat -- /opt/postfiat/releases/store-handle-20261021/postfiat-node fastswap-committee-migrate \
     --data-dir /var/lib/postfiat/validator-$N \
     --committee-file $I/7.2-activate-committee.fastlane-primary.json --anchor-file $I/7.1-anchor-checkpoint.fastlane-primary.json"
  $M --dry-run                                                               # outcome would-migrate; read notes
  $M --backup-dir /var/lib/postfiat/fastswap-v1-epoch2-backup-validator-$N   # outcome migrated
  systemctl start postfiat-validator-$N.service postfiat-validator-$N-rpc.service
```

Expected: `migrated` and `service open: committee epoch 2, root 72836827…, enabled true`.
Then, before the next host: both units active; `fastswap_capabilities` shows committee epoch 2,
root `72836827…`, `enabled: true`; `fastswap_checkpoint_status` answers;
`observe-fleet.py --stage $S --applied <all six>` shows six-way agreement. Keep each report
in [observed/committee-migration/](observed/committee-migration/).

validator-5 may be refused because its store never applied the epoch-1 fence or the anchor
(the command accepts such a store only if its replayed state still equals the anchored drained
state, with a note). If refused: record the refusal as the note for validator-5, start its two
units unchanged, and continue with the others.

**Result (2026-10-21, height 1137 throughout, no block produced or needed).** All six migrated;
each host was back at 1137 / `18a4dc86…` / `1277c76c…` at the first poll (11–18 s after its start), both units
active with 0 restarts, `rpc.ready.json` ready and not degraded, six-way agreement before and after
every host ([observations](observed/committee-migration/observations.json)). Store before: epoch 1,
root `a2eebcba…`; after: epoch 2, root `72836827…`, 2 objects kept, anchor `355fe608…` recorded.
Before the run `fastswap_capabilities` reported epoch 1 `enabled: false` on validators 0–4 and
`fastswap_unavailable` on validator-5.

| Host | Dry run | Real run | Store before → after | Stopped → started (UTC) | `fastswap_capabilities` after |
|---|---|---|---|---|---|
| validator-5 | would-migrate, note¹ | migrated (base+WAL, 26 records; WAL moved aside) | 1 `a2eebcba…` → 2 `72836827…` | 12:07:44 → 12:08:09 | epoch 2, `72836827…`, enabled true; checkpoint status answers |
| validator-0 | would-migrate | migrated (snapshot; WAL and snapshot moved aside) | 1 `a2eebcba…` → 2 `72836827…` | 12:12:25 → 12:12:30 | epoch 2, `72836827…`, enabled true; checkpoint status answers |
| validator-1 | would-migrate | migrated (snapshot; WAL and snapshot moved aside) | 1 `a2eebcba…` → 2 `72836827…` | 12:15:51 → 12:15:53 | epoch 2, `72836827…`, enabled true; checkpoint status answers |
| validator-2 | would-migrate | migrated (snapshot; WAL and snapshot moved aside) | 1 `a2eebcba…` → 2 `72836827…` | 12:19:35 → 12:19:50 | epoch 2, `72836827…`, enabled true; checkpoint status answers |
| validator-3 | would-migrate | migrated (snapshot; WAL and snapshot moved aside) | 1 `a2eebcba…` → 2 `72836827…` | 12:23:21 → 12:23:26 | epoch 2, `72836827…`, enabled true; checkpoint status answers |
| validator-4 | would-migrate | migrated (snapshot; WAL and snapshot moved aside) | 1 `a2eebcba…` → 2 `72836827…` | 12:27:06 → 12:27:08 | epoch 2, `72836827…`, enabled true; checkpoint status answers |

Every real run reported `service open: committee epoch 2, root 72836827…, enabled true`; reports
`committee-migrate-validator-N{.dry-run,}.json`, RPC reads `fastswap-rpc-after-validator-N.json`.
Backups: `/var/lib/postfiat/fastswap-v1-epoch2-backup-validator-N` on each host.

¹ validator-5 was not refused. Note, verbatim: "store (base+WAL load, 26 WAL record(s), 0 fence and
0 anchor record(s)) never applied the epoch-1 prepare fence for policy epoch(s) [1] or the anchored
drain checkpoint 355fe608…; its replayed state equals the anchored drained state (no reservations,
no pending burns, every checkpoint root and total matches), so the migration subsumes them".
Its FastSwap service, unavailable before, now opens at epoch 2.

Store rollback for one host: stop both units, restore `fastswap-v1/` from
`/var/lib/postfiat/fastswap-v1-epoch2-backup-validator-N` with `cp -a`, start both units.

## 8. After state — covered by the operator's go of the day

```bash
python3 $PACKET/observe-fleet.py --stage $S \
  --applied validator-1,validator-0,validator-2,validator-3,validator-4,validator-5 \
  --output $PACKET/observed/after-migration.json
python3 $PACKET/demo-preflight.py
```

Expected: six on `eea96ba6…` with six-way agreement; FastSwap committee epoch 2 (root `72836827…`,
`enabled: true`) on every migrated host. Record the result in `RELEASE-ID.txt` and the plan.

- [x] **GO 8.** 2026-10-21 12:30Z: [after-migration.json](observed/after-migration.json) PASS, six on
  `store-handle-20261021` at 1137 / `18a4dc86…` / `1277c76c…`, mempools 0;
  [demo preflight](observed/demo-preflight-after-migration.json) READY, 60 PASS, 0 ATTENTION, 0 FAIL;
  `fastswap_capabilities` epoch 2, `72836827…`, enabled true on all six.

## Rollback (one validator, emergency only)

`safe-rollout` has no rollback subcommand and makes no per-host data anchor.
Each host keeps the `fastlane-cap-20261014` executable (`b1b3dbb1…`) and
release directory; its units are in the local 2026-10-15 stage.
[rollback-one.sh](rollback-one.sh) stops the validator, checks the 2026-10-15
executable against the data in place at the given identity, reinstalls the
2026-10-15 units, verifies the signed 2026-10-15 manifest (`4f379c57…`) and restarts:

```bash
scp ~/.postfiat/deployments/fastlane-cap-20261014/stage/rootfs/etc/systemd/system/postfiat-validator-N{,-rpc}.service \
  root@HOST:/etc/postfiat/releases/fastlane-cap-20261014/
ssh root@HOST bash -s -- validator-N HEIGHT TIP ROOT < $PACKET/rollback-one.sh
```

After §7 on that host, restore its `fastswap-v1/` from the backup first (above): the
`fastlane-cap-20261014` executable reading a migrated epoch-2 store was not rehearsed.
If the executable cannot verify the data in place, the script stops before installing
anything; the fallback is the signed validator-1 backup from §4.
