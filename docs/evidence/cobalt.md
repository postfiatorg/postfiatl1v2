# Cobalt Evidence

Cobalt evidence proves bounded controlled-devnet protocol behavior. It does not
prove public operator decentralization, and every operational claim needs an
observation time plus an exact binary identity.

## Current Operational Observation

An authenticated, read-only validator/RPC probe ran from
`2026-08-26T01:40:51Z` through `01:41:04Z`. It established:

- all six validator and all six RPC services were active;
- every validator reported chain `postfiat-wan-devnet-2`, status `running`,
  height 919, zero pending mempool items, and identical tip/state roots; and
- every active validator binary had SHA-256
  `c7cb0c25001a0bfe22eba32ce870f3739f9710471906e27c32797670ea9f6337`
  under release `cobalt-verifier-92b63f5a` with embedded revision `92b63f5a`.

That probe did not inspect the shadow services or re-run the separate governance
auditor. The last authenticated full audit at `2026-08-25T15:37:40Z` returned
`ACTIVATED` on validator 0 with every `live-status` check passing and found all
six advisory shadows active under binary SHA-256
`43ac8a7df13f41d5cfdd783fc983de4e8e91b625e6a705ae342569c5771ad935`.
Cobalt remained limited to validator trust while Consensus v2 remained block
finality.

[Current State](../status/chain-state-current.md) contains the exact roots,
runtime/auditor/shadow identities, repository state, campaign status, probe
scope, and separate freshness boundaries.

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
not query the fleet. Its `source-pins.json` label for
`cobalt-verifier-92b63f5a` agrees with the later all-six process observation.
The packet is retained unchanged as historical evidence. Current operational
claims still require the newer observation and its capture time.

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
`9302b3555ab9091b2cae9b2d372d0548fe9f2fb1e67be43dfb3f63d89140b600`.
E3 ran on disposable clones and did not query or mutate the devnet.

E6 evidence is stored under
`benchmarks/cobalt-adversarial-verification/e6/`. It locks the design-only
decision to reinstate the independent-operator gate as its own mandatory
follow-on milestone. The packet's `SHA256SUMS.txt` hash is
`52a4ef91a7ec1c8dad385344edf3b498a96d45f5a8568d8c1371e3f3d6d05c81`.
It recruits no operators, authorizes no live migration, and does not prove
operator decentralization.

E4 evidence is stored under
`benchmarks/cobalt-adversarial-verification/e4/`. The final unchanged clean run
passed 500 baseline and 500 attack rounds with six-validator convergence at
height 501 in each lane, no Consensus v2 stop or fork, and a `+0.452099%`
attack-lane p95 finality delta inside the locked 5% budget. The packet preserves
two remediated harness-oracle receipts, full rejection and resource receipts,
and packet root
`93ba3db0bcc145144713088b612606fbb3b92c0f542809f258da49a555c14508`.
It was an isolated local campaign and did not query or mutate devnet.

E5 has not started. The milestone-wide `KEEP_ACTIVE` gate remains open.

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
