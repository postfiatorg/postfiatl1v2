# Dynamic UNL L1 Evidence-Source Decision Note

**Status:** Decision note — Text Improvement Harness full gate passed on 2026-08-27 — average 89.80/100 (GPT 92.60, Fable 89.20, GLM 87.60; five runs per lane; run group `dynamic-unl-l1-evidence-source-note`); scored content SHA-256 `ac0ff91d183e331f9e581342462ba74f0b2e85e8b3a04f29bd5ceb47ab2e4b25`

**Date:** 2026-08-27

**Author:** Domagoj Ravlić (`dravlic`)

**Decision owner:** Post Fiat

**Related:** [Dynamic UNL proposal-source research specification](dynamic-unl-proposal-source-research-spec.md) and [deferred milestone draft](../deferred-plans/dynamic-unl-proposal-source-milestone.md)

## Plain-English summary

This L1 has the ratifier but no evaluator: Cobalt can ratify a bounded validator-registry or trust-graph change, but no live service evaluates which L1 validators deserve it. The PFT Ledger has the evaluator but no ratifier: Dynamic UNL scores PFT Ledger validators and seals a result, but it does not govern this L1 registry. The research specification joins those roles through a deterministic adapter. It leaves one decision implicit: whether the adapter should use evidence about PFT Ledger validators, evidence about L1 validators, or the first source temporarily while the second is built.

## What exists today

### PFT Ledger: evaluator pipeline

| Stage | Current behavior | Repository evidence |
| --- | --- | --- |
| Observe | Collect VHS agreement, topology and manifests, `postfiatd /crawl` addresses, ASN, and geolocation for PFT Ledger validators. VHS agreement is currently the only validator-quality signal and comes from one Foundation observer. | `dynamic-unl-scoring/docs/CurrentRoadmap.md` |
| Freeze | Freeze the round input package and manifest; pin the model/runtime, score formula, selector, and canonical output rules. | `dynamic-unl-scoring/docs/phase2/FrozenRoundBoundary.md`; `dynamic-unl-scoring/docs/phase2/ExecutionManifestSchema.md` |
| Publish and anchor | Put the frozen package on IPFS, announce its CID and hash in a signed PFT Ledger memo transaction, and later anchor the result. IPFS transports content; the ledger transaction is the trust anchor. | `dynamic-unl-scoring/docs/CurrentRoadmap.md`; `validator-scoring-sidecar/docs/Overview.md` |
| Replay and attest | Sidecars fetch and verify the same package, replay model/formula/selector execution, and commit then reveal result fingerprints through PFT Ledger memo transactions. The validator secp256k1 master key signs the payload; a separate relay wallet signs, funds, and sends the payment. | `validator-scoring-sidecar/docs/Overview.md`; `validator-scoring-sidecar/docs/Configuration.md` |
| Converge | Aggregate valid reveals, seal the convergence report, publish it, and anchor its hash and CID on the PFT Ledger. | `dynamic-unl-scoring/docs/phase2/ConvergenceReporting.md`; `dynamic-unl-scoring/docs/CurrentRoadmap.md` |
| Maturity | Phase 2 is live on devnet and testnet; testnet formula rounds run weekly and the operator records more than 20 completed rounds. Phase 3A evidence transparency and Phase 3B governance handoff have not started. | `dynamic-unl-scoring/docs/CurrentRoadmap.md` |

The scored identity is a PFT Ledger validator's secp256k1 master public key. The sidecar delegates signatures to `postfiatd validator-keys`; it does not hold the master-key seed (`validator-scoring-sidecar/docs/Overview.md`).

### Rust L1: governance pipeline

