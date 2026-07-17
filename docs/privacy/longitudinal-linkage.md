# Privacy Longitudinal Linkage

The longitudinal-linkage packet extends the privacy floor model from one
observation to repeated observations and multi-target search.

This is controlled-testnet evidence only. It does not mutate registry state and
does not transfer authority.

## Model

The verifier does not multiply per-window probabilities. If a public observer
can link several windows, the remaining candidate set is the actual
intersection across those linked windows:

```text
k_long = |C_1 intersect C_2 intersect ... intersect C_n|
posterior_bps = ceil(10000 / k_long)
```

For multi-target search, the verifier uses a union bound:

```text
multi_target_bps = ceil(10000 * target_count / candidate_count)
```

The v1 fixture uses four linked windows and four target queries. To keep the
same 625 bps ceiling, the longitudinal intersection must stay at least 16 and
the four-target candidate set must stay at least 64.

```mermaid
flowchart LR
  Deposit[Deposit window<br/>public bridge or funding metadata]
  Spend[Shielded spend window<br/>nullifier timing and fee class]
  Withdraw[Withdraw window<br/>recipient envelope and timing]
  Linker[Observer linkage attempt<br/>intersect candidate sets across windows]
  KLong[k_long candidate set<br/>C1 intersect C2 intersect C3]
  Bound[Posterior bound<br/>ceil(10000 / k_long)]
  Route[Route decision<br/>baseline, hold for batching,<br/>downgrade, or disclosure]

  Deposit --> Linker
  Spend --> Linker
  Withdraw --> Linker
  Linker --> KLong --> Bound --> Route
```

## Routes

| Condition | Route |
| --- | --- |
| Longitudinal and multi-target bounds pass | `baseline-private` |
| Linked-window intersection shortfall | `downgrade-privacy-claim` |
| Multi-target union bound exceeds 625 bps | `downgrade-privacy-claim` |
| Repeated disclosure link | `explicit-disclosure-required` |
| Same exchange-side batch link | `downgrade-privacy-claim` |
| Recurring timing pattern | `hold-for-batching` |
| Direct RPC reuse | `hold-for-private-relay` |
| Thin asset-policy reuse | `downgrade-privacy-claim` |
| Declared side information | `downgrade-privacy-claim` |
| Ungoverned linkage policy | `fail-closed` |

## Fixture Coverage

The valid fixture leaves 16 candidates after intersecting four linked windows
and leaves 64 candidates for four target queries. Negative controls cover
linked-window shortfall, multi-target shortfall, repeated disclosure, same
exchange-side batch, recurring timing, direct RPC reuse, thin asset-policy
reuse, declared side information, ungoverned policy, root mismatch,
statement-hash mismatch, missing required cases, and verifier-claim removal.

## Verification

```bash
scripts/privacy-longitudinal-linkage-verify --fixtures
scripts/privacy-longitudinal-linkage-verify --write-report
scripts/privacy-longitudinal-linkage-verify --verify-report
```

The canonical valid fixture is:

```text
docs/governance/agent/fixtures/privacy_longitudinal_linkage/valid_longitudinal_linkage.json
```

Current roots:

| Root | Value |
| --- | --- |
| Valid packet hash | `b5feba5894ff8d01821cf32316a85571dcc1fadc0c8e5cb82244a296dd256d291cb947e38379bb97f7022d40c5e15bc0` |
| Statement hash | `c29e85ed4751d65a23f8957ec0e50766d021e0de45254484957bb7063973e38370ce82df554ee4a9b0a64c70c08a711d` |
| Linkage root hash | `dec51f066a643382a9f999fbee7029a7fdf1ef863158dc2db0c80818836ddc485ec700d1f24d583189503ccbabd1220b` |

## Status

The next implementation step is to replace fixture cohorts with public testnet
pool-flow linkage telemetry.
