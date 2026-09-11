# Storage and snapshots review — 2026-09-11

Status: findings recorded; P1/P2 repairs pending; no release or deployment authorized

Reviewed checkout: `c25b3389d5c221ff1c23e700d53460003cc50b2a`

## Scope and method

This fresh-eyes pass reviewed `crates/storage` in full and traced snapshot
export, snapshot import, finalized-checkpoint verification, storage migration,
ordered-commit recovery, and writer-lease consumers in `crates/node`.
The review emphasized crash consistency, partial writes, tampered or truncated
restore inputs, writer fencing, migration rollback, and growth bounds. Frozen
deployment and benchmark evidence was read only.

The pre-repair focused baselines were green:

- `cargo test -p postfiat-storage --locked`: **88 passed, 2 ignored** plus
  **1 process-crash integration test passed**.
- `cargo test -p postfiat-node snapshot --lib --locked`: **22 passed**.
- `cargo test -p postfiat-node lifecycle_checkpoint --lib --locked`:
  **7 passed**.

## Findings

1. **P2 / reproduced defect — rejected snapshot imports publish partial
   destination state.**

   `import_snapshot_with_basis` creates the final destination in
   [`batch_snapshot.rs`](../../crates/node/src/batch_snapshot.rs) before it
   authenticates each declared file and before restored-state replay at lines
   3241–3399. An error after any write leaves the final path present, so the
   required no-overlay check rejects a corrected retry. The lifecycle wrapper
   compounds this at
   [`lifecycle_checkpoint.rs`](../../crates/node/src/lifecycle_checkpoint.rs)
   lines 448–528 by importing validators directly into their final directories
   one at a time.

   Concrete failure scenario: a signed two-validator lifecycle checkpoint has
   an intact top-level manifest but validator-1's `ledger.json` is truncated
   in transit. Validator-0 is fully restored into
   `target_root/validator-0`; validator-1 then fails its file hash check and
   the command returns failure. The target now looks like a partially restored
   fleet, and rerunning after replacing the bad file refuses to overlay the
   already-created validator-0 path.

2. **P2 / reproduced defect — FastSwap WAL growth is unbounded and bounded
   file checks occur after whole-file allocation.**

   [`fastswap_store.rs`](../../crates/storage/src/fastswap_store.rs) bounds
   individual WAL records to 1 MiB, but `append_synced_record` at lines
   1470–1486 never fences total WAL length. `read_records` at lines 1630–1638
   uses `read_to_end` without first checking metadata. The snapshot and vote
   artifact readers at lines 1019–1027 and 1531–1541 similarly call
   `fs::read` before applying their size limits. Compaction occurs only when a
   caller explicitly anchors a checkpoint, so neither normal append nor reopen
   establishes a total bound.

   Concrete failure scenario: sustained valid FastSwap activity without a
   primary checkpoint grows the WAL until a validator restart allocates the
   entire file before checking any record. A filled or sparse oversized WAL
   can exhaust memory and kill the process before MAC or sequence validation;
   repeated restarts reproduce the same availability failure.

3. **P3 / reproduced defect — ordered-history index rebuild removes the last
   usable generation before publishing its replacement.**

   [`ordered_history.rs`](../../crates/storage/src/ordered_history.rs) lines
   270–274 removes the current derived index directory and only then renames
   the completed build directory into place. The summary is published after
   that rename.

   Concrete failure scenario: a crash or power loss lands after
   `remove_dir_all(target_dir)` and before `rename(build_dir, target_dir)`.
   Bounded-JSONL membership reads then have neither the old generation nor a
   published replacement and require an operator rebuild. The deployed
   transactional backend does not use this comparison-only index, so this is
   recorded as P3 and is not repaired in this campaign.

4. **P3 / reproduced defect — legacy receipt materialization performs the same
   atomic state write twice.**

   [`lib.rs`](../../crates/storage/src/lib.rs) lines 453–455 invokes
   `write_json(RECEIPTS_FILE, receipts)` twice before removing the append log.

   Concrete failure scenario: every legacy receipt compaction serializes,
   writes, fsyncs, renames, and directory-syncs the same 256 MiB-bounded state
   twice. This doubles local I/O and enlarges the time window before append-log
   retirement without improving crash semantics. The path is comparison-only
   on the deployed transactional configuration, so it is recorded as P3 and
   is not repaired.

5. **P2 / evidence gap — the deployed snapshot repair has no post-repair
   fleet export receipt and the current-state page still describes the old
   source defect as open.**

   [`chain-state-current.md`](../status/chain-state-current.md) lines 83–92
   says the block-924 finalized-checkpoint export defect remains open.
   Repository commit `353156c3` repaired that exact certificate-registry replay
   path, is an ancestor of the deployed base `707e006f`, and retains exact
   regressions in
   [`validator_registry_continuation_tests.rs`](../../crates/node/src/tests/validator_registry_continuation_tests.rs).
   No permitted evidence shows a successful signed finalized-checkpoint export
   from the running fleet after that repair.

   Concrete failure scenario: an operator reading the canonical current-state
   page cannot distinguish “source repair is deployed” from “fleet backup
   usability is proven.” They may unnecessarily bypass signed snapshots or,
   conversely, assume ancestry alone proves the backup path. A host-side export
   would write files and is prohibited by this campaign, so the remaining
   operational proof must stay explicit.

6. **P1 / reproduced defect — a torn FastSwap WAL suffix is ignored but not
   truncated before later durable appends.**

   [`fastswap_store.rs`](../../crates/storage/src/fastswap_store.rs) lines
   1642–1671 treats an incomplete final length, payload, or MAC as a safe torn
   append and returns the verified prefix. The store retains that suffix, and
   `append_synced_record` at lines 1470–1486 always appends at the physical end
   of the file.

   Concrete failure scenario: a process dies midway through a WAL record. On
   restart, replay ignores the incomplete tail and the validator accepts a new
   vote, appends and syncs that complete record behind the tail, and emits its
   signature. On the next restart, the old length prefix consumes bytes from
   the later record and fails its MAC, or the scanner again stops before the
   later record. The signature escaped but its safety record is not replayable,
   violating the durable-before-signing invariant.

## Areas with no findings

- Transactional finalized-block updates use immediate durable redb
  transactions, validate the expected tip and canonical append, and retain
  old-or-new crash semantics in both logical cut-point and process-kill tests.
- The process-shared transactional writer lease holds only weak registry
  references, serializes same-process opens, retries cross-process contention
  for a bounded interval, and fails closed without mutation when exhausted.
- Snapshot manifests allowlist an exact versioned file set, bind file lengths
  and hashes, exclude signer material, and require the trusted ML-DSA publisher
  signature for signed imports.
- The block-924 registry replay defect is covered in current source by both
  same-subject and unrelated-subject rollback/return histories; this review
  found no surviving source-level reproduction.
- Verify-only transactional migration opens both source and candidate read
  only, rejects pending recovery, and validates logical source/candidate
  manifests plus the canonical export before reporting success.

## Repair boundary

Findings 1, 2, and 6 require owner-level repairs and regressions: restore into
a private sibling staging directory and publish only after all verification
succeeds; apply pre-allocation file limits and a total FastSwap WAL fence; and
durably truncate every unauthenticated torn WAL suffix before allowing another
append. Finding 5 requires a documentation correction that separates deployed
source ancestry from missing live export evidence. Findings 3 and 4 remain
recorded under the campaign's P3 rule.

None of these repairs changes a consensus rule, state-transition result, signed
bytes, or on-disk format, and none authorizes deployment or live activation.