| Stage | Current behavior | Repository evidence |
| --- | --- | --- |
| Identify | The governed registry identifies validators by validator ID and ML-DSA-65 hot key. Live changes require old-registry ML-DSA-65 authorization and consensus ordering. | `docs/governance/validator-registry.md`; `crates/types/src/core_chain.rs` |
| Observe | Doctor reports and monitor snapshots expose point-in-time endpoint health, height lag, service state, RPC status, history readiness, and Orchard counters. Configured topology and draft evidence schemas also exist. There is no continuous evaluator-ready service that freezes per-validator agreement, uptime, or observed topology with signed lineage. | `docs/validators/monitors.md`; `crates/network/src/lib.rs`; `docs/governance/validator-evidence-packet-schema.md` |
| Evaluate | The DGA design says that “a policy proposes bounded validator-registry actions,” but its policy output, collector integration, and verifier tier are decision support, not live authority. No live policy scores the registry population or creates Dynamic UNL proposals. | `docs/governance/deterministic-governance-overview.md`; `docs/governance/deterministic-governance-agent-plan.md` |
| Propose | Every accepted Cobalt registry/trust proposal so far originated from Foundation-administered validators. | `docs/status/chain-state-current.md` |
| Ratify | Cobalt has ratified bounded registry and trust-graph changes since height 916. Consensus v2 still orders and finalizes blocks. | `docs/governance/deterministic-governance-overview.md`; `docs/status/chain-state-current.md` |
| Suspend | `suspend` is a governed registry update. There is no automatic missed-round or availability suspension path. | `docs/governance/validator-registry.md`; `docs/status/OPEN-SOURCE-PRODUCTIONIZATION-AUDIT-20260716.md` |
| Carry data | Signed `payment_v2` transactions can carry up to four lower-hex memos, bounded to 512 total decoded bytes, and can be submitted and found through RPC/history. No Dynamic UNL announcement, commit/reveal, report-anchor profile, validator-identity binding, or indexer exists on this lane. | `crates/types/src/transactions_mempool_receipts.rs`; `crates/types/src/core_chain.rs`; `docs/rpc/methods.md`; `crates/node/src/block_finality.rs` |

The L1 therefore has raw finalized consensus and Cobalt history plus operational snapshots, but not an observer that turns those records into frozen, windowed, per-validator evidence. Its generic memo lane could carry compact hashes and CIDs; an L1 evidence protocol would still need schemas, authorization rules, indexing, replay, and a binding from the memo signer to the registered ML-DSA-65 validator identity.

### Terms used in this decision

- **Evidence source** is the chain and observation system whose validator
  behavior is actually measured. It is not the chain on which the score is
  later consumed.
- **Identity binding** is a governed projection between already-known
  identities. It does not transfer measurements from one process or chain to
  another.
- **Evaluator** produces a reproducible score and selected set from frozen
  evidence. It has no registry authority.
- **Ratifier** accepts or rejects exact bounded proposal bytes under current
  registry and trust rules. It does not create the evidence behind them.

## Option A — reuse the sealed PFT Ledger result plus a governed identity binding

### How it works

Use the research specification as written. A standalone verifier authenticates a sealed PFT Ledger convergence report and its complete frozen lineage, then resolves a governance-approved `DynamicUnlValidatorBindingV1` from each PFT Ledger secp256k1 master key to exactly one current L1 validator ID and ML-DSA-65 hot key. The canonical adapter applies the governed L1 policy and safe-churn limits to emit byte-identical proposal content. Cobalt decides whether to ratify those bytes; Consensus v2 orders any separately authorized change.

### What must be built

- Add canonical source-certificate, identity-binding, and proposal schemas in `new: crates/types/src/dynamic_unl_proposal.rs`, wired through `crates/types/src/core_chain.rs`.
- Build the offline verifier and operator CLI in `new: python/postfiat_rpc/dynamic_unl_proposal.py`.
- Add source admission and graph-budget checks in `new: crates/consensus_cobalt/src/dynamic_unl_source.rs` and `crates/consensus_cobalt/src/validator_admission_policy.rs`.
- Carry verified, immutable artifacts into `crates/node/src/cobalt_handoff.rs` and the existing governance path without network access during consensus.
- Define governance, expiry, rotation, and revocation rules for the cross-chain identity binding before a scored round begins.

### Assumptions and risks

This assumes material operator overlap between the two validator sets and assumes a PFT Ledger validator's performance is an acceptable proxy for the bound L1 validator. A correct binding proves identity continuity, not that the L1 process had the scored uptime, topology, agreement, hosting diversity, or operational behavior. Mapping coverage may be partial, and a shared operator may run the two processes differently. The source-chain trust rule, currently single-observer agreement input, unfinished Phase 3A vote lineage, and Foundation report assembly remain explicit dependencies.

