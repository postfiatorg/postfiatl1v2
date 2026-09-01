# AI Governance Validity Review

**Date:** 2026-08-31  
**Status:** decision record  
**Scope:** PostFiat L1 validator governance, the existing `postfiatd` Dynamic UNL system, validator scoring sidecars, and the published AI replay evidence.

## Decision

**Do not give an AI model direct governance authority. Keep Cobalt as the authority boundary.**

The valid architecture is:

```text
signed public evidence
  -> deterministic checks
  -> optional replayable model classification
  -> deterministic bounded proposal
  -> independent Cobalt ratification
  -> consensus ordering
```

The invalid architecture is:

```text
model output -> validator admission/removal, quorum change, or governance vote
```

The existing `postfiatd` Dynamic UNL system is valuable audit and experimentation infrastructure, but **it is not usefully decentralized today**. The Foundation still controls the authoritative evidence observer, scoring round, model and prompt, selector, validator-list signing key, publication path, and emergency overrides. Validator sidecars reproduce Foundation-defined work in shadow; they do not presently control the network's authoritative validator list.

The PostFiat L1 proposal is valid only in its narrow form: a model may turn ambiguous, source-bound evidence into a typed, replayable classification that deterministic code can reject or convert into a bounded proposal. The model must not vote, hold a governance key, change the registry, select its own rules, or become necessary for chain liveness.

**Go/no-go today:**

- **GO:** AI research, shadow scoring, replay, dissent receipts, and bounded proposal generation under Cobalt.
- **NO-GO:** AI-controlled validator membership, AI votes, AI-selected quorum rules, or transfer of authority from Cobalt.
- **NO-GO:** claiming that the current Dynamic UNL deployment decentralizes authority.
- **KEEP:** Cobalt/PoA while operator, evidence-source, and proposal-source independence are built and measured.

## What “usefully decentralized” means

A governance system is usefully decentralized only if the failure or capture of one organization cannot silently determine validator membership or suppress every competing proposal.

For this review, that requires all of the following:

1. No single party can curate the authoritative evidence set without detectable omission.
2. Independent parties can submit competing, valid proposals.
3. Registry changes require independently controlled validator keys.
4. Deterministic safety rules constrain every proposer, including the Foundation.
5. No publisher key, web repository, administrator endpoint, model endpoint, or GPU account can unilaterally change authority.
6. The chain remains live under the last valid registry if inference disappears.
7. Operators can verify the authority-relevant result at a sustainable cost.
8. Operator, funding, key-custody, release-management, infrastructure, and observation independence are measured rather than inferred from node count.

Exact replay by several machines is not enough. Five sidecars operated by one organization are five executions in one trust domain, not five independent governors.

## Systems reviewed

The review used repository state rather than product descriptions alone.

