# Feedback — Pillar 9 — Finance Hub User Spec

*Review round 3.*

> **Mean score:** 77/100 · Gemini 4 Neon 74 · Claude Opus 5.5 80
>
> **Method:** each model received (1) the founder's original prompt and (2) [this pillar's spec](../../pillars/09-finance-hub.md), once, with the question below. Run 2026-11-26 22:31 UTC via OpenRouter.

## The question

> On a scale from 0-100, to what extent does this sub-page correspond to the founder's intention, and how likely is it to output something that works for the founder and the project — with 'works' meaning technically robust and economically coherent?
>
> Output your reasoning at length, enumerating problems clearly so that subsequent agents can investigate and improve.

## Gemini 4 Neon — 74/100

*Model `google/gemini-4-neon` · 19950 output tokens*

#### Summary judgement

This is a well-grounded, unusually codebase-aware specification that accurately reflects the founder's core Finance Hub user story and catches several serious live vulnerabilities in the current implementation (unsandboxed Goodbrief scripts running on Task Node's origin, whole-bundle decryption keys undermining per-file revocation, and unsigned `scp` writes directly to the production database). Its phased delivery structure and operational realism (daily top-30 snapshots, decoupling Goodbrief publishing from model builds, schema-versioned section degradation, visit baselines) are strong foundations.

