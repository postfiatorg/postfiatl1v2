# PostFiat: An Authority-Validated Settlement Ledger

### Governed trust evolution, shielded settlement, replayable machine classification, and post-quantum authorization

**Whitepaper, Version 3 — June 2026**

**Canonical protocol-document candidate.** This is the only repository document
intended to describe the protocol. It remains a controlled pre-testnet
conformance draft: present-tense statements are limited by the explicit current
implementation boundaries in this document and by `SECURITY.md`.

---

## Abstract

PostFiat is a Layer 1 settlement-ledger design in the XRP category — known
validators, deterministic certificate finality, fixed native supply, fee burn,
and no native validator rewards — with a target architecture for signed Cobalt
trust evolution, Asset-Orchard privacy, replayable governance evidence, and
post-quantum account/validator authorization. The repository implements some of
that target and deliberately disables other parts where the authorization or
recovery story does not yet close.

> **Current implementation boundary (July 2026):** Section 6 describes the
> target signed Cobalt-governance protocol, not an enabled live transition
> path. The repository's legacy amendment and validator-update evidence names
> supporters but does not carry registry-verifiable signatures. The production
> candidate therefore rejects those artifacts at live proposal admission and
> direct apply, while retaining explicit historical replay and offline analysis.
> The active registry is fixed at genesis until the signed authorization
> protocol is implemented and its adversarial gates pass.

> **Privacy boundary:** Asset-Orchard is the supported private settlement path.
> The legacy cleartext note decoder is historical replay only, and transparent
> transfers remain supported; this candidate does not claim all routine state is
> private by default.

This paper states the protocol argument: the threat model and assumptions, the ledger and consensus design, the admission economics that justify zero issuance, the Cobalt transition machinery and its safety argument, the shielded settlement path and its honest leakage profile, the bounded role of machine classification, the cryptographic cost accounting, and the named recovery surface for every failure mode. Empirical measurements appear only as hash-bound artifacts in an evidence register; every empirical sentence in the body names the artifact that backs it, and every artifact backs only its own sentence.

---

## 1. Introduction

XRP demonstrated that a financial settlement ledger can operate without proof of work, proof of stake, validator rewards, or inflationary block subsidies. Known validators, deterministic finality in seconds, fixed supply, and fee burn form a coherent economic model, not a historical accident: the parties who depend on settlement finality operate the machines that produce it, and resource use is priced by destroying the native unit rather than by paying rent to a block-producer class.

PostFiat keeps that category. The paper makes one claim about the category and four claims about its weakest surfaces.

The category claim is that proof of authority is the correct settlement primitive when validators are natural stakeholders rather than subsidy-seeking block producers (§5). The four surface claims are: validator-list evolution should be protocol state under Cobalt-checked transitions rather than social coordination around privately published lists (§6); routine financial state should be shielded by default, with public supply accounting and holder-controlled disclosure (§7); machine judgment is admissible in governance only as a typed, public, replayable artifact that the protocol is free to ignore (§8); and authorization should be post-quantum from genesis, because settlement keys and settlement value outlive cryptographic eras (§9).

The claim is not that the underlying risks disappear. The claim is that they become typed, priced, replayable, and governable instead of being hidden in inflation schedules, opaque list publication, transparent portfolio state, or migrate-later cryptography.

### 1.1 Design principles

Five principles recur throughout the protocol and are worth naming once, because every later section is an application of them.

*Fail closed.* Missing, stale, conflicting, or oversized evidence produces a hold, a no-op, or continued operation under the last valid rules. No surface of the protocol defaults to permit.

*Old rules validate new rules.* No object — registry, trust graph, checker, safety profile, model profile, or selector — participates in validating its own activation. Every transition is judged by the rules that were active before it was proposed. Changing the rules that judge transitions is itself a transition under the previous rules; changing them any other way is a fork, not governance.

*Least machinery.* An exact predicate in code is preferred to a model; a deterministic selector is preferred to a committee; a precommitted recovery action is preferred to discretion. Heavier machinery must earn its place by handling cases the lighter machinery demonstrably cannot.

*Hash-bound artifacts.* Every governance input and output is committed by root before it can take effect. Deviation from the accepted process is detectable by hash mismatch rather than contestable by narrative.

*Deletion monotonicity.* Removing the machine-classification layer (§8) can only make outcomes more conservative — more holds, never more admissions. The chain must remain safe, merely slower at adjudicating ambiguity, with the model deleted.

### 1.2 Scope

This paper is the protocol argument. Engineering measurements, benchmark hashes, fixture inventories, and machine reports are evidence for specific sentences, not part of the argument itself; they are collected in the Evidence Register (Appendix A) and referenced inline as [E1]–[E8]. Numbers quoted from the register are budget measurements on named hardware and software profiles, never consensus constants.

---

## 2. Threat Model and Assumptions

PostFiat is designed against five adversary classes.

| Adversary | Capability | Protocol response |
|---|---|---|
| Byzantine validator minority | Equivocation, invalid certificates, stale votes, liveness disruption below the fault threshold. | BFT certificates, canonical state transitions, replay rejection, bounded registry updates, fail-closed recovery (§4, §10). |
| Correlated validator group | Shared operator, hosting provider, jurisdiction, funding source, or release path masquerading as independence. | Evidence packets, concentration caps, trust-graph constraints, Cobalt-linked registry changes (§5, §6). |
| Founder or foundation override | Publishing a validator, prompt, model, or selector change outside the accepted process. | Hash-bound packet roots, selector roots, registry-delta roots, and Cobalt gates make any deviation detectable (§6, §8). |
| Model or runtime operator | Prompt injection, hidden evidence substitution, model drift, nonreproducible inference. | Closed option sets, source hashes, pinned replay profiles, replay certificates, governed replacement (§8). |
| Public observer | Balance analysis, flow correlation, timing analysis, fee fingerprinting, strategy inference. | Shielded notes remove routine state from public view; disclosure is explicit, scoped, and holder-controlled (§7). |

The formal assumptions behind these responses are explicit. Within an epoch with $n$ active validators, at most $f=\lfloor(n-1)/3\rfloor$ are Byzantine. Across a registry transition, the actively Byzantine set is bounded by the transition profile's budget $B$. Safety is asynchronous: no commit, registry transition, or shielded state update depends on timing, only on certificate arithmetic. Consensus v2 safety therefore does not use a clock; its liveness after a failed proposer assumes partial synchrony so a quorum of timeout and phase votes can eventually be delivered. Networks whose genesis omits the v2 activation height retain the legacy safety-first single-view mode and can halt after a failed proposer. Challenge-window progress separately assumes partial synchrony.

