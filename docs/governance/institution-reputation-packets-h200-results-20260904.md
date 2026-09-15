# H200 institution-reputation results from identity-only profiles: XRPL and PostFiat

**Date:** 2026-09-04

**Mode:** `SHADOW_ONLY`

**Replay verdict:** **PASS** (192/192 byte-identical across two distinct-owner H200-class hosts, two runs each)

**Supersedes:** [2026-09-03 packet results](institution-reputation-packets-h200-results-20260903.md), whose packets
carried validator-list membership lines in 42 of 55 cases and whose scoring prompt told the model to ignore
operational data instead of never showing it.

## Plain-English result

The researcher was given only network, validator key, and claimed domain, and asked who the
organization behind the validator is. The scorer was given only the resulting profile and asked
whether it genuinely recognizes that organization and, if so, what legitimacy and reputational
value it brings to a Layer-1. Nothing about lists, rounds, uptime, verification flags, hashes, or
shadow labels reached the model.

The same 55 requests ran twice on an H200 NVL in Czechia and twice on an H200 in Saudi Arabia,
different owners and driver versions. All 192 responses were byte-identical.

36 of 55 score zero; mean 14.64. Top: University of Waterloo 72, UNC Kenan-Flagler 72,
Australian National University 72, Ripple Labs 62, Blockdaemon 52, Interledger Foundation 48.
Versus the 09-03 run, 12 fell and 3 rose; five lost recognition because the
identity-only researcher reached stricter conclusions (two domain-less keys previously attributed to
Berkeley Haas and Bitso on registry hints are now "not established", since no primary source names
the key). All 20 PostFiat validators remain at 0: the model does not recognize Post Fiat or its
community operators as institutions.

## Determinism proof

| Measurement | Result |
| --- | --- |
| Model | `Qwen/Qwen3.8-27B-FP8`, revision `017b9c7af6b5689d5dd426a76e0bc077eb5ca20a` |
| Runtime | pinned SGLang image, loopback only, deterministic inference, seed `438916795` |
| Primary host | NVIDIA H200 NVL, driver 575.57.08, Vast host 214845 |
| Replay host | NVIDIA H200, driver 580.159.03, Vast host 445596 |
| Byte comparison | 192/192 identical; 165 scoring, 27 padding |
| Aggregate response SHA-256, all four runs | `9d6935e2c194441f7a09dc33324d276001ef7548dd6a371d7fd524a80ed4328f` |
| Comparison SHA-256 | `95a97ade7965d793d9731952c4b02afeebcf05e32fa2643c064258bcb627a4ef` |
| Identity corpus packet-set SHA-256 | `8051f392e60d84a687076dc241ddf722859db7c06718dd12139c3109548523df` |
| Package | [`benchmarks/ai-governance/institution-reputation-packets-20260904/`](../../benchmarks/ai-governance/institution-reputation-packets-20260904/README.md) |

## Per-validator scores

| Score | Prev packet run | Name-only | Network | Organization assessed | Sanctions risk |
| --- | --- | --- | --- | --- | --- |
| 72 | 62 | 0 | xrpl | University of Waterloo | negligible |
| 72 | 62 | 0 | xrpl | University of North Carolina Kenan-Flagler Business School | negligible |
| 72 | 72 | 0 | xrpl | Australian National University | negligible |
| 62 | 72 | 78 | xrpl | Ripple Labs Inc. | low |
| 52 | 57 | 57 | xrpl | Blockdaemon Inc. | low |
| 48 | 45 | 0 | xrpl | Interledger Foundation | low |
| 42 | 42 | 57 | xrpl | XRPL Commons | low |
| 42 | 45 | 72 | xrpl | Institute for Information Sciences (I2S), University of Kansas | negligible |
| 42 | 42 | 72 | xrpl | The Integrators B.V. | low |
| 42 | 42 | 52 | xrpl | GateHub Limited | low |
| 42 | 47 | 52 | xrpl | University of Nicosia | low |
| 42 | 42 | 0 | xrpl | Bitrue | low |
| 25 | 25 | 52 | xrpl | Bithomp | low |
| 25 | 27 | 52 | xrpl | Scrambled Egg Technologies, LLC | low |
| 25 | 25 | 0 | xrpl | AESTHETES S.R.L. | low |
| 25 | 27 | 0 | xrpl | Squid | low |
| 25 | 25 | 0 | xrpl | Aureus Ox LLC | low |
| 25 | 42 | 52 | xrpl | Peersyst Technology S.L. | low |
| 25 | 25 | 0 | xrpl | Anodos Labs Inc. | low |
| 0 | 0 | 0 | postfiat | — | unknown |
| 0 | 0 | 0 | postfiat | — | unknown |
| 0 | 0 | 0 | postfiat | — | unknown |
| 0 | 0 | 0 | postfiat | — | unknown |
| 0 | 0 | 0 | postfiat | — | unknown |
| 0 | 0 | 0 | postfiat | — | unknown |
| 0 | 0 | 0 | postfiat | — | unknown |
| 0 | 0 | 0 | postfiat | — | unknown |
| 0 | 0 | 0 | postfiat | — | unknown |
| 0 | 0 | 0 | postfiat | — | unknown |
| 0 | 0 | 0 | postfiat | — | unknown |
| 0 | 0 | 0 | postfiat | — | unknown |
| 0 | 0 | 0 | postfiat | — | unknown |
| 0 | 0 | 0 | postfiat | — | unknown |
| 0 | 0 | 0 | postfiat | — | unknown |
| 0 | 0 | 0 | postfiat | — | unknown |
| 0 | 0 | 0 | postfiat | — | unknown |
| 0 | 0 | 0 | postfiat | — | unknown |
| 0 | 0 | 0 | postfiat | — | unknown |
| 0 | 0 | 0 | postfiat | — | unknown |
| 0 | 25 | 0 | xrpl | — | unknown |
| 0 | 0 | 0 | xrpl | — | unknown |
| 0 | 57 | 0 | xrpl | — | unknown |
| 0 | 0 | 0 | xrpl | — | unknown |
| 0 | 25 | 0 | xrpl | — | unknown |
| 0 | 0 | 0 | xrpl | — | unknown |
| 0 | 78 | 0 | xrpl | — | unknown |
| 0 | 0 | 0 | xrpl | — | unknown |
| 0 | 27 | 0 | xrpl | — | unknown |
| 0 | 0 | 0 | xrpl | — | unknown |
| 0 | 0 | 0 | xrpl | — | unknown |
| 0 | 0 | 0 | xrpl | — | unknown |
| 0 | 0 | 0 | xrpl | — | unknown |
| 0 | 0 | 0 | xrpl | — | unknown |
| 0 | 0 | 0 | xrpl | — | unknown |
| 0 | 0 | 0 | xrpl | — | unknown |

## Correlation

Deterministic, model-free, from the profile summaries plus the input claimed domain: one strong
cluster (the three Post Fiat foundation validators); 106 of 1485 pairs share a weak signal.
See `outputs/correlation.md`.

## Interpretation

- With operational metadata removed from both prompts, the score is purely a judgment about the
  organization the profile identifies. Where the researcher could not establish an organization
  from primary sources, the scorer correctly returns 0 rather than crediting a registry hint.
- Scores remain `SHADOW_ONLY` research. The independent human publication review has not been
  performed. A profile is cited public research, not proof of legal identity or key control.
