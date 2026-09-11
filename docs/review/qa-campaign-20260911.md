# QA campaign log — 2026-09-11

This is the canonical progress record for the
[burn 3 campaign](qa-campaign-20260911-burn3-brief.md). The campaign began at
2026-09-11T10:43:34Z from clean `main` commit
`c25b3389d5c221ff1c23e700d53460003cc50b2a`. It is a review-and-repair
campaign, not a release, deployment, or live-authority action.

**Status:** campaign closed with A5 and B outstanding. A1 through A4 are done;
A5 and B were not started.

## Campaign state

| Order | Surface | Status | P1 | P2 | P3 | Evidence or fixes |
| --- | --- | --- | ---: | ---: | ---: | --- |
| A1 | Storage and snapshots | done | 1 | 3 | 2 | [Review](storage-snapshots-review-20260911.md); findings `8533f5d7`, `0d015227`; repairs `69e1f1ce` |
| A2 | Execution | done | 1 | 2 | 0 | [Review](execution-review-20260911.md); findings `0449de41`; repairs `e95efbdf` |
| A3 | Cobalt ratification | done | 1 | 1 | 2 | [Review](cobalt-ratification-review-20260911.md); findings `b5c16c2c`; repairs `c9a61fcd` |
| A4 | Network and mempool admission | done (repaired) | 2 | 1 | 1 | [Review](network-mempool-review-20260911.md); findings `13a23d91`; repairs `f2dea308` |
| A5 | Operational Python CLIs | not started | — | — | — | — |
| B | Defect inventory and TIH gate | not started | — | — | — | — |

Current finding totals: **5 P1, 7 P2, 5 P3**.

## Network and mempool admission review result

The crate review and node reachability trace found two P1 defects in the
long-running validator transport. The service could spawn one operating-system
thread for every pre-authentication TCP connection up to its lifetime budget,
and one unauthenticated persistent connection could submit unlimited rejected
frames while every status-bearing rejection was retained in memory and appended
to the optional event log. One P2 affected the standalone batch service: only
successful batches consumed its termination budget, so unauthenticated
rejections could grow its report without bound. The [network and mempool review](network-mempool-review-20260911.md)
also records one unfixed P3: the unreachable legacy ordering API can
deserialize a validator set with a caller-selected false quorum.

Repair `f2dea308` limits the validator service to 16 simultaneous connection
workers independently of its lifetime connection budget, caps each connection
at 4,096 requests, closes a connection after a rejection, and retains at most
1,024 response or rejection summaries while maintaining exact saturating
counters. The standalone batch service now derives a bounded rejection budget
from `max_batches` and fails closed when it is exhausted. No A4 repair is
consensus-affecting.

Verification:

- `cargo check -p postfiat-node --locked`: passed.
- Focused validator accept-loop and resource-bound tests: 5 passed, including
  the new bounded-resource regression.
- Pre-repair crate baseline: 9 network, 15 mempool DAG, and 32 ordering tests
  passed.

## Cobalt ratification review result

The Cobalt pass found one P1: old/new safety witnesses compare raw subset
membership overlap rather than the minimum possible overlap of valid quorums.
A five-of-seven single rotation can therefore pass with an old/new quorum
intersection no larger than the Byzantine budget. One P2 is recorded: signed
DABC pending pairs bind a candidate ID, but activation checks only that their
slot is ratified. Both repairs tighten ratification or transition admission and
will be labeled consensus-affecting. The [Cobalt review](cobalt-ratification-review-20260911.md)
also records two unfixed P3s: the unused live-mode beacon abstraction has no
authentication material, and the frozen first-oracle input validator permits
incomplete validator classifications.

Pre-repair verification:

- `cargo test -p postfiat-consensus-cobalt -p postfiat-cobalt-decision-oracle -p postfiat-cobalt-adversarial-oracle --locked`:
  72 Cobalt tests, 9 genesis registry checker tests, and 3 tests in each oracle
  passed.

Repair `c9a61fcd` now evaluates the minimum possible old/new quorum
intersection and binds every signed DABC pending candidate ID to the ratified
candidate at that slot. Both changes are consensus-affecting and remain
source-only.

Post-repair verification:

- Cobalt: 73 tests; genesis registry checker: 9 tests; both oracles: 3 tests
  each.
- The focused safety-witness example passed all six checks.
- Node Cobalt authority: 1 test; Cobalt shadow/runtime: 15 tests.
- Strict clippy for the Cobalt crate and both oracles, plus workspace
  formatting, passed.

## Execution review result

The full execution crate and its node-side state-transition, archive-replay,
proposal, commit, and validator-registry entry points were reviewed. The pass
found one P1: a repeated transaction produces duplicate receipt IDs that
proposal construction and validation accept, although ordered commit rejects
them after certification. Two P2s are recorded: the ordered OwnedDeposit path
bypasses the declared 100,000-object cap, and proposal construction can certify
a replicated state value above the storage layer's 256 MiB serialization
limit. All three repairs affect consensus admission or state-transition results
and will be labeled consensus-affecting. The findings are detailed in the
[execution review](execution-review-20260911.md).

Pre-repair verification:

- `cargo test -p postfiat-execution --locked`: 196 passed.

Repair `e95efbdf` rejects duplicate receipt IDs at proposal construction and
supplied-proposal validation, applies the declared object cap to every owned
value path, and performs a write-free exact state-file-size check before a
proposal can expose an unpersistable state root. The duplicate and size checks
cover validator reconstruction as well as local proposal creation. No on-disk
format changed.

Post-repair verification:

