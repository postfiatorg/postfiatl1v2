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

### 1.1 Custody is allowed; private proof meaning is not

StakeHub is installed on the operator machine, its agent is unlocked, and it
holds the reserve accounts used by A666. That is not itself the architecture
defect. A custody tool may unlock a key and sign an exact transaction or
domain-separated reserve-owner statement constructed by public code. It may
also submit a public-reader invocation or pay deployment gas and rent.

What is forbidden is treating StakeHub as the authority that decides what the
portfolio owns, what a source balance means, what price applies, or what NAV
is. Those facts must be collected, validated, valued, and aggregated by public
PostFiat code. For BFT-checkpointed sources, five of the six PFTL validators
must independently reproduce the source checkpoint and sign with their own
local validator keys. The proof kit then assembles and verifies that public
certificate. An old StakeHub aggregate NAV or balance attestation is never a
substitute.

In this document, an "external signer" means a key outside the public proof
process, not a person or machine unavailable to the operator. On this host,
StakeHub supplies the reserve-owner and deployment signatures without
exporting its keys. The PFTL validator hosts supply checkpoint votes without
exporting their keys. A clean public checkout must still be able to construct
every exact statement, verify every signature, and reproduce every asserted
quantity and valuation.

### 1.2 Plain-English end state

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
| Aave on Arbitrum | Public collector and verifier bind checkpoint, owner, positions, token mapping slots, reserve indexes, rates, code, prices, liabilities, and conservative valuation. Two fresh governed epochs reproduce nonzero collateral and debt with distinct cryptographic commitments; retained fuzz passed. | No | Verify both aggregate Groth16 proofs, close exact controlled migration/lifecycle evidence, and record the final per-source qualification decision. |
| Complete EVM spot set | Public multichain account/storage proof collector and quantity verifier plus policy-pinned Chainlink state-proof valuation. Two fresh governed epochs reproduce the complete position set and prices with distinct cryptographic commitments; retained fuzz passed. | No | Verify both aggregate Groth16 proofs, close exact controlled migration/lifecycle evidence, and record the final per-source qualification decision. |
| Hyperliquid | Public reader contract, unsigned snapshot construction, BFT checkpoint/owner workflow, receipt-proof collector, receipt-trie verifier, and exact reader identity. Two fresh governed epochs produce distinct cryptographic receipt commitments; retained fuzz passed. | No | Verify both aggregate Groth16 proofs, close exact controlled migration/lifecycle evidence, and record the final per-source qualification decision. |
| Staked NEAR | Public stateless reader, exact deployed Wasm identity, finalized invocation, checkpoint/owner workflow, outcome/block proof verification, and Chainlink state-proof valuation. Two fresh governed epochs produce distinct cryptographic quantity/valuation commitments; retained fuzz passed. | No | Verify both aggregate Groth16 proofs, close exact controlled migration/lifecycle evidence, and record the final per-source qualification decision. |
| Staked Solana | Public stateless reader, reproducible immutable SBF identity, finalized transaction/block collector, program identity and authority checks, BFT checkpoint, and Chainlink state-proof valuation. Two fresh governed epochs produce cryptographic commitments; retained fuzz passed. The historical signed-RPC adapter remains separately labeled attested and is not used by the successor. | No | Verify both aggregate Groth16 proofs, close exact controlled migration/lifecycle evidence, and record the final per-source qualification decision. |
| Monero | Public ReserveProofV2, transaction/RingCT/ownership/inclusion/header and key-image-status verification plus Chainlink state-proof valuation. Two fresh governed nonzero epochs produce distinct cryptographic quantity/valuation commitments; retained fuzz passed. | No | Verify both aggregate Groth16 proofs, close exact controlled migration/lifecycle evidence, and record the final per-source qualification decision. |
| pfUSDC overlay | Provider-neutral finalized PFTL route/vault/supply derivation and version-2 packet binding are implemented. Exact NAV, supply, precision, overlay, source root, valuation root, and packet hash tampering fail closed. | Not sufficient by itself | Use fresh six-validator snapshots after the base proof completes, then exercise the exact controlled and live migration gates. |

