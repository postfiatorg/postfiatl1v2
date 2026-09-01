# Deterministic Sub-Scorer Shadow Evaluation — 2026-09-01

Shadow evaluation of deterministic sub-scorers v1 against the eight frozen
testnet scoring rounds 12–19 of the PFT Ledger fork's Dynamic UNL pipeline
(milestone item C2 of the
[public-testnet path](../plans/active/l1v2-public-testnet-path-milestone.md)).
Under prompt v8+ the model's overall score is advisory; its authority is the
five dimensional sub-scores that feed the published deterministic score
formula. This evaluation replaces the model's judgment per dimension with
transparent rules computed from each round's frozen evidence and measures
what that substitution would have changed. It produces the evidence for the
operator's keep-or-demote decision on model authority (Gate Zero Z2); **it
does not make that decision**.

Harness: `benchmarks/ai-governance/dunl-subscorer-shadow-20260901/`
(`fetch_rounds.py`, `subscorer.py`, `shadow_eval.py`, machine-readable
`results.json`, per-file SHA-256 of every frozen artifact in
`rounds-manifest.json`). The fork's production `compute_final_score` and
`select_unl` are imported read-only from the `dynamic-unl-scoring` clone;
each round's selector parameters (cutoff, max size, min gap — the gap
changed 5→3 at round 19) and previous UNL come from that round's own frozen
manifest and inputs, never from constants. Rounds 9–11 predate the
frozen-input-package contract and are not replayable.

## Evaluation criteria (stated before results)

Judged in this order; the flip count is the headline number:

1. **Cutoff flips.** Validators crossing the score-40 eligibility line in
   either direction between the round's published authoritative baseline and
   the deterministic candidate. The baseline is era-correct: the score
   formula over the model's published sub-scores where the round's manifest
   pins `code.score_formula` (rounds 16–19), the model's overall score
   before that (rounds 12–15).
2. **UNL reproduction under frozen parameters.** Deterministic finals fed
   through the production selector with the round's frozen cutoff, max size,
   min gap, and previous UNL, compared seat-by-seat against the published
   selection. Any seat difference must be explainable from the scores.
3. **Per-dimension drift.** Mean and max absolute delta of the deterministic
   sub-scores against the model's published sub-scores, per dimension.
4. **Internal control.** The baseline scores themselves must reproduce the
   published UNL exactly under the frozen parameters before the candidate is
   compared; a control failure would invalidate the round's comparison.
5. **Cutoff-boundary cases.** Every validator whose final lands within 5
   points of 40 under either scorer is listed individually, whether or not
   it flips.

## Deterministic sub-scorer rules (v1)

Full rule statements live in the module docstring of `subscorer.py`; in
summary — consensus: floor of the worst frozen agreement window at full
resolution; reliability: rubric-shaped bands over where the losses sit in
time (recent windows vs 30-day residue); software: version ordering against
the round's own set (fee votes carry no signal in these rounds — `base_fee`
is uniformly 10); diversity: counts-based concentration penalties over
provider family and country, with the missing-endpoint policy at a flat 30
(rounds 12–16 carry no family-grouping evidence, so the raw ASN is the
provider key there); identity: domain evidence only (the formal identity
field is null in all eight rounds). Integer 0–100 outputs, no network, no
model, deterministic by construction.

## Results

Per-round outcome (full detail in `results.json` and `results-tables.md`):

| Round | Prompt | Baseline | n | Final Δ mean/max | Cutoff flips out/in | UNL overlap | Seats changed | Control |
|---|---|---|---|---|---|---|---|---|
| 12 | v5 | model score | 45 | 7.87 / 22 | **1** / 0 | 19/20 | 1 | pass |
| 13 | v5 | model score | 45 | 6.80 / 18 | 0 / 0 | 20/20 | 0 | pass |
| 14 | v6 | model score | 42 | 5.05 / 24 | 0 / 0 | 20/20 | 0 | pass |
| 15 | v6 | model score | 50 | 6.58 / 19 | 0 / 0 | 19/20 | 1 | pass |
| 16 | v8 | formula | 51 | 2.84 / 7 | **1** / 0 | 19/20 | 1 | pass |
| 17 | v9 | formula | 53 | 2.49 / 8 | 0 / 0 | 20/20 | 0 | pass |
| 18 | v9 | formula | 55 | 2.47 / 8 | 0 / 0 | 20/20 | 0 | pass |
| 19 | v10 | formula | 54 | 2.52 / 8 | 0 / 0 | 19/20 | 1 | pass |

**Headline: 2 cutoff flips across eight rounds (both eligible→ineligible,
none the other way).** The internal control reproduced the published UNL in
all eight rounds. All 12–19 artifacts fetched completely; no fetch or
evaluation failures.

