# PostFiat NAV Infrastructure: Repository Map

**Prepared:** 2026-08-28  
**L1 source revision:** [`postfiatorg/postfiatl1v2@1f478e0c`](https://github.com/postfiatorg/postfiatl1v2/commit/1f478e0c473de42ecf43b4dd0925893de8f181ed)  
**Public-site source revision:** [`postfiatorg/postfiatorg.github.io@e4b2a19`](https://github.com/postfiatorg/postfiatorg.github.io/commit/e4b2a19f79a3c210374f4f07b35f26298391eb46)  
**Purpose:** A reviewer-oriented map of the NAVCoin infrastructure, what each component does, and where it lives.

> This is a source map, not a deployment attestation. The repository contains
> current, controlled-testnet, production-deployment, migration, and historical
> route lineages. A deployment audit must separately pin the enabled protocol
> version, route configuration, binaries, proof-program identities, contract
> addresses, and observed chain state. Start with the repository's
> [current-state boundary](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/docs/status/chain-state-current.md)
> and
> [A666 current-state record](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/docs/status/A666-PFUSDC-PRIVATE-SWAP-CURRENT-STATE-20260730.md).

## The system in plain English

A NAVCoin is a floating-NAV issued asset, not a fixed-price stablecoin. The L1
records which reserve evidence is admissible, finalizes a reserve/NAV epoch, and
allows supply to move only through bounded issue, redemption, bridge, and
market-operation rules.

```text
reserve sources and custody
  -> normalized observations or cryptographic proof
  -> proof profile + reserve packet
  -> deterministic verification and freshness checks
  -> finalized NAV epoch on PFTL
  -> issue / redeem / halt / settlement
  -> transparent balances, private Asset-Orchard notes, or wrapped venue supply
```

The central accounting requirement is:

```text
verified net assets >= valid global supply * NAV-per-unit floor
```

The reserve packet is public protocol state. Privacy can hide later transfers
or swaps, but it does not hide or replace the aggregate reserve, supply, and NAV
facts needed to audit backing.

External APIs are never fetched inside consensus. Off-chain readers normalize
outside data into deterministic commitments; the chain then verifies the
configured proof or registered-attestor result. This proves the disclosed
perimeter and computation. It cannot prove the absence of undisclosed
liabilities or make a dishonest custodian truthful.

## Source and audit size

The companion
[core feature LOC inventory](https://gist.github.com/0xPostFiatChad/b454ae72baeba6e8019322214a9182f0)
counts **37,174 nonblank production source lines** dedicated to NAV markets,
reserves, issuance, and external settlement at the pinned L1 revision.

That is not the complete trust boundary. The same inventory gives these
non-overlapping audit packages:

| Audit package | Included surface | Nonblank production LOC |
| --- | --- | ---: |
| NAV-dedicated code | NAV types, reserve/market execution, proof programs, NAV bridge and EVM settlement | **37,174** |
| Transparent NAV core | NAV + consensus + node runtime + canonical types/ML-DSA + base ledger + storage | **148,920** |
| Transparent NAV plus Cobalt | Transparent NAV core + active validator-governance implementation | **168,633** |
| Private NAV | Transparent NAV + Cobalt + Asset-Orchard | **198,053** |
| Private NAV plus generic bridge verification | Private NAV + generic bridge | **204,740** |

The counting methodology excludes explicit tests, scripts, documentation,
clients, wallets, generated reports, and vendored dependencies. Those excluded
surfaces still matter operationally; they are mapped below.

## Repository map

### 1. Product model and canonical accounting

| Location | What it does |
| --- | --- |
| [NAVCoin documentation index](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/docs/navcoins/index.md) | Defines NAVCoins, current asset names, architecture, proof limits, and the documentation reading order. |
| [Canonical primary-market accounting](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/docs/navcoins/primary-market-accounting.md) | Defines the adopted issue/redemption economics: subscriber value enters reserves when supply is created, and reserve principal leaves when supply is retired. It also defines rounding, fees, capacity, bridge conservation, and failure behavior. |
| [Reserve primitives](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/docs/navcoins/reserve-primitives.md) | Explains proof profiles, reserve packets, attestors, challenges, freshness, finalization, SP1, and the public/private boundary. |
| [Assets and venues](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/docs/navcoins/assets-and-venues.md) | Maps a651, a652, a666, wA666, pfUSDC, source-chain custody, PFTL ledger state, and Ethereum venues. It also marks legacy route lineage. |

### 2. Canonical protocol state, identities, and signed operations

| Location | What it does |
| --- | --- |
| [`market_nav_asset_types.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/types/src/market_nav_asset_types.rs) | Owns NAV proof profiles, reserve packets, attestors, redemptions, vault-bridge receipts and allocations, market-operation data, PFTL/Uniswap route state, canonical IDs, hashes, bounds, and validation. This is the main NAV state-model file. |
| [`transactions_mempool_receipts.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/types/src/transactions_mempool_receipts.rs) | Defines the signed transaction operations that register profiles/assets/attestors, submit and challenge packets, finalize epochs, mint, redeem, settle, halt, move vault receipts, and operate bridge routes. It also binds each operation into canonical signing bytes. |
| [`nav_reserve_public_values.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/types/src/nav_reserve_public_values.rs) | Defines the bounded public output decoded from NAV reserve proofs: valuation unit, asset/liability buckets, trust counts, policy identity, and statement bindings. |
| [`core_chain.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/types/src/core_chain.rs) | Supplies shared chain, genesis, active-profile, and network-state types that bind NAV behavior to the correct protocol instance. |
| [`ledger_assets.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/types/src/ledger_assets.rs) | Supplies issued-asset ledger primitives used to represent NAVCoin and source-labeled settlement balances. |

### 3. Deterministic NAV execution

| Location | What it does |
| --- | --- |
| [`entrypoints.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/execution/src/entrypoints.rs) | Dispatches canonical NAV, vault-bridge, market-operation, and PFTL/Uniswap operations into state transitions and receipts. |
| [`nav_vault_asset_execution.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/execution/src/nav_vault_asset_execution.rs) | Implements the main state machine: reserve-profile/packet lifecycle, vault receipts, counted settlement value, subscription overlays, mint/redeem paths, bridge conservation, and transparent/private A666 primary transitions. |
| [`market_policy.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/execution/src/market_policy.rs) | Implements deterministic fixed-point NAV floors, backing capacity, venue evidence replay, premium/discount metrics, alignment-reserve limits, and bounded mint/deploy decisions. |
| [`vault_bridge_policy.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/execution/src/vault_bridge_policy.rs) | Applies haircut/bucket policy to source-chain receipts and computes counted and redeemable value. |
| [`vault_bridge_profile_resolution.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/execution/src/vault_bridge_profile_resolution.rs) | Resolves which registered proof profile governs a vault-bridge route and rejects ambiguous or mismatched profile use. |
| [Issued-asset ledger helpers](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/execution/src/issued_asset_ledger_helpers.rs) | Applies the underlying issued-balance and trustline accounting used by NAV operations. |

### 4. Reserve-proof and receipt-proof boundary

| Location | What it does |
| --- | --- |
| [`nav_sp1_verifier.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/execution/src/nav_sp1_verifier.rs) | Performs bounded SP1 Groth16 verification and independently checks that decoded public values match the exact asset, epoch, profile, supply, policy, and reserve context being consumed. |
| [`nav_reserve_protocol`](https://github.com/postfiatorg/postfiatl1v2/tree/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/nav_reserve_protocol) | Defines the shared composite-source-root protocol used to bind reserve evidence and subscription settlement. |
| [`pfusdc_proofs`](https://github.com/postfiatorg/postfiatl1v2/tree/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/pfusdc_proofs) | Host-side types and verification support for pfUSDC ingress/egress proof statements. |
| [`pftl_uniswap_proofs`](https://github.com/postfiatorg/postfiatl1v2/tree/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/pftl_uniswap_proofs) | Host-side types and verification support for proof-bound PFTL/Uniswap receipt and handoff statements. |
| [SP1 guest programs](https://github.com/postfiatorg/postfiatl1v2/tree/1f478e0c473de42ecf43b4dd0925893de8f181ed/programs) | Contains the compiled proof programs for pfUSDC ingress variants, pfUSDC egress, and PFTL/Uniswap receipts. Each deployed route must pin the exact guest/program identity; the presence of several lineages does not mean all are active. |

### 5. External-source observation and source-labeled cash

| Location | What it does |
| --- | --- |
| [`python/postfiat_rpc/navcoin.py`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/python/postfiat_rpc/navcoin.py) | Builds NAV operation JSON, profile identities, reserve packets, attestor registrations/attestations, and redemption-settlement operations. |
| [`hyperliquid.py`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/python/postfiat_rpc/hyperliquid.py) | Fetches and normalizes public Hyperliquid account state outside consensus for deterministic observation roots. |
| [`solana.py`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/python/postfiat_rpc/solana.py) | Fetches and normalizes Solana token and stake-account state for reserve observations. |
| [`basis_policy.py`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/python/postfiat_rpc/basis_policy.py) | Applies basis-strategy valuation, hedge-gap, margin, and policy-hash rules to external observations. |
| [Vault-bridge NAV profile](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/docs/specs/vault-bridge-navcoin-profile.md) | Specifies the generic source-domain receipt primitive behind pfUSDC-style assets. The consensus machinery is generic; names such as USDC or pfUSDC belong in route configuration, not special protocol branches. |

### 6. Node orchestration, RPC, and audit views

| Location | What it does |
| --- | --- |
| [`market_bridge.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/node/src/market_bridge.rs) | Exposes market/vault status, PFTL/Uniswap route and packet views, supply status, replay tools, route initialization, primary subscription, export, destination consume, refund, return burn, and return import. |
| [`vault_bridge_workflows.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/node/src/vault_bridge_workflows.rs) | Builds operator-safe bundles for source deposits, relay, claim, withdrawal, reserve-packet export/replay, and burn-to-redeem workflows. |
| [`vault_bridge_conservation.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/node/src/vault_bridge_conservation.rs) | Produces route/deposit/redemption conservation reports and checks bridge accounting across lifecycle states. |
| [`rpc_dispatch.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/node/src/rpc_dispatch.rs) | Connects NAV proof status, bridge routes, packets, claims, supply status, and replay/preflight operations to the node RPC surface. |
| [`rpc_sdk`](https://github.com/postfiatorg/postfiatl1v2/tree/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/rpc_sdk) | Defines and validates the client-facing request/response schema for NAV and bridge queries. |

### 7. Ethereum custody, verification, and venue contracts

| Contract | What it does |
| --- | --- |
| [`ERC20BridgeVault.sol`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/ethereum-contracts/src/ERC20BridgeVault.sol) and [later vault lineages](https://github.com/postfiatorg/postfiatl1v2/tree/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/ethereum-contracts/src) | Custody source-chain ERC-20 deposits and pay accepted withdrawals. Deployment review must identify the exact active version. |
| [`PFTLFinalityVerifierV1.sol`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/ethereum-contracts/src/PFTLFinalityVerifierV1.sol) and [`PFTLReceiptFinalityVerifierV1.sol`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/ethereum-contracts/src/PFTLReceiptFinalityVerifierV1.sol) | Verify compact proof outputs that bind finalized PFTL blocks or receipts before Ethereum-side value movement. |
| [`PfUsdcIngressAnchorV1.sol`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/ethereum-contracts/src/PfUsdcIngressAnchorV1.sol) | Anchors accepted source-chain ingress statements for the proof-backed pfUSDC path and replay protection. |
| [`PFTLUniswapPrimaryMarketV2.sol`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/ethereum-contracts/src/PFTLUniswapPrimaryMarketV2.sol) | Enforces the Ethereum side of proof-bound primary issue/redemption and wrapped-supply movement for the current route design. |
| [`PFTLBridgeAdapter.sol`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/ethereum-contracts/src/PFTLBridgeAdapter.sol) | Admits PFTL-finalized market-operation envelopes through its configured verification/challenge boundary. |
| [`MarketOpsEnvelope.sol`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/ethereum-contracts/src/MarketOpsEnvelope.sol), [`MarketOpsVault.sol`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/ethereum-contracts/src/MarketOpsVault.sol), and [`PolicyRegistry.sol`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/ethereum-contracts/src/PolicyRegistry.sol) | Mirror compact PFTL policy outputs, register accepted policy identities, custody optional alignment reserves, and execute bounded venue actions. |
| [`NAVGuardHook.sol`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/ethereum-contracts/src/NAVGuardHook.sol) | Provides the controlled-launch Uniswap-v4-shaped venue-evidence/hook boundary. It is not the source of NAV truth. |
| [`A651ToA666MigrationV1.sol`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/ethereum-contracts/src/A651ToA666MigrationV1.sol) | Implements the one-way legacy a651-to-a666 successor conversion without mint authority or a mutable conversion ratio. |

### 8. Optional private NAV settlement

| Location | What it does |
| --- | --- |
| [`privacy_orchard`](https://github.com/postfiatorg/postfiatl1v2/tree/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/privacy_orchard) | Implements the Asset-Orchard adapter, circuits, commitments, anchors, nullifiers, note encryption, authorization, and proof verification used for private issued-asset transfer and swap. |
| [`asset_orchard.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/privacy_orchard/src/asset_orchard.rs) | Owns the multi-asset Orchard transaction model and public statement binding. |
| [`asset_orchard_circuit.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/privacy_orchard/src/asset_orchard_circuit.rs) | Constrains private note spends/outputs and per-asset conservation for shielded NAV swaps. |
| [Orchard node policy and state application](https://github.com/postfiatorg/postfiatl1v2/tree/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/node/src) | Wires policy-bound shielded actions into state application, roots, nullifiers, RPC, and recovery. Key entry files include `orchard_policy_actions.rs`, `orchard_state_application.rs`, and `shielded_batch_actions.rs`. |

Private execution changes what transaction observers can see. It does not relax
reserve proof, NAV epoch, issuance, redemption, or aggregate supply accounting.

### 9. Inherited L1 trust boundary

NAV correctness also depends on code not named “NAV”:

| Location | Why NAV depends on it |
| --- | --- |
| [ML-DSA crypto provider](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/crypto_provider/src/lib.rs) | Authenticates accounts, validators, and signed protocol objects. |
| [Consensus ordering](https://github.com/postfiatorg/postfiatl1v2/tree/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/ordering_fast) | Establishes the final order of NAV, reserve, mint, redeem, and bridge operations. |
| [Node finality](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/node/src/block_finality.rs) | Binds accepted transaction receipts and state roots into finalized blocks. |
| [Transactional storage](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/storage/src/transactional.rs) | Persists balances, supply, reserve state, receipts, and replay history atomically. This source candidate still has open qualification gates before public testnet. |
| [Cobalt governance](https://github.com/postfiatorg/postfiatl1v2/tree/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/consensus_cobalt) | Governs validator-registry/trust-graph evolution in the recorded controlled-devnet lineage; it is not the source of reserve price or portfolio truth. |

### 10. Operator tooling and evidence

| Location | What it does |
| --- | --- |
| [NAVCoin tools map](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/docs/navcoins/pftl-tools.md) | Maps CLI commands, Python modules, Solidity contracts, and evidence outputs, and explicitly labels historical/archived tools. |
| [NAV-related scripts](https://github.com/postfiatorg/postfiatl1v2/tree/1f478e0c473de42ecf43b4dd0925893de8f181ed/scripts) | Contains current A666, pfUSDC, PFTL/Uniswap, reserve-proof, conservation, recovery, and private-swap workflows. Script presence is not evidence that its corresponding deployment is active. |
| [Evidence index](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/docs/evidence/index.md) | Entry point for code-, test-, report-, and packet-backed capability claims. |
| [Public launch boundary](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/docs/security/public-launch-boundary.md) | Lists the release-safety boundaries that remain before treating controlled evidence as public real-value readiness. |

## What an auditor should pin first

1. The exact PFTL source and binary, chain/genesis, protocol activation heights,
   validator registry, and storage backend.
2. The enabled NAV proof profile, SP1 verifying key/program identity, canonical
   public-input schema, valuation-policy hash, freshness limits, and challenge
   rules.
3. The exact NAV asset, reserve operator, settlement asset/source domain,
   decimal scale, rounding rules, supply definition, and bridge in-flight terms.
4. Every active source-chain contract address, bytecode hash, proxy/admin
   boundary, verifier, vault, route cap, replay domain, and finality assumption.
5. Whether transparent-only or Asset-Orchard private paths are reachable.
6. Which a651/a666, Arbitrum/Ethereum/Sepolia, legacy/current, and migration
   lineages are actually enabled.
7. A reconciliation of native spendable supply, wrapped spendable supply,
   shielded value, custody balances, pending redemptions, and in-flight bridge
   claims.

## Appendix A: published PostFiat NAV articles

The website source repository is named
[`postfiatorg.github.io`](https://github.com/postfiatorg/postfiatorg.github.io),
but its configured and published domain is **postfiat.org**. Each entry below
links both the live article and its source at the pinned website revision.

| Date | Article | Why it matters | Source |
| --- | --- | --- | --- |
| 2026-06-10 | [The NAVCoin Proposal](https://postfiat.org/blog/navcoin-proposal/) | Defines a NAVCoin as a machine-verified floating-NAV claim, introduces proof profiles, freshness, challenges, disclosure tiers, and the limits of reserve verification. | [Markdown](https://github.com/postfiatorg/postfiatorg.github.io/blob/e4b2a19f79a3c210374f4f07b35f26298391eb46/content/blog/navcoin-proposal.md) |
| 2026-06-11 | [Pricing Counterparty Risk into the NAVCoin](https://postfiat.org/blog/navcoin-counterparty-risk/) | Separates verified balances from venue credit risk and proposes a public risk signal rather than pretending proof removes counterparty failure. | [Markdown](https://github.com/postfiatorg/postfiatorg.github.io/blob/e4b2a19f79a3c210374f4f07b35f26298391eb46/content/blog/navcoin-counterparty-risk.md) |
| 2026-06-14 | [Proof of Disclosed Leverage](https://postfiat.org/blog/proof-of-leverage/) | Describes the SP1 reserve-evidence primitive and its honest boundary: it reconciles a disclosed account set but does not prove global solvency or completeness. | [Markdown](https://github.com/postfiatorg/postfiatorg.github.io/blob/e4b2a19f79a3c210374f4f07b35f26298391eb46/content/blog/proof-of-leverage.md) |
| 2026-06-18 | [Private NAV Subscriptions and OTC Swaps](https://postfiat.org/research/private-nav-otc-swaps/) | Defines primary reserve-forming subscriptions versus secondary swaps and the design target for shielded settlement. | [Markdown](https://github.com/postfiatorg/postfiatorg.github.io/blob/e4b2a19f79a3c210374f4f07b35f26298391eb46/content/research/private-nav-otc-swaps.md) |
| 2026-06-20 | [Heavy ZK: Circuit Anatomy and Prover Optimization](https://postfiat.org/research/heavy-zk-optimization-v2/) | Maps the Asset-Orchard swap constraints, privacy boundary, measured prover/verification costs, and remaining external-audit limits. | [Markdown](https://github.com/postfiatorg/postfiatorg.github.io/blob/e4b2a19f79a3c210374f4f07b35f26298391eb46/content/research/heavy-zk-optimization-v2.md) |
| 2026-06-25 | [Private OTC Swaps](https://postfiat.org/research/private-otc-swaps/) and [illustrated primer](https://postfiat.org/research/private-otc-swaps/primer/) | Reports controlled transparent/shielded demonstrations and explains the user-visible path from counted cash through private swap and public redemption. | [Report source](https://github.com/postfiatorg/postfiatorg.github.io/blob/e4b2a19f79a3c210374f4f07b35f26298391eb46/content/research/private-otc-swaps.md), [primer source](https://github.com/postfiatorg/postfiatorg.github.io/blob/e4b2a19f79a3c210374f4f07b35f26298391eb46/content/research/private-otc-swaps-primer.md) |
| 2026-06-27 | [Canonical NAVCoin Transaction](https://postfiat.org/research/canonical-navcoin-transaction/) and [visual walkthrough](https://postfiat.org/canonical-navcoin-transaction/) | Gives the clearest end-to-end map: reserve epoch, USDC ingress, PFTL as source of truth, private Orchard swap, bridge proof, wrapped representation, and Uniswap venue. | [Research source](https://github.com/postfiatorg/postfiatorg.github.io/blob/e4b2a19f79a3c210374f4f07b35f26298391eb46/content/research/canonical-navcoin-transaction.md), [deck source](https://github.com/postfiatorg/postfiatorg.github.io/blob/e4b2a19f79a3c210374f4f07b35f26298391eb46/content/blog/canonical-navcoin-transaction.md) |
| 2026-06-27 | [Trustless Bridges from PFTL to Uniswap](https://postfiat.org/research/trustless-pftl-uniswap-bridges/) | Specifies the target supply-conserving lock/burn, finality-proof, mint/unlock, replay, and failure boundaries between PFTL and Ethereum venues. | [Markdown](https://github.com/postfiatorg/postfiatorg.github.io/blob/e4b2a19f79a3c210374f4f07b35f26298391eb46/content/research/trustless-pftl-uniswap-bridges.md) |
| 2026-07-10 | [Private NAV Swaps, Explained From Zero](https://postfiat.org/research/private-nav-swap-explainer/) | Plain-English product and architecture explanation covering pfUSDC, NAV accounting, public/private flows, counterfeit resistance, RFQ, latency, and status boundaries. | [Markdown](https://github.com/postfiatorg/postfiatorg.github.io/blob/e4b2a19f79a3c210374f4f07b35f26298391eb46/content/research/private-nav-swap-explainer.md) |
| 2026-07-16 | [Trustless Wrapped Stablecoins on PFTL](https://postfiat.org/research/trustless-wrapped-stablecoins/) | Specifies the generic source-labeled stablecoin receipt and vault path behind pfUSDC, including observer, challenge, redemption, and proof-tier boundaries. | [Markdown](https://github.com/postfiatorg/postfiatorg.github.io/blob/e4b2a19f79a3c210374f4f07b35f26298391eb46/content/research/trustless-wrapped-stablecoins.md) |
| 2026-07-19 | [pfUSDC: A Stablecoin Bridge Secured by Proofs, Not Committees](https://postfiat.org/pfusdc-trustless-bridge/) | Explains the bidirectional proof-based bridge design and exact conservation/replay goals for source USDC and PFTL pfUSDC. | [Markdown](https://github.com/postfiatorg/postfiatorg.github.io/blob/e4b2a19f79a3c210374f4f07b35f26298391eb46/content/blog/pfusdc-trustless-bridge.md) |
| 2026-08-02 | [A Controlled Private FX Swap: pfUSDC–pNOK](https://postfiat.org/private-fx-executed-pnok/) | Adjacent implementation evidence showing how the same source-labeled cash and Asset-Orchard machinery composes into an atomic private two-asset settlement, with explicit controlled-run limitations. | [Markdown](https://github.com/postfiatorg/postfiatorg.github.io/blob/e4b2a19f79a3c210374f4f07b35f26298391eb46/content/blog/private-fx-executed-pnok.md) |
| 2026-08-15 | [Glass: Institutionalizing NAVCoins](https://postfiat.org/research/glass-institutionalizing-navcoins/) | Research extension that reuses reserve-evidence machinery for policy-specific collateral passports; explicitly not a deployed consensus or custody system. | [Markdown](https://github.com/postfiatorg/postfiatorg.github.io/blob/e4b2a19f79a3c210374f4f07b35f26298391eb46/content/research/glass-institutionalizing-navcoins.md) |
| 2026-08-18 | [Trustless UltraShort Tokens](https://postfiat.org/blog/trustless-ultrashort-tokens/) | Research proposal for a NAVCoin backed by an isolated on-chain short-perpetual strategy, illustrating how proof profiles can support a new product without changing the base NAV accounting model. | [Markdown](https://github.com/postfiatorg/postfiatorg.github.io/blob/e4b2a19f79a3c210374f4f07b35f26298391eb46/content/blog/trustless-ultrashort-tokens.md) |

## Appendix B: relevant source-only drafts

These files exist in the website repository but are marked `draft: true` at
the pinned revision and return 404 on the public site. They are useful design or
historical context, not published/current claims.

| Draft | Use and caution | Source |
| --- | --- | --- |
| *Minting a651: One Portfolio, Many Access Venues* | Explains global backing versus local access venues. It is an a651-era architecture draft. | [Markdown](https://github.com/postfiatorg/postfiatorg.github.io/blob/e4b2a19f79a3c210374f4f07b35f26298391eb46/content/blog/navcoin-ethereum.md) |
| *NAVCoin Collateralization Without Spot Redemption* | Earlier market-operations proposal. Its no-spot-redemption framing is superseded for A666 by the adopted symmetric primary-market accounting. | [Markdown](https://github.com/postfiatorg/postfiatorg.github.io/blob/e4b2a19f79a3c210374f4f07b35f26298391eb46/content/blog/navcoin-collateralization.md) |
| *pfUSDC: Source-Labeled Cash Receipts for NAVCoin* | Early proposal for counted source-chain cash receipts; useful for the core trust model but not a current deployment record. | [Markdown](https://github.com/postfiatorg/postfiatorg.github.io/blob/e4b2a19f79a3c210374f4f07b35f26298391eb46/content/blog/pfusdc.md) |
| *pfUSDC × NAVCoin: A Proven End-to-End OTC Swap MVP* | Historical Arbitrum + WAN-devnet round-trip report. Current route and deployment claims must use later status documents. | [Markdown](https://github.com/postfiatorg/postfiatorg.github.io/blob/e4b2a19f79a3c210374f4f07b35f26298391eb46/content/blog/navcoin-otc-mvp-proven.md) |
| *Heavy ZK* (older blog draft) | Earlier copy retained in source; use the published `heavy-zk-optimization-v2` research article instead. | [Markdown](https://github.com/postfiatorg/postfiatorg.github.io/blob/e4b2a19f79a3c210374f4f07b35f26298391eb46/content/blog/heavy-zk-optimization.md) |
| *Anatomy of a Proven Private Swap* | Detailed evidence narrative retained as a draft; not a public deployment or independent-audit claim. | [Markdown](https://github.com/postfiatorg/postfiatorg.github.io/blob/e4b2a19f79a3c210374f4f07b35f26298391eb46/content/research/proven-private-swap.md) |
