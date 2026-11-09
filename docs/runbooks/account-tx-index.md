# Account Tx Index

Status: v3 retained-history index (rows in bounded per-account chunks, metadata-only aggregate); block commits extend it
Date: 2026-10-26

`account_tx` is a read-only public RPC method for account history: transparent
transactions, including `payment_v2` memo contents, and transparent credits
from shielded withdrawals. Row fields are documented in
`docs/rpc/account-history.md`.
It reads an account's rows from its chunk files when `account_tx_index_meta.json`
matches the local chain/genesis/protocol/tip. If the disk metadata is absent,
stale, or unreadable, the aggregate `account_tx_index.json` (the same counts and
ranges, written last) can vouch for the same chunk files; if that is also
absent or stale, the node falls back to the existing bounded retained-history
scan instead of failing the public read.

## Layout

```text
<data-dir>/account_tx_index_meta.json            disk metadata: domain, counts, ranges, tip, chunk_rows
<data-dir>/account_tx_index.json                 aggregate metadata: the same fields
<data-dir>/account_tx_accounts/<h>/head.json     per account: row_count, chunk_count, first and last height
<data-dir>/account_tx_accounts/<h>/chunk-00000000.json   rows 0..63, then chunk-00000001.json, ...
```

`<h>` is the hash of the address. A chunk holds at most `chunk_rows` rows (64,
recorded in the metadata); every chunk but the last is full. Neither metadata
file lists accounts or rows, so both stay the same size as history grows. The
head is the authority for an account's row count: rows that a chunk holds past
it were left by an interrupted append and are ignored and overwritten.

This index is not consensus state. It is an operator cache rebuilt from local
block/archive/receipt data. Stale or unusable aggregate/disk status stays
visible through `account_tx_index_status`.

## Following the chain

Once `account-tx-index-build` has written the indexes, every block commit
extends both the disk index and the aggregate `account_tx_index.json`. No
rebuild is needed after each block, and `account_tx_index_status` keeps
`index_usable` and `disk_index_usable` true.

1. **Order.** The append runs only after the block, its archive entry and its
   receipts are durable: after the transactional commit returns, or after the
   ordered commit journal is applied and removed (also on journal recovery at
   restart).
2. **What is written.** The block's rows are computed from the commit's own
   block, archive entry and receipts. The writes go in this order:
   1. per touched account, its last chunk (or the next one, when the last is
      full) and then its head;
   2. `account_tx_index_meta.json`;
   3. the aggregate file.

   Each index is extended only from the block's parent.
3. **Idempotent.** A replayed commit whose block an index already ends at
   changes nothing in it. A replay after a stop between the account files and
   the metadata skips every account whose head already holds the block, so no
   row is added twice.
4. **Never fails the commit.** If an append fails, or the node stops before
   an index's metadata write, that index stays at an older tip, and later
   commits do not extend it across the gap. Reads use the next source that is
   current, in this order: the disk index, the aggregate, then the bounded
   archive scan (`index_used: false`). Run `account-tx-index-build` to catch it
   up; commits follow again after that.
5. **Cost per commit.** Per touched account, at most one chunk of 64 rows plus
   a head of about 0.5 KB; then the two metadata files, about 0.5 KB each. None
   of it grows with history. On a local fixture chain after 68 commits
   (`a_commit_rewrites_only_the_last_chunk_of_each_touched_account_and_the_metadata`)
   one commit wrote 16,006 bytes in 6 files: the sender's last chunk (4 rows,
   2.2 KB), the recipient's only chunk (23 rows, 11.8 KB), two heads and the two
   metadata files. The v2 layout wrote 148,412 bytes in 4 files for the same
   commit, because it rewrote the aggregate (89 KB) and both shards whole. On the
   1137 import (964 rows, 255 accounts) v2 rewrote up to 290 KB for the busiest
   account plus 1.36 MB of aggregate and 93 KB of metadata per commit, growing
   with history.
6. **No consensus change.** The append changes no block, receipt, ledger or
   state root. Its time is reported as `refresh_account_tx_index_ms` in the
   commit write timings.

`account_tx_index_status` reports each index's range against the tip:

- `indexed_from_height`, `indexed_to_height` and `tip_hash` for the aggregate,
  and `disk_indexed_from_height`, `disk_indexed_to_height` and `disk_tip_hash`
  for the disk index;