The cryptographic assumptions include EUF-CMA security of ML-DSA signatures;
collision resistance of the domain-separated SHA3 constructions; RedPallas
spend-authorization security; ChaCha20-Poly1305 note confidentiality and
integrity; and the soundness and zero-knowledge properties of the Halo2 proving
system over its curve cycle, which carries no per-circuit trusted setup. The NAV
proof profile separately depends on its pinned SP1/Groth16 verifier and program
key. ML-DSA is post-quantum; RedPallas, Halo2, Groth16, Orchard note encryption,
and Ethereum-compatible paths are classical assumptions. Sections 7, 9, and 11
treat that hybrid boundary explicitly rather than presenting the entire system
as post-quantum. A break maps to a named containment or recovery requirement
(§10), not a silent security claim.

---

## 3. Ledger Model

The implemented replicated state is broader than four components. It contains a
transparent account map; issued assets and trustlines; escrows, NFTs, offers and
matching state; NAV assets, reserve packets, redemption/challenge and proof
profiles; vault-bridge and currently disabled external-handoff history;
FastLane/FastSwap policy, reserve, object and checkpoint state; an Asset-Orchard
pool with commitment tree and nullifier set; generic bridge state; ordered
batch/receipt history; and the active validator/governance state. The canonical
state commitment tags and length-delimits these domains. Caches and local
operator artifacts are excluded and must be reproducibly rebuildable.

The native unit has a fixed supply set at genesis. There is no issuance of any kind. All transaction fees are burned, so supply is monotonically non-increasing. Fee classes price bytes, signature verifications, shielded actions, and registry operations as distinct resources rather than through a single gas scalar, and fee burn is the only protocol-level economic flow: no validator rewards, no foundation tax, no fee redistribution.

Live transactions include transparent payments, issued-asset operations,
escrows, NFTs, offers, NAV and vault-bridge operations, dual-authorized atomic
swaps, FastLane primary operations, and Asset-Orchard actions. Signed Cobalt
registry/parameter transitions and receipt-aggregate ordering accountability are
target protocols, not enabled live classes in this candidate. Asset-Orchard notes
carry asset and value inside commitments so supported issued assets can settle
under per-asset public turnstile accounting.

Every block is verified under exactly one active registry root and commits one
ordered transition across all replicated domains. The current registry is fixed
at genesis; any future signed transition must preserve the one-root-per-height
property described in §6 and §7.

New networks activate the complete replicated-state-v2 commitment at genesis,
including every FastLane reserve, receipt, authorization, policy, committee,
fence, checkpoint, and activation field. A legacy chain can cross that encoding
boundary only at an irreversible, future height committed before the validator
upgrade; an uncoordinated restart or a permissive legacy-root fallback is not a
valid migration.

---

## 4. Consensus and Ordering

### 4.1 Certified ordering

The candidate implements two explicitly versioned ordering modes. If genesis
omits `consensus_v2_activation_height`, the legacy single-view direct-certificate
rule remains in force and every nonzero view fails closed. A network configured
with an activation height uses that legacy rule below the height and consensus
v2 at and above it. Existing networks cannot silently rewrite genesis to enable
v2; until signed protocol governance exists, that migration requires a new
genesis/reset with the old chain retained as independently replayable history.

In consensus v2, for $n$ active validators the fault bound is
$f=\lfloor(n-1)/3\rfloor$ and the quorum is
$q=\lfloor 2n/3\rfloor+1$. The deterministic proposer is selected from the
canonical committee by height and view. Every proposal, vote, QC and timeout
certificate binds chain ID, genesis hash, protocol version, committee epoch and
root, height, view, parent block ID, payload hash, resulting state root,
validator identity and phase.

The commit rule is explicit two-phase BFT, not chained HotStuff. A quorum of
`prepare` votes forms a prepare QC and establishes each honest validator's
durable lock. A quorum of `precommit` votes for that exact non-nil block forms a
precommit QC; only that QC commits. A prepare QC or the legacy certificate alone
cannot commit after activation. Safety state—highest prepare, precommit and
timeout rounds, locked/high QC and last signed digests—is atomically persisted
before a signature leaves the validator and is preserved in snapshot v6.

If a view fails, a quorum of signed timeout votes forms a timeout certificate.
A proposal for view $v+1$ must carry the certificate for view $v$ and its highest
typed prepare-QC reference. Every reference resolves to a full verified QC;
opaque or lexicographically ranked QC identifiers have no v2 authority. A
locked block may be reproposed in the later view, while a conflicting block
cannot use the lock's QC as ancestry. The final block record embeds the proposal,
timeout ancestry, prepare QC and precommit QC, making replay independent of an
in-memory cache. Exact `n=4` and `n=6` transport regressions cover a failed
view-zero proposer and view-one recovery; adversarial models cover loss,
duplication, reorder, partition, Byzantine equivocation and restart. Equivocation
evidence remains available for admission and removal (§5, §6).

### 4.2 Deterministic inclusion order

The production mempool groups transactions by transaction family and drains
those families in a fixed implementation-defined sequence. Within each family,
currently admitted entries retain insertion order. This is deterministic for a
given replicated mempool state, but it is not a fee-class/admission-bucket/hash
auction and does not remove proposer or ingress ordering power. Operators and
wallets must therefore treat transaction ordering and omission as observable
but not yet protocol-attributable risks. The stronger canonical-order model in
`crates/ordering_fast` is a reference model, not the production inclusion path.

### 4.3 Censorship accountability

The production node does not currently issue threshold admission-receipt
aggregates or consume omission evidence in consensus or governance. Related
types and fixtures in `crates/ordering_fast` are research/reference machinery.
The current public evidence can show that a transaction was included or
omitted from an observed block and mempool snapshot; it cannot prove when a
quorum first observed that transaction or automatically attribute censorship
to a proposer. No stronger accountability claim is made for this release.

### 4.4 Availability suspension

The live protocol has no automatic Negative-UNL-style missed-round suspension,
expiry, or reduced-quorum state machine. If a validator is unavailable, the
current committee either still forms the normal quorum or the chain halts.
The current candidate cannot change validator status live: unsigned legacy
registry transitions fail closed, and the active registry remains fixed at
genesis. A future signed transition under §6 is not an automatic liveness
shortcut and must not retroactively alter certificate thresholds.

---

## 5. Authority Validation: Economics and Admission

### 5.1 A different security purchase, not a compromise

