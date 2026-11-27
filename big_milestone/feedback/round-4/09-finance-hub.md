# Feedback — Pillar 9 — Finance Hub User Spec

*Execution-hole audit · Round 4.*

> **Mean score:** 75/100 · Gemini 4 Neon 75 · Claude Opus 5.5 75
>
> **Method:** each model received (1) the founder's original prompt, (2) locked founder decisions & live state, (3) master `README.md`, and (4) [this pillar's spec](../../pillars/09-finance-hub.md), once, scored across 5 explicit rubric dimensions (Execution Holes /30, Economics /20, Founder Alignment /20, Milestones & 4 Gates /15, Readability /15). Run 2026-11-26 23:35 UTC via OpenRouter.

## Gemini 4 Neon — 75/100

*Model `google/gemini-4-neon` · 16026 output tokens · `tech=16/30 econ=15/20 align=18/20 ship=12/15 read=14/15`*

#### Summary judgement

Pillar 9 is a clean, readable product spec grounded in the real Task Node codebase and faithful to every locked founder decision. It rightly prioritizes closing today's whole-bundle leak paths, isolating Goodbrief HTML, replacing `scp`/direct-DB publishing with a signed Publish API, and uniting equities, crypto, and macro into a daily **Today** desk.

Before engineering starts, however, there are **three critical security/architecture holes and four major execution/sequencing holes** that will either break in production or undermine the exact leak and XSS defenses this pillar is meant to establish. Specifically: uploading `app.js` into a same-origin viewer via a hot cron key defeats XSS protection; `<iframe sandbox="allow-scripts">` breaks authenticated Goodbrief asset loading while still allowing network exfiltration without a strict CSP; placing `agent_brief_members.json` inside `ga-models` re-opens single-request bulk scraping of all ~60 models; the Top-30 hysteresis/grace state machine allows an infinite grace loop; and the pre-launch content-rights review is scheduled in M5 *after* M4 opens Finance Hub to Top-30 users.

#### Rubric breakdown

- **Execution Holes & Technical Robustness (16/30):** Grounded in real code paths, but contains 3 critical and 4 major security, isolation, state-machine, and cross-pillar interface holes that will bite during build and rollout.
- **Economic & Incentive Coherence (15/20):** Strong validator/contributor motivation loop, weakened by an infinite-grace loophole at ranks 34–36, suspended accounts hogging Top-30 seats, and a single-file bulk-scrape leak via `agent_brief_members.json`.
- **Founder Decision Alignment & Zero Open Questions (18/20):** Cleanly incorporates every locked founder decision with zero open questions; minor deduction for deferring concrete rate-limit/staleness defaults and placing the pre-launch rights check after M4.
- **Actionable Milestones & 4 Ship Gates (12/15):** Clear `[ ]` milestones and all four mandatory ship gates with strong user tests, but M4/M5 sequencing is inverted for the rights check and M1 omits dual-chain validator transition handling.
- **Plain-English Readability & Zero Jargon Bloat (14/15):** Crisp, direct prose with zero `R1..R31` bloat and a helpful architecture diagram; only minor terseness where compressed phrasing hides state-machine ambiguity.

#### Execution holes

##### H1 — Same-origin `ga-models` viewer + hot Publish API key = full Task Node XSS
- **Severity:** `critical`
- **Where:** Locked-in design decisions ("Keep same-origin viewer"), Section 3 ("Signed Publish API"), Code references (`navstrategies-company-models/apps/company_model_viewer/app.js`).
- **Hole:** Today the viewer JavaScript (`app.js`) lives inside the external `navstrategies-company-models` repo and is published into `ga-models`, which renders **same-origin** with Task Node. Moving from `scp` to a Publish API signed by an automated **hot cron key** on the research host does not isolate Task Node's origin. Dropping `'unsafe-inline'` does not stop same-origin `<script src>` files published inside the bundle.
- **Why it breaks execution:** Any compromise of the research host, its Python/JS build dependencies, or its hot 15-minute cron key lets an attacker ship malicious `app.js` directly onto Task Node's origin — stealing sessions and acting as every validator and Top-30 contributor who opens Finance Hub.
- **Minimal plain-English fix:** Move the first-party viewer code (`app.js`, HTML, CSS) into Task Node's own code-reviewed repository and deployment pipeline, and restrict the `ga-models` Publish API to a **strict data-only file allowlist** (`.json` and raster `.png`/`.webp` only — reject `.js`, `.html`, and `.svg`).

