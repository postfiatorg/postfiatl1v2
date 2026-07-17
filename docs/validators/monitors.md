# Monitors

Monitoring is covered by doctor reports and monitor snapshots.

## Monitor Snapshot

The monitor snapshot summarizes:

- endpoint health;
- height lag;
- validator service state;
- RPC method status;
- account-history index readiness;
- Orchard public pool counters when available;
- warnings and criticals.

## Monitoring Pipeline

```mermaid
flowchart LR
  Services[Validator and RPC services]
  Probes[Health probes<br/>process, port, RPC methods]
  Chain[Chain probes<br/>height, root, block tip,<br/>certificate freshness]
  Index[Data probes<br/>history index, receipts,<br/>Orchard pool counters]
  Snapshot[Monitor snapshot<br/>warnings and criticals]
  Doctor[Doctor report<br/>remediation hints]
  Operator[Operator action<br/>restart, resync, rotate key,<br/>or escalate]

  Services --> Probes
  Services --> Chain
  Services --> Index
  Probes --> Snapshot
  Chain --> Snapshot
  Index --> Snapshot
  Snapshot --> Doctor --> Operator
```

## Commands

```bash
scripts/testnet-validator-doctor-smoke
scripts/testnet-monitor-snapshot-smoke
scripts/testnet-rpc-doctor
```

## Evidence

- `reports/testnet-validator-doctor/`
- `reports/testnet-monitor-snapshot/`
- `reports/testnet-rpc-doctor/`