### What it can and cannot claim

It can prove that a sealed PFT Ledger evaluator result was authenticated, deterministically projected through pre-existing bindings, safely bounded, and presented to Cobalt without a human editing the list. It cannot score an L1 validator that runs no PFT Ledger validator. It cannot claim that PFT Ledger evidence directly measured the bound L1 node, nor that proposal submission or either chain became decentralized.

## Option B — run the pipeline on L1-native evidence

### How it works

Port the same architectural pattern—not merely the final scores—to L1-native evidence. An observer derives bounded windows of signed Consensus v2 participation, Cobalt participation, reachability/uptime, and observed topology for ML-DSA-65 registry identities; freezes and publishes the package; announces and anchors it on this L1; and runs the pinned model, formula, and selector. L1-aware sidecars independently replay the round and commit/reveal. A sealed convergence report then identifies L1 validators directly, so the proposal adapter needs no cross-chain identity proxy.

### What must be built

- Define observable fields, completeness rules, windows, canonical records, and ML-DSA-65 evidence identities in `new: crates/types/src/validator_evidence_round.rs` and the existing validator-evidence documentation.
- Build a continuous collector and finalized-history index in `new: crates/node/src/validator_evidence_observer.rs`, using Consensus v2/Cobalt records and `crates/network/src/lib.rs` without treating configured topology as observed topology.
- Define an announcement, commit/reveal, convergence-anchor, and query profile over `payment_v2` memos and history in `crates/types/src/transactions_mempool_receipts.rs`, `crates/node/src/block_finality.rs`, and `docs/rpc/methods.md`; add a dedicated transaction only if the bounded memo profile cannot meet authorization or indexing needs.
- Add an L1 source adapter to `validator-scoring-sidecar` for discovery, ML-DSA-65 identity signing, replay, commit/reveal, restart, and equivocation handling.
- Add an L1 evidence profile to `dynamic-unl-scoring` while retaining version-pinned model, formula, selector, frozen package, and convergence semantics.
- Feed the native source certificate into `new: crates/consensus_cobalt/src/dynamic_unl_source.rs` and expose a human-readable verifier through `python/postfiat_rpc`.

### Assumptions and risks

This is a protocol and operations program measured in months, not an adapter change. The observer's coverage and independence determine what the score means. Reachability alone does not prove honest agreement; configured peers do not prove actual topology; and validator-authored reports can be gamed. The design must avoid circularly trusting the registry to attest its own quality, preserve evidence through restarts and partitions, define missed-observation semantics, and prevent a memo sender account from being mistaken for the registered validator key. Reusing `payment_v2` avoids inventing transport but does not solve those trust questions.

### What it can and cannot claim

It can eventually score every observable L1 validator under its native registry identity and tie a Cobalt proposal to evidence about the actual chain being governed. It cannot claim objective or decentralized evaluation until observer diversity, signed source coverage, frozen lineage, independent replay, commit/reveal, and convergence thresholds are verified. It also cannot inherit Phase 2 maturity merely because it reuses the PFT Ledger design.

## Option C — shadow A while building B

### How it works

Implement Option A only as `SHADOW_ONLY` on the controlled devnet. Use real sealed PFT Ledger rounds and governed bindings to prove artifact resolution, deterministic identity projection, adapter output, Cobalt admission/dry-run behavior, restart, and rejection paths without giving the source authority to mutate the registry. In parallel, design and build the L1 observer, anchor profile, and sidecar support required by Option B. Switch the configured evidence source only after B's signed evidence lineage, coverage, replay, convergence, and failure semantics are independently verifiable.

### What must be built

- Build A's canonical schemas, verifier, bindings, adapter, CLI, and Cobalt shadow admission in the modules named above.
- Keep the emitted status and every UI surface explicitly `SHADOW_ONLY`; submit no authority-bearing proposal from this evidence source.
- Start B with the evidence-field contract, observer prototype, memo anchor profile, and retained raw/frozen lineage before porting model execution or commit/reveal.
- Define a versioned source-profile field so the adapter cannot silently reinterpret PFT Ledger evidence as L1-native evidence.
- Require a new decision and release gate before enabling an L1-native source or any registry mutation.