The public successor no longer lacks collectors, governed A666 inputs, fresh
epochs, valuation proofs, or retained fuzz evidence. Those prerequisites are
published in
`tools/nav-reserve-proof/qualifications/a666-public-successor-20260802/`.
Every source has two independently collected bounded inputs and two distinct
quantity/valuation commitments. Both complete witnesses execute under the
immutable successor guest and report six cryptographic quantity claims, six
cryptographic valuation claims, zero attested value, and zero controlled
value. A clean public checkout reproduced the witnesses and public values.

The remaining `0/6` result is deliberate: source implementation evidence is
not the same as production activation. The two CPU Groth16 proofs, exact
nonzero-supply six-validator migration, overlay-aware packet rehearsal,
complete transparent/private/export/return lifecycle, rollback, and final
clean public reproduction must still pass. Until those gates close, none of
the six rows may be marked production-qualified and StakeHub is not
deprecated.

### 3.5 First complete live public shadow epoch

On 2026-08-02 the public pipeline completed its first six-source,
context-bound A666 shadow epoch. This is the first end-to-end evidence that the
public implementation can verify the real portfolio without accepting a
StakeHub aggregate balance or NAV attestation. StakeHub was used only to sign
the exact public reserve-owner challenges with keys already in its custody.
All source checkpoint statements were independently reproduced by the six
PFTL validators; every assembled certificate contains six valid votes against
a quorum of five.

The qualification-only epoch-6 witness binds PFTL observation window
`776..784`, source-manifest hash
`8abe3e59198b72945d4778a7fa91e5af157a6c65032d8940cca486850ffe59fcb567268ca5942669ff6977ef32dd3a41`,
and source-observation root
`4aabb014d0fef575ffb65feca9f74aaa9938ea200ac6b811c0bd05a776bd8bee81d96e5f833a723602370f04234e1e7f`.
Native execution produced:

| Source | Gross value (USD scale 1e8) | Liabilities |
|---|---:|---:|
| Aave on Arbitrum | 56,041,873,124 | 20,095,433,833 |
| Complete EVM spot | 63,509,977,968 | 0 |
| Hyperliquid | 1,802,523,722,983 | 0 |
| Staked NEAR | 827,968,163,344 | 0 |
| Staked Solana | 105,346,569,694 | 0 |
| Monero (`0.15419024 XMR`) | 5,602,533,208 | 0 |
| **Total** | **2,860,992,840,321** | **20,095,433,833** |

Verified net assets are `2,840,897,406,488`, or `$28,408.97406488`.
All six quantity claims and all six valuation claims are cryptographic;
attested and controlled value are both zero. The canonical CBOR witness is
1,338,874 bytes with SHA-256
`b9bb2c155fa1654c73f2e4013bf77060e8fa1d6ca0059e2cf38f38c2e0007447`.
The 584-byte public values have SHA-256
`d02a243f6cf684843ffb7cdf458c0dc41daa9799b0b9d966cbb71875e22953f6`.

This was a qualification milestone, not production qualification or live
activation. At that checkpoint the second fresh epoch and retained fuzz were
still open; Section 3.6 records their subsequent completion. Aggregate proof
verification, complete NAV/supply/overlay reconciliation, and the controlled
migration gates remain open.

### 3.6 Fresh public epochs 7 and 8 and immutable successor identity

Two additional fresh six-source observations were subsequently collected
through the same public adapters. Their complete bounded source inputs,
contexts, result pins, and reproduction procedure are published at:

```text
tools/nav-reserve-proof/qualifications/a666-public-successor-20260802/
```

Both epochs bind the exact existing A666 asset, PFTL genesis, public source
manifest, valuation policy, and immutable successor proof profile. Each has
six cryptographic quantity claims, six cryptographic valuation claims, zero
attested value, and zero controlled value. No aggregate amount, liability,
valuation, or NAV attestation is present.

