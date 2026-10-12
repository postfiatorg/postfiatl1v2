# Deploy sheet — signer-rotation-20261007

The release-day sequence (2026-10-09 at the earliest), run from the signing workstation. It follows the
[2026-09-28 sheet](../combined-fastpay-20260928/DEPLOY-SHEET.md), the
[rollout runbook](../../docs/runbooks/safe-validator-rollout.md), the
[store conversion runbook](../../docs/runbooks/fastpay-committee-recovery.md#converting-the-fastswap-store-before-the-rotation)
and the [release-day procedure](../../docs/review/signer-committee-rotation-dry-run-20261005.md#live-procedure-for-release-day).
Keys are used by path only; no key material is printed, copied to a host or committed.

**Status: §0–§6 done on 2026-10-09** (deployed at height 1121,
[rollout record](observed/rollout-record.json); `demo-preflight.py` READY WITH ATTENTION).
§7 done on 2026-10-12 ([records](observed/store-conversion/)). §8 on 2026-10-12: FastPay
half done (epoch 2 installed at 1123); bridge half stopped at 8.2, refused by the RPC size
limit, after the `StopPrepare` at 1122 ([records](observed/signer-activation/)). Every
step marked **GO** needs the operator's go on the day. The name keeps its date
suffix; it is not the rollout day.

```bash
D=~/.postfiat/deployments/signer-rotation-20261007   # local, not in Git; stage prepared 2026-10-06
S=$D/stage; E=$D/evidence; R=signer-rotation-20261007
C=$S/rootfs/etc/postfiat/releases/$R; U=$S/rootfs/etc/systemd/system
B=$S/rootfs/opt/postfiat/releases/$R/postfiat-node
PACKET=deployments/signer-rotation-20261007
K=~/.postfiat/deployments/cobalt-activation-8694b99d/snapshot-keys
VULTR=~/.postfiat/deployments/cobalt-activation-8694b99d/vultr-api-key
```

## 0. Go/no-go — needs the operator's go on the day

- [ ] **GO 0.** The conference has ended (2026-10-08) and the objection window has closed (2026-10-08 evening UTC), so 2026-10-09 at the earliest; no other rollout or session is active on the fleet; `apt-daily-upgrade` is not running.

## 1. Before state (read-only)

```bash
python3 $PACKET/observe-fleet.py --output $PACKET/observed/before.json
```

Expected: six validators on `combined-fastpay-20260928` (`1f8b332d…`, build
`c93b2137`, manifest `d2fdb687…`), same height, tip and root, mempools empty.

- [ ] **GO 1.** Before state matches. Otherwise stop.

## 2. Stage and local preflight (local)

The stage and the signed manifest were prepared on 2026-10-06 (manifest
`fa4649aa…`, publisher `pfc531e0…`, valid 1791285560–1822821860). Recheck them:

```bash
sha256sum $B $C/deployment-manifest.json   # decaa411…, fa4649aa…
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

- [ ] **GO 4.** Take the backup (writes one export on validator-1; check its free disk first).

```bash
scripts/postfiat-safe-rollout backup --state-file $E/rollout-state.json \
  --evidence-dir $E/pre-rollout-backup \
  --snapshot-publisher-key-file $K/snapshot-publisher.private.json \
  --snapshot-publisher-public-key-file $K/snapshot-publisher.public.json > $E/backup.json 2> $E/backup.stderr
X=~/.cache/deploy-signer-rotation-20261007/backup-reverify
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
only on applied hosts (2026-09-28 note in the
[previous README](../combined-fastpay-20260928/README.md#notes); done this way on 2026-10-09).

For each validator, only after the previous check passed:

- [ ] **GO 5.N** (one go per validator: validator-1, validator-0, validator-2, validator-3, validator-4, validator-5).

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

Expected: all 12 validator and RPC processes on `decaa411…`;
`deployment_manifest_verified=true` on every host (the units now set
`POSTFIAT_DEPLOYMENT_VERIFIED_RECORD`); RPC units run with `--max-requests 100000`
and `RestartSec=1`; an `account_tx` read reports `truncated` only when rows are omitted.

- [x] **GO 6.** After state passes; proceed to the store conversion. Passed on 2026-10-09
  ([after state](observed/after.json)). `demo-preflight.py` ran at 2026-10-09T12:43:09Z:
  READY WITH ATTENTION, 55 PASS, 5 ATTENTION (the hand-started relays on validators
  0, 2, 3, 4 and 5), 0 FAIL.

## 7. FastSwap store conversion, one host at a time

validator-5 first, then validators 0–4. Both units of that host are stopped;
the other five keep quorum.

- [x] **GO 7.5** validator-5, 2026-10-12
- [x] **GO 7.0** validator-0, 2026-10-12
- [x] **GO 7.1** validator-1, 2026-10-12
- [x] **GO 7.2** validator-2, 2026-10-12
- [x] **GO 7.3** validator-3, 2026-10-12
- [x] **GO 7.4** validator-4, 2026-10-12

`/var/lib/postfiat` is `root:root 755`, so the `postfiat` user cannot create the
backup directory: root creates it empty first (the command refuses a non-empty one).

```bash
ssh root@HOST 'systemctl stop postfiat-validator-N-rpc.service postfiat-validator-N.service;
  ! systemctl is-active --quiet postfiat-validator-N-rpc.service && ! systemctl is-active --quiet postfiat-validator-N.service'
ssh root@HOST 'install -d -o postfiat -g postfiat -m 700 /var/lib/postfiat/fastswap-v1-backup-validator-N'
ssh root@HOST 'runuser -u postfiat -- /opt/postfiat/releases/signer-rotation-20261007/postfiat-node fastswap-store-migrate \
  --data-dir /var/lib/postfiat/validator-N --dry-run'      # would-convert, legacy_wal_records == wal_records
ssh root@HOST 'runuser -u postfiat -- /opt/postfiat/releases/signer-rotation-20261007/postfiat-node fastswap-store-migrate \
  --data-dir /var/lib/postfiat/validator-N --backup-dir /var/lib/postfiat/fastswap-v1-backup-validator-N'   # converted
ssh root@HOST 'systemctl start postfiat-validator-N.service postfiat-validator-N-rpc.service'
```

Then `fastswap_checkpoint_status` on that host's RPC must answer instead of
`fastswap_unavailable` (validators 0–4; see validator-5 below), and
`observe-fleet.py` must show six-way agreement, before the next host. Rollback
per host: stop both units, move `fastswap-v1/` aside, `cp -a` the backup back,
start the units (FastSwap is then unavailable on that host again).

**Done on 2026-10-12** with [fastswap-convert-host.py](observed/store-conversion/fastswap-convert-host.py)
(these steps, one host per run). Records: [observed/store-conversion/](observed/store-conversion/).

| Host | Stopped → started (UTC) | Rejoined | WAL bytes before → after | Store bytes before → after | Records legacy → keyed | Backup files | Key fingerprint | `fastswap_checkpoint_status` |
|---|---|---|---|---|---|---|---|---|
| validator-5 | 10:57:18 → 10:57:36 | 12 s | 177,899 → 177,899 | 1,231,643 → 1,231,692 | 26 → 26 | 33 | `7ac0eb5d…` (new) | `fastswap_unavailable` (key, below) |
| validator-0 | 11:00:39 → 11:00:49 | 22 s | 177,899 → 177,899 | 1,231,707 → 1,231,755 | 26 → 26 | 33 | `e6397873…` (new) | answers |
| validator-1 | 11:03:37 → 11:03:42 | 19 s | 177,899 → 177,899 | 1,231,830 → 1,231,831 | 26 → 26 | 34 | `0308c2a7…` (existing) | answers |
| validator-2 | 11:06:42 → 11:07:06 | 14 s | 177,899 → 177,899 | 1,231,704 → 1,231,753 | 26 → 26 | 33 | `6b09f698…` (new) | answers |
| validator-3 | 11:09:52 → 11:10:02 | 8 s | 177,899 → 177,899 | 1,231,808 → 1,231,857 | 26 → 26 | 33 | `68ac6e5b…` (new) | answers |
| validator-4 | 11:12:40 → 11:12:44 | 7 s | 177,899 → 177,899 | 1,231,600 → 1,231,649 | 26 → 26 | 33 | `67b6b305…` (new) | answers |

- Every host: dry run `would-convert` (26 of 26 records legacy), then `converted` with
  "normal open verified 26 record(s) with the keyed MAC", then a second dry run
  `nothing-to-convert`. The six original WALs were identical (SHA-256 `35d8b337…`)
  and each backup WAL matches it. The store grew by the new 48-byte `.integrity.key`
  (validator-1 already had one) and the converter's lock PID.
- `observe-fleet.py` PASS before the first host and after each: six on
  `decaa411…` at height 1121, tip `329bc2f1…`, root `aa462feb…`. No block was
  needed; no stop condition was hit and no rollback was used.
- Validators 0–4 return the same checkpoint: committee epoch 1, root `a2eebcba…`,
  `drain_ready: true`, `rotation_ready: false`, `highest_wal_sequence` 25, one active
  policy `356e5cca…`. The canonical refresh appended no `anchor_checkpoint` record
  (WAL still 177,899 B, no snapshot), so no epoch-1 checkpoint is anchored yet.
- Validator-5: its local key (rotated at heights 917 and 924) is not its member key in
  the epoch-1 committee (public-key digests `aad48519…` and `b8d197ab…`, read before
  the change). The service refuses at its key self-check before it opens the store
  (the lock file is still the converter's after the read), so FastSwap stays
  unavailable there. That is the §8 committee change; the store is converted and verified.

## 8. Signer-group activation

From the [release-day procedure](../../docs/review/signer-committee-rotation-dry-run-20261005.md#live-procedure-for-release-day).
After the objection window, so 2026-10-09 at the earliest.
Signatures are requested separately under the
[decision proposal](../../docs/review/validator-5-signer-committees-decision-proposal-20261001.md).

**Rollback (written 2026-10-12, before the first signature).** The chain records
of this section cannot be removed or undone: no `StopPrepare` fence, anchored
checkpoint, `ActivateCommittee`, route policy update or FastPay committee record
is ever deleted. Before `ActivateCommittee` (8.3) is admitted, nothing changes
who signs: epoch 1 keeps signing and the steps can stop. After it, the only
correction is forward: epoch 3, by repeating the steps under epoch 2 (a
`StopPrepare` per policy epoch, a new drained final checkpoint, `ActivateCommittee`
epoch 3, and the issuer's update to `authority_epoch` 3). The route stays at
epoch 1 until the issuer's update (8.4), so a wrong epoch-2 committee is simply
never adopted by the route. The FastPay epoch-2 record starts at 10001 and can
only be followed by epoch 3 at its own `new_orders_through_height` + 1; before
the batch commits, the signatures are discarded. The release executable can
still be rolled back (below); the chain decisions stay.

- [x] **GO 8.0.** Read-only: route `outstanding_bridge_claims_atoms` and pending returns are 0; list `fastswap_policy_snapshots`; build the epoch-2 `FastSwapCommitteeV1` from the six current registry keys (quorum 5) and record its root; `fastswap_checkpoint_status` answers on validators 0–4.
- [x] **GO 8.1.** Bridge: one epoch-1 `StopPrepare` per FastSwap policy epoch, if any (`fastswap-control-prepare --kind stop-prepare --policy-epoch N`, `fastswap-control-vote-sign` on validators 0–4, `fastswap-control-assemble`). Not reversible.
- [ ] **GO 8.2.** Bridge: final drained epoch-1 checkpoint (`fastswap_checkpoint_status` votes, `AnchorCheckpoint`) from validators 0–4. **Refused on 2026-10-12** (below); 8.3–8.5 not run.
- [ ] **GO 8.3.** Bridge: `ActivateCommittee` epoch 2 (`fastswap-control-prepare --kind activate-committee --epoch 2 --committee-root <8.0 root>`, vote-sign on each of 0–4, assemble, submit as `Control`). Not reversible; harmless until 8.4.
- [ ] **GO 8.4.** Bridge: the issuer signs `pftl_uniswap_route_bridge_policy_update` (`scripts/a666-build-route-epoch-advance.py bridge-policy-update`, `--committee-root` from 8.0, `authority_epoch 2`). Other lane; the epoch only increases.
- [ ] **GO 8.5.** Bridge: next checkpoint with `ethereum-checkpoint-vote-sign`, validator-5 with its current key; assemble 5 of 6.
- [x] **GO 8.6.** FastPay: `fastpay-committee-prepare` on one validator; confirm valid from 10001, six members, quorum 5.
- [x] **GO 8.7.** FastPay: `governance-authorization-sign` by all six at the agreed proposal slot, `governance-amendment-assemble`, `fastpay-recovery-governance-bootstrap-assemble`, submit, all below height 10001.

**Run on 2026-10-12** ([record](observed/signer-activation/signer-activation-record.json);
state read before and after each live step with [state-read.py](observed/signer-activation/state-read.py),
`observe-fleet.py` PASS each time). Each validator signed on its own host with its own key file,
by path; the work files are in `/var/lib/postfiat/signer-activation-20261012/` on each host.

| Step | Result |
|---|---|
| 8.0 | Height 1121. A666 and BMNRC routes: claims 0, pending returns 0. One FastSwap policy, `356e5cca…`, policy epoch 1. Active committee epoch 1, root `a2eebcba…`. Epoch-2 committee over the six registry keys (validator-5 `aad48519…`), quorum 5: root **`72836827f9f3ecd3042261a77abff72235f0238b6a81f673630a62199415021a1be0cbd5451878edca0e110434c1da37`**, the same on all six hosts ([committee](observed/signer-activation/epoch2-bridge-committee.json)) |
| 8.1 | `StopPrepare` (committee epoch 1, policy epoch 1, finalized height 1121), action `20d0ccfc…`, votes from validators 0–4. Submitted through `mempool_submit_fastlane_primary_finality` on validator-0: tx `fbafffe4…`, **height 1122**, block `506a634f…`, `fastlane_control_applied` on all six. The RPC replied `rpc_finality_submit_failed` because its handler only accepts `owned_deposit_applied` (`crates/node/src/rpc_cli.rs:2427`); the block committed |
| 8.2 | Five identical `drain_ready` checkpoint votes from validators 0–4 (fenced policy epochs `[1]`, no exit claims, `previous_checkpoint_id` null). **Refused** at validator-1: `fastlane_primary_json` must not exceed 65536 bytes (`crates/rpc_sdk/src/protocol_requests.rs:197`, `:2352`); the `AnchorCheckpoint` is 70,944 bytes ([transaction](observed/signer-activation/final-checkpoint-epoch1.fastlane-primary.json)). Nothing admitted; stopped here |
| 8.6 | Validator-1, read-only: epoch 2, six members, quorum 5, valid 10001–19990, registry root `86125482…`, replaces validator-5's entry ([output](observed/signer-activation/8.6-fastpay-committee-prepare-validator-1.json)) |
| 8.7 | Six authorizations at proposal slot 1123 (expiry 1133), validator-5 with its current key. Batch `66746fa2…` ([batch](observed/signer-activation/fastpay-epoch2.batch.json)) through `transport-peer-certified-batch-round --batch-kind governance` on validator-1, the proposer of 1123: **height 1123**, block `bafcc6ef…`, certificate `b2f5c70c…` (5 votes), `fastpay_recovery_committee_rotated` on all six |
| 8.8 | On all six: the latest FastPay committee is epoch 2, valid 10001–19990; `owned_recovery_capabilities` still answers epoch 1 at 1123. Payments are unchanged until 10001 |

**State after.** Height 1123, root `942d1fc2…`, six-way agreement. The bridge stays on
epoch 1 (root `a2eebcba…`) with policy epoch 1 fenced: new epoch-1 FastSwap prepares
are stopped; Ethereum route checkpoints still need validators 0–4. Validator-5 still
answers `fastswap_unavailable`.

**Before 8.2 can resume.** `mempool_submit_fastlane_primary_finality` cannot carry an
`AnchorCheckpoint` with five ML-DSA-65 votes, and the `ActivateCommittee` certificate,
which embeds that checkpoint, would be larger still. The RPC limit for these two
operations (or a compact codec) and the handler's receipt check need a fix in a
release, or another admission path needs review. The fence and the epoch-2 root
stay valid; 8.2 restarts from fresh checkpoint votes.

## Rollback (one validator, emergency only)

`safe-rollout` has no rollback subcommand and makes no per-host data anchor.
Each host keeps the `combined-fastpay-20260928` executable (`1f8b332d…`) and
release directory; its units are in the local 2026-09-28 stage.
[rollback-one.sh](rollback-one.sh) stops the validator, checks the 2026-09-28
executable against the data in place at the given identity, reinstalls the
2026-09-28 units, verifies the signed 2026-09-28 manifest and restarts:

```bash
scp ~/.postfiat/deployments/combined-fastpay-20260928/stage/rootfs/etc/systemd/system/postfiat-validator-N{,-rpc}.service \
  root@HOST:/etc/postfiat/releases/combined-fastpay-20260928/
ssh root@HOST bash -s -- validator-N HEIGHT TIP ROOT < $PACKET/rollback-one.sh
```

Rollback is pre-activation only: after 8.1 (`StopPrepare`) or 8.3
(`ActivateCommittee`) the executable can go back, the chain decisions cannot.
If the 2026-09-28 executable cannot verify the data in place, the script stops
before installing anything; the fallback is the signed validator-1 backup from §4.
