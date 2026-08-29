# Certified-Send Eager Index Migration: Option 1 Remediation Spec

**Status:** Complete — remediation passed; the later remediated G4 campaign failed separately on vote-lock migration and both height ratios
**Date:** 2026-08-29
**Planning baseline:** `main` at `9f554001` (final G4 closeout docs); failed frozen candidate `e52e0502`
**Runner lineage:** gate logic remains `15d059d1`; test-only successor `a3c7bea9285ab02871fd2111038764c6174b905b` on `postfiatchad/corrected-g4-vote-lock-gate`
**Motivating failure:** [Final G4 qualification failure](../../handoffs/2026-08-29___postfiatchad__final_g4_qualification_failure.md)
**Predecessors:** [Final G4 qualification plan](final-g4-qualification-campaign-plan.md) (closed, failed),
[Certified-send tombstone bounding plan](certified-send-tombstone-bounding-plan.md) (implemented; source of the defect this spec removes),
[Storage scaling milestone](storage-scaling-milestone.md) (governing gates)

**Later outcome:** this spec's certified-send fix passed its affected gates in
all ten selected windows. The separately authorized campaign nevertheless
failed on the vote-lock marker contract and both selected height ratios; see
the [remediated G4 failure](../../handoffs/2026-08-29___postfiatchad__remediated_g4_qualification_failure.md).

## BLUF for the reviewer

The final G4 campaign failed on exactly one contract mismatch:
`CERTIFIED_SEND_INDEX_MIGRATION_AFTER_FIRST_VALIDATOR_RESUME`. Five validators
had no certified-send outbox on their first resume, so the candidate deferred
its one-time completed-index migration to the resume after deliveries created
the outbox — their second observed resume — and the runner permits the one
migration only on observation 1.

This spec chooses the **candidate-owned** contract (Option 1 of the failure
handoff): the node must complete migration on every validator's first
successful resume, outbox or not. The durable invariant becomes a node
property, not a benchmark-runner convention. The mechanical change is small —
route the no-outbox early return in
`crates/node/src/certified_send_completed_index.rs` through the existing
`ensure_index` migration, which already degenerates to writing an empty index
when the completed directory is absent. The runner gate at `15d059d1` becomes
correct as written and needs no logic change.

The cost is the full candidate evidence cascade: new source freeze, new binary,
G1/G2 refresh, and re-targeting the still-open remediated G3 height-915 replay.
That cost is accepted deliberately: production validators are born outbox-less
and will hit this exact path on their first real resume, so the fix belongs in
the node.

## The contract being locked

> After any validator's first successful certified-send resume, the durable
> completed-set index exists and is bound. Index migration is observable at
> most once per data directory lifetime, and only on that first successful
> resume.

Notes on wording:

- **"Successful"**: a resume that crashes before the index write never
  returned, is never observed by the runner (only completed rounds emit
  telemetry), and the retry remains that validator's first successful resume.
  `write_index` uses `postfiat_storage::atomic_write`, so a crash leaves either
  no index (clean retry) or a complete index (migration already done).
- **"At most once"**: the runner's existing repeated-migration and
  late-migration checks (`run_campaign.py` at `15d059d1`, migration allowance
  block near line 1246) enforce exactly this. Its comment already states the
  allowance is "per-validator first use"; this spec makes first use and first
  observation coincide.

## Code map (verified at `main` `9f554001`; `rg` to re-locate if drifted)

All paths in `crates/node/src/certified_send_completed_index.rs` unless noted.