Per-dimension mean absolute delta (deterministic vs model sub-scores):

| Round | consensus | reliability | software | diversity | identity |
|---|---|---|---|---|---|
| 12 | 4.02 | 16.22 | 0.00 | 21.22 | 0.33 |
| 13 | 4.93 | 19.00 | 0.67 | 17.44 | 1.33 |
| 14 | 5.29 | 14.88 | 0.71 | 15.48 | 0.60 |
| 15 | 4.68 | 20.40 | 0.00 | 14.90 | 0.80 |
| 16 | 0.69 | 16.67 | 10.00 | 23.33 | 0.78 |
| 17 | 0.43 | 2.45 | 0.00 | 16.66 | 1.32 |
| 18 | 0.33 | 5.09 | 0.00 | 15.27 | 1.36 |
| 19 | 0.37 | 3.98 | 0.00 | 17.96 | 1.39 |

Cutoff-boundary cases (final within 5 of 40 under either scorer — these two
are the only ones in all eight rounds, and both flipped):

| Round | Validator | Baseline final | Deterministic final | Flipped | Evidence |
|---|---|---|---|---|---|
| 12 | `nHU5tRRb…hqr5` | 45 | 23 | YES | 1h 0.0, 24h 0.069, 30d 0.952 — offline now; model scored consensus 80/reliability 60 on the 30-day history |
| 16 | `nHBWFVzx…CLYCN` | 41 | 37 | YES | 1h 1.0, 24h 0.165, 30d 0.773 — chronic 24h degradation; model reliability 85 (old-incident read) vs deterministic 10 (broken read) |

Seat changes in the four 19/20 rounds are all deep-in-the-pack reorderings
among validators scoring 85–94 under both scorers — rank and churn-control
movement far above the cutoff, not the eligibility-anomaly class:

| Round | Seat gained (base→det) | Seat lost (base→det) |
|---|---|---|
| 12 | `nHUzVW1u…` 85→93 | `nHDfSHLu…` 91→91 |
| 15 | `nHUdwzTW…` 82→90 | `nHBcnQZ9…` 88→91 |
| 16 | `nHBYKjxj…` 91→94 | `nHBgZupJ…` 92→89 |
| 19 | `nHBcnQZ9…` 92→90 | `nHBgZupJ…` 92→89 |

## Findings

**Where the deterministic rules agree with the model.** On the current-era
rounds (17–19, prompt v9/v10) consensus agrees almost exactly — mean
absolute delta ≤ 0.43, max 1 — because the model now applies the same
worst-window ceiling at full resolution that the deterministic rule states.
Identity (mean ≤ 1.4) and software (exact in 17–19) are equally close;
round 16's uniform 10-point software offset is the model's v8-era banding
(90 for current, 70 for outdated) against the deterministic 100/80 anchors,
a set-wide shift with no selection effect. Reliability converges to mean
2.5–5 in the v9/v10 era. Selection outcomes are near-identical everywhere:
19–20/20 overlap in every round, zero flips in six of eight.

**Where they diverge, and why.** Three real divergence classes:

1. **Old-era consensus generosity (round 12 flip).** The model scored an
   offline validator (dead 1h window, 6.9% 24h) at consensus 80 on the
   strength of its 30-day history, keeping it eligible at 45 overall. The
   deterministic worst-window rule scores consensus 0 and the consensus gate
   caps the final at 23. This is precisely the inconsistency class that
   motivated the score formula, and prompt v8+ semantics already score
   offline consensus at 0 — under the current regime this validator would
   not have been eligible either. The flip measures the old era, not a
   disagreement with the current model.
2. **Reliability on chronically degraded validators (round 16 flip).** For
   a validator holding a clean 1h window over a 16.5% 24h window, the model
   read a recovering incident (reliability 85) where the deterministic rule
   reads currently-broken (10). The published rubric places recent/chronic
   24h degradation at 40–70 and broken below 40, so the model's 85 is above
   its own rubric band; but the deterministic 10 is below it, and the flip
   margin is only 4 points — with a 40 instead of 10 the validator stays
   eligible at 41. This flip is real but parameter-sensitive, and the
   validator was already deep-degraded (consensus 16) under both scorers.
3. **Diversity curve shape (no selection effect).** Diversity is the one
   dimension that never converges: mean delta 15–23 in every round, max up
   to 50. The v1 linear count penalties and the model's judgment order
   concentration similarly but curve differently (v1 is harsher on megabloc
   members, gentler on mid-concentration providers). At 10% formula weight
   this never moved a cutoff outcome and at most one seat.

Both flips are in the strict direction — the deterministic rules never made
an ineligible validator eligible. No round shows the divergence class that
would indicate invented signal (large deltas on consensus or identity, the
evidence-anchored dimensions).

