# Append Storage Slice For Latency

Status: scoped follow-up from the 2026-06-06 latency WHIP
Date: 2026-06-06
Owner: protocol/storage engineering

## Objective

Reduce transparent PFT finality tail latency by removing per-commit rewrites of
full retained `blocks.json` and `batch_archive.json`, while preserving:

- deterministic block verification;
- checkpoint/prune semantics;
- archive-window export/import and backfill;
- crash recovery;
- existing JSON state readability during migration;
- controlled-testnet live upgrade safety.

The LAT-021 benchmark shows the current remaining local 100-round p95 storage
cost after the compact commit journal:

| Stage | p95 |
|---|---:|
| `write_blocks` | `151.488653ms` |
| `write_batch_archive` | `49.320329ms` |
| account-index refresh | `26.491596ms` |
| `write_journal` | `5.86ms` |

The next durable win is therefore block/archive storage, not more certificate
work.

## Current Contract

Active files:

- `blocks.json` stores a `BlockLog { blocks: Vec<BlockRecord> }`.
- `batch_archive.json` stores a `BatchArchive { batches: Vec<BatchArchiveEntry> }`.
- `receipts.json` stores `Vec<Receipt>`.
- `ordered_batches.json` remains part of the replicated state root and must stay
  durable.
- `history_checkpoint.json` may provide the pruned prefix base height/hash/state
  for partial-history validators.

Critical readers currently call `NodeStore::read_blocks()` and
`NodeStore::read_batch_archive()`, then operate on in-memory arrays. This
includes:

- `verify_blocks`;
- `history-status`;
- archive handoff/window export/import;
- prune plan/prune/recover;
- block/certificate reconstruction;
- account transaction index refresh;
- live doctor and release gates.

Any append format must be invisible to these readers until each reader is
explicitly made append-aware or `NodeStore` provides a compatibility materialized
view.

## Proposed Format

Add append-backed retained-history files beside the existing JSON arrays:

```text
blocks.snapshot.json          # optional compact retained snapshot
blocks.append.jsonl           # one canonical JSON BlockRecord per line
batch_archive.snapshot.json   # optional compact retained snapshot
batch_archive.append.jsonl    # one canonical JSON BatchArchiveEntry per line
history_storage_manifest.json # active storage mode and offsets/checksums
```

The first implementation should keep `blocks.json` and `batch_archive.json` as
materialized compatibility files until gates prove every active reader can use
the append view. The latency path can then choose:

1. append the new block/archive entry to the JSONL logs;
2. update the manifest with previous/new byte offsets and roots;
3. defer full materialized JSON rewrites to checkpoint, prune, or operator
   compaction windows.

The manifest must be domain-bound:

```text
schema
chain_id
genesis_hash
protocol_version
checkpoint_height
checkpoint_block_hash
blocks_snapshot_hash
blocks_append_hash
archive_snapshot_hash
archive_append_hash
last_height
last_block_hash
last_batch_key
manifest_hash
```

## Crash Recovery

The existing compact `postfiat-ordered-commit-delta-journal-v1` should remain
the commit recovery object. Append storage must add a second local recovery
marker only if a block/archive append can be observed without a matching
manifest update.

Safe order:

1. write compact ordered-commit delta journal;
2. append block/archive records to temp append files or append with fsync;
3. write `history_storage_manifest.pending.json` binding expected offsets/roots;
4. publish updated append files and manifest atomically;
5. update remaining state files;
6. remove ordered-commit journal and pending storage marker.

Recovery must be idempotent:

- if the block exists and matches exactly, do not append again;
- if the archive entry exists and matches exactly, do not append again;
- if either exists but conflicts, fail closed;
- if manifest and append files disagree, fail closed unless the pending marker
  unambiguously completes the interrupted commit.

## Migration

Phase 1: compatibility append mode.

- On node start, if append files are absent, create append files from current
  `blocks.json` / `batch_archive.json` and write a manifest.
- Keep writing the old materialized JSON files on explicit compaction and
  release-gate commands, not necessarily every commit.
- `NodeStore::read_blocks()` and `read_batch_archive()` continue returning the
  materialized view. A new `read_blocks_retained_view()` can merge snapshot and
  append data for gates.

Phase 2: reader migration.

- Move `verify_blocks`, history prune/export/import, account index refresh, and
  live doctor to append-aware retained views.
- Keep a command to regenerate `blocks.json` and `batch_archive.json` for
  inspection and backwards tooling.

Phase 3: hot-path removal.

- Remove per-commit materialized rewrites from `apply_ordered_commit`.
- Keep append fsync and manifest fsync in the hot path.

## Required Gates

Local:

- unit test: append view equals legacy `BlockLog` / `BatchArchive`;
- unit test: append recovery after crash before manifest publish;
- unit test: append recovery after crash after manifest publish;
- unit test: conflicting appended block fails closed;
- unit test: conflicting archive entry fails closed;
- `status_recovers_pending_ordered_commit_delta_journal`;
- `verify-state` and `verify-blocks` on append-backed state;
- history prune plan/prune/recover after append migration;
- archive handoff/window export/verify/import after append migration;
- local 100-round latency benchmark with append hot path.

Transport/live:

- partial-outage peer-certified smoke;
- quorum-early smoke;
- live binary compatibility;
- controlled binary upgrade;
- validator doctor before/after benchmark;
- live benchmark with `CANARY_TIMEOUT_SECONDS` set so SSH/SCP stalls fail
  closed.

## Claim Boundary

This slice is a storage-layout optimization. It must not change:

- block hash input;
- certificate/vote verification;
- batch payload hash;
- replicated state root;
- quorum thresholds;
- controlled write-edge policy;
- archive handoff proof semantics.

The expected latency gain is bounded by the current `write_blocks` and
`write_batch_archive` p95 share. It should not be claimed until local 100-round
evidence passes and the live fleet clears doctor after rollout.
