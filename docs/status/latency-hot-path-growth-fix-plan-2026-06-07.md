# Post Fiat L1 v2 Hot-Path Growth Fix Plan

Date: 2026-06-07 UTC
Status: implemented; 1000-round growth gate passed
Owner: latency/storage worker
Scope: remove avoidable O(chain-height) work from the certified-finality hot path without weakening consensus safety

## Objective

The v2 private XRPL comparison packet found that Post Fiat L1 v2 still wins the matched local benchmark, but the Post Fiat path degrades as the local chain grows:

```text
first 100 rounds p50:  ~175 ms
full 1000 rounds p50:  ~545 ms
last 100 rounds p50:  ~1.1 s
```

The goal is to make per-block finality latency depend primarily on the current transaction, current batch, validator vote round, and bounded tip metadata, not on full local history size.

The target after the fix is:

```text
1000-round Post Fiat p50 <= 250 ms
1000-round Post Fiat p95 <= 500 ms
last-100-round p50 / first-100-round p50 <= 1.5x
```

Those are engineering targets, not claims until re-benchmarked.

## Diagnosis

The current implementation has avoidable full-history work in the per-block path.

### Measured growth

From the v2 packet, session 1:

| Stage | Rounds 1-100 p50 | Rounds 901-1000 p50 |
|---|---:|---:|
| `submit_to_finality` | 174.6 ms | 1105.0 ms |
| `vote_requests` | 51.8 ms | 370.8 ms |
| `local_apply` | 43.6 ms | 241.9 ms |
| `submit_admission` | 14.8 ms | 107.8 ms |
| `write_blocks` | 6.3 ms | 69.7 ms |
| `read_blocks` | 2.3 ms | 50.3 ms |
| `blocks_clone` | 2.8 ms | 35.7 ms |

This is not random noise. All five sessions show the same shape: first-100 p50 around 174-179 ms, full-1000 p50 around 542-549 ms, last-100 p50 around 1.10-1.13 s.

### Code path

`apply_batch_with_timings_inner` reads and carries full history state before each commit:

- `store.read_ordered_batches()` plus `Vec::contains` for duplicate detection;
- `store.read_blocks()` to compute next height and tip hash;
- `store.read_receipts()` and `store.read_batch_archive()`;
- then passes those full structures into `prepare_ordered_commit_timed`.

Relevant files:

```text
crates/node/src/lib_parts/part_02.rs
crates/node/src/lib_parts/part_03.rs
crates/storage/src/lib.rs
crates/node/src/block_finality.rs
```

The most direct hotspots are:

| Code area | Current behavior | Why it grows |
|---|---|---|
| `part_02.rs` apply path | reads `ordered_batches`, `blocks`, `receipts`, `batch_archive` before every commit | full JSON/log parse and merge grows with height |
| `part_03.rs::prepare_ordered_commit_timed` | clones `ordered_batches`, `receipt_log`, `archive`, and `blocks` | full-vector clone grows with height |
| `part_03.rs::apply_ordered_commit_delta_journal_timed` | delta journal still re-reads receipts, ordered batches, archive, and blocks | append path is wrapped in full-log validation work |
| `NodeStore::read_blocks` | reads `blocks.json` and all `blocks.append.jsonl` records, then merges them | append log replay grows every block |
| account tx cache refresh | refreshes/revalidates an operator cache after every commit | useful cache, not consensus-critical hot-path work |

This is a storage/indexing problem, not a consensus-shortcut problem.

## Safety Boundary

Do not change:

- block hash inputs;
- proposal hash inputs;
- certificate inputs;
- state-root semantics;
- quorum rules;
- proposal-vote lock behavior;
- parent-hash validation;
- full history replay/audit availability.

The fix must change how local state is indexed and appended, not what is committed or signed.

Every optimization must preserve fail-closed behavior for:

