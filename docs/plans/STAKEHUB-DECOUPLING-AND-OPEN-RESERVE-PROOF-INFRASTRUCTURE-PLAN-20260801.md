# StakeHub Deprecation and Public NAVCoin Reserve Verification — Canonical Execution Plan

**Created:** 2026-08-01

**Rewritten after executable audit:** 2026-08-02

**Priority:** P0

**Status:** **INCOMPLETE — StakeHub is not deprecated for A666 reserve publication**

**Sole authoritative Markdown:** this file is the implementation plan, current
status, continuation handoff, and definition of done for StakeHub deprecation.
The former
`docs/handoffs/STAKEHUB-DECOUPLING-CONTINUATION-HANDOFF-20260802.md` was merged
into this file and deleted. It must not be restored or maintained in parallel.

Machine-readable readiness JSON and historical evidence may support this plan,
but neither is a second architecture document and neither can weaken a
requirement here. Historical paths may retain `stakehub` in their names for
evidence integrity; no new executable path, manifest, policy, or runbook may
depend on them.

**Current bottom line:** the wallet boundary and generic proof framework are
public, but the complete A666 reserve proof is not. The readiness gate is
`0/6`, the live A666 proof lineage remains historical, and
`stakehub_deprecated=false`.

## 1. Objective

A NAVCoin represents a governed claim on a portfolio's net asset value. The
reserve evidence, liability accounting, valuation rules, aggregation, proof
construction, and L1 verification that establish that NAV must be implemented
in auditable public code.

StakeHub is an internal operator application. It is not an acceptable public
implementation, protocol dependency, proof authority, or source of meaning for
a NAVCoin. A user, auditor, NAVCoin issuer, reserve operator, or validator must
not need access to StakeHub's repository, API, filesystem, agent, credentials,
executables, or undocumented behavior to understand or reproduce what a
reserve proof verifies.

“Deprecate StakeHub” therefore means all of the following:

1. Every validation required to calculate A666 assets, liabilities, and NAV is
   implemented in `postfiatl1v2` or another genuinely public, versioned,
   licensed, reproducible repository.
2. The public proof program actually executes those validations. It must not
   merely prove that an internal operator signed aggregate numbers.
3. L1 verifies the proof under an immutable public program identity and
   enforces the exact asset, profile, manifest, policy, epoch, freshness,
   supply, reserve, and trust-class bindings.
4. A clean public checkout can collect or consume the disclosed source
   artifacts, reproduce the verified public values, build the proof, construct
   the packet, and submit it without StakeHub.
5. Live A666 is migrated through governance to that public successor without
   rewriting or deleting its historical proof profile and packets.

Removing the word “StakeHub” from the wallet is useful containment, but it is
not StakeHub deprecation. Building a generic proof framework is necessary, but
it is not StakeHub deprecation. Replacing cryptographic source validation with
operator attestations is a security downgrade, not StakeHub deprecation.

### 1.1 Plain-English end state

An auditor starting from a clean public checkout must be able to answer all of
these questions without asking the NAVCoin operator or reading StakeHub:

1. Which accounts, contracts, staking positions, exchange positions, tokens,
   debts, and liabilities are included in the NAVCoin?
2. Which public source artifacts establish each balance, position, debt, and
   ownership claim at a specific finalized source state?
3. Which public code validates those artifacts, and exactly which facts does
   it prove?
4. Which public evidence supplies each price, haircut, decimal conversion, and
   valuation rule?
5. How are verified assets and liabilities aggregated into NAV?
6. Which immutable proof program ran that code, and how can its ELF and vkey be
   reproduced?
7. How does PFTL bind the proof to the correct NAVCoin, policy, manifest,
   reserve epoch, circulating supply, and pfUSDC reserve overlay?

The required public pipeline is:

```text
public source state
  -> public collector emits bounded raw artifacts
  -> public source verifier checks inclusion, ownership, quantity, and debt
  -> public valuation verifier checks price, scale, haircut, and freshness
  -> public aggregate guest computes assets - liabilities and NAV
  -> reproducible SP1 proof commits the complete public context
  -> PFTL verifies the proof and finalizes the NAV packet
```

After deprecation, StakeHub may remain an optional internal dashboard or an
alternative client of this public pipeline. It may not contain the only
collector, validation rule, source mapping, valuation rule, policy, proof
builder, or explanation of what the proof means.

## 2. Non-negotiable NAVCoin verification model

For a NAVCoin with circulating supply `S`, the system establishes:

```text
verified net assets = sum(verified source assets) - sum(verified liabilities)
NAV per unit        = verified net assets / circulating supply
```

That arithmetic is meaningful only when the inputs have public, enforceable
semantics. The complete public system must bind:

- PFTL genesis and chain domain;
- NAVCoin asset ID;
- immutable proof-profile ID and program vkey;
- source-manifest hash;
- valuation-policy hash and valuation unit/scale;
- reserve owner or account identity for every source;
- quantity-verifier identity for every source;
- valuation-verifier identity for every source;
- observation epoch and bounded observation interval;
- source-specific freshness requirements;
- gross assets and attributable liabilities;
- source-specific haircuts;
- cryptographic, attested, and controlled trust buckets;
- circulating supply;
- pfUSDC NAV-subscription reserve overlay;
- source, observation, attestor, and disclosure roots; and
- packet uniqueness, finality, expiry, and replay protection.

Quantity and valuation are separate claims, and both require public evidence.
A cryptographically verified balance does not prove its USD price. A verified
price does not prove that the reserve owns the asset. The public profile must
state and enforce the verification method for each dimension rather than
collapsing them into a vague “proof of reserves” label.

Trust classes mean:

- **CRYPTOGRAPHIC:** the public guest verifies the relevant chain proof,
  protocol receipt, ownership proof, or other registered cryptographic
  evidence.
- **ATTESTED:** the public guest verifies a signature over the complete bounded
  statement, but the signer remains the trust source for that fact.
- **CONTROLLED:** test-only evidence. It must never authorize live value.

An adapter cannot claim `CRYPTOGRAPHIC` merely because an SP1 proof aggregated
its result. The SP1 guest must execute that adapter's verification. A source
that was cryptographically verified in the historical A666 proof cannot be
silently converted to `ATTESTED` in the successor.

For this A666 migration, these additional rules are absolute:

- no operator, StakeHub service, or replacement service may sign aggregate
  balances, liabilities, or NAV and have that signature presented as reserve
  verification;
- no source quantity or liability that can be established from public chain
  state, protocol state, receipts, ownership proofs, or reserve proofs may be
  downgraded to `ATTESTED`;
- a BFT source checkpoint is an anchor for independently validated source
  state, not a committee attestation to asset amounts; each voter must run the
  public deterministic source validation before signing the header/root;
- a signed oracle price is acceptable only when the public verifier checks the
  governed oracle identity, exact signed payload, freshness, scale, and source
  binding; an operator-entered price is not;
- the legacy Solana RPC-attestation verifier is an interim compatibility
  scaffold. The public reserve-reader successor is implemented but remains
  non-production until its immutable deployment, governed inputs, fuzzing,
  fresh epochs, reconciliation, and independent reproduction pass; and
- if a required source cannot meet the public verification standard, the
  successor profile is not qualified. The source cannot be hidden inside an
  aggregate attestation to make the gate pass.

## 3. Current state

### 3.1 What is complete

- The browser wallet and public proxy no longer require or advertise StakeHub.
- The obsolete A651 public wallet-proxy route was removed. A651 is historical
  and is not being recreated.
- The node's public runtime no longer embeds the StakeHub operator workflow.
- A standalone constrained signer and generic Ethereum export/return relays
  exist.
- `NavReservePublicValuesV1` provides a bounded provider-neutral public-values
  ABI.
- L1 supports immutable proof profiles, proof/public-value limits, freshness,
  replay protection, finalized packet history, and route/policy pinning.
