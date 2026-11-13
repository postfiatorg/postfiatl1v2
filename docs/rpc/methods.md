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
| `navcoin_bridge_routes` | PFTL-to-Uniswap NAVCoin bridge route status, including each route's Ethereum `authority_epoch` and whether it matches the active checkpoint committee. |
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

The fields and values are the same as before, with four exceptions:

- `metrics.ok` reports only the governance read; it no longer means the whole
  stored history is readable;
- `storage` describes rpc-serve's own store handle;
- the answer can be up to 1 s old, like `status`, with the count read in the
  same refresh as the status;
- a failed status read returns `rpc_status_failed`.

Main answers it in-process since `9f76e2e5e`. The
deployed release still runs the child-process path, which recomputes the status
and reads the whole stored history. That took 6.6–14.1 s on the devnet
validators on 2026-11-09. Until the next release carries `9f76e2e5e` (see the
[release plan](../plans/active/next-validator-release-plan-20261115.md)),
callers should allow 30 s; the Python client does
(`SERVER_INFO_TIMEOUT_SECONDS`). The local `rpc --method server_info` command
still computes the full metrics.

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

Each row also carries three additive route-authority fields:

- `authority_epoch`: the route's Ethereum verification `authority_epoch` as
  stored; `null` when the route has no governed Ethereum verification policy.
- `authority_committee_current`: `true` when that epoch is the epoch of the
  newest governed checkpoint committee (`fastswap_committees`, the committee
  execution treats as current), `false` when it is older, `null` when the node
  cannot tell.
- `authority_committee_unknown_reason`: `null` when the flag is known;
  otherwise `route_has_no_ethereum_verification_policy`,
  `no_active_checkpoint_committee_recorded`,
  `route_authority_epoch_ahead_of_active_committee` or
  `consensus_ledger_unavailable` (a node without a consensus ledger).

The flag only says whether the route's epoch matches the newest governed
checkpoint-committee epoch; it is not update eligibility. A `false` route
still names an older committee. A `pftl_uniswap_route_bridge_policy_update`
moves a route exactly one epoch, to the newest governed committee (exact
epoch, root and domain, a valid committee), and only with no prepare fence
on that epoch and no open route handoff. A route two or more epochs behind
therefore cannot be caught up by updates, and no recovery path exists today:
the update is the only operation that changes a stored route policy, and
route initialization refuses an existing route id. Older nodes omit the
three fields.

## Controlled Write Methods

