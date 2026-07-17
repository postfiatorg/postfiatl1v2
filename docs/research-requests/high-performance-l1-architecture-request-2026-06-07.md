# Research Request: High-Performance PostFiat-Compatible L1 Architecture

Date: 2026-06-07 UTC  
Status: research request  
Primary objective: identify concrete architecture paths for much faster PostFiat-compatible transaction finality without violating PostFiat's monetary, validator, governance, cryptographic, or evidence constraints.

## The Ask

PostFiat wants really fast transactions. Not "good for an XRPL-style chain"; fast in the sense that a user submitting a simple native transfer should see finality in the low-latency class expected from modern high-performance L1s.

Produce well researched, well reasoned proposals for making a high-performance blockchain compatible with PostFiat L1 v2. The proposals should be concrete enough that an engineering team can turn the best option into an ADR, prototype milestone, and benchmark plan.

Do not produce generic blockchain advice. Compare the available design space against PostFiat's actual constraints and current evidence.

## Current PostFiat Context

PostFiat L1 v2 currently has:

- Rust authority-validator L1 implementation.
- Known validators, no proof of work, no proof of stake, no native validator reward schedule.
- Fixed native supply; fees price resource use and are burned rather than paid as validator yield.
- Cobalt-style validator registry/governance direction.
- ML-DSA-style post-quantum account and validator authorization.
- Orchard/Halo2-style privacy work, with privacy proof-system security treated separately from ML-DSA authorization.
- Account/balance/sequence transparent transfer path with XRP-style product vocabulary.
- Artifact-bound performance methodology: local/testnet numbers are useful only with explicit workload, hardware, validator count, finality definition, and public-vs-local boundary.

Relevant whitepaper constraints:

- Validators are natural stakeholders / known infrastructure operators, not anonymous reward farmers.
- The chain should not require permanent inflation or staking yield to pay validators.
- Fast-path changes must preserve deterministic consensus, state-root integrity, versioned state transitions, and auditable finality receipts.
- Governance and validator-set evolution must remain explicit, signed, replayable, and challengeable.
- Performance claims must remain packet-bound and reproducible.

## Existing PostFiat Performance Work

Current public article:

```text
postfiatorg.github.io/content/blog/postfiat-l1v2-private-xrpl-latency-benchmark.md
```

It reports a local private 6-validator comparison against stock and fast-timing private `rippled` controls:

```text
Post Fiat L1 v2 1000-transfer packet:
  p50 ~= 182.951 ms
  p95 ~= 214.417 ms
  p99 ~= 223.374 ms

Fast-timing private rippled control:
  p50 ~= 573.680 ms

Stock private rippled control:
  p50 ~= 3002.330 ms
```

Newer local real signed-transfer evidence in this repo is cleaner and should be treated as the current local baseline:

```text
docs/status/real-transaction-latency-benchmark-plan-2026-06-07.md

6 validators, 1000 real signed native transfers, full vote:
  wallet_to_finality_ms p50 = 89.061525
  p95 = 105.776512
  p99 = 117.092231

6 validators, 1000 real signed native transfers, quorum-fast:
  wallet_to_finality_ms p50 = 84.277622
  p95 = 100.484681
  p99 = 105.198309
```

Safety companion:

```text
reports/testnet-finality-chaos-gate/real-tx-latency-20260607/testnet-finality-chaos-gate.json
```

That gate passed 9/9 adversarial finality cases with `residual_work=[]`.

Important interpretation:

- PostFiat is already materially faster than matched private XRPL controls in local tests.
- This does not prove public WAN performance.
- The current transparent path still has XRPL-shaped account/balance/sequence constraints.
- The obvious next question is whether PostFiat should keep optimizing the account lane or add a different settlement primitive for simple value transfer.

## Current External Architecture Landscape

Use official sources first and record retrieval dates.

### XRPL

XRPL remains the most relevant historical control because PostFiat started from the XRP-style authority-validator settlement category.

Current structure:

- Validators reach agreement through trusted validator sets / UNLs.
- XRPL docs describe new ledgers usually closing every 3 to 5 seconds.
- Transactions require fees and account sequence handling.
- XRPL does not pay validators with native block rewards or staking yield.
- Its core state model is account-ledger oriented, with strong compatibility/product value but limited natural parallelism for hot accounts.

Research implication:

