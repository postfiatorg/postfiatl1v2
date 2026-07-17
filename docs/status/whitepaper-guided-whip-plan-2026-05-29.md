# Whitepaper-Guided L1 Whip Plan - 2026-05-29

This lane treats the whitepaper score as the external product requirement.
The work is not "write prose until it sounds better." The work is to develop
the L1, its evidence packets, its verifiers, and its public argument until a
severe model reviewer can no longer find cheap objections.

The operating target is simple: raise the confirmed public whitepaper score
without weakening technical truth. If an implementation slice improves the
system but tanks the score, keep the implementation only as repo evidence and
do not promote it into the public paper until it can be framed cleanly.

## Objective

Maximize the confirmed score of `docs/whitepaperv2.md` under the current
scoring harness while preserving:

- fixed supply and fee burn;
- no native validator rewards;
- Cobalt-governed trust evolution;
- shielded settlement with explicit disclosure;
- post-quantum authorization;
- replayable but non-authoritative AI governance;
- no promotion of a lower-scoring paper.

The active public paper is V2 `1.1.30`, confirmed at `86,86,86` on
`anthropic/claude-4.8-opus-20260528` under the explicit `86/86/86` gate for
this run.

## Core Loop

1. Score the current public paper once to verify the baseline did not drift.
2. Ask for ten targeted score-moving edits in the context of the latest score
   blockers, not a generic review.
3. Create one candidate draft from the highest-leverage edit set.
4. Run a one-score screen.
5. If promising, run three-score confirmation.
6. If score improves or stays flat with cleaner framing, consider promotion.
7. If score falls, discard the whitepaper integration and keep only any useful
   implementation/evidence artifacts.
8. If two editorial attempts on the same blocker fail, implement the easiest
   claim that would make the objection no longer cheap.
9. After implementation, add a short evidence packet, verifier, negative
   fixtures, and report.
10. Integrate the evidence into the candidate paper in one paragraph or one
    table.
11. Cut repeated thesis, long hashes, and status texture.
12. Re-score.

The loop is score-gated development: critique -> edit -> score -> implement ->
packetize -> integrate -> cut -> score.

## Editorial Levers

Use these before assuming implementation is required:

- 10-item targeted synthesis from Opus 4.8 or GPT-5.5 Pro.
- Bloat cuts after any evidence insertion.
- Removal of unnecessary hedging.
- Conversion of negative caveats into bounded affirmative claims.
- Re-ordering so the strongest theorem/evidence appears before the objection.
- Replacement of raw hash/path inventory with paper-level claim summaries.
- Narrowing a claim until it becomes reviewer-resistant.

Do not blindly apply all generated edits. The correct unit is a coherent edit
family that can be scored and reverted.

## Implementation Levers

When prose cannot move a blocker, build the smallest artifact that makes the
objection harder:

- schema;
- valid fixture;
- adversarial invalid fixtures;
- verifier command;
- report root;
- docs page or evidence page;
- one-paragraph whitepaper integration;
- cut pass.

Evidence must include negative controls. A packet without adversarial rejects
usually reads like status reporting, not proof.

## Current Highest-Value Blockers

1. **Registry-addition liveness under thin attestors.** Attempts to state that
   a thin attestor market simply holds registry additions screened `84`; the
   next version needs a verifier/packet that bounds proposal expiry, retry
   cadence, challenge griefing, and the difference between settlement liveness
   and registry-expansion liveness.
2. **Real source-bound economic magnitudes.** V2 `1.1.30` now makes the live
   attestation lifecycle concrete for \(B_i,C_i,G_i(w),L_i(w)\), and the
   risk-constant verifier exists, but fixture-shaped paper insertions screened
   `84` or failed confirmation. Do not add more round-number examples unless
   they are source-shaped and adversarially checked.
3. **Cross-vendor replay evidence.** A prose boundary saying non-NVIDIA or
   non-Apple stacks need zero selector-relevant parsed-root divergence was
   useful but failed confirmation inside the attestor candidate. A score-moving
   version likely needs actual cross-vendor evidence or an explicit promotion
   rule backed by fixtures.
4. **Cobalt proof rigor.** Pure Cobalt lemma-chain prose now screened `84`
   after a previous lock-import proof screened `85`. Keep the proof text
   archived; retry only with implementation evidence or a checker artifact that
   makes the lock-import step less assertion-like.
5. **Challenge/removal incentive pricing.** The public paper prices
   manufacture-\(B_i\) and conflict-erasure capture paths against the maximum
   short-window value-at-stake. The next economic blocker is narrower: who pays
   to challenge false attestations, what loss attaches to a proved false
   attestation, and how unresolved challenge evidence routes before registry
   mutation.
6. **Launch threshold wording.** Before adding more numeric examples, reconcile
   the language around \(n=35\), \(f=11\), \(q=24\), and \(3f+1=34\): \(35\)
   validators can tolerate \(f=11\), while \(34\) is the minimum active count
   preserving that fault budget after one loss.
7. **Security measurements as reproducible claims.** The paper still carries
   measured proof-size, verification-throughput, benchmark, and replay numbers
   that a reviewer can call single-run or environment-specific unless the
   method is either fully artifact-bound or demoted in text.
8. **AI governance scale.** The residual-router evidence is narrow. More packet
   families, more adversarial cases, and a clearer "convenience layer, not
   correctness authority" frame may move score.
9. **M_cover=64 and colluding omission cost.** Latest reviews still ask for an
   absolute worst-case cover-extractor cost and a named liveness degradation
   route when a colluding quorum withholds receipts.
10. **Evidence bloat.** Recent integrations add useful artifacts but can read as
   project-management texture if not cut aggressively.

## Latest Attestor Ecosystem Attempts

Result:

- fresh baseline score for V2 `1.1.30`: `86`;
- targeted Opus 4.8 synthesis identified the attestor/source-class ecosystem,
  fixture-shaped capture constants, genesis evidence, and cross-vendor replay
  as the current score hinge;
- candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/attestor-ecosystem-sensitivity/whitepaperv2-attestor-ecosystem-sensitivity.md`;
- candidate score: screen `86`, confirm `85,86,86`;
- second candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/attestor-cobalt-lemma-chain/whitepaperv2-attestor-cobalt-lemma-chain.md`;
- second candidate score: screen `84`;
- third candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/attestor-bootstrap-boundary/whitepaperv2-attestor-bootstrap-boundary.md`;
- third candidate score: screen `84`;
- no public-paper promotion; active paper remains V2 `1.1.30`.

Interpretation: the reviewer accepted the economic sensitivity table as useful
but unstable. Pure Cobalt proof expansion made the paper heavier, and explicit
bootstrap honesty was penalized as weakening the public admission story. Do not
retry these as prose. The next score-moving version needs either a bounded
registry-addition liveness/griefing packet or a real source-bound economic
magnitude packet; otherwise the loop will keep restating the same assumptions.

## Latest Control Surface Taxonomy Slice

Result:

- failed candidates:
  `docs/archive/whitepaper-drafts/2026-05-29/economic-admission-adversary/whitepaperv2-economic-admission-adversary.md`
  screened `87` but confirmed `87,86,84`, and
  `docs/archive/whitepaper-drafts/2026-05-29/economic-admission-adversary/whitepaperv2-economic-admission-adversary-v2.md`
  screened `87` but confirmed `84,86,86`;
- promoted candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/control-surface-taxonomy/whitepaperv2-control-surface-taxonomy.md`;
- screen score: `86`;
- confirmed score: `86,86,86`;
- promoted as public V2 `1.1.25` under the explicit `86/86/86` gate;
- the promoted Section 1 now states that control surfaces are entries in a
  Cobalt-governed taxonomy registry with roots, equivalence classes, merge
  rules, and challenge windows; unknown or applicant-created surfaces hold,
  ungoverned taxonomy changes fail closed, and shared owner/signing/KMS/release
  manager/data/attestor-control paths collapse back into one surface for the
  \(B+1\) budget.

