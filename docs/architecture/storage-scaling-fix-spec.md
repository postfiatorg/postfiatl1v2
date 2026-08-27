# Storage Scaling Fix Implementation Specification

**Status:** Active implementation specification — PUBLIC TESTNET BLOCKED
**Date:** 2026-08-26
**Decision owner:** Post Fiat
**Source baseline:** `b40c31a55dbe6b59330621de2cb409eb59461c0e`
**Derived from:** [Storage Scaling and Bounded Finality Research Specification](storage-scaling-research-spec.md)
**Tracked by:** [Storage Scaling and Bounded Finality Milestone](../plans/active/storage-scaling-milestone.md)

## Purpose

Block finality on the controlled devnet gets slower as the chain gets longer.
The deployed JSON/JSONL storage path repeatedly reads and verifies growing
history during each append, and the legacy proposal path reconstructs the full
ordered-batch list. The E4 campaign measured the result: p95 consensus time was
about 1.66 seconds in the first 50 rounds and about 14.9 seconds by round 500,
with an approximately linear relationship to height.

This specification defines the implementation that removes chain-length work
from proposal, validation, and finalized commit. It turns the scored research
specification into a concrete engineering contract. It does not authorize a
live deployment or assert that the problem is already fixed.

## Required outcome

For every newly finalized block:

- proposal construction and validator reconstruction perform no full-history
  read, scan, clone, or hash;
- duplicate-batch lookup and indexed reads perform `O(log n)` work or better;
- ordered-history commitment update performs `O(1)` work;
- all durable effects commit in one atomic storage transaction;
- crash recovery returns either the complete previous height or the complete
  new height, never a mixture; and
- the amount of synchronous work does not grow linearly with chain height.

The selected release path is an embedded transactional ordered key-value or
B-tree store. The authenticated JSONL-head v2 work remains the bounded legacy
import, audit, and comparison path. The current fixed-slot bitmap proves a
bounded-work concept, but its roughly 2.10 MB read and 1.05 MB write per measured
proposal-plus-append cycle and its high debug-build latency disqualify it as the
selected store.

## Current state versus required state

| Boundary | Current source | Required release state |
| --- | --- | --- |
| JSONL append | Authenticated v2 heads avoid accepted-prefix scans and inspect at most one crash suffix | Retain for legacy import and audit; do not use growing JSONL files as the primary finality-path database |
| Ordered membership | Fixed-slot file index with a full fixed bitmap read and rewrite | Transactional ordered index with bounded page reads and writes |
| Ordered commitment | Versioned fixed-size accumulator exists | Retain the versioned accumulator and prove identical proposer, validator, commit, replay, and restart results |
| Finalized commit | Several files are coordinated through an ordered-commit journal | One database write transaction contains every durable effect for one finalized height |
| Activation | Optional genesis-only `ordered_history_v2_activation_height` | Genesis activation for new chains plus a consensus-ordered activation record for the existing chain |
| Evidence | Synthetic work counters pass through height 5,000 | Exact replay, tamper, crash, paired six-validator performance, migration, CLI, and browser gates all pass |
| Deployment | Nothing from this milestone is deployed | Separate authorization and a fleet-bound deployment receipt are required after every gate passes |

## Scope

### In scope

- the primary node store, blocks, receipts, batch archive, ordered-batch
  membership, current replicated state, chain tip, and storage metadata;
- proposal construction, local validator reconstruction, duplicate detection,
  ordered-history commitment, finalized commit, restart, catch-up, snapshot,
  import, export, pruning, and full replay;
- legacy JSON/JSONL import and exact pre-activation compatibility;
- a versioned activation path for both new chains and the existing controlled
  devnet lineage;
- storage observability, migration tooling, packet verification, and the
  read-only evidence interface; and
- release-mode six-validator measurements at heights 50, 100, 500, 1,000, and
  5,000.

### Out of scope

- changing Consensus v2 voting, quorum, locks, timeout, proposer, certificate,
  or receipt-success rules;
- changing Cobalt validator-trust authority or treating Cobalt as block
  finality;
- changing transaction, block, receipt, or certificate bytes below the
  activation boundary;
- putting database pages, file paths, host-local keys, or backend-specific
  hashes into consensus state;
