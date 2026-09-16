# QA campaign log — 2026-09-16

This is the progress record for the [burn 5 campaign](qa-campaign-20260916-burn5-brief.md). Work began on clean `main` at `c4303717` after `git pull --rebase origin main`. This task covers A1 only, with a 30-minute time box; no deployment or live activation.

**Status:** A1, A2, A3 and A4 closed. Work stops after A4 as requested; A5 and B remain pending and not started. A4 leaves SMG-07 unfixed because its repair depends on a file outside A4. No live activation, Task Node or fleet action. The opening paragraph, earlier surface results and Final summary retain historical closeouts; this status, the table and the A4 result record current progress.

## Campaign state

| Order | Surface | Status | P1 | P2 | P3 | Evidence or fixes |
| --- | --- | --- | ---: | ---: | ---: | --- |
| A1 | Mempool proposals | done | 0 | 1 | 1 | [Review](mempool-proposals-review-20260916.md); findings `6f2332cf`; repair `1c9f44f1` (not consensus-affecting; full Rust suite verdict pending); MPL-02 recorded without repair |
| A2 | Vote locks and view recovery | done | 1 | 1 | 0 | [Review](vote-locks-review-20260916.md); findings `1511ea09`; repair `090bd17e` (VLK-01 and VLK-02 both consensus-affecting; full Rust suite verdict pending); all P1/P2 repaired |
| A3 | Cobalt handoff and authority | done | 0 | 4 | 2 | [Review](cobalt-handoff-review-20260916.md); findings `34611d66`; repair `21cf30f8` (CHO-01/02 consensus-affecting, CHO-03/04 not consensus-affecting; full Rust suite verdict pending); CHO-05/06 recorded without repair |
| A4 | Storage migration and activation, certified-send index | done; one repair blocked by scope | 0 | 5 | 2 | [Review](storage-migration-review-20260916.md); findings `307214ef`, correction/dependency `514f7e15`; repair `401fa055` (SMG-01–04 not consensus-affecting; full Rust suite verdict pending); SMG-07 depends on out-of-scope `transport_cli.rs`; SMG-05/06 recorded without repair |
| A5 | Swap and recovery services | pending | — | — | — | Not started |
| B | Defect inventory and TIH gate | pending | — | — | — | Not started |

Current finding totals: **1 P1, 11 P2, 5 P3** (A1, A2, A3 and A4).

## Mempool proposals review result

The [A1 review](mempool-proposals-review-20260916.md) read all 3,337 lines of `crates/node/src/mempool_proposals.rs` at `c4303717` at the admission, ordering, duplicate/conflict, size/count, fee/nonce, malformed-input and replay boundaries. MPL-01 (P2) is the omitted offer family in sender admission quotas. Repair `1c9f44f1` reuses the complete sender-count helper, with two regressions that failed on the old guard and pass after repair. All 16 selected mempool library tests passed. The only source file edited was `crates/node/src/mempool_proposals.rs`, including its new in-file regressions. The repair is **not consensus-affecting**: it changes local admission policy only. **Full Rust suite verdict pending** CI. MPL-02 (P3) is reversed atomic-swap/FastLane priority in latest-ID reporting and remains unfixed. Called implementations outside this file were not reviewed; review limits are in the findings document.

## Vote locks and view recovery review result

A2 began on clean `main` at `6888b4e1` after `git pull --rebase origin main`, within a 30-minute time box. The [A2 review](vote-locks-review-20260916.md) read all 2,006 lines of the four allowed files: `crates/node/src/vote_locks.rs` (1,371), `finality_view_recovery.rs` (458), `storage_vote_guard.rs` (124), and `node_types_block_vote_timing.rs` (53). The focus covered lock bypass/early release, cross-view equivocation, crash recovery, timing assumptions, and lock persistence/reload. No other source implementation was reviewed as A2.

- **VLK-01 (P1):** canonical lock publication lacked a directory durability barrier. Repair `090bd17e` syncs the lock directory before reservation succeeds, including identical retries after a sync error, while preserving the lock and holding the mutation guard. The regression injects sync failures and checks retained evidence, retry behavior, and conflict rejection after reopening. **Consensus-affecting: yes**, conservatively, because this tightens signer-safety persistence.
- **VLK-02 (P2):** migration silently skipped non-regular `.json` entries before marking completion. Repair `090bd17e` rejects these entries during read-only preflight. The regression covers symlink and directory entries with and without a regular lock alongside them, checking no evidence removal, completion marker, or canonical lock creation. **Consensus-affecting: yes**, conservatively, because this tightens signer admission on ambiguous restored state.