- The public reserve-proof framework can build a deterministic guest, execute,
  produce a CPU Groth16 proof, locally verify it, construct a packet, and pass
  exact consensus verification.
- The public framework implements generic Ed25519 attestation and
  protocol-receipt evidence.
- The successor feature implements public source-specific verifier and
  collector code for Aave, the complete EVM spot set, Hyperliquid, staked
  NEAR, staked Solana, and Monero. Solana retains the old attested-RPC adapter
  only for historical compatibility and adds a separate cryptographic
  reserve-reader/BFT-checkpoint adapter. None of the six source families is
  production-qualified; section 3.4 records the exact remaining work.
- The public CLI emits source-checkpoint vote statements, canonically assembles
  independently signed ML-DSA votes, and rejects invalid committee bindings,
  sub-BFT-quorum certificates, duplicates, unknown validators, and bad
  signatures. Every source adapter also has an optional validator-local atomic
  mode that reproduces governed source state from the validator's RPC before
  reading its permission-restricted key, then persists anti-equivocation state
  and emits only the public vote. There is no arbitrary-checkpoint signing
  command, and no private key enters a checkpoint, certificate, observation,
  witness, proof, or packet.
- The generic ABI and route lifecycle passed a controlled six-validator qNAV
  qualification.

Public framework location:

```text
tools/nav-reserve-proof/
```

Primary implementation files:

```text
tools/nav-reserve-proof/crates/reserve-proof-types/src/lib.rs
tools/nav-reserve-proof/crates/reserve-proof-types/src/evm_checkpoint.rs
tools/nav-reserve-proof/crates/reserve-proof-cli/src/main.rs
tools/nav-reserve-proof/crates/reserve-proof-cli/src/evm_adapter.rs
tools/nav-reserve-proof/programs/reserve-proof-guest/src/main.rs
crates/types/src/nav_reserve_public_values.rs
crates/nav_reserve_protocol/src/lib.rs
crates/execution/src/nav_sp1_verifier.rs
```

Machine-readable A666 source-adapter readiness lives at:

```text
docs/status/A666-PUBLIC-ADAPTER-READINESS-20260802.json
```

CI rejects a claim that StakeHub is deprecated unless every required adapter
in that file is implemented and production-qualified.

### 3.2 What is not complete

The public repository does not yet contain a production-complete,
independently reproducible path for any of these required A666 source
families:

- Aave collateral and debt;
- A666's complete Ethereum spot portfolio;
- Hyperliquid balances and positions;
- staked NEAR;
- staked Solana; or
- Monero reserves.

The existing historical A666 public shadow represents all six sources as
`ed25519-attestation-v1`. That shadow proves that an attestor signed the
numbers and that the aggregation arithmetic is consistent. It does not
reproduce the source verification performed by the internal StakeHub proof.

That shadow is **DISQUALIFIED FOR LIVE ACTIVATION**:

```text
tools/nav-reserve-proof/qualifications/a666-shadow-20260730
```

It may remain as labeled historical reconciliation evidence. It must not be
registered, governed live, or used to claim that StakeHub has been deprecated.

### 3.3 Live A666 was not downgraded

Live A666 remains on its historical StakeHub-derived proof profile. No work in
this continuation replaced its real proofs with attestations, changed the live
route, moved funds, modified validator state, or changed deposited pfUSDC.

This preserves current behavior, but it also means fresh A666 reserve
publication still depends on the internal implementation. The live system is
not yet the desired public architecture.

### 3.4 Exact adapter implementation truth on 2026-08-02

| A666 source family | Public implementation state | Production-qualified | What remains |
|---|---|---:|---|
| Aave on Arbitrum | Provider-neutral verifier, public checkpoint/collection workflow, and typed candidate policy/committee commitment derived from historical public state proofs implemented; partial | No | Review/tighten the candidate freshness bound, retain production fuzz campaigns, run fresh epochs and complete A666 reconciliation, independently reproduce, and qualify |
| Complete EVM spot set | Provider-neutral quantity verifier, public checkpoint/collection workflow, public Chainlink state-proof valuation successor, typed two-chain quantity policy, and typed Arbitrum ETH/USD + USDC/USD valuation policy/committee commitments derived from retained historical state proofs implemented; partial | No | Review the candidate freshness bound, collect fresh committee-certified feed proofs, retain production fuzz campaigns, run fresh epochs and complete A666 reconciliation, independently reproduce, and qualify |
| Hyperliquid | Provider-neutral verifier, public HyperCore receipt-reader contract, unsigned snapshot construction, checkpoint/owner workflow, and receipt-proof collector implemented; partial | No | Deploy the hardened public reader, govern policy/committee inputs, fuzz, reproduce complete historical and fresh epochs, complete full A666 reconciliation, and qualify |
| Staked NEAR | Provider-neutral quantity verifier, public reader contract, unsigned invocation construction, finalized-head checkpoint workflow, owner authorization, outcome/block-proof collector, and public Chainlink state-proof valuation successor implemented; partial | No | Deploy the public reader, govern quantity/valuation policy and committee inputs, collect exact NEAR/USD proofs, add fuzzing, reproduce historical and fresh epochs, complete full A666 reconciliation, and qualify |
| Staked Solana | Public stateless reserve-reader program, exact unsigned transaction construction, finalized transaction/block collector, immutable program identity check, owner authorization, BFT source checkpoint, bounded parser, successor verifier, guest dispatch, public Chainlink state-proof valuation successor, and independently repeated reproducible SBF build implemented; partial. The old attested-RPC adapter remains separately labeled historical evidence. | No | Deploy the exact built reader immutably, publish its on-chain ProgramData identity and governed A666 quantity/valuation policy and committee inputs, collect exact SOL/USD proofs, fuzz, run fresh epochs, reconcile, independently reproduce, and qualify |
| Monero | Provider-neutral cryptographic quantity verifier, context-bound challenge, public ReserveProofV2 parser, transaction/block/header collector, certified key-image status workflow, and public Chainlink state-proof valuation successor implemented; partial | No | Produce a fresh governed nonzero proof and independently signed checkpoint, govern and collect the exact XMR/USD proof, fuzz, complete A666 reconciliation, independently reproduce, and qualify |
| pfUSDC overlay | Implemented and pushed | Not sufficient by itself | Exact-tip remote CI must pass; this covers only PFTL-accounted subscription reserves, not the six external source families |