- allowing an index, checkpoint, snapshot, or packet manifest to replace full
  replay; and
- public-testnet deployment, mainnet readiness, or a finality service-level
  claim before the completion gates pass.

## Architecture

### 1. Typed storage boundary

Define a narrow storage interface in `crates/storage` and make node code depend
on it instead of individual JSON and JSONL files. The interface must expose:

- a consistent read snapshot bound to one finalized tip;
- point reads for current state, blocks, receipts, archived batches, and
  ordered-batch membership;
- deterministic ordered range reads for replay, export, and RPC history;
- the current ordered-history count and accumulator;
- an atomic `commit_finalized_block` operation with an expected-parent check;
- snapshot, import, export, integrity-check, and deterministic rebuild
  operations; and
- explicit work counters for records, bytes, pages, transactions, and fsync
  time.

The interface must not expose backend iteration order to consensus code.
Consensus callers request an explicit table, key range, and canonical order.

### 2. Transactional data model

The selected embedded store must provide serializable read/write transactions,
crash-safe atomic commit, ordered keys, and deterministic backup or export.
Physical database layout is local implementation detail. Logical tables are:

| Table | Canonical key | Value |
| --- | --- | --- |
| `meta` | fixed versioned names | chain/genesis/protocol binding, storage format, finalized tip, counts, ordered-history accumulator, activation state |
| `blocks_by_height` | unsigned height in canonical big-endian form | canonical `BlockRecord` bytes |
| `block_height_by_hash` | canonical block hash bytes | height |
| `receipts_by_id` | canonical transaction/receipt ID | canonical receipt bytes and finalized height |
| `batch_archive` | batch kind plus batch ID | canonical archived payload and payload hash |
| `ordered_by_id` | batch ID | one-based ordinal and finalized height |
| `ordered_by_ordinal` | unsigned ordinal in canonical big-endian form | batch ID |
| `current_state` | fixed domain name | canonical ledger, governance, shielded, bridge, registry, and required current-state bytes |
| `history_indexes` | versioned index key | rebuildable account/history lookup values |

Every key and value has a closed schema, a size bound, a version, and a domain
separator. Height and ordinal encodings must preserve numeric order. Unknown
versions and non-canonical encodings fail closed before mutation.

### 3. One finalized-height transaction

`commit_finalized_block` performs the following in one write transaction:

1. Read and validate the stored chain, genesis, protocol, storage version,
   finalized height, block hash, state root, ordered count, and accumulator.
2. Require the proposed commit to extend that exact tip and reject a replayed
   or already-ordered batch except for the defined idempotent recovery case.
3. Write the new current-state objects produced by deterministic execution.
4. Insert every accepted and rejected transaction receipt from the finalized
   block, preserving literal receipt status.
5. Insert the archived batch and both ordered-batch index entries.
6. Append the batch ID to the versioned ordered-history accumulator and require
   its count to equal the new finalized height semantics.
7. Insert the certified block and update the finalized tip and all counts.
8. Commit durably once.

Any validation, encoding, database, capacity, or durability error aborts the
entire transaction. After restart, the store exposes either the old tip or the
new tip. The selected path must not require the existing multi-file
ordered-commit journal for new-format commits.

### 4. Consensus commitment remains backend-independent

The database index answers local membership questions; it is not consensus
authority. Consensus state commits only to canonical protocol values:

- ordered-history commitment schema and version;
- chain ID, genesis hash, and protocol version;
- ordered-batch count; and
- the domain-separated append-only accumulator.

Below activation, state roots and historical artifacts use the legacy
ordered-batch-list encoding exactly. At and above activation, proposal
construction, validator reconstruction, commit, current-state verification,
and replay use the same fixed-size ordered-history commitment. Two nodes using
different qualifying database backends must produce identical proposals,
state roots, blocks, receipts, and certificates.

### 5. Canonical history and audit

The transactional store is the primary durable store for the selected path.
Full-archive nodes retain all canonical blocks, receipts, and batch payloads.
Retained-history nodes may prune only through the existing authenticated
checkpoint rules.

JSONL becomes a deterministic export and legacy-import format, not a required
write on the finality path. A full verifier must be able to:

- replay from genesis or an authenticated checkpoint without trusting local
  indexes or fast metadata;
- recompute every block, receipt, state root, ordered-history commitment, and
  chain tip;
