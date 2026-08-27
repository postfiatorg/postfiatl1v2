# Storage Scaling and Bounded Finality Milestone

**Status:** Active — PUBLIC TESTNET BLOCKED
**Started:** 2026-08-26
**Decision owner:** Post Fiat
**Research specification:** [Storage Scaling and Bounded Finality](../../architecture/storage-scaling-research-spec.md)
**Implementation specification:** [Storage Scaling Fix](../../architecture/storage-scaling-fix-spec.md)
**Implementation source:** cc9e32d71d697a6db97fac76744df26d320441b9

The operator directly authorized implementation on 2026-08-26 and explicitly
instructed this session not to use Task Node. This milestone records that
operator-directed exception to the repository's normal Task Node workflow.

## E1 — freeze the cost boundary

- [x] Attribute the E4 height curve to full-prefix JSONL verification and
  full ordered-history proposal/state-root work.
- [x] Add process-local counters for checkpoint bytes, crash-suffix work,
  legacy-prefix work, index bitmap bytes, and index slots touched.
- [x] Add reproducible manual measurements at heights 50, 100, 500, 1,000,
  and 5,000.
- [ ] Freeze five 50-round six-validator windows at every required height,
  including raw receipts, host load, fsync time, CPU, RSS, disk, network,
  source/binary/snapshot identities, variance, and model residuals.

Evidence: [development packet](https://github.com/postfiatorg/postfiatl1v2/tree/main/benchmarks/storage-scaling).

## E2 — bounded append and proposal history

- [x] Replace synchronous JSONL full-prefix scans with authenticated v2 heads
  bound to chain, genesis, protocol, log kind, accepted byte offset, current
  and previous MAC, finalized height, block hash, and state root.
- [x] Keep v1 heads compatible through one authenticated full scan followed by
  a v2 rewrite.
- [x] Recover at most one complete crash-suffix record; truncate only a partial
  suffix; reject longer, ambiguous, missing-log, rolled-back, substituted, or
  cross-log states.
- [x] Add per-log mutation locks and rebind affected checkpoints only after the
  ordered chain tip commits.
- [x] Add a deterministic authenticated ordered-batch index and fixed-size,
  domain-separated append-only accumulator.
- [x] Add the explicit genesis field
  ordered_history_v2_activation_height; preserve legacy serialization and
  state roots when it is absent.
- [x] Keep the index synchronized before activation, switch proposal and state
  commitment at the activation height, and reject count/domain mismatches.
- [x] Remove full ordered-batch materialization from transparent, governance,
  shielded, and bridge proposal/commit paths after activation.
- [x] Replay legacy state roots below activation and v2 roots at and above it.
- [x] Add the offline
  postfiat-node ordered-history-index-rebuild --offline-confirmed
  operator command.
- [x] Prove all ordered-commit persistence prefixes recover to one accepted
  activation block and that v2 proposal and commit roots agree.
- [x] Implement the specification's embedded ordered B-tree candidate with
  atomic write transactions. The selected `redb` path replaces the fixed-slot
  candidate for active finality; release performance qualification remains E3.
- [x] Move finalized blocks, receipts, archived batches, ordered membership,
  current state, and chain-tip metadata into one atomic per-height transaction
  as required by the implementation specification.
- [ ] Run every original E3 tamper case and the full new checkpoint, index-page,
  stale-head, journal disagreement, and crash-cut matrix.
- [ ] Add snapshot/import behavior and prove clean and rebuilt indexes are
  byte-identical.

Primary code:

- crates/storage/src/lib.rs
- crates/storage/src/ordered_history.rs
- crates/node/src/mempool_proposals.rs
- crates/node/src/batch_snapshot.rs
- crates/node/src/storage_commit.rs
- crates/node/src/state_commitment.rs
- crates/node/src/block_replay_wallet.rs
- crates/node/src/history.rs

## E3 — exact replay and paired scaling

- [x] Synthetic work counters remain bounded through height 5,000: JSONL
  append verifies zero accepted-prefix records; proposal/index operations read
  a fixed bitmap and write one slot.
- [x] Replace the fixed bitmap candidate with the transactional `redb` ordered
  index and constant-size ordered-history accumulator on the active path.
- [ ] Replay the exact 915-block quarantine archive and authenticated
  controlled-devnet history through height 924 byte for byte.
- [ ] Run paired legacy, bounded-JSONL, and selected indexed-store lanes at all
  five heights with six validators and literal receipt acceptance.
- [ ] Prove height-5,000 p95 consensus_round_ms and
  wallet_to_finality_ms are each at most 110% of height-50 p95.
- [ ] Prove no material synchronous stage retains a positive linear
  relationship with height.

## E4 — migration and rollback rehearsal

- [x] Select a versioned Foundation-governance activation record for the
  existing chain; the controlled genesis remains unchanged.
- [x] Implement consensus-ordered storage-commitment activation and
  pre-activation cancellation records without expanding Cobalt's authority.
- [x] Write the versioned side-by-side migration, signing, cancellation, and
  rollback operator workflow in the storage-scaling evidence README.
- [ ] Rehearse side-by-side rebuild, full replay, staggered restart, activation,
  post-activation finality, pre-activation rollback, forward recovery,
  catch-up, and convergence on six disposable clones.
- [ ] Bind disk-capacity, mixed-version rejection, backups, stop conditions,
  and unchanged Cobalt/Consensus v2 receipts.
- [ ] Obtain separate authorization before any fleet probe, deployment,
  service restart, or live mutation.

## Interfaces and completion

- [x] Deliver `python -m postfiat_rpc.storage_scaling verify PACKET` with
  independent checksum, replay, performance, tamper, migration, and redaction
  verification.
- [x] Deliver the loopback-only read-only browser view from the same verified
  packet.
- [ ] Publish the final checksum manifest, redaction result, replay identities,
  paired curves, migration result, and remaining limits.
- [ ] Run the proportional final release suite from a clean checkout.
- [ ] Move this milestone to completed only when every PASS gate holds.

Until every unchecked PASS item closes, the controlled JSON/JSONL
configuration remains research software and no public testnet or finality SLA
is authorized.