The Aave, complete-EVM-spot, Hyperliquid, NEAR, Solana, and Monero verifiers
pass their source tests and registered guest dispatch tests. Aave reproduces the
historical A666 collateral and debt results. Its public CLI now constructs the
source checkpoint candidate, emits owner authorization, and collects every
policy-pinned token, user mapping slot, reserve-index, rate, oracle source,
Chainlink round, and capped-stable proof from the certified EVM block. During
collector implementation, the previously witness-selected per-user token
mapping slot was moved into the policy commitment; substitution now fails
closed. Aave remains unqualified pending governed A666 inputs, adversarial/fuzz
coverage, fresh epochs, and reconciliation. EVM spot reconstructs every
historical native and ERC-20 account/storage proof; Hyperliquid and NEAR
reconstruct their historical receipt/Merkle evidence; Solana reconstructs the
historical stake quantities and authority data. Its successor uses a public
reader transaction whose exact message, ordered accounts, canonical output,
reader program-data hash, absent upgrade authority, slot, owner signature,
and quorum-certified finalized checkpoint are verified in public code. This
is cryptographic relative to the disclosed BFT checkpoint, not direct Solana
consensus verification. Monero reconstructs the real
historical nonzero transaction/RingCT/ownership/inclusion proof and also
verifies a synthetic context-bound zero-reserve test vector without treating
an aggregate amount signature as proof. Its public collector parses the wallet
ReserveProofV2 and collects complete transactions, Merkle inclusion, bounded
header anchors, and certified key-image status. Its production path still
lacks a fresh governed nonzero header-chain/spent-status proof and separate
XMR/USD valuation evidence. The EVM spot adapter proves
reserve quantities only and deliberately leaves USD prices in the separately
declared valuation trust dimension. That dimension now has a public
`evm-chainlink-state-proof-valuation-v1` successor: it reruns the registered
quantity verifier, proves policy-pinned Chainlink feed state beneath a
quorum-certified EVM state root, applies governed haircuts, and recomputes the
aggregate valuation with checked conservative rounding. It accepts neither an
operator-signed aggregate value nor an RPC-returned price as proof. The
candidate A666 EVM-spot policy now pins the Arbitrum ETH/USD and USDC/USD
proxy/aggregator identities proven in the retained historical Aave witness,
and derives verifier commitment
`39f33f6763ff108f7f6ab88e50325a3438125ef2ebaa88fc23d2059c43b61103335269d1dca231117d6a5b187084ade6`.
Fresh committee-certified price proofs and freshness-policy review remain
open. The EVM
spot public CLI constructs deterministic
per-chain checkpoint candidates from pinned RPC heights, supports independent
checkpoint voting and assembly, emits the exact owner-authorization statement,
and collects the complete native/ERC-20 proof set from an exact reviewed RPC
map. It is still unqualified until governed A666 inputs, adversarial/fuzz
coverage, fresh epochs, and reconciliation exist. The Solana implementation
does not relabel the old RPC snapshots: that adapter remains `attested`, while
the distinct reserve-reader adapter is classified `cryptographic` only under
its policy-pinned public reader and BFT checkpoint. It is still partial and
cannot satisfy `G3` until the deployment and qualification gates pass. The
implementations pass strict verifier-crate lint and the provider-neutral
shipped-code boundary. They are excluded from the immutable legacy profile.
That identity is reproduced from the exact public source commit pinned beside
its ELF hash and vkey; current-checkout Cargo metadata is not mistaken for the
legacy source. The successor receives a distinct identity only after full
qualification. These changes do not alter the
`0/6` production-qualification result and cannot be cited as evidence that
StakeHub is deprecated. Their collectors, fuzz targets, fresh source epochs,
complete A666 reconciliation, and production qualification remain open.

## 4. Required public code boundary

All code required to interpret and verify NAVCoin reserve claims must live in:

1. `postfiatl1v2`; or
2. a separate public repository that is licensed, release-tagged, pinned by
   exact commit and artifact hashes, reproducibly built in CI, and consumed by
   `postfiatl1v2` qualification tests.

For the current A666 work, the decided implementation home is this public
`postfiatl1v2` repository. Do not create another private service or repository
to finish the missing adapters. A later extraction is acceptable only into a
genuinely public repository under the pinning and reproducibility rules above.

Code ownership inside this repository is:

| Responsibility | Public location |
|---|---|
| Source schemas, policies, and verification logic | `tools/nav-reserve-proof/crates/reserve-proof-types/` |
| Collectors, checkpoint assembly, witness construction, proving, and packet CLI | `tools/nav-reserve-proof/crates/reserve-proof-cli/` |
| SP1 aggregate guest | `tools/nav-reserve-proof/programs/reserve-proof-guest/` |
| Public source-reader contracts/programs | `crates/ethereum-contracts/` and source-specific public program directories added under the same repository |
| A666 source manifest and governed policy fixtures | `tools/nav-reserve-proof/manifests/a666/` |
| Historical/adversarial fixtures and qualification outputs | `tools/nav-reserve-proof/fixtures/` and `tools/nav-reserve-proof/qualifications/` |
| Provider-neutral public-values ABI and packet construction | `crates/types/` and `crates/nav_reserve_protocol/` |
| Deterministic L1 proof/profile/packet enforcement | `crates/execution/` |
| Machine readiness truth | `docs/status/A666-PUBLIC-ADAPTER-READINESS-20260802.json` plus CI gates |

The preferred structure is:

```text
tools/nav-reserve-proof/
  crates/
    reserve-proof-types/
    reserve-proof-cli/
    adapter-evm-erc20/
    adapter-evm-spot/
    adapter-aave/
    adapter-hyperliquid/
    adapter-near/
    adapter-solana/
    adapter-xmr/
  programs/
    reserve-proof-guest/
  fixtures/
    evm-erc20/
    evm-spot/
    aave/
    hyperliquid/
    near/
    solana/
    xmr/
  manifests/
    a666/
  qualifications/
```

The exact crate split may change, but these public responsibilities may not:

- source artifact schemas;
- source collectors that can run with ordinary operator-supplied RPC/API
  endpoints;
- ownership-challenge construction;
- receipt, state-proof, and reserve-proof parsing;
- verifier logic executed by the guest;
- canonical encoding and domain-separated hashing;
- source normalization and ordering;
- quantity, liability, valuation, and haircut calculations;
- trust classification;
- witness construction;
- proof execution and production;
- local proof and public-value verification;
- packet construction and submission; and
- positive and adversarial fixtures.

Credentials, private keys, authenticated API tokens, and operator deployment
configuration remain private. Private credentials do not justify private
verification logic. A proprietary collector may be supported as an optional
integration, but it cannot be the only way to produce a source artifact or the
only implementation of a validation claimed by the proof profile.

Validators must not call external chains, exchanges, custodians, or RPC
providers during consensus execution. External data collection happens before
proof submission. Validators deterministically verify the bounded proof and
its public bindings.

## 5. Required A666 source adapters

The internal StakeHub files named below are migration source material only.
They are not acceptable runtime dependencies, public trust anchors, or final
code locations. Provider-specific names, hard-coded owners, fixed six-leg
enums, internal paths, and `stakehub-*` hash domains must not be copied into the
successor semantics.

### 5.1 Aave on Arbitrum

The public adapter must verify:

- Arbitrum chain identity and governed Aave deployment identity;
- reserve owner/account control;
- the relevant account and storage state under an accepted state root;
- collateral assets and balances;
- all attributable debt, without omission or understatement;
- token identities and decimal normalization;
- collateral and debt valuation evidence;
- duplicate reserve/position rejection;
- block and observation freshness;
- checked asset, liability, and net arithmetic; and
- canonical evidence and disclosure commitments.

Internal migration inputs include:

```text
/home/postfiat/repos/StakeHub/zk/contracts/src/StakeHubAaveVerifier.sol
/home/postfiat/repos/StakeHub/zk/shared/src/locked.rs
/home/postfiat/repos/StakeHub/stakehub/prove_reserves.py
```

The existing public ERC-20 MPT adapter is not by itself sufficient for an Aave
position because the NAV calculation must include both collateral and debt.

### 5.2 Ethereum and EVM spot assets

The public adapters must verify:

- governed chain, token, owner, account, and storage-slot identities;
- owner authorization where required;
- account and balance-slot inclusion beneath the accepted state root;
- block identity, confirmation/finality policy, and freshness;
- token decimals and quantity normalization;
- duplicate token/account/chain rejection;
- separate valuation evidence and policy binding; and
- checked aggregation across every configured spot position.

Internal migration input:

```text
/home/postfiat/repos/StakeHub/zk/shared/src/evm_spot.rs
```

Reuse and extend the public `evm-erc20-bft-checkpoint-mpt-v1` adapter rather
than inventing a second incompatible EVM proof format.

### 5.3 Hyperliquid

The public adapter must reproduce or strengthen the existing checks for:

- pinned HyperEVM block/header identity;
- receipts-root extraction and receipt inclusion;
- governed snapshot-reader contract and event topic;
- snapshot commitment and salt binding;
- reserve account identity;
- margin summary and withdrawable balance consistency;
- spot total/hold/unlocked calculations;
- perpetual positions and notional calculations;
- governed token IDs, decimal scales, and price inputs;
- duplicate spot and perpetual rows;
- negative or invalid balances and prices;
- bounded position counts and proof sizes;
- checked arithmetic; and
- observation freshness and replay resistance.

Internal migration inputs include:

