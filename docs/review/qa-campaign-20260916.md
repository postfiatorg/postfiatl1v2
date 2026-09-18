# QA campaign log — 2026-09-16

This is the progress record for the [burn 5 campaign](qa-campaign-20260916-burn5-brief.md). Work began on clean `main` at `c4303717` after `git pull --rebase origin main`. A1–A5 were completed in separate bounded units. B and this final closeout began on clean `main` at `330d38be` after `git pull --rebase origin main`, within a 40-minute time box. The surface-result and verification sections preserve their historical unit-time scope and pending statements; the Status, campaign-state table and Final summary give the final disposition.

**Status:** closed. A1–A5 and B are complete within their recorded limits. Burn 5 recorded 22 findings: 14 repaired, seven P3s recorded without repair, and SMG-07 blocked by its out-of-scope dependency. B added all 22 inventory rows and passed the first full Text Improvement Harness gate at **86.33/100**; no rewrite or rescore. All five repair commits retain a full Rust suite verdict pending. No live activation, Task Node or fleet action. Work stops after this closeout.

## Campaign state

| Order | Surface | Status | P1 | P2 | P3 | Evidence or fixes |
| --- | --- | --- | ---: | ---: | ---: | --- |
| A1 | Mempool proposals | done | 0 | 1 | 1 | [Review](mempool-proposals-review-20260916.md); findings `6f2332cf`; repair `1c9f44f1` (not consensus-affecting; full Rust suite verdict pending); MPL-02 recorded without repair |
| A2 | Vote locks and view recovery | done | 1 | 1 | 0 | [Review](vote-locks-review-20260916.md); findings `1511ea09`; repair `090bd17e` (VLK-01 and VLK-02 both consensus-affecting; full Rust suite verdict pending); all P1/P2 repaired |
| A3 | Cobalt handoff and authority | done | 0 | 4 | 2 | [Review](cobalt-handoff-review-20260916.md); findings `34611d66`; repair `21cf30f8` (CHO-01/02 consensus-affecting, CHO-03/04 not consensus-affecting; full Rust suite verdict pending); CHO-05/06 recorded without repair |
| A4 | Storage migration and activation, certified-send index | done; one repair blocked by scope | 0 | 5 | 2 | [Review](storage-migration-review-20260916.md); findings `307214ef`, correction/dependency `514f7e15`; repair `401fa055` (SMG-01–04 not consensus-affecting; full Rust suite verdict pending); SMG-07 depends on out-of-scope `transport_cli.rs`; SMG-05/06 recorded without repair |
| A5 | Swap and recovery services | done | 0 | 3 | 2 | [Review](swap-recovery-services-review-20260916.md); findings `696eeffa`; repair `d679f8e8` (SWP-01 consensus-affecting, SWP-02/03 not consensus-affecting; full Rust suite verdict pending); all P2 repaired, SWP-04/05 recorded without repair |
| B | Defect inventory and TIH gate | done | — | — | — | [Inventory](defect-inventory-20260910.md) extended to 126 rows in `17640b29`; first full gate **86.33/100**; run group `qa-defect-inventory-burn5-20260916`; scored SHA-256 `f81ff48dd6f1cd9afe49f57ffe0b8548cc2af14a13ed2b964641ca2970a24c35` |

Current finding totals: **1 P1, 14 P2, 7 P3** (A1, A2, A3, A4 and A5).

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

A5 began on clean `main` at `3bfcbfed` after `git pull --rebase origin main`, at approximately 12:38 UTC within a 30-minute time box. The [A5 review](swap-recovery-services-review-20260916.md) read all **4,171 lines** of the five allowed files under `crates/node/src/`: `pftl_swap_service.rs` (2,028), `atomic_swap_rpc.rs` (453), `atomic_swap_rpc_server.rs` (320), `fastpay_recovery_node.rs` (986), and `operator_attestations.rs` (384), including in-file tests. Focus: amount arithmetic/rounding, swap/recovery replay, attestation verification, state-change interlocks and malformed input. No other source implementation was reviewed as A5.