| Result (USD e8 atoms) | Epoch 7 | Epoch 8 |
|---|---:|---:|
| PFTL observation window | 776–784 | 776–784 |
| Gross assets | 2,855,886,091,629 | 2,859,789,254,961 |
| Liabilities | 20,094,872,960 | 20,094,965,843 |
| Verified net assets | 2,835,791,218,669 | 2,839,694,289,118 |
| Canonical witness bytes | 1,340,409 | 1,339,971 |
| Witness SHA-256 | `8f4538b111a6cebf97a71406895af40d4bc4c6ed369c3ecd540b6050765b6dd6` | `4bb36f68ebee3537afe62f6d9ed61d68513a96c747fffc3622024b0a2f9efe89` |
| Public-values SHA-256 | `a215726624267dc5c5a60ac2829b24a149855a3edcfc798c965826e17bca7e68` | `1bc443108e0f2b78d92037d986378cd6df51bd3fc069e64594a521f83a36b9dd` |

The successor is a new immutable proof profile for the same NAVCoin, not a
mutation of the legacy guest and not a new asset:

```text
source commit: 5b8f0317375af6fb46d586d9d9152b511457b802
ELF SHA-256: 2b41e4e8095b1dacdc519b2f0a2b4831ebc57cc8003a4d3686f6d9e4687e81df
SP1 vkey: 0x00f3857f96ef97e00bd15b4030acd8d6b0a72740b28c6160d154bc2c9bb141bf
profile ID: f8784629ff7338002d836c1988b8e2c0f19caf448429e0eb7fdc39fa2b08f7d9a44171fc1e7239bc25e06ad833c14e91
```

Two isolated Docker builds from the pinned public source commit produced the
same ELF and vkey. A clean public checkout reassembled both witnesses
byte-for-byte. It also executed epoch 8 under the exact successor ELF and
reproduced the 584-byte public values byte-for-byte. CI now enforces the same
manifest/profile/ELF bindings and regenerates both published qualification
epochs.

The fresh multi-epoch source collection requirement is therefore satisfied at
the artifact level. The retained ten-target fuzz campaign has also completed
successfully. Production qualification remains open until Groth16 proof
verification, exact six-validator migration rehearsal, NAV/supply/overlay
reconciliation, and controlled lifecycle gates record passing evidence. No
live profile, route, reserve packet, balance, or validator state was changed
by this qualification work.

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
certified receipts root. The hardened public runtime was deployed to HyperEVM
mainnet at `0xddb4ed1edf1f0d81f7531cddb27810080601a2cb` in transaction
`0xcad045dbe7edcdbccbcdebae357525fd0bb5fe86e53f3b5a72417a92c5e37237`.
Its 5,729-byte runtime has the pinned Keccak-256
`c252f32acd9fdcfe2b4f9b1d70c3de17acf83649a6313fc3ab9155bca1010db3`.
The governed A666 policy/committee inputs are published, two fresh epochs
carry distinct receipt commitments, and the retained fuzz campaign passed.
Aggregate Groth16 verification and the controlled migration/lifecycle gates
remain before production qualification.

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

The exact public Wasm is deployed on NEAR mainnet at
`eed15bedebb4ac46d1528187a8c2f00aa59b441398d3e346c44eb2dcb2fc1d9a`,
with code hash `5swZhNNqpD6HsqFXhNjRUiSYoXtnkWPipiW8hRbrkbN`. Transaction
`BVEXnwEuYmZKrc36VJnnjPHFAev8QvuDuS5994mXpEmJ` successfully executed the
first snapshot and finalized. A typed candidate Arbitrum NEAR/USD Chainlink
valuation policy now pins the official
registry feed, live aggregator, exact code hashes and OCR2 storage slots,
committee, valuation context, decimals, and haircut. Its verifier commitment is
`260ce714ab04e1f1d48676a0067b9a58894d4dc5673bd0f5356491ab90f6c2703071348906f362e17eb8f735dcb63015`.
The deployment identity now exists and is publicly pinned. Governed quantity
inputs and NEAR/USD proofs are published in two fresh governed epochs with
distinct cryptographic commitments, and the retained fuzz campaign passed.
Aggregate Groth16 verification and the controlled migration/lifecycle gates
remain. Therefore this implementation is not yet production-qualified.

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