```text
/home/postfiat/repos/StakeHub/zk/shared/src/hl_receipt_leg.rs
/home/postfiat/repos/StakeHub/zk/shared/src/hl_leg.rs
/home/postfiat/repos/StakeHub/zk/script/src/bin/fetch_hl_receipt.rs
/home/postfiat/repos/StakeHub/zk/script/src/bin/fetch_hl_leg.rs
/home/postfiat/repos/StakeHub/zk/contracts/src/HyperCoreReader.sol
```

The reader contract has now been ported, hardened, and tested publicly at
`crates/ethereum-contracts/src/HyperCoreReserveReader.sol`. The verifier now
also requires the receipt's spot rows to equal the complete policy-pinned set,
not merely be members of an allowlist. It requires the exact policy-pinned
perpetual set and requires its recomputed total notional to equal HyperCore's
account-wide `ntlPos`, preventing an unlisted live perpetual from being hidden
by request omission. Duplicate snapshot events fail closed. The private paths
above are historical migration references only; they are not acceptable
runtime or build inputs.
Public transaction construction, block-header validation, reader-code
checking, receipt retrieval, canonical receipt RLP, receipt-trie
reconstruction, inclusion proof extraction, source checkpoint creation, owner
authorization, and verified observation output now live in
`tools/nav-reserve-proof/crates/reserve-proof-cli/src/hyperliquid_adapter.rs`.
The snapshot and collection path accepts no owner private key and its
historical public-RPC test reproduces the existing certified receipts root.
The optional validator-local checkpoint mode reads only that validator's
permission-restricted ML-DSA key after reproducing the header, reader code,
and confirmation depth, and never exports the key.

HyperEVM currently exposes a zero block `stateRoot` and rejects
`eth_getProof`, so reader bytecode identity cannot be account-MPT proven under
the header. The successor therefore makes this trust boundary explicit: the
quorum-certified source checkpoint commits the receipts root, reader address,
and reader bytecode hash, and every checkpoint validator must independently
reproduce the exact header, code hash, and minimum depth before signing. The
snapshot receipt, payload, exact position sets, quantities, liabilities, and
HyperCore-derived prices are still cryptographically checked under that
certified receipts root. Deployment of the hardened reader, governed A666
policy/committee inputs, fuzz qualification, fresh epochs, and full A666
reconciliation remain open.

The current A666 shadow marks Hyperliquid quantity and valuation as attested.
That is not equivalent to the historical receipt validation and cannot be the
live successor.

### 5.4 Staked NEAR

The public adapter must verify:

- NEAR mainnet and accepted light-client/head identity;
- execution outcome and block/outcome Merkle proofs;
- governed reader account and code hash;
- governed staking-pool identity and code hash;
- reserve account ownership;
- snapshot event schema and commitment;
- staked and unstaked yoctoNEAR quantities;
- duplicate or substituted receipt rejection;
- observation freshness;
- checked denomination conversion; and
- separate NEAR/USD valuation evidence.

Internal migration inputs include:

```text
/home/postfiat/repos/StakeHub/zk/shared/src/near_receipt_leg.rs
/home/postfiat/repos/StakeHub/zk/shared/src/near_leg.rs
/home/postfiat/repos/StakeHub/zk/near-stake-reader/src/lib.rs
/home/postfiat/repos/StakeHub/zk/script/src/bin/fetch_near_receipt_leg.rs
```

The provider-neutral successor implementation now lives publicly at:

```text
tools/nav-reserve-proof/contracts/near-stake-reader/
tools/nav-reserve-proof/crates/reserve-proof-cli/src/near_adapter.rs
tools/nav-reserve-proof/crates/reserve-proof-types/src/near_receipt.rs
```

The public reader accepts no funds, queries the standard staking-pool
interface, emits the canonical `postfiat-nav` snapshot event, and returns the
same raw payload. The public CLI emits an unsigned external-wallet invocation,
constructs an independently reproducible finalized-head checkpoint candidate,
checks the policy-pinned reader and pool code hashes at that exact head,
collects the callback receipt outcome/block Merkle proof, emits the exact
reserve-owner signing statement, and verifies the complete quantity evidence
before writing an observation. The proof kit never receives a NEAR private
key. Code identity and the finalized head are explicitly BFT-checkpointed;
the RPC response is not mislabeled as trustless NEAR consensus finality.

The reader is not deployed under a governed successor account yet. Governed
policy/committee inputs, public NEAR/USD valuation evidence, parser fuzzing,
fresh multi-epoch collection, full A666 reconciliation, and independent
production qualification remain open. Therefore this implementation does not
yet make the staked-NEAR source production-qualified.

The current A666 shadow marks staked-NEAR quantity as attested. That is not
equivalent to the historical receipt/light-client validation.

### 5.5 Staked Solana

The public adapter and collector must make the intended trust boundary
explicit and verify at least:

- Solana cluster and finalized slot identity;
- reserve owner and stake authority;
- stake-account and vote-account identities;
- lamport balance and denomination conversion;
- stake activation, deactivation, delegated, and withdrawable state;
- duplicate stake-account rejection;
- bounded account lists and response sizes;
- observation freshness; and
- separate SOL/USD valuation evidence.

Internal migration inputs include:

```text
/home/postfiat/repos/StakeHub/zk/shared/src/solana_leg.rs
/home/postfiat/repos/StakeHub/zk/script/src/bin/fetch_solana_leg.rs
```

The current signed-RPC snapshot path may remain as labeled historical or
diagnostic evidence, but it is not the production successor. The production
path must add a public source-state mechanism, for example a governed public
Solana reserve-reader program that reads the exact policy-pinned stake
accounts and emits a commitment, combined with public verification of the
reader program identity, transaction/block inclusion, finalized source
checkpoint, complete account set, account state, and reserve ownership. The
exact mechanism must be selected and qualified in public code. Signing an
amount returned by RPC is not sufficient.

The selected successor is now implemented in public source. The stateless
`contracts/solana-stake-reader` program reads the exact ordered, read-only
stake-account set and Clock sysvar, rejects wrong owners, duplicates,
writable/executable accounts, malformed stake state, and invalid delegation
amounts, and
emits a canonical salted snapshot. The public CLI constructs the exact legacy
transaction message for an external wallet, fetches the successful finalized
transaction and containing block, enforces policy-pinned finality depth and
maximum slot lag, verifies that the reader is an immutable upgradeable-loader
deployment with the exact policy-pinned ProgramData hash, and emits the shared
BFT checkpoint candidate. The successor verifier checks the transaction
signature and exact message/account/instruction structure, reader output and
return-data hash, all position/authority/vote bindings, owner authorization,
and the assembled ML-DSA checkpoint certificate before producing quantity
evidence. No Solana or reserve-owner private key enters the proof kit. The
optional validator-local checkpoint mode reads a permission-restricted PFTL
validator key only after reproducing the complete Solana source checkpoint,
persists anti-equivocation state, and exports only its vote. This proves the
source values relative to the disclosed BFT checkpoint; it does not claim
direct Solana consensus verification.

This code is still partial. A `solana-verify` 0.5.1 build under the exact
published Docker image digest was repeated and produced the same executable
program hash, raw ELF hash, and byte length. That identity is machine checked
by `scripts/check-solana-stake-reader-identity`. No immutable public deployment
has been recorded, no governed A666 reader/policy/committee/SOL-USD valuation
bundle exists, and parser fuzzing, fresh multi-epoch collection, full A666
reconciliation, and independent production reproduction remain open.
Therefore staked Solana remains `0/1` qualified and does not make `G3` pass.

### 5.6 Monero reserves

The public adapter must verify:

- governed Monero network and reserve address;
- a fresh, domain-separated challenge bound to the NAVCoin, profile, manifest,
  policy, epoch, and observation interval;
- the Monero reserve proof against public inputs;
- proven quantity and any spent/unavailable amount required by the proof
  semantics;
- address/control substitution rejection;
- replay and stale-proof rejection;
- bounded proof and parser behavior; and
- separate XMR/USD valuation evidence.

