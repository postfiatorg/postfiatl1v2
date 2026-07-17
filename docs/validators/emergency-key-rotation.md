# Emergency Key Rotation

Emergency key rotation is a rehearsed operator workflow.

## Goals

- rotate validator identity or operational keys without silent divergence;
- preserve governance replayability;
- record evidence;
- fail closed if the transition cannot be verified.

## Emergency Rotation Procedure

```mermaid
flowchart TD
  Detect[Detect key incident<br/>compromise, loss, or algorithm emergency]
  Freeze[Freeze or constrain affected authority<br/>operator and governance notice]
  Generate[Generate replacement key<br/>protect private material]
  Evidence[Build rotation evidence<br/>old key binding if available<br/>operator manifest<br/>incident scope]
  Gate[Governance gate<br/>Cobalt registry update<br/>safety and quorum checks]
  Activate[Activate new key at governed height]
  RejectOld[Reject old key after activation]
  Audit[Retain redaction-safe evidence<br/>and replay packet]
  Fail[Fail closed<br/>keep old registry or suspend validator]

  Detect --> Freeze --> Generate --> Evidence --> Gate
  Gate -->|accepted| Activate --> RejectOld --> Audit
  Gate -->|rejected or incomplete| Fail
```

## Source

- `docs/runbooks/validator-emergency-key-rotation.md`
- `scripts/testnet-remote-emergency-key-rotation-rehearsal`
- `reports/testnet-remote-emergency-key-rotation-rehearsal/`
