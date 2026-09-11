# QA campaign log — 2026-09-11

This is the canonical progress record for the
[burn 3 campaign](qa-campaign-20260911-burn3-brief.md). The campaign began at
2026-09-11T10:43:34Z from clean `main` commit
`c25b3389d5c221ff1c23e700d53460003cc50b2a`. It is a review-and-repair
campaign, not a release, deployment, or live-authority action.

**Status:** in progress. Storage, execution, and Cobalt ratification are done;
network and mempool admission is the next surface in the mandated order.

## Campaign state

| Order | Surface | Status | P1 | P2 | P3 | Evidence or fixes |
| --- | --- | --- | ---: | ---: | ---: | --- |
| A1 | Storage and snapshots | done | 1 | 3 | 2 | [Review](storage-snapshots-review-20260911.md); findings `8533f5d7`, `0d015227`; repairs `69e1f1ce` |
| A2 | Execution | done | 1 | 2 | 0 | [Review](execution-review-20260911.md); findings `0449de41`; repairs `e95efbdf` |
| A3 | Cobalt ratification | done | 1 | 1 | 2 | [Review](cobalt-ratification-review-20260911.md); findings `b5c16c2c`; repairs `c9a61fcd` |
| A4 | Network and mempool admission | pending | 0 | 0 | 0 | — |
| A5 | Operational Python CLIs | pending | 0 | 0 | 0 | — |
| B | Defect inventory and TIH gate | pending | — | — | — | — |

Current finding totals: **3 P1, 6 P2, 4 P3**.

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
  source has been modified.
- No broad workspace or Orchard/Halo2 suite has been started; the storage unit
  does not change an Orchard boundary.

## Verification

The mandatory strict documentation, public-link, and public-secret gates run
before every campaign commit. Focused source results and pushed commit IDs are
added as each unit completes.

## Scores

The final inventory Text Improvement Harness gate is pending.
