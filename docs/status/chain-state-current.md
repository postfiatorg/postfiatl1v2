# PostFiat L1 Current State

Documentation reconciled: `2026-09-07`. Status: **canonical dated operational-state reference**.

The latest storage deployment receipt reviewed in the pinned audit records
**transactional activation at height 930 and six-validator continuation at 931
on August 31**. Earlier height-924 rollout failures remain history. This page
records retained evidence; the documentation audit did not query the fleet,
reproduce deployment, or establish current network health.

## Latest retained storage observation

| Field | Recorded value |
| --- | --- |
| Source receipt | `deployments/storage-lease-20260831/deploy-receipt.json` |
| Receipt status/date | `DEPLOYED_AND_ACTIVE`, `2026-08-31` |
| Chain | `postfiat-wan-devnet-2` |
| Release | `storage-lease-af9b83c3` |
| Source revision | `af9b83c355267f18cd2b1b173b25fed57553a8ed` |
| Node binary SHA-256 | `383f4325a157f554786b6c8868defedcef8faeb320f9998eeef69c07c7141a7a` |
| Previous binary SHA-256 | `6b07a8c31ee5f306995e12df23d644348c3ab074beb68800f7251f3f38ef7de6` |
| Qualification reference | `benchmarks/storage-scaling/deployment-exact-gate/gate-926-receipt.json` |
| Activation | Height 930, all six transactional-active |
| Recorded continuation | Height 931, all six converged after both-service restart |
| Tip | `8e3639ee748255636d09adb8b3fe70f20f8ae0aa9388b024eaffb98b7a94f3c7c4b0e5710f0d8c2479b08d9bc8665c4b` |
| State root | `ef18f8ca8eee67e4f25e0752b54012f1e66b8b4de16444d2583a81e890aec1f3492c1544280279e5acf090559fd55e0f` |
| Commitment | `postfiat.replicated_state.v2` |
| Full-history scans reported | 0 in the receipt's final state; not a universal performance proof |
| Z1 clock started | `2026-08-31T04:29:41Z`; start is not completion |

The recorded sequence is a legacy transfer at 927, scheduled activation at 928,
a preactivation transfer at 929, cutover at 930, restart of both services, then
transactional continuation at 931. The receipt also discloses rollout incidents,
including manifest-install ordering, interrupted rebuild sessions, directory
ownership, and a certified-round CLI returning nonzero after a successful commit.
Its verified-commit claim must not be replaced with an inference from exit status.

A retained receipt establishes the recorded observation and its stated scope.
Re-probe service state, binaries, chain identity, tip/roots and authority before
making a later “running now” claim. The audit baseline
`d351353e57b295368450a57866ace17b5e1ce6ad` is a source-review identity, not the
receipt's deployed binary identity.

## Authority and evidence planes

| Plane | Supported statement | Limit |
| --- | --- | --- |
| Block finality | Consensus v2 prepare/precommit remains the block-finality protocol when activated. | Cobalt does not finalize blocks. |
| Validator trust | The retained Cobalt campaign records active bounded validator-trust authority through its final height-924 drills. | The storage receipt is not a fresh independent audit of every governance root. |
| Proposals/operators | The campaign used Foundation-administered validators and custody. | No public operator decentralization follows from six processes or correct certificates. |
| Storage | A distinct later release records activation and continuation at 930/931. | Earlier failed candidates remain disqualified; arbitrary descendants are not deployment-qualified. |
| Public-testnet gates | The existing storage/testnet journals retain unresolved gates and operator decisions. | Deployment or an observation-clock start does not close all gates. |
| Source | Current code can implement more than the last deployed binary. | Source availability is not evidence a feature is running or activated. |

## Historical rollout sequence

