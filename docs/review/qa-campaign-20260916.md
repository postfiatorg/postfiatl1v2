# QA campaign log — 2026-09-16

This is the progress record for the [burn 5 campaign](qa-campaign-20260916-burn5-brief.md). Work began on clean `main` at `c4303717` after `git pull --rebase origin main`. This task covers A1 only, with a 30-minute time box; no deployment or live activation.

**Status:** A1 findings and repair units complete; closeout pending. A2–A5 and B pending and not started.

## Campaign state

| Order | Surface | Status | P1 | P2 | P3 | Evidence or fixes |
| --- | --- | --- | ---: | ---: | ---: | --- |
| A1 | Mempool proposals | repaired; closeout pending | 0 | 1 | 1 | [Review](mempool-proposals-review-20260916.md); findings `6f2332cf`; MPL-01 repaired in this unit (not consensus-affecting; full Rust suite verdict pending); MPL-02 unfixed |
| A2 | Vote locks and view recovery | pending | — | — | — | Not started |
| A3 | Cobalt handoff and authority | pending | — | — | — | Not started |
| A4 | Storage migration and activation, certified-send index | pending | — | — | — | Not started |
| A5 | Swap and recovery services | pending | — | — | — | Not started |
| B | Defect inventory and TIH gate | pending | — | — | — | Not started |

Current finding totals: **0 P1, 1 P2, 1 P3** (A1 only).

## Mempool proposals review result

The [A1 review](mempool-proposals-review-20260916.md) read all 3,337 lines of `crates/node/src/mempool_proposals.rs` at `c4303717` at the admission, ordering, duplicate/conflict, size/count, fee/nonce, malformed-input and replay boundaries. MPL-01 (P2) is the omitted offer family in sender admission quotas. It is repaired by reusing the complete sender-count helper, with two regressions that failed on the old guard and pass after repair. The repair is **not consensus-affecting**: it changes local admission policy only. **Full Rust suite verdict pending** CI. MPL-02 (P3) is reversed atomic-swap/FastLane priority in latest-ID reporting and remains unfixed. Called implementations outside this file were not reviewed; review limits are in the findings document.

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
- MPL-02 (P3) remains recorded without repair as required. No P1/P2 repair required an excluded file.
- Called implementations outside `mempool_proposals.rs` were not reviewed: storage/concurrency/crash behavior, cryptographic verification, cross-family execution semantics and archived replay remain unverified. Excluded test/type files were not opened for review or edited; compiler diagnostics supplied fixture field names and focused tests compiled dependencies normally.
- No full workspace or long Orchard/Halo2 suite was run: local admission accounting does not change those boundaries; the full Rust suite is CI's verdict.

## Verification

- `git pull --rebase origin main`: passed; already up to date at task start.
- Findings commit `6f2332cf`: `.venv-docs/bin/mkdocs build --strict`, `scripts/public-doc-links` (400 files), and `scripts/public-secret-scan` passed before commit, including a scan after the new documents were staged. No Rust tests ran in that unit.
- All Cargo commands below used `CARGO_NET_OFFLINE=true` to prevent dependency network access.
- `cargo test -p postfiat-node mempool_proposals::burn5_tests --lib --locked`: before repair, 0 passed, 2 failed at the missing quota checks. Four earlier fixture-compilation attempts executed no tests; constructor-field corrections stayed in the new in-file fixtures.
- `cargo check -p postfiat-node --locked`: passed.
- `cargo test -p postfiat-node mempool --lib --locked`: 16 passed, 0 failed, 0 ignored, 345 filtered out, including both new regressions.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- The same three gates are run before the repair unit commit; closeout verification remains pending. No full Rust suite ran locally; **full Rust suite verdict pending** CI.

## Scores

Not run. The inventory scoring gate belongs to B, which is not started.

## Final summary

A1 findings: 0 P1, 1 P2, 1 P3. MPL-01 is repaired and not consensus-affecting; MPL-02 remains recorded without repair. Findings commit: `6f2332cf`; repair is in this unit. A1 closeout remains pending. A2–A5 and B remain pending; no other surface or inventory work started.