Findings were committed and pushed as `1511ea09` before repair. Both new regressions failed on the original behavior; after repair, 20 focused tests passed with one existing manual test ignored. The only source edited was `crates/node/src/vote_locks.rs`, including in-file regressions. All P1/P2 findings are repaired; there are no P3 findings and no excluded-file repair dependency. Neither repair changes the serialized lock schema or signed/hashed bytes. **Full Rust suite verdict pending** CI for `090bd17e`; no live behavior, deployment, or activation is claimed.

The complete review limits are in the findings document. Called implementations outside the four files were not reviewed: QC/TC verification, actual signing and cross-phase round floors, activation resolution, shared atomic-write internals, backend consistency/state-root calculations, outer RPC limits, and historical replay remain unverified by A2. Files excluded by the brief, previously reviewed surfaces, and frozen artifacts were not reviewed or edited. The release branch and checkout were not touched. This closes A2 only; A3–A5 and B remain pending.

## Cobalt handoff and authority review result

A3 began on clean `main` at `204fd33f` after `git pull --rebase origin main`, with a 30-minute time box beginning at approximately 11:55 UTC. The [A3 review](cobalt-handoff-review-20260916.md) read all 4,931 lines of the four allowed source files: `crates/node/src/cobalt_handoff.rs` (2,044), `cobalt_authority_certificate.rs` (910), `cobalt_shadow_runtime.rs` (643), and `cobalt_handoff_rehearsal.rs` (1,334), including their in-file tests. The focus covered certificate/quorum requirements, replay, rehearsal/live separation, shadow authority, and malformed certificates. No other source implementation was reviewed as A3.

- **CHO-01 (P2):** valid older full-knowledge checkpoints were accepted for a newer decision. Repair `21cf30f8` binds checkpoint interval/coverage and signed check height/pending pair to the current ratification. **Consensus-affecting: yes**, because authority-certificate admission is tightened.
- **CHO-02 (P2):** compact-certificate shared checks were duplicated without bounding their expanded product. The same repair bounds serialized expansion to 16 MiB before cloning, with checked arithmetic and a matching compressor guard. **Consensus-affecting: yes**, because the authority verifier rejects over-bound expansion.
- **CHO-03 (P2):** response-write failures stopped the shadow listener. The repair contains them within the connection; an in-memory regression covers malformed and ordinary requests, disabled shutdown and a subsequent successful probe. **Consensus-affecting: no**.
- **CHO-04 (P2):** valid update finalization panicked on the correct Foundation route for unrelated governance. The rehearsal now checks mixed-batch rejection, returns an ordinary error on unexpected success and labels that evidence accurately. **Consensus-affecting: no**.
- **CHO-05 (P3):** negative rehearsal probes discard their signed approvals, weakening the reported rejection evidence. Recorded without repair.
- **CHO-06 (P3):** manifest digest checks accept lowercase letters outside hexadecimal. Recorded without repair.

Findings were committed and pushed as `34611d66` before repair. All four P2 findings have a regression that reproduced the original defect; the expansion fixture required a canonical-encoding correction before its reproduction. All **17 focused post-repair tests passed**, including the 20-validator certificate and full update-finalization function. Repair `21cf30f8` changes only the four A3 source files and the findings document; `cobalt_handoff.rs` has test additions only. No P1/P2 repair was skipped or required an excluded file. Certificate formats and signed/hashed encodings remain unchanged. **Full Rust suite verdict pending** CI for `21cf30f8`; no deployment or live activation is claimed.

The complete review limits are in the findings document. Delegated shadow-service, cryptographic/protocol, historical registry/replay, storage, execution, governance admission, state-commitment, RPC and command-wrapper implementations were not reviewed. Rehearsal report consumers outside A3 were not tested; the result key is now `mixed_authority_batch_rejected`. Release-candidate source files, previously reviewed surfaces, excluded crates and frozen artifacts were not reviewed or edited. No work stopped for time or usage. This closes A3 only; A4, A5 and B remain pending.

## Storage migration and activation, certified-send index review result

A4 began on clean `main` at `41bd85ff` after `git pull --rebase origin main`, at approximately 12:16 UTC with a 30-minute time box. The [A4 review](storage-migration-review-20260916.md) read all 3,860 lines of the four allowed files: `crates/node/src/storage_migration.rs` (1,005), `storage_activation_cli.rs` (378), `storage_backend_config.rs` (143), and `certified_send_completed_index.rs` (2,334), including its in-file tests. Focus: interrupted migration, activation preconditions, backend selection, index crash consistency and growth bounds. No other source implementation was reviewed as A4.