- duplicate block height with conflicting material;
- out-of-order append;
- stale/corrupt tip index;
- corrupt append log;
- duplicate ordered batch id;
- receipt delta mismatch;
- crash after journal write but before all append/index writes.

## Plan

### Phase 1: Add bounded tip metadata

Add a small local file, for example:

```text
chain_tip.json
```

Suggested schema:

```json
{
  "schema": "postfiat-chain-tip-v1",
  "chain_id": "...",
  "genesis_hash": "...",
  "protocol_version": 1,
  "height": 1000,
  "block_hash": "...",
  "state_root": "...",
  "ordered_batch_count": 1000,
  "receipt_count": 1000,
  "history_base_height": 0
}
```

Add `NodeStore` methods:

```text
read_chain_tip()
write_chain_tip()
read_chain_tip_or_reconstruct()
```

`read_chain_tip_or_reconstruct()` should rebuild the tip from `read_blocks()` and checkpoint state if the file is absent. If the file is present but invalid, the node should fail closed for commit and expose an explicit repair command or status error.

Use this for:

- next block height;
- parent hash;
- current state root/tip status;
- cache freshness checks that only need tip hash.

Acceptance:

- legacy stores without `chain_tip.json` still open;
- first commit after legacy open writes tip metadata;
- corrupt `chain_tip.json` causes an explicit error, not silent divergence;
- `verify_blocks` still performs full replay and catches tampering.

### Phase 2: Stop full `BlockLog` clone for new commits

Refactor `OrderedCommitArtifacts` so a commit carries the new block and deltas, not a full cloned `BlockLog`.

Current shape:

```text
OrderedCommitArtifacts {
  receipts: Vec<Receipt>,
  ordered_batches: Vec<String>,
  archive: BatchArchive,
  blocks: BlockLog,
}
```

Target shape:

```text
OrderedCommitArtifacts {
  height: u64,
  receipt_delta: Vec<Receipt>,
  ordered_batch_id: String,
  archive_entry: BatchArchiveEntry,
  block: BlockRecord,
}
```

The state root can still be computed from the canonical replicated state. If `ordered_batches` is still part of that root, replace the full `Vec<String>` clone with a bounded append-root strategy only after a dedicated consensus review. For the first pass, preserve the exact state-root input and optimize everything else.

Acceptance:

- `blocks_clone_ms` disappears or stays near zero;
- generated block hash and certificate id match the old implementation on deterministic fixtures;
- existing block verification tests pass.

### Phase 3: Add append-only indexes for hot-path validation

Add small index files that make membership/tip checks bounded:

```text
ordered_batch_index.json
block_height_index.json
receipt_index.json
batch_archive_index.json
```

Do not use these as consensus truth. Treat them as local acceleration indexes over append-only truth files. On mismatch, fail closed or rebuild explicitly.

Required operations:

- `ordered_batch_exists(batch_id) -> bool`
- `block_tip() -> height/hash/state_root`
- `receipt_exists(tx_id) -> bool`
- `archive_entry_exists(kind, batch_id) -> bool`

Use deterministic encodings and `BTreeMap`/`BTreeSet` where ordering matters.

Acceptance:

- duplicate ordered batch rejection no longer scans a full vector;
- duplicate receipt/block/archive rejection no longer parses full logs in the normal append case;
- corrupted index vs append log is detected by a repair/status command and by full verification.

### Phase 4: Make delta commit truly incremental

Change `apply_ordered_commit_delta_journal_timed` so the normal append case does not call full:

```text
store.read_receipts()
store.write_receipts()
store.read_ordered_batches()
store.write_ordered_batches()
store.read_batch_archive()
store.read_blocks()
```

Use append primitives instead:

```text
append_receipt_record()
append_ordered_batch_record()
append_batch_archive_entry()
append_block_record()
update_chain_tip()
update_hot_indexes()
```

Keep the existing delta journal as the crash recovery boundary. Recovery should be idempotent:

