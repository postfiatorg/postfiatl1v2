# Feedback — Pillar 10 — Information Hive Mind

*Review round 2.*

> **Mean score:** 72/100 · Gemini 4 Neon 75 · Claude Opus 5.5 70
>
> **Method:** each model received (1) the founder's original prompt and (2) [this pillar's spec](../../pillars/10-information-hive-mind.md), once, with the question below. Run 2026-11-26 20:38 UTC via OpenRouter.

## The question

> On a scale from 0-100, to what extent does this sub-page correspond to the founder's intention, and how likely is it to output something that works for the founder and the project — with 'works' meaning technically robust and economically coherent?
>
> Output your reasoning at length, enumerating problems clearly so that subsequent agents can investigate and improve.

## Gemini 4 Neon — 75/100

*Model `google/gemini-4-neon` · 14192 output tokens*

#### Summary judgement

This is a thoughtful, well-grounded Round 2 specification that faithfully captures the founder's vision for Deliverable 10 and resolves many of the most dangerous failure modes of an initial draft—most notably MNPI/confidentiality boundaries (R6–R9), private-context leakage from Deep Memory (R4–R5), Finance Hub access gating against prompt injection (R11, AC5), and genuine multi-contributor pooling rather than isolated Q&A (R14). It is also unusually honest about the current codebase, accurately distinguishing live systems from migration-only tables (`158_finance_agent.sql`) and explicitly marking where it interprets the founder's truncated final sentence.

However, the spec is **not yet ready to hand directly to engineering or launch economically**. Beneath its polished structure sit three critical gaps: (1) an unresolved economic paradox where restricting questions strictly to public data (R6) makes the system trivially farmable via AI web search while offering weak incentives to genuine domain experts; (2) an unspecified check-event resolution engine paired with a payout loophole in R18 that lets contributors earn full rewards before claims are verified; and (3) a technical contradiction between pre-storage MNPI refusal (R8) and persistent chat / third-party LLM processing (R10). Add to that privilege-boundary mixing on `158_finance_agent.sql`, a missing handoff from claim clusters to model proposals, corroboration vulnerabilities, and 12 unprioritized founder blockers with no MVP phasing, and the result is a strong conceptual blueprint that still requires targeted systems and mechanism design before build.

---

#### Correspondence to the founder's intention

**Strong (high alignment, with two subtle drifts).**

The sub-page maps directly to all four sentences of Deliverable 10:
- *"done a large number of tasks on the task node"* → Eligibility gates built on live Expert badges (`expert-badge.js`) and badge policies (`072_network_badges_identity_approvals.sql`).
- *"classified to contribute specific types of information ... via chat"* → Two-axis classification (knowledge areas + information types, R2) feeding Task Node chat interviews (R10).
- *"agent who is ramped up on a company's history can triangulate with you based on your experience"* → Lazy, cached company briefs (R11) plus an explicit two-phase interview where Phase 1 triangulates observability and recency before Phase 2 asks targeted model questions (R10).
- *"The coverage universe in finance hub is linked to specific users"* → Quoted verbatim without fabricating punctuation, clearly labeled as interpretation, and preserved as Open Question 1.

Two subtle drifts from the founder's broader framing remain:
1. **Crowd rotation vs. persistent user-company stewardship.** The spec commits its architecture to algorithmic matching with rotating invitations (R3) and anonymous claim pooling (R14), treating persistent user-to-company linkage only as an external "model owner" assumption or an open question. If the founder intends top Task Node contributors to *become* ongoing domain scouts/stewards linked to specific companies in Finance Hub, R3's rotation actively works against longitudinal specialization.
2. **Post-human information packets feel bolted on.** The founder's overarching thesis is that Post Fiat makes post-human activity legible through standardized, machine-native information packets uploaded and acted upon (Deliverables 1 and 2). Here, Pillar 2 packets appear late (R17) with a fallback to raw database rows and no defined claim-packet schema, key topology, or block-explorer representation.

---

#### Technical robustness

**Moderately strong foundation, but several architectural seams break under implementation scrutiny.**

What works well:
- **Repo grounding:** Real migrations, workers, and boundaries (`024_network_task_profiles.sql`, `072_network_badges_identity_approvals.sql`, `158_finance_agent.sql`, `hive-group-worker.js`, `recommended-connections.js`) are accurately referenced.
- **Privacy and gating design:** The "contributor brief" firewall (R4) and access-tiered company briefs (R11) are clean, testable defenses against both Deep Memory leakage and prompt-injection extraction of gated Finance Hub numbers.
- **Cost instrumentation:** Lazy agent instantiation, cached per-publish briefs, turn/token caps, and `cost_usd` logging (R10–R11) show sound operational awareness.

Where technical robustness breaks down:
- **MNPI screening vs. chat persistence & LLM telemetry (R8 vs. R10):** Running interviews on `finance_agent_threads` / `_messages` while guaranteeing refused content "never enters the claim store, agent context or reviewer queues" leaves unanswered how messages are intercepted pre-write, how thread state stays coherent after a refusal/redaction, and whether refused MNPI is still transmitted to and logged by third-party LLM APIs.
- **Privilege mixing and broken proposal pipeline (`158_finance_agent.sql`, R10, R15, R16):** Reusing operator model-editing tables (`finance_agent_threads`, `_messages`, `_proposals`) for untrusted contributor interviews creates a confused-deputy risk. Worse, R15–R16 assume model owners approve `finance_agent_proposals` created from contributor insights, yet neither the contributor nor the Task Node reviewer agent has access to the sealed model required to author a model-edit diff.
- **Unbuilt verification/oracle subsystem (R12, R18):** Check events (`verified`, `falsified`, `unresolvable`) sit at the center of rewards and track-record weighting, yet no component is specified to track check dates, ingest outcomes, adjudicate disputes, or reconcile months-long deferred states with `task-review-reward-worker.md`.
- **No extraction sign-off before cryptographic signing (R10, R12, R17):** Claims extracted by an LLM from free-form chat are apparently stored, signed with Pillar 6 keys, and staked against contributor reputation without an explicit contributor review-and-sign step.
- **No phased delivery plan:** R1–R22 form a single tightly coupled system spanning five pillars and twelve open founder decisions, with no clear MVP cutline.

