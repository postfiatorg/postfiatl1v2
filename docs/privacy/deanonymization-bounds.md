# Privacy Deanonymization Bounds

The privacy metadata/anonymity-bound packet defines the fields and downgrade
routes. The deanonymization-bound packet adds the residual model behind those
floors.

The model is intentionally narrow. It assumes a public observer with the
declared metadata channels and no off-chain side information. If off-chain side
information, compromised wallet infrastructure, or direct third-party RPC
observation is present, the result downgrades or holds. The packet is
controlled-testnet evidence only; it does not mutate registry state and does
not transfer authority.

## Residual Model

All probabilities are represented as integer basis points. The verifier uses
the conservative single-target posterior bound:

```text
posterior_bps = ceil(10000 / candidate_count)
```

The baseline route requires:

| Bound | v1 value |
| --- | ---: |
| Maximum single-channel posterior | 1,250 bps |
| Maximum joint metadata posterior | 625 bps |
| Activity window candidates | 128 |
| Same fee-class candidates | 32 |
| Same asset / policy candidates | 16 |
| Same timing-bucket candidates | 16 |
| Batch size | 8 |
| Timing bucket | 300 seconds |
| Disclosure-hash reuse count | 16 |
| Joint metadata candidate count | 16 |
| Observation freshness | 3,600 seconds |

The batch-size floor gives the weakest single-channel bound: 1/8, or 1,250
bps. The baseline privacy route still requires a 16-candidate joint metadata
cohort, so combining timing, fee, asset-policy, disclosure, and admission
metadata must leave a posterior no worse than 1/16, or 625 bps. If the joint
cohort falls below that floor, the flow is not baseline-private even if every
single channel independently passes.

```mermaid
flowchart TD
  CandidateSet[Candidate shielded actions]
  Metadata[Declared metadata filters<br/>timing, fee class, asset policy,<br/>batch, disclosure, RPC]
  Residual[Residual candidate count]
  Posterior[Single-target posterior<br/>ceil(10000 / candidate_count)]
  Bounds[Deanonymization bounds<br/>single-channel and joint floors]
  Decision[Privacy route<br/>baseline, hold, downgrade,<br/>disclosure, fail closed]

  CandidateSet --> Metadata --> Residual --> Posterior --> Bounds --> Decision
```

## Routes

| Condition | Route |
| --- | --- |
| All floors and posterior bounds pass | `baseline-private` |
| Joint metadata cohort below 16 | `downgrade-privacy-claim` |
| Activity window below 128 | `downgrade-privacy-claim` |
| Same fee-class below 32 | `downgrade-privacy-claim` |
| Same asset / policy below 16 | `downgrade-privacy-claim` |
| Batch below 8 or timing bucket below floor | `hold-for-batching` |
| Unique disclosure hash | `explicit-disclosure-required` |
| Direct third-party RPC observer | `hold-for-private-relay` |
| Stale observation window | `hold` |
| Ungoverned bound policy | `fail-closed` |
| Declared external side information | `downgrade-privacy-claim` |

## Fixture Coverage

The valid fixture covers the baseline route. Negative fixtures verify:

| Fixture class | Expected failure |
| --- | --- |
| Root mismatch | bound-root mismatch |
| Statement hash mismatch | attestation mismatch |
| Joint candidate shortfall | baseline route rejected |
| Batch shortfall | hold-for-batching required |
| Timing shortfall | hold-for-batching required |
| Unique disclosure hash | explicit disclosure required |
| Direct third-party RPC | private relay required |
| Low activity window | privacy claim downgraded |
| Fee-class shortfall | privacy claim downgraded |
| Asset-policy shortfall | privacy claim downgraded |
| Ungoverned policy | fail closed |
| Stale observation | hold |
| External side information | privacy claim downgraded |

## Verification

```bash
scripts/privacy-deanonymization-bound-verify --fixtures
scripts/privacy-deanonymization-bound-verify --write-report
scripts/privacy-deanonymization-bound-verify --verify-report
```

The canonical valid fixture is:

```text
docs/governance/agent/fixtures/privacy_deanonymization_bound/valid_deanonymization_bound.json
```

Current roots:

| Root | Value |
| --- | --- |
| Valid packet hash | `5c949e3fa40f33a8854129086a09b9b8870476f38f79a28b99793ef1bf166225204950415103d71e5bdb5558ed5cba55` |
| Statement hash | `806fbb22ef07b55ee058956b0f6848ced03fbcf4376784dc998b6e9b16db3a83e77ad415f266ac7a2f7ff91c80f6752a` |
| Bound root hash | `0799e5536a40bb053f19d1d95c5e2cd0bca6c341610c23ca97b1181299762c3046f6e278fff81f219a395d3e57c8aec9` |

## Status

The next implementation step is to connect these bounds to real wallet
batching telemetry, pool activity telemetry, relay selection, and
shielded-asset policy gates.