However, the spec is not yet ready to execute without revision. It carries **two critical gaps**—a flawed Goodbrief/viewer sandboxing design that will either break subresources or leave data-exfiltration and same-origin XSS paths open, and an existential third-party licensing risk (Bloomberg-derived Goodbrief redistribution) with no fallback plan—alongside **several major architectural and economic defects**: "GoodAlexander Content" (explicitly half of the founder's validator premise) has no design, build step, or acceptance criteria; the delivery sequence contains a dependency inversion that risks breaking production; a single Top-30 snapshot is overloaded across five pillars with conflicting incentive needs; the signed publishing pipeline has underspecified security and lifecycle details; and the anti-leak controls target bulk scraping while missing the much easier daily copy-paste leak vector.

---

#### Correspondence to the founder's intention

**What aligns strongly:**
- **Dual access paths:** Captures both qualifying validators and top-30 Task Node users, including clear locked-state onboarding and expiry visibility.
- **Daily research portal ("Today"):** Translates the founder's request for model updates, earnings releases, coverage changes, and the Goodbrief macro roundup into a sensible single-page daily workflow with search, watchlists, *since your last visit* markers, and mobile responsiveness.
- **Strategic linkage to new-chain migration:** Correctly identifies that today's acceptance of any old-chain validator binding undermines the founder's goal of pulling validators onto `postfiatl1v2`, and delegates validator status to Pillar 3.
- **Pillar 10 handoff:** Defines concrete hooks (`coverage_manifest.json`, `agent_brief_*.json`, `contributor_insights.json`, and `Ask the agent`) connecting Finance Hub to the Information Hive Mind.

**Where it falls short of the founder's intention:**
1. **GoodAlexander Content is largely sidelined:** The founder states that *"the basic premise of the Post Fiat Validator program is that Validators should be able to access Finance Hub as well as GoodAlexander Content."* The sub-page acknowledges this in the intro and R26 ("a Library"), then pushes all substance into Open Question 8, leaves it out of the four-step delivery plan, and omits it entirely from Acceptance Criteria 1–13.
2. **Missing user-to-coverage linkage from Deliverable 10:** The founder's Deliverable 10 explicitly states *"The coverage universe in finance hub is linked to specific users."* R19 adds anonymous/generic `contributor_insights.json` and an `Ask the agent` button, but Finance Hub does not surface which contributors are linked to or classified for specific covered companies/protocols (nor how attribution, anonymity, or contributor reputation appears on company pages).
3. **Daily-habit UX gaps:** To make Finance Hub *"an intuitively designed place where I want to log in every day and see what is going on in markets,"* the spec needs more than a static section list. It omits a **Goodbrief archive** (if a user misses two days, can they read prior briefs?), leaves **live movers** mechanically unspecified, and does not define cross-asset sorting/ranking or read-only vs. export rules for company models.
4. **Local Task Node transition friction:** Making Finance Hub web-only in Pillar 5 (R25) is a sensible content-protection trade-off despite Deliverable 5's *"virtually everything"* phrasing, but the spec does not define how a user authenticated to the local app transitions cleanly into a hosted browser session without hitting a jarring auth wall.

---

#### Technical robustness

**Strengths:**
- Thorough audit of live Task Node code (`FinanceHubView.jsx`, `validator-bundles.js`, `index-viewer-routes.js`, `publish-validator-bundle.mjs`, `directory-leaderboard.js`, `alignment-score.js`) and private research pipelines.
- Correctly prioritizes closing live security holes (Step 1) before expanding distribution to top-30 users (Step 4).
- Smart operational ideas: decoupling `goodbrief` from `ga-models`, retaining the last good snapshot/export on job failure with alerting, breaking ties deterministically on `account_id`, and using a `visit_baseline_at` + idle-gap model rather than naive last-load timestamps.

**Key technical weaknesses:**
- **Broken/incomplete sandboxing model (R8):** Serving Goodbrief with CSP `sandbox allow-scripts` puts it in an opaque (`null`) origin. Any relative CSS/image/JS subresources behind Task Node's gated per-file route will lose session cookies and fail unless Goodbrief is guaranteed to be a single self-contained HTML document. Simultaneously, `sandbox allow-scripts` *without* `connect-src 'none'`, `default-src 'none'`, `img-src data:`, and `form-action 'none'` still lets a malicious or prompt-injected script inside the LLM-generated HTML exfiltrate the entire Goodbrief to an external server or run phishing UI. Meanwhile, the main `ga-models` viewer remains unsandboxed on Task Node's origin, meaning any bug in `app.js` or compromise of the research host still inherits full user session privileges on Task Node.
- **Sequencing inversion in Delivery Order:** Step 1 executes R7 (disabling `unlock` and returning 404 on `…/content`), while Step 3 executes R23 (confirming per-file loading is actually live in production). Today's status table explicitly notes *"Production use is unconfirmed."* Running R7 before R23 risks a production outage. Similarly, Step 1 implements R3 (which reads Pillar 3's *lapsed* grace state), while R5 (the grace mechanism) is deferred to Step 4.
- **Publishing pipeline (R20–R21) underspecification:** Staging file uploads have no specified authentication (DoS vector); signed commits and rollbacks lack replay protection (e.g., `issuedAt` expiry window and single-use rollback nonces) and signature algorithm choice; 96 publications/day (every 15 minutes) with full file re-upload and per-file DB encryption will cause rapid database bloat without content deduplication and garbage collection; atomic version swaps create mid-session version skew for active viewers; and R21 omits where the structured Goodbrief summary JSON comes from once Goodbrief is decoupled from `build_models_static.py`.
- **Caching vs. immediate revocation (R3 vs. R9):** R3 caches validator eligibility so file requests never wait on the L1 registry, while R9/AC3–AC4 require the *next* file request to fail upon revocation, suspension, or blacklisting. Cache boundaries, TTLs, and explicit invalidation hooks are unspecified.
- **Visit heartbeat bug (R16):** Sending a heartbeat "while open" will continually refresh `last_active_at` for traders who leave Finance Hub pinned in a background tab overnight, preventing `visit_baseline_at` from ever advancing.
- **Tainted public-brief generation (R19):** Relying on a build check to detect leaked model numbers/targets in `agent_brief_public.json` is fragile (prose paraphrases evade regexes; public historical financials trigger false positives). Public briefs should be generated from a strictly public-only input schema.
- **Unspecified live movers & per-file caching overhead:** "Live movers" (Hyperliquid + Market Lens fallback) is never wired up architecturally (client-side fetches clash with strict CSP and expose third-party API keys; build-time fetches aren't live). Meanwhile, serving every static viewer asset with server-side decryption and `private, no-store` wastes CPU/DB I/O and degrades mobile/desktop latency without clearly justifying what threat per-file DB encryption defends against once `unlock` is removed.

---

#### Economic coherence

**Strengths:**
- Recognizes that Finance Hub is a scarce, high-value retention/acquisition reward whose value collapses if it leaks or stalls.
- Correctly flags key incentive traps: old-chain validator deadweight, incumbent entrenchment under all-time leaderboards, outsized weight of the LLM-judged alignment score (up to 100 points = 33 network tasks), and seat cannibalization if validators already dominate the top 30.

**Key economic weaknesses:**
1. **No fallback for the Bloomberg redistribution bottleneck (Open Question 5):** Standard Bloomberg licensing strictly prohibits external redistribution—including LLM syntheses—to non-employees, and scrubbing the vendor name increases rather than mitigates legal exposure. The entire Today UX and Steps 2–3 assume Goodbrief ships to validators and top-30 users, yet there is no contingency design (e.g., licensed redistributable feeds, public filings, or original macro commentary) if legal review says no.
2. **Overloading one Top-30 snapshot across five pillars (R4):** Sharing a single Top-30 table across Pillars 1 (governance), 4 (NFTs), 6 (Hive identity), 8 (on-chain identity), and 9 (Finance Hub) forces incompatible trade-offs. Finance Hub wants anti-farming, active-contribution incentives (e.g., rolling window, potentially excluding validators to add 30 incremental seats, plus a hysteresis/grace band). Excluding validators or using a short rolling window would distort governance or reputation credentials in the other pillars.
3. **Blind spot on the real leak economics (R10):** Monitoring "unusual counts of distinct companies read per day" deters bulk scraping of the 60-model archive, but the highest-value daily content is the 1-page Goodbrief and 3–5 daily updates on Today. A single member can copy-paste or screenshot Today into a paid/free Telegram or Discord group every morning in 10 seconds without ever reading more than 1–2 pages, or share credentials across a small group. Without mandatory lightweight watermarking/fingerprinting (currently punted to Open Question 6) and concurrent-session/device controls, exclusivity will erode quickly once 30+ non-validator accounts gain access.
4. **Too many foundational economic choices left unrecommended:** Nine open questions defer critical policy decisions (validator migration sunset, top-30 scoring formula, seat count scaling, grace rules, GoodAlexander content parity) without providing a recommended baseline or cost-of-entry / Sybil-resistance analysis. Given the perceived value of institutional research ($200–$2,000+/month), a weak top-30 scoring rule will be immediately farmed.

---

#### Problems

##### P1 — Goodbrief `sandbox allow-scripts` breaks gated subresources, leaves exfiltration open, and leaves the main viewer unsandboxed on Task Node's origin
- **Severity:** critical
- **Where:** `Architecture`, `Technical requirements` (R8), `Acceptance criteria` (AC5)
- **What is wrong:**
  1. Serving the full HTML Goodbrief with CSP `sandbox allow-scripts` (without `allow-same-origin`) gives it an opaque `null` origin. Any subresource referenced by that HTML (CSS, JS, fonts, images) served from Task Node's cookie-gated per-file route will fail authentication because browsers do not send credentials from an opaque origin.
  2. `sandbox allow-scripts` alone does *not* prevent a script (or prompt-injected markup from upstream news articles) inside the Goodbrief from exfiltrating content via `fetch`/`<img>` or navigating/phishing, because `connect-src`, `img-src`, `default-src`, and `form-action` are not locked down.
  3. Requiring Goodbrief to report its height via `postMessage` from an opaque origin forces the viewer to accept `event.origin === "null"` and assumes the Goodbrief pipeline injects a height-reporter script.
  4. Keeping the main `ga-models` viewer same-origin and unsandboxed leaves Task Node's authenticated user session exposed to any XSS in `app.js` or compromised research-host output.
- **Why it matters:** Either Goodbrief rendering breaks in production (missing CSS/images), or an XSS/prompt-injection in LLM-synthesized HTML exfiltrates proprietary research or hijacks Task Node user sessions (including wallet/task actions).
- **How to investigate:** Inspect the actual HTML output of `navstrategies/data_sources/bloomberg/news_summary_pipeline.py` to see what scripts, stylesheets, charts, and external URLs Goodbrief actually uses today. Check `FinanceHubView.jsx` and `validator-bundles.js` to see what cookies/auth tokens are accessible on Task Node's origin and whether a separate sandboxed subdomain (e.g., `hub-content.<domain>`) is feasible.
- **Suggested direction for fix:**
  - Preferably **strip all scripts from Goodbrief** via strict server/build-time HTML sanitization (DOMPurify/allowlist) and serve with `Content-Security-Policy: sandbox; default-src 'none'; style-src 'unsafe-inline'; img-src data:; form-action 'none'; frame-ancestors 'self'`. If Goodbrief genuinely needs interactive JS (e.g., charts), require it to be a single self-contained HTML bundle (zero external or gated subresources) with strict `connect-src 'none'; default-src 'none'; script-src 'unsafe-inline' <or hash>; img-src data:` and an explicit, verified `event.source === iframe.contentWindow` height handshake.
  - Evaluate isolating the `ga-models` viewer itself onto a dedicated origin (or `sandbox="allow-scripts"` with capability-scoped short-lived bearer tokens passed via `postMessage` rather than ambient Task Node session cookies) so a research-host compromise cannot act as the user on Task Node.

##### P2 — Bloomberg/third-party content redistribution risk lacks a fallback plan
- **Severity:** critical
- **Where:** `What exists today` (Goodbrief), `Technical requirements` (R11), `Economics`, `Open questions for the founder` (Q5)
- **What is wrong:** Goodbrief is an LLM synthesis of Bloomberg articles (plus options data and Market Lens fallback), currently scrubbing the vendor's name and already exposed to outside old-chain validators. The spec correctly flags that scrubbing is not a rights measure, but treats the issue as an open question whose answer simply toggles metadata in R1—without providing a fallback content pipeline if external redistribution is prohibited.
- **Why it matters:** Bloomberg Terminal and Data License contracts aggressively restrict external redistribution (including derived/summarized text). If legal review blocks external distribution of Bloomberg-derived Goodbriefs, the centerpiece daily hook of Finance Hub disappears for both validators and top-30 users.
- **How to investigate:** Audit all upstream data sources in `news_summary_pipeline.py`, options-implied move feeds, and Market Lens fallback to identify the exact license/ToS for each source and determine whether any source permits external member redistribution.
- **Suggested direction for fix:** Gate Step 4 (and ideally external Goodbrief exposure in Step 1) on an explicit rights sign-off, and add a concrete **Plan B** to the spec: a redistributable macro roundup pipeline built on licensed redistributable feeds, public filings/central-bank/earnings transcripts, and original GoodAlexander commentary, so Finance Hub's daily utility does not collapse if Bloomberg must be restricted to internal-only access.

##### P3 — "GoodAlexander Content" (half of the founder's validator premise) has no architecture, build step, or acceptance criteria
- **Severity:** major
- **Where:** `User story`, `Technical requirements` (R26), `Delivery order`, `Acceptance criteria`, `Open questions for the founder` (Q8)
- **What is wrong:** The founder explicitly frames the validator program around access to *"Finance Hub as well as GoodAlexander Content."* R26 sketches a "Library" in four sentences, defers everything to Open Question 8, omits it from Steps 1–4 of the delivery order, and includes **zero** acceptance criteria covering R26 (neither the Library nor Research API key access/revocation).
- **Why it matters:** Shipping Pillar 9 as written delivers Finance Hub while leaving the other half of the validator value proposition unbuilt and untested.
- **How to investigate:** Inventory existing GoodAlexander content channels (essays, videos/films, research reports, Deep Research chat, Research API keys in `docs/wiki/surfaces/validator-research.md`) and determine how they are currently authored, hosted, and gated.
- **Suggested direction for fix:** Define a concrete minimum viable Library spec (metadata schema, upload/publish mechanism reusing R20's signed publish API or an operator admin flow, UI placement inside Finance Hub, Research API key entitlement check on every request, and signed-URL TTLs for large media), place it explicitly in the delivery order, and add dedicated acceptance criteria for Library access, per-basis gating, and Research API key revocation.

##### P4 — Delivery order has dependency inversions (R7 before R23; R3 before R5)
- **Severity:** major
- **Where:** `Technical requirements` (R3, R5, R7, R23), `Delivery order`
- **What is wrong:**
  1. Step 1 executes **R7** (disabling `unlock` and returning 404 on `…/content`), yet **R23** (confirming per-file loading is actually live in production—currently *"unconfirmed"*) is placed in Step 3.
  2. Step 1 implements **R3** (which reads Pillar 3's *lapsed* grace state), while **R5** (the unified grace mechanism) is deferred to Step 4.
- **Why it matters:** Disabling whole-bundle paths in Step 1 before verifying per-file loading in production can break Finance Hub for all current validators. Implementing validator grace logic in Step 1 without R5 creates throwaway code or broken UI states for lapsed validators.
- **How to investigate:** Inspect production `ga-models` DB metadata (`metadata.fileFormat`) and `FinanceHubView.jsx` / `validator-bundles.js` network traces in production to verify whether PR #384's per-file loading is actually active.
- **Suggested direction for fix:** Move **R23** to a mandatory pre-flight gate at the start of Step 1 (before R7 executes), and split **R5** so the shared grace data model and UI banner ship in Step 1 alongside R3 (for validators), with top-30 grace parameters simply plugged into the same mechanism in Step 4.

##### P5 — Sharing one Top-30 snapshot across five pillars creates conflicting economic incentives
- **Severity:** major
- **Where:** `Technical requirements` (R4), `Economics`, `Open questions for the founder` (Q2, Q3)
- **What is wrong:** R4 states that *"Pillars 1, 4, 6 and 8, which consider top-30 status as a gate or credential, read this snapshot."* Meanwhile, Open Question 2 considers excluding validators from the Finance Hub top-30 or creating a dedicated rolling-window/capped-alignment score specifically to prevent research farming.
- **Why it matters:** If Finance Hub excludes validators from its top-30 to maximize incremental seats, Pillars 1, 4, 6, and 8 reading that same snapshot would falsely treat top-contributing validators as non-top-30. Conversely, if Finance Hub is forced to use an all-time leaderboard to satisfy identity/governance credentials in other pillars, incumbents entrench and Finance Hub loses its incentive pull for active Task Node contributors.
- **How to investigate:** Check Pillars 1, 4, 6, and 8 specs to list the exact semantics each pillar expects from "top-30 status" (all-time reputation vs. active 30-day work, validators included vs. excluded, public vs. private accounts).
- **Suggested direction for fix:** Separate the **canonical contributor ranking snapshot** (all accounts, deterministic score, clear time window, shared across pillars) from **Finance Hub's seat allocation rule** (e.g., top N *non-validator* accounts derived from that snapshot, or a rolling-window Finance Hub qualification query). Also provide a recommended default anti-gaming scoring formula (e.g., 30- or 90-day rolling rewarded network tasks + capped alignment component + minimum on-chain task floor) rather than leaving the founder with an unguided menu.

##### P6 — Signed Publish API (R20) and Goodbrief split (R21) omit critical security, lifecycle, and schema details
- **Severity:** major
- **Where:** `Technical requirements` (R12, R20, R21), `Acceptance criteria` (AC9, AC10)
- **What is wrong:**
  1. **Unauthenticated staging uploads:** R20 specifies a signed commit, but does not specify authentication or quotas for uploading files "one by one into staging."
  2. **Replay and algorithm gaps:** No signature algorithm is specified (e.g., Ed25519 vs. ML-DSA-65); no `issuedAt` freshness window is specified; and signed rollbacks naming an older version lack replay protection (monotonic rollback nonce/sequence ID).
  3. **Database bloat and upload churn:** Publishing every 15 minutes (96 times/day) by uploading and re-encrypting every file for ~60 companies without content-addressed deduplication (skipping unchanged SHA-256 blobs) or DB garbage collection of old versions and abandoned staging runs will bloat Postgres rapidly.
  4. **Mid-session version skew:** Atomic swaps on a single unversioned path mean a user who loaded `index.html`/`app.js` from version N may fetch a lazy JSON/chunk from version N+1 if a 15-minute publish lands mid-session.
  5. **Missing Goodbrief summary producer after R21:** Today, `build_models_static.py` extracts/renders the Goodbrief summary into `ga-models`. Once R21 decouples Goodbrief to push directly from `pfrpc`, the spec checks only `size floor, HTML doctype, brief_date` and never specifies what produces the structured summary JSON (`schema` version, headline, bullets, status) that Today (R12) consumes from `goodbrief`.
- **Why it matters:** These omissions cause storage outages, brittle 15-minute cron failures, subtle frontend crashes during swaps, and broken Today summaries when R21 lands.
- **How to investigate:** Check `publish_to_tasknode.py`, `publish-validator-bundle.mjs`, `159_validator_bundle_files.sql`, and `hub.py` to see current average bundle size, file count, chunk-change frequency per 15-minute cycle, and where the Goodbrief summary text is currently extracted.
- **Suggested direction for fix:**
  - Require authenticated staging sessions (signed manifest pre-declaration returning upload intents only for SHA-256s not already stored—enabling content-addressed dedup).
  - Specify signature scheme (Ed25519 or ML-DSA-65), ±5-minute `issuedAt` window, strictly increasing `commitSeq` (even for rollbacks, which reference a prior `targetVersion`), and explicit DB retention/GC (e.g., keep last 5 committed versions, expire uncommitted staging uploads after 1 hour).
  - Version asset/data fetches to the publication `version` loaded at page init (with a non-disruptive "New update available — refresh" banner).
  - Explicitly define the `goodbrief` publication manifest contract (`summary.json` + `brief.html`) and specify which pipeline (`news_summary_pipeline.py` on the market-data box) produces `summary.json` before R21 ships.

##### P7 — Anti-leak controls miss the highest-value daily leak vectors
- **Severity:** major
- **Where:** `Technical requirements` (R9, R10), `Economics`, `Open questions for the founder` (Q6)
- **What is wrong:** R10 relies on rate limiting and alerting on *"unusual counts of distinct companies read per day."* Meanwhile, per-user watermarking is deferred to Open Question 6, and credential/session sharing is unaddressed (R10 only ensures *"normal use across a phone and a laptop passes"*).
- **Why it matters:** The highest-frequency, easiest-to-monetize leak is **not** scraping all 60 DCF models—it is copying Today's Goodbrief and the 3–5 daily model updates into a Discord/Telegram group (1–5 requests/day, completely invisible to R10's anomaly detector) or sharing one top-30 login across a syndicate. Once 30+ non-validator accounts have access, exclusivity—which the Economics section rightly calls the basis of the reward—will break down without attribution/deterrence.
- **How to investigate:** Review existing session management in Task Node (`app-state.js`, auth middleware) to see how concurrent sessions/IPs/devices are tracked, and check how `hub.py` / `app.js` render text and HTML so lightweight watermarking cost can be estimated.
- **Suggested direction for fix:**
  - Make **lightweight per-account watermarking/fingerprinting a default requirement** (not an open question) for Step 4: visible subtle footer/overlay watermark on the Goodbrief and model pages plus deterministic invisible zero-width/typographic fingerprinting on served text per account, so leaked screenshots or copy-pastes can be traced to the leaking account.
  - Add concurrent-session / distinct-IP velocity limits (e.g., cap active Finance Hub sessions to 3 devices, alert on geographically impossible concurrent access) alongside R10's distinct-company counter.
  - Clarify whether model spreadsheets/CSVs are view-only or downloadable.

##### P8 — "Live movers" data flow is unspecified and conflicts with hardened CSP and rights controls
- **Severity:** major
- **Where:** `User story` (#2), `What exists today`, `Technical requirements` (R8, R12, R13)
- **What is wrong:** The spec calls for *"Movers, refreshed live"* backed by *"public Hyperliquid prices, with a Market Lens fallback,"* while simultaneously tightening the viewer's CSP (R8) and flagging Market Lens as a potential content-rights/licensing concern (Open Question 5). It never states whether live movers are fetched directly from the browser (requiring CSP `connect-src` allowances and risking API key exposure for Market Lens), proxied through Task Node, or baked into the 15-minute static bundle (not actually live).
- **Why it matters:** Implementing this ad hoc either breaks under a hardened CSP, leaks Market Lens credentials to the browser, or delivers 15-minute-stale prices while promising "live" movers. It also leaves unclear how equities vs. crypto/perps movers behave during/outside market hours.
- **How to investigate:** Inspect `apps/company_model_viewer/app.js` and `hub.py` to see how movers and Market Lens fallback are currently fetched, authenticated, and mapped across equities, crypto, and macro tickers.
- **Suggested direction for fix:** Explicitly specify the movers architecture: e.g., a lightweight Task Node cached proxy endpoint (polling Hyperliquid and Market Lens server-side every 15–60 seconds, keeping API keys server-side, honoring rights flags, and serving gated Finance Hub users on the same origin without widening CSP to third-party domains), with explicit asset-class handling for 24/7 crypto vs. market-hours equities.

##### P9 — Validator cache (R3) conflicts with immediate revocation (R9 / AC3–AC4), and Research API key revocation is ambiguous
- **Severity:** major
- **Where:** `Technical requirements` (R1, R3, R9, R26), `Acceptance criteria` (AC3, AC4)
- **What is wrong:** R3 states *"Results are cached, so file requests never wait on the registry,"* while R9 and AC3–AC4 require that once an account stops qualifying, is suspended, or is blacklisted, *"its next file request fails."* The spec does not separate caching of the upstream L1 registry state from live local checks (`account_blacklist`, Finance Hub suspensions, local validator unbinding), nor specify cache TTLs or invalidation hooks. Additionally, R26 states *"Validators keep research API keys"* without verifying whether existing Research API key routes call `accountMeetsTier` per request or rely on long-lived static grants.
- **Why it matters:** A cached composite `accountMeetsTier` result can allow a suspended, blacklisted, or unbound user to keep downloading files for the remainder of the cache TTL, violating AC3–AC4. Similarly, stale Research API keys can remain usable after suspension.
- **How to investigate:** Inspect `server/validator-bundles.js`, `server/repositories/validator-bindings.js`, `server/l1-validator-identity.js`, and the Research API key verification code referenced in `docs/wiki/surfaces/validator-research.md`.
- **Suggested direction for fix:** Specify a two-layer check in `accountMeetsTier`: (1) **uncached / explicitly invalidated local checks** (blacklist, Finance Hub suspension, local binding existence, latest top-30 snapshot row) evaluated per request or invalidated synchronously on write; and (2) **cached upstream L1 registry/uptime state** with an explicit TTL (e.g., 5 minutes) and stale-feed fallback. Explicitly verify and test that Research API key requests enforce the same live suspension/blacklist/basis check.

##### P10 — `agent_brief_public.json` relies on brittle post-hoc output checking, and Finance Hub omits Pillar 10's user-to-coverage linkage
- **Severity:** major
- **Where:** `User story` (#4), `Technical requirements` (R19), `Acceptance criteria` (AC13)
- **What is wrong:**
  1. R19 states *"The build fails if the public file has model numbers or targets."* Checking generated text/JSON after the fact for proprietary figures is inherently leaky (qualitative paraphrases slip past regexes; public historical financials cause false positives).
  2. R19 does not specify failure behavior if Task Node is unreachable when the 15-minute model build pulls `contributor_insights.json` (carry forward last known insights vs. fail the build).
  3. Neither User Story #4 nor R19 surfaces Pillar 10's founder requirement that *"The coverage universe in finance hub is linked to specific users"* (e.g., showing linked domain contributors/coverage specialists on a company page, subject to privacy/opt-in settings).
- **Why it matters:** A leaky public-brief filter risks exposing paid/proprietary price targets to non-members during public Pillar 10 interviews; a hard failure on insight pulls would stall model updates whenever Task Node restarts; and omitting user-coverage linkage weakens the bridge between Finance Hub and the Information Hive Mind.
- **How to investigate:** Review Pillar 10 (`10-information-hive-mind.md`) and `158_finance_agent.sql` to check how contributors are classified per company/protocol and what fields `agent_brief_public.json` actually needs.
- **Suggested direction for fix:**
  - Generate `agent_brief_public.json` strictly from a **taint-free, public-only source schema** (company metadata, asset class, tags, public filings/events, open interview questions) that never reads model targets/forecasts as inputs—keeping the output check only as secondary defense-in-depth.
  - Specify graceful carry-forward (with staleness timestamp + alert) if the insight pull fails.
  - Add a **Linked contributors / domain specialists** section (opt-in handle/Hive ID or verified specialist count + attribution on insights) to company pages and `coverage_manifest.json` aligned with Pillar 10.

##### P11 — Visit-tracking heartbeat breaks for pinned/background browser tabs and omits first-visit behavior
- **Severity:** minor
- **Where:** `Technical requirements` (R16), `Acceptance criteria` (AC7)
- **What is wrong:** R16 updates `last_active_at` on *"a heartbeat while open"* and advances `visit_baseline_at` only when a load occurs after a configured idle gap. It does not gate the heartbeat on tab visibility or user activity, nor define what `visit_baseline_at` is on a user's very first visit.
- **Why it matters:** Financial users routinely keep market dashboards pinned in an open browser tab for days. An ungated background heartbeat keeps `last_active_at` fresh forever, so `visit_baseline_at` never rolls forward and *new* markers accumulate indefinitely until the user clicks "Mark caught up." On a first visit, a `null` baseline could mark all 60 companies as "new."
- **How to investigate:** Check `FinanceHubView.jsx` and `app-realtime.js` to see how activity/presence heartbeats are currently implemented in Task Node.
- **Suggested direction for fix:** Gate heartbeats on `document.visibilityState === "visible"` plus recent user interaction (e.g., pointer/scroll/key event within the last 5 minutes), handle visibility-resume transitions after the idle gap as a new visit (not only full page loads), and set initial first-visit `visit_baseline_at` to `now - 24h` (or `now - 48h`) so new members see recent activity without marking the entire archive as unread.

##### P12 — Daily-habit UX gaps: no Goodbrief archive, unspecified cross-asset ranking, and unspecified local-to-hosted auth handoff
- **Severity:** minor
- **Where:** `User story` (#2, #5), `Technical requirements` (R12, R13, R25)
- **What is wrong:**
  1. Goodbrief handling only covers the latest/today's brief—there is no archive (e.g., last 7–30 days of briefs) for members who miss a day.
  2. Merging Equities, Crypto, and Macro onto one Today page does not specify default sort order within each section (recency vs. magnitude of upside change vs. watchlist pinning) or how non-equity assets rank alongside equities with earnings/targets.
  3. R25 specifies that the native local Task Node app opens hosted Finance Hub in the system browser, but does not specify how authentication transfers from the local app to the browser.
- **Why it matters:** Small daily friction—missing yesterday's Goodbrief, noisy mixed-asset lists, or repeated logins from the local app—directly undermines the founder's goal of a daily-habit portal.
- **How to investigate:** Check whether `pfrpc` / `news_summary_pipeline.py` retains historical daily briefs and inspect Pillar 5 (`05-local-task-node.md`) to see how the local app authenticates against hosted Task Node APIs.
- **Suggested direction for fix:** Include a rolling archive (e.g., last 30 Goodbriefs) in the `goodbrief` publication index; specify deterministic default sorting per Today section (watchlist items pinned first, then reverse-chronological within section, with magnitude badges); and define a clean auth handoff (e.g., short-lived single-use login handoff token or standard hosted session) from the local app in R25.

##### P13 — Per-file server-side encryption with `private, no-store` on all assets lacks threat-model clarity and risks latency/DB drag
- **Severity:** minor
- **Where:** `What exists today`, `Technical requirements` (R7, R9, R10, R20)
- **What is wrong:** Every file—including static viewer JS/CSS/icons as well as research JSON—is encrypted per publication in Postgres, decrypted on the server on every request, logged in R10, and served with `Cache-Control: private, no-store`. Once R7 removes client-side `unlock`, the spec does not explain where publication decryption keys are stored (if in the same DB, encryption adds negligible security) or separate static viewer code from sensitive research data.
- **Why it matters:** Disabling browser caching (`no-store`) for non-secret viewer JS/CSS forces full re-downloads and DB decryption on every reload/mobile visit, inflates R10 request logs with static asset noise, and increases server load unnecessarily.
- **How to investigate:** Inspect `server/validator-bundle-files.js`, `scripts/publish-validator-bundle.mjs`, and `159_validator_bundle_files.sql` to check key storage location, response latency per file, and how static viewer assets are currently bundled alongside `hub/*.json` and `models/*`.
- **Suggested direction for fix:** Separate **public/static viewer shell assets** (immutable content-hashed JS/CSS, cacheable with `public, max-age=31536000, immutable`) from **gated research data** (`hub/*.json`, `models/*`, `goodbrief/*`, served `private, no-store` and logged for R10). Document where per-publication encryption keys live (KMS/env master key envelope vs. same DB) and eliminate redundant encryption if keys reside alongside ciphertext without envelope isolation.

## Claude Opus 5.5 — 80/100

*Model `anthropic/claude-opus-5.5` · 9567 output tokens*

#### Summary judgement

This is a strong, code-grounded spec. It matches the founder's Deliverable 9 closely. It covers both access paths (validator and top-30), a single daily landing page with model updates, earnings, coverage changes and the Goodbrief, and the "validators also get GoodAlexander content" premise. It also finds real, present-day weaknesses that the founder did not mention but that decide whether the perk is worth anything:

- a whole-version key handed out forever;
- public whole-bundle ciphertext;
- the Goodbrief's own scripts running on Task Node's origin;
- unsigned `scp` publishing straight into the production database;
- old-chain validators getting access, which works against the new-chain migration goal.

The delivery order, open questions and acceptance criteria are concrete and mostly testable.

What keeps it from the high 80s or 90s:

- **Sequencing inconsistencies.** R7 sits in Step 1 but depends on R23 in Step 3. R11 claims "no code change", yet the spec admits the Goodbrief split must land first.
- **The founder's headline path is deferred.** The top-30 route is pushed to the last step and blocked on open questions, rather than shipped with a sensible default.
- **An unexamined failure mode in the sandboxing design.** Gated, cookie-authenticated subresources may not load from an opaque origin.
- **A content-rights risk that already exists** (Bloomberg-derived text read by outside operators) is treated as an open question to apply "whenever", not as a first-step blocker.
- **The daily-habit UX goal is thinly specified.** Sections are listed, but there is no design or iteration loop, no load-time target, and no general "what's going on in markets" context outside the coverage universe.

#### Correspondence to the founder's intention

**Strong matches:**

- **Access for validators OR top-30 users.** This is explicit in R1, R3 and R4, with the basis shown in the sidebar and a locked page that explains both ways in.
- **"Clear portal of all the most recent model updates, earnings releases and changes across the coverage universe."** This maps directly to the Today sections: *Just reported*, *Model updates*, *Reporting this week* and *Coverage changes*. R15 adds a coverage diff that does not exist today.
- **Goodbrief access.** Covered by the summary on Today, the full brief in a sandboxed frame, its own signed publication (R21) and opt-in notices (R24).
- **"A place I want to log in every day."** Covered by the visit and new-since-last-visit markers (R16), watchlist and search (R17), mobile width (R13) and usage metrics (R18).
- **Validator premise: Finance Hub plus GoodAlexander content.** R26 adds a Library and keeps research API keys under the same entitlement.
- **Ties to Deliverable 10.** "Coverage universe linked to specific users" is served by the coverage manifest, the contributor-insights files and the Ask-the-agent hand-off (R19).
- **Ties to the larger goal.** The "Bubbles up to Outcome B" framing correctly sees Finance Hub as a pull toward the new chain. Open question 1(b) names the conflict with today's any-binding rule.

**Gaps or drift:**

- **The top-30 path is last.** Step 4 is gated on open questions 2–4, even though the founder named it as one of the two core access paths. A default (for example, the current leaderboard formula over a rolling window, everyone ranked privately) could ship sooner, with the questions tuning parameters afterwards.
- **"See what is going on in markets"** is broader than the coverage universe. Today shows movers of covered names and the Goodbrief. There is no broad market strip (indices, rates, FX, BTC/ETH, funding) or a "macro calendar today".
- **Security and publishing hardening outweighs the user experience.** R7–R11 and R20–R23 are well justified, but the founder's story is mostly about the reader's experience. The "intuitively designed" part is reduced to a section list plus founder sign-off. There are no wireframes, no information hierarchy rationale and no design iteration step.
- **On-chain representation of the entitlement is barely mentioned** (only an optional Pillar 6 gating). The founder's broader prompt is heavily on-chain: identity, the block explorer, deterministic records.
- **Density.** The page is written for implementers. A founder reading it would struggle to find the five decisions that actually need their input.

#### Technical robustness

**Strengths:**

- One access function returning a structured basis.
- Per-request, metadata-driven allowed bases.
- A fail-closed default for unknown tiers.
- A two-step tier migration with an integration test.
- A daily snapshot with a deterministic tiebreak and last-good fallback.
- A signed, versioned, per-index publish manifest with rotation, rollback and revocation.
- Removing `'unsafe-inline'` from the viewer's CSP.
- Retiring the pfrpc database credential.
- A schema version per hub file, with a contract test.
- A visit model built to survive refreshes, extra tabs and devices.

**Weaknesses:**

- **Sequencing bug between R7 and R23.** Step 1 includes refusing `unlock` and returning 404 on `…/content`, but production use of per-file loading is unconfirmed until R23 in Step 3. If production still loads the whole bundle, Step 1 breaks the hub for everyone.
- **Sandbox side-effects not examined.** A sandboxed opaque-origin Goodbrief will likely fail to fetch its own images, CSS or JS from the cookie-gated per-file route. Cross-site cookie rules (SameSite) and CORS from a `null` origin both get in the way.
- **The viewer stays same-origin and unsandboxed.** It renders model notes and "why" fields, which may themselves be LLM-written. A single escaping bug gives full session compromise. Serving from a separate origin would give defense in depth.
- **No bound on stale validator data.** "A stale validator feed never revokes access" has no maximum staleness, so a dead feed means indefinite access.
- **Background tabs keep a visit alive.** Heartbeats are not tied to page visibility, so an open background tab extends a visit indefinitely and degrades the "since last visit" meaning.
- **Rank churn.** A daily snapshot plus grace can let an account oscillate around rank 30 and keep access permanently. This is noted in open question 4, but there is no default.
- **No performance target.** R23 measures bytes, not time to interactive on a phone.

#### Economic coherence

- **The incentive logic is sound.** The perk only has value if it is exclusive and keeps arriving. Tying the validator path to Pillar 3's active state (open question 1(b)) aligns the perk with migration to the new chain. The page correctly notes that today's any-binding rule rewards the old chain.
- **Gaming analysis is present but stops before a recommendation:**
  - All-time totals entrench incumbents.
  - The alignment score can add up to 100 points, as much as 33 network tasks.
  - There may be heavy overlap between validators and the top 30.
- **Supply costs are explicitly left unpriced** (licences, LLM compute, labour, two hosts). There is no estimate of cost per seat, or of whether licence cost scales with outside readers.
- **Content rights are the largest economic and legal risk.** The Goodbrief is an LLM synthesis of Bloomberg articles, already served to outside operators, with the vendor name scrubbed. This belongs in Step 0, not a deferred open question.
- **Exclusivity versus marketing is not discussed.** The founder's broader aim is public storytelling about Post Fiat. Some teaser or public layer, such as the public agent brief, may serve growth better than a fully closed hub. The page never weighs this tension.

#### Problems

**P1 — R7 depends on R23, but they sit in different steps**
- **Severity:** critical
- **Where:** R7; Delivery order Steps 1 and 3; R23.
- **What is wrong:** Step 1 ships refusal of `unlock` and a 404 on `…/content` for `finance_hub` indices. R7 itself says the switch should happen only once R23 confirms per-file loading in production, and R23 is in Step 3.
- **Why it matters:** If production viewers still fetch the whole bundle, Step 1 takes the hub down for every validator.
- **How to investigate:**
  - Check the live `ga-models` row for `metadata.fileFormat`.
  - Check whether the deployed viewer build includes PR #384's demand loading.
  - Grep the static sites for `/indices/` content calls.
- **Suggested fix:** Move R23 (and the static-site audit) to the start of Step 1 as a precondition, or make R7's endpoint closure conditional on a feature flag flipped after R23 passes.

**P2 — The sandboxed Goodbrief may not load its own assets**
- **Severity:** major
- **Where:** R8, Goodbrief frame; acceptance criterion 5.
- **What is wrong:** The full HTML brief is served with `sandbox allow-scripts`, which gives it an opaque origin. Any images, CSS or JS it references through the cookie-gated per-file route may not receive cookies (SameSite or third-party rules) and will face CORS from a `null` origin.
- **Why it matters:** The brief could render broken, or the team will be tempted to relax the gate.
- **How to investigate:** Inventory the brief's subresources. Prototype the sandboxed load in Chrome, Safari and Firefox (including iOS Safari) against the gated route.
- **Suggested fix:** Make the build inline all brief assets as data URIs or a single self-contained HTML file, or issue short-lived signed URLs for its subresources. Add an acceptance criterion that the brief renders fully when sandboxed on desktop and mobile Safari.

**P3 — The top-30 access path is deferred to the last step**
- **Severity:** major
- **Where:** Delivery order Step 4; R4; R5.
- **What is wrong:** One of the founder's two named access paths waits until open questions 2–4 are answered.
- **Why it matters:** The founder's story ("Once I am a validator or am a top 30 task node user") is half unmet for an unknown period. Task Node contributors get no new reason to do great work.
- **How to investigate:** Ask whether the open-question answers change code or only parameters. The spec itself says they are parameters.
- **Suggested fix:** Ship R4 and R5 in Step 2 with explicit defaults. For example:
  - a rolling 90-day window over the existing formula;
  - alignment capped;
  - all accounts ranked privately;
  - 14-day grace with no repeat within 30 days.
  
  The founder's answers then retune those parameters.

**P4 — The content-rights exposure is not prioritized**
- **Severity:** major
- **Where:** R11; open question 5; Economics; Delivery order ("R11 applies whenever open question 5 is answered").
- **What is wrong:** A Bloomberg-derived LLM synthesis, options data and Market Lens fallback data are already served to outside readers. The vendor name is scrubbed, and the page treats this as a deferred question.
- **Why it matters:** This is a licensing and legal risk, and it gets worse as top-30 access widens the audience.
- **How to investigate:** Review the relevant licence terms for redistribution of derived content. List who can read the Goodbrief today, counting old-chain bindings.
- **Suggested fix:** Make the open question 5 decision a Step 0 gate before any audience widening. Default to option (c), models only for outside readers, until cleared. Note that this needs R21's Goodbrief split, so either R21 moves earlier or the interim restriction is implemented on the combined bundle.

**P5 — R11's "no code change" claim contradicts the Goodbrief split requirement**
- **Severity:** major
- **Where:** R11; R1; Delivery order Step 3 note.
- **What is wrong:** R11 says open question 5's answer is applied through allowed bases "with no code change". But the Goodbrief currently ships inside `ga-models`, and the spec says the split (R21) must land before any basis-specific Goodbrief rule.
- **Why it matters:** Founder answers of 5(b) or 5(c) cannot be honoured quickly, which undermines the claim that "the build is the same under every option".
- **How to investigate:** Trace how the Goodbrief files sit inside the `ga-models` bundle, and whether per-file allowed bases could exclude the Goodbrief paths before R21.
- **Suggested fix:** Either support per-path allowed bases within an index, or pull R21 forward ahead of R11. Correct the wording either way.

**P6 — The viewer stays same-origin and unsandboxed**
- **Severity:** major
- **Where:** "Rendering decision"; R8; the View row of *What exists today*.
- **What is wrong:** The viewer renders model notes, "why" text, call takeaways and contributor insights, some of it LLM-written, inside a same-origin iframe without `sandbox`. Security rests entirely on the escaping renderer and the CSP.
- **Why it matters:** One escaping bug or CSP gap means the Task Node session is compromised, including wallets, tasks and possibly keys.
- **How to investigate:**
  - Audit which viewer fields can carry LLM or external text.
  - Check what Task Node APIs a same-origin frame can call with the user's cookies.
  - Assess the cost of moving the viewer to a separate origin with token-based access.
- **Suggested fix:** Serve the viewer from a separate origin, such as `hub.<domain>`, with a short-lived scoped token passed over `postMessage`. Or at minimum use `sandbox="allow-scripts"` without `allow-same-origin` and route data through the bridge. Add escaping tests for every LLM-populated field, not only contributor insights.

**P7 — No market-wide context**
- **Severity:** major
- **Where:** User story, Today sections; R12.
- **What is wrong:** The founder wants to "see what is going on in markets". Today shows covered-name movers and the Goodbrief summary, but no broad context:
  - indices, rates, dollar, oil;
  - BTC/ETH and funding rates;
  - today's macro calendar.
- **Why it matters:** The daily-habit goal depends on the page being the first place to look. Without market context, users still go elsewhere first.
- **How to investigate:** Ask what market data is already available on the market-data box or from Hyperliquid at no licensing cost.
- **Suggested fix:** Add a compact "Markets" strip at the top of Today (sourced and licence-checked), plus a "Macro calendar today" item. Include both in the render gate and acceptance criterion 6.

**P8 — The UX design process is thin**
- **Severity:** minor
- **Where:** R13; acceptance criterion 6.
- **What is wrong:** "Intuitively designed" becomes a list of sections plus a final founder sign-off. There are no wireframes, no prioritization rationale, no load-time target and no iteration loop.
- **Why it matters:** The likely outcome is a functional but uninspiring list page, with founder rejection discovered late.
- **How to investigate:** Check whether any mock exists. Measure current time to interactive on a mid-range phone.
- **Suggested fix:**
  - Add a wireframe or mock step for founder review before Step 2 build.
  - Set a performance budget, for example Today interactive in under 2 s on 4G for a phone.
  - Add one iteration round after a week of real usage, using R18 data.

**P9 — No maximum staleness for the validator feed**
- **Severity:** minor
- **Where:** R3; acceptance criterion 2.
- **What is wrong:** "A stale validator feed never revokes access" has no upper bound.
- **Why it matters:** A broken feed silently freezes the access list forever, including validators that have since lapsed.
- **How to investigate:** Check how Pillar 3 defines feed freshness and its own fallback.
- **Suggested fix:** Add a configurable hard ceiling (for example 7 days). Past it, grants move to grace or alert loudly. Add an acceptance test for this.

**P10 — Background tabs keep visits open**
- **Severity:** minor
- **Where:** R16; acceptance criterion 7.
- **What is wrong:** A heartbeat "while open" from a background tab keeps `last_active_at` fresh, so the visit never ends and the "since last visit" markers lose meaning.
- **Why it matters:** This degrades the main return-visit feature.
- **How to investigate:** Review how the heartbeat is implemented.
- **Suggested fix:** Send heartbeats only while `document.visibilityState === "visible"` and the user has interacted recently.

**P11 — Rank oscillation and the grace default**
- **Severity:** minor
- **Where:** R5; open question 4.
- **What is wrong:** With daily snapshots, an account hovering near rank 30 can chain grace periods into permanent access. No default is proposed.
- **Why it matters:** The scarcity the Economics section relies on erodes.
- **How to investigate:** Simulate on historical leaderboard data how many accounts would cross the rank-30 boundary each month.
- **Suggested fix:** Propose defaults: no new grace within N days of the last one, or a hysteresis band (enter at 30, keep until 40).

**P12 — Unpriced economics and no cost-per-seat view**
- **Severity:** minor
- **Where:** Economics, "Supply cost".
- **What is wrong:** Licences, LLM compute, labour and hosts are explicitly left unpriced.
- **Why it matters:** Decisions to widen access (open question 9) or to buy licences cannot be made without at least an order-of-magnitude figure.
- **How to investigate:** Pull current host bills, LLM spend for the Goodbrief and models, and licence terms.
- **Suggested fix:** Add a rough monthly cost table and cost per eligible account. Make open question 9's thresholds reference it.

**P13 — Exclusivity versus public storytelling is not weighed**
- **Severity:** minor
- **Where:** Economics, "Scarcity"; R19 public agent brief.
- **What is wrong:** The founder's wider prompt stresses public visibility of Post Fiat's experiment. A fully closed hub gives no funnel, and the spec never weighs a teaser layer.
- **Why it matters:** The hub could drive recruitment of validators and contributors if non-members could see what they are missing.
- **How to investigate:** Review what `agent_brief_public.json` already contains and whether it is licence-clean.
- **Suggested fix:** Consider a public locked-page preview, such as the coverage list, headline counts and yesterday's Goodbrief headline (if rights allow), linking to the validator and top-30 paths.

**P14 — Minimal on-chain linkage of the entitlement**
- **Severity:** minor
- **Where:** Dependencies (Pillar 6, "optional").
- **What is wrong:** Access is checked purely off-chain from Task Node tables. The founder's broader vision favours deterministic, explorer-visible records, and the snapshot itself is a natural candidate.
- **Why it matters:** This is a missed narrative alignment, and other pillars (1, 4, 6, 8) read the top-30 snapshot but cannot verify it.
- **How to investigate:** Check whether the Pillar 2 message protocol could carry a signed daily snapshot hash.
- **Suggested fix:** Optionally publish a signed hash of each daily top-30 snapshot as a message-protocol memo, so other pillars and the block explorer can verify it.

**P15 — Unverifiable or odd references**
- **Severity:** minor
- **Where:** The per-file loading plan file name `finance-hub-demand-loading-20261111.md`; PR #384; private repository references.
- **What is wrong:** The date in the file name looks like a typo or a future date. Several claims rest on private code that reviewers cannot check.
- **Why it matters:** The status claims drive the sequencing (see P1).
- **How to investigate:** Verify that the links resolve and that the status statements match `main` and production.
- **Suggested fix:** Correct the references. Mark claims as "verified on <date>" in the status block.

**P16 — Readability for the founder**
- **Severity:** minor
- **Where:** The whole page; Open questions.
- **What is wrong:** There are nine dense open questions, each with sub-options and no recommended default.
- **Why it matters:** Decision latency blocks Step 4 and R11.
- **How to investigate:** Check which questions actually gate shipping.
- **Suggested fix:** Give a recommended default for each open question, and list the two or three decisions that truly block the work at the top of the page.