---

#### Economic coherence

**Conceptually improved over naive pay-per-answer designs, but still vulnerable to farming, adverse selection, and missing budget closure.**

Moving rewards from instant LLM approval to deferred verification (R18), enforcing caps in code (R19), and requiring an ROI/accuracy dashboard (R20) are strong instincts. Nevertheless, the economic mechanism is not yet incentive-compatible:

1. **The AI-slop / adverse-selection squeeze:** R6 rightly forbids non-public/confidential info, restricting interviews to public prices, open datasets, bought products, open-source code, and on-chain data. That creates a severe asymmetry: a low-effort contributor can paste Phase 2 questions into an LLM with web search and submit fluent, cited, checkable claims at near-zero marginal cost to farm PFT. Meanwhile, a genuine operator or domain expert with costly, high-signal mosaic observations faces attestation friction, capped fixed payouts, zero upside participation, and months-long payout deferral—giving the best contributors little reason to participate.
2. **Premature payout loophole (R18):** Releasing the main reward upon *either* check-event confirmation *or* citation in an applied model change decouples payout from truth. A persuasive contributor who gets a plausible-sounding wrong claim cited in a model change collects the full reward immediately; if the claim is later falsified, R18 explicitly says it merely "releases nothing further and lowers weight."
3. **Corroboration gaming & cold-start sparsity:** Rewarding corroboration across "independent" contributors (R13–R14) fails to distinguish independent real-world observation from two people parroting the same viral tweet/Substack post, or colluding accounts bypassing recommend-only Sybil review. Conversely, in an early network with sparse coverage across dozens of companies, most genuine claims will have zero second contributors, starving the corroboration mechanism at launch.
4. **No economic closure:** The spec describes PFT outflows and cost tracking (R18–R20) but leaves the funding source and value-capture loop entirely open (Open Question 8), with no link to Finance Hub access economics, validator staking, or NAVCoin fees.

---

#### Problems

##### P1 — AI-slop farming vs. adverse selection of genuine experts
- **Severity:** critical
- **Where:** Technical requirements (R6, R10, R13, R18); User story (steps 4, 6).
- **What is wrong:** R6 confines interview questions to publicly observable market data, open-source code, and on-chain activity. At the same time, R18 offers capped, deferred PFT payouts. Nothing in the spec prevents a contributor from pasting the agent's Phase 2 questions into an external LLM with web search to generate polished, plausible claims ("AI slop"), nor does anything explain why a genuinely informed domain operator would trade time and compliance friction for small, months-delayed rewards.
- **Why it matters:** If the marginal cost of LLM-assisted farming is lower than the expected PFT payout while real experts self-select out, the hive mind becomes an expensive, slow wrapper around public web search that degrades rather than improves Finance Hub models.
- **How an agent should investigate:** Benchmark Task Node's current average task payout and contributor demographics (from `task-review-reward-worker.md` and `024_network_task_profiles.sql`) against what an off-the-shelf web-search LLM already produces for sample Finance Hub open questions. Check whether existing Task Node anti-LLM / proof-of-work-evidence mechanisms can be reused.
- **Suggested direction for fix:** (1) Add a **baseline-novelty gate** in R13/R15: the company agent or reviewer agent runs its own public web/on-chain search baseline first, and claims already recoverable from top public search results receive zero or near-zero novelty score. (2) Focus Phase 2 on **primary-observation evidence** that web-search LLMs cannot fabricate easily (e.g., signed on-chain transactions/addresses, dated receipts/quotes from public marketplaces, reproducible benchmarks, primary channel checks with verifiable methodology). (3) Differentiate rewards by **information value / surprise** (non-consensus claims that later verify earn significantly more than consensus restatements), and consider non-monetary incentives that experts actually value (e.g., earned Finance Hub access, reputation/leaderboard standing, or fee-share alignment).

##### P2 — Premature-payout loophole and missing check-event resolution engine
- **Severity:** critical
- **Where:** Technical requirements (R12, R18); Acceptance criteria (AC7); What exists today (`task-review-reward-worker.md`).
- **What is wrong:** Two tightly coupled flaws exist in the deferred-reward design:
  1. **Unbuilt oracle/resolution workflow:** R12 defines `check event` and terminal statuses (`verified`, `falsified`, `unresolvable`), and R18/AC7 tie main payouts to them—yet no service, schedule, human role, or dispute process is specified to monitor target dates, evaluate outcomes, or transition claim states. It is also unverified whether `task-review-reward-worker.md` supports tasks/verifications that stay open for 3–12 months or split two-tranche payouts.
  2. **Payout loophole:** R18 states the verification event is *"the check event confirming the claim, **or** the claim being cited in an applied model change."* Paying the full main reward upon model citation—before the check event occurs—means a persuasive but wrong claim still extracts the full payout, since later falsification only "releases nothing further."
