# Fast Validator Reads: Measurements And Design Note

**Date:** 2026-11-24
**Status:** Study and design note. Step one is implemented on this branch for `blocks`, `receipts` and `tx`; steps two and three are proposals.
**Base:** origin/main `f82ed6986`.

> **Status note (25 November 2026):** step one is on `main` as `9d34b58e0`
> (PR 227). The live build `04600868` descends from that commit and carries the
> same `block_finality.rs` and storage code (`git diff` between the two is empty
> for those files). On 25 November a `blocks` read by height on validator 0
> answered in 0.3 to 0.5 seconds through a tunnel. Steps two and three are
> still proposals.

Validator reads are slow in production: 7 to 10 seconds per read. They are
behind the slow screens, the readiness timeouts and much of the wait before a
proof. This note measures where the time goes and proposes three steps that can
be released one at a time.

All numbers below are from one engineering work server, not from a validator.
They were taken with a release `postfiat-node` built from `f82ed6986` against
a read-only copy of one validator's data directory at height 1854. `rpc-serve`
ran on loopback inside an isolated network namespace. No live validator was
written to or asked a timed question.

## 1. Measurements

Method:
- 10 runs per read, sequential, one client.
- "Cold" means every file of the copy was evicted from the page cache first (`posix_fadvise(DONTNEED)`); "warm" means it was not.
- The time is the client's wall clock from connect to the end of the reply line.

| Read | Path | Cold median | Cold worst | Warm median | Warm worst | Reply bytes |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `status` | in-process, cached | 0.008 s | 0.096 s | 0.010 s | 0.010 s | 11,954 |
| `server_info` | in-process, cached | 0.004 s | 0.103 s | 0.010 s | 0.010 s | 3,081 |
| `account_lines` | child | 0.085 s | 0.098 s | 0.067 s | 0.075 s | 2,189 |
| `account_assets` | child | 0.116 s | 0.126 s | 0.091 s | 0.102 s | 2,182 |
| `vault_bridge_route` | child | 0.109 s | 0.135 s | 0.084 s | 0.102 s | 3,120 |
| `vault_bridge_status` | child | 0.136 s | 0.161 s | 0.117 s | 0.139 s | 531,171 |
| `blocks`, one block | child | 2.585 s | 2.638 s | 2.045 s | 2.084 s | 169,027 |
| `receipts`, one transaction | child | 2.615 s | 2.739 s | 2.006 s | 2.125 s | 542 |
| `tx` | child | 4.233 s | 4.333 s | 3.647 s | 3.798 s | 170,938 |
| `pfusdc_egress_witness`, 3 blocks of ancestry | child | 6.192 s | 6.504 s | 5.689 s | 5.877 s | 583,609 |

### Where the time of one read goes

I traced one child read of each kind with `strace -f -tt` and `time -v`.

**`vault_bridge_route`** (171 ms under the tracer, 42 MB peak memory):

| Part | Time | Share |
| --- | ---: | ---: |
| Starting the child process, up to reading the request | 20.5 ms | 12% |
| Integrity key, genesis and storage configuration files | 12.0 ms | 7% |
| A first database open and close, only to check the storage activation | 11.6 ms | 7% |
| The second database session: open, read and authenticate the governance, ledger and shielded state, decode them, build the answer, close | 116.1 ms | 68% |
| Serializing and writing the answer | 6.4 ms | 4% |
| Exit and teardown | 4.4 ms | 3% |

**`blocks` for one block** (2,583 ms, 838 MB peak memory):

| Part | Time | Share |
| --- | ---: | ---: |
| Process start and configuration files | 13 ms | 0.5% |
| First database session (activation check) | 9 ms | 0.4% |
| Second database session: every block of the retained history is read, authenticated and decoded, then filtered down to the one asked for | 2,457 ms | 95% |
| Serializing and writing the answer | 25 ms | 1% |
| Teardown, freeing the decoded history | 77 ms | 3% |

**Where the database session's time goes.** System calls are 1.7% of the `blocks` read: 1,907 `pread64` calls take 27 ms. The rest is CPU in user space, made of three things:
- an HMAC-SHA3-384 check of every record;
- JSON decoding of every block;
- a second JSON encoding of every block, done only to count bytes for an instrumentation counter (`record_full_history_scan`).

**Reads also write.** Every child, even a read, performs 18 `pwrite64` and 10 `fdatasync` calls. The redb version in use rewrites its allocator state on every close (STO-07 in `crates/storage/src/transactional/lease.rs`). The database file's modification time changes on every read.