The public implementation and immutable deployment are complete. A
`solana-verify` 0.5.1 build under the exact
published Docker image digest was repeated and produced the same executable
program hash, raw ELF hash, and byte length. That identity is machine checked
by `scripts/check-solana-stake-reader-identity`. A typed candidate Ethereum
SOL/USD Chainlink valuation policy now pins the official registry feed, live
aggregator, exact code hashes and OCR2 storage slots, committee, valuation
context, decimals, and haircut. Its verifier commitment is
`1ae3bf34e5433836b81710c6c5d41b0ec46c469c15d8a21e6ac735893676104fe5465af51441980dfee00ce01e274756`.
The exact 33,120-byte artifact is now immutably deployed on Solana mainnet-beta
as program `Gp2oTn6VjFF22n98H6YSH4uVvQxWFHNCL7pp1tcAPF36`, with ProgramData
account `9xVv6Q8Z1AJsK4aWKydhYyEGeA7Ai8k6t3gpreR7QBh8` and no upgrade authority.
The on-chain bytes match raw ELF SHA-256
`af70e82df3f1d519da5c5c7ddb62ab594d7babdd95dbc67c1692c2d6cea96716`.
Governed quantity inputs and SOL/USD proofs are published in two fresh epochs
with cryptographic commitments, the clean checkout reproduced them, and the
retained fuzz campaign passed. Aggregate Groth16 verification and the
controlled migration/lifecycle gates remain. Therefore staked Solana remains
`0/1` production-qualified and does not yet make `G3` pass.

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

The public parser has now verified the live reserve's `0.15419024 XMR` as
unspent. The earlier zero result was false: the temporary wallet scanner
defaulted to a recent 4,320-block window even though the unspent reserve output
was created at height `3694232`. The scanner now defaults to a genesis-safe
restore height, while an explicitly supplied bounded restore height remains
supported. The policy permits a pinned historical output block only while the
validators independently reproduce and certify current key-image spent status.
For the first complete shadow epoch, all six validators reproduced the Monero
checkpoint, the Optimism XMR/USD Chainlink state proof valued XMR at
`$363.352`, and the resulting `$56.02533208` source observation passed the
public aggregate verifier. No aggregate amount attestation was used.

The adapter now has a second fresh nonzero epoch, distinct quantity and
valuation commitments, retained fuzz evidence, and clean-checkout
reproduction. Aggregate Groth16 verification and the controlled
migration/lifecycle gates remain before production qualification.

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
- [x] Require green exact-tip remote CI. Product-security run
  `30753373194` passed at implementation tip
  `45801200cb0f63b6752373b5db397c10ac87b4a3`, including the ten-target
  attacker-input fuzz campaign, deterministic public reader rebuilds, pinned
  SP1 host compile, and immutable legacy guest identity rebuild. This closes
  the Phase 1 implementation gate; it does not qualify any live source or make
  `G3` pass.

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
- [x] Publish governed A666 Solana policy/committee/valuation inputs, retain
  the production fuzz campaign, reproduce two fresh epochs, and reconcile the
  complete A666 profile. The exact reader build and immutable mainnet-beta
  deployment are also complete.
- [ ] Record the final independent Solana production-qualification decision
  after both aggregate proofs and the controlled lifecycle gates pass.
- [x] Implement and register the public XMR reserve-proof quantity adapter.
- [x] Implement the public Monero reserve-proof, header-chain, transaction
  inclusion, ownership, and key-image spent-status collector for zero and
  nonzero reserves.
- [x] Implement and register a public Chainlink account/storage-proof
  valuation adapter and collector that reruns the registered EVM-spot, NEAR,
  Solana-reader, or Monero quantity verifier and derives the exact valued
  amount and governed haircut without an aggregate operator attestation.
- [x] Remove all provider-specific hash domains and compiled operator identities
  from successor semantics. CI rejects the internal provider name anywhere in
  the public proof-kit tree. The successor commits typed governed owners,
  committees, reader identities, source domains, and policy fields supplied by
  public manifests; it does not compile a private service identity or opaque
  provider hash into verification. The retained Aave state proof used for
  policy reproduction is published under a neutral, integrity-inventoried
  fixture path.