Next target: cost-to-forge versus value-at-stake for attestor-signed economic
evidence. The likely implementation slice is a verifier packet that compares
`capture_cost_floor_usd` to `max_short_window_value_at_stake_usd` and routes
underpriced evidence paths to hold or fail-closed.

## Completed Candidate

Build `validator-economic-bound-policy-v1`.

Required shape:

- evidence classes for \(B_i(t)\), \(C_i(t)\), \(G_i(w)\), and \(L_i(w)\);
- source classes for flow exposure, conflicting external position, contractual
  liability, exclusion loss, affiliate/control links, and registry-thinning;
- deterministic bins that route:
  - high exposure with bounded conflict to challenge or admit;
  - high exposure with unbounded short-window conflict to hold/reject;
  - stale liability evidence to hold;
  - hidden affiliate position to reject;
  - missing exclusion-loss evidence to hold;
  - essential-subset `safe_count <= q_S` to fail closed;
- verifier with negative fixtures;
- timestamped report;
- short docs page;
- candidate whitepaper insertion plus cut pass;
- three-run score only if the screen is not worse.

Status: completed and promoted at `86,87,86`.

## Most Recent Candidate

Build `performance-methodology-policy-v1`.

Result:

- verifier, valid fixture, negative fixtures, report, and docs page completed;
- whitepaper candidates scored `86,86,86`;
- not promoted because the active promoted draft remains stronger at
  `86,87,86`;
- keep the evidence and use it as the harness for a fresh ML-DSA verification
  benchmark before attempting another performance-section promotion.

## Latest Implementation Slice

Bind `validator-economic-bound-policy-v1` into the bounded registry-delta
candidate contract.

Result:

- `valid_noop_candidate.json` now commits to the economic-bound packet file
  hash, canonical packet hash, statement hash, policy root, schema hash, and
  verifier hash;
- `scripts/qwen-cobalt-bounded-registry-delta-candidate-contract` executes
  `scripts/validator-economic-bound-policy-verify` during preflight;
- the route gate is explicit: only `admit` can feed a registry mutation, while
  `hold`, `reject`, and `fail-closed` remain non-mutating routes;
- QBRD-002 report regenerated with the economic-bound checks.

## Latest Editorial Slice

Tighten the Cobalt transition proof sketch.

Result:

- candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/cobalt-proof-tightening/whitepaperv2-cobalt-proof-tightening.md`;
- screen score: `86`;
- confirmed score: `86,87,86`;
- promoted because it matches the active baseline while making the transition
  proof cleaner;
- the public paper now states the single-correct-signer lemma for both
  same-registry commits and old/new registry transitions: quorum intersections
  of size `>B`, combined with `B <= min_S t_S`, force a correct shared signer
  across conflicting certificates, and the two-chain lock rule prevents that
  signer from voting for both branches.

## Latest Guarded-Add Slice

Exercise the economic-bound gate on a non-no-op candidate.

Result:

- fixture:
  `docs/governance/agent/fixtures/validator_evidence/bounded_registry_delta_candidate/valid_guarded_add_rehearsal_candidate.json`;
- verifier:
  `scripts/qwen-cobalt-bounded-registry-delta-guarded-add-rehearsal`;
- report:
  `reports/qwen-cobalt-internal-validation/qbrd-003-guarded-add-rehearsal-report.json`;
- the candidate proposes exactly one `add`, binds it to the economic-bound
  clean-admit packet, references Gate 9.5 / VC-061 guarded-apply roots, and
  keeps every authority flag closed;
- negative probes reject wrong action mode, more than one mutation, non-`admit`
  mutation route, mismatched guarded roots, and live-registry authority.
- concise whitepaper integration confirmed `86,86,86`, so it remains archived
  and was not promoted over the active `86,87,86` public paper.

## Latest Promoted Score Slice

Make the zero-issuance economics more checkable while cutting appendix bloat.

Result:

- candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/economic-attestation-cut/whitepaperv2-economic-attestation-cut.md`;
- confirmed score: `86,86,87`;
- promoted because it matches the previous `86,87,86` baseline distribution,
  makes source-class attestation explicit, and removes raw Appendix A hash
  clutter;
- a prior privacy assurance construction candidate confirmed `86,86,86` and
  remains archived only.
- next reviewer blocker: prove or tightly argue Cobalt cover-extractor
  correctness and completeness, because the transition theorem depends on the
  extractor rather than on proposer-supplied cover claims.

## Latest Replay-Profile Slice

Bound replay-profile breakage and mid-dispute behavior.

Result:

- cover-extractor lemma candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/cover-extractor-lemma/whitepaperv2-cover-extractor-lemma.md`;
- cover-extractor confirmed score: `86,86,86`; not promoted;
- promoted candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/replay-profile-breakage/whitepaperv2-replay-profile-breakage.md`;
- confirmed score: `86,87,86`;
- promoted because it matches the active baseline distribution, classifies
  tolerated audit noise versus profile-fatal drift, defines replay breakage as
  failed replay rather than a judgment call, and states that open disputes
  hold/no-op while ordinary settlement continues;
- next reviewer blocker: undeclared off-chain/social correlation and
  attestor-trust circularity in the natural-validator / Cobalt trust model.

## Latest Undeclared-Correlation Slice

Name silent undeclared collusion as the residual adversary.

Result:

- candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/undeclared-correlation/whitepaperv2-undeclared-correlation.md`;
- confirmed score: `86,87,86`;
- promoted because it preserves the active baseline while adding an explicit
  sixth adversary class for undeclared collusion / captured attestors,
  separating attestor-group evidence from self-declaration, and stating that
  Cobalt proves declared graph safety rather than discovering silent off-chain
  cartels;
- next reviewer blocker: the cross-registry safety composition still reads as
  a proof sketch. The next proof edit should state a lock-import lemma showing
  how linkedness closure and old/new intersection force a correct signer to
  carry the old commit lock into the joint transition.

## Latest Lock/Economic Attempts

Test the next proof and economics hypotheses without touching the public paper.

Result:

- lock-import lemma candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/lock-import-lemma/whitepaperv2-lock-import-lemma.md`;
- confirmed score: `86,86,86`; not promoted because it did not beat the active
  `86,87,86` public baseline;
- deviation-gain bound candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/deviation-gain-bound/whitepaperv2-deviation-gain-bound.md`;
- confirmed score: `86,86,86`; not promoted because structural \(G_i(w)\)
  language still left the magnitude-calibration objection open;
- economic-bound worked-example candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/economic-bound-worked-example/whitepaperv2-economic-bound-worked-example.md`;
- screen score: `84`; rejected without confirmation because hand-assigned
  ordinal values made the economic argument look author-calibrated;
- public paper remained unchanged.

Next blocker: build `validator-economic-bound-calibration-v1`, a small
source-anchored calibration packet that derives \(B_i,C_i,G_i(w),L_i(w)\)
magnitudes from signed evidence classes and rejects unbounded external gain,
stale liability evidence, self-declared loss terms, missing exclusion loss,
favorable-attestor inflation, and source-class mismatch.

