# Privacy Observer Inference Model

The observer-inference packet closes a narrow gap in the privacy argument:
candidate count \(k\) is not asserted by the paper. It is derived by
intersecting the candidate partitions left visible to a declared observer.

This is controlled-testnet evidence only. It does not mutate registry state and
does not transfer authority.

## Model

The verifier takes the observer's declared view:

| Channel |
| --- |
| `timing_bucket` |
| `fee_class` |
| `rpc_observer` |
| `disclosure_hash` |
| `asset_policy_partition` |
| `offchain_side_information` |

Each channel contains a measured partition: the candidate shielded actions that
remain plausible after that channel is observed. The verifier computes:

```text
k = |timing_bucket
     ∩ fee_class
     ∩ rpc_observer
     ∩ disclosure_hash
     ∩ asset_policy_partition
     ∩ offchain_side_information|

posterior_bps = ceil(10000 / k)
```

No independence assumption is used. If two channels are correlated, the
intersection gets smaller and the route changes.

```mermaid
flowchart TD
  Observer[Declared observer view]
  Partitions[Measured candidate partitions<br/>timing, fee, RPC, disclosure,<br/>asset policy, side information]
  Intersect[Intersect partitions<br/>no independence shortcut]
  K[Joint candidate count k]
  Posterior[posterior_bps = ceil(10000 / k)]
  Countermeasures[Countermeasures<br/>batching, delay, private relay,<br/>disclosure policy, downgrade]
  Route[Route decision]

  Observer --> Partitions --> Intersect --> K --> Posterior --> Route
  Posterior --> Countermeasures --> Route
```

## Bounds

| Bound | v1 value |
| --- | ---: |
| Minimum single-channel partition | 8 candidates |
| Minimum timing-bucket partition | 16 candidates |
| Minimum disclosure partition | 16 candidates |
| Minimum joint candidate count \(k\) | 16 candidates |
| Maximum joint posterior | 625 bps |
| Observation freshness | 3,600 seconds |

## Routes

| Condition | Route |
| --- | --- |
| All observer partitions intersect to \(k \ge 16\) | `baseline-private` |
| Joint candidate shortfall | `downgrade-privacy-claim` |
| Stale observation | `hold` |
| Direct RPC observer | `hold-for-private-relay` |
| Policy-root mismatch | `fail-closed` |
| Declared off-chain side information | `downgrade-privacy-claim` |
| Unique or low disclosure partition | `explicit-disclosure-required` |
| Timing partition shortfall | `hold-for-batching` |
| Missing observer channel | `fail-closed` |

## Fixture Coverage

The valid fixture derives \(k=16\) from the full six-channel intersection.
Negative fixtures and selector cases cover low joint partitions, stale
observations, direct RPC observation, policy-root mismatch, declared off-chain
side information, unique disclosure, timing shortfall, missing channels, root
mismatch, statement-hash mismatch, missing required cases, and verifier-claim
removal.

## Verification

```bash
scripts/privacy-observer-inference-model-verify --fixtures
scripts/privacy-observer-inference-model-verify --write-report
scripts/privacy-observer-inference-model-verify --verify-report
```

The canonical valid fixture is:

```text
docs/governance/agent/fixtures/privacy_observer_inference_model/valid_observer_inference_model.json
```

Current roots:

| Root | Value |
| --- | --- |
| Valid packet hash | `c54364f69198dced8780f5bab3b70677fb2656ff74734c13f648e4e27113c777567ca84c590ea64e85caaf254c9565cc` |
| Statement hash | `dabec86ff0680b8c26c04a98f6cb99d3d6d8ffa8d8a9895f8868fa30bd73e60d4d233dbd70b8d54af83aac684bfa1e5e` |
| Observer model root hash | `424cda0de92d9c148d032fb1e1f8771d6a4e1b79bd6e6d6724da837a6df1b0468046ec2bb4b7ddd4fbc4e47904fd1c00` |

## Status

The next implementation step is to feed observed partitions from wallet
batching, private-relay, disclosure-reuse, and per-asset pool telemetry into
the same route computation.
