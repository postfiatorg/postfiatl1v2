# Unified Whitepaper Milestone

Status: implemented
Date: 2026-05-19
Owner: PostFiat L1
Purpose: merge the current technical paper, mathematics paper, and XRP-origin
paper into one coherent whitepaper.

## Objective

Create one document that explains why PostFiat exists, why it is built the way
it is built, what is implemented now, and how the pieces fit together.

The unified paper replaces the prior split between:

- [whitepaper.md](../whitepaper.md);
- archived mathematics draft, formerly `alternate_whitepaper.md` (not shipped in
  the supported public source tree);
- archived XRP-origin draft, formerly `we-built-a-new-version-of-xrp.md` (not
  shipped in the supported public source tree).

The target reader is a serious crypto, finance, or infrastructure reader who
understands XRP, Bitcoin, institutional settlement, validator economics,
privacy, and quantum risk at a high level. The paper should be clear enough for
a non-engineer to follow, but grounded enough that a technical reviewer can
trace claims to code and evidence.

## Core Thesis

PostFiat is an XRP-style authority-validator settlement chain rebuilt around
six design decisions:

1. proof-of-authority settlement instead of proof-of-work or proof-of-stake;
2. privacy as a core settlement requirement;
3. Cobalt validator governance;
4. fixed supply and fee burn;
5. no native validator reward schedule;
6. post-quantum authorization from genesis.

The paper should explain each decision from first principles, then show what
has already been implemented.

## First-Principles Arguments

### 1. Proof Of Authority

Argument:

Bitcoin's proof-of-work block reward buys security by paying miners. That model
works, but it is expensive. XRP showed that a known-validator settlement network
can carry large economic value without mining rewards, validator inflation, or
proof-of-stake reward markets. XRP's market capitalization and long production
history are evidence that the authority-validator category is real.

PostFiat keeps the useful part of that model:

- known validators;
- fast deterministic settlement;
- low operating cost;
- fee burn for spam resistance;
- validators as infrastructure operators and natural stakeholders.

The paper should not claim that proof of authority is always better than proof
of work. It should say that for institutional settlement, paying an endless
block reward is not obviously necessary if credible validators already have a
business reason to keep settlement reliable.

### 2. Privacy

Argument:

Public transparent ledgers are useful for auditability, but they leak
positions, counterparties, balances, timing, and execution intent. Buy-side
workflows, tokenized assets, prime brokerage, treasury movement, and capital
markets settlement require confidentiality. Crypto has moved toward
speculation and capital markets as a dominant use case; those users do not want
every financial workflow broadcast in cleartext.

PostFiat therefore treats privacy as base settlement infrastructure, not as a
cosmetic feature.

The paper should explain:

- why transparent-only settlement is insufficient for buy-side workflows;
- what Orchard/Halo2-style shielded flow gives the chain now;
- how deposit, spend, withdraw, scanning, disclosure, nullifiers, commitments,
  and pool state fit together;
- what operational and compliance surfaces still have to be mature before
  privacy can be called production.

The tone should be affirmative. Do not frame privacy as a distant apology. Say
what exists and what has to be completed.

### 3. Cobalt

Argument:

Validator governance should not depend on informal off-chain validator history
services, ad hoc trust-list coordination, or opaque social updates. That is not
professional enough for a settlement system. Ripple-funded XRPL research
produced Cobalt as a solution to trust evolution in open validator networks,
but XRPL did not deploy that model as its production validator-governance path.

PostFiat uses the Cobalt lineage to make validator-set evolution explicit,
signed, replayable, and part of the chain's governance state.

The paper should explain:

- what XRP's UNL model solved;
- why off-chain trust-list history is unsatisfactory;
- what Cobalt adds conceptually: local trust views, essential subsets,
  thresholds, linkage, and democratic atomic broadcast for amendments;
- how PostFiat separates Cobalt governance from fast transaction ordering;
- what is implemented today after the full-Cobalt burndown.

Current implementation state to reflect:

- non-identical trust views exist;
- essential subsets carry `t_S` and `q_S`;
- unsafe trust graphs fail before activation;
- non-uniform governance certificates exist;
- RBC, ABBA, MVBA, and DABC amendment mechanics exist;
- validator registry and trust graph transitions are Cobalt-ratified;
- release/replay/controlled-readiness gates are green;
- latest controlled readiness expects 19 Cobalt packets, including amendment
  replay evidence.

The paper should say the Cobalt implementation and controlled-testnet mechanics
are built, while public operator diversity is a launch/topology evidence
question.

### 4. Fixed Supply

Argument:

Fixed supply is a major reason people trust Bitcoin and XRP. It is easy to
understand and hard to inflate away. XRP's 100 billion fixed supply and fee burn
show that an authority-validator chain does not need native issuance to pay
validators. Bitcoin's fixed cap shows that hard monetary constraints are a
central crypto value proposition.

PostFiat should state a fixed-supply monetary rule:

- genesis supply: 100 billion units;
- no native inflation;
- fees are burned or otherwise removed from circulating supply according to
  protocol accounting;
- fees price spam, state growth, and resource use; they are not validator
  rewards.

### 5. No Native Validator Incentives

Argument:

On-chain validator rewards create a paid validator class and can introduce
economic failure modes: reward farming, stake centralization, MEV incentives,
governance capture, and dependence on inflation. Validators in an institutional
settlement network should be natural stakeholders: exchanges, custodians,
market makers, issuers, infrastructure operators, allocators, and protocol
participants that benefit from the network working.

PostFiat should say:

- no native validator reward schedule;
- no inflation-funded validator subsidy;
- validators run because reliable settlement matters to them;
- operational costs are meant to stay low enough that this is realistic;
- fee burn is for spam resistance, not validator compensation.

The paper should be clear that "no validator incentives" means "no artificial
native subsidy," not "validators have no reason to operate."

### 6. Quantum Resistance

Argument:

Quantum migration risk is becoming a serious institutional question. A new
settlement chain does not need to start with long-lived classical account and
validator keys and hope to migrate later. If quantum risk has any material
long-horizon probability, a greenfield chain should price the larger signatures
and bandwidth into the design from day one.

PostFiat starts the base account and validator authorization path with
ML-DSA-style signatures and accepts the cost:

- larger signatures;
- larger certificates;
- more bandwidth pressure;
- different wallet and custodian ergonomics from BIP32/ECDSA systems.

The paper should explain why this is a design choice, not a slogan.

## Implementation Inventory To Explain

The unified paper should include a "What Exists Now" section with code and
evidence anchors for each implemented area:

- Rust settlement core: accounts, signed transfers, state roots, fees, blocks,
  certificates, receipts, deterministic replay.
- Fast ordering: HotStuff-family transaction ordering and finality evidence.
- Cobalt governance: trust views, essential subsets, linkage checking,
  non-uniform certificates, RBC, ABBA, MVBA, DABC, registry transitions,
  replay bundles, release/replay gates.
- Post-quantum authorization: account and validator signatures, wallet vectors,
  certificate-size handling.
- Privacy: Orchard/Halo2 deposit, spend, withdraw, scanning, disclosure,
  nullifiers, commitments, pool state, fee/resource policy, RPC gating.
- Fixed-supply economics: 100 billion supply, fee burn, no native validator
  rewards.
- RPC and wallet tooling: read RPC, controlled write paths, wallet flows,
  account transaction history, Python client, monitor and validator doctor
  tools.
- Validator operations: partial-history validation, launch packets, readiness
  gates, controlled topology, evidence reports.
- Rust rationale: memory safety, deterministic protocol code, smaller
  implementation risk surface than a C++ fork for this design.

## Completed Source Cleanup

The final paper replaced the three-doc structure. The completed merge:

1. Extracted the best thesis language from `we-built-a-new-version-of-xrp.md`.
2. Extracted the rigorous economic and mathematical framing from
   `alternate_whitepaper.md`.
3. Extracted the implementation inventory, current evidence anchors, and protocol
   explanations from `whitepaper.md`.
