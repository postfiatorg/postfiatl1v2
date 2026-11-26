# Feedback — Pillar 10 — Information Hive Mind

> **Mean score:** 54/100 · Gemini 4 Neon 46 · Claude Opus 5.5 62
>
> **Method:** each model received (1) the founder's original prompt and (2) [this pillar's spec](../pillars/10-information-hive-mind.md), once, with the question below. Run 2026-11-26 20:13 UTC via OpenRouter.

## The question

> On a scale from 0-100, to what extent does this sub-page correspond to the founder's intention, and how likely is it to output something that works for the founder and the project — with 'works' meaning technically robust and economically coherent?
>
> Output your reasoning at length, enumerating problems clearly so that subsequent agents can investigate and improve.

## Gemini 4 Neon — 46/100

*Model `google/gemini-4-neon` · 16190 output tokens*

#### Summary judgement

This sub-page is a well-written, repo-aware **early conceptual sketch**, not a buildable engineering or economic specification. Its main strengths are readability, honest acknowledgement of where the founder's prompt cuts off, and concrete reconnaissance of the existing Task Node codebase (correctly identifying live profile/badge tables, privacy boundaries in `hive-group-worker.js`, and the unused `158_finance_agent.sql` migration).

However, if handed to an engineering team as written, this spec would **fail to produce a working system** across legal, technical, and economic dimensions:

1. **Legally/operationally hazardous:** Its flagship example solicits non-public supplier-audit data on publicly traded US equities (`MU`, `NVDA`) in exchange for financial rewards and distributes it to gated Finance Hub subscribers—textbook Material Non-Public Information (MNPI) / expert-network liability—with zero compliance guardrails.
2. **Economically fragile and easily gamed:** Paying contributors upon single-claim approval without outcome verification, reputation history, or anti-AI-slop defenses invites LLM-generated farming that will immediately drown an undefined, unincentivized human "model owner" bottleneck.
3. **Technically contradictory:** It simultaneously forbids private memory from entering the agent context (R3/AC4) while prompting the agent with task-specific details derived from profiles built on private memory (Step 3, R2). It also creates a direct access-control and IP-leakage conflict between Finance Hub's Validators/Top-30 gate (Pillar 9) and the broader Task Node contributor pool.
4. **Conceptually incomplete:** Despite being titled *Information Hive Mind*, the design is purely 1-to-1 (one user chatting with one agent to file one proposal). There is no multi-contributor triangulation, corroboration, or collective memory, and its scoring rubric ("consistency with known facts") actively penalizes high-value contrarian alpha.

#### Correspondence to the founder's intention

The founder's Deliverable 10 prompt is short and cuts off mid-thought, so some extrapolation was necessary. The sub-page captures the **surface loop** faithfully—inferring contributor capabilities from Task Node history, matching them to covered companies/protocols, running a chat-based company agent ramped up on company history, and feeding outputs into Finance Hub—and properly flags the truncated ending (*"linked to specific users"*) as an open question.

Where it diverges from the founder's intent in consequential ways:

- **Misses the "Hive Mind" and shifts the meaning of "triangulate":** The founder wrote: *"An agent who is ramped up on a company's history can triangulate with you based on your experience to what extent you could have useful information about a company or protocol."* That describes an interactive **vantage-discovery / qualification** dialogue—and, across users, **triangulating signals across multiple contributors** to form a Hive Mind. The spec replaces this with static tag matching followed by 3–5 rigid Q&As scored post-hoc by an LLM, with zero cross-contributor synthesis.
- **Drops the qualification prerequisite:** The founder starts with *"I have done a large number of tasks on the task node."* While the spec mentions `expert-badge.js` (`>=20` tasks, score `>=80`) in "What exists today", none of R1–R8 enforce any minimum task-history or reputation threshold to participate.
- **Narrows "specific types of information" to sector tags:** The founder describes being classified to contribute *"specific types of information"* (which includes vantage points and modalities like supply-chain checks, protocol security, regulatory tracking, or on-chain analysis). R1 collapses this into a flat `sector / sub-industry / protocol` tag taxonomy.
- **Treats the founder's core "information packet" primitive as an optional afterthought:** The founder's overarching framing is that Post Fiat is *"post human governance... information packets are to be uploaded, and acted upon."* Here, Pillar 2 (encrypted IPFS protobufs) is only mentioned as an optional public transcript export, while core workflow approval remains bottlenecked on a manual human "model owner."
- **Leaves the chat surface ambiguous:** The founder specifies contributing *"via chat"* alongside a major Hive Chat / NOSTR push (Deliverables 1 and 6), yet the spec puts the entry point on gated Finance Hub company pages without clarifying whether non-top-30 contributors can use it or how it relates to Task Node chat and Hive Chat.

#### Technical robustness

