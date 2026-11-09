# Account History

Account history is exposed through server-side bounded `account_tx` reads and a
disk-backed retained-history index.

## What It Solves

Wallets, explorers, custodians, and operators need a way to ask:

- which transactions affected this account;
- when did a transaction finalize;
- which receipt proves it;
- which deposit tag or memo a payment carried;
- can the history index be rebuilt from retained data.

## Current Capabilities

- bounded account transaction reads;
- `payment_v2` and `issued_payment_v2` memo contents in `account_tx` and `tx`;
- transparent credits from shielded withdrawals;
- disk-backed per-account shards;
- index status reporting;
- catch-up after archive pruning;
- Python client access;
- CSV-style export support.

## Rows

A row is listed for an account when the account is the row's `from` or `to`.
Rows come from two block kinds:

- `batch_kind: "transparent"`: transfers, `payment_v2`, asset, atomic swap,
  escrow, NFT and offer operations, including offer fills.
- `batch_kind: "shielded"`: shielded withdrawals that credit a transparent
  account (see below).

Escrow rows name the account that signed and paid: `escrow_create` runs
owner → recipient with the escrowed `amount`; `escrow_finish` is signed by the
recipient, so `from` and `to` are the recipient and `amount` is 0 (a terminal
row: the credited amount is on the create row); `escrow_cancel` runs
owner → owner with `amount` 0. A finish is therefore listed only in the
recipient's history and a cancel only in the owner's.

Every row carries `accepted` and `receipt_code` from the receipt. A row with
`accepted: false` did not transfer its `amount`; a `tec_` receipt may still
have charged its fee (see below). Credit a deposit only from a row with
`accepted: true`.

## Memo Contents

`payment_v2` and `issued_payment_v2` carry up to 4 signed memos, 512 bytes in
total. Each row keeps `memo_hash`, `memo_count` and `memo_bytes` and adds
`memos`, one entry per memo in signed order (an empty list when the payment has
no memos):

- `memo_type`, `memo_format`, `memo_data`: the signed fields as lowercase hex,
  exactly as stored in the block. Empty when the signer left the field out.
- `memo_type_text`, `memo_format_text`, `memo_data_text`: the same bytes
  decoded as UTF-8. Present only when the field is nonempty and valid UTF-8.

Match deposit tags on the hex fields: the `*_text` fields are a convenience.
`memos` is absent on every other transaction kind. The contents are read from
the archived block payload, so they are available for every finalized
`payment_v2` and `issued_payment_v2`, not only for mempool entries.

## Shielded Withdrawals

A shielded withdrawal credits a transparent account from a shielded pool. It is
listed in the recipient's history with:

- `batch_kind`: `shielded`.
- `transaction_kind`: `orchard_withdraw_v1` (native PFT),
  `asset_orchard_egress_v1` or `asset_orchard_private_egress_v1` (issued
  assets).
- `tx_role`: `shielded_withdrawal`.
- `from`: the shielded pool id, for example `orchard-v1` or
  `asset-orchard-v1`.
- `to`: the credited transparent account.
- `amount`: the exact amount credited to `to`.
- `asset_id`: absent for native PFT; the issued asset id otherwise.
- `fee`: the fee burned from the shielded value; the recipient pays nothing.
- `sequence`: `0`; no account sequence is consumed.
- `tx_id`: the action's receipt id; `tx` and `receipts` accept it.
- `block_height`, `batch_id`, `transaction_index`: the block, the shielded
  batch, and the action's index in it.

Example `account_tx` result with a memo-tagged deposit and a shielded
withdrawal credit (hashes shortened):

```json
{
  "schema": "postfiat-account-tx-v1",
  "address": "pf2222222222222222222222222222222222222222",
  "index_used": true,
  "row_count": 2,
  "rows": [
    {
      "tx_id": "9b41…e07c",
      "block_height": 812,
      "batch_kind": "transparent",
      "batch_id": "5d0a…91f2",
      "transaction_index": 0,
      "transaction_kind": "payment_v2",
      "from": "pf1111111111111111111111111111111111111111",
      "to": "pf2222222222222222222222222222222222222222",
      "amount": 25000000,
      "fee": 1200,
      "sequence": 14,
      "memo_hash": "c3e8…4a19",
      "memo_count": 1,
      "memo_bytes": 34,
      "memos": [
        {
          "memo_type": "6465706f7369745f746167",
          "memo_format": "746578742f706c61696e",
          "memo_data": "637573746f6d65722d34323432",
          "memo_type_text": "deposit_tag",
          "memo_format_text": "text/plain",
          "memo_data_text": "customer-4242"
        }
      ],
      "accepted": true,
      "receipt_code": "accepted"
    },
    {
      "tx_id": "e2d7…0b5c",
      "block_height": 815,
      "batch_kind": "shielded",
      "batch_id": "7af3…c2d8",
      "transaction_index": 0,
      "transaction_kind": "orchard_withdraw_v1",
      "from": "orchard-v1",
      "to": "pf2222222222222222222222222222222222222222",
      "amount": 2500000,
      "fee": 7,
      "sequence": 0,
      "tx_role": "shielded_withdrawal",
      "accepted": true,
      "receipt_code": "accepted"
    }
  ]
}
```

