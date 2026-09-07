# PostFiat: An Authority-Validated Settlement Ledger

### Governed trust evolution, shielded settlement, replayable machine classification, and post-quantum authorization

**Whitepaper, Version 3 — September 2026**

---

## Abstract

PostFiat is a Layer 1 settlement-ledger design built around known validators,
deterministic certificate finality, fixed native supply, fee burn, and no native
validator rewards. Its economic premise is that institutions which depend on
reliable settlement can have sufficient reason to operate validators without a
protocol subsidy. This premise makes validator admission and independence
central security questions.

The design combines four mechanisms: Cobalt-governed changes to validator
trust; shielded settlement with public supply accounting; bounded, replayable
machine classification of governance evidence; and post-quantum authorization
for accounts and validators. Cobalt makes trust changes subject to the preceding
rules. Shielded notes conceal private settlement details while preserving
explicit boundaries with public balances. Machine classifications inform
decisions without acquiring authority to change the ledger. Post-quantum
signatures protect account and validator authorization; the privacy system
retains classical cryptographic assumptions.

This paper develops the economic argument, protocol rules, and conditional
security reasoning for that design. The central distinction is between checking
a decision under declared assumptions and establishing that those assumptions
are true: quorum arithmetic cannot discover hidden common control, replay
cannot establish the truth of evidence, and public pool accounting cannot
replace proof soundness.

---

## 1. Introduction

Financial settlement does not necessarily require a market in block-production
rewards. The XRP Ledger illustrates a different category: known validators,
certificate-based agreement, fixed native supply, and transaction fees destroyed
rather than paid to validators. In this category, the institutions that rely on
settlement have reasons to maintain the infrastructure on which their business
depends.

PostFiat adopts this economic premise and concentrates on four associated
questions. How can validator trust change without breaking agreement? How can
financial activity be private while public value accounting remains checkable?
Where can machine judgment help evaluate governance evidence without becoming
an unaccountable authority? How should long-lived settlement authorization
respond to quantum risk?

The proposed answers are governed trust evolution under Cobalt (§6), shielded
notes with per-asset public accounting and holder-controlled disclosure (§7),
typed and replayable advisory classification (§8), and post-quantum account and
validator signatures (§9). Transparent settlement and public pool boundaries
remain part of the ledger. Each mechanism has a specific security scope; their
composition depends on the assumptions stated below.

### 1.1 Design principles

*Fail closed.* Missing or conflicting prerequisites prevent the action they
would authorize. An unresolved governance proposal leaves the preceding valid
rules in force.

*Old rules validate new rules.* A proposed registry, trust graph, checker,
safety profile, or classification policy cannot authorize its own activation.
Changing the rules for future transitions requires a valid transition under the
preceding rules.

*Least machinery.* Exact predicates handle facts that can be checked exactly.
Deterministic selection follows declared policy. Machine classification is
reserved for evidence interpretation and cannot override either.

*Hash-bound evidence.* Governance decisions commit to the evidence, policy, and
result they concern. A substituted document or decision produces a different
commitment. A matching commitment establishes identity of content, not its
truth.

*Deletion monotonicity.* Where a policy requires machine classification,
removing that classification can delay an admission but cannot create one.
Safety must not depend on accepting an unreviewable model answer.

### 1.2 Scope

This paper specifies a protocol design and its security obligations. A
mechanism's description states what it must do to support the argument.
Security conclusions are conditional on the fault, evidence, and cryptographic
assumptions; the economic premise requires independent empirical evaluation.

---

## 2. Threat Model and Assumptions

PostFiat considers five adversary classes.

| Adversary | Capability | Required defense |
|---|---|---|
| Byzantine validator minority | Equivocation, invalid proposals, stale votes, and disruption. | Quorum certificates, persistent signing locks, and domain-bound authorization. |
| Correlated validator group | Common control or dependencies presented as independence. | Authenticated admission evidence, challenges, concentration limits, and conservative fault assumptions. |
| Governance authority acting outside its scope | Attempted registry or policy changes without the preceding rules' authorization. | Explicit authority scopes and transitions verified under the active rules. |
| Model or runtime operator | Prompt injection, evidence substitution, model drift, or inconsistent classification. | Closed outputs, committed evidence, replay profiles, and deterministic decision gates. |
| Public observer | Balance analysis, flow correlation, timing analysis, or strategy inference. | Shielded notes and scoped disclosure, subject to public-boundary and metadata leakage. |

