# Execution review — 2026-09-11

This review covers `crates/execution` in full and the transparent,
governance, validator-registry, proposal, replay, and ordered-commit entry
points it serves in `crates/node`. It is the A2 surface of the
[burn 3 campaign](qa-campaign-20260911-burn3-brief.md).

The review traced fee and balance arithmetic, account sequence and replay
checks, transaction-family ordering within a block, atomic publication of
state transitions, validator-registry activation, and committed-state resource
limits. The pre-repair execution baseline was 196 passing tests.

## Findings

### 1. P1 — a duplicate receipt ID can be ratified but cannot be committed

**Source:** `crates/node/src/batch_snapshot.rs:22-50`,
`crates/node/src/block_finality.rs:3894-3899`, and
`crates/node/src/storage_commit.rs:2598-2609`.

Proposal construction copies every execution receipt ID without requiring
uniqueness. Proposal validation checks only that the declared count equals the
vector length. Honest validators rebuild the same proposal from its batch, so
a Byzantine proposer can submit the same signed transaction twice: the first
copy is accepted, the second is rejected for its now-stale sequence, and both
receipts retain the same transaction ID. The proposal is deterministic and
eligible for votes, but ordered commit rejects the duplicate ID after
certification. A quorum-certified height therefore cannot be persisted by any
validator, violating Byzantine fault tolerance and halting progress at that
height.

The repair must reject duplicate receipt IDs both when a proposal is built and
when an externally supplied proposal is validated. This changes proposal
admissibility and is consensus-affecting; it must not be presented as active
without a separate activation and deployment decision.

### 2. P2 — ordered OwnedDeposit bypasses the declared owned-object limit

**Source:** `crates/execution/src/owned_transfer.rs:681-769` and
`crates/types/src/core_chain.rs:251`.

Owned transfers and unwraps reject a net result above
`MAX_OWNED_OBJECTS` (100,000), but the consensus-ordered
`apply_owned_deposit` path appends one object without consulting that limit.
The retired direct `wrap_to_owned` helper has the same omission. At a ledger
already at the declared maximum, a valid source-signed OwnedDeposit debits the
account and commits object 100,001. Further deposits can continue growing this
committed vector beyond the limit enforced on the other owned-object paths.

The repair must use one checked capacity calculation across transfer, unwrap,
wrap, and ordered deposit paths, and prove that a cap rejection leaves the
ledger unchanged. Rejecting a formerly accepted deposit at the cap changes a
state-transition result and is consensus-affecting.

### 3. P2 — validators can certify replicated state that the storage layer refuses

**Source:** `crates/node/src/batch_snapshot.rs:22-42` and
`crates/storage/src/lib.rs:67,759-775,825-829`.

The storage layer rejects an individual serialized state file above 256 MiB,
but proposal construction computes and exposes a state root without first
checking whether the resulting ledger, governance, shielded, or bridge state
can be serialized within that limit. Several fee-paying transition classes
grow durable vectors, and FastPay pre-state effects can grow the ledger on any
batch family. Once a batch crosses the bound, honest validators can rebuild
and vote for its proposal; ordered commit then fails while writing the state
value. The certified block is deterministic but not persistable, so subsequent
heights cannot advance.

The repair must reuse the exact state-file serialization bound before proposal
publication and validation, covering all replicated state components without
performing a write. This adds a proposal-admission rule and is
consensus-affecting.

## P3 observations

No execution P3 is recorded. Compatibility-only replay preimages are selected
by exact archived chain identities in the node, rejected transitions publish
no partial ledger mutation, and the fixed transaction-family order is included
in the batch commitment.

## Repair result

Repair `e95efbdf` closes all three findings in source:

- proposal construction and supplied-proposal validation now share one
  duplicate receipt-ID rejection, before any validator vote can be emitted;
- every owned-object transition uses one checked net-growth calculation, and
  ordered deposit and retired direct wrap reject growth at the declared cap;
- proposal construction counts the exact pretty-JSON state-file representation
  and keyed integrity trailer for ledger, governance, shielded, and bridge
  state before exposing a state root. Validator reconstruction uses the same
  check. The counter writes nothing and does not allocate the serialized form.

These repairs change proposal admissibility or an ordered state-transition
result and are consensus-affecting. They are source-only: no activation,
deployment, live-chain mutation, or on-disk format change occurred.

Post-repair verification:

- `cargo test -p postfiat-execution --locked`: 198 passed.
- `cargo test -p postfiat-storage --locked`: 90 passed, 2 ignored; process-crash
  integration: 1 passed.
- Both duplicate-receipt proposal regressions and the state-file-size regression
  passed.
- Node block-proposal tests: 3 passed; validator-registry continuation tests: 6
  passed.
- Five focused transparent asset, replay, NFT, offer, and atomic-swap ordering
  tests passed.
- Strict clippy for `postfiat-execution`, `postfiat-storage`, and
  `postfiat-node`, plus the workspace formatting check, passed.

## Repair scope

Only the three findings above were repaired. No Orchard, privacy, bridge,
proof-program, deployment, live-chain, or frozen-artifact change is part of
this review.