Proof of authority is usually described as a compromise. For financial settlement it is better understood as a different security purchase. Proof of work buys Sybil resistance with energy and hardware. Proof of stake buys it with bonded capital and a reward schedule. Authority validation buys it with known operators, reputational and legal exposure, operational history, and organic economic reliance on the ledger working correctly.

That purchase is not weaker by definition. It is weaker when the validator set is opaque, captured, too small, or chosen for convenience — which is exactly the surface §6 exists to govern. And when validators are natural stakeholders, manufacturing a paid validator class can make incentives strictly worse: a reward schedule recruits participants whose only stake is the reward, and over time the rewarded class becomes a constituency that politically entrenches the reward.

### 5.2 Zero issuance as a conditional claim

Zero issuance is therefore justified conditionally, not ideologically. When natural exposure and accountability are publicly visible enough to pass an admission predicate, native validator rewards are not needed to recruit validators, and fee burn prices resource use without creating a validator-rent market:

$$\text{fixed supply} + \text{fee burn} + \text{natural validators}.$$

The condition creates the engineering burden: "natural stakeholder" must be verifiable about a specific candidate from source-bound public evidence, not asserted by a foundation. The economic premise is operationalized as an admission predicate, not as private dollar modeling.

### 5.3 The admission predicate

A candidate must provide source-bound evidence across five fields, each of which the selector maps into ordinal bins. Missing or conflicting required evidence holds the candidate rather than defaulting in either direction.

| Field | Evidence mapping |
|---|---|
| $exposure_i$ | Source-bound reliance on finality: exchange, custody, gateway, public volume class, or on-chain usage. |
| $accountability_i$ | Signed operator manifest, domain control, jurisdiction and contact surface, incident history, revocation path. |
| $reliability_i$ | Uptime, version freshness, monitoring, history retention, infrastructure redundancy. |
| $attack_i$ | Governance power, transaction-flow access, conflicting incentives, value at risk during the influence window. |
| $\rho_i$ | Correlation: shared operator, ASN, cloud, country, funding source, release manager, monitoring endpoint, or affiliate evidence. |

The selector evaluates evidence fields rather than a single model-owned number:

$$
admit_i = \mathbf{1}[exposure_i \ge x_{min}] \cdot \mathbf{1}[reliability_i \ge r_{min}] \cdot \mathbf{1}[accountability_i \ge a_{min}] \cdot \mathbf{1}[attack_i \le b_{max}] \cdot \mathbf{1}[\rho_i \le \rho_{max}] \cdot \mathbf{1}[linkedness(G_t, i) = safe].
$$

Weights, floors, and caps are governance parameters, but they are public parameters bound to evidence fields. Launch defaults require source-bound reliance; $r_{min} = 0.995$ over the active observation window; signed operator identity plus domain control; no direct conflict above $b_{max}$; and $\rho_{max} = 0$ for a shared release manager, key-management vendor, or funding controller with an existing validator, unless a later Cobalt packet raises the cap for a named exception.

Two properties of the predicate deserve emphasis. First, the economic screen does not override the correlation veto: a high-exposure candidate with shared release management, key management, or funding control still fails, because natural stake does not launder cosmetic diversity. Second, the predicate is executable, not aspirational. The controlled-testnet selector consumes a `ValidatorAdmissionEvidencePacket` and emits a `ValidatorAdmissionDecision`: missing or conflicting required fields hold, below-floor reliability or accountability and above-cap correlation reject, and only a clean pass emits an *add* registry-delta candidate. Fixture coverage spans clean admit, shared-control reject, missing-domain hold, contradictory-evidence hold, and unknown-model-field hold [E4]. The $linkedness$ term is defined in §6.4; admission and registry governance share one trust-graph vocabulary by construction.

### 5.4 Target public launch certificate

The genesis validator set is the unavoidable bootstrap act. A future public
launch is intended to freeze that act in the following signed certificate rather
than pretending it is already decentralization. The current controlled pre-testnet
has genesis bundles and operator manifests but does not implement this exact
`LaunchCertificate` type or enforce the seven-ratifier public-launch minimum:

```
LaunchCertificate = {
  genesisRegistryRoot,
  trustGraphRoot,
  checkerRoot,
  safetyProfileRoot,
  evidenceRoot,
  ratifierSet,
  signatures
}
```

No public launch may be claimed until this artifact and its verification path are
implemented. The target profile requires at least seven ratifiers, no single
control group above one third, and no silent shared release manager,
key-management vendor, or funding controller across a quorum. Even then, the
certificate would not make genesis trustless; it would make the bootstrap trust
assumption explicit, auditable, and frozen.

---

## 6. Target Cobalt-Governed Registry Evolution

This section is a protocol specification, not a description of an enabled live
mutation path. The current fail-closed boundary is stated in the Abstract.

### 6.1 From recommended lists to protocol state

XRP's strongest design idea is also its governance weakness: each server trusts a Unique Node List, and safety depends on sufficient overlap between local trust lists. This avoids proof-of-work and proof-of-stake markets, but it leaves validator-list evolution as social coordination around privately published recommendations. The live question — *will this list change preserve safety?* — is answered by reputation rather than by a checkable object.

PostFiat makes validator-list evolution protocol state. Genesis commits to one manifest root covering the initial registry $G_0$, trust graph $T_0$, checker $\chi_0$, safety profile $\pi_0$, witness schema $\omega_0$, chain id, and launch ratification certificate. After finalization, genesis ratifiers have no override opcode. Changing the genesis manifest changes chain identity; it is a fork, not governance.

### 6.2 Transition packets

A transition from $G_t$ to $G_{t+1}$ must bind the parent registry root, parent trust-graph root, delta root, next registry root, evidence root, active safety profile, checker root, challenge state, activation epoch, expiry, and governance certificate. The previous active rules validate the transition; a new checker, policy, or trust graph cannot validate itself. The accompanying safety witness has a fixed schema:

```
SafetyWitness = {
  local_threshold_rows,
  essential_subset_cover,
  old_new_intersection_bounds,
  linkedness_paths,
  rejected_counterexamples
}
```

### 6.3 Local safety rows

For each essential subset $S$ with $n_S = |S|$, Byzantine budget $t_S$, and quorum $q_S$, the local Cobalt inequalities are

$$0 \le t_S, q_S \le n_S, \qquad t_S < 2 q_S - n_S, \qquad 2 t_S < q_S.$$

The first bound is well-formedness; the second guarantees that any two quorums of $S$ intersect in at least one correct validator under the subset's own fault budget; the third keeps the budget small enough that a quorum cannot be majority-Byzantine.

### 6.4 Linkedness