XRPL is the right historical comparison, but not necessarily the right target architecture if the goal is much faster simple payment finality.

Sources:

- `https://xrpl.org/docs/concepts/consensus-protocol`
- `https://xrpl.org/docs/concepts/consensus-protocol/unl`
- `https://xrpl.org/docs/concepts/ledgers/ledger-close-times`
- `https://xrpl.org/docs/introduction/transactions-and-requests`

### Canton

Canton is useful because it separates global finance workflows from public-chain-style replicated state.

Current structure:

- Daml/eUTXO-style ledger model.
- State is active contracts, not mutable account balances.
- Contracts are immutable objects created by transactions and archived by later transactions.
- Stakeholders see only the transaction views they are entitled to see.
- Synchronizers coordinate ordering and consensus without seeing decrypted transaction content.
- Sequencer orders encrypted messages; mediator collects confirmation/rejection verdicts.

Research implication:

Canton suggests a PostFiat-compatible design might keep core settlement object-like and push user balances, aliases, compliance routing, and workflow services into API/projection layers. It is not a direct blockchain-performance template, but its state model and privacy/visibility split are highly relevant.

Sources:

- `https://docs.canton.network/overview/learn/ledger-model`
- `https://docs.canton.network/overview/learn/architecture`
- `https://docs.canton.network/overview/reference/canton-protocol-specification`
- `https://docs.daml.com/canton/architecture/overview.html`

### Sui

Sui is useful because it directly attacks the account hot-state problem with an object-centric model.

Current structure:

- Asset-oriented/object-centric state model.
- Transactions take objects as inputs and produce objects as outputs.
- Owned-object transfers can avoid the same global ordering path used for shared objects.
- Shared objects require consensus and should be measured separately from owned-object transfers.
- Validators are paid through a delegated proof-of-stake economy, staking rewards, gas economics, and storage-fund mechanics.

Research implication:

Sui's owned-object/shared-object separation is probably the most important design pattern for PostFiat to evaluate. But PostFiat should not accidentally import Sui's validator economics. If PostFiat copies the object model, it still needs a validator-cost story compatible with no native staking rewards.

Sources:

- `https://docs.sui.io/`
- `https://docs.sui.io/paper/sui.pdf`
- `https://docs.sui.io/paper/tokenomics.pdf`
- `https://docs.sui.io/paper/sui-lutris.pdf`

### Avalanche / AVAX

Avalanche is useful because it offers a mature low-latency consensus family and customizable Avalanche L1s.

Current structure:

- Snowman/Avalanche consensus uses repeated random subsampled voting.
- Avalanche L1s are dynamic validator sets reaching consensus over one or more blockchains.
- The post-ACP-77 Avalanche L1 direction reduces old Subnet validator coupling to the Primary Network and changes validator requirements.
- Avalanche L1s are attractive as a comparison/control for low-latency EVM-style settlement, but the economics and probabilistic consensus assumptions differ from PostFiat's authority-validator/Cobalt direction.

Research implication:

Avalanche is relevant both as a peer benchmark and as a source of consensus ideas. The research should assess whether any Snowman-style sampling, transitive voting, or Avalanche L1 mechanics can be adapted to PostFiat's known-validator/fixed-supply/no-validator-reward constraints, or whether the mismatch is too deep.

Sources:

- `https://docs.avax.network/docs/nodes/architecture/consensus`
- `https://docs.avax.network/academy/avalanche-l1/avalanche-fundamentals/02-avalanche-consensus-intro/03-snowman-consensus`
- `https://support.avax.network/en/articles/4064861-what-is-a-subnetwork-subnet`
- `https://build.avax.network/docs/acps/77-reinventing-subnets`

## Initial Architecture Ideas To Evaluate

These are starting points, not conclusions.

### 1. Owned-Value Settlement Lane

Add a PostFiat-native object/value lane:

```text
ValueObject {
  object_id
  version
  asset_id
  amount
  owner
  policy_root
}
```

Simple transfer:

```text
inputs: owned value objects
outputs: new owned value objects
authorization: owner ML-DSA signature
validity: each input object can be consumed once at its declared version
```

Account balances become projections over active owned value objects, not the canonical write target for simple payments.

Research questions:

- Can this coexist with the current account lane?
- Can it preserve XRP-style wallet/RPC usability while changing canonical settlement state?
- What finality object should clients receive?
- Can validators certify independent object transfers without a full account-ledger write conflict?
- How do we prevent double spends under concurrent submission?
- What exact safety theorem is needed?

