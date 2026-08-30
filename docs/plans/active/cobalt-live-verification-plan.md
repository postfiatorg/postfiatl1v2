# Cobalt Live Verification on the Qualified Storage Lineage

**Goal:** the devnet runs the G4-qualified storage build, Cobalt is on, and one
real transition proves it works. One page, one pass.

Binary under deployment: source `d0ae79f3`, release SHA-256 `9e82d928…8c80c`
(G4 PASS 2026-08-30; G1 `8df8f7a6…`, G2 `689a96dc…`).

## Steps

- [ ] 1. Read-only fleet probe: six validators, converged height, deployed
      binary hash, `authority_mode`, registry/trust roots recorded
- [ ] 2. *(operator: authorize six stopped copies)* Six-clone migration
      rehearsal with `benchmarks/storage-scaling/run_migration_rehearsal.py`
      against the `d0ae79f3` binary: activation lane, cancelled lane,
      restart/catch-up/rollback, mixed-version refusal, all-six convergence
- [ ] 3. *(operator: authorize deployment)* Rolling deploy of `9e82d928…8c80c`
      to the six devnet validators with pinned rollback (`8cc7d15e` binary
      retained on-host); fleet receipt binds source, binary, and services
- [ ] 4. Post-deploy convergence check: all six at one height/root lineage,
      certified rounds finalizing, storage telemetry showing transactional
      backend and zero full-history reads
- [ ] 5. Cobalt authority check: `authority_mode = 1` with current configs —
      Cobalt remains the registry authority across the upgrade, no governance
      config changes
- [ ] 6. One real signed registry transition through the full path (proposal →
      old-rule ML-DSA quorum → Cobalt check → commit); all six accept one root
- [ ] 7. Negative check: one stale/wrong-root transition rejects on every
      validator with a named reason and no durable mutation
- [ ] 8. Converged receipts from all six hosts; update
      `docs/status/chain-state-current.md`; short handoff. **Gate Zero Z1
      window starts here** — Cobalt live on the qualified lineage

## Rules

Rollback to `8cc7d15e` is the abort path at any step 3–7 failure; one
diagnosis, no retry-blind loops. Steps 2–3 are the only new authority asks.
Existing tooling only: rehearsal harness, E5 drill machinery
(`crates/node/src/cobalt_e5_live_drill.rs`), fleet observation runbook.