The linkedness predicate is a graph check, not a trust score. A trust view $V_i$ declares essential subsets $ES_i$. Two views $V_i, V_j$ are *linked* when they share an essential subset $S \in ES_i \cap ES_j$ whose active-fault count is at most $t_S$, and *fully linked* when the same subset also leaves at least $q_S$ correct validators:

$$faults(S) \le t_S, \qquad |S| - faults(S) \ge q_S.$$

For a validator $i$, $linkedness(G_t, i) = safe$ means: follow derived-UNL edges through the rooted trust graph and require every pair of views in that closure to be fully linked. The linkage report is recomputable from the rooted graph alone:

```
LinkageReport = {
  trustGraphRoot,
  activelyByzantineSet,
  linkedPairs,
  fullyLinkedPairs,
  unsafePairs,
  weaklyConnectedValidators,
  stronglyConnectedValidators
}
```

An unsafe pair fails the transition. A proposer cannot declare linkedness by supplying a favorable summary; the checker recomputes it. Essential subsets are themselves governed objects: an edit replaces one validator's trust view or subset list, records old and new view ids and graph roots, and is validated by the previous active graph before the new graph can matter.

### 6.5 Cross-registry quorum intersection

Local rows are necessary but not sufficient. Let $Q_{old}$ be the covered old governance quorums, $Q_{new}$ the covered proposed quorums, and $B$ the active Byzantine budget for the transition profile. The old–new check is

$$\forall q_o \in Q_{old},\ \forall q_n \in Q_{new}: \quad |q_o \cap q_n| > B.$$

A counterexample shows why the check exists. Take an old registry $\{A,B,C,D,E,F,G\}$ and a proposed registry $\{A,B,H,I,J,K,L\}$, each declared as a single essential subset with $n_S = 7$, $q_S = 5$, $t_S = 2$. Both local rows pass. But the old quorum $\{A,B,C,D,E\}$ and the new quorum $\{A,B,H,I,J\}$ intersect only in $\{A,B\}$, and under a budget $B = 2$ that entire intersection can be Byzantine: the old quorum could certify one transition while the new quorum certifies a conflicting one, with no correct validator forced to sign both. The local rows pass, the old–new matrix fails, and the old registry remains active. This is exactly the failure class that recommended-list publication cannot even express, because there is no protocol object on which to check it.

### 6.6 Bounded cover extraction

The intersection matrix is only as good as the set of quorums it covers, so cover construction is taken away from the proposer. The active profile fixes $M_{cover}$, the maximum number of old plus new essential subsets that may participate in one transition. If the cover exceeds the bound, the old registry remains active — fail closed, not best effort. Within the bound, the cover extractor takes only the old and new rooted trust graphs as input, walks every trust view, deduplicates every distinct active essential subset by subset id, rejects inactive or conflicting rows, and emits a hash-bound `CobaltCoverExtractionReport`. The safety witness is accepted only if its old and new cover exactly matches the derived report; a proposer cannot omit an unfavorable subset from the matrix.

Let $m_o = |cover_{old}|$, $m_n = |cover_{new}|$, $M = m_o + m_n$, and $V = |G_t \cup G_{t+1}|$. Extraction plus matrix evaluation costs

$$O(M \log M) + O(m_o\, m_n\, V \log V) \le O(M_{cover}^2\, V \log V).$$

There is no proposer-supplied branch pruning. A pathological trust graph either fits inside $M_{cover}$ and is fully enumerated, or exceeds it and fails closed. A sizing run over grouped institutional-style trust views fits comfortably inside the current $M_{cover} = 64$ profile: a 35-validator graph with five seven-validator groups produced $M = 12$, and a 100-validator graph with ten ten-validator groups produced $M = 22$ [E3]. The point is not that these are the only valid shapes; it is that ordinary global-plus-group subset declarations do not make the extractor unusable, while deeply nested or adversarial covers still fail closed.

### 6.7 Transition safety

**Proposition (transition safety).** Fix an active graph $G_t$, proposed graph $G_{t+1}$, and transition profile $\pi$. Assume: graph roots, registry roots, signatures, and parent links verify; the extracted cover is complete and within $M_{cover}$; every active essential subset in the cover satisfies the local Cobalt inequalities; the linkedness closure is safe for every signing view; every covered old quorum and new quorum intersect in more than $B$ validators; at most $B$ validators in the transition window are actively Byzantine; and correct validators sign at most one value for a given height, view, registry root, and transition root. Then an accepted transition preserves agreement and transition validity across the registry boundary.

*Proof sketch.* Cover completeness means the checker enumerates every quorum row that can authorize the old or the new side of the transition under the rooted graphs. Local rows plus linkedness supply the same-graph agreement premise for the covered trust views. The old–new matrix supplies the cross-graph premise: because every old and new quorum pair intersects in more than $B$ validators, each old/new certificate pair shares at least one correct validator, and that validator cannot sign both a parent-root-valid transition and a conflicting one. The child graph cannot validate itself, because the old active checker must accept the parent root, challenge state, complete cover, and intersection matrix before the child root can become active. A conflicting child certificate therefore requires an omitted cover row, a failed intersection bound, a bad linkedness report, a forged signature, or more than $B$ Byzantine validators — all outside the accepted-transition assumptions. The argument composes over a finite chain $G_0 \rightarrow \cdots \rightarrow G_n$ by induction on accepted, parent-root-bound steps, each step carrying its own Byzantine budget, challenge state, complete cover, and intersection witness. ∎

The honest scope of the proposition: it relies on MacBrough's pen-and-paper Cobalt construction and deliberately narrows deployment to rooted, bounded transition packets. The checker accepts bounded rotations and rejects large unsafe deltas, stale parent roots, open challenges, and oversized covers. Parent-root rejection and controlled registry mutation are exercised in artifact [E8].

### 6.8 Adversarial covers

Adversarial covers inside $M_{cover}$ get no special escape hatch. The extractor recognizes only declared protocol trust: every active essential subset referenced by any rooted trust view is included, deduplicated by a subset id derived from the subset contents, and checked against its declared validators, $t_S$, and $q_S$. Adding extra subsets increases $M$ and adds rows to the matrix; it cannot hide a bad pair. Reusing a subset id for different contents fails validation by construction. Marking an unfavorable subset inactive, stale, or outside the activation window fails extraction. Omitting an off-chain social dependency is not a cover attack at all — the protocol can only verify declared trust edges — it is an admission-evidence or operator-correlation failure, handled in §5. Within the declared rooted graphs, a Byzantine-controlled old/new certificate pair can pass only if the stated budget $B$ is false, a correct signer violates the signing rule, or a signature or checker assumption breaks.

