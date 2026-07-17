# Performance Methodology Policy

The performance-methodology policy keeps whitepaper numbers in their lane. A
number can be:

- artifact-bound measurement from a named local or testnet run;
- derived sizing budget from parameter bytes or formulas;
- rejected as unsupported until a fresh benchmark packet exists.

This matters because a controlled-testnet number is useful only when its
boundary is visible. A local loopback latency run is not a WAN finality promise.
A certificate byte formula is not a verifier-throughput benchmark. A stale
throughput figure must be rebenchmarked before it becomes a fee constant.

## Current Packet

| Card | Status | Bound Claim |
| --- | --- | --- |
| `orchard-two-action-halo2-budget` | `supported` | Local release-build Orchard/Halo2 action verification: two actions, `7,264` proof bytes, cached verify repeats `[91, 80, 78]` ms, median `80` ms. |
| `local-loopback-submit-to-finality` | `supported` | Five-round local peer-certified loopback path: p50 submit-to-finality `1545.305443` ms. Not WAN, not production scheduler, not privacy proving. |
| `compact-certificate-size-metrics` | `supported` | Controlled-testnet certificate artifacts: 18 artifacts, 68 votes, max certificate file `28,851` bytes, compact votes, registry-root-bound. |
| `mldsa65-certificate-byte-budget` | `derived_budget` | ML-DSA-65 certificate bytes from key/signature constants and quorum size: `80,184` bytes for 35 validators / q=24, `223,847` bytes for 100 / q=67. |
| `mldsa65-throughput-claim-boundary` | `unsupported_rejected` | The old `~6,000 ML-DSA-65 verifications/sec` figure is not treated as measured unless a current artifact is supplied. It is a target or must be rebenchmarked. |

## Verification

Run from the repository root:

```bash
scripts/performance-methodology-policy-verify --fixtures
scripts/performance-methodology-policy-verify --write-report
scripts/performance-methodology-policy-verify --verify-report
```

Current fixture roots:

| Artifact | SHA3-384 |
| --- | --- |
| Valid packet | `c32b6cb8ecb037823b03f2b40223dc5caf9bb4244cdf91e2d20d55fd46ac71bdac78eb4775b693fcb6051dae76a519f2` |
| Statement | `df4ab9bcd201baa30b57df078aa476158d78264333f3009c4555943c285b4016ad903ee70528794e313fb9b3fec9a36e` |
| Methodology root | `bf8e6acbe48383d566bf4c7d9cde7650aadec78eb0dcaadddbbe07442c649be412469eb8a48e5eca8bb1e078fd9faadc` |

Current report:

- `reports/performance-methodology-policy/20260529T024012Z/performance-methodology-policy-report.json`

## Negative Fixtures

The suite rejects these failure modes:

- certificate-size artifact hash mismatch;
- local-loopback latency promoted to production/WAN latency;
- missing latency boundary;
- methodology-root mismatch;
- unsupported ML-DSA throughput marked as measured;
- Orchard value mismatch against the bound artifact;
- stale or missing artifact path.

## Status

This packet is a whitepaper-claim guardrail, not a live fee-constant gate. The
next implementation step is a fresh ML-DSA verifier-throughput benchmark using
the same artifact-bound methodology, then binding fee and block-budget constants
to that report instead of to prose.