| Component | Revision reviewed | Relevant role |
|---|---|---|
| [postfiatl1v2](https://github.com/postfiatorg/postfiatl1v2/tree/30e50e2a10b608eac359e69b0d59d85487d0f48c) | `30e50e2a` | Cobalt authority, deterministic governance agent, L1 whitepaper and observer proposal |
| [postfiatd](https://github.com/postfiatorg/postfiatd/tree/dcfe281b052153130accf4001dca63c84b1b7b6d) | `dcfe281b` | Signed validator-list consumption and historical Dynamic UNL design |
| [dynamic-unl-scoring](https://github.com/postfiatorg/dynamic-unl-scoring/tree/f18e6b4c40d3df17afb46804ef4f2bb0d879b439) | `f18e6b4c` | Foundation scoring, artifacts, selection, publication, commit/reveal and roadmap |
| [validator-scoring-sidecar](https://github.com/postfiatorg/validator-scoring-sidecar/tree/e6907faa5bb554199fcd2102a5ec1e7fbec8db90) | `e6907faa` | Independent replay and validator commit/reveal client |
| [postfiat.org source](https://github.com/postfiatorg/postfiatorg.github.io/tree/e4b2a19f79a3c210374f4f07b35f26298391eb46) | `e4b2a19f` | AI replay publications |

This is an architectural and evidence review. It does not authorize a mainnet change, spend on rented compute, or mutate either network.

## Current implementation: what actually has authority

### PostFiat L1

Cobalt is active for validator-registry and trust-graph changes. Consensus v2 orders and finalizes blocks. The current operational record reports six validators on transactional storage at height 931 and states that Cobalt remains the validator-trust authority. It also states the critical limitation: current proposals and authorizations originate from Foundation-administered validators. See [PostFiat L1 current state](../status/chain-state-current.md).

The L1 repository contains substantial deterministic-governance-agent code, but its documented boundary is decision support, not mutation authority. Model output cannot bypass typed policy, operator authorization, or Cobalt ratification. See the [core feature audit](../security/core-feature-loc-audit-inventory.md) and [deterministic governance overview](deterministic-governance-overview.md).

There is **no deployed L1-native AI governance sidecar with authority**. The L1 observer and Dynamic UNL adapter are research specifications. The recommended sequence is explicitly `SHADOW_ONLY` until native evidence lineage, observer coverage, replay, convergence, and failure semantics qualify. See the [L1 evidence-source decision note](dynamic-unl-l1-evidence-source-note.md) and [observer research specification](l1-observer-research-spec.md).

### PFT Ledger / `postfiatd`

The model does not run inside `postfiatd` consensus. Nodes fetch a signed validator list from a configured URL and trust its configured publisher key. The current testnet setup points operators to the Foundation-published list at `postfiat.org`; an unknown publisher key is rejected. See [NodeSetup.md at the reviewed revision](https://github.com/postfiatorg/postfiatd/blob/dcfe281b052153130accf4001dca63c84b1b7b6d/docs/NodeSetup.md).

The current authoritative path is:

```text
Foundation observer/data collection
  -> Foundation scoring service
  -> Foundation-selected model, prompt and runtime profile
  -> Foundation selector
  -> Foundation validator-list signing key
  -> Foundation-controlled GitHub Pages path
  -> postfiatd nodes accept the signed list
```

The scoring roadmap says this plainly: Phase 2 does not transfer validator-list authority. The Foundation continues to collect evidence, score rounds, select the canonical UNL, sign it, and publish it. See [CurrentRoadmap.md](https://github.com/postfiatorg/dynamic-unl-scoring/blob/f18e6b4c40d3df17afb46804ef4f2bb0d879b439/docs/CurrentRoadmap.md).

The authoritative system also retains administrator-guarded custom-publish and rollback-publish paths. Those are reasonable operational kill switches during a controlled phase, but they are incompatible with a claim that authority has already been decentralized.

### Validator sidecars

The sidecars are real and useful. They can:

- obtain the frozen input package;
- validate a pinned execution manifest;
- run the model through their own Modal account or local SGLang;
- reproduce raw output, parsed scores, and selected-UNL hashes;
- commit a salted result and reveal it later on-chain; and
- publish convergence or dissent evidence.

The strongest live evidence reviewed is:

- a devnet sidecar completed round 273 through score, commit, and reveal;
- testnet round 13 produced a 5/5 valid convergence report; and
- the five testnet participants in that recorded gate were Foundation-operated validators.

The community rollout was announced, but the reviewed roadmap does not establish a comparable round with multiple independently administered organizations. The safe conclusion is therefore that the mechanism works, not that independent governance has been demonstrated.

The participation path also deserves a stronger key boundary. It mounts a validator key file read-only and invokes `postfiatd validator-keys sign`, while a funded relay wallet submits the memo. Read-only mounting limits mutation but does not make validator master-key exposure to a complex sidecar/inference deployment acceptable for authority-bearing governance. A production design should use a narrowly scoped governance/replay key and isolated signer, not expose a validator master key to the inference container. See the [sidecar repository](https://github.com/postfiatorg/validator-scoring-sidecar/tree/e6907faa5bb554199fcd2102a5ec1e7fbec8db90).

## What the AI replay evidence proves

The [LLM governance replay](https://postfiat.org/blog/llm-governance-replay/) is credible evidence for a narrow proposition: with a frozen evidence packet and pinned Qwen/SGLang execution profile, useful governance classifications can be replayed deterministically.

The published results include:

- 60/60 historical vote selections;
- 70/70 historical vote-state results;
- 47/47 held-out vote-state results;
- 44/46 held-out terminal outcomes; and
- exact historical output-hash convergence.

The result also contains the most important safety warning. Held-out triage achieved only 31/47 and emitted one unsafe `PROCEED` for DepositPreauth. The publication correctly concludes that this evidence does not support automatic model-driven positive votes.

The [cross-hardware replay](https://postfiat.org/blog/sglang-cross-hardware-replay/) strengthens the execution claim: H100 NVL and H200 runs converged under one pinned, adjacent same-vendor CUDA profile. A separate Apple/MLX exercise was a capability or constitutional-decision test, not target-model equivalence across arbitrary hardware.

These experiments prove:

- bounded replay can work;
- execution profiles can be committed and audited;
- sidecars can detect output drift; and
- model output can be made less private than an informal committee conversation.

They do **not** prove:

- that the model is correct;
- that the evidence packet is complete;
- that the question or option set is unbiased;
- that the selected validator set is safe;
- that participating operators are independent;
- that the Foundation cannot censor a candidate or proposal;
- that GPU access is sufficiently open; or
- that the model adds value over a deterministic rule on the production decision distribution.

Replay is an integrity property of a computation. Decentralization is a control property of a system. They are related, but they are not interchangeable.

## Review of the L1 whitepaper proposal

The [PostFiat L1 whitepaper](../whitepaper.md) makes the right distinction.

Its validator admission design makes source-bound exposure, accountability, reliability, attack surface, and correlation explicit. Missing or conflicting evidence holds rather than admits. Deterministic checks own signature validation, root matching, staleness, concentration caps, churn limits, and Cobalt certificates.

Its model has one narrow job: translate genuinely qualitative, public evidence into a closed-schema classification with citations to registered evidence fields. It cannot admit a validator, change a threshold, alter its prompt, modify the selector, or cast a vote. Removing the model makes ambiguous cases hold, which is more conservative rather than less safe.

That design is valid under three conditions:

1. **Least machinery:** use a model only where a deterministic predicate cannot express the question without recreating a private human committee.
2. **Deletion monotonicity:** deleting inference must never grant authority or halt the chain; it may only turn uncertain proposals into holds.
3. **Independent ratification:** a bounded proposal changes the registry only after verification under the old active rules and independent Cobalt authorization.

The whitepaper's protocol boundary is stronger than the current Dynamic UNL deployment. It should be treated as a target to prove, not as evidence that its independence assumptions already hold.

## Decentralization scorecard

Scores are from 0 (none/unacceptable) to 5 (strong). “Authority decentralization” concerns who can actually change validator authority, not how many copies of a model execute.

| Architecture | Audit/replay | Authority decentralization | Fail-safe liveness | Decision |
|---|---:|---:|---:|---|
| Current PFT Dynamic UNL Phase 1/2 | 4 | 1 | 3 | Keep as research/shadow; do not claim decentralized governance |
| Current L1 Cobalt with Foundation-administered proposers/operators | 4 | 2 | 4 | Keep active; decentralize operators and proposal origination |
| AI shadow advisor under Cobalt | 4 | 2 | 5 | Approve |
| Deterministic admission rules + open proposals + independent Cobalt | 5 | 4 | 5 | Preferred near-term architecture |
| Bounded AI proposals + deterministic selector + independent Cobalt | 5 | 4, conditional | 5 | Target architecture after gates pass |
| AI votes or direct AI registry authority | 2 | 0 | 1 | Reject |

AI does not by itself raise the authority score. It may raise transparency and proposal throughput. The decentralization gain comes from independent observers, open proposal rights, independent keys, deterministic safety constraints, and removal of unilateral publication authority.

## Threat model and required controls

| Threat | Current exposure | Required control |
|---|---|---|
| Curated or omitted evidence | One Foundation-operated VHS is the source of the agreement metric | Publish signed raw votes; federate independent observers; report gaps and disagreements |
| Biased prompt, model or option set | Foundation governs the execution question and profile | Versioned public governance; delayed activation; old-rule ratification; open challenges |
| Publisher-key capture | A trusted publisher key makes the authoritative `postfiatd` list | Remove publisher discretion from content authority; require protocol/Cobalt-certified content |
| Admin override capture | Custom and rollback publication can replace the selected list | Eliminate unilateral override at authority transfer; use bounded old-rule emergency governance |
| Sidecar/operator correlation | Recorded testnet convergence used five Foundation-operated validators | Require separately administered, funded, hosted and keyed participants |
| Shared GPU/runtime dependency | Modal/H100-class execution is a practical common dependency | Make inference optional for liveness; support admitted profiles; measure cost and accessibility |
| Validator-key exposure | Participation path signs with a mounted validator key file | Separate scoped replay/governance key; isolated signer; no key in inference container |
| Unsafe positive model result | Held-out replay produced an unsafe `PROCEED` | Model may hold/review/reject evidence; exact code alone authorizes positive mutation |
| Vendored verifier capture | Sidecar runs Foundation-selected parser and selector | Content hashes plus independent implementation, conformance fixtures and reproducible builds |
| Cosmetic validator diversity | Node count can hide shared funding, keys or release management | Evidence and hard caps for operator, funding, key, release and infrastructure linkedness |
| Proposal censorship | Foundation currently initiates and publishes the canonical path | Open authenticated proposal lane with deterministic admission and expiry |
| Replay divergence or outage | A strict runtime profile can split or disappear | Quarantine/hold; never slash from model divergence alone; retain last valid registry |
| Model supply-chain change | Weights, tokenizer, image or kernels may drift | Bind every artifact in a replay profile; promotion requires an old/new shadow campaign |

## The eight decisions that must be made

These are the actual governance decisions. “Use AI or not” is too coarse.

1. **Authority boundary**  
   **Recommendation:** Cobalt remains the only validator-registry authority. AI never votes or mutates state.

2. **Evidence authority**  
   **Recommendation:** require signed raw evidence and at least three independently administered observation domains before any authority-bearing use.

3. **Proposal rights**  
   **Recommendation:** allow any authenticated party to submit a schema-valid, bonded or rate-limited proposal. The Foundation must not be the sole originator.

4. **Decision split**  
   **Recommendation:** deterministic code handles every exact condition. AI may classify only enumerated ambiguous fields and must cite packet fields.

5. **Replay rule**  
   **Recommendation:** replay convergence is an admissibility check for the classification, not a governance vote. Divergence means hold/no-op.

6. **Compute and accessibility**  
   **Recommendation:** do not make an H100, Modal account, or one runtime provider a validator requirement. Publish cost/latency budgets and admit more than one independently reproducible profile where possible.

7. **Keys and publication**  
   **Recommendation:** use scoped governance/replay keys and isolated signing. No inference container gets a validator master key. No web publisher key can choose registry content.

8. **Promotion and rollback**  
   **Recommendation:** promote only through an old-rule Cobalt transition after shadow evidence and an adversarial review. Removing the model must leave the last valid registry and ordinary finality intact.

## Gates before authority-bearing AI proposals

A separate recorded promotion decision should require all of these:

- [ ] Signed raw observations are publicly reconstructable for the complete scoring window.
- [ ] At least three observers satisfy an explicit independence test covering administration, credentials, infrastructure and publication.
- [ ] At least three replay participants are independently operated; Foundation-controlled validators count as one trust domain.
- [ ] An open proposal lane has demonstrated that a non-Foundation operator can submit a valid competing packet.
- [ ] The deterministic baseline and the model are evaluated on a pre-registered, held-out set of genuinely ambiguous cases.
- [ ] The model shows material lift over the deterministic baseline without an unsafe positive-admission error.
- [ ] Model deletion and inference outage produce hold/no-op while the chain remains live.
- [ ] Registry concentration, churn, linkedness and old/new quorum intersection are enforced in deterministic code.
- [ ] Sidecars use scoped keys and isolated signers; inference processes cannot read validator master keys.
- [ ] Replay cost, latency and hardware access are published for independent operators.
- [ ] Censorship, omission, equivocation, stale packet, split replay, compromised publisher, and rollback drills pass.
- [ ] Independent operator control—not merely separate hosts—is evidenced for the Cobalt quorum.

Until every item passes, output remains `SHADOW_ONLY`.

## Should we rent large GPU machines now?

**No, not for this decision.**

The existing H200/H100 and held-out replay work already answers the narrow compute question well enough: pinned-profile execution can converge, and the model is not safe enough to emit automatic positive governance decisions.

The unresolved questions are institutional and architectural:

- who chooses and observes the evidence;
- who can originate a proposal;
- who holds the authority keys;
- who controls publication;
- whether independent operators participate;
- whether deterministic rules perform as well as the model; and
- whether the system survives model deletion.

Renting more of the same GPU cannot answer those questions.

A future compute campaign is justified only after the governance controls above exist. Its purpose should be narrowly pre-registered:

1. compare the model against a deterministic-only baseline on ambiguous held-out cases;
2. test cross-provider and materially different hardware/runtime accessibility;
3. measure per-round cost and time for an independent operator; and
4. test graceful hold behavior under disagreement and outage.

That campaign requires a separate budget approval. No rental is authorized by this review.

## Recommended implementation sequence

### Stage 0 — preserve the current authority boundary

- Keep Cobalt active.
- Keep all AI/Dynamic UNL integration `SHADOW_ONLY`.
- Do not expose L1 registry mutation to the current PFT scoring result.
- Treat the current L1 observer document as a research spec, not a deployed claim.

### Stage 1 — eliminate avoidable centralization without AI

- Publish signed raw vote/participation evidence.
- Add independent observers and a deterministic merge/gap-reporting rule.
- Open proposal origination to non-Foundation operators.
- Separate validator, replay, relay and publisher keys.
- Enforce admission floors, concentration caps, linkedness and churn in deterministic code.

This stage creates more real decentralization than another model benchmark.

### Stage 2 — qualify AI as a removable classifier

- Define closed questions for only the fields that remain genuinely qualitative.
- Require field citations and reject unknown or uncited claims.
- Compare against deterministic and human-review baselines.
- Permit the classifier to emit `HOLD`, `REVIEW`, or evidence classifications—not direct `ADMIT` or `PROCEED`.
- Publish replay, disagreement, cost and accessibility receipts.

### Stage 3 — bounded proposals under Cobalt

- Let the deterministic selector convert an admissible classification into a bounded candidate delta.
- Allow independent proposers to submit the same packet.
- Validate it under the previous active registry, graph, checker and safety profile.
- Require Cobalt ratification and normal consensus ordering.
- Preserve model-free rollback and last-valid-registry operation.

## Final recommendation

AI governance is valid for PostFiat only if the phrase means **public, replayable decision support and bounded proposal generation beneath Cobalt**.

It is not valid if it means a model decides who validates, casts a vote, owns a governance key, controls a publisher, or becomes a liveness dependency.

The current `postfiatd` system demonstrates that replayable scoring and sidecar commit/reveal are feasible. It does not yet demonstrate usefully decentralized authority. The main blockers are not model size or GPU supply. They are centralized evidence observation, proposal origination, publisher control, administrator overrides, key custody, and correlated operators.

Therefore:

> **Keep Cobalt. Keep AI in shadow. First decentralize evidence, proposals, keys, and operators. Then consider promoting a removable AI classifier that can only create bounded proposals which deterministic code and independent Cobalt validators may reject.**
