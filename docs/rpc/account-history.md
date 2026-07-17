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

## Evidence

- `docs/runbooks/account-tx-index.md`
- `scripts/postfiat-rpc-account-tx`
- `reports/testnet-six-wallet-account-tx-smoke/`
- `reports/testnet-account-tx-disk-index-smoke/`
