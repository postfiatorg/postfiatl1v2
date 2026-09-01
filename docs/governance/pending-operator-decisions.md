# Pending Operator Decisions

**Status:** Awaiting the operator's answers — updated 2026-09-01

These are the open decisions only the operator can make. Each can be answered
with one line in his next handoff. The recommendations below are pre-filled so
"confirmed" is a complete answer.

| Decision | Recommendation on record | Where it came from | Your answer |
| --- | --- | --- | --- |
| Dynamic UNL as proposal-content source inside the L1 DGA envelope, deterministic formula as shadow baseline and fail-closed fallback | Confirm | Recorded by his goal run from the [validator-evaluator alternatives note](validator-evaluator-alternatives-note.md) (89.27/100); [storage milestone G7](../plans/active/storage-scaling-milestone.md) and [handoff decision 3](../handoffs/2026-08-28___dravlic__storage_g4_time_budget_decision.md); connects to the testnet-path [Gate Zero Z2](../plans/active/l1v2-public-testnet-path-milestone.md) AI-governance decision | — |
| Evidence sequence Option C, all PFT-derived integration `SHADOW_ONLY` | Confirm | Recorded by his goal run from the [L1 evidence-source note](dynamic-unl-l1-evidence-source-note.md) (89.80/100); same G7 record and handoff decision 3; connects to Gate Zero Z2 | — |
| Model authority in Dynamic UNL scoring: keep the LLM on the five sub-scores, or demote to the deterministic sub-scorer formulas | Neutral — the evidence is now two-sided: the [sub-scorer shadow evaluation](dunl-subscorer-shadow-eval-20260901.md) (v2: one cutoff flip across the eight frozen rounds, UNL overlap 19–20/20) shows the deterministic formulas nearly reproduce the model, while the [H200 reputation results](reputation-scoring-h200-results-20260901.md) (v4: determinism 864/864, first measured lift — prestige AUC 0.9732 vs deterministic baseline 0.9643) shows the model adding measurable value; the decision needs his own weighing of marginal lift vs added complexity, per the Phase 3 research charter's "failure to beat the baseline" rule | Testnet-path [Gate Zero Z2](../plans/active/l1v2-public-testnet-path-milestone.md) plus the two notes | — |
| L1 observer service owner | Name an owner | `unassigned` in the [deferred Dynamic UNL milestone](../deferred-plans/dynamic-unl-proposal-source-milestone.md); [observer spec](l1-observer-research-spec.md) ready (89.40/100) | — |
| Independent-operator submitter owner | Name an owner | `unassigned` in the same deferred-milestone owner slot | — |
| Who bears pinned model inference cost | Open | Unanswered cost question in the deferred milestone's owner section | — |
| Height-924 validator-directory custodian + read-only copy authorization (G3/G5 external input) | Name the host and authorize one copy | [Storage milestone](../plans/active/storage-scaling-milestone.md) G3 external input; handoff decision 2; G5 cannot be `OFFLINE QUALIFIED` without it | — |
| Height-915 quarantine archive re-supply (the other G3 input) | Re-supply, or record it as lost | Storage milestone G3 status `HEIGHT 915 INPUT OPEN`; testnet-path task A1 | — |
| Lock the L1 observer and anchor-profile research specs via Task Node | Lock both | Both Status lines read "Task Node lock pending the operator's decision" ([observer](l1-observer-research-spec.md), [anchor profile](l1-anchor-profile-research-spec.md)); handoff decision 6; he has resumed Task Node himself for his own plans | — |

When every row is answered, the deferred Dynamic UNL milestone can be activated
at the G7 boundary without further preparation.
