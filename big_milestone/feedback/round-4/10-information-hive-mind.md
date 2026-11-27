# Feedback — Pillar 10 — Information Hive Mind

*Execution-hole audit · Round 4.*

> **Mean score:** 70/100 · Gemini 4 Neon 70 · Claude Opus 5.5 71
>
> **Method:** each model received (1) the founder's original prompt, (2) locked founder decisions & live state, (3) master `README.md`, and (4) [this pillar's spec](../../pillars/10-information-hive-mind.md), once, scored across 5 explicit rubric dimensions (Execution Holes /30, Economics /20, Founder Alignment /20, Milestones & 4 Gates /15, Readability /15). Run 2026-11-26 23:35 UTC via OpenRouter.

## Gemini 4 Neon — 70/100

*Model `google/gemini-4-neon` · 9736 output tokens · `tech=15/30 econ=11/20 align=18/20 ship=12/15 read=14/15`*

#### Summary judgement

Pillar 10 is cleanly written, tightly aligned with all locked founder decisions, and reuses Task Node's existing profiles, badges, and `158_finance_agent.sql` tables sensibly. As a product narrative it is clear; as an engineering spec, however, it is not ready to build without closing several real security, state-machine, and economic loopholes.

The biggest blockers are three critical execution holes: (1) gated Finance Hub research still leaks to non-members through Phase 2's open model questions and pooled claims even when the base brief is public; (2) "pre-storage" MNPI screening fails to prevent confidential text from landing in HTTP logs, chat history, LLM provider logs, or multi-turn context; and (3) the claim-to-payout state machine is broken in two places—there is no return loop proving a `CHANGELOG` citation happened, and nobody is assigned to resolve `check_event` outcomes. On top of that, the accelerated alpha reward design currently allows free-option straddle farming, and the Pillar 2/Pillar 6 signing design contradicts optional on-chain identity and pseudonymity.

#### Rubric breakdown

- **Execution Holes & Technical Robustness (15/30):** Good high-level architecture and privacy intent, but contains three critical security/state-machine holes (gated research leakage, MNPI persistence/multi-turn gaps, broken citation/resolution loops) plus major cross-pillar signing and prompt-injection gaps.
- **Economic & Incentive Coherence (11/20):** Correctly targets Sybil corroboration farming and links to badge caps, but omits concrete payout/split bounds and leaves open binary straddle farming and model-owner self-citation collusion.
- **Founder Decision Alignment & Zero Open Questions (18/20):** Faithfully implements all locked Pillar 10 decisions with zero open questions; minor deduction for leaving rate limits and reward parameters unquantified instead of locking concrete engineering defaults.
- **Actionable Milestones & 4 Ship Gates (12/15):** Clear M1–M5 build milestones and all four mandatory ship gates with concrete user tests, but milestones miss the citation return loop, check-event resolution workflow, and multi-turn MNPI red-teaming.
- **Plain-English Readability & Zero Jargon Bloat (14/15):** Concise, readable plain English with a helpful Mermaid diagram and zero claudese bloat.

#### Execution holes

##### H1
- **Severity:** `critical`
- **Where:** Section 1 (*Coverage manifest, classification & privacy-safe matching*) and Section 2 (*Two-phase chat*, *Member vs public context split*)
- **Hole:** Non-Finance-Hub contributors receive `agent_brief_public.json`, but Phase 2 still prompts them with **open model questions** and **existing pooled claims** for corroboration/challenge without segregating those pools by access tier.
- **Why it breaks execution:** Any open question generated from a gated model assumption (e.g., *"Do you see evidence against our Q4 42% margin estimate?"*) or any pooled claim submitted by a Finance Hub member who read the gated brief leaks proprietary Finance Hub targets straight into non-member chat sessions—defeating Pillar 9's access boundary and failing Ship Gate 2.
- **Minimal plain-English fix:** Tag every open question and pooled claim as `public` or `member_only`. Build-verify `public` questions alongside `agent_brief_public.json` to ensure zero gated numbers, and restrict non-member Phase 2 prompts strictly to `public` briefs, `public` questions, and `public`-tier claims.

