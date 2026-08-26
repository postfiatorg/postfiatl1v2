# PostFiat L1 Current State

Updated: `2026-08-26T06:35:50Z`

Status: **canonical operational-state reference**

This page separates the last observed controlled-devnet state, deployed runtime
lineage, repository state, and adversarial campaign. These are different planes.
A repository commit is not deployed unless a fleet receipt binds its source and
binary to the running services.

!!! warning "Point-in-time evidence"

    The latest authenticated all-six observation ran from
    `2026-08-26T06:34:55Z` through `2026-08-26T06:35:50Z`. It is the last
    committed observation, not a real-time query now. Re-probe before making a
    later “right now” claim.

## Operational summary

| Plane | Recorded state | Exact identifier | Observed or updated at | Evidence and freshness |
| --- | --- | --- | --- | --- |
| Running devnet | Six validators converged at height 924 with empty mempools; all validator, RPC, and advisory shadow services were active. | Chain `postfiat-wan-devnet-2`; genesis `ce22ca8c…e90a9`; tip `ebeb0e1e…a7649fbef`; state `0854bc47…1ee6f413e`. | `2026-08-26T06:34:55Z`–`06:35:50Z` | Authenticated post-drill fleet observation; point in time, not a current network query. |
| Validator-trust authority | Cobalt is active for validator-registry and trust-graph ratification. The final signed drill rollback committed at 922, return to Cobalt at 923, and legitimate validator-5 rotation at 924. Consensus v2 remains block finality. | Registry root `08a451e0…2b9b1d`; trust root `89f18aef…08f0307`; ratification anchor sequence 2, ID `5eada38d…c21153c8`. | Accepted history through height 924; fleet-audited through `2026-08-26T06:35:50Z`. | E5 packet contains signed transitions, update, finality, rejection, and all-six fleet receipts. |
| Deployed runtime | Every validator used the same node binary; every shadow used the same sidecar binary. The node reports embedded revision `8cc7d15e`. | Node SHA-256 `d5e5ef630155e61b001b84edb404a4def7d29a9205f23d33d2ad9c37c2696caf`; shadow SHA-256 `d61e6d0f6767998c4abfbf4f85e1f6bd5edfeef8a7a27cf965c17b676b1a0a4a`. | `2026-08-26T06:34:55Z`–`06:35:50Z` | Direct process, binary, status, governance-verifier, registry-root, and shadow-status receipts on all six hosts. |
| Repository | `main` contains the deployed lineage, all E1–E6 packets, the authenticated final packet/interfaces, publication, and documentation. Repository descendants after `8cc7d15e` are not themselves proven installed on the fleet. | E5 evidence commit `ee6707c4`; E5 packet root `0695284a7b38ac0129c47e1242f4a2227ad25096147920e79569a924e5f3b3db`. | 2026-08-26 | Source and evidence state only. Use `git rev-parse HEAD` for the moving checkout identity and the final handoff for the pushed completion commit. |
| Adversarial campaign | E1–E6 passed their locked gates. The consolidated decision is `KEEP_ACTIVE` for Cobalt's bounded controlled-devnet validator-trust role. | E1 `9151c9…cbbc05`; E2 `8742d960…d7cba3`; E3 `9302b355…40b600`; E4 `93ba3db0…c14508`; E5 `0695284a…f3b3db`; E6 `fa6255b5…22de6e2`. | Completed 2026-08-26 | [Completed milestone](../plans/completed/cobalt-adversarial-verification-milestone.md) and [results](../governance/cobalt-adversarial-verification-results.md). |
| Operator boundary | The campaign proves protocol capability, not operator decentralization. Current proposals and authorizations originate from Foundation-administered validators. | E6 decision: independent-operator proposal path remains a mandatory follow-on milestone. | 2026-08-26 | No independent operators were recruited and no mainnet authority was granted. |

## Baseline latency versus chain height

E4's first 50 baseline rounds reproduce the activation run's
`consensus_round_ms` p95: 1,664 ms versus 1,660 ms. Latency then rises almost
linearly with height to about 14.9 s at round 500, with correlation approximately
0.9998. The cause is chain-height amplification in the integrity-checked
JSON/JSONL storage and proposal-rebuild paths: `read_jsonl_tail` in
`crates/storage/src/lib.rs` rescans the hash chain on every append. The 5% gate
remains valid as a paired, same-length A/B comparison, but the absolute latency
numbers are not a finality SLA.

## Last observed devnet values

| Field | Value |
| --- | --- |
| Chain | `postfiat-wan-devnet-2` |
| Genesis hash | `ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9` |
| Observation window | `2026-08-26T06:34:55Z`–`2026-08-26T06:35:50Z` |
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
