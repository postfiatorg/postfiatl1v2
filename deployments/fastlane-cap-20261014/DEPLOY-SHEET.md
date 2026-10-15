# Deploy sheet — fastlane-cap-20261014

The release-day sequence (2026-10-15), run from the signing workstation. It follows the
[signer-rotation-20261007 sheet](https://github.com/postfiatorg/postfiatl1v2/blob/release/signer-rotation-20261007/deployments/signer-rotation-20261007/DEPLOY-SHEET.md),
the [rollout runbook](../../docs/runbooks/safe-validator-rollout.md) and the
[FastLane control submission notes](../../docs/runbooks/fastpay-committee-recovery.md#submitting-fastlane-control-transactions).
Keys are used by path only; no key material is printed, copied to a host or committed.

**Status: §0–§7 done on 2026-10-15** (deployed at height 1129, [rollout record](observed/rollout-record.json);
bridge committee epoch 2 active at height 1131, [record](observed/bridge-activation/bridge-activation-record.json)).
The issuer's route update (7.3) is prepared unsigned and waits for the other lane.
Every step marked **GO** needs the operator's go on the day.
No store conversion (done 2026-10-12) and no FastPay committee record (done 2026-10-12, epoch 2 at height 1123).

```bash
D=~/.postfiat/deployments/fastlane-cap-20261014   # local, not in Git; stage prepared 2026-10-15
S=$D/stage; E=$D/evidence; R=fastlane-cap-20261014
C=$S/rootfs/etc/postfiat/releases/$R; U=$S/rootfs/etc/systemd/system
B=$S/rootfs/opt/postfiat/releases/$R/postfiat-node
PACKET=deployments/fastlane-cap-20261014
K=~/.postfiat/deployments/cobalt-activation-8694b99d/snapshot-keys
VULTR=~/.postfiat/deployments/cobalt-activation-8694b99d/vultr-api-key
```

## 0. Go/no-go — needs the operator's go on the day

- [x] **GO 0.** No other rollout or session is active on the fleet; `apt-daily-upgrade` is not running; CI on `8bed9b90` has no failed workflow (`gh run list --branch main`).

## 1. Before state (read-only)

```bash
python3 $PACKET/observe-fleet.py --output $PACKET/observed/before.json
```

Expected: six validators on `signer-rotation-20261007` (`decaa411…`, build
`0fa55d0b`, manifest `fa4649aa…`), height 1123, tip `bafcc6ef…`, root `942d1fc2…`
(or a later height with six-way agreement), mempools empty.

- [x] **GO 1.** Before state matches. Otherwise stop.

## 2. Stage and local preflight (local)

The stage and the signed manifest were prepared on 2026-10-15 (manifest
`4f379c57…`, publisher `pfc531e0…`, valid 1792057923–1823594223). Recheck them:

```bash
sha256sum $B $C/deployment-manifest.json   # b1b3dbb1…, 4f379c57…
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

Expected: six-way agreement, six signer rosters valid, 0 deletions, order 1, 0, 2, 3, 4, 5.

## 4. Fresh signed canary backup from validator-1

- [x] **GO 4.** Take the backup (writes one export on validator-1; check its free disk first).

```bash
scripts/postfiat-safe-rollout backup --state-file $E/rollout-state.json \
  --evidence-dir $E/pre-rollout-backup \
  --snapshot-publisher-key-file $K/snapshot-publisher.private.json \
  --snapshot-publisher-public-key-file $K/snapshot-publisher.public.json > $E/backup.json 2> $E/backup.stderr
X=~/.cache/deploy-fastlane-cap-20261014/backup-reverify
$B snapshot-import-signed-finalized-checkpoint --data-dir $X --snapshot-dir $E/pre-rollout-backup/backup-signed \
  --trusted-publisher-key-file $K/snapshot-publisher.public.json --node-id validator-1
$B verify-finalized-checkpoint --data-dir $X     # verified: true, the before-state height, tip and root
$B verify-state --data-dir $X                    # full replay, same root
```

Record the backup manifest hash in `RELEASE-ID.txt`.

## 5. One validator at a time

Order: validator-1 (canary), then 0, 2, 3, 4, 5. Before the first grant, point
StakeHub's `~/.pft/config.toml` `local_node_binary` at this release. Point
`runtime_binary` and `topology_file` at it only after all six are applied: the
faucet runs `runtime_binary status` on all six hosts, and the new path exists
only on applied hosts (done this way on 2026-10-09).

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

## 6. After state and live checks

```bash
python3 $PACKET/observe-fleet.py --stage $S \
  --applied validator-1,validator-0,validator-2,validator-3,validator-4,validator-5 \
  --output $PACKET/observed/after.json
python3 $PACKET/demo-preflight.py
```

Expected: all 12 validator and RPC processes on `b1b3dbb1…`;
`deployment_manifest_verified=true` on every host; RPC units with
`--max-requests 100000` and `RestartSec=1`.

- [x] **GO 6.** After state passes; proceed to §7.

## 7. Signer-group activation, bridge half

The steps the previous sheet stopped at (its §8.2–8.4). The epoch-1 `StopPrepare`
committed at 1122 and stays valid.

**Rollback (written 2026-10-15, before the first signature).** The chain records of
this section cannot be removed or undone: no anchored checkpoint, `ActivateCommittee`
or route policy update is ever deleted. Before 7.2 is admitted nothing changes who
signs: epoch 1 keeps signing and the steps can stop; an anchored drained checkpoint
alone changes no signer. After 7.2 the only correction is forward: epoch 3, by repeating
the steps under epoch 2 (a `StopPrepare` per policy epoch, a new drained final
checkpoint, `ActivateCommittee` epoch 3, and the issuer's update to `authority_epoch` 3).
The route stays at `authority_epoch` 1 until the issuer's update (7.3), so a wrong
epoch-2 committee is simply never adopted by the route. The executable can still be
rolled back (below); the chain decisions stay.

Submit each transaction as a file (`--fastlane-primary-file`, Linux caps one argument at 128 KiB)
through `mempool_submit_fastlane_primary_finality` on the proposer of the next height.

- [x] **GO 7.1** (previous 8.2). Final drained epoch-1 checkpoint: fresh `fastswap_checkpoint_status` votes from validators 0–4 (identical, `drain_ready`, fenced policy epochs `[1]`, `previous_checkpoint_id` null), assembled as an `AnchorCheckpoint`; the receipt must carry `fastlane_checkpoint_anchored`. Do not reuse the 2026-10-12 votes. Needs the operator's go on the day.
- [x] **GO 7.2** (previous 8.3). `ActivateCommittee` epoch 2: `fastswap-control-prepare --kind activate-committee --epoch 2 --committee-root 72836827f9f3ecd3042261a77abff72235f0238b6a81f673630a62199415021a1be0cbd5451878edca0e110434c1da37`, `fastswap-control-vote-sign` on each of 0–4, `fastswap-control-assemble`, submit as `Control`; the receipt must carry `fastlane_control_applied`. Not reversible; harmless until 7.3. Needs the operator's go on the day.
- [x] **GO 7.3** (previous 8.4). Build the unsigned `pftl_uniswap_route_bridge_policy_update` (`scripts/a666-build-route-epoch-advance.py bridge-policy-update`, the root above, `authority_epoch 2`) and hand it to the issuer to sign; the other lane submits it. The epoch only increases. Needs the operator's go on the day.

After 7.3: the next Ethereum checkpoint is signed with `ethereum-checkpoint-vote-sign`,
validator-5 with its current key, 5 of 6 (previous 8.5), on the operator's go.

**Run on 2026-10-15** ([record](observed/bridge-activation/bridge-activation-record.json);
state read before and after each step with [state-read.py](observed/bridge-activation/state-read.py),
`observe-fleet.py` PASS each time). Each validator signed on its own host with its own key file,
by path; work files are in `/var/lib/postfiat/bridge-activation-20261015/` on validators 0–4.
Each transaction was sent from its file through the RPC socket (no command-line argument).

| Step | Result |
|---|---|
| 7.0 | Height 1129, tip `41154d2c…`, root `053fb35c…`, mempools 0. A666 and BMNRC routes: claims 0, pending returns 0. Committee epoch 1, root `a2eebcba…`; policy epoch 1 fenced; validators 0–4 `drain_ready`, `previous_checkpoint_id` null. Epoch-2 root recomputed on all six: `72836827…` (registry file `43e58834…`, as on 2026-10-12) |
| 7.1 | Five fresh identical votes from validators 0–4 (WAL 26, no exit claims, checkpoint `355fe608…`). `AnchorCheckpoint` (71,005 bytes) through validator-2, the proposer of 1130: tx `f34dd1f5…`, **height 1130**, block `4dafdfc8…`, root `fe991e14…`, `fastlane_checkpoint_anchored` on all six; the RPC reported success |
| 7.2 | `fastswap-control-prepare` on each of 0–4 gave the same control file (`291670cc…`), action `0ffe278b…`. Votes from 0–4, assembled on validator-3. `ActivateCommittee` (176,546 bytes) through validator-3, the proposer of 1131: tx `2b2efa37…`, **height 1131**, block `9b054f09…`, root `a12e7c5f…`, `fastlane_control_applied` on all six; the RPC reported success. All six now report active epoch 2; block 1131 carries root `72836827…`, quorum 5, six members |
| 7.3 | [Unsigned update](observed/a666-bridge-policy-update.unsigned.json), sha256 `24aa2e28ff942d0be9557416d2b796d6b9a8131abf7704c6ad1348539f51c03f`: A666 route, issuer `pffcb93d…`, `authority_epoch` 2, root `72836827…`, echoed fields from the [current policy](observed/bridge-activation/a666-route-policy-current.json) (12 confirmations, `4c62b7d8…`, `671ee905…`). Not signed, not submitted; `key_file` is a placeholder for the issuer's key |

**State after.** Height 1131, root `a12e7c5f…`, six-way agreement, mempools 0. The routes stay on
`authority_epoch` 1 (root `a2eebcba…`) until the issuer's update. `fastswap_capabilities` still
reports each validator's local FastSwap committee (`fastswap-v1/committee.json`: epoch 1,
`enabled` false); the local committee is not switched by this section.

**Remaining (other lane).** The issuer signs and submits the update; the receipt must carry
`route_bridge_policy_updated`. The BMNRC route also stays on epoch 1 until its issuer sends
the same update. Then the next Ethereum checkpoint (previous 8.5).

## Rollback (one validator, emergency only)

`safe-rollout` has no rollback subcommand and makes no per-host data anchor.
Each host keeps the `signer-rotation-20261007` executable (`decaa411…`) and
release directory; its units are in the local 2026-10-07 stage.
[rollback-one.sh](rollback-one.sh) stops the validator, checks the 2026-10-07
executable against the data in place at the given identity, reinstalls the
2026-10-07 units, verifies the signed 2026-10-07 manifest (`fa4649aa…`) and restarts:

```bash
scp ~/.postfiat/deployments/signer-rotation-20261007/stage/rootfs/etc/systemd/system/postfiat-validator-N{,-rpc}.service \
  root@HOST:/etc/postfiat/releases/signer-rotation-20261007/
ssh root@HOST bash -s -- validator-N HEIGHT TIP ROOT < $PACKET/rollback-one.sh
```

After 7.1 or 7.2 the executable can go back, the chain decisions cannot. The 2026-10-07
executable executes these operations with the same receipt codes (`7f9d0d98` only names them
in `crates/execution/src/fastlane_primary.rs`); only its RPC refuses them above 64 KiB. Replay
of such a block by it was not rehearsed. If it cannot verify the data in place, the script
stops before installing anything; the fallback is the signed validator-1 backup from §4.
