# Pending Operator Decisions

**Status:** Partly answered — updated 2026-10-20; Coinbase listing rows added 2026-10-26. The [other lane's answers of 2026-10-20](../handoffs/2026-10-20___nazgul__founder_lane_resync_and_answers_to_open_asks.md) confirm the AI governance direction, record the height-915 archive as lost and name me the height-924 custodian. Rows still marked — await the operator.

These are the open decisions only the operator can make. Each can be answered
with one line in his next handoff. The recommendations below are pre-filled so
"confirmed" is a complete answer.

| Decision | Recommendation on record | Where it came from | Your answer |
| --- | --- | --- | --- |
| Dynamic UNL as proposal-content source inside the L1 DGA envelope, deterministic formula as shadow baseline and fail-closed fallback | [Confirm the AI governance direction](ai-governance-direction-20260903.md) | Recorded by his goal run from the [validator-evaluator alternatives note](validator-evaluator-alternatives-note.md) (89.27/100); [storage milestone G7](../plans/completed/storage-scaling-milestone.md) and [handoff decision 3](../handoffs/2026-08-28___dravlic__storage_g4_time_budget_decision.md); connects to the testnet-path [Gate Zero Z2](../plans/active/l1v2-public-testnet-path-milestone.md) AI-governance decision | **Confirmed 2026-10-20** ([answer (i)](../handoffs/2026-10-20___nazgul__founder_lane_resync_and_answers_to_open_asks.md)): the model is demoted to one advisory flag; everything model-derived stays `SHADOW_ONLY`. Z2 stays open until the decided configuration runs the fork's rounds. |
| Evidence sequence Option C, all PFT-derived integration `SHADOW_ONLY` | Confirm | Recorded by his goal run from the [L1 evidence-source note](dynamic-unl-l1-evidence-source-note.md) (89.80/100); same G7 record and handoff decision 3; connects to Gate Zero Z2 | — |
| Institution-legitimacy score for provider claims | Use pinned, self-hosted `Qwen/Qwen3.8-27B-FP8` to score whether the named institution is recognizable and valuable to a Layer-1; an institution the model does not recognize receives 0. OpenRouter and deterministic validator sub-scorer formulas are outside this scoring path. | [Institution legitimacy scoring](institution-legitimacy-scoring.md); operator clarification on 2026-09-01 | **Confirmed:** self-hosted AI scores institutional legitimacy; unrecognized means 0. |
| L1 observer service owner | Name an owner | `unassigned` in the [deferred Dynamic UNL milestone](../deferred-plans/dynamic-unl-proposal-source-milestone.md); [observer spec](l1-observer-research-spec.md) ready (89.40/100) | — |
| Independent-operator submitter owner | Name an owner | `unassigned` in the same deferred-milestone owner slot | — |
| Who bears pinned model inference cost | Open | Unanswered cost question in the deferred milestone's owner section | **Answered 2026-10-23 for the weekly identity-flag runs** ([Z2 plan](../plans/active/z2-advisory-flag-execution-plan-20261022.md#status-on-2026-10-23)): the founder's OpenRouter account pays. That is the account whose key the server already holds, at about $5 per weekly round. He is informed in the 2026-10-23 handoff and may object. The pinned profile runs on two rented GPU hosts and calls no OpenRouter API, so how the rental is paid from that account is settled before the first run. |
| Height-924 validator-directory custodian + read-only copy authorization (G3/G5 external input) | Name the host and authorize one copy | [Storage milestone](../plans/completed/storage-scaling-milestone.md) G3 external input; handoff decision 2; G5 cannot be `OFFLINE QUALIFIED` without it | **Answered 2026-10-20** ([answer (h)](../handoffs/2026-10-20___nazgul__founder_lane_resync_and_answers_to_open_asks.md)): I am the custodian; the six stopped copies of 2026-08-30 are on the work server; one read-only copy is approved. Both copies' manifest signatures are verified before any use; the replay waits for STO-07. |
| Height-915 quarantine archive re-supply (the other G3 input) | Re-supply, or record it as lost | Storage milestone G3 (was `HEIGHT 915 INPUT OPEN`); testnet-path task A1 | **Lost** ([answer (h)](../handoffs/2026-10-20___nazgul__founder_lane_resync_and_answers_to_open_asks.md)). I propose the signed validator-1 canary archive at height 1123 (manifest `d957141e68a2…`) as the replacement replay input ([storage G3](../plans/completed/storage-scaling-milestone.md#g3-exact-replay)). |
| Lock the L1 observer and anchor-profile research specs via Task Node | Lock both | Both Status lines read "Task Node lock pending the operator's decision" ([observer](l1-observer-research-spec.md), [anchor profile](l1-anchor-profile-research-spec.md)); handoff decision 6; he has resumed Task Node himself for his own plans | — |
| Whether the next validator release carries merged, default-disabled PR #38 YOLO target-receipt code | [A — Carry it in the next routine release; leave activation unscheduled](yolo-deploy-decision-20260907.md) | PR #38 merge `1412b4dc`; [YOLO target receipt v1](../yolo/target-receipt-v1.md); release and rollback guidance summarized in the decision sheet | **A recorded 2026-09-07:** carry in the next qualified routine release; activation unscheduled. Composition decision only, no deployment authority. |
| Deploy a release carrying consensus signing fix `bbb291ce` to the six controlled-devnet validators | [C — Hold for a fuller release-lineage and deployment-exact campaign](signing-fix-deploy-decision-20260909.md) | Current `main` omits deployed A666 runtime lineage, clean-build hashes differ, validator-0 RPC state is unknown, and no current all-six clone or exact local rollback binary is available | — |

Fact check of 25 November 2026: the signing fix `bbb291ce` and the YOLO
target-receipt code `1412b4dc` are both ancestors of every release the
validators have run since 21 November 2026 (`main-r64-20261121` and the build
`04600868`). The two rows above are left for the operator to close.

When every row is answered, the deferred Dynamic UNL milestone can be activated
at the G7 boundary without further preparation.

## Coinbase listing path: PFT as an ERC-20 on Base (2026-10-26)

These rows come from the [milestone](../plans/active/pft-erc20-base-listing-milestone.md) and the [ERC-20 options paper](../listings/coinbase/pft-erc20-representation-options-20261026.md). The first six belong to the other
lane; the last one is this lane's and needs only funding.

| Decision | Recommendation on record | Where it came from | Your answer |
| --- | --- | --- | --- |
| The total supply number | Decided; record it in the token document | 2026-10-26, milestone P1.5, paper section 9. Blocks the token document, the Base mainnet deployment (P1.7) and any Phase 2 genesis code | **Answered 2026-10-28:** 100 billion PFT, fixed, with no validator rewards (source: the exchange guidance handoff, 00:26 UTC). |
| The split at launch between the ERC-20 slice and genesis | Open; the genesis external lane later reserves exactly the ERC-20 slice | 2026-10-26, milestone P1.5. Blocks the token document, the token's constructor supply for P1.7 and the lane's reserved amount in Phase 2 | — |
| The treatment of PFT already paid on the PFT Ledger | Open; a snapshot and a swap into the ERC-20, or into native PFT at genesis (paper section 7) | 2026-10-26, milestone P1.5. Blocks the token document | — |
| The treasury Safe's signers and threshold | Name the signers and set the threshold; never a single key | 2026-10-26, milestone P1.5 and P1.6. Blocks the Safe on Base (P1.6) and the treasury address the deployment needs (P1.7) | — |
| The audit budget and vendor | Approve the budget and choose an established firm; about one to three weeks of calendar time | 2026-10-26, milestone P1.4. Blocks the audit, and nothing goes to Base mainnet before it | — |
| The Base mainnet deployment with the other lane's keys | Deploy once the token document, the audit and the Safe exist, with the other lane's deployer key and gas | 2026-10-26, milestone P1.7. Blocks source verification (P1.8) and the listing itself | — |
| This lane's Base Sepolia test deployment | Fund a deployer with a faucet amount of Base Sepolia ETH; the server's test wallet `0xC75Bf05Ce82d6f4b6139dd9446D6De5F5994a4CB` can serve once funded | 2026-10-26, milestone P1.3 (the dry run is done). Blocks the test deployment and its verified source | — |