### Reads wait for each other

Every child takes the database file with an exclusive lock (`flock(LOCK_EX|LOCK_NB)`), once for the activation check and once for the read. A child that finds it taken retries with a backoff of 50 ms up to 500 ms, for up to 20 s. Measured on the copy:

| Load | Result |
| --- | --- |
| Six `vault_bridge_route` reads at once | 0.07 s, 0.25 s, 0.43 s, 0.85 s; two refused by the per-peer child limit (4) |
| One `blocks` read with five `vault_bridge_route` reads behind it | `blocks` 2.0 s; the route reads 2.3 s, 2.8 s and 3.4 s; two refused by the same limit |

### Why production sees 7 to 10 seconds

This part is inferred from the code and the numbers above. It was not measured on a validator.
- A short read costs about 0.1 s here, and the same code runs in production. So the 7 to 10 s comes from waiting, not from the read itself.
- The validator's own block commits hold the same lock.
- So do the long reads that the relay and the payout worker make every few seconds: `blocks`, `receipts`, `tx` and the witness, each 2 to 6 s here.
- Each short read queues behind them with backoff.
- Each of the 10 `fdatasync` calls costs far more on a virtual machine's network disk than on this server's local disk.

## 2. Why every request pays the full load

The code path:
1. `rpc-serve` (`crates/node/src/rpc_cli.rs`, `handle_rpc_serve_connection`) answers `status`, `server_info` and a few others in process. Everything else is spooled to a file and run in a fresh child process: `postfiat-node rpc --request-file` (`run_rpc_request_via_child`).
2. The child (`crates/node/src/rpc_dispatch.rs`) maps the request to a query function. That function builds a new `NodeStore`.

What every child repeats:
- **Process start and configuration.** It loads the integrity key, genesis (MAC-checked), the storage backend mode and the generation pointer, read twice.
- **Two database sessions.** One answers "is transactional storage active", which never changes between requests. The second does the read. Each session is an exclusive open, which reloads the allocator state, and a close, which rewrites it and syncs it to disk.
- **Whole state domains instead of the record asked for.**
  - `account_lines` decodes the whole ledger to answer for one account.
  - `vault_bridge_route` decodes governance, the ledger and the shielded state.
  - Each domain is authenticated (HMAC-SHA3-384 over the whole value) and JSON-decoded on every request.
- **The whole block history for one block.**
  - `blocks` calls `NodeStore::read_blocks`, which decodes every retained block (`blocks_in_height_order`) and then keeps one. The store already has a lookup by height (`TransactionalStore::block`).
  - `receipts` and `tx` do the same over blocks and receipts (`receipts_in_block_order`).
  - The witness reads blocks, receipts and the batch archive.
- **A second encoding for a counter.** `record_full_history_scan` JSON-encodes every block again to count bytes.
- **Nothing survives the request.** No index, decoded state or MAC result outlives the child. No signature checks are repeated on reads; the MAC checks are.

## 3. Options

"Saves" below is what the option removes from the measurements in section 1. The lock waits of section 1 shrink in proportion, because each read holds the lock for less time.

**(a) A resident read process that holds the state and applies each finalized block.**
- **Saves:** nearly everything: about 0.1 s becomes about 1 ms for state reads, and 2 to 6 s becomes milliseconds for history reads. It also ends the lock contention for reads.
- **Size:** large, weeks. It needs a second apply path for blocks, or a feed of committed state from the writer, plus indexes by height, transaction and account, and a protocol for "which height is this".
- **Consistency:**
  - It may only expose a height after that block's commit has completed. It must read the committed tip from the store, never from the block in flight.
  - Every answer must name its height.
  - Behind the writer it is stale by design. It must say so, and refuse when it lags by more than a bound.
- **Restart and release:** it rebuilds from the store, taking seconds and growing with state size. A release must restart it with the writer.
- **Memory:** 0.1 to 0.2 GB of decoded current state today, from the 42 MB child peak. Keeping the decoded history would be 0.8 GB today and about 8 GB at ten times, so history must stay on disk behind an index.
- **Risk:** a second implementation of state transitions can disagree with the writer. The answer must be built from the writer's committed state, not re-derived.