- `current_tip_height`;
- `index_lag_blocks` and `disk_index_lag_blocks`: 0 when that index follows the
  chain, the number of blocks it is behind otherwise, and absent when its tip
  is not on the local chain.

The build first extends the disk index block by block from its tip, through the
same append the commit uses, when that tip is on the local chain and the index
matches the same chain/genesis/protocol and index schema. Otherwise it removes
the metadata files and the account files and rebuilds from every retained
block; the metadata is written last, so reads use the archive scan until then.

## Upgrade Backfill

Index schema `postfiat-account-tx-index-v4` (disk index
`postfiat-account-tx-disk-index-v4`, account files
`postfiat-account-tx-account-head-v4` and `postfiat-account-tx-account-chunk-v4`)
is the chunked layout above. v4 rows differ from v3 only in the
`escrow_finish` row, whose `from` is now the recipient who signed and paid it
(v3 named the owner). Schema v3 introduced the chunked layout; v2 added
`payment_v2` memo contents and shielded-withdrawal rows. After installing a binary with a new schema, run
the build once on every node that serves `account_tx`:

```bash
postfiat-node account-tx-index-build --data-dir <validator-data>
postfiat-node account-tx-index-status --data-dir <validator-data>
```

An older index (v1, v2 or v3) is never read or extended, so this build re-reads
every retained block and writes the v4 files; for a v1 index that also
backfills memo contents and historical shielded withdrawals. Before the
rebuild, status reports `index_usable: false` with
`unsupported account_tx index schema` (and `disk_index_reason` with the disk
schema), commits leave the old files as they are, and reads use the bounded
archive scan (`index_used: false`), which already returns the current rows.
After it, status reports both indexes usable and reads return
`index_used: true`. The build removes the old shard files. Rows for history
pruned below the retained base are not rebuilt.

## Build

```bash
target/debug/postfiat-node account-tx-index-build \
  --data-dir /var/lib/postfiat/validator-0
```

The local build report includes the aggregate and disk metadata paths, indexed
height range, indexed transparent and shielded block and row counts, account
count, shard count, tip hash, `index_usable`, and `disk_index_usable`.

## Check

```bash
target/debug/postfiat-node account-tx-index-status \
  --data-dir /var/lib/postfiat/validator-0
```

The status report is also exposed as read-only RPC method
`account_tx_index_status`. The public report intentionally returns only index
filenames, not operator filesystem paths.

## Query

```bash
target/debug/postfiat-node account-tx \
  --data-dir /var/lib/postfiat/validator-0 \
  --address "$POSTFIAT_ADDRESS" \
  --from-height 0 \
  --limit 25
```

Green indexed reads return:

- `index_used: true`
- `archive_lookup_count: 0`
- `scanned_block_count: 0`

Fallback reads return `index_used: false` and keep the bounded scan limits.

## Current Follow-Ups

- Monitor alerts for stale indexes.

## Disk-Only Smoke

```bash
scripts/testnet-account-tx-disk-index-smoke
```

This local operator smoke starts a four-validator harness, finalizes a
transparent canary transfer, removes the serving validator's aggregate
`account_tx_index.json`, and proves the disk-backed per-account shards still
serve both CLI and read-only RPC `account_tx` with `index_used: true`,
`scanned_block_count: 0`, and `archive_lookup_count: 0`. It also confirms the
public `account_tx_index_status` response is path-redacted and reports the
aggregate index absent while the disk index remains usable.
The paired monitor snapshot should remain `ok` because effective
account-history readiness is aggregate-or-disk; missing aggregate JSON is not a
launch warning when disk shards are usable.
Validator doctor uses the same effective readiness rule for
`account_tx_index.ready`.

## Six-Wallet Fan-In Smoke

```bash
scripts/testnet-six-wallet-account-tx-smoke
```

This local operator smoke starts a four-validator harness, generates six
sender wallets plus one sink wallet, funds each sender, signs six transparent
wallet transfers into the sink, seals the six fan-in transactions in one
transparent batch, rebuilds the retained-history index, and queries read-only
RPC `account_tx_history` for the sink and every sender. A green run proves the
sink history registers all six inbound transactions and every sender history
registers both the faucet funding row and outbound sink transfer with
`all_index_used: true`, `total_scanned_block_count: 0`, and
`total_archive_lookup_count: 0`.