Within an epoch of $n$ active validators, the consensus fault bound is
$f=\lfloor(n-1)/3\rfloor$. Across a trust transition, the Byzantine population
during the relevant signing window must satisfy the transition profile's
budget $B$. Operator independence is an assumption supported by evidence; it
does not follow from the number of keys in a registry.

Safety must hold without a bound on message delay. Consensus liveness after a
failed proposer assumes partial synchrony and enough responsive correct
validators to form a quorum. A trust transition also needs sufficient
participation to complete agreement and resolve challenges. An elapsed timeout
alone never authorizes conflicting finality.

The cryptographic assumptions include unforgeability of ML-DSA signatures,
collision resistance of domain-separated hashes, security of shielded spend
authorization and note encryption, and soundness and zero knowledge of the
privacy proofs. ML-DSA supplies the post-quantum authorization premise.
RedPallas, Orchard-style key agreement, and Halo2 retain classical assumptions.
Consequently, post-quantum account and validator authorization does not imply
post-quantum privacy or shielded spend security.

---

## 3. Ledger Model

The replicated ledger contains public account and asset state, shielded note
commitments and nullifiers, per-asset pool accounting, active governance and
validator-trust state, and certified transaction history. Application state,
including issued assets, markets, and reserve-backed instruments, must be
committed wherever it affects a valid state transition. A state commitment is
complete only if equal commitments imply equal state for future validation.

Native PFT has a fixed genesis supply. There is no subsequent native issuance
or validator subsidy, and transaction fees burn native PFT. Issued assets have
their own authorization and supply rules; the native-supply constraint does not
prohibit their creation. Fees price demands on bandwidth, verification, and
persistent state. Their economic adequacy is separate from the mathematical
fact that charging and burning them preserves the declared supply accounting.

An ordered block commits one transition across these state domains. A valid
block certificate establishes agreement on the ordered block; an individual
transaction succeeds only if its authorization and execution rules also pass.
Rejected actions cannot partially transfer value or alter trust.

Each height has one active registry root. A registry change is authorized by
the preceding rules and takes effect at a specified ordered boundary. Privacy
state, public balances, and governance state must agree on that boundary.

---

## 4. Consensus and Ordering

### 4.1 Certified ordering

For $n$ active validators, let
$f=\lfloor(n-1)/3\rfloor$ and $q=\lfloor 2n/3\rfloor+1$. Quorums count distinct
authenticated validator identities. The proposer is determined by the height
and view. Proposals and votes bind the chain, active committee, height, view,
parent, proposed transition, resulting state commitment, and voting phase.

Agreement uses prepare and precommit phases. A quorum of prepare votes forms a
prepare certificate and establishes a lock. A quorum of precommit votes for the
same nonempty block forms the certificate required to commit it. A prepare
certificate alone does not finalize a block.

Validators preserve their signing history and locks before releasing votes.
A restart cannot erase an obligation created by an earlier signature. This
persistent state, together with quorum intersection, prevents correct
validators from contributing to incompatible certificates.

A quorum of timeout votes permits advancement to a later view. The next
proposal carries the preceding timeout certificate and its highest justified
prepare certificate. References must resolve to authenticated certificates;
a higher view number alone cannot invalidate a lock. Reproposing a locked
block is permitted, while a conflicting proposal must satisfy the protocol's
lock rules. Safety depends on these rules across views, not merely on avoiding
two signatures within one view.

### 4.2 Deterministic inclusion order

A proposal specifies an ordered sequence of transactions. Agreement on that
sequence and deterministic execution give validators a common resulting state.
They do not establish a global first-seen order: ingress conditions and
proposer choices can affect both inclusion and position.

Fair ordering and censorship accountability require additional evidence about
admission, deadlines, and proposer obligations.

### 4.3 Censorship accountability

An inclusion proof establishes that a transaction appears in certified history.
Its absence from a block does not establish when a quorum first received it,
whether it was valid at that time, or which party caused the omission.
Attributing censorship would require authenticated admission evidence and an
explicit inclusion obligation. Ordinary finality certificates do not supply
those facts.