**(b) A per-height snapshot that a short-lived child maps instead of parsing.**
- **Saves:** decode and authentication: most of the 116 ms state session, and nothing of the history reads.
- **Size:** medium. It needs a stable binary layout for every state domain, plus writing it at each commit.
- **Consistency:** written after the commit and named by height. A reader maps one complete file, so it is never half written.
- **Restart:** none; the files persist.
- **Memory:** mapped and shared, so only the pages touched count.
- **Risk:** a new on-disk format with its own integrity rule, and duplicated state on disk (about the state size per retained height).

**(c) A cache of whole answers keyed by height and request.**
- **Saves:** repeated identical reads; the relay polls the same few reads. The first read at a new height still pays the full cost.
- **Size:** small to medium, in `rpc-serve`.
- **Consistency:**
  - The key must include the committed height, read cheaply, and an answer may only be filed if its height equals the height read before and after it.
  - Several answers do not name their height today (`account_lines`, `account_assets`). They must, before they are cached.
- **Restart:** an empty cache.
- **Memory:** bounded by size; `vault_bridge_status` alone is 0.5 MB per height.
- **Risk:** low if the height rule holds, wrong answers if it does not.

**(d) Loading only what a method needs.**
- **Saves:** for `blocks`, 2.0 to 2.6 s becomes about 0.03 s (measured in section 5). For `receipts` and `tx`, the same once they look up by transaction id instead of scanning. The single-domain state reads (`account_*`) save the decode of the domains they do not touch.
- **Size:** small per method, and it can be released method by method.
- **Consistency:** the same store, the same read transaction rules, and the same answer, provable by a differential test.
- **Restart:** nothing new.
- **Memory:** less than today; the `blocks` child no longer holds the whole history (838 MB).
- **Risk:** low. A window read no longer checks that every block outside the window is present, so a corrupt history is found by `verify-blocks` rather than by a read.

**(e) What transactional storage already gives.**
- The redb store already has:
  - lookups by height (`block`) and by hash;
  - a point lookup for FastPay anchors;
  - `read_snapshot`, which binds reads to one finalized tip.
- The single-writer plan (`docs/plans/active/devnet-storage-single-writer-deployment-plan.md`) designed read-only opens next to a live writer. Its 2026-09-10 correction records that the deployed mechanism is exclusive access with short leases instead.
- So nothing above needs a new database. Option (d) is that store's own indexes, used. Option (a) is the IPC delegation that plan rejected for the devnet and deferred to public-testnet planning.
- Read-only opens that do not take the exclusive lock would remove the waiting in section 1 without any of (a) to (c). They depend on the redb version and are that plan's open item, not this note's.

## 4. Recommendation

Three steps, each releasable alone.

1. **Read only what is asked (option d), method by method.**
   - Start with `blocks` (implemented in section 5).
   - Then `receipts` and `tx` by transaction-id lookups (section 6), then the witness.
   - Then drop the duplicate activation-check open and the counter's second encoding.
   - Expected: `blocks` about 0.03 s instead of 2 to 2.6 s (measured); `receipts` and `tx` similar once indexed; the witness down by the history-scan part. Short reads unchanged at about 0.1 s, but they wait less, because long reads no longer hold the lock for seconds.
2. **A shared, read-only, height-stamped cache in `rpc-serve` (option c)** for the polled reads: route, status of the vault bridge, account lines and assets.
   - Every answer names its height first.
   - Expected: a repeated read at the same height about 1 ms, the first one unchanged.
3. **A resident reader (option a)**, fed from the writer's committed state through the store, once the read-only open question of option (e) is settled.
   - Expected: about 1 ms for state reads and a few ms for history reads, with no child processes.

**Tests that prove the fast path equals the slow path, byte for byte:**
- **Unit:** for every window, the windowed read encodes to the same bytes as the full read after the query's own filter (`block_window_equals_the_filtered_full_history`).
- **Differential over a replay of the live chain:**
  - Take a read-only copy at a known tip.
  - Run the old and the new binary for the same request over a grid of parameters: every method in the step, heights at the history base, inside, at the tip and above it, and several limits.
  - Compare the replies with `cmp`.
  - Repeat at each release on a fresh copy.
  - For step two, compare a cached reply with a fresh child reply at the same height. For step three, compare the resident reply with a child reply at the same height.

## 5. Step one, implemented: `blocks` reads only the requested window

The change:
- `TransactionalStore::blocks_window(from_height, limit)` reads the requested heights by key.
- `NodeStore::read_blocks_window` uses it on transactional storage, and keeps the legacy file path as it was.
- The `blocks` query calls it.
- It applies the same checks to each returned block as the full read: present, and its embedded height matches.