### 6.9 Deadlock and bounded recovery

Cobalt deadlock preserves the last valid registry but remains a liveness
failure. Challenge windows are time-limited, and an expired transition leaves
the parent registry active. The current implementation does not provide the
four-action emergency recovery or capped availability-suspension state machine
described in earlier drafts. Recovery beyond rejecting or expiring the pending
transition requires an ordinary governed amendment authorized under the active
rules.

This is the concrete improvement over recommended-list publication: the live question becomes whether an exact transition packet verifies under rules that were active before it was proposed — a question with a yes/no answer and a replayable witness — rather than whether enough operators copied the right file.

---

## 7. Shielded Settlement

### 7.1 Notes, not private balances

Transparent ledgers reveal balances, counterparties, timing, inventory shifts, fund flows, liquidation levels, and strategy. For buy-side finance, market making, custody, treasury, and institutional settlement, that leakage has direct economic cost. The useful primitive is not a private account balance; it is a private spend of a note with public supply accounting, which is why PostFiat adopts Orchard/Halo2-style note semantics rather than account-level obfuscation.

A private Asset-Orchard spend exposes

$$public = \{root,\ nullifier,\ outputCommitments,\ fee,\ burn,\ policyHash,\ disclosureHash\}$$

and hides

$$hidden = \{asset,\ value,\ owner,\ memo,\ noteRandomness,\ witnessPath\}.$$

A valid spend proves note opening, membership under an accepted commitment root, correct nullifier derivation, value conservation, valid output commitments, and fee or burn correctness. Validators check the proof, nullifier uniqueness, root availability, resource limits, policy hash, and transaction envelope. For the private swap action they do not learn the spent or recipient note openings, raw asset IDs, values, owners, recipients, bilateral price, or memo.

The privacy statement is action-specific at the turnstile. Asset-Orchard ingress
v2 publicly reveals the signed burn source, asset, amount, pool, output
commitment, and authenticated `PFAOENC1` ciphertext; the note opening remains
wallet-local. Private egress deliberately reveals the public destination,
asset, amount, fee, nullifier, policy/disclosure bindings, and proof material.
Timing, action count, public pool accounting, and the ingress/egress linkage
remain observable. Historical ingress v1 carried a clear note opening and is
accepted only by archive replay; live proposal and execution reject it without
mutation. Legacy cleartext Mint/Spend actions have the same replay-only status
and are not represented as private settlement.

### 7.2 Verification is the consensus surface

The proof-system posture is explicit: Halo2 has no per-circuit trusted setup,
and the consensus transition verifies rather than generates proofs. The
repository also contains wallet/prover tooling and local prover-service code;
that code's presence in the workspace is not evidence that a validator process
generates proofs. A production deployment must keep proving work outside the
validator service boundary. Validators cache the action verifying key and
enforce a per-block action-count cap before verification, making shielded
settlement a priced transaction class rather than a free path. A local
release-build budget run measured a two-action output proof at 7,264 bytes and
cached verification at 80 ms median [E2]; these are budget measurements that
justify the cap design, not consensus constants.

### 7.3 Envelope binding

The PostFiat-specific boundary is authorization. Asset-Orchard swap and private
egress actions use randomized RedPallas spend-authorization signatures over a
chain/genesis/protocol-bound action sighash. That sighash binds the action's
anchor, nullifiers, randomized keys, commitments, encrypted outputs, accounting
records, fee, and swap/egress binding fields; the Halo2 public instance binds
the corresponding proof statement. `ShieldedActionBatch` is chain-bound by its
batch id, but the current action batch does not add an account-level ML-DSA
outer signature or carry a registry root/disclosure policy envelope. ML-DSA
instead authenticates validator proposals and certificate votes over the block
that contains the verified action (§9). The narrower implemented claim is that
action-field substitution or cross-chain replay invalidates the RedPallas/proof
bindings and that a quorum-certified block authenticates inclusion. This paper
does not claim a nonexistent ML-DSA shielded-envelope layer.

### 7.4 Turnstile accounting

Supply integrity does not rest on proof-system soundness alone. The boundary between transparent and shielded state is a turnstile: the protocol tracks net value entering each shielded pool, and cumulative withdrawals from a pool can never exceed its net deposits. If a soundness failure ever allowed counterfeit notes inside a pool, the counterfeit value could not exit past the turnstile without becoming arithmetically visible, at which point the affected action class freezes (§10) while transparent settlement continues. This is the containment discipline Zcash adopted for its pools, and it converts the worst-case cryptographic failure from silent inflation into a detectable, scoped incident. It also directly addresses the assumption asymmetry of §2: authorization is post-quantum, proof soundness is classical, and the turnstile bounds the damage of the classical assumption failing.

### 7.5 Atomicity with registry rotation

Registry rotation is atomic with respect to shielded state. Each block is verified under exactly one active registry root, and nullifier insertions, commitment-root updates, fee accounting, and any registry transition commit as one ordered state transition. A nullifier spent before a registry boundary is already spent after it; there is no rotation window in which a note can be double-spent across registries.

### 7.6 Disclosure and honest leakage

Selective disclosure is chosen by the holder of viewing material. A custodian or wallet can reveal a scoped note opening, policy tag, auditor proof, or transaction binding to a chosen party; the public chain receives only the commitment, nullifier, root, fee, and disclosure hash unless the holder elects to disclose more.

The privacy claim is correspondingly narrow and stated as such. Fee classes, admission buckets, timing, and disclosure hashes still leak metadata. The public observer learns that some shielded action paid a fee class, used an accepted root, consumed a nullifier, created commitments, and bound to a policy hash — and learns nothing else from the ledger. Timing remains visible, so wallets and custodians must still manage batching, relaying, withdrawal timing, and disclosure discipline; a careless workflow or a narrow anonymity set leaks regardless of the protocol. One further asymmetry is named rather than hidden: note encryption currently uses classical key agreement, so confidentiality — unlike authorization — is exposed to harvest-now, decrypt-later adversaries, and migrating note encryption to a post-quantum KEM is a governed upgrade path (§9, §11). PostFiat treats privacy as a baseline reduction in ambient leakage, not as anonymity insurance.

---

## 8. Replayable Machine Classification

### 8.1 Where the judgment already lives

Validator governance contains irreducibly qualitative evidence: operator independence, source conflicts, domain-control ambiguity, infrastructure concentration, behavior inconsistent with stated control. A static rubric handles exact predicates but punts interpretation of conflicting evidence to a committee, and a committee interprets privately. PostFiat uses a model for exactly one narrow step: converting public evidence into a typed classification that can be replayed, parsed, challenged, and ignored when it fails process checks. The design goal is not "AI governance." It is removing the last private room from a governance process that is otherwise hash-bound end to end.

