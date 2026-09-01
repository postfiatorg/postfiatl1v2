# Institution reputation replay: XRPL + PostFiat UNLs

**Frozen:** 2026-09-01T17:17:45Z

**Status:** `SHADOW_ONLY`

**Verdict:** `PASS`

This package measures institutional legitimacy for the current XRPL publisher
lists and the current published PostFiat UNL. It is evidence only: inference is
not called by consensus and these scores do not mutate either validator list.

## Result

Two distinct-owner Vast.ai hosts ran the same pinned request set twice each:

| Host | GPU | Vast host / machine | Runs | Aggregate response hash |
| --- | --- | --- | --- | --- |
| replay | NVIDIA H200 | `317686 / 32374` | 2 | `a1875309748195422b6bdfd0ac951fda54930a4d0bd3c7090026d1250a7c45cf` |
| primary | NVIDIA H200 NVL | `178654 / 35900` | 2 | `a1875309748195422b6bdfd0ac951fda54930a4d0bd3c7090026d1250a7c45cf` |

All **165/165 validator comparisons** and **27/27 fixed-batch padding
comparisons** matched as raw UTF-8 response bytes. The comparison-array SHA-256
is `7b10b3ba83b79820b48a4355a2a8f12ee021f8f8c5a1634de42a6f2486f088f7`.
There were zero failures.

The frozen set contains 35 XRPL validators and 20 PostFiat validators. The two
XRPL publisher endpoints independently returned the same 35-member set. The
PostFiat set is completed scoring round 20.

## Pinned execution profile

- model: `Qwen/Qwen3.8-27B-FP8`
- revision: `017b9c7af6b5689d5dd426a76e0bc077eb5ca20a`
- SGLang image: the digest pinned in `manifest.json`
- deterministic inference enabled; seed `438916795`
- temperature `0`, top-p `1`, thinking disabled
- radix cache, CUDA graphs, and overlap scheduling disabled
- Triton attention and linear-attention backends
- two fixed batches of 32 requests
- loopback-only self-hosted inference; **OpenRouter was not used**

The prompt defines every five-point band from 0–4 through 95–100. Recognition
and a plausible institution/domain match are mandatory; otherwise the exact
score is zero. Recognized institutions are judged on prestige,
sanctions/integrity risk, and reputational value to a Layer-1.

## Inputs

- XRPL publisher snapshots:
  - `https://vl.ripple.com`
  - `https://unl.xrplf.org`
- XRPL public validator metadata snapshot:
  - `https://api.xrpscan.com/api/v1/validator`
- PostFiat current published UNL:
  - `https://scoring-testnet.postfiat.org/api/scoring/unl/current`
- PostFiat round-20 validator map and model request

Every source, validator set, prompt, request file, and batch schedule is
content-addressed in `manifest.json`.

## Files

- `sources/`: frozen public source responses
- `inputs/prompt.txt`: exact scoring prompt and bands
- `inputs/validators.json`: normalized 55-member scoring set
- `inputs/requests.json`: exact model requests
- `outputs/*-run*.json`: four complete raw response records
- `outputs/comparison.json`: all 192 byte comparisons
- `outputs/scores.json`: canonical per-validator scores and explanations
- `outputs/rental-teardown.json`: redaction-safe destruction receipt
- `build_package.py`: source fetch and package builder
- `bootstrap_host.sh`: pinned local SGLang launch
- `run_host.py`: fixed-batch executor and response validator
- `compare_runs.py`: byte-exact comparator
- `test_package.py`: package invariants

## Verify

Without running inference:

```bash
python3 test_package.py
python3 compare_runs.py
```

The rentals were destroyed after outputs were downloaded. The complete session
credit delta was approximately **$5.20**, including two discarded rental
attempts. No inference credential or external model API was used.
