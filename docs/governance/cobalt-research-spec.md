# Cobalt Research Specification

Status: locked — text-improvement score 89.93/100
Date: 2026-08-22
Decision scope: the role of Cobalt in PostFiat L1 consensus and governance

!!! note "Historical design specification"

    The implementation and activation milestones described below later passed;
    Cobalt activated for validator-trust ratification at height 916. Preserve
    this document as the original design boundary and use
    [Current State](../status/chain-state-current.md) for operational claims.

## Decision

PostFiat should retain its versioned prepare/precommit protocol as the only
block-ordering and transaction-finality mechanism. Cobalt should be developed
as a separate, slow-path governance protocol for validator trust evolution:
proposing, checking, agreeing on, and activating validator-registry and trust
graph transitions. It should not replace block consensus, and the existing
Cobalt crate should not gain live mutation authority merely because it can
produce a certificate or replay report.

Activation should happen only behind a new, explicit protocol boundary after
the Cobalt path is integrated with durable node state, production transport,
resource limits, signer safety, old-to-new registry handoff, and adversarial
restart/replay tests. Until that milestone passes, live governance must keep
requiring distinct ML-DSA-65 authorizations from the active old-rule registry
and normal consensus ordering.

This is a hybrid architecture:

1. consensus v2 orders blocks and establishes ledger finality;
2. Cobalt decides the governance sequence that may change the validator trust
   graph and future transaction committee;
3. the current active rules validate every transition before any new rules can
   become authoritative; and
4. clients still determine transaction success from both block finality and the
   accepted execution receipt.

## Why Cobalt Is Relevant

The 2018 Cobalt paper separates fast transaction ordering from adaptable
governance. Its open-network model permits validators to hold non-identical
trust views composed of essential subsets. Safety and liveness are stated
against local thresholds rather than a single globally agreed participant
list. The paper's key operational claim is not that Cobalt is a faster block
protocol. It explicitly recommends using Cobalt for amendments while a faster,
fixed-membership network orders transactions.

That separation fits PostFiat. Consensus v2 already provides deterministic
proposer rotation, prepare and precommit quorum certificates, durable locks,
signed timeout certificates, and replayable committed artifacts. Replacing it
would duplicate a working finality boundary and import Cobalt's asynchronous
RBC/ABBA/MVBA costs into every payment. The unsolved problem is narrower and
more strategic: how a network changes the validators and trust assumptions that
secure future rounds without relying forever on an informal operator list.

## Reproduction of Existing PostFiat Work

The current implementation is not a paper sketch. The owning crate is
`crates/consensus_cobalt`, with 13,756 lines across its source modules, 70
in-crate tests, and 24 examples. Fourteen adversarial examples require the
`cobalt-unsafe-simulation` feature; production builds leave that feature off.

| Existing surface | Reproduced behavior | Source anchor | Present limitation |
| --- | --- | --- | --- |
| Trust graph model | Builds domain-bound trust views, essential subsets, derived UNLs, rooted graphs, and parent-linked transitions. It checks `t_S < 2q_S - n_S` and `2t_S < q_S`. | `crates/consensus_cobalt/src/core_types.rs`; `crates/consensus_cobalt/src/trust_graph_governance.rs` | Configuration and validation substrate, not live distributed graph discovery. |
| Linkage and support | Computes weak/strong support and reports linked, fully linked, unsafe, weakly connected, and strongly connected views. | `crates/consensus_cobalt/src/trust_graph_governance.rs:738`; `crates/consensus_cobalt/src/trust_graph_governance.rs:894`; `crates/consensus_cobalt/src/trust_graph_governance.rs:920` | Safety depends on declared trust and fault assumptions; shared operational control is external evidence. |
| Transition witness | Derives a bounded, proposer-independent old/new essential-subset cover and rejects stale parents, open challenges, incomplete covers, excessive Byzantine budgets, or unsafe old/new quorum intersections. | `crates/consensus_cobalt/src/cobalt_cover_extractor.rs`; `crates/consensus_cobalt/src/trust_graph_governance.rs:314` | A bounded checker proves only its declared model and configured cover profile. |
| Signed Cobalt messages | Domain-separates and ML-DSA-signs RBC propose/echo/ready/accept and ABBA init/aux/conf/finish messages; detects tampering, non-committee signers, duplicate support, and equivocation. | `crates/consensus_cobalt/src/rbc_abba_mvba.rs` | The deterministic common coin is simulation-only; a production random source and service lifecycle remain activation requirements. |
| MVBA and DABC | Deterministically selects valid candidates, creates a parent-linked amendment order, gates activation on full-knowledge checkpoints, and verifies replay bundles. | `crates/consensus_cobalt/src/rbc_abba_mvba.rs:1744`; `crates/consensus_cobalt/src/dabc_registry.rs` | Library mechanics and drills do not by themselves constitute an always-on node protocol. |
| Registry evolution | Binds validator updates to old/new registry roots, trust graph roots, activation heights, lifecycle records, transaction-network membership, rollback, and replay rejection. | `crates/consensus_cobalt/src/trust_graph_governance.rs:1052`; `crates/consensus_cobalt/src/dabc_registry.rs`; `crates/consensus_cobalt/src/tests.rs:3261` | Live mutation still follows old-rule signed governance. |
| Node integration | Verifies historical amendment and registry replay, supports canonical and non-uniform verification modes, and binds optional Cobalt trust metadata in operator manifests. | `crates/node/src/governance.rs:1188`; `crates/node/src/governance.rs:1980`; `crates/node/src/consensus_artifacts.rs:3102` | Verification mode is not live authorization. The node has no enforcement branch that delegates governance admission to the recorded Cobalt authority mode. |