- `cargo test -p postfiat-execution --locked`: 198 passed.
- `cargo test -p postfiat-storage --locked`: 90 passed, 2 ignored; process-crash
  integration: 1 passed.
- Duplicate-receipt proposal regressions: 2 passed; serialized state-size
  regression: 1 passed.
- Node block-proposal tests: 3 passed; validator-registry continuation tests: 6
  passed.
- Focused transparent asset, replay, NFT, offer, and atomic-swap ordering tests:
  5 passed.
- Strict clippy for `postfiat-execution`, `postfiat-storage`, and
  `postfiat-node`, and the workspace formatting check, passed.

All three A2 repairs are consensus-affecting. They are source-only and were not
activated or deployed.

## Storage review result

The full `crates/storage` surface and the node snapshot, restore, checkpoint,
migration, commit-recovery, and writer-lease paths were reviewed. The pass found
one P1 source defect: a torn FastSwap WAL suffix remains in place, so a later
synced record can be appended behind it and become unreplayable. It also found
two P2 source defects: failed snapshot imports can publish partial destination
state, and FastSwap WAL reads can allocate an unbounded whole file while total
WAL growth has no fence. Two comparison-only P3 defects are recorded without
repair: destructive ordered-index replacement and a duplicate legacy receipt
materialization write. The block-924 snapshot source repair is present in the
deployed source ancestry, but no post-repair fleet export receipt exists; the
remaining P2 evidence gap cannot be closed without a prohibited host write.

Repair `69e1f1ce` now truncates torn FastSwap WAL tails before later appends,
fences total WAL and bounded-file reads before allocation, and publishes a
snapshot or complete lifecycle validator root only after every verification
passes. Transactional generation pointers are rebound before the atomic move,
so active-storage restores remain usable at the final path. The mutable chain
state now separates deployed source repair ancestry from the missing fleet
export receipt. No A1 repair is consensus-affecting.

Pre-repair verification:

- `cargo test -p postfiat-storage --locked`: 88 passed, 2 ignored; process
  crash integration: 1 passed.
- `cargo test -p postfiat-node snapshot --lib --locked`: 22 passed.
- `cargo test -p postfiat-node lifecycle_checkpoint --lib --locked`: 7 passed.

Post-repair verification:

- `cargo test -p postfiat-storage --locked`: 89 passed, 2 ignored; process
  crash integration: 1 passed.
- `cargo test -p postfiat-node snapshot --lib --locked`: 24 passed.
- `cargo test -p postfiat-node lifecycle_checkpoint --lib --locked`: 7 passed.
- `cargo test -p postfiat-node validator_registry_continuation --lib --locked`:
  6 passed.
- Focused transactional migration regressions: 2 passed.
- Strict clippy for `postfiat-storage` and `postfiat-node`, and the workspace
  formatting check, passed.

## Skips and boundary decisions

- No Task Node action occurred.
- No fleet, chain, deployment, restart, configuration, or remote-host mutation
  occurred.
- A post-repair fleet snapshot export was skipped because it would write to a
  validator host. Source ancestry and local regressions do not substitute for
  that operational evidence.
- Ordered-history index publication and the duplicate legacy receipt write are
  P3 findings, so they are recorded but not repaired.
- No frozen artifact or out-of-scope bridge, proof, program, or Orchard/privacy
  source was modified.
- No broad workspace or Orchard/Halo2 suite was started; no burn 3 repair
  crossed an Orchard boundary.
- A5 and B were not started during the time-limited closeout.

## Verification

The mandatory strict documentation, public-link, and public-secret gates passed
before every campaign commit. Focused source results and pushed commit IDs are
recorded with each completed surface.

## Scores

The final inventory Text Improvement Harness gate was not run because B was not
started.

## Final summary

| Surface | P1 | P2 | P3 | Repair commits |
| --- | ---: | ---: | ---: | --- |
| A1 — Storage and snapshots | 1 | 3 | 2 | `69e1f1ce` |
| A2 — Execution | 1 | 2 | 0 | `e95efbdf` |
| A3 — Cobalt ratification | 1 | 1 | 2 | `c9a61fcd` |
| A4 — Network and mempool admission | 2 | 1 | 1 | `f2dea308` |
| A5 — Operational Python CLIs | not started | not started | not started | — |
| **Completed-surface total** | **5** | **7** | **5** | — |

Consensus-affecting repairs, all source-only and not activated or deployed:

- Duplicate receipt-ID rejection during local proposal construction and
  supplied-proposal validation — `e95efbdf`.
- The declared owned-object cap applied to every owned value path, including
  `OwnedDeposit` — `e95efbdf`.
- Exact serialized state-size admission before an unpersistable state root can
  be proposed or accepted — `e95efbdf`.
- Minimum possible old/new quorum intersection required by Cobalt transition
  safety witnesses — `c9a61fcd`.
- Signed DABC pending candidate IDs bound to the ratified candidate at each
  slot — `c9a61fcd`.

Remaining risks:

- A1 still lacks a post-repair fleet snapshot export receipt, and retains the
  P3 destructive ordered-index replacement and duplicate legacy receipt write.
- A3 retains the P3 unauthenticated live-mode beacon abstraction and incomplete
  first-oracle validator classifications.
- A4 retains the P3 deserializable legacy validator set with a false quorum;
  no unauthenticated production reachability was found.
- A5 remains unreviewed. B has not added burn 3 findings to the consolidated
  defect inventory or run its Text Improvement Harness gate.
- All burn 3 repairs remain source-only; no deployment or live activation was
  performed.

The campaign closed early with A5 and B as the resume point.
