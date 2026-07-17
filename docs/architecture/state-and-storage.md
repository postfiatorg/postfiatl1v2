# State And Storage

PostFiat stores enough data to verify current state, serve account history, and
replay evidence without forcing every validator to retain every byte forever.

## State Objects

- accounts and balances;
- blocks and block headers;
- receipts and transaction finality records;
- validator registry and governance roots;
- Orchard pool roots, commitments, nullifiers, and public telemetry;
- retained-history indexes for account transaction reads;
- snapshots and archive material.

```mermaid
flowchart TB
  Execution[Deterministic execution] --> StateRoot[Canonical state root]

  StateRoot --> Accounts[Transparent account map<br/>address, balance, sequence, flags]
  StateRoot --> Shielded[Shielded pool<br/>note commitment tree<br/>nullifier set<br/>Orchard roots]
  StateRoot --> Registry[Registry state<br/>validator-set root<br/>Cobalt trust-graph root<br/>amendment root<br/>evidence registry root]
  StateRoot --> Parameters[Governed parameters<br/>fee policy<br/>bridge policy<br/>privacy policy<br/>history-retention policy]

  Accounts --> Receipts[Execution receipts]
  Shielded --> PrivacyReceipts[Shielded validity receipts]
  Registry --> GovernanceReceipts[Governance amendment receipts]
  Parameters --> ReplayRules[Deterministic replay rules]
```

## Storage Layers

```mermaid
flowchart LR
  CertifiedBlocks[Certified block stream] --> BlockStore[Block store<br/>headers, payload hashes, certificates]
  CertifiedBlocks --> ReceiptStore[Receipts<br/>transaction results and finality records]
  CertifiedBlocks --> HistoryIndex[History index<br/>account transaction reads and retained windows]

  Execution[State transition] --> StateRootIndex[State root index<br/>height to root mapping]
  Execution --> OrchardPool[Orchard pool storage<br/>commitment tree, nullifier set, pool roots]
  Execution --> ValidatorRegistry[Validator registry storage<br/>active set, trust graph, Cobalt roots]

  StateRootIndex --> Snapshots[Snapshots<br/>export, import, restore evidence]
  BlockStore --> Archive[Archive material<br/>full replay and whitepaper evidence]
  ReceiptStore --> ReadRPC[Read RPC]
  HistoryIndex --> ReadRPC
  OrchardPool --> PrivacyReplay[Privacy replay and restore checks]
  ValidatorRegistry --> GovernanceReplay[Governance replay checks]
```

## Partial History

Validators can have history roles. Full archive behavior and retained-history
behavior are separated so ordinary validators can operate without unbounded
chain-size growth.

```mermaid
flowchart TD
  Chain[Certified chain and canonical state roots] --> FullArchive[Full archive node<br/>all blocks, receipts, reports, and historical indexes]
  Chain --> Retained[Retained-history validator<br/>current state plus configured recent history window]
  Chain --> Pruned[Pruned validator<br/>current state, root commitments, snapshots, and minimal proofs]

  FullArchive --> FullReplay[Full replay and Appendix A hash reconciliation]
  FullArchive --> HistoricalQueries[Long-range account and receipt queries]

  Retained --> OperationalRPC[Operational account history RPC within retention window]
  Retained --> ValidatorDuties[Validator duties without unbounded disk growth]

  Pruned --> ConsensusParticipation[Consensus participation from current state root]
  Pruned --> RestoreFromSnapshot[Restore from trusted snapshot plus certificate chain]
```

Important sources:

- `crates/storage/src/lib.rs`
- `crates/node/src/history.rs`
- `docs/runbooks/validator-history-retention.md`
- `docs/runbooks/account-tx-index.md`
- `docs/status/controlled-testnet-history-roles.json`

## Snapshot And Replay

Snapshot export/import is part of the operator evidence surface. Privacy
snapshot evidence verifies Orchard pool counters and roots after restore.
Governance replay evidence verifies Cobalt lifecycle and amendment bundles.
