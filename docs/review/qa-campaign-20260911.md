# QA campaign log — 2026-09-11

This is the canonical progress record for the
[burn 3 campaign](qa-campaign-20260911-burn3-brief.md). The campaign began at
2026-09-11T10:43:34Z from clean `main` commit
`c25b3389d5c221ff1c23e700d53460003cc50b2a`. It is a review-and-repair
campaign, not a release, deployment, or live-authority action.

**Status:** in progress. Storage and snapshots are done; execution is the next
surface in the mandated order.

## Campaign state

| Order | Surface | Status | P1 | P2 | P3 | Evidence or fixes |
| --- | --- | --- | ---: | ---: | ---: | --- |
| A1 | Storage and snapshots | done | 1 | 3 | 2 | [Review](storage-snapshots-review-20260911.md); findings `8533f5d7`, `0d015227`; repairs `69e1f1ce` |
| A2 | Execution | pending | 0 | 0 | 0 | — |
| A3 | Cobalt ratification | pending | 0 | 0 | 0 | — |
| A4 | Network and mempool admission | pending | 0 | 0 | 0 | — |
| A5 | Operational Python CLIs | pending | 0 | 0 | 0 | — |
| B | Defect inventory and TIH gate | pending | — | — | — | — |

Current finding totals: **1 P1, 3 P2, 2 P3**.

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
