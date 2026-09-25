# Deploy sheet — combined-fastpay-20260925

Exact commands used on 2026-09-25 on the signing workstation. They mirror the r4
rollout ([handoff](../../docs/handoffs/2026-09-25___postfiatchad__r4_view_recovery_chain_unstuck.md#deployment-evidence),
[runbook](../../docs/runbooks/safe-validator-rollout.md)). Keys are used by path
only; no key material is printed, copied to a host or committed.

**Status: prepared and signed, not applied** (`status=PREPARED_SIGNED_NOT_APPLIED`).
Steps 1–5 were done on 2026-09-25: before state at height 1044, staging, signed
manifest `7a682ffe…`, rollout preflight PASS, signed canary backup at 1044 with
root `8f22d40f…`. No apply was run. The step-6 gate (full workspace test run)
was not met in the time available.

```bash
D=~/.postfiat/deployments/combined-fastpay-20260925   # local, not in Git
S=$D/stage; E=$D/evidence; R=combined-fastpay-20260925
C=$S/rootfs/etc/postfiat/releases/$R; U=$S/rootfs/etc/systemd/system
B=$S/rootfs/opt/postfiat/releases/$R/postfiat-node
PACKET=deployments/combined-fastpay-20260925
BIN=~/.cache/release-repair-20260925/binaries/candidate-1
```

## 1. Before state (read-only)

`apt-daily-upgrade.timer` on all six hosts: next run 2026-09-26 06:02–06:56Z; no
upgrade running. The six RPC tunnels `127.0.0.1:27650..27655` of `pft-fastpay`
were already up.

```bash
python3 $PACKET/observe-fleet.py --output $PACKET/observed/before.json
```

## 2. Stage from the r4 layout

The r4 files on validator-0 (`/etc/postfiat/releases/fastpay-committee-20260925-r4/`
and both units) were read with `ssh cat` and matched the local r4 stage byte for
byte. The executable is `~/.cache/release-repair-20260925/binaries/candidate-1`
(`d66cecc3…`, rechecked).

```bash
L=$D/r4-layout-validator-0
$BIN deployment-validator-units-stage --release-id $R \
  --topology-file $L/topology.json --binary-file $BIN \
  --swap-circuit-metadata-file $L/swap.metadata.json \
  --private-egress-circuit-metadata-file $L/private-egress.metadata.json \
  --output-dir $S
install -m 0644 ~/.postfiat/deployments/fastpay-committee-20260925-r4/keys/deployment.public.json $C/deployment.public.json
```

Against r4, 27 of the 33 generated files are identical after substituting the
release ID. The six RPC units differ in one line only, `Restart=on-failure` →
`Restart=always` (combined-line change `15af691d`). Topology and both circuit
metadata files are unchanged (`4df248e0…`, `3750cda7…`, `d371570a…`).

## 3. Sign and verify locally

As in r4, the top-level unit and environment are validator-0's RPC unit and
environment.

```bash
$B deployment-manifest-create --deployment-id $R \
  --valid-from-unix 1790338211 --valid-until-unix 1821874511 \
  --chain-id postfiat-wan-devnet-2 --genesis-hash ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9 \
  --git-revision f60e9639f83649769f29276a9de14f5f16271877 --binary-file $B \
  --build-profile release --build-features privacy,rpc,transport \
  --protocol-version 1 --rpc-schema postfiat-local-rpc-v1 \
  --service-unit-file $U/postfiat-validator-0-rpc.service \
  --environment-file $C/validator-0.rpc.env \
  --validator-bindings-file $S/validator-bindings.signing.json \
  --topology-file $C/topology.json --swap-circuit-metadata-file $C/swap.metadata.json \
  --private-egress-circuit-metadata-file $C/private-egress.metadata.json \
  --publisher-key-file ~/.postfiat/deployments/fastpay-committee-20260925-r4/keys/deployment-publisher.key.json \
  --manifest-file $C/deployment-manifest.json
python3 $PACKET/local-preflight.py --stage $S --require-signed   # runs deployment-manifest-verify x6
```

## 4–5. Preflight and signed canary backup

```bash
scripts/postfiat-safe-rollout preflight --stage-report $S/stage-report.json \
  --inventory-file $PACKET/inventory.txt \
  --vultr-api-key-file ~/.postfiat/deployments/cobalt-activation-8694b99d/vultr-api-key \
  --state-file $E/rollout-state.json --canary-validator-id validator-1 \
  --rpc-tunnel-base-port 27650 > $E/preflight.json
K=~/.postfiat/deployments/cobalt-activation-8694b99d/snapshot-keys
scripts/postfiat-safe-rollout backup --state-file $E/rollout-state.json \
  --evidence-dir $E/pre-rollout-backup \
  --snapshot-publisher-key-file $K/snapshot-publisher.private.json \
  --snapshot-publisher-public-key-file $K/snapshot-publisher.public.json > $E/backup.json
```

## 6. Gate, then one validator at a time

Gate: `~/.cache/release-repair-20260925/logs/full-workspace-tests.log` must end
with the cargo summary and have no failure outside the known flaky family. Not
met on 2026-09-25 ([gate record](observed/workspace-test-gate.json)).

To resume after the gate passes:

- **Chain still at 1044:** `apply-next` with the existing rollout state under
  `$D` (`$E/rollout-state.json`). It reads `inventory.txt` from the worktree
  `~/repos/postfiatl1v2-combined-fastpay-deploy` and checks its SHA-256
  (`6c11341d…`); recreate that worktree from branch
  `release/combined-fastpay-20260925` first.
- **Chain has moved:** fresh before read (§1), preflight and backup (§4–5) first.

After the canary apply, its check needs the canary to certify a new block; the
chain is idle at 1044, so send one devnet transaction. Then, one at a time, each
only after the previous check passed:

```bash
scripts/postfiat-safe-rollout apply-next --state-file $E/rollout-state.json > $E/apply-N.json
python3 $PACKET/observe-fleet.py --stage $S --applied validator-1[,validator-0,...] \
  --output $PACKET/observed/after-<validator>.json
```

## Rollback (one validator, emergency only)

`safe-rollout` has no rollback subcommand and makes **no per-host data
anchor**; each host keeps the r4 executable and r4 release directory. The r4
units are in the local r4 stage. [rollback-one.sh](rollback-one.sh) stops the
validator, checks r4 against the data in place at the given identity, reinstalls
the r4 units, verifies the r4 signed manifest and restarts:

```bash
scp ~/.postfiat/deployments/fastpay-committee-20260925-r4/stage/rootfs/etc/systemd/system/postfiat-validator-N{,-rpc}.service \
  root@HOST:/etc/postfiat/releases/fastpay-committee-20260925-r4/
ssh root@HOST bash -s -- validator-N HEIGHT TIP ROOT < $PACKET/rollback-one.sh
```

If r4 cannot verify the data in place, the script stops before installing r4;
the remaining fallback is the signed validator-1 backup at height 1044
(`$E/pre-rollout-backup/backup-signed`).
validator-1 has about 3.3 GB free.

The backup step left an unsigned snapshot (221 MB,
`/var/lib/postfiat/pre-rollout-snapshots/combined-fastpay-20260925-validator-1-finalized-checkpoint`)
and a copy of the new executable on validator-1. Nothing was deleted.