4. Reconciled stale Cobalt language with the current full-Cobalt
   controlled-readiness evidence.
5. Replaced scattered companion-paper references with one canonical whitepaper.
6. Moved obsolete companion drafts into
   `docs/archive/whitepaper-drafts/2026-05-19/`.

## Proposed Unified Paper Structure

1. Title and one-paragraph thesis.
2. What XRP proved.
3. Why a new chain exists.
4. Design principles.
5. Monetary model: fixed supply, fee burn, no validator subsidy.
6. Validator model: proof of authority and natural stakeholders.
7. Cobalt governance: validator trust evolution on chain.
8. Transaction ordering: fast BFT path separate from Cobalt governance.
9. Quantum-resistant authorization.
10. Privacy and confidential settlement.
11. Rust implementation rationale.
12. What is implemented now.
13. How to operate and inspect the network.
14. Evidence anchors.
15. Roadmap from controlled testnet to public launch.

## Writing Rules

The unified paper should:

- be one document;
- use plain English first, then math or code references where they help;
- state reasons before implementation details;
- say what exists in active voice;
- explain tradeoffs without apology;
- distinguish controlled-testnet mechanics from public operator diversity;
- avoid marketing insults toward XRP;
- avoid claiming PostFiat is categorically better than XRP;
- avoid self-defeating caveat phrases;
- avoid scattered disclaimers that interrupt the thesis;
- keep caveats in a short "Launch Boundary" or "Current Boundary" section;
- avoid multiple companion-paper links inside the final document.

## Acceptance Criteria

This milestone is complete when:

- there is one canonical whitepaper draft;
- the three old documents are either archived or clearly marked superseded;
- the unified paper explains all six first-principles design choices;
- every major implemented subsystem has a code or evidence anchor;
- the Cobalt section matches the current full-Cobalt controlled-readiness state;
- the privacy section says what exists without apology and without overclaiming;
- fixed supply is stated as 100 billion;
- no native validator reward schedule is stated clearly;
- Rust is justified as a memory-safety and implementation-risk decision;
- the document can be read end to end without needing a separate technical
  paper, math rant, or XRP-origin essay.

## Immediate Work Items

| ID | Priority | Status | Work |
| --- | --- | --- | --- |
| UWP-001 | P0 | Done | Draft the unified paper skeleton using the proposed structure above. |
| UWP-002 | P0 | Done | Pull the XRP-origin thesis into the opening sections without overstating superiority. |
| UWP-003 | P0 | Done | Move the fixed-supply, zero-issuance, and validator-subsidy math into the monetary and validator sections. |
| UWP-004 | P0 | Done | Rewrite the Cobalt section against current full-Cobalt controlled-readiness evidence. |
| UWP-005 | P0 | Done | Rewrite the privacy section around actual Orchard/Halo2 flows and remaining production work. |
| UWP-006 | P0 | Done | Refresh implementation and evidence anchors from current status docs and reports. |
| UWP-007 | P1 | Done | Archive superseded companion drafts under `docs/archive/whitepaper-drafts/2026-05-19/`. |

## Current Reference Evidence

- Full-Cobalt burndown:
  [full-cobalt-burndown.md](full-cobalt-burndown.md).
- Current chain state:
  [chain-state-current.md](chain-state-current.md).
- Privacy burndown:
  [privacy-production-burndown.md](privacy-production-burndown.md).
- Controlled Cobalt readiness:
  `reports/testnet-cobalt-controlled-readiness-gate/amendment-replay-contract-clean-v0-20260519T145213Z/testnet-cobalt-controlled-readiness-gate.json`.
- Amendment replay packet:
  `reports/testnet-cobalt-amendment-replay-bundle/cleanup-clean-v1-20260519T150324Z/testnet-cobalt-amendment-replay-bundle.json`.
- Gate-selection contract:
  `reports/testnet-cobalt-gate-selection/amendment-replay-contract-clean-v0-20260519T145213Z/testnet-cobalt-gate-selection-self-test.json`.
