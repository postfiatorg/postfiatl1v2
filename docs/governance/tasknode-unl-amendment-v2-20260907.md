# Task Node UNL Amendment V2: evidence is not identity

**Date:** 2026-09-07. **Scope:** proposed shadow policy, not an activated admission rule. This is a separate amendment to the [published proposal](https://postfiat.org/research/deterministic-unl-task-node-cobalt/), not a revision of its locked text. No registry, consensus rule, token emission, or live signing authority changes here. A research lock means that the design is fixed for evaluation—not that its security claims have passed implementation tests.

## Decision

Do not settle the Task Node admission design on the present attack-defense table. Preserve V1 as the reproducible baseline; evaluate V2 beside it. V2 separates unsolicited observations from consented relationships, treats observed control changes as a new eligibility epoch, and makes a cluster-cap violation an explicit unresolved state rather than silently equating “no attacker admitted” with safety. It does **not** promise to detect a private sale of an otherwise unchanged account.

The immediate cost is real: removing unilateral funding edges also removes a useful, though unsafe, common-funder heuristic. V2 therefore remains shadow-only until a paired liveness/adversarial gate demonstrates that the replacement is worth that loss. Failing the gate means no promotion, not a lower threshold or an automatic return to V1 for live admission.

## 1. What the experiment actually established

The [attack simulation](tasknode-unl-attack-simulation-20260907.md) invokes the real accountability, edge, trust-graph and admission-policy modules. Its population is synthetic: 240 accounts, 20 incumbent seats, 17 non-Foundation seeds, a fixed window and a fixed seed vector. It is not a measurement of Task Node participants or a multi-window registry transition.

| Observation at V1 constants | Consequence for this amendment |
| --- | --- |
| Fourteen honest accounts can be admitted over successive one-add rounds. One selected aged-account purchase gains a seat. | Public work history is transferable if control changes invisibly. A binding signature proves key control, not continuity of a human operator. |
| A rival was funded before the window; one in-window unit from an incumbent denies it. | This is **majority-of-window inflow taint**, not replacement of the historical first funder. Quiet-window denominators make dust decisive. |
| Cross-cluster links can combine more incumbents than the two-seat cap permits. | The cap constrains additions; it is not preserved automatically when the partition changes. Existing seats cannot simply disappear. |
| Damping 0.90, ten walk steps, or a floor of 1/N each remove every honest admission. | Zero attacker admissions can mean total admission failure. The floor, finite walk length and damping must be evaluated together with honest controls. |

The disconnected ring, farmed identities and Foundation-supported favorite gain no seat within the reported sweeps. This supports only those bounded cases. A purchased bridge into the honest graph, unobserved collusion, false work judgments and changing future seeds are outside that evidence. The original “two sponsors in practice” explanation is withdrawn for V2: two outside vouches were not sufficient in this population.

**Evidence identity.** The original proposal SHA-256 is `319c1588c6575f6d9bd6c06c9fc1f055336b00cc394befd3cff54e146d68e584`; simulation `results.json` is `81e27d4c16d689e2a1361b76720abe10030faee0e8962efe6387fd543137c2bd`. Its committed `output-manifest.json` and `determinism.json` bind the supporting artifacts under `benchmarks/ai-governance/tasknode-unl-attack-simulation-20260907/`. Those are V1 results. No V2 admission result is claimed in this document.

## 2. Bought aged accounts: reset observable continuity, admit the limit

### Eligibility epochs

V2 introduces a separate **control epoch**, identified by a public binding-event digest. A declared transfer, validator-key replacement, wallet-binding replacement, or recovery starts a new epoch. Routine signing does not. The event must be authorized through the existing binding/recovery mechanism; an accusation or an unsolicited transfer cannot reset someone else's epoch.

A new binding must survive one complete 180-day evaluation window before its account is eligible for an addition. During that window it must independently satisfy the existing work, quality, standing and accountability floors using evidence dated after the epoch event. Pre-event history remains visible but does not satisfy the new epoch's admission score. A declared or recovered binding cannot inherit old endorsements: directed vouches require renewal, and co-work credit must concern work completed in the new epoch. Established unchanged bindings at the V2 shadow baseline must have a full verifiable window; missing continuity evidence yields `HOLD_CONTINUITY`, not an invented history.

For an incumbent, an observed rotation does not evict a functioning validator or disable its consensus key. It creates a reported continuity hold for admission-policy review. Operational key rotation and emergency recovery remain possible through their existing governed paths. Any later removal follows the existing registry/churn mechanism, not this scoring event. This asymmetry intentionally avoids turning a security-motivated key rotation into an immediate consensus-liveness failure.

### What this cannot detect

If a buyer receives the account and all existing keys without changing any public event, the buyer and the original operator can produce identical public transcripts. No deterministic rule over that transcript can distinguish them. V2 must therefore report an unchanged-key aged purchase as **undetectable by these inputs**, including when it passes admission. Fresh work can also be performed by the buyer; a cooling period adds delay, not proof of personhood.

Signed continuity statements, renewals and disclosed control groups make observable accountability clearer. They do not make accounts nontransferable. No biometric, hidden reputation feed or Foundation discretion is introduced as an unstated identity oracle. A policy requiring stronger personhood would need its own explicit trust model and review.

## 3. Unsolicited funding: observations cannot impose a veto

### Two evidence planes

V2 retains an **observation plane** containing V1 funding extraction and other public facts for audit and side-by-side comparison. First-funder and majority-inflow facts—including dust—remain inspectable. They do not, by themselves, affect eligibility mass, clustering, control-group membership, admission denial, continuity, or removal.

The **admission plane** contains only qualified relationships:

- A directed vouch requires the voucher's existing public signature plus the target's signed acceptance of that exact statement digest. Acceptance preserves direction and weight; it is not a reverse vouch. Either party can revoke it for the next window.
- A co-work unit retains existing evidence validation and additionally requires both bound accounts to acknowledge the exact work-unit digest. Repeated acknowledgements do not multiply credit; the existing three-unit pair cap remains.
- An explicit common-control declaration requires signatures from every member named in that declaration. It joins a separate control-group relation; it contributes **no positive walk weight**. Transitive group membership is computed from valid declarations, and a new declaration cannot name an unsigned third party into a group.

Each acknowledgement binds the chain and genesis identity, policy version, relation kind, statement digest, both endpoint account IDs, their control epochs, effective window and expiry. Signatures use the existing public-binding custody boundary, with a new domain-separated statement type. Missing signatures, wrong epochs, unknown versions and replayed acknowledgements are rejected. The implementation must freeze closed schemas, canonical encoding, bounds and golden vectors before any V2 evaluation can be considered reproducible.

New or renewed relations become effective only at a window boundary after ledger finality; no mid-window graph mutation occurs. A revocation removes future positive trust, but does not erase prior public declarations. An acknowledged common-control relation stays active through one full window after a jointly signed dissolution; unilateral repudiation does not immediately split a previously declared group. A compromised key can still sign damaging statements: this is key compromise, not the unsolicited-dust guarantee.

### Admission consequences and trade-off

An addition must pass both the social-cluster cap and the V1-style one-seat-per-control-group restriction, using **declared control groups** rather than inferred inbound funding links. Existing multi-seat groups are reported and held against further additions; they are not automatically expelled. Malformed declarations are individually rejected and reported, not allowed to poison an otherwise valid global snapshot. Missing or invalid snapshot commitments, in contrast, make the whole round unverifiable and produce no proposal.

A minimum dust value, count, or longer funding denominator is not the repair: each merely changes the price of unilaterally tainting another person. The required invariant is stronger: adding transfers to the observation plane alone cannot change an admission result. Even a large unsolicited transfer has no automatic veto authority.

This deliberately sacrifices detection of common funding that recipients refuse to acknowledge. Two secretly coordinated accounts may appear independent. Social mixing and disclosed group constraints still apply, but V2 must not claim the original common-funder defense without new evidence. Consent also creates friction: missing acknowledgements reduce edges and can harm honest admissions. Both costs belong in the evaluation, not in a footnote after deployment.

## 4. Reclustering: a cap breach is a state, not a removal instruction

At each 180-day window boundary, compute a frozen admission graph, a frozen non-Foundation seed vector from the ratified opening registry, the opening list size N, and a canonical partition using the existing exact-rational walk and deterministic conductance sweep. Keep damping 17/20, twenty steps, conductance threshold 1/10, vouch weight one, co-work weight one with cap three, and connectivity floor 1/(2N). The walk is a specified finite iteration, not a claim of convergence to the mathematical stationary distribution.

For every proposal within the window, recount the **current** incumbent seats in that frozen partition. Newly admitted validators consume seats immediately but do not become seeds until the next window. New relationships also wait for the next boundary. This prevents an accepted addition from escaping the cap through stale opening seat counts while preserving a stable graph for the current round.

Let the social cap be `max(2, N/10)` with exact rational comparison, as in V1. Integer occupied seats must not exceed that value after an addition; do not silently round the cap upward. Track three distinct outcomes:

1. `CLEAR`: no incumbent breach; an addition still needs all other gates.
2. `SATURATED`: at the limit for integer additions; no additional seat fits.
3. `EXISTING_BREACH`: current occupancy exceeds the limit before an addition. Record the member set, seat IDs, excess count and causative relation digests. Admit no further member of that cluster or declared control group.

Unaffected clusters may still produce additions. A local cap breach must not automatically halt all admission or remove honest validators elsewhere. Conversely, the report must never label the registry “cap clean” merely because no new seat was added.

### Repair and appeal

A breach opens a public review record. Disputed relationship evidence can be corrected only through its signed revocation/dissolution or an independently authorized correction with explicit public evidence; an unsigned complaint changes nothing. An authorized correction is not an informal override: until a versioned correction procedure is specified and ratified, the relation's existing rules remain binding. If a breach persists through a full window, the report proposes either staged remediation or continued unresolved status. It does not choose identities to evict automatically.

Any removal still requires the existing public rationale, registry authorization, one-add-or-remove pre-39 churn constraint, and old-root overlap of at most one round. A reviewed transition must show that the active committee, quorum and replay rules remain valid at every step. Where these conditions conflict with immediate cap repair, **preserve the registry and report the violation**. This is a candid temporary concentration risk, not a proof that the cap is an invariant. A cap-repair/removal selector is a separate prerequisite for live activation, not a hidden implementation detail delegated to an operator.

Consenting to a vouch or co-work link can still merge social clusters and create a hold. V2 prevents an unconsented edge from doing so; it does not promise that every socially useful bridge preserves cap headroom. That tension is a required liveness measurement.

## 5. Honest-admission liveness is a release gate

No constant changes automatically to make an attack chart look better. V2 keeps the numerical constants above for its first experiment; changing evidence semantics is already a substantial perturbation. A constant change requires a separate version, preregistered comparison and renewed evaluation. A zero-honest result cannot pass as improved Sybil defense.

Before implementing V2, freeze the fixture generator, attack budgets, expected control labels, parameter grid and metric definitions in a content-addressed experiment manifest. Report every trial, not just a favorable topology. The minimum paired suite is:

| Test | Required observation or acceptance condition |
| --- | --- |
| Baseline replay | V1 reproduces the committed outputs byte-for-byte. V2 is a separate implementation and output namespace. |
| Quiet-window taint | The original one-unit attack, and larger repeated unsolicited inflows, leave the target's V2 result, mass, cluster and control group unchanged. The audit plane still records them. |
| Control changes | Declared sale, binding rotation and recovery cannot reuse pre-epoch eligibility. Hidden unchanged-key sales are explicitly counted as unresolved attacks, never scored as detected. |
| Reclustering | Every incumbent breach is reported; affected additions are blocked; a qualifying candidate in an unaffected cluster can proceed. No seat is silently removed or omitted from counting. |
| Original honest controls | In a paired consent-complete version of the original fixture, all fourteen originally admissible honest controls remain admissible across the corresponding one-add rounds. If not, report the losses and do not promote this version. |
| Consent availability | At 100%, 75%, 50% and 0% acknowledgement availability, report honest admission count, time to eligibility, held reasons and community distribution. Missing consent is never fabricated to satisfy the preceding row. |
| Attack expansion | Repeat the original budget sweeps, then add purchased seed-connected bridges, undeclared common funding, colluding accepted vouches, relation revocation and deliberate cluster merging. Report best seats at or below budget, not only exact-budget seats. |
| Parameter sensitivity | Repeat the original low/base/high one-at-a-time grid, plus the Cartesian product of damping {0.75,0.85,0.90}, steps {10,20,40} and floor {1/N,1/(2N),1/(4N)}. Report honest and attacker outcomes together. |
| Multi-window churn | Run at least three complete windows, updating N, non-Foundation seeds, relation expiries and eligibility epochs at each boundary; exercise additions, removals, rotation and persistent breaches. Fixed-window results cannot substitute. |

Repeat paired trials on the original topology and on at least thirty published integer-seeded variants varying community size, density, Foundation concentration and cross-community bridges. Include adversarially selected sparse and near-threshold cases, not only random graphs. Exact floor equality passes; an infinitesimal exact-rational deficit fails. Include dangling nodes, empty eligible seed sets and duplicate facts; empty seeds produce no proposal, not uniform mass over attacker accounts.

For each trial report honest admission rate over the predeclared eligible control set, waiting windows, zero-admission windows, attacker seats, unsolicited-denial count, incumbent breach count/duration and group concentration. Separate a detected declared sale from an undetectable one; separate admission from incumbent retention. Replay each input with reordered facts and in two independent runs, requiring identical canonical outputs.

The fourteen-control condition is a conservative regression floor, **not** a population guarantee. Broader live-admission service levels and tolerable residual capture risk cannot responsibly be inferred from this synthetic population. They must be ratified using representative, consented public evidence before activation. Without that evidence and the missing cap-repair transition, the release decision remains shadow-only even if every test and the text-scoring gate passes.

## 6. Authority, migration and accountable claims

The unchanged MVP modules under `python/postfiat_rpc/tasknode_unl_*.py` remain V1. V2 requires a new schema/policy identifier and explicit selection; old inputs are never silently reinterpreted. Shadow reports must bind the policy digest, input root, registry root, window, graph and declaration roots, constants, per-candidate reasons and unresolved limits. Mismatched versions or roots yield no proposal.

The work-score formula, public digest provenance, binding signature validation, Foundation seed exclusion, Cobalt ratification, registry signature quorum and token economics are not changed here. In particular, reproducible Task Node scores establish faithful processing of supplied judgments—not that the Foundation's underlying judgments or private payload reviews were correct.

The next authorized work is a Task Node-generated implementation milestone for the locked amendment, followed by a Python CLI and then a user-facing report. Those surfaces must distinguish `SHADOW_ONLY`, `HOLD_CONTINUITY`, cap breaches, admission denial and unknown sale/control status without a misleading all-green badge. No such implementation or milestone is claimed complete here.

Live promotion requires separate approval of the wire schemas, acknowledgement UX/custody, representative liveness thresholds, residual hidden-control risk, and a safe cap-repair/removal policy, followed by registry/governance authorization. Neither a model's favorable review nor a successful synthetic experiment supplies that authority.

**Bottom line:** V2 aims to remove a unilateral griefing mechanism and make known concentration and liveness failures explicit. It cannot turn transferable work history into proof of independent human operators. Until those limits are measured and accepted, Task Node is admission evidence and decision support—not an autonomous validator-membership authority.