##### H2 — Sandboxed Goodbrief iframe breaks authenticated loading/assets and allows network exfiltration
- **Severity:** `critical`
- **Where:** Locked-in design decisions & Section 3 ("sandbox full HTML Goodbrief inside `<iframe sandbox="allow-scripts">` with matching CSP").
- **Hole:** An `<iframe sandbox="allow-scripts">` runs in an opaque `null` origin (no `allow-same-origin`). The spec neither explains how authenticated Goodbrief HTML and its relative CSS/JS/image subresources load into an opaque origin, nor locks down outbound network requests (`connect-src 'none'`). It also leaves `postMessage` origin verification unspecified (`event.origin` is `"null"`).
- **Why it breaks execution:** Pointing `src=` at a Bearer-authenticated endpoint fails auth, while loading via `srcdoc` breaks relative asset URLs. Under pressure, engineers add `allow-same-origin` to make assets load — which completely destroys the sandbox (`allow-scripts allow-same-origin` lets scripts strip the `sandbox` attribute and hijack Task Node). Meanwhile, without `connect-src 'none'`, malicious LLM/HTML script inside the opaque iframe can still `fetch()` the full Goodbrief text out to an attacker server.
- **Minimal plain-English fix:** Require the `goodbrief` publisher to emit a **single self-contained HTML file** (all CSS, scripts, and charts/images inlined); have the authenticated Task Node shell fetch it and inject it via `srcdoc` into `<iframe sandbox="allow-scripts">` (never `allow-same-origin`) with a strict CSP (`default-src 'none'; img-src data:; style-src 'unsafe-inline'; script-src <sha256-hash or 'unsafe-inline'>; connect-src 'none'; form-action 'none'`); allow only a height-resize message from the frame and never trust `event.origin === "null"` for privileged actions.

