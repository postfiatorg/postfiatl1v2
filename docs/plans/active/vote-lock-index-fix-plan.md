# Vote-Lock Index Fix: Executable Implementation Plan

**Status:** Implemented and verified locally — handoff and commit in progress
**Date:** 2026-08-28
**Baseline:** `main` at `1f478e0c473de42ecf43b4dd0925893de8f181ed`
**Motivating failure:** [Storage G4 structural vote-path failure](../../handoffs/2026-08-28___postfiatchad__storage_g4_structural_vote_path_failure.md)
**Tracked by:** [Storage scaling milestone](storage-scaling-milestone.md)

## BLUF for the executing agent

The G4 storage campaign failed because block-vote signing performs an
O(chain-history) scan. Before every vote, every validator opens
`<data_dir>/block_proposal_vote_locks/`, reads, and JSON-decodes **every**
historical anti-equivocation lock file (4,999 files / ~21 MiB per validator at
height 5,000) just to find locks for the *current* slot. Measured effect:
consensus-round p95 grew 2.808× from height 50 to height 5,000 against a
required ratio of ≤1.10, with R² > 0.99 on the two vote stages.

The fix: the lock file path is **already a deterministic function of the
slot** — `block_proposal_vote_lock_path` derives the exact filename from
(chain, height, view, validator). Replace the per-vote directory scan with an
O(1) derived-path lookup, and move handling of legacy locks at non-derived
filenames into a one-time, resumable, fail-closed migration of the lock
directory. Add per-vote bounded-work counters so the gate is directly
measurable. Do not change what constitutes a conflict; change only how the
existing lock is found.

This is consensus-critical signer-safety code. Read the "Hard invariants"
section before touching anything.

## Baseline code map (all line numbers at the pinned commit)

Before this implementation, all lock logic lived in
`crates/node/src/block_finality.rs`:

| Item | Location |
| --- | --- |
| `BLOCK_PROPOSAL_VOTE_LOCK_DIR` = `"block_proposal_vote_locks"` | line 1 |
| `BLOCK_PROPOSAL_VOTE_LOCK_SCHEMA_V1` = `postfiat.block_proposal_vote_lock.v1` | line 2 |
| `BLOCK_PROPOSAL_VOTE_LOCK_SCHEMA` = `postfiat.block_proposal_vote_lock.v2` | line 3 |
| `struct BlockProposalVoteLock` (schema, chain_id, genesis_hash, protocol_version, block_height, view, validator, proposal_hash) | lines 6–15 |
| `create_block_vote_for_target_with_timings` — the signing path; calls `reserve_block_proposal_vote_lock` and records `vote_lock_reservation_ms` | ~line 2410 |
| `block_proposal_vote_lock_path` — deterministic path derivation | line 2554 |
| `reserve_block_proposal_vote_lock` | line 2585 |
| `validate_prior_block_proposal_vote_locks` — **the O(n) scan to remove** | line 2619 |
| `block_proposal_vote_lock_binds_view` | line 2661 |
| `write_new_block_proposal_vote_lock` — atomic temp-write + `hard_link` | line 2670 |
| `read_block_proposal_vote_lock_file` | line 2706 |
| `validate_block_proposal_vote_lock` | line 2710 |
| `mod block_proposal_vote_lock_tests` | line 2752 |

Related surfaces:

- `crates/node/src/node_types.rs:2299` — the baseline location of
  `BlockVoteCreationTimingReport`. The implementation extracts that struct to
  `node_types_block_vote_timing.rs` and retains schema
  `postfiat.block_vote_creation_timing.v1`.
- `crates/node/src/tests/consensus_history.rs:2388` —
  `cross_view_vote_and_legacy_lock_migration_fail_closed`. This test writes a
  v1-schema lock at the **arbitrary filename** `1.0.legacy-v1.json`, deletes
  the derived-path lock, and requires the conflicting proposal to still be
  rejected with error text containing
  `conflicting block proposal vote already recorded`. This is the load-bearing
  reason the full scan currently exists. The test must keep passing; the
  arbitrary-filename lock must now bind via migration instead of a per-vote
  scan.
- `crates/node/src/transport_runtime.rs:2975–3167` — local and remote vote
  fan-out and join. **Do not modify.** Both stages call the same signing path,
  so fixing `block_finality.rs` fixes both measured regressions.