- **SWP-01 (P2):** FastPay rollback zipped ledger-position-sorted inverse records with certificate-ordered inputs, rejecting valid differing orders. Repair `d679f8e8` compares exact identity/version sets and requires unique ascending inverse positions. The regression applies the real transfer model in two ledger orders with untouched objects, restores the complete original ledger, and rejects four malformed inverse cases without mutation. **Consensus-affecting: yes**, because recovery reconciliation results change; no schema or signed/hashed encoding change.
- **SWP-02 (P2):** forward PFTL journal transitions could substitute the prepared/published batch hash. The repair rejects substitution at publication and resolution; its regression preserves durable bytes on conflicting publish/commit/reject requests, permits prepublication reproof and completes/reloads the correct batch. **Consensus-affecting: no.**
- **SWP-03 (P2):** the journal writer could publish more than the reader's 32 MiB limit. The repair bounds actual serialized bytes before replacement. Its regression uses 1,536 valid entries with 64 bounded transitions each and verifies refusal preserves the existing readable journal. **Consensus-affecting: no.**
- **SWP-04 (P3):** timing retries double-count already recorded stages against capacity. Recorded without repair.
- **SWP-05 (P3):** attestation timestamps accept impossible calendar/time values. Recorded without repair.

Findings were pushed as `696eeffa` before repair. All three P2 regressions reproduced the original defects. Repair `d679f8e8` changes only `fastpay_recovery_node.rs`, `pftl_swap_service.rs` and the findings document. All **11 focused tests passed**; no P1/P2 repair was skipped or required an excluded-file change. **Full Rust suite verdict pending** CI for `d679f8e8`; no deployment, live activation or CI verdict is claimed.

The findings document records the full limits. Delegated execution/types, certificate/cryptographic verification, storage durability, historical replay, RPC dispatch/limits, caller serialization, quote snapshot coherence, proof construction and external attestation trust/freshness were not reviewed. The recovery regression uses a synthetic certificate/fence around the real transfer model; it is not quorum-certificate admission or archived-chain replay evidence. Release-candidate source files, prior burns' source surfaces, excluded crates and frozen artifacts were not reviewed or edited. No work stopped for time or usage. A5 is closed; B remains pending and is not started.

## Skips and boundary decisions

- Historical A4 closeout: A5 and B were outside that unit and remained pending; no inventory edit or scoring occurred during A4.
- Release-candidate files listed in the brief, excluded crates, previously reviewed surfaces and frozen artifacts are not reviewed or edited. Only remote-ref path metadata is compared to establish exclusions.
- No Task Node or fleet action, release-branch or release-checkout mutation, spend, signup or deployment.
- A1: MPL-02 (P3) remains recorded without repair as required. No A1 P1/P2 repair required an excluded file.
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

- A5: SWP-04/05 remain unfixed P3 findings as required. All three P2 findings were repaired within the two allowed source files; no excluded-file repair dependency arose.
- A5: no source outside the five listed files was reviewed; delegated implementations and excluded test/type files were not opened for review or edited. Fixture field names came from compiler diagnostics. Remote-ref comparisons returned filenames only.
- A5: no full Rust/Orchard suite, historical replay, physical crash experiment, network/socket exercise or CI status query ran. The changed FastPay owned-object inverse and local PFTL journal boundaries do not change Orchard execution, proofs or historical Orchard replay. The full Rust suite remains CI's verdict.
- A5: no Task Node, fleet action, release-branch/checkout mutation, deployment, activation, other surface, inventory or scoring work occurred. B remains pending. No time or usage limit curtailed A5.