## Latest Calibration Slice

Build `validator-economic-bound-calibration-v1`.

Result:

- schema:
  `docs/governance/agent/validator_economic_bound_calibration_schema.json`;
- fixture suite:
  `docs/governance/agent/fixtures/validator_economic_bound_calibration/`;
- verifier:
  `scripts/validator-economic-bound-calibration-verify`;
- report:
  `reports/validator-economic-bound-calibration/20260529T044940Z/validator-economic-bound-calibration-report.json`;
- docs:
  `docs/governance/validator-economic-bound-calibration.md`;
- the valid packet derives \(B_i,C_i,G_i(w),L_i(w)\) from raw USD
  source measurements and governed threshold tables instead of packet-authored
  ordinal units;
- negative controls reject or hold unbounded external gain, stale liability,
  self-declared reputation loss, missing exclusion loss, favorable attestor
  inflation, source-class mismatch, participation-margin decay, and
  loss-below-gain;
- candidate paper:
  `docs/archive/whitepaper-drafts/2026-05-29/economic-bound-calibration/whitepaperv2-economic-bound-calibration.md`;
- screen score: `86`;
- confirmed score: `86,84,86`; not promoted because it did not preserve the
  active `86,87,86` public baseline.

Next blocker: add a quantitative adversarial analysis of the calibration layer:
what a colluding-attestor or staleness-gaming adversary can and cannot change
before the selector routes to `hold`, `reject`, or `fail-closed`, and how any
undetected residual maps to the Cobalt Byzantine budget \(B\) and the weakest
covered \(t_S\).

## Latest Adversary-Pricing Slice

Build `validator-economic-bound-adversary-pricing-v1`.

Result:

- schema:
  `docs/governance/agent/validator_economic_bound_adversary_pricing_schema.json`;
- fixture suite:
  `docs/governance/agent/fixtures/validator_economic_bound_adversary_pricing/`;
- verifier:
  `scripts/validator-economic-bound-adversary-pricing-verify`;
- report:
  `reports/validator-economic-bound-adversary-pricing/20260529T050434Z/validator-economic-bound-adversary-pricing-report.json`;
- docs:
  `docs/governance/validator-economic-bound-adversary-pricing.md`;
- the packet prices undetected attestor cartels against \(B\) and weakest
  \(t_S\), admits one silent cartel only as budget-consuming residual, and
  fails closed if residual exceeds either bound;
- negative controls hold stale liability and source-class mismatch, reject
  unbounded gain, favorable attestor inflation, and detected collusion, and
  fail closed on ungoverned threshold capture or \(B > \min t_S\);
- candidate paper:
  `docs/archive/whitepaper-drafts/2026-05-29/economic-adversary-pricing/whitepaperv2-economic-adversary-pricing.md`;
- screen score: `86`;
- confirmed score: `86,86,86`; not promoted because it did not preserve the
  active `86,87,86` public baseline.

Next blocker: derive \(G_i(w)\) itself. The reviewer now accepts more of the
source/adversary machinery but still sees short-window deviation gain as the
least specified load-bearing scalar. The next implementation slice should
derive \(G_i(w)\) from bounded proposer slots, fee-class spread,
omission-window gain, registry-delay gain, and external-position caps, then
reject missing caps or unbounded windows.

## Latest Deviation-Gain Slice

Build `validator-economic-deviation-gain-v1`.

Result:

- schema:
  `docs/governance/agent/validator_economic_deviation_gain_schema.json`;
- fixture suite:
  `docs/governance/agent/fixtures/validator_economic_deviation_gain/`;
- verifier:
  `scripts/validator-economic-deviation-gain-verify`;
- report:
  `reports/validator-economic-deviation-gain/20260529T052040Z/validator-economic-deviation-gain-report.json`;
- docs:
  `docs/governance/validator-economic-deviation-gain.md`;
- the packet computes
  \(G_i(w)=\min(\sum component\_caps, external\_position\_cap)\) from
  source-bound proposer-slot, fee-spread, omission-window, registry-delay, and
  external-position-cap evidence;
- negative controls reject loss-margin failure, missing external cap,
  unbounded omission window, unbounded external-position cap, and favorable
  external-cap attestors; they hold stale proposer-slot evidence, wrong
  fee-spread source class, and missing registry-delay caps;
- candidate paper:
  `docs/archive/whitepaper-drafts/2026-05-29/deviation-gain-derived/whitepaperv2-deviation-gain-derived.md`;
- screen score: `87`;
- confirmed score: `86,86,85`; not promoted because it failed the active
  `86,87,86` public baseline.

Next blocker: attestor capture. The reviewer now treats \(G_i(w)\) as more
specified, but asks what prevents a candidate from manufacturing independent
attestors or understating the external-position cap through a captured auditor.
The next implementation or edit slice should price attestor capture by source
class and route under-collateralized attestor sets to hold, reject, or
fail-closed. One score sample also resurfaced the HotStuff two-chain lock lemma
as an ordinary-path proof obligation.

## Latest Attestor-Capture Slice

Build `validator-economic-attestor-capture-v1`.

Result:

- schema:
  `docs/governance/agent/validator_economic_attestor_capture_schema.json`;
- fixture suite:
  `docs/governance/agent/fixtures/validator_economic_attestor_capture/`;
- verifier:
  `scripts/validator-economic-attestor-capture-verify`;
- report:
  `reports/validator-economic-attestor-capture/20260529T053442Z/validator-economic-attestor-capture-report.json`;
- docs:
  `docs/governance/validator-economic-attestor-capture.md`;
- the packet requires separate attestor groups for exposure, liability,
  independence, conflict, and external-position-cap evidence;
- detected capture rejects; missing required attestors, stale attestors, source
  class mismatch, or group overlap hold; ungoverned separation-policy changes,
  \(B > \min t_S\), or undetected capture above \(B\) fail closed;
- candidate paper:
  `docs/archive/whitepaper-drafts/2026-05-29/attestor-capture-pricing/whitepaperv2-attestor-capture-pricing.md`;
- screen score: `86`;
- confirmed score: `84,86,84`; not promoted.

Next blocker: \(L_i(w)\) measurement. The reviewer now asks for a concrete
source-bound procedure for exclusion and reputation loss, ideally with a
worked attack fixture where \(L_i(w)-G_i(w)\) flips sign and the route changes.
Genesis defaults and privacy metadata also appeared, but the repeated economic
blocker is the loss side of the inequality.

## Latest Loss-Measurement Slice

Build `validator-economic-loss-measurement-v1`.

Result:

- schema:
  `docs/governance/agent/validator_economic_loss_measurement_schema.json`;
- fixture suite:
  `docs/governance/agent/fixtures/validator_economic_loss_measurement/`;
- verifier:
  `scripts/validator-economic-loss-measurement-verify`;
- report:
  `reports/validator-economic-loss-measurement/20260529T054837Z/validator-economic-loss-measurement-report.json`;
- docs:
  `docs/governance/validator-economic-loss-measurement.md`;
- the packet derives \(L_i(w)\) from contractual-liability,
  registry-exclusion-flow, route/customer-loss, public-incident-reputation,
  and governed challenge/exclusion source floors;
- the admit fixture has \(L_i(w)=12.5m\), \(G_i(w)=1.2m\), and \(11.3m\)
  margin;
- the attack fixture lowers source-bound loss to \(0.7m\), keeps
  \(G_i(w)=1.2m\), and routes to `reject`;
