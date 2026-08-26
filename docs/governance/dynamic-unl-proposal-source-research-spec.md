# Dynamic UNL Proposal Source Research Specification

**Status:** Text Improvement Harness full gate passed on 2026-08-26 — average 89.13/100 (GPT 92.00, Fable 86.00, GLM 89.40; five runs per lane; run group `dynamic-unl-proposal-source-research-spec`); scored content SHA-256 `62938fea81f10cc0dbda531593666e9b469eebb80d02c948db644c90e58647c8`; Task Node lock pending operator decision
**Date:** 2026-08-26
**Decision owner:** Post Fiat
**Author:** Domagoj Ravlić (`dravlic`)
**Prior work:** [Dynamic UNL roadmap](https://github.com/postfiatorg/dynamic-unl-scoring/blob/f18e6b4c40d3df17afb46804ef4f2bb0d879b439/docs/CurrentRoadmap.md), [commit-reveal protocol](https://github.com/postfiatorg/dynamic-unl-scoring/blob/f18e6b4c40d3df17afb46804ef4f2bb0d879b439/docs/phase2/CommitRevealProtocol.md), [convergence reporting](https://github.com/postfiatorg/dynamic-unl-scoring/blob/f18e6b4c40d3df17afb46804ef4f2bb0d879b439/docs/phase2/ConvergenceReporting.md), [validator sidecar overview](https://github.com/postfiatorg/validator-scoring-sidecar/blob/e6907faa5bb554199fcd2102a5ec1e7fbec8db90/docs/Overview.md), [Deterministic Governance Overview](deterministic-governance-overview.md), [Deterministic Governance Agent Plan](deterministic-governance-agent-plan.md), [Cobalt Independent-Operator Proposal Path Research Specification](cobalt-independent-operator-proposal-path-research-spec.md), [Validator Registry](validator-registry.md), [Cobalt adversarial-verification results](cobalt-adversarial-verification-results.md)
**Decision scope:** whether a sealed, threshold-qualified Dynamic UNL result on the PFT Ledger should become the deterministic content source for Cobalt-ratified validator-registry proposals on this Rust L1, and the exact cross-chain, identity, churn, evidence, and authority boundaries required before that source may affect the controlled devnet

## Plain-English directive

PostFiat already has two working but separate governance components. On the PFT
Ledger, the Dynamic UNL pipeline freezes an immutable input package, pins the
model and runtime contract, scores validators deterministically, applies a
versioned final-score formula and mechanical selector, and lets validator
sidecars independently re-execute the round and commit-reveal their result
hashes. The Foundation convergence service then seals the report, pins it, and
anchors its CID on the PFT Ledger. Phase 2 is live; for example, testnet round 13
sealed a 5/5-valid report. Phase 3A authority transfer is not live.

On this Rust L1, Cobalt is live only as the ratification authority for
validator-registry and trust-graph changes. Consensus v2 still orders and
finalizes every block. E6 established that every current proposal originates
inside the Foundation administration boundary.

This research decides whether the Dynamic UNL pipeline should supply the
deterministic content of a Cobalt proposal: the evaluation layer decides who
deserves trust, a canonical adapter turns that sealed result into a bounded
registry delta, and Cobalt ratifies or rejects that exact delta under the current
registry and trust graph. A CID, model output, selector result, proposal
signature, or convergence percentage alone is never authority.

The proposal-content question and proposal-submission question are independent.
The [independent-operator proposal-path specification](cobalt-independent-operator-proposal-path-research-spec.md)
decides **who may submit** canonical proposal bytes and how a proposer is chosen.
This specification decides **what those bytes may contain**, how they are derived
from public artifacts, and how every validator verifies them. Passing either
specification cannot substitute for passing the other, and this work does not
itself decentralize proposal submission.

Dynamic UNL is the concrete, already-verified evaluator needed by the DGA
plan's step in which “the policy proposes bounded validator-registry actions.”
It is not a replacement for DGA's constitutional layer. DGA still owns the
governed objectives and constraints, accepted evidence fields and source
versions, allowed action types, freshness and conflict rules, hard concentration
and churn caps, linkedness requirements, no-op and emergency behavior, upgrade
and revocation rules, and the boundary between validator policy and all other
governance scopes. Dynamic UNL model governance continues to select and pin the
scoring model; DGA decides whether that governed evaluator lineage is admissible
for an L1 action and applies the L1 constitutional bounds.

The two pipelines remain on different chains. No L1 consensus path may fetch
IPFS, call a PFT Ledger RPC, run a model, or trust an operator summary. Validators
must resolve and verify public source artifacts before Cobalt voting, then vote
on identical canonical proposal bytes. Missing, unavailable, ambiguous,
unmapped, stale, or inconsistent evidence produces a named hold or rejection
with no registry mutation.

This specification grants no mainnet authority, does not change which chain
either pipeline runs on today, and does not authorize a live change. It permits
research design and a later controlled-devnet qualification only. The Task Node
lock remains pending the operator's decision.

## Claims, evidence, and gaps

| Claim | Evidence today | Gap | Closes it |
| --- | --- | --- | --- |
| Dynamic UNL execution is deterministic and replayable | A frozen input package binds CID and hash; the execution manifest pins the Qwen/Qwen3.6-27B-FP8 revision, SGLang image digest, H100 class, request, prompt, formula, selector, and canonicalization; sidecars re-execute before reveal | No canonical adapter maps these artifacts to an L1 registry proposal or binds them to current L1 state | E1 |
| Validator agreement is publicly auditable | Sidecars sign commits and reveals with validator master keys; reports retain accepted transaction identities, output-match levels, participation outcomes, a sealed report CID, and a PFT Ledger anchor | The report is assembled and anchored by the Foundation service; the L1 has no defined source-chain proof, artifact-availability, or cross-chain finality rule | E1 and E2 |
| Phase 3A defines an authority threshold | The roadmap requires at least 10 participating validators and output convergence greater than 95% for four consecutive rounds | The current shipped report has an observed-committer population, no L1 admission rule, and no canonical four-round qualification certificate or denominator definition | E2 |
| Mechanical selection controls list construction | Final scores use a versioned integer formula; selection uses a manifest-pinned cutoff, maximum size, minimum displacement gap, previous-list semantics, and deterministic score/key ordering | Those rules were designed for a PFT Ledger UNL, not the L1's current registry, identity model, Cobalt transition budget, or quorum arithmetic | E1 and E3 |
| Cobalt safely ratifies validator-trust changes | The controlled devnet has Cobalt-scoped registry authority, old-rule authorization, fail-closed admission, replay protection, and a 10,240-case adversarial trust-graph corpus | Cobalt currently accepts no Dynamic UNL source certificate and has no proof that selector-driven churn stays inside every generated graph's safe transition budget | E2 and E3 |
| A sealed result says which L1 validator record to add, retain, or remove | Dynamic UNL artifacts identify PFT Ledger validator master keys | No governed, versioned binding maps those identities to L1 validator IDs, hot keys, operator manifests, and admission evidence; post-hoc or partial mapping would add discretion | E1 |
| A controlled-devnet integration can be reversed | Every live Cobalt change is ordered and replayable; the standing operator rule requires a disposable-clone rehearsal before live transitions | No real Dynamic UNL result has been adapted, clone-ratified, applied live, or reversed on the L1 | E4 |
| Agreement scores can be traced to protocol evidence | Every PFT Ledger validation vote is signed at the protocol source | Dynamic UNL Evidence milestone E.1 is not started: the vote sets behind agreement scores are not yet persisted, published, and linked to scoring rounds | E5 |

## Decision question

> Can a standalone verifier take a sealed, four-round-qualified Dynamic UNL result and its frozen public lineage, reproduce one canonical and safely bounded L1 registry-update proposal against the current registry and trust roots, and can every correct Cobalt validator reject stale, forged, non-converged, unmapped, or quorum-unsafe variants while ratifying the byte-identical valid proposal without changing either chain's current protocol role?

The answer is yes only if the adapter contains no discretionary edit, source
artifacts have an explicit verifiable PFT Ledger trust boundary, every affected
identity is pre-bound, the proposed delta fits Cobalt's graph-specific transition
budget, signed-vote lineage is available, and clone-first migration and rollback
evidence passes. A useful shadow report is not sufficient for live authority.

## Scope

### In scope

- Frozen Dynamic UNL input packages, execution manifests, score-formula and
  selector versions, final output hashes, commit-reveal transactions, sealed
  convergence reports, and their PFT Ledger anchors.
- A versioned cross-chain source certificate and standalone verifier with online
  public-artifact and offline checksum-bound packet modes.
- A governed identity-binding registry from a PFT Ledger validator master key to
  an L1 validator ID, active hot key, operator-manifest hash, admission-evidence
  root, validity interval, and authorizing L1 registry root.
- A deterministic adapter that derives an add, retain, remove, rotate, staged
  transition, or no-op candidate from a selected list without human rewriting.
- Cobalt proposal admission, current-state binding, replay/freshness checks,
  graph-specific churn limits, quorum arithmetic, ratification, ordering,
  recovery, and separately authorized forward rollback.
- The E1 10,240-case adversarial trust-graph corpus, generated selected-list
  transitions, and the six-validator controlled devnet.
- The interface between Dynamic UNL evaluation, DGA constitutional policy,
  independent proposal origination, Cobalt ratification, and Consensus v2
  ordering.

### Out of scope

- Moving Dynamic UNL execution, PFT Ledger commit-reveal, or model governance
  onto this Rust L1.
- Moving Cobalt or Consensus v2 onto the PFT Ledger.
- Letting L1 consensus fetch network artifacts, run Qwen/SGLang, interpret prose,
  or accept a mutable URL as evidence.
- Changing the Dynamic UNL scoring formula, model-selection process, selector
  semantics, or PFT Ledger consensus.
- Replacing DGA's constitution, evidence-source governance, hard protocol
  guards, emergency controls, or governance of non-validator scopes.
- Replacing or satisfying the independent-operator proposal-path milestone.
- Mainnet authority, public-testnet authority, operator recruitment, or a claim
  of decentralized proposal submission.
- A live controlled-devnet mutation before separate operational authorization
  and the exact disposable-clone rehearsal pass.

## Responsibility split

| Layer | Owns | Cannot do |
| --- | --- | --- |
| Dynamic UNL evaluation and its model governance | Freeze evidence; pin and govern the scoring model/runtime/formula/selector; compute and independently reproduce final scores and selected list; seal convergence evidence | Submit or ratify an L1 change merely by publishing a result |
| DGA constitutional policy | Approve source and schema versions; define admissible evidence, mappings, action types, freshness, conflicts, concentration/churn/linkedness caps, no-op, upgrade, revocation, and emergency rules | Re-score validators ad hoc, rewrite a selected list, or bypass Cobalt |
| Proposal path | Authenticate an eligible proposer, transport exact bytes, and select a proposer/view deterministically | Change adapter output or count proposal signatures as ratification |
| Cobalt | Validate current-state and source preconditions; ratify one canonical bounded validator-trust proposal under current trust views | Decide qualitative merit, finalize blocks, or authorize unrelated governance |
| Consensus v2 | Order and finalize the already-authorized governance batch | Treat Dynamic UNL or Cobalt as a second block-finality protocol |

## Shared invariants

1. **Canonical source lineage:** every source certificate binds the PFT Ledger
   network and genesis, round number, announcement transaction and ledger
   position, input package CID and canonical hash, execution-manifest hash,
   formula and selector identities and parameters, final-bundle hash, selected
   list and hash, report bytes and hash, convergence bundle CID, anchor
   transaction and ledger position, and every artifact needed for recomputation.
2. **Current L1 binding:** every proposal binds the L1 chain ID and genesis,
   protocol version, current authority-transition ID and history sequence,
   current registry root and trust-graph root, proposal slot, activation and
   expiry heights, source round, source certificate hash, target selected-list
   hash, exact registry delta, resulting registry and trust roots, and evidence
   packet root.
3. **No network in consensus:** artifact fetch and PFT Ledger verification finish
   before Cobalt voting. Correct validators sign only the canonical bytes emitted
   by the standalone verifier. L1 execution verifies hashes, bounds, signatures,
   the Cobalt decision certificate, and old-registry authorizations without
   external I/O.
4. **Pre-bound identity:** an affected PFT Ledger master key maps through exactly
   one current, governance-authorized identity binding. Missing, expired,
   conflicting, post-report, wrong-root, or many-to-one bindings hold. The
   adapter never invents an L1 key or operator record.
5. **Threshold qualification:** an authoritative candidate references four
   consecutive sealed normal rounds. Each has at least 10 distinct eligible
   validators with accepted in-window reveals and a convergence rate greater
   than 95.00% under one frozen integer-basis-point formula and roster rule.
   Duplicate, ineligible, missing-reveal, and divergent outcomes cannot inflate
   either threshold.
6. **Deterministic content:** identical source artifacts, identity bindings, DGA
   policy version, and L1 state produce byte-identical proposal bytes. Ordering
   uses explicit canonical keys; no map iteration, wall clock, RPC ordering, or
   operator choice affects the result.
7. **Safe churn:** a selected target never bypasses Cobalt linkedness, strong
   support, old-rule authorization, quorum, or displacement limits. If the full
   delta exceeds the graph-specific budget, a versioned deterministic staging
   rule emits only the highest-priority safe prefix or a no-op and retains the
   full target hash. It never silently weakens or reorders the Dynamic UNL
   ranking.
8. **Evidence completeness:** a proposal is live-eligible only when its agreement
   scores link to published signed validation-vote sets and the independent
   verifier reproduces the scores. Until Dynamic UNL Evidence milestone E.1
   lands, status is `VOTE_LINEAGE_PENDING` and the integration remains shadow
   only. E.1 proves correctness over published votes, not observation
   completeness; that residual limitation remains disclosed until E.2.
9. **Failure atomicity:** every rejection, hold, unavailable artifact, failed
   ratification, crash, stale proposal, and rollback attempt leaves both the
   active L1 registry and finalized history unchanged.

## Experiment 1 — canonical proposal adapter and standalone verifier

Define `DynamicUnlSourceCertificateV1`,
`DynamicUnlValidatorBindingV1`, and
`DynamicUnlRegistryProposalV1` with closed schemas, size bounds, domain
separation, canonical encoding, and versioned hash rules.

The adapter consumes a sealed convergence report plus its frozen input package,
execution manifest, final bundle, commit-reveal records, source-chain
announcement and anchor evidence, four-round qualification history, governed
identity bindings, the active DGA policy version, and an authenticated L1 state
snapshot. It recomputes file hashes, formula output, selector output, selected
list, convergence arithmetic, identity projection, registry delta, resulting
roots, and proposal bytes. It binds the proposal to the current registry root,
trust-graph root, round number, input package CID and hash, manifest hash,
selected-list hash, and convergence-report anchor, plus every shared invariant
above.

Specify the exact PFT Ledger trust root. The verifier must authenticate the
announcement, commit/reveal, and report-anchor transactions against a pinned
network/genesis and a stated validated-ledger checkpoint/finality rule. If the
available PFT Ledger data cannot provide a self-contained inclusion and finality
proof, the packet must name the remaining RPC/publisher trust assumption and the
proposal may not be described as trustless. IPFS supplies content; it is never
the authority anchor.

The standalone Python CLI supports an offline packet and an online resolution
mode, but both produce the same canonical bytes. Fetch order, gateway, RPC
endpoint, JSON field order, and local time must not change output. Freeze valid
vectors and one-field mutations for every bound field, identity ambiguity, file
absence, non-canonical encoding, oversized artifact, and unavailable source.

### Required result

- Two independent implementations reproduce byte-identical source certificates,
  identity projections, deltas, proposal bytes, and hashes for every valid
  vector without importing each other's canonicalization or adapter code.
- The verifier recomputes the selected list from the frozen input, manifest,
  formula, and selector and exactly matches the sealed result and final bundle.
- Every affected identity was governed and valid before the source round; no
  post-hoc, partial, or operator-edited mapping enters a proposal.
- Online and offline verification emit identical bytes from at least two
  independent PFT Ledger RPCs and two artifact gateways, or the difference
  fails with a named source-trust error.
- Every one-field mutation, wrong chain, wrong root, stale snapshot, missing
  artifact, ambiguous identity, and oversized input rejects or holds before
  Cobalt voting with zero state mutation.

## Experiment 2 — Phase 3A ratification preconditions

Add a Dynamic UNL source precondition to the Cobalt proposal-admission path. It
accepts only normal rounds and verifies the source certificate, current L1
bindings, and a four-report qualification certificate. Freeze the threshold
arithmetic in integer basis points: participation is the count of distinct,
eligible, signature-valid validators with an accepted in-window reveal;
convergence is the number whose acceptance-level hashes match divided by that
frozen eligible participant denominator. Each of four consecutive round numbers
must have participation at least 10 and convergence strictly greater than 9,500
basis points.

Admission must reject, with distinct reason codes: unsealed report; missing or
invalid report anchor; fewer than four qualifying rounds; stale source round;
duplicate or skipped round; non-converged report; participation below 10;
wrong-round artifact; wrong input CID or hash; manifest, formula, selector, or
selected-list mismatch; report or anchor hash mismatch; source-chain replay;
wrong L1 chain, registry, trust, authority-history, slot, activation, or expiry
binding; unavailable artifacts; and an otherwise valid proposal whose bytes do
not equal standalone recomputation.

Network resolution remains outside L1 consensus. Cobalt votes carry the source
certificate hash and proposal hash. Restart and catch-up retain enough canonical
evidence to reproduce admission without querying a mutable endpoint.

### Required result

- Every correct validator admits the same valid proposal bytes and reason-codes
  every invalid vector identically.
- Four consecutive sealed rounds at participation 10 and convergence above 95%
  pass; participation 9, convergence exactly 95%, any broken streak, and every
  named negative case reject or hold without durable registry mutation.
- A report cannot qualify itself through duplicated identities, copied reveals,
  missing reveals, ineligible validators, diagnostic-only selected-list
  agreement, or Foundation summary fields not reproduced from raw records.
- Restart, replay, and honest catch-up reproduce the same admission decision and
  proposal hash without external I/O during consensus.

## Experiment 3 — selector churn and Cobalt quorum alignment

Reuse the frozen E1 corpus of 10,240 trust graphs spanning 6–20 validators and
every strict linkage boundary. For each graph, generate current registries,
previous selected lists, score vectors, cutoff crossings, incumbent failures,
ties, maximum-size pressure, and displacement-gap challenges. Run the exact
manifest-pinned Dynamic UNL selector, project through only pre-bound identities,
and calculate the direct add/remove/rotate delta.

For every transition, derive the maximum safe per-round displacement from the
old registry's Cobalt essential subsets, local trust views, tolerated Byzantine
count, blocking sets, certificate quorum, and post-transition linkedness. Test
the direct target and the deterministic staging/no-op rule. Include mass cutoff
failure, maximum-size reduction, identity-binding expiry, one incumbent refusing
to ratify removal, and candidate sets larger than the L1 registry limit.

The selector's displacement gap and maximum size are evidence inputs, not a
substitute for Cobalt arithmetic. Integration parameters pass only if every
proposal they emit stays within the derived transition budget. If a selected
target requires several rounds, staging order is deterministic from the original
ranking and current state; each later step requires a fresh qualifying report
and old-rule ratification.

### Required result

- Across all 10,240 graphs and generated transitions, zero admitted proposal
  exceeds the graph-specific displacement budget, violates either Cobalt
  inequality, loses required linkedness, lets the new set authorize itself, or
  makes one accepted root fork from another.
- The manifest maximum-size and displacement-gap settings plus the versioned
  adapter rule produce one byte-identical direct, staged, or no-op result in two
  independent implementations.
- Every transition that cannot fit safely is held or split deterministically;
  no operator truncates, reorders, substitutes, or hand-edits the selected list.
- Five-of-six progress and four-of-six halt remain true for the active
  controlled-devnet topology before, during, and after each admissible step.
- The unchanged E1 corpus hashes and production classifications still verify.

## Experiment 4 — end-to-end controlled-devnet shadow and live drill

Select one real Dynamic UNL normal round only after it seals and its complete
public artifacts are available. Freeze its source certificate, four-round
qualification evidence, identity bindings, DGA policy version, and current L1
snapshot before examining the proposed delta. The adapter must produce a
non-noop bounded proposal without a human editing the selected list or mapping.

First run the complete path in shadow mode: all six L1 validators independently
resolve the public artifacts, reproduce identical proposal bytes, and record
whether they would vote, while the registry remains unchanged. Then rehearse the
exact ratification, ordering, restart, catch-up, and rollback sequence on
disposable six-validator clones bound to the current chain, registry, authority
history, trust state, validator identities, deployed binary, and authenticated
tip.

Only after the clone packet, E5 signed-vote lineage, and a separate live
operational authorization pass, submit those frozen bytes through the current
proposal path, ratify them
with Cobalt, order them with Consensus v2, verify the resulting registry and
trust roots, and execute the separately authorized forward rollback. The
rollback restores policy state through a new finalized transition; it never
deletes or rewrites history. Rehearse wrong-root, stale-report, report-swap,
mapping-swap, and rollback-replay cases on the clone before sending the exact
negative cases live.

### Required result

- All six shadow validators independently produce one byte-identical proposal
  and source certificate from the public artifacts.
- The exact live sequence and its rollback pass first on disposable clones,
  including restart, catch-up, post-change finality, and proof that the
  post-rollback roots and history are the expected forward state.
- The valid live proposal commits once with the expected proposer, Cobalt
  signers, old-registry authorizers, Consensus v2 receipt, registry root,
  trust-graph root, and source lineage; every named negative case rejects live
  without durable mutation.
- Consensus v2 finalizes throughout, and Cobalt gains no authority outside
  validator registry and trust graph changes.
- The evidence states whether the submitter was Foundation-administered. This
  experiment does not claim proposal-submission decentralization.

## Experiment 5 — signed validation-vote lineage

Implement or consume Dynamic UNL roadmap Evidence milestone E.1. Persist the
signed PFT Ledger validation votes used by each agreement window, publish
canonical sorted and deduplicated per-window vote sets with content hashes, and
link the exact 1-hour, 24-hour, and 30-day inputs to the frozen scoring round.
The standalone verifier checks every available signature and recomputes the
agreement scores used by the model input.

Bind the vote-set roots, window boundaries, known-validator inventory, ledger
count, recomputation result, and E.1 status into the source certificate and
proposal evidence root. Test omission, duplication, bad signatures, wrong
windows, inventory substitution, ledger-count substitution, and score/report
mismatch.

The current status is `VOTE_LINEAGE_PENDING`: the Dynamic UNL roadmap marks
E.1 not started and the present VHS path discards signed messages after reducing
them to aggregates. Until E.1 is implemented and a proposal links to those
votes, E1–E4 may run in shadow or on disposable clones, but no Dynamic
UNL-sourced live registry change passes this specification.

### Required result

- Every agreement score used by the source round is linked to canonical
  published vote sets and reproduced exactly from signature-valid votes.
- Every named mutation fails with a reason, and omitted or incomplete lineage
  cannot be represented as complete.
- The source certificate states the residual observation-completeness assumption:
  E.1 proves the score follows from published votes; only the planned independent
  observer federation in E.2 reduces single-observer omission risk.
- If signed-vote publication remains pending, the packet says
  `VOTE_LINEAGE_PENDING`, the overall decision remains `SHADOW_ONLY`, and no
  live-authority claim is published.

## Gates

### ADOPT FOR CONTROLLED DEVNET

Adoption passes only when all of the following are true:

- E1: two independent verifiers emit byte-identical, current-state-bound proposal
  bytes from authenticated public source artifacts and pre-existing identity
  bindings.
- E2: the four-round rule passes with at least 10 participants and greater than
  95% convergence in every round; every negative precondition rejects
  identically.
- E3: every emitted transition fits its graph-specific Cobalt budget across the
  unchanged 10,240-case corpus; unsafe targets stage or hold deterministically.
- E4: the all-six shadow run, exact disposable-clone rehearsal, authorized live
  ratification, restart, catch-up, negative cases, and forward rollback pass.
- E5: signed vote sets reproduce every agreement score and the proposal binds
  their roots.
- The CLI, read-only browser, checksum-bound packet, independent verifier,
  strict docs build, redaction scan, and required publication pass.

### SHADOW ONLY

The integration remains `SHADOW_ONLY` if any authority prerequisite is absent,
including E.1 vote lineage, a self-contained and disclosed source-chain trust
rule, four consecutive Phase 3A-qualified reports, a complete pre-existing
identity map, safe churn, clone rehearsal, or separate live authorization.
Shadow output may be published as research evidence but cannot be submitted as
an authority-bearing proposal.

### REJECT OR REMEDIATE

A conflicting canonical result, source-proof failure, accepted stale or
non-converged proposal, identity ambiguity, unsafe churn, wrong-root acceptance,
Cobalt safety/liveness regression, Consensus v2 regression, or unverifiable
rollback is a failed gate. Preserve the corpus and failed receipt, repair the
owning adapter, verifier, identity, DGA policy, Cobalt, or operations boundary,
and rerun the unchanged affected experiment. Do not weaken thresholds or
reinterpret an artifact after seeing its result.

## Required evidence packet

- `dynamic-unl-proposal-source-status.json` with `ADOPT_CONTROLLED_DEVNET`,
  `SHADOW_ONLY`, or `REMEDIATION_REQUIRED`, bound to both chain identities,
  current roots, source and L1 revisions, and every gate.
- Exact source pins for `dynamic-unl-scoring`,
  `validator-scoring-sidecar`, this L1, the model revision, SGLang image,
  prompt, score formula, selector, adapter, DGA policy, and identity-binding
  schema.
- Frozen input and final bundle manifests, all file hashes, report bytes and
  CID, announcement and anchor transactions, validated-ledger proof or disclosed
  source trust receipt, four-round qualification arithmetic, and artifact
  availability receipts.
- Canonical schemas, valid and negative vectors, source certificate, identity
  map, standalone-verifier outputs, proposal bytes, resulting roots, and
  cross-implementation equality report.
- E3 corpus manifest and unchanged hashes, per-graph safe-churn derivations,
  generated transitions, staging decisions, Cobalt classifications, and quorum
  results.
- Shadow, clone, and authorized live receipts with proposal, signer, authorizer,
  Consensus v2, registry/trust-root, restart, catch-up, negative-case, and
  forward-rollback evidence.
- Signed validation-vote sets, score-recomputation report, residual observer
  limitation, or an explicit `VOTE_LINEAGE_PENDING` receipt that forces
  `SHADOW_ONLY`.
- CLI and browser outputs, `SHA256SUMS.txt`, redaction report, and a verifier
  that fails on missing, mutated, non-canonical, stale, mismapped, unsafe,
  incomparable, or internally inconsistent evidence.

## Human interfaces

Per the developer mandate, the implementation milestone delivers the CLI before
the browser interface:

1. A Python application such as
   `python -m postfiat_rpc.dynamic_unl_proposal verify <packet>` that a human
   can open and run to resolve or verify public artifacts, reproduce threshold
   arithmetic and proposal bytes, explain every identity and churn decision,
   compare all six validators, and report `ADOPT_CONTROLLED_DEVNET`,
   `SHADOW_ONLY`, or the exact failure reason.
2. A read-only browser view backed by the same verified packet that shows source
   round and four-round streak, manifest and selected-list lineage, vote
   lineage, identity bindings, proposed delta, Cobalt transition budget,
   proposer versus ratifiers, shadow/clone/live state, rollback, and remaining
   trust assumptions. It exposes no proposal, ratification, registry mutation,
   or rollback route.

Both interfaces fail closed on checksum, schema, chain, source proof, artifact,
threshold, identity, churn, proposal, signature, root, replay, or status
inconsistency.

## Required publication

- Add a first-page architecture diagram and text that keep the chains and roles
  separate: Dynamic UNL evaluation and commit-reveal remain on the PFT Ledger;
  Cobalt ratification and the validator registry remain on the Rust L1;
  Consensus v2 remains block finality.
- Say “Dynamic UNL supplies deterministic proposal content; Cobalt ratifies”
  only after the adoption gate. Before then, say “researching” or “shadow.”
- Keep these statements together: the evaluation layer decides who deserves
  trust; DGA owns the constitutional policy and hard L1 bounds; the
  independent-operator path decides who may submit; Cobalt ratifies; Consensus
  v2 orders and finalizes.
- Publish the exact four-round participation/convergence arithmetic, source-chain
  trust assumption, identity-map governance, selector and staging rules, churn
  derivation, E.1 vote-lineage state, live proposer/ratifier identities, and
  remaining operator concentration.
- State plainly that the work does not move either pipeline to the other chain,
  grant mainnet authority, or by itself decentralize proposal submission.
- Update the DGA overview and plan to reference Dynamic UNL as the concrete
  evaluator for validator merit without deleting DGA's constitutional,
  model/evidence governance, or non-validator scope.
- Publish failures and `SHADOW_ONLY` status with the same prominence as a pass.

## Decisions recorded

1. Dynamic UNL is evaluated as the deterministic validator-merit and
   proposal-content source; Cobalt remains the ratifier and Consensus v2 remains
   block finality.
2. The independent-operator specification remains mandatory and separate: it
   decides who may submit, while this specification decides valid content and
   verification.
3. DGA is not replaced. It retains constitutional objectives and constraints,
   source and schema admission, allowed actions, hard safety bounds, upgrade and
   revocation, emergency behavior, and governance outside validator evaluation.
4. The adapter may stage or hold an unsafe selected target but may never rewrite
   its ranking or conceal the full selected-list hash.
5. No network access enters L1 consensus. Public PFT Ledger and IPFS evidence is
   resolved and authenticated before Cobalt voting, with every remaining trust
   assumption explicit.
6. Signed validation-vote lineage is pending Dynamic UNL Evidence milestone E.1
   and blocks live adoption until it is verifiable.
7. This score-only pipeline makes no Task Node request. The operator decides
   whether to request the lock and any later implementation or live action.

## Work sequence

1. Run the Text Improvement Harness full scoring gate with five runs in each
   selected lane. Rewrite through a direct OpenRouter
   `openai/gpt-5.6-sol-pro` call only while the average is below 86/100, keep
   E1–E5 and their gates intact, and stop at the first compliant score.
2. Record lane scores, average, run group, and scored document SHA-256 in the
   Status line. Leave the Task Node lock pending operator decision.
3. If the operator approves, request the research-specification lock through
   Task Node, inspect the generated task, and complete its lifecycle.
4. After the lock, request conversion into a concise milestone document through
   Task Node.
5. Execute E1–E5 as substantial Task Node work; deliver the Python CLI before
   the read-only browser, publish the checksum-bound packet, and retire the
   milestone only after every authorized interface and publication gate passes.