No model runs inside consensus. Consensus consumes roots, certificates, and deterministic selector output, and by the deletion-monotonicity principle (§1.1), removing the model layer entirely leaves a chain that holds on ambiguous cases instead of adjudicating them — more conservative, never less safe.

### 8.2 The pipeline and its hard boundaries

Let $E$ be the evidence packet, $P$ the prompt and governed question, $Q$ the closed option set and schema, $M$ the pinned replay profile, $V_Q$ the parser, $A$ the parsed output, $S$ the selector, and $G_t$ the active registry. The model step is only

$$Y_{raw} = M(P, E, Q), \qquad A = V_Q(Y_{raw}).$$

A valid $A$ contains only labels from $Q$ and citations to field ids inside $E$. Unknown fields, uncited claims, or output outside the closed schema invalidate the result. The model cannot admit validators, alter thresholds, change its own prompt, modify the selector, or cast a governance vote; the only thing it can move is a typed classification, and only through the parser.

After replay convergence, the deterministic selector computes

$$\Gamma = S(policy_t, G_t, E, A),$$

where $S$ is total, hash-bound code with no model calls and no network access. The Cobalt proposal carries the full commitment tuple

$$(h(P),\ h(E),\ h(Q),\ h(M),\ h(V_Q),\ h(A),\ h(S),\ h(\Gamma),\ C_R),$$

and only a valid Cobalt transition changes protocol state. The live-effect boundary is therefore a one-way pipe:

```
model classification -> replay certificate -> deterministic selector -> Cobalt transition
```

By the least-machinery principle, the model is justified only where it handles conflicts a score table would punt to a private committee; where a static rule is equally good, the static rule is mandatory. Schema validation, signature checks, root matching, unknown-field rejection, stale-evidence rejection, concentration caps, churn limits, and Cobalt certificate validation are all exact predicates in code and never depend on model judgment.

### 8.3 Replay is profile-specific

Replay is a property of a pinned profile, not a claim of universal LLM determinism:

```
ReplayProfile = {
  model_weights_root,
  tokenizer_root,
  runtime_image_root,
  kernel_backend,
  hardware_class,
  batching_policy,
  parser_root,
  quantization_rule
}
```

A replay certificate is valid only if $q_R$ admitted independent replay keys sign the same packet root, replay-profile root, parser root, quantization root, and parsed-output root. Agreement means equality of the parsed-output root the selector consumes:

$$root(A) = \mathrm{SHA3\text{-}384}(canonicalJSON(validate_Q(A))).$$

Raw response hashes and provider envelopes are audit evidence only; they may legitimately differ across servers that add timing, request identifiers, or formatting outside the parsed decision. Top-logprob and full-vocabulary fixed-point roots are stronger replay evidence used for profile admission, dispute, and promotion; they are not hidden authority and never replace the parsed root. If the parsed root, packet root, profile root, parser root, or a required logprob root is missing, stale, below quorum, or split across certificates, the result is hold/no-op and becomes challenge evidence.

### 8.4 A worked admission packet

A concrete case shows what the model layer buys. Consider candidate evidence:

```
ValidatorAdmissionEvidencePacket {
  candidate:            validator-V
  uptime_30d:           0.997
  operator_manifest:    signed
  domain_control:       verified
  asn_country:          distinct from current registry
  release_manager:      same as validator-17
  monitoring_endpoint:  same as validator-17
  funding_control:      shared disclosure with validator-17
}
```

A static rubric checking only uptime, manifest, domain, ASN, and country would admit. The governed question is narrower: classify `operator_independence_evidence` as `independent | cosmetic_diversity | contradictory | insufficient`, citing only registered fields. The valid parsed answer is

```json
{
  "classification": "cosmetic_diversity",
  "citations": ["release_manager", "monitoring_endpoint", "funding_control"]
}
```

Replay keys sign the same evidence root, profile root, parser root, parsed-output root, and required logprob root. The selector consumes the parsed root and emits

```
Gamma = {
  candidate: validator-V,
  reliability_floor: pass,
  accountability_floor: pass,
  rho_cluster_after_admission: fail,
  action: hold-no-op
}
```

The Cobalt-facing packet is a non-transition: the parent registry root remains active, the next registry root equals the parent, the failed constraints are public, and any later admission must either resolve the shared-control cluster with new evidence or pass a governed selector change. The model did not admit or reject by fiat; it converted a qualitative conflict into a typed input that forced the deterministic selector to hold.

### 8.5 What the replay evidence shows — and only that

The empirical record has three layers, each backing one sentence. Same-stack repeatability: a saved 29-validator UNL cohort scored under one pinned Qwen/SGLang profile produced 2,900 of 2,900 parseable responses, 100 complete score maps, a single score-map hash, and zero variance in both scores and raw output [E5]. Cross-hardware convergence: six governed questions, each run three times on H100 NVL and three times on H200 under one named profile family, converged on identical parsed-output roots and top-logprob commitment roots, and a separate full-vocabulary next-token probe converged across 248,320 fixed-point logprob entries [E6]. Cross-runtime constitutional convergence: the first small-profile constitutional packet — adopt a Cobalt-governed on-chain registrar, or retain XRP-style off-chain UNL authority, with options `adopt-cobalt-registrar | retain-offchain-unl | hold-no-op` — was run on Apple M5 MLX BF16 (300/300 parseable) and on an H200 SGLang deterministic-inference profile (100/100 parseable); both runtime families selected `adopt-cobalt-registrar` and produced identical decision and parsed-output roots [E7].

The claim this supports is narrow and useful: under bounded packets and pinned profiles, independent runtimes converged on the same selector-facing decision. It is not a proof of universal LLM determinism, and it says nothing yet about cross-vendor profiles, cross-version drift, or adversarial prompts.

### 8.6 Admission is not promotion

Profile admission says one exact tuple — model, tokenizer, runtime, kernel, hardware class, batching policy, parser, quantizer — may produce replay-bound artifacts for one question class. Promotion to governance default is a separate, heavier event: an old/new shadow run on the same packet set, a published replay signer set and quorum, zero selector-relevant parsed-root divergence, published disagreement classes, cost and latency evidence, a defined rollback, and a Cobalt transition. Model replacement is itself a Cobalt transition, and until promotion completes, the old profile remains the historical replay anchor. The model layer is governed by exactly the machinery it serves.

