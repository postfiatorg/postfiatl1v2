# RPC Overview

PostFiat RPC is read-first for the controlled network. Write paths exist, but
they are bounded and explicit because public write exposure changes abuse and
operator-risk assumptions.

## RPC Surfaces

| Surface | Purpose |
| --- | --- |
| Read RPC | Status, ledger, fee, validators, blocks, receipts, transaction finality, account history, pool reports. |
| Controlled write path | Bounded operator-approved transaction submission for live evidence and wallet tests. |
| Local request-file path | Local node command path for building batches from prepared request files. |
| Privacy batch creation | Opt-in bounded Orchard batch creation with rate and concurrency limits. |

## Write And Read Paths

```mermaid
flowchart TD
  subgraph Write[Controlled write path]
    Submit[RPC submit signed transaction]
    Mempool[Mempool admission<br/>bounds, signature, fee, sequence]
    Batch[Batch builder<br/>payload hash and receipts domain]
    Order[Certified ordering<br/>proposal and votes]
    Certify[Quorum certificate<br/>block finality]
    Apply[Deterministic execution<br/>state root update]
  end

  subgraph Read[Read RPC path]
    Account[Account state]
    Finality[Transaction finality]
    History[Account history]
    Receipts[Receipts and block metadata]
  end

  Submit --> Mempool --> Batch --> Order --> Certify --> Apply
  Apply --> Account
  Apply --> Finality
  Apply --> History
  Apply --> Receipts
```

## Current Tooling

- `crates/node/src/rpc_cli.rs`
- `crates/rpc_sdk/src/lib.rs`
- `scripts/testnet-rpc-doctor`
- `scripts/testnet-rpc-method-inventory`
- `scripts/postfiat-rpc-account-tx`
- `python/postfiat_rpc/client.py`

## Read Next

- [Methods](methods.md)
- [Account History](account-history.md)
- [Write Policy](write-policy.md)
- [Examples](examples.md)
