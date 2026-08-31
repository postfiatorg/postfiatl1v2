# Storage single-writer gate PASSED; production rollout staged, not started

- **Operator:** PostFiatChad (`postfiatchad`)
- **Date:** 2026-08-31 UTC
- **Plan:** [single-writer deployment plan](../plans/active/devnet-storage-single-writer-deployment-plan.md) (locked, TIH 88.60/100)

## BLUF

The two-process database lock conflict that killed the 2026-08-30 canary is
fixed and proven. A bounded writer lease (`f0013c29`) lets
`transport-validator-serve` and `rpc-serve` share the transactional `redb`
store safely; a new **deployment-exact gate ran on the real validator hosts**
— six exact height-926 clones over the live WireGuard mesh, production-parity
systemd units, signed manifest, both start orders — and passed end to end:
migration, RPC finality rounds, governance-certified activation, transactional
finality with zero full-history reads, restart, and exact rollback. The
**live fleet is untouched** at height 926 on `registry-fix-291d1eb1`. The
production release is signed and staged; the rollout driver is written; no
production step has run.

## What passed (gate receipt: `benchmarks/storage-scaling/deployment-exact-gate/gate-926-receipt.json`)

- Writer lease: weak-registry acquire/release with retry and fail-closed
  `storage_writer_busy` (`crates/storage/src/transactional.rs`); 88 storage
  tests, 5 lease tests, focused node tests all green.
- Both services ready on all six clones in **both start orders** under the
  real sandbox — the exact prior failure condition.
- Chain sequence on clones: legacy round 927 → activation scheduled 928 →
  pre-activation 929 → **cutover 930** → transactional round 931; all six
  converged, `postfiat.replicated_state.v2`, `full_history_scans=0`.
- CLI certified round ran **while services were up** (lease coexistence).
- Rollback: mutated clone restored to the exact height-926 identity and
  served by the currently deployed binary.

## The gate caught real defects (which was its job)

1. **Generation directory must live inside the data directory** or
   `ProtectSystem=strict` kills the service — the exact canary failure class.
   Corrected layout: `<data-dir>/transactional-generation`.
2. Failed round attempts leave stale vote locks and durable consensus-v2
   round state; recovery is restore-and-remigrate (documented).
3. Deferred certified sends need `transport-certified-send-outbox-resume`
   for six-way propagation after RPC finality rounds.
4. `rpc-serve` status caches go stale across the cutover; the rollout must
   restart services post-activation (already in the sequence).

## Production rollout — staged and ready, awaiting go

- Signed release `storage-lease-af9b83c3` (binary `383f4325…141a7a`,
  git `af9b83c3`, same publisher key the fleet trusts) staged at
  `~/.postfiat/deployments/storage-lease-af9b83c3`.
- Driver: `~/.postfiat/deployments/storage-lease-af9b83c3/rollout/deploy.py`
  with `probe` → `canary` (validator-1: unit backup, raw data-dir copy,
  manifest verify, offline rebuild+verify, restart, identity check) →
  `fleet` (remaining five in parallel), then the gate-proven sequence:
  legacy liveness round, refreeze, activation, cutover, resume outbox,
  post-activation restart, final transactional rounds, receipts, Z1 clock.
- Rollback surface per host: unit backups under
  `/root/postfiat-deploy-backups/storage-lease-af9b83c3/`, full raw
  pre-rollout data-dir copy, old release directories untouched.

## Cleanups owed

- Remove gate ufw rule (`26750:26760` on wg0) on all six hosts.
- Remove `/var/lib/postfiat/gate926/` (~8 GB/host) after receipts are
  archived; gate transient units already removed.
- Commit `benchmarks/storage-scaling/deployment-exact-gate/` receipt and
  this handoff.

## Next decision or action

Run `deploy.py probe canary`, verify the canary, then `fleet`, then the
activation sequence. Estimated 60-90 minutes of operator time. Everything
up to the first production mutation is done.
