# Storage Scaling and Bounded Finality Research Specification

**Status:** Text Improvement Harness full gate passed on 2026-08-26 — average 88.67/100 (GPT 91.20, Fable 85.60, GLM 89.20; five runs per lane; run group `storage-scaling-research-spec`); scored content SHA-256 `573788c9d39239aa53523c1f3b7bb8063a00e0c39526927f548e580163768916`; Task Node lock pending operator decision
**Date:** 2026-08-26
**Decision owner:** Post Fiat
**Author:** Domagoj Ravlić (`dravlic`)
**Prior work:** [State And Storage](state-and-storage.md), [Evidence Model](evidence-model.md), [Cobalt adversarial-verification results](../governance/cobalt-adversarial-verification-results.md), [Current State](../status/chain-state-current.md), [E3 adversarial-recovery packet](https://github.com/postfiatorg/postfiatl1v2/tree/main/benchmarks/cobalt-adversarial-verification/e3), [E4 finality-isolation packet](https://github.com/postfiatorg/postfiatl1v2/tree/main/benchmarks/cobalt-adversarial-verification/e4), [915-block release qualification](https://github.com/postfiatorg/postfiatl1v2/blob/main/benchmarks/cobalt-handoff-rehearsal/release-qualification-v1.json)
**Decision scope:** replace chain-length-dependent work in the synchronous storage and proposal paths with bounded O(1) or O(log n) work per height while preserving deterministic state commitments, authenticated history, crash recovery, and byte-identical replay of the existing chain

## Plain-English directive

The controlled-devnet JSON/JSONL store is not ready for a public testnet. E4
made the limiting behavior measurable: its first 50 baseline rounds reproduced
the activation run's `consensus_round_ms` p95 at 1,664 ms versus 1,660 ms, but
latency then rose almost linearly with height to about 14.9 seconds at round
500, with correlation approximately 0.9998. Those absolute values are not a
finality service-level commitment.

The synchronous path repeatedly pays for all prior history. In
`crates/storage/src/lib.rs`, every JSONL append calls `read_jsonl_tail`,
which starts at genesis and verifies the entire keyed hash chain before writing
one record and its authenticated head. Proposal construction reads the full
ordered-batch history, tests membership by scanning it, and hashes that growing
list into the state root. Every voting validator then rebuilds the proposal from
its local state and compares it byte for byte before voting.

The required outcome is a storage and proposal design whose work per new height
does not grow with chain length. An append must touch O(1) authenticated tail
state; an indexed lookup may touch O(log n) pages; proposal construction,
validation, and state commitment must use bounded current-state summaries
instead of rebuilding full ordered history. Full history remains available and
independently replay-verifiable.

This is a storage, state-commitment, and proposal-path change. It does not change
Cobalt authority or any Consensus v2 rule: validator membership, proposer
rotation, prepare/precommit votes, quorum, locks, timeout certificates, commit
certificates, and receipt success semantics remain unchanged. Any new
state-commitment encoding requires an explicit version and activation height;
the legacy encoding remains authoritative for replay below that boundary.

No public testnet may launch until every gate in this specification passes.
This document authorizes research and evidence design only. It does not
authorize implementation deployment, a live migration, or a Task Node lock.

## Claims, evidence, and gaps

| Claim | Evidence today | Gap | Closes it |
| --- | --- | --- | --- |
| E4's 5% stress gate passed | Same-length 500-round baseline and attack lanes had p95 `wallet_to_finality_ms` of 14,133.574 ms and 14,197.471 ms, a +0.452099% delta | The paired gate measures relative Cobalt stress overhead, not acceptable absolute finality | E1 and E3 |
| Initial local finality is about 1.66 seconds | E4's first 50 baseline rounds have `consensus_round_ms` p95 about 1,664 ms; activation measured 1,660 ms over 50 rounds | E4 then grows to about 14.9 seconds by round 500; no evidence exists at heights 1,000 or 5,000 | E1 |
| JSONL append history is integrity checked | Each record binds the prior MAC and canonical payload; the authenticated head detects deleted tails; append records and heads are fsynced | `read_jsonl_tail` rescans and re-MACs the full log on every append; crash recovery relies on a full scan | E1 and E2 |
| Proposal votes bind locally verified state | Each validator rebuilds the unsigned proposal from its local batch and state, then compares it before voting | Rebuild reads, scans, clones, and commits the full ordered-batch list into every state root | E1 and E2 |
| Archived history is deterministic and replayable | The release lineage replayed all 915 archived controlled-devnet blocks to the expected tip and state root | A new store and commitment format have not replayed the archive or the authenticated live history through height 924 | E3 |
| Tampered history fails closed | E3 rejected 24 truncated, padded, reordered, and modified histories and 18 forged catch-up cases; six recoveries were byte-identical | A persisted tail checkpoint and indexed store introduce new crash and rollback states that E3 did not cover | E2 and E3 |
| The running six-validator devnet can change storage safely | Existing operator rules require every live authority transition to be rehearsed on a disposable clone | There is no storage-format migration, mixed-version, rollback, or clone-rehearsal packet | E4 |

## Decision question

> Can PostFiat replace full-prefix JSON/JSONL verification and full ordered-history proposal reconstruction with bounded authenticated tail state, a deterministic indexed lookup path, and a versioned fixed-size ordered-history commitment, while replaying the existing 915-block archive and authenticated live history byte for byte, rejecting every existing and new tamper case, and keeping p95 finality at height 5,000 within 10% of height 50?

The answer is yes only when the evidence proves both complexity and behavior.
A faster benchmark that weakens integrity, replay, determinism, crash recovery,
or proposal validation fails.

## Scope

### In scope

- `crates/storage/src/lib.rs`, its integrity envelopes and authenticated JSONL
  heads, log compaction, mutation locks, and ordered-commit journal.
- The blocks, receipts, batch archive, ordered-batch history, chain tip,
  snapshots, retained-history roles, and replay paths.
- Proposal construction in `crates/node/src/mempool_proposals.rs` and
  `crates/node/src/batch_snapshot.rs`.
- Local proposal reconstruction and comparison before voting in
  `crates/node/src/block_finality.rs`.
- Ordered commit, state-root construction, duplicate detection, recovery, and
  versioned migration in `crates/node/src/storage_commit.rs`,
  `crates/node/src/state_commitment.rs`, and `crates/node/src/history.rs`.
- The existing six-validator local harness, the exact 915-block archived
  lineage, and the authenticated controlled-devnet history through height 924.
- A bounded JSONL-tail implementation and at least one embedded indexed-store
  implementation behind the same typed storage interface.

### Out of scope

- Changing Cobalt trust, authority, ratification, or proposal rules.
- Changing Consensus v2 membership, proposer selection, prepare/precommit
  phases, quorum, locks, timeouts, certificates, or finality semantics.
- Rewriting historical state roots, blocks, receipts, transaction IDs, or
  certificates.
- Treating an index, cache, checkpoint, benchmark report, or hash manifest as a
  substitute for full archive replay.
- Public-testnet launch, mainnet authorization, or live migration before the
  clone and evidence gates pass.
- Unrelated Orchard/Halo2, NAVCoin, settlement-lane, RPC, or wallet features.

## Integrity, crash, and replay invariants

Both candidate stores must enforce these invariants:

1. **Domain binding:** every tail checkpoint binds its schema and storage format,
   chain ID, genesis hash, protocol version, log kind, record count, byte offset,
   current chain MAC, previous chain MAC, finalized height, block hash, and state
   root under the node-local integrity key. A checkpoint from another log,
   chain, format, or height fails closed.
2. **Bounded append:** under a log-scoped exclusive lock, append verifies the
   authenticated checkpoint and file length, validates only a fixed-size crash
   suffix, writes one canonical envelope, fsyncs the log, atomically replaces
   and fsyncs the checkpoint, and completes through the ordered-commit journal.
   It never scans the accepted prefix in the synchronous path.
3. **Crash recovery:** test every interruption before and after journal write,
   record write, log fsync, checkpoint publish, chain-tip publish, and journal
   removal. Recovery may accept the old committed state or finish exactly one
   journaled commit. It may truncate only an unauthenticated partial suffix
   after the checkpoint. More than the bounded suffix, a mismatched complete
   record, or an ambiguous state fails closed without mutation.
4. **Rollback detection:** the chain tip and ordered-commit journal bind the
   checkpoints for all affected logs. Missing, substituted, independently
   rolled-back, or jointly inconsistent log/head pairs reject. Whole-store
   rollback is resolved against an authenticated snapshot or certified peer
   history, never local timestamps.
5. **Index derivation:** indexes are versioned, bounded, and derived from the
   authenticated record stream. Keys and values use canonical encodings; no
   unordered database iteration enters consensus. A missing or corrupt index is
   rejected or rebuilt and verified from history before use.
6. **State commitment:** ordered-batch membership and sequence use an indexed
   set plus a fixed-size append-only accumulator for new heights. The
   accumulator has a domain-separated version and explicit activation height.
   Legacy heights continue to hash the historical ordered-batch list exactly as
   before.
7. **Replay:** a full verifier can still authenticate every record from genesis
   or an authenticated snapshot, recompute every historical state root and
   receipt, and reproduce canonical output without trusting the fast checkpoint
   or index.
8. **Failure atomicity:** no rejected append, proposal, index update, restart,
   migration, or tamper case changes durable consensus state.

## Experiment 1 — freeze and measure the current cost model

Build a frozen benchmark harness from the current release-mode path. Create
authenticated snapshots at starting heights 50, 100, 500, 1,000, and 5,000 with
the same six-validator topology, keys, full-vote policy, transaction shape,
binary, host allocation, storage medium, retry policy, and instrumentation.
Run five independent 50-round windows from each snapshot.

Record p50 and p95 `wallet_to_finality_ms`, `consensus_round_ms`, proposal
construction, per-validator proposal rebuild/compare, state-root construction,
duplicate lookup, ordered-commit journal, and each JSONL append. Record bytes
read and written, records MAC-verified, index or list entries visited, fsync
count and time, CPU, RSS, disk, and network. Freeze source hashes, binary hash,
snapshot roots, harness seed, host description, and raw per-round receipts.

Fit the measured work to constant, logarithmic, and linear models. Publish the
raw observations and residuals; do not infer complexity from elapsed time alone.
The instrumentation counter for prefix bytes and records is the controlling
complexity evidence.

### Required result

- The frozen harness reproduces the E4 first-50 behavior within 10% and shows the
  current height-dependent curve at all required heights.
- Every material synchronous contributor is attributed to a named source stage,
  including `read_jsonl_tail`, ordered-batch read/membership/state-root work,
  proposal construction, remote rebuild/compare, and commit fsync.
- Repeated runs use identical inputs and disclose variance, host load, compiler,
  storage device, binary, and snapshot identity.
- The corpus and measurement definitions are checksum-bound before candidate
  implementation work begins.

## Experiment 2 — implement bounded integrity and indexed proposal state

Implement two selectable candidates behind one typed storage interface:

1. **Bounded JSONL:** retain canonical append envelopes, add the persisted
   domain-bound tail checkpoint and bounded crash-suffix recovery above, and
   keep full-prefix verification as an explicit audit/replay operation.
2. **Indexed store:** implement at least one embedded ordered key-value or
   B-tree candidate with atomic write batches, versioned canonical keys,
   authenticated record values, deterministic range order, snapshot/export,
   corruption detection, and a reproducible rebuild from the canonical log.

For both candidates, replace linear ordered-batch membership scans with indexed
lookups. Introduce the versioned ordered-history accumulator needed to keep
proposal construction, validation, and state commitment bounded after its
activation height. Keep legacy replay logic byte-for-byte compatible below the
boundary.

Extend the E3 adversarial-recovery corpus to both candidates: truncated,
padded, reordered, and one-entry-modified durable history; fabricated
transitions; wrong-root certificates; omitted latest updates; interrupted
catch-up; missing or replaced checkpoints; checkpoint/log substitution;
stale-but-valid head replay; corrupted index pages; journal/head disagreement;
and every defined crash cut point.

### Required result

- Synchronous JSONL append verifies and reads a fixed maximum number of suffix
  records and bytes, independent of height; indexed reads and writes touch no
  more than O(log n) pages.
- Proposal duplicate lookup, ordered-history update, state commitment, and
  validator rebuild/compare perform O(1) or O(log n) history work and never
  materialize the full ordered-batch list after activation.
- Every crash cut recovers the old commit or exactly one new commit with no
  duplicate, omission, partial mutation, or manual repair.
- Every original E3 case and every new checkpoint/index case rejects with a
  named reason before rejoin, with zero rejected-state mutation.
- Full verification from genesis produces the same authenticated record
  sequence as the fast path, and an index rebuilt from that sequence is
  byte-identical to a clean index build.

## Experiment 3 — paired scaling and exact replay

Using the frozen E1 harness, run legacy JSONL, bounded JSONL, and indexed-store
lanes from the same authenticated snapshot at each starting height. Each pair
uses the same binary with only the storage mode changed, the same host,
topology, keys, transactions, full-vote policy, CPU allocation, and five
independent 50-round windows. Compare all primary, secondary, and stage metrics;
do not compare lanes of different lengths or heights.

Replay the exact 915-block migrated quarantine archive and the authenticated
controlled-devnet history through height 924 through each candidate. Below the
new activation boundary, require byte-identical canonical blocks, receipts,
batch archive, ordered-batch history, tip, state root, and replay manifest. For
post-boundary synthetic history, require both candidates to produce identical
versioned commitments and consensus artifacts from identical signed inputs.

Run the complete original and extended E3 tamper/recovery corpus against the
replayed stores and restart each store at every checkpoint boundary.

### Required result

- For the selected path, aggregate p95 `consensus_round_ms` and
  `wallet_to_finality_ms` across the height-5,000 windows are each no more than
  110% of their corresponding height-50 p95; no material synchronous stage has
  a positive linear relationship with height.
- Instrumentation proves fixed-prefix work for bounded JSONL and O(log n) or
  better work for indexed lookup/update. No full history read, clone, scan, or
  hash occurs in proposal construction, voting, or commit after activation.
- Replay of all 915 archived blocks and authenticated history through height 924
  is byte-identical and reaches the recorded tips and state roots.
- Every original E3 tamper case and every new case rejects; every honest recovery
  restores byte-identical accepted history without manual mutation.
- All six validators converge in every lane, every receipt is checked for
  literal acceptance, and Consensus v2 never stops or forks.

## Experiment 4 — rehearse migration and rollback

Write a versioned migration and rollback runbook for the running six-validator
controlled devnet. The migration builds the candidate store side by side from
an authenticated snapshot plus certified history, fully replays it, compares
canonical state, and retains the old store read-only through a stated rollback
window. Mixed software must fail closed on an unknown storage or commitment
version; no node may vote across an unrecognized activation boundary.

Before any live action, rehearse the exact six-validator sequence on disposable
clones bound to the current chain, registry, authority history, trust state,
validator identities, deployed binary, and latest authenticated tip. Rehearse
preflight, side-by-side build, restart, staggered rollout, activation, one
post-activation block, rollback before activation, separately authorized
forward recovery after activation, catch-up, and final all-six convergence.
This applies the standing clone-first rule in `docs/governance/cobalt.md`.

A post-activation rollback must not reinterpret or delete finalized blocks. It
uses a compatible reader or deterministic reverse export and resumes from the
same certified tip. The old directory remains recoverable and immutable until
the evidence-bound rollback window closes.

### Required result

- The runbook names every command, owner, compatibility check, stop condition,
  backup, disk requirement, activation height, rollback boundary, and evidence
  artifact.
- The disposable-clone migration and both rollback branches pass before a live
  change is scheduled.
- Every clone replays to the expected pre-migration tip, finalizes after
  migration, restarts, catches up, and converges with identical state and
  storage-version receipts.
- Cobalt authority and all Consensus v2 rules remain unchanged; the evidence
  records only a storage/state-commitment activation.
- No live validator is queried, restarted, or mutated until a separately
  authorized implementation milestone reaches this gate.

## Gates

### PASS

Storage scaling passes only when all of the following are true:

- E1 freezes a reproducible current-path cost model at heights 50, 100, 500,
  1,000, and 5,000 and attributes the linear work.
- E2 proves bounded append, bounded proposal history work, crash atomicity,
  deterministic index rebuild, and rejection of every original and extended E3
  case.
- E3 keeps height-5,000 p95 finality within 10% of height 50, removes every
  synchronous linear-history stage, and replays both the 915-block archive and
  authenticated height-924 history byte for byte.
- E4 passes the exact clone migration, activation, restart, catch-up, and
  rollback rehearsals with all six validators converged.
- The CLI, browser, evidence packet, verifier, redaction scan, and publication
  below pass from a clean checkout.

### PUBLIC TESTNET BLOCKED

Until PASS, the JSON/JSONL configuration remains controlled-devnet research
software. Do not launch a public testnet, publish a finality SLA, deploy the new
store to the live fleet, or describe the scaling problem as fixed.

### REMEDIATION

Any failed gate creates P0 work in the owning storage, state-commitment,
proposal, validation, replay, or migration boundary. Preserve the frozen E1
corpus and failed receipt, add a named regression, and rerun the unchanged
affected experiment from clean state. Do not weaken an integrity, replay,
crash, or latency gate to obtain PASS.

## Required evidence packet

- `storage-scaling-status.json` with `PASS`, `REMEDIATION_REQUIRED`, or
  `PUBLIC_TESTNET_BLOCKED`, plus exact source, binary, format, activation, and
  snapshot identifiers.
- Frozen E1 harness, configuration, seeds, snapshots, source hashes, raw
  per-round timings, instrumentation counters, host/resource receipts, fitted
  cost models, and variance report.
- Versioned storage schemas; checkpoint and index canonical encodings; write,
  fsync, recovery, and activation state machines; and the explicit complexity
  argument.
- Per-case results for the original E3 corpus, the extended checkpoint/index
  corpus, and every crash cut, including before/after durable hashes and named
  rejection reasons.
- Paired legacy/bounded/indexed receipts at every height, with equal-length
  validation, six-validator convergence, receipt acceptance, and stage metrics.
- Byte-identical 915-block and height-924 replay manifests, expected and observed
  tips/state roots, and independent static verification.
- Clone migration, activation, restart, catch-up, rollback, disk-capacity, and
  unchanged-Cobalt/Consensus v2 receipts.
- CLI and browser outputs, `SHA256SUMS.txt`, redaction report, and a verifier
  that fails on missing, mutated, inconsistent, incomparable, or unbounded
  evidence.

## Human interfaces

Per the developer mandate, the implementation milestone delivers the CLI before
the browser interface:

1. A Python command such as
   `python -m postfiat_rpc.storage_scaling verify <packet>` that a human can
   run to authenticate the packet, display the height curve and complexity
   counters, compare paired lanes, verify archive replay, list every tamper and
   crash result, and report the migration gate with named failures.
2. A read-only browser view using the same verified packet that shows current
   versus candidate latency by height, per-stage work, append/index complexity,
   replay identity, recovery results, clone-rehearsal state, and the public
   testnet block. It exposes no migration, activation, rollback, or mutation
   route.

Both interfaces fail closed on checksum, schema, source pin, lane comparability,
replay, tamper, migration, or status inconsistency.

## Required publication

- Update [State And Storage](state-and-storage.md), the evidence index, current
  state, operator runbooks, and public benchmark text with the selected design
  and exact evidence packet.
- State on the first page that E4 exposed linear chain-height finality growth,
  that its 5% gate remains a valid same-length A/B stress comparison, and that
  its absolute latency is not a finality SLA.
- Publish p50 and p95 by starting height, per-stage work counters, host and
  storage setup, variance, archive replay result, tamper result, migration
  status, and remaining limits.
- Until live fleet receipts exist, say “qualified on disposable clones,” not
  “deployed.” Until every PASS gate holds, say “public testnet blocked.”
- State plainly that the work changes storage and a versioned state commitment,
  not Cobalt authority or Consensus v2 rules.

## Decisions recorded

1. The E4 height curve is a release blocker for any public testnet, even though
   the paired 5% Cobalt-stress gate remains valid.
2. The synchronous append path may trust only a domain-bound authenticated
   checkpoint plus a bounded crash suffix; full-prefix verification remains
   mandatory for audit, migration, index rebuild, and replay.
3. The implementation evaluates both bounded JSONL and an embedded indexed
   store, then selects by the complete integrity, replay, operations, and
   performance gates rather than latency alone.
4. Ordered history remains archived. New-height proposal and state-root work
   uses a versioned fixed-size accumulator and index; legacy replay retains the
   exact historical list encoding.
5. No Cobalt or Consensus v2 protocol rule changes. Any state-commitment change
   is explicit, versioned, activation-bound, replayable, and separately
   evidenced.
6. The operator decides whether to request the Task Node lock. This scoring
   task makes no Task Node request and authorizes no implementation or live
   migration.

## Work sequence

1. Run the Text Improvement Harness full scoring gate with five runs in each
   selected lane. Rewrite through a direct OpenRouter
   `openai/gpt-5.6-sol-pro` call only while the average is below 86/100, keep
   E1-E4 and their gates intact, and stop at the first compliant score.
2. Record the lane scores, average, run group, and scored document SHA-256 in
   the Status line. Leave the Task Node lock pending the operator's decision.
3. If the operator approves, request the lock as a Task Node task, inspect the
   proposed task, and complete the Task Node lifecycle.
4. After locking, request conversion to a concise milestone document as a Task
   Node task.
5. Execute E1 through E4 under substantial Task Node work, deliver the CLI
   before the read-only browser, publish the checksum-bound packet, and retire
   the completed milestone only after the interfaces and publication pass.