##### H3 — Pillar 10 brief files (`agent_brief_members.json`, `agent_brief_public.json`, `contributor_insights.json`) break leak defense, non-member access, and MNPI takedowns
- **Severity:** `critical`
- **Where:** Section 2 ("GoodAlexander Library & Pillar 10 bridge") and M5.
- **Hole:** Four coupled flaws in the Pillar 9 ↔ Pillar 10 file contract:
  1. Storing `agent_brief_members.json` inside the `ga-models` user-servable bundle lets any member fetch a summary of all ~60 proprietary models in **one GET request**, defeating M1's whole-bundle leak lockdown and per-file rate limits.
  2. Storing `coverage_manifest.json` and `agent_brief_public.json` behind the `finance_hub` per-file route blocks **non-member** Pillar 10 users (who must be able to run company interviews per Pillar 10's locked design).
  3. Baking Pillar 10's `contributor_insights.json` into static `ga-models` bundles makes post-hoc MNPI retractions slow and leaves redacted claims inside immutable snapshots.
  4. Testing that `agent_brief_public.json` does not leak model targets via negative checks misses free-text commentary leaks (e.g., prose quoting upside or fair value).
- **Minimal plain-English fix:** Tag bundle files with three visibility scopes in the manifest: `internal` (server-only, never served by browser routes — used for `agent_brief_members.json`, read per-ticker by Task Node's agent backend), `public_meta` (readable by Pillar 10 for non-members — `coverage_manifest.json` and `agent_brief_public.json`, built strictly from a **positive field allowlist** with zero model numbers or internal prose), and `finance_hub` (user-facing per-file route). Serve `contributor_insights` live from Task Node's Pillar 10 database so MNPI takedowns take effect immediately.

##### H4 — Top-30 hysteresis, grace, suspension, and validator-overlap state machine has an infinite-grace loophole and seat-count ambiguity
- **Severity:** `major`
- **Where:** Locked-in design decisions & Section 1 ("Unified `finance_hub` tier & daily Top-30 snapshot").
- **Hole:** The spec leaves four state transitions undefined:
  1. **Infinite grace loop:** If a user drops to rank 36 (enters 7-day grace) and recovers to rank 33 on day 5, does `keep-until rank 35` reset them to active? If so, oscillating between rank 34 and 36 once a week grants permanent free access without ever re-entering Top 30.
  2. **Seat count:** Do rank 31–35 incumbents consume seats inside the 30 cap (blocking a new #30) or sit on top of the 30 seats?
  3. **Suspended leaker seat hogging:** Suspended accounts are blocked at `accountMeetsTier`, but only "blacklisted" accounts are excluded from the leaderboard pool — letting a suspended scraper hog a Top-30 seat.
  4. **Validator-grace boundary:** It is unspecified whether a validator in Pillar 3's 14-day grace window still counts as a "validator" (excluded from the 30 non-validator seats) or abruptly re-enters the non-validator pool and evicts #30.
- **Minimal plain-English fix:** Lock the exact daily state rule: (1) exclude blacklisted, Finance-Hub-suspended, and active-or-grace-window validators before ranking; (2) ranks `1..30` always get active `top30` status; (3) previous-day **active** (non-grace) `top30` holders at ranks `31..35` stay active *in addition* to the 30 seats (so 30–35 active seats total); (4) dropping below 35 starts a 7-day grace window that only clears by re-entering ranks `1..30` (never via `31..35`).

##### H5 — Signed Publish API contradicts monotonic versions with rollback, lacks key rotation, and exposes staging uploads to DoS
- **Severity:** `major`
- **Where:** Section 3 ("Signed Publish API & Goodbrief split").
- **Hole:**
  1. Enforcing a strictly monotonic manifest version (for replay protection) conflicts with "signed rollback" unless rollback is a **new higher-sequence manifest** pointing to a retained prior snapshot. Re-submitting an old manifest either fails the monotonic check during an incident or disables replay protection (letting an attacker replay retracted/bad research).
  2. There is no `key_id`, dual-key overlap rotation, or emergency key revocation for the pinned per-index keys.
  3. Staging uploads before manifest commit lack transport/signature auth, per-file/total-bundle byte quotas, and automatic garbage collection of abandoned uploads.
- **Minimal plain-English fix:** Separate strictly monotonic `manifest_seq` (always increments, even on rollback) from content-addressed `snapshot_id` (keep the last 10 snapshots for instant rollback); identify signing keys by `key_id` with 2 active key slots per index and an instant revoke flag; require authenticated staging uploads with strict byte/file-count caps (e.g., 50 MB / 500 files per commit) and 1-hour GC on uncommitted blobs.

##### H6 — Dual-chain validator migration and Pillar 3 feed outage behavior are missing
- **Severity:** `major`
- **Where:** Section 1 ("Unified `finance_hub` tier"), M1, and Wave 1 execution order.
- **Hole:** M1 ships in Wave 1 and wires `finance_hub` to Pillar 3's active validator state while Pillar 3's `postfiatl1v2` uptime observers and outside-validator migration are still being rolled out. Unlike the Top-30 snapshot (which explicitly keeps the last good snapshot on failure), Pillar 9 defines neither how existing **old-chain validators** bridge across Waves 1–3 without staying unlocked forever after lapsing, nor what `accountMeetsTier` does when Pillar 3's liveness feed stalls.
- **Why it breaks execution:** Failing closed locks out every active validator during a single monitoring glitch or locks out unmigrated old-chain validators on Day 1 of Wave 1; failing open with no bound lets offline validators keep access indefinitely.
- **Minimal plain-English fix:** Consume a single Pillar 3 per-operator entitlement view that covers both old-chain validators (until Stage 4 sunset, one grant per operator) and `postfiatl1v2` validators, and cache the last verified entitlement snapshot in Task Node with a bounded staleness fallback (honor last-known-good for up to 24 hours while paging operators, rather than instantly locking out validators on a collector hiccup).

##### H7 — Content-rights review is sequenced in M5 *after* M4 opens Finance Hub to Top-30 users (and Today-page Goodbrief coupling risks bypassing `allowed_bases`)
- **Severity:** `major`
- **Where:** Build milestones M4 & M5, Section 2 ("Today view").
- **Hole:** M4 opens Finance Hub (including Goodbrief, consensus comparisons, options-implied moves, and live movers) to 30 non-validator contributors, while M5 runs the "pre-launch content-rights review for Goodbrief/market-data distribution." In addition, the Today view combines `ga-models` and `goodbrief` on one screen without specifying how it behaves if `allowed_bases` disables `goodbrief` for a tier.
- **Why it breaks execution:** Redistributing licensed market-data/brief inputs to an expanded rewarded user tier before verifying vendor rights risks immediate takedown or having to yank Goodbrief away from Top-30 users right after launch. Worse, if `ga-models` duplicates Goodbrief summary text inside its own `hub/today.json`, toggling `goodbrief`'s `allowed_bases` will silently fail to gate the summary.
- **Minimal plain-English fix:** Move the content-rights review and initial `allowed_bases` config to a hard gate **before M4** opens Top-30 access; keep Goodbrief summary data strictly inside the `goodbrief` index and have the Today page gracefully hide or show a locked notice for section 1 if the `goodbrief` fetch returns `403`.

##### H8 — Leak controls leave concrete defaults undefined and allow three bypasses (Library signed URLs, client-only watermarks, browser/Mac disk caching)
- **Severity:** `minor`
- **Where:** Locked-in design decisions, Section 2 (GoodAlexander Library), Section 3 (Content isolation & leak defense).
- **Hole:**
  1. Serving Library reports via "short-lived signed URLs" bypasses per-file auth, mid-TTL suspension, rate limiting, and per-user watermarking.
  2. Purely visual DOM watermarks are bypassed by calling the per-file JSON endpoints directly, and no concrete rate-limit/scrape thresholds are set.
  3. Promising "(never storing protected research bundles on disk)" on the Mac app link fails under default browser and `WKWebView` disk caches unless `Cache-Control: private, no-store` is set — and passing auth tokens in a Mac-to-browser URL leaks credentials to logs/history.
- **Minimal plain-English fix:** Serve all Library text/HTML/JSON/PDF through per-file auth (reserve `<= 60s` signed URLs strictly for large audio/video media); send `Cache-Control: private, no-store` on every gated response; embed a lightweight server-side per-account fingerprint/watermark into served text/JSON/PDF payloads; set concrete default rate limits (e.g., 30 distinct model files/hour, 60/day per account, alert at 15 distinct models in 10 minutes); and hand off Mac app users via normal browser login or a single-use 30-second POST-exchanged code (never a query-string bearer token).

## Claude Opus 5.5 — 75/100

*Model `anthropic/claude-opus-5.5` · 6960 output tokens · `tech=19/30 econ=13/20 align=17/20 ship=14/15 read=12/15`*

#### Summary judgement

This is a solid, grounded spec. It is honest about what exists today, including the unsandboxed iframe, `'unsafe-inline'`, whole-bundle unlock routes and unsigned `scp` publishing. It follows the locked decisions closely: existing leaderboard formula, daily UTC snapshot, 30 non-validator seats, 7-day grace with keep-until rank 35, Finance Hub for both tiers, API keys for validators only, `allowed_bases`, and a pre-launch rights check. It has five sensible milestones and all four ship gates.

The holes are in the details that decide whether the security and access model actually holds:
- **Goodbrief sandbox:** an opaque-origin iframe still sends credentialed requests to Task Node.
- **Signed publishing:** rollback semantics and key custody are underspecified.
- **Validator access:** a new "30-day ramp-in" is invented, and the old-chain binding sunset is open-ended. Both create free-access farming windows.
- **Seat count:** keep-until rank 35 makes "30 seats" ambiguous.
- **Launch ordering:** the content-rights review sits in M5, after the Top-30 tier opens in M4.
- **Missing defaults:** several knobs (rate limits, snapshot time, idle gap) are left as "TBD by implication", which the global standards forbid.

#### Rubric breakdown

- **Tech (19/30):**
  - The hardening direction is right.
  - The iframe/cookie threat model is incomplete.
  - Publish-key custody, rotation and rollback-versus-replay are undefined.
  - The public-brief anti-leak check is scrub-based instead of allowlist-based.
- **Econ (13/20):**
  - The ramp-in plus grace window lets throwaway validators collect around 44 days of free access.
  - There is no sybil handling for Top-30 seats, which are worth farming.
  - Seat-count semantics are ambiguous.
- **Align (17/20):**
  - Faithful to the locked decisions overall.
  - The added "initial 30-day ramp-in" is not in the Pillar 3 rule.
  - The Mac "hosted web view" wording risks contradicting the no-WebView decision.
- **Ship (14/20 → 14/15):**
  - Clear M1–M5 and all four gates.
  - Gate 2 is concrete.
  - The M4/M5 ordering versus the rights review is wrong.
- **Read (12/15):**
  - Mostly plain English.
  - Some dense, run-on bullets packed with identifiers (e.g. `visit_baseline_at`, `coverage_changes.json`) could be trimmed.

#### Execution holes

**H1**
- **Severity:** major
- **Where:** §3 Content isolation; Locked decisions → "sandbox full HTML Goodbrief inside `<iframe sandbox="allow-scripts">`"
- **Hole:** An opaque origin stops the Goodbrief scripts from *reading* Task Node data. It does not stop them from *sending* requests. If the Goodbrief is served from Task Node's own host, two failures follow:
  - Session cookies set with `SameSite=None` will still ride on requests the scripts send.
  - With Lax/Strict cookies, the brief's own authed sub-resources (charts, images) quietly fail to load.
- **Why it breaks execution:** LLM-generated or third-party HTML in a brief could fire a POST at a Task Node endpoint that lacks CSRF tokens. Examples: sending a message or changing settings, carried by the viewer's cookies. Alternatively, the brief simply renders broken. Neither failure appears in the gate-2 test as written.
- **Minimal plain-English fix:**
  - Serve the full Goodbrief from a separate cookieless host, such as `brief-sandbox.postfiat.org`, through a short-lived signed URL.
  - Set the frame's CSP to `connect-src 'none'` (or images-only).
  - Confirm Task Node session cookies are `SameSite=Strict` and that state-changing APIs require CSRF tokens.
  - Add to gate 2: "the brief cannot make any request to Task Node."

**H2**
- **Severity:** major
- **Where:** §3 Signed Publish API ("monotonic version … with signed rollback")
- **Hole:** "Signed rollback" contradicts "monotonic version" unless it is defined.
  - If rollback means accepting a lower version, any old signed manifest can be replayed to force stale or wrong models live.
  - Key custody, rotation and revocation are not specified. Today the research host and `pfrpc` each become a single point of compromise with no recovery path.
- **Why it breaks execution:** Two scenarios:
  - An attacker, or a buggy cron job, re-submits last week's signed manifest and the hub silently serves stale targets.
  - A compromised `pfrpc` key can publish arbitrary Goodbrief content until someone hand-edits the server.
- **Minimal plain-English fix:**
  - Define rollback as a new, higher-version manifest that points to earlier file hashes. Reject any version that is not greater than the current one, and reject `issuedAt` older than 1 hour.
  - Keep a server-side allowlist of one active key per index, rotated by an operator-signed config change.
  - Keep the private keys out of repos and environment files on shared hosts.
  - Document emergency revocation in the runbook.

**H3**
- **Severity:** major
- **Where:** Locked decisions → Validator qualification ("95%/30-day uptime after initial 30-day ramp-in"); "old-chain bindings sunset once migration completes"
- **Hole:**
  - The locked Pillar 3 rule is "active in registry and ≥95% over 25 of 30 days, with 14-day grace". The spec adds a 30-day ramp-in with no uptime requirement.
  - Old-chain bindings stay valid until an undated "migration completes". Today those bindings never expire.
- **Why it breaks execution:**
  - A new validator key gets 30 days of ramp-in plus 14 days of grace, about 44 days of full Finance Hub access, with near-zero uptime.
  - Old-chain binders keep access indefinitely with no liveness check, which contradicts "active liveness required".
- **Minimal plain-English fix:**
  - Drop the separate ramp-in. Read Pillar 3's access state verbatim; if Pillar 3 defines a first-30-days rule, Finance Hub inherits it rather than inventing one.
  - Grant only one validator-basis seat per operator, tied to the Pillar 6 identity (already mandatory for validator rewards).
  - Put a fixed sunset date on old-chain bindings. Until then, apply the same uptime rule to old-chain validators, using the old-chain UNL uptime feed.

**H4**
- **Severity:** major
- **Where:** M4 vs M5; Locked decisions → "pre-launch content-rights review"
- **Hole:** The Top-30 tier ships in M4, which the README places in Wave 2. The rights review for redistributing Goodbrief and market data sits in M5.
- **Why it breaks execution:** Licensed market data, options-implied moves or third-party content would go to roughly 30 non-validator accounts before anyone confirms the right to do so. That breaks the founder's "pre-launch" requirement.
- **Minimal plain-English fix:** Move the content-rights review into M4 as a blocking item. Until it passes, any item with unresolved rights keeps `allowed_bases = [validator]`.

**H5**
- **Severity:** major
- **Where:** §1 Daily Top-30 snapshot
- **Hole:** Finance Hub seats are now a valuable, leakable prize, and they are ranked over all non-blacklisted accounts. There is no duplicate-person check. The formula is locked and is not challenged here; the problem is that nothing stops one person from holding several of the 30 seats with farmed accounts.
- **Why it breaks execution:** One contributor with three high-ranking accounts takes three seats. That gives them extra scrape quotas, defeats per-user watermark attribution, and pushes out real contributors.
- **Minimal plain-English fix:** Before granting a new Top-30 seat, run the existing blacklist/abuse review plus a linked-account check (shared payout address, device, IP cluster). Allow one seat per person. Flagged accounts are skipped and the next rank fills the seat. Identity minting stays optional.

**H6**
- **Severity:** minor
- **Where:** §1 Snapshot + grace + hysteresis
- **Hole:** "30 seats" plus "keep-until rank 35" plus a 7-day grace can mean up to 35 or more concurrent holders. The spec never says which is true. `basis = grace` also doesn't say whose grace it is: the 7-day contributor grace or the 14-day validator grace.
- **Why it breaks execution:** Engineers will guess differently, and the behaviour tested at rank 31 won't match the docs.
- **Minimal plain-English fix:** Rewrite the rule plainly:
  - The top 30 by rank always qualify.
  - Current holders keep access while ranked 31–35.
  - Holders who fall below 35 get 7 days of grace.
  - Total seats may therefore exceed 30.

  Split `grace` into `validator_grace` and `top30_grace`, each with its own `graceEndsAt`.

**H7**
- **Severity:** minor
- **Where:** §2 Pillar 10 bridge ("build fails if public brief leaks model targets")
- **Hole:** A leak check that scans the public brief for target numbers is fragile. Rounded figures, percentages written as words, and "upside" phrased in prose all slip through. The founder requires zero leaked model numbers to non-members.
- **Why it breaks execution:** A non-member Pillar 10 interview could surface model targets through the public brief.
- **Minimal plain-English fix:**
  - Build `agent_brief_public.json` from an explicit allowlist of public fields: name, ticker, sector, public filings, coverage status. Model fields are never copied in.
  - Keep the numeric scan as a second check.
  - Add a gate-2 test with a seeded canary number.

**H8**
- **Severity:** minor
- **Where:** Engineering defaults (rate limits, snapshot time, idle gap, stale thresholds)
- **Hole:** "Fixed UTC time", "rate limits", "idle gap", "unusual-scrape alerts" and the failed-snapshot staleness limit have no values. The global standard requires concrete defaults.
- **Why it breaks execution:**
  - Rate limits cannot be tested in gate 2.
  - If snapshot jobs fail for days, an indefinitely "last good" snapshot keeps dropped users in and blocks new ones.
- **Minimal plain-English fix:** State defaults, for example:
  - Snapshot at 00:15 UTC.
  - At most 300 model-file fetches per account per day, alert at 150.
  - Idle gap of 2 hours.
  - If the snapshot is older than 48 hours, page operators and freeze grace countdowns.

**H9**
- **Severity:** minor
- **Where:** Locked decisions → Notifications & local Mac app ("links cleanly to the hosted web view")
- **Hole:** The phrasing is ambiguous. It could mean an embedded WKWebView inside `tasknode-apple`, which the locked decision rules out, or opening the hosted page in the system browser.
- **Why it breaks execution:** An engineer could embed a WebView that shares cookies with the app and pull Finance Hub into the native app's attack surface, against the founder's decision.
- **Minimal plain-English fix:** Say "opens Finance Hub in the user's default browser; the Mac app embeds no WebView and stores no research files" until a native SwiftUI view ships.
