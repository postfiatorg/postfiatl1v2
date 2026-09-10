# PostFiat L1 Current State

Updated: `2026-09-10T10:40:22Z`

Status: **canonical operational-state reference**

!!! warning "2026-09-10: validator-0 RPC diagnosis, not repair"

    Read-only diagnosis identified why validator-0 timed out on September 9.
    Its deployed RPC accepts at most 10,000 connections. The event log for the
    affected process ends at request index 10,000, while one long-lived
    keep-alive connection had not returned an event. The deployed
    [serve loop](https://github.com/postfiatorg/postfiatl1v2/blob/634e3625b48aa75cb6b9fc2225aa5ce83584bb3c/crates/node/src/rpc_cli.rs#L690-L820)
    then stops accepting and waits for every active connection,
    leaving systemd `active` while new clients wait in the unserviced socket
    backlog. Validator-0 alone receives repeated wallet bridge-readiness
    traffic through a long-lived authenticated SSH tunnel; the other five RPC
    processes had accepted only 121–124 connections when validator-0's current
    process had accepted 3,404.

    A separate session restarted the RPC at `2026-09-09T20:19:37Z`, after the
    failed observation and before this diagnosis. This campaign did not
    restart or change anything. The replacement answered `status`,
    `server_info`, and `mempool_status` from `2026-09-10T10:24:20Z` through
    `10:24:31Z`: height 1020, tip `9d02b8ee…b1768feb`, state root
    `587c6526…d39bead6`, empty mempool, and the same pre-fix binary. Current
    service availability is restored but the finite-request/keep-alive defect
    remains and can recur; no repair or fleet mutation was performed.

!!! warning "2026-09-09: fresh read-only observation has five-of-six RPC agreement"

    The capture ran from `2026-09-09T09:24:54Z` through `09:27:00Z`.
    Validators 1–5 answered `status`, `server_info`, and `mempool_status` and
    agreed at height 1020, tip `9d02b8ee…b1768feb`, state root
    `587c6526…d39bead6`, with empty mempools. Validator-0's three RPC reads
    each timed out after eight seconds, so its height, root, and mempool are
    unknown for this observation; its host remained reachable and both node
    services were active. Read-only process-identity checks found all 12
    validator/RPC processes running release `a666-source-route-20260907`,
    binary `57b0f4d1…634eec83`. No corrective action or fleet/chain write was
    performed.

!!! success "2026-08-31: transactional storage DEPLOYED AND ACTIVE at height 931"

    Release `storage-lease-af9b83c3` (writer lease `f0013c29` + storage
    rework, binary `383f4325…141a7a`) is live on all six validators after
    passing the deployment-exact gate on real-host clones. The chain migrated
    and activated transactional `redb` storage at height 930 and finalized
    block 931 on it: all six converged at tip `8e3639ee…665c4b`, commitment
    `postfiat.replicated_state.v2`, zero full-history scans. Z1 observation
    started `2026-08-31T04:29:41Z`. Receipts:
    `deployments/storage-lease-20260831/deploy-receipt.json` and
    `benchmarks/storage-scaling/deployment-exact-gate/gate-926-receipt.json`.
    Height-926 details below are historical.

!!! success "2026-08-31 (earlier): chain unwedged and live at height 926"

    The height-924 wedge was the superseded validator-registry reapplication
    defect in the **deployed** `8cc7d15e` lineage, not only in the storage
    candidate. Minimal release `registry-fix-291d1eb1`
    (`8cc7d15e` + backported `2c7aa36f`, binary
    `6b07a8c3…8ef7de6`, no storage changes) was signed and rolled to all six
    validators. Certified transfer rounds then committed blocks 925 and 926;
    all six validators converged at tip `9e738e87…c2098b`, state root
    `687b45d5…ac4ad5`. Three operational faults were also repaired
    (validator-0 RPC accept-queue wedge, validator-1's unrestored
    `ordered_batches` JSONL head, root-owned certified-send outbox jobs). The
    fleet-wide snapshot finalized-checkpoint export defect at block 924
    remains open. See the
    [registry-continuation wedge postmortem](../postmortems/devnet-registry-continuation-wedge-2026-08-31.md)
    and `deployments/registry-fix-20260831/deploy-receipt.json`. Sections
    below describing the height-924 state are historical.

This page separates the last observed controlled-devnet state, deployed runtime
lineage, repository state, and adversarial campaign. These are different planes.
A repository commit is not deployed unless a fleet receipt binds its source and
binary to the running services.

!!! warning "Point-in-time evidence"

    The latest read-only diagnosis queried validator-0 from
    `2026-09-10T10:24:20Z` through `10:40:22Z`; it is not a fresh simultaneous
    six-validator observation. The partial fleet capture from
    `2026-09-09T09:24:54Z` through `09:27:00Z` remains historical evidence of
    the failure. Every observation is point-in-time evidence, not a real-time
    query now.

## Operational summary

| Plane | Recorded state | Exact identifier | Observed or updated at | Evidence and freshness |
| --- | --- | --- | --- | --- |
| Validator-0 RPC diagnosis | The September 9 timeouts were an exhausted 10,000-connection accept budget combined with a still-active keep-alive connection. An out-of-campaign restart restored responses but did not repair the recurrence condition. | Affected event sequence ended at request index 10,000; current process had accepted 3,404 connections versus 121–124 on each peer. | `2026-09-10T10:24:20Z`–`10:40:22Z` | Read-only RPC, event-log, journal, socket, process, and sysstat inspection. Diagnosis only; no restart or write. |
| Running devnet, latest read-only observation | Validators 1–5 answered all three health reads and agreed at height 1020 with empty mempools. Validator-0's RPC timed out; its host and both services were reachable/active, but full-six ledger agreement is not established by this capture. | Chain `postfiat-wan-devnet-2`; genesis `ce22ca8c…e90a9`; tip `9d02b8ee…b1768feb`; state `587c6526…d39bead6` on validators 1–5. | `2026-09-09T09:24:54Z`–`09:27:00Z` | Authenticated SSH forwarding to loopback RPC plus read-only process identity; point in time. |
| Deployed runtime, latest identity | All 12 validator and RPC service processes were active/running from one release and one executable hash. No process runs a build containing repository signing fix `bbb291ce`. | Release `a666-source-route-20260907`; node SHA-256 `57b0f4d1d42d66878d7dbb8c33919c7fa0f87c6cc1a4b9cc1a85d75b634eec83`. | `2026-09-09T09:26:45Z`–`09:27:00Z` | Direct read-only systemd, `/proc`, release-path, and SHA-256 identity checks on all six hosts. |
| Running devnet, historical 2026-08-30 observation | Six validators converged at height 924 with empty mempools after validator-1 was rolled back from the failed storage canary; all validator, RPC, and advisory shadow services were active. | Chain `postfiat-wan-devnet-2`; genesis `ce22ca8c…e90a9`; tip `ebeb0e1e…a7649fbef`; state `0854bc47…1ee6f413e`. | `2026-08-30T23:00:24Z`–`23:00:39Z` | Authenticated post-rollback fleet observation; point in time, not a current network query. |
| Validator-trust authority | Cobalt remains active for validator-registry and trust-graph ratification. The final signed drill rollback committed at 922, return to Cobalt at 923, and legitimate validator-5 rotation at 924. Consensus v2 remains block finality. | Registry root `08a451e0…2b9b1d`; trust root `89f18aef…08f0307`; ratification anchor sequence 2, ID `5eada38d…c21153c8`. | Accepted history through height 924; fleet-audited through `2026-08-30T23:00:39Z`. | The recovery probe found authority mode 1 and identical registry/trust roots on all six. |
| Deployed runtime | Every validator uses the pre-storage node binary again; every validator, RPC, and shadow service is active. Validator-1 briefly ran successor transport while its RPC failed, then returned to the signed `8cc7d15e` deployment. | Node SHA-256 `d5e5ef630155e61b001b84edb404a4def7d29a9205f23d33d2ad9c37c2696caf`; stopped candidate `0cc664a3…ad4183` is inactive. | `2026-08-30T23:00:24Z`–`23:00:39Z` | Direct process, binary, status, service, signed-unit, and post-rollback storage comparisons. |
| Storage rollout | `d0ae79f3` failed height-925 continuation. Successor `10dd9f20` fixed that defect and passed the old G6 runner, but is **not deployment-qualified and must not deploy**: the runner omitted the live concurrent transport/RPC topology. The validator-1 canary hit an exclusive `redb` lock and required exact data-plus-binary rollback. | Successor `10dd9f20`; binary `0cc664a3…ad4183`; reason `TRANSACTIONAL_DATABASE_MULTI_PROCESS_LOCK_CONFLICT`. | Canary stopped and rollback verified through `2026-08-30T23:00:39Z`. | [Canary rollback report](../postmortems/devnet-storage-live-canary-rollback-2026-08-30.md) and `benchmarks/storage-scaling/devnet-rollout/canary-rollback-20260830.json`. No block, governance, Cobalt, or storage activation occurred; Z1 did not start. |
| Repository | `main` contains the deployed lineage, Cobalt evidence, the registry-history repair, and the undeployed transactional storage candidate. Repository descendants after `8cc7d15e` are not installed on the fleet. | Current local source `10dd9f20`; candidate binary `0cc664a3…ad4183`; old G6 `PASS` invalidated by the live topology gap. | `2026-08-30T23:00:39Z` | Repair requires one authoritative transactional writer and a replacement gate using the exact signed systemd topology plus exact deployed-binary/data rollback; use `git rev-parse HEAD` for the checkout. |
| Adversarial campaign | E1–E6 passed their locked gates. The consolidated decision is `KEEP_ACTIVE` for Cobalt's bounded controlled-devnet validator-trust role. | E1 `495a59a2…4dfcd90`; E2 `8742d960…d7cba3`; E3 `9302b355…40b600`; E4 `93ba3db0…c14508`; E5 `0695284a…f3b3db`; E6 `ee6848f5…121be0b`. | Completed 2026-08-26 | [Completed milestone](../plans/completed/cobalt-adversarial-verification-milestone.md) and [results](../governance/cobalt-adversarial-verification-results.md). |
| Operator boundary | The campaign proves protocol capability, not operator decentralization. Current proposals and authorizations originate from Foundation-administered validators. | E6 decision: independent-operator proposal path remains a mandatory follow-on milestone. | 2026-08-26 | No independent operators were recruited and no mainnet authority was granted. |

## Baseline latency versus chain height

E4's first 50 baseline rounds reproduce the activation run's
`consensus_round_ms` p95: 1,664 ms versus 1,660 ms. Latency then rises almost
linearly with height to about 14.9 s at round 500, with correlation approximately
0.9998. In the deployed lineage measured by E4, chain-height amplification came
from full-prefix JSONL verification and full ordered-history proposal rebuilds.
The repository now contains an undeployed transactional `redb` candidate. Its
height-915 replay, tamper/crash matrix, and a development-only height-501
six-clone workflow pass. On 2026-08-30, all six exact height-924 transactional
rebuilds and independent verifies passed, but the first certified continuation
round failed on superseded validator-registry history reapplication. That
candidate is not clone-qualified. Successor `10dd9f20` repaired the history
fault and passed the existing runner, but the validator-1 canary proved the
runner was not deployment-exact: it never co-started transport and RPC against
one transactional database or exercised the signed systemd sandbox. The
successor is not deployment-qualified. A partial three-binary performance
run through legacy height 100 was stopped after its selected verifier was found
to mutate inspected state; source `785806bd` corrects that verifier and passes
whole-directory mutation sentinels. The existing three-binary,
lane-native-snapshot harness still conflicts with the locked research
specification and cannot close the performance gate. The 5% gate remains valid as a
paired, same-length A/B comparison; the absolute latency numbers are not a
finality SLA and public testnet remains blocked.

## Storage-scaling implementation boundary

The committed implementation selects transactional `redb` storage for the
finality path while retaining authenticated bounded JSONL heads for legacy
import and audit. It includes the fixed-size ordered-history accumulator,
atomic per-height commit, replay/rebuild tooling, the closed 69-case
tamper/crash matrix, exact height-915 replay, and local paired-binary rollback
evidence. The live canary invalidated any broader rollback claim: the writable
rebuild upgrades legacy JSONL heads in the source directory, and the exact
deployed `8cc7d15e` binary rejects those v2 heads. Its offline verifier opens
source and target read-only and
refuses every repair-required state without durable mutation. A local
height-501 development rehearsal also completed the ten-phase six-clone
workflow. The later exact height-924 rehearsal stopped before its first new
height: the candidate treated a superseded registry update as due again and
failed its previous-root check. Successor `10dd9f20` fixed that defect, but the
old G6 service model started only transport processes. On the live host,
transport retained the transactional database lock and RPC could not start.
The canary was rolled back, and no current candidate is deployment-eligible.
Nothing in this section proves deployment. See the
[active milestone](../plans/active/storage-scaling-milestone.md) and
[development evidence](https://github.com/postfiatorg/postfiatl1v2/tree/main/benchmarks/storage-scaling).

## Last observed devnet values

### Validator-0 read-only diagnosis — 2026-09-10 { #validator-0-rpc-diagnosis-20260910 }

This was a diagnosis of the failed September 9 endpoint, not a new all-fleet
observation. The replacement RPC process answered all three documented reads:
height 1020, tip `9d02b8ee…b1768feb`, state root
`587c6526…d39bead6`, and zero pending transactions. Its release remains
`a666-source-route-20260907`, binary SHA-256
`57b0f4d1d42d66878d7dbb8c33919c7fa0f87c6cc1a4b9cc1a85d75b634eec83`.
That agrees with the September 9 values from validators 1–5, but the captures
were not simultaneous and do not establish a new all-six observation.

The affected process ran from `2026-09-07T01:10:26Z` until a separate session
restarted it at `2026-09-09T20:19:37Z`. Its event sequence reached the unit's
`--max-requests 10000` ceiling. One connection remained active, so the process
stayed alive in its post-accept drain instead of exiting; new requests were no
longer accepted. Host telemetry around the failed `09:24`–`09:27` capture
showed 94.56% CPU idle, no blocked tasks, 0.14% I/O wait, and ample available
memory, so host resource exhaustion does not explain the timeouts.

The replacement process still has a persistent keep-alive client and the same
10,000-connection budget. At the diagnostic capture it reported 3,404 accepted
connections and a peak of three active connections; the five peers reported
121–124 accepted connections and peaks of two or three. This is a reproduced
service-lifecycle defect and a recurrence risk, not evidence of a live repair.
No service, host, configuration, or chain state was changed by this diagnosis.

### Historical partial fleet observation — 2026-09-09

| Field | Value |
| --- | --- |
| Chain | `postfiat-wan-devnet-2` |
| Genesis hash | `ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9` |
| Observation window | `2026-09-09T09:24:54Z`–`2026-09-09T09:27:00Z` |
| Agreement | Validators 1–5 agree; validator-0 RPC status is unknown after timeouts. Full-six agreement is not established by this observation. |
| Height | 1020 on validators 1–5; validator-0 unknown |
| Mempool | 0 pending on validators 1–5; validator-0 unknown |
| Tip hash | `9d02b8eecb78408e8f1de12ae1e2607ad4987c2c8d883f1718593df7c2d9ca529f0707bd581e3e414361f202b1768feb` on validators 1–5 |
| State root | `587c6526a2549c97458b371f42e849c49274a1f522e7dcb841b74cd74bdb3d6747c2e6ca646c08ac51733796d39bead6` on validators 1–5 |
| Running release | `a666-source-route-20260907` on all 12 validator/RPC processes |
| Running binary | SHA-256 `57b0f4d1d42d66878d7dbb8c33919c7fa0f87c6cc1a4b9cc1a85d75b634eec83` on all 12 processes |
| Repository/fleet boundary | `origin/main` signing fix `bbb291ce` is not deployed on any validator. |

| Validator | RPC status, server info, mempool | Observed ledger | Running identity | Identity captured |
| --- | --- | --- | --- | --- |
| validator-0 | Timed out after 8 seconds on each read; host reachable; validator and RPC services active/running | Height, tip, root, and mempool unknown | `a666-source-route-20260907`; `57b0f4d1…634eec83` | `2026-09-09T09:26:45Z` |
| validator-1 | Running; mempool 0 | Height 1020; `9d02b8ee…b1768feb`; `587c6526…d39bead6` | `a666-source-route-20260907`; `57b0f4d1…634eec83` | `2026-09-09T09:26:47Z` |
| validator-2 | Running; mempool 0 | Height 1020; `9d02b8ee…b1768feb`; `587c6526…d39bead6` | `a666-source-route-20260907`; `57b0f4d1…634eec83` | `2026-09-09T09:26:52Z` |
| validator-3 | Running; mempool 0 | Height 1020; `9d02b8ee…b1768feb`; `587c6526…d39bead6` | `a666-source-route-20260907`; `57b0f4d1…634eec83` | `2026-09-09T09:26:55Z` |
| validator-4 | Running; mempool 0 | Height 1020; `9d02b8ee…b1768feb`; `587c6526…d39bead6` | `a666-source-route-20260907`; `57b0f4d1…634eec83` | `2026-09-09T09:26:56Z` |
| validator-5 | Running; mempool 0 | Height 1020; `9d02b8ee…b1768feb`; `587c6526…d39bead6` | `a666-source-route-20260907`; `57b0f4d1…634eec83` | `2026-09-09T09:27:00Z` |

### Drift from the 2026-09-07 machine handoff

The five validators with readable RPC status advanced from height 1005 to 1020
(+15 blocks), and their state root changed from `6ed69ca9…ced065f9` to
`587c6526…d39bead6`. They agree with one another. Validator-0's current ledger
state could not be compared because its RPC timed out. All six hosts still run
the same release and byte-identical binary recorded on September 7. Because the
running SHA-256 remains `57b0f4d1…634eec83`, none runs a build containing the
later signing fix `bbb291ce`.

The observation used authenticated SSH forwarding to each loopback-bound RPC
and the repository's read-only `status`/`server_info`/`mempool_status` checks.
Release identity came from read-only systemd `MainPID`, `/proc/<pid>/exe`, and
SHA-256 inspection. No restart, deployment, configuration change, host write,
or chain write was performed.

### Historical 2026-08-30 all-six values

| Field | Value |
| --- | --- |
| Chain | `postfiat-wan-devnet-2` |
| Genesis hash | `ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9` |
| Observation window | `2026-08-30T23:00:24Z`–`2026-08-30T23:00:39Z` |
| Validators | 6, all converged |
| Height | 924 |
| Mempool | 0 pending on every validator |
| Tip hash | `ebeb0e1ee27f30ba480255728832719d94eac1a89d762a7aa7019eae269008fac53098cf6495f477a241d63a7649fbef` |
| State root | `0854bc47f78996b2dcd279206cbdcc0b4858395c5937e0e0d56b3d645ca6b6a9d9c9578f5ac77bb14bea9dd1ee6f413e` |
| Registry root | `08a451e07aeaf9ada41a69e7c26dfd3fd86fce11c02f5567127c598b3cf775ac054b2add85295cc8c0d429bb6d2b9b1d` |
| Trust-graph root | `89f18aef2c5726ae43043407eb4d638ee8f3b6027e58ec3553296478602232cf3c2fc5d1dfebc4058d720b16508f0307` |
| Ratification anchor | Sequence 2; `5eada38d23c83709a44f2cfa7eb7897d9d4b1da906e6ef66fc5dfec7e64102edda2e82b33d71346c1d8f75ccc21153c8` |
| Validator-trust authority | Cobalt |
| Block finality | Consensus v2 |
| Validator registry updates | 2 |
| Latest update | Validator-5 key rotation `a6b806eb304ffc5d4c329fc179fa628745a2e609724cd67d654805ebfc4cc12bc4338ed5fa1f3bf301384ca8aaf8f18a` |

Cobalt ratifies validator-registry and trust-graph changes. A separate layer
decides which validators deserve trust. Current proposals originate from
Foundation-administered validators. Cobalt does not order blocks, replace
Consensus v2, or prove operator decentralization.

## Accepted live drill history

| Height | Accepted action | Proposal identity | Authorization identities | Result |
| ---: | --- | --- | --- | --- |
| 920 | Initial rollback to Foundation | validator-2 | validators 0–4 | Accepted; retained as remediation history. |
| 921 | Initial return to Cobalt | validator-3 | validators 0–4 | Accepted, but its trust binding did not match the protocol-native post-return graph; not used as the final gate. |
| 922 | Corrective rollback to Foundation | validator-4 | validators 0–4 | Accepted final-gate rollback. |
| 923 | Corrective return to Cobalt | validator-5 | validators 0–4 | Accepted final-gate return with the correct trust binding. |
| 924 | Legitimate validator-5 key rotation | validator-0 | validators 0–4 | Accepted; old/stolen validator-5 key was not an authorizer. |

All six validators reported the same height-920-through-924 history. Every block
had at least five Consensus v2 votes. The E5 packet preserves the first pair,
the correction, and the final state; it does not rewrite the remediation out of
the record.

## Evidence boundaries

The final E5 observation performed authenticated service, process/binary,
validator status, governance verification, registry-root, finality-history, and
shadow-status checks. Negative drills confirmed the durable governance and
registry file hashes were unchanged. The replacement validator private material
never entered the repository packet.

Verify the live drill packet:

```bash
python3 benchmarks/cobalt-adversarial-verification/e5/verify_packet.py
```

Verify the consolidated campaign and render the operator result:

```bash
python3 benchmarks/cobalt-adversarial-verification/packet/verify_packet.py
PYTHONPATH=python python3 -m postfiat_rpc.cobalt adversarial
```

These commands authenticate committed evidence. They do not query the fleet.

Dated handoffs and completed plans are historical snapshots. When their mutable
operational statements conflict with this page, this page owns the current
record; the observation time still limits every claim.
