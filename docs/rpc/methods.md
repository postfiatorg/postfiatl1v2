# RPC Methods

This page is the hosted summary of the public wallet and protocol surface.
[RPC Dispatch Method Coverage](method-coverage.md) documents the remaining
local, gated, compatibility, and verification arms. The complete code-derived
authorization inventory is
[RPC Method Inventory](../runbooks/rpc-method-inventory.md).

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
| `tx` | Transaction finality lookup, with the transaction's `account_tx` rows (`transaction_rows`), including native and issued payment memo contents. |
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
| `account_tx` | Bounded account transaction history: transparent rows with native and issued payment memo contents, plus shielded-withdrawal credits. |
| `account_tx_index_status` | Account-history index readiness. |

`rpc-serve` answers `server_info` in-process, from three sources:

- its cached status (the same 1 s cache as `status`);
- one governance read, for `validators.registry_update_count`;
- the constant fees.

The response is unchanged. Releases before this change answered it in a child
process that recomputed the status and read the whole stored history, which
took 6.6–12 s on the devnet validators on 2026-11-09. Until a deployed release
carries the in-process answer, callers should allow 30 s; the Python client
does (`SERVER_INFO_TIMEOUT_SECONDS`). The local `rpc --method server_info`
command still computes the full metrics.

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

`account_tx` and `tx` row fields, memo decoding, shielded-withdrawal rows and
the index rebuild that backfills them are documented in
[Account History](account-history.md).

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
| `mempool_submit_signed_asset_transaction` | Submit signed asset create, TrustSet, issued payment (`issued_payment`, or `issued_payment_v2` with memos), clawback, or `account_set` (owner account flags). |
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

Signed submit methods reject a destination address that is not a canonical
`pf` + 40 lowercase hex address before admission. The error `code` is
`invalid_recipient_address`, `invalid_recipient_checksum` or
`noncanonical_recipient_address`; see
[Addresses And Checksums](../wallets/address-format.md). The `account` result
adds `address_checksummed`, the display form of `address`.

The fee quotes take the next sequence from the mempool and price the
transaction on a ledger that includes pending entries.

- `transfer_fee_quote` prices on the ledger admission dry-runs the transfer
  on. Its base is the committed ledger with the confirmed FastPay effects
  that are not yet in a block, the same base admission starts from. On top of
  it come every sender's pending plain transfers; a memo quote adds the
  pending `payment_v2` entries. This is the order a block runs them in.
- So its `minimum_fee`, `state_expansion_fee`, `recipient_exists`,
  `sender_balance` and reserve checks count a recipient that one of those
  entries creates, and their debits.
- They do not count kinds that run after the transfer: asset, escrow, NFT or
  offer entries, or `payment_v2` entries for a plain transfer. A plain transfer
  to an account that only such an entry creates pays the creation fee.
- Its `sender_sequence` is the sequence on that base, before the pending
  entries.
- The other four quotes start from the same base and price on the ledger
  after the whole executable mempool.

A quote holds only while the pending entries do: if one is dropped before the
block, a fee signed from the quote can be below the minimum the block then
requires. A pending entry that no longer executes makes a quote fail with
`mempool dry-run blocked by stale pending …`, and it makes admission refuse as
well. For `transfer_fee_quote` this applies to any sender's pending plain
transfer (or `payment_v2`, for a memo quote); for the other quotes, to any
pending entry.

The escrow and offer reserve flags count the native PFT the transaction
locks, as execution does after taking the fee. In `escrow_fee_quote`,
`sender_meets_reserve_after_fee` is false unless the balance after the fee and
a native `escrow_create`'s `amount` (`sender_balance_after_amount_and_fee`)
keeps the account reserve. In `offer_fee_quote`,
`sender_meets_reserve_after_fee_and_reserve` is false unless the balance after
the fee, a native sell side's `taker_gets_amount` and the offer object
reserve (`sender_balance_after_amount_fee_and_reserve`) keeps it. Issued
assets lock nothing native. The after-amount fields are `null` when the lock
exceeds the balance; `sender_balance_after_fee` and
`sender_balance_after_fee_and_reserve` still count only the fee and the
object reserve.

Receipts from these submissions use `accepted: true` with code `accepted` (or
an offer fill code) when applied. A rejected receipt whose code starts with
`tec_` was fee-claimed: its fee was burned and its sequence consumed, nothing
else changed. `account` returns `flags` (`1` = require memo) only when set.
Both behaviours are governed; see
[Custody And Exchange](../wallets/custody-and-exchange.md#fees).

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