- **SMG-01 (P2):** activation/cancellation output used a check-then-replace sequence. Repair `401fa055` publishes synced JSON with an atomic no-replace hard link and directory sync. Its regression preserves a competing artifact and dangling symlink, verifies exact successful JSON output and temporary-file cleanup. **Consensus-affecting: no.**
- **SMG-02 (P2):** append recovery skipped move-directory sync when a previous rename was already visible. The repair retries both directory barriers before index publication. Its regression injects repeated failures at each directory, checks retained intent and unchanged index, then successfully recovers. **Consensus-affecting: no.**
- **SMG-03 (P2):** pending-intent recovery could adopt unrelated directory additions/deletions into its fresh stamp. The repair checks bounded directory membership before index publication; its regression covers both discrepancies and preserves recovery evidence. **Consensus-affecting: no.**
- **SMG-04 (P2):** backend post-validation failure left the candidate mode selected. The repair restores the previous mode after selection/post-check errors and explicitly reports restoration failure. Its regression checks the failed candidate selection, restored mode on reopen and a later successful selection. **Consensus-affecting: no.**
- **SMG-05 (P3):** manifest/checksum reads are unbounded, and index metadata checks precede a separate unbounded read. Recorded without repair.
- **SMG-06 (P3):** bare relative migration output can fail disk-space preflight; the parent can also represent a different filesystem from an existing mounted output. Recorded without repair.
- **SMG-07 (P2):** interrupted prune recovery rejects the already moved retention payload as non-canonical. **Unfixed dependency:** `crates/node/src/transport_cli.rs` owns `read_validated_durable_certified_send_payloads` and is outside A4. Its implementation was not reviewed or edited; only a filename-only symbol lookup located it. The conditional prune sync path also remains unqualified behind this dependency.

Findings were pushed as `307214ef`. Regression exploration exposed SMG-07 and corrected the initial SMG-02 prune claim; that evidence was pushed separately as `514f7e15` before fixes. Partial regression work was stashed during the correction, then restored with the stash removed. Repair `401fa055` fixes four P2 findings in three source files: `storage_activation_cli.rs`, `storage_backend_config.rs` and `certified_send_completed_index.rs`, with four in-file regressions. `storage_migration.rs` was reviewed but not edited. All four repaired defects reproduced before repair. **23 focused tests passed**, with two existing manual checks ignored. **Full Rust suite verdict pending** CI for `401fa055`; no deployment or live activation is claimed.

The findings document gives the full review limits. Delegated storage/atomic-write internals, journal recovery, receipt exceptions, replay, state commitments, governance authorization/readiness, certified-send payload/quarantine helpers, RPC and command dispatch were not reviewed. Release-candidate source files, prior burns' source surfaces, excluded crates and frozen artifacts were not reviewed or edited. No time or usage limit curtailed the four-file review. A4 closes with one P2 dependency and two P3 findings explicitly unfixed; A5 and B remain pending.

## Swap and recovery services review result

Pending; not reviewed.

## Skips and boundary decisions

- A5 and B are outside the A4 task and remain pending. No inventory edit or scoring.
- Release-candidate files listed in the brief, excluded crates, previously reviewed surfaces and frozen artifacts are not reviewed or edited. Only remote-ref path metadata is compared to establish exclusions.
- No Task Node or fleet action, release-branch or release-checkout mutation, spend, signup or deployment.
- MPL-02 (P3) remains recorded without repair as required. No P1/P2 repair required an excluded file.
- A1: called implementations outside `mempool_proposals.rs` were not reviewed: storage/concurrency/crash behavior, cryptographic verification, cross-family execution semantics and archived replay remain unverified. Excluded test/type files were not opened for review or edited; compiler diagnostics supplied fixture field names and focused tests compiled dependencies normally.
- A1: no full workspace or long Orchard/Halo2 suite was run; local admission accounting does not change those boundaries, and the full Rust suite is CI's verdict.
- A2: no physical power-loss test or additional storage-guard runtime test was run. Deterministic injected-sync-failure and store-reopen regressions cover the repaired durability boundary; the storage guard was unchanged and its delegated implementations were excluded from review.
- A2: the existing ignored manual release-mode 5,000-lock spot check was not enabled; normal bounded-work and concurrency tests passed. No broad workspace/Orchard run or CI status query was performed; the full Rust suite verdict remains pending. No work stopped for time or usage, and no A2 P1/P2 repair was skipped.

