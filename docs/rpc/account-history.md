# Account History

Account history is exposed through server-side bounded `account_tx` reads and a
disk-backed retained-history index.

## What It Solves

Wallets, explorers, custodians, and operators need a way to ask:

- which transactions affected this account;
- when did a transaction finalize;
- which receipt proves it;
- can the history index be rebuilt from retained data.

## Current Capabilities

- bounded account transaction reads;
- disk-backed per-account shards;
- index status reporting;
- catch-up after archive pruning;
- Python client access;
- CSV-style export support.

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

The disk index and the archive scan build their rows with the same function
(`account_tx_rows_for_transparent_block`) and list a row for an account when it
is the row's `from` or `to`, so they return the same rows: transfers,
`payment_v2`, asset, atomic swap, escrow, NFT and offer operations, including
offer fills. The Python fallback (`_account_tx_client_side_scan`) reads the
same kinds, including atomic swaps (one row per leg the account owns or
receives, `tx_role` `leg_0` or `leg_1`), and numbers escrow, NFT and offer rows
after the block's swaps so `transaction_index` and `tx_id` match the index and
the archive scan (`python/tests/test_account_tx_fallback.py`).

`truncated` is `true` when a matching row was omitted. The archive scan (used
when no index is usable; `index_used: false`) and the Python fallback read at
most `limit` blocks. If the height range holds more blocks than that, the
unread blocks are not checked and `truncated` is `true` even if none of them
match: for these paths the flag means "possibly incomplete". Narrow the range
with `from_height`/`to_height`, or use `account_tx_history`, to page through
it.

## Evidence

- `docs/runbooks/account-tx-index.md`
- `scripts/postfiat-rpc-account-tx`
- `reports/testnet-six-wallet-account-tx-smoke/`
- `reports/testnet-account-tx-disk-index-smoke/`