- **Why it matters:** Without a concrete resolution pipeline, deferred rewards will stall indefinitely in `pending`/`accepted` limbo. With the citation-equals-payout loophole, contributors are incentivized to optimize for *persuading the model owner* rather than *being right*.
- **How an agent should investigate:** Inspect `task-review-reward-worker.md` and its underlying task/reward migrations to see how verification timeouts, multi-stage payouts, and long-lived pending rewards are handled. Review Finance Hub's model update/earnings-ingest pipeline (Pillar 9) to determine what structured public events (earnings dates, protocol upgrades, on-chain metrics) can actually be resolved automatically vs. manually.
- **Suggested direction for fix:** (1) Split rewards strictly by claim type: if a claim has a dated check event, citation in a model change unlocks at most an intermediate tranche (or zero extra payout), while the main tranche unlocks **only** when the check event resolves `verified`; reserve citation-only payout strictly for explicitly tagged `unresolvable` / structural-context claims (subject to tighter caps). (2) Specify a dedicated **Resolution Worker & Adjudication Flow**: a scheduled job triggered by `check_event_date` that pulls the relevant earnings/on-chain/public release, drafts a resolution recommendation via the reviewer agent, requires model-owner or operator sign-off (with a contributor appeal window), and writes a terminal status + payout trigger. (3) Confirm or extend Task Node's reward worker to support deferred milestone payouts instead of assuming existing single-shot task verification works out of the box.

##### P3 — Contradiction between pre-storage MNPI refusal/redaction and persistent chat / LLM logging
- **Severity:** critical
- **Where:** Technical requirements (R8, R10); Acceptance criteria (AC3).
- **What is wrong:** R8 guarantees that every answer is screened *before storage* and that refused content *"never enters the claim store, agent context or reviewer queues."* Yet R10 runs interviews directly on `finance_agent_threads` and `finance_agent_messages`. The spec does not define: (a) how user messages are intercepted and screened *before* being written to `finance_agent_messages` or server/APM logs; (b) what gets persisted in the thread so UI history and multi-turn context stay intact without retaining the refused text; (c) whether third-party LLM APIs used for screening or chat have zero-data-retention (ZDR) agreements so refused MNPI isn't stored by model vendors; (d) whether `redact` discards the raw input pre-persistence and requires contributor re-approval; and (e) how the system behaves fail-closed on ambiguous classifier outputs or repeat offenders.
- **Why it matters:** If an LLM chat controller writes user input to `finance_agent_messages` first (the standard pattern) or sends it to a non-ZDR LLM endpoint, the legal guarantee in R8 is violated on day one, creating serious securities-law and confidentiality liability.
- **How an agent should investigate:** Inspect `158_finance_agent.sql` and Task Node's existing chat ingestion, logging, and LLM client middleware (`server/` LLM wrappers) to check where raw user prompts are logged, persisted, and sent (and whether ZDR endpoints or self-hosted models are used).
- **Suggested direction for fix:** Specify an explicit **pre-persistence screening gateway**: (1) Incoming interview messages pass through a synchronous, fail-closed screening stage (deterministic keyword/pattern rules + a dedicated screening model on a ZDR or self-hosted endpoint, with request-body logging disabled for interview routes) *before* any database write. (2) On `refuse`, discard the raw payload in memory, write only a sanitized tombstone marker to `finance_agent_messages` (never re-injected as raw content into future prompts) plus the hash-only compliance audit record, and enforce an automatic cooldown/lockout after repeated refusals. (3) On `redact`, persist only the redacted text after the contributor explicitly confirms it. (4) Expand AC3 to verify zero raw refused strings exist in `finance_agent_messages`, application logs, and LLM prompt replays.

##### P4 — Privilege mixing on `158_finance_agent.sql` and broken handoff from claims to model proposals
- **Severity:** major
- **Where:** What exists today; Technical requirements (R10, R12, R15, R16).
- **What is wrong:** Two architectural defects stem from over-reusing migration 158:
  1. **Confused-deputy / privilege boundary mixing:** `158_finance_agent.sql` was built for an allowlisted operator interacting with an agent that reads a sealed model copy. Storing untrusted/semi-trusted contributor interviews in the exact same `finance_agent_threads` and `_messages` tables creates a high-risk privilege-escalation surface if any query, cache, or research-host pull fails to filter strictly by thread type and access tier.
  2. **Missing translation hop:** R15 says *"The model owner decides only on model changes, via `finance_agent_proposals`,"* and R16 says *"The model host pulls approved proposals and accepted clusters."* However, neither the contributor nor the Task Node reviewer agent has access to the sealed model (R11 explicitly restricts them to a brief). So **who authors the concrete model-edit diff inside `finance_agent_proposals`** from an accepted cluster of contributor claims?
- **Why it matters:** Mixing privileged operator threads with untrusted contributor threads risks leaking gated Finance Hub models (violating AC5). Leaving the cluster-to-proposal hop undefined means the loop from "accepted claim" to "applied model change and CHANGELOG citation" (AC7) cannot actually be implemented.
- **How an agent should investigate:** Read `server/db/migrations/158_finance_agent.sql` in full, checking its columns, constraints, foreign keys, and comments regarding the research host's service token and proposal schema. Also inspect Pillar 9's research-host model pipeline docs to see how model edits are currently drafted and applied.
- **Suggested direction for fix:** (1) Keep contributor interviews in dedicated, physically isolated tables (e.g., `hive_interview_threads`, `hive_interview_messages`, `hive_claims`, `hive_claim_clusters`) rather than sharing operator tables from 158—reusing only shared code patterns/helpers. (2) Define the two-stage handoff explicitly: Task Node exports **accepted claim clusters** to the research host; the **research-host model agent** (which holds the sealed model) reads those clusters and generates a concrete `finance_agent_proposal` (model diff + cited `claim_id`s); the **model owner** then approves or rejects that proposal, triggering the CHANGELOG citation.