- A3: CHO-05 and CHO-06 are P3 findings and remain unfixed as required. No P1/P2 repair was skipped. No Task Node, fleet, release-branch mutation, deployment or activation occurred.
- A3: the existing local socket drill was explicitly skipped because network use is limited to git and permitted scoring. No sockets or validator hosts were contacted by the A3 tests; the listener regression uses in-memory streams. No broad Rust/Orchard run, physical fault test or CI status query was performed.
- A3: a broad instruction-filename search returned the release checkout's `AGENTS.md` path despite the intended exclusion; its contents were not opened and the checkout was not mutated. All source review and edits stayed in the four allowed files. Remote-ref comparisons read filenames only.
- A3: an initial `burn5_` test filter also reran four existing A1/A2 regressions. They passed; no A1/A2 source was re-reviewed or edited and no additional surface was started. Later filters were restricted to the A3 modules.

- A4: SMG-07 is recorded without repair because its shared retention-path resolver is in `crates/node/src/transport_cli.rs`, outside the allowed four files. The exploratory prune test hit this refusal before the intended sync assertion; it was removed after the failure and dependency were recorded. No shared resolver or excluded test file was reviewed or edited.
- A4: SMG-05/06 remain unfixed P3 findings. The two existing ignored index release-mode performance spot checks were not enabled. No physical power-loss experiment, historical/Orchard replay, broad Rust suite, network/socket exercise or CI status query ran. The changed operator-output, configuration-rollback and completed-index boundaries do not change shielded execution or historical replay; the full Rust suite remains CI's verdict.
- A4: no Task Node, fleet action, release-branch/checkout mutation, deployment, activation or other surface work occurred. Remote-ref comparisons and the one delegated-symbol lookup returned filenames only. No work stopped for time or usage.

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

**A3 verification:**

- Initial `git pull --rebase origin main`: passed, already up to date at `204fd33f`; tree clean. All Cargo build/test commands below used `CARGO_NET_OFFLINE=true`.
- Findings exploration: `cargo test -p postfiat-node burn5_cobalt_authority_rejects_replayed_full_knowledge_checkpoint --lib --locked -- --nocapture` ran twice successfully through compilation, each with 0 passed, 1 failed, 363 filtered out: the verifier returned a decision using the older checkpoints. One preceding compile attempt failed on a missing `std::fs` qualification and executed no tests. The exploratory source edit was removed before the findings-only commit.
- Before repair, `cargo test -p postfiat-node burn5_ --lib --locked`: 4 passed, 4 failed, 0 ignored, 359 filtered out. Three A3 failures reproduced CHO-01/03/04; the expansion fixture initially failed canonical decoding. The four passes were existing A1/A2 regressions selected by the broad filter.
- After fixing the expansion fixture's canonical field order, `cargo test -p postfiat-node burn5_cobalt_authority_bounds_shared_check_expansion --lib --locked`: 0 passed, 1 failed, 0 ignored, 366 filtered out. The old decoder returned the expanded transcript exceeding 32 MiB. The assertion was subsequently changed to avoid printing the large public fixture.
- `cargo check -p postfiat-node --locked`: passed.
- `cargo test -p postfiat-node cobalt_handoff::tests --lib --locked`: **13 passed**, 0 failed, 0 ignored, 354 filtered out. This exercises both repaired authority-certificate boundaries and rehearsal finalization with the existing signed fixtures, plus the existing 20-validator certificate, quorum, replay and scope tests.
- `cargo test -p postfiat-node cobalt_shadow_runtime::tests --lib --locked -- --skip local_network_drill_runs_real_signed_protocol_over_three_sockets`: **3 passed**, 0 failed, 0 ignored, 364 filtered out. The socket drill was explicitly filtered out.
- `cargo test -p postfiat-node cobalt_handoff_rehearsal::tests --lib --locked`: **1 passed**, 0 failed, 0 ignored, 366 filtered out.
- `cargo fmt --all -- --check` and `git diff --check`: passed. Rustfmt emitted existing stable-toolchain warnings about nightly-only configuration options.
- Before findings `34611d66`, repairs `21cf30f8`, and this closeout commit: `.venv-docs/bin/mkdocs build --strict`, `scripts/public-doc-links` (402 files), and `scripts/public-secret-scan` passed. The new findings document was staged before its scan.
- Each A3 unit uses its required separate commit followed by `git pull --rebase origin main && git push origin main`; pushes go only to this repository's `origin main`. Findings and repairs were pushed with clean trees before the next unit. This closeout changes only the campaign log and reruns no Rust tests.
- A3 post-repair total: **17 passed, 0 failed**, with the local socket drill intentionally skipped. **Full Rust suite verdict pending** CI for `21cf30f8`; no CI verdict is claimed.

