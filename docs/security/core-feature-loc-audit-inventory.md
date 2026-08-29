# Core Feature LOC and Audit-Scope Inventory

**Repository:** `postfiatl1v2`  
**Counted revision:** `1f478e0c` (`main`, 2026-08-28)  
**Core production total:** **225,070 nonblank source lines**

## Counting boundary

This inventory measures the core protocol implementation that an auditor could
reasonably treat as the product and trust boundary. It excludes:

- explicit test files and Rust `#[cfg(test)]` blocks;
- benchmarks, fuzzers, evidence campaigns, and generated reports;
- documentation, configuration, deployment scripts, and operator tooling;
- Python, JavaScript, web applications, wallets, and client SDKs;
- vendored FIPS-204 and Halo2 source; and
- development-only benchmark, simulation, drill, rehearsal, payload-generator,
  shadow-service, and CLI binaries.

The count includes nonblank Rust source in the owning L1 crates, production
Solidity contracts, and NAV proof guest programs. Nonblank comment lines remain
included. Files are assigned to exactly one bucket, so the feature totals do not
double-count shared code.

Several canonical type, execution, RPC, and node files serve more than one
feature. Those files are assigned to a shared-core bucket rather than being
arbitrarily charged to every consumer. Consequently, a feature's dedicated LOC
is not its complete transitive audit scope.

## Feature inventory

