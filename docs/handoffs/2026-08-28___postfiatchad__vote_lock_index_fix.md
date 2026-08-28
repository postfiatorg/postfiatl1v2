# Vote-lock index fix

- **Operator:** PostFiatChad (`postfiatchad`)
- **Date:** 2026-08-28 UTC

## BLUF

Commit `be4c7f44` implements and locally verifies the
[vote-lock index plan](../plans/active/vote-lock-index-fix-plan.md). Normal
block-vote signing no longer enumerates and decodes every historical
anti-equivocation lock: after a chain-bound migration marker exists, it reads
only the marker and the one deterministic current-slot path. Legacy misplaced
locks are re-homed once under a serialized, resumable, fail-closed migration.
The release-mode check with 5,000 historical locks examined two paths, decoded
312 bytes, performed no migration, and reserved the new lock in 0.118138 ms.
This is an offline source fix, not storage qualification, deployment, or public
testnet authorization.

## Current state

### Repository and authority boundary

- Implementation commit: `be4c7f44` on `main`, based on
  `1f478e0c473de42ecf43b4dd0925893de8f181ed`.
- The implementation commit is local at this handoff boundary and has not been
  pushed or deployed.
- No controlled-devnet query, validator-data copy, service action, deployment,
  or live fleet probe was performed. The dated network evidence remains in
  [Current State](../status/chain-state-current.md).
- No Task Node or parallel-agent workflow was used.
- The unrelated untracked auditor inventories under `docs/security/` were
  preserved and excluded from the implementation commit.

### Changed invariant

Before `be4c7f44`, every validator scanned and JSON-decoded the complete
`block_proposal_vote_locks/` directory before every proposal vote. At height
5,000 that meant 4,999 historical files and roughly 21 MiB per validator per
vote.

After `be4c7f44`:

1. every reservation takes the Unix `flock` mutation guard;
2. a valid
   `.vote_lock_index_state.v1` marker binds schema, chain, genesis, protocol,
   and the ordered v2/v3 path-domain set;
3. normal signing computes and reads only the deterministic current-slot path;
4. the existing atomic write plus `hard_link` reservation remains the durable
   conflict boundary; and
5. reservation still completes before the ML-DSA message is signed.

A missing marker permits exactly one directory enumeration for migration.
Migration sorts all JSON paths, preflights every record and every same-slot
conflict before mutation, creates and syncs all canonical hard links, rechecks
them, removes only unchanged misplaced sources, syncs again, and writes the
marker last. Crashes before or during cleanup resume from the safe derived
copies. Malformed locks, conflicting proposal hashes, malformed or wrong-chain
markers, and unsupported locking fail closed.

The operator recovery boundary is unchanged: copy the complete lock directory
for diagnosis, but do not delete, rewrite, or choose between conflicting signer
records without a separate forensic signer-safety procedure.

### Surfaces changed

- `crates/node/src/vote_locks.rs` owns deterministic lookup, migration,
  mutation locking, work accounting, and the lock regression tests.
- `crates/node/src/block_finality.rs` retains durable-reservation-before-signing
  order and copies the work counters into the vote timing report.
- `crates/node/src/node_types_block_vote_timing.rs` adds serde-defaulted
  `vote_lock_files_examined`, `vote_lock_bytes_decoded`, and
  `vote_lock_migration_performed` fields while retaining timing schema v1.
- `crates/node/src/lib.rs` wires the extracted owner; the signing file is now
  below the 5,000-line ceiling.
- `crates/node/src/tests/atomic_swap_consensus.rs` now writes the canonical
  empty `ordered_batches.json` in its synthetic fixture. The first workspace
  run exposed that pre-existing incomplete fixture in three tests; all three
  pass after the fixture-only correction.
- Two semantics-preserving workspace-Clippy corrections use
  `is_multiple_of(2)` in the Cobalt oracle and place the E1 harness test module
  after `main`.
- The [storage milestone](../plans/active/storage-scaling-milestone.md) now
  records the completed failed G4 result and the corrected critical path.

### Verification

All commands below ran against the final source using the repository Zig
compiler/linker wrappers where Cargo compilation required them.

| Gate | Result |
| --- | --- |
| `cargo test -p postfiat-node vote_locks --locked` | PASS: 14 passed; the explicit release spot check remained ignored in this focused debug run |
| `cargo test -p postfiat-node cross_view_vote_and_legacy_lock_migration_fail_closed --locked` | PASS: unchanged consensus-history behavior |
| `cargo test -p postfiat-node block_vote_timing_tests --locked` | PASS: legacy timing JSON defaults new fields to zero/false |
| Two atomic-swap quote tests plus required-parent regression | PASS after adding the missing empty ordered-batch fixture |
| `cargo test -p postfiat-node --release release_spot_check_emits_bounded_work_with_5000_lock_history --locked -- --ignored --nocapture --test-threads=1` | PASS: 5,000 history locks; 2 paths; 312 bytes; no migration; 0.118138 ms reservation |
| `cargo fmt --all -- --check` | PASS |
| `cargo check --workspace --all-targets --locked` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS with exit code 0; node library 329 passed / 3 intentionally ignored; node binary 128 passed / 2 intentionally ignored; all remaining workspace and doc tests completed successfully |
| `git diff --check` and staged diff check | PASS |

The ambient `cc` command is raw Zig and rejected Cargo's `-m64` flag during
the first focused attempt. All recorded passing compilation commands therefore
used `scripts/zig-cc`, `scripts/zig-ar`, and
`POSTFIAT_ZIG=/home/postfiatchad/.local/zig-0.17.0-dev.1857/zig`.

## Next decision or action

Do not rerun the old failed output, contact the devnet, or describe this commit
as qualified. The next bounded qualification sequence is:

1. freeze and hash one corrected source tree and release binary containing
   `be4c7f44`;
2. refresh G1 provenance, binary-bound G2 safety evidence, and the locally
   available height-915 G3 replay;
3. make the corrected G4 verifier require the new vote-lock work counters;
4. build one hash-bound height-5,000 prepared input with the corrected binary and
   run the unchanged 5+5+5 G4 matrix under its existing four-hour measurement
   budget;
5. require both height ratios at or below 1.10, no material positive
   height/latency relationship, literal receipts, six-validator convergence,
   bounded storage work, and bounded vote-lock work; and
6. complete height-924 G3 only after a custodian and separate read-only copy
   authorization exist, then close G5 before any deployment decision.

The corrected G4 campaign, controlled-devnet work, height-924 replay, G5 packet,
G6 clone rehearsal, deployment, and public-testnet authorization were
intentionally not performed here and remain blocked by their own gates.

## References

- [Vote-lock index implementation plan](../plans/active/vote-lock-index-fix-plan.md)
- [Storage scaling milestone](../plans/active/storage-scaling-milestone.md)
- [Motivating G4 structural failure](2026-08-28___postfiatchad__storage_g4_structural_vote_path_failure.md)
- [Storage scaling fix specification](../architecture/storage-scaling-fix-spec.md)
- [Current recorded network state](../status/chain-state-current.md)