- [x] Add public fixtures and adversarial tests for every adapter. Aave and EVM
  spot reconstruct retained account/storage proofs; Hyperliquid reconstructs a
  retained header, receipt trie, and payload; NEAR reconstructs retained
  outcome/block Merkle evidence; Solana reconstructs retained stake state and
  separately tests the successor reader transaction/program identity; Monero
  reconstructs a retained nonzero reserve proof and context-bound zero vector.
  Per-adapter tests reject policy, owner, position, omission, duplicate,
  freshness, proof, checkpoint, and value substitutions. The retained
  ten-target campaign and two fresh production-shaped epochs subsequently
  passed as recorded in Sections 3.6 and 10.2.
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
  coverage; the retained campaign below subsequently closed the longer fuzz
  prerequisite.
- [x] Prove malformed or unsupported adapter evidence fails closed without
  panic or unbounded work. The ten-target retained campaign completed
  `326,405,841` executions with no crash, timeout, or OOM.

### Phase 3 — build the source-equivalent A666 profile

- [x] Create a public A666 source manifest selecting the real adapter for each
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
  The governed A666 input bundle and exact public reader identities are now
  published in the successor qualification fixture.
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
  Two fresh committee-certified proof sets now use these policies and reconcile
  under the successor guest. Final independent production qualification remains
  open pending aggregate proof and controlled lifecycle gates.
- [x] Publish typed Monero quantity and Optimism XMR/USD valuation candidate
  policies. The quantity policy derives the spend and view keys from the public
  mainnet reserve address and matches the tracked real nonzero proof fixture. Its
  verifier commitment is
  `cb82196be2ff0dbfa3c6926bc92329e763dbaf00c4ec60c562ffab06ef7cae103c3f1dc21fc275f2434999fde1eee004`.
  The valuation policy pins the official Chainlink registry proxy, its live
  phase aggregator, exact proxy/aggregator code hashes, OCR2 storage slots,
  portfolio policy, committee, decimals, and haircut. Its verifier commitment
  is
  `7451679e24a92e5839545f42b51619acaac98ecb304e1d5826eb90db1de0e5e1aea4490974550bae5880a1fd945e34c6`.
  The public provenance record pins the official registry commit and the
  independently queried Optimism block/state root. Two fresh context-bound
  nonzero quantity/price proofs now reconcile under the successor guest. Final
  production qualification remains open pending aggregate proof and controlled
  lifecycle gates.
- [x] Publish typed NEAR/USD and SOL/USD Chainlink valuation candidates. The
  NEAR policy uses the active Arbitrum feed because the registry-listed
  Ethereum NEAR/USD proxy is decommissioned; the Solana policy uses the active
  Ethereum SOL/USD feed. Both pin the official registry commit, observed
  block/state root, live aggregator, exact code hashes, OCR2 storage slots,
  candidate committee, successor valuation context, decimals, and haircut.
  Their verifier commitments are
  `260ce714ab04e1f1d48676a0067b9a58894d4dc5673bd0f5356491ab90f6c2703071348906f362e17eb8f735dcb63015`
  and
  `1ae3bf34e5433836b81710c6c5d41b0ec46c469c15d8a21e6ac735893676104fe5465af51441980dfee00ce01e274756`.
  The corresponding public NEAR and Solana quantity policies are published
  against the exact deployed reader identities. Two fresh committee-certified
  quantity and price proof sets now reconcile under the successor guest. Final
  production qualification remains open pending aggregate proof and controlled
  lifecycle gates.
- [x] Derive and publish the candidate source-checkpoint committee from the
  public six-validator PFTL registry. The generic builder accepts only
  ML-DSA-65 public keys, canonicalizes validator ordering, rejects quorums
  below the BFT threshold, and derives its root. The manifest builder repeats
  the BFT-threshold check. A666's candidate is epoch 1, quorum 5,
  committee root
  `99b0a3b8af49f3c91537d24a698e47f5761eec65890d00cdc4070ec99b18b333dfe514199c258cda5b81bec33821e245`.
  It contains no validator private keys.