##### H2
- **Severity:** `critical`
- **Where:** Locked-in design decisions (*Refused MNPI/NDA content*) and Section 2 (*Pre-storage screening*)
- **Hole:** The spec states answers are screened "before storage" and discarded on refusal, but does not enforce where raw text travels before screening, how multi-turn MNPI is caught, or how `redact` previews and refusal hashes avoid holding MNPI.
- **Why it breaks execution:** If raw chat messages hit web/proxy access logs, get written to `finance_agent_messages` before classification, are sent to a third-party LLM with prompt retention, or sit server-side awaiting user `redact` confirmation, Task Node still stores or transmits MNPI. Splitting confidential info across two short messages also bypasses single-answer screening, and an unsalted SHA-256 hash of a short factual claim (e.g., `"Q3 EPS is 2.14"`) is trivially brute-forceable.
- **Minimal plain-English fix:** Disable request-body logging on interview endpoints; run screening on a zero-retention inference endpoint against the active in-memory sliding turn window *before* any DB write or company-agent call; handle `redact` previews client-side before re-submission; and hash refused text with HMAC-SHA256 using a rotated server-side key.

##### H3
- **Severity:** `critical`
- **Where:** Section 3 (*Claims table*, *Research host bridge & Pillar 2 packets*, *Accelerated alpha rewards*)
- **Hole:** Two critical state transitions have no defined executor or data path: (1) fast payouts depend on citation in an applied model update, yet the Research Host bridge is specified as a read-only pull *from* Task Node with no verified return path confirming publication in a model `CHANGELOG`; (2) nothing defines who transitions `accepted` claims to `verified` / `falsified` / `unresolvable` when a `check_event` arrives, or on what timeline.
- **Why it breaks execution:** Engineers cannot trigger fast payouts without risking either paying unapplied proposals or waiting forever for manual confirmation. Worse, without a resolution workflow and SLA, checkable claims pile up in `accepted` indefinitely—deadlocking the "accelerated alpha bonus" and angering contributors.
- **Minimal plain-English fix:** (1) Have Finance Hub's signed build publish a `citations_manifest.json` (mapping `contribution_id` → model version + `CHANGELOG` hash) that Task Node ingests to trigger idempotent fast payouts. (2) Require every `check_event` to specify a public source and bounded horizon (`<= 90 days`); run an automated Reviewer Agent draft check within 48 hours of `check_event_at` and queue one-click confirmation for the model owner (escalating to the operator queue after a 7-day SLA).

##### H4
- **Severity:** `major`
- **Where:** Locked-in design decisions (*Reward design*) and Section 3 (*Accelerated alpha rewards & ROI dashboard*)
- **Hole:** The reward mechanism has no concrete payout/multiplier bounds, duplicate-splitting rule, anti-hedging rule, or owner conflict-of-interest check—while falsified claims carry zero financial downside (only domain-weight decay).
- **Why it breaks execution:** Two colluding Expert accounts (or one user hedging across names) can submit opposite `challenges` on upcoming binary `check_event`s: the losing account pays nothing, while the winning account captures an accelerated alpha bonus (a free call option). Likewise, a model owner can cite their own or a friendly contributor's account to drain PFT rewards unchecked.
- **Minimal plain-English fix:** Lock concrete v1 defaults: cap total per-claim reward (fast citation payout + `1.5x–2.5x` alpha bonus) inside existing badge caps and a per-company monthly PFT budget; split bonuses across corroborators with a first-mover weight instead of paying full bonus per account; require pre-event acceptance with falsifiable mechanism/evidence (blocking bare directional guesses) plus automatic interview cooldowns after consecutive falsifications; and prohibit model owners from citing linked accounts (with operator audit on high-spend owners).

##### H5
- **Severity:** `major`
- **Where:** Locked-in design decisions (*Privacy & pseudonymity*) and Section 3 (*Research host bridge & Pillar 2 packets*)
- **Hole:** Requiring accepted claims to emit "encrypted Pillar 2 packets signed by the contributor's Pillar 6 identity or pseudonymous key" breaks on three fronts: (1) Pillar 6 identity is optional for non-validators, (2) contributors are offline when Reviewer/Owner acceptance happens hours later so a client-side key cannot sign asynchronously, and (3) neither the encryption recipient key nor the on-chain linkability impact on pseudonymity is defined.
- **Why it breaks execution:** Either the server must custody user signing keys (breaking non-custodial signing), or accepted claims block waiting for users to come back online to sign. Furthermore, if a "pseudonymous" contributor signs with their wallet/Pillar 6 key or receives a public FastPay payout tied to the claim CID, anyone inspecting the block explorer can deanonymize them.
- **Minimal plain-English fix:** Sign the structured claim payload client-side *at submission time* using the user's Task Node wallet key (or optional Pillar 6 key / client-generated pseudonymous subkey), encrypt the Pillar 2 packet to the Research Host's published X-Wing public key, and pin it to IPFS without attaching contributor identity on-chain. Explicitly document in the UI that v1 pseudonymity hides handles on Finance Hub while on-chain FastPay payouts remain pseudonymous by address (not shielded until Pillar 5 payout routing is added).

