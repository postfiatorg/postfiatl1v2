# RPC Methods

This page is the hosted summary. The complete code-derived authorization
inventory is [RPC Method Inventory](../runbooks/rpc-method-inventory.md).

## Core Read Methods

| Method | Purpose |
| --- | --- |
| `status` | Node status, height, health, and service posture. |
| `server_info` | Server metadata and runtime information. |
| `metrics` | Public node metrics. |
| `ledger` | Current ledger state summary. |
| `fee` | Fee quote and policy information. |
| `validators` | Validator set and registry-related data. |
| `manifests` | Validator/operator manifest information where available. |
| `blocks` | Block lookup. |
| `receipts` | Receipt lookup. |
| `tx` | Transaction finality lookup. |
| `mempool_status` | Mempool status. |
| `bridge_status` | Bridge-simulation status. |
| `navcoin_bridge_routes` | PFTL-to-Uniswap NAVCoin bridge route status. |
| `navcoin_bridge_packet` | PFTL-to-Uniswap export packet status by route and packet hash. |
| `navcoin_bridge_claims` | Bounded outstanding export and return-claim status for a bridge route. |
| `navcoin_bridge_supply_status` | Supply, per-wallet native NAV balances, and invariant status for a bridge route. |
| `navcoin_bridge_receipt_replay` | Deterministic receipt replay verification for a bridge route. |
| `shield_turnstile` | Shielded turnstile accounting. |
| `orchard_pool_report` | Public Orchard pool counters. |
| `account` | Account state. |
| `account_tx` | Bounded account transaction history. |
| `account_tx_index_status` | Account-history index readiness. |

## Ledger Object Reads

| Method | Purpose |
| --- | --- |
| `asset_info` | Issued-asset definition lookup. |
| `account_lines` | Trustline lookup for an account. |
| `account_assets` | Issued-asset balance view for an account. |
| `issuer_assets` | Issuer-side asset inventory. |
| `escrow_info` | Escrow object lookup. |
| `account_escrows` | Escrows by owner or recipient. |
| `nft_info` | NFT object lookup. |
| `account_nfts` | NFTs by account. |
| `issuer_nfts` | NFTs by issuer and optional collection. |
| `offer_info` | DEX offer lookup. |
| `account_offers` | Offers by owner. |
| `book_offers` | Offers in a deterministic book pair. |

`navcoin_bridge_routes` route rows expose the bridge `handoff_controller` and
the `settlement_adapter` separately. Wallets must not treat either as the
Uniswap router/path; those execution fields are bound by the route config and
launch config digests.

## Controlled Write Methods

| Method | Purpose |
| --- | --- |
| `transfer_fee_quote` | Quote native PFT payment fees. |
| `mempool_submit_signed_transfer` | Submit signed native PFT transfer. |
| `mempool_submit_signed_payment_v2` | Submit signed native PFT payment with memos. |
| `asset_fee_quote` | Quote issued-asset and TrustSet-style transactions. |
| `mempool_submit_signed_asset_transaction` | Submit signed asset create, TrustSet, issued payment, or clawback. |
| `escrow_fee_quote` | Quote escrow create, finish, or cancel. |
| `mempool_submit_signed_escrow_transaction` | Submit signed escrow transaction. |
| `nft_fee_quote` | Quote NFT mint, transfer, or burn. |
| `mempool_submit_signed_nft_transaction` | Submit signed NFT transaction. |
| `offer_fee_quote` | Quote DEX offer create or cancel. |
| `mempool_submit_signed_offer_transaction` | Submit signed DEX offer transaction. |
| `atomic_settlement_template` | Build reciprocal escrow-leg templates for atomic swaps. |
| `atomic_swap_fee_quote` | Quote a W6 dual-authorized atomic swap against the exact parent state. |
| `mempool_submit_signed_atomic_swap_transaction` | Submit a signed W6 atomic swap when the controlled signed-submit surface is enabled. |
| `mempool_submit_signed_atomic_swap_transaction_finality` | Submit and return certificate/receipt finality for a signed W6 atomic swap. Success still requires an accepted receipt code. |
| `mempool_submit_fastlane_primary` | Submit a source-signed, sequence-bound FastLane deposit or an ordered FastPay recovery action when the controlled signed-submit surface is enabled. |
| `mempool_submit_fastlane_primary_finality` | Submit the same FastLane primary transaction through the finality-returning path. |