- [x] Preserve or strengthen historical cryptographic trust classifications.
- [x] Eliminate operator-signed aggregate quantity, liability, and NAV inputs;
  permit signed external data only under the exact restrictions in section 2.
- [x] Implement and bind public valuation evidence for every non-pfUSDC source;
  a quantity proof without its governed price does not establish NAV.
- [x] Bind reserve owners, verifier keys/committees, source domains, position
  identities, freshness policies, haircuts, and valuation policy.
- [x] Rebuild the canonical SP1 guest in the pinned Docker toolchain.
- [x] Reproduce the ELF hash and vkey from independent checkout paths.
- [x] Register nothing live; first derive the immutable candidate profile ID
  and publish its complete manifest and program identity for review.

### Phase 4 — source-by-source qualification

- [ ] Reconstruct at least two historical A666 epochs with the public adapters
  where source artifacts remain available.
- [x] Compare each public adapter output against the historical source result,
  not merely the aggregate NAV.
- [ ] Explain and govern every conservative difference.
- [x] Reject any unexplained trust downgrade.
- [x] Run at least two fresh A666 shadow epochs from newly collected source
  artifacts without StakeHub.
- [ ] Verify gross assets, liabilities, net assets, trust buckets, supply,
  pfUSDC overlay, and NAV per unit.
- [ ] Produce and independently verify CPU Groth16 proofs.
- [x] Implement a versioned fail-closed packet-preparation path that derives
  exact floor NAV, supply, overlay roots, valuation roots, and packet identity
  from canonical proof and finalized PFTL inputs.
- [ ] Construct overlay-aware reserve packets with the public CLI.
- [x] Prove wrong source, owner, profile, vkey, policy, manifest, epoch,
  interval, valuation, overlay, and proof substitutions fail.
- [x] Preserve the old profile and packets as immutable historical records.

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

`G4` remains fail/open despite two reproducible fresh six-source shadow
epochs: finalized overlay/supply reconciliation, independently verified
Groth16 proofs, exact packet construction, and controlled migration evidence
are still required.

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

Repository and pushed implementation tip at this update:

```text
repository: /home/postfiat/repos/a666-eth-fast-lane-combined-20260724
branch: feature/pnok-private-fix
tip: 843554ba23bf8dc11c4d42e7547c6241d4e6da89
remote: origin/feature/pnok-private-fix
```

Frozen demonstration checkout — do not modify:

```text
/home/postfiat/tmp/a666-pfusdc-monday-demo-2246d257
```

The latest pushed qualification and migration commits are:

```text
c53e872 Refresh reserve proof source inventory
d5e4476 Rehearse exact A666 public proof migration
4a450e2 Publish fresh A666 public qualification epochs
0c942ae Derive A666 public NAV packets fail closed
92f17ff Test A666 successor against live supply shape
843554b Exercise reserve packet tamper boundaries
```

The worktree has no tracked modification at this checkpoint. It still contains
large pre-existing untracked deployment, evidence, and scratch trees. Never
use `git add .`, bulk clean, reset, or delete those paths. Stage only named,
reviewed files.

### 10.1 Exact public successor and fresh epochs

The public successor qualification bundle is committed at:

```text
tools/nav-reserve-proof/qualifications/a666-public-successor-20260802/
```

It contains the complete bounded inputs for fresh epochs 7 and 8. A clean
public checkout rebuilt the successor ELF, reproduced both witnesses and both
584-byte public-values blobs byte-for-byte, and executed both witnesses under
the exact successor program. The immutable identity is:

```text
A666 asset:
521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c

successor profile:
f8784629ff7338002d836c1988b8e2c0f19caf448429e0eb7fdc39fa2b08f7d9a44171fc1e7239bc25e06ad833c14e91

source manifest:
8abe3e59198b72945d4778a7fa91e5af157a6c65032d8940cca486850ffe59fcb567268ca5942669ff6977ef32dd3a41

valuation policy:
350eaee0a1ca12ba51637781ba52661b8685f868657a7c5e7d07c31b2899869c

source commit:
5b8f0317375af6fb46d586d9d9152b511457b802

ELF SHA-256:
2b41e4e8095b1dacdc519b2f0a2b4831ebc57cc8003a4d3686f6d9e4687e81df

SP1 vkey:
0x00f3857f96ef97e00bd15b4030acd8d6b0a72740b28c6160d154bc2c9bb141bf
```