### 4.4 Quorum loss

Unavailable validators continue to count in the active committee. Progress
requires its normal quorum; if that quorum cannot form, the chain halts.
Timeouts and missed rounds do not independently lower the threshold. A
membership change must be authorized under the applicable preceding rules.

---

## 5. Authority Validation: Economics and Admission

### 5.1 The economic case

Authority validation relies on known operators, accountability, operational
history, and economic dependence on the ledger. Its premise differs from
purchasing Sybil resistance through proof-of-work expenditure or bonded stake:
the institutions benefiting from settlement may already have reason to maintain
it.

That premise is credible only when the validator set is sufficiently
independent and durable. An opaque or commonly controlled set does not become
secure by acquiring more identities. Admission must justify the operator
choice; trust governance must preserve agreement when that choice changes.

Validator rewards can recruit operators, but they can also attract participants
whose interest is primarily the subsidy. PostFiat's economic question is whether
natural demand for reliable settlement supplies enough independent operators
without that subsidy.

### 5.2 Zero issuance as a conditional claim

The proposed economic model is

$$\text{fixed native supply} + \text{fee burn} + \text{natural validators}.$$

It depends on operators having sufficient benefits from settlement reliability
to cover their costs, and sufficient exposure and accountability to discourage
misconduct. Public evidence about a specific candidate must support this
premise. Neither a fixed supply nor a successful admission vote proves it.
Whether the policy recruits a durable, independent operator set remains an
empirical question.

### 5.3 Admission policy and the evidence boundary

Admission has three obligations: establish facts about an operator, evaluate
those facts under a policy, and authorize a registry change. A deterministic
policy evaluator can satisfy the second obligation without establishing the
first or performing the third.

The economic policy considers five dimensions:

| Dimension | Relevant evidence |
|---|---|
| Economic exposure | Dependence on reliable settlement, including custody, exchange, gateway, or other verifiable use. |
| Accountability | Operator identity, control of public identifiers, jurisdiction, incident history, and revocation paths. |
| Reliability | Availability, maintenance, monitoring, retained history, and infrastructure redundancy. |
| Attack risk | Conflicting incentives, governance influence, transaction access, and value exposed during an attack window. |
| Correlation | Common operation, infrastructure, funding, release management, or key custody. |

A proposed admission predicate is

$$
\begin{aligned}
\operatorname{admit}(i) ={}&
(x_i \ge x_{\min}) \land
(r_i \ge r_{\min}) \land
(a_i \ge a_{\min}) \\
&{}\land (b_i \le b_{\max}) \land
(\rho_i \le \rho_{\max}) \land
\operatorname{safe}(T_t,i),
\end{aligned}
$$

where $x_i$, $r_i$, and $a_i$ summarize exposure, reliability, and
accountability; $b_i$ bounds assessed attack risk; $\rho_i$ measures assessed
correlation; and $\operatorname{safe}(T_t,i)$ requires an acceptable trust-graph
change under §6. The active policy must define how observations support these
quantities. High exposure cannot compensate for prohibited common control.

The predicate is meaningful only with justified inputs. A supplied signature
flag is not a signature verification; a source hash is not evidence of truth;
different operator labels do not establish independent control. Authenticated
observations, reproducible scoring, and challenges are required to support the
real-world premises. Some uncertainty, especially about undisclosed
relationships and future incentives, remains irreducible.

Missing, stale, or conflicting required evidence causes a hold. An established
policy violation causes rejection and takes precedence over a hold. For
example, confirmed prohibited shared release management remains a reason to
reject even if unrelated evidence is missing.

A passing evaluation recommends a candidate. Only an authorized trust
transition can change the registry. Cobalt agreement establishes a decision
under the declared evidence and rules; it cannot establish the truth of an
unsupported operator claim.

### 5.4 Public launch commitments

Genesis is a bootstrap trust decision. The proposed public-launch policy makes
that decision explicit through a signed commitment to the initial registry,
trust graph, checker, safety profile, admission evidence, and ratifier set.

The proposed profile requires at least seven ratifiers, no single control group
above one third, and disclosure of common release management, key custody, and
funding control. A launch commitment records who accepted the initial
assumptions and exactly which assumptions they accepted. It does not make
genesis trustless.

---

