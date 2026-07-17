# Transaction Lifecycle

Transparent transfers follow a direct path from wallet signature to finality
receipt.

```mermaid
sequenceDiagram
  participant W as Wallet
  participant R as RPC / write edge
  participant P as Proposer
  participant V as Validators
  participant S as State
  participant Q as Read RPC

  W->>R: signed transfer
  R->>P: queued transaction
  P->>V: proposed block or batch
  V->>V: verify transaction and proposal
  V-->>P: votes
  P->>S: quorum certificate and block
  S->>S: apply, burn fees, write receipt
  W->>Q: tx finality query
  Q-->>W: finality proof and receipt
```

## Steps

1. The wallet signs a transaction using post-quantum account authorization.
2. The transaction enters a controlled write path or validator mempool.
3. A proposer forms a block or batch.
4. Validators verify the proposal and vote.
5. Quorum votes form a certificate.
6. The node commits the block, advances the state root, burns fees, and writes
   receipts.
7. Clients use read RPC to query `tx`, account state, receipts, and account
   history.

## Source Anchors

- `crates/types/src/lib.rs`
- `crates/execution/src/lib.rs`
- `crates/node/src/lib.rs`
- `crates/node/src/block_finality.rs`
- `crates/rpc_sdk/src/lib.rs`