- negative controls reject self-declared or affiliated reputation evidence,
  hold stale or missing exclusion evidence, hold source mismatches and
  unverified/unbounded loss sources, and fail closed on ungoverned loss-policy
  changes;
- candidate paper:
  `docs/archive/whitepaper-drafts/2026-05-29/loss-measurement/whitepaperv2-loss-measurement.md`;
- screen score: `86`; not promoted.

A separate liveness-boundary paper candidate:
`docs/archive/whitepaper-drafts/2026-05-29/liveness-recovery-bound/whitepaperv2-liveness-recovery-bound.md`
also screened at `86`. It made the social recovery boundary explicit, and the
review accepted that honesty, but the score blocker shifted back to
attestor-capture and manufactured exogenous flow.

Next blocker: a patient-capital / synthetic-flow adversary packet. The reviewer
now asks why an attacker cannot manufacture plausible exchange, custody,
gateway, or route flow and then use captured or affiliated attestors to fake
the exposure side of the zero-issuance predicate.

## Latest Synthetic-Flow Slice

Build `validator-economic-synthetic-flow-v1`.

Result:

- schema:
  `docs/governance/agent/validator_economic_synthetic_flow_schema.json`;
- fixture suite:
  `docs/governance/agent/fixtures/validator_economic_synthetic_flow/`;
- verifier:
  `scripts/validator-economic-synthetic-flow-verify`;
- report:
  `reports/validator-economic-synthetic-flow/20260529T060516Z/validator-economic-synthetic-flow-report.json`;
- docs:
  `docs/governance/validator-economic-synthetic-flow.md`;
- the packet derives a \(B_i\) floor from exchange-volume, custody-assets,
  gateway-route-flow, and customer-settlement-flow sources;
- the valid fixture derives \(B_i=15m\) USD from four distinct source-control
  groups against a `10m` USD floor;
- negative controls reject candidate-owned exchange flow, self-declared flow,
  under-threshold flow, and wash-routed gateway flow; they hold stale custody
  statements, missing gateway routes, source-class mismatch, low counterparty
  diversity, and attestor-group overlap; they fail closed on ungoverned source
  policy changes;
- candidate paper:
  `docs/archive/whitepaper-drafts/2026-05-29/synthetic-flow-adversary/whitepaperv2-synthetic-flow-adversary.md`;
