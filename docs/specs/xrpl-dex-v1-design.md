# XRPL DEX V1 Design

Status: design gate for controlled-testnet feature parity
Date: 2026-05-20
Scope: transparent PFT and issued-asset order book design before implementation

This design is the `DEX-001` gate from
`docs/status/xrpl-feature-parity-burndown.md`. It does not authorize DEX
execution code by itself. DEX implementation starts only after the matching
model is benchmarked and the ordering-fairness policy is accepted.

## V1 Decision

PostFiat DEX v1 should be a native transparent limit-order book for PFT and
issued assets. It should not be an AMM, NFT marketplace, shielded DEX,
cross-chain bridge market, or royalty system.

Atomic settlement templates remain a separate swap toolkit. They are useful for
negotiated bilateral settlement, but they do not replace an order book because
they do not provide standing liquidity, book discovery, or deterministic
crossing.

## Non-Goals

- no shielded or private order flow;
- no AMM pools;
- no NFT bids, asks, auctions, or royalties;
- no issuer clawback or freeze policy changes;
- no public MEV-resistance claims beyond the concrete ordering policy below;
- no off-ledger matching engine that can produce consensus-visible fills.

## Asset Model

DEX v1 supports only these asset identifiers:

- `PFT`, the native reserve and fee asset;
- issued-asset ids already defined by the asset/trustline layer.

Every offer must trade two different assets. At least one side may be `PFT`;
issued-asset to issued-asset books are allowed only when both assets already
exist and the account has the required trustlines.

Trustline authorization and freeze state are checked at offer creation and at
every deterministic fill. A fill that would violate a trustline limit,
authorization rule, freeze rule, reserve rule, or balance rule is skipped or
causes the taker transaction to reject, depending on where the violation occurs:
malformed or unauthorized taker input rejects the transaction, while a maker
offer that became unfillable is removed with a deterministic `offer_unfunded`
receipt.

## Offer Object

Offers are ledger objects. The deterministic id is:

```text
offer_id = H("postfiat.offer_id.v1", chain_id, owner, owner_sequence)
```

An offer stores:

- `offer_id`;
- `owner`;
- `owner_sequence`;
- `taker_gets_asset_id`;
- `taker_gets_amount_remaining`;
- `taker_pays_asset_id`;
- `taker_pays_amount_remaining`;
- `original_taker_gets_amount`;
- `original_taker_pays_amount`;
- `created_height`;
- optional `expiration_height`;
- `state`: `open`, `filled`, `canceled`, or `unfunded`.

The naming follows XRPL convention from the offer creator's perspective:
`taker_gets` is what a future taker receives from the offer, and `taker_pays` is
what a future taker pays to the offer owner.

All amounts are unsigned integers. Price comparison never uses floating point.
The canonical price is the ratio:

```text
taker_pays_amount_remaining / taker_gets_amount_remaining
```

Comparisons use checked cross multiplication with a widened integer type. If the
product cannot fit the deterministic widened type, the offer is malformed and
rejected before mempool admission.

## Reserves And Locked Funds

Offer creation is state expansion and must pay a PFT state-expansion fee plus
maintain a per-offer reserve. The reserve is released when the offer reaches a
terminal state.

DEX v1 should lock the offer creator's sell-side funds at offer creation. This
is stricter than a lazy-funded book, but it is the safer controlled-testnet
choice because it makes replay and mempool admission deterministic:

- PFT sell-side funds reduce available native balance while the offer is open;
- issued sell-side funds reduce available trustline balance while the offer is
  open;
- account and trustline read RPC should expose total balance, locked balance,
  and spendable balance once offers exist.

The ledger may store locked totals directly or derive them from open offers, but
execution must have one canonical rule for spendable balance. Mempool dry-runs
must include pending offer locks from the same sender before accepting another
transaction.

## Transactions

DEX implementation should add versioned signed transaction envelopes for:

- `offer_create`;
- `offer_cancel`.

`offer_create` fields:

- `source`;
- `taker_gets_asset_id`;
- `taker_gets_amount`;
- `taker_pays_asset_id`;
- `taker_pays_amount`;
- optional `expiration_height`;
- `fee`;
- `sequence`.

`offer_cancel` fields:

- `source`;
- `offer_id`;
- `fee`;
- `sequence`.

No legacy transparent transfer signing bytes may change. Offer signing bytes
must be versioned, domain-separated, and include `chain_id`, `genesis_hash`,
`protocol_version`, `address_namespace`, `transaction_kind`, algorithm id,
source, fee, sequence, and operation fields in canonical order.

## Matching Order

Matching occurs only during `offer_create`. There is no background matcher.
Given the taker offer, execution computes the opposing book by canonical pair
and side, then scans offers in this order:

1. best price for the taker;
2. lower `created_height`;
3. lower `owner_sequence`;
4. lexicographic `offer_id` as a final tie-breaker.

The maker list is bounded. A single taker transaction may cross at most
`MAX_DEX_CROSSES_PER_TRANSACTION` offers. The initial value should be 64 unless
benchmarks in `DEX-002` justify a smaller cap.