**A4 verification:**

- Initial `git pull --rebase origin main`: passed, already up to date at `41bd85ff`; tree clean. All Cargo build/test commands below used `CARGO_NET_OFFLINE=true`.
- Before repair, `cargo test -p postfiat-node storage_activation_cli::tests::burn5_ --lib --locked`: **0 passed, 1 failed**, 0 ignored, 367 filtered out; the competing artifact was replaced.
- Before repair, `cargo test -p postfiat-node storage_backend_config::tests::burn5_ --lib --locked`: **0 passed, 1 failed**, 0 ignored, 368 filtered out; reopening still selected `BoundedJsonl` after the injected error instead of restoring `Transactional`. Only a behavior-preserving selection helper had been extracted at that point.
- Before repair, `cargo test -p postfiat-node certified_send_completed_index::completed_index_tests::burn5_ --bin postfiat-node --locked`: **0 passed, 3 failed**, 0 ignored, 170 filtered out. Append durability and unrelated-directory membership reproduced their intended defects. The prune fixture failed on the earlier retention-path refusal, not on the intended sync failure.
- `cargo test -p postfiat-node certified_send_completed_index::completed_index_tests::burn5_prune_ --bin postfiat-node --locked`: **0 passed, 1 failed**, 0 ignored, 172 filtered out. Added error reporting identified `certified send durable payload path is not canonical`, yielding SMG-07. That exploratory test and prune-only injection changes were removed; this is recorded failure evidence, not a passing prune-recovery claim.
- `cargo check -p postfiat-node --locked`: passed.
- `cargo test -p postfiat-node storage_activation_cli::tests --lib --locked`: **1 passed**, 0 failed, 0 ignored, 368 filtered out.
- `cargo test -p postfiat-node storage_backend_config::tests --lib --locked`: **1 passed**, 0 failed, 0 ignored, 368 filtered out.
- `cargo test -p postfiat-node certified_send_completed_index::completed_index_tests --bin postfiat-node --locked`: **21 passed**, 0 failed, 2 existing manual checks ignored, 149 filtered out. Includes both new index regressions and existing migration, interruption, retention, duplicate and bounded-work checks.
- `cargo fmt --all -- --check` and `git diff --check`: passed. Rustfmt emitted existing stable-toolchain warnings about nightly-only options; incidental whole-file index formatting was removed to keep the repair minimal.
- Before findings `307214ef`, evidence correction `514f7e15`, repair `401fa055`, and this closeout commit: `.venv-docs/bin/mkdocs build --strict`, `scripts/public-doc-links` (**403 files**), and `scripts/public-secret-scan` passed. The new findings file and all subsequent unit changes were staged before their scans.
- Each A4 commit was followed by `git pull --rebase origin main && git push origin main`; pushes go only to this repository's `origin main`. Findings, their correction and repairs were pushed with clean trees before the next unit. This closeout changes only the campaign log and reruns no Rust tests.
- A4 post-repair total: **23 passed, 0 failed, 2 ignored**. **Full Rust suite verdict pending** CI for `401fa055`; no CI verdict is claimed.

## Scores

Not run. The inventory scoring gate belongs to B, which is not started.

## Final summary

A1 is closed with **0 P1, 1 P2, 1 P3**: one repaired P2 and one recorded, unfixed P3. Findings commit: `6f2332cf`; repair commit: `1c9f44f1`. **Consensus-affecting repairs: none.** Sixteen focused tests passed; the full Rust suite verdict remains pending CI. Remaining risks are the latest-ID reporting discrepancy and the unreviewed delegated boundaries named above.

Only `crates/node/src/mempool_proposals.rs` was reviewed as source, in full at the A1 focus; no other source file was reviewed. The release branch and its checkout were not mutated. The A1 findings, repair and closeout units are committed and pushed separately to `origin main`, within the 30-minute time box. Work stops here with A2–A5 and B pending; no other surface, inventory, scoring or Task Node work started.
