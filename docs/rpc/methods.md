# RPC Methods

This page is the hosted summary. The source inventory is
`docs/runbooks/rpc-method-inventory.md`.

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

`navcoin_bridge_routes` route rows expose the bridge `handoff_controller` and
the `settlement_adapter` separately. Wallets must not treat either as the
Uniswap router/path; those execution fields are bound by the route config and
launch config digests.
| `account_offers` | Offers by owner. |
| `book_offers` | Offers in a deterministic book pair. |

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

## FastPay Wallet Methods

| Method | Purpose |
| --- | --- |
| `owned_objects` | Read FastPay owned objects for an owner public key. Wallet-owned-object lookups use a 2048 object limit so fragmented wallets can still build standard unwraps. |
| `wrap_owned` | Move account-lane PFT into a FastPay owned object. |
| `owned_sign` | Validator vote after owner-authenticated FastPay transfer admission; `order_json` is the complete signed envelope, not a bare order. |
| `owned_apply` | Finalize a quorum-certified FastPay owned-transfer certificate. The wallet proxy durably journals the 5-of-6 certificate, requires one validator to cryptographically validate and durably apply it, then returns `certificate_final=true` while its recoverable outbox replicates to the remaining validators. This changes apply latency, not the certificate threshold. |
| `owned_unwrap_sign` | Validator vote for a signed FastPay unwrap order. |
| `owned_unwrap_apply` | Finalize a quorum-certified FastPay unwrap certificate through the same durable certificate/outbox path. Standard unwrap is amount-based, can consume multiple input objects up to the 2048 input cap, and returns change as a FastPay object. |
| `unwrap_owned` | Disabled compatibility path. Public wallet flows must use signed/certified unwrap instead. |

## Method Classification

- Read-only: safe for public read RPC when bounded.
- Controlled-write: useful for controlled network operations, not open by
  default.
- Local-only: intended for local node/request-file flows.

## Source Anchors

- `docs/runbooks/rpc-method-inventory.md`
- `crates/rpc_sdk/src/lib.rs`
- `crates/node/src/rpc_cli.rs`
- `reports/testnet-rpc-method-inventory/`
