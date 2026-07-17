# Latency Evidence

Latency evidence is attached to a specific workload, validator count, and
controlled environment.

## Current Numbers

| Evidence | Result |
| --- | --- |
| Local 5-validator 20-round benchmark | submit-to-finality p50 1.563s, p95 1.709s, p99 1.753s |
| Remote 5-validator normal-run smoke | certified round p50 1.032s, p95 1.116s, p99 1.139s |

## Evidence Paths

- `reports/testnet-tx-finality-latency-benchmark/local-gate-20round-20260514T142731Z/testnet-tx-finality-latency-benchmark.json`
- `reports/testnet-remote-ssh-smoke/optimized-latency-20260514T143534Z/testnet-remote-ssh-smoke.json`

## Note

These are controlled finality numbers. Public TPS claims need separate benchmark
packets with workload, hardware, validator count, RPC posture, and measurement
conditions attached.
