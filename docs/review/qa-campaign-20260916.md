# QA campaign log — 2026-09-16

This is the progress record for the [burn 5 campaign](qa-campaign-20260916-burn5-brief.md). Work began on clean `main` at `c4303717` after `git pull --rebase origin main`. This task covers A1 only, with a 30-minute time box; no deployment or live activation.

**Status:** A1 and A2 closed. Work stops after A2 as requested; A3–A5 and B remain pending and not started. No live activation, Task Node or fleet action. The opening paragraph and Final summary retain the earlier A1 closeout; this status, the table, and the A2 result record current progress.

## Campaign state

| Order | Surface | Status | P1 | P2 | P3 | Evidence or fixes |
| --- | --- | --- | ---: | ---: | ---: | --- |
| A1 | Mempool proposals | done | 0 | 1 | 1 | [Review](mempool-proposals-review-20260916.md); findings `6f2332cf`; repair `1c9f44f1` (not consensus-affecting; full Rust suite verdict pending); MPL-02 recorded without repair |
| A2 | Vote locks and view recovery | done | 1 | 1 | 0 | [Review](vote-locks-review-20260916.md); findings `1511ea09`; repair `090bd17e` (VLK-01 and VLK-02 both consensus-affecting; full Rust suite verdict pending); all P1/P2 repaired |
| A3 | Cobalt handoff and authority | pending | — | — | — | Not started |
| A4 | Storage migration and activation, certified-send index | pending | — | — | — | Not started |
| A5 | Swap and recovery services | pending | — | — | — | Not started |
| B | Defect inventory and TIH gate | pending | — | — | — | Not started |

Current finding totals: **1 P1, 2 P2, 1 P3** (A1 and A2).

## Mempool proposals review result

The [A1 review](mempool-proposals-review-20260916.md) read all 3,337 lines of `crates/node/src/mempool_proposals.rs` at `c4303717` at the admission, ordering, duplicate/conflict, size/count, fee/nonce, malformed-input and replay boundaries. MPL-01 (P2) is the omitted offer family in sender admission quotas. Repair `1c9f44f1` reuses the complete sender-count helper, with two regressions that failed on the old guard and pass after repair. All 16 selected mempool library tests passed. The only source file edited was `crates/node/src/mempool_proposals.rs`, including its new in-file regressions. The repair is **not consensus-affecting**: it changes local admission policy only. **Full Rust suite verdict pending** CI. MPL-02 (P3) is reversed atomic-swap/FastLane priority in latest-ID reporting and remains unfixed. Called implementations outside this file were not reviewed; review limits are in the findings document.

## Vote locks and view recovery review result

A2 began on clean `main` at `6888b4e1` after `git pull --rebase origin main`, within a 30-minute time box. The [A2 review](vote-locks-review-20260916.md) read all 2,006 lines of the four allowed files: `crates/node/src/vote_locks.rs` (1,371), `finality_view_recovery.rs` (458), `storage_vote_guard.rs` (124), and `node_types_block_vote_timing.rs` (53). The focus covered lock bypass/early release, cross-view equivocation, crash recovery, timing assumptions, and lock persistence/reload. No other source implementation was reviewed as A2.

- **VLK-01 (P1):** canonical lock publication lacked a directory durability barrier. Repair `090bd17e` syncs the lock directory before reservation succeeds, including identical retries after a sync error, while preserving the lock and holding the mutation guard. The regression injects sync failures and checks retained evidence, retry behavior, and conflict rejection after reopening. **Consensus-affecting: yes**, conservatively, because this tightens signer-safety persistence.
- **VLK-02 (P2):** migration silently skipped non-regular `.json` entries before marking completion. Repair `090bd17e` rejects these entries during read-only preflight. The regression covers symlink and directory entries with and without a regular lock alongside them, checking no evidence removal, completion marker, or canonical lock creation. **Consensus-affecting: yes**, conservatively, because this tightens signer admission on ambiguous restored state.

Findings were committed and pushed as `1511ea09` before repair. Both new regressions failed on the original behavior; after repair, 20 focused tests passed with one existing manual test ignored. The only source edited was `crates/node/src/vote_locks.rs`, including in-file regressions. All P1/P2 findings are repaired; there are no P3 findings and no excluded-file repair dependency. Neither repair changes the serialized lock schema or signed/hashed bytes. **Full Rust suite verdict pending** CI for `090bd17e`; no live behavior, deployment, or activation is claimed.

The complete review limits are in the findings document. Called implementations outside the four files were not reviewed: QC/TC verification, actual signing and cross-phase round floors, activation resolution, shared atomic-write internals, backend consistency/state-root calculations, outer RPC limits, and historical replay remain unverified by A2. Files excluded by the brief, previously reviewed surfaces, and frozen artifacts were not reviewed or edited. The release branch and checkout were not touched. This closes A2 only; A3–A5 and B remain pending.

## Cobalt handoff and authority review result

Pending; not reviewed.

## Storage migration and activation, certified-send index review result

Pending; not reviewed.

## Swap and recovery services review result

Pending; not reviewed.

## Skips and boundary decisions

