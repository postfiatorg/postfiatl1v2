# AI Governance Direction Decision

**Status:** Direction decision — Text Improvement Harness full gate passed on 2026-09-03 — average 88.80/100 (GPT 86.80, Fable 89.60, GLM 90.00; five runs per lane; run group `ai-governance-direction-20260903`); scored content SHA-256 `321c1ae09a074363a646c3d2bc3bab79722a7672cfadfb5b8855e6053546e162`

**Date:** 2026-09-03

**Author:** Domagoj Ravlić (`dravlic`)

**Decision owner:** Post Fiat

**Related:** the [validator evaluator alternatives note](validator-evaluator-alternatives-note.md) (Dynamic UNL content inside the DGA envelope, formula as fail-closed baseline), the [L1 evidence-source note](dynamic-unl-l1-evidence-source-note.md) (Option C, `SHADOW_ONLY`), the [AI governance validity review](ai-governance-validity-review.md), the [pending operator decisions sheet](pending-operator-decisions.md), and testnet-path [Gate Zero Z2](../plans/active/l1v2-public-testnet-path-milestone.md)

## The operator's question

> can we make an intellectually defensible / strong ai governance thing with
> minimal tech headache that doesnt blow our audit costs out

## Plain-English answer

Yes, and the shape is already most of the way built. Let the small, published,
integer score formula and the mechanical selector hold every binding decision
about who is eligible and who is seated; they already hold that authority on
the fork under prompt v8 and later. Give the model exactly one job it has
measurably done well: reading a frozen identity packet and flagging an
identity that looks fabricated. A flag never scores, punishes, or removes
anyone; it only moves that operator onto a stricter set of deterministic entry
checks that a real but obscure operator can pass while making a fabricated
identity costlier to sustain.
Treat every change to the formula, the model, the prompt, or a threshold as a
recorded governance decision, and keep every model-derived output in
`SHADOW_ONLY` until the recorded gates pass. The binding path then stays a small,
replayable integer-code surface, which is the part an auditor has to read, and
the model stays outside it.

## The design

### 1. The deterministic formula holds all binding authority