---

## 9. Post-Quantum Authorization

### 9.1 Why genesis, not migration

Settlement chains carry long-lived value and long-lived public keys. A chain launched on classical signatures is betting that a coordinated migration will complete before cryptographically relevant quantum attacks matter — after years of exposing public keys on a permanent ledger. A new chain does not have to make that bet: it can make the default authorization path post-quantum from genesis and treat the known byte and CPU costs as a priced design input rather than a future emergency.

PostFiat uses ML-DSA because it is the standardized lattice signature family for general digital signatures (FIPS 204), with straightforward deterministic verification. Validator certificates bind to registry roots and compact validator identifiers rather than repeating full public keys in every vote:

$$Cert_B = \{(validatorID_i,\ sig_i)\}_{i \in Q} + registryRoot.$$

### 9.2 The cost accounting

For ML-DSA-65 in the current provider, public keys are 1,952 bytes and signatures are 3,309 bytes. A 35-validator set with $q = 24$ carries 79,416 signature bytes plus 768 bytes of validator identifiers — 80,184 detached certificate bytes before framing. A 100-validator set with $q = 67$ carries 223,847 detached certificate bytes. The certificate is large but bounded and separable: block headers commit to a certificate digest while audit nodes fetch and re-verify the detached certificate.

Current release-build measurements are roughly 6,000 ML-DSA-65 verifications per second, about 160 microseconds each; serialized verification of the 35-validator certificate is about 4 ms and of the 100-validator certificate about 11 ms [E1]. At a one-second certified-round target, 11 ms is about 1.1% of the round; on the observed 1.5-second submit-to-finality path it is below 1%. Certificate bytes, not signature CPU, are the dominant operational cost of the current profile, and every release profile must publish verifier throughput, certificate bytes, and the signature share of the block verification budget.

### 9.3 Alternatives and current recovery limitation

The alternatives were weighed, not ignored. Falcon-class schemes are smaller on the wire but carry a more delicate implementation and side-channel posture. Hash-based signatures are the most conservative assumption but make validator certificates and high-frequency account authorization substantially heavier. Hybrid classical-plus-post-quantum signatures are valuable for migrating legacy chains; a greenfield chain can simply not carry the classical dependency in its default path. Parameter and library changes are governed cryptographic upgrades under §6.

The current genesis and validator-registration schemas do not contain an
SLH-DSA recovery-key commitment, and the runtime has no FIPS 205 verification
or activation path. ML-DSA-65 is therefore the only implemented validator and
transparent-account signature family. A future independent recovery family
requires a versioned commitment schema, activation rules, migration tooling,
and external cryptographic review before it can become a protocol claim. The
remaining classical surfaces — Halo2 proof soundness and note-encryption key
agreement — are bounded by the turnstile (§7.4) and named as upgrade paths
(§7.6, §11), not solved by a currently deployed recovery key.

---

## 10. Failure Modes and Recovery

Some failure classes have fail-closed state-machine responses; others remain
operational recovery gaps. This section states the current boundary instead of
assuming every future recovery path is precommitted.

A governed registry transition can remove or suspend a validator under the
active registry's authorization rules. Replay divergence in the classification
pipeline is hold/no-op evidence. Shielded accounting limits remain visible
through the turnstile. An ML-DSA break, however, has no precommitted alternate
signature activation path in the current release and requires a coordinated
software and governance migration while affected authorization is halted.
Availability below the normal quorum likewise halts liveness; there is no
automatic suspension that lowers the certificate threshold.

Implemented recovery checks aim to fail closed. That principle is a review
requirement, not evidence that unimplemented recovery mechanisms already exist.

---

## 11. Limitations and Non-Claims

A paper that hedges every paragraph hides its real limits, so they are consolidated here and stated plainly.

Genesis is trusted. The launch certificate (§5.4) makes the bootstrap assumption explicit, frozen, and auditable; it does not make it disappear, and nothing can. Authority validation retains residual capture risk: a sufficiently patient coalition of natural stakeholders, or a jurisdictional correlation no evidence packet captures, is not eliminated by the machinery — it is made expensive, visible, and contestable. Relatedly, the protocol verifies only declared trust edges and registered evidence fields; an off-chain social dependency that no one declares is invisible to the Cobalt checker and must be caught, if at all, at the admission and correlation layer.

The privacy model reduces ambient leakage; it is not anonymity insurance. Timing, fee classes, and disclosure hashes remain visible, anonymity sets must be managed by wallets and custodians, and a careless workflow leaks regardless of the protocol. Note confidentiality rests on classical key agreement and is exposed to harvest-now, decrypt-later adversaries until the governed KEM migration; proof-system soundness is likewise a classical assumption, bounded by the turnstile but not removed by it.

The replay evidence is narrow by design and the paper claims nothing beyond it: one model family, pinned profiles, bounded packets, no cross-vendor convergence claim, no adversarial-prompt robustness claim. The Cobalt deployment is a rooted, bounded profile of MacBrough's construction, not the full open-network result; the transition-safety proposition holds for the narrowed profile and its stated assumptions only. ML-DSA certificate bytes are a real and growing cost at larger validator sets, mitigated by digest commitment and detached verification but not eliminated. The current committee has no automatic availability-suspension shortcut; loss of the normal quorum is a liveness halt.

Finally, the controlled evidence supports controlled claims only. Replay-profile convergence is not public validator diversity; controlled registry mutation is not market legitimacy; shielded accounting is not wallet anonymity. Missing evidence defaults to hold, no-op, or continued operation under the last valid rules — never to the favorable interpretation.

---

## 12. Related Work

PostFiat draws on the XRP Ledger lineage: the original Ripple consensus design
of Schwartz, Youngs, and Britto, and the Chase–MacBrough analysis of UNL-overlap
safety. The current runtime does not implement XRPL's Negative UNL. MacBrough's
Cobalt motivates the trust-evolution checker; PostFiat narrows that machinery
to rooted graphs, bounded covers, proposer-independent extraction, and
fail-closed transition packets.

Stellar's SCP is the closest cousin in spirit: both replace global membership with locally declared trust. The difference is the governance object. SCP quorum slices are discovered, open, and continuously self-asserted; PostFiat trust views are registered, rooted, and changed only through checkable transitions, trading openness for a transition-safety argument that a checker can actually evaluate. Consensus v2 uses quorum-certificate arithmetic and deterministic proposer rotation common to authority-validated BFT systems, but its explicit prepare/precommit rule is not a claim of HotStuff's chained commit protocol. The shielded pool adapts Zcash's Orchard action model, Halo2's setup-free proving, and Zcash's pool turnstile discipline. Ordering and omission remain explicit risks because the production lane does not yet consume the reference admission-receipt protocol. The implemented post-quantum authorization posture follows NIST FIPS 204; FIPS 205 recovery remains future work. The replay design draws on recent work in deterministic LLM inference — batch-invariant kernels and reproducible serving — while deliberately claiming only profile-pinned, packet-bounded convergence.

