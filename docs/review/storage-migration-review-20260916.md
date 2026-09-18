# Storage migration and activation, certified-send index review — 2026-09-16

This is A4 of the [burn 5 campaign](qa-campaign-20260916-burn5-brief.md), reviewed on clean `main` at `41bd85ff` after `git pull --rebase origin main`. All 3,860 lines of the four allowed source files were read, including the index's in-file tests: `storage_migration.rs` (1,005), `storage_activation_cli.rs` (378), `storage_backend_config.rs` (143), and `certified_send_completed_index.rs` (2,334), all under `crates/node/src/`. Line references below identify that pre-repair revision. Focus: interrupted migration, activation preconditions, backend selection, crash consistency and bounded growth.

## Findings

### 1. SMG-01 — P2 — activation artifact publication does not enforce exclusive creation

**Source:** `crates/node/src/storage_activation_cli.rs:85-94`; consumers at `:190`, `:255`, `:311`, and `:376`.

Two local template or batch writers target the same output. Both can pass `path.exists()` before serialization finishes, then the second atomic replacement can overwrite the first artifact. A dangling destination symlink also passes the existence check. An operator can therefore inspect one activation/cancellation artifact and subsequently consume different bytes at the same path, despite this helper promising exclusive output. This does not establish unauthorized on-chain activation.

The minimal repair is atomic no-replace publication of a fully written, synced temporary file, with a regression that inserts a competing artifact during serialization and verifies its bytes survive. Preserve existing JSON bytes and refuse all existing destination entries. **Consensus-affecting: no**; this is an operator-output interlock.

### 2. SMG-02 — P2 — interrupted index moves can be acknowledged without their directory durability barriers

**Source:** `crates/node/src/certified_send_completed_index.rs:975-988`, `:1026-1044`, and `:1070-1087`.

An append rename succeeds, then its directory sync fails or the process exits before sync. A retry sees the destination already present and skips the sync inside the `source_exists` branch. Recovery can then publish the index and durably clear the intent. A subsequent power loss may roll back the unsynced move while retaining the new index, leaving a completed job back in the active outbox. The same barrier must be retried even when no new rename is needed. The prune branch has the same conditional placement, but the normal retained-job fixture is rejected earlier by the separate path-validation dependency in SMG-07; no successful prune recovery through that dependency is claimed.

The minimal repair is to sync the append move's source and destination directories before recovered index publication, including destination-only recovery and identical retries. Add deterministic sync-failure regressions covering both append directories; retain the old index and intent on failure. Leave the blocked prune path to a repair that can also address SMG-07. **Consensus-affecting: no**; this changes local delivery-maintenance durability, without changing a storage format, ledger transition or signed/hashed encoding.

### 3. SMG-03 — P2 — intent recovery can bless unrelated completed-directory divergence

**Source:** `crates/node/src/certified_send_completed_index.rs:1055-1087` and `:1392-1402`.

Restore an index plus a pending append intent, but also restore an unrelated extra completed-job directory or omit an unrelated indexed directory. Recovery checks the intent's job only, writes a fresh directory stamp for the entire directory, and skips the old-stamp check. The unrelated discrepancy is now hidden behind a matching stamp: an omitted job remains indexed, or an extra job is never indexed or selected for retention pruning. This is distinct from intentionally deferring checks of untouched payload bytes.

The minimal repair is a bounded comparison of completed-directory membership against the recovered index before publishing its fresh stamp or clearing the intent. Add a regression for both an unrelated addition and deletion while an append intent is pending; require failure with the intent and prior index preserved. **Consensus-affecting: no**; this is a local index-consistency interlock.

### 4. SMG-04 — P2 — failed backend post-validation leaves the requested mode selected

**Source:** `crates/node/src/storage_backend_config.rs:110-124`.

An offline backend change passes the reference checks, writes the new mode, then reopening or reading the selected backend fails, or its selected tip differs from the reference. The function returns an error while leaving that mode published. A caller treating the failed command as an unchanged configuration can restart into the backend that just failed validation. The source explicitly returns the post-selection mismatch without undoing selection; no failure of the excluded storage implementation is assumed.

