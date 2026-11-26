# Feedback — Pillar 10 — Information Hive Mind

*Review round 3.*

> **Mean score:** 64/100 · Gemini 4 Neon 62 · Claude Opus 5.5 66
>
> **Method:** each model received (1) the founder's original prompt and (2) [this pillar's spec](../../pillars/10-information-hive-mind.md), once, with the question below. Run 2026-11-26 22:31 UTC via OpenRouter.

## The question

> On a scale from 0-100, to what extent does this sub-page correspond to the founder's intention, and how likely is it to output something that works for the founder and the project — with 'works' meaning technically robust and economically coherent?
>
> Output your reasoning at length, enumerating problems clearly so that subsequent agents can investigate and improve.

## Gemini 4 Neon — 62/100

*Model `google/gemini-4-neon` · 18364 output tokens*

#### Summary judgement

This sub-page is unusually well-grounded in the actual Task Node codebase and displays strong instincts around privacy isolation, LLM-slop defense, and real-world securities/MNPI compliance risk—risks the founder's four-sentence prompt did not mention at all. It is also refreshingly honest about where the founder's prompt ends and interpretation begins.

However, as a blueprint meant to produce something **technically robust and economically coherent**, it falls short in its current form. In trying to solve every compliance, quality, and attribution hazard up front, the spec mutates the founder's lightweight "triangulating research agent" into an over-engineered, compliance-heavy, prediction-market-style adjudication apparatus. That design introduces **critical economic exploits** (unbonded hedged-bet collusion and base-rate harvesting), **a core logical contradiction** between requiring strictly public data (R7) while penalizing anything a web-search LLM can find (R15), **severe human operational bottlenecks** across four distinct review roles, and **several concrete security/privacy bugs** (an information-extraction leak in R12, direct on-chain wallet doxxing of pseudonymous contributors via R21 task evidence, and source-spoofing/prompt-injection exposure in R15/R17). Finally, Step 1 is framed as a "standalone pilot" yet requires building ~80% of the full system, silently depends on Pillar 9 R19, and is blocked behind 13 multi-part open questions (~35+ unresolved policy decisions) without even validating whether Task Node's contributor base possesses real mosaic alpha on Finance Hub's coverage universe.

The foundations are salvageable—and much of the codebase-integration analysis should be kept—but the economic mechanism, interview-to-insight abstraction, security boundaries, and MVP scoping need a substantial rethink before engineering begins.

---

#### Correspondence to the founder's intention

The founder's Deliverable 10 prompt is brief and open-ended:
> *I have done a large number of tasks on the task node. I am classified to contribute specific types of information to the task node and this can be done via chat. An agent who is ramped up on a company's history can triangulate with you based on your experience to what extent you could have useful information about a company or protocol. The coverage universe in finance hub is linked to specific users*

**Where the spec aligns well:**
- **Core loop preserved:** Experienced Task Node users are classified into information types and knowledge areas, matched/linked to covered companies/protocols, and interviewed in chat by a company-aware agent that first triangulates what the user can realistically observe before probing for insights.
- **Faithful scoping of ambiguity:** Explicitly quoting the unfinished last sentence and offering three link interpretations (`matched`, `following`, `steward`) respects the founder's authority rather than silently hardcoding a guess.
- **Responsible guardrail addition:** Flagging MNPI and confidentiality risk is not scope creep; paying internet contributors for non-public information about listed equities like NVDA would expose Post Fiat and the founder to severe legal liability.

**Where the spec drifts from the founder's intention:**
1. **Prediction-market distortion of "useful information":** The founder asks for an *information hive mind* where an agent triangulates useful domain experience about a company or protocol. In institutional research, the highest-value mosaic insights are frequently **structural, operational, or qualitative** (e.g., switching-cost dynamics, supply-chain bottlenecks, customer dissatisfaction, protocol UX friction, shifts in MEV/validator behavior). By making "dated public checkable claim cards" the centerpiece of the architecture and punting non-checkable "context claims" to Open Question 10, the spec turns a qualitative expert-triangulation system into a clunky short-horizon forecasting game.
2. **"Ramped up on a company's history" reduced to a scrubbed static JSON brief:** R13 gives the agent only a cached summary JSON (`agent_brief_public.json` or `agent_brief_members.json`), with no direct historical corpus (past filings, earnings history, prior model evolution). For non-Finance-Hub contributors, stripping all model numbers and targets risks producing a generic chatbot rather than an agent deeply versed in the company's trajectory.
3. **Institutional compliance bureaucracy vs. rapid post-human experimentation:** The founder's overarching prompt emphasizes fast, legible primitives ("exploration of what truly post human activity looks like"). Requiring four human review roles, formal counsel sign-offs per mode, red-team suites, appeal queues, and ~35 policy answers before running even a pilot is badly mismatched to Post Fiat's stage and team velocity.
4. **Misaligned outcome framing:** The header claims Pillar 10 bubbles up to **Outcome A (Post-human governance in public)**, yet Steps 1 and 2 feed only private Finance Hub models, on-chain packets are deferred to Step 3, and Open Question 12 leaves public publication undecided because it threatens Finance Hub's exclusivity.

---

#### Technical robustness

**Genuine strengths:**
- Excellent audit of existing Task Node infrastructure (`024_network_task_profiles.sql`, `expert-badge.js`, `072_network_badges_identity_approvals.sql`, `chat-api-models.js`, `task-review-reward-worker.md`, `hive-group-worker.js`, `158_finance_agent.sql`).
- Correctly identifies critical codebase traps: ordinary chat loads Deep Memory and triggers the memory worker (which would contaminate user memory with interview material and leak Deep Memory to the company agent); `task-review-reward-worker` expects one terminal `pf.reward.v1` per task and alerts after 15 minutes; `158_finance_agent.sql` is migration-only and operator-scoped.
- Smart architectural separation keeping the interview service out of the research host and isolated from sealed models (R11).

