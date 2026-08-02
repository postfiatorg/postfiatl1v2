# StakeHub Deprecation and Public NAVCoin Reserve Verification Plan

**Created:** 2026-08-01

**Rewritten after executable audit:** 2026-08-02

**Priority:** P0

**Status:** **INCOMPLETE — StakeHub is not deprecated for A666 reserve publication**

**Canonical document:** this is the sole implementation plan and continuation
handoff for this work.

The deleted continuation handoff is not a second source of truth. The JSON
readiness file referenced below is only a machine-enforced status ledger; it
does not define architecture or weaken any requirement in this document.

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

Quantity and valuation are separate claims. A source can have a
cryptographically verified balance and an attested USD price. The system must
display and enforce that distinction rather than collapsing both into a vague
“proof of reserves” label.

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
- The successor feature implements public source-specific verifiers for Aave,
  the complete EVM spot set, Hyperliquid, staked NEAR, staked Solana at its
  accurately disclosed attested trust level, and Monero. These verifiers are
  implemented but not production-qualified; section 3.4 records the exact
  remaining work.
- The public CLI emits source-checkpoint vote statements, canonically assembles
  independently signed ML-DSA votes, and rejects invalid committee bindings,
  sub-quorum certificates, duplicates, unknown validators, and bad signatures.
  It does not receive validator private keys.
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
| Aave on Arbitrum | Provider-neutral verifier and public checkpoint/collection workflow implemented; partial | No | Add governed A666 policy/committee inputs, fuzz, run fresh epochs and complete A666 reconciliation, and qualify |
| Complete EVM spot set | Provider-neutral quantity verifier and public checkpoint/collection workflow implemented; partial | No | Add the governed A666 policy/committee fixture, bind separately disclosed valuation evidence, fuzz, run fresh epochs and complete A666 reconciliation, and qualify |
| Hyperliquid | Provider-neutral verifier implemented; partial | No | Add public collection and fuzzing, reproduce complete historical and fresh epochs, complete full A666 reconciliation, and qualify |
| Staked NEAR | Provider-neutral quantity verifier implemented; partial | No | Add public collection and fuzzing, reproduce complete historical and fresh epochs, bind separately attested valuation, complete full A666 reconciliation, and qualify |
| Staked Solana | Provider-neutral attested-state verifier implemented; partial | No | Add the public collector and governed independent signer/policy fixture, fuzz, run fresh epochs and complete A666 reconciliation, and qualify at the accurately disclosed attested quantity trust level |
| Monero | Provider-neutral cryptographic quantity verifier implemented; partial | No | Add the public collector, produce a fresh governed nonzero proof with certified head chain and spent-status set, bind separately disclosed XMR/USD valuation evidence, fuzz, complete A666 reconciliation, and qualify |
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
historical stake quantities and authority data. Monero reconstructs the real
historical nonzero transaction/RingCT/ownership/inclusion proof and also
verifies a synthetic context-bound zero-reserve test vector without treating
an aggregate amount signature as proof. Its production path still lacks a
public collector, a fresh governed nonzero header-chain/spent-status proof, and
separate XMR/USD valuation evidence. The EVM spot adapter proves
reserve quantities only and deliberately leaves USD prices in the separately
declared valuation trust dimension. Its public CLI now constructs deterministic
per-chain checkpoint candidates from pinned RPC heights, supports independent
checkpoint voting and assembly, emits the exact owner-authorization statement,
and collects the complete native/ERC-20 proof set from an exact reviewed RPC
map. It is still unqualified until governed A666 inputs, adversarial/fuzz
coverage, fresh epochs, and reconciliation exist. The Solana adapter does not
relabel RPC snapshots as cryptographic: it publicly verifies the exact position set,
stake/withdraw/vote authorities, state parsing, signer policy, agreement, and
signatures while retaining an `attested` quantity classification. The
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

If Solana quantity remains intentionally attested, the public collector,
canonical statement, ownership binding, signature verification, and wallet
trust disclosure must still be complete. It must not depend on StakeHub.

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
  private keys.
- [x] Implement and register the public Aave adapter.
- [x] Implement the public Aave checkpoint candidate, owner authorization,
  and complete RPC proof collector; policy-pin the user balance mapping slots.
- [x] Implement and register the complete public EVM spot quantity adapter set.
- [x] Implement the public complete-EVM-spot checkpoint candidate, owner
  authorization, and multichain RPC collector workflow.
- [x] Implement and register the public Hyperliquid adapter.
- [x] Implement and register the public staked-NEAR adapter.
- [ ] Implement the public staked-Solana collector/verifier at its governed
  trust level.
- [x] Implement and register the public XMR reserve-proof quantity adapter.
- [ ] Remove all provider-specific hash domains and compiled operator identities
  from successor semantics.
- [ ] Add public fixtures and adversarial tests for every adapter.
- [ ] Add fuzz targets for every new parser handling external proof material.
- [ ] Prove malformed or unsupported adapter evidence fails closed without
  panic or unbounded work.