### 2. Two-Lane Execution: Owned Objects vs Shared Objects

Split the transaction universe:

```text
owned lane:
  simple payments, object splits/merges, private-note-like value transitions

shared lane:
  DEX/order books, trustlines, issuer controls, governance, escrow, account-key rotation, global policy roots
```

Research questions:

- Which current PostFiat/XRPL-style features must remain shared?
- Which features can become owned-object operations?
- Can the shared lane keep current certified ordering while owned transfers use a faster certificate path?
- What happens when a transaction touches both lanes?

### 3. Canton-Style Service Boundary

Keep the core ledger minimal and move service complexity outward:

```text
core:
  object spends, state roots, certificates, policy roots, finality receipts

API/projection layer:
  balances, aliases, account history, compliance labels, wallet UX, custodial reporting

apps/PIP-like services:
  customer identity, routing, managed wallets, account views, workflows
```

Research questions:

- What should be consensus state vs projection?
- Can account history and balances become rebuildable views rather than finality hot-path writes?
- Can this reduce validator cost while keeping exchange/custody UX acceptable?

### 4. Consensus Path Improvements Without Changing State Model

Keep the account lane and improve it:

- better batching;
- read/write-set declaration;
- deterministic conflict scheduler;
- append-only storage improvements;
- signature verification batching;
- certificate-size compression;
- quorum-fast completion with full propagation/audit;
- remote/WAN transport tuning.

Research questions:

- How much headroom remains before the account model itself dominates?
- Is sub-50 ms local transfer finality plausible without changing canonical state?
- What workload breaks first: same-sender burst, many-to-one fan-in, DEX operations, history indexing, or certificate propagation?

### 5. Avalanche-Inspired Sampling Or DAG Ordering

Evaluate whether PostFiat should borrow from Avalanche/Snowman or DAG mempool ideas.

Research questions:

- Is randomized sampling compatible with PostFiat's Cobalt/authority-validator governance and deterministic audit needs?
- Would sampling reduce message cost enough to matter for a 6-35 validator set?
- Are probabilistic finality assumptions acceptable for a PostFiat settlement chain?
- Could a DAG/mempool layer improve throughput without changing the finality certificate semantics?

## Required Proposal Format

Produce at least three proposals:

1. **Incremental:** improve current account/certified-finality lane.
2. **Moderate:** add owned-value lane while preserving current account/RPC compatibility.
3. **Radical:** redesign settlement around object/state-view principles while preserving PostFiat monetary and validator constraints.

For each proposal, include:

- one-paragraph thesis;
- state model;
- transaction format;
- finality path;
- validator message flow;
- double-spend/conflict handling;
- compatibility with current PostFiat account/RPC/wallet semantics;
- migration path from current account balances;
- interaction with privacy/Orchard-style notes;
- validator economics and operating-cost impact;
- expected latency/throughput shape with clear uncertainty;
- failure modes;
- safety invariants;
- benchmark plan;
- implementation milestones;
- reasons to reject the proposal.

## Hard Constraints

Do not propose:

- native staking rewards;
- inflation-funded validator subsidies;
- proof of work;
- abandoning ML-DSA authorization without a stronger post-quantum replacement;
- public-mainnet performance claims from local tests;
- centralized-sequencer designs unless they are explicitly marked as non-public/testnet-only;
- changes that make validator-set evolution opaque or non-replayable;
- changes that require validators to retain unbounded history to validate current state.

## Desired Output

Return a research memo with:

1. executive recommendation;
2. comparison table across XRPL, Canton, Sui, Avalanche, and PostFiat;
3. the three required proposals;
4. preferred prototype path;
5. explicit "unknowns that must be measured";
6. evidence packet plan;
7. final ADR outline for the chosen direction.

The best answer will be opinionated but not hand-wavy. If the right conclusion is "owned-value settlement lane is the obvious next step," say that and explain exactly why. If the better answer is "optimize current account lane first," say what measured condition would prove that.

## Acceptance Bar

The response is useful only if it can drive engineering. It should let a PostFiat engineer create:

- `docs/architecture/owned-value-settlement-lane.md` or a competing ADR;
- a prototype milestone;
- a safety test list;
- a benchmark packet plan;
- a public-claim boundary for the next blog article.