**What this means for the keep-or-demote decision.** The decision is the
operator's (Gate Zero Z2) and is not made here. What the numbers establish:
the selection surface actually at stake between the model's sub-scores and
fully deterministic v1 rules is small — over eight real rounds, two
eligibility flips (both stricter, one attributable to superseded old-era
semantics) and at most one seat per round, all deep-in-the-pack. A
demotion would therefore change little of the observed selection record,
and what it changes is defensible from frozen evidence. Conversely, the
dimensions where the model still exercises judgment that v1 rules render
crudely — the reliability band edge and the diversity curve — are exactly
the surfaces a demotion would hand to rules that are 4 points and one band
choice away from different outcomes; a keep retains that judgment at the
cost of non-recomputable authority. Both readings are consistent with this
data; the choice between them is a policy weighting, not a measurement.

**What a v2 of the sub-scorers would need.**

- Provider-family grouping evidence for rounds before 17 (or a published
  family map), so corporate ASN variants stop counting as distinct
  providers in historical replays.
- A governance-chosen diversity curve — diversity is the largest persistent
  divergence, and v1's linear penalties are one defensible choice among
  many; the curve should be picked deliberately, not inherited from v1.
- A deliberate reliability value for the chronically-degraded band edge:
  the round 16 flip moves with the broken-band constant (10 vs 40), so v2
  should pin that value against the rubric's "below 40" band with a stated
  rationale.
- Use of the `incomplete` measurement flags (present in the frozen evidence
  from round 17 on; v1 ignores them).
- Rules for fee votes and formal identity, currently no-ops because the
  frozen evidence is uniform (`base_fee` 10) and null (identity) across all
  eight rounds — v2 should state what happens when that stops being true.
- If sub-scores ever become authoritative, a content-hash pin and manifest
  section for the sub-scorer module, mirroring the parser/selector/formula
  conventions.

## Sub-scorers v2 (2026-09-01, second pass)

v2 addresses the three divergence classes above. It is a separate versioned
module — `subscorer_v2.py`, `SUBSCORER_VERSION = 2`, FORMULA-style — and v1
(`subscorer.py`) is untouched and stays reproducible; everything above this
section is the immutable v1 record. Artifacts: `results-v2.json` (with an
embedded per-round v1 comparison) and `results-tables-v2.md`, produced by
`shadow_eval.py --scorer-version 2`. Full rule statements live in the v2
module docstring; the changes:

1. **Era-aware consensus.** The rule now follows the round's own frozen
   prompt version: v8+ keeps the worst-window ceiling (v1's rule); v5/v6
   rounds get the era's whole-record reading — `floor(100·30d)` for
   participating validators — *with the era's own written non-participation
   penalty*: a dead 1h window pins consensus at 10, the value the era's
   model itself used for seven of its eight offline cases. A pure 30-day
   reading without that penalty was evaluated and rejected: it manufactured
   seven ineligible→eligible flips for validators even the era's model had
   scored ineligible.
2. **Rubric-text reliability banding.** Classification now keys on *which
   window carries the losses*, as the published rubric states: "broken"
   requires a dead 1h window; a clean 1h over a degraded 24h window is the
   recent/chronic band (40–70) and lands at its floor of 40, not at 10.
   The in-band ladders are v1's, which were already rubric-shaped.
3. **Rubric-banded diversity.** The linear count penalties are replaced by
   a band matrix over the same concentration counts (provider: unique /
   small / medium / dominant; country: rare / moderate / common), shaped to
   the model's observed banding in rounds 17–19, monotone in both axes,
   multiples of 5. Disclosed: this is in-sample fitting to the model's
   bands, and the model's own banding violates its monotonicity constraint
   in spots (e.g. round 18 scores a dominant-provider validator 40 in a
   2-count country but 60–75 in a 5-count country); v2 stays monotone, so
   those cells cannot be matched.

Software and identity rules are unchanged. The `incomplete` window flags,
fee votes, and formal identity are explicitly read-but-neutral with stated
revisit triggers in the module docstring (in the only round carrying the
`incomplete` flag, the model itself scored those windows at face value).

### v2 results

| Round | Prompt | n | Final Δ mean/max | Cutoff flips out/in | UNL overlap | Seats changed | Control |
|---|---|---|---|---|---|---|---|
| 12 | v5 | 45 | 8.84 / 23 | **1** / 0 | 20/20 | 0 | pass |
| 13 | v5 | 45 | 8.31 / 25 | 0 / 0 | 20/20 | 0 | pass |
| 14 | v6 | 42 | 6.02 / 25 | 0 / 0 | 20/20 | 0 | pass |
| 15 | v6 | 50 | 7.92 / 22 | 0 / 0 | 19/20 | 1 | pass |
| 16 | v8 | 51 | 2.80 / 8 | 0 / 0 | 19/20 | 1 | pass |
| 17 | v9 | 53 | 1.17 / 8 | 0 / 0 | 20/20 | 0 | pass |
| 18 | v9 | 55 | 1.36 / 8 | 0 / 0 | 20/20 | 0 | pass |
| 19 | v10 | 54 | 1.24 / 8 | 0 / 0 | 19/20 | 1 | pass |

**Headline: 1 cutoff flip across eight rounds under v2 (v1: 2), still
strictly in the eligible→ineligible direction — v2, like v1, never made an
ineligible validator eligible.** The internal control reproduced the
published UNL in all eight rounds.

### v1 → v2 comparison

Flips and overlap per round (v1 → v2):

| Round | Cutoff flips | UNL overlap | Seats changed |
|---|---|---|---|
| 12 | 1 → 1 | 19/20 → **20/20** | 1 → 0 |
| 13 | 0 → 0 | 20/20 → 20/20 | 0 → 0 |
| 14 | 0 → 0 | 20/20 → 20/20 | 0 → 0 |
| 15 | 0 → 0 | 19/20 → 19/20 | 1 → 1 |
| 16 | **1 → 0** | 19/20 → 19/20 | 1 → 1 |
| 17 | 0 → 0 | 20/20 → 20/20 | 0 → 0 |
| 18 | 0 → 0 | 20/20 → 20/20 | 0 → 0 |
| 19 | 0 → 0 | 19/20 → 19/20 | 1 → 1 |

Per-dimension mean absolute delta vs the model's sub-scores (v1 → v2;
unchanged dimensions elided — software and identity are identical in every
round, reliability changes only in round 16):

