# PostFiat L1 Storage, State Commitment, and Determinism Inventory

**Audit date:** 2026-07-16
**Code baseline:** `4b5af7bc6bb6e793ed8a60219d13d6d35be03058`
**Status:** STEP 1 evidence; public-source feature containment implemented, production engine remains a real-value launch gate

## 1. Executive finding

The node has meaningful crash-safety work: synced atomic file replacement,
append records with trailing-partial repair, a persist-before-apply ordered-commit
journal, idempotent journal replay, chain-tip reconstruction, canonical state-root
encoding, and historical compatibility roots. That is stronger than an
uncoordinated collection of JSON files.

The audit also reproduced `P0-STATE-01`: the root encoder stopped after
`owned_objects` and omitted all ten later FastLane/FastSwap ledger domains. The
candidate now commits them, with a regression for every field and an exhaustive
destructure that makes future `LedgerState` additions fail compilation until
the root inventory changes.

The candidate now bounds the previously unbounded allocation and append
surfaces: primary JSON files are capped at 256 MiB before allocation; JSONL is
streamed with a 512 MiB file cap, 16 MiB record cap and one-million-record cap;
and receipt/ordered-batch writes append rather than reread and rewrite the full
history. Oversized sparse-file and pre-mutation append regressions pass. A
cross-process mutation lock now covers the complete mempool read-modify-write
boundary: the pre-fix 24-writer regression retained only one append despite all
writers returning success; the fixed boundary retains 24/24. A separate
cross-process lock serializes the full ordered-commit WAL write/apply/remove and
startup-recovery boundary so independent processes cannot overwrite the
singleton journal or interleave committed domains.

It is still not production storage. Consensus state and history are represented
as whole JSON values and JSONL files that are repeatedly loaded into memory.
Current complexity remains linear within the enforced ceilings as accounts,
ledger objects, receipts, blocks, batches, and history grow; several mutations
rewrite entire state vectors. There is no transactional indexed engine, production-size
growth result, online schema migration, corruption/bit-rot repair model, or
tested backup/restore/point-in-time recovery contract. This is
the residual `P1-STORAGE-01` production gap. It blocks a real-value production
claim, while the implemented ceilings make the controlled pre-testnet failure
mode bounded and fail-closed.

Core long-running validator, RPC, run-loop, certified-batch, and private-egress
services refuse to start without the exact
`--unsafe-devnet-json-storage` acknowledgement. This prevents the bounded
JSON/JSONL implementation from being mistaken for a production storage engine;
it does not waive any acceptance test in section 9.

## 2. Persisted domains

`crates/storage/src/lib.rs` stores these primary files:

| Domain | Materialized file | Append/delta behavior |
|---|---|---|
| Genesis | `genesis.json` | Whole atomic write |
| Governance | `governance.json` | Whole atomic write |
| Ledger | `ledger.json` | Whole atomic write |
| Node-local state | `node_state.json` | Whole atomic write |
| Chain tip/cache | `chain_tip.json` | Whole atomic write; reconstructable |
| Blocks | `blocks.json` | `blocks.append.jsonl` merged on read |
| Batch archive | `batch_archive.json` | `batch_archive.append.jsonl` merged on read |
| Ordered batch IDs | `ordered_batches.json` | `ordered_batches.append.jsonl` merged on read |
| Receipts | `receipts.json` | `receipts.append.jsonl` merged on read |
| Mempool | `mempool.json` | Whole atomic write for each family mutation |
| Shielded state | `shielded.json` | Whole atomic write |
| Bridge state | `bridge.json` | Whole atomic write |
| Ordered commit WAL | `ordered_commit_journal.json` | Synced before multi-file apply; removed after apply |
| FastSwap/FastLane owned state | files under `fastswap_store` | Separate WAL/base/vector design with canonical maps/sets |

`faucet_account.json` is also an immutable replay-base input. It is not a
second source of supply authority: new genesis hashes commit
`native_supply_atoms=1000000000`, and both new and legacy replay reject a faucet
record whose balance differs from that protocol constant. A regression proved
that the baseline accepted a coordinated faucet-plus-ledger rewrite before this
binding was added.

Canonical replay now also proves native conservation block by block across
account, escrow, offer-reserve, owned-object, FastLane-reserve and Orchard live
custody against explicit receipt burns. History checkpoint schema v2 commits
the cumulative native fee-burn total and validates `live + burned == genesis`;
legacy v1 checkpoints fail closed and require archive-backed rebuild.