- compare database history with a canonical JSONL export; and
- rebuild every derived index and produce the same logical entries and root.

Byte-identical rebuild applies to canonical exported records and manifests.
Backend page layout, compaction, and file bytes are explicitly not consensus
artifacts and are not required to be byte-identical.

## Activation and compatibility

### New chains

A new chain may declare `ordered_history_v2_activation_height` in genesis. The
field remains optional so historical genesis JSON and hashes do not change when
it is absent. The node must build and verify the new store before it reaches the
activation height.

### Existing controlled devnet

The existing genesis cannot be edited without creating a different chain. Add
a versioned, consensus-ordered storage-commitment activation record under the
existing non-validator governance authority. Cobalt does not authorize this
record because its live scope is validator registry and trust graph only.

The activation record must bind:

- schema and feature ID;
- chain ID, genesis hash, and protocol version;
- scheduling block and activation height;
- legacy and new commitment versions;
- pre-activation finalized height, block hash, state root, ordered count, and
  ordered-history accumulator;
- migration packet root and required verifier version; and
- cancellation height and reason when activation is cancelled before it takes
  effect.

It is valid only if activation is in the future, the frozen prefix matches the
locally replayed chain, and the node has a fully verified new-format store.
Scheduling changes governance state under the legacy commitment rules.
The switch to the new ordered-history state commitment occurs only at the
recorded activation height.

Every validator must run dual-capable code before scheduling. A legacy node or
a node with a missing, stale, or mismatched migrated store must stop before
voting across activation. Mixed interpretations must never form a certificate.

### Rollback

Before activation, the record may be cancelled through a consensus-ordered
governance action and validators may return to the immutable legacy store.
After activation, finalized blocks are never deleted or reinterpreted. A
software rollback must still understand and replay the activated commitment
version and resume from the same certified tip. Restoring a pre-activation data
directory after post-activation finality is forbidden.

## Migration procedure

Migration is side-by-side and offline for each node:

1. Stop the disposable clone and copy the legacy data directory to immutable,
   checksum-bound backup storage.
2. Authenticate all legacy JSON/JSONL files from genesis or the retained
   checkpoint and verify the current certified tip.
3. Build the new database in a separate generation directory.
4. Replay all canonical blocks and populate current state, receipts, archive,
   ordered indexes, and the accumulator.
5. Run a second independent logical scan and compare counts, IDs, payload
   hashes, receipts, state roots, and the current tip.
6. Atomically publish the generation pointer; never overwrite the legacy
   directory.
7. Start in legacy commitment mode, finalize and restart, then exercise the
   scheduled activation on the clone.

The rebuild command remains guarded by an explicit offline confirmation. It
must support `--output-dir`, `--expected-tip`, `--expected-state-root`, and
`--verify-only`, refuse an existing non-empty output, report required and
available disk, and emit a redaction-safe signed or checksum-bound manifest.

## Failure and adversarial requirements

The test corpus must cover the original E3 cases and all new storage states:

- truncated, padded, reordered, duplicated, omitted, or modified canonical
  history;
- wrong chain, genesis, protocol, storage, commitment, table, or key domain;
- valid but stale database generation, metadata record, tip, or accumulator;
- missing or substituted tables, pages, snapshots, exports, or generation
  pointers;
- index entry without history, history without index, conflicting ordinal and
  ID entries, and incorrect count or accumulator;
- forged receipts, archive payloads, state roots, certificates, and catch-up
  responses;
- disk full, permission loss, write failure, sync failure, process kill, and
  power-loss simulation before, during, and after transaction commit;
- restart at every injected cut, repeated recovery, and idempotent retry; and
- migration, activation, cancellation, restart, catch-up, pre-activation
  rollback, and compatible post-activation software rollback.

Every rejected case must have a stable reason code, make no durable mutation,
and leave the node unable to vote until its local state is unambiguous.

## Performance and resource gates

Use optimized release binaries and the frozen six-validator topology. At each
starting height 50, 100, 500, 1,000, and 5,000, run five independent 50-round
windows from authenticated snapshots with identical keys, inputs, host
allocation, storage medium, full-vote policy, and instrumentation.

The selected store passes only when:

