# Certified-send tombstone bounding complete

- **Operator:** Post Fiat (`postfiatchad`)
- **Date:** 2026-08-29 UTC

## BLUF

The certified-send remediation plan is implemented, verified, pushed, and
frozen at source `e52e050269a2f9fdd28c5083c3888debf3a85063`. The proposal path
no longer rereads and rehashes every retained delivery tombstone: a release
spot check fell from 66.893 ms and 6,144 retained-file reads to 2.098 ms and
zero retained payload reads. The G1 source/binary candidate freeze and the
binary-sensitive G2 refresh passed locally; campaign-input binding remains
pending authorization. This does **not** qualify storage or authorize another
performance campaign; the next 5+5+5 run needs a separate operator decision.
The governing
records are the [completed remediation plan](../plans/active/certified-send-tombstone-bounding-plan.md)
and the [storage scaling milestone](../plans/active/storage-scaling-milestone.md).

## Current state

### Exact lineage

| Boundary | Current fact |
| --- | --- |
| Node implementation | `e52e050269a2f9fdd28c5083c3888debf3a85063` (`fix(node): bound certified-send tombstone resume`), pushed to `origin/main` before the documentation closure |
| Frozen release binary | SHA-256 `6b130a1f9c81bd64bc9dc42043595f5a27e84185cf3f40b13b5f37a40d72a82e`; 51,978,232 bytes; embedded revision `e52e0502`; profile `release` |
| G1 source/binary candidate manifest | SHA-256 `895ec768927a2d630c7d90fd73c5275efe1e54e857248a7cf12441fe97df9ffe`; campaign inputs not yet bound |
| Runner/verifier | Branch `postfiatchad/corrected-g4-vote-lock-gate`, pushed commit `15d059d1`; not merged into `main` |
| G2 safety manifest | PASS; SHA-256 `dc01f9770fc2344cce0dcfcfd58dbe37b968f9ffc0276c329bb4f4fea47378e7` |
| Campaign state | No post-fix performance campaign exists and none was authorized |
| Qualification state | Storage remains **SELECTED, NOT OFFLINE QUALIFIED**; deployment and public testnet remain blocked |

The documentation commit containing this handoff is a docs-only successor to the
frozen runtime source. It is not a new node candidate and is not deployed.

### Changed invariant

Per-proposal certified-send resume is now bounded by active/pending jobs plus one
bounded index read and the specific entries being compacted or pruned. The
full-set validation authority moves from every proposal to: **(a) one-time
index migration, (b) explicit startup/repair, (c) the specific entries touched
by compaction or pruning.**

That is the only deliberate authority change. Pending jobs still block a
proposal. Completion, acknowledgement, quarantine, canonical job names, replay
and duplicate rejection, size limits, staging and retention cleanup, atomic
moves, and fsync ordering remain fail closed. Certificates, receipts, batches,
signing bytes, consensus voting, Consensus v2, and Cobalt authority are
unchanged.

### What changed

- `crates/node/src/certified_send_completed_index.rs` owns the durable ordered
  index, canonical checksums, topology/chain/genesis/protocol bindings, raw
  job/batch/certificate SHA-256 bindings, one-time migration, atomic mutation
  intent, serialized `flock`, bounded compaction/pruning, crash reconciliation,
  explicit verification/rebuild, work reports, and owner tests.
- `crates/node/src/transport_cli.rs` delegates completed-tombstone work to that
  module, returns the expanded resume report, and exposes the explicit operator
  verification path.
- `crates/node/src/transport_runtime.rs` names the resume validator and reports
  `outbox_resume_ms`, compaction/validation/prune timings, files/bytes/index
  reads, enumerations, compacted/pruned jobs, and migration state in every round.
- CLI dispatch/runtime helper files connect the explicit repair command without
  changing normal proposal semantics.
- Runner `15d059d1` adds a certified-send bounded-work gate and round-coverage
  residual gate, packages and hash-binds their receipts, and makes the
  independent verifier recompute them. It also re-keys vote-lock migration and
  certified-send migration to each validator's first observed reservation or
  resume after restore, while rejecting a genuine second migration.

The state, intent, and mutation-lock files are stored at the data-dir root:

- `.certified-send-completed-index-state.v1`
- `.certified-send-completed-index-intent.v1`
- `.certified-send-completed-index-mutation.lock`

They are deliberately outside `certified-send-outbox/` and `completed/` because
the prior compatible binary rejects unknown/noncanonical entries in those
directories. Root placement lets old and new binaries use the same data
directory during the verified rollback sequence.

### Before and after

| Release spot check at 1,024 retained tombstones | Pre-fix `442c5a4d` | Frozen candidate `e52e0502` |
| --- | ---: | ---: |
| Resume time | 66.893 ms | 2.098 ms |
| Retained payload files read | 6,144 | 0 |
| Retained bytes read | 3,952,142 | 0 |
| Payload hashes | 4,096 | 0 |
| Payload bytes hashed | 73,728 | 0 |
| Index work | none | one 687,566-byte read |

The release proposer-rotation assertion also passed: the validator holding
1,024 tombstones was only 2.054 ms slower than the fastest peer. Executable
owner tests prove flat retained-payload work at 0, 240, and 1,024 tombstones.

### Round-path directory-scan audit