Validator registry, history checkpoints/archives, account indexes, finality vote
artifacts, outboxes, key/config files, prover vaults, and operator evidence are
additional persisted surfaces owned by node modules rather than `NodeStore`.
They require the same migration, permissions, integrity, size, and recovery
classification before STEP 1 closes.

## 3. Current ledger/state model

`LedgerState` is a vector-based aggregate containing:

- accounts;
- asset definitions and trustlines;
- escrows, NFTs and offers;
- NAV assets, reserve packets, redemptions, proof profiles and attestors;
- market-operations policies and finalized envelopes;
- vault-bridge receipts, buckets, allocations, redemptions and deposits;
- PFTL/Uniswap route and receipt state;
- owned objects;
- FastLane reserves, deposit receipts, redeemed exits, asset rules and holder
  permits;
- FastSwap policy snapshots, committees, prepare fences and checkpoint anchors;
- FastSwap activation height.

The whitepaper's four-component state description is therefore not a complete
description of the current committed state. The public protocol must document
the actual domains and their version/migration rules.

## 4. State-root construction

`crates/node/src/state_commitment.rs::replicated_state_root` computes
`hash_hex("postfiat.replicated_state.v1", canonical_bytes)` over:

1. chain ID;
2. genesis hash;
3. protocol version;
4. governance state;
5. ledger state and all active monetary/FastLane/FastSwap subdomains;
6. ordered batch IDs;
7. shielded state;
8. bridge state.

The active implementation uses explicit tagged, length-delimited canonical
append helpers for the original domains. Each vector is sorted by its stable
semantic key before commitment. FastLane/FastSwap records use deterministic
Serde struct encodings, sort the resulting byte strings, and commit each
record's length plus SHA3-384 digest under a distinct field tag. These records
contain no maps or floating point. Serialization failure returns an error
rather than substituting empty bytes. Mutation tests cover every FastLane field,
order invariance, and amount sensitivity. The final exhaustive field inventory
and mutation suite covers every supported governance, bridge and shielded state
type; adding a replicated field without classifying it now fails the gate.

The repository also retains compatibility root functions for earlier state
schemas: legacy JSON, incomplete NAV commitments, SP1-uncommitted NAV profiles,
domainless vault-withdrawal packets, incomplete deposit attestations, and early
NAV-asset omission. Replay conditionals based on chain and activation height are
consensus code. They must be frozen into versioned conformance vectors and
tested across upgrade, archive replay, snapshot import, pruning, and rollback;
they cannot remain undocumented special cases.

### State-root inclusion gate

Every field in `Genesis`, `GovernanceState`, `LedgerState`, `ShieldedState`, and
`BridgeState` must have one explicit disposition:

- consensus state and committed exactly once;
- derived cache, excluded and reproducibly rebuildable;
- local/operator metadata, excluded and never accepted as consensus input;
- historical compatibility only, with bounded chain/height activation.

`append_ledger_state` now exhaustively destructures `LedgerState`, so adding a
field fails compilation until the inventory is updated. Its FastLane mutation
test proves that changing each newly audited field changes the root and that
storage-order changes do not. Equivalent exhaustive field inventories and
mutation tests remain required for `Genesis`, `GovernanceState`,
`ShieldedState`, and `BridgeState`, including declared cache/local-only fields.

## 5. Ordered commit crash protocol

`write_ordered_commit_with_journal_timed`:

1. derives a delta journal containing the post-transition domain snapshots plus
   receipt, ordered-batch, archive, block and optional registry deltas;
2. atomically writes and fsyncs the journal;
3. applies state files and append records;
4. writes the chain tip after appending the block;
5. applies optional validator registry;
6. removes the journal and fsyncs its parent directory.

On startup, `recover_ordered_commit_journal` parses either the current delta or
legacy full-journal schema, reapplies it idempotently, and removes it. Delta
application rejects height/parent/batch/archive/receipt inconsistencies and
recognizes an already committed identical tip.

This design provides a useful WAL invariant, but atomicity spans separate file
replacements and appends. The final production test suite must crash/kill at
every fsync, rename, append, tip write, registry write and journal removal point,
then prove one of exactly two states after recovery: the complete prior commit or
the complete new commit. It must also inject ENOSPC, short writes, permission
loss, corrupted/truncated journal, stale journal, reordered durable writes and
directory-fsync failure.

## 6. Complexity and denial-of-service findings