Epoch 7 proves net assets of `2,835,791,218,669` USD-e8 atoms. Epoch 8
proves `2,839,694,289,118`. Each proves all six quantity and all six
valuation claims as cryptographic, with zero attested and zero controlled
value.

### 10.2 Completed retained fuzz qualification

The supervised ten-target parser and construction fuzz campaign completed
successfully on 2026-08-02:

```text
service: pft-a666-public-adapter-fuzz-300s
targets: 10
duration per target: 301 seconds
total executions: 326,405,841
slowest unit: 0 seconds for every target
peak target RSS: 80 MiB
service peak memory: 249.7 MiB
crashes: 0
timeouts: 0
OOMs: 0
result: success
```

The targets cover witness/evidence input, EVM, Hyperliquid, NEAR, Solana,
Monero, source-checkpoint, manifest, and related bounded construction
boundaries. This closes the retained long-fuzz prerequisite for these exact
inputs. It does not by itself qualify source freshness, live deployment
identity, reconciliation, proof generation, or migration.

### 10.3 Overlay-aware packet and exact-NAV hardening

Commit `0c942ae` removes manual NAV and packet-identifier entry from the
successor migration path:

- `packet prepare` derives a version-2 packet template from canonical proof
  public values, actual issued supply, asset precision, and the finalized
  pfUSDC overlay;
- the CLI derives the exact conservative floor NAV and a domain-separated
  packet hash over the complete statement;
- `packet build` recomputes and rejects changes to the NAV, packet hash,
  source root, valuation root, supply, precision, or overlay;
- `scripts/a666-build-live-nav-mark-ops.py` now consumes finalized PFTL
  status, route, settlement-vault, and A666 supply snapshots and builds the
  packet plus submit/finalize operations in one fail-closed flow; and
- consensus requires nonzero-supply `sp1-nav-reserve-v1` packets to use the
  exact conservative floor NAV. Historical verifier kinds and historical
  packets are unchanged.

The CLI unit suite, strict clippy, wrapper regressions, provider-neutral
boundary gate, and the complete transparent route lifecycle regression pass.
The exact live-state overlay-aware packet is still pending the completed
Groth16 proof and fresh pre-migration fleet snapshots.

The version-2 derivation path was also exercised against the real epoch-7
public values and the last observed A666 supply. With a deliberately synthetic
overlay root and the last observed `21,032,560,900` USD-e8 overlay value, it
derived NAV `0.90413834` and a content-bound 48-byte packet hash. This proves
the arithmetic and binding path only; the synthetic root is not a migration
artifact and cannot be submitted.

### 10.4 Long proof and controlled migration status

The epoch-7 CPU Groth16 proof remains active under the supervised service:

```text
service: pft-a666-public-reserve-proof-epoch7-bounded
started: 2026-08-02 20:27:28 UTC
peak memory observed: 60,091,985,920 bytes
current state at this update: active/running
output directory:
/home/postfiat/.pft/public-reserve-qualification/20260802-epoch-2/proof
```

Do not terminate or duplicate this job. When it completes, independently
verify its proof and public values, record their hashes, run the exact
six-validator A666 successor migration/restart/snapshot test, and then start
the epoch-8 Groth16 proof sequentially. The two proof jobs must not run
concurrently because each can peak near 60 GB.

The ignored exact migration test is committed at
`crates/node/tests/atomic_swap_local_six.rs`. It binds the exact A666 asset,
genesis, successor profile, proof paths, six independent validators, finality,
restart, snapshot/import, and replay. It has compiled in release mode but has
not yet run because the epoch-7 proof is not complete. Commit `92f17ff`
removed its zero-supply bootstrap shortcut: the rehearsal now recreates the
known `31,597,197,455`-atom A666 supply and requires the proof-derived
pre-overlay NAV floor `0.89748188`.