| Surface | `read_dir` classification | Synchronous unbounded round-history work? |
| --- | --- | --- |
| `block_finality.rs`, `storage_commit.rs`, `mempool_proposals.rs` | No hits | No |
| `transport_runtime.rs` proposal request directory | Bounded request queue; `.take(MAX + 1)` with cap 1,024 | No |
| `transport_runtime.rs` private-egress/certified-loop list helpers | Outer operator work-queue/tooling loops, outside synchronous proposal finality | No |
| `transport_cli.rs` disposable job cleanup | Bounded to 16 entries | No |
| `transport_cli.rs` staging/outbox/enqueue/resume/quarantine scans | Bounded by fixed outbox caps (at most 1,024 active jobs plus the explicit over-cap sentinel) | No |
| `transport_cli.rs` retention cleanup | Bounded to one retention entry | No |
| `certified_send_completed_index.rs` completed set | Full enumeration only during one-time migration or explicit repair, capped at 2,048 transient entries; normal resume reads the index and enumerates only active work | No |
| `batch_snapshot.rs` rootfs/snapshot/restore scans | Deployment packaging or import/export tooling, with explicit caps where applicable; not the proposal path | No |

Result: zero unbounded synchronous round-history scans remain in the audited
surfaces.

### Verification completed

| Gate | Result |
| --- | --- |
| `cargo test -p postfiat-node certified_send --locked` | PASS: 30 passed, one intentional manual release check ignored |
| `cargo test -p postfiat-node vote_lock --locked` | PASS: 15 passed, one intentional manual release check ignored |
| `cargo test -p postfiat-storage --locked` | PASS: 83 passed, two manual scaling checks ignored; transactional process-crash test passed |
| Explicit release proposer-rotation check | PASS at 1,024 retained tombstones |
| `cargo fmt --all -- --check` | PASS |
| `cargo check --workspace --all-targets --locked` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS; node library 329 passed / 3 ignored and node binary 138 passed / 3 ignored; complete command exited 0 |
| Runner, packager, independent verifier | PASS: 95 focused tests on `15d059d1` |
| `cargo test -p postfiat-node transactional_verify_only --locked` | PASS: 2 passed; missing input remains absent and stale generation refusal is non-mutating |

A clean detached checkout was used for the final release build. Rust 1.95.0,
`Cargo.lock` SHA-256
`fc3c51db25aed8fcc1bd16c9a24ada186add85dc8da4909246fb9b177a7f6397`,
the Zig-backed compiler wrappers, host, filesystem, build commands, durations,
and binary metadata are bound in the G1 manifest.

### Fresh local G2 evidence

| Evidence | Result |
| --- | --- |
| Compatible rollback | PASS; six validators converged from current post-activation state through older compatible binary `0ac4e190` and forward recovery with the frozen candidate; zero full-history reads; report SHA-256 `af37f0e4de9d23689b131532d0fded2593905b5d88a5de1600431c511d6904be` |
| Tamper/crash matrix | PASS; 69 cases, 37 unique owner tests, complete coverage, zero uncovered requirements; report SHA-256 `df45e0bb478299e7778bc50537fd6bb059f04a19c52f01d0c5adb444331c2ceb` |
| Stale-generation receipt | PASS; refusal with no partial mutation; SHA-256 `f38071d7d574513cfe37c526386456ad695077ed928ebbdd947a69b8445fc66c` |
| Scope | Offline only; no network or devnet contact; no deployment or performance campaign |

These G1/G2 directories are local and private. They include disposable keys and
other non-public material. Do not commit, publish, delete, or treat them as a
redaction-safe G5 packet.

### Explicit omissions and boundaries

- No performance campaign was started. The prior corrected campaign remains
  closed failed evidence and cannot be resumed or relabeled.
- No devnet, fleet, service, deployment, validator-directory, or height-924
  action occurred.
- No Task Node or subagents were used.
- No prior private campaign/fleet directory was changed or deleted.
- The two unrelated untracked auditor inventories under `docs/security/` were
  not edited or staged.
- G3 exact replay, a future G4 pass, the redaction-safe G5 packet, G6 clone
  rehearsal, deployment authorization, and public-testnet eligibility remain
  outside this completed remediation plan.

## Next decision or action

The next local action requires an explicit **yes/no authorization for exactly
one new 5+5+5 measurement campaign**. If authorized, freeze new prepared-input
and runner identities around candidate source `e52e0502`, follow the corrected
G4 campaign structure, apply the new certified-send work and round-coverage
gates, preserve the closed prior campaign, and stop after the single result.
Do not run it implicitly.

Separately, G3 still needs a corrected height-915 replay and an authorized
read-only height-924 validator directory. Missing height-924 access must not
cause idle waiting or broaden authorization.

## References

- [Certified-send tombstone bounding plan](../plans/active/certified-send-tombstone-bounding-plan.md)
- [Storage scaling milestone](../plans/active/storage-scaling-milestone.md)
- [Corrected G4 campaign failure](2026-08-28___postfiatchad__corrected_g4_campaign_failure.md)
- [Vote-lock index implementation handoff](2026-08-28___postfiatchad__vote_lock_index_fix.md)
- [`certified_send_completed_index.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/e52e050269a2f9fdd28c5083c3888debf3a85063/crates/node/src/certified_send_completed_index.rs)
- [`transport_cli.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/e52e050269a2f9fdd28c5083c3888debf3a85063/crates/node/src/transport_cli.rs)
- [`transport_runtime.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/e52e050269a2f9fdd28c5083c3888debf3a85063/crates/node/src/transport_runtime.rs)
- Runner branch `postfiatchad/corrected-g4-vote-lock-gate` at `15d059d1`
