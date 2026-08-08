# A666 Five-Demo Program: repro, concurrency, fixes, verification

- **Created:** 2026-08-09 UTC (principal directive 2026-08-08 ~23:5xZ)
- **Principal directive (verbatim intent):** five end-to-end demos. Batch 1: a single repro of the 2026-08-08 round. Batch 2: three concurrent repros. Then a fix round for every defect and friction found. Final batch: one verification run.
- **Authority:** plan A666-UNIFIED-EXECUTION-PLAN-20260808.md Section 2 standing rulings apply unchanged. The only principal touchpoint is spend above $1,000. Everything else resolves at the manager.
- **Prerequisite:** the prototype round (2026-08-08 campaign) closes A6 first. Its debugged leg tooling (commits 2faf581, 50db9d9, 9d14fdc, f583cf8 lineage) is the baseline for all five demos.

## Demo definition (one round = all of this, with live funds)

USDC deposit -> pfUSDC -> A666 subscribe -> export -> Ethereum mint (checkpoint + proof) -> Uniswap sell -> Uniswap buy-back -> return burn -> PFTL import -> primary redeem -> bridge-out to external USDC. Receipt-by-receipt evidence, two-source verification per leg, exact conservation table at the end. Protected baselines untouchable. Deposit size ~10 USDC per demo (recycled each round; well inside standing spend authority).

## Batches and gates

### Batch 1 — Demo 1: single repro (Gate D1R)
- One executor, fresh order lineage, same route. No code changes except defects found (root-cause, fix, commit, continue).
- Gate D1R: full round closed with conservation exact; wall-clock and per-leg timings recorded (this is the baseline timing table); defect list appended.

### Batch 2 — Demos 2, 3, 4: three concurrent rounds (Gate D3C)
- Three independent order lineages run interleaved. PFTL-side concurrency is real (independent reservations/orders, shared NAV epochs, admission ordering). Ethereum legs share the single custody wallet and therefore serialize on its nonce; interleaving across demos is required, strict global ordering of legs is not.
- One executor process MAY drive all three lineages (single money-path writer preserved) but must interleave legs and record per-demo receipt chains separately. A read-only verifier agent reconciles all three conservation tables independently.
- Gate D3C: three closed rounds, three exact conservation tables, contention/defect notes (nonce serialization waits, admission conflicts, epoch freshness misses).

### Batch 3 — fix round (Gate DFIX)
- Consolidate every defect and friction item from the prototype + batches 1-2 into a numbered list with file:line evidence. Fix each (code, tooling, runbook), commit by exact filename, with a regression check per fix where feasible.
- Gate DFIX: list fully dispositioned (FIXED with commit, or DEFERRED with reason), committed, tracker updated.

### Batch 4 — Demo 5: verification run (Gate DVER)
- One clean round on the fixed tooling, ideally zero stops. Compare wall-clock against the Batch 1 baseline; record the improvement.
- Gate DVER = program done: five closed demos, five exact conservation tables, timing table, dispositioned defect ledger.

## Rules of engagement

- Same discipline as the unified plan: one money-path writer at a time, one attempt per mutation, STOP-no-retry with full reconciliation, fleet/mempool gates on PFTL mutations, never re-run prior transactions, no git add ., secrets by location/class only, disk floor 20 GB.
- Never touch: protected wA666 baseline, migration reserve, frozen worktrees, Track B/C artifacts.
- Every batch appends to docs/status/A666-FIVE-DEMO-TRACKER-20260809.md (created at batch-1 launch) with receipts, timings, and defect entries.
- Relationship to Track D (legacy-lane retirement): unchanged standing directive; B4 rehearsal and D1/D2 migration proceed when the a666 worktree is free between batches or after DVER, whichever the manager schedules. If D2 lands mid-program, later demos run on the successor NAV lane and record that fact — that is a feature, not a deviation.