Internal migration inputs include:

```text
/home/postfiat/repos/StakeHub/zk/shared/src/xmr_reserve_leg.rs
/home/postfiat/repos/StakeHub/zk/shared/src/xmr_sidecar.rs
/home/postfiat/repos/StakeHub/zk/script/src/bin/fetch_xmr_reserve_leg.rs
/home/postfiat/repos/StakeHub/stakehub/monero_scan.py
```

The provider-neutral successor implementation now lives publicly at:

```text
tools/nav-reserve-proof/crates/reserve-proof-cli/src/monero_adapter.rs
tools/nav-reserve-proof/crates/reserve-proof-types/src/monero_reserve.rs
```

The public CLI emits the exact NAVCoin/profile/manifest/policy/epoch-bound
challenge for an external Monero wallet, parses the wallet-created
`ReserveProofV2` with bounded canonical decoding, fetches each complete
transaction and source block, reconstructs the transaction-tree branch,
builds a bounded header-chain anchor or an explicitly policy-allowed pinned
historical anchor, queries the exact key-image status set, and emits the source
checkpoint candidate for independent validator reproduction and signing. It
then attaches the assembled certificate, runs the complete public quantity
verifier and separate valuation verifier, and writes the source observation.
No wallet seed, spend key, or view key enters the proof kit. The optional
validator-local checkpoint mode reads a permission-restricted PFTL validator
key only after reproducing the complete Monero source checkpoint, persists
anti-equivocation state, and exports only its vote.

The public parser has successfully decoded an existing real wallet-created
proof and the public RPC client has independently decoded and hash-checked a
finalized Monero mainnet block. The old proof is intentionally rejected by the
new collection workflow because its message predates the context-bound public
challenge. A fresh nonzero wallet proof, independently signed source
checkpoint, governed XMR/USD valuation evidence, fuzzing, multi-epoch A666
reconciliation, and independent production reproduction remain open.

A zero XMR balance in one historical epoch does not remove the requirement.
The adapter must correctly verify both zero and nonzero reserves before it can
be part of the production profile.

### 5.7 pfUSDC NAV-subscription reserve overlay

pfUSDC deposited through A666 primary issuance is part of the NAVCoin reserve.
It is PFTL-accounted reserve state, not an off-chain operator inventory and not
an attested number.

The public packet builder and consensus must use the same canonical function
to bind:

- base proof public values;
- proof-verified external net assets;
- finalized pfUSDC subscription source root;
- finalized pfUSDC subscription value; and
- checked total verified net assets.

The implementation centralizes this host-side construction in:

```text
crates/nav_reserve_protocol/src/lib.rs
```

and makes both consensus and the public packet builder use it. This crate is
deliberately outside `postfiat-types`: the SP1 guest links `postfiat-types`, so
adding a host-only packet helper there would unnecessarily change the
immutable guest ELF and vkey. The separated implementation reproduces the
existing guest ELF SHA exactly.

## 6. Implementation plan

### Phase 0 — freeze unsafe shortcuts

- [x] Leave the live A666 profile and route unchanged.
- [x] Label the old attestation-only A666 shadow as retired/historical.
- [x] State explicitly that the shadow is not activatable.
- [x] State explicitly that the generic framework does not deprecate StakeHub.
- [x] Add a CI/documentation gate that rejects any claim that `G3` or complete
  StakeHub deprecation has passed while required public adapters are absent.

### Phase 1 — finish the pfUSDC overlay packet path

- [x] Add a shared checked composite-root/total function to the public
  `postfiat-nav-reserve-protocol` crate without changing the SP1 guest.
- [x] Make execution use that shared function instead of a duplicate formula.
- [x] Extend the public packet template and builder with optional subscription
  overlay root/value fields.
- [x] Add stable-vector, malformed-root, zero-value, overflow, proof-only,
  overlay, and mismatch unit tests.
- [x] Complete adversarial review of the consensus equivalence.
- [x] Run full `postfiat-types`, `postfiat-execution`, and proof-kit suites.
- [x] Run formatting and check targets required by CI, and `git diff
  --check`.
- [x] Commit and push the narrow patch without live governance operations.
- [ ] Require green exact-tip remote CI.

### Phase 2 — port the source validators

- [x] Implement the shared provider-neutral BFT checkpoint statement,
  certificate assembly, and validation workflow without centralizing signer
  private keys. Require the BFT threshold in the shared verifier, not only in
  manifest construction.
- [x] Implement validator-local atomic reproduce-and-sign mode for Aave, EVM
  spot, Chainlink valuation, Hyperliquid, NEAR, Solana, and Monero. Validate
  permission-restricted key ownership against the governed committee, emit no
  secret material, persist same-source/epoch/height anti-equivocation state,
  and expose no arbitrary-checkpoint signer.
- [x] Implement and register the public Aave adapter.
- [x] Implement the public Aave checkpoint candidate, owner authorization,
  and complete RPC proof collector; policy-pin the user balance mapping slots.
- [x] Implement and register the complete public EVM spot quantity adapter set.
- [x] Implement the public complete-EVM-spot checkpoint candidate, owner
  authorization, and multichain RPC collector workflow.
- [x] Implement and register the public Hyperliquid adapter.
- [x] Port and test the HyperCore receipt-reader contract publicly and reject
  omitted, added, reordered, or duplicated governed spot rows.
- [x] Implement public HyperCore snapshot transaction construction plus
  HyperEVM header, receipt, and receipt-trie proof collection. It accepts a
  transaction hash signed and submitted by an external wallet and never
  requires a private key inside the proof kit.
- [x] Implement and register the public staked-NEAR adapter.
- [x] Implement the public NEAR reader invocation and complete finalized
  outcome/block proof collector.
- [x] Implement the interim public staked-Solana attested-RPC verifier for
  historical reconstruction.
- [x] Implement the separate public Solana reserve-reader, finalized
  transaction/block collector, owner authorization, source checkpoint,
  successor verifier, bounded parser, and guest dispatch described in section
  5.5; keep the interim signed-RPC adapter explicitly attested.
- [ ] Build and deploy the Solana reader immutably, publish governed A666
  policy/committee/valuation inputs, fuzz it, reproduce fresh epochs, reconcile
  the complete A666 profile, and qualify it independently.
- [x] Implement and register the public XMR reserve-proof quantity adapter.
- [x] Implement the public Monero reserve-proof, header-chain, transaction
  inclusion, ownership, and key-image spent-status collector for zero and
  nonzero reserves.
- [x] Implement and register a public Chainlink account/storage-proof
  valuation adapter and collector that reruns the registered EVM-spot, NEAR,
  Solana-reader, or Monero quantity verifier and derives the exact valued
  amount and governed haircut without an aggregate operator attestation.
- [ ] Remove all provider-specific hash domains and compiled operator identities
  from successor semantics.
- [ ] Add public fixtures and adversarial tests for every adapter.
- [x] Add the initial coverage-guided full-witness and tagged source-evidence
  fuzz harness, guarded temporary-corpus runner, and fixed-duration CI smoke
  campaigns. The first local campaigns completed 1,860,664 full-witness and
  3,340,640 source-evidence executions without a crash, timeout, or OOM. This
  broad harness does not replace the required parser-specific targets below.
- [x] Add coverage-guided targets for every current public reserve-proof input
  boundary: canonical witness JSON and CBOR, tagged source evidence, shared EVM
  RPC quantities/state proofs, HyperEVM headers/receipts/tries, NEAR
  heads/light proofs, Solana program/transaction/reader payloads, Monero
  ReserveProofV2/transactions, and source-checkpoint committee/vote/certificate
  material. A clean nine-target smoke campaign on 2026-08-02 completed
  4,010,896 executions without a crash, timeout, or OOM. This closes target
  coverage, not the retained per-source production fuzz qualification below.
- [ ] Prove malformed or unsupported adapter evidence fails closed without
  panic or unbounded work.