| Method | Purpose |
| --- | --- |
| `transfer_fee_quote` | Quote native PFT payment fees. |
| `mempool_submit_signed_transfer` | Submit signed native PFT transfer. |
| `mempool_submit_signed_payment_v2` | Submit signed native PFT payment with memos. |
| `asset_fee_quote` | Quote issued-asset and TrustSet-style transactions. For `external_lane_export_v1`, `export_refusal` (`code`, `message`) is execution's refusal of the export on the ledger asset admission executes it on (pending plain transfers, `payment_v2` entries and asset transactions; later families excluded), and `sender_balance` is read there too (for `external_lane_export_above_released`, the message names released and exported on that ledger); absent when it would apply. The export's quote also carries `export_checked: true` (no other kind's quote has it); the holder's commands refuse a quote without it (`external_lane_export_not_checked`), which comes from a node too old to check an export. |
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

A quote is the node's answer, not the request: the asset, escrow, NFT and
offer signers sign the quote's `operation`, `minimum_fee` and `sequence`.
Compare them with your request before you sign; the Python client does
([what it checks](../integrate/exchange-guide.md#what-the-python-client-checks-before-it-signs)).

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
| `owned_objects` | Read one page of the FastPay owned objects of `owner_public_key_hex` (optional `asset`, `limit` up to 2048, `cursor`, `omit_owner_keys`). See [Owned objects pages](#owned-objects-pages). |
| `owned_recovery_capabilities` | Read the active v3 recovery domain, committee, and bounded validity/reveal windows. |
| `fastpay_recovery_committee` | Read the committed FastPay recovery committee (validator ids, ML-DSA-65 public keys, `registry_root`, `valid_from_height`..`new_orders_through_height`) and the recovery policy, with `chain_id`, `genesis_hash` and the `height` read. No params returns the committee admitting new orders (`active: true`); `epoch` returns that epoch, and an epoch the ledger lacks is refused with `fastpay_recovery_committee_unknown_epoch`. Public keys only. |
| `owned_certificate` | Retrieve a persisted complete certificate by digest or lock ID for permissionless recovery. |
| `owned_recovery_status` | Read ordered recovery state for a lock ID. |
| `owned_fence_status` | This validator's ML-DSA-65-signed `FastPayFenceStatusV2` for `lock_id` (the only param), signed under `postfiat-l1-v2/fastpay-fence-status/v2` from certified blocks only: `decision` (`{"confirmed": ...}` or `"cancelled"`), `operation`, `fence_digest` (the apply acknowledgement's `terminal_state_digest`) and `anchored_at_or_before_height` (the lowest certified height at which this validator can show the effect anchored or the cancellation run: the block's height, or its history checkpoint's after a prune), or all four `null` for "not terminal at `observed_height`". Speculative and mempool state never count. Refused with `fastpay_fence_status_signer_unavailable` when rpc-serve signs through `--finality-signer-socket`, and `fastpay_fence_status_not_a_signer` when the node holds no key of the committee. One status proves nothing: the gateway returns every validator's answer, and a client accepts a quorum of distinct members of a committee whose registry root it pinned, with the same decision, operation and fence digest. |
| `owned_sign_v3` | Persist-before-sign validator vote for a v3 owner-authorized transfer envelope with a derived lock and recovery window. |
| `owned_apply_v3` | Apply a v3 transfer certificate and return authenticated durable acknowledgements. The wallet requires a cryptographic quorum of acknowledgements before reporting product finality. |
| `owned_unwrap_sign_v3` | Persist-before-sign validator vote for a v3 signed amount-based unwrap. |
| `owned_unwrap_apply_v3` | Apply a v3 unwrap certificate with the same quorum-acknowledgement rule. Multiple inputs and one change object are supported up to the protocol cap. |
| `owned_sign`, `owned_apply` | Versioned legacy signed/certified transfer compatibility below the governed v3 activation boundary. |
| `owned_unwrap_sign`, `owned_unwrap_apply` | Versioned legacy signed/certified unwrap compatibility below the governed v3 activation boundary. |
| `wrap_owned` | Disabled unsafe compatibility path. FastPay funding uses a source-signed `mempool_submit_fastlane_primary` deposit committed by consensus. |
| `unwrap_owned` | Disabled compatibility path. Public wallet flows must use signed/certified unwrap instead. |

### Owned objects pages

`owned_objects` returns objects in (asset, object id, version) order. A page
ends after `limit` objects (default 2048) or earlier, so that the compact JSON
result never exceeds 960 KiB (`OWNED_OBJECTS_PAGE_MAX_BYTES`): with the owner
key each object is about 4 KB, so a page holds about 235 objects, well inside a
2 MiB WebSocket message. When objects remain, `truncated` is true and
`next_cursor` is set; pass it back as `cursor` with the same owner and `asset`
to read the next page. The last page has `next_cursor: null`. A call without
`cursor` answers as before, plus the new fields.

- `total_value`: the sum of every matching object in this view, not only this page.
- `view_digest`: SHA-256 (64 hex) over every object matching the owner and `asset` filter, by asset, id, version and value, in page order. It identifies the object view; every page of one consistent read has the same `view_digest`.
- `cursor`: the cursor this page continues from; `null` on the first page.
- `next_cursor`: opaque; `null` on the last page. Bound to the owner, the `asset` filter, the `view_digest` and the last object returned.
- `height`, `state_root`: the finalized tip when the page was read; information only. They do not identify the object view: FastPay effects create and spend objects without a new block. The page and `view_digest` are read under the store's ordered-commit lock, so they are one moment.
- `owner_keys_omitted`: present and `true` when `omit_owner_keys` was sent. Objects then omit `owner_pubkey_hex` (the key is the top-level `owner_public_key_hex`; about 160 bytes per object).

The node keeps no snapshot per client: a cursor names a position in the order
and the view it was issued from, so validators hold no per-client state. When
the owner's objects changed since the cursor was issued (a block or a FastPay
apply), the continuation is refused with `owned_objects_view_changed` instead
of mixing two views; restart from the first page. A cursor from before the
view digest (`ooc1.`) gets the same refusal. The Python client's
`owned_objects_all` and the web wallet restart at most 3 times, then fail with
an error; a truncated answer without `next_cursor` or `view_digest` (a node
from before paging) is an error, never a complete set. The Python wallet's
`wrap_fastpay`, `unwrap_fastpay` and `send_fastpay` read through
`owned_objects_all`; a resumed send treats a missing input as spent only on an
untruncated answer. The gateway's `wallet_subscribe` feed reads one page per
validator per tick and adds `owned_complete` (`false` for a truncated page);
never select from a partial page. Refusals carry typed
codes and are never ignored: `owned_objects_view_changed`,
`owned_objects_cursor_malformed`,
`owned_objects_cursor_owner_mismatch` (issued for another owner),
`owned_objects_cursor_asset_mismatch` (issued for another `asset` filter),
`owned_objects_cursor_invalid_object_id`, and `owned_objects_foreign_owner_key`
(`omit_owner_keys` with an object of another key on the page).

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