**Major technical weaknesses:**
- **Information-extraction leak (R12):** Showing another contributor's existing claim during Phase 2—even after the current user answers and without author attribution—allows any eligible user (including non-Finance-Hub users) to self-initiate interviews across the coverage universe, give boilerplate answers, and extract fresh unpublished hive-mind claims.
- **Unworkable "deterministic resolution" and untrusted source ingestion (R15, R17):** Arbitrary user-named "public sources" cannot be resolved deterministically without pre-built structured source adapters/oracles; meanwhile, fetching user-specified URLs or open web pages for R15/R17 opens straightforward SSRF, source-spoofing, and indirect prompt-injection attacks against scoring and resolution.
- **MNPI screening false confidence and boundary gaps (R9, AC3):** Turn-by-turn LLM screening cannot reliably separate MNPI or NDA-covered commercial facts from legitimate public mosaic observations (it lacks private context and live public-record verification). Requiring 100% refusal on a static red-team suite creates dangerous false legal confidence, and R9 specifies screening "before any write" without explicitly gating *before* the turn reaches the interview LLM or hardening HTTP/APM/client logs.
- **Broken pseudonymity (R21 vs. R24):** R21 attaches the claim ID and resolution record as evidence on a standard Task Node reward task whose payout is public on the ledger. That creates a **direct link** between a pseudonymous claim and the contributor's public wallet—far worse than the timing-correlation caveat in R24.
- **Hidden dependencies and unbuilt runtimes (Build Order, R19):** Step 1 silently depends on Pillar 9 R19 (manifest, dual briefs, open questions) without listing it as a Step 1 blocker, and Step 2 assumes a working `158` research-host model agent even though `158_finance_agent.sql` currently has zero application code.
- **Reward-worker integration ambiguity (R21):** Spawning synthetic reward tasks at trigger time avoids the 15-minute alert, but the spec never defines whether `task-review-reward-worker` runs its normal verification LLM over them (risking overturning R17 verdicts) or bypasses verification (requiring an explicit signing/authorization trust boundary).

---

#### Economic coherence

The spec correctly diagnoses two real economic hazards—**AI-slop farming** (addressed via the R15 web/on-chain baseline) and **sycophancy/persuasion gaming** (addressed via paying on verification rather than owner approval). Unfortunately, its proposed mechanism breaks under basic adversarial and unit-economic scrutiny:

1. **Exploitable zero-downside payout structure:** Contributors post no stake and face no material penalty on `falsified` claims (at worst, lower weight in that area). Combined with a **consensus-challenge multiplier** (R21), this creates a free call option:
   - **Hedged-bet collusion:** Two colluding accounts (or two real people coordinating off-platform) submit *opposing* claims on a binary/directional check event. R16's Sybil burst detector looks for *near-identical* claims, not opposing hedges. One side is guaranteed to verify and collect the main tranche + contrarian multiplier while the other loses nothing.
   - **Base-rate harvesting:** Because payouts are fixed rather than scaled by surprise/prior probability (as in a proper scoring rule or market), contributors maximize expected earnings by dressing up high-base-rate outcomes (80–90% likely events) with bespoke primary-sounding anecdotes.
2. **The R7–R15 squeeze:** Forbidding non-public information (R7) while scoring anything findable by a web-and-chain-searching reviewer agent near zero (R15) leaves almost no valid ground for contributors unless "baseline" is carefully scoped to *already-synthesized consensus coverage* rather than raw public existence.
3. **Short-term noise bias:** R15 explicitly rewards nearer check dates, and deferred verification heavily discounts long-horizon payouts for task-economy workers. This drives contributors toward noisy short-term trivia rather than durable fundamental insight that actually improves multi-quarter Finance Hub models.
4. **Unmodeled human unit economics and supply-side risk:** R23 tracks only token/API compute (`cost_usd`) and reward payouts, omitting the far more expensive bottleneck: human time across model owners, compliance reviewers, resolvers, and appeal operators. Worse, the spec never validates whether Task Node's existing contributor population (largely crypto/AI builders and taskers) actually has non-trivial mosaic insight into Finance Hub's covered equities before committing to a massive engineering build.
5. **Treasury over-reservation deadlock (R22):** Reserving 100% of maximum potential main tranches at claim acceptance—when only a fraction will resolve `verified` and check dates may be months away—locks up the program envelope prematurely and halts new interviews, with no specified release or rollover accounting.

---

#### Problems

