# Validator Launch

Validator launch is package-driven.

## Launch Flow

1. build release package;
2. generate operator-private artifact;
3. run launch prep check;
4. run remote join rehearsal;
5. install validator/RPC service pairs;
6. verify service activity;
7. verify state convergence;
8. run a certified round;
9. retain redaction-safe launch evidence.

## Launch Certificate Structure

```mermaid
flowchart LR
  subgraph Certificate[Launch certificate]
    Chain[chain id<br/>genesis hash<br/>activation height]
    Registry[genesis registry root<br/>validator identities<br/>initial quorum set]
    Trust[trust graph root<br/>checker root<br/>safety profile root]
    Evidence[evidence root<br/>release package hash<br/>operator manifest hash<br/>placement manifest hash]
    Signatures[ratifier signatures<br/>operator signature<br/>release-manager signature]
  end

  Certificate --> Verify[Launch verifier<br/>checks hashes, signatures,<br/>registry roots, and activation rules]
  Verify -->|valid| Install[Install validator and RPC service pair]
  Verify -->|invalid| Reject[Reject launch package]
```

## Launch Requirements

```mermaid
flowchart TD
  Package[Release package<br/>binary hash and config]
  Private[Operator-private artifact<br/>validator keys and service secrets]
  Registry[Registry entry<br/>identity, network address,<br/>public keys, quorum role]
  Placement[Placement evidence<br/>host, region, provider,<br/>operator domain]
  Health[Health checks<br/>service starts<br/>RPC reachable<br/>height advances]
  Convergence[Convergence checks<br/>same height<br/>same state root<br/>certified round succeeds]
  Evidence[Redaction-safe launch evidence<br/>hash-bound report retained]

  Package --> Certificate[Launch certificate]
  Private --> Certificate
  Registry --> Certificate
  Placement --> Certificate
  Certificate --> Health --> Convergence --> Evidence
```

## Source

- `docs/runbooks/controlled-testnet-operator-launch.md`
- `scripts/testnet-release-package`
- `scripts/testnet-controlled-launch-prep-check`
- `scripts/testnet-release-live-launch`
- `scripts/testnet-operator-launch-packet`
