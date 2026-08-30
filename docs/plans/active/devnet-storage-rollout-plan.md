# Devnet Storage Rollout

**The job:** roll the G4-qualified storage build onto the live devnet and prove
the chain — including the already-active Cobalt governance — works on it.

Binary: source `d0ae79f3`, release SHA-256 `9e82d928…8c80c` (G4 PASS
2026-08-30; G1 `8df8f7a6…`, G2 `689a96dc…`). Cobalt has been the registry
authority since height 916; it is not being changed, only proven compatible.

- [ ] 1. Read-only fleet probe: six validators, height, deployed binary hash,
      `authority_mode`, registry/trust roots — ground truth first
- [ ] 2. *(operator go)* Six-clone migration rehearsal against `d0ae79f3`
      (`benchmarks/storage-scaling/run_migration_rehearsal.py`): activation,
      cancellation, restart/catch-up/rollback, mixed-version refusal,
      all-six convergence
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