### Phase 3 — build the source-equivalent A666 profile

- [ ] Create a public A666 source manifest selecting the real adapter for each
  quantity and valuation dimension.
- [x] Implement the generic typed manifest builder. It derives reserve-owner,
  quantity-verifier, valuation-verifier, and haircut commitments from the
  actual public policies and complete committees. It also derives the global
  32-byte valuation-policy hash from a typed, domain-separated policy binding
  the NAV asset, valuation unit and scale, exact source/position set, valuation
  method, and asset/liability treatment. It rejects arbitrary pasted hashes,
  policy/manifest source drift, missing committees, unsupported integrated
  valuation, valuation context drift, and any price-row/quantity-position or
  decimal mismatch.
  The actual governed A666 input bundle remains open until the public reader
  deployments and real policy values are complete.
- [x] Publish the typed six-source A666 candidate portfolio valuation policy
  and its independently derivable hash
  `350eaee0a1ca12ba51637781ba52661b8685f868657a7c5e7d07c31b2899869c`.
  This is not a live policy or a source qualification; it removes the legacy
  pasted-hash dependency and gives every source-specific candidate policy one
  public successor valuation context.
- [x] Publish typed Aave-Arbitrum and two-chain EVM-spot candidate quantity
  policies and pinned verifier commitments. Tests deserialize the public
  policies, derive the commitments against the six-validator committee, and
  match contract, position, slot, and code-hash fields back to the retained
  historical public state-proof artifacts. Aave's candidate commitment is
  `48c7f466100b4dc7d72f57cfff3c9dadb6d84c7c13869adb09be59a8f6043ca1b2d1fed77b2681f91cd13d22aa6eb969`;
  EVM spot's is
  `4e7bee12bfa7cefa615210d08d74f773eaf58e8c492837222a6e36ddbe23d21f96fe1f7634d441db401c102be00b6d54`.
  EVM spot's typed Arbitrum Chainlink valuation policy covers all four exact
  quantity positions with ETH/USD or USDC/USD, binds the portfolio policy hash,
  committee, storage slots, code hashes, decimals, and haircuts, and derives
  commitment
  `39f33f6763ff108f7f6ab88e50325a3438125ef2ebaa88fc23d2059c43b61103335269d1dca231117d6a5b187084ade6`.
  Tests match its feed identities to the retained historical Aave state proofs.
  These remain unqualified candidates; fresh committee-certified feed proofs,
  freshness-bound review, reconciliation, and independent qualification remain
  open.
- [x] Derive and publish the candidate source-checkpoint committee from the
  public six-validator PFTL registry. The generic builder accepts only
  ML-DSA-65 public keys, canonicalizes validator ordering, rejects quorums
  below the BFT threshold, and derives its root. The manifest builder repeats
  the BFT-threshold check. A666's candidate is epoch 1, quorum 5,
  committee root
  `99b0a3b8af49f3c91537d24a698e47f5761eec65890d00cdc4070ec99b18b333dfe514199c258cda5b81bec33821e245`.
  It contains no validator private keys.
- [ ] Preserve or strengthen historical cryptographic trust classifications.
- [ ] Eliminate operator-signed aggregate quantity, liability, and NAV inputs;
  permit signed external data only under the exact restrictions in section 2.
- [ ] Implement and bind public valuation evidence for every non-pfUSDC source;
  a quantity proof without its governed price does not establish NAV.
- [ ] Bind reserve owners, verifier keys/committees, source domains, position
  identities, freshness policies, haircuts, and valuation policy.
- [ ] Rebuild the canonical SP1 guest in the pinned Docker toolchain.
- [ ] Reproduce the ELF hash and vkey from independent checkout paths.
- [ ] Register nothing live; first derive the immutable candidate profile ID
  and publish its complete manifest and program identity for review.

### Phase 4 — source-by-source qualification

- [ ] Reconstruct at least two historical A666 epochs with the public adapters
  where source artifacts remain available.
- [ ] Compare each public adapter output against the historical source result,
  not merely the aggregate NAV.
- [ ] Explain and govern every conservative difference.
- [ ] Reject any unexplained trust downgrade.
- [ ] Run at least two fresh A666 shadow epochs from newly collected source
  artifacts without StakeHub.
- [ ] Verify gross assets, liabilities, net assets, trust buckets, supply,
  pfUSDC overlay, and NAV per unit.
- [ ] Produce and independently verify CPU Groth16 proofs.
- [ ] Construct overlay-aware reserve packets with the public CLI.
- [ ] Prove wrong source, owner, profile, vkey, policy, manifest, epoch,
  interval, valuation, overlay, and proof substitutions fail.
- [ ] Preserve the old profile and packets as immutable historical records.

CPU proof production is sufficient for correctness qualification. CUDA or an
authenticated network prover is a separate operational-throughput gate and
must not be confused with source-verification completeness.

### Phase 5 — controlled six-validator migration rehearsal

- [ ] Start a clean project-controlled six-validator environment at the exact
  candidate commit and release artifacts.
- [ ] Register the immutable candidate profile.
- [ ] Submit and finalize a fresh overlay-aware reserve packet.
- [ ] Govern an A666-shaped route to the candidate profile and packet.
- [ ] Verify identical state roots across all validators.
- [ ] Execute transparent issue and redeem.
- [ ] Execute private-middle issue and redeem.
- [ ] Export the wrapped asset through the generic constrained signer.
- [ ] Return the wrapped asset and restore native NAVCoin.
- [ ] Test replay, stale proof, wrong profile, wrong overlay, restart, snapshot
  restore, partial outage, and malformed input.
- [ ] Verify reserve, asset-supply, bridge-supply, and balance conservation.
- [ ] Rehearse governed pause and rollback. Never edit validator state.

### Phase 6 — live A666 migration

Before any live transaction:

- [ ] `G3` is green for all six source families.
- [ ] Exact-tip CI and release artifacts are green.
- [ ] Fresh multi-epoch shadow reconciliation is green.
- [ ] Six-validator migration and rollback rehearsal is green.
- [ ] Current A666 profile, packet, route, supply, reserve, and pfUSDC overlay
  state are read independently from every validator.
- [ ] Current fresh source artifacts and public adapter outputs are reviewed.
- [ ] Candidate profile ID, vkey, manifest hash, policy hash, public values,
  packet hash, and trust classes are independently reproduced.
- [ ] Generic signer and Ethereum route policy are ready.
- [ ] Governance activation, pause, and rollback bundles are staged with exact
  preconditions.
- [ ] Required keys have secure off-host backups.

Then:

- [ ] Register the immutable public A666 successor profile.
- [ ] Submit and finalize the fresh public reserve packet.
- [ ] Govern the existing A666 route to the successor.
- [ ] Verify fleet convergence and state-root equality.
- [ ] Run small transparent issue/redeem.
- [ ] Run small private issue/redeem.
- [ ] Run Ethereum export/return.
- [ ] Verify balances, supply, reserves, packet freshness, replay rejection,
  and historical queryability.
- [ ] Keep the route active only if every gate passes; otherwise pause it and
  execute the rehearsed governed recovery.

The proof-profile migration does not require a new NAVCoin. A666 remains A666.

### Phase 7 — clean public reproduction

- [ ] From a clean tagged public checkout, build the node, wallet, signer,
  proof kit, every A666 adapter, guest, and prover.
- [ ] Reproduce the guest ELF hash and vkey.
- [ ] Collect or consume every documented source artifact without StakeHub.
- [ ] Reproduce all public values, trust classifications, profile ID, and
  packet hash.
- [ ] Complete the six-validator transparent/private and Ethereum lifecycle.
- [ ] Publish a redacted evidence bundle containing no credentials or private
  keys.

Project-controlled machines are sufficient for controlled-testnet engineering.
Reproduction by an unaffiliated operator is a later public-credibility gate,
not a blocker to completing the code and controlled qualification.

## 7. Mandatory adversarial coverage

