# Governance Inference Provider Bringup - 2026-05-23

Status: provider automation ready; no paid GPU currently running.

## Purpose

`docs/research-requests/heavy_redesign.md` adds deterministic governance-agent
gates that need a pinned Qwen/SGLang endpoint for replay testing. The local
operator path should not depend on manually driving provider CLIs.

## Local Automation

The operational wrapper is:

```text
scripts/gov-inference-provider
```

It reads provider keys from environment variables first, then from:

```text
$REPOS_ROOT/aicr.txt
```

The key file is now mode `600`. The wrapper does not print secrets. Paid
provider mutations require an explicit `--execute`.

## Verified State

- `autonomous_ai_org` is present at `$REPOS_ROOT/autonomous_ai_org`.
- The autonomous repo virtualenv was created at
  `$REPOS_ROOT/autonomous_ai_org/.venv`.
- `tensorcash-runpod` is installed and callable through the wrapper.
- `vastai` is installed and callable through the wrapper.
- RunPod API key works.
- Vast API key works.
- RunPod inventory: 25 pods total, 0 running pods.
- Vast inventory: 0 active instances.
- RunPod exposes usable GPU classes including H100, H200, A100 80GB, RTX 6000
  Ada, and RTX PRO 6000 Blackwell classes.
- RunPod dry-run SGLang payload was validated for the autonomous repo's `6090`
  alias and for the wrapper's `h100` governance-gate default.
- The SGLang payload uses
  `Qwen/Qwen3.6-27B-FP8`, TP=1, `--enable-deterministic-inference`,
  `--max-running-requests 1`, and the pinned SGLang image used by the
  autonomous repo.
- Vast search using the GB-style `gpu_ram>=80` filter returns offers, including
  H100 NVL and RTX PRO 6000-class machines. The older `gpu_ram>=80000` query is
  too strict for the Vast CLI filter syntax on this machine.

## No-Spend Commands Used

```text
scripts/gov-inference-provider check
scripts/gov-inference-provider bootstrap
scripts/gov-inference-provider inventory
scripts/gov-inference-provider runpod-gpus
scripts/gov-inference-provider runpod-create-sglang --gpu 6090 --name postfiat-gov-agent-qwen36-sglang
scripts/gov-inference-provider runpod-create-sglang --name postfiat-gov-agent-qwen36-sglang
scripts/gov-inference-provider vast-search --query 'rentable=true verified=true num_gpus=1 gpu_ram>=80 direct_port_count>=2' --limit 20 --output reports/gov-inference-provider/vast-offers-gpuram80-latest.json
scripts/gov-inference-provider vast-create-sglang 32381911 --label postfiat-gov-agent-qwen36-sglang
```

The last command was a dry run and did not create a Vast instance.

## Execution Path

Use RunPod first for the governance-agent endpoint because the autonomous repo
already has a complete RunPod SGLang launcher and wait/stop/delete lifecycle.
Use Vast as the fallback when RunPod capacity is unavailable or when a specific
SKU comparison is required.

Paid RunPod launch shape:

```text
scripts/gov-inference-provider runpod-create-sglang --gpu h100 --name postfiat-gov-agent-qwen36-sglang --execute
scripts/gov-inference-provider runpod-wait <pod_id>
```

Paid Vast fallback shape:

```text
scripts/gov-inference-provider vast-search --query 'rentable=true verified=true num_gpus=1 gpu_ram>=80 direct_port_count>=2' --limit 20 --output reports/gov-inference-provider/vast-offers-gpuram80-latest.json
scripts/gov-inference-provider vast-create-sglang <offer_id> --label postfiat-gov-agent-qwen36-sglang --execute
```

Cleanup shape:

```text
scripts/gov-inference-provider runpod-stop <pod_id>
scripts/gov-inference-provider runpod-delete <pod_id>
scripts/gov-inference-provider vast-destroy <instance_id> --execute
```

Do not leave a paid provider resource running after the deterministic
governance-agent smoke window unless there is an active long-running test.