##### P5 — Monolithic scope with no phased rollout (MVP) and unprioritized founder blockers
- **Severity:** major
- **Where:** Architecture; Technical requirements (R1–R22); Dependencies; Open questions for the founder (1–12).
- **What is wrong:** The spec presents 22 interconnected requirements spanning five external pillars (Pillars 2, 5, 6, 9, plus research-host model pipelines) and ends with 12 unranked open questions—at least six of which (Q1, Q2, Q3, Q4, Q8, Q10) are fundamental legal, economic, or architectural blockers. There is no MVP cutline, no phased delivery plan, and no default recommendation for each open question.
- **Why it matters:** Engineering teams cannot start building cleanly when every component is coupled to unfinished pillars (e.g., Pillar 2 packets, Pillar 6 on-chain identities, Pillar 5 shielded payouts) and unanswered legal/economic decisions. Attempting to build all 22 requirements at once maximizes schedule risk and delays learning whether contributors actually produce useful insights.
- **How an agent should investigate:** Cross-check the readiness status of Pillars 2, 5, 6, and 9 in the surrounding planning docs (`02-message-protocol.md`, `05-local-task-node.md`, `06-hive-identity.md`, `09-finance-hub.md`) to map real dependency timelines against what Task Node can ship standalone today.
- **Suggested direction for fix:** Add a **Phased Delivery Plan** (e.g., *Phase 0: Legal sign-off + protocol/token-only pilot with manual resolution and DB-backed claims*; *Phase 1: Automated matching, claim clustering, research-host proposal bridge, and R20 value dashboard*; *Phase 2: Pillar 2 encrypted packets, Pillar 6 signing, Pillar 5 shielded payouts, and expanded asset classes*). Reorganize the 12 open questions into **Launch Blockers (must decide before Phase 0)** vs. **Deferrable Policy Choices**, attaching a concrete **recommended default** to each so the founder can approve by exception.

##### P6 — Corroboration gaming (shared sources & collusion) and cold-start sparsity
- **Severity:** major
- **Where:** Technical requirements (R3, R13, R14, R19); Acceptance criteria (AC6); What exists today (`sybil-review-detection.md`).
- **What is wrong:** R13 and R14 elevate claims when corroborated by "independent contributors" and weight clusters by contributor track records. Two problems arise:
  1. **False independence:** Two honest contributors citing the same viral X thread, Discord post, or sell-side note look like "two independent supporters," inflating cluster confidence without adding independent evidence. Meanwhile, Sybil review is explicitly *recommend-only*, leaving colluding accounts or coordinated Discord groups able to cross-corroborate claims before manual review catches up.
  2. **Cold-start sparsity:** Early in deployment (e.g., 20–50 active validators/top contributors across dozens of covered companies), most companies will have only 0–1 eligible contributors. Track records start empty, and genuine single-source insights may never get corroborated.
- **Why it matters:** Treating unverified overlap as independent corroboration amplifies herd noise and rewards collusion, while over-relying on corroboration/track-record weighting causes the scoring model to stall during early adoption.
- **How an agent should investigate:** Review `sybil-review-detection.md` to see what signals are already computed (timing, device/wallet graphs, text similarity) and estimate the actual count of current Expert-badge holders and domain overlap from `024_network_task_profiles.sql` and `expert-badge.js`.
- **Suggested direction for fix:** (1) Require every claim to record **evidence provenance/source type** (primary observation, on-chain data, public filing, secondary media); the reviewer agent should only grant a corroboration boost when supporters rely on **distinct primary evidence** rather than identical secondary links or near-duplicate phrasing. (2) Automatically flag sudden bursts of near-identical claims for Sybil/collusion review *before* boosting score or paying rewards. (3) Define explicit **cold-start defaults** in R3/R14 (e.g., focus the pilot on a narrow 5–10 company/protocol subset with high contributor density, and allow single-contributor high-evidence claims to advance to model-owner review without waiting for a second contributor).

##### P7 — Missing funding loop and economic closure
- **Severity:** major
- **Where:** Technical requirements (R18–R20); Open question 8.
- **What is wrong:** The spec defines caps (`max_payout_pft`), deferred payouts, and a spend-tracking dashboard (R20), but leaves the funding source, emission budget, and value-return mechanism completely undefined (deferred to Open Question 8). There is also no budget allocation rule across companies (e.g., how much PFT a high-priority covered company gets per quarter vs. a low-priority one) and no explicit link between Finance Hub / NAVCoin value creation and hive-mind funding.
- **Why it matters:** Without a bounded budget envelope and a clear tie to protocol revenue or treasury allocation, the hive mind risks becoming an unbounded PFT emission sink whose spend scales with contributor activity rather than research value delivered.
- **How an agent should investigate:** Check Pillar 3 (Validator Rewards), Pillar 7 (NAVCoin Fees), Pillar 9 (Finance Hub), and Task Node's existing `payout_policy_json` usage in `072_network_badges_identity_approvals.sql` to see how PFT budgets and per-program caps are structured elsewhere in Post Fiat.
- **Suggested direction for fix:** Add a **Program Budget & Allocation Requirement** alongside R19: define a hard per-epoch global PFT budget for the Hive Mind, per-company bounty/budget caps weighted by Finance Hub priority (set in the coverage manifest), and a circuit breaker tied to R20 metrics (e.g., automatically tightening caps if verified-claim rate or model-citation rate drops below threshold). Explicitly propose funding options in Open Question 8 (e.g., dedicated Task Node research emission slice vs. validator/NAVCoin fee recycle) with a recommended default.