The minimal repair is to retain the previous mode and restore it when selection or post-validation fails, reporting restoration failures explicitly. Add a regression that injects failure after publishing a different mode and checks the persisted previous selection on reopen, plus a successful-selection case. This handles returned failures; interruption during configuration and concurrent non-cooperating writers remain separate review limits. **Consensus-affecting: no**; this restores node-local configuration after a failed command.

### 5. SMG-05 — P3 — local artifact reads do not enforce an end-to-end byte bound

**Source:** `crates/node/src/storage_migration.rs:866-906` and `crates/node/src/certified_send_completed_index.rs:402-427`.

A supplied migration manifest/checksum can be arbitrarily large before parsing. Index reads check path metadata and then perform a separate unbounded `fs::read`; a non-cooperating local writer can grow or replace the file between those operations. These local inputs can consume memory beyond the intended small control-artifact size. No remote-write path is established by this review.

A future minimal repair would cap manifest/checksum reads and read each index through one checked regular-file handle with a limiting reader, rejecting excess bytes and symlink substitution. This P3 is recorded without repair.

### 6. SMG-06 — P3 — bare relative migration output fails disk-space preflight

**Source:** `crates/node/src/storage_migration.rs:155-161` and `:968-972`.

With a bare relative output such as `generation`, `Path::parent()` produces the empty path. The disk-space helper searches its ancestors without normalizing it to `.` and can return `no existing output ancestor`, preventing an otherwise valid offline rebuild or verification before reaching the target. Existing output mount points can also differ from the parent filesystem used for this estimate.

A future minimal repair would resolve the output's nearest existing filesystem location, treating an empty relative ancestor as `.`, with relative-path and mounted-output coverage. This P3 is recorded without repair.

### 7. SMG-07 — P2 — recovery rejects the payload path of a job already moved into retention

**Source:** `crates/node/src/certified_send_completed_index.rs:1015-1024`, delegating through `:773-795` to `read_validated_durable_certified_send_payloads` in `crates/node/src/transport_cli.rs`.

A prune intent is durable and its completed job has moved to the retention directory, but the index has not yet been rewritten. The recovery caller validates that retained directory using the ordinary durable-payload resolver. A local fixture using the existing tombstone builder reproduces `certified send durable payload path is not canonical`; recovery cannot finish this normal interrupted-prune state. This finding was discovered while testing SMG-02, before implementing repairs. The initial three-test run had two confirmed recovery defects and this earlier path refusal, rather than three successful reproductions of the intended assertions.

The minimal repair needs the shared resolver to support authenticated, constrained retention relocation, then retry the prune directory durability barriers before index publication. **Dependency:** `crates/node/src/transport_cli.rs`, outside the four-file A4 scope. A filename-only symbol lookup located the owner; its implementation was not reviewed. Do not duplicate or bypass its path validation in A4. **Repair skipped at this boundary.** Consensus impact of a future resolver repair requires review in its owning surface; no such repair is made or activated here.

## Areas with no findings

- `storage_migration.rs:69-455`: pending-journal refusal in verify-only, source replay call, expected tip/root checks, retained-history count arithmetic, source/transactional manifest comparison, canonical-export recheck and publication ordering. A partial generation is not published by this function before its checks complete; rerunning into a nonempty output refuses replacement. No additional finding in these orchestration paths.
- `storage_migration.rs:458-645,648-919,922-965,993-1005`: verification compares source domain/tip, both reconstructed manifests, metadata, verifier version and exact checksum/export; receipt mapping and disk arithmetic use explicit error paths. No additional finding beyond the resource/path issues above. Called replay, receipt exceptions and storage internals were not reviewed.
- `storage_activation_cli.rs:96-378`: templates require a fully verified migration at the current tip, future activation height and matching ordered count; cancellation must precede activation and identify the scheduled record. Batch construction calls readiness verification before output, and ratification checks placeholder authorization and height conversion. No additional finding at these call sites; delegated authorization/admission implementations remain unverified.
- `storage_backend_config.rs:35-108`: pending-journal, activation, exact verified transactional tip, legacy commitment and bounded-index equality checks precede mode publication. No additional finding beyond failed post-validation handling.
- `certified_send_completed_index.rs`: schema/checksum/order/duplicate validation, entry and intent bounds, mutation locking, one-time migration, bounded ordinary compaction, retention ordering and existing in-file tests were read. No additional finding in those paths. Idle passes deliberately defer untouched-payload validation; successful normal moves sync before index publication.

## Review limits and skips

