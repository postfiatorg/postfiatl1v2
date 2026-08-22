# Cobalt Governance Milestone

## What this milestone is building

PostFiat already knows how to order transactions and finalize blocks. It does
that with consensus v2, and this milestone does not change or replace that
system.

The problem addressed here is how PostFiat changes its validators. Today, a
validator or trust-graph change is authorized by the currently active validator
registry and then ordered through normal block consensus. The repository also
contains a substantial Cobalt implementation that can check whether old and new
trust graphs overlap safely and can coordinate agreement on a sequence of
governance changes. However, that Cobalt code is not yet a live node service and
has no authority to change the validator registry.

This milestone turns Cobalt into a deliberately slow governance lane for
validator changes only. The intended end-to-end flow is:

1. An operator proposes a specific validator-registry or trust-graph change.
2. Cobalt nodes exchange authenticated messages and either agree on the exact
   ordered change or reject it.
3. Before Cobalt receives any authority, the currently active validator
   registry must approve an exact handoff record with ML-DSA-65 signatures. That
   record binds the Cobalt lock, trust-graph root, validator-registry root,
   amendment sequence, protocol version, and activation height.
4. The handoff is ordered by the existing block consensus. Only after its
   activation height may Cobalt authorize the validator and trust-graph changes
   covered by the new governance version.
5. Blocks and transactions continue to use consensus v2 exactly as before. A
   Cobalt outage can stop validator governance from advancing, but it cannot
   become a second block-finality protocol.

The work is staged so we can stop safely: first reproduce the existing Cobalt
logic in a human-readable CLI; then run a durable multi-node shadow service with
no authority; then implement and test the explicit authority handoff; and only
then expose the real state in a browser interface. Before activation, any
failure leaves the current validator registry in control. After activation, a
rollback must be a newly authorized, forward-moving transition—it may not
rewrite finalized history.

The milestone is done when a human can use both the Python CLI and browser
interface to inspect the real trust graph, follow a proposal, see whether the
nodes converged, and understand whether activation is permitted. It does not
deliver faster payments, automatic validator selection, or permission for a new
validator set to authorize itself.

**Status:** Active
**Current task:** `task_e936a36597bc83239e61400e455417e6`
**Locked specification:**
[`cobalt-research-spec.md`](../../governance/cobalt-research-spec.md)
**Decision boundary:** Cobalt governs validator trust evolution only. Consensus
v2 remains the sole block-ordering and transaction-finality protocol.

## Task Node ledger

The tasks are accepted and must run in this dependency order.

1. [x] `task_e0016cefcbad1e7f70da32b28f69502e` — create and verify this milestone journal. **Rewarded: 2 PFT.**
2. [x] `task_a7464ea1e003c388845a43d7144360e0` — build the human-readable Python CLI for trust-graph checks, transition witnesses, protocol replay, and shadow readiness. **Rewarded: 1.5 PFT.**
3. [x] `task_4588cb739847318708ad3ea844380e60` — build the complete Rust governance-only shadow service with durable state, bounded transport, production randomness, restart/replay safety, adversarial convergence, and CLI observability. **Rewarded: 0 PFT.**
4. [ ] `task_e936a36597bc83239e61400e455417e6` — implement the versioned controlled-testnet governance-authority handoff with old-registry ML-DSA-65 authorization and consensus ordering.
5. [ ] `task_79398d1434bcb45b06fccdf9d1de0c51` — build and verify the browser interface, refresh concise documentation, and retire this milestone only after the CLI and interface work.

## Milestones

### 1. Reproducible Python CLI

- [x] Expose trust-graph validation from `crates/consensus_cobalt/src/core_types.rs` and `crates/consensus_cobalt/src/trust_graph_governance.rs` through `python/postfiat_rpc/cobalt.py`.
- [x] Expose bounded transition-witness validation from `crates/consensus_cobalt/src/cobalt_cover_extractor.rs`.
- [x] Replay representative RBC, ABBA, MVBA, and DABC governance flows from `crates/consensus_cobalt/src/rbc_abba_mvba.rs` and `crates/consensus_cobalt/src/dabc_registry.rs`.
- [x] Provide readable and JSON shadow-readiness output through `python/postfiat_rpc/cobalt.py`, covered by `python/tests/test_cobalt.py`.

### 2. Governance-only shadow service

- [x] Integrate signer-authenticated durable protocol state and a mode-0600 ML-DSA-65 signer without granting live authority in `crates/node/src/cobalt_shadow.rs`.
- [x] Add bounded authenticated governance ingress and an OS-entropy, signed threshold commit/reveal common-randomness lifecycle in `crates/node/src/cobalt_shadow.rs`.
- [x] Prove restart, replay, partition-healing, equivocation, censorship, randomness failure, and member-loss behavior with the four-node adversarial drill and focused Rust tests.
- [x] Expose signer state, transport health, governance-message counters, and drill results through `postfiat-cobalt-shadow` and `python/postfiat_rpc/cobalt.py`. The service remains opt-in and has no block-processing or authority-activation call site.

### 3. Versioned authority handoff

- [ ] Require the old active registry's distinct ML-DSA-65 approvals and normal consensus ordering.
- [ ] Bind the exact Cobalt lock, graph root, registry root, amendment sequence, activation height, and protocol version.
- [ ] Enforce old/new exclusivity, durable replay, and forward-only rollback.
- [ ] Keep block finality and transaction success semantics unchanged.

### 4. Browser interface and closure

- [ ] Show real trust state, proposals, shadow convergence, and activation readiness through a browser interface backed by the CLI and node surfaces.
- [ ] Do not expose invented or unauthorized governance actions.
- [ ] Refresh concise operator and architecture documentation after the interface works.
- [ ] Move this journal to `docs/plans/completed/` only after the Python CLI and browser interface are verified.

## Completion rule

This milestone is complete only when all five Task Node tasks reach their final
rewarded outcome and the CLI and browser interface work against real Cobalt and
node surfaces. Until the versioned handoff is separately proven and activated,
live governance remains under the old active registry.