- already appended identical block: ok;
- already appended identical receipt: ok;
- same key with different value: error;
- later height before prior height: error.

Acceptance:

- `write_blocks_ms`, `write_receipts_ms`, `write_ordered_batches_ms`, and `write_batch_archive_ms` are bounded across a 1000-block run;
- crash-recovery tests pass for each partial stage;
- append conflicts fail closed.

### Phase 5: Move `account_tx` cache refresh out of the finality path

The account transaction index is explicitly an operator cache, not consensus state. It should not block finality latency.

Replace automatic refresh inside the commit path with one of:

1. append a small `account_tx_index_pending.jsonl` work item;
2. update only the touched account shards with the current block's rows;
3. run refresh through a background maintenance command/service;
4. mark the cache stale and let `account_tx` rebuild or scan on query.

Preferred first implementation:

```text
Commit path writes a bounded pending-index delta.
Background/index command applies pending deltas to account shards.
RPC reports index freshness separately.
```

Acceptance:

- finality does not wait on full account-tx index refresh;
- `account_tx` remains correct if index is stale by falling back to scan or reporting bounded stale status;
- tests prove stale index cannot return false confirmed history silently.

### Phase 6: Bound full-log materialization

Keep `read_blocks()` and full JSON reconstruction for audit/debug compatibility, but do not use it on the normal hot path.

Add targeted APIs:

```text
read_block_by_height(height)
read_block_tip()
iter_blocks_from(height)
verify_blocks_from_checkpoint()
```

These may initially scan append files internally, but the hot path should use indexes. Later, move to per-height files or a real embedded store if needed.

Acceptance:

- block query APIs still work;
- full verification still catches tampering;
- hot commit path does not call `read_blocks()` in the normal append case.

## Test Matrix

Run existing focused tests first:

```text
cargo test -p postfiat-storage
cargo test -p postfiat-node consensus_block_history_snapshot_tests
cargo test -p postfiat-node governance_history_manifest_tests
scripts/testnet-finality-chaos-gate
```

Add new tests:

| Test | Required assertion |
|---|---|
| legacy store tip reconstruction | absent `chain_tip.json` reconstructs and writes correct tip |
| corrupt tip fail-closed | bad tip metadata refuses commit until repaired |
| duplicate block append | identical duplicate is idempotent, conflicting duplicate fails |
| out-of-order append | block height gap or stale height fails |
| journal replay idempotence | crash after journal write can replay once safely |
| partial append recovery | block/receipt/archive/batch partial stages converge or fail closed |
| hot path does not full-read blocks | benchmark/timing report shows bounded `read_blocks_ms` |
| stale account index | `account_tx` does not silently serve stale false data |

## Benchmark Gate

After implementation, run a focused Post Fiat-only benchmark before rerunning XRPL controls:

```text
VALIDATORS=6 ROUNDS=1000 CARGO_BUILD_MODE=release scripts/testnet-tx-finality-latency-benchmark
```

Required report:

- first-100 p50/p95;
- buckets of 100 through 1000;
- full-1000 p50/p95/p99/mean;
- last-100 p50/p95;
- stage table for `vote_requests`, `local_apply`, `submit_admission`, `read_blocks`, `write_blocks`, `blocks_clone`, `refresh_account_tx_index`.

Only rerun the full XRPL comparison packet after Post Fiat passes the hot-path growth gate.

## Implementation Result

Implementation date: 2026-06-07 UTC.

The growth bug was real. The strongest source was not consensus itself; it was
local storage plumbing around the finality path:

- `status`, proposal construction, fee quote, mempool verification, and batch
  proposal paths were reading full block history where bounded chain-tip metadata
  was enough.
- ordered commit was carrying full cloned receipts/archive/block structures where
  a one-block delta was enough.
- append-backed block/archive writes called JSONL crash repair before every
  append, and that repair read the full append file to check for a trailing
  partial line.

