# Pending Operator Decisions

**Status:** Awaiting the operator's answers — updated 2026-09-09

These are the open decisions only the operator can make. Each can be answered
with one line in his next handoff. The recommendations below are pre-filled so
"confirmed" is a complete answer.

| Decision | Recommendation on record | Where it came from | Your answer |
| --- | --- | --- | --- |
| Dynamic UNL as proposal-content source inside the L1 DGA envelope, deterministic formula as shadow baseline and fail-closed fallback | [Confirm the AI governance direction](ai-governance-direction-20260903.md) | Recorded by his goal run from the [validator-evaluator alternatives note](validator-evaluator-alternatives-note.md) (89.27/100); [storage milestone G7](../plans/active/storage-scaling-milestone.md) and [handoff decision 3](../handoffs/2026-08-28___dravlic__storage_g4_time_budget_decision.md); connects to the testnet-path [Gate Zero Z2](../plans/active/l1v2-public-testnet-path-milestone.md) AI-governance decision | — |
| Evidence sequence Option C, all PFT-derived integration `SHADOW_ONLY` | Confirm | Recorded by his goal run from the [L1 evidence-source note](dynamic-unl-l1-evidence-source-note.md) (89.80/100); same G7 record and handoff decision 3; connects to Gate Zero Z2 | — |
| Institution-legitimacy score for provider claims | Use pinned, self-hosted `Qwen/Qwen3.8-27B-FP8` to score whether the named institution is recognizable and valuable to a Layer-1; an institution the model does not recognize receives 0. OpenRouter and deterministic validator sub-scorer formulas are outside this scoring path. | [Institution legitimacy scoring](institution-legitimacy-scoring.md); operator clarification on 2026-09-01 | **Confirmed:** self-hosted AI scores institutional legitimacy; unrecognized means 0. |
| L1 observer service owner | Name an owner | `unassigned` in the [deferred Dynamic UNL milestone](../deferred-plans/dynamic-unl-proposal-source-milestone.md); [observer spec](l1-observer-research-spec.md) ready (89.40/100) | — |
| Independent-operator submitter owner | Name an owner | `unassigned` in the same deferred-milestone owner slot | — |
| Who bears pinned model inference cost | Open | Unanswered cost question in the deferred milestone's owner section | — |
| Height-924 validator-directory custodian + read-only copy authorization (G3/G5 external input) | Name the host and authorize one copy | [Storage milestone](../plans/active/storage-scaling-milestone.md) G3 external input; handoff decision 2; G5 cannot be `OFFLINE QUALIFIED` without it | — |
| Height-915 quarantine archive re-supply (the other G3 input) | Re-supply, or record it as lost | Storage milestone G3 status `HEIGHT 915 INPUT OPEN`; testnet-path task A1 | — |
| Lock the L1 observer and anchor-profile research specs via Task Node | Lock both | Both Status lines read "Task Node lock pending the operator's decision" ([observer](l1-observer-research-spec.md), [anchor profile](l1-anchor-profile-research-spec.md)); handoff decision 6; he has resumed Task Node himself for his own plans | — |
| Whether the next validator release carries merged, default-disabled PR #38 YOLO target-receipt code | [A — Carry it in the next routine release; leave activation unscheduled](yolo-deploy-decision-20260907.md) | PR #38 merge `1412b4dc`; [YOLO target receipt v1](../yolo/target-receipt-v1.md); release and rollback guidance summarized in the decision sheet | **A recorded 2026-09-07:** carry in the next qualified routine release; activation unscheduled. Composition decision only, no deployment authority. |
| Deploy a release carrying consensus signing fix `bbb291ce` to the six controlled-devnet validators | [C — Hold for a fuller release-lineage and deployment-exact campaign](signing-fix-deploy-decision-20260909.md) | Current `main` omits deployed A666 runtime lineage, clean-build hashes differ, validator-0 RPC state is unknown, and no current all-six clone or exact local rollback binary is available | — |

When every row is answered, the deferred Dynamic UNL milestone can be activated
at the G7 boundary without further preparation.