| Feature | Core LOC | Description | Primary repository locations | Audit complexity |
| --- | ---: | --- | --- | --- |
| NAV markets, reserves, issuance, and external settlement | **37,174** | Floating-NAV issued assets, reserve packets and profiles, supply/backing checks, mint and redeem rules, market policy, vault-bridge conservation, SP1-bound reserve verification, PFTL/PFUSDC proof guests, and production external-settlement contracts. | [NAV types](../../crates/types/src/market_nav_asset_types.rs), [reserve public values](../../crates/types/src/nav_reserve_public_values.rs), [NAV execution](../../crates/execution/src/nav_vault_asset_execution.rs), [market policy](../../crates/execution/src/market_policy.rs), [SP1 verifier](../../crates/execution/src/nav_sp1_verifier.rs), [reserve protocol](../../crates/nav_reserve_protocol/src/lib.rs), [market bridge](../../crates/node/src/market_bridge.rs), [vault workflows](../../crates/node/src/vault_bridge_workflows.rs), [vault conservation](../../crates/node/src/vault_bridge_conservation.rs), [PFUSDC proofs](../../crates/pfusdc_proofs/src/lib.rs), [PFTL Uniswap proofs](../../crates/pftl_uniswap_proofs/src/lib.rs), [proof guest programs](../../programs), [Ethereum contracts](../../crates/ethereum-contracts/src) | **Extreme** — financial conservation, valuation, proof, freshness, issuance, redemption, and cross-system boundaries. |
| Consensus v2, ordering, mempool, and transport | **34,542** | Deterministic proposal ordering, prepare/precommit finality, quorum certificates, timeout/view changes, durable signer safety, mempool admission, peer transport, and state-commitment bindings. | [ordering_fast](../../crates/ordering_fast/src), [mempool_dag](../../crates/mempool_dag/src), [network](../../crates/network/src), [block finality](../../crates/node/src/block_finality.rs), [consensus artifacts](../../crates/node/src/consensus_artifacts.rs), [v2 finality](../../crates/node/src/consensus_v2_finality.rs), [v2 store](../../crates/node/src/consensus_v2_store.rs), [mempool proposals](../../crates/node/src/mempool_proposals.rs), [transport protocol](../../crates/node/src/transport_protocol.rs), [transport runtime](../../crates/node/src/transport_runtime.rs), [state commitment](../../crates/node/src/state_commitment.rs) | **Extreme** — consensus safety, determinism, quorum identity, persistence-before-signing, replay, and untrusted network input. |
| Shared node runtime, RPC, and orchestration | **36,803** | The executable node composition layer: protocol wiring, runtime state, RPC dispatch, CLI-to-runtime routing, lifecycle queries, certified operations, and shared execution orchestration. | [node crate](../../crates/node/src), [node types](../../crates/node/src/node_types.rs), [RPC dispatch](../../crates/node/src/rpc_dispatch.rs), [RPC CLI](../../crates/node/src/rpc_cli.rs), [lifecycle queries](../../crates/node/src/lifecycle_queries.rs), [main runtime](../../crates/node/src/main.rs), [runtime parts](../../crates/node/src/main_parts) | **Extreme** — largest cross-feature attack surface and the principal place where otherwise sound components can be composed incorrectly. |
| Asset-Orchard privacy and proof verification | **29,420** | Orchard/Halo2 adapter, action circuits, proof and authorization verification, note encryption, anchors, commitments, nullifiers, private transfer/swap actions, and transparent/private value turnstiles. | [privacy interface](../../crates/privacy/src/lib.rs), [Orchard adapter](../../crates/privacy_orchard/src), [Asset-Orchard](../../crates/privacy_orchard/src/asset_orchard.rs), [circuit](../../crates/privacy_orchard/src/asset_orchard_circuit.rs), [verification](../../crates/privacy_orchard/src/verify.rs), [Orchard policy actions](../../crates/node/src/orchard_policy_actions.rs), [Orchard state application](../../crates/node/src/orchard_state_application.rs), [shielded batch actions](../../crates/node/src/shielded_batch_actions.rs) | **Specialist/extreme** — circuit/public-input agreement, nullifier safety, conservation, verifier bounds, upstream dependency, and privacy leakage. |
| Storage, snapshots, migration, and replay | **22,485** | Transactional finalized-state persistence, expected-parent atomic commits, integrity checks, ordered-history accumulation, snapshots, replay, retained history, migration, backend activation, and verify-only behavior. | [storage crate](../../crates/storage/src), [transactional store](../../crates/storage/src/transactional.rs), [ordered history](../../crates/storage/src/ordered_history.rs), [storage commit](../../crates/node/src/storage_commit.rs), [storage migration](../../crates/node/src/storage_migration.rs), [batch snapshots](../../crates/node/src/batch_snapshot.rs), [history](../../crates/node/src/history.rs), [replay wallet](../../crates/node/src/block_replay_wallet.rs) | **Extreme/current release blocker** — crash atomicity, corruption, exact replay, migration, rollback, retained-history equality, and read-only verification. |
| Cobalt validator governance | **19,713** | Validator trust-graph evaluation, RBC/ABBA/MVBA/DABC agreement, admission policy, registry transition rules, authority certificates, governance ordering, handoff, shadow comparison, and recovery boundaries. | [consensus_cobalt](../../crates/consensus_cobalt/src), [trust-graph governance](../../crates/consensus_cobalt/src/trust_graph_governance.rs), [DABC registry](../../crates/consensus_cobalt/src/dabc_registry.rs), [node governance](../../crates/node/src/governance.rs), [authority certificate](../../crates/node/src/cobalt_authority_certificate.rs), [Cobalt handoff](../../crates/node/src/cobalt_handoff.rs), [Cobalt shadow](../../crates/node/src/cobalt_shadow.rs) | **Very high** — old/new-registry authorization, linkedness, proposal lineage, ratification scope, rollback, and separation from block finality. |
| Base ledger, issued assets, fees, NFTs, and execution | **11,793** | Transparent accounts and balances, issued-asset state, deterministic transaction entrypoints, fee burning, offers, NFTs/escrows, general ledger helpers, and shared state-transition behavior. | [core chain types](../../crates/types/src/core_chain.rs), [owned asset types](../../crates/types/src/account_owned_asset_types.rs), [execution entrypoints](../../crates/execution/src/entrypoints.rs), [issued-asset helpers](../../crates/execution/src/issued_asset_ledger_helpers.rs), [fees and offers](../../crates/execution/src/fees_offer_planning.rs), [NFT/escrow execution](../../crates/execution/src/nft_escrow_asset_execution.rs) | **Very high** — conservation, authorization, arithmetic, fees, replay protection, and rejection without partial mutation. |
| FastSwap DvP and recovery | **8,674** | Dual-owner prefunded-asset DvP, atomic reservation, prepare/decision/effects certificates, confirm-or-cancel finality, bridge/checkpoint controls, durable storage, catch-up, and restart recovery. | [FastSwap types](../../crates/types/src/fastswap_types.rs), [execution modules](../../crates/execution/src/fastswap.rs), [FastSwap service](../../crates/node/src/fastswap_service.rs), [FastSwap store](../../crates/storage/src/fastswap_store.rs), [FastSwap model](../../crates/fastswap_model/src/lib.rs) | **Very high** — two-party authorization, both-or-neither effects, late certificates, cancellation, terminal fencing, and recovery. |
| Generic bridge and Ethereum verification | **6,687** | Generic checkpoint and receipt verification, Ethereum proof processing, bridge policy types, and node-side checkpoint signing and receipt construction. Feature-specific NAV settlement contracts are counted in the NAV row. | [bridge crate](../../crates/bridge/src), [Ethereum checkpoint](../../crates/bridge/src/ethereum_checkpoint.rs), [receipt verifier](../../crates/bridge/src/ethereum_receipt.rs), [bridge types](../../crates/types/src/ethereum_bridge_types.rs), [checkpoint signing](../../crates/node/src/ethereum_checkpoint_signing.rs), [receipt proof builder](../../crates/node/src/ethereum_receipt_proof_builder.rs) | **Very high** — finality translation, proof validation, replay, chain identity, custody, and cross-ledger conservation. |
| Shared canonical types and ML-DSA authorization | **6,123** | Cross-feature signed transaction envelopes, receipts, validation helpers, canonical bytes, identifiers, domain separation, ML-DSA signing, and verification. | [transaction and receipt types](../../crates/types/src/transactions_mempool_receipts.rs), [validation helpers](../../crates/types/src/transactions_validation_helpers.rs), [types crate](../../crates/types/src), [crypto provider](../../crates/crypto_provider/src/lib.rs) | **Critical** — every feature depends on canonical encoding, domain binding, signature verification, bounds, and receipt semantics. |
| DGA/model governance | **6,254** | Deterministic model-generated governance rules, ruleset hashing, guarded-apply drills, evidence lineage, and verifier receipts. This is decision support and is not live mutation authority. | [governance agent](../../crates/node/src/governance_agent.rs), [agent parts](../../crates/node/src/governance_agent_parts) | **High but deferrable** — model output must remain outside consensus and cannot bypass typed policy, operator authorization, or Cobalt ratification. |
| FastPay object payments and recovery | **4,665** | Single-owner prefunded-object payments, owner authorization, certified apply, consume-or-cancel recovery, version fencing, and durable indexes. | [FastPay types](../../crates/types/src/fastpay_recovery_types.rs), [FastPay model](../../crates/fastpay-prototype/src), [primary-lane execution](../../crates/execution/src/fastlane_primary.rs), [owned transfer](../../crates/execution/src/owned_transfer.rs), [recovery](../../crates/execution/src/owned_transfer_recovery.rs), [node recovery](../../crates/node/src/fastpay_recovery_node.rs), [transactional index](../../crates/storage/src/transactional/fastpay_index.rs) | **High** — owner authorization, certificate uniqueness, durable application, cancellation, and late-certificate safety. |
| W6 consensus atomic swap | **737** | Dedicated RPC and server handling for a two-owner atomic swap executed as one ordinary consensus transaction. Canonical intent and execution code shared with the ledger remain in shared/base buckets. | [atomic-swap RPC](../../crates/node/src/atomic_swap_rpc.rs), [RPC server](../../crates/node/src/atomic_swap_rpc_server.rs), [transaction lifecycle](../architecture/transaction-lifecycle.md) | **High** — exact dual authorization, quote/freshness binding, conservation, and both-or-neither mutation. |