Path derivation detail (line 2554): Consensus v2 active at the height →
domain `postfiat.block_proposal_vote_lock_path.v3`, material
`{height}:{view}:{validator}`, filename `{height}.{view}.{lock_id}.json`.
Legacy → domain `postfiat.block_proposal_vote_lock_path.v2`, material
`{height}:{validator}`, filename `{height}.{lock_id}.json`. Activation is
fixed by genesis, so the derivation for a given height never changes for a
given chain. Only v2/v3 path domains exist in the shipped source; v1 refers to
a lock **schema**, not a path derivation, and v1-schema files may sit at
arbitrary filenames.

## Hard invariants — violating any of these is a failed execution

1. **Signer safety is unchanged.** A durable lock must exist on disk before
   the ML-DSA signature is produced. Keep the existing order inside
   `create_block_vote_for_target_with_timings`: reserve lock, then build
   message, then sign. Keep the atomic temp-write + `hard_link` reservation
   and the validate-on-`AlreadyExists` path byte-for-byte in behavior.
2. **Conflict semantics are unchanged.** Legacy protocol: one proposal per
   (height, validator) across all views. Consensus v2: one proposal per
   (height, view, validator); a verified timeout view gets an independent
   lock. Same slot + same proposal hash is idempotent `Ok`. Same slot +
   different proposal hash is `io::ErrorKind::AlreadyExists` with the existing
   `conflicting block proposal vote already recorded ...` message.
3. **Fail closed everywhere.** Malformed lock file, unreadable directory,
   marker bound to a different chain/genesis/protocol, or two locks for the
   same slot with different proposal hashes discovered during migration → the
   node must refuse to sign. No panics on untrusted input; contextual
   `io::Error`s only.
4. **No historical enumeration in the signing path.** After the one-time
   migration marker exists, a vote reservation may read only: the marker file
   plus the derived lock path(s) for the current slot. Bounded constant work
   regardless of chain height.
5. **No consensus-byte changes.** Lock files are node-local safety state, not
   signed protocol bytes. Do not change vote/certificate encodings, quorum
   logic, proposer rotation, or anything in `crates/types`.
6. **Determinism discipline.** No local time, randomness, or unordered
   iteration influencing any decision. (The existing temp-file name already
   uses time/pid for uniqueness only — that is acceptable; do not extend it.)

## Step 0 — Update the milestone first

Per the motivating handoff, update
`docs/plans/active/storage-scaling-milestone.md` **before** implementation so
"campaign complete" cannot be read as "candidate qualified". Add to the status
narrative:

- The 2026-08-28 prepared-input G4 measurement campaign completed
  (report SHA-256 `88502bca7aaa4e576e5e9684b3d9b72d8c1b66e24b6c6c8e746f11807ac7eabb`)
  and the candidate **failed** release: consensus-round p95 ratio 2.808 and
  wallet-to-finality p95 ratio 2.762 versus required ≤1.10;
  `no_positive_linear_height_relationship` false; `evidence_eligible: false`;
  `PUBLIC TESTNET BLOCKED`.
- The redb store's own per-window gates passed; the residual unbounded path is
  the vote-lock full-directory scan in `crates/node/src/block_finality.rs`.
- The next bounded work is this plan; a corrected candidate must re-run the
  unchanged 5+5+5 G4 matrix.

Do not commit the unrelated untracked files in the working tree
(`docs/security/*.md`, the handoff) unless separately instructed; the handoff
document itself should be committed with this work if it is still untracked.

## Step 1 — O(1) reservation path

The vote-lock owner is extracted to `crates/node/src/vote_locks.rs` so
`block_finality.rs` remains below the repository's 5,000-line ceiling.
`create_block_vote_for_target_with_timings` still reserves the durable lock
before building or signing the ML-DSA message.

Every reservation:

1. takes the cross-process mutation lock
   `block_proposal_vote_locks/.vote_lock_index_mutation.v1`;
2. reads and validates the chain-bound migration marker, or performs Step 2;
3. computes the one derived path for (height, view, validator);
4. reads only that path, if present, and applies the existing v1/v2 schema,
   target, view, and proposal-hash validation;
5. retains the existing atomic temp-write plus `hard_link` reservation and
   validate-on-`AlreadyExists` second line; and
6. returns a `VoteLockWorkReport` to the timing report.

After the marker exists, the signing path never enumerates the lock directory.
A successful new reservation examines at most the marker and missing derived
path; an idempotent/racing reservation may re-read the derived path and remains
bounded to three examined paths.

## Step 2 — serialized, deterministic, two-phase migration