The codebase inventory is a strong starting point, but the architecture has severe internal contradictions and unverified assumptions:

- **Privacy vs. personalization contradiction:** The spec cites `024_network_task_profiles.sql` as built from *"context, deep memory and tasks"* and cites `hive-group-worker.js` as enforcing the privacy boundary. Yet Step 3 shows the company agent quoting specific user tasks (*"Your tasks mention HBM supplier audits"*), while R3 and AC4 mandate that private memory never enters agent context without consent. There is no data-classification boundary defining what is sanitized vs. private before entering prompts or transcripts.
- **Gated-IP and access-control conflict:** Pillar 9 restricts Finance Hub to Validators and Top-30 Task Node users. Putting *"Ask the agent"* on Finance Hub pages (R8) while loading the full published model, changelog, and notes into the agent's context (R3) either locks out almost all contributors or exposes gated Finance Hub IP to prompt-injection extraction by ungated contributors—and R4 allows contributors to publish transcripts publicly via IPFS.
- **Unverified database schema and missing Finance Hub write-back bridge:** R3 and R5 assume the unused `158_finance_agent.sql` tables already fit the interview, scoring, attribution, and proposal flows without inspecting or specifying required schema deltas. Simultaneously, the spec admits Finance Hub is *"live as a bundle"* and *"coverage universe appears nowhere in code"*, yet R1/R6 assume runtime Postgres rows cleanly tag the "hub build" and link into model `CHANGELOG` entries with no synchronization architecture.
- **Unworkable LLM scoring and missing retrieval/verification architecture:** Scoring "novelty" against an entire company's multi-year model, earnings history, notes, and prior contributor claims requires a defined structured-claim/RAG store (not raw context stuffing). Scoring "checkability" without a post-hoc resolution pipeline leaves the metric unused.
- **Broken anonymity model:** R7 allows anonymous Finance Hub attribution while R6 pays via the standard Task Node reward path linked to CHANGELOG updates—making de-anonymization trivial via payout/timestamp correlation.
- **Zero compliance/MNPI architecture:** Soliciting and monetizing non-public operational information on public equities without compliance filtering or asset-class partitioning is a showstopper technical and legal omission.

#### Economic coherence

Economic design is the weakest dimension of this sub-page. As specified, the incentive loop is structurally prone to adverse selection, spam, and operator burnout:

- **LLM-slop farming & asymmetric verification cost:** Paying per approved claim without staking, contributor track-record weighting, or deferred outcome verification creates a lucrative attack surface: contributors can feed public filings/news into frontier LLMs to generate plausible, "novel, consistent, checkable" claims at near-zero cost. Verifying whether each claim is genuine insight vs. plausible hallucination is expensive for the human model owner, creating an economic Denial-of-Service on reviewers.
- **Undefined reward pricing, funding source, and reviewer incentives:** The spec simply says *"rewarded through the normal reward path (a task, or a reward line)"*. It does not specify where payouts are funded from, how high-alpha vs. trivial insights are priced, or why "model owners" would expend analyst hours reviewing contributor proposals.
- **Anti-alpha scoring incentives:** Rewarding *"consistency with known facts"* systematically favors consensus-confirming restatements and penalizes non-consensus, contrarian intelligence—the exact category of information that generates financial alpha.
- **Value leakage & alpha cannibalization:** Finance Hub's economic moat (Deliverables 3 and 9) relies on exclusive, high-value research for Validators and Top-30 users. Allowing contributors to interact with full model context and optionally publish transcripts publicly on IPFS undermines that exclusivity.
- **Missing expert-participation & anonymous-credibility economics:** Real domain experts face high opportunity cost and career risk; generic micro-rewards will not attract them unless compensation scales with realized impact and anonymity is cryptographically sound *while* preserving verifiable credibility (e.g., an anonymous contributor's historical accuracy score).

#### Problems

##### P1 — Unaddressed MNPI / insider-trading compliance risk on public equities
- **Severity:** critical
- **Where it occurs:** User story (Steps 2–3, `MU`/`NVDA` HBM supplier audit example), R3–R6, R8.
- **What is wrong:** The spec explicitly illustrates paying contributors for non-public, issuer-specific operational intelligence (e.g., HBM supplier audit lead times for Micron Technology) and publishing those insights into gated investment models for Finance Hub users, with zero MNPI policy, legal screening, contributor attestation, or separation between public equities and permissionless crypto protocols.
- **Why it matters:** Soliciting, purchasing, and redistributing non-public operational data on publicly traded securities creates existential securities-law liability (insider trading / tipper-tippee exposure) for the founder, operators, model owners, and contributors.
- **How an agent should investigate:** Audit the Finance Hub coverage universe (Pillar 9) to categorize covered assets (US/global public equities vs. open crypto protocols/tokens) and check existing Task Node ToS, compliance docs, or Finance Hub disclaimers.
- **Suggested direction for fix:** Partition the coverage universe by regulatory regime. For public equities, require mandatory pre-interview MNPI/NDA attestations, add an automated compliance classifier that rejects/redacts confidential issuer-specific disclosures before storage, and restrict equity prompts to public-domain synthesis, industry-level mosaic trends, and verifiable public filings. Reserve deep operational/governance intelligence gathering primarily for open protocols and public-data analysis.

##### P2 — Gameable reward-on-approval economics, LLM-slop farming, and model-owner bottleneck
- **Severity:** critical
- **Where it occurs:** Architecture diagram, R5, R6, Acceptance criteria #3, Open questions #2–3.
- **What is wrong:** Contributors are rewarded upon single-claim approval by a human "model owner", with no minimum contributor eligibility gate, no anti-AI-slop filtering, no reward sizing/budgeting model, no post-hoc outcome verification, and no incentive or capacity model for model owners.
- **Why it matters:** In an incentivized network, rational actors will farm rewards with LLM-synthesized finance prose. Reviewing plausible AI-generated claims is far slower than generating them, causing either rubber-stamping (polluting Finance Hub models) or reviewer abandonment (stalling the system).
- **How an agent should investigate:** Inspect Task Node's existing reward sizing/budget logic, Sybil/quality controls, `server/expert-badge.js`, and how Finance Hub model updates are currently staffed and cadenced.
- **Suggested direction for fix:** (1) Gate paid interviews behind proven Task Node history (e.g., Expert badge or domain-verified tier); (2) replace flat pay-on-approval with a split structure: a modest intake/citation credit plus a larger deferred payout or reputation multiplier tied to resolved accuracy / analyst impact rating; (3) add contributor calibration scores and per-window submission quotas; (4) explicitly define model-owner compensation or an automated pre-screening/peer-triage tier before proposals hit human owners.

##### P3 — Privacy boundary vs. personalization contradiction (private memory leakage)
- **Severity:** critical
- **Where it occurs:** What exists today (`024_network_task_profiles.sql`, `hive-group-worker.js`), User story Step 3, R2, R3, AC2, AC4.
- **What is wrong:** `network_task_profiles` is built from *"context, deep memory and tasks"*, and Step 3 has the company agent cite specific contributor tasks (*"Your tasks mention HBM supplier audits"*). Yet R3 and AC4 state that the contributor's private memory must *never* enter the agent context without consent. The spec fails to define what separates private memory from task profiles/titles, or how derived profiles are sanitized before entering an agent whose transcripts are reviewed by third parties or published.
- **Why it matters:** Derived task profiles and specific task references can trivially leak private context-doc details (employer, client NDAs, sensitive personal background) into LLM prompts, model-owner queues, and public Finance Hub pages, violating Task Node's core privacy contract.
- **How an agent should investigate:** Read `docs/wiki/architecture/network-task-profile-worker.md`, `prompts/memory/network_task_profile_v2.md`, `server/db/migrations/024_network_task_profiles.sql`, and `server/hive-group-worker.js` to map exact data lineage from private memory into profile fields.
- **Suggested direction for fix:** Introduce an explicit, schema-enforced **Hive-safe contributor projection** generated strictly from non-private/consented fields (confirmed knowledge areas + user-approved shareable experience summaries). Require explicit per-session consent before exposing specific task snippets to a company agent, and strip user-identifying prompt context from transcripts before they reach model owners or Finance Hub.

##### P4 — Access-control and IP-leakage conflict between Finance Hub gating and contributors
- **Severity:** critical
- **Where it occurs:** User story Steps 3 & 6, R3, R4, R8, Dependencies (Pillar 9).
- **What is wrong:** Pillar 9 restricts Finance Hub to Validators and Top-30 Task Node users. Yet R8 puts *"Ask the agent"* on Finance Hub company pages, R3 loads the gated published model, changelog, and notes directly into the interviewing agent, and R4 allows contributors to pin transcripts publicly via Pillar 2.
- **Why it matters:** Either (a) non-top-30 contributors are locked out of Finance Hub, shrinking the Hive Mind to <=30 users plus validators, or (b) ungated contributors can chat with an agent loaded with proprietary models and extract gated alpha via prompt injection—while public IPFS transcripts leak Finance Hub's open research questions and paid-for insights to non-subscribers.
- **How an agent should investigate:** Review Pillar 9 (`09-finance-hub.md`) and Finance Hub's authentication/gating code to determine what content is strictly gated vs. public-safe.
- **Suggested direction for fix:** Decouple the **Contributor Interview Surface** (inside Task Node, accessible to all qualified contributors regardless of top-30/validator status) from the **Finance Hub Insights Surface** (gated to Validators/Top-30). Feed the interviewing agent a sanitized *"public company brief + anonymized open questions"* rather than raw proprietary model outputs, and default Pillar 2 IPFS pinning to **encrypted** packets readable only by authorized Finance Hub/model-owner keys.

##### P5 — Missing the "Hive Mind": no multi-contributor triangulation, corroboration, or collective memory
- **Severity:** major
- **Where it occurs:** In one paragraph, User story Step 4, Architecture diagram, R5.
- **What is wrong:** The entire workflow is 1-to-1 (one user -> one agent -> isolated `finance_agent_proposal` -> human owner). There is no cross-contributor corroboration, deduplication, conflict detection, or shared claim graph. It also misconstrues the founder's *"triangulate with you based on your experience to what extent you could have useful information"* (adaptive vantage qualification) as static post-hoc scoring.
- **Why it matters:** Without multi-contributor synthesis, ten contributors reporting the same signal create ten redundant proposals, conflicting accounts are never reconciled, and the agent cannot leverage anonymized claims from Contributor A to ask targeted corroborating questions of Contributor B.
- **How an agent should investigate:** Inspect `server/db/migrations/158_finance_agent.sql` and `server/hive-group-worker.js` to see how multi-user state and summaries are currently structured.
- **Suggested direction for fix:** Add a per-entity **Research Question & Claim Store** tracking open questions, normalized claims, supporting/contradicting contributor links (weighted by contributor reputation), and consensus vs. divergence state. Split interviews into two phases: (1) *vantage qualification* (triangulating what direct experience the user actually holds) and (2) *targeted discovery/corroboration* against open questions and existing anonymized claims.

##### P6 — Flawed triangulation rubric: "consistency" penalizes contrarian alpha and "checkability" has no feedback loop
- **Severity:** major
- **Where it occurs:** User story Step 4, Architecture diagram, R5.
- **What is wrong:** Scoring claims positively on *"consistency with known facts"* penalizes claims that contradict management guidance or current model assumptions—precisely where valuable non-consensus alpha lives. Meanwhile, *"checkability"* is scored at intake, yet there is no mechanism to ever resolve whether a checkable claim proved true or false later.
- **Why it matters:** The rubric incentivizes safe consensus echoing over high-value contrarian signals and never produces empirical ground truth to calibrate contributor credibility.
- **How an agent should investigate:** Check existing Task Node evaluation/scoring prompts and Finance Hub's earnings/event calendar structures to see how time-bound predictions or catalysts are represented.
- **Suggested direction for fix:** Replace "consistency" with (a) **model stance** (`confirms`, `refines`, `challenges`), (b) **evidence/mechanism quality** (does the user provide a concrete, falsifiable mechanism for why the model or consensus may be wrong?), and (c) **corroboration score** across independent contributors. Attach structured `resolution_date` / `catalyst_event` fields to checkable claims and build a post-event resolution job that updates each contributor's track record.

##### P7 — Unverified reliance on `158_finance_agent.sql` and missing bridge to Finance Hub's static bundle/CHANGELOG
- **Severity:** major
- **Where it occurs:** What exists today, R1, R3, R5, R6, Code references.
- **What is wrong:** The spec binds R3 and R5 to an unused migration (`158_finance_agent.sql`) without verifying whether its schema supports entity/ticker IDs, knowledge areas, triangulation scores, consent flags, anonymity, or reward IDs. It also notes Finance Hub is *"live as a bundle"* and *"coverage universe appears nowhere in code"*, yet R1 and R6 assume runtime Postgres data updates the "hub build" and links into model `CHANGELOG` entries without defining any integration contract.
- **Why it matters:** Engineers will hit immediate schema mismatches and an unbridged architectural gap between Task Node's live database and Finance Hub's build-time static bundle.
- **How an agent should investigate:** Read `server/db/migrations/158_finance_agent.sql` in full, and inspect Finance Hub's repository/build pipeline to see how models, changelogs, and ticker metadata are authored, versioned, and deployed.
- **Suggested direction for fix:** Audit `158_finance_agent.sql` and explicitly list required migration deltas (e.g., `coverage_entities`, `knowledge_areas`, `entity_claims`, `claim_verifications`). Define a canonical **Coverage Registry** shared by Task Node and Finance Hub, plus a concrete sync contract (e.g., Finance Hub reading approved insights via authenticated API, and approved proposals generating structured PR/changelog entries with immutable proposal IDs).

##### P8 — Brittle static `knowledge_areas` taxonomy ignores existing embeddings and information modalities
- **Severity:** major
- **Where it occurs:** User story Steps 1–2, R1, R2, What exists today (`recommended-connections.js`).
- **What is wrong:** R1 relies solely on a manually maintained tag taxonomy (`sector, sub-industry, protocol tags`) to match contributors to covered entities, ignoring the embedding-based matching system already live in `server/repositories/recommended-connections.js`. It also omits *information modality / vantage point* (e.g., supply-chain operator, smart-contract developer, customer, regulator, quant).
- **Why it matters:** Flat sector tags miss indirect exposures (e.g., a packaging engineer matching a downstream semiconductor ticker, or a DeFi liquidator matching a lending protocol), degrade recall/precision, and require manual re-tagging whenever the coverage universe changes.
- **How an agent should investigate:** Inspect `server/repositories/recommended-connections.js` and `server/repositories/capability-profiles.js` to see how embeddings, profiles, and similarity thresholds are implemented today.
- **Suggested direction for fix:** Use a two-axis classification (**domain/entity exposure** + **information modality/vantage**) backed by hybrid matching: embedding similarity between sanitized contributor profiles and each entity's live open research questions/supply-chain description, combined with user-confirmed structured tags for transparency and control.

##### P9 — Unresolved coverage-to-user linkage and undefined "model owner" role
- **Severity:** major
- **Where it occurs:** User story (founder's note), R5, Acceptance criteria #3, Open questions #1 & #3.
- **What is wrong:** The spec defers *"The coverage universe in finance hub is linked to specific users"* to an open question while simultaneously depending on an undefined `"model owner"` role in R5 and AC3 to approve `finance_agent_proposals` and update model changelogs.
- **Why it matters:** Leaves core permissions, operational ownership, review SLAs, and fallback behavior completely unspecified—if a company has no assigned model owner or an unresponsive one, the entire contribution and reward pipeline halts.
- **How an agent should investigate:** Inspect existing role/approval schemas in Task Node (`072_network_badges_identity_approvals.sql`, `capability-profiles.js`) and check how Finance Hub models currently attribute authorship/ownership.
- **Suggested direction for fix:** Specify a unified **Coverage Linkage & Governance** model (`coverage_user_links` with roles: `model_owner`, `coverage_steward`, `matched_contributor`), including default fallback routing (e.g., Finance Hub operator queue or AI pre-triage + steward consensus) and explicit review SLAs so the founder only chooses policy defaults rather than designing the permission system from scratch.

##### P10 — Broken anonymity guarantee via reward/CHANGELOG correlation and unverified anonymous credibility
- **Severity:** major
- **Where it occurs:** User story Step 6, R6, R7, Dependencies (Pillar 6).
- **What is wrong:** R7 offers contributors `"anonymous"` attribution on Finance Hub, while R6 rewards them via the normal Task Node reward path linked to model CHANGELOG entries. Anyone inspecting public Task Node/on-chain reward payouts around a CHANGELOG update can correlate timing and amounts to de-anonymize the contributor. Meanwhile, raw `"anonymous"` quotes on Finance Hub give readers no basis to trust the source.
- **Why it matters:** Contributors expecting anonymity face accidental doxxing, while Finance Hub readers will discount uncredentialed anonymous blurbs.
- **How an agent should investigate:** Inspect Task Node's reward payout visibility/explorer links and review Pillar 5 (local shielded transactions) and Pillar 6 (deterministic NOSTR/on-chain identity).
- **Suggested direction for fix:** For anonymous contributions, decouple reward settlement via batched/delayed payouts or Pillar 5 shielded transactions so no 1-to-1 public link exists between a CHANGELOG entry and a wallet. On Finance Hub, display **pseudonymous credential badges** (e.g., *"Anonymous Verified Expert — 25+ Semiconductor Tasks, 82% Historical Accuracy"*) to preserve signal quality without leaking identity.

##### P11 — Rigid interview UX, missing user-initiated submission path, and absent eligibility gate
- **Severity:** minor
- **Where it occurs:** User story Steps 1–3, R4, Acceptance criteria #1–2.
- **What is wrong:** R4 fixes interviews to *"3–5 targeted questions"* with no adaptive follow-up probing, no user-initiated entry point for contributors who have breaking/unsolicited intel, no rate limits or cooldowns, and no explicit eligibility threshold (despite the founder's *"large number of tasks"* premise).
- **Why it matters:** Rigid Q&A feels like a survey rather than an intelligent triangulation interview, valuable unsolicited intelligence has no intake path, and lacking rate limits/eligibility thresholds increases noise and LLM token spend.
- **How an agent should investigate:** Review Task Node's existing chat UI components, context-editor calibration flow (`prompts/context/context_edit_jobs_v1.xml`), and `server/expert-badge.js`.
- **Suggested direction for fix:** Enforce a configurable eligibility tier (e.g., Expert badge or minimum task/score floor), support both agent-invited and contributor-initiated interviews with adaptive follow-up turns (bounded by a turn/token cap), and add per-user/per-company cooldown windows.

##### P12 — Thin Pillar 2 ("information packet") integration under-delivers on the post-human thesis
- **Severity:** minor
- **Where it occurs:** Bubbles up to, R4, Dependencies (Pillar 2).
- **What is wrong:** Pillar 2 (encrypted IPFS protobuf information packets) is treated as an optional public transcript export in R4 rather than the canonical, machine-readable artifact carrying signed claims, triangulation metadata, and settlement references.
- **Why it matters:** Misses the architectural unification across Post Fiat's pillars; storing insights solely in opaque Postgres tables makes them harder for external agents, block explorers, and deterministic governance modules (Pillar 1) to verify and consume.
- **How an agent should investigate:** Review Pillar 2 (`02-message-protocol.md`) and Pillar 6 (`06-hive-identity.md`) specs to check protobuf envelope conventions, encryption keys, and X402/Fastpay hooks.
- **Suggested direction for fix:** Define canonical Pillar 2 protobuf schemas (`HiveClaimPacket`, `InterviewTranscriptPacket`) signed by the contributor's Pillar 6 identity (or ephemeral pseudonymous key), pinned encrypted to IPFS by default, and referenced by hash in both `finance_agent_proposals` and Finance Hub CHANGELOG entries.

## Claude Opus 5.5 — 62/100

*Model `anthropic/claude-opus-5.5` · 6604 output tokens*

#### Summary judgement

This is a well-grounded and readable spec. It inventories real code: profiles, badges, capability profiles, recommended connections, context documents, the unused `158_finance_agent.sql` tables and the Hive privacy boundary. It turns the founder's short, loosely worded Deliverable 10 into a concrete pipeline: classify, match, interview, triangulate, credit, surface. The happy path is plausible to build, and the privacy constraint is handled with care.

It is weakest in three areas:

1. **It narrows "hive mind" to one-on-one interviews.** The spec has each user talk to an agent separately. It drops the collective, multi-contributor triangulation that the founder's phrase implies.
2. **It omits the legal and compliance risk at the centre of the feature.** Paying people to tell a finance agent what they have seen about companies is close to soliciting material non-public information (MNPI). The spec's own worked example ("HBM supplier audits") does exactly that.
3. **The incentive design is thin.** Rewards are paid on an LLM's judgement of "novelty" before any verification. There is no budget, no gaming resistance, no later check of claims, and no measure that contributions actually improve models.

The spec is a good skeleton but not yet a design that would hold up economically or legally.

#### Correspondence to the founder's intention

What the founder asked for:

- Users classified, through their Task Node history, as able to contribute specific types of information.
- Contribution happens through chat.
- An agent ramped up on a company's history "triangulates with you based on your experience to what extent you could have useful information."
- The coverage universe is linked to specific users.

How the spec matches that:

| Founder's element | Spec's coverage |
|---|---|
| Classification | Knowledge areas (R1–R2) |
| Chat | Structured interview (R4) |
| Company-history agent | One agent per company (R3) |
| Link between coverage universe and users | Matching via the knowledge-area taxonomy |

The open question about assignment versus matching is a fair one to raise.

Where the spec drifts:

- **"Triangulation" is redefined.** The founder uses the word for judging *how far a user is likely to hold useful information*. That is a relevance and credibility assessment of the person. The spec turns it into scoring individual claims (novelty, consistency, checkability) against the model. Both matter. However, the founder's sense has no explicit mechanism: no credibility or prior-reliability score for the contributor on a given company.
- **"Hive mind" implies aggregation across many contributors.** Examples would be corroborating claims, spotting consensus and divergence, and combining many weak signals. The spec has no cross-contributor layer.
- **The "post-human" framing is weak.** The founder's thesis is that information packets are uploaded and acted on. Here, contributions become on-chain or message-protocol packets only if the contributor makes them public. Approval is by a human "model owner", which is the least post-human gate available.
- **The spec presents the founder's text as truncated** ("The founder's text ends here", with an added ellipsis). The final sentence is actually complete. This is a small misreading, but it frames the spec's additions as filling a gap rather than as interpretation.

#### Technical robustness

Strengths:

- The current state is mapped accurately.
- The spec reuses existing tables.
- The consent-gated private memory rule comes with a test (acceptance criterion 4).
- The surfaces and dependencies are clear.

Weaknesses:

- **Reuse of the existing tables is assumed, not verified.** `finance_agent_proposals` was designed for proposals that need operator approval. Its schema may not have room for contributor identity, claim-level scores, evidence or reward linkage.
- **The taxonomy (R1) rests on an undefined concept.** "Coverage universe" does not exist in code. Pillar 9 must define it first, so the taxonomy has no anchor yet.
- **Inference (R2) may breach the privacy boundary.** Knowledge areas are inferred from network task profiles, and those profiles are built partly from deep memory. The "reason" shown to the user could also leak private context.
- **There is no verification loop for "checkability".** A claim is labelled checkable but is never checked later.
- **Agent context is not specified.** The spec does not say how per-company context is refreshed, sized or cost-bounded across the whole coverage universe.
- **Acceptance criterion 3 depends on an unspecified pipeline.** It requires the model-update cadence and the CHANGELOG-generation process, and the spec defines neither.

#### Economic coherence

The economic design is the weakest part of the spec.

- **Rewards are paid before verification.** An LLM's novelty and consistency score creates a direct incentive to invent plausible, unverifiable "insights", or to farm small rephrasings.
- **There is no reward budget or pricing.** The spec does not say where the money comes from, whether rewards are paid in PFT, what a claim is worth, or what caps apply.
- **There is no value-capture loop.** The spec never measures whether contributions improve model accuracy, Finance Hub engagement or NAVCoin performance, so the programme cannot justify its spending.
- **Gaming is easy.** The "You may know about…" suggestion invites self-selection into high-reward companies. Users can confirm their own knowledge areas, so the gate is self-asserted.
- **The approval bottleneck does not scale.** Model-owner approval is a human chokepoint across a whole coverage universe.
- **Anonymity and on-chain rewards conflict.** On-chain rewards link the anonymous contributor to an identity anyway.
- **MNPI and confidentiality exposure is a legal liability.** This matters especially if Finance Hub models feed tradable NAVCoins. Legal exposure undermines the economics of the whole project.

#### Problems

**P1 — Solicitation of material non-public information and confidential information**
- **Severity:** critical.
- **Where:** Augmented story step 3 (the "HBM supplier audits" example); R3–R5; Finance Hub surfacing in step 6.
- **What is wrong:** The feature pays users to disclose what they have observed about specific listed companies. The flagship example asks about supplier audits, which is plausibly confidential and material information. The spec contains no compliance policy, refusal behaviour or disclaimer.
- **Why it matters:**
  - Insider-trading and tipping exposure for the project and its users, especially if the models inform NAVCoin positions.
  - Breach-of-NDA exposure for contributors.
  - Reputational risk.
- **How to investigate:** Check whether Finance Hub models or outputs influence any NAVCoin (Pillars 7 and 9). Review how expert-network firms (GLG, Tegus) handle MNPI screening.
- **Fix direction:**
  - Add a compliance section: contributor attestations, no current-employer questions, an agent that refuses and flags likely MNPI, and an auditable log.
  - Rewrite the example around public or mosaic-theory information.
  - Add a founder open question on legal review.

**P2 — "Hive mind" reduced to one-on-one interviews; no cross-contributor triangulation**
- **Severity:** major.
- **Where:** In one paragraph; Architecture; R5.
- **What is wrong:** Every claim is checked only against the existing model. Nothing aggregates, corroborates or contrasts claims from multiple contributors.
- **Why it matters:** The founder's "hive mind" implies collective intelligence. Corroboration is also the main defence against fabricated claims.
- **How to investigate:** Re-read the founder's wording. Check whether the `finance_agent_*` schema can group claims by topic.
- **Fix direction:** Add a claim-clustering and corroboration step. Weight claims by the number of independent corroborating contributors, and surface consensus and dissent on the company page.

**P3 — "Triangulation" diverges from the founder's meaning (assessing the user's likely usefulness)**
- **Severity:** major.
- **Where:** Augmented story step 4; R5.
- **What is wrong:** The founder triangulates the *person's* potential to hold useful information. The spec scores *claims* only, and matching is a simple overlap of tags.
- **Why it matters:** The founder's sense of the word is unaddressed. Without it, there is no per-company credibility score for each user.
- **How to investigate:** Compare the founder's sentence with R2 and R5.
- **Fix direction:** Add a per-user, per-company credibility prior. Build it from relevant task history and the past accuracy of their claims, and use it in both matching and claim weighting.

**P4 — Rewards paid on LLM judgement before verification; easy to game**
- **Severity:** critical.
- **Where:** R5–R6; Augmented story step 5.
- **What is wrong:** Novelty is easy to manufacture. "Checkability" is never followed by an actual check.
- **Why it matters:** Fabricated or recycled claims drain rewards and pollute the models.
- **How to investigate:** Red-team the scoring prompt with fabricated but plausible claims.
- **Fix direction:**
  - Split each reward into a small immediate payment and a larger deferred payment released when the claim is verified (for example, at the next earnings release).
  - Add reputation decay or slashing for claims shown to be false.
  - Cap rewards per user per period.

**P5 — No reward budget, pricing or source of funds**
- **Severity:** major.
- **Where:** R6; Open questions.
- **What is wrong:** The spec says rewards go through "the normal reward path" but gives no amounts, token, budget or caps.
- **Why it matters:** Without these, the programme's economics are undefined and the cost cannot be bounded.
- **How to investigate:** Look at the existing Task Node reward economics and emission schedule.
- **Fix direction:** Define a budget per company or per period, rewards tiered by contribution type, and their relation to existing task rewards.

**P6 — No measure of value or effect on models**
- **Severity:** major.
- **Where:** Acceptance criteria.
- **What is wrong:** No metric ties contributions to better forecasts or better Finance Hub engagement.
- **Why it matters:** The programme cannot be judged or tuned without one.
- **How to investigate:** Check what accuracy metrics the Finance Hub models already track.
- **Fix direction:** Track forecast error before and after contributions, the share of contributions verified, and spend per verified insight.

**P7 — Privacy leakage through inference and match reasons**
- **Severity:** major.
- **Where:** R2; Augmented story step 2 ("with a reason for each").
- **What is wrong:** Network task profiles use deep memory. Inferred areas and the stated reasons could surface private facts. Public attribution of knowledge areas could also deanonymise users.
- **Why it matters:** This contradicts the privacy boundary the spec itself says must be kept.
- **How to investigate:** Trace what feeds `network_task_profile_v2` and whether it includes private memory.
- **Fix direction:** Infer areas only from non-private sources unless the user opts in. Never show private-derived reasons in shared surfaces. Extend the test in acceptance criterion 4 to cover inference and matching.

**P8 — Anonymous attribution conflicts with on-chain rewards**
- **Severity:** minor to major.
- **Where:** R7; R6.
- **What is wrong:** A reward paid to an address, or recorded on a minted identity, links the "anonymous" insight back to the contributor.
- **Why it matters:** The anonymity promise is false, and that matters especially alongside the MNPI risk in P1.
- **How to investigate:** Check whether reward transactions are public, and whether the Pillar 5 shielded path could be used.
- **Fix direction:** Pay anonymous contributions through shielded transactions (Pillar 5), or batch them. State clearly what level of anonymity is offered.

**P9 — Human model-owner approval is a bottleneck and conflicts with the post-human framing**
- **Severity:** major.
- **Where:** R5; Open questions.
- **What is wrong:** Every claim needs manual approval.
- **Why it matters:** This does not scale across a coverage universe, and it contradicts the "post-human activity" story.
- **How to investigate:** Estimate claim volume against owner capacity.
- **Fix direction:** Have a reviewer agent triage claims first. Leave humans to approve only those above a materiality threshold, or link approval to the Pillar 1 deterministic governance process.

**P10 — Reuse of `finance_agent_*` tables assumed to fit**
- **Severity:** minor.
- **Where:** R3; R5; What exists today.
- **What is wrong:** The migration was written for operator-approved proposals. It may lack fields for contributor identity, per-claim scores, evidence, reward linkage and verification status.
- **How to investigate:** Read `158_finance_agent.sql` and any associated design notes.
- **Fix direction:** Specify a schema delta or a separate `knowledge_contributions` table.

**P11 — Taxonomy depends on an undefined coverage universe**
- **Severity:** minor to major.
- **Where:** R1; dependency on Pillar 9.
- **What is wrong:** The term "coverage universe" is not in code, and it is unclear who maintains the tags.
- **How to investigate:** Check how the Pillar 9 hub build defines and lists the companies it covers.
- **Fix direction:** Make the coverage-universe manifest an explicit Pillar 9 output that includes area tags. Define a small controlled vocabulary.

**P12 — Weak tie to the message protocol and chain**
- **Severity:** minor.
- **Where:** R4; R7.
- **What is wrong:** Contributions become Pillar 2 packets only if the contributor makes them public.
- **Why it matters:** This misses the founder's thesis that information packets are uploaded and then acted on.
- **Fix direction:** Record every accepted contribution as an encrypted Pillar 2 packet, with a hash on chain. Make public decryption optional.

**P13 — Self-confirmed knowledge areas and self-selection**
- **Severity:** minor.
- **Where:** R2; Augmented story step 2.
- **What is wrong:** Users can confirm areas freely and gravitate toward high-reward companies.
- **Fix direction:** Weight matching by evidence from history, not by confirmation alone. Rotate invitations, and limit concurrent interviews per user.

**P14 — Agent cost and context freshness are not specified**
- **Severity:** minor.
- **Where:** R3.
- **What is wrong:** The spec does not say how many agents run, how their context is refreshed after model updates, or what token and compute budget applies.
- **Fix direction:** Instantiate agents lazily from a per-company context bundle rebuilt at each hub build. Cache the bundles.

**P15 — Founder's text presented as truncated**
- **Severity:** minor.
- **Where:** User story ("ends here", added ellipsis).
- **What is wrong:** The founder's last sentence is complete.
- **Why it matters:** Presenting it as truncated frames the spec's additions as filling a gap rather than as interpretation.
- **Fix direction:** Drop the ellipsis and the note. Label the additions as interpretation.