Eligibility and selection are decided only by published integer code over
frozen evidence: the score formula `min((50c+20r+10s+10d+10i)//100, c+25)` and
the selector's cutoff, size, churn-gap, and tie-break rules, with the sub-scores
supplied by the versioned deterministic sub-scorer rules (v2) rather than the
model. On the fork the formula and selector are already the authority over the
final score and the seat list; the model's overall score has been advisory
since prompt v8, and its remaining authority is the five sub-scores that feed
the formula ([shadow evaluation](dunl-subscorer-shadow-eval-20260901.md);
[fork design](https://github.com/postfiatorg/dynamic-unl-scoring/blob/f18e6b4c40d3df17afb46804ef4f2bb0d879b439/docs/DeterministicFinalScore.md)).
This decision removes that last binding input. The formula and selector parameters — weights,
consensus gate margin 25, cutoff 40, maximum size, and minimum gap — are
content-hash pinned in the round manifest. The v2 sub-scorer is committed and
versioned but not yet manifest-pinned; adding that pin is an explicit
prerequisite in the work sequence. Once it is pinned, any operator can
recompute the exact outcome on ordinary hardware. This is the "demote" answer
to Gate Zero Z2, and it is the configuration that must run the fork's weekly
rounds before Z2 can close.

### 2. The model keeps one narrow advisory job: identity and fabrication flagging

- **Input:** the exact frozen Markdown identity packet for the operator
  ([identity packets](validator-identity-packets.md)), plus the frozen fork
  evidence record (claimed domain, upstream domain-verification value). No
  browsing, no rerun, no operator-supplied name mapping.
- **Output:** one typed classification per operator, `identity_flag` in
  `{clear, flag}`, with the packet fields it relies on and the packet SHA-256
  echoed back. A `flag` means the packet does not establish a real organization
  or person behind the claimed identity, or the claimed institution and domain
  pairing contradicts the cited evidence. The institution-legitimacy score the
  operator confirmed on 2026-09-01 (recognized or exactly 0;
  [institution legitimacy scoring](institution-legitimacy-scoring.md)) keeps
  running as the review artifact it is today; the flag is the only bit of it
  that may touch anything binding.
- **Effect, all deterministic:** a flagged operator is evaluated under the
  strict entry profile instead of the standard one. Proposed defaults, to be
  fixed by a recorded decision: a verified domain is mandatory (the identity
  dimension is already domain evidence only); the entry profile requires four
  consecutive clean rounds (matching the count in the
  [deferred milestone's E2](../deferred-plans/dynamic-unl-proposal-source-milestone.md#e2-phase-3a-ratification-preconditions));
  and the operator receives no seat (fork) or trust-view membership (l1v2)
  until those rounds are complete. Real but obscure operators can pass by
  waiting and proving domain control. A fabricated entrant must maintain the
  same externally checkable signals; this raises its cost but is not proof of
  identity or independence.
- **What a flag can never do:** lower a score, remove an incumbent, change a
  threshold, or block a round. An incumbent that gets flagged keeps its seat;
  only the formula's ordinary rules can remove it. A flag is recomputed from
  the new packet every round; the strict-entry hold clears once the strict bar
  is met.
- **Admissibility:** a flag counts only if the pinned, loopback-only,
  two-distinct-owner-host replay converges byte for byte, exactly as the
  institution replay did. A divergent, missing, or truncated response yields
  no flag; the operator is then checked under the standard profile, which is
  the fork's configuration today. The model cannot block anyone by being absent
  or wrong, so it is never a liveness dependency
  ([validity review](ai-governance-validity-review.md), decision 5).

### 3. Every change is a recorded governance decision

No silent tweak to the formula weights, the sub-scorer rules, the selector
parameters, the strict-profile parameters, the flag model, its prompt, or its
execution profile. The fork already runs this discipline for the scoring model:
a model changes only through a completed public governance round with the
standing 5-point incumbent-replacement margin
([roadmap G.1](https://github.com/postfiatorg/dynamic-unl-scoring/blob/f18e6b4c40d3df17afb46804ef4f2bb0d879b439/docs/CurrentRoadmap.md)),
and formula and selector versions are pinned in every manifest. This decision
extends the same rule to the flag pipeline: a candidate flag model or prompt
replaces the incumbent only if it beats it by the margin on a pre-registered,
held-out set, and a rule change ships as a new versioned module with a content
hash and a one-line decision record. On l1v2 the DGA policy pins the admitted
versions and may reject or hold, never rescore
([alternatives note](validator-evaluator-alternatives-note.md)).

### 4. Everything model-derived stays `SHADOW_ONLY` until the recorded gates pass

Flags, legitimacy scores, and any future classification are frozen,
content-addressed review artifacts under the Option C decision
([evidence-source note](dynamic-unl-l1-evidence-source-note.md)). They cross
into the binding path only after the gate list already recorded in the
[validity review](ai-governance-validity-review.md) passes: signed
reconstructable observations, three independent observers and replayers, an
open proposal lane, a pre-registered held-out evaluation with material lift and
no unsafe positive admission, hold/no-op on model deletion, published replay
cost, and the drill set. Nothing unverifiable ever makes a binding decision:
a binding decision must be recomputable from frozen evidence and pinned
integer code by any operator, which is the genesis specification's
invariant 4
([genesis and launch spec](../architecture/l1v2-testnet-genesis-and-launch-spec.md)).

## Evidence

| Evidence | What it shows | Numbers | Where |
| --- | --- | --- | --- |
| Deterministic sub-scorer shadow evaluation, v1 and v2 | Replacing the model's five sub-scores with transparent rules changes almost nothing in eight real rounds | v2: [1 cutoff flip across rounds 12–19](dunl-subscorer-shadow-eval-20260901.md#v2-results), strictly eligible→ineligible; UNL overlap 19–20/20 every round; internal control reproduced all eight published UNLs; v1: 2 flips | `benchmarks/ai-governance/dunl-subscorer-shadow-20260901/results-v2.json`, `results.json` |
| H200 reputation results, profiles v2–v4 | A pinned 27B model replays deterministically and shows a first small lift on one lane, with real residual harm | [v4 determinism 864/864](reputation-scoring-h200-results-20260901.md#v3v4-reasoning-profile-follow-ups-same-day-new-profiles) across distinct-owner hosts × two runs; prestige AUC 0.9732 vs deterministic baseline 0.9643; all four fabrications scored 0–5, Coinbase name-squat at 0; two of three real obscure operators misbanded (Greenhost 5, OpenBSD Amsterdam 7); two lanes still invalid (79/90, 85/90); cost $77.32 cumulative | `benchmarks/ai-governance/reputation-h200-20260901/determinism-artifact-v4.json` |
| Institution-reputation two-UNL replay | The recognition-first legitimacy score replays byte-identically across owners | [192/192 identical](institution-reputation-unl-h200-results-20260901.md#determinism-proof) across two distinct-owner hosts, four runs; every current PostFiat claim scored 0 | `benchmarks/ai-governance/institution-reputation-unl-20260901/` |
| Validator identity-packet corpus | Frozen, cited identity evidence exists for every current XRPL and PostFiat validator | [55/55 strict PASS](validator-identity-packets.md#published-evidence); descriptive: [45 identities established or likely, 10 not established, and 541 cited URLs](https://github.com/postfiatorg/postfiatl1v2/blob/main/benchmarks/ai-governance/validator-identity-packets-20260901/README.md#corpus-result) | `benchmarks/ai-governance/validator-identity-packets-20260901/README.md` |
| Packet-based identity replay (in flight) | The legitimacy score run from exact packet bytes instead of an entity-name mapping | Frozen 2026-09-03; 55 scoring plus 9 padding slots in two fixed batches; packet-set SHA-256 `b198e232…`; result **pending**, no numbers claimed | [`benchmarks/ai-governance/identity-packet-replay-20260903/`](https://github.com/postfiatorg/postfiatl1v2/blob/main/benchmarks/ai-governance/identity-packet-replay-20260903/README.md) |
| Phase 3 research charter (fork) | The pre-registered rule this decision applies | "Failure to beat it means the additional LLM complexity is not justified for binding governance"; thresholds registered before the evaluation set is opened | [charter §8 and §10](https://github.com/postfiatorg/dynamic-unl-scoring/blob/f18e6b4c40d3df17afb46804ef4f2bb0d879b439/docs/research/Phase3ResearchCharter.md) |
| Recorded directions this must stay consistent with | The envelope this decision sits inside | Dynamic UNL content in the DGA envelope, formula as fail-closed baseline (89.27/100); Option C `SHADOW_ONLY` (89.80/100); storage milestone "Decisions recorded" | [alternatives note](validator-evaluator-alternatives-note.md), [evidence-source note](dynamic-unl-l1-evidence-source-note.md), [storage milestone](../plans/active/storage-scaling-milestone.md#decisions-recorded) |
| Audit-scope inventory | Where the model sits relative to the audited core | Cobalt governance 19,713 core LOC, "Very high"; DGA/model governance 6,254 LOC, "High but deferrable", separately scopable; core total 225,070 | [LOC and audit-scope inventory](../security/core-feature-loc-audit-inventory.md) |

## Why this meets the three criteria

### Intellectually defensible

The design follows the fork's own pre-registered rule instead of arguing
around it. The charter says the LLM is justified for binding governance only
if it beats a strong transparent baseline without weakening reproducibility
or safety, and that rejecting binding automation may leave "a transparent
advisory system as the correct near-term outcome". The measurements say the
baseline reproduces the model's selections (one flip in eight rounds), the
model's one measured lift is small and on a 14-real/4-fabrication cohort, and
the same run
misbanded two real operators. Giving the model an advisory flag that can only
tighten deterministic checks is the exact reading of that evidence: keep the
value it demonstrated (all four fabrications at 0–5), refuse the authority it
has not earned, and make the residual harm bounded and reversible. It also
matches the validity review's decision split: deterministic code handles every
exact condition; the model classifies only enumerated ambiguous fields and
must cite packet fields.

### Minimal tech headache

Nothing new has to be invented on the binding path. The formula, selector,
manifest pinning, and sidecar commit-reveal already run weekly on the fork;
the sub-scorer v2 module, its tests, and its eight-round replay harness are
committed. The identity-packet corpus and packet-replay framework are also
committed; the institution-replay profile it reuses is the surface
[proven at 192/192](institution-reputation-unl-h200-results-20260901.md#determinism-proof).
The work that
remains is small and deterministic: a typed flag schema, a strict-profile rule
module with fixture cases, a content-hash pin for the sub-scorers, and a
change-control register. No GPU enters any validator's requirements; the flag
replay is a periodic batch job on rented hardware that is destroyed after the
evidence is downloaded, as the institution replay was.

### Audit-cheap

The audit-critical zone is the code that can change registry or trust-graph
state. Under this design that zone is the Cobalt checker (already in scope,
[19,713 lines, "Very high"](../security/core-feature-loc-audit-inventory.md))
plus a compact surface of integer rules: the formula, the selector, the
sub-scorer rules, and the entry-profile checks. The
model, its runtime image, its weights, and its prompt never enter the audited
binary; their only product is a replay-verified bit that deterministic code may
use to tighten, never to loosen. That is why the audit-scope inventory lists DGA/model governance as "High but
deferrable" and "separately disabled or scoped" rather than as part of the
Cobalt trust boundary. Retaining model authority would do the opposite: the
genesis specification's invariant 4 makes the pinned model, the GPU replay
lane, its cost, and its reproducibility limits explicit L4 liabilities that
every verifier and every reviewer must then carry. This decision keeps
verification inside the reviewed commodity-hardware budget, so the audit scope
grows by rule code, not by an inference stack.

## Counterarguments answered honestly

**"v4 showed lift; the model adds value, so keep it binding."** The lift is
real and small: prestige AUC 0.9732 against 0.9643, on a 14-real/4-fabrication
cohort, in one of three lanes. The other two lanes were invalid on truncation
(79/90 and 85/90), the fabrication packets were built by the same person who
holds the rubric and baseline, and the model put two of three real obscure
operators into fabrication bands. Pre-registered composite acceptance was not
met. On the surface that decides seats, the sub-scorer evaluation shows the
selection difference between model and rules is one flip in eight rounds. The
value the model demonstrated is fabrication detection; the flag keeps exactly
that. What would change the answer: a model that beats the deterministic
baseline on a pre-registered, held-out, independently constructed set of
fabrications and obscure real operators, with every lane valid, no real
operator in a fabrication band, determinism across distinct-owner hosts, and
published replay cost. Then a recorded decision could promote the model from
flag to weighted input, still under the formula, never to direct authority.

**"Demotion hands the reliability band edge and the diversity curve to crude
rules."** v1 did; v2 states the reliability band from the rubric text and fits
a monotone band matrix for diversity, converging to mean delta 3.4–6.8 on the
current-era rounds. What is left is a genuine policy choice about the curve,
and point 3 makes it a recorded decision instead of an inherited accident.

**"A flag that cannot punish is toothless."** Its purpose is to raise the cost
of a fabricated identity, not to be the Sybil defence. A flagged entrant must
hold a verified domain and a clean consensus record under its own name for
four rounds; a fabrication has to spend real time and real infrastructure per
identity. The hard limits stay in deterministic code: consensus gate,
concentration caps, churn gap, and the independence checks the charter
requires, because identity verification is a Sybil input and not proof of
independence.

**"Real small operators get a worse deal."** True, and bounded: the cost of a
false flag is a wait and a domain proof, both transparent and reversible. The
alternative was measured. A weighted model put Greenhost and OpenBSD Amsterdam
in fabrication bands, and the recognition-first legitimacy run scored
[all 20 current PostFiat validators 0](institution-reputation-unl-h200-results-20260901.md#interpretation). A punitive flag would have fallen on the whole
community this project depends on; a non-punitive one does not.

**"This contradicts the confirmed institution-legitimacy direction."** It does
not. The model still scores institutional legitimacy from the packet, and an
unrecognized institution still scores 0; that judgment is not replaced by
performance formulas. This decision fixes what may flow from that judgment into
anything binding: only the flag, only as a switch to stricter deterministic
checks.

**"Why not drop the model entirely?"** Because the rules' identity dimension is
domain evidence only, and fabricated-identity reading is exactly what rules
render crudely. Keeping one narrow model job preserves the one measured value
at zero binding cost. If the packet replay shows the model cannot separate
fabricated from obscure at all, dropping the flag is the fallback and loses
nothing binding.

## What this decides and what it does not

This document is a direction, not a deployment. It recommends the "demote"
answer to Z2 with one advisory flag, and it sits inside the recorded
directions: Dynamic UNL content inside the DGA envelope, the formula as
fail-closed baseline, and Option C `SHADOW_ONLY`. It does not change the
fork's weekly rounds, any registry, any trust graph, any policy, or any chain
state; it grants no authority to any model, proposer, or process; it locks no
specification and makes no Task Node request. Z2 closes only when the decided
configuration actually runs the fork's production rounds.

The operator adopts it with one line in the model-authority row of the
[pending operator decisions sheet](pending-operator-decisions.md): "confirmed"
adopts the four points as written; a different answer replaces this document.

## Work sequence

Each step is bounded, lands with focused governance tests only, crosses no
Orchard boundary, and stays `SHADOW_ONLY`. Steps will be requested as Task
Node tasks at the normal cadence; none is requested here.

1. **Flag-pipeline specification.** Typed schema (`identity_flag`, cited
   packet fields, echoed packet SHA-256), pinned execution profile, fixed
   batches, two-distinct-owner replay, admissibility by byte convergence,
   outage and divergence semantics as stated in point 2. Reuse the
   `identity-packet-replay-20260903` package layout. Output: a governance
   note plus a package skeleton under `benchmarks/ai-governance/`.
2. **Strict entry profile as deterministic checks.** A versioned rule module
   with a content hash: `new: crates/consensus_cobalt/src/validator_entry_profile.rs`
   beside `validator_admission_policy.rs`, a Python mirror in
   `new: python/postfiat_rpc/validator_entry_profile.py`, and fixture cases:
   flagged real operator becomes eligible after four clean rounds, no flagged
   entrant seats before satisfying the strict profile, flagged incumbent is not
   removed, model outage yields zero flags, divergent replay yields no flag.
3. **Sub-scorer v2 promotion on the fork.** Add the content-hash pin and
   manifest section the shadow evaluation lists as open, record the
   reliability band value and the diversity curve as decisions, and run v2
   beside the model for four weekly rounds in shadow before the switch. This
   is the step that makes Z2 "live".
4. **Change-control register.** One page listing formula, sub-scorer, selector
   parameters, strict-profile parameters, flag model, prompt, and execution
   profile with versions and hashes; every later change is a dated line with
   its decision record and, for model or prompt candidates, the margin result.
5. **Packet replay read-out.** When `identity-packet-replay-20260903`
   completes, publish its comparison and use it descriptively: does the flag
   separate the [10 not-established identities from the 45 established or
   likely](https://github.com/postfiatorg/postfiatl1v2/blob/main/benchmarks/ai-governance/validator-identity-packets-20260901/README.md#corpus-result)
   without flagging real small operators? This is the first test of the flag, not a
   promotion.
6. **Promotion gates recorded.** Adopt the validity review's gate list and the
   charter's pre-registered thresholds as the single recorded gate list that
   any model-derived output must pass before it becomes a binding input.
7. **CLI and status page.** A `postfiat_rpc` command that prints the current
   authority map (which code binds, which artifacts are shadow, which
   parameters are pinned) and the generated docs page that consumes it, per
   the mandate; requested after steps 1–4 land.