Every public adapter and the aggregate guest must reject:

- wrong PFTL genesis or chain domain;
- wrong NAVCoin, profile, program vkey, policy, manifest, or valuation unit;
- stale, future, inverted, or overlong observation intervals;
- wrong reserve owner, account, token, pool, contract, or verifier identity;
- duplicate sources, positions, accounts, or non-canonical source order;
- malformed encodings, lengths, offsets, Merkle paths, signatures, receipts,
  headers, events, and proofs;
- quantity/valuation evidence substitution;
- attested evidence mislabeled as cryptographic;
- missing or understated liabilities;
- decimal, denomination, price-scale, and rounding confusion;
- arithmetic overflow or underflow;
- proof, packet, epoch, observation, and challenge replay;
- source, proof, and public-value payloads above governed bounds; and
- parser inputs that could panic, allocate without bounds, or perform
  unbounded work.

Source-specific minimums:

- **Aave:** wrong pool/reserve/state root, omitted debt, duplicated collateral,
  invalid decimal or price, and collateral/debt overflow.
- **EVM spot:** wrong chain/token/owner/storage slot/state root, duplicate
  balance inclusion, and insufficient checkpoint quorum.
- **Hyperliquid:** wrong reader/account/topic/commitment, invalid receipt
  inclusion, duplicate positions, hold greater than total, negative account
  value, withdrawable greater than account value, invalid price, and overflow.
- **NEAR:** wrong account/pool/reader or code hash, invalid outcome/block proof,
  stale or mismatched head, malformed event, and substituted ownership.
- **Solana:** wrong owner/stake/vote account or slot, invalid activation state,
  duplicate stake account, malformed RPC artifact, and stale observation.
- **XMR:** wrong address/challenge/proof, stale or replayed proof, malformed
  proof, and quantity/valuation substitution.
- **pfUSDC overlay:** wrong source root/value, zero overlay, overflow, stale
  PFTL state, double counting, and mismatch with packet verified assets.

## 8. Acceptance gates

| Gate | Pass condition | Current state |
|---|---|---|
| `G0` Wallet/runtime boundary | Shipped wallet, proxy, node, signer, and relays require no StakeHub code, API, path, token, or agent. | PASS |
| `G1` Public proof standard | Versioned bounded ABI and immutable profiles bind the complete proof context and trust classes. | PASS |
| `G2` Generic proof framework | Clean checkout reproduces generic guest/vkey and CPU execute/prove/verify/packet flow. | PASS |
| `G3` Public A666 adapters | Aave, EVM spot, Hyperliquid, NEAR, Solana, and XMR have public collectors, source-state/ownership/quantity/liability verifiers, valuation verifiers, bounded parsers, adversarial tests, and fuzz qualification with no aggregate-operator-attestation shortcut. | **FAIL/OPEN — 0/6 production-qualified** |
| `G4` Source-equivalent A666 proof | Fresh public A666 proofs reproduce source results and NAV without StakeHub and without unapproved trust downgrades. | **FAIL/OPEN** |
| `G5` Controlled migration | Six validators complete activation, transparent/private issue/redeem, export/return, restart, replay, conservation, pause, and rollback. | OPEN |
| `G6` Live migration | Existing A666 route is governed to the public successor and passes all live verification. | OPEN |
| `G7` Clean public reproduction | Tagged public checkout reproduces the complete lifecycle without internal filesystem or code access. | OPEN |
| `G8` Accurate UX | Wallet/RPC exposes freshness and quantity/valuation trust classes without provider-brand inference. | PARTIAL; requalify against successor |

StakeHub is deprecated only when `G0` through `G7` pass. Passing `G0`, `G1`,
or `G2` alone is not deprecation.

## 9. Monday demonstration boundary

The frozen Monday demonstration may use the existing live A666 historical
profile if the objective is to demonstrate the current USDC → pfUSDC → A666 →
wrapped Ethereum asset flow. That does not change reserve custody and does not
activate the rejected attestation shadow.

It must not be described as:

- a demonstration that StakeHub is deprecated;
- a demonstration that the public repository verifies all A666 reserves; or
- a demonstration of the public A666 successor profile.

Attempting a live proof-profile migration before `G3`–`G5` pass is prohibited.

## 10. Exact continuation state

Repository and branch:

```text
/home/postfiat/repos/a666-eth-fast-lane-combined-20260724
feature/pnok-private-fix
pushed baseline before the current valuation continuation:
3183350 Pin Solana reader artifact identity
origin/feature/pnok-private-fix matched that baseline when this continuation began
```

Frozen demonstration checkout — do not modify:

```text
/home/postfiat/tmp/a666-pfusdc-monday-demo-2246d257
```

The public implementation completed and pushed so far is:

```text
af9ae4e Harden public reserve overlay protocol boundary
14f3697 Make public reserve verification plan canonical
c763e6a Keep adapter readiness outside shipped proof kit
36e0a5f Add complete public EVM spot quantity verifier
03dde8c Add public Solana stake attestation verifier
dc501db Add public Monero reserve verifier
303d071 Keep migration fixture outside public proof kit
36fbb18 Add public source checkpoint assembly
6ac6897 Add public EVM spot collection workflow
2e56111 Add public Aave collection workflow
3d3915b Publish HyperCore reserve reader
48af691 Clarify public NAV reserve proof completion plan
92e9c61 Fix legacy guest source archive in CI
397ab2b Implement public Hyperliquid reserve collection
2f87084 Implement public NEAR reserve collection
e9c2e5d Implement public Monero reserve collection
89a4b76 Implement public Solana reserve reader
ce9c6e0 Make Solana reader SBF build reproducible
3183350 Pin Solana reader artifact identity
aef4ce8 Prove reserve valuations from public chain state
dfce1ff Enforce one canonical StakeHub deprecation plan
4024dea Fuzz public reserve proof inputs
b4447c8 Fuzz every public reserve proof parser
```

This work implements the pfUSDC overlay, provider-neutral source checkpoint
assembly, all six initial verifier modules, complete public Aave and EVM-spot
collection workflows, public HyperCore and NEAR reader contracts, the public
Monero collector, and the public Solana reader/collector with a pinned
reproducible SBF identity. This continuation also implements the public
Chainlink account/storage-proof valuation successor described in section 3.4.
It is not remotely qualified until the commit containing it has green exact-tip
CI. None of this completes a source's production qualification or changes live
A666.

Current machine gates:

```text
scripts/test-proof-public-input-inventory
  passed after the current valuation continuation; 5 systems, 70 public fields,
  50 source hashes

scripts/check-nav-reserve-proof-fuzz-smoke
  passed locally against guarded temporary corpora. In addition to the initial
  retained witness/evidence campaigns, a clean 2026-08-02 run exercised all
  nine then-current boundaries for 4,010,896 executions with no crash,
  timeout, or OOM. The typed manifest builder added a tenth bounded input
  target; its first complete local smoke run brought that ten-target run to
  3,265,852 executions with no crash, timeout, or OOM. Exact-tip CI must
  reproduce the pinned smoke campaigns. Longer retained
  per-source campaigns and regression-corpus review remain open and are part of
  production qualification.

scripts/check-a666-public-adapter-readiness
  passed; qualified=0/6, stakehub_deprecated=false

scripts/check-provider-neutral-wallet-boundary
  passed

scripts/check-a666-public-reader-candidates
  passed locally; the hardened HyperEVM runtime, public NEAR Wasm, and
  reproducible Solana SBF identities match their pinned candidate artifacts.
  The NEAR release build pins Rust 1.95.0 and remaps checkout, toolchain/cache,
  and optional local `rust-src` paths to the compiler's canonical source
  prefix. Independent archive checkouts and the clean GitHub runner reproduced
  the same 147398-byte artifact, SHA-256
  `013fec10bba9cd623af8800c465f702f50e5950cc13443799c7a8661940dd01d`,
  and NEAR code hash `5swZhNNqpD6HsqFXhNjRUiSYoXtnkWPipiW8hRbrkbN`.
  This is a build-identity gate, not a deployment qualification. Public chain
  checks on 2026-08-02 prove that the historical HyperEVM reader runtime
  (`0x7e4007...74f8`) differs from the public candidate (`0xc252f3...0db3`),
  the historical NEAR reader code (`4mdew...wUhx`) differs from the public
  candidate (`5swZ...kbN`), and the Solana candidate remains undeployed.

public Solidity suite after HyperCore reader addition
  143 passed, 0 failed

proof-kit verifier/CLI suites and strict clippy after each adapter change
  passed locally; rerun after every continuation change
```

