# SGLang Determinism Evidence

This page records provider-backed SGLang determinism evidence for the current
Qwen governance profile. It is evidence for replayability under a pinned
profile, not a claim that all LLM inference is deterministic under all
hardware, batching, tensor-parallel, or runtime settings.

## Vast H100/H200 Run

Run date: 2026-05-26

Model/runtime:

- `Qwen/Qwen3.6-27B-FP8`
- SGLang server
- tensor parallelism: `1`
- `--enable-deterministic-inference`
- `max_running_requests=1`
- `temperature=0`
- `top_p=1`
- attention backend selected by SGLang for deterministic mode: `fa3`
- sampling backend selected by SGLang for deterministic mode: `pytorch`

Machines:

| Label | Vast instance | Hardware |
| --- | ---: | --- |
| `h100` | `37928285` | `NVIDIA H100 80GB HBM3` |
| `h200` | `37928328` | `NVIDIA H200` |

Main report:

```text
reports/sglang-determinism-vast/20260526/sglang-determinism-vast-report.json
```

Verification command:

```bash
scripts/sglang-determinism-vast-experiment --verify-report \
  --report reports/sglang-determinism-vast/20260526/sglang-determinism-vast-report.json
```

Verified checks:

| Check | Result |
| --- | --- |
| Two provider machines | pass |
| Two hardware classes | pass |
| Repeated simple text outputs converged within each machine | pass |
| Simple text outputs converged across H100/H200 | pass |
| Top-logprob roots converged within each machine | pass |
| Top-logprob roots converged across H100/H200 | pass |
| Full-vocabulary vector contained 248,320 entries on each machine | pass |
| Full-vocabulary vector root converged across H100/H200 | pass |
| Redaction scan | pass |

Observed simple outputs:

| Prompt | Converged output |
| --- | --- |
| Capital sentence | `The capital of France is Paris.` |
| Finality sentence | `Settlement finality means that a payment transaction is irrevocable and unconditional, ensuring the transfer of funds cannot be reversed.` |

Full-vocabulary probe:

| Field | Value |
| --- | --- |
| Probe output text | ` the` |
| Probe output token id | `279` |
| Vocabulary entries committed | `248,320` |
| Cross-machine vector root | `560ea13c99f73a60c184ec07ba3554ea11c72487d56ac28c366495d58ce8913c` |

The vector files are stored at:

```text
reports/sglang-determinism-vast/20260526/h100/full_vocab_next_token_probe.lgp1
reports/sglang-determinism-vast/20260526/h200/full_vocab_next_token_probe.lgp1
```

Cleanup proof:

```text
reports/sglang-determinism-vast/20260526/sglang-determinism-vast-cleanup-report.json
```

The cleanup report verifies that both created Vast instances were destroyed and
that the post-destroy Vast inventory reported zero instances.

## Interpretation

This run directly answers the narrow determinism objection that matters for the
governance lane:

1. Simple generated text was stable across repeated runs.
2. Token-level top-logprob commitments were stable across repeated runs.
3. A full 248,320-entry next-token logprob vector was stable across H100/H200.
4. The provider resources were destroyed after evidence collection.

The result does not make arbitrary future prompts or model profiles acceptable.
It shows that this pinned Qwen/SGLang profile can produce replayable evidence
objects across two Vast GPU classes, including a full-vector commitment rather
than only a final text string.

## Small Profile Admission Probes

Two smaller Qwen3-1.7B profile runs extend the evidence without claiming
cross-runtime logit or parsed-root identity:

| Profile | Machine | Result |
| --- | --- | --- |
| `qwen3-1.7b-sglang-fp16-nightly-20260523-v1` | A100-SXM4-80GB | `100/100` parseable decisions; one parsed root; one top-logprob root. |
| `qwen3-1.7b-mlx-bf16-v1` | GitHub `macos-15-xlarge`, Apple Silicon M2 / MLX | `600/600` parseable decisions across two closed-option packets; one decision root per packet. |

The Apple/MLX run used Qwen3-1.7B-BF16 on GitHub-hosted Apple Silicon. It ran
300 repeats of the adversarial `validator-independence` packet and 300 repeats
of the `cobalt-registrar` packet. The profile selected `hold` for all 300
validator-independence runs, citing `release_manager`, `monitoring_endpoint`,
and `funding_control`; it selected `adopt-cobalt-registrar` for all 300
registrar runs. The evidence is therefore within-profile repeatability and
typed decision stability, not a cross-profile raw-logit or parsed-root
admission certificate.

Reports:

```text
reports/qwen-sglang-profile-portability/20260528-vast-a100/machine_report.json
reports/qwen-mlx-profile-portability/github-macos-15-xlarge-26687502688/github-26687502688-validator-independence/machine_report.json
reports/qwen-mlx-profile-portability/github-macos-15-xlarge-26687502688/github-26687502688-cobalt-registrar/machine_report.json
```

Verification:

```bash
scripts/qwen-sglang-replay-profile-runner verify-report \
  --report reports/qwen-sglang-profile-portability/20260528-vast-a100/machine_report.json

scripts/qwen-mlx-replay-profile-runner verify-report \
  --report reports/qwen-mlx-profile-portability/github-macos-15-xlarge-26687502688/github-26687502688-validator-independence/machine_report.json

scripts/qwen-mlx-replay-profile-runner verify-report \
  --report reports/qwen-mlx-profile-portability/github-macos-15-xlarge-26687502688/github-26687502688-cobalt-registrar/machine_report.json
```

SGLang's public documentation explains the relevant root cause and mitigation:
ordinary temperature-zero inference can still vary because batching changes GPU
reduction order, while deterministic mode uses batch-invariant operations
enabled through `--enable-deterministic-inference`.

Reference:
[`docs.sglang.ai/advanced_features/deterministic_inference.html`](https://docs.sglang.ai/advanced_features/deterministic_inference.html)