---

## Appendix A: Evidence Register

Each artifact backs the specific sentences that cite it and nothing more. Roots are SHA3-384 or provider-native commitments as produced by the runs; machine reports are retained alongside the artifacts.

**[E1] ML-DSA-65 verification budget.** Supports §9.2. Release-build measurements: ~6,000 verifications/s (~160 µs each); detached certificate of 80,184 bytes for a 35-validator, $q=24$ set and 223,847 bytes for a 100-validator, $q=67$ set; serialized certificate verification ≈ 4 ms and ≈ 11 ms respectively. Budget evidence, not consensus constants.

**[E2] Orchard verification budget.** Supports §7.2. Local release-build run: two-action output proof of 7,264 bytes; cached verification 80 ms median. Justifies per-block action caps and the priced-class design.

**[E3] Cobalt cover sizing.** Supports §6.6. Grouped institutional-style trust views under the $M_{cover}=64$ profile: 35 validators in five seven-validator groups → $M=12$; 100 validators in ten ten-validator groups → $M=22$.

**[E4] Admission selector fixtures.** Supports §5.3. Deterministic selector over `ValidatorAdmissionEvidencePacket` inputs with fixture coverage: clean admit, shared-control reject, missing-domain hold, contradictory-evidence hold, unknown-model-field hold.

**[E5] Same-stack repeatability.** Supports §8.5. Qwen3.6/SGLang pinned profile; saved 29-validator XRPL UNL cohort; one validator-domain record per request; temperature 0, JSON response mode, non-thinking output; prompt: "score this validator's credibility on a scale from 0-100 where credibility is defined as useful institutional proof of a blockchain's legitimacy"; parser accepts only JSON with one integer score field in [0,100]. Two batches × 50 repeats per domain: 2,900/2,900 parseable responses, 100 complete score maps, zero score variance, zero raw-output variance. Score-map hash: `9f7f95a7be238e2b6bb1cc081986f8b5dffc07b9397578d723c6f6d7c77c81c8`.

**[E6] Cross-hardware convergence.** Supports §8.5. One named Qwen/SGLang profile family; six governed questions, three runs each on H100 NVL and on H200; parsed-output roots and top-logprob commitment roots converged for every question. Separate H100/H200 full-vocabulary next-token probe converged over 248,320 fixed-point logprob entries; vector root: `560ea13c99f73a60c184ec07ba3554ea11c72487d56ac28c366495d58ce8913c`.

**[E7] Cross-runtime constitutional packet.** Supports §8.5. Closed options `adopt-cobalt-registrar | retain-offchain-unl | hold-no-op`; prompt hash `277e4174662841fe8d0802f0d055fec0528afbae09173a49d1f9067fc9a5ad68`. Apple M5 MLX BF16 with Qwen3-1.7B: 300/300 parseable. Vast H200 SGLang deterministic-inference profile with Qwen3-1.7B: 100/100 parseable, one top-logprob root. Both selected `adopt-cobalt-registrar`. Decision root: `08b3d570e746a4bd4c761ab280aa1f6f4992704810f03de2377c8a38b0fc0cf8`. Parsed-output root: `1f667e5d8d63fbc8852b10085062e13579f864f27f3b3c53481b68bc4b2fbc1e`. H200 top-logprob root: `af228110a9782fcfdca48dd681636360d24d3a995933443bd06e1374e8cbda07`. Machine reports: `reports/qwen-mlx-profile-portability/20260528T155652Z/machine_report.json`, `reports/qwen-sglang-profile-portability/20260528T162243Z/machine_report.json`.

**[E8] Consensus and registry fixtures.** Supports §4.1, §4.3, §6.7. Quorum-certificate arithmetic, height-wide vote-lock and equivocation fixtures; a separate non-production ordering model; signed admission-receipt aggregation fixtures; Cobalt parent-root rejection; controlled registry mutation under the previous active rules.

---

## References

David Schwartz, Noah Youngs, and Arthur Britto. "The Ripple Protocol Consensus Algorithm." Ripple Labs, 2014.

Brad Chase and Ethan MacBrough. "Analysis of the XRP Ledger Consensus Protocol." arXiv:1802.07242, 2018.

Ethan MacBrough. "Cobalt: BFT Governance in Open Networks." arXiv:1802.07240, 2018.

Maofan Yin, Dahlia Malkhi, Michael K. Reiter, Guy Golan Gueta, and Ittai Abraham. "HotStuff: BFT Consensus with Linearity and Responsiveness." PODC 2019.

Miguel Castro and Barbara Liskov. "Practical Byzantine Fault Tolerance." OSDI 1999.

Cynthia Dwork, Nancy Lynch, and Larry Stockmeyer. "Consensus in the Presence of Partial Synchrony." Journal of the ACM, 1988.

Leslie Lamport, Robert Shostak, and Marshall Pease. "The Byzantine Generals Problem." ACM TOPLAS, 1982.

Gabriel Bracha. "Asynchronous Byzantine Agreement Protocols." Information and Computation, 1987.

David Mazières. "The Stellar Consensus Protocol: A Federated Model for Internet-Level Consensus." Stellar Development Foundation, 2015.

XRP Ledger documentation: Unique Node List, Consensus Protocol, Negative UNL, Transaction Cost, XRP Overview.

Zcash Improvement Proposal 224, "Orchard Shielded Protocol"; Zcash Protocol Specification; the halo2 Book.

NIST FIPS 204, "Module-Lattice-Based Digital Signature Standard"; NIST FIPS 205, "Stateless Hash-Based Digital Signature Standard"; CRYSTALS-Dilithium specification; Open Quantum Safe ML-DSA parameter summaries.

Philip Daian, Steven Goldfeder, Tyler Kell, Yunqi Li, Xueyuan Zhao, Iddo Bentov, Lorenz Breidenbach, and Ari Juels. "Flash Boys 2.0: Frontrunning in Decentralized Exchanges, Miner Extractable Value, and Consensus Instability." IEEE S&P 2020.

SGLang deterministic inference documentation; vLLM batch-invariance and reproducibility documentation; Thinking Machines Lab, "Defeating Nondeterminism in LLM Inference," 2025.