- B: no source review or repair was reopened. All 22 findings were copied from the five existing review records; the original 104 inventory finding rows remain byte-for-byte unchanged. The seven new P3s are labelled code-observed because their records do not claim executed reproductions.
- B: SMG-07 is blocked by A4's four-file scope through `crates/node/src/transport_cli.rs`. No review record identifies a repair blocked specifically by the release candidate; the inventory preserves the documented scope dependency rather than assigning an unsupported release-candidate blocker.
- B: a broad harness-location filename search inadvertently returned paths inside the excluded release checkout because its exclusion glob did not match absolute paths. No returned release file was opened, no contents were searched, and no release checkout or branch was mutated. Subsequent harness reads used its exact external directory.
- B: no Task Node or fleet action, source edit, deployment, live activation, release branch operation, spend outside permitted scoring, signup or toolchain install occurred. Network use was limited to git and OpenRouter scoring. Frozen artifacts were unchanged. Rust/Orchard tests, physical fault tests, archived replay and CI status queries were skipped because B is documentation-only and preserves the existing verification limits.

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

**A5 verification:**

- Initial `git pull --rebase origin main`: passed, already up to date at `3bfcbfed`; tree clean. All Cargo build/test commands below used `CARGO_NET_OFFLINE=true`.
- Findings exploration: `cargo test -p postfiat-node fastpay_recovery_node::burn5_recovery_tests --lib --locked -- --nocapture`: **0 passed, 1 failed**, 0 ignored, 369 filtered out, at the original rollback input-order check after the real transfer model succeeded. Before this reproduction, six fixture-compilation attempts executed no tests (the first omitted `-- --nocapture`), and two earlier runs each failed one test during fixture deserialization; those were fixture failures, not defect evidence. The exploratory source edit was removed before the findings-only commit.
- Before PFTL repair, `cargo test -p postfiat-node pftl_swap_service::tests::burn5_ --lib --locked`: **0 passed, 2 failed**, 0 ignored, 370 filtered out. Publication accepted the conflicting batch, and oversized persistence returned success.
- `cargo check -p postfiat-node --locked`: passed.
- `cargo test -p postfiat-node fastpay_recovery_node::burn5_recovery_tests --lib --locked`: **1 passed**, 0 failed, 0 ignored, 371 filtered out. This command passed first for the initial two-input case and again after adding both ledger orders, untouched objects and four invalid inverse cases; the final unique test count is one.
- `cargo test -p postfiat-node pftl_swap_service::tests --lib --locked`: **10 passed**, 0 failed, 0 ignored, 362 filtered out; includes both new PFTL regressions.
- `cargo fmt --all -- --check` and `git diff --check`: passed. Rustfmt emitted existing stable-toolchain warnings for nightly-only configuration options.
- Before findings `696eeffa`, repair `d679f8e8`, and this closeout commit: `.venv-docs/bin/mkdocs build --strict`, `scripts/public-doc-links` (**404 files**) and `scripts/public-secret-scan` passed. Every unit's changes were staged before its secret scan.
- Each A5 unit uses the required separate commit followed by `git pull --rebase origin main && git push origin main`; pushes go only to this repository's `origin main`. Findings and repairs were pushed with clean trees before the next unit. This closeout changes only the campaign log and reruns no Rust tests.
- A5 post-repair total: **11 unique tests passed, 0 failed, 0 ignored**. **Full Rust suite verdict pending** CI for `d679f8e8`; no CI verdict is claimed.

**B verification:**