| Item | Location |
| --- | --- |
| **The defect**: no-outbox early return in `compact_completed_with_index_locked` — accepts an absent outbox without creating an index; only reaches `ensure_index` when the outbox exists | 1173–1195 |
| `ensure_index` — reads the index or performs the one-time migration: `fully_validate_completed_jobs` → `write_index` → sets `index_migration_performed` | 731–754 |
| `completed_directory_names` — returns an **empty list** for a missing completed directory | 582–589 |
| `write_index` — deterministic sort, checksum, directory stamp at write time, atomic write | 426–451 |
| `completed_directory_stamp` — records `directory_exists: false` with zeroed metadata for a nonexistent completed directory | 310–348 |
| `require_completed_directory_stamp` — tamper seal comparing stored versus observed stamp | 354–366 |
| `discover_active_completed_jobs` — scans the **outbox** for completed-but-unindexed jobs | 956 |
| Existing zero-tombstone bounded-resume test (pre-creates the outbox; keep) | 1472 |
| `certified_send_completed_dir` = `outbox/completed` — structural fact: **no outbox implies no completed directory** | `transport_cli.rs:1043` |
| `resume_durable_certified_send_outbox` — calls compaction **before** its own outbox-existence check | `transport_cli.rs:2367` |
| Telemetry wire-through of `index_migration_performed` into round timings | `transport_runtime.rs:3733` |

## Why eager creation is coherent (correctness walk of the failed sequence)

The stamp design makes Option 1 sound. The completed-tombstone directory is
mutated **only** by compaction, pruning, and explicit repair — job completion
marks `completed = true` in place in the outbox (`transport_cli.rs:2033`), and
jobs move into `outbox/completed` only when compaction appends them to the
index. The stamp is therefore a seal against out-of-band mutation, and eager
creation never violates it:

1. **First resume, no outbox.** `ensure_index` runs: no index file, no intent,
   `fully_validate_completed_jobs` returns empty (missing directory), an empty
   index is atomically written with stamp `directory_exists: false`, and
   `index_migration_performed = true`. Telemetry: migration on observation 1;
   tombstone validation, payload reads/hashes, completed-directory enumeration,
   compaction, and pruning are all zero. The deliberate post-write index reread
   records one bounded index file/byte read. Runner: migration-position and
   bounded-index gates pass; the migration arithmetic check
   `validated == enumerated + compacted + pruned` holds as `0 == 0`.
2. **Deliveries create the outbox.** Jobs are enqueued and completed in place
   in the outbox. The completed directory still does not exist.
3. **Second resume.** `ensure_index` reads the existing empty index — **no
   migration flag**. `discover_active_completed_jobs` finds the completed
   outbox jobs; the stamp check compares the stored `directory_exists: false`
   against the observed still-absent completed directory — **match**.
   `append_completed_job` then moves each job in, rewriting the index and
   stamp per entry. Telemetry: `validated == compacted + pruned` — the
   ordinary (non-migration) runner check passes.
4. **Later idle no-outbox resumes** (a validator that never sends): one
   bounded index read per resume, no migration flag, O(1) work.

Guards that must survive unchanged:

- **Non-empty index without an outbox** (line ~1185) stays a hard error. The
  no-outbox path only ever writes an *empty* index, and a non-empty index
  requires the completed directory (inside the outbox) to have existed, so
  this guard remains reachable only through tampering. Do not weaken it.
- **Intent without index** (`ensure_index`, line ~740) stays a hard error and
  now also protects the no-outbox branch, which currently bypasses it.
- **Explicit repair** (`verify_and_rebuild_completed_index`) keeps sole
  authority for full-set revalidation and stamp reconciliation.

## Change specification

### Step 1 — candidate change (single function)

In `compact_completed_with_index_locked`, replace the no-outbox early-return
block. Current behavior: `read_index` and error only if a non-empty index
exists, otherwise return an empty report without creating anything. Required
behavior: call `ensure_index` (creating and binding the empty index on first
use, setting `index_migration_performed`), then retain the non-empty-entries
guard against a migrated-then-tampered outbox deletion, then return the
report. No other call site, schema, constant, stamp rule, intent rule, fsync
rule, or consensus byte changes.

State table for the no-outbox branch:

| Pre-state (no outbox) | Today | After this spec |
| --- | --- | --- |
| No index, no intent | Silent no-op; migration deferred to second resume — **campaign failure** | Empty index written; migration reported on this resume |
| Empty index present | Silent no-op | Bounded index read; no migration flag |
| Non-empty index present | Error: explicit verify repair required | Unchanged error |
| Intent present, index missing | Silent no-op — **latent gap** | Unchanged `ensure_index` error: explicit verify repair required |

