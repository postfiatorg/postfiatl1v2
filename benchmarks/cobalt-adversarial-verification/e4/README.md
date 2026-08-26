# Cobalt E4: Finality Isolation Under Governance Stress

## Result

**PASS.** This checksum-bound packet records an isolated local six-validator
Consensus v2 campaign. It is not a devnet probe or deployment receipt.

The unchanged campaign ran 500 baseline rounds and 500 attack-lane rounds from
the same signed initial state, topology, binaries, full-vote policy, and CPU
allocation. All six validators converged independently in both lanes at height
501. Consensus v2 never stopped or forked, all 1,000 client submissions finalized,
and the attack-lane p95 finality regression was `+0.4520990899943289%`, within
the locked 5% budget.

Cobalt was active only as validator-trust governance during the attack lane.
Consensus v2 remained responsible for block finality. This result proves protocol
capability, not operator decentralization, provider diversity, WAN behavior, or
mainnet readiness.

## Source and binaries

- Frozen remediated campaign source: `451c2ad0e924f8be72feeac69c1356b3828a4f58`
- Executed comparator source: `add07a7cce416daeaa61073085734937477f2b71`
- `postfiat-node` SHA-256: `634f08368c174a288bfc42211dc52ef0725c7f6933acc816e4a9006606189a41`
- Cobalt liveness simulation SHA-256: `6bef2df8a2ef18c11c774309713c878470f54819b98e15face09a9f9ffa62028`
- Frozen campaign manifest SHA-256: `838a0bccda40f13c6f999fd119706739d9384509bc9495165e0cd6f04fc4c68d`

## Measured outcome

| Measure | Baseline | Attack |
| --- | ---: | ---: |
| Consensus rounds | 500 | 500 |
| Final height | 501 | 501 |
| Wallet-to-finality p50 | 7,471.082586 ms | 7,500.377266 ms |
| Wallet-to-finality p95 | 14,133.573682 ms | 14,197.471440 ms |
| Resource samples | 43,989 | 44,412 |
| Validator CPU ticks | 843,095 | 848,487 |
| Peak validator RSS | 3,031,264 KiB | 3,031,788 KiB |
| Network received | 1,148,906,329 bytes | 5,605,077,560 bytes |
| Network transmitted | 1,149,482,815 bytes | 5,576,139,957 bytes |
| Node disk delta | 1,956,792,954 bytes | 1,956,792,954 bytes |
| Validator writes | 2,108,579,840 bytes | 2,107,133,952 bytes |

The attack lane completed 47 governance-stress runs covering 940 proposals,
329 safe halts, and 329 view changes. It recorded 987 boundary rejections,
846 named limit rejections, and 752 flood rejections while preserving durable
state. Validator 5 restarted automatically 12 times; no manual operator action
was required.

The packet records named behavior at the 1 MiB decision-certificate and 2 MiB
RPC-frame boundaries. Near-limit malformed inputs rejected for their structural
or protocol defect; oversized inputs rejected at the size boundary.

## Harness remediation history

Two redaction-safe receipts preserve failures found before this passing rerun:

1. The initial harness retry window ended before the deliberately restarted
   validator became available. The transport retry window was lengthened and a
   focused restart/convergence check passed. The campaign corpus did not change.
2. The next run passed both 500-round lanes, but the postprocessor incorrectly
   required identical tips and state roots across independent runs. Randomized
   ML-DSA transaction and consensus authentication changes transaction IDs,
   certificates, hashes, and roots between executions. The corrected oracle
   requires six-validator convergence inside each lane and compares the
   signed-message-independent workload and round outcomes across lanes. The
   corpus, topology, quota, binaries, crash cadence, and adversarial inputs did
   not change.

Cross-lane hash equality is therefore disclosed but is not a safety gate.

## Verify

From the repository root:

```bash
python3 benchmarks/cobalt-adversarial-verification/e4/verify_packet.py
```

The verifier checks the frozen manifest and source revisions, both remediation
receipts, 500 ordered rounds per lane, lane-local convergence, semantic workload
equality, latency calculations and budget, stress and rejection coverage,
resource receipts, checksums, and redaction.