## 6. Cobalt Trust Evolution

Cobalt governs changes to declared validator trust. A trust update must become
an agreed, authorized transition under the preceding rules before it can affect
future validation. Reliable broadcast, binary and multivalue agreement, and
ratification provide the agreement structure; the registry and trust graph
determine whose participation can authorize a decision.

This authority has an explicit scope. A handoff to Cobalt must itself be
authorized under the preceding rules. Once Cobalt governs validator trust, an
authorization from the previous authority cannot substitute for the required
Cobalt decision. Authority over unrelated governance does not follow from
authority over validator trust.

Trust ratification and block finality serve different purposes. Ratification
authorizes a particular trust change; consensus orders that action with the
rest of the ledger. Its execution must satisfy the transition rules before the
new registry becomes active.

### 6.1 Trust as protocol state

A trust configuration comprises a validator registry $G_t$, trust graph $T_t$,
checker $\chi_t$, and safety profile $\pi_t$. The trust graph records the
essential subsets on which validators rely. The safety profile supplies the
fault budgets, quorum requirements, and bounds used to judge a change.

The initial configuration is committed at genesis. Later changes are explicit
proposals with authenticated parent commitments. A proposed configuration
cannot supply the authority or checking rules needed to accept itself.

### 6.2 Transition packets

A transition binds its parent registry and trust graph, the proposed change,
the resulting configuration, admission evidence, active checking rules,
challenge state, activation boundary, expiry, and authorization certificate.
Signatures bind the exact proposal and its history; they cannot be transferred
to a different parent or successor.

The safety witness supplies local threshold rows, a complete cover of relevant
essential subsets, old–new intersection bounds, and linkedness evidence.
Validators recompute the required predicates from the committed inputs.
Unresolved required challenges prevent activation.

### 6.3 Local safety rows

For an essential subset $S$ with $n_S=|S|$, Byzantine budget $t_S$, and quorum
$q_S$, the local Cobalt inequalities are

$$0 \le t_S,q_S \le n_S,\qquad t_S < 2q_S-n_S,\qquad 2t_S < q_S.$$

The first bound establishes well-formedness. The second ensures that any two
quorums within $S$ share at least one correct validator under its fault budget.
The third ensures that correct validators outnumber Byzantine validators in a
quorum. Availability additionally requires enough correct participants to form
that quorum.

### 6.4 Linkedness

A trust view $V_i$ declares essential subsets $ES_i$. Two views $V_i,V_j$ are
linked when they share an essential subset whose faults stay within its budget.
They are fully linked when that subset also contains enough correct validators
to form its quorum:

$$S\in ES_i\cap ES_j,\qquad
\operatorname{faults}(S)\le t_S,\qquad
|S|-\operatorname{faults}(S)\ge q_S.$$

For the transition argument, every relevant pair of views in the reachable
trust closure must satisfy the required linkedness condition. The check is
relative to an explicit fault model. When several fault allocations are
admissible, the safety claim must cover all of them; evaluating a convenient
allocation is insufficient.

Graph arithmetic cannot identify real Byzantine operators. Undisclosed common
control can invalidate the fault model even when every declared graph
predicate passes. Admission evidence and graph safety therefore remain
separate obligations.

### 6.5 Cross-registry quorum intersection

Local rows alone do not establish safe membership changes. Let $Q_{\rm old}$
and $Q_{\rm new}$ denote the certificate-authorizing quorums covered by the old
and proposed configurations. Under transition fault budget $B$, require

$$
\forall q_o\in Q_{\rm old},\ \forall q_n\in Q_{\rm new}:
\quad |q_o\cap q_n|>B.
$$

Consider an old registry $\{A,B,C,D,E,F,G\}$ and proposed registry
$\{A,B,H,I,J,K,L\}$. Each is a single essential subset with $n_S=7$, $q_S=5$,
and $t_S=2$. Both local rows pass. Yet the old quorum $\{A,B,C,D,E\}$ and new
quorum $\{A,B,H,I,J\}$ share only $\{A,B\}$. With fault budget $B=2$, every
shared signer could be Byzantine.

The proposed change therefore fails the intersection check and leaves the old
registry active. Safe endpoints do not, by themselves, imply a safe transition.

### 6.6 Bounded cover extraction