##### P8 — Missing contributor review and canonical sign-off before signing and staking reputation
- **Severity:** minor
- **Where:** Technical requirements (R10, R12, R17, R18).
- **What is wrong:** Interviews occur as multi-turn natural-language chat (R10), after which structured claims (text, stance, evidence, check event) are stored (R12), signed with the contributor's Pillar 6 identity or pseudonymous key (R17), and used to update their track record (R18). There is no step where the agent presents the **extracted canonical claim and proposed check event** back to the contributor for explicit confirmation and cryptographic signature.
- **Why it matters:** If an LLM misinterprets a nuanced chat answer, exaggerates confidence, or attaches an imprecise check event, the contributor's identity is attached to a claim they didn't intend—and a subsequent `falsified` status unfairly penalizes their track record. Technically, client-side Pillar 6 signing is impossible without presenting a canonical payload to the user.
- **How an agent should investigate:** Check how Task Node currently handles user confirmation and signing of structured task submissions or context edits (`context_edit_jobs_v1.xml`, Pillar 6 signing flows).
- **Suggested direction for fix:** Add a **Phase 3 (Claim Confirmation & Signing)** step to R10/R12: at the end of an interview, the agent displays 1–3 structured claim cards (claim text, stance, evidence summary, proposed check event, attribution mode). The contributor can edit, drop, or explicitly approve/sign each card; only confirmed cards enter the claim store and Pillar 2 packet pipeline.

##### P9 — Underspecified Pillar 2 encryption topology, pseudonymous credentials, and relationship privacy
- **Severity:** minor
- **Where:** Technical requirements (R7, R12, R17, R21, R22).
- **What is wrong:** Three privacy/crypto details are left vague:
  1. R17 says accepted claims are "encrypted by default," without specifying **encrypted to whose public key(s)** (Task Node operator? Research host? Finance Hub validator group key?) or how decryption works when the membership set changes.
  2. R17 and R21 allow signing with a "pseudonymous key" paired with a verified credential ("12 checked claims in GPU markets"), without specifying how that credential is attested or bound to the pseudonymous key without linking back to the contributor's primary Pillar 6 identity on-chain.
  3. R7/R12 record the contributor's declared relationship (e.g., `customer`, `supplier`, `former employee`), but R21/R22 never specify whether that relationship tag is hidden or shown publicly in *Contributor insights* on Finance Hub—potentially de-anonymizing pseudonymous contributors or exposing them to employer scrutiny.
- **Why it matters:** Ambiguous key topology stalls Pillar 2 integration, and leaking declared relationships alongside pseudonymous claims undermines R21's pseudonymity promise.
- **How an agent should investigate:** Review `02-message-protocol.md` (Pillar 2 encryption/key-exchange spec) and `06-hive-identity.md` (Pillar 6 identity derivation) to align key management and credential proofs with existing protocol primitives.
- **Suggested direction for fix:** Specify in R17 that: (a) claim packets are encrypted to a dedicated research-host / Finance Hub ingestion key (with key rotation documented); (b) in Phase 1, pseudonymous credentials ("12 checked claims in GPU markets") are operator-attested badges bound to a per-contributor pseudonymous keypair (with zero-knowledge/unlinkable proofs deferred until Pillar 5/6 maturity); and (c) in R21, declared relationships are strictly internal to compliance and reviewer triage, never displayed verbatim on public/member Finance Hub pages (at most coarsened to non-identifying information types like "market participant" with user opt-in).

##### P10 — Unresolved architectural tension between rotating invitations and persistent user-company linkage
- **Severity:** minor
- **Where:** User story; Technical requirements (R3, R15); Open question 1.
- **What is wrong:** While Open Question 1 commendably flags the ambiguity of *"The coverage universe in finance hub is linked to specific users,"* the spec's technical requirements (R3: *"Invitations rotate, and concurrent interviews per contributor are limited"*) already hard-code an ephemeral, rotating crowd-interview model. There is no representation of a persistent **user-to-company beat / specialist link** where a proven contributor tracks a specific company across multiple quarters and accumulates longitudinal memory with that company's agent.
- **Why it matters:** In financial research, the highest-value insights rarely come from one-off cold interviews; they come from contributors who follow a company or protocol continuously over quarters and notice subtle inflection points. Hard-coding rotation risks designing away the exact "linked to specific users" workflow the founder may want most.
- **How an agent should investigate:** Check Finance Hub's coverage schema (Pillar 9) and `024_network_task_profiles.sql` (`domain_expertise` already maps users to 5–10 specific public companies) to see how persistent user-company links could be modeled cleanly alongside open matching.
- **Suggested direction for fix:** Design the user-company link table in R1–R3 to support **both** modes cleanly via a `link_type` enum (`matched_candidate`, `recurring_contributor`, `designated_steward`). Rotate invitations for *new* discovery while explicitly allowing high-track-record contributors to opt into persistent **beat/steward links** on 1–3 covered companies with recurring quarterly check-ins before earnings or major protocol milestones.

## Claude Opus 5.5 — 70/100

*Model `anthropic/claude-opus-5.5` · 8596 output tokens*

#### Summary judgement

