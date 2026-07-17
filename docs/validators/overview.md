# Validator Overview

Validators are known infrastructure operators. They verify transactions, vote on
blocks, publish certificates, serve read RPC, retain role-appropriate history,
and participate in Cobalt governance.

## Operator Responsibilities

- run validator and RPC services;
- keep keys and private launch material secure;
- monitor state, height, service health, and account-history indexes;
- participate in restart, outage, and recovery drills;
- preserve enough history for the configured role;
- handle emergency key rotation through the runbook.

## Operation Flow

```mermaid
flowchart TD
  Startup[Startup<br/>load config, data dir, and keys]
  Registry[Load active registry<br/>validator set<br/>trust graph root<br/>governed parameters]
  Peers[Peer discovery<br/>connect to configured topology<br/>exchange height and root]
  Consensus[Consensus loop<br/>propose, validate, vote,<br/>collect certificates]
  Commit[Block commit<br/>apply deterministic execution<br/>persist state root and receipts]
  Serve[Serve read RPC<br/>status, account history,<br/>receipts, finality records]
  Monitor[Operator monitoring<br/>height, root, service health,<br/>history retention]

  Startup --> Registry --> Peers --> Consensus --> Commit --> Serve --> Monitor
  Monitor -->|healthy| Consensus
  Monitor -->|fault or outage| Recovery[Restart, restore, or escalation runbook]
  Recovery --> Startup
```

## Key Rotation Flow

```mermaid
sequenceDiagram
  participant O as Operator
  participant Old as Old validator key
  participant New as New validator key
  participant C as Cobalt governance
  participant R as Validator registry
  participant V as Peers

  O->>New: Generate and protect new key material
  O->>Old: Sign key-continuity evidence
  O->>C: Submit registry update packet with old-new binding
  C->>C: Verify evidence, safety rules, and quorum certificate
  C->>R: Activate new key at governed height
  R->>V: Publish updated registry root
  New->>V: Sign proposals and votes after activation
  Old--xV: Old key rejected after activation height
```

## Current Tooling

- `scripts/testnet-validator-doctor-smoke`
- `scripts/testnet-live-validator-doctor`
- `scripts/testnet-monitor-snapshot-smoke`
- `scripts/testnet-remote-restart-drill`
- `scripts/testnet-remote-snapshot-drill`
- `scripts/testnet-remote-emergency-key-rotation-rehearsal`