The implemented fix adds `chain_tip.json`, append-backed receipts/ordered
batches, delta commit artifacts, bounded chain-tip reads for hot paths, compact
snapshot export of append-backed data, and tail-only JSONL partial-line repair.
The JSONL repair keeps the same fail-closed crash-repair semantics but seeks from
the end of the file and scans backward only when the last byte is not a newline.

### Evidence

| Gate | Result | Artifact |
|---|---:|---|
| storage unit tests | 10/10 pass | `cargo test -p postfiat-storage` |
| node type/check gate | pass | `cargo check -p postfiat-node` |
| post-patch chaos gate | pass | `reports/testnet-finality-chaos-gate/run-20260607T114026Z/testnet-finality-chaos-gate.json` |
| 300-round sanity benchmark | p50/p95/p99 174.23 / 187.80 / 193.61 ms | `reports/testnet-tx-finality-latency-benchmark/testnet-tx-finality-benchmark-tail-repair-300-20260607T113250Z.json` |
| 1000-round benchmark | p50/p95/p99 182.95 / 214.42 / 223.37 ms | `reports/testnet-tx-finality-latency-benchmark/testnet-tx-finality-benchmark-hotpath-final-1000-20260607T114241Z.json` |

Bucketed 1000-round `submit_to_finality` p50:

| Rounds | p50 ms | p95 ms | local apply p50 ms |
|---|---:|---:|---:|
| 1-100 | 167.4 | 180.6 | 30.2 |
| 101-200 | 168.6 | 183.8 | 33.7 |
| 201-300 | 177.0 | 184.6 | 37.5 |
| 301-400 | 178.2 | 192.9 | 40.7 |
| 401-500 | 183.1 | 202.4 | 43.9 |
| 501-600 | 188.3 | 198.5 | 46.4 |
| 601-700 | 193.5 | 205.5 | 50.0 |
| 701-800 | 200.6 | 213.9 | 54.4 |
| 801-900 | 204.4 | 218.2 | 57.3 |
| 901-1000 | 210.3 | 226.1 | 60.6 |

Acceptance target comparison:

| Target | Result |
|---|---:|
| 1000-round p50 <= 250 ms | 182.95 ms |
| 1000-round p95 <= 500 ms | 214.42 ms |
| last-100 / first-100 p50 <= 1.5x | 1.257x |

### Residual

The original pathological curve is gone. The remaining late-bucket slope is
mostly `refresh_account_tx_index_ms`, which is an operator-query cache rather
than consensus state. It is still awaited in the commit path, so Phase 5 remains
a valid follow-up if we want to remove another roughly 10-30 ms of late-chain
local apply cost. This residual does not block the current growth gate, but it
should be separated from finality before using account-history-heavy workloads
as a marketing benchmark.

## Expected Effect

The highest-confidence wins are:

- remove `blocks_clone_ms` growth by not cloning the full `BlockLog`;
- reduce `read_blocks_ms` and `write_blocks_ms` by using tip metadata and append indexes;
- reduce `local_apply_ms` by removing full-log JSON parse/write from commit;
- reduce `submit_admission_ms` if it is indirectly blocked by growing mempool/history file work;
- reduce tail latency by moving account-tx cache refresh off the commit path.

The vote-request growth needs a second pass after storage is fixed. It may be:

- validators doing the same growing local apply path before responding;
- local process contention caused by disk writes;
- transport service serialization around per-node storage;
- or a separate vote-path bug.

Do not optimize vote networking first. Fix full-history storage work first, then remeasure.

## Done Definition

This work is done when:

1. normal local commit no longer performs full `BlockLog` read/clone/write in the append case;
2. account-tx cache work is no longer finality-critical;
3. full replay/audit verification still passes;
4. adversarial finality gate still passes;
5. 1000-round bucketed benchmark shows no material height-growth latency curve;
6. the public v2 evidence packet can be superseded by a v3 packet with lower and more stable Post Fiat p50/p95.
