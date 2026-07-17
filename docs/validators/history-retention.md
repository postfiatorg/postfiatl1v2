# History Retention

Validators should not need to retain infinite history to validate current state.
PostFiat has explicit history roles and account-history indexing.

## Current Concepts

- retained block and receipt history;
- account transaction index;
- partial-history validation;
- archive/export roles;
- snapshot import/export;
- index status reporting.

## Retention Roles And Transitions

```mermaid
flowchart TD
  Full[Full archive role<br/>all blocks, receipts,<br/>history indexes, replay evidence]
  Retained[Retained-history role<br/>current state plus configured<br/>recent block and receipt window]
  Pruned[Pruned role<br/>current state root,<br/>snapshots, minimal proof material]

  Full -->|operator changes retention policy| Retained
  Retained -->|storage pressure or policy change| Pruned
  Pruned -->|import archive or replay from snapshot| Retained
  Retained -->|promote with archive backfill| Full

  Full --> ArchiveQueries[Long-range account history<br/>and Appendix A hash reconciliation]
  Retained --> LocalQueries[Recent account history<br/>and validator operations]
  Pruned --> Consensus[Consensus participation<br/>from current state and certificates]
```

## Source

- `docs/runbooks/validator-history-retention.md`
- `docs/runbooks/account-tx-index.md`
- `crates/node/src/history.rs`
- `crates/storage/src/lib.rs`