The existing test inventory covers the important failure classes: malformed
domains and subset math, wrong local views, forged or non-committee signatures,
RBC conflicting accepts, ABBA equivocation, candidate flooding, DABC activation
and replay, stale or unsafe graph transitions, old-set blocks after activation,
rollback, and admission evidence with hidden shared control. The examples extend
that inventory with partitions, process kills, resource pressure, governance
spam, crash/restart, stale replay, trust poisoning, key compromise, and cover
sizing.

This reproduction also found an evidence boundary that the implementation plan
must not hide. Current documentation names historical JSON reports under
`reports/`, but those report files are not present in the current repository or
at `HEAD`. They cannot serve as reviewable evidence for a new activation
decision. The source, tests, and examples remain; new milestone work must
regenerate compact evidence from the current commit rather than resurrect the
old report sprawl.

## Consensus Benefits and Their Conditions

### 1. Local safety under non-uniform trust

Cobalt can make safety a property of adequately linked local views. A badly
configured remote view need not automatically destroy the safety relationship
between two correctly linked validators. PostFiat's checker further requires a
complete bounded cover and verifies every covered old/new quorum intersection
against the active Byzantine budget.

This benefit is real only when trust views are authenticated, fresh, complete
within the declared profile, and grounded in independent operator evidence.
Cobalt cannot infer undeclared common ownership, hosting, funding, jurisdiction,
or key custody.

### 2. Safer validator-set evolution

The old active graph can validate a proposed child graph before activation.
Parent roots, activation heights, challenge state, key continuity, and
cross-graph quorum intersections make a transition auditable and replayable.
This is stronger than treating a validator-list edit as an operator file change.

The benefit disappears if a child graph can authorize itself, if old and new
committees can both sign the same post-activation domain, or if signer locks and
transition state are not durable across restart.

### 3. Governance liveness without a permanent leader set

RBC, ABBA, MVBA, and DABC provide a path to agreement and ordered amendment
activation under non-identical trust views and arbitrary message delay. This
could let governance recover from a failed or censored transaction committee
without granting one permanent coordinator unilateral replacement authority.

This is a probabilistic liveness claim, not a latency claim. It depends on a
production common random source, fully linked/unblocked conditions, bounded
messages and candidates, and a transport that eventually delivers authenticated
messages. It does not justify promising faster transaction finality.

### 4. Separation of adaptability from the hot path

Using Cobalt only for governance preserves fast, comprehensible block finality
while moving validator evolution into a richer trust protocol. The transaction
committee can remain small enough to operate efficiently, while Cobalt provides
the mechanism for replacing it when the current governance conditions support
that change.

This separation is also a containment boundary: a Cobalt stall pauses governance
evolution, not ordinary transaction ordering under the still-valid active
committee.

## Rejected Implementations

### Replace consensus v2 with Cobalt for every block