- confirmed score: `86,86,86`; not promoted;
- tighter source-control-independence candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/synthetic-flow-independence/whitepaperv2-synthetic-flow-independence.md`;
- screen score: `86`; not promoted.

Next blocker: an end-to-end scalar-reduction map for
\(B_i,C_i,G_i(w),L_i(w)\). The reviewer now accepts that separate packets exist,
but wants the paper to show how attested fields reduce to those four scalars
and which attestor classes can move each term.

## Latest Scalar-Reduction Slice

Build `validator-economic-scalar-reduction-v1`.

Result:

- schema:
  `docs/governance/agent/validator_economic_scalar_reduction_schema.json`;
- fixture suite:
  `docs/governance/agent/fixtures/validator_economic_scalar_reduction/`;
- verifier:
  `scripts/validator-economic-scalar-reduction-verify`;
- report:
  `reports/validator-economic-scalar-reduction/20260529T062230Z/validator-economic-scalar-reduction-report.json`;
- docs:
  `docs/governance/validator-economic-scalar-reduction.md`;
- maps \(B_i\) to source-bound exogenous exposure floors, \(C_i\) to current
  verified operating-cost ceilings, \(G_i(w)\) to bounded short-window gain
  caps plus external-position cap, and \(L_i(w)\) to verified loss floors;
- verifies the invariants \(B_i-C_i\ge0\) and \(L_i(w)-G_i(w)\ge1m\) USD;
- negative controls fail closed on missing scalar and ungoverned scalar policy,
  reject forbidden source classes or missing \(G_i(w)\) external-position cap,
  and hold missing linked verifier or adversarial coverage;
- candidate paper:
  `docs/archive/whitepaper-drafts/2026-05-29/scalar-reduction-map/whitepaperv2-scalar-reduction-map.md`;
- confirmed score: `86,86,86`; not promoted.

Next blocker: quantitative attestor-capture budget. The reviewer now asks how
many independent control surfaces or attestor groups an adversary must subvert
to manufacture \(B_i\) or erase a conflict without leaving evidence. A prose
table alone is unlikely to move the score without budgeted capture cases.

## Latest Attestor-Budget Slice

Build `validator-economic-attestor-budget-v1`.

Result:

- schema:
  `docs/governance/agent/validator_economic_attestor_budget_schema.json`;
- fixture suite:
  `docs/governance/agent/fixtures/validator_economic_attestor_budget/`;
- verifier:
  `scripts/validator-economic-attestor-budget-verify`;
- report:
  `reports/validator-economic-attestor-budget/20260529T064042Z/validator-economic-attestor-budget-report.json`;
- docs:
  `docs/governance/validator-economic-attestor-budget.md`;
- derives the three-group and three-control-surface floor from \(B+1\), with
  launch fixture values \(B=2\) and \(\min_S t_S=2\);
- verifies both attack paths: manufacturing \(B_i\) and erasing a conflict or
  external-position signal;
- negative controls fail closed on two-group/two-surface capture,
  above-\(B\) capture, and ungoverned budget policy; reject detected capture;
  and hold stale or incomplete budget evidence;
- first candidate paper:
  `docs/archive/whitepaper-drafts/2026-05-29/attestor-capture-budget/whitepaperv2-attestor-capture-budget.md`;
- screen score: `84`; rejected because it asserted the three-group floor
  without deriving it;
- derived candidate paper:
  `docs/archive/whitepaper-drafts/2026-05-29/attestor-capture-budget/whitepaperv2-attestor-capture-budget-derived.md`;
- confirmed score: `86,86,86`; not promoted over the active `86,87,86`
  public baseline.

Next blocker: population-level zero-issuance stability. The reviewer now asks
what happens over a long horizon if the count of natural validators satisfying
\(B_i-C_i>0\) falls below the BFT-safe count, and what protocol route handles
drift, underfilled sets, stale natural-stake evidence, or registry thinning.

## Latest Population-Stability Slice

Build `validator-economic-population-stability-v1`.

Result:

- schema:
  `docs/governance/agent/validator_economic_population_stability_schema.json`;
- fixture suite:
  `docs/governance/agent/fixtures/validator_economic_population_stability/`;
- verifier:
  `scripts/validator-economic-population-stability-verify`;
- report:
  `reports/validator-economic-population-stability/20260529T065847Z/validator-economic-population-stability-report.json`;
- docs:
  `docs/governance/validator-economic-population-stability.md`;
- models a 35-validator profile with \(f=11\), \(q=24\), BFT-safe count 34,
  and liveness count 24;
- routes one decayed natural validator to `removal-and-recruitment-window`,
  BFT-margin erosion to `recruitment-window`, liveness-margin erosion to
  `fail-closed`, stale natural-stake evidence to `hold`,
  one-validator temporary outage to `negative-unl-temp-exclusion`, unsafe
  temporary exclusion and registry thinning to `fail-closed`, and underfilled
  active sets to `recruitment-window`;
- candidate paper:
  `docs/archive/whitepaper-drafts/2026-05-29/population-stability/whitepaperv2-population-stability.md`;
- screen score: `86`;
- confirmed score: `84,86,86`; not promoted.

Next blocker: privacy metadata/anonymity bounds. The reviewer now asks for a
Section 3 privacy threat model that states what timing, fee-class,
admission-bucket, asset-type, disclosure-hash, and RPC-observer metadata remain
public or statistically inferable, and which low-volume or bursty shielded-pool
conditions downgrade the privacy claim.

## Latest Privacy Metadata/Anonymity-Bound Slice

Build `privacy-metadata-anonymity-bound-v1`.

Result:

- schema:
  `docs/governance/agent/privacy_metadata_anonymity_bound_schema.json`;
- fixture suite:
  `docs/governance/agent/fixtures/privacy_metadata_anonymity_bound/`;
- verifier:
  `scripts/privacy-metadata-anonymity-bound-verify`;
- report:
  `reports/privacy-metadata-anonymity-bound/20260529T072015Z/privacy-metadata-anonymity-bound-report.json`;
- docs:
  `docs/privacy/metadata-anonymity-bounds.md`;
- public paper:
  `docs/whitepaperv2.md` version `1.1.19`;
- valid packet hash:
  `c71a811cb9ee1eb010164f548a24f10c390110e205759f0bfced024c03fcf9c13481043de2a7bb09f3c7eae6d86b609b`;
- statement hash:
  `42d924d4c704e86c528ce42a5820ecd2d89ee63583ee9ccc60fd20bea0bb8ce99e1e568e8d3c2293cb4cea04c2ab66b4`;
- metadata root:
  `7af983eec4861bd846e58929a028a00cae8c5c3b2dc3ba119fb5d084234cc7b8c488f20c96fbbca115d341685ab0b081`;
- routes baseline, low-volume downgrade, bursty-timing hold, thin-asset
  downgrade, unique-disclosure explicit treatment, third-party RPC hold,
  private-field fail-closed, ungoverned-policy fail-closed, incomplete threat
  model hold, and fee-class singleton downgrade;
- candidate paper:
  `docs/archive/whitepaper-drafts/2026-05-29/privacy-metadata-bound/whitepaperv2-privacy-metadata-bound.md`;
- screen score: `86`;
- confirmed score: `87,86,86`; promoted because it matched the active baseline
  distribution and made the privacy claim more bounded.

Next blocker: derive or bound residual deanonymization under the stated floors
and inferable metadata channels. A good next artifact should explain why the
chosen floors are conservative or route any floor with no derivation to a
stricter downgrade policy.

## Latest Privacy Deanonymization-Bound Slice

Build `privacy-deanonymization-bound-v1`.

Result:

- schema:
  `docs/governance/agent/privacy_deanonymization_bound_schema.json`;
- fixture suite:
  `docs/governance/agent/fixtures/privacy_deanonymization_bound/`;
- verifier:
  `scripts/privacy-deanonymization-bound-verify`;
- report:
  `reports/privacy-deanonymization-bound/20260529T073724Z/privacy-deanonymization-bound-report.json`;
- docs:
  `docs/privacy/deanonymization-bounds.md`;
- public paper:
  `docs/whitepaperv2.md` version `1.1.20`;
- valid packet hash:
  `5c949e3fa40f33a8854129086a09b9b8870476f38f79a28b99793ef1bf166225204950415103d71e5bdb5558ed5cba55`;
- statement hash:
  `806fbb22ef07b55ee058956b0f6848ced03fbcf4376784dc998b6e9b16db3a83e77ad415f266ac7a2f7ff91c80f6752a`;
- bound root:
  `0799e5536a40bb053f19d1d95c5e2cd0bca6c341610c23ca97b1181299762c3046f6e278fff81f219a395d3e57c8aec9`;
- derives single-channel posterior as `ceil(10000 / candidate_count)` basis
  points and requires maximum single-channel posterior <= 1,250 bps, joint
  metadata posterior <= 625 bps, and a 16-candidate joint metadata cohort;
- negative controls cover root/hash mismatch, joint shortfall, batch/timing
  shortfall, unique disclosure, direct RPC observation, low activity, fee-class
  shortfall, asset-policy shortfall, ungoverned policy, stale observation, and
  declared external side information;
- candidate paper:
  `docs/archive/whitepaper-drafts/2026-05-29/privacy-deanonymization-bound/whitepaperv2-privacy-deanonymization-bound.md`;
- screen score: `86`;
- confirmed score: `86,87,87`; promoted because it beat the active baseline
  average and turned the privacy floor table into a routed residual model.

Next blocker: inter-channel and temporal correlation. A good next artifact
should model correlated metadata partitions rather than independent uniform
counts, then route the correlated shortfall to downgrade/hold/private-relay or
explicit disclosure.

## Latest Privacy Correlation-Bound Slice

Build `privacy-correlation-bound-v1`.

Result:

- schema:
  `docs/governance/agent/privacy_correlation_bound_schema.json`;
- fixture suite:
  `docs/governance/agent/fixtures/privacy_correlation_bound/`;
- verifier:
  `scripts/privacy-correlation-bound-verify`;
- report:
  `reports/privacy-correlation-bound/20260529T075414Z/privacy-correlation-bound-report.json`;
- docs:
  `docs/privacy/correlation-bounds.md`;
- candidate paper:
  `docs/archive/whitepaper-drafts/2026-05-29/privacy-correlation-bound/whitepaperv2-privacy-correlation-bound.md`;
- screen score: `86`;
- confirmed score: `86,86,86`; not promoted over the active `86,87,87`
  V2 `1.1.20` public paper;
- the valid packet models observed joint partitions rather than multiplying
  independent uniform counts across timing, fee class, admission bucket,
  asset/policy partition, disclosure hash, RPC observer metadata, temporal
  links, cross-window links, repeated disclosure, compromised wallet
  infrastructure, and declared off-chain side information;
- negative controls reject root/hash mismatch and force non-baseline routes
  for joint tuple shortfall, pairwise shortfall, temporal and cross-window
  shortfall, direct RPC observation, RPC cohort shortfall, repeated disclosure,
  declared off-chain side information, compromised wallet infrastructure,
  missing channels, stale observation, and ungoverned policy.

Next blocker: complete the Cobalt transition-safety proof or reduce it
explicitly to MacBrough's local Cobalt agreement result plus the PostFiat
old/new quorum-intersection check. The next paper attempt should handle
cross-registry lock import and certificate conflict cases directly instead of
adding more privacy text.

## Latest Cobalt Transition-Safety Proof Slice

Build `cobalt-transition-safety-proof-v1`.

Result:

- schema:
  `docs/governance/agent/cobalt_transition_safety_proof_schema.json`;
- fixture suite:
  `docs/governance/agent/fixtures/cobalt_transition_safety_proof/`;
- verifier:
  `scripts/cobalt-transition-safety-proof-verify`;
- report:
  `reports/cobalt-transition-safety-proof/20260529T081141Z/cobalt-transition-safety-proof-report.json`;
- docs:
  `docs/governance/cobalt-transition-safety-proof.md`;
- candidate paper:
  `docs/archive/whitepaper-drafts/2026-05-29/cobalt-transition-safety-proof/whitepaperv2-cobalt-transition-safety-proof.md`;
- screen score: `87`;
- confirmed score: `86,87,86`; not promoted over the active `86,87,87`
  V2 `1.1.20` public paper;
- the valid fixture models a one-validator 10-node rotation with \(q=8\),
  \(t=2\), \(B=2\), verifies old/new key-continuity quorum intersection, and
  includes same-registry and old/new conflict cases;
- negative controls fail closed on unsafe old/new intersection, \(B>\min t_S\),
  bad local row, open challenge state, oversized cover, missing old-checker
  validation, and missing key continuity.

Next blocker: specify key-continuity enforcement to proof-level rigor. A good
next artifact should bind old identity, old consensus key, new consensus key,
operator manifest, parent registry root, next registry root, and transition
root into a continuity receipt, then reject key-swap, identity-reuse,
unratified rotation, and missing parent-root binding fixtures.

## Latest Cobalt Key-Continuity Receipt Slice

Build `cobalt-key-continuity-receipt-v1`.

Result:

- schema:
  `docs/governance/agent/cobalt_key_continuity_receipt_schema.json`;
- fixture suite:
  `docs/governance/agent/fixtures/cobalt_key_continuity_receipt/`;
- verifier:
  `scripts/cobalt-key-continuity-receipt-verify`;
- report:
  `reports/cobalt-key-continuity-receipt/20260529T083000Z/cobalt-key-continuity-receipt-report.json`;
- docs:
  `docs/governance/cobalt-key-continuity-receipts.md`;
- candidate paper:
  `docs/archive/whitepaper-drafts/2026-05-29/cobalt-key-continuity-receipt/whitepaperv2-cobalt-key-continuity-receipt.md`;
- screen score: `86`; not confirmed and not promoted over active `86,87,87`;
- the valid fixture grants old/new intersection credit only to validators with
  receipts binding old key, new key, validator id, operator id, operator
  manifest, parent root, next root, transition root, old checker root, and
  issue/expiry epochs;
- negative controls fail closed on key-swap, missing parent-root binding,
  reused identity under a new operator, stale receipt, child-checker receipt,
  missing old-key signature, missing new-key signature, and insufficient
  continuity credit.

## Latest Privacy Observer-Inference Slice

Result:

- schema:
  `docs/governance/agent/privacy_observer_inference_model_schema.json`;
- fixture suite:
  `docs/governance/agent/fixtures/privacy_observer_inference_model/`;
- verifier:
  `scripts/privacy-observer-inference-model-verify`;
- report:
  `reports/privacy-observer-inference-model/20260529T083924Z/privacy-observer-inference-model-report.json`;
- docs:
  `docs/privacy/observer-inference-model.md`;
- candidate papers:
  `docs/archive/whitepaper-drafts/2026-05-29/privacy-observer-inference-model/whitepaperv2-privacy-observer-inference-model.md`
  and
  `docs/archive/whitepaper-drafts/2026-05-29/privacy-observer-inference-model/whitepaperv2-privacy-observer-inference-model-v2.md`;
- screen scores: `85` then `86`; neither was confirmed or promoted over active
  `86,87,87`;
- the valid fixture derives \(k=16\) from the full intersection of timing,
  fee-class, RPC-observer, disclosure-hash, asset/policy, and declared
  off-chain-side-information partitions;
- negative controls cover low joint partitions, stale observations, direct RPC
  observation, policy-root mismatch, declared side information, unique
  disclosure, timing shortfall, missing channels, root mismatch,
  statement-hash mismatch, and verifier-claim removal.

Next blocker: the score review accepted the full-intersection observer model
but moved to the constants behind it. A good next artifact should derive or
calibrate `k >= 16`, 625 bps, five-minute timing buckets, 128-action windows,
and single-channel floors against a concrete deanonymization adversary.

## Latest Privacy Floor-Calibration Slice

Result:

- schema:
  `docs/governance/agent/privacy_floor_calibration_schema.json`;
- fixture suite:
  `docs/governance/agent/fixtures/privacy_floor_calibration/`;
- verifier:
  `scripts/privacy-floor-calibration-verify`;
- report:
  `reports/privacy-floor-calibration/20260529T085352Z/privacy-floor-calibration-report.json`;
- docs:
  `docs/privacy/floor-calibration.md`;
- candidate papers:
  `docs/archive/whitepaper-drafts/2026-05-29/privacy-floor-calibration/whitepaperv2-privacy-floor-calibration.md`
  and
  `docs/archive/whitepaper-drafts/2026-05-29/privacy-floor-calibration/whitepaperv2-privacy-floor-calibration-v2.md`;
- screen scores: `86` and `86`; neither was confirmed or promoted over active
  `86,87,87`;
- the valid fixture derives 625 bps -> `k >= 16`, 1,250 bps -> batch size 8,
  and eight five-minute buckets with 16 candidates each -> 128-action activity
  window;
- negative controls cover broken derivations, perfect channel correlation,
  timing/activity/batch shortfall, thin asset-policy partition, exchange-side
  batching observation, direct RPC observation, side information, ungoverned
  policy, root mismatch, statement-hash mismatch, and verifier-claim removal.

Next blocker: the score review accepted the direct \(1/|C|\) posterior
derivation and shifted to realistic institutional privacy: repeated
observations, multi-target search, and longitudinal linkage across windows.

## Latest Privacy Longitudinal-Linkage Slice

Result:

- schema:
  `docs/governance/agent/privacy_longitudinal_linkage_schema.json`;
- fixture suite:
  `docs/governance/agent/fixtures/privacy_longitudinal_linkage/`;
- verifier:
  `scripts/privacy-longitudinal-linkage-verify`;
- report:
  `reports/privacy-longitudinal-linkage/20260529T090720Z/privacy-longitudinal-linkage-report.json`;
- docs:
  `docs/privacy/longitudinal-linkage.md`;
- candidate paper:
  `docs/archive/whitepaper-drafts/2026-05-29/privacy-longitudinal-linkage/whitepaperv2-privacy-longitudinal-linkage.md`;
- screen score: `86`; not confirmed or promoted over active `86,87,87`;
- the valid fixture leaves 16 candidates after intersecting four linked
  windows and leaves 64 candidates for four target queries under the union
  bound;
- negative controls cover linked-window shortfall, multi-target shortfall,
  repeated disclosure, exchange-side batch linkage, recurring timing, direct
  RPC reuse, thin asset-policy reuse, declared side information, ungoverned
  policy, root mismatch, statement-hash mismatch, and verifier-claim removal.

Next blocker: the score review accepted measured linked-window intersection and
the multi-target union bound. The next lane should model adversarial
misreporting of the economic admission inputs \(B_i,C_i,G_i(w),L_i(w)\) and
the challenge/removal cost that makes false attestation costly.

## Latest Economic Input-Misreporting Slice

Result:

- schema:
  `docs/governance/agent/validator_economic_input_misreporting_schema.json`;
- fixture suite:
  `docs/governance/agent/fixtures/validator_economic_input_misreporting/`;
- verifier:
  `scripts/validator-economic-input-misreporting-verify`;
- report:
  `reports/validator-economic-input-misreporting/20260529T092459Z/validator-economic-input-misreporting-report.json`;
- docs:
  `docs/governance/validator-economic-input-misreporting.md`;
- candidate papers:
  `docs/archive/whitepaper-drafts/2026-05-29/economic-input-misreporting/whitepaperv2-economic-input-misreporting.md`
  and
  `docs/archive/whitepaper-drafts/2026-05-29/economic-input-misreporting/whitepaperv2-economic-input-misreporting-v2.md`;
- screen scores: `86` and `86`; neither was confirmed or promoted over active
  `86,87,87`;
- the valid fixture derives \(B_i=15.0m\), \(C_i=3.2m\),
  \(G_i(w)=1.2m\), and \(L_i(w)=12.5m\) from source-bound components before
  admitting;
- negative controls cover inflated \(B_i\), understated \(C_i\), understated
  \(G_i(w)\), overstated \(L_i(w)\), self-declared-only \(B_i\), stale
  evidence, attestor overlap, missing challenge bond, missing liability /
  exclusion source classes, ungoverned input policy, root mismatch, statement
  hash mismatch, and verifier-claim removal.

Next blocker: the score review accepted the misreporting route mechanics but
treated the USD values as controlled-fixture calibration. The next lane should
stress those margins under market downturn and volume-decay factors, then state
the registry-level condition for how many validators must retain positive
\(B_i-C_i\) and \(L_i(w)-G_i(w)\) margins.

## Latest Cobalt B-Budget Theorem Slice

Result:

- candidate paper:
  `docs/archive/whitepaper-drafts/2026-05-29/cobalt-b-budget-theorem/whitepaperv2-cobalt-b-budget-theorem.md`;
- screen score: `86`;
- confirmed score: `86,86,87`;
- promoted as public V2 `1.1.21` under the explicit `86/86/86` gate;
- the promoted Section 2 now states the budget boundary directly: Cobalt
  checks the declared rooted trust graph and cannot prove off-chain operators
  are not secretly coordinated;
- the transition proposition is now conditional on the true active-Byzantine
  count staying within \(B\), while admission, correlation checks, challenge
  records, and Cobalt removal are empirical controls for keeping the live
  validator population inside that budget.

Next blocker: make the proof chain more formal. The next lane should define
the shared-correct-signer lemma, define the lock-import lemma across the
old/new registry boundary, make the transition-chain induction explicit, and
only then add a semi-quantitative residual model for silent collusion if the
proof pass stays flat.

## Latest Attestor-Reach-Bound Slice

Result:

- rejected candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/cobalt-proof-lemmas/whitepaperv2-cobalt-proof-lemmas.md`
  screened `85`;
