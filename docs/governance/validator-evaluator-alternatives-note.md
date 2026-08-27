# Validator Evaluator Alternatives Decision Note

**Status:** Decision note — Text Improvement Harness full gate passed on 2026-08-27 — average 89.27/100 (GPT 90.60, Fable 88.40, GLM 88.80; five runs per lane; run group `validator-evaluator-alternatives-note`); scored content SHA-256 `1986dec6196a71fce83dcea38a4d0ed7f8c27c4fb2655eeba2aef6c88f539fa7`
**Date:** 2026-08-27
**Author:** Domagoj Ravlić (`dravlic`)
**Decision owner:** Post Fiat
**Related:** [Dynamic UNL Proposal Source Research Specification](dynamic-unl-proposal-source-research-spec.md) and [deferred milestone draft](../deferred-plans/dynamic-unl-proposal-source-milestone.md)

## Decision question

What should decide which validators deserve to be added, retained, or removed?
The answer must decentralize proposal content without confusing evaluation,
submission, Cobalt ratification, or block finality.

The registry is governed state and old-registry authorization plus consensus
ordering guard its mutation [today](validator-registry.md). Cobalt is active only
for validator-registry and trust-graph ratification; Consensus v2 remains block
finality [and a Cobalt failure can only pause validator governance](cobalt.md).
The last authenticated chain observation records that every proposal and
authorization still came from Foundation-administered validators and says a
separate layer must decide merit
([Current State](../status/chain-state-current.md)). The production helpers also
assign the first current validator as proposer, so deterministic proposal
construction is not independent origination
([independent-operator specification](cobalt-independent-operator-proposal-path-research-spec.md)).

There are useful pieces, but the slot is still empty. The admission policy is a
pure deterministic add/hold/reject selector over a supplied evidence packet;
its delta is decision support, not authority ([Validator Registry](validator-registry.md),
[`validator_admission_policy.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/10abc3d7d9ac1a396eab6b28ac7fbd34a0bf35bf/crates/consensus_cobalt/src/validator_admission_policy.rs)).
The DGA repository code has bundle validation, canonical rulesets, frozen
evidence types, a no-op interpreter, dry-run records, and a local guarded-apply
fixture. Its Gate 9.5 candidate always admits fixture `validator-3`; it does not
evaluate the live candidate population. No DGA-generated production proposal
has been submitted or changed the live registry
([overview](deterministic-governance-overview.md),
[`implementation_guarded_apply.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/10abc3d7d9ac1a396eab6b28ac7fbd34a0bf35bf/crates/node/src/governance_agent_parts/implementation_guarded_apply.rs)).
The DGA plan's intended “policy proposes bounded validator-registry actions”
pipeline therefore remains an end state, not live behavior
([DGA plan](deterministic-governance-agent-plan.md)).

## Criteria