## Core totals and proposed audit packages

| Scope | Included surfaces | LOC |
| --- | --- | ---: |
| All counted core production code | Every row above | **225,070** |
| Transparent NAV core | NAV + consensus + shared node + shared canonical types/ML-DSA + base ledger + storage | **148,920** |
| Transparent NAV plus active Cobalt governance | Transparent NAV core + Cobalt | **168,633** |
| Private NAV | Transparent NAV plus Cobalt + Asset-Orchard | **198,053** |
| Private NAV plus generic bridge verification | Private NAV + generic bridge | **204,740** |

DGA/model governance, FastPay, FastSwap, and W6 can be separately disabled or
scoped unless they are part of the release configuration under audit.

## Auditor interpretation

The **37,174-line NAV surface is the largest dedicated product feature**. It is
not optional in a PostFiat NAV audit. Its transitive trust boundary includes
canonical types and ML-DSA authorization, consensus, shared node execution,
base-ledger accounting, and storage.

The full 225,070-line core should not be confused with the earlier 628,151-line
first-party repository count. The larger number includes tests, clients,
wallets, web applications, audit/evidence tooling, and operational automation
that this inventory deliberately excludes.

LOC is a scoping measure, not a security result. Final audit scope must pin the
release binary, enabled feature configuration, protocol/activation versions,
reachable RPC methods, storage backend, proof profiles, contract addresses, and
deployed source lineage.