- rejected candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/dynamic-economic-stability/whitepaperv2-dynamic-economic-stability.md`
  screened `86` and confirmed `86,86,84`;
- rejected candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/economic-units-example/whitepaperv2-economic-units-example.md`
  screened `86` and confirmed `86,86,83`;
- promoted candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/attestor-reach-bound/whitepaperv2-attestor-reach-bound.md`;
- screen score: `86`;
- confirmed score: `86,86,86`;
- promoted as public V2 `1.1.22` under the explicit `86/86/86` gate;
- the promoted Section 1 now states that an economic evidence path is
  inadmissible if it can be flipped by at most \(B\) coordinated attestor
  groups, and that the current verifier requires \(B+1\) independent attestor
  groups plus \(B+1\) non-equivalent control surfaces to manufacture \(B_i\) or
  erase a conflict/external-position signal.

Next blocker: define a measurement/bounding model for \(B_i\) and \(L_i(w)\)
themselves. Avoid a single clean numeric example unless the launch-threshold
wording is fixed first and the example includes sensitivity to false
attestation or source-class capture.

## Latest Economic Measurement Model Slice

Result:

- candidate paper:
  `docs/archive/whitepaper-drafts/2026-05-29/economic-measurement-model/whitepaperv2-economic-measurement-model.md`;
- backing verifiers:
  `scripts/validator-economic-synthetic-flow-verify --verify-report`,
  `scripts/validator-economic-loss-measurement-verify --verify-report`, and
  `scripts/validator-economic-input-misreporting-verify --verify-report`;
- screen score: `86`;
- confirmed score: `86,86,86`;
- promoted as public V2 `1.1.23` under the explicit `86/86/86` gate;
- the promoted Section 1 now states that \(B_i\) and \(L_i(w)\) are
  USD-equivalent floors over active observation windows, names the allowed
  source classes, and routes candidate-owned, self-declared, stale,
  wash-routed, circular, under-diversified, affiliated, wrong-source,
  unbounded, or ungoverned evidence away from admit.

Next blocker at the time: privacy linkage. The following slice addressed it by
scoping the basis-point privacy derivation against a concrete
repeated-observation linkage adversary without adding another artifact
inventory to the paper.

## Latest Privacy Linkage Scope Promotion

Result:

- candidate paper:
  `docs/archive/whitepaper-drafts/2026-05-29/privacy-linkage-scope/whitepaperv2-privacy-linkage-scope.md`;
- backing verifiers:
  `scripts/privacy-longitudinal-linkage-verify --verify-report`,
  `scripts/privacy-correlation-bound-verify --verify-report`, and
  `scripts/privacy-deanonymization-bound-verify --verify-report`;
- screen score: `86`;
- confirmed score: `86,86,86`;
- promoted as public V2 `1.1.24` under the explicit `86/86/86` gate;
- the promoted Section 3 now states that the basis-point privacy number is a
  declared-channel routing bound, not a universal anonymity theorem, and that
  repeated linkage, amount/timing side information, thin asset-policy reuse,
  direct RPC reuse, repeated disclosure, or hidden off-chain priors route to
  batching, delay, private relay, self-hosted RPC, explicit disclosure, or
  downgrade.

Next blocker: economic-admission adversarial measurement. A good next
candidate should walk one dominant-stakeholder attack through source-class,
freshness, and \(B+1\) attestor/control-surface checks, while explicitly
stating which trust assumptions remain residual.

## Latest Attestor Cost Promotion

Result:

- backing verifier:
  `scripts/validator-economic-attestor-cost-verify --verify-report`;
- report:
  `reports/validator-economic-attestor-cost/20260529T112123Z/validator-economic-attestor-cost-report.json`;
- candidate paper:
  `docs/archive/whitepaper-drafts/2026-05-29/attestor-cost-model/whitepaperv2-attestor-cost-model.md`;
- screen score: `86`;
- confirmed score: `86,86,86`;
- promoted as public V2 `1.1.26` under the explicit `86/86/86` gate;
- the promoted Section 1 now states that \(B+1\) independent attestor groups
  and surfaces are necessary but not sufficient: each capture path must also
  exceed the governed capture/value floor. The controlled fixture uses \(B=2\),
  a 4,000,000 USD short-window value-at-stake cap, a 15,000 bps ratio, a
  6,000,000 USD required floor, and 7,500,000 USD floors for both
  manufacture-\(B_i\) and conflict-erasure paths.

Next blocker: challenge/removal economics and silent-collusion residual risk.
The next lane should not rebuild the attestor-cost model unless the score
regresses; ask for targeted edits first, then implement only if the objection
requires a verifier-backed challenge/removal packet.

## Latest Privacy Linked-Window Promotion

Result:

- fresh baseline score for V2 `1.1.26`: `86`;
- candidate paper:
  `docs/archive/whitepaper-drafts/2026-05-29/privacy-linked-window-example/whitepaperv2-privacy-linked-window-example.md`;
- screen score: `86`;
- confirmed score: `86,86,86`;
- promoted as public V2 `1.1.27` under the explicit `86/86/86` gate;
- the promoted Section 3 now derives the privacy floor as a route computation
  over measured metadata intersections rather than channel independence, and
  gives a repeated-linkage example: window cohorts of 64, 48, 40, and 32 still
  downgrade if their linked intersection is 12; the valid longitudinal fixture
  preserves the 625 bps route only when four linked windows leave at least 16
  candidates and a four-target search leaves at least 64 candidates.

Next blocker: run a fresh targeted synthesis. The last two promotions held the
gate rather than increasing it, and the likely next high-value technical edit
is either the cross-registry lock-import lemma or challenge/removal incentive
pricing, depending on the next baseline review.

## Latest Ratio-Unit Promotion

Result:

- fresh baseline score for V2 `1.1.27`: `86`;
- failed composite candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/claims-ledger-ratio-clarity/whitepaperv2-claims-ledger-ratio-clarity.md`;
- failed composite score: screen `86`, confirm `86,84,84`;
- promoted candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/attestor-ratio-unit-fix/whitepaperv2-attestor-ratio-unit-fix.md`;
- screen score: `86`;
- confirmed score: `86,86,86`;
- promoted as public V2 `1.1.28` under the explicit `86/86/86` gate;
- the promoted Section 1 and Appendix A now state that the governed
  capture/value floor is `15,000 bps`, meaning `150%` or `1.5x`, so
  4,000,000 USD of capped value-at-stake maps to a 6,000,000 USD required
  capture-cost floor.

Next blocker: do not retry a broad claims ledger immediately. Build or draft a
capture/value sensitivity model and a detectability assumption for silent
collusion, then integrate only if it can be made concise and score-safe.

## Latest Economic Payoff Promotion

Result:

- fresh baseline score for V2 `1.1.28`: `86`;
- failed candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/cobalt-lock-import-proof/whitepaperv2-cobalt-lock-import-proof.md`;
- failed candidate score: screen `85`;
- promoted candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/economic-payoff-sensitivity/whitepaperv2-economic-payoff-sensitivity.md`;
- screen score: `86`;
- confirmed score: `86,86,86`;
- promoted as public V2 `1.1.29` under the explicit `86/86/86` gate;
- the promoted Section 1 now scopes incentive compatibility to attributable
  deviation classes where \(L_i^a(w)-G_i^a(w)>0\), and says deviations with no
  attribution path or unbounded value-at-stake route to hold, fail-closed, or
  residual disclosure. It also turns the capture/value ratio into sensitivity:
  at a 4,000,000 USD value cap, 1.0x requires 4,000,000 USD, 1.5x requires
  6,000,000 USD, and 2.0x would require 8,000,000 USD.

Next blocker: silent-collusion detectability. A useful next packet or edit
should enumerate which source classes, challenge signals, conflict records,
and correlation controls must all be suppressed for a cartel to leave no
contradictory evidence.

## Latest Economic Attestation Lifecycle Promotion

Result:

- fresh baseline score for V2 `1.1.29`: `86`;
- new verifier:
  `scripts/validator-economic-attestation-lifecycle-verify`;
- report:
  `reports/validator-economic-attestation-lifecycle/20260529T121904Z/validator-economic-attestation-lifecycle-report.json`;
- promoted candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/economic-attestation-lifecycle/whitepaperv2-economic-attestation-lifecycle.md`;
- screen score: `87`;
- confirmed score: `86,86,86`;
- promoted as public V2 `1.1.30` under the explicit `86/86/86` gate;
- the promoted Section 1 now states that economic admission consumes an
  `EconomicAttestationRoot`, not naked USD values: each component binds a
  source class, attestor group, control surface, observation epoch, expiry
  epoch, refresh epoch, value semantics, content hash, and challenge state.
  Stale or refresh-due sources hold, unresolved bonded challenges hold, proved
  false attestations reject, and wrong roots or ungoverned lifecycle policies
  fail closed.