##### P1 — Unbonded verification payouts invite hedged-bet collusion, base-rate harvesting, and short-term forecasting bias
- **Severity:** Critical
- **Where:** *In one paragraph*; R14; R15; R16; R21; *Economics*
- **What is wrong:** Contributors earn a positive bounty (plus a consensus-challenge multiplier) when a claim resolves `verified`, while risking no stake and suffering no real financial penalty on `falsified`. Payouts are not scaled by prior probability (not a proper scoring rule), and R15 explicitly awards higher scores to nearer check dates.
- **Why it matters:** Rational or adversarial actors can farm the treasury by (1) coordinating opposing directional claims across two accounts so one is guaranteed to win (invisible to R16's identical-burst Sybil check), (2) submitting high-base-rate "safe" predictions wrapped in bespoke anecdotes, and (3) flooding the system with near-term noisy metrics instead of medium-term fundamental research that actually helps Finance Hub models.
- **How to investigate:** Model expected payoff under (a) two-account opposing hedges, (b) 80%-prior claim farming, and (c) genuine 6-month fundamental insights with realistic time-discounting. Inspect `sybil-review-detection.md` to confirm whether it can detect low-volume two-account collusion across real identities (almost certainly not).
- **Suggested direction:** Replace flat per-claim bounties with either (a) **reputation-weighted periodic payouts** (e.g., quarterly/monthly pool split by net calibration / proper scoring rule—Brier or log score—where falsified claims actively reduce a contributor's period payout, eliminating single-claim hedged-bet profitability), or (b) **small PFT stake/bond or reputation escrow** on contrarian/high-payout claims; evaluate claims against an explicit prior probability (market-implied or model-implied) so trivial high-prior bets pay near zero; stop rewarding shorter check horizons by default.

##### P2 — Operational deadlocks from four human review roles, omitted labor costs, and unvalidated contributor supply fit
- **Severity:** Critical
- **Where:** R10; R17; R18; R19; R23; *Build order*; Open questions 2, 8
- **What is wrong:** The workflow creates four distinct human bottlenecks: model owners (R18/R19), compliance reviewers (R10/R18), resolvers for non-deterministic claims (R17/R18), and Finance Hub appeal/fallback operators (R17/R18)—plus ongoing curation of fresh `open_questions` and versioned vocabularies per company (R1/R13). R23 omits human-hour costs from ROI tracking, and Step 1 builds almost the entire pipeline before validating whether Task Node users actually possess valuable mosaic alpha on covered assets.
- **Why it matters:** For a small team, manual compliance review, manual verdict resolution, and manual owner model updates will immediately bottleneck at low volume or degrade into rubber-stamping. If human labor costs exceed the value added to Finance Hub subscriptions/validators—or if Task Node's contributor pool only has edge in crypto/AI protocols rather than listed equities—the entire pillar is economically underwater.
- **How to investigate:** Query the live `network_task_profiles` table (`024_network_task_profiles.sql`) and intersect `domain_expertise` (the 5–10 mapped public companies per user) against Finance Hub's actual coverage universe to measure real overlap and contributor depth. Estimate human minutes per claim across all four roles at 50, 200, and 1,000 monthly claims.
- **Suggested direction:** Run a **Wizard-of-Oz / concierge pilot first** on 3–5 crypto/protocol or AI-infra assets where Task Node overlap is highest, with one combined operator role and a small manual stipend, before building automated clustering, resolution workers, and multi-role queues. Add operator/reviewer hours explicitly to R23 unit-economics metrics. Collapse roles in early phases (e.g., compliance + resolver + fallback operator as one defined steward role with strict volume caps).

##### P3 — Core contradiction between R7 (strictly public data) and R15 (zero novelty if found by web/chain baseline), plus marginalization of qualitative mosaic insights
- **Severity:** Critical
- **Where:** *In one paragraph*; *User story*; R7; R14; R15; R21; Open question 10
- **What is wrong:** R7 restricts contributors to public observations ("public prices and datasets, products bought openly, open-source code, on-chain data"), yet R15 instructs the reviewer agent to search "the web and chain data" for a public baseline and score anything already in that baseline near zero. Simultaneously, making dated checkable claim cards the primary rewarded artifact sidelines qualitative/structural domain insights ("context claims") into an unresolved footnote (Open Question 10).
- **Why it matters:** Any reproducible public/on-chain evidence (e.g., a GitHub commit, a public cloud pricing page, a Dune metric, a job-board trend) *exists on the web/chain*; if the reviewer agent searches the cited artifact or nearby keywords and penalizes novelty to zero, legitimate public mosaic contributors get rejected. Conversely, forcing qualitative operational insights into binary dated-event bets either excludes the best domain experts or yields contrived, low-value predictions.
- **How to investigate:** Draft 15–20 realistic high-value mosaic observations (across listed equities like NVDA and protocols like Solana) that pass R7 compliance, then test them against a web-search LLM prompt and try converting each into an R14 checkable claim card. Measure what percentage fail novelty or cannot cleanly map to a dated deterministic check.
- **Suggested direction:** Redefine the R15 baseline specifically as **"mainstream consensus and model brief"** (company filings, analyst summaries, top news, and the current Finance Hub brief), explicitly *excluding* raw/dispersed primary public observations (field pricing, developer/customer experience, obscure commits, cross-source synthesis). Create a first-class workflow and explicit evaluation rubric for **qualitative mosaic briefs** (e.g., corroborated operator/builder channel checks) rather than treating everything non-binary as an afterthought.

##### P4 — R12 Phase 2 creates a free-rider information-extraction leak of unpublished hive-mind claims
- **Severity:** Major
- **Where:** R12; R13; R16
- **What is wrong:** R12 Phase 2 allows the interview agent to describe an existing contributor's claim after the current contributor answers ("without its author; any agreement is marked `prompted`"), while also allowing contributors (including non-Finance-Hub users under R13) to self-initiate interviews.
- **Why it matters:** A low-effort or adversarial user can initiate interviews across every covered company, give generic answers in Phase 2, and harvest fresh, unpublished, potentially pre-compliance-reviewed claims contributed by real experts—undermining Finance Hub exclusivity and leaking unvetted (possibly sensitive) claims across users. It also introduces sycophantic `prompted` agreement that R16 already acknowledges is low-value.
- **How to investigate:** Trace the data flow of `hive_claims` back into Phase 2 prompts and check whether unvetted (`pending`) or members-only claims can be surfaced to non-members or arbitrary self-initiated threads.
- **Suggested direction:** Remove cross-contributor claim disclosure from Phase 2 entirely. Keep interviews strictly independent (maximizing wisdom-of-crowds signal and eliminating the extraction vector). If targeted follow-up on a contested hypothesis is needed, synthesize it into an anonymized, compliance-cleared update to the company's `open_questions` during the hub publish cycle (and respect member vs. public tiers).

##### P5 — R17 "deterministic resolution" over arbitrary user-named sources is technically unworkable and vulnerable to source spoofing and indirect prompt injection
- **Severity:** Major
- **Where:** R14; R15; R17; AC8
- **What is wrong:** R14 lets claim cards carry an arbitrary "named public source," and R17 asserts that "if the named source gives a number, a deterministic rule decides and records a hashed snapshot." No curated source registry or parser catalog exists to turn arbitrary URLs/documents (PDFs, earnings releases, web pages) into deterministic rules. Meanwhile, both R15 (web search) and R17 (fetching named sources) ingest untrusted external web content into LLM agents without defense against source spoofing or indirect prompt injection.
- **Why it matters:** First, AC8 ("resolves identically on re-run") cannot be satisfied for arbitrary web sources without either limiting claims to a tiny set of structured APIs/oracles or quietly relying on non-deterministic LLM parsing. Second, an attacker can name a controllable or low-credibility web page/blog/on-chain pool as the source, manipulate its value on the check date, or embed prompt-injection payloads in the page to force a `verified` verdict or high R15 novelty score.
- **How to investigate:** Inventory what data sources Finance Hub's research host already ingests deterministically (e.g., SEC EDGAR XBRL, market-data APIs, Dune/RPC queries) and test how many realistic claims can actually map to structured endpoints. Threat-model indirect prompt injection through web snippets into the R15 and R17 agents.
- **Suggested direction:** Restrict deterministic auto-resolution strictly to an **allowlisted registry of canonical, schema-typed source adapters** (e.g., EDGAR XBRL metrics, verified RPC/on-chain queries, vetted market-data APIs) with fixed query templates; route all other sources to human-in-the-loop resolution with structured evidence extraction and strict prompt-injection isolation (sandboxing fetched text as untrusted data, never allowing user-supplied arbitrary URLs to trigger auto-payouts).

##### P6 — R9 MNPI screening gateway creates false legal confidence and has pipeline-ordering and logging-leakage gaps
- **Severity:** Major
- **Where:** R6; R7; R9; AC3
- **What is wrong:** R9 relies on pattern rules plus a turn-level screening LLM to keep MNPI and confidential information out of the system, and AC3 requires 100% refusal on a fixed red-team test set. Additionally, R9 specifies screening "before any write" rather than *before the turn is sent to the interview LLM*, does not specify how the agent explains "why" a line was dropped without seeing the refused content, and does not account for HTTP reverse-proxy logs, APM/error traces, or browser local storage.
- **Why it matters:** Turn-level LLMs cannot reliably tell whether a specific fact ("our cloud vendor cut H100 rates 18% this week") is public pricing or an NDA-violating commercial disclosure; passing a fixed red-team set creates dangerous legal overconfidence while real-world tuning will either miss disguised MNPI or falsely refuse valid mosaic insights. Furthermore, sending raw user input to the interview LLM or logging it in Express/proxy middleware before screening violates the stated zero-retention boundary.
- **How to investigate:** Inspect `chat.md`, `chat-api-models.js`, and Task Node's Express/API middleware and error-tracking stack to identify every place request bodies or error payloads are logged or cached. Consult legal/compliance benchmarks for expert-network controls (e.g., GLG/AlphaSights-style upfront screening vs. post-hoc automated text filters).
- **Suggested direction:** Treat automated screening as **defense-in-depth, not the primary legal barrier**—the real legal boundary must be **asset-class scoping** (start with open crypto protocols/on-chain ecosystems where securities MNPI risk is lower), **strict relationship exclusions** (R8), and **clear contractual/UI prohibitions**. Architecturally, mandate synchronous screening *before* the interview LLM is invoked, generate refusal explanations strictly from deterministic screening reason codes (never passing refused text to the interview model), disable body logging/APM capture on interview routes, and replace AC3's static "100% pass" test with held-out adversarial evaluation plus false-positive rate thresholds so UX usability is also tested.

##### P7 — R21 task evidence directly breaks R24 pseudonymity on the public ledger
- **Severity:** Major
- **Where:** R20; R21; R24; Open question 13
- **What is wrong:** R24 warns that until shielded payouts exist (Pillar 5), pseudonymous contributors are hidden from readers and only linkable "by timing." However, R21 states that each reward tranche is created as a normal Task Node task *"with the claim and resolution record as evidence,"* while *What exists today* notes that Task Node payouts (`pf.reward.v1`) are public on the ledger.
- **Why it matters:** Attaching the claim ID and resolution record as task evidence creates a **direct, deterministic public link** between the contributor's main Task Node wallet and their pseudonymous claim—completely defeating pseudonymity even against casual observers. High-signal industry participants (builders, operators, customers) who fear commercial retaliation will refuse to participate if their claims are trivially doxxed.
- **How to investigate:** Inspect `task-review-reward-worker.md` and the `pf.reward.v1` memo/IPFS schema to verify exactly what task evidence fields are published on-chain or exposed in public Task Node APIs/block explorer views.
- **Suggested direction:** Never place raw claim IDs, company identifiers, or resolution text in public task evidence or on-chain reward memos for pseudonymous claims. Use opaque, salted commitments (or internal operator-only evidence pointers) in the reward task payload, and explicitly align R21 with R24's privacy guarantees (or restrict pseudonymous rewards to batched/periodic aggregate payouts until Pillar 5 shielded payouts ship).

##### P8 — Over-engineered Step 1 pilot gated by 35+ policy sub-questions and hidden cross-pillar / unbuilt-runtime dependencies
- **Severity:** Major
- **Where:** *What exists today*; R1; R13; R19; *Build order*; *Open questions 1–13*
- **What is wrong:** Step 1 ("standalone pilot") requires implementing R1–R16, R18, R23, and private R25—nearly the entire complex architecture—yet explicitly omits rewards unless Q9 is also answered, leaving contributors with unclear motivation to endure the pilot. Meanwhile, Step 1 claims it only "needs open questions 1–8," hiding its hard prerequisite on **Pillar 9 R19** (manifest, dual briefs, open questions) and grouping ~35+ separate sub-questions into 13 bullets. Step 2 likewise depends on a working `158` research-host model agent that currently has zero code behind its migration.
- **Why it matters:** A spec blocked on 35+ founder/legal decisions before a single line of pilot code can run—and requiring an unbuilt Pillar 9 dependency and an unbuilt `158` agent runtime—will stall indefinitely or absorb months of engineering before testing the core hypothesis ("can an agent triangulate valuable company insights from Task Node users?").
- **How to investigate:** Check Pillar 9's actual implementation status for R19 (`coverage_manifest.json`, `agent_brief_*.json`, `open_questions`) and inspect `158_finance_agent.sql` plus the research-host repository to confirm how much of the model-agent runtime exists.
- **Suggested direction:** Split Step 1 into a **Phase 0 validation spike** (no new 6-table schema, no automated reviewer/clustering/screening gateway—just manually curated open questions for 3–5 crypto/protocol assets, isolated interview chat mode, manual review, and simple existing network-task bounty) that needs only 3–4 founder decisions. Explicitly document Pillar 9 R19 and the `158` agent runtime as hard external milestones on the build-order diagram, and provide opinionated **recommended defaults** for each open question so the founder can approve by exception rather than designing 35 policies from scratch.

##### P9 — R22 full-tranche reservation logic causes budget deadlocks, and R21 synthetic reward tasks create a verification-worker conflict
- **Severity:** Major
- **Where:** *What exists today*; R21; R22; AC10
- **What is wrong:** (1) R22 reserves each claim's full main tranche at acceptance so deferred rewards stay funded, and stops new paid interviews once the period envelope is spent—without specifying fractional/expected-value reserves, release of unspent reserves on `falsified`/`unresolvable` claims, or cross-period rollover. (2) R21 pushes synthetic tasks created at trigger time onto the "normal Task Node path" (`task-review-reward-worker`), without specifying whether the normal LLM task verifier re-evaluates the claim or is bypassed.
- **Why it matters:** (1) If 30–40% of claims verify and contrarian multipliers apply, reserving 100% of max payout locks up 2.5x–4x actual spend for months, prematurely starving the program of new interviews. (2) If `task-review-reward-worker` runs its normal LLM verifier on R21 synthetic tasks, it can randomly reject already-resolved claims (breaking R17 finality); if it bypasses verification, an explicit authorization/signature check is needed so a bug or exploit in the hive-mind service cannot drain PFT.
- **How to investigate:** Inspect `task-review-reward-worker.md` and `072_network_badges_identity_approvals.sql` / `network-badges.js` to check how pre-verified or programmatic reward tasks are currently authorized, capped, and audited.
- **Suggested direction:** Specify explicit reserve-pool accounting (e.g., rolling liability ledger where `falsified`/`unresolvable` immediately releases reserved funds back into the active envelope, or reserving at expected payout × safety factor backed by a protocol buffer). Explicitly define a dedicated, cryptographically/role-authenticated reward-task type (or pre-verified attestation hook) inside `task-review-reward-worker` that enforces badge caps (`072`) without re-litigating R17 verdicts.

##### P10 — Tension between "ramped up on a company's history" and R13's static, scrubbed briefs
- **Severity:** Major
- **Where:** *User story*; R3; R12; R13; AC5
- **What is wrong:** The founder wants an agent "ramped up on a company's history" that triangulates what a user knows. R13 restricts the agent to a cached static JSON brief—and for non-Finance-Hub contributors (`agent_brief_public.json`), strips all model numbers and targets—with no access to historical filings, prior earnings transcripts, or changelog depth.
- **Why it matters:** Two failure modes emerge: either `agent_brief_public.json` is scrubbed so hard that the agent asks shallow, textbook questions (wasting domain experts' time and failing triangulation), or its `open_questions` stay specific enough to be useful—in which case the questions themselves indirectly reveal the Finance Hub analyst's proprietary thesis and focus areas to non-members (undermining AC5's secrecy goal).
- **How to investigate:** Draft an actual pair of `agent_brief_members.json` and `agent_brief_public.json` files for one covered equity and one covered protocol using real Finance Hub data, and run mock Phase 1/Phase 2 interviews to test both conversational depth and thesis leakage.
- **Suggested direction:** Give the interview agent read-only retrieval (RAG) over **strictly public historical company artifacts** (past public filings, public earnings transcripts, protocol governance forums, public post-mortems) separate from the sealed model, so it is genuinely "ramped up on a company's history" without touching proprietary Finance Hub numbers. In Phase 2, also allow open-ended anomaly discovery ("what is the market currently misunderstanding about X?") rather than anchoring solely to pre-written `open_questions`.

##### P11 — Nine-step contributor onboarding gauntlet and Deep-Memory pre-fill consent trap
- **Severity:** Minor
- **Where:** *User story*; R2; R5; R8; R12
- **What is wrong:** Before submitting a single insight, a contributor must confirm suggested areas, confirm information types, write/edit and per-field approve an experience summary, choose/accept a link, declare a company relationship, sign a legal attestation, complete Phase 1 triangulation, complete Phase 2 questioning, and review/edit/approve Phase 3 claim cards. Additionally, R2 seeds candidates from `024_network_task_profiles.sql` (which is built from Deep Memory/context docs), creating a risk that if the UI pre-fills the R5 experience summary from those profiles, users clicking through will inadvertently leak Deep Memory summaries into the contributor brief.
- **Why it matters:** High upfront friction will severely depress conversion among the exact busy, experienced operators/builders the hive mind wants to attract; sloppy pre-filling of the brief could silently violate R5's privacy guarantee.
- **How to investigate:** Review the existing `capability-profiles.js` UI/UX flow and `network_task_profile_v2.md` output fields to see what is already safely structured vs. derived from private Deep Memory prose.
- **Suggested direction:** Separate **one-time profile opt-in** (confirming coarse vocabulary tags only—never pre-filling free-text experience summaries from Deep Memory) from a **lightweight 2-click per-interview entry** (relationship + attestation modal straight into chat), and let Phase 1 conversationally build the brief rather than forcing form-filling upfront.

##### P12 — Unresolved structural conflict between Outcome A (public governance) and Pillar 9 (Finance Hub exclusivity)
- **Severity:** Minor
- **Where:** *Bubbles up to*; R20; *Economics*; Open question 12
- **What is wrong:** The page frames Pillar 10 as bubbling up to **Outcome A (Post-human governance in public)**, yet its economic justification relies on making Finance Hub exclusive enough to drive validator and top-30 demand, and it punts cluster-summary publication to Step 3 / Open Question 12 without proposing a resolution.
- **Why it matters:** Downstream specs (Pillar 1 governance packets and Pillar 9 Finance Hub) cannot reliably plan around Pillar 10 outputs until this boundary is resolved; making summaries public immediately erodes Finance Hub's moat, while keeping them permanently private breaks the Outcome A governance story.
- **How to investigate:** Cross-check Pillar 1 (`01-hive-governance.md`) and Pillar 9 (`09-finance-hub.md`) to see what concrete artifacts each expects from Pillar 10 and on what cadence.
- **Suggested direction:** Propose a clean default split in the spec: **protocol/Post Fiat ecosystem insights** publish immediately as public Pillar 1 governance packets (advancing Outcome A), while **covered equity/market research clusters** remain Finance Hub members-only until an embargo window expires (e.g., post-earnings or post-check-event), after which verified track-record summaries publish publicly as proof of the hive mind's accuracy (marketing Finance Hub without giving away live alpha).

## Claude Opus 5.5 — 66/100

*Model `anthropic/claude-opus-5.5` · 10507 output tokens*

#### Summary judgement

This is a careful, unusually self-aware spec. It reads the founder's short, ambiguous Deliverable 10 faithfully and marks its own interpretations. It grounds itself in real Task Node components: profiles, badges, the zero-retention chat routing, the reward worker, Sybil review and the 158 migration. It also identifies the largest real-world hazard, which the founder never mentions: soliciting material non-public information (MNPI) or confidential information about covered companies. Its privacy and isolation boundaries (R5, R6, R11) and acceptance tests are concrete and testable.

The score is held down by four things:

1. **The core incentive is not sound.** The reward mechanism ("pay when a claim's check event confirms it") is effectively a stake-free prediction market. Without a proper scoring rule, it rewards spraying cheap directional claims.
2. **It is heavily human-gated.** The design has several human roles and many open questions, which cuts against the "post-human" story that is the reason for the whole programme.
3. **It defaults to private chat.** Interviews run in private Task Node chat, and the connection to public governance arrives only in Step 3. The founder wants the network's activity to be public.
4. **Several mechanics are underspecified or contradictory.** These include synthetic reward tasks, Sybil semantics, the pseudonymity claims, the screening data flow, and immutable encrypted pinning of claims.

The build order also promises independence ("each step works under any answer to questions it does not need"). In practice, Step 1 is blocked on eight open questions, including counsel sign-off, and on Pillar 9's unbuilt coverage manifest.

#### Correspondence to the founder's intention

**What maps well:**
- **"I have done a large number of tasks… I am classified."** R2 builds classification from task-history profiles, badges and capability profiles, and has the contributor confirm each candidate. Open question 7 asks what "a large number" means. This is faithful.
- **"Via chat."** Interviews happen through a chat mode (R6, R12).
- **"An agent who is ramped up on a company's history can triangulate with you."** This maps directly to the per-company agent brief (R13) and Phase 1 triangulation with a relevance score (R4).
- **"The coverage universe in finance hub is linked to specific users."** R3 offers matched, following and steward link types. Open question 1 honestly flags that the sentence is ambiguous.
- **Tie to the overall thesis.** The "Bubbles up to" line and R20 connect cluster summaries to Pillar 1 governance packets. This echoes "information packets are uploaded and acted upon."

**Where it drifts:**
- **Private versus public.** The founder's whole programme is framed around public, visible post-human activity, with the Hive Chat as the user experience. The spec reads "via chat" as private Task Node chat and keeps interviews private. Cluster summaries may be unpublished (open question 12), and any public output arrives only in Step 3. Privacy is defensible for MNPI reasons, but the spec does not offer a public-by-default layer for contexts where the information is plainly public, such as on-chain or protocol data. Examples would be public Hive Chat threads per covered protocol, or public claim cards.
- **Post-human versus human-heavy.** The design names several human roles: model owner, compliance reviewer, resolver, steward and Finance Hub operator queue. It also calls for counsel sign-off and human review before pooling. The founder wants observers to say "this is an exploration of what truly post-human activity looks like." The spec notes the tension in open question 8 but defaults to humans everywhere.
- **Scope inflation.** The founder wrote four sentences. The spec has 25 requirements, 13 open questions, a reward-tranche economy, an appeals process and a compliance regime. Much of this is warranted. However, the founder likely wanted a lightweight first version: an agent per company plus chats with qualified users. Instead the minimum viable path is long.
- **Hive mind as network knowledge.** The founder's framing suggests the network's collective knowledge, presented in public. The spec turns it into a research-desk claim pipeline whose main consumer is Finance Hub's private models, with a validator-only audience. That fits "coverage universe in finance hub" but narrows the vision.

#### Technical robustness

**Strengths:**
- **Credential isolation.** The interview service holds no model-host credential (R11) and runs in a separate chat mode with no context document or memory (R6).
- **Consented brief.** The agent sees only a contributor brief whose fields the contributor approves (R5).
- **Order of operations.** Screening happens before any write, and the system fails closed when the screening model is unavailable (R9).
- **Reproducible judgements.** LLM judgements are stored with model id, prompt version and input hash (R15).
- **Deterministic resolution.** Numeric sources are resolved by a rule and the source is snapshotted (R17).
- **Testable acceptance criteria.** Several criteria (3–5, 7) are concrete tests.

**Weaknesses:**
- **Screening data flow.** It is ambiguous whether the contributor's raw turn reaches the interview model or a third-party provider before screening completes.
- **MNPI detection by LLM.** This is inherently unreliable. A 100% pass on a pre-published red-team set gives false assurance.
- **Immutable pinning.** Claims are pinned encrypted to IPFS, which conflicts with any need to delete or claw back screened content later.
- **Pseudonymity claims.** The stated protection is overstated, because ledger payouts and reward memos may link directly to the claim.
- **Synthetic reward tasks.** Creating a task per tranche fits the one-reward-per-task invariant. Its interaction with verification, Sybil review and top-30 ranking is unexplored.
- **Changed Sybil semantics.** "An open flag withholds reward tasks" contradicts the live system's "recommend-only, never blocks" design.
- **Unbuilt dependencies.** Pillar 9 R19 (the manifest and briefs) and 158 (migration only, no code) both underpin the work and are not built.
- **Source reliability.** Resolution against contributor-named sources is open to source shopping and manipulation, and scraping arbitrary sources deterministically is brittle.

#### Economic coherence

The stated logic is: pay for costly-to-gather public observations, strip what is freely findable via a baseline, and pay on verification to strip persuasion. That is directionally right, but the mechanism has gaps.

- **Spraying is profitable.** Directional or threshold claims with no stake and no proper scoring rule have positive expected value. A falsified claim costs only weight in that area. The consensus-challenge multiplier makes contrarian spraying more profitable still.
- **"Verified" is not "valuable."** A claim that matches what the market already expected verifies, but it adds nothing. Payment should depend on how much better the claim did than consensus or the model, not on binary truth.
- **Acceptance is underdefined.** Novelty is scored, but it is unclear who accepts a claim and at what threshold. Acceptance gates both reservation and the acceptance credit.
- **Reservation locks budget.** Reserving each main tranche at acceptance against long-dated check events can tie up a whole period's envelope.
- **No funding source.** Is it PFT emissions or treasury? The return to the token or network is asserted ("Finance Hub is the reason validators join") rather than modelled.
- **Weak value metric.** "Cited changes moved estimates toward outcomes" is statistically underpowered at pilot scale.
- **Per-claim costs are unmeasured.** Each claim incurs web search for the baseline, an LLM screening call and interview tokens. These may exceed rewards for low-value claims. R22 caps compute per company, but no cost-per-claim target exists.

#### Problems

**P1 — Reward-on-verification is gameable without a proper scoring rule**
- **Severity:** critical.
- **Where:** R14 (the check: "metric, direction or threshold"), R17, R21 (main tranche, challenge multiplier, falsified claims only lower weight), open question 9.
- **What is wrong:** Claims are effectively binary forecasts with no stake and no penalty beyond lost weight. A contributor can submit many cheap directional claims near 50/50 and collect on the hits. The multiplier on verified consensus-challenging claims raises the payoff of contrarian guessing.
- **Why it matters:** The budget will flow to volume and luck rather than information, and R23's "share verified" figures will look fine while value is near zero.
- **How to investigate:** Simulate contributors with no information against the policy at various caps and multipliers, and compute expected PFT per claim.
- **Suggested fix:**
  - Require a probability or point estimate and score it with a proper scoring rule (Brier or log), measured relative to the model's or consensus prior.
  - Pay on improvement over the baseline forecast, not on binary truth.
  - Make falsified claims net-negative on future eligibility and credits.
  - Apply the challenge multiplier only through the scoring rule's natural payoff for beating the prior, not as a flat bonus.

**P2 — "Verified" conflates truth with informational value; acceptance path undefined**
- **Severity:** major.
- **Where:** R15–R18, R21, open question 8.
- **What is wrong:** R15 scores novelty, but the spec never states what score threshold accepts a claim or who applies it: the reviewer agent, the model owner or the compliance reviewer. Claims whose check events are near-certain, or that restate the current public state, can verify trivially.
- **Why it matters:** Acceptance gates reservation and the acceptance credit, and trivial verified claims drain the budget.
- **How to investigate:** Trace a claim from approval to `accepted` and list each decision point.
- **Suggested fix:**
  - Specify the acceptance rule explicitly (score thresholds and a role) and make novelty against the baseline a hard gate.
  - Require check events to be future-dated and uncertain against the prior.

**P3 — The post-human thesis is undercut by pervasive human gating**
- **Severity:** major.
- **Where:** R18 roles, R10 counsel sign-off and pre-pooling human review, R17 resolver, R19 owner approval, open question 8.
- **What is wrong:** Every stage is human-approved by default. The founder's framing is that Post Fiat explores post-human governance.
- **Why it matters:** The pillar won't demonstrate the narrative it "bubbles up to", and human roles become bottlenecks at scale.
- **How to investigate:** Count the human touchpoints per claim and estimate operator hours per 100 claims.
- **Suggested fix:**
  - Define an agent-first default path for low-risk modes, for example protocols with on-chain data, with deterministic resolution and agent acceptance.
  - Keep human review as the exception, triggered by risk score or asset class.
  - State this explicitly as the post-human mode.

**P4 — Private-by-default contradicts the founder's public framing**
- **Severity:** major.
- **Where:** the "via chat" interpretation, R6, R12, R20, open question 12, Build order Step 3.
- **What is wrong:** Interviews are private and summaries may never be published. Public output arrives only in Step 3, which depends on other pillars.
- **Why it matters:** An observer of Post Fiat sees nothing of the hive mind for a long time.
- **How to investigate:** Check the founder's intent for Hive Chat as the public surface (Deliverables 1 and 6). Also decide which asset classes carry negligible MNPI risk.
- **Suggested fix:**
  - Add a public mode for protocols and on-chain topics: public Hive Chat threads or public claim cards with attribution.
  - Bring a public cluster-summary feed into Step 1 or Step 2 for those modes.

**P5 — Build order "independence" is not real; Step 1 is blocked**
- **Severity:** major.
- **Where:** Build order Step 1 ("Needs open questions 1–8"), R1 (depends on Pillar 9 R19), the R10 approval record.
- **What is wrong:**
  - Step 1 cannot start without answers on legal review, asset classes, relationships, refused-content policy and approvers.
  - It also needs Pillar 9's coverage manifest and briefs, which the spec admits do not exist in Task Node.
- **Why it matters:** The pillar may stall indefinitely, and "early" status becomes permanent.
- **How to investigate:** Check Pillar 9's build status for R19, and list which open questions have safe defaults.
- **Suggested fix:**
  - Define a Step 0 with safe defaults: protocols only, no current or former employees, hash-only refusals, no payouts and an operator-run manifest.
  - Under those defaults, run the Step 0 pilot without counsel sign-off or the Pillar 9 build.

**P6 — Screening data flow is ambiguous; LLM-based MNPI detection is over-trusted**
- **Severity:** major.
- **Where:** R9, user story step 5 ("keeps none of the text"), acceptance criterion 3.
- **What is wrong:**
  - "Held in memory and screened before any write" does not say whether the raw turn reaches the interview model or provider before screening.
  - The user story says none of the text is kept, while open question 6 contemplates keeping it in a restricted log. These conflict.
  - Passing every item of a known, pre-published red-team set every time can be achieved by tuning to the set. It is no evidence of real-world recall.
- **Why it matters:** This is legal exposure, the exact risk the spec exists to manage.
- **How to investigate:** Draw a sequence diagram of one turn: client, screener, interview model, storage. Measure screening recall on a held-out adversarial set.
- **Suggested fix:**
  - Make the order explicit: screen first, and send the turn to the interview model only on pass.
  - Use a held-out red-team set and report recall and precision.
  - Align the user-story wording with open question 6.
  - Treat screening as risk reduction rather than a guarantee, and pair it with conservative question design.

**P7 — Immutable encrypted pinning conflicts with compliance**
- **Severity:** major.
- **Where:** R20.
- **What is wrong:**
  - Every accepted claim is pinned to IPFS as an envelope encrypted to three parties.
  - If a claim is later found to contain MNPI or personal data, or the contributor requests deletion, pinned content cannot be reliably removed.
  - Ciphertext persists, so key compromise exposes it all.
- **Why it matters:** This contradicts the promise that the network "never holds" confidential information, and may conflict with data-protection rights.
- **How to investigate:** Review Pillar 2's pinning and retention model, and legal requirements for erasure.
- **Suggested fix:**
  - Anchor hashes only, and keep claim bodies in deletable storage.
  - Alternatively, pin only cluster summaries after a cooling-off period, with crypto-shredding (destroying the keys) as the deletion path.

**P8 — Pseudonymity guarantees are overstated**
- **Severity:** major.
- **Where:** R24, open question 13, R21 (reward tasks with the claim as evidence, payouts public on the ledger).
- **What is wrong:**
  - The UI warns only about timing linkage.
  - A `pf.reward.v1` paid to the contributor's known wallet, for a task whose evidence references the claim, may link the pseudonym to the account directly. An operator-attested credential also reveals the area of expertise.
- **Why it matters:** Contributors could be identified, which can create employer or legal risk for them.
- **How to investigate:** Inspect the reward memo contents and task metadata visible on the ledger and block explorer.
- **Suggested fix:**
  - Pay pseudonymous claims from batched, delayed payouts that carry no claim references.
  - Strip claim ids from public memos.
  - Make the UI disclose the actual linkage vectors.

**P9 — Synthetic per-tranche tasks may distort the existing task and reward system**
- **Severity:** minor (could become major).
- **Where:** R21.
- **What is wrong:** Creating a task when a trigger fires sidesteps the one-reward-per-task rule and the 15-minute alert. It is unclear:
  - who answers the mandatory verification request;
  - whether these tasks count toward the expert badge (at least 20 rewarded tasks), top-30 rankings or profile generation;
  - how Sybil review treats them.
- **Why it matters:** Claim rewards could inflate eligibility loops. Verifying by a rubber-stamp agent would weaken the existing guarantees.
- **How to investigate:** Read `task-review-reward-worker.md` and `expert-badge.js` to see how rewarded tasks are counted.
- **Suggested fix:**
  - Add a distinct task type, excluded from ranking unless open question 11 says otherwise.
  - Define its verifier (the resolution record) explicitly.

**P10 — Change to Sybil review semantics is unacknowledged**
- **Severity:** minor.
- **Where:** R16, acceptance criterion 7, versus the "What exists today" row (recommend-only, never blocks).
- **What is wrong:** The spec makes an open flag withhold corroboration and reward tasks. That is a blocking behaviour the current system deliberately avoids.
- **Why it matters:** False positives will withhold honest rewards with no defined unblock path.
- **How to investigate:** Check why Sybil review is recommend-only, and whether it has a resolution workflow.
- **Suggested fix:** Specify a flag-review service level and an unblock path, or make withholding time-limited.

**P11 — Resolution against contributor-named sources invites source shopping and brittle scraping**
- **Severity:** major.
- **Where:** R14 (check: "named public source"), R17.
- **What is wrong:**
  - Contributors pick the source that will judge them; some sources are manipulable or thin.
  - A deterministic rule over arbitrary web sources needs per-source parsers.
  - "Source stays silent" ends as `unresolvable` with no penalty, so failure is free.
- **Why it matters:** Resolution integrity is the foundation of the economics.
- **How to investigate:** Sample likely claim types, such as GPU rates, token on-chain metrics and earnings line items, and assess source availability.
- **Suggested fix:**
  - Maintain a curated, versioned source registry per manifest entry, so contributors choose from it.
  - Prefer on-chain and official-filing sources.
  - Count `unresolvable` claims against checkability scores.

**P12 — Budget reservation for long-dated checks can starve the programme**
- **Severity:** minor.
- **Where:** R22.
- **What is wrong:** Reserving each main tranche at acceptance against far-future check dates can lock a period's envelope, which stops new interviews.
- **Why it matters:** This creates throughput collapse and stale engagement.
- **How to investigate:** Model the distribution of check-date horizons against the envelope.
- **Suggested fix:**
  - Cap check horizons per mode.
  - Reserve at expected value rather than maximum.
  - Allow cross-period rollover.

**P13 — Funding source and value capture are unmodelled**
- **Severity:** major.
- **Where:** the Economics preamble, R22, R23, open question 9.
- **What is wrong:**
  - Nothing specifies who pays: PFT emissions, treasury or Finance Hub subscriptions.
  - The claim that better models attract validators is asserted, not tied to measurable validator acquisition.
  - The R23 "estimates moved toward outcomes" metric is statistically weak at pilot scale.
  - The per-claim compute cost of interview, screening and baseline search is not estimated against expected reward value.
- **Why it matters:** Without these, the programme can't be judged economically coherent or sized.
- **How to investigate:**
  - Estimate per-interview LLM and search cost using the 158 `cost_usd` patterns.
  - Estimate value per verified claim.
  - Connect the economics to the validator-programme economics in Pillar 3 and Pillar 9.
- **Suggested fix:** Add a unit-economics table (cost per claim, target spend per verified claim) and name a funding source with a hard cap.

**P14 — Contamination from the members' brief via Ask the agent**
- **Severity:** minor.
- **Where:** R13, R25 (Finance Hub members open interview mode via ask-agent).
- **What is wrong:** Members who have seen model numbers and targets give "independent" claims anchored on the model. Phase 2's independence rule covers other contributors' claims, not the model itself.
- **Why it matters:** Corroboration and novelty become inflated by echoing the model.
- **How to investigate:** Check whether members' interviews are flagged differently.
- **Suggested fix:** Mark member-originated claims as model-exposed and weight them like `prompted` in pooling.

**P15 — Interview depth and cost are unspecified**
- **Severity:** minor.
- **Where:** R12 ("under a cap on turns and tokens"), R22.
- **What is wrong:** There are no target interview lengths or cost ceilings, and no definition of how the agent helps a contributor turn an observation into a checkable claim.
- **Why it matters:** Most contributors can't author good checks unaided, so claim yield will be low.
- **How to investigate:** Run a small manual pilot: five contributors, two companies.
- **Suggested fix:**
  - Specify default caps.
  - Provide claim-card templates per information type, drawn from the source registry (P11).

**P16 — Classification relies on profile fields derived from Deep Memory**
- **Severity:** minor.
- **Where:** R2, R5, the "What exists today" profiles row.
- **What is wrong:** `domain_expertise` is built from Deep Memory and the context document. Surfacing it as suggestions means private-memory-derived inferences appear in a new product context. "You may know something about NVDA" could surprise users. The consent step mitigates this but does not explain the inference to the user.
- **Why it matters:** Users may lose trust if they feel profiled.
- **How to investigate:** Review the profile-worker prompt outputs for sensitive inferences.
- **Suggested fix:** Show the reason for each suggestion only from rewarded tasks, or let users opt into memory-derived suggestions.

**P17 — Narrative linkage to governance packets is thin and late**
- **Severity:** minor.
- **Where:** R20, the "Bubbles up to" line, Build order Step 3.
- **What is wrong:** The governance tie, where an information packet is acted upon, is the founder's central story. Here it is limited to a steward optionally citing summaries in Pillar 1, and only after Pillar 2 runs on the new chain.
- **Why it matters:** It misses the chance to make the hive mind a visible input to the founder's governance sessions.
- **How to investigate:** Read the Pillar 1 packet schema for an input slot.
- **Suggested fix:** In Step 1, export a database-backed, cluster-summary digest per company that the Pillar 1 packet builder can include automatically.