| Round | consensus | reliability | diversity |
|---|---|---|---|
| 12 | 4.02 → 3.60 | 16.22 | 21.22 → 14.67 |
| 13 | 4.93 → 5.07 | 19.00 | 17.44 → 10.56 |
| 14 | 5.29 → 5.31 | 14.88 | 15.48 → 6.55 |
| 15 | 4.68 → 4.38 | 20.40 | 14.90 → 10.80 |
| 16 | 0.69 | 16.67 → 16.08 | 23.33 → 15.88 |
| 17 | 0.43 | 2.45 | 16.66 → 6.79 |
| 18 | 0.33 | 5.09 | 15.27 → 3.36 |
| 19 | 0.37 | 3.98 | 17.96 → 4.72 |

Cutoff-boundary cases under v2 (the same two validators as v1):

| Round | Validator | Baseline final | v1 final | v2 final | Flipped under v2 |
|---|---|---|---|---|---|
| 12 | `nHU5tRRb…hqr5` | 45 | 23 | 28 | YES (out) |
| 16 | `nHBWFVzx…CLYCN` | 41 | 37 | 41 | no — reliability 40 (band floor) lands the final exactly at the baseline's 41 |

**What improved.** Diversity, the largest persistent v1 divergence, converges
sharply where family evidence exists (rounds 17–19: mean delta 3.4–6.8, was
15.3–18.0) and materially everywhere else. The round-16 flip resolves
in-band: the rubric-aligned chronic floor of 40 keeps the validator eligible
at exactly the baseline's 41, so the flip that moved with v1's 10-vs-40 band
constant is gone, with the classification now stated from rubric text rather
than an ad-hoc threshold. Era-aware consensus removes the artificial
0-versus-model gap on old-era offline validators (round 12 consensus max
delta drops 80→70, its UNL overlap reaches 20/20), and the headline falls to
one flip without ever loosening eligibility.

**What did not improve.** The round-12 flip itself persists, deliberately:
the era model's consensus 80 for that offline validator violated the era's
own written near-zero penalty policy (the same round scored a nearly
identical offline validator at 10), and v2 does not reproduce a
self-inconsistency — the flip now measures the era model's inconsistency
rather than an era-semantics mismatch in the rules. Old-era reliability
deltas (mean 14.9–20.4, rounds 12–16) are untouched: the v5/v6 reliability
rubric read different evidence (domain verification, UNL-membership
context), and v2 keeps one operational-stability rule for all eras rather
than reconstructing superseded semantics. Old-era consensus means barely
move (the 30-day and worst-window readings are near-identical for
participating validators; the max-38 outlier survives under both). Round
16's set-wide 10-point software offset remains by design, and rounds 12–16
diversity still lacks family-grouping evidence, so corporate ASN variants
keep counting as distinct providers there. One deep-in-the-pack seat still
changes in each of rounds 15, 16, and 19. The v1-requirements items that
remain open: a published provider-family map for historical rounds, the
governance choice of a diversity curve (v2 supplies a deliberate,
model-shaped candidate — not a decision), and the content-hash pin, which
only becomes relevant if sub-scores become authoritative.