| Operation | Current asymptotic behavior | Risk |
|---|---|---|
| `read_ledger`, governance, shielded, bridge, mempool | deserialize whole file, `O(state bytes)` | Any query/mutation that needs state pays history/current-state growth |
| `append_receipt` | validates and appends one capped JSONL record | `O(record bytes)` normal write; total file capped at 512 MiB |
| `read_receipts` | materialized vector plus streamed capped JSONL append merge | linear within file/record/count ceilings; still not indexed |
| `read_blocks`, archive, ordered batches | materialized file plus streamed capped append file | linear replay/read cost within hard ceilings; still not paged/indexed storage |
| mempool append per family | deserialize and rewrite whole mempool | attacker-controlled pending set can amplify I/O |
| state-root computation | walks all committed vectors and allocates canonical bytes | `O(total committed state)` per block; no incremental authenticated structure |
| receipt/block duplicate checks | linear vector scans on merge | quadratic recovery/import behavior at scale |
| JSON text reads | metadata cap before `read_to_string` | fails before allocation above 256 MiB; corrupt data still incurs bounded parse work |

The current chain-tip cache reduces some status work, and append records reduce
normal block/history rewrite cost, but they do not bound read/recovery/root cost.
The FastSwap store's BTree maps/sets and WAL are a separate design and do not
solve the main ledger/history path.

## 7. Determinism and canonical-encoding review

### 7.1 Replicated-transition nondeterminism audit

The production portions of `crates/execution`, `crates/ordering_fast`,
`crates/mempool_dag`, and `crates/node/src/state_commitment.rs` were scanned for
wall clocks, runtime randomness, environment reads, filesystem enumeration,
unordered collections, and floating point. None occurs in those replicated
transition, ordering, batch-identity, or state-root boundaries. This result is
now executable: `scripts/test-consensus-determinism-surface` checks 23 Rust
source files across those boundaries and fails on a new use in any of six
nondeterminism classes. It is a blocking product-security CI step and has no
allowlist that could silently normalize a new use.

The wider-node matches were then classified by use:

- `Instant` and `f64` in finality, transport, RPC, proposal, storage-commit and
  Orchard application code feed latency measurements, retry/timeout control or
  metrics only; they are absent from the signed proposal, receipt and state-root
  builders;
- `SystemTime` in storage and finality artifact code names local temporary or
  lock files, while the transaction-layer `unix_now` updates `NodeState` local
  process status rather than `LedgerState` or a block;
- Orchard `OsRng` creates proof/wallet ciphertext material before submission;
  validators receive and deterministically verify the resulting bytes;
- the Cobalt environment variables select an example-report destination and do
  not alter an amendment, vote, certificate or state transition;
- `read_dir` consumers that affect job processing sort paths first; other uses
  are presence checks, commutative byte totals, local spool management or tests;
- production `HashMap`/`HashSet` uses are key lookups and duplicate detection.
  Any collection serialized or committed is a `BTreeMap`/`BTreeSet`, explicitly
  sorted, or emitted through a canonical encoder. Archive-map iteration may
  select which mismatch is reported first, but cannot make an invalid archive
  valid or alter committed state.

This is an audit conclusion about consensus determinism, not a claim that local
observability and timeout behavior is deterministic across hosts.

### 7.2 Canonical encoding and hash-input audit

The enabled hash/signature families have the following concrete encodings:

| Boundary | Encoding and ambiguity control | Regression evidence |
|---|---|---|
| Account/payment/asset/escrow/NFT/offer transactions | Fixed field order and named transcript lines; control characters and surrounding whitespace reject; variable memo/display fields carry lengths/counts; transaction kind, chain, genesis, protocol and signature algorithm are in the outer transcript | stable preimage tests in `crates/types/src/tests.rs`; atomic-swap golden preimages in `atomic_swap_type_tests.rs` |
| W6 atomic swap | Both complete legs are encoded in fixed order under one transaction kind/domain and both authorizations sign the same bytes | signing/transaction-id golden vectors plus dual-auth mutation tests |
| FastLane/FastSwap | Versioned binary encoder with fixed-width integers, explicit lengths/tags, canonical party/vote ordering, bounded decode, and decode/re-encode equality | codec golden/mutation/reject vectors in `fastswap_types` and the 15-target adversarial harness |
| Block proposal/QC/TC/admission artifacts | Domain-separated canonical writer with named, length-delimited fields and canonical distinct validator order | proposal-ID golden vector, quorum/dedup/domain mutation tests in `ordering_fast` and node finality suites |
| Mempool batch references | Versioned hash domains over fixed Serde struct/vector order, chain/genesis/protocol, family counts and payload hash; no unordered maps occur in payload types | fixed batch-id/payload-hash golden vector and per-family tamper verification in `mempool_dag` |
| Replicated state root | Explicit field tags, lengths and fixed-width integer encodings; semantic-key sorting; cache exclusions; versioned legacy replay boundary | empty/non-empty golden roots, field mutation coverage and order-invariance tests in node state-commitment regressions |
| Governance-agent JSON evidence | Recursive canonical JSON key ordering; floating-point JSON rejects; typed hashes use distinct domains | canonical-key-order and bundle/replay hash tests |
| Bridge/Uniswap configuration evidence | Fixed Rust structs and `BTreeMap` state serialized under versioned hash domains | digest golden/mutation tests; externally asserted live transitions remain disabled |
| Orchard/Asset-Orchard | Fixed-size canonical field/point wrappers; versioned action/circuit IDs; explicit public-input encoder; parameter, VK, layout and runtime fingerprints | host/circuit differential, canonical parser, proof and encryption negative tests |