The active safety profile fixes a maximum $M_{\rm cover}$ on the total old and
new essential subsets considered in one transition. A deterministic extractor
derives the cover from both rooted trust graphs. It includes every relevant
active subset, rejects conflicting declarations, and permits no
proposer-selected omissions. A cover exceeding the bound prevents the
transition.

Threshold subsets admit a useful pairwise bound. For subsets $S_o,S_n$ with
thresholds $q_o,q_n$, the smallest possible intersection between their
threshold-sized signer sets is

$$\max\!\left(0,\ q_o+q_n-|S_o\cup S_n|\right).$$

Thus the corresponding intersection test need not enumerate every signer
combination. With $m_o$ old rows, $m_n$ new rows, $M=m_o+m_n$, and
$V=|G_t\cup G_{t+1}|$, sorting the cover and evaluating these pairwise bounds
has cost

$$
O(M\log M)+O(m_o m_n V\log V)
\le O(M_{\rm cover}^2 V\log V).
$$

This is a bound on the threshold-cover calculation. Applying it to a trust
configuration also requires showing that the extracted cover represents every
case that can authorize a certificate. A short list of subsets is not, by
itself, a proof of complete quorum coverage.

### 6.7 Conditional transition safety

Call two transition histories incompatible if they select different successors
of the same certified parent or otherwise diverge. The same compatible history
may be proposed again in a later view.

**Proposition (cross-registry certificate compatibility).** Suppose the
authenticated old and new configurations satisfy the local safety and
linkedness premises, their cover includes every certificate-authorizing case,
and every relevant old–new quorum pair intersects in more than $B$ validators.
Assume at most $B$ distinct validators behave Byzantine over the entire relevant
signing window. Further assume that correct validators persistently refuse to
authorize incompatible transition histories, including across view changes,
registry changes, and restarts. Then an old certificate and a new certificate
cannot authorize incompatible histories.

*Proof.* Every such certificate pair shares more than $B$ signers, so at least
one shared signer is correct. Incompatible certificates would require that
signer to violate its persistent history constraint. This contradicts the
assumption. ∎

The persistent constraint is essential. Signing at most once for each
individual tuple of height, view, registry root, and transition root is
insufficient: incompatible proposals can use different tuples. Locks and any
unlocking justification must preserve compatible history across those changes.
A later view or a new registry root alone cannot release this obligation.

This proposition establishes a conditional certificate-exclusion result.
A complete protocol proof must also derive the persistent signing constraint
from the agreement rules, establish cover completeness and same-configuration
agreement, and bind activation to the preceding authority. Those obligations
are part of the design's security argument; intersection arithmetic alone does
not discharge them.

### 6.8 Adversarial covers

Adding declared subsets adds constraints; it cannot remove an unfavorable
pair. A reused subset identifier with different contents is invalid. A proposer
cannot omit an active subset by presenting it as inactive or outside the
transition window.

An undisclosed social dependency is a different failure: it can make the
declared fault budget false without changing the graph. The cover protects
against selective presentation of declared trust. Evidence review must address
the relationship between that declaration and actual operator control.

### 6.9 Deadlock and recovery

A stalled trust update leaves the last valid registry in force. Expiry ends
the proposal's eligibility; it does not lower quorum requirements or authorize
a replacement configuration.

Returning to an earlier configuration is a new forward transition under the
active rules, with its own parent, authorization, and safety checks. It cannot
erase certified history or restore spent authorization. If the active
authority cannot form the required quorum, the protocol may remain halted.
Recovery outside its accepted rules is a separately coordinated change to the
trust assumptions.

---

## 7. Shielded Settlement

### 7.1 Notes and public supply

Transparent settlement reveals balances, counterparties, inventory movements,
and financial strategy. PostFiat uses Asset-Orchard note semantics, based on
Orchard and Halo2, to support private transfers with public supply accounting.

A private note commits to its asset, value, owner, and associated randomness.
A spend proves knowledge of an authorized note opening, membership under an
accepted commitment root, correct nullifier derivation, conservation of value,
and valid outputs. Nullifiers prevent reuse of spent notes without publishing
their openings.

