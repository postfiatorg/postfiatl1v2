# PostFiat L1 Current State

Updated: `2026-08-30T14:48:45Z`

Status: **canonical operational-state reference**

This page separates the last observed controlled-devnet state, deployed runtime
lineage, repository state, and adversarial campaign. These are different planes.
A repository commit is not deployed unless a fleet receipt binds its source and
binary to the running services.

!!! warning "Point-in-time evidence"

    The latest authenticated all-six observation ran from
    `2026-08-30T14:48:27Z` through `2026-08-30T14:48:45Z`, after the G6 clone
    rehearsal stopped. It is a point-in-time observation, not a real-time query
    now. Re-probe before making a later “right now” claim.

## Operational summary

| Plane | Recorded state | Exact identifier | Observed or updated at | Evidence and freshness |
| --- | --- | --- | --- | --- |
| Running devnet | Six validators converged at height 924 with empty mempools; all validator, RPC, and advisory shadow services were active before and after the stopped G6 rehearsal. | Chain `postfiat-wan-devnet-2`; genesis `ce22ca8c…e90a9`; tip `ebeb0e1e…a7649fbef`; state `0854bc47…1ee6f413e`. | `2026-08-30T14:48:27Z`–`14:48:45Z` | Authenticated post-stop fleet observation; point in time, not a current network query. |
| Validator-trust authority | Cobalt remains active for validator-registry and trust-graph ratification. The final signed drill rollback committed at 922, return to Cobalt at 923, and legitimate validator-5 rotation at 924. Consensus v2 remains block finality. | Registry root `08a451e0…2b9b1d`; trust root `89f18aef…08f0307`; ratification anchor sequence 2, ID `5eada38d…c21153c8`. | Accepted history through height 924; fleet-audited through `2026-08-30T14:48:45Z`. | The fresh probe found authority mode 1 and identical registry/trust roots on all six. |
| Deployed runtime | Every validator still uses the pre-storage node binary; every validator, RPC, and shadow service is active. The node reports embedded revision `8cc7d15e`. | Node SHA-256 `d5e5ef630155e61b001b84edb404a4def7d29a9205f23d33d2ad9c37c2696caf`; candidate `9e82d928…8c80c` was not deployed. | `2026-08-30T14:48:27Z`–`14:48:45Z` | Direct process, binary, status, governance, registry-root, and service receipts on all six hosts. |
| Storage rollout | `d0ae79f3` is **not clone-qualified and must not deploy**. Six exact height-924 clones rebuilt and independently verified one shared transactional packet, but the first height-925 certified round failed closed while reapplying superseded validator-registry history. | Candidate `d0ae79f3`; binary `9e82d928…8c80c`; migration packet `6a6b53ea…e4807d5`; reason `VALIDATOR_REGISTRY_HISTORY_REAPPLICATION_ROOT_MISMATCH`. | Failed `2026-08-30T14:46:05Z`; fleet unchanged through `14:48:45Z`. | [Rollout plan](../plans/active/devnet-storage-rollout-plan.md) and `benchmarks/storage-scaling/devnet-rollout/g6-failure-20260830.json`. No deployment, rollback, storage activation, Cobalt transition, or Z1 clock occurred. |
| Repository | `main` contains the deployed lineage, Cobalt evidence, and the undeployed transactional storage candidate. Repository descendants after `8cc7d15e` are not installed on the fleet. | G4-qualified source `d0ae79f3`; binary `9e82d928…8c80c`; exact height-924 G6 result **FAIL**. | `2026-08-30T14:48:45Z` | The registry-history continuation defect is repaired on `main` (applied-prefix activation in `crates/node/src/block_replay_wallet.rs` with an exact superseded-rotation regression); a successor candidate must still be frozen and repeat the invalidated qualification gates before another deployment decision; use `git rev-parse HEAD` for the checkout. |
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
round failed on superseded validator-registry history reapplication. The
candidate is not clone-qualified. A partial three-binary performance
run through legacy height 100 was stopped after its selected verifier was found
to mutate inspected state; source `785806bd` corrects that verifier and passes
whole-directory mutation sentinels. The existing three-binary,
lane-native-snapshot harness still conflicts with the locked research
specification and cannot close the performance gate. The 5% gate remains valid as a
paired, same-length A/B comparison; the absolute latency numbers are not a
finality SLA and public testnet remains blocked.

## Storage-scaling implementation boundary

The committed implementation lineage through `785806bd` selects transactional
`redb` storage for the finality path while retaining authenticated bounded
JSONL heads for legacy import and audit. It includes the fixed-size ordered
history accumulator, atomic per-height commit, replay/rebuild tooling, the
closed 69-case tamper/crash matrix, exact height-915 replay, and compatible
two-binary rollback. Its offline verifier opens source and target read-only and
refuses every repair-required state without durable mutation. A local
height-501 development rehearsal also completed the ten-phase six-clone
workflow. The later exact height-924 rehearsal stopped before its first new
height: the candidate treated a superseded registry update as due again and
failed its previous-root check. That is a fail-closed result, not deployment
eligibility. Nothing in this section proves deployment. See the
[active milestone](../plans/active/storage-scaling-milestone.md) and
[development evidence](https://github.com/postfiatorg/postfiatl1v2/tree/main/benchmarks/storage-scaling).

## Last observed devnet values

| Field | Value |
| --- | --- |
| Chain | `postfiat-wan-devnet-2` |
| Genesis hash | `ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9` |
| Observation window | `2026-08-30T14:48:27Z`–`2026-08-30T14:48:45Z` |
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