##### H6
- **Severity:** `major`
- **Where:** Section 2 (*Two-phase chat*) and Section 3 (*Reviewer agent & human approval*, *Research host bridge*)
- **Hole:** Untrusted contributor text from the claim store is fed into three downstream surfaces without sanitization or prompt-injection boundaries: the Reviewer Agent (scoring novelty/challenges), Phase 2 prompts of *other* interviewees (corroboration), and Finance Hub (`contributor_insights.json`).
- **Why it breaks execution:** An adversarial Expert contributor can embed prompt-injection instructions in an accepted claim to manipulate the Reviewer Agent into awarding high novelty/challenge scores, hijack interviews of other users, or inject malicious HTML/script payloads into Finance Hub's Contributor Insights panel.
- **Minimal plain-English fix:** Store claims as length-capped, plain-text structured fields and render them strictly as escaped plain text on Finance Hub (never raw HTML/Markdown); wrap claim text in explicit data-only delimiters with schema-validated JSON output in the Reviewer Agent; and only pass Reviewer-normalized summaries (never raw user prose) into other contributors' Phase 2 prompts.

##### H7
- **Severity:** `minor`
- **Where:** Section 1 (*Evidence-weighted matching*) and Section 2 (*Two-phase chat*)
- **Hole:** Self-initiated interviews (including Finance Hub's *Ask the agent* button) reference "concurrent-interview caps" and "turn and token budgets" without locking concrete numeric limits or per-company cooldowns.
- **Why it breaks execution:** A handful of Expert users or automated scripts can repeatedly open self-initiated interviews across 60 companies—running up unbounded LLM inference bills and flooding the Reviewer Agent and human model-owner approval queue with low-value proposals.
- **Minimal plain-English fix:** Lock concrete v1 defaults in the spec: max 2 concurrent open interviews per user, max 5 interviews per week per user, 14-day cooldown per user-company pair (unless invited on a newly posted open question), max 12 turns / 25k tokens per interview, and a per-company cap on queued unreviewed proposals.

## Claude Opus 5.5 — 71/100

*Model `anthropic/claude-opus-5.5` · 7197 output tokens · `tech=17/30 econ=11/20 align=18/20 ship=13/15 read=12/15`*

#### Summary judgement

This spec is well aligned with the locked decisions:

- matching is open to any Expert or verified user;
- scope is crypto plus perp-backed megacap equities;
- rewards are accelerated for alpha;
- non-members get a public-only brief.

It has a clear loop diagram, five concrete milestones and all four ship gates. It also correctly reuses existing assets: the Expert badge, badge payout caps, Sybil review, and migration `158`. The writing is mostly plain English, with some over-compressed bullets.

The holes cluster in two places: the claim and reward state machine, and the data-handling boundaries.

- **Corroboration is not blind.** Showing pooled claims to other contributors undermines independence and creates a copy-to-earn loop.
- **Verification can be gamed.** Alpha verification has no claim-time snapshot or event-deadline rule, so hedging, stale, or after-the-fact claims can collect the multiplier.
- **Refused text can still be retained.** "Never stored" is not true unless screening runs before the message log, the LLM context, and provider retention.
- **The Pillar 2 packet step is unbuildable as written.** It asks the contributor to sign at acceptance time, when they are offline, and pins claim records permanently.
- **Reward numbers are missing.** No concrete payout figures are given, which conflicts with the "concrete defaults" standard.

#### Rubric breakdown

- **Tech (17/30):** Sound overall architecture. However:
  - the MNPI "never stored" guarantee leaks through the persistence and inference path;
  - Phase 2 can leak gated content to non-members;
  - claims have no timeout states;
  - packet signing and pinning conflict with privacy and the async flow.
- **Econ (11/20):** The direction is right: uncheckable claims pay only on citation, and falsified claims lose weight. But there are three problems:
  - no numbers are set for the base payout, multiplier, or budget;
  - the multiplier and the badge cap interact undefined;
  - Sybils can hedge by submitting both sides of a claim, and visible corroboration can be farmed.
- **Align (18/20):** Matches every locked Pillar 10 decision and has no open-questions section. The undefined multiplier and payout amounts are deferred parameters the standard says should be concrete.
- **Ship (13/15):** M1–M5 are concrete and all four gates are present, with a strong user test. Gate 2 does not test hedging/Sybil resistance or claim expiry.
- **Read (12/15):** Mostly plain English. A few dense, parenthetical, run-on bullets (sections 2–3) could be split.

#### Execution holes

**H1**
- **Severity:** critical
- **Where:** §2 Phase 2 ("anonymously asks for independent corroboration/challenge of existing pooled claims"); §3 reviewer agent
- **Hole:** Phase 2 shows existing pooled claims to later contributors. Their answers are then counted as "independent" corroboration.
- **Why it breaks execution:**
  - Contributor B sees A's claim, replies "confirms," and earns, so the claim cluster becomes an echo chamber.
  - Pooled claims may contain Finance Hub member-derived numbers, which then reach non-member interviewees and bypass the public-brief firewall.
  - A Sybil ring seeds a claim from one account and "corroborates" it from others.
- **Minimal plain-English fix:**
  - Ask corroboration questions blind: describe the topic, never the claim text or numbers.
  - Count a corroboration only if it was elicited blind and comes from an account not linked by Sybil review.
  - A bare `confirms` of a shown claim earns nothing.

**H2**
- **Severity:** critical
- **Where:** §3 accelerated alpha rewards; Locked design "Reward design"
- **Hole:** Nothing records what "consensus" or the model estimate was when the claim was made. The contributor can also choose a `check_event` that has effectively already happened.
- **Why it breaks execution:**
  - A user files a "challenge" minutes after an earnings release or on-chain event and collects the verification multiplier.
  - A Sybil pair files opposite challenges on the same number. One always verifies. The falsification penalty lands on a throwaway account.
  - "Non-consensus" cannot be computed deterministically, so bonuses become discretionary.
- **Minimal plain-English fix:**
  - At submission, snapshot the model's current estimate and the claim timestamp.
  - `check_event` must be a future, dated public event confirmed by the reviewer at acceptance.
  - Only claims already accepted by the reviewer are bonus-eligible.
  - "Non-consensus" means the claim's direction differs from the snapshot.
  - If linked accounts file opposite claims on the same cluster, all of them lose bonus eligibility.

**H3**
- **Severity:** major
- **Where:** §2 pre-storage screening; Locked "Refused MNPI/NDA content"
- **Hole:** "Never stored" is not true in practice.
  - The answer reaches the chat agent (and `finance_agent_messages`) as a normal turn before or alongside screening.
  - It stays in the LLM context for later turns.
  - It may be retained by the inference provider.
  - The `redact` path does not say the removed spans are discarded.
  - A plain SHA-256 of short text (for example, a single revenue figure) can be guessed by brute force, so the "audit hash" effectively preserves the content.
- **Why it breaks execution:** One logged turn, provider log, or context replay means the platform holds MNPI, which is exactly what counsel sign-off depends on not happening.
- **Minimal plain-English fix:**
  - Screen synchronously before anything is written or passed to the interview agent.
  - Purge refused and redacted spans from the agent context.
  - Use only zero-retention inference for the screener and agent.
  - Log a keyed hash (HMAC with a rotating secret key) instead of plain SHA-256.
  - Add "refused text absent from DB, context, and logs" to the M2 red-team suite.

**H4**
- **Severity:** major
- **Where:** §1 coverage manifest ("open model questions"); §2 member vs public split
- **Hole:** Only `agent_brief_public.json` is build-checked for gated content. Phase 2 questions are generated from "live blind spots" and open model questions, which come from the gated model.
- **Why it breaks execution:** A non-member is asked something like "is $X ARPU too high given …?" This leaks targets and uncertainty through the question itself. Prompt injection can then ask the agent to recite its open-question list.
- **Minimal plain-English fix:**
  - Split open questions into public and member lists in the manifest.
  - Run the same build-time no-numbers check on the public list.
  - The non-member agent only ever loads the public list.

**H5**
- **Severity:** major
- **Where:** §3 claims status list; Locked "Model change authority"
- **Hole:** No timeouts are defined.
  - Claims can sit in `pending` forever if the model owner never acts.
  - Checkable claims have no rule if the event never occurs or slips.
  - "Fast payout" depends on an approval queue with no deadline, across about 60 models.
- **Why it breaks execution:** The queue backs up, contributors wait indefinitely, "accelerated" rewards are never actually fast, and the ROI dashboard shows stalls with no automatic resolution.
- **Minimal plain-English fix:**
  - Set a review deadline, for example 7 days. After that, the claim escalates to the Finance Hub operator queue.
  - After 30 days unreviewed, the claim becomes `unresolvable`.
  - A checkable claim whose event deadline passes plus 14 days becomes `unresolvable`, with no penalty.
  - Show queue age on the dashboard.

**H6**
- **Severity:** major
- **Where:** §3 "Accepted claims emit encrypted Pillar 2 packets signed by the contributor's Pillar 6 identity"
- **Hole:** Acceptance happens later on the server, when the contributor is offline, so they cannot sign then. Doing so would require Task Node to custody their keys.
  - Signing with a Pillar 6 identity publicly links pseudonymous contributors to their claims.
  - Task Node pins all its traffic permanently, so a claim later found to be MNPI stays on IPFS forever.
- **Why it breaks execution:**
  - Engineers must either break key custody or ship unsigned packets.
  - The pseudonymity promise ("hidden from public readers") fails through on-chain linkage.
  - Problem content becomes undeletable.
- **Minimal plain-English fix:**
  - The contributor's client (in the native Mac app) signs the screened claim at submission.
  - On acceptance, Task Node adds its own signature and publishes.
  - Pseudonym mode uses a per-company derived key, not the Pillar 6 identity.
  - The body is encrypted only to Task Node and the contributor.
  - Pin only after acceptance, and allow unpinning when a claim is rejected for compliance.

**H7**
- **Severity:** major
- **Where:** Locked "Reward design"; M5
- **Hole:** The base payout, the multiplier value, per-claim and per-month caps, and the total budget are unspecified. How the multiplier interacts with the existing badge `max_payout_pft` cap is undefined: either the cap silently nullifies the multiplier, or the multiplier silently bypasses the cap.
- **Why it breaks execution:** Engineers cannot implement M5 without inventing economics, and the Sybil and budget exposure cannot be reasoned about.
- **Minimal plain-English fix:** State concrete defaults:
  - a citation payout (a fixed PFT amount);
  - a multiplier, for example 3×, or 5× for verified `challenges`;
  - the multiplier applies inside a separate alpha cap per contributor per month;
  - a monthly program budget, paid via Pillar 2's default FastPay "Task reward" path.

**H8**
- **Severity:** minor
- **Where:** Locked "Model change authority"; §3
- **Hole:** Nothing stops a model owner or operator from submitting claims under their own account or a pseudonym and approving them for payout.
- **Why it breaks execution:** Insiders self-deal the alpha budget, and the citation-only path for uncheckable claims makes this easy.
- **Minimal plain-English fix:** An owner cannot approve claims from their own or linked accounts; those claims route to the operator queue. Owner and operator claims are ineligible for the multiplier.

**H9**
- **Severity:** minor
- **Where:** §1 "self-initiate an interview (… Ask the agent)"; member vs public split
- **Hole:**
  - Self-initiated interviews only have per-interview turn and token caps, with no per-day limit, which allows inference-cost griefing.
  - Member status is not re-checked when a thread resumes, so a user who drops out of Top-30 can resume a thread with member context.
- **Why it breaks execution:**
  - Cheap Expert accounts burn inference budget.
  - Gated briefs persist in a thread after access is revoked.
- **Minimal plain-English fix:**
  - Cap self-initiated interviews per user per day.
  - Re-check Finance Hub membership on every thread resume. If it has lapsed, close the thread and restart it with the public brief.

**H10**
- **Severity:** minor
- **Where:** Locked "Relationship exclusions"
- **Hole:** The employee/insider exclusion rests only on self-declaration, although Task Node privately holds signals such as capability profiles, context docs, and task evidence that could reveal an employer.
- **Why it breaks execution:** An insider lies on the attestation, and the platform ignores data it already holds. That weakens the compliance posture counsel will review.
- **Minimal plain-English fix:** Use the private profile server-side to automatically flag likely employer matches and block them, without sending that data to agents. This keeps the privacy boundary while adding a second check.