The public statement includes commitment roots, nullifiers, output commitments,
and the applicable fee, accounting, policy, and disclosure bindings. Private
settlement conceals the relevant note openings, including their asset, value,
owner, memo, and membership witness. A private swap can therefore conceal the
underlying note values and bilateral price while proving the required
conservation and authorization relations.

Privacy changes at the boundary with public balances. Ingress reveals the
public source, asset, amount, and destination pool commitment. Egress reveals
the public destination, asset, and amount. Those disclosures are necessary to
verify the public side of the transfer; they are not hidden by a subsequent
private action.

### 7.2 Verification and resource bounds

Proof generation belongs to the sender or a prover acting for the sender.
Validators verify proofs against the exact public statement consumed by the
ledger. Halo2 does not require a separate trusted setup for each circuit.

Before accepting an action, validation checks its proof and authorization,
accepted anchor, nullifier uniqueness, conservation rules, and policy bindings.
Proof size and verification work must be bounded and priced. These limits
protect availability; they do not strengthen the proof system's soundness
assumption.

### 7.3 Authorization binding

Private spend authorization uses randomized RedPallas signatures bound to the
chain and the exact action. The signed statement binds the relevant anchor,
nullifiers, keys, commitments, encrypted outputs, accounting, fees, and
swap or withdrawal conditions. The proof and signature must refer to the same
action.

Shielded action authorization does not add an account-level ML-DSA signature.
ML-DSA validator signatures authenticate the block containing the verified
action. That establishes certified inclusion; it does not replace RedPallas
spend authorization or make a classical privacy proof post-quantum.

Chain and action bindings prevent a valid authorization from being reused on
another chain or substituted into a different transfer.

### 7.4 Turnstile accounting

For each asset, public accounting tracks value entering and leaving the
shielded pool, including any pool-funded fee or burn. Accounted outflow cannot
exceed accounted inflow, and the remaining public pool total cannot become
negative.

This turnstile bounds how much value can leave through the public boundary.
It does not prove that each claimant owns a legitimate note. If proof soundness
fails, counterfeit notes can compete for existing pool value without violating
the accounting bound. The turnstile therefore cannot guarantee depositor
recovery or detect every counterfeit spend.

A governed pause requires a separate detection and authorization process.
Neither pausing nor public accounting repairs a broken proof assumption.

### 7.5 Atomicity with registry rotation

Every block is verified under one active registry root. Nullifier insertions,
commitment updates, public accounting, and any registry transition commit as
one ordered state transition. A spent nullifier remains spent across the
registry boundary. Changing the validator set cannot recreate a note or reset
its spend history.

### 7.6 Disclosure and leakage

Selective disclosure allows a holder to reveal evidence about chosen
transactions without granting spending authority. Its privacy scope depends on
the disclosed material: a narrow transaction disclosure and a viewing
capability reveal different amounts of history.

Public observers still see timing, action counts, fees, ciphertexts,
commitments, and public pool accounting. Ingress and egress expose the fields
specified in §7.1. Transaction patterns, withdrawal timing, and the available
anonymity set can support linkage even when note openings remain secret.

Orchard-style note encryption uses classical key agreement. An adversary can
retain ciphertext today in anticipation of a future cryptographic break.
Post-quantum account signatures do not address that confidentiality risk;
protecting it requires an appropriate encryption scheme and a governed
migration.

---

## 8. Replayable Machine Classification

### 8.1 Bounded evidence interpretation

Validator governance includes qualitative questions about operator
independence, conflicting sources, and control relationships. Exact rules can
reject a known prohibited dependency; interpreting ambiguous evidence may
require judgment.

The proposed classification process makes that judgment explicit. A model
converts committed public evidence into a typed answer to a bounded question.
The answer can be replayed, challenged, and excluded when its prerequisites
fail. It remains advisory: admission policy and governance authorization retain
their separate roles.

Model inference takes place outside consensus. No model response can directly
change a registry, alter a threshold, or authorize its own policy.

### 8.2 The classification pipeline

Let $E$ be the evidence packet, $P$ the governed question and prompt, $Q$ the
closed output schema, $M$ the pinned inference profile, $V_Q$ the parser, and
$A$ the parsed answer:

$$Y_{\rm raw}=M(P,E,Q),\qquad A=V_Q(Y_{\rm raw}).$$