This is a careful, well-grounded spec. It inventories real code paths, separates the founder's words from its own interpretation, and catches the largest real-world risk the founder's prompt leaves out: soliciting material non-public information (MNPI) about listed companies. Its core mechanics match Deliverable 10 well:

- an agent ramped up on each company;
- a triangulation phase that first assesses the person;
- eligibility based on task history;
- pooling claims per company.

It falls short of a high score for five reasons:

1. **Internal contradictions.** The clearest is "refused content is stored nowhere" versus interviews running on a message table that persists every turn.
2. **Underspecified core mechanisms.** The relevance score, check-event resolution and the "citation counts as verification" rule are all vague, and the economics hang on them.
3. **A weak value case.** Rewards are deferred until a check event such as an earnings release. Answers are limited to public, ordinary-participant observations. The spec never argues why this information is worth paying PFT for, or how it feeds the validator and Finance Hub flywheel.
4. **No minimal first slice.** The spec has 22 requirements across several other pillars, all marked "Early", with no phasing.
5. **Weak ties to the founder's central theme.** The links to post-human governance (Hive Chat, information packets acted on in public, determinism) are mostly labels.

#### Correspondence to the founder's intention

**What matches well:**

- **"Done a large number of tasks… classified to contribute specific types of information"** maps onto existing systems: profiles (`domain_expertise`, `primary_contribution_ability`), Expert badges, capability profiles, and the new "information types" axis. The spec makes the founder's vague "classified" concrete in a sensible way: contributors confirm or remove areas and information types.
- **"Done via chat"** maps to Task Node chat interviews.
- **"Agent ramped up on a company's history… triangulate… to what extent you could have useful information"** is the right reading. Phase 1 assesses the person and produces a relevance score before any content questions. This is the founder's most distinctive idea, and the spec keeps it.
- **"Coverage universe in Finance Hub is linked to specific users"** is honestly flagged as ambiguous (open question 1). The spec also notes that profiles already list 5–10 companies per user, which is a link of a kind.

**Where it drifts or under-delivers:**

- **"Chat" is resolved to Task Node chat without saying so.** The founder's governance story centres on Hive Chat as the public surface. The spec puts interviews in private Task Node chat, which is defensible for privacy. It never says it made that choice, and it never offers a public layer, such as published corroborated clusters in Hive Chat.
- **The "Bubbles up to Outcome A · Post-human governance in public" line is asserted, not designed.**
  - The founder's core loop is that information packets are uploaded and acted on.
  - R17 turns claims into Pillar 2 packets, but nothing connects claim clusters to the Deliverable 1 governance session.
  - There is no mention of determinism, which is the founder's first priority ("formalizing our deterministic governance framework"). Scoring, screening and clustering are all non-deterministic LLM judgements.
- **The "hive mind" is mostly compliance and plumbing.** Roughly a third of the spec is MNPI safeguards. That is warranted, but the founder's aspiration (collective intelligence attached to a coverage universe) gets less design attention than legal risk. For example, the spec says nothing about how pooled knowledge accumulates over time or feeds back into what agents ask.
- **Payout speed is inverted.** The founder stresses fast payouts and FastPay for task rewards. The spec defers the main reward to a check event, often a quarter away. The trade-off is reasonable but is never acknowledged against the founder's stated preference.

#### Technical robustness

**Strengths:**

- Real file paths, with honest labels: "migration only", "no code uses it".
- A clear privacy boundary. The contributor brief (R4) prevents Deep Memory and the context document leaking into agents.
- Screening outcomes (pass, redact, refuse) with an audit trail.
- Caps reuse the existing badge-policy JSON, checked twice as the badge-routing spec requires.
- Acceptance criteria include red-team, privacy and gating tests.

**Weaknesses:**

- **Persistence contradiction.** R10 runs interviews on `finance_agent_messages`, which persists turns. R8 says refused content is never stored.
- **Ordering of screening and the LLM call.** The screening step itself sends raw answers to an LLM provider, a third party. This is not addressed.
- **Agent isolation.** The 158 design is an operator agent on the model host with a sealed model copy. The spec reuses its tables without saying whether the interview agent is a separate process that lacks model access. The R11 gating claim depends on that separation.
- **Undefined core algorithms:**
  - the Phase 1 relevance score;
  - check-event selection and resolution (who resolves, how, and what "falsified" means for an observational claim such as "rental prices fell");
  - clustering thresholds.
- **Corroboration is not independent.** Phase 2 asks contributors to corroborate existing claims. That leaks claim content and anchors answers, which undermines the independence that R13 and R14 rely on.
- **Packet encryption has no key management.** Encrypted to whom? Who can decrypt for audit? What is the cost per claim on-chain?
- **Tests have no thresholds.** The acceptance criteria rely on non-deterministic LLM behaviour but set no pass rates, test-set sizes or consent definitions.

#### Economic coherence

**Strengths:** deferred rewards resist LLM-flattery farming; caps already exist in code; the R20 dashboard measures value; falsification penalties; Sybil review.

**Weaknesses:**

- **The value proposition is thin.** If only public observations are allowed, the information may be low-novelty. The spec should state the mosaic-theory thesis: dispersed public observations that are costly to gather centrally. It should also say what "valuable" means before money is spent.
- **Funding source and flywheel are undefined.** Better Finance Hub content should attract validators and top-30 users, which should strengthen the network. The spec does not state this loop, so the PFT spend has no articulated return.
- **"Cited in an applied model change" counts as a verification event (R18).** This lets a non-market judgement unlock the main reward. If the reviewer agent or model owner is lenient or colluding, it recreates the "paid when an LLM likes it" problem the spec claims to avoid.
- **Corroboration is collusion-prone.** Corroboration that is rewarded or weighted invites Sybil and ring behaviour. Sybil review is only recommend-only.
- **Latency.** Earnings-cycle latency may kill engagement, and this is not weighed.
- **No cost model.** LLM cost per interview is not compared with expected reward or value.