### Phase 3 — build the source-equivalent A666 profile

- [ ] Create a public A666 source manifest selecting the real adapter for each
  quantity and valuation dimension.
- [ ] Preserve or strengthen historical cryptographic trust classifications.
- [ ] Permit attestation only where explicitly intended and accurately
  disclosed.
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
| `G3` Public A666 adapters | Aave, EVM spot, Hyperliquid, NEAR, Solana, and XMR are publicly implemented, guest-registered where cryptographic, and adversarially qualified. | **FAIL/OPEN** |
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
implementation baseline before this document update:
c763e6a8f12197efe202657a6f30202215e00fbd
upstream matched that baseline when audited
```

Frozen demonstration checkout — do not modify:

```text
/home/postfiat/tmp/a666-pfusdc-monday-demo-2246d257
```

The pfUSDC overlay protocol boundary, canonical-plan consolidation, and
readiness-gate placement are pushed as:

```text
af9ae4e Harden public reserve overlay protocol boundary
14f3697 Make public reserve verification plan canonical
c763e6a Keep adapter readiness outside shipped proof kit
```

It implements the pfUSDC overlay packet path described in section 5.7, keeps
the proof guest identity unchanged, and adds the machine-readable adapter
readiness gate. Verification completed before commit:

```text
cargo test -p postfiat-types
  119 passed

cargo test -p postfiat-nav-reserve-protocol
  1 passed

cargo test -p postfiat-execution
  176 passed

cd tools/nav-reserve-proof
cargo test --locked
  18 passed
cargo check --locked -p postfiat-reserve-proof --features sp1
  passed

cargo check --workspace --all-targets
  passed

scripts/test-proof-public-input-inventory
  passed; 5 systems, 70 public fields, 10 source hashes

scripts/check-a666-public-adapter-readiness
  passed; 0/6 production-qualified, StakeHub deprecated=false

pinned Docker SP1 guest rebuild
  expected ELF SHA: 0f8476431677bfe0a8f9f19db7439abce1a879ba5736cfa3225ae7de4e5b0e52
  rebuilt  ELF SHA: 0f8476431677bfe0a8f9f19db7439abce1a879ba5736cfa3225ae7de4e5b0e52

git diff --check
  passed
```

At the time of this document update, the exact-tip remote CI run for the
`c763e6a` implementation baseline was still in progress:

```text
https://github.com/postfiatorg/postfiatl1v2/actions/runs/30727267430
```

Do not close Phase 1 until that exact SHA is green. The worktree also contains
the uncommitted adapter development described in section 3.4 and hundreds of
unrelated untracked deployment and evidence paths. Never use `git add .`, bulk
clean, or delete untracked evidence. Stage only explicitly reviewed files.

Current canonical reserve-proof vkey:

```text
0x000c7271e0711abce0c61d293222fd4a144599a779db8cadadc4df35e31a4100
```

The retired A666 shadow used an obsolete vkey and must be regenerated only
after the real source adapters are ported.

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

A new encrypted Ed25519 attestor key exists outside the repository at:

```text
/home/postfiat/.pft/a666-reserve-proof-v1
```

It may be used only for source dimensions intentionally governed as
`ATTESTED`. It must not be used to sign all six old aggregate numbers as a
shortcut around porting source validators. Before legitimate live use, it
requires a secure off-host backup. Never print or commit its private key or
passphrase.

Existing A666 issuer and reserve-operator keys remain owner-only under the
existing `.pft` runtime trees. Never copy their contents into this repository,
logs, evidence, or new temporary paths.

## 11. Current continuation actions

Start read-only:

```bash
cd /home/postfiat/repos/a666-eth-fast-lane-combined-20260724
git status --short --branch
git rev-parse HEAD
git diff --check
git diff -- \
  crates/types/src/nav_reserve_public_values.rs \
  crates/execution/src/nav_sp1_verifier.rs \
  tools/nav-reserve-proof/crates/reserve-proof-cli/src/main.rs \
  docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md
```

Then:

1. Require green exact-tip remote CI for the latest public-adapter commit; fix
   failures without touching live governance or the frozen demonstration
   checkout.
2. Implement public collectors for Hyperliquid, NEAR, Solana, and Monero;
   adversarially qualify the implemented Aave and exact-EVM-spot collectors.
   Each collector must independently
   validate the certified source state before emitting an observation.
3. Add structured fuzz targets and complete the adversarial matrix in section
   8 for every externally sourced parser and proof type.
4. Create the governed A666 manifest, quantity/valuation policies, committee
   fixtures, and successor guest build only after all six adapters have their
   public production inputs. Do not rotate the immutable legacy guest once per
   adapter.
5. Reproduce both historical epochs and at least two fresh independently
   collected epochs, reconcile every source and the pfUSDC overlay, and retain
   negative evidence for omission, replay, staleness, and source substitution.
6. Keep the live A666 route and frozen Monday checkout untouched until every
   controlled migration gate passes.

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