An acceptable answer uses only permitted labels and cites evidence fields in
$E$. Unknown fields, unsupported citations, or output outside the schema
invalidate it. Schema compliance makes the answer well-formed; it does not
prove that its interpretation is correct.

After the required replay certificate $C_R$ is verified, a deterministic
selector $S$ applies the active policy to the current registry $G_t$, evidence,
and classification:

$$\Gamma=S(\operatorname{policy}_t,G_t,E,A).$$

The resulting proposal commits to the complete decision context:

$$
\bigl(h(P),h(E),h(Q),h(M),h(V_Q),h(A),h(S),h(\Gamma),C_R\bigr).
$$

The selector makes no model calls or external observations. Signature checks,
root matching, freshness requirements, concentration limits, and authorization
thresholds are exact gates. A model classification cannot override them.

A replay certificate authenticates agreement on the classification. A
governance certificate authorizes a state change. Neither certificate can
substitute for the other or establish facts that the underlying evidence does
not support.

### 8.3 Replay is profile-specific

A replay profile commits to the model, tokenizer, runtime, numerical and
quantization rules, hardware class, batching policy, and parser. The
reproducibility claim concerns that profile and its allowed inputs, not all
language models or all execution environments.

A replay certificate $C_R$ requires $q_R$ admitted replay signers to authenticate
the same evidence packet, profile, parser, and parsed-output commitments.
The selector consumes equality of the canonical parsed answer:

$$
\operatorname{root}(A)
=\operatorname{SHA3\text{-}384}\!\left(
\operatorname{canonicalJSON}(\operatorname{validate}_Q(A))
\right).
$$

Raw responses may differ in irrelevant formatting without changing this root.
Where a profile requires additional numerical commitments, those checks are
also prerequisites. Missing signatures, disagreement, or stale bindings leave
the classification unavailable for an action requiring it.

Distinct replay keys establish distinct signers. Independent operation needs
separate evidence; a collection of keys controlled by one provider does not
supply independent replay.

### 8.4 A worked admission decision

Consider a candidate with adequate reliability, an authenticated operator
manifest, and a distinct hosting location. Its evidence also establishes that
it shares a prohibited release manager and funding controller with an
incumbent.

The relevant question is whether apparent diversity establishes independent
control. A classification might label the evidence as cosmetic diversity and
cite the release-management and funding disclosures. The deterministic policy
nevertheless has a simpler decisive fact: confirmed prohibited common control.

The result is rejection, and the parent registry remains active. A favorable
classification cannot override the violation; unrelated missing evidence
cannot downgrade it to a hold. This example needs no model to determine the
outcome. A model is useful only where interpretation remains necessary, such as
unresolved source conflicts, and an unresolved classification cannot authorize
admission.

### 8.5 What replay establishes

Replay establishes that the admitted participants obtained the same
selector-facing answer under the declared conditions. It does not establish
the truth of the evidence, the correctness of the classification, or freedom
from shared model bias.

Profile evaluation must distinguish repeatability from decision quality.
Relevant questions include sensitivity to adversarial evidence, disagreement
across permitted environments, stability near classification boundaries, and
the consequences of an incorrect answer. Agreement on an error remains an
error.

### 8.6 Profile changes

Admitting a profile for a particular question class does not authorize its
successor. A proposed replacement requires comparison against the preceding
profile, published disagreement analysis, replay requirements, resource bounds,
and an authorized activation rule.

Where classification is mandatory, deleting it leaves the decision on hold.
A successor policy cannot silently convert that missing prerequisite into
permission. Historical decisions remain bound to their original profiles, and
any return to an earlier profile is a new authorized transition.

---

## 9. Post-Quantum Authorization

### 9.1 Authorization from genesis

Settlement value and public keys can outlive the cryptographic assumptions
under which they were created. A new ledger can choose post-quantum account and
validator authorization at genesis, avoiding dependence on a later migration
of those exposed keys.

PostFiat selects ML-DSA, standardized in FIPS 204, for that purpose. Validator
certificates identify the active committee and authenticate its distinct
signers. This choice addresses authorization under the signature assumption;
the classical privacy dependencies remain as described in §7.

### 9.2 Signature cost

ML-DSA-65 uses 1,952-byte public keys and 3,309-byte signatures. For an
illustrative 32-byte signer identifier, one quorum's signatures and identifiers
require