#### Problems

**P1 — Refused content is persisted despite the claim it is not**
- **Severity:** critical
- **Where:** R8 vs R10; AC3.
- **What is wrong:**
  - R10 runs interviews on `finance_agent_messages`, which stores each message.
  - R8 says refused content never enters the claim store or context and "stores nothing".
  - The transcript row would contain the MNPI.
  - The screening LLM call also transmits it to a provider.
- **Why it matters:** Holding MNPI is the precise legal risk the spec exists to avoid. AC3 would pass while the database holds the content.
- **How to investigate:** Read `158_finance_agent.sql` to see whether messages are written before any screening hook. Check the retention and backup policy for that table.
- **Fix direction:**
  - Screen before persisting: hold the turn in memory, screen it, then write either a redacted turn or a hash-only placeholder.
  - Define the provider data-retention terms.
  - Extend AC3 to inspect the message and log tables and backups.

**P2 — The interview agent is not isolated from the model-host agent**
- **Severity:** major
- **Where:** R10, R11, AC5; the "What exists today" row for 158.
- **What is wrong:** The 158 agent runs on the model host with a sealed model copy. The spec reuses its tables and says the brief carries no numbers. It does not require that the interview agent be a separate process with no model access.
- **Why it matters:** Prompt-injection safety rests on what the agent can reach, not on what its brief contains.
- **How to investigate:** Trace the 158 design doc: where does the agent run, and what files and tokens does it hold?
- **Fix direction:**
  - Specify a separate interview agent with no model-host credentials.
  - Let only the bridge (R16) move approved artefacts across.
  - Make AC5 test capability, not just output.

**P3 — Check events and falsification are undefined**
- **Severity:** major
- **Where:** R12, R18, AC7; open question 12.
- **What is wrong:** The spec never says:
  - who selects a claim's check event;
  - what source resolves it;
  - whether resolution is automated, by an LLM, or by a person;
  - how a qualitative claim is judged true or false.
- **Why it matters:** The whole reward design rests on this. Vague resolution means disputes, gaming, or rewards that are never paid.
- **How to investigate:** List 20 realistic claims across equities, tokens and protocols. Try to assign each a check event and a resolution rule.
- **Fix direction:**
  - Require structured, resolvable claims: metric, direction, magnitude, date, and data source.
  - Write a resolution spec, deterministic where possible, which ties to the founder's deterministic-governance theme.
  - Add a dispute path.

**P4 — "Cited in an applied model change" unlocks the main reward**
- **Severity:** major
- **Where:** R18.
- **What is wrong:** Citation is a judgement by the model owner or reviewer agent, not an external check. This reopens the "paid because someone liked it" channel.
- **Why it matters:** It enables collusion between owner and contributor, and favours inflating citations.
- **How to investigate:** Model the incentives of the owner, contributor and reviewer agent when citation pays.
- **Fix direction:**
  - Pay citation as a small, capped credit.
  - Keep the main amount for external verification.
  - Or require that cited changes later improve forecast accuracy, using the R20 metric, before full release.

**P5 — Corroboration prompts break independence**
- **Severity:** major
- **Where:** R10 Phase 2 ("may be asked to corroborate a claim"); R13; R14; AC6.
- **What is wrong:** Showing a claim and asking for confirmation creates anchoring and herding. It also leaks other contributors' claims. The resulting corroboration is not independent, yet R14 weights clusters by it.
- **Why it matters:** The hive-mind signal is inflated, and coordinated rings can farm it.
- **How to investigate:** Review the forecasting literature on independent versus anchored elicitation. Simulate a Sybil ring against R14.
- **Fix direction:**
  - Elicit independently first, for example by asking about the underlying variable without revealing the claim.
  - Weight unprompted agreement above prompted agreement.
  - Require Sybil clearance before corroboration counts.

**P6 — The Phase 1 relevance score is undefined**
- **Severity:** major
- **Where:** R10, R3, AC2.
- **What is wrong:** This is the founder's central "triangulation" concept, yet the spec gives no inputs, scale, calibration or update rule. It is an LLM's impression, and contributors can talk it up.
- **Why it matters:** It drives matching and weighting, and is the most founder-specific part of the deliverable.
- **How to investigate:** Draft a rubric and test it on sample contributor briefs for consistency between scorers.
- **Fix direction:**
  - Define the score as observation channel × directness × recency.
  - Combine it with evidence (rewarded tasks, checked claims) and down-weight self-report.
  - Update it from verification outcomes.

**P7 — The value case and funding flywheel are missing**
- **Severity:** major
- **Where:** Economics section; open question 8.
- **What is wrong:** The spec does not explain:
  - why public, ordinary-participant observations are worth paying for;
  - where the PFT comes from;
  - how better Finance Hub content returns value to the network (validator attraction, NAVCoin quality).
- **Why it matters:** Without this, there is no basis for setting the budget, and R20 cannot be judged against anything.
- **How to investigate:** Estimate cost per interview from LLM `cost_usd` plus rewards, against a benchmark such as analyst or data-vendor cost per insight.
- **Fix direction:**
  - Add a short thesis section: the mosaic of dispersed public signals, plus the loop from validator perk to network security.
  - Add explicit pilot budget and success thresholds.