If the taker offer is fully consumed, no new offer object is created. If it is
partially consumed and still satisfies minimum amount and reserve rules, the
remaining taker side becomes a new open offer. If the remaining amount rounds to
zero or violates bounds, the transaction rejects before committing any partial
fills.

## Partial Fills

Partial fills preserve the original price ratio. For each maker offer:

- compute the maximum executable amount from maker remaining and taker
  remaining;
- transfer exact integer amounts in both assets;
- reduce maker and taker remaining amounts proportionally;
- reject any fill that would require fractional asset units;
- close the maker offer when either remaining side reaches zero.

Rounding is deterministic and conservative. The engine must never mint value
through division. If a proposed partial fill cannot be represented as integer
asset units without violating the quoted ratio, execution moves to the next
offer or rejects the taker when no deterministic fill is possible.

## Fee Accounting

Fees are paid in PFT by the transaction source.

`offer_create` fee components:

- base transaction fee;
- state-expansion fee when a residual offer is stored;
- match-computation fee based on crossed offer count;
- serialized-weight fee for receipt rows.

`offer_cancel` fee components:

- base transaction fee;
- serialized-weight fee.

Makers do not pay an execution fee at fill time because the maker already paid
to create and reserve the offer. There is no protocol spread, royalty, or
validator reward extraction from trade notional in v1.

## Receipts

An accepted `offer_create` receipt includes:

- taker transaction id;
- optional created offer id;
- taker source;
- input pair and amounts;
- total filled amounts by asset;
- remaining amounts if a residual offer was created;
- fill count;
- deterministic fee charged;
- receipt code: `accepted`, `filled`, or `partially_filled`.

Each fill subrecord includes:

- fill index;
- maker offer id;
- maker owner;
- taker address;
- asset ids;
- maker amount sent;
- taker amount sent;
- maker offer remaining amounts after the fill;
- terminal maker state if the offer closes.

Rejected receipts must distinguish malformed input, unsupported asset,
missing trustline, frozen trustline, insufficient unlocked balance,
insufficient reserve, excessive crosses, and no deterministic integer fill.

## Account History

`account_tx` must index DEX activity for both taker and maker accounts.

Rows should include:

- `transaction_kind`;
- `tx_role`: `offer_taker`, `offer_maker`, or `offer_cancel`;
- `offer_id`;
- `counterparty_offer_id` for maker rows produced by another taker's fill;
- asset ids;
- sent and received amounts;
- fee paid by the row account, if any;
- receipt code and message;
- block height, transaction index, and receipt index.

Maker account rows are emitted from fill subrecords even though the maker did
not submit the taker transaction. This is required for exchange-grade history
and replay-visible custody reconciliation.

## RPC And SDK Surface

Read RPC should include:

- `offer_info(offer_id)`;
- `account_offers(account, state?, limit?)`;
- `book_offers(taker_gets_asset_id, taker_pays_asset_id, limit?)`;
- DEX-aware `account_tx` rows;
- `tx` finality with full fill summary.

Write and build RPC should include:

- `offer_fee_quote`;
- `mempool_submit_signed_offer_transaction`;
- SDK request/response validation for every new method;
- Python client methods for quote, submit, and read RPC;
- Python wallet helpers for create/cancel after protocol execution exists.

Public write RPC remains gated by the existing controlled write-edge policy.

## Mempool And Replay

Mempool admission must run the same deterministic dry-run as block execution,
including pending sender sequence, pending locked funds, pending offer closes,
and max-cross limits. Admission must reject malformed, underfunded,
under-reserved, unsupported, or over-limit offer transactions before they enter
the pending set.

Archived block replay must reconstruct:

- open and terminal offer objects;
- locked balances;
- book indexes;
- account history rows for makers and takers;
- `tx` finality summaries and receipt fill subrecords.

No wall-clock time participates in offer expiry. Expiry uses ledger height.

## Ordering Fairness And MEV Position

Controlled-testnet DEX v1 may use deterministic transaction order within a
finalized batch, but it must not make public fairness or MEV-resistance claims
from that alone. The consensus rule is:

- validators agree on an exact batch order;
- execution processes offers in that order;
- offer crossing inside a transaction uses only deterministic book order;
- ties never depend on local mempool arrival time.

This is deterministic but not enough for public market fairness. Before public
DEX claims, `DEX-003` must either accept this as an explicit controlled-network
tradeoff or replace it with a stronger policy such as frequent batch auction,
commit-reveal, encrypted order flow, or deterministic multi-validator ordering.

## Implementation Gate

Before any DEX execution code lands:

1. `DEX-002` benchmarks must estimate worst-case crossing cost, receipt size,
   book index update cost, and replay cost for the max-cross cap.
2. `DEX-003` must freeze the ordering-fairness/MEV policy.
3. The implementation plan must define protocol types, execution, mempool,
   fees, receipts, account history, read RPC, SDK, Python wallet helpers,
   replay tests, and controlled-validator evidence paths.

The first implementation slice after those gates should be protocol/state
types only: deterministic offer ids, offer objects, canonical signing bytes,
and legacy-safe serialization.
