# Vote locks and view recovery review — 2026-09-16

This is A2 of the [burn 5 campaign](qa-campaign-20260916-burn5-brief.md). The review read all 2,006 lines of `crates/node/src/vote_locks.rs` (1,371), `finality_view_recovery.rs` (458), `storage_vote_guard.rs` (124), and `node_types_block_vote_timing.rs` (53) at `6888b4e1`. The focus was lock bypass or early release, cross-view equivocation, crash recovery, timing assumptions, and persistence/reload. Source references below describe that revision. No other source implementation was reviewed as A2.

## Findings

### VLK-01. P1 — successful lock publication omits the directory durability barrier

**Source:** `crates/node/src/vote_locks.rs:123-130,548-592`.

`write_new_block_proposal_vote_lock` writes a temporary file, creates the canonical lock name with `hard_link`, removes the temporary name, and returns success without syncing the containing directory after those namespace changes. The earlier temporary-file write cannot make a later hard link durable. Reservation then returns permission to continue signing. A machine crash after that return but before the directory changes reach stable storage can lose the canonical lock; after restart, a conflicting proposal at the same slot can obtain a fresh reservation. An ordinary process restart test does not exercise this crash window. Same-proposal retries also return without explicitly establishing durability of the retained canonical name.

The minimal repair is to sync the lock directory after publishing or validating the canonical lock and before reservation returns success, propagating sync failures while retaining the lock and holding the mutation guard. Regress a failed directory sync on first reservation and an identical retry, verify that the canonical record exists before the barrier, and verify that a conflicting retry after reopening the store remains blocked. This tightens signer-safety persistence and is conservatively **consensus-affecting**; it changes neither serialized lock format nor signed bytes. No activation or live behavior is claimed.

### VLK-02. P2 — migration ignores non-regular JSON lock entries before marking completion

**Source:** `crates/node/src/vote_locks.rs:224-261`.

The migration scan retains an entry only when it is both a regular file and has the `.json` extension. A malformed restored lock directory containing an arbitrarily named `.json` symlink to a legacy conflicting lock, or a directory at a `.json` lock name, is silently skipped. If no regular lock remains, migration writes its completed marker and a conflicting reservation can proceed without resolving that ambiguous state. With other regular locks present, the ambiguous entry is still omitted from the migration preflight. This differs from direct canonical-path reads, which reject non-regular entries. The scenario requires malformed local/restored state; no remote filesystem-write capability is claimed.

The minimal repair is to reject every non-regular `.json` entry during the read-only migration scan, without following symlinks, publishing a completion marker, deleting evidence, or reserving a new lock. Regress both a symlink to conflicting lock evidence and a directory entry, including the case with an ordinary lock alongside it. This tightens signer admission on ambiguous local state and is conservatively **consensus-affecting**; the on-disk format and signed bytes remain unchanged. The repair requires no excluded file.

## Areas with no findings

- `vote_locks.rs:84-131,133-191,264-410,438-545,595-685`: no additional findings in cross-process mutation serialization, chain/genesis/protocol binding, same-slot conflict checks, legacy height-wide versus activated per-view path separation, migration conflict preflight, preservation of canonical copies before source cleanup, or malformed canonical record/marker rejection. These observations remain subject to VLK-01 and VLK-02. The existing tests cover idempotence, ordinary restart, interrupted migration, conflicts, bounded normal lookup, and competing first reservations.
- `finality_view_recovery.rs:1-458`: requested height increments are checked; legacy nonzero views are rejected; the RPC attempt window is bounded; view zero rejects timeout envelopes; later views require chunk decoding and delegate certificate aggregation for the preceding view. The decoded envelope has a 2 MiB limit and its vote count is checked against validator count. The RPC mutex rejects contention/poisoning before timeout signing. Cryptographic and durable signer behavior in delegated implementations was not re-reviewed.
- `storage_vote_guard.rs:1-124`: the reviewed guard fails closed on propagated read failures, activated zero height, parent-height/hash mismatches, transactional metadata binding mismatches, absent current-state components, and inconsistent parent/ordered-tip metadata. The JSONL path compares the current replicated-state root with its tip. Delegated storage coherence and root calculations remain outside this review.
- `node_types_block_vote_timing.rs:1-53`: timing fields are report data; new lock-work fields default for legacy reports. Wall-clock use within `vote_locks.rs` constructs a temporary filename, not a timeout deadline or lock-expiry rule. No lock-expiry or early-release timer appears in these four files.

## Review limits and skips

This review covers the four named files in full, including their in-file tests. It does not certify every caller or callee: activation resolution, signature/QC/TC verification, cross-phase round floors, actual signature release, shared atomic-write durability, backend transactions and coherent snapshots, state-root calculation, RPC outer request limits, and historical replay were not reviewed. In particular, the activated per-view lock assumes timeout verification by its caller; it is not itself a cross-view QC lock. No claim is made that the storage guard independently revalidates all transactional state contents.

Release-candidate files and directories enumerated in the brief, previously reviewed surfaces, all excluded crates, and frozen artifacts were not reviewed or edited. `git diff` read only remote-ref path metadata to identify the additional release exclusions. No branch checkout or release-checkout access occurred. Build manifests and the repository guidance supplied context, not additional reviewed surfaces. No finding requires an excluded-file repair.

The regressions proposed above exercise deterministic filesystem error handling and restart interlocks; they do not constitute a physical power-loss test. The manual ignored release-mode spot check, full Rust/Orchard suites, and live-chain or fleet tests are outside this focused unit. The full Rust suite is CI's verdict. A3–A5 and B, including the defect inventory and scoring, are not started. No Task Node or fleet action is authorized or performed.