- height-50 p95 `consensus_round_ms` and `wallet_to_finality_ms` are each no
  more than 110% of the frozen legacy height-50 baseline;
- height-5,000 p95 for both metrics is each no more than 110% of the selected
  store's height-50 p95;
- no material synchronous stage has a positive linear relationship with
  height after accounting for measured host variance;
- proposal, validator reconstruction, and commit report zero full-history
  records and bytes read after activation;
- indexed point operations touch `O(log n)` pages or better, and accumulator
  update is constant work;
- every round reaches literal accepted or rejected receipts as expected and all
  six validators converge on the same height, block hash, and state root; and
- CPU, RSS, disk growth, bytes read/written, page count, fsync count/time, and
  variance are published rather than hidden behind elapsed-time summaries.

A height-independent but uniformly slow store fails the first gate. A fast
cache that cannot survive restart, tamper, and exact replay also fails.

## Observability and operator interfaces

Node status and the evidence packet must report:

- storage format and schema version;
- database generation and backend identity;
- commitment version and scheduled activation height;
- finalized height, block hash, state root, ordered count, and accumulator;
- last full verification and rebuild height;
- transaction commit and fsync latency;
- records, bytes, and pages read and written by synchronous stage; and
- migration, integrity, or recovery status with stable reason codes.

Deliver both required interfaces from the same checksum-bound packet:

```bash
python -m postfiat_rpc.storage_scaling verify PACKET
```

The Python CLI must verify the manifest, source and binary identities, replay
results, performance windows, tamper matrix, migration receipts, and redaction
result without network access. The read-only browser view must consume only a
successfully verified packet and show the live/deployed/repository distinction.
Neither interface may claim that packet verification is a live fleet probe.

## Code ownership

| Boundary | Primary source |
| --- | --- |
| Typed backend and transaction implementation | `crates/storage/src/` |
| Legacy JSONL head and importer | `crates/storage/src/lib.rs` |
| Ordered-history commitment and index adapter | `crates/storage/src/ordered_history.rs` |
| Proposal and duplicate lookup | `crates/node/src/mempool_proposals.rs`, `crates/node/src/batch_snapshot.rs` |
| Atomic finalized commit and recovery | `crates/node/src/storage_commit.rs` |
| Versioned state root | `crates/node/src/state_commitment.rs` |
| Full replay, snapshots, import, export, and retained history | `crates/node/src/block_replay_wallet.rs`, `crates/node/src/history.rs`, `crates/node/src/batch_snapshot.rs` |
| Activation types and execution | `crates/types`, `crates/execution`, `crates/node` |
| Operator CLI | `crates/node/src/main_parts/`, `python/postfiat_rpc/` |
| Evidence | `benchmarks/storage-scaling/` |

Reusable storage rules belong in `crates/storage`; consensus encoding and
activation types belong in `crates/types` and their owning execution boundary;
`crates/node` remains orchestration.

## Completion gates

The problem is fixed only when all of the following are true:

- [ ] A selected transactional ordered store replaces the fixed bitmap and
      growing JSONL writes on the finality path.
- [ ] One finalized block is durable through one atomic transaction.
- [ ] Proposal construction, validator reconstruction, state commitment, and
      commit contain no post-activation full-history work.
- [ ] The 915-block quarantine archive and authenticated history through height
      924 replay exactly below activation.
- [ ] Clean build, migrated build, restart, and deterministic rebuild produce
      identical logical records, commitments, state roots, and tips.
- [ ] Every original and extended tamper and crash case rejects or recovers as
      specified without partial mutation.
- [ ] The paired six-validator performance and resource gates pass at every
      required height.
- [ ] The existing-chain activation, cancellation, migration, restart,
      catch-up, and rollback sequence passes on six disposable clones.
- [ ] Snapshot, import, export, pruning, and archive-node behavior pass.
- [ ] The Python verifier and read-only browser interface pass from the same
      checksum-bound, redaction-safe evidence packet.
- [ ] The proportional Rust, Python, replay, formatting, lint, and strict
      documentation gates pass from a clean checkout.
- [ ] A separate deployment decision records the selected source, binary,
      snapshot, packet, activation, and rollback identities.

Until every item passes, the implementation remains an undeployed development
candidate, the existing controlled-devnet observation remains the only live
evidence, and public testnet remains blocked.