The principal compatibility risk is deliberate historical JSON/struct encoding:
reordering a Rust field or changing an optional-field serialization rule can
change an old digest. Existing golden vectors freeze the enabled high-value
boundaries, and the public release process must treat every remaining signed
Serde transcript as an append-only versioned ABI. Cross-architecture replay and
complete per-artifact conformance-vector generation remain launch assurance
work; no reachable P0/P1 encoding ambiguity was found in this pass.

### Favorable evidence

- committed state uses integer arithmetic and explicit checked operations in
  inspected monetary paths;
- canonical state-root helpers use explicit tags/lengths and deterministic
  vector order;
- many set-like validation paths use `BTreeMap`/`BTreeSet` or sort stable keys;
- transaction/proof randomness is generated before submission and the resulting
  bytes, not the RNG, are verified deterministically;
- `Instant` uses observed in consensus/node modules are timings, timeouts,
  transport liveness or metrics, not committed arithmetic in inspected paths;
- `SystemTime` in atomic temp/vote-lock names affects only unique local filenames,
  not signed/committed contents.

### 7.3 Remaining real-value launch proof obligations

- audit all integer casts, overflow/underflow, division and rounding across every
  asset decimal/scale and both atomic-swap directions;
- extend canonical golden vectors to every historical signed JSON ABI and reject
  unknown fields at every newly versioned external request boundary;
- replay the same archive on x86_64 and aarch64 from a clean snapshot and compare
  every receipt, block hash and state root byte-for-byte.

## 8. Required STEP 2 storage design

The public production candidate needs one transactional, indexed, versioned
storage contract. RocksDB, SQLite, redb, or a custom segmented design may be
selected only after benchmarks and crash semantics are explicit. The design
must provide:

- atomic write batches spanning all consensus domains, receipt, block, archive,
  ordered-batch and registry changes;
- schema/version metadata and deterministic forward migration;
- indexed key lookup for accounts, object IDs, assets, trustlines, nullifiers,
  receipts, heights and transaction IDs;
- bounded range/pagination APIs without whole-history materialization;
- immutable/segmented block and receipt history with checksums;
- incremental state commitments or a measured bounded snapshot strategy;
- snapshots, pruning and archive handoff without changing canonical roots;
- backup/restore and point-in-time recovery with signed/checksummed manifests;
- corruption detection, quarantine and operator repair rules;
- explicit maximum sizes, quotas, compaction policy and disk-full behavior;
- migration and rollback compatibility for the currently deployed devnet data.

## 9. Acceptance tests

- [ ] Pre-fix growth benchmark demonstrates current latency/memory curves at
      10K, 100K, 1M and target multi-year object/receipt/block counts.
- [ ] Target engine meets documented p50/p95/p99 read, commit, root and restart
      budgets with bounded memory.
- [ ] Crash injection at every atomic-commit durability boundary yields exactly
      the old or new complete state, never a mixed state.
- [ ] Migration of a copied current devnet store preserves tip, every receipt,
      every balance/object/nullifier and state root.
- [ ] Rollback to the staged prior binary is either proven safe or explicitly
      refused before incompatible migration.
- [ ] Corrupt/truncate/duplicate/reorder/bit-flip tests fail closed with no
      silent repair of consensus data.
- [ ] Snapshot/export/import and pruning preserve byte-identical replay results.
- [ ] Multi-architecture replay produces identical receipts, block hashes and
      state roots.
- [ ] Field-to-state-root mutation coverage proves completeness and cache
      exclusions.
- [ ] Fuzz/property tests cover canonical encodings, JSON/schema migration,
      journal recovery and indexed key/value invariants.

Until these gates pass on the exact public candidate, the existing JSON/WAL
store remains a controlled-devnet implementation rather than production storage.
