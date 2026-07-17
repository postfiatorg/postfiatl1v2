# Validator Operations

Day-two validator work is about keeping services healthy and making recovery
boring.

## Routine Checks

- service active;
- height convergence;
- state root convergence;
- RPC read response;
- account-history index readiness;
- Orchard public pool counters when privacy is enabled;
- disk usage and history retention;
- latest launch/evidence revision.

## Operational Duties And Block Production

```mermaid
flowchart TD
  Health[Routine health checks<br/>service, disk, keys, RPC]
  Registry[Load active registry<br/>quorum set and trust roots]
  Mempool[Accept bounded transactions<br/>mempool and batch validation]
  Propose[Leader proposes block or batch]
  Verify[Validators verify payload<br/>state transition, signatures, fees]
  Vote[Sign vote<br/>one canonical vote per height/view]
  Cert[Quorum certificate]
  Commit[Commit certified block<br/>persist state root and receipts]
  Observe[Publish status and monitor signals]

  Health --> Registry --> Mempool --> Propose --> Verify --> Vote --> Cert --> Commit --> Observe
  Observe --> Health
```

## Drills

- restart;
- partial outage;
- below-quorum no-advance and recovery;
- snapshot export/import;
- RPC read-load;
- RPC edge-load;
- emergency key rotation rehearsal.

## Source

- `docs/runbooks/operator-day-two.md`
- `docs/runbooks/validator-doctor.md`
- `docs/runbooks/validator-history-retention.md`