## FastPay Wallet Methods

| Method | Purpose |
| --- | --- |
| `owned_objects` | Read FastPay owned objects for an owner public key. Wallet-owned-object lookups use a 2048 object limit so fragmented wallets can still build standard unwraps. |
| `owned_recovery_capabilities` | Read the active v3 recovery domain, committee, and bounded validity/reveal windows. |
| `owned_certificate` | Retrieve a persisted complete certificate by digest or lock ID for permissionless recovery. |
| `owned_recovery_status` | Read ordered recovery state for a lock ID. |
| `owned_sign_v3` | Persist-before-sign validator vote for a v3 owner-authorized transfer envelope with a derived lock and recovery window. |
| `owned_apply_v3` | Apply a v3 transfer certificate and return authenticated durable acknowledgements. The wallet requires a cryptographic quorum of acknowledgements before reporting product finality. |
| `owned_unwrap_sign_v3` | Persist-before-sign validator vote for a v3 signed amount-based unwrap. |
| `owned_unwrap_apply_v3` | Apply a v3 unwrap certificate with the same quorum-acknowledgement rule. Multiple inputs and one change object are supported up to the protocol cap. |
| `owned_sign`, `owned_apply` | Versioned legacy signed/certified transfer compatibility below the governed v3 activation boundary. |
| `owned_unwrap_sign`, `owned_unwrap_apply` | Versioned legacy signed/certified unwrap compatibility below the governed v3 activation boundary. |
| `wrap_owned` | Disabled unsafe compatibility path. FastPay funding uses a source-signed `mempool_submit_fastlane_primary` deposit committed by consensus. |
| `unwrap_owned` | Disabled compatibility path. Public wallet flows must use signed/certified unwrap instead. |

## FastSwap Methods

FastSwap mutations are public protocol messages, not general state-write RPCs.
Each request is authorized by the signed intent, phase certificate, policy
command, or committee vote that the method verifies.

| Method | Purpose |
| --- | --- |
| `fastswap_capabilities` | Read supported FastSwap protocol/wire capabilities. |
| `fastswap_preview` | Validate a signed intent without reserving or mutating objects. |
| `fastswap_prepare` | Validate the dual-owner intent, atomically reserve all inputs, and return a persisted prepare vote. |
| `fastswap_commit` | Verify a LockQC and return the validator's durable decision vote. |
| `fastswap_apply` | Verify the confirmed DecisionQC and apply both conserved effects atomically. |
| `fastswap_catch_up` | Idempotently repair a replica from verified intent/certificate evidence. |
| `fastswap_status` | Read terminal or in-progress swap state. |
| `fastswap_effects` | Read certified terminal effects. |
| `fastswap_votes` | Retrieve persisted phase votes for recovery/relaying. |
| `fastswap_new_round_vote` | Vote to advance a stuck decision round. |
| `fastswap_propose_round` | Propose the Confirm-or-Cancel value for a recovery round. |
| `fastswap_precommit` | Persist and vote for a valid recovery-round proposal. |
| `fastswap_commit_round` | Commit a verified recovery-round precommit QC. |
| `fastswap_cancel_apply` | Apply a certified terminal Cancel decision without moving either leg. |
| `fastswap_checkpoint_status` | Read checkpoint/drain state. |
| `fastswap_objects` | Read FastSwap objects by owner and optional asset/object key. |
| `fastswap_policy` | Read active or selected policy snapshots. |

FastLane primary deposit/exit/checkpoint/control methods connect owned-object
reserves to the consensus ledger. Their exact posture is listed in the generated
inventory rather than duplicated here.

## Method Classification

- Read-only: safe for public read RPC when bounded.
- Controlled-write: useful for controlled network operations, not open by
  default.
- Local-only: intended for local node/request-file flows.

## Source Anchors

- [RPC Method Inventory](../runbooks/rpc-method-inventory.md)
- `crates/rpc_sdk/src/lib.rs`
- `crates/node/src/rpc_cli.rs`
- `reports/testnet-rpc-method-inventory/`
