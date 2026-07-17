# XRPL Private Control Release Latency Status

Date: 2026-06-06 UTC
Status: final local 100-round evidence packet captured
Scope: matched local private benchmark comparison between stock/modified
`rippled` and PostFiat L1 v2

## Bottom Line

The current performance result is now captured as a single final evidence
packet:

```text
reports/xrpl-private-control/final-local-6v-100rounds-20260606T192422Z/
```

Claim supported by that packet:

```text
In a matched local private 6-validator, 100-round sequential native-transfer
benchmark, fast-timing private rippled p50 submit-to-validated latency was
3.35x PostFiat L1 v2 release full-vote submit-to-finality latency, and stock
private rippled p50 latency was 17.60x PostFiat latency. Equivalently,
PostFiat was 70.2% lower than fast-timing private rippled and 94.3% lower than
stock private rippled at p50. At p95, the control/PFT ratios were 2.89x and
13.04x.
```

This is a local private-topology benchmark. It is not a public XRPL
mainnet/testnet claim, a WAN-latency claim, or a decentralization claim.

## Final Evidence Packet

Packet README:

```text
reports/xrpl-private-control/final-local-6v-100rounds-20260606T192422Z/README.md
```

Machine-readable summary:

```text
reports/xrpl-private-control/final-local-6v-100rounds-20260606T192422Z/summary.json
```

Raw reports:

```text
reports/xrpl-private-control/final-local-6v-100rounds-20260606T192422Z/raw/postfiatl1v2-release-fullvotes/testnet-tx-finality-latency-benchmark.json
reports/xrpl-private-control/final-local-6v-100rounds-20260606T192422Z/raw/xrpl-stock/xrpl-private-control-benchmark.json
reports/xrpl-private-control/final-local-6v-100rounds-20260606T192422Z/raw/xrpl-fasttiming/xrpl-private-control-benchmark.json
```

Hash manifest:

```text
reports/xrpl-private-control/final-local-6v-100rounds-20260606T192422Z/SHA256SUMS.txt
```

Exact commands:

```text
reports/xrpl-private-control/final-local-6v-100rounds-20260606T192422Z/commands.sh
```

Metadata:

```text
reports/xrpl-private-control/final-local-6v-100rounds-20260606T192422Z/metadata.json
```

Packet runner:

```text
scripts/xrpl-pft-local-final-evidence-packet
```

## Results

All rows are 6 validators and 100 measured rounds on local private networks.

| Run | Build / policy | p50 ms | p95 ms | p99 ms | Mean ms |
|---|---|---:|---:|---:|---:|
| Stock `rippled` private XRPL | `rippled` 3.1.3 stock ledger timing | 3003.195 | 3055.146 | 3064.787 | 3031.699 |
| Fast-timing `rippled` private XRPL | same 3.1.3 commit, reduced local timing constants | 572.110 | 676.252 | 985.277 | 524.292 |
| PostFiat L1 v2 | release, full-vote finality, quorum-early disabled | 170.625 | 234.300 | 243.327 | 172.206 |

Ratios:

| Metric | Fast-timing `rippled` / PostFiat | Stock `rippled` / PostFiat |
|---|---:|---:|
| p50 | 3.35x | 17.60x |
| p95 | 2.89x | 13.04x |
| p99 | 4.05x | 12.60x |
| mean | 3.04x | 17.61x |

## Build Metadata

PostFiat L1 v2 commit captured in metadata:

```text
c13113817602093f1bf3bae00c957e8c7e6b0418
```

The PostFiat worktree was dirty at measurement time; `metadata.json` records
the exact `git status --short` output.

`rippled` commit for both controls:

```text
46b241ace8b30d9c9775d60ffba7d24b21903896
```

Preserved control binary hashes:

```text
stock rippled:       c67d10b48bd6a2e62cc33a0771d4428f6786e0b05a71e9febb180ac1af438bf3
fast-timing rippled: adf438b16daa6da59da78ffbdc8aa4af54813ebe291c1ce0124778abfec97d2e
```

## Safety Boundary

The performance packet is backed by the local adversarial finality gate:

```text
docs/status/adversarial-finality-gate-2026-06-06.md
reports/testnet-finality-chaos-gate/run-20260606T190718Z/testnet-finality-chaos-gate.json
sha256 07d97d1564d7f0463c1ea86a59ae6c2502be9d7415d193963a7f29b996d001dd
```

That gate supports the narrower safety statement that the fast path has passed
a focused local adversarial finality suite, including duplicate vote refusal,
stale certificate/vote rejection, under-quorum partition rejection, Byzantine
disjoint-proposal rejection, process restart persistence, delayed-vote retry,
and malformed transport/certified-batch rejection.

The safety gate does not create the performance result. It supports the claim
that the benchmarked path is not merely fast because local adversarial finality
checks were skipped.

## Interpretation

The stock `rippled` row shows public-network-oriented default ledger timing.
The fast-timing `rippled` row shows that much of stock XRPL's local latency is
deliberate timer policy rather than raw implementation slowness. PostFiat is
still lower-latency than the fast-timing control in this matched local
benchmark because it finalizes a certified batch directly instead of waiting
for XRPL's ledger-close cadence.

## Superseded Evidence

Older 25-round packets under `reports/xrpl-private-control/` are useful history
but are no longer the headline performance result. Use the final 100-round
packet above for current claims.

## Next Step For Stronger Claims

A stronger public-network-style claim requires a remote matched topology:
same host class, same regions, same validator count, same client location, same
transaction count, same workload, and private `rippled` vs PostFiat L1 v2
clusters run side by side.