**Marker:** `<data_dir>/block_proposal_vote_locks/.vote_lock_index_state.v1`.
It has no `.json` extension, so the old scanner ignores it. Its exact JSON
binding is schema `postfiat.block_proposal_vote_lock_index_state.v1`,
`chain_id`, `genesis_hash`, `protocol_version`, and ordered path domains
`["v2","v3"]`. A present but malformed, unknown-schema, wrong-chain, or
wrong-domain marker fails closed; it never silently triggers remigration.

The mutation lock uses Unix `flock` and covers marker validation, migration,
current-slot validation, and durable reservation. Unsupported cross-process
locking fails closed. This serializes simultaneous first reservations across
processes; the existing `hard_link` collision check remains the safe boundary
against a concurrently running old binary.

When the marker is absent, migration is deterministic and split so pre-existing
conflicts cannot be partially rewritten:

**Phase 1 — read-only preflight**

1. Enumerate regular `.json` files, sort their paths, and parse them through
   the bounded JSON reader.
2. Validate v1/v2 schema plus chain, genesis, protocol, validator, and proposal
   fields.
3. Compute the derived path from each record's content and group by derived
   slot.
4. Reject every group containing more than one proposal hash before changing
   any file. Same-proposal v1/v2 duplicates are equivalent; legacy-height views
   collapse onto the height-wide derived path.
5. A new empty directory defers the marker until its first durable JSON lock
   exists. This preserves the existing legacy-migration regression sequence
   without introducing a historical scan after a completed migration.

**Phase 2 — re-home only after clean preflight**

1. For each sorted group, keep an existing derived record or hard-link the first
   sorted source to the derived path. An `AlreadyExists` collision is decoded
   and must bind the same slot and proposal.
2. Sync the directory, then re-read and validate every derived path before
   deleting anything.
3. Re-read every misplaced source and require it to equal its preflight value;
   only then remove it.
4. Sync the directory again and atomically write the marker.

A crash before cleanup leaves the original plus a safe derived hard link; the
next marker-absent run preflights both and resumes. A crash during cleanup leaves
at least the synced derived path and also resumes. No marker is written until
every derived record validates and cleanup completes.

An old binary continues to write derived v2/v3 paths and scans only `.json`
files. During migration it may observe equal duplicates or the derived record,
both of which preserve its historical conflict check. A filesystem that cannot
support the already-required same-directory hard links fails closed.

**Operator recovery boundary:** malformed records, wrong bindings, ambiguous
marker state, or conflicting same-slot proposal hashes stop signing and preserve
the files. Operators may copy the complete directory for diagnosis, but must
not delete, rewrite, or choose between conflicting records without a separate
forensic signer-safety procedure.

## Step 3 — Direct bounded-work instrumentation

In `crates/node/src/node_types_block_vote_timing.rs`, extend
`BlockVoteCreationTimingReport` with additive, serde-defaulted fields while
keeping schema string `postfiat.block_vote_creation_timing.v1`. A compatibility
test removes the new fields from an otherwise complete legacy report and proves
they deserialize to zero/false:

```rust
#[serde(default)]
pub vote_lock_files_examined: u64,
#[serde(default)]
pub vote_lock_bytes_decoded: u64,
#[serde(default)]
pub vote_lock_migration_performed: bool,
```

Populate them in `create_block_vote_for_target_with_timings` from the
reservation's `VoteLockWorkReport`. Normal post-migration operation must
report `vote_lock_files_examined <= 3` (marker + derived path + possible
`AlreadyExists` re-read) at any height.

## Step 4 — Tests

Move the two baseline tests into the extracted `vote_locks.rs` owner while
preserving their behavioral assertions:
`activated_consensus_v2_lock_rejects_same_view_equivocation_but_allows_timeout_view`
and `legacy_lock_remains_height_wide_across_views`. Add scenario-named tests:

1. `same_slot_same_proposal_reservation_is_idempotent` — reserve twice with
   identical (slot, proposal_hash) → both `Ok`, one lock file.
2. `conflicting_lock_still_binds_after_restart` — reserve, drop the store,
   build a fresh `NodeStore` over the same dir, reserve a different
   proposal_hash for the same slot → `AlreadyExists`.
3. `legacy_arbitrary_filename_lock_is_migrated_and_binds` — write a
   v1-schema lock at a non-derived filename (mirror consensus_history's
   `1.0.legacy-v1.json` construction), no marker; reserve a conflicting
   proposal → migration re-homes the file and the reservation fails with the
   existing conflict message.