The legacy archive/rebuild CI defect is fixed. Exact-tip runs `30732980432`,
`30733515641`, and `30733599841` are fully green through `dfce1ff`, including
the deterministic Solana reader and immutable legacy guest rebuilds. Run
`30749886246` correctly rejected `b4447c8`: its fuzz runner referenced one
local untracked Solana seed that did not exist in a clean checkout. The seed
did not exercise the new reader parser and that dependency is removed.
Exact-tip run `30750345429` then reached the SP1 installation step but GitHub
rate-limited the installer's unauthenticated release lookup with HTTP 403;
none of the proof-kit tests ran. The workflow now supplies the scoped
read-only workflow token to the pinned SP1 installer and must pass on a new
exact-tip run before this continuation is remotely qualified. A green older
run is not permission to mark a newer unverified tip green or to overwrite the
immutable legacy ELF or vkey.

```text
https://github.com/postfiatorg/postfiatl1v2/actions/runs/30732561619
https://github.com/postfiatorg/postfiatl1v2/actions/runs/30732980432
```

Do not close Phase 1 until exact-tip CI is green. The worktree contains many
unrelated untracked deployment and evidence paths. Never use `git add .`, bulk
clean, or delete untracked evidence. Stage only explicitly reviewed files.

The current program identity is an immutable legacy reference reproduced from
public source commit `bfe0ded03033085ab1db9df274f93cc41d0d2690`:

```text
ELF SHA-256: 0f8476431677bfe0a8f9f19db7439abce1a879ba5736cfa3225ae7de4e5b0e52
program vkey:
0x000c7271e0711abce0c61d293222fd4a144599a779db8cadadc4df35e31a4100
```

Do not mutate that identity to absorb successor adapter work. After every
source collector, verifier, valuation path, and policy is qualified, build a
distinct reproducible successor guest and register its distinct identity.

Two genuine historical observation epochs are available for later
source-by-source reconciliation:

```text
docs/evidence/a666-variable-size-nav-roundtrip-20260728/stakehub-nav-mark/stable-policy-preview/aggregate-witness-report.json
docs/evidence/a666-variable-size-nav-roundtrip-20260728/stakehub-nav-mark/nav-epoch-2/live-nav-mark-manifest.json

docs/evidence/a666-pfusdc-reserve-demo-20260730/live-run-01/por-preissue/aggregate-witness-report.json
docs/evidence/a666-pfusdc-reserve-demo-20260730/live-run-01/por-preissue/nav-epoch-3/live-nav-mark-manifest.json
```

Historical totals:

| Epoch | External proof net assets | pfUSDC overlay | Total net assets | Supply | NAV/unit |
|---|---:|---:|---:|---:|---:|
| 2026-07-28 / epoch 2 | 2,825,975,143,580 | 20,400,000,000 | 2,846,375,143,580 | 31,590,197,455 | 90,103,113 |
| 2026-07-30 / epoch 3 | 2,826,373,076,806 | 11,299,585,500 | 2,837,672,662,306 | 31,489,197,455 | 90,115,750 |

Historical epoch 4 reused the epoch-3 external proof with a changed pfUSDC
overlay. It is not a third independent source-observation epoch.

Existing attestor, issuer, and reserve-operator keys remain outside the public
repository. No attestor key may be used to sign the six aggregate source
amounts as a shortcut around public validation. Never copy private-key
contents or passphrases into the repository, logs, evidence, or temporary
paths.

## 11. Current continuation actions

Start read-only:

```bash
cd /home/postfiat/repos/a666-eth-fast-lane-combined-20260724
git status --short --branch
git rev-parse HEAD
git diff --check
git diff -- \
  docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md
```

Then:

1. Require green exact-tip CI while preserving the pinned legacy ELF and vkey.
   The legacy archive/rebuild defect is fixed; do not rewrite that identity.
2. Qualify Hyperliquid: deploy the hardened public reader, publish governed
   A666 policy and committee inputs, collect fresh externally signed snapshots,
   retain longer parser-fuzz campaigns and add adversarial network fixtures,
   reconcile at least two
   fresh epochs, and independently reproduce the workflow. The public snapshot,
   checkpoint, owner-authorization, and receipt-proof collection code is now
   implemented; it is not yet production-qualified.
3. Qualify NEAR: deploy the implemented reader, publish governed A666
   policy/committee and public valuation inputs, retain longer fuzz campaigns,
   reproduce fresh epochs independently, and reconcile the full profile.
4. Qualify Solana: immutably deploy the exact reproducibly built reader,
   publish its ProgramData hash and governed A666
   policy/committee/SOL-USD valuation inputs, retain longer fuzz campaigns for the
   transaction/payload/program-state parsers, collect fresh epochs, reconcile,
   and reproduce independently. The pinned SBF build identity is complete and
   CI-rebuilt; the old signed-RPC adapter remains ineligible.
5. Qualify Monero: create a fresh context-bound nonzero wallet proof, assemble
   an independently reproduced checkpoint and spent-status certificate, bind
   public XMR/USD valuation evidence, retain longer fuzz campaigns, collect
   fresh epochs, and reconcile the full profile.
6. Finish public valuation for every source. Bind governed price origin,
   signature or chain proof, scale, timestamp, freshness, asset identity,
   haircut, and rounding. Do not accept operator-entered aggregate USD values.
7. Adversarially harden the existing Aave and complete-EVM-spot collectors and
   every new collector. Add bounded parsers, property tests, fuzz targets,
   malformed-proof tests, omission tests, and deterministic replay vectors.
8. Create the public A666 source manifest and per-source quantity/valuation
   policies, owners, committees, source domains, freshness rules, haircuts,
   bounds, and pfUSDC overlay binding. No item may refer to a private path,
   provider-specific domain, compiled operator identity, or StakeHub API.
9. Reproduce both historical epochs source by source, then collect at least two
   fresh epochs entirely through the public pipeline. Reconcile every asset,
   liability, trust classification, price, overlay value, total, supply, and
   NAV; explain every difference.
10. Build one distinct successor guest only after the preceding inputs are
    stable. Reproduce its ELF and vkey from clean independent checkouts and
    publish the immutable profile ID, manifest, policies, and test vectors.
11. Run the controlled six-validator migration, full transparent/private
    issue/redeem and Ethereum export/return lifecycle, restart/outage/replay/
    rollback tests, and conservation checks.
12. Only after `G0` through `G5` are green, govern the existing A666 asset to
    the public successor, verify the live lifecycle, complete clean-checkout
    reproduction, and set `stakehub_deprecated=true`.

Throughout all steps, keep the live A666 route and frozen Monday checkout
untouched until the controlled migration gate authorizes the exact governed
change.

## 12. Definition of done

This work is complete only when a clean public checkout can independently
collect or consume, validate, aggregate, prove, and submit a fresh A666 NAV
packet covering Aave, EVM spot, Hyperliquid, staked NEAR, staked Solana, XMR,
and the finalized pfUSDC overlay; PFTL deterministically verifies it; the live
A666 route uses the governed public successor; transparent/private
issue/redeem and Ethereum export/return pass; and no executable step requires
StakeHub.

Until then, the accurate status is:

> The wallet and generic infrastructure are decoupled, but A666 reserve
> publication is not. StakeHub is not deprecated.
