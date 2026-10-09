# Deploy sheet — signer-rotation-20261007

The release-day sequence (2026-10-09 at the earliest), run from the signing workstation. It follows the
[2026-09-28 sheet](../combined-fastpay-20260928/DEPLOY-SHEET.md), the
[rollout runbook](../../docs/runbooks/safe-validator-rollout.md), the
[store conversion runbook](../../docs/runbooks/fastpay-committee-recovery.md#converting-the-fastswap-store-before-the-rotation)
and the [release-day procedure](../../docs/review/signer-committee-rotation-dry-run-20261005.md#live-procedure-for-release-day).
Keys are used by path only; no key material is printed, copied to a host or committed.

**Status: §0–§5 and the §6 after state done on 2026-10-09** (deployed at height 1121,
[rollout record](observed/rollout-record.json)); `demo-preflight.py`, §7 and §8 not yet run. Every
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

- [ ] **GO 6.** After state passes; proceed to the store conversion.

## 7. FastSwap store conversion, one host at a time

validator-5 first, then validators 0–4. Both units of that host are stopped;
the other five keep quorum.

- [ ] **GO 7.N** (one go per host: validator-5, validator-0, validator-1, validator-2, validator-3, validator-4).

```bash
ssh root@HOST 'systemctl stop postfiat-validator-N-rpc.service postfiat-validator-N.service;
  ! systemctl is-active --quiet postfiat-validator-N-rpc.service && ! systemctl is-active --quiet postfiat-validator-N.service'
ssh root@HOST 'runuser -u postfiat -- /opt/postfiat/releases/signer-rotation-20261007/postfiat-node fastswap-store-migrate \
  --data-dir /var/lib/postfiat/validator-N --dry-run'      # would-convert, legacy_wal_records == wal_records
ssh root@HOST 'runuser -u postfiat -- /opt/postfiat/releases/signer-rotation-20261007/postfiat-node fastswap-store-migrate \
  --data-dir /var/lib/postfiat/validator-N --backup-dir /var/lib/postfiat/fastswap-v1-backup-validator-N'   # converted
ssh root@HOST 'systemctl start postfiat-validator-N.service postfiat-validator-N-rpc.service'
```

Then `fastswap_checkpoint_status` on that host's RPC must answer instead of
`fastswap_unavailable`, and `observe-fleet.py` must show six-way agreement,
before the next host. Rollback per host: stop both units, move `fastswap-v1/`
aside, `cp -a` the backup back, start the units (FastSwap is then unavailable
on that host again).

## 8. Signer-group activation

From the [release-day procedure](../../docs/review/signer-committee-rotation-dry-run-20261005.md#live-procedure-for-release-day).
After the objection window, so 2026-10-09 at the earliest.
Signatures are requested separately under the
[decision proposal](../../docs/review/validator-5-signer-committees-decision-proposal-20261001.md).

- [ ] **GO 8.0.** Read-only: route `outstanding_bridge_claims_atoms` and pending returns are 0; list `fastswap_policy_snapshots`; build the epoch-2 `FastSwapCommitteeV1` from the six current registry keys (quorum 5) and record its root; `fastswap_checkpoint_status` answers on validators 0–4.
- [ ] **GO 8.1.** Bridge: one epoch-1 `StopPrepare` per FastSwap policy epoch, if any (`fastswap-control-prepare --kind stop-prepare --policy-epoch N`, `fastswap-control-vote-sign` on validators 0–4, `fastswap-control-assemble`). Not reversible.
- [ ] **GO 8.2.** Bridge: final drained epoch-1 checkpoint (`fastswap_checkpoint_status` votes, `AnchorCheckpoint`) from validators 0–4.
- [ ] **GO 8.3.** Bridge: `ActivateCommittee` epoch 2 (`fastswap-control-prepare --kind activate-committee --epoch 2 --committee-root <8.0 root>`, vote-sign on each of 0–4, assemble, submit as `Control`). Not reversible; harmless until 8.4.
- [ ] **GO 8.4.** Bridge: the issuer signs `pftl_uniswap_route_bridge_policy_update` (`scripts/a666-build-route-epoch-advance.py bridge-policy-update`, `--committee-root` from 8.0, `authority_epoch 2`). Other lane; the epoch only increases.
- [ ] **GO 8.5.** Bridge: next checkpoint with `ethereum-checkpoint-vote-sign`, validator-5 with its current key; assemble 5 of 6.
- [ ] **GO 8.6.** FastPay: `fastpay-committee-prepare` on one validator; confirm valid from 10001, six members, quorum 5.
- [ ] **GO 8.7.** FastPay: `governance-authorization-sign` by all six at the agreed proposal slot, `governance-amendment-assemble`, `fastpay-recovery-governance-bootstrap-assemble`, submit, all below height 10001.

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