**P8 — Payout latency conflicts with the founder's speed preference**
- **Severity:** minor
- **Where:** R18.
- **What is wrong:** Main rewards wait for quarterly events. The founder wants maximum-speed task payouts using FastPay.
- **Why it matters:** Engagement and retention suffer, and the founder may reject the design.
- **How to investigate:** Compare the distribution of check-event lead times for likely claims.
- **Fix direction:**
  - State the trade-off explicitly.
  - Prefer short-horizon resolvable claims (weekly prices, on-chain metrics).
  - Pay the immediate credit via FastPay.

**P9 — Weak link to post-human governance and Hive Chat**
- **Severity:** major
- **Where:** Status "Bubbles up to"; R17; the choice of Task Node chat.
- **What is wrong:**
  - The founder's core story is information packets uploaded publicly in Hive Chat and acted on.
  - The spec puts everything in private chat.
  - Pooled clusters never flow into the Deliverable 1 governance session or any public surface beyond member-gated Finance Hub.
  - Determinism is not mentioned.
- **Why it matters:** The deliverable risks reading as a private research-sourcing tool rather than part of the "post-human primitive" narrative.
- **How to investigate:** Read the Pillar 1 and Pillar 2 specs for packet hooks. Decide what can safely be public.
- **Fix direction:**
  - Publish hashed, redacted cluster summaries as packets that can serve as Deliverable 1 governance inputs.
  - State explicitly why interviews themselves stay private.
  - Make the scoring rubric versioned and reproducible.

**P10 — No phasing or minimum viable slice**
- **Severity:** major
- **Where:** The whole Technical requirements section; Dependencies.
- **What is wrong:**
  - There are 22 requirements depending on Pillars 2, 5, 6 and 9 plus the research host.
  - Nothing says what ships first.
  - The status is "Early".
- **Why it matters:** The risk is a spec that never ships, or ships piecemeal and incoherently.
- **How to investigate:** Mark each requirement for whether it can work without other pillars.
- **Fix direction:** Define a Phase 0:
  - protocols and tokens only;
  - an unpaid or flat-credit pilot with a few Expert-badge users;
  - manual compliance review;
  - the database row as the record.
  
  Add packets, pseudonyms and deferred rewards later.

**P11 — Packet encryption and signing are underspecified**
- **Severity:** minor
- **Where:** R17, R21.
- **What is wrong:** The spec says "encrypted by default" without saying to whom. It does not cover key custody, decryption for compliance audit, the on-chain cost per claim, or how pseudonymous keys are bound to a "verified pseudonym" credential.
- **Why it matters:** These choices are hard to change once claims are written.
- **How to investigate:** Check the Pillar 2 and Pillar 6 drafts for key models.
- **Fix direction:** Specify recipients (contributor, model owner, compliance reviewer), batch anchoring of hashes, and the credential issuance flow.

**P12 — Acceptance criteria lack thresholds and definitions**
- **Severity:** minor
- **Where:** AC3, AC4 ("without consent"), AC5, AC8.
- **What is wrong:**
  - LLM behaviour is stochastic, but there are no set sizes, pass rates or repeat runs.
  - "Consent" is undefined.
  - AC8 "real figures" sets no bar.
- **Why it matters:** The criteria cannot be falsified, and the consent loophole could swallow AC4.
- **How to investigate:** Draft concrete test harnesses.
- **Fix direction:**
  - Use, for example, at least 200 red-team items, a 0 % false-pass rate on a high-severity subset, and repeated runs.
  - Define consent as an explicit per-field approval of the contributor brief.
  - Set pilot KPIs.

**P13 — Screening reliability and legal exposure**
- **Severity:** major
- **Where:** R6–R9.
- **What is wrong:**
  - LLM screening has false negatives.
  - "Redact" may keep a partially MNPI-tainted remainder.
  - There is no human review before claims reach reviewers for listed equities.
  - Legal decisions are deferred to open questions 2–5, but paid interviews could start without them, since R9 depends only on founder approval.
- **Why it matters:** Regulatory risk falls on the organisation.
- **How to investigate:** Get a counsel memo. Measure the false-negative rate on adversarial sets.
- **Fix direction:**
  - Make counsel sign-off a hard gate on any paid listed-equity mode.
  - Add human compliance review of accepted claims in the early phase.
  - Consider excluding listed equities from the pilot.

**P14 — The information-type axis and the classification source are loosely defined**
- **Severity:** minor
- **Where:** R2; story step 1.
- **What is wrong:**
  - The controlled vocabulary for knowledge areas and information types is not given.
  - The candidate derivation comes from a profile built on private sources (Deep Memory). The suggestions shown to the user could still leak private inference into the brief through the approved "experience summary".
- **Why it matters:** This is a privacy edge case and a source of inconsistent matching.
- **How to investigate:** Inspect `network_task_profile_v2.md` outputs for sensitive content.
- **Fix direction:**
  - Publish the vocabulary.
  - Require the experience summary to be written or edited by the contributor, with a diff shown, before approval.

**P15 — Operational cost is not budgeted**
- **Severity:** minor
- **Where:** R10, R11, R20.
- **What is wrong:** The spec does not estimate the combined cost of:
  - per-company agents and briefs rebuilt at each hub publish;
  - screening calls;
  - reviewer-agent clustering.
- **Why it matters:** The economics may be dominated by compute rather than rewards.
- **How to investigate:** Estimate tokens per interview and per publish across the size of the coverage universe.
- **Fix direction:** Add cost caps per company per cycle, rebuild briefs only for companies with active interviews, and report compute in R20.