Only the four A4 source files were reviewed. Cargo target/dependency metadata and existing review documents were read for test selection, exclusions and document format. Remote-ref comparisons read filenames only; the release branch and checkout were not accessed as worktrees or mutated. The brief's excluded node files, release-candidate files, excluded crates, prior burns' source surfaces and frozen artifacts were not reviewed or edited.

The delegated `NodeStore`/transactional backend, shared atomic writer, journal recovery, block/receipt replay, state commitments, cryptographic verification, governance authorization/readiness, certified-send payload/quarantine helpers, RPC and command dispatch implementations were not reviewed. These include excluded `crates/node/src/lib.rs`, `block_finality.rs`, `block_replay_wallet.rs`, `governance.rs`, `state_commitment.rs`, `transport_runtime.rs`, `transport_runtime_tests.rs`, `tests/`, and the already reviewed `crates/storage` implementation. No conclusion about those implementations is implied by their callers passing tests.

No physical power-loss experiment, historical/Orchard replay, broad Rust suite, network/socket exercise, CI query, Task Node, fleet action, deployment or live activation is part of A4. Local deterministic regressions cover repaired boundaries; the full Rust suite is CI's verdict. P3 findings remain unfixed. A5 and B, including inventory edits and scoring, are not started.

## Repair result

A4 records **0 P1, 5 P2 and 2 P3**. Findings were pushed as `307214ef`; the SMG-02 evidence correction and newly observed SMG-07 dependency were pushed in a second findings-only commit, `514f7e15`, before repairs. Source regression work was temporarily stashed for that correction, then restored; the stash was removed.

- **SMG-01:** synced temporary JSON is published with an atomic no-replace hard link, then the temporary name is removed and the output directory synced. The regression preserves a competing artifact created during serialization, rejects a dangling destination symlink, verifies unchanged JSON encoding on success and checks temporary-file cleanup. **Consensus-affecting: no.**
- **SMG-02:** append recovery retries both move-directory syncs even when the destination is already present. The regression injects two consecutive failures for each directory, requires unchanged persisted index and retained intent, then verifies successful recovery after the failure is removed. **Consensus-affecting: no.** No claim is made for the separate blocked prune path.
- **SMG-03:** recovery compares bounded completed-directory membership with the resulting index before publishing a fresh stamp or clearing the intent. The regression rejects unrelated addition and omission while preserving the prior index and intent. Ordinary maintenance retains its existing bounded work. **Consensus-affecting: no.**
- **SMG-04:** selection captures the previous backend mode and restores it after a selection/post-validation error; restoration errors include both causes. The regression verifies the failed candidate really was selected, then confirms the previous mode on reopen, followed by a successful selection. **Consensus-affecting: no.**
- **SMG-07:** unfixed because its shared retention-path resolver is in `crates/node/src/transport_cli.rs`, outside A4. The exploratory prune regression was removed after its observed canonical-path refusal was recorded. Fixing or bypassing that resolver, or claiming a successful prune retry through it, would exceed this review.
- **SMG-05/06:** P3 findings remain recorded without repair.

Only `storage_activation_cli.rs`, `storage_backend_config.rs` and `certified_send_completed_index.rs` changed as source, with four in-file regressions. Each repaired defect reproduced against the prior behavior before its repair. The full module checks then passed:

- `CARGO_NET_OFFLINE=true cargo check -p postfiat-node --locked`: passed.
- `CARGO_NET_OFFLINE=true cargo test -p postfiat-node storage_activation_cli::tests --lib --locked`: **1 passed**, 0 failed, 0 ignored, 368 filtered out.
- `CARGO_NET_OFFLINE=true cargo test -p postfiat-node storage_backend_config::tests --lib --locked`: **1 passed**, 0 failed, 0 ignored, 368 filtered out.
- `CARGO_NET_OFFLINE=true cargo test -p postfiat-node certified_send_completed_index::completed_index_tests --bin postfiat-node --locked`: **21 passed**, 0 failed, 2 existing manual release checks ignored, 149 filtered out.
- `cargo fmt --all -- --check` and `git diff --check`: passed.

Post-repair total: **23 passed, 0 failed, 2 ignored**. **Full Rust suite verdict pending** CI. No repair changes a consensus rule, state-transition result, storage format or signed/hashed encoding, and none is deployed or activated.