Next blocker: derive or defend the institutional risk constants and continue
cutting repeated evidence inventory if scores stay flat.

## Latest Risk-Constant And Silent-Boundary Attempts

Result:

- fresh baseline score for V2 `1.1.30`: `86`;
- new verifier:
  `scripts/validator-economic-risk-constant-policy-verify`;
- report:
  `reports/validator-economic-risk-constant-policy/20260529T123359Z/validator-economic-risk-constant-policy-report.json`;
- failed candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/economic-risk-constant-policy/whitepaperv2-economic-risk-constant-policy.md`;
- failed candidate score: screen `84`;
- second failed candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/silent-collusion-boundary/whitepaperv2-silent-collusion-boundary.md`;
- second candidate score: screen `86`, confirm `86,86,84`;
- no public-paper promotion; active paper remains V2 `1.1.30`.

The useful artifact is the verifier: it derives the 4,000,000 USD value cap
from governed bucket maxima and confirms that 7,500,000 USD clears 1.0x and
1.5x but holds at 2.0x. The paper insertion was penalized as another
fixture-tuned numeric story. The next high-value move is now either a real
Cobalt lemma chain or an attestor-bootstrap/blast-radius statement, not more
numeric fixture exposition.

## Latest Registry-Addition Liveness Promotion

