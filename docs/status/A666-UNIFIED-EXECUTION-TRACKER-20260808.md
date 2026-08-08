# A666 Unified Execution Tracker

- **Plan:** ../plans/A666-UNIFIED-EXECUTION-PLAN-20260808.md
- **Started:** 2026-08-08 UTC
- **Principal GO:** received 2026-08-08 ("get this done") — Section 2 envelope active
- **Manager:** Codex session, postfiatfoundationv2

## Assignment table

| Agent | Role | Worktree(s) | Status |
|---|---|---|---|
| Manager | orchestration, docs, evidence commits | pftl-validation-20260807 (docs only) | ACTIVE |
| Executor-A | live loop | per runbook | UNASSIGNED |
| Verifier-A | read-only verification | n/a | UNASSIGNED |
| Prover-B | Groth16 env + proves | a666-eth-fast-lane-combined-20260724 | UNASSIGNED |
| Qualifier-B pool | per-source qualification | a666-eth-fast-lane-combined-20260724 | UNASSIGNED |
| Surveyor-C pool | read-only inventory | in-scope worktrees | UNASSIGNED |
| Integrator-C | PR series | canonical checkouts | UNASSIGNED |

## Gate states

| Gate | State | Evidence |
|---|---|---|
| Docs publication (Step 0) | OPEN | |
| A0 preflight | OPEN | |
| A1 NEAR fix | OPEN | |
| A2 reader session | OPEN | |
| A3 E6 proof | OPEN | |
| A4 E6 finalize | OPEN | |
| A5 legs 2a-5b | OPEN | |
| A6 closeout | OPEN | |
| B1 Groth16 env | OPEN | |
| B2 epoch 7/8 proofs | OPEN | |
| B3 qualification 6/6 | OPEN | |
| B4 G5 rehearsal | OPEN | |
| C0 inventory | OPEN | |
| C1 target architecture | OPEN | |
| C3 integration complete | OPEN | |
| C4 retirement complete | OPEN | |
| D1 migration packet | OPEN | |
| D2 live migration (PRINCIPAL GO) | OPEN | |
| D3 legacy lane retired | OPEN | |

## STOP log

(append-only)

## Stage state journal

(append-only; every agent writes before/after each gate)

- 2026-08-08: Manager: plan committed to docs/plans/, tracker created, fire-discipline skill read and bound to Track A. Envelope active per principal GO.