Reject. The paper itself separates amendment consensus from fast transaction
ordering. PostFiat would take on asynchronous broadcast and agreement overhead
for every block, invalidate existing finality assumptions, and require a new
client, storage, replay, and receipt proof without a demonstrated user benefit.

### Treat Cobalt outputs as advisory forever

Reject as the end state. This preserves today's security boundary but never
solves protocol-governed validator evolution. Advisory mode is appropriate
during development and shadow operation, not as the claimed destination.

### Flip `authority_mode` to Cobalt-ratified

Reject. `GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED` is currently a typed state
value and is included in state commitment and query/replay surfaces. Repository
search shows no live authorization branch that changes amendment admission based
on that value. Recording `1` therefore does not activate a Cobalt governance
protocol. Treating it as activation would be a dangerous label-only cutover.

## Activation Contract for a Future Milestone

This research specification does not authorize implementation. A later
milestone may design an activation only if it preserves these requirements:

1. **Explicit version boundary:** Cobalt governance authority begins at a
   committed activation height under the old active authorization rules.
2. **Narrow authority:** Cobalt may ratify only versioned governance kinds; it
   cannot bypass block consensus, transaction execution, receipts, or state-root
   verification.
3. **Old-rule handoff:** the last old-rule state imports an exact Cobalt lock,
   graph root, registry root, amendment sequence, and protocol version. New-rule
   certificates cannot conflict with that imported state.
4. **Durable signer safety:** RBC/ABBA/DABC high-water marks, votes, locks,
   checkpoints, and activation records persist before signatures are returned.
5. **Production randomness:** live ABBA uses an audited, domain-separated common
   random source with recovery semantics; simulation randomness remains
   impossible in production builds.
6. **Bounded networking:** every message, candidate set, pending interval,
   signature set, queue, and verification path has an enforced bound before
   allocation or cryptography.
7. **Replay-complete storage:** snapshot, restart, catch-up, rollback, and full
   history replay produce the same authority state and reject stale old/new
   committee artifacts.
8. **Operator evidence:** trust configuration is accompanied by redaction-safe,
   current evidence for key custody and correlated control. The protocol states
   precisely what is declared and what remains unknowable.
9. **Shadow convergence:** a multi-validator deployment runs Cobalt in shadow
   mode through partitions, equivocation, censorship, member loss, and restart
   before any live cutover.
10. **Fail-safe rollback:** failure before activation leaves the old graph
    authoritative. Any post-activation rollback is a separately authorized,
    forward-moving transition, never a history rewrite.

## Falsification and Reproduction Plan

A milestone derived from this locked specification should begin by reproducing
the current substrate from a clean checkout:

```bash
cargo test -p postfiat-consensus-cobalt --locked
cargo test -p postfiat-consensus-cobalt --features cobalt-unsafe-simulation --locked
cargo run -p postfiat-consensus-cobalt --example cobalt_safety_witness
cargo run -p postfiat-consensus-cobalt --example cobalt_cover_extractor
cargo run -p postfiat-consensus-cobalt --example cobalt_cover_sizing
```

It should then add integration tests that falsify the activation contract:
conflicting old/new certificates, crash before and after each durable write,
stale graph and registry roots, malformed or oversized messages, candidate
floods, common-random-source failure, partitions followed by healing, and a
complete node replay across the activation boundary. Evidence should be a small
machine-readable summary tied to the commit and exact commands, not thousands
of generated packet files in `docs/`.

The present environment does not have `cargo` on `PATH`, so this research pass
could inventory the current tests but could not execute them. That limitation
must remain explicit and must be closed by the implementation milestone before
activation work claims reproducibility.

## Locked Scope

The decision to carry forward is: **Cobalt is the candidate governance consensus
for safe validator trust evolution, while consensus v2 remains the block and
transaction finality protocol.** The existing crate is substantial enough to
justify an integration milestone, but it is not already the live governance
oracle. No authority change, milestone implementation, CLI, UI, or deployment is
authorized by this document.

## Primary References

- `docs/references/cobalt-bft-governance-in-open-networks.md`
- `docs/architecture/finality.md`
- `docs/governance/cobalt.md`
- `docs/governance/cobalt-implementation.md`
- `crates/consensus_cobalt/src/`
- `crates/node/src/governance.rs`
- `crates/node/src/consensus_artifacts.rs`
- `crates/types/src/shielded_bridge_governance.rs`
- `README.md`
