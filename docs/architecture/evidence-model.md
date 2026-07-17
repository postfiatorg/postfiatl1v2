# Evidence Model

PostFiat treats evidence as a first-class engineering output.

## Evidence Types

| Type | Purpose |
| --- | --- |
| Smoke report | Proves a focused local or live behavior. |
| Gate report | Aggregates required checks and fails closed. |
| Replay packet | Proves deterministic verification of prior state or governance. |
| Doctor report | Gives operator health and remediation information. |
| Audit packet | Curates redaction-safe proof for a whole workstream. |

## Rule

If a docs page says a major capability exists, it should cite at least one of:

- code path;
- script;
- test;
- redaction-safe report;
- runbook.

## Hash-Bound Artifact Flow

```mermaid
flowchart LR
  Produce[Evidence produced<br/>test, smoke, gate, replay, or audit packet] --> Canonicalize[Canonicalize artifact bytes]
  Canonicalize --> Root[Compute SHA3-384 root]
  Root --> Register[Evidence register<br/>artifact path, root, timestamp, scope]
  Register --> Citation[Whitepaper or docs citation<br/>Appendix A [E#] plus hash]
  Citation --> Fetch[Verifier fetches archived artifact]
  Fetch --> Rehash[Recompute SHA3-384 root]
  Rehash --> Match{Hash matches citation?}
  Match -->|yes| Accepted[Evidence accepted for the cited claim]
  Match -->|no| Reject[Reject or investigate provenance drift]
```

## Evidence Layering

```mermaid
flowchart TB
  Claim[Protocol capability claim] --> CitationRule[Citation rule<br/>major claims need code, script, test, report, or runbook evidence]

  Code[Code path<br/>deterministic implementation anchor] --> CitationRule
  Script[Operational script<br/>repeatable command surface] --> CitationRule
  Test[Test or simulation<br/>bounded adversarial behavior] --> CitationRule
  Report[Redaction-safe report<br/>machine-readable observed result] --> CitationRule
  Runbook[Runbook<br/>operator procedure and remediation] --> CitationRule

  CitationRule --> EvidencePacket[Hash-bound evidence packet]
  EvidencePacket --> EvidenceRegister[Evidence register]
  EvidenceRegister --> WhitepaperRef[Whitepaper Appendix A citation by [E#]]
  WhitepaperRef --> Verification[Hash-match verification before relying on the claim]
```

## Current Evidence Index

Use [Evidence](../evidence/index.md) as the curated front door. Do not point a
new reader at the raw `reports/` directory without context.