These criteria come from the PFT Ledger's phased Dynamic UNL design: frozen
round packages and manifests, independently pinned execution, validator
commit/reveal, deterministic formula and selection, and staged removal of the
Foundation. See the pinned [roadmap](https://github.com/postfiatorg/dynamic-unl-scoring/blob/f18e6b4c40d3df17afb46804ef4f2bb0d879b439/docs/CurrentRoadmap.md),
[Phase 3 charter](https://github.com/postfiatorg/dynamic-unl-scoring/blob/f18e6b4c40d3df17afb46804ef4f2bb0d879b439/docs/research/Phase3ResearchCharter.md),
[deterministic final-score design](https://github.com/postfiatorg/dynamic-unl-scoring/blob/f18e6b4c40d3df17afb46804ef4f2bb0d879b439/docs/DeterministicFinalScore.md),
and [sidecar overview](https://github.com/postfiatorg/validator-scoring-sidecar/blob/e6907faa5bb554199fcd2102a5ec1e7fbec8db90/docs/Overview.md).

| Code | Named criterion | Test |
| --- | --- | --- |
| FE | Frozen public evidence | Inputs, provenance, freshness, conflicts, and hashes are public and fixed before evaluation. |
| DR | Deterministic replay | Pinned versions and integer/mechanical rules reproduce byte-identical results. |
| IC | Independent convergence | Every validator can re-execute and commit/reveal; no publisher's output is authority. |
| GM | Governed method | Evidence schemas, formula, selector, model/runtime, upgrades, and revocation are versioned governance choices. |
| MA | Merit and anti-capture | The method measures useful operation and resists Sybil identities, correlated control, and wealth or incumbent capture. |
| BC | Bounded Cobalt-safe churn | Every delta fits a graph-derived transition budget and preserves old-rule authorization and linkedness. |
| FC | Fail-closed continuity | Missing, stale, conflicting, divergent, or unsafe evidence retains the last known-good registry. |
| FD | Foundation exit | The design has explicit phases that reduce the Foundation to a non-authoritative participant. |

`✓` means the candidate naturally satisfies the criterion when built as stated;
`partial` means an additional trusted input or unbuilt control is material; `✗`
means the candidate conflicts with the criterion.

## Candidate 1 — Dynamic UNL supplies proposal content

A sealed, threshold-qualified PFT Ledger scoring round becomes the content
source. An L1 adapter verifies its frozen package, manifest, formula, selector,
commit/reveal convergence, signed-vote lineage, and governed identity map, then
projects the selected list into one canonical L1 delta. This is the architecture
already specified and harness-scored at 89.13
([research specification](dynamic-unl-proposal-source-research-spec.md)).

| Criterion | Score and one-line reason |
| --- | --- |
| FE | partial — packages are frozen and public, but signed validation-vote publication and independent observers are still pending. |
| DR | ✓ — model, runtime, prompt, formula, selector, inputs, and canonical outputs are manifest-pinned. |
| IC | ✓ — sidecars already independently rerun and commit/reveal their own result rather than accept a publisher hash. |
| GM | partial — model-governance rounds exist, but sidecar verification and authority-transfer gates remain open. |
| MA | partial — broad evidence and contextual scoring help, but identity, observer completeness, and common-control proof remain weak. |
| BC | partial — the selector bounds list churn, but its projection through every L1 Cobalt graph budget is unbuilt. |
| FC | ✓ — the proposed adapter holds on missing lineage, divergence, unsafe churn, or mapping ambiguity. |
| FD | ✓ — the roadmap explicitly moves from Foundation publication to validator-reproduced content and later decentralized publication. |

- **Build:** `new: crates/types/src/dynamic_unl_proposal.rs`, `new: crates/consensus_cobalt/src/dynamic_unl_source.rs`, and `new: python/postfiat_rpc/dynamic_unl_proposal.py`; then bind the verifier to the existing admission and Cobalt paths.
- **Main failure:** manipulated or incomplete evidence can create a shared wrong answer; a common-mode model error can then converge perfectly.
- **Composition and reuse:** deterministically stage or no-op any target beyond Cobalt's graph budget, submit its unchanged bytes through the independent-operator envelope, and reuse the running PFT frozen-package, pinned inference, formula, selector, sidecar, commit/reveal, and convergence pipeline.

## Candidate 2 — L1-native DGA policy

A Cobalt-governed model bundle generates a typed ruleset; a networkless Rust
interpreter runs it over frozen L1 evidence and emits a bounded
`RegistryDeltaCandidate`. This keeps policy native to the chain's constitution.
Existing node code proves schemas and local fixture gates, including one fixed
add and rollback, but not live candidate discovery, validator-wide replay, a
real merit policy, or production proposal origination.

| Criterion | Score and one-line reason |
| --- | --- |
| FE | partial — frozen evidence roots and field registries exist, but no decentralized live evidence collector fills them. |
| DR | ✓ — canonical rulesets, compiled-policy hashes, sandboxed interpretation, and replay receipts are implemented for fixtures. |
| IC | partial — provider replays exist, but active validators do not independently generate and commit/reveal a live policy result. |
| GM | partial — the object model is designed for Cobalt governance, but its bundle and policy lineage are not live governed state. |
| MA | partial — control-group and reliability fields exist, but the Gate 9.5 subject and passing evidence are fixture constants. |
| BC | partial — one-add caps and Cobalt acceptance exist locally, but the linkedness flag is not a live graph-derived displacement proof. |
| FC | ✓ — ambiguous evidence no-ops, forbidden access rejects, and dry-run/guarded-apply gates are separated. |
| FD | partial — authority transfer is a planned phase while model experiments and proposal construction remain Foundation-operated. |

- **Build:** move consensus-owned objects into `new: crates/types/src/governance_agent_policy.rs`, add `new: crates/consensus_cobalt/src/governance_agent_admission.rs`, and add a validator service/CLI that constructs live frozen packets and independently replays policy.
- **Main failure:** a governed but flawed ruleset or common-mode model generation can mechanically encode bad policy; unavailable inference can freeze upgrades.
- **Composition and reuse:** make the interpreter output pass the same graph-specific Cobalt budget and independent-operator envelope; reuse existing DGA schemas, gate receipts, admission fields, dry-run state, and guarded-apply code, but not the fixture's hard-coded candidate.

## Candidate 3 — deterministic formula baseline

Each validator reconstructs a frozen epoch packet of agreement, uptime, latency,
and defensible provider/geo correlation observations. A versioned integer formula
scores candidates and a mechanical selector applies eligibility, diversity,
continuity, ties, and churn rules. This is the Phase 3 charter's null hypothesis
and operational fallback; provider and geo claims must be signed observations,
not self-declared labels.

| Criterion | Score and one-line reason |
| --- | --- |
| FE | partial — agreement and uptime can be chain-derived, while latency and real provider/geo control need independent signed observers. |
| DR | ✓ — integer metrics, formula, tie-breaks, and selector are fully replayable without inference. |
| IC | ✓ — every validator can recompute and commit/reveal the same epoch result at modest cost. |
| GM | ✓ — formula coefficients, evidence schemas, selector, and activation versions can all be governed objects. |
| MA | partial — transparent metrics deter some failures but invite threshold gaming and miss contextual or beneficial-control evidence. |
| BC | ✓ — the selector can apply the exact L1 graph budget and deterministic staging before proposal creation. |
| FC | ✓ — incomplete or conflicting observations can deterministically hold the candidate and retain the registry. |
| FD | ✓ — no Foundation scorer or privileged inference output is required once observation is independently replicated. |

- **Build:** `new: crates/types/src/validator_evidence_epoch.rs`, `new: crates/consensus_cobalt/src/validator_formula_policy.rs`, and `new: python/postfiat_rpc/validator_evaluator.py`, plus signed observer packets and commit/reveal storage.
- **Main failure:** validators optimize visible thresholds or disguise correlated infrastructure; manipulated latency/location observations create a false diversity signal.
- **Composition and reuse:** calculate only a safe staged prefix under Cobalt and use the independent proposal envelope; reuse PFT's final-score, selector, manifest, canonical-artifact, and commit/reveal patterns, but not its model runtime.

## Candidate 4 — current validators vote directly

Any current member nominates an admission, retention, or removal and Cobalt
members cast discretionary ballots, optionally after an evidence window. The
accepted ballot result supplies proposal content. This decentralizes one
Foundation decision across the incumbent set, but turns reproducible evaluation
into political judgment by the parties most affected by entry and exit.

| Criterion | Score and one-line reason |
| --- | --- |
| FE | partial — evidence can be mandatory and frozen, but voters can interpret or ignore it unless every decision is mechanically constrained. |
| DR | ✗ — ballot counting replays, but the validators' merit judgments do not. |
| IC | ✗ — members reveal preferences rather than independently reproduce one deterministic evaluator result. |
| GM | partial — ballot procedure is governable, while the substantive merit rule remains discretionary. |
| MA | ✗ — incumbents can cartelize, block entrants, protect weak peers, or coordinate removals. |
| BC | partial — Cobalt can cap the accepted delta, but a safe-sized cartel decision can still be substantively bad. |
| FC | ✓ — failure to reach the old-set threshold leaves the current registry active. |
| FD | partial — it removes a Foundation monopoly but replaces it with incumbent control rather than open evaluation. |

- **Build:** `new: crates/consensus_cobalt/src/validator_membership_ballot.rs`, canonical nomination/evidence types in `crates/types`, and the independent-operator proposal envelope and transport.
- **Main failure:** incumbent lock-in or cartel capture; a liveness freeze is rational when members expect a proposal to remove them.
- **Composition and reuse:** Cobalt's transition budget remains a hard outer bound and the independent path broadens nomination; PFT commit/reveal mechanics can hide early ballots, but the scoring/formula pipeline is mostly unused.

## Candidate 5 — bond-gated admission plus formula

A validator locks a PFT bond for a fixed term; expiry, provable protocol faults,
and a challenge process govern return or slashing. The bond is an eligibility
and Sybil-cost gate, while Candidate 3's formula ranks and retains operators.
Bond-only admission is not scored separately because wealth is not validator
merit and therefore does not genuinely fill the evaluator slot.

| Criterion | Score and one-line reason |
| --- | --- |
| FE | partial — bonds are chain-visible, but performance, control, and slash evidence still need frozen external or protocol observations. |
| DR | ✓ — bond state, expiry, formula, selector, and objective slash predicates can replay deterministically. |
| IC | ✓ — validators can independently verify the same bond and formula state without accepting one scorer. |
| GM | ✓ — bond amounts, terms, predicates, formula, selector, and upgrades can be versioned governance parameters. |
| MA | partial — a bond raises Sybil cost but favors wealthy controllers and does not prove operational independence. |
| BC | ✓ — formula selection and Cobalt staging can bound entries and exits regardless of available capital. |
| FC | partial — objective faults can fail closed, but disputed/off-chain slashing evidence risks either injustice or permanent no-op. |
| FD | partial — it removes the Foundation evaluator but may concentrate power in capital custodians and large holders. |

- **Build:** `new: crates/types/src/validator_bond.rs`, `new: crates/execution/src/validator_bond.rs`, storage/RPC/CLI lifecycle support, and Candidate 3's evidence formula and selector.
- **Main failure:** a wealthy Sybil cluster buys admission, or an ambiguous slashing oracle becomes a censorship and confiscation point.
- **Composition and reuse:** the bond never bypasses Cobalt's transition budget or independent submission; reuse PFT selector/commit-reveal patterns and existing L1 asset, receipt, governance, and registry machinery, but no running PFT bond service exists.

## Candidate 6 — Foundation proposes, Cobalt ratifies

The Foundation continues choosing the intended delta and operating every current
proposer and authorizer; Cobalt checks and ratifies the transition, and Consensus
v2 orders it. This is the controlled-devnet status quo, not an evaluator design.
It has demonstrated bounded protocol authority, replay, negative rejection, and
rollback, but not decentralized merit judgment or proposal origination.

| Criterion | Score and one-line reason |
| --- | --- |
| FE | ✗ — proposal intent is an administrative decision, not the output of a frozen public evidence rule. |
| DR | partial — proposal bytes and ratification replay, but the Foundation's selection judgment does not. |
| IC | ✗ — validators ratify one administrator's proposed content instead of reproducing an evaluator. |
| GM | ✗ — Cobalt scope is governed, but validator merit and proposal-content policy are not. |
| MA | partial — manual review may catch nuance, but one administration controls identity interpretation and selection. |
| BC | partial — Cobalt and old-rule checks constrain transitions, but no mechanical content selector derives the safe staged target. |
| FC | ✓ — invalid, stale, under-authorized, or unratified transitions leave governed state unchanged. |
| FD | ✗ — the Foundation remains proposer, validator administrator, and practical evaluator. |

- **Build:** no new evaluator component; only continued operations and evidence publication.
- **Main failure:** administrative capture or compromise produces biased proposal content; loss of the Foundation process freezes registry evolution.
- **Composition and reuse:** it already uses Cobalt and can later adopt the independent proposal envelope, but it reuses none of PFT's independent scoring, formula, selector, or commit/reveal authority-transfer work.

## Comparison matrix

| Candidate | FE | DR | IC | GM | MA | BC | FC | FD | Effort | PFT reuse |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1. Dynamic UNL content | partial | ✓ | ✓ | partial | partial | partial | ✓ | ✓ | High | Very high |
| 2. L1-native DGA | partial | ✓ | partial | partial | partial | partial | ✓ | partial | High | Medium |
| 3. Formula baseline | partial | ✓ | ✓ | ✓ | partial | ✓ | ✓ | ✓ | Medium | High |
| 4. Direct member vote | partial | ✗ | ✗ | partial | ✗ | partial | ✓ | partial | Medium | Low |
| 5. Bond plus formula | partial | ✓ | ✓ | ✓ | partial | ✓ | partial | partial | Very high | Medium |
| 6. Status quo | ✗ | partial | ✗ | ✗ | partial | partial | ✓ | ✗ | Low | None |

An opaque AI oracle was excluded because one service's answer would be authority,
even if the model were deterministic. Random sortition or permissionless entry
was excluded because it allocates seats without evaluating merit and makes Sybil
resistance the whole unsolved problem.

## Recommendation

Choose a precise combination: **Candidate 1 supplies the ranked target and
proposal content; Candidate 2 supplies the L1 constitutional envelope and hard
bounds; the independent-operator path supplies the submitter; Cobalt ratifies;
Consensus v2 orders.** DGA must govern the admitted Dynamic UNL source/schema,
model and runtime version, evidence and identity rules, selector, graph-derived
churn cap, staging/no-op rule, revocation, and emergency behavior. It must not
rescore validators, rewrite the ranking, or conceal the full selected-target
hash.

Keep Candidate 3 running as the published null hypothesis and fallback. A
Dynamic UNL source failure, divergence, or missing evidence must first produce
`no_op` and retain the current registry; it must not silently switch formulas.
The formula may become proposal authority only under a separately pre-governed,
versioned fallback activation with the same frozen-evidence, independent
commit/reveal, Cobalt-budget, and independent-submission gates.

In plain English, Dynamic UNL best reuses a system where validators already do
the work themselves and prove what they computed. DGA is still needed to say
which system version is legitimate and to enforce L1 safety, while the simpler
formula shows whether the model adds enough value and provides a recoverable
path that does not depend on inference.

### Exact decision to record

> Post Fiat selects Dynamic UNL as the target validator evaluator and canonical
> proposal-content source, constrained by a Cobalt-governed L1 DGA policy and
> graph-specific transition budget. Independent admitted operators may submit
> the unchanged canonical bytes, Cobalt alone ratifies validator-trust changes,
> Consensus v2 orders them, and the deterministic formula remains a shadow
> baseline and separately activated fail-closed fallback; no live authority
> changes until the research specification's evidence, identity, replay,
> churn, independent-operator, clone, and authorization gates pass.

## Questions that would change the recommendation

1. Can PFT Ledger Evidence E.1 publish the exact signed vote sets, and can E.2
   demonstrate genuinely independent observers rather than one observation
   domain? A negative answer moves Candidate 3 to primary.
2. Can affected PFT master keys be mapped to L1 validator/operator identities
   before each round with expiry, revocation, and common-control evidence? A
   negative answer blocks Candidates 1, 3, and 5 from live admission.
3. What is the proved graph-specific displacement budget for each active Cobalt
   topology, including removals and failed incumbent cooperation? A smaller or
   liveness-breaking budget changes the staging design or can block automation.
4. Will enough independent operators bear pinned inference cost and govern model
   choice without one provider/runtime becoming a practical authority? A
   negative answer favors the formula primary and Dynamic UNL only as advisory.

## What this note does not do

This note grants no proposer, model, DGA process, validator, Foundation process,
or external chain any authority. It changes no registry, trust graph, policy,
service, deployment, or chain state. It locks no research specification or
milestone, authorizes no implementation or live drill, and makes no Task Node
request.