### Step 2 — the missing fixtures (candidate tests)

The final-campaign preflight ran 95 tests and still missed this defect because
no fixture exercised *no outbox → deliveries → second resume*. Add, in
`certified_send_completed_index.rs` tests:

- [x] `no_outbox_first_resume_migrates_empty_index`: fresh data dir with no
  outbox → resume → index file exists, `index_migration_performed` true,
  `tombstones_validated == files_read == bytes_hashed == 0`.
- [x] `no_outbox_second_resume_reads_index_without_migration`: continue → second
  resume → no migration flag, `index_files_read == 1`, bounded work.
- [x] `campaign_replay_no_outbox_then_deliveries_then_resume`: the exact failed
  sequence — fresh dir → resume (eager migration) → create five completed
  deliveries in the outbox → resume → no migration flag,
  `jobs_compacted == 5`, and the runner's arithmetic
  `tombstones_validated == jobs_compacted + jobs_pruned` holds.
- [x] `no_outbox_intent_without_index_fails_closed`: intent file present, no
  index, no outbox → resume errors requiring explicit repair.
- [x] Tamper guard regression: non-empty index with deleted outbox still errors.

Keep `resume_with_zero_tombstones_is_bounded` (it pre-creates the outbox and
tests a different state). Update any existing assertion that a no-outbox
resume performs no migration.

### Step 3 — runner posture (no logic change; reviewer decision on rebinding)

The migration-position, repeated-migration, telemetry-validity, and work
gates at `15d059d1` are correct under the locked contract and must not
change. One decision is left to review: whether to add a runner-side fixture
replaying the step-1 telemetry sequence against the gate code. Adding it
changes the runner tree and therefore the bound runner commit. Recommendation:
add the fixture and rebind, since any future campaign already requires a new
reviewed campaign plan and fresh input binding; but a reviewer may keep the
runner byte-frozen at `15d059d1` instead. Either choice must be recorded in
the future campaign plan.

**Decision recorded:** adopt the fixture. Runner gate logic is unchanged; the
new test-only runner identity is
`a3c7bea9285ab02871fd2111038764c6174b905b`. The focused runner, packager, and
independent-verifier suite passes 96 tests. Any future campaign must bind this
new runner identity rather than `15d059d1`.

## Hard invariants — violating any of these is a failed execution

1. **Durable-delivery semantics unchanged.** Pending-job proposal refusal,
   completion, acknowledgement matching, quarantine, staging-orphan cleanup,
   retention handling, canonical job-ID names, symlink refusal, size bounds,
   and fsync ordering keep their current behavior.
2. **Fail closed at the same authority points.** Full-set validation authority
   remains: one-time migration, explicit repair, and touched entries. Eager
   creation adds no new authority; it moves the (empty) migration earlier.
3. **Bounded per-proposal work unchanged.** After the index exists, resume
   work stays bounded by pending jobs plus O(1) index reads at 0, 240, and
   1,024 tombstones. The eager write is a one-time O(1) cost.
4. **No consensus-byte changes.** The outbox and index are node-local durable
   state. Certificates, receipts, batches, and signing bytes are untouched.
5. **Determinism discipline.** No time, randomness, or unordered iteration in
   any decision; the existing deterministic entry ordering is unchanged.

## Evidence and identity refresh (the accepted cost of Option 1)

