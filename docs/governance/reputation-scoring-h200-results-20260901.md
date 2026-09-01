# Reputation Scoring H200 Run — Results Note (2026-09-01)

**Status:** SHADOW_ONLY benchmark result. No governance authority, no L1
registry proposal, no Cobalt packet, no live validator action.

Plan: [reputation-scoring-h200-run-plan.md](reputation-scoring-h200-run-plan.md)
(pre-registered at commit `06a23b6b`; launch-flag erratum and Vast.ai
single-provider deviation recorded pre-output in `6b3ce76f`).
Artifact: `benchmarks/ai-governance/reputation-h200-20260901/determinism-artifact.json`.

## Determinism gate — PASS

- 2 Vast.ai H200 hosts from distinct machine owners (machine `148661`/host
  `667715`; machine `131918`/host `445596`), pinned image digest and model
  revision `017b9c7a…`, execution profile
  `qwen3.8-27b-fp8-h200-sglang-strict-fixed32-schema-v2`.
- 2 full runs per host, 288 fixed-batch slots per run (270 scoring + 18
  padding), lane-major, 9 batches of 32.
- **864/864 slot comparisons byte-identical** (run-vs-run and
  primary-vs-replay), including both truncated responses and all padding
  slots. Single aggregate SHA-256 across all four runs:
  `79a34c1c7cec941e8be132e6236b17fddbf96805d02bfd313efd41760dad6143`.
- Zero strict solo-replay recovery invocations.
- Cost: **$5.34** by account credit delta, against the $150 cap.

## Semantic acceptance — FAIL (pre-registered, published as negative result)

- **Lane validity:** prestige 90/90 valid; censorship_resistance 89/90 and
  sanctions_safety 89/90 — one `finish_reason: length` truncation each
  (`anc-002`, `anc-028`) at the 1024-token cap. Per plan §2 a schema-invalid
  response fails its lane; no repair was performed. Both truncations replay
  byte-identically.
- **Criterion 5 (lift): FAIL.** Model prestige-lane AUC 0.9464 vs
  deterministic baseline 0.9643 on the 14-real/4-fabrication set; composite
  model AUC not computable with failed lanes. No measured lift over the
  deterministic control in this run.
- **Criterion 8 (monotonicity): PASS.** Zero gross inversions. Two
  strict-chain ties disclosed (Yandex=VK at 15 in C; Sberbank=Garantex at 10
  in S).
- Anchor windows: 42 misses of 88 scored anchor cells (reported, not failed).
- Weights-prior audit frame: 268 valid classifications, 366 prior claims
  across 157 responses, 193 abstentions.
- Augmentation labels: commitment `b89dc8f4…` verified and revealed at
  `package/outputs/augmentation_labels.REVEALED.json`. Fabrication packets
  were constructed by the session operator, who also holds the rubric and
  baseline; no independence claim is made.

## Interpretation

Per plan §8, failing the lift criterion while the execution gates hold is a
publishable negative result: this frozen 27B profile shows no measured lift
over a trivial deterministic feature baseline on this small cohort, and its
truncation behavior at the 1024-token cap breaks two lanes outright. The
results are immutable. A revised rubric, a higher token cap, or a larger
model requires a new profile id and a new pre-registered run.