- Initial `git pull --rebase origin main`: passed; already up to date at `330d38be`, with a clean tree.
- Inventory commit `17640b29`: all 104 prior finding rows compared byte-for-byte equal to the starting revision. An ID audit matched all 22 new rows to the five review documents, with no duplicate or missing finding: MPL- 2, VLK- 2, CHO- 6, SMG- 7, SWP- 5. Classification, severity, disposition and completeness counts independently sum to 126; new links and findings anchors resolve locally.
- Before inventory commit `17640b29`: `.venv-docs/bin/mkdocs build --strict`, `scripts/public-doc-links` (**404 files**) and `scripts/public-secret-scan` passed. The inventory was staged before the scan; `git diff --check` passed. The commit was followed by `git pull --rebase origin main && git push origin main`, with a clean tree before scoring.
- The gate completed with fifteen valid scores; the stored models, prompt, indices, group, inventory hash and raw-response scores were verified. The score table and retained record paths are in Scores below. The scored inventory has not changed since `17640b29`.
- Before this closeout commit: `.venv-docs/bin/mkdocs build --strict`, `scripts/public-doc-links` (**404 files**) and `scripts/public-secret-scan` passed again, with the final log staged before the scan; `git diff --check` passed. This documentation-only closeout is followed by `git pull --rebase origin main && git push origin main`; only this repository's `origin main` receives pushes, and the tree is clean at completion.
- No Rust suite or CI status query ran during B. The five repair commits remain **full Rust suite verdict pending**: `1c9f44f1`, `090bd17e`, `21cf30f8`, `401fa055` and `d679f8e8`. A1–A5's recorded post-repair focused counts are respectively 16, 20, 17, 23 and 11 passes (87 in total), with three existing manual tests ignored and A3's socket drill excluded. These are retained results, not new runs or a full-suite verdict.

## Scores

