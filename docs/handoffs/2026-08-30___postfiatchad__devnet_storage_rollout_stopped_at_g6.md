# Devnet storage rollout stopped at G6

Date: 2026-08-30

## Result

**STOP — `d0ae79f3` is not clone-qualified and was not deployed.**

The preflight fleet probe found all six validators converged at height 924 with
empty mempools, identical deployed binary `d5e5ef63…c2696caf`, Cobalt authority
mode 1, identical registry/trust roots, and active validator/RPC/shadow services.

The exact height-924 G6 run then:

- authenticated six distinct stopped validator clones;
- rebuilt six transactional generations and passed six independent verify-only
  checks with packet root `6a6b53ea…e4807d5`; and
- failed closed on the first certified height-925 transfer round because
  validator-1 rejected all three deliveries with
  `live validator registry activation previous validator registry root mismatch`.

The clones remained at height 924. No live binary, service, data directory,
storage mode, governance state, or Cobalt authority was changed. A post-failure
probe confirmed the live fleet was unchanged, so rollback was unnecessary and
Z1 did not start.

## Evidence

- `benchmarks/storage-scaling/devnet-rollout/fleet-probe-20260830.json`
- `benchmarks/storage-scaling/devnet-rollout/g6-failure-20260830.json`
- `benchmarks/storage-scaling/devnet-rollout/stop-receipt-20260830.json`
- `docs/plans/active/devnet-storage-rollout-plan.md`
- `docs/status/chain-state-current.md`

Raw clones, signer material, logs, and the partially completed rehearsal remain
private under `~/.postfiat/deployments/storage-rollout-d0ae79f3/` and must not be
committed or published.

## Next gate

Repair the owning validator-registry continuation logic so an already-applied,
superseded update is not treated as due again. Add an exact two-update
height-924-to-925 regression, then repeat every invalidated candidate
qualification gate and G6 under a new source/binary identity. A new written
deployment decision is required after that pass.
