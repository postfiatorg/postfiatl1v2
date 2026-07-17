# Qwen MLX GitHub Actions Replay Profile

Status: completed 300-repeat profile run
Date: 2026-05-30
GitHub repository: `postfiatorg/qwen-mlx-replay`
Workflow run: `26687502688`

## Purpose

This packet answers the narrow hardware-accessibility critique against the AI
governance replay layer:

```text
Replay profiles are admitted independently. A profile must prove repeatability
under pinned weights, tokenizer, runtime, quantization, hardware class, prompt,
parser, and decoding policy. Cross-profile raw-logit equality is not required;
selector-facing governance compares typed parsed decisions and routes split
certificates to hold/no-op.
```

The result is evidence for one Apple Silicon / MLX profile. It does not prove
that Apple/MLX logits match NVIDIA/SGLang logits, that a small model should
become the production governance model, or that the production Qwen3.6 profile
is portable to Apple hardware.

## Machine Profile

| Field | Value |
| --- | --- |
| Hardware class | Apple Silicon M2 GitHub `macos-15-xlarge` |
| Platform | `macOS-15.7.7-arm64-arm-64bit-Mach-O` |
| Runtime | `mlx` 0.31.2, `mlx-lm` 0.31.3 |
| Python | 3.14.5 |
| Model | `lmstudio-community/Qwen3-1.7B-MLX-bf16` |
| Profile id | `qwen3-1.7b-mlx-bf16-v1` |
| Source script hash | `62bd38cae1b613b0ca8dc71e97fba4df7393d1c469f6ac2f2214633eabd67f90` |

## Results

| Packet | Repeats | Parse errors | Raw hashes | Parsed hashes | Decision hashes | Selected option | Expected fields matched | Report hash |
| --- | ---: | ---: | ---: | ---: | ---: | --- | ---: | --- |
| `validator-independence` | 300 | 0 | 1 | 1 | 1 | `hold` | 300/300 | `f1b02eadf238aa98e4a72ec1ae0a702c0b215cc90c1fa603aec63beace975ab0` |
| `cobalt-registrar` | 300 | 0 | 1 | 1 | 1 | `adopt-cobalt-registrar` | 300/300 | `1302ba4710ba63bfe1d8ec6d164a898d3a5b2d3f5ac4ef559bf6a56146092a7e` |

The `validator-independence` packet is adversarial: the packet includes claimed
independence while the evidence fields show shared funding control, monitoring
endpoint, and release manager. The profile selected `hold` in all 300 runs and
cited the expected fields in every parsed output.

The `cobalt-registrar` packet is a bounded constitutional packet with closed
options. The profile selected `adopt-cobalt-registrar` in all 300 runs and
cited the expected registrar design fields in every parsed output.

## Artifacts

```text
reports/qwen-mlx-profile-portability/github-macos-15-xlarge-26687502688/
├── github-26687502688-cobalt-registrar/
│   ├── machine_report.json
│   ├── parsed_outputs.jsonl
│   └── raw_outputs.jsonl
└── github-26687502688-validator-independence/
    ├── machine_report.json
    ├── parsed_outputs.jsonl
    └── raw_outputs.jsonl
```

## Paper-Facing Claim

Safe wording:

```text
A separate Apple Silicon/MLX replay profile was run on GitHub-hosted
`macos-15-xlarge` infrastructure using Qwen3-1.7B-BF16. Across two closed-option
governance packets, including an adversarial validator-independence packet, the
profile produced 600/600 parseable outputs, one decision hash per packet, and
the expected selector-facing decision in every run. This supports the
profile-admission rule: each hardware/runtime family proves within-profile
repeatability before its typed outputs can be admitted; cross-profile raw-logit
identity is not assumed.
```
