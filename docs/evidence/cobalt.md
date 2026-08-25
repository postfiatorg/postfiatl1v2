# Cobalt Evidence

Cobalt evidence proves bounded controlled-devnet protocol behavior. It does not
prove public operator decentralization, and every operational claim needs an
observation time plus an exact binary identity.

## Current Operational Observation

An authenticated, read-only probe completed at `2026-08-25T15:37:40Z`.
It established:

- all six validator, RPC, and Cobalt shadow services were active;
- every validator reported chain `postfiat-wan-devnet-2`, status `running`,
  height 919, zero pending mempool items, and identical tip/state roots;
- every active validator binary had SHA-256
  `431f194ba28391eba16c18a96d49c358bd2047d3b37eb0115216f46c6a6783f4`
  under release `cobalt-activation-8694b99d`;
- every shadow binary had SHA-256
  `43ac8a7df13f41d5cfdd783fc983de4e8e91b625e6a705ae342569c5771ad935`;
- the separate governance auditor, SHA-256
  `055077582342dd54af0212df82e626cc83aae8af119c09f1cd1309dad906293e`,
  returned `ACTIVATED` on validator-0 with every `live-status` check passing;
- Cobalt remained limited to validator trust, while Consensus v2 remained block
  finality and the shadow runtime remained advisory.

[Current State](../status/chain-state-current.md) contains the exact roots,
runtime/auditor/shadow identities, repository HEAD, campaign status, probe scope,
and freshness boundary. The fresh full authority audit was run on validator-0;
all-six convergence was independently checked from each validator's status,
service state, and binary hash.

## Historical Activation Packet

The checksum-bound activation artifact is
`benchmarks/cobalt-activation-live/packet/`. It captured the initial height-919
observation at `2026-08-25T05:13:45Z`, including activation at height 916, the
validator-5 key rotation at height 917, and packet manifest root
`b603b59d0245a7c73e766d0ba7fb19975f11e1e39bdd7263bf87e65250438bfb`.

```bash
python3 benchmarks/cobalt-activation-live/packet/verify_packet.py
```

A pass authenticates the committed packet and its selected source pins; it does
not query the fleet. Its `source-pins.json` labels
`cobalt-verifier-92b63f5a` as the live consensus binary, but that label does not
match the active `cobalt-activation-8694b99d` validator services observed by the
fresh probe. The packet is retained unchanged as historical evidence. Current
operational claims must use the fresh runtime identities above.

## Post-Activation Adversarial Evidence

E1 evidence is stored under
`benchmarks/cobalt-adversarial-verification/e1/`. The independent oracle,
production comparison, reconciled rerun, and clean rerun agree across all 10,240
generated trust graphs. E1 is off-chain verification tooling; it is not a node
deployment.

E2 evidence is stored under
`benchmarks/cobalt-adversarial-verification/e2/`. The frozen source revision
`15ef2307732cf46ff3b921bf02f3ad096dda15f3` derives `f=1` from the pinned
six-validator topology. The first run passed all 108 validator/strategy cases
and 442,368 schedules without remediation; the clean rerun has the same
classification SHA-256, and the signed-evidence verifier passes. The packet's
`SHA256SUMS.txt` hash is
`8742d9603621408339d99c3d9fcc1ba8cc43dafdc900acdfccbf86cc60d7cba3`.

E2 uses simulation ML-DSA identities and production signed-message/transcript
validation. It did not connect to or mutate the devnet, does not allege live
operator misbehavior, and does not prove operator decentralization.

E3 evidence is stored under
`benchmarks/cobalt-adversarial-verification/e3/`. All 24 durable-history tamper
cases and 18 forged catch-up cases rejected with named reasons and zero durable
mutation. All six interrupted recoveries restored byte-identical accepted
history without manual repair. The packet's `SHA256SUMS.txt` hash is
`bbab4cab822a17eaaad6a621740b65288cb0caad872e04326223d349a5a61372`.
E3 ran on disposable clones and did not query or mutate the devnet.

E6 evidence is stored under
`benchmarks/cobalt-adversarial-verification/e6/`. It locks the design-only
decision to reinstate the independent-operator gate as its own mandatory
follow-on milestone. The packet's `SHA256SUMS.txt` hash is
`52a4ef91a7ec1c8dad385344edf3b498a96d45f5a8568d8c1371e3f3d6d05c81`.
It recruits no operators, authorizes no live migration, and does not prove
operator decentralization.

E4 remains in progress and E5 has not started. The first E4 frozen run passed
all 500 baseline rounds but stopped before attack round 1 when the harness retry
window was shorter than the locked validator restart. The redaction-safe
failure receipt is under
`benchmarks/cobalt-adversarial-verification/e4/remediation/`; the corpus was not
changed, a focused full-vote crash/restart check passed, and the required clean
500+500 rerun is in progress. The milestone-wide `KEEP_ACTIVE` gate remains
open.

## Historical And Supporting Evidence

These artifacts remain useful qualification and regression evidence, but they
predate the terminal activation:

- `reports/testnet-cobalt-controlled-readiness-gate/amendment-replay-contract-clean-v0-20260519T145213Z/testnet-cobalt-controlled-readiness-gate.json`
- `reports/cobalt-safety-witness/20260526/cobalt-safety-witness-report.json`
- `reports/testnet-cobalt-gate-selection/amendment-replay-contract-clean-v0-20260519T145213Z/testnet-cobalt-gate-selection-self-test.json`
- `reports/testnet-cobalt-amendment-replay-bundle/cleanup-clean-v1-20260519T150324Z/testnet-cobalt-amendment-replay-bundle.json`
- `reports/testnet-cobalt-adversarial/`
- `reports/testnet-cobalt-controlled-launch-gate/strict-expected-fail-clean-head-v0-20260519T1438Z/testnet-cobalt-strict-launch-expected-fail.json`
- `reports/cobalt-cover-extractor-v1-report.json`
- `reports/cobalt-cover-sizing-v1-report.json`

These prove bounded readiness and regression properties. They do not establish
the current network tip or replace a fresh fleet probe.