$$q(3309+32)\ \text{bytes}.$$

At $n=35$, $q=24$, this is 80,184 bytes; at $n=100$, $q=67$, it is 223,847
bytes. These are component-size calculations. Complete certificate costs also
include any transmitted public keys, framing, proposal evidence, voting phases,
and timeout ancestry.

The design must budget bandwidth and verification work for all required
certificates. Fixed signature sizes make part of that cost explicit; they do
not establish throughput or latency.

### 9.3 Cryptographic evolution and recovery

Signature selection balances security assumptions, bandwidth, verification
work, and operational robustness. An independent hash-based signature family
can diversify assumptions, but a recovery key is useful only if its commitment
and activation rules remain trustworthy when the primary family fails.

A safe cryptographic transition therefore requires authenticated successor
keys, an unambiguous activation boundary, and authorization under rules that
the assumed attacker cannot forge. Ordinary governance using only a broken
signature family cannot provide that authorization. Without an independently
secured recovery path, coordinated recovery may be necessary.

Changes to privacy proofs and note encryption are separate obligations.
A signature upgrade does not restore proof soundness, conceal previously
exposed data, or protect recorded classical ciphertext against later
decryption.

---

## 10. Failure Modes and Recovery

Invalid authorization, proofs, or transition bindings prevent the affected
action from taking effect. A certified block does not excuse an invalid
transaction. Rejection must preserve balances, spent-note history, and the
active trust configuration.

Loss of a normal consensus quorum halts progress. A stalled governance
proposal leaves the preceding configuration active. Neither condition permits
validators to infer a lower threshold or invent a new authority from elapsed
time.

Missing or disputed classification prevents any decision that requires it.
Correcting the evidence or adopting a new policy requires the applicable
review and authorization process; the classifier cannot resolve the dispute
by granting itself additional discretion.

Cryptographic failures have a different scope. Public pool accounting limits
outflow but cannot identify every counterfeit note or guarantee repayment.
A pause requires valid independent authorization and does not repair lost
soundness. If the authorization family itself fails, ordinary signatures under
that family cannot establish a trustworthy recovery decision. Recovery then
depends on an independently secured path or a coordinated change in the
system's trust assumptions.

---

## 11. Limitations

Genesis remains trusted. Public launch commitments can make its assumptions
auditable, but cannot establish independence that does not exist. Hidden
control relationships and changing economic incentives can invalidate an
otherwise internally consistent admission and fault model.

The Cobalt transition argument is conditional on complete quorum coverage,
valid fault bounds, and persistent compatible-history signing rules.
Preserving safety during a stalled transition does not guarantee liveness.
The economic case for subsidy-free validation likewise requires evidence of
sustained participation.

Shielded settlement retains public-boundary and metadata leakage, classical
proof and encryption assumptions, and the recovery limits described in §7.
Replay establishes reproducibility under a declared profile, not the truth of
a governance judgment. Post-quantum account and validator signatures protect
their authorization scope without extending that protection to every
cryptographic dependency.

---

## 12. Related Work

PostFiat builds on the XRP Ledger's model of known validators and
subsidy-free settlement, and on the overlap analysis of Chase and MacBrough.
MacBrough's Cobalt provides the foundation for agreement under declared trust.
This paper focuses on making changes to that trust explicit, authenticated,
and subject to bounded transition checks.

Stellar's SCP also expresses trust through local declarations. PostFiat's
emphasis is on the authorization and safety obligations of changing a committed
trust configuration. Its ordering design uses quorum certificates, persistent
locks, and view changes from the broader Byzantine agreement literature,
including PBFT and work on partially synchronous consensus. A prepare/precommit
design must justify its own commit and lock rules; a resemblance to another
BFT protocol does not supply that proof.

Shielded settlement draws directly on Zcash's Orchard action model, Halo2, and
public pool turnstile accounting. The privacy contribution concerns their
composition with issued-asset settlement, governance, and ledger authorization.
It does not require a new zero-knowledge proof system.

ML-DSA follows NIST's post-quantum signature standard. The classification
proposal draws on reproducible inference and deterministic parsing, with
replay treated as a bounded process guarantee. Across these components, the
protocol argument concerns how their assumptions and authority boundaries
compose.

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