### Assumptions and risks

This assumes the adapter/Cobalt integration has independent value even when the input measures another chain. It risks a temporary path becoming permanent or shadow output being described too broadly. Those risks are controlled only by explicit source labels, no mutation authority, expiry of cross-chain bindings, separate evidence roots, and a recorded switch gate. It also carries some duplicated integration work, although the canonical adapter, proposal constraints, and Cobalt safety tests should be reusable.

### What it can and cannot claim

It can prove end-to-end mechanical compatibility from a sealed external result to a bounded Cobalt decision path and expose identity, availability, churn, and replay failures early. Until B qualifies, it cannot claim that Dynamic UNL evaluated L1 validators or that its output deserves live registry authority. After B qualifies, claims remain limited to the measured fields and verified observer coverage.

## Comparison

| Option | What is scored | Evidence source | Identity binding | New components | Time order of magnitude | Main risk | What it proves |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A | PFT Ledger validators | PFT Ledger VHS, topology, manifests, crawl, ASN, geolocation | Governed secp256k1 master key → L1 validator ID/ML-DSA-65 key | Binding registry, source verifier, adapter, Cobalt admission, CLI | Weeks for a shadow-quality adapter; longer for authority gates | Proxy evidence is mistaken for L1 behavior | Sealed PFT result → deterministic bounded L1 proposal content |
| B | L1 validators | L1 Consensus v2/Cobalt history, uptime, reachability, observed topology | Native ML-DSA-65 registry identity | Observer, evidence schema, anchor profile, L1 scoring profile, sidecar support, convergence and adapter | Months | Incomplete or gameable observation is presented as objective | Native L1 evidence → independently reproduced L1 proposal content |
| C | PFT validators first; L1 validators after switch | A in shadow, then qualified B lineage | Governed cross-chain binding first; native identity after switch | A's shadow adapter plus staged B observer and source-profile switch | Weeks to shadow; months to native | Shadow proxy quietly becomes permanent or gains authority | Adapter → Cobalt compatibility now; native evidence lineage later |

## Recommendation

Choose Option C.

It preserves the research specification's useful work without confusing two different claims. A sealed PFT Ledger round is mature enough to exercise source verification, identity projection, deterministic proposal construction, Cobalt admission, and operator interfaces. It is not evidence that an L1-only validator performed well. Waiting for all of B before testing the adapter defers independent integration knowledge; granting A authority would overstate the source. `SHADOW_ONLY` keeps that distinction enforceable while the native observer is built.

The operator should record this exact decision:

> Adopt Option C as the working evidence-source sequence: use sealed PFT Ledger Dynamic UNL results only for a governed-binding, `SHADOW_ONLY` adapter-to-Cobalt integration on the controlled devnet; build and qualify an L1-native observer, anchor profile, scoring replay, and sidecar convergence path; and require a separate recorded decision before switching the evidence source or authorizing any registry mutation.

The answers to these questions change the plan:

1. Do the same operators run validators on both chains, and what percentage of each active set has a stable one-to-one mapping? Low coverage reduces A to an adapter fixture, not a representative shadow.
2. Is an L1 observer service already planned, who owns it, and which signed Consensus v2, Cobalt, uptime, and topology records can it retain across the scoring window? This determines B's evidence contract and schedule.
3. Is the existing bounded `payment_v2` memo lane acceptable for hash/CID announcements and commit/reveal anchors, including ML-DSA-65 validator attribution and indexing, or must the L1 add a dedicated data-carrying transaction/profile? This determines B's protocol surface.

## What this note does not do

- It grants no model, Dynamic UNL service, sidecar, observer, proposer, or operator authority.
- It changes neither chain, sends no transaction, and activates no suspension or registry mutation.
- It does not lock, amend, replace, or satisfy the research specification or deferred milestone.
- It does not claim the existing L1 monitor is a validator evaluator or the generic memo lane is an evidence protocol.
- Task Node was not used for this research/design note.