Tests:
- **Unit:** `block_window_equals_the_filtered_full_history` (storage) compares the encoded bytes of the window and the filtered full history for every window over a two-block store.
- **Differential on the copy:** see the result below.

Measured on the copy, 10 runs each:

| Read | Before: cold / warm median | After: cold / warm median (worst) | Peak child memory before / after |
| --- | --- | --- | --- |
| `blocks`, one block | 2.585 s / 2.045 s | 0.031 s / 0.025 s (0.047 s) | 838 MB / 13 MB |
| `blocks`, eight blocks | 2.623 s / 1.977 s | 0.058 s / 0.056 s (0.065 s) | |

Differential on the copy:
- The grid: 48 requests, with `from_height` of none, 0, 1, 2, 500, 925 to 928, 1500, 1846, 1851, 1853, 1854 (the tip), 1855 and 999999, each with limits 1, 8 and 64.
- The main binary and the step-one binary gave identical replies for all 48 (`cmp`), including the empty replies above the tip.
- The change is under 100 lines of code, plus its test.

## 6. Step one continued: `receipts` and `tx` read by transaction id

The store already has the index these reads need. `receipts_by_id_v1` holds, under each
transaction id, every finalized occurrence of its receipt with the block height. There is no
index by account. No new table or format was added.

Why the keyed read is exact:
- A normal commit refuses a receipt stored under an id other than its `tx_id`.
- The legacy migration, the only caller that relaxes that check, pairs each block receipt id
  with the persisted receipt of that `tx_id`. So every record's key equals its receipts' `tx_id`.
- Occurrences are kept only for heights inside the retained history, in height order, which is
  the order of the full scan.

The change:
- `TransactionalStore::receipts_for_tx_id` and `receipt_occurrences_for_tx_id` read one record.
  An id that is not a valid identifier matches nothing, as in the scan.
- `receipts` with a `tx_id` uses it.
- `tx` reads the occurrences, the blocks at their heights and the tip block. The other
  receipts of the chosen block, which its account rows need, are read by key too.
- The full read stays for `receipts` without a `tx_id` (the latest receipts), for
  `tx` with `audit_block_log` (a full replay is its purpose), and for the legacy file store.

Tests:
- **Unit:** `receipts_for_tx_id_equal_the_filtered_full_scan` (storage) compares the encoded
  bytes with the full scan's for known, unknown, empty, control-byte and oversized ids.
- **Differential on the copy** (tip 1854), the main binary against this branch, `cmp` on the
  whole reply:
  - `receipts`: 49 requests, all identical. They cover 16 transaction ids from blocks across
    the history, among them the three burns of 23 November; 10 unknown or malformed ids (all
    zeros, empty, upper case, one character short or long, spaces, a path); limits of none, 1,
    2, 0 and 1001 (the last two refused); and three requests without a `tx_id`.
  - `tx`: 27 requests, all identical: the same 16 ids, the same 10 unknown or malformed ids,
    and one request without a `tx_id`.
  - Neither method takes a height, so "above the tip" does not apply to them.

Measured on a fresh copy at the same tip, 10 runs each, through `rpc-serve`. "Before" is a
main build of `fec6ff729`, whose crates equal `f82ed6986`. The read is the burn at block 1851.

| Read | Before: cold / warm median | After: cold / warm median (worst) | Peak child memory before / after |
| --- | --- | --- | --- |
| `receipts`, one transaction | 2.642 s / 2.031 s | 0.023 s / 0.020 s (0.031 s) | 837 MB / 12 MB |
| `tx` | 4.348 s / 3.566 s | 0.035 s / 0.030 s (0.048 s) | 842 MB / 21 MB |

Not done in this round:
- **`pfusdc_egress_witness`** still reads the whole block, receipt and archive history. Every
  part it needs has a keyed read already: the block by height, the prior checkpoint by
  `block_height_by_hash_v1`, the ancestry as a window, the block's receipts by id, and the
  governance payload by batch. It builds a proof input, so it gets its own round with a
  differential over the three burns of 23 November (blocks 1846, 1848 and 1851).
- **The duplicate activation-check open and the counter's second encoding.** The keyed paths
  above no longer run the counter's encoding. Removing the second open touches the storage
  lease rule (STO-07) and needs its own review.

## Not measured

- Nothing here ran on a validator: not the lock waits under a real writer, not `fdatasync` on its disk, and not the relay's concurrency.
- The 7 to 10 s figure comes from the operator's handoff; section 1 explains it but does not reproduce it.
