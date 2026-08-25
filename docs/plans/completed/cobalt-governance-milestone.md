# Cobalt Governance Milestone

!!! note "Historical implementation milestone"

    This plan describes the pre-activation delivery boundary. Cobalt was later
    activated for validator-trust ratification at height 916. See
    [Current State](../../status/chain-state-current.md) for the active runtime,
    repository, campaign, and freshness identities.

## What this milestone delivered

PostFiat still orders transactions and finalizes blocks with consensus v2. This
milestone did not change that system. It added a separate, deliberately slow
Cobalt governance lane for validator and trust-graph changes only.

The delivered flow is:

1. An operator proposes a specific validator-registry or trust-graph change.
2. Durable Cobalt nodes exchange authenticated messages and either converge on
   the exact ordered change or reject it. Their default shadow mode has no live
   authority.
3. Before Cobalt receives any authority, the currently active validator
   registry must approve an exact handoff record with ML-DSA-65 signatures. That
   record binds the Cobalt lock, trust-graph root, validator-registry root,
   amendment sequence, protocol version, and activation height.
4. Existing block consensus orders the handoff. Only after its activation
   height may Cobalt authorize validator-trust changes covered by that version.
5. Blocks and transactions continue to use consensus v2 exactly as before. A
   Cobalt outage can stop validator governance from advancing, but it cannot
   become a second block-finality protocol.

Operators can inspect the same real state through a Python CLI and a read-only
browser interface. The interface shows the verified trust graph, recorded node
proposals, signed shadow convergence, and the exact activation gates. It shows
zero proposals when node state is empty and exposes no governance action.

Before activation, any failure leaves the Foundation registry in control.
After activation, rollback must be a newly authorized forward transition; it
cannot rewrite finalized history. The result does not provide faster payments,
automatic validator selection, or permission for a new set to authorize
itself.

**Status:** Completed
**Current task:** None
**Locked specification:**
[`cobalt-research-spec.md`](../../governance/cobalt-research-spec.md)
**Decision boundary:** Cobalt governs validator trust evolution only. Consensus
v2 remains the sole block-ordering and transaction-finality protocol.

## Task Node ledger

The tasks are accepted and must run in this dependency order.

1. [x] `task_e0016cefcbad1e7f70da32b28f69502e` — create and verify this milestone journal. **Rewarded: 2 PFT.**
2. [x] `task_a7464ea1e003c388845a43d7144360e0` — build the human-readable Python CLI for trust-graph checks, transition witnesses, protocol replay, and shadow readiness. **Rewarded: 1.5 PFT.**
3. [x] `task_4588cb739847318708ad3ea844380e60` — build the complete Rust governance-only shadow service with durable state, bounded transport, production randomness, restart/replay safety, adversarial convergence, and CLI observability. **Rewarded: 0 PFT.**
4. [x] `task_e936a36597bc83239e61400e455417e6` — implement the versioned controlled-testnet governance-authority handoff with old-registry ML-DSA-65 authorization and consensus ordering. **Rewarded: 2.7 PFT.**
5. [x] `task_79398d1434bcb45b06fccdf9d1de0c51` — build and verify the browser interface, refresh concise documentation, and retire this milestone only after the CLI and interface work. **Rewarded: 4 PFT.**

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

- [x] Require the old active registry's distinct ML-DSA-65 approvals and normal consensus ordering in `crates/node/src/cobalt_handoff.rs` and `crates/node/src/consensus_artifacts.rs`.
- [x] Bind the exact Cobalt lock, graph root, registry root, amendment sequence, activation height, and protocol version in `crates/types/src/shielded_bridge_governance.rs`.
- [x] Enforce old/new exclusivity, durable replay, and forward-only rollback in `crates/node/src/cobalt_handoff.rs`.
- [x] Keep block finality and transaction success semantics unchanged; Cobalt transitions remain governance actions in the existing consensus-ordered batch path.

### 4. Browser interface and closure

- [x] Show real trust state, proposals, shadow convergence, and activation readiness through `python/postfiat_rpc/cobalt_ui.py`, backed by the CLI, MAC-validated node state, and signed shadow status.
- [x] Expose no invented or unauthorized governance actions; the HTTP service implements GET/HEAD only and returns 405 for POST.
- [x] Refresh concise operator and architecture documentation in `docs/governance/cobalt.md` and `docs/governance/cobalt-implementation.md`.
- [x] Verify the Python CLI and responsive browser interface against a freshly initialized node and real four-validator shadow drill, then retire this journal to `docs/plans/completed/`.

## Completion rule

All five Task Node tasks reached their final rewarded outcome. The CLI and
browser interface were verified against real Cobalt and node surfaces. The
checked fresh node correctly remained under Foundation authority because no
versioned handoff had been ordered into that node state.