## `tx` Transaction Rows

`tx` adds `transaction_rows`: the `account_tx` rows of that transaction, built
by the same code from the archived batch of the block that finalized it. A
`payment_v2` or `issued_payment_v2` therefore returns its memo contents from
`tx` too, and a shielded withdrawal returns its credit row. `transaction_rows`
is omitted when the transaction affects no transparent account or when its archived batch is not
retained. It is not part of `proof_id`.

## Result Window and `truncated`

`account_tx` returns at most `limit` rows in ascending height order. With
`from_height` it returns the oldest matching rows of the range; without it, the
newest. The disk index, the archive scan and the Python client fallback agree
on this.

The archive scan and the Python fallback read a window of at most `limit`
blocks inside the requested range: the oldest blocks from `from_height`, or,
without it, the newest blocks at or below `to_height` (the chain tip when
`to_height` is absent). An end height below the tip therefore never reads
blocks above it.

The disk index, the archive scan and `tx` build their rows with the same
function (`account_tx_rows_for_block`), so they return the same rows. The
Python fallback (`_account_tx_client_side_scan`) reads the same kinds,
including atomic swaps (one row per leg the account owns or receives,
`tx_role` `leg_0` or `leg_1`), memo contents and shielded withdrawals. It
numbers rows after a block's swaps and FastLane primary transactions so
`transaction_index` and `tx_id` match the index and the archive scan
(`python/tests/test_account_tx_fallback.py`,
`python/tests/test_account_tx_exchange_rows.py`).

Every row keeps its receipt's `accepted` flag and `receipt_code`. A row whose
`receipt_code` starts with `tec_` was rejected but fee-claimed: `fee` left
`from` and `sequence` was consumed, while `amount` did not move.
`issued_payment_v2` rows carry `memo_count`, `memo_bytes`, the same
`memo_hash` digest and the same decoded `memos` structure as `payment_v2`.

`truncated` is `true` when a matching row was omitted. The archive scan (used
when no index is usable; `index_used: false`) and the Python fallback read at
most `limit` blocks. If the height range holds more blocks than that, the
unread blocks are not checked and `truncated` is `true` even if none of them
match: for these paths the flag means "possibly incomplete". Narrow the range
with `from_height`/`to_height`, or use `account_tx_history`, to page through
it.

## Index Rebuild and Backfill

The index is an operator cache rebuilt from the retained blocks, archived
batches and receipts; it is not consensus state. Ordered commit does not
refresh it. Run the build as maintenance:

```bash
postfiat-node account-tx-index-build --data-dir <validator-data>
```

The build extends a current index from its tip, or rebuilds it from every
retained block when it cannot be extended.

The current index schema is `postfiat-account-tx-index-v4`: the v3 layout (rows
in bounded per-account chunks and metadata-only aggregate files), with the
`escrow_finish` row attributed to the recipient who signed it (v3 named the
owner). Schema v2 added memo contents and shielded withdrawals. An index
written with an older schema (v1, v2 or v3) is never read or extended. After
upgrading, run `account-tx-index-build` once: it rebuilds the index from all
retained blocks, which for a v1 index also backfills memo contents and
historical shielded withdrawals. Until then,
`account_tx_index_status` reports the index unusable with
`unsupported account_tx index schema`, and `account_tx` answers from the
bounded archive scan, which already returns the new rows. History pruned below
the node's retained base is not in the rebuilt index; serve exchange history
from a node that retains it. See `docs/runbooks/account-tx-index.md`.

## Not Yet Listed

These transparent balance changes are not `account_tx` rows yet:

- transparent debits into a shielded pool (`orchard_deposit_v1` funding
  transfers, `asset_orchard_ingress_*` burns);
- FastLane primary deposits and redemptions;
- FastPay owned-lane certificates. Their memos are returned with the order by
  `owned_certificate`.

## Evidence

- `docs/runbooks/account-tx-index.md`
- `scripts/postfiat-rpc-account-tx`
- `crates/node/src/tests/account_tx_exchange_history.rs`
- `crates/node/tests/exchange_rpc_history.rs`
- `reports/testnet-six-wallet-account-tx-smoke/`
- `reports/testnet-account-tx-disk-index-smoke/`
