# QA campaign log — 2026-09-10

This is the canonical progress record for the [2026-09-10 QA campaign](qa-campaign-20260910-brief.md). The campaign began at 2026-09-10T09:31:59Z from `main` commit `3ec59c0bea6383445d1465d9a006317a33973003`. It is a review-and-repair campaign, not a release or deployment authorization.

## Current state

| Order | Surface | Status | P1 | P2 | P3 | Evidence or fix commits |
| --- | --- | --- | ---: | ---: | ---: | --- |
| A1 | Arc-facing code | reviewing | 0 | 0 | 0 | Pending |
| D | Bounded whitepaper corrections | pending | — | — | — | Pending |
| B1 | Initial defect inventory | pending | — | — | — | Pending |
| C | Validator-0 RPC diagnosis | pending | — | — | — | Pending |
| A2 | Consensus and storage | pending | 0 | 0 | 0 | Pending |
| A3 | Wallet, proxy, and RPC SDK | pending | 0 | 0 | 0 | Pending |
| A4 | StakeHub `fix/pr8-safety-20260907` | pending | 0 | 0 | 0 | Read-only review only |
| A5 | Task Node UNL V1 and V2 modules | pending | 0 | 0 | 0 | Pending |
| B2 | Final defect inventory and TIH gate | pending | — | — | — | Pending |

No finding has yet been dispositioned. No score has yet been recorded.

## Completed units

None.

## Skips and boundary decisions

- No Task Node action is permitted or planned.
- No fleet mutation, deployment, restart, configuration change, host write, or live-chain write is permitted.
- The validator-0 investigation remains pending and will use only the documented read-only status, log, and socket procedure.
- Frozen simulations, V2 gate outputs, deployment evidence, `docs/whitepaper_legacy.md`, the locked amendment, and lock records will not be modified.
- `docs/whitepaper.md` will remain unchanged unless the bounded candidate strictly exceeds the recorded 87.13 score under the specified gate.

## Verification

The strict documentation build is required before every campaign commit. Focused tests and review evidence will be linked from each surface's findings document as work is completed.
