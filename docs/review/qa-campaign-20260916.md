# QA campaign log — 2026-09-16

This is the progress record for the [burn 5 campaign](qa-campaign-20260916-burn5-brief.md). Work began on clean `main` at `c4303717` after `git pull --rebase origin main`. This task covers A1 only, with a 30-minute time box; no deployment or live activation.

**Status:** A1 in progress. A2–A5 and B pending and not started.

## Campaign state

| Order | Surface | Status | P1 | P2 | P3 | Evidence or fixes |
| --- | --- | --- | ---: | ---: | ---: | --- |
| A1 | Mempool proposals | findings recorded; repair pending | 0 | 1 | 1 | [Review](mempool-proposals-review-20260916.md); MPL-01 repair pending; MPL-02 recorded without repair |
| A2 | Vote locks and view recovery | pending | — | — | — | Not started |
| A3 | Cobalt handoff and authority | pending | — | — | — | Not started |
| A4 | Storage migration and activation, certified-send index | pending | — | — | — | Not started |
| A5 | Swap and recovery services | pending | — | — | — | Not started |
| B | Defect inventory and TIH gate | pending | — | — | — | Not started |

Current finding totals: **0 P1, 1 P2, 1 P3** (A1 only).

## Mempool proposals review result

The [A1 review](mempool-proposals-review-20260916.md) read all 3,337 lines of `crates/node/src/mempool_proposals.rs` at `c4303717` at the admission, ordering, duplicate/conflict, size/count, fee/nonce, malformed-input and replay boundaries. MPL-01 (P2) is the omitted offer family in sender admission quotas; its local admission repair is pending and not consensus-affecting. MPL-02 (P3) is reversed atomic-swap/FastLane priority in latest-ID reporting and remains unfixed. Called implementations outside this file were not reviewed; review limits are in the findings document.

## Vote locks and view recovery review result

Pending; not reviewed.

## Cobalt handoff and authority review result

Pending; not reviewed.

## Storage migration and activation, certified-send index review result

Pending; not reviewed.

## Swap and recovery services review result

Pending; not reviewed.

## Skips and boundary decisions

- A2–A5 and B are outside this task and remain pending. No inventory edit or scoring.
- Release-candidate files listed in the brief, excluded crates, previously reviewed surfaces and frozen artifacts are not reviewed or edited. Only remote-ref path metadata is compared to establish exclusions.
- No Task Node or fleet action, release-branch or release-checkout mutation, spend, signup or deployment.

## Verification

- `git pull --rebase origin main`: passed; already up to date at task start.
- Findings unit: no Rust tests run; source inspection only. The three required commit gates are run before committing this unit.

## Scores

Not run. The inventory scoring gate belongs to B, which is not started.

## Final summary

A1 findings: 0 P1, 1 P2, 1 P3. MPL-01 repair and A1 closeout remain pending; MPL-02 is recorded without repair. A2–A5 and B remain pending.