The broader provider-neutral controlled route test passes transparent
subscribe, export, return/refund, replay, malformed input, restart-state, and
conservation behavior. That is compositional evidence only. `G5` remains
open until the exact A666 environment also completes transparent and private
issue/redeem, export/return, outage, pause, rollback, and conservation checks.

### 10.5 CI and live-state truth

Exact-tip product-security CI for `843554b` must complete green before any
release or live migration. Run `30771049323` covers that exact tip. Run
`30770789317` covers prior tip `fa017e1`; its public-tree inventory correction
passed, while its open proof-kit job was still running at the last check. An
older green run is not evidence for the current tip. The
official Ethereum fork job may accurately record that no mainnet RPC secret is
configured; that skipped real-value assertion is not source qualification.

No live mutation was made during this continuation. The existing A666 asset,
legacy profile lineage, reserve packets, route, balances, and validator state
remain unchanged. Immediately before any governed migration, reread the
profile, route, supply, reserve, pfUSDC overlay, height, state root, and
mempool independently from all six validators. The proof observation window
ends at PFTL height 784; freshness and current-height checks must pass at
submission time.

The readiness gate remains deliberately truthful:

```text
G0 PASS
G1 PASS
G2 PASS
G3 OPEN
G4 OPEN
G5 OPEN
G6 OPEN
G7 OPEN
stakehub_deprecated=false
```

The fresh artifacts and long fuzz campaign materially advance `G3` and
`G4`, but do not authorize changing the machine-readable status from
`qualified=0/6` until source-by-source deployment, freshness,
reconciliation, proof, and independent reproduction evidence is reviewed.

## 11. Current continuation actions

Execute in this order:

1. Preserve and monitor
   `pft-a666-public-reserve-proof-epoch7-bounded`; do not start another
   high-memory proof concurrently.
2. Require exact-tip CI for `843554b` to finish green. Fix any failure,
   commit, push, and require the replacement exact-tip run to pass.
3. When epoch 7 completes, independently run `packet verify`, compare the
   public values byte-for-byte with the committed epoch-7 pins, hash every
   proof artifact, and retain the supervised service result.
4. Use the verified epoch-7 proof to run
   `a666_public_successor_proof_migrates_and_survives_six_validator_restart`
   in release mode with the existing local issuer/reserve key files supplied
   only through environment paths. Never print or copy key contents.
5. Generate an A666-shaped overlay-aware v2 packet from controlled finalized
   route/vault/supply snapshots with
   `scripts/a666-build-live-nav-mark-ops.py`. Exercise wrong overlay, wrong
   supply, wrong profile, wrong NAV, wrong packet hash, stale proof, replay,
   restart, and snapshot rejection.
6. Start the epoch-8 bounded CPU Groth16 proof only after the epoch-7 service
   and exact migration test release their memory. Independently verify and
   reproduce it in the same way.
7. Finish the source-by-source qualification table. For each of Aave, EVM
   spot, Hyperliquid, NEAR, Solana, and XMR, record the exact public reader or
   proof identity, governed policy/committee/owner inputs, fresh checkpoint,
   valuation proof, liability result, fuzz evidence, two-epoch
   reconciliation, and independent reproduction. Do not replace a missing
   cryptographic check with aggregate attestation.
8. Run the exact controlled A666 lifecycle: transparent issue/redeem,
   private-middle issue/redeem, Ethereum export/return, partial outage,
   replay, pause, rollback, restart, and full reserve/supply/balance
   conservation.
9. Only after `G0` through `G5` are genuinely green, reread every live
   validator, stage exact preconditions and rollback, roll the required
   release to all six validators, pause the route, register/rebind the
   successor profile, submit/finalize the overlay-aware packet, advance the
   route policy, verify fleet convergence, run a minimal lifecycle, and
   unpause.
10. Reproduce the final live state from a clean public checkout. Only then
    mark `G6`, `G7`, all six adapters, and
    `stakehub_deprecated=true`.

Throughout these steps, preserve the legacy profile and packets as immutable
history. Keep the frozen Monday checkout and all unrelated untracked
deployment/evidence paths untouched.

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
