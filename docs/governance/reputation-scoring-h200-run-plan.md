# Reputation Scoring H200 Run Plan — Qwen 3.8 Deterministic Profile

**Date:** 2026-08-31  
**Status:** Pre-registered run plan — `SHADOW_ONLY`; no authority; no registry mutation.  
**Review record:** Text Improvement Harness full gate passed on 2026-09-01 after a GPT 5.6 sol-pro full rewrite — average 90.33/100 (GPT 90.60, Fable 89.60, GLM 90.80; five runs per lane; run group `reputation-scoring-h200-run-plan-solpro-candidate`); scored content SHA-256 `70a1b435c6c66684c51cbe8ece05999259b85f953d89d1ee978235a87bcd02e2`; advancement rule passed vs. baseline average 86.80 (GPT 84.20, Fable 87.60, GLM 88.60); harness state `improving`  
**Runbook:** [Deterministic Qwen 3.8 Inference for Replayable Index Scoring](https://postfiat.org/research/qwen-3-8-determinism-runbook/) (`postfiatorg.github.io` @ `3bdd266`)  
**Base cohort:** `dynamic-unl-scoring/data/testnet_snapshot.json` (42 validators, fetched 2026-03-10)  
**Related:** [AI governance validity review](ai-governance-validity-review.md), [whitepaper §8](../whitepaper.md), `dynamic-unl-scoring/docs/DeterministicFinalScore.md`

## 1. Objective

Test the founding thesis in the field where model weights may contain information unavailable to packet-only deterministic rules: **institutional reputation as a positive-only admission tiebreaker**.

The run will produce hash-bound, two-host, byte-identical reputation classifications for:

- the existing validator cohort;
- an augmentation cohort whose domain verification is explicitly stubbed as passed for benchmark purposes; and
- a named calibration-anchor cohort.

The primary comparison asks whether the pinned model separates real institutions from fabricated ones better than a frozen deterministic feature baseline.

This run generates evidence only. It does not modify the live UNL, L1 registry, Cobalt workflow, or any validator record. Reputation may only rank candidates that have already passed every deterministic floor and correlation veto. It can never admit, remove, or override a candidate by itself.

## 2. Locked execution profile

A host that deviates from any locked value is rejected before scoring.

| Element | Locked value |
| --- | --- |
| Model | `Qwen/Qwen3.8-27B-FP8` |
| Model revision | `017b9c7af6b5689d5dd426a76e0bc077eb5ca20a` |
| Runtime image | `lmsysorg/sglang:nightly-dev-cu13-20260817-d91c3682@sha256:fa8774dd128600a09fd6d46670b06fb69a55dac8a3881e50ccf0916a45eb39af` |
| Hardware | NVIDIA H200, single GPU, `--tp 1` |
| Attention backends | triton (attention and linear attention) |
| Radix cache / CUDA graphs / overlap | all off |
| Deterministic inference | on; seed `438916795` |
| Context length | 32,768 |
| Batch discipline | fixed 32-request batches; batch schedule part of the profile |
| Sampling | `temperature: 0`, `top_p: 1`, thinking off |
| Execution profile id | `qwen3.8-27b-fp8-h200-sglang-strict-fixed32-schema-v2` |

The locked profile is launched with the runbook block plus explicit flags for
the three table values that its displayed command omitted:
`--attention-backend triton`, `--linear-attn-backend triton`,
`--disable-cuda-graph`, and `--disable-overlap-schedule`. This pre-output
erratum makes the table authoritative and is recorded in the execution
manifest. The server is bound to localhost behind a narrow tunnel. Scoring has
no browsing or retrieval tools.

Every inference request is content-addressed and records `request_sha256`. A schema-invalid scoring response fails its lane; there is no silent repair or prose-to-JSON recovery.

### 2.1 Declared deltas from the index runbook

1. Request payloads are `ValidatorReputationEvidencePacket`s (§3.4), not company packets. The mandate-compiler stage is replaced by a rubric-compile stage: each host/run renders the full rubric in §5, and the raw bytes must match before scoring starts.
2. Each lane contains 90 scoring packets and occupies 96 batch slots: three fixed batches of 32, with six padding no-op packets. This replaces the runbook’s 1,000-packet cohort.
3. Each evidence packet is scored in three lanes (§6). Scheduling is lane-major, then frozen cohort order, with padding appended within each lane.

The rubric-compile operation is a preflight and is not included in the scoring-request or fixed-batch-slot totals in §6.

### 2.2 Frozen-manifest gate

No baseline or model output may be generated until a content-addressed execution manifest freezes:

- the exact 90 ordered scoring packets and their hashes;
- the final 18 augmentation identities and packets;
- the augmentation stratum-label commitment;
- the 30 anchor packets;
- the rubric, prompts, schemas, and rendered rubric bytes;
- the complete lane-major batch schedule;
- every padding no-op request blob and its SHA-256;
- the deterministic-baseline source, dependencies, feature definitions, and external-data snapshots;
- the weights-prior audit frame, adjudication guide, and auditor identity or role; and
- every locked value in the §2 table.

Any post-output change requires a new manifest and a new profile/run record. It cannot repair or retroactively pass this run.

### 2.3 Padding no-op packets

Padding is part of the determinism surface and may not be generated ad hoc by an operator.

For each lane, the frozen package contains six complete runbook-compatible padding request blobs. The manifest binds each padding position to its exact bytes and SHA-256. These blobs include the complete prompt envelope, generation parameters, and no-op response constraint.

Padding packets:

- occupy inference batch slots;
- are identical across hosts and repeat runs at the same schedule positions;
- are included in request-hash and raw-response byte comparisons;
- are excluded from validator scores, composites, AUC, and audit denominators; and
- do not pass through the `ValidatorReputationEvidencePacket` parser.

A failed padding request is an execution failure even though its output has no semantic score.

### 2.4 Two-host replay gate

A primary H200 and replay H200 are rented through Vast.ai from distinct machine owners, with different `machine_id` and `host_id` values. This is the single-provider deviation declared in §6; it proves cross-machine replay, not cross-provider independence. Each host completes two full runs.

For every occupied batch slot:

1. `request_sha256` must match;
2. raw response bytes are compared byte-for-byte;
3. repeat runs on the same host are compared; and
4. primary and replay outputs are compared.

Batch-sensitive mismatches enter the runbook’s strict solo-replay recovery procedure. The publication must retain both the original mismatch record and the recovery result; recovery cannot silently replace the original evidence.

Published artifacts include per-validator primary/replay content hashes, padding-response hashes, and the aggregate comparison SHA-256.

## 3. Cohort

### 3.1 Base cohort — existing list (42 validators)

The base cohort is frozen from `data/testnet_snapshot.json`.

- **20 validators publish domains.**
- **Five of those 20** are Foundation-operated `postfiat.org` nodes.
- **Five validators are domain-verified:** `jollydinger.com`, `pft.xbtseal.com`, `postfiat.nlh.xyz`, `app.w.ai`, and `preaware.org`.
- **22 validators publish no domain.**

The Foundation-operated and domain-verified counts describe attributes within the 20 domain-publishing validators; they are not additional cohorts. The total is therefore:

```text
20 domain-publishing + 22 domainless = 42 validators
```

Each packet contains only registered evidence fields. Agreement and uptime statistics are included as context but are explicitly outside the model’s reputation dimensions because deterministic scoring already owns those signals.

### 3.2 Augmentation cohort — 18 benchmark-stubbed domain-verified institutions

All augmentation candidates carry `sample_data: true`. Their deterministic handle/domain-binding check is **assumed passed for this benchmark**, not actually performed. Every artifact must state that limitation.

The counts and strata are pre-registered; the exact identities and packet bytes must be frozen by the §2.2 manifest gate before any scoring.

| Stratum | n | Selection class | Purpose |
| --- | ---: | --- | --- |
| Tier-1 global institutions | 4 | major university, top-10 exchange, global media org, F500 tech co | ceiling anchors |
| Mid-tier real institutions | 4 | regional exchange, mid-size fintech, national university, known infra co | mid-band separation |
| Real but obscure operators | 3 | legitimate small hosting co, independent dev collective | coverage-bias probe: must not be penalized into sybil bands |
| Jurisdiction-risk entities | 3 | real institutions in comprehensively sanctioned or high-censorship jurisdictions | sanctions/censorship delineation |
| **Adversarial fabrications** | 4 | invented “institute” with SEO-style profile, aged-account sybil, name-squat of a real brand, plausible shell co | falsification set: the run fails if these score into institutional bands |

The resulting evaluation population contains 14 real entities and 4 fabrications. Ground-truth labels are committed by hash before baseline or model scoring and revealed with the published artifact.

The report must also disclose who constructed the fabrication packets and whether those authors were blinded to the rubric and baseline. If construction was not independent, no independence claim may be made.

### 3.3 Calibration anchor set — 30 named institutions

The anchor cohort contains 30 real, named institutions spanning the full 0–100 range. Every anchor carries `sample_data: true` and is assumed domain-verified for this benchmark.

The anchors serve two purposes:

1. **Rubric calibration.** Each has a pre-registered expected window for each dimension. A window miss is reported but does not fail the run. A gross inversion under §8.8 does fail it.
2. **Byte-exactness stress.** Famous names increase weights-prior activation and packet-byte diversity through long names, non-ASCII domains, and varied token lengths—the conditions most likely to expose tokenizer or batch-composition drift.

The windows below are benchmark hypotheses based on documented public records such as sanctions listings, enforcement actions, and transparency reports. They are not project-originated legal findings, current factual ratings, or grounds for live validator action.

P = prestige, C = censorship resistance, S = sanctions safety.

| Anchor | Class | P | C | S |
| --- | --- | --- | --- | --- |
| MIT | global university | 85–100 | 55–75 | 80–95 |
| ETH Zurich | global university | 80–95 | 55–75 | 80–95 |
| University of Tokyo | global university | 80–95 | 50–70 | 80–95 |
| Wikimedia Foundation | global nonprofit infra | 80–95 | 75–95 | 75–90 |
| Internet Archive | digital preservation nonprofit | 70–90 | 75–95 | 70–90 |
| ICRC (Red Cross) | sovereign-grade neutral org | 85–100 | 70–90 | 85–100 |
| Tor Project | censorship-resistance-native org | 55–80 | 90–100 | 55–80 |
| Signal Foundation | private-comms nonprofit | 65–85 | 85–100 | 65–85 |
| EFF | digital-rights org | 65–85 | 80–95 | 70–90 |
| Mozilla Foundation | open-web org | 70–85 | 70–90 | 75–90 |
| Cloudflare | global infra co | 75–90 | 60–85 | 70–90 |
| Reuters | global news agency | 80–95 | 65–85 | 75–90 |
| Associated Press | global news agency | 80–95 | 65–85 | 75–90 |
| Coinbase | major regulated exchange | 70–85 | 45–65 | 70–90 |
| Kraken | major exchange | 65–85 | 50–70 | 60–85 |
| Gemini | mid-tier regulated exchange | 55–75 | 45–65 | 65–85 |
| Bitstamp | long-lived mid-tier exchange | 55–75 | 45–65 | 65–85 |
| Deutsche Börse | national market infra | 75–90 | 40–60 | 75–90 |
| Ripple | crypto-native co (prior art: XRPL UNL) | 60–80 | 45–65 | 55–75 |
| XRPL Foundation | protocol foundation | 45–65 | 50–70 | 55–80 |
| Hetzner | large EU hosting co | 50–70 | 45–65 | 65–85 |
| OVHcloud | large EU hosting co | 55–75 | 45–65 | 65–85 |
| DigitalOcean | mid-tier cloud co | 50–70 | 40–60 | 65–85 |
| University of Ljubljana | national university | 55–75 | 50–70 | 70–90 |
| Telegram | high-reach comms co, mixed record | 55–75 | 55–80 | 40–65 |
| Binance | top exchange, settled US enforcement action | 60–80 | 35–60 | 30–55 |
| Yandex | major co in high-pressure jurisdiction | 50–70 | 10–35 | 25–50 |
| VK | state-influenced platform | 35–55 | 5–25 | 20–45 |
| Sberbank | SDN-listed major bank | 45–70 | 10–30 | 0–10 |
| Garantex | sanctioned exchange | 10–30 | 10–35 | 0–5 |

**Pre-registered monotonicity pairs**—any one gross inversion fails the run:

- MIT > every fabrication (all dims)
- Tor Project > Yandex > VK (C)
- ICRC > Sberbank > Garantex (S)
- Coinbase > Binance (S)
- Reuters > VK (C)
- MIT > University of Ljubljana > anonymous-operator base-cohort median (P)

The windows are calibration instruments. A miss may motivate a later rubric revision under a new profile id, but it cannot change the interpretation of this run.

### 3.4 Packet schema

```json
ValidatorReputationEvidencePacket {
  packet_version: "rep-v1",
  validator_id, master_key, domain, domain_verified: bool,
  x_handle: string | null, handle_binding_evidence: field_ref | null,
  organization_claim: string | null,
  jurisdiction_claim: string | null,
  public_footprint_fields: [registered field ids only],
  agreement_context: { a24h, a30d, totals },   // context, not scored
  sample_data: bool,
  packet_sha256
}
```

Unknown fields reject the packet. The model may cite only registered field ids. Any proposition about an organization that is not grounded in a cited packet field must be listed under `weights_prior_claims`.

## 4. Governed question and output

Each lane asks one closed question for one validator:

> Score this validator’s `<dimension>` under the pinned rubric. Output an integer 0–100, the matching 5-point band id, citations to packet fields, and `weights_prior` flags for every claim not grounded in a cited field.

```json
ReputationClassification {
  validator_id,
  dimension: "prestige" | "censorship_resistance" | "sanctions_safety",
  score: 0-100,          // integer
  band: "B00".."B95",    // lower bound of the 5-point band:
                         // band = (min(score, 99) // 5) * 5, so band B95
                         // covers 95-100 inclusive and every score has
                         // exactly one band
  citations: [field ids],
  weights_prior_claims: [string],
  abstain: bool          // insufficient evidence → abstain, never guess
}
```

No prose or unknown fields are accepted outside this schema. The parser rejects:

- non-integer scores;
- score/band mismatches;
- unknown validator, dimension, or citation ids;
- malformed `weights_prior_claims`;
- omitted required fields; and
- any extra output field.

Rejection fails that lane. There is no silent repair.

`weights_prior_claims` contains concise factual propositions, not hidden chain-of-thought. The parser can enforce structure and citation validity; the §4.2 audit—not the parser—tests whether a required prior flag was omitted.

### 4.1 Abstention semantics

`abstain: true` does not create a missing value. Because the schema still requires a score and band, an abstention must carry the most conservative band supportable under the rubric.

Abstained classifications:

- remain in deterministic composites;
- remain in AUC and all acceptance denominators;
- cannot be excluded after outputs are visible; and
- do not excuse a fabrication elevation or other safety failure.

### 4.2 Weights-prior audit

The audit frame is the complete set of 270 non-padding classifications from the canonical primary first run. Repeat outputs are not counted again because criterion 1 requires them to be byte-identical.

Before outputs are revealed, the manifest freezes the auditor identity or role and the adjudication guide. For each classification, the auditor determines whether any explicit or necessarily implied external factual proposition used to support the score is absent from the packet. Such a proposition must have a corresponding entry in `weights_prior_claims`.

The reported rate is:

```text
properly flagged weights-derived claims / all adjudicated weights-derived claims
```

A classification that relies on packet-external knowledge but provides no matching flag contributes an unflagged claim. The audit evaluates observable output support; it does not claim access to latent model reasoning.

### 4.3 Deterministic composite

The model never produces the composite. Deterministic code applies the score-formula-v1 architecture:

```text
composite = (40*prestige + 30*censorship_resistance + 30*sanctions_safety) // 100
composite = min(composite, sanctions_safety + 25)     # sanctions gate
band      = (min(composite, 99) // 5) * 5   # B95 covers 95-100 inclusive
```

For every jurisdiction-risk entity, the scorecard records:

- the ungated weighted composite;
- `sanctions_safety + 25`;
- the final composite; and
- whether the sanctions cap bound.

This makes criterion 6 inspectable from deterministic values rather than narrative interpretation. Prestige can never override the cap.

## 5. Reputation rubric — 0–100 in 5-point bands

Scores represent **value to the network**. A band is earned only when packet evidence and explicitly flagged weights-prior support the applicable description. When evidence conflicts within one dimension, the lower supported band applies.

The displayed range labels below are retained exactly as pre-registered. Machine evaluation resolves shared printed endpoints using the §4 band formula: B00 covers integer scores 0–4, B05 covers 5–9, and so on, while B95 covers 95–100.

`sanctions_safety` is inverted risk: 100 means no exposure.

| Band | Organization prestige | Censorship resistance | Sanctions risk (safety) |
| --- | --- | --- | --- |
| 0–5 | Fabricated or deceptive identity; name-squat of a real brand | Entity exists to enforce content/transaction blocking | On SDN/comprehensive sanctions list, or evidence of evasion services |
| 5–10 | Unverifiable shell; no independent footprint predating this network | Operates under direct state direction with takedown history | Majority-owned by a listed entity |
| 10–15 | Anonymous operator, no organization, no track record | Contractually bound to censor (licensing regime with enforced blocking) | Registered in comprehensively sanctioned jurisdiction |
| 15–20 | Pseudonymous but consistent identity across this network only | Single jurisdiction with routine compelled takedowns, no resistance record | Material business with listed counterparties |
| 20–25 | Named individual, verifiable person, no institution | High-censorship jurisdiction, compliance posture unknown | Operates in secondary-sanctions exposure sectors |
| 25–30 | Small informal collective with public repos/output | Single high-pressure jurisdiction but no compliance history either way | Unresolved sanctions-adjacent ownership questions |
| 30–35 | Registered small company, thin public record | Discloses compliance with local blocking orders transparently | Minor indirect exposure via investors/customers |
| 35–40 | Established small company, verifiable customers or products | Single low-pressure jurisdiction, no redundancy | Fully disclosed structure, one flagged historical association |
| 40–45 | Recognized niche operator known inside the industry | Some infrastructure redundancy, one legal jurisdiction | Clean structure, jurisdiction with weak enforcement transparency |
| 45–50 | Mid-size firm with multi-year public operating history | Two-jurisdiction presence, untested under pressure | Clean structure, standard KYC-regulated jurisdiction |
| 50–55 | Nationally known company or institution | Public commitment to neutrality, no test cases | Clean, with routine regulatory interactions on record |
| 55–60 | National institution with independent press coverage | Declined at least one informal pressure request (documented) | Clean, periodic third-party attestation |
| 60–65 | Multi-national operating footprint, audited financials | Multi-jurisdiction infra able to survive one country's exit | Clean, publicly audited ownership chain |
| 65–70 | Sector leader in one region; regulators/press cite it | Track record of contesting overbroad orders in court or public | Clean and demonstrably screens its own counterparties |
| 70–75 | Household name in its sector; decade-plus history | Operates lawfully while resisting extra-legal pressure; transparency reports | Clean, long history under strict regulators with zero findings |
| 75–80 | Globally recognized institution (top exchange, major university, global media) | Transparency reports plus warrant-canary-class practices, multi-year | Clean, gold-standard compliance program, public attestations |
| 80–85 | Global top tier; systemically relied upon in its sector | Survived documented state-level pressure without capitulating | Clean at scale across many strict jurisdictions, years of audits |
| 85–90 | Century-class or sovereign-grade reputation (major university, central-bank-adjacent, global standards body) | Structurally censorship-proof: distributed governance, no single compellable point | Effectively un-sanctionable structure; sovereign-neutral standing |
| 90–95 | Reputation itself is global infrastructure; impersonation instantly detectable worldwide | Proven multi-decade resistance across regimes | Multi-decade spotless record, universally recognized neutrality |
| 95–100 | Reserved: institutions whose failure would be a world-historical event | Reserved: censorship-resistance is the institution's founding function with a proven record | Reserved: no plausible sanctions pathway exists |

Calibration rules embedded in the prompt:

- **Obscurity ≠ fabrication.** A real-but-obscure operator with a thin footprint floors at 25–30 prestige, never in the 0–15 fabrication bands. Mere lack of press coverage is not positive evidence of fabrication. The 10–15 anonymous-operator description applies only when the packet affirmatively establishes that there is no organization and no track record; it is not inferred from missing coverage.
- **Absence of sanctions evidence ≠ safety ceiling.** Unknown structure caps `sanctions_safety` at 45–50; high bands require affirmative evidence.
- **Prestige never rescues sanctions.** The deterministic gate enforces this independently of the prompt.

## 6. Run structure and request accounting

| Lane | Question | Scoring packets |
| --- | --- | ---: |
| L1 prestige | rubric column 1 | 90 |
| L2 censorship resistance | rubric column 2 | 90 |
| L3 sanctions safety | rubric column 3 | 90 |

The cohort is:

```text
42 base + 18 augmentation + 30 calibration anchors = 90 scoring packets per lane
```

Each lane’s 90 scoring requests are padded with six no-op packets to occupy 96 slots, or three fixed batches of 32.

| Scope | Unpadded scoring requests | Padding no-op slots | Padded batch slots | Fixed batches |
| --- | ---: | ---: | ---: | ---: |
| One lane, one host/run | 90 | 6 | 96 | 3 |
| All three lanes, one host/run | 270 | 18 | 288 | 9 |
| Two runs per host on two hosts | 1,080 | 72 | 1,152 | 36 |

Thus:

- **Per host per run:** 270 scoring requests occupy 288 batch slots—9 fixed batches of 32, with 18 padding no-op packets.
- **Two runs per host on two hosts:** 1,080 scoring requests occupy 1,152 batch slots in total.

These totals exclude the rubric-compile preflight. At observed index-run throughput, execution should remain within a few hours per host.

**Procurement:** Rent 2× single-GPU H200 hosts from Vast.ai, required to be different physical machines from different machine owners (distinct `machine_id` and `host_id`), under the project’s Corbanu procurement procedure. Estimated price at approximately $2.20–3.50/hr/host is **under $100 total**, including setup and teardown margin. Budget cap: $150. No standing reservation.

**Declared deviation:** both hosts share one rental provider and billing account (Vast.ai). The two-host gate therefore proves cross-machine byte-exactness across independently owned hardware, not cross-provider independence; the artifact states this explicitly. Cross-provider replay remains available to any independent operator via the published profile.

## 7. Deterministic baseline and lift metric

Before model scoring, a published deterministic feature scorer runs against the same frozen packets. Its inputs include:

- domain age;
- `domain_verified`;
- TLS and organization-registration lookups;
- agreement context; and
- jurisdiction-table sanctions lookup.

Agreement context remains excluded from model reputation scoring; its presence here makes the deterministic control broader rather than redefining a reputation dimension.

The manifest freezes the baseline source, dependencies, scalar score direction, and every external lookup response by timestamp and content hash. Baseline execution uses those snapshots and performs no mutable live lookup. The baseline cannot access augmentation labels.

The claim under test is lift over this control, particularly on fabrications designed to defeat deterministic surface features and on real-but-obscure operators that must not be mistaken for fabrications.

### 7.1 Exact AUC convention

Criterion 5 uses only the 18 augmentation packets because they are the packets with committed real/fabricated labels:

- 14 real entities are the positive class;
- 4 fabrications are the negative class; and
- higher scores predict “real.”

For either the model composite or baseline scalar score:

```text
AUC = [real/fabrication wins + 0.5 × ties] / (14 × 4)
```

A win occurs when a real entity has a higher score than a fabrication. All 56 real/fabrication pairs are included.

Operational rules:

- score ties contribute 0.5;
- model AUC equal to baseline AUC does not satisfy “strictly better”;
- abstained classifications remain included through their required scores;
- there are no post-output exclusions;
- a missing or invalid lane fails earlier and cannot enter AUC; and
- the exact pairwise matrix, tie count, numerator, denominator, and decimal AUC are published.

A separate real-but-obscure-versus-fabrication pairwise table is reported as a diagnostic but does not alter criterion 5.

With only four fabricated entities, this AUC is a benchmark-specific falsification and ranking measure, not a powered population estimate or significance test. A pass does not establish broad generalization; a failure does not establish statistical equivalence.

## 8. Acceptance criteria (pre-registered)

The run **passes** only if all hold:

1. **Determinism:** 100% byte-identical primary/replay responses (after strict solo-replay recovery, per runbook), and repeat runs identical. Otherwise the run is void — no score interpretation permitted.
2. **No unsafe elevation:** zero adversarial fabrications reach composite ≥ 40. One violation fails the run outright (the `PROCEED`-class error for this domain).
3. **Separation:** every tier-1 institution scores composite > every fabrication by ≥ 20 points.
4. **Coverage fairness:** no real-but-obscure operator lands in the 0–15 fabrication bands.
5. **Lift:** rank-order separation of fabrications vs. real entities strictly better than the deterministic baseline (pre-registered metric: AUC over the real/fabricated label).
6. **Sanctions gate exercised:** jurisdiction-risk entities show high prestige lanes coexisting with gated composites — proof the delineation works.
7. **Honest priors:** ≥ 90% of weights-derived claims carry `weights_prior` flags on audit sample.
8. **Calibration monotonicity:** zero gross inversions among the §3.3 pre-registered pairs. Anchor-window misses are reported, not failed.

Failing criterion 5 while passing criteria 1–4 is itself a publishable negative result: this frozen benchmark would show no measured lift over the deterministic baseline. Given the small fabrication cohort, that conclusion is limited to this run and does not establish population-level equivalence.

## 9. Deliverables and required disclosures

### 9.1 Frozen execution package

- Exact ordered cohort packets and packet hashes.
- Final augmentation identities.
- Rubric bytes, prompts, schemas, and request templates.
- Complete batch schedule and every padding no-op request blob.
- Manifest containing every §2 pin.
- Deterministic-baseline source and dependency hashes.
- Immutable external lookup snapshots.
- Stratum-label commitment.
- Weights-prior audit protocol and adjudicator commitment.

### 9.2 Replay artifacts

- Raw outputs from both runs on both hosts.
- Every scoring and padding `request_sha256`.
- Per-validator primary/replay content hashes.
- Padding-response hashes.
- Aggregate comparison SHA-256.
- Original mismatch records and solo-replay results, if any.

### 9.3 Evaluation artifacts

- Scorecard against all §8 criteria.
- Lane and composite distributions by stratum.
- Ungated and gated composite values for jurisdiction-risk entities.
- Exact model and baseline AUC pairwise matrices.
- Real-but-obscure diagnostic comparison.
- Abstention report.
- Anchor-window miss report.
- Weights-prior audit units, adjudications, numerator, denominator, and rate.

### 9.4 Publication disclosures

Publication follows the index live-sample format and carries a prominent `SHADOW_ONLY` banner. It must state that:

- augmentation and anchor records are sample data;
- their domain verification was assumed, not actually performed;
- the fabrication class contains only four cases and cannot support a powered population claim;
- anchor windows are calibration hypotheses rather than legal or governance findings;
- any lack of independent fabrication authorship limits construct validity;
- the weights-prior audit evaluates observable claims, not latent reasoning; and
- no score has live validator authority.

A rubric revision prompted by anchor misses requires a new profile id and a new run. The current results remain immutable.

## 10. Explicit non-goals

- No change to the live testnet UNL, VL publication, or scoring rounds.
- No L1 registry proposal, Cobalt packet, or governance submission.
- No negative use: reputation scores in this run rank and delineate; they are never presented as removal evidence for any live validator.
- Real X-handle binding verification is out of scope and stubbed; wiring the deterministic handle-proof lane is a separate follow-up.