- A3–A5 and B are outside the A2 task and remain pending. No inventory edit or scoring.
- Release-candidate files listed in the brief, excluded crates, previously reviewed surfaces and frozen artifacts are not reviewed or edited. Only remote-ref path metadata is compared to establish exclusions.
- No Task Node or fleet action, release-branch or release-checkout mutation, spend, signup or deployment.
- MPL-02 (P3) remains recorded without repair as required. No P1/P2 repair required an excluded file.
- A1: called implementations outside `mempool_proposals.rs` were not reviewed: storage/concurrency/crash behavior, cryptographic verification, cross-family execution semantics and archived replay remain unverified. Excluded test/type files were not opened for review or edited; compiler diagnostics supplied fixture field names and focused tests compiled dependencies normally.
- A1: no full workspace or long Orchard/Halo2 suite was run; local admission accounting does not change those boundaries, and the full Rust suite is CI's verdict.
- A2: no physical power-loss test or additional storage-guard runtime test was run. Deterministic injected-sync-failure and store-reopen regressions cover the repaired durability boundary; the storage guard was unchanged and its delegated implementations were excluded from review.
- A2: the existing ignored manual release-mode 5,000-lock spot check was not enabled; normal bounded-work and concurrency tests passed. No broad workspace/Orchard run or CI status query was performed; the full Rust suite verdict remains pending. No work stopped for time or usage, and no A2 P1/P2 repair was skipped.

## Verification

**A1 verification (retained):**

- `git pull --rebase origin main`: passed; already up to date at task start.
- Findings commit `6f2332cf`: `.venv-docs/bin/mkdocs build --strict`, `scripts/public-doc-links` (400 files), and `scripts/public-secret-scan` passed before commit, including a scan after the new documents were staged. No Rust tests ran in that unit.
- All Cargo commands below used `CARGO_NET_OFFLINE=true` to prevent dependency network access.
- `cargo test -p postfiat-node mempool_proposals::burn5_tests --lib --locked`: before repair, 0 passed, 2 failed at the missing quota checks. Four earlier fixture-compilation attempts executed no tests; constructor-field corrections stayed in the new in-file fixtures.
- `cargo check -p postfiat-node --locked`: passed.
- `cargo test -p postfiat-node mempool --lib --locked`: 16 passed, 0 failed, 0 ignored, 345 filtered out, including both new regressions.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- Before repair commit `1c9f44f1` and this closeout commit: `.venv-docs/bin/mkdocs build --strict`, `scripts/public-doc-links` (400 files), and `scripts/public-secret-scan` passed. All three gates passed before each of the three A1 unit commits.
- After each unit commit: `git pull --rebase origin main && git push origin main`; pushes only to this repository's `origin main`, with a clean tree between units. The closeout changes only this campaign log; no Rust tests were rerun for it.
- No full Rust suite ran locally; **full Rust suite verdict pending** CI for `1c9f44f1`. No CI verdict is claimed.

**A2 verification:**

- Initial `git pull --rebase origin main`: passed, already up to date at `6888b4e1`; tree clean. All Cargo build/test commands below used `CARGO_NET_OFFLINE=true` to prevent dependency network access.
- Before repair, `cargo test -p postfiat-node vote_locks::tests::burn5_ --lib --locked`: 0 passed, 2 failed, 0 ignored, 361 filtered out. The failure-injection seam and regressions were present; the old behavior still authorized reservation in both failure scenarios.
- `cargo check -p postfiat-node --locked`: passed.
- `cargo test -p postfiat-node vote_locks::tests --lib --locked`: 18 passed, 0 failed, 1 ignored, 344 filtered out; includes both new regressions.
- `cargo test -p postfiat-node finality_view_recovery_tests --bin postfiat-node --locked`: 1 passed, 0 failed, 0 ignored, 169 filtered out.
- `cargo test -p postfiat-node block_vote_timing_tests --lib --locked`: 1 passed, 0 failed, 0 ignored, 362 filtered out.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- Before findings `1511ea09`, repairs `090bd17e`, and this closeout commit: `.venv-docs/bin/mkdocs build --strict`, `scripts/public-doc-links` (401 files), and `scripts/public-secret-scan` passed. The new findings document was staged before its scan.
- Each A2 unit uses its required separate commit, followed by `git pull --rebase origin main && git push origin main`; only this repository's `origin main` receives pushes. Findings and repairs were pushed with a clean tree before starting the next unit. This closeout changes only the campaign log and does not rerun Rust tests.
- A2 total: **20 passed, 0 failed after repair, 1 ignored**. **Full Rust suite verdict pending** CI for `090bd17e`; no CI verdict is claimed.

## Scores

Not run. The inventory scoring gate belongs to B, which is not started.

## Final summary

A1 is closed with **0 P1, 1 P2, 1 P3**: one repaired P2 and one recorded, unfixed P3. Findings commit: `6f2332cf`; repair commit: `1c9f44f1`. **Consensus-affecting repairs: none.** Sixteen focused tests passed; the full Rust suite verdict remains pending CI. Remaining risks are the latest-ID reporting discrepancy and the unreviewed delegated boundaries named above.

Only `crates/node/src/mempool_proposals.rs` was reviewed as source, in full at the A1 focus; no other source file was reviewed. The release branch and its checkout were not mutated. The A1 findings, repair and closeout units are committed and pushed separately to `origin main`, within the 30-minute time box. Work stops here with A2–A5 and B pending; no other surface, inventory, scoring or Task Node work started.
