# Cobalt Adversarial Verification Research Specification

**Status:** Locked — Text Improvement Harness average 89.27/100 (GPT 89.20, Fable 88.40, GLM 90.20; run group `round-20260825T124034Z`); scored content SHA-256 `b95308f9a320d962ea2a933353290bdb49105e5ae2fa33eb78ef327476ec88bc`; date 2026-08-25; Task Node `task_158622307482e23fb4519889b53b475f`
**Date:** 2026-08-25
**Decision owner:** Post Fiat
**Author:** Domagoj Ravlić (`dravlic`)
**Prior work:** [Cobalt Activation Research Specification](cobalt-activate-or-retire-research-spec.md), [completed Cobalt Activation Milestone](../plans/completed/cobalt-activate-or-retire-milestone.md), [Cobalt: Further Evaluation](https://postfiat.org/blog/cobalt-further-evaluation/), [controlled-testnet activation packet](https://github.com/postfiatorg/postfiatl1v2/tree/main/benchmarks/cobalt-activation-live/packet)
**Decision scope:** whether the Cobalt authority activated on the controlled testnet at height 916 holds under adversarial validators, adversarial trust views, adversarial history, and adversarial load, and whether the public claims match the evidence

## Plain-English directive

Cobalt is live. Since height 916 it is the authority for validator-registry and trust-graph changes on the controlled testnet, and the first Cobalt-authorized key rotation committed at height 917. Consensus v2 still orders and finalizes blocks.

The evidence that justified activation is cooperative evidence. Faults were scheduled by the harness, not chosen by an adversary. The oracle and the 18-case corpus were written by the same team that wrote the implementation. All six validators are administered by one operator. The blog says the evidence supports "a reversible controlled-testnet activation with explicit stop conditions" and that "obviously" the result must be tested more thoroughly and adversarially.

This specification is that test. The required outcome is **KEEP ACTIVE**: Cobalt stays live if it survives an adversary that chooses the worst schedule, lies about its trust view, replays and forges history, and pushes the certificate and RPC limits. A failed gate creates P0 implementation work and a rerun of the unchanged adversarial corpus. Live rollback to Foundation authority is triggered only by the stop conditions the blog already published, never by a benchmark result alone.

The specification also closes the gap between what is published and what is proven. Cobalt ratifies validator-trust changes; it does not decide which validators deserve trust, and today it does not decentralize who proposes a change. The public text must say so.

## Claims, evidence, and gaps

Each row is one published claim, the evidence behind it, the gap, and the experiment that closes it.

| Claim (blog, README, milestone) | Evidence today | Gap | Closes it |
|---|---|---|---|
| Determinism and safety: zero conflicting roots, byte-identical replay | 18 frozen cases; oracle written by the implementing team; three 20-validator overlap cases | Small corpus; no independent oracle; no random or adversarially chosen trust graphs | E1 |
| Liveness: five of six progress, four of six halt | Six isolated simulated domains; 14 governance rounds; scheduled delay, loss, reorder, duplicate, crash, partition, one equivocation | Faults are scheduled, not adversarial; one Byzantine strategy only; no lying trust views; no withholding at each protocol stage | E2 |
| Recovery: a lagging validator restores the exact history | Proof-carrying catch-up from honest peers | No tampered durable state; no forged or truncated catch-up from a Byzantine peer | E3 |
| Finality isolation: p95 finality +2.63%, inside 5% | 50 baseline rounds and 50 integration rounds | Fifty rounds; no governance storm; no certificate near the 1 MiB cap; no RPC frame flood | E4 |
| Reversibility: a separately authorized transition restores Foundation authority | Forward rollback rehearsed on a disposable clone | Never executed live; authorized by the same six Foundation-run validators | E5 |
| Independent operation | The locked activation specification required at least three independently controlled operator groups (Experiment 4 and the ACTIVATE gate). The milestone cancelled that task (`task_46d1707cb9e11f04648ea54a7163fbee`) and re-scoped independence to simulation. The specification states that no milestone may redefine the decision gates without a newly locked research specification. | Activation proceeded with a gate the locked specification did not allow to be redefined. This is a process gap as much as a technical one. | E6 and the decision in **Open decisions** |
| "Cobalt controls validator-trust governance" | True in code: `controls_block_consensus: false`, `writes_validator_registry: true` in `activation-status.json` | The proposal for every live change came from the Foundation operator. Cobalt gates changes; it does not yet decentralize who asks for them. "Validator governance" in headlines reads as the whole governance. | E6 and **Required publication** |
| "Cobalt authority remains off today" (blog, line 154 of `content/blog/cobalt-further-evaluation.md`, `lastmod 2026-08-24`) | Activation landed 2026-08-25 at height 916 | The published article contradicts the live state | **Required publication** |

## Decision question

> With Cobalt already live on the controlled testnet, does the implementation preserve one accepted registry history, keep Consensus v2 finality within budget, and reject every forged, replayed, or self-authorized change when up to the tolerated number of validators behave arbitrarily, the network schedule is chosen by an adversary, and durable history is tampered with, and do the published claims describe exactly that and nothing more?

When the answer is yes, Cobalt stays active and the publication is corrected. Until then, fix the implementation and rerun the unchanged adversarial corpus.

## Scope

### In scope

- The live six-validator controlled testnet and its isolated-validator simulation harness.
- Byzantine validators up to the fault bound the trust-graph rules tolerate for the active configuration.
- Adversarial message schedules, trust-view claims, durable-state tampering, and resource pressure.
- The live forward rollback to Foundation authority and the return to Cobalt authority.
- Who may propose a registry change and who may authorize it.
- Correction of the public article and the first-page claims.

### Out of scope

- Breaking ML-DSA or the hash functions.
- More Byzantine validators than the formal model tolerates; safety is not claimed there.
- Attacks on Consensus v2 itself, except where a Cobalt event is the cause.
- Mainnet activation.
- Recruiting real independent operators. E6 designs the path and records the decision; it does not perform recruitment.

## Threat model

The adversary controls at most `f` validators, where `f` is the largest number the active six-validator trust graph tolerates under the local Cobalt rows (`t_S < 2q_S - n_S`, `2t_S < q_S`). The concrete `f` must be derived from the live trust graph and frozen before execution. The adversary may:

1. **Behave arbitrarily as a validator:** equivocate at any protocol stage (RBC, ABBA, MVBA, DABC), withhold votes selectively, vote late, re-propose, and sign conflicting contributions.
2. **Lie about trust:** declare a trust view that is compatible on paper but is not what the validator acts on, change the declared view during a decision, and declare views designed to sit exactly on a linkage boundary.
3. **Choose the schedule:** delay, drop, duplicate, and reorder any message, and time partitions to the moments a decision or activation height is reached.
4. **Forge history:** serve fabricated, truncated, or padded catch-up material to a recovering validator, and restart a validator from tampered durable state.
5. **Use a stolen key:** hold one validator's ML-DSA key and attempt registry changes, self-admission of a new validator set, and rotation to a key the adversary controls.
6. **Exhaust resources:** send certificates and contributions near the 1 MiB certificate cap and the 2 MiB RPC frame, flood the sidecar RPC, and trigger repeated halts while Consensus v2 is under load.
7. **Abuse authority transitions:** submit early, stale, replayed, wrong-root, cross-chain, mixed-authority, and self-authorized transitions, including rollback transitions.

The adversary cannot break signatures, cannot exceed `f`, and cannot control Consensus v2 proposers.

## Shared property under test

For every adversarial case, each correct validator must report:

1. **Agreement:** no two correct validators accept different registry roots for the same transition.
2. **Validity:** a correct validator accepts only a proposal authorized under its formal trust conditions by the currently active registry.
3. **Liveness:** when the correct validators form a compatible configuration with strong support and the synchrony assumptions hold, every correct validator decides.
4. **Safe halt:** when they do not, no correct validator decides and the current registry stays authoritative.
5. **Recovery:** a restarted or lagging validator rejects forged history and restores the exact accepted sequence without operator-written state repair.
6. **Isolation:** no Cobalt event stops, forks, or rewrites Consensus v2 finality.

Every case reports per-validator votes, decisions, roots, rejection reasons, timing, and resource use. Aggregate counts alone do not pass.

## Experiment 1 — independent oracle and generated corpus

Build a second oracle from the formal essential-subset, strong-support, and linkage rules, written by a different author than the production code and the first oracle, with no import of production Cobalt code. Then:

- Generate at least 10,000 random trust graphs for 6 to 20 validators, including graphs constructed to sit on each linkage inequality boundary.
- Classify every graph with both oracles and with production `analyze_trust_graph`, `has_strong_support`, and non-uniform certificate validation.
- Freeze every disagreement as a named case before any fix.

### Required result

- The two oracles agree on every generated graph, or every disagreement is traced to a stated rule and resolved by a reviewed correction to exactly one oracle.
- Production Cobalt matches the reconciled oracle on every graph.
- Every mismatch that survives is an implementation defect fixed with a regression case, and the unchanged corpus is rerun from clean state.

## Experiment 2 — Byzantine validator campaign

Extend the isolated-validator harness with adversary strategies driven by the threat model, executed by up to `f` domains at once:

- equivocation at each protocol stage separately, then combined;
- selective withholding aimed at the smallest quorum-minimal signer set;
- declared-versus-acted trust-view divergence, including a view change mid-decision;
- two simultaneous proposals from one Byzantine proposer plus one honest proposal;
- late votes arriving after a decision, and re-proposal of a decided update;
- adversarial schedules generated by a search over delay, drop, and partition timings that maximizes disagreement or delay, not a fixed schedule list.

### Required result

- Zero conflicting accepted roots across the campaign.
- Every compatible configuration of correct validators decides within the stated synchrony bound.
- Every incompatible configuration halts without registry mutation.
- The Byzantine validators are identified in the evidence by signed misbehavior, not by operator assertion.

## Experiment 3 — adversarial recovery

For each of the six validators in turn, on disposable clones bound to the live registry root:

- restart from tampered durable state: truncated, padded, reordered, and one-entry-modified histories;
- serve forged catch-up material from a Byzantine peer: fabricated transitions, valid-looking certificates over the wrong root, and a history that omits the latest update;
- interrupt catch-up mid-transfer and resume from a different peer.

### Required result

- Every tampered state is detected before the validator rejoins.
- Every forged catch-up is rejected with a named reason.
- The validator restores the byte-identical accepted history from honest peers with no manual repair.

## Experiment 4 — finality isolation under governance stress

Run at least 500 Consensus v2 rounds in a baseline lane and 500 in an attack lane from the same signed state, on the same fleet, binary, and CPU quota. In the attack lane run concurrently:

- a governance storm: repeated proposals, halts, and view changes;
- certificates and contributions padded to just below the 1 MiB cap and requests to just below the 2 MiB frame;
- a sidecar RPC flood from one adversarial validator;
- one validator crash-looping.

### Required result

- Consensus v2 never stops and never forks.
- p95 client-visible finality in the attack lane is within 5% of the baseline lane.
- Every oversized certificate, frame, and flood is rejected at the documented limit, and the rejections are counted in the evidence.

## Experiment 5 — live authority drills

On the live controlled testnet, at scheduled future heights:

1. execute the signed forward rollback to Foundation authority, prove Foundation authority is live, then execute the return to Cobalt authority;
2. run every negative transition case live: early, stale, replayed, wrong-root, cross-chain, mixed-authority, self-authorized, and a replayed rollback;
3. run a key-compromise drill: treat one validator key as stolen, attempt a Cobalt-authorized rotation from the stolen key, then rotate it out through the legitimate path;
4. record, for each drill, which identities proposed and which authorized the change.

### Required result

- Both live authority transitions commit at the scheduled heights with one accepted history, and Consensus v2 finalizes throughout.
- Every negative case rejects without mutation, live, not only on a clone.
- The stolen-key rotation is rejected and the legitimate rotation commits.
- The evidence shows that today every proposal and every authorization comes from Foundation-administered validators.

## Experiment 6 — proposal source and independence

This experiment produces a design and a decision, not a live change.

- Document the current proposal path: which process constructs a registry update, which keys sign it, and which validators authorize it.
- Design the path by which a validator that is not Foundation-administered proposes a registry change and how the trust graph must be configured so that no single administrator can reach the quorum or block it alone. Reuse the operator-admission boundary and the onboarding packet retained from the activation milestone.
- State whether the locked activation specification's independent-operator gate is reinstated as its own milestone or formally deferred. Either outcome must be recorded in a newly locked research specification, because the activation specification forbids redefining that gate inside a milestone.

### Required result

- A reviewed design document with the proposal path, key custody boundaries, and quorum arithmetic.
- An explicit, dated decision on the independent-operator gate.

## Gates

### KEEP ACTIVE

Cobalt stays live when all of the following hold:

- E1: production matches the reconciled independent oracle on every generated graph.
- E2: zero conflicting roots, zero false halts, zero false accepts under the Byzantine campaign.
- E3: every tampered state and forged catch-up is rejected; honest recovery is byte-identical.
- E4: Consensus v2 never stops or forks; p95 finality within 5% under attack.
- E5: both live authority transitions commit; every live negative case rejects; the stolen-key drill rejects.
- E6: the design and the independent-operator decision are recorded and locked.
- The publication corrections below are live.

### ROLL BACK

Roll back to Foundation authority through the rehearsed signed transition only on the stop conditions the blog already published, observed live: a conflicting accepted root, failed five-of-six progress under an honest majority, divergent catch-up history, unexpected block authority, or a sustained Consensus v2 finality regression. A failed benchmark or simulation result does not trigger rollback; it creates P0 remediation.

### REMEDIATION

Any failed gate creates P0 implementation work: diagnose the owning code boundary, fix, add regression coverage, and rerun the unchanged adversarial corpus from clean state. The corpus and oracles stay frozen unless an independently demonstrated oracle defect requires a separately reviewed correction.

## Performance and operations

The packet must report, for the baseline and attack lanes: governance decision latency, recovery time under forged catch-up, CPU, memory, network, and disk per validator, Consensus v2 p50 and p95 finality, rejected certificates and frames by reason, sidecar restarts, and operator actions required. No recovery may require manual mutation of durable history.

## Required evidence packet

- `adversarial-status.json` with `KEEP_ACTIVE`, `REMEDIATION_REQUIRED`, or `ROLLED_BACK`, bound to the live authority state.
- The frozen threat model, derived `f`, adversary strategy manifest, and both oracles with source pins.
- Generated-graph corpus manifest and per-graph classifications.
- Per-validator results for every E2 and E3 case, with signed misbehavior evidence.
- Baseline and attack-lane finality receipts and resource metrics.
- Live transition, negative-case, and key-compromise receipts with secrets excluded.
- The E6 design document and decision record.
- CLI and UI output, `SHA256SUMS.txt`, and a verifier that fails on missing, mutated, or inconsistent evidence.

## Human interfaces

Per the developer mandate, the milestone delivers:

1. A `python -m postfiat_rpc.cobalt adversarial` command that verifies the packet, reports the gate state, and lists every rejected adversarial case with its reason.
2. A read-only browser panel beside the existing Cobalt observatory that shows the adversarial gate state, the last live authority transitions, and the proposal and authorization identities for each live change.

## Required publication

- Correct the article: Cobalt authority has been active since height 916; remove "Cobalt authority remains off today".
- Use "validator-registry ratification" or "validator-trust governance" in the title and summary, never bare "validator governance".
- State on the first page that Cobalt ratifies registry changes, that a separate layer decides which validators deserve trust, and that today every proposal originates from Foundation-administered validators.
- Carry the milestone's sentence that the result proves protocol capability, not operator decentralization, into the summary, not only the body.
- Publish the adversarial results with the same first-page discipline: what was attacked, what held, what was fixed, and what remains open.

## Decisions recorded

1. Adversarial verification is the next research specification; the activation handoff names it as the open work and no active plan exists.
2. The independent-operator gate from the locked activation specification is decided inside E6 and locked with this specification, as reinstated milestone or formal deferral.
3. The article's "Cobalt authority remains off today" statement is corrected now, ahead of the adversarial results, because it is a factual error about the live state.
4. This work runs on the shared `0xPostFiatChad` Task Node link; the operator name in handoffs and commit messages identifies the human.

## Work sequence

1. Score this specification with the Text Improvement Harness; rewrite only while the average is below 86/100.
2. Lock it through a Task Node task.
3. Convert it into a milestone document through a Task Node task.
4. Run E1 through E6 as substantial Task Node tasks, roughly one per experiment, with the CLI and UI work governed inside the same tasks.
5. Publish the packet and the corrected article; retire the milestone document to completed plans.
