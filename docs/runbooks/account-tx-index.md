# Account Tx Index

Status: v0 auto-refreshed retained-history index
Date: 2026-05-16

`account_tx` is a read-only public RPC method for transparent account history.
It now prefers disk-backed per-account shards when `account_tx_index_meta.json`
matches the local chain/genesis/protocol/tip. If the disk index is absent,
stale, or unreadable, the node falls back to the aggregate
`account_tx_index.json`; if that is also absent or stale, it falls back to the
existing bounded retained-history scan instead of failing the public read.

This index is not consensus state. It is an operator cache rebuilt from local
block/archive/receipt data. Ordered block commit refreshes it automatically on
a best-effort basis; a cache refresh failure does not block commit, and stale
or unusable aggregate/disk status remains visible through
`account_tx_index_status`.

The commit-time refresh first tries a bounded incremental append when the
existing cache tip is still a known ancestor of the new local tip and matches
the same chain/genesis/protocol. If that proof does not hold, the node falls
back to the full retained-history rebuild path.

## Build

```bash
target/debug/postfiat-node account-tx-index-build \
  --data-dir /var/lib/postfiat/validator-0
```

The local build report includes the aggregate and disk metadata paths, indexed
height range, transparent row count, account count, shard count, tip hash,
`index_usable`, and `disk_index_usable`.

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
- Make the aggregate JSON index optional/metadata-only once the disk-shard path
  has enough soak.

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
