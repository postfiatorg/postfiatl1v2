# Storage Scaling and Bounded Finality Milestone

**Status:** Active — PUBLIC TESTNET BLOCKED
**Started:** 2026-08-26
**Decision owner:** Post Fiat
**Research specification:** [Storage Scaling and Bounded Finality](../../architecture/storage-scaling-research-spec.md)
**Implementation specification:** [Storage Scaling Fix](../../architecture/storage-scaling-fix-spec.md)
**Implementation source:** 0fdcc2b37022406415ef14a0aa504007e7fd7226

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

The paired release workflow is implemented on clean source `0fdcc2b3`. It
runs the frozen legacy source `8cc7d15e`, bounded-JSONL source `dfd0b9f1`,
and selected source under qualification with their exact owning release
binaries. It binds one shared six-validator public-key identity, deterministic
accounts and transfer semantics, host allocation, storage device, lane-native
authenticated snapshots, literal receipts, full latency distributions,
constant/logarithmic/linear cost models, and raw resource samples. Packet
verification independently reconstructs CPU, RSS, disk, process I/O, host load,
memory, and network totals and refuses any foreground benchmark process with
fewer than two samples.

The first clean full campaign attempt used source `71e539bb`. It completed the
legacy advance to height 50 and three complete 50-round height-50 windows;
window four reached round 12. The run was intentionally stopped before a final
report existed because the shared 90-second request/server timeout could censor
the known slow legacy curve at height 5,000 instead of measuring it. All child
node processes exited. The partial directory is not evidence eligible and does
not pass or fail a performance gate.

Source `0fdcc2b3` gives every lane the same 900-second fail-closed timeout,
binds that policy into packet verification, and cleans up a benchmark child on
interruption. A clean offline development smoke captured at
`2026-08-27T10:20:13Z` passed one measured round at height 2 in all three lanes.
Its report SHA-256 is
`fab4d28a7797658307d8bf47608e074da6a8cf935a275934faee9deb8d929328`;
legacy, bounded, and selected binary SHA-256 identities are
`d2a91b11…f289b1ee`, `ab2e84b3…d1166679`, and
`c75637bf…c8d2b4`. All normalized lanes record the 900-second policy, and their
raw resource streams rehash with foreground sample minima of six, seven, and
four. The report explicitly records `evidence_eligible: false`, `offline: true`,
`network_contacted: false`, and `devnet_queried_or_mutated: false`. This proves
the corrected paired mechanics and fail-closed evidence binding only; it does
not close the five-height E1 window, either E3 latency ratio, or the
height-relationship gate.

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
- [x] Run every original E3 tamper case and the full new checkpoint, index-page,
  stale-head, journal disagreement, and crash-cut matrix.
- [x] Add snapshot/import behavior and prove clean, restored, and rebuilt
  canonical logical records and history-index entries are byte-identical.

Clean offline qualification on source `abf74668` closed 69 classified cases
through 37 nonzero test/campaign filters with no uncovered requirement. Tamper
report SHA-256:
`8bb4ee30f8b55d10ff41b66aa1163be9ca3afeead4b56b941b0ae1e47c61c9d6`.
The runner preserved the frozen E3 manifest identity `c23320d4…7167fa7`,
rebound only its five audited source hashes in manifest SHA-256
`80fc5dfe…1a30350`, and independently verified 42 rejected attacks plus six
byte-identical recoveries in report SHA-256 `10d081b6…e610cb7` with
classification `ab53b5dd…b90d3`. This closes the source-level E2 tamper gate;
final packet publication remains open.

The same closed 69-case matrix passed again from the clean current source
`fb2bf1cc` on 2026-08-27, through the same 37 nonzero test/campaign filters,
with no uncovered requirement and no network contact. The current tamper report
SHA-256 is `eb278d7a…b43cc6a`; its frozen-E3 campaign and rebound-manifest
SHA-256 identities are `96b50b37…1a74dd6` and `de7cc292…19e40ed`. Case 64 is
bound to the clean compatible-rollback rehearsal below rather than a synthetic
success receipt.

Primary code:

- crates/storage/src/transactional.rs
- crates/storage/src/transactional/canonical_export.rs
- crates/storage/src/transactional/generation.rs
- crates/storage/src/transactional/tamper_tests.rs
- crates/storage/src/transactional/export.rs
- crates/storage/src/integrity.rs
- crates/storage/src/lib.rs
- crates/storage/src/ordered_history.rs
- crates/node/src/storage_migration.rs
- crates/node/src/storage_vote_guard.rs
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
- [x] Replay the exact 915-block quarantine archive below activation: all six
  source verifiers, transactional rebuild, canonical logical comparison, and
  independent verify-only passed offline on `1985cd3f`. Receipt SHA-256:
  `2596d7874edc348fd232bf6d97b7880c339f31d0f3a4516892d913cbc54d207a`;
  source-tree SHA-256:
  `6c9c9c11955b761a9e7b80b5fbf5b482f307fa602bc6d139b27868b76135139a`;
  release-binary SHA-256:
  `811fb4921ec326bedeec88c37bc92730bd65943dcc795f542d0f5dd9065b7483`.
- [ ] Replay the authenticated controlled-devnet history through exact height
  924 byte for byte; no complete height-924 data directory is present in the
  local archive, and no fleet access is authorized by this milestone.
- [ ] Run paired legacy, bounded-JSONL, and selected indexed-store lanes at all
  five heights with six validators and literal receipt acceptance.
- [ ] Prove height-5,000 p95 consensus_round_ms and
  wallet_to_finality_ms are each at most 110% of height-50 p95.
- [ ] Prove no material synchronous stage retains a positive linear
  relationship with height.

Paired qualification code:

- benchmarks/storage-scaling/run_paired_campaign.py
- benchmarks/storage-scaling/run_campaign.py
- benchmarks/storage-scaling/package_packet.py
- python/postfiat_rpc/storage_scaling.py
- python/tests/test_storage_scaling.py
- python/tests/test_storage_scaling_packager.py

## E4 — migration and rollback rehearsal

- [x] Select a versioned Foundation-governance activation record for the
  existing chain; the controlled genesis remains unchanged.
- [x] Implement consensus-ordered storage-commitment activation and
  pre-activation cancellation records without expanding Cobalt's authority.
- [x] Write the versioned side-by-side migration, signing, cancellation, and
  rollback operator workflow in the storage-scaling evidence README.
- [x] Rehearse compatible post-activation software rollback across two distinct
  release binaries and six disposable local validators. Clean source
  `fb2bf1cc` finalized height 2, compatible ancestor `20c95ec2` resumed the exact
  certified tip and finalized height 3, and `fb2bf1cc` resumed the exact
  height-3 tip and finalized height 4. All six converged with literal accepted
  receipts, bounded page and accumulator work, and zero full-history reads.
  Report SHA-256: `07a67e3c…fabd746`; current/rollback binary SHA-256:
  `4e14999a…209062a9` and `affc6bac…e9a937`.
- [x] Rehearse side-by-side rebuild, full replay, staggered restart, activation,
  post-activation finality, pre-activation rollback, forward recovery,
  catch-up, and convergence on six disposable clones.
- [x] Bind disk-capacity, mixed-version rejection, backups, stop conditions,
  and unchanged Cobalt/Consensus v2 receipts.
- [ ] Obtain separate authorization before any fleet probe, deployment,
  service restart, or live mutation.

The fail-closed six-clone runner completed the full ten-phase sequence on the
stopped local height-501 development fixture. The tightened current-format pass
at `2026-08-27T08:09:48Z` used source `20c95ec2`, six distinct immutable source
and backup roots, release binary SHA-256 `161f64d2…cab53187`, and incompatible
binary SHA-256 `f3a58e2e…b4b514a2`. It finalized heights 502 through 511 with
literal accepted receipts, retained Consensus v2 certificates, cancelled the
first scheduled activation, restarted all six validators in three stages,
formed the activation block from an exact five-vote quorum while validator-4
was held back, caught validator-4 up from that certificate, and converged at
tip `c68d3bfb…a0857d9e` and state root `20323b47…19863092`. Its source-stop
receipt inspected 45 processes with zero unreadable or matching processes, and
the incompatible binary failed with literal reason `storage_unsupported_schema`.
The redaction-safe report SHA-256 is `62571689…2e618476`.

This is development evidence only: the fixture is not the controlled height-924
archive, the source worktree was dirty, and the report is explicitly ineligible
for final packet publication. The exact clean-source height-924 rehearsal remains
open with E3 replay and the final packet.

Primary code:

- benchmarks/storage-scaling/run_migration_rehearsal.py
- crates/node/src/transport_cli.rs
- crates/node/src/transport_protocol.rs
- crates/node/src/transport_runtime.rs
- benchmarks/storage-scaling/package_packet.py
- python/postfiat_rpc/storage_scaling.py
- python/tests/test_storage_scaling.py

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