| Identity | Final result |
| --- | --- |
| Candidate source `e52e0502` | Superseded by pushed source `a92bb085ceb6a9f405e916608e6b7bb6010fcc9b`; the failed lineage stays preserved and labeled |
| Candidate binary `6b130a…a82e` | Superseded by release binary SHA-256 `902773e00e5226dab9e027ebce2b932b2cf26509dba08424f6ebe46db985e182`, 51,977,656 bytes, embedded revision `a92bb085`, profile `release` |
| G1 candidate manifest `895ec7…9ffe` | Refreshed PASS; SHA-256 `ed66a6375234f64d5aab863bccb6415b07c77fc5a3a028c5a6c2f01f41af0190` |
| G2 safety manifest `dc01f9…78e7` | Refreshed PASS; manifest `dd300bcb8130f91ab54e26f969fe7dca37335d99cc5bf4ca78a939a79584d170`, rollback `9c32319693df1f55a6c1ecd75449fe8341d180317e11547c604f135741c3e8a5`, tamper/crash `6b63fe1070a2981e5d2720bf25b3cf3b8ad95beece364d9fd76579f027b146e0` |
| G3 remediated height-915 replay | Still open; it must target binary `902773…e182` |
| Prepared fleets (built by `ae658441`) | Reused without mutation; new prepared-input manifest `c9fb32e7c3cebcf2ef16a90843c63dd96b7ed0ebc3c20ce94d2fd21707e7da42` and independent 18-reference rehash receipt `6848d49d2488cd0730efd14863c5fe446a1f31827cec98346583beee8b9cbb58` |
| Runner `15d059d1` | Gate logic unchanged; test-only successor `a3c7bea9285ab02871fd2111038764c6174b905b` passed 96 focused tests; rebuilt helper SHA-256 `ad70ca685cfaf1d0a67eb80f4805438c0e4363c8957598d1d884abd03690014a` embeds `a3c7bea9` / `release` |
| Both failed campaigns' artifacts | Preserved, closed, never resumed or relabeled |

## Completion evidence

- [x] Source `a92bb085` is pushed to `origin/main`; its implementation changes
      only the no-outbox branch and adds the five required owner fixtures.
- [x] `cargo fmt --all -- --check` passed.
- [x] `cargo test -p postfiat-node completed_index_tests --locked` passed 15
      tests with one intentional manual release check ignored.
- [x] `cargo test -p postfiat-node certified_send --locked` passed 35 tests
      with one intentional manual release check ignored.
- [x] The release 1,024-tombstone proposer-rotation check passed: 2.064 ms
      resume, zero retained payload reads/hashes, one bounded index read, and
      2.020 ms proposer/peer delta.
- [x] Runner, packager, and independent-verifier tests passed 96 tests on
      pushed runner `a3c7bea9`; its only change from the gate-logic commit is
      the accepted eager-migration fixture.
- [x] The compatible six-validator rollback, 69-case tamper/crash matrix, and
      two read-only `--verify-only` owner tests passed against the frozen
      candidate.
- [x] The prepared-input derivation and a separate read-only rehash of all 18
      referenced files/directories passed. No measurement process was started.

Per the milestone time controls, only the focused certified-send and runner
suites were required. The full workspace and Orchard suites were deliberately
not run: this change does not cross an Orchard boundary, and the accepted spec
explicitly excludes those broad gates.

## Boundaries

- The operator explicitly accepted this contract and authorized its offline
  implementation on 2026-08-29. That authorization covers the candidate fix,
  focused verification, source/binary freeze, and binary-sensitive local
  evidence refresh only. It does **not** authorize a performance campaign,
  devnet action, or deployment. Per the operator's standing instruction, no
  Task Node workflow is used.
- Offline only: no devnet, deployment, fleet, height-924, or service action.
- No vote-lock, `redb`, timing-gate, or matrix changes. The 5+5+5 matrix and
  all gate thresholds are out of scope here.
- Private failed-run directories remain untouched: never commit, publish,
  delete, or relabel.

## Acceptance for the review of this spec

- [x] The locked contract sentence is accepted as the durable invariant.
- [x] The state table for the no-outbox branch is accepted, including keeping
  both fail-closed guards.
- [x] The step-3 runner fixture and rebinding decision is recorded at
  `a3c7bea9`; gate logic is unchanged.
- [x] The identity-refresh table is accepted as the complete list of
  invalidated evidence.