4. `migration_conflicting_locks_for_same_slot_fail_closed` — two lock files
   whose contents map to one slot with different proposal hashes → migration
   error, no signature-path success, neither file deleted.
5. `interrupted_migration_resumes` — simulate the crash window: derived copy
   present *and* misplaced original present, no marker → second run removes
   the duplicate, writes the marker, and reservation proceeds.
6. `truncated_lock_file_fails_closed` — truncated JSON at the derived path →
   reservation error, no lock overwrite.
7. `marker_bound_to_other_chain_fails_closed` — valid-shaped marker with a
   different genesis_hash → reservation error.
8. `vote_lock_work_is_bounded_with_large_history` — seed ≥ 2,000 lock files
   for other heights at their derived paths plus the marker; reserve for a
   new slot; assert `vote_lock_files_examined <= 3` and
   `vote_lock_migration_performed == false`.
9. `malformed_marker_fails_closed_without_remigration` — corrupt marker
   content refuses signing and remains untouched.
10. `concurrent_first_reservations_serialize_and_preserve_one_lock` — two
    simultaneous conflicting first reservations yield one success, one
    `AlreadyExists`, and one durable JSON lock.
11. `equivalent_v1_and_v2_legacy_duplicates_canonicalize` — same-proposal
    duplicates across supported schemas migrate to one height-wide lock.
12. `mixed_binary_marker_is_ignored_and_v1_derived_lock_binds` — the marker is
    invisible to the old `.json` filter and a v1 record at the derived path
    remains binding.

Integration: run the existing
`cross_view_vote_and_legacy_lock_migration_fail_closed` in
`crates/node/src/tests/consensus_history.rs` unchanged — it must pass. If its
assertions inspect lock-directory contents, adjust only what the migration
legitimately changes (the legacy file now sits at its derived path), never
the required error text.

## Step 5 — Verification gates

Run in order; all must pass from a clean tree:

```bash
cargo test -p postfiat-node vote_locks --locked
cargo test -p postfiat-node cross_view_vote_and_legacy_lock_migration_fail_closed --locked
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

(Use the actual node package name from `crates/node/Cargo.toml` if it differs.)

Then run the ignored local bounded-work spot check in release mode:

```bash
cargo test -p postfiat-node --release \
  release_spot_check_emits_bounded_work_with_5000_lock_history \
  --locked -- --ignored --nocapture --test-threads=1
```

It creates and removes a throwaway data directory, seeds exactly 5,000
historical locks, produces a real signed vote, prints the complete timing report,
and asserts bounded `vote_lock_files_examined` with no migration. The observed
2026-08-28 release result was two examined paths, 312 decoded bytes, no
migration, and 0.118138 ms in `vote_lock_reservation_ms`.

## Explicit non-goals — do not do these

- Do **not** re-run the G4 measurement campaign in this task. That is a
  separate, operator-scheduled run of the unchanged 5+5+5 matrix with one
  clean candidate binary, gated on ratios ≤1.10 and bounded vote-lock work.
- Do **not** move vote locks into redb or any database. The filesystem
  derived-path scheme *is* the index; changing the durability substrate of
  signer-safety state is out of scope.
- Do **not** touch `transport_runtime.rs` vote fan-out/join, quorum logic,
  timeout certificates, proposer rotation, or anything in `crates/types`.
- Do **not** contact the controlled devnet, copy validator directories
  (G3 height-924 replay has its own authorization gate), or deploy anything.
- Do **not** publish or commit anything from
  `/home/postfiatchad/repos/postfiat-storage-g4-measurement-1f478e0c-ae658441-v1`
  (contains validator private material).

## Exit criteria

- [x] Milestone updated with the failed G4 result (Step 0).
- [x] Per-vote lock work is O(1): no `read_dir` of the lock directory
      anywhere in the post-marker signing path.
- [x] One-time migration is resumable, idempotent, concurrency-safe, and
      fail-closed, with a durable chain-bound marker.
- [x] All existing lock and consensus-history tests pass unmodified in
      behavior; all twelve new scenario tests and the timing compatibility test
      pass.
- [x] `vote_lock_files_examined` / `vote_lock_bytes_decoded` /
      `vote_lock_migration_performed` reported per vote and bounded.
- [x] fmt, check, clippy `-D warnings`, and full workspace tests pass.
- [ ] Handoff written per repo convention stating: the changed invariant
      (scan → indexed lookup + one-time migration), surfaces touched,
      commands run, tests omitted (G4 re-run, devnet), and that
      qualification, G3, G5, and deployment remain blocked pending their own
      gates.