| Date/height | Event | Retained reference |
| --- | --- | --- |
| August 26, through 924 | Cobalt final E5 drills and six-validator convergence; original node binary `d5e5ef63…c2696caf`. | [E5 packet](https://github.com/postfiatorg/postfiatl1v2/blob/d351353e57b295368450a57866ace17b5e1ce6ad/benchmarks/cobalt-adversarial-verification/e5/README.md) |
| August 30, 924 | Candidate `d0ae79f3` rebuilt/verified six clones but failed first continuation on superseded registry history. | [G6 rehearsal stop](../postmortems/devnet-storage-g6-rehearsal-stop-2026-08-30.md) |
| August 30, 924 | Successor `10dd9f20` repaired that defect; the old clone runner omitted concurrent transport/RPC. Live canary hit the exclusive database lock and was rolled back. | [Canary rollback](../postmortems/devnet-storage-live-canary-rollback-2026-08-30.md) |
| August 31, 925–926 | `registry-fix-291d1eb1` backported the registry-history repair to the old deployed lineage and continued the chain. | `deployments/registry-fix-20260831/deploy-receipt.json`; [wedge postmortem](../postmortems/devnet-registry-continuation-wedge-2026-08-31.md) |
| August 31, 927–931 | Separate `storage-lease-af9b83c3` rollout, governed activation and post-restart continuation. | `deployments/storage-lease-20260831/deploy-receipt.json` |

The former “no candidate is deployment-eligible” summary described the August 30
candidates. It must not be used to erase the subsequent distinct rollout. The
[active storage milestone](../plans/active/storage-scaling-milestone.md) remains
open for explicit gate reconciliation rather than silently changing every old
unchecked item to complete.

## Accepted Cobalt drill history

| Height | Action | Proposer | Authorizers | Recorded result |
| ---: | --- | --- | --- | --- |
| 920 | Initial rollback to Foundation | validator-2 | validators 0–4 | Accepted remediation history. |
| 921 | Initial return to Cobalt | validator-3 | validators 0–4 | Accepted; trust binding did not match the protocol-native post-return graph, so not the final gate. |
| 922 | Corrective rollback | validator-4 | validators 0–4 | Accepted final-gate rollback. |
| 923 | Corrective return | validator-5 | validators 0–4 | Accepted with the correct trust binding. |
| 924 | Legitimate validator-5 key rotation | validator-0 | validators 0–4 | Accepted; old/stolen validator-5 key was not an authorizer. |

The E5 packet records all six validators sharing that history, each block with
at least five Consensus v2 votes, and nine negative cases rejecting without
durable governance mutation. It preserves both the first pair and its correction.
The campaign's final authenticated observation ran
`2026-08-26T06:34:55Z`–`06:35:50Z`; the later post-canary rollback probe ran
`2026-08-30T23:00:24Z`–`23:00:39Z`. Neither is a live query now.

Historical height-924 identities:

| Field | Value |
| --- | --- |
| Genesis | `ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9` |
| Tip | `ebeb0e1ee27f30ba480255728832719d94eac1a89d762a7aa7019eae269008fac53098cf6495f477a241d63a7649fbef` |
| State root | `0854bc47f78996b2dcd279206cbdcc0b4858395c5937e0e0d56b3d645ca6b6a9d9c9578f5ac77bb14bea9dd1ee6f413e` |
| Registry root | `08a451e07aeaf9ada41a69e7c26dfd3fd86fce11c02f5567127c598b3cf775ac054b2add85295cc8c0d429bb6d2b9b1d` |
| Trust root | `89f18aef2c5726ae43043407eb4d638ee8f3b6027e58ec3553296478602232cf3c2fc5d1dfebc4058d720b16508f0307` |
| Original node binary SHA-256 | `d5e5ef630155e61b001b84edb404a4def7d29a9205f23d33d2ad9c37c2696caf` |

## Performance and remaining evidence limits

E4's first 50 baseline rounds recorded `consensus_round_ms` p95 1,664 ms, compared
with 1,660 ms in the activation run. The old lineage then showed nearly linear
height amplification to about 14.9 seconds at round 500. These are historical
measurements, not current service commitments. Later source work addressed
history scans, certified-send indexes, registry replay and transactional writer
coordination; the retained rollout is not a substitute for every separate
performance/replay/public-testnet gate.

The height-924 finalized-checkpoint snapshot-export issue, public-testnet gate
reconciliation and later health require their own closure evidence. Pending
cleanup items in the storage receipt also do not become complete by elapsed time.
See the [testnet path](testnet-path.md),
[adversarial results](../governance/cobalt-adversarial-verification-results.md),
and [whitepaper gap G11](../architecture/whitepaper-gaps.md#g11-operational-freshness-and-gate-reconciliation).

## Verify retained campaign packets

```bash
python3 benchmarks/cobalt-adversarial-verification/e5/verify_packet.py
python3 benchmarks/cobalt-adversarial-verification/packet/verify_packet.py
PYTHONPATH=python python3 -m postfiat_rpc.cobalt adversarial
```

These commands inspect committed evidence rather than querying the fleet. Dated
handoffs and completed plans remain historical snapshots. This page owns the
reconciled operational record, with each observation's date limiting its claim.
