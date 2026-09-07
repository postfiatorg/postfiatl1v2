# H200 identity-packet replay results: XRPL and PostFiat

**Date:** 2026-09-03

**Mode:** `SHADOW_ONLY`

**Replay verdict:** **PASS**

**Human review status:** The independent publication review required by the
[identity-packet runbook](../runbooks/validator-identity-packets.md) Section 8
has not been performed.

## Plain-English result

The exact frozen Markdown packets for 35 XRPL validators and 20 PostFiat
validators were scored twice on an H200 and twice on an H200 NVL rented from
two distinct Vast.ai owners. Every validator response and padding response was
identical byte-for-byte across all four runs. All four runs had zero invalid
responses.

The packet-based scorer marked 22 of 55 validators recognized with non-zero
scores and 33 not recognized with score 0. All 22 positive results were XRPL;
all 20 PostFiat validators scored 0. That result is deliberately narrower than
the packet research's descriptive finding of 45 identities established or
likely and 10 not established: the score prompt additionally requires the model
to independently recognize an institution.

## Frozen inputs

| Input | Frozen value |
| --- | --- |
| Validator set | 35 XRPL plus 20 PostFiat validators |
| Identity corpus | [`validator-identity-packets-20260901`](https://github.com/postfiatorg/postfiatl1v2/blob/main/benchmarks/ai-governance/validator-identity-packets-20260901/README.md) |
| Corpus verification | 55/55 `PASS`; independent human publication review not performed |
| Packet-set SHA-256 | `b198e232baa644731b38e2f6db3989c798156700ebc67856a193b32bb941d4bd` |
| Corpus-manifest SHA-256 | `db4257138e3a3facf366631e6b6f62d70a7e99959b577c2c16e0f05f62f838ac` |
| Prompt SHA-256 | `25f5f32c79c2583bc171f745e7aded86244542796299ea0eabcf7acdadab2ccc` |
| Requests SHA-256 | `ab99350528853f90eb02b2012f89f40ab7dbfa2c207a20a9c757cddc72a0a0b9` |
| Schedule | 55 scoring plus 9 fixed padding slots in two batches of 32 |

Each scoring request embeds the exact packet bytes and binds the network,
validator key, packet SHA-256, and packet-set SHA-256. The replay did not use
live web search, OpenRouter, another agent run, or the corpus JSONL logs.

## Hosts

| Role | GPU identity | Instance | Host / machine | Rate | Start | Destroyed |
| --- | --- | ---: | --- | ---: | --- | --- |
| Primary | NVIDIA H200; driver 580.159.03 | 49731139 | 445596 / 131697 | $4.017543859649122/h | 2026-09-03 08:38:51Z | 2026-09-03 08:52:10Z |
| Replay | NVIDIA H200 NVL; driver 610.43.02 | 49734743 | 207608 / 49537 | $4.054666666666667/h | 2026-09-03 09:35:28Z | 2026-09-03 09:47:14Z |

The primary start is the retained instance `created_at`; the replay start is
the workload `START` recorded by the watchdog. Both used Python 3.12.3. The
different Vast host IDs record the distinct-owner placement used for the gate.

## Pinned settings

- Model `Qwen/Qwen3.8-27B-FP8`, revision
  `017b9c7af6b5689d5dd426a76e0bc077eb5ca20a`.
- Runtime image
  `lmsysorg/sglang:nightly-dev-cu13-20260817-d91c3682@sha256:fa8774dd128600a09fd6d46670b06fb69a55dac8a3881e50ccf0916a45eb39af`.
- Deterministic inference enabled with seed `438916795`; temperature 0,
  top-p 1, thinking disabled, and maximum output 2,048 tokens.
- Radix cache, CUDA graphs, and overlap scheduling disabled; Triton attention
  and linear-attention backends; loopback-only inference.
- Two runs per host, fixed batch size 32, and maximum 32 running requests.

## Determinism proof

The comparator uses the raw UTF-8 bytes of
`choices[0].message.content`. It compares `primary-run1` with each of the other
three runs, so every one of the 55 validator slots contributes three
comparisons and every one of the 9 padding slots contributes three.

| Measurement | Result |
| --- | ---: |
| Validator slots identical across all four runs | **55/55** |
| Validator byte comparisons | **165/165 identical** |
| Padding slots identical across all four runs | **9/9** |
| Padding byte comparisons | **27/27 identical** |
| Total | **192/192 identical; 0 failures** |
| Invalid responses | **0 in every run** |
| Aggregate response SHA-256, every run | `824820af71f14df0a11a9237a7ff71e2e8684350bd6d3a0161f18a7bccfcad4e` |
| Comparison SHA-256 | `56d1616f7e903f51057687008f91d2e56053430de4d169862ac050ef50cb8d0c` |

The four raw runs and complete per-slot comparison are in the
[replay package](https://github.com/postfiatorg/postfiatl1v2/blob/main/benchmarks/ai-governance/identity-packet-replay-20260903/README.md).

## Scores and explanations

The score schema's `recognized` field is the model's answer under the frozen
prompt, not a legal or factual adjudication of identity. The output totals are:

| Network | Recognized / non-zero | Not recognized / zero | Total |
| --- | ---: | ---: | ---: |
| XRPL | 22 | 13 | 35 |
| PostFiat | 0 | 20 | 20 |
| **Total** | **22** | **33** | **55** |

The exact score distribution is 0 (33), 25 (5), 27 (2), 42 (5), 52 (3), 57
(2), 62 (2), 72 (2), and 78 (1). The two-or-three-paragraph explanations and
per-validator results are retained in
`benchmarks/ai-governance/identity-packet-replay-20260903/outputs/scores.json`.

The descriptive corpus and scoring output cross-tab as follows. These are
separate classifications: the corpus asks whether public identity is
established or likely, while the scorer requires independent model recognition
of an institution before awarding any positive score.

| Packet descriptive finding | Scorer recognized | Scorer not recognized | Total |
| --- | ---: | ---: | ---: |
| Identity established or likely | 22 | 23 | 45 |
| Identity not established | 0 | 10 | 10 |
| **Total** | **22** | **33** | **55** |

The scorer rejected all 10 identities that the packets did not establish, but
also rejected all 12 individual profiles, 10 micro profiles, and 1 small
profile that the packet research described as established or likely. Its
explanations consistently cite either the prompt's individual-person exclusion
or lack of independent recognition for a niche or young entity. That divergence
does not overturn the packet findings; it shows that recognition-first
institutional scoring is much narrower than descriptive identity research.

## Rental teardown

Both successful instances were destroyed after their outputs were collected,
and the retained local receipts record them absent after destruction. Replay
instance 49734743 records the exact reason `completed and outputs collected`;
the primary receipt records `outputs_collected=true`, successful destruction,
and post-destroy absence.

Three unsuccessful attempts were also destroyed: 49731142 and 49732579 failed
to leave GPU provisioning, while 49733887 failed because its reverse tunnel
could not bind. Their retained receipt does not contain rates or exact
start/destroy timestamps, so the teardown artifact leaves those fields null.

| Account measurement | USD |
| --- | ---: |
| Credit before campaign | 90.18945196145705 |
| Credit after campaign | 85.36298219145874 |
| **Total spend, including discarded attempts** | **4.82646976999831 (about $4.83)** |

The redaction-safe machine-readable receipt is
`benchmarks/ai-governance/identity-packet-replay-20260903/outputs/rental-teardown.json`.
It contains no API keys or secrets.

## Boundary

This result is `SHADOW_ONLY` research. It is not consensus data, validator
weight, validator admission or removal authority, proof of current validator
key or domain control, a sanctions determination, or public-production
evidence. The independent human publication review remains incomplete. The
result applies only to the frozen inputs and pinned execution profile above;
changing the packets, prompt, model, revision, runtime image, or inference
settings creates a new experiment.

## Reproduce the local checks

```bash
cd benchmarks/ai-governance/identity-packet-replay-20260903
python3 test_package.py
python3 compare_runs.py
```