The exact 126-row inventory bytes from `17640b29`, SHA-256
`f81ff48dd6f1cd9afe49f57ffe0b8548cc2af14a13ed2b964641ca2970a24c35`,
received fifteen fresh OpenRouter reviews at temperature 0 with an 8,000-token
response limit: the same three judges, five reviews each, and procedure as the
[burn 2 gate](qa-campaign-20260910.md#burn-2-final-text-improvement-harness-gate)
and [burn 4 example](qa-campaign-20260915.md#scores). The prompt was
`Rate this document on a scale of 1-100. Output the score and your reasoning.`
The harness's strict-JSON wrapper matches the retained burn 4 prompts verbatim.
The credential came from vault label `openroutertih` and was passed in memory;
no credential was written to the log or database.

| Judge | Scores (run order) | Average |
| --- | --- | ---: |
| `openai/gpt-6-astra-pro` | 86, 86, 86, 84, 86 | 85.60 |
| `anthropic/claude-fable-5.1` | 84, 80, 86, 84, 85 | 83.80 |
| `z-ai/glm-5.3` | 88, 90, 90, 90, 90 | 89.60 |
| **All fifteen** | — | **86.33** |

Run group: `qa-defect-inventory-burn5-20260916`. The first compliant full
score exceeded the 86/100 stop condition; **no rewrite or rescore** occurred,
and the scored inventory bytes remain unchanged. The harness ran `score` with
`--gate full --runs 5 --force --temperature 0 --max-tokens 8000 --concurrency 15`,
the explicit prompt above, and the named run group. Fable run 5 and GLM run 4
each used the harness's built-in second-attempt retry; this was one full gate
with fifteen valid scored responses, without a model or prompt substitution.

All fifteen SQLite records were checked against the exact prompt, prompt hash,
document SHA-256, run group, three model identities, run indices 1–5 and parsed
raw-response scores. The external score log and SQLite record are
`/home/postfiatchad/pastedocs/.qa-campaign-defect-inventory-burn5-20260916/score.log`
and `scores.sqlite3` in the same directory.

## Final summary

| Surface | P1 | P2 | P3 | Findings commit | Repair commit |
| --- | ---: | ---: | ---: | --- | --- |
| A1 — Mempool proposals | 0 | 1 | 1 | `6f2332cf` | `1c9f44f1` |
| A2 — Vote locks and view recovery | 1 | 1 | 0 | `1511ea09` | `090bd17e` |
| A3 — Cobalt handoff and authority | 0 | 4 | 2 | `34611d66` | `21cf30f8` |
| A4 — Storage migration, activation and certified-send index | 0 | 5 | 2 | `307214ef`, correction `514f7e15` | `401fa055` |
| A5 — Swap and recovery services | 0 | 3 | 2 | `696eeffa` | `d679f8e8` |
| **A1–A5 total** | **1** | **14** | **7** | — | — |

Fourteen findings are repaired in source: the P1 and thirteen P2s. Seven P3s
remain recorded without repair; one P2, SMG-07, remains blocked by A4 scope.
**Full Rust suite verdict pending** for every repair commit in the table:
`1c9f44f1`, `090bd17e`, `21cf30f8`, `401fa055` and `d679f8e8`.
No CI success, deployment or live activation is claimed.

**Consensus-affecting repairs, explicitly:**

- `090bd17e`: VLK-01 tightens signer-safety persistence by requiring the
  canonical lock directory durability barrier; VLK-02 tightens signer admission
  on ambiguous restored state by rejecting non-regular JSON migration entries.
  Both are conservatively consensus-affecting.
- `21cf30f8`: CHO-01 tightens authority-certificate admission by binding
  full-knowledge evidence to the current ratification; CHO-02 rejects over-bound
  transcript expansion before cloning shared checks.
- `d679f8e8`: SWP-01 changes recovery reconciliation results by matching exact
  input identity/version sets while preserving unique ascending inverse positions.

These five repairs are source-only, not activated or deployed. CHO-03/04,
SMG-01–04, SWP-02/03 and MPL-01 are not consensus-affecting. The repaired lock,
certificate and recovery paths retain their schemas and signed/hashed encodings;
admission, persistence and recovery results are still material changes.

**Blocked repair and remaining risks:**

- SMG-07 requires the shared retention-path resolver in
  `crates/node/src/transport_cli.rs`, outside A4's four-file scope. Interrupted
  prune recovery still rejects the relocated payload, and prune directory-sync
  behavior remains unqualified. The SMG-02 append repair does not resolve this
  dependency. **No finding is documented as repair-blocked specifically by the
  release candidate**; that exclusion is distinct from the recorded A4 limit.
- Unfixed P3s: MPL-02 latest-ID ordering; CHO-05 approval-free negative rehearsal
  probes; CHO-06 non-hex manifest digests; SMG-05 unbounded/racy local artifact
  reads; SMG-06 output-filesystem preflight; SWP-04 timing replay at capacity;
  SWP-05 impossible attestation timestamps.
- Delegated cryptography, execution/storage, historical replay, caller
  serialization, RPC limits, registry reconstruction and external attestation
  trust/freshness remain outside the reviewed surfaces as detailed above.
  Deterministic error injection is not physical power-loss evidence; the
  FastPay inverse fixture is not quorum admission or archived-chain replay.
  Rehearsal report consumers outside A3 were not tested. Existing inventory
  operational and evidence limits remain unchanged.

B added **22 rows**: MPL- **2**, VLK- **2**, CHO- **6**, SMG- **7**, SWP- **5**.
All **104 existing finding rows remain byte-for-byte unchanged**. The inventory
now has **126 rows: 26 P1, 68 P2 and 32 P3**. Classifications total 92 reproduced
defects, seven code-observed defects, 22 evidence gaps, one economic assumption
and four proposed capabilities. Dispositions total 84 fixed, 16 dispositioned,
five reproduced and retained, eight recorded P3s without repair, one repair
blocked by scope, six needing a live environment and six needing an operator
decision; zero bare open.

The inventory gate passed on its first full score at **86.33/100**, run group
`qa-defect-inventory-burn5-20260916`, inventory SHA-256
`f81ff48dd6f1cd9afe49f57ffe0b8548cc2af14a13ed2b964641ca2970a24c35`.
No wording rewrite or rescore occurred. Inventory commit `17640b29` and this
closeout are the final B units, each gated and pushed separately to `origin main`.
The release branch and checkout were not mutated; the filename-search boundary
slip is recorded under Skips. No Task Node or fleet action occurred. The campaign
is closed within the requested 40-minute B time box; work stops here.