Result:

- fresh baseline score for V2 `1.1.30`: `86`;
- new verifier:
  `scripts/validator-registry-addition-liveness-verify`;
- report:
  `reports/validator-registry-addition-liveness/20260529T131018Z/validator-registry-addition-liveness-report.json`;
- promoted candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/registry-addition-liveness/whitepaperv2-registry-addition-liveness.md`;
- screen score: `86`;
- confirmed score: `86,86,86`;
- promoted as public V2 `1.1.31` under the explicit `86/86/86` gate.

The promoted Section 2 now separates registry-addition liveness from settlement
liveness. Thin attestor or control-surface evidence can hold a validator
addition for a bounded window, but cannot activate an under-evidenced add,
keep an add pending forever, or halt ordinary settlement. Do not retry this
blocker as prose unless a later score specifically reopens it.

Next blocker: residual silent/unattributable collusion sizing and other
load-bearing assumptions that are not cryptographically observable. A useful
next packet should change selector routes or challenge evidence, not just add
another prose caveat.

## Latest Attestor B-Collusion Promotion

Result:

- fresh baseline score for V2 `1.1.31`: `86`;
- new verifier:
  `scripts/validator-attestor-market-calibration-verify`;
- report:
  `reports/validator-attestor-market-calibration/20260529T133200Z/validator-attestor-market-calibration-report.json`;
- failed calibration candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/attestor-market-calibration/whitepaperv2-attestor-market-calibration.md`;
- failed calibration score: screen `84`;
- promoted candidate:
  `docs/archive/whitepaper-drafts/2026-05-29/attestor-b-collusion-example/whitepaperv2-attestor-b-collusion-example.md`;
- screen score: `86`;
- confirmed score: `86,86,87`;
- promoted as public V2 `1.1.32` under the explicit `86/86/86` gate.

The promoted Section 1 now gives a concrete \(B=2\) attestor-collusion attack:
two colluding attestors can create a dispute record, but cannot create an
admissible `EconomicAttestationRoot`; buying the third required path leaves the
\(B\)-bounded case and must clear the cost-to-forge gate. The direct
calibration paragraph was rejected as too fixture-shaped, so keep calibration
evidence in the repo and use it only where it supports an adversarial example
or selector route.

Next blocker: if economics recurs, prefer adversarial-attestor sensitivity or
distribution evidence over another single-number derivation. Otherwise move to
AI cross-profile packet-family coverage or omission/receipt liveness.

## Promotion Rules

- Never promote below the active gate. If no explicit gate is supplied, use the
  confirmed public score as the gate.
- Flat score is not automatic promotion; promote only if the paper is cleaner
  or the repo evidence is important enough to preserve in the public argument.
- If an evidence insertion lowers score, keep the committed evidence and
  remove the public-paper insertion.
- Do not let implementation inventory accumulate in the whitepaper body.
- Every promoted score result gets a short journal entry.

## Stop Rules

Stop or pivot when:

- 90 minutes pass without either a score result, a concrete candidate draft, or
  a passing verifier;
- three score attempts hit the same blocker with no new information;
- an implementation artifact cannot be verified with negative controls;
- the candidate grows by more than 500 words without a score improvement;
- the paper begins to optimize for the scorer by making a technically false
  claim.

When in doubt, preserve truth and improve the implementation. The score is the
gate, not permission to lie.
