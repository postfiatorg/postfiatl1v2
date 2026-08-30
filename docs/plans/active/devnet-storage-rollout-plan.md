# Devnet Storage Rollout

**Status:** **STOPPED at step 2 on 2026-08-30 — DO NOT DEPLOY `d0ae79f3`.**
The exact height-924 clone rehearsal rebuilt and independently verified all six
transactional generations, then the first height-925 certified round failed
closed on validator-registry history reapplication. No live deployment occurred.

**The job:** roll the G4-qualified storage build onto the live devnet and prove
the chain — including the already-active Cobalt governance — works on it.

Binary: source `d0ae79f3`, release SHA-256 `9e82d928…8c80c` (G4 PASS
2026-08-30; G1 `8df8f7a6…`, G2 `689a96dc…`). Cobalt has been the registry
authority since height 916; it is not being changed, only proven compatible.

- [x] 1. Read-only fleet probe: six validators, height, deployed binary hash,
      `authority_mode`, registry/trust roots — PASS; all six remained converged
      at height 924 with the deployed `d5e5ef63…c2696caf` binary
- [ ] 2. *(operator go)* Six-clone migration rehearsal against `d0ae79f3`
      (`benchmarks/storage-scaling/run_migration_rehearsal.py`) — **FAIL** after
      six rebuild/verify passes: the first height-925 certified round rejected
      `live validator registry activation previous validator registry root mismatch`
- [ ] 3. *(operator go)* Rolling deploy to all six validators; old binary kept
      on-host as instant rollback; fleet receipt recorded
- [ ] 4. Health on the new storage: six converged, certified rounds finalizing,
      transactional backend active, zero full-history reads
- [ ] 5. Cobalt-on-new-storage check: one real signed registry transition
      commits (all six, one root); one stale/wrong-root transition rejects
      everywhere with no durable mutation; storage telemetry stays bounded
      through the governance rounds
- [ ] 6. Receipts from all six hosts; update `docs/status/chain-state-current.md`;
      short handoff. Z1 clock starts.

Failure anywhere in 3–5: roll back to the retained binary, one diagnosis, stop.
Existing tooling only. Operator authorization points: steps 2 and 3.

## Stop receipt

- Fleet probe: `benchmarks/storage-scaling/devnet-rollout/fleet-probe-20260830.json`
- G6 failure: `benchmarks/storage-scaling/devnet-rollout/g6-failure-20260830.json`
- Post-failure live-fleet receipt:
  `benchmarks/storage-scaling/devnet-rollout/stop-receipt-20260830.json`

The bounded diagnosis is a candidate compatibility defect: with both accepted
validator-registry updates present through height 924, the first new round tries
to reapply superseded registry history and fails its previous-root check. The
working clones remained at height 924. The live fleet was re-probed unchanged;
rollback was unnecessary. Steps 3–5 and the Z1 clock did not start.