Latest local evidence:
`reports/testnet-six-wallet-account-tx-smoke/testnet-six-wallet-account-tx-smoke-csv-account-tx-20260517T030434Z.json`.
That run passed at height `7` with `12` indexed rows across `8` accounts,
exported the sink history to a six-row CSV through
`scripts/postfiat-rpc-account-tx --csv-output`, and removed generated wallet
backup/key material before writing the final report.

Controlled live rollout was completed on 2026-05-16. Evidence:
`reports/testnet-live-account-tx-index-refresh/live-account-tx-index-20260516T073517Z/testnet-live-account-tx-index-refresh.json`.
That refresh built `account_tx_index.json` on all five validators at height
`42`, kept chain state unchanged, and recorded present/usable indexes with 25
indexed rows across 20 accounts on every validator.

Incremental commit-refresh code evidence:
`reports/testnet-account-tx-index-incremental/account-tx-index-incremental-20260516T075611Z/testnet-account-tx-index-incremental.json`.
The regression proves multi-block catch-up keeps `account_tx` indexed after an
older archived payload has been pruned, while the full rebuild path is still
used when the cache cannot be safely extended.

Disk-backed read-index local evidence:
`reports/testnet-account-tx-disk-index/account-tx-disk-index-20260516T083817Z/testnet-account-tx-disk-index.json`.
The focused regression deletes the aggregate `account_tx_index.json` and proves
`account_tx` still serves indexed rows from per-account shards with zero
archive scans. RPC doctor, monitor snapshot, validator doctor, and the Python
client smoke all expose the disk index as usable.

Disk-only RPC/CLI smoke evidence:
`reports/testnet-account-tx-disk-index-smoke/testnet-account-tx-disk-index-smoke-20260516T093544Z.json`.
The smoke removes the aggregate index and proves CLI plus read-only RPC
`account_tx` still return a finalized canary row from disk shards with zero
retained-history scan and zero archive lookup. Paired monitor report:
`reports/testnet-monitor-snapshot/testnet-account-tx-disk-index-smoke-20260516T093544Z.json`.
Paired validator doctor report:
`reports/testnet-validator-doctor/testnet-account-tx-disk-index-smoke-20260516T093544Z.json`.

Live deployment evidence for the incremental catch-up binary:
`reports/testnet-live-orchard-binary-upgrade/live-account-tx-index-catchup-upgrade-20260516T080506Z/testnet-live-orchard-binary-upgrade.json`.
Post-upgrade live account-history refresh:
`reports/testnet-live-account-tx-index-refresh/post-upgrade-account-tx-index-20260516T081246Z/testnet-live-account-tx-index-refresh.json`.
That refresh passed at height `45` with 27 indexed rows across 21 accounts on
all five validators.

Live deployment evidence for the disk-backed read-index binary:
`reports/testnet-live-orchard-binary-upgrade/live-account-tx-disk-index-upgrade-20260516T084228Z/testnet-live-orchard-binary-upgrade.json`.
Post-disk-index live account-history refresh:
`reports/testnet-live-account-tx-index-refresh/live-account-tx-disk-index-20260516T084521Z/testnet-live-account-tx-index-refresh.json`.
After a fresh wallet-finality round and Orchard direct-deposit round, live
validator doctor passed at height `48` with 29 indexed rows and 22 disk account
shards on every validator:
`reports/testnet-live-validator-doctor/live-disk-index-validator-doctor-20260516T085327Z/testnet-live-validator-doctor.json`.

Fresh overnight live evidence passed at height `51`:
`reports/testnet-live-wallet-finality/overnight-finality-refresh-20260516T090806Z/testnet-live-wallet-finality.json`,
`reports/testnet-live-orchard-direct-deposit/overnight-orchard-refresh-20260516T090925Z/testnet-live-orchard-direct-deposit.json`,
`reports/testnet-live-account-tx-index-refresh/overnight-account-tx-index-20260516T091319Z/testnet-live-account-tx-index-refresh.json`,
and
`reports/testnet-live-validator-doctor/overnight-validator-doctor-20260516T091420Z/testnet-live-validator-doctor.json`.
The latest validator doctor reports 31 indexed rows and 23 disk account shards
on every validator, with aggregate and disk index paths redacted and usable.
