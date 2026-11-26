# Changelog — Pillar 9 — Finance Hub User Spec

How each reviewer problem was handled, round by round. The spec itself is [here](../pillars/09-finance-hub.md); the reviews are under [feedback](../feedback/README.md).

## Round 1 review → revision 1

Reviews: [round 1](../feedback/round-1/09-finance-hub.md).

| Reviewer item | Change |
|---|---|
| Gemini P1, Opus P1 | R3, open question 1, acceptance 2. Partly disputed: L1 binding already exists, with ML-DSA, a registry check and one identity per account. The gaps are that old-chain bindings also count and that liveness is never re-checked. |
| Gemini P2 | Rendering decision, R7, R8. Partly disputed: the iframe is same-origin with a working bridge, and files are decrypted on the server on every request. |
| Gemini P3, Opus P3 | Leaderboard facts verified (all-time totals, public accounts only, ties end on handle). R4, R5, Economics, open questions 2–4. |
| Gemini P4 | R18–R20. Partly disputed: the hub already republishes every 15 minutes, movers refresh live, and a failed Goodbrief fetch never fails the build. |
| Gemini P5 | Confirmed: the tier is hard-coded, and so is the revocation query. R2. |
| Gemini P6 | R11–R13. Partly disputed: `updates.json` already has before → after values, and *Just reported* exists. The real gaps are pre-print estimates and the coverage diff. |
| Gemini P7, Opus P7 | R17, story step 4, and the Ask-the-agent link to Task Node chat. Migration 158 is now described accurately. How users link to coverage is decided in Pillar 10. |
| Gemini P8, Opus P11 | R24, open question 8. |
| Gemini P9, Opus P4 | R7–R10, open questions 5–6. |
| Gemini P10, Opus P12 | R22, acceptance 10, open question 7. Verified: there is no email sender. |
| Gemini P11 | R14, R15, acceptance 5–6. |
| Gemini P12, Opus P14 | R21, R23. Disputed: the plan's date (2026-11-11, merged in #384) is correct. The production state is now stated. |
| Opus P2 | R7, R8, acceptance 3. The leak is the `unlock` key path, not the in-app view. |
| Opus P5 | R11. *Just reported* already exists; it now sits on Today. |
| Opus P6 | R11, R14–R16. Usage metrics added without invented targets. |
| Opus P8 | Confirmed: nothing is signed today. R18, acceptance 7. |
| Opus P9 | Clarified: the Goodbrief ships inside `ga-models`. R19 makes it its own publication. |
| Opus P10 | R5, open question 4. |
| Opus P13 | Existing safeguards listed. Operator alerting added (R20). |
| Opus P15 | Acceptance criteria now cover access, revocation, publishing, Today, retention, the Pillar 10 inputs and metrics. |

## Round 2 review → revision 2

Reviews: [round 2](../feedback/round-2/09-finance-hub.md).

| Item | Change |
|---|---|
| Gemini P1 | Verified: the viewer iframe has no `sandbox`, the bundle CSP allows `'unsafe-inline'` scripts, and the full HTML Goodbrief (which carries its own scripts) loads on Task Node's origin via *Open full brief* and the fallback frame. Partly disputed: session cookies are `HttpOnly`, notes go through an escaping renderer, and the native Goodbrief summary renders text only. New R8 (no inline script, Goodbrief served with a `sandbox allow-scripts` CSP and frame, escaped-only rendering for LLM and contributor text), acceptance 5, Rendering decision. |
| Gemini P2 | R12 reads the Goodbrief from its own publication through the same gated route (from `ga-models` until R21 lands); render gate uses a fixture brief; R21 keeps the existing size/doctype/date checks; diagram; acceptance 10. |
| Gemini P3 | R19: the research host pulls accepted insights with a read-only service token, matching migration 158's design and Pillar 10 R16. Reverse arrow added to the diagram. Kept in the bundle (not served live) for consistency with Pillar 10. |
| Gemini P4 | R20: per-file staged upload plus a signed commit whose manifest lists path, SHA-256 and size; keys pinned per index; two keys during rotation; immediate revocation; acceptance 9. |
| Gemini P5, Opus P4 | R16 redefines visits with `visit_baseline_at`, `last_active_at` and a configured idle gap; story step 5; acceptance 7 covers refresh, tabs and devices. |
| Gemini P6 | Verified: `…/content` serves the whole-bundle ciphertext with no session check, and the publisher writes that blob. R7 returns 404 and stops writing the blob after R23. R1 and R9 add blacklist and suspension overrides. Movers verified as public Hyperliquid prices (CSP `connect-src`), so no gated endpoint is needed. R10 adds a distinct-company anomaly alert. |
| Gemini P7 | R1 adds per-publication allowed bases, so open questions 5 and 8 become metadata changes; acceptance 4. Overlap: Economics and open question 2 (all accounts or non-validators). Cost arbitrage: disputed, Pillar 3 admission is permissioned and ratified (noted in Economics). Open question 1 gains a ramp-in option (iv). |
| Gemini P8 | R25: per Pillar 5 the native app has no web view, so Finance Hub is web-only and opens hosted in the system browser; chat stays hosted; any later web view loads the hosted origin top-level. |
| Gemini P9 | R2 split into two deploy steps. R3 follows Pillar 3: a stale feed never revokes. R4 keeps the last good snapshot on failure. R22 alerts on snapshot and validator-feed failures. |
| Gemini P10 | R19 splits the agent brief into members and public files with a build check; R13 makes fields follow asset class; R19 defines the `ask-agent` bridge message. Contributor attribution stays in Pillar 10 (R21–R22 there). |
| Gemini P11 | R26: large media use short-lived signed URLs behind the same check; text and PDF stay on the per-file path. |
| Opus P1 | R11: rights cover all outside access, including today's old-chain validators; scrubbing is not a rights measure; open question 5 options and the "name the source" question, designed to work under each option via allowed bases. |
| Opus P2 | Open question 1 (designed to work under each option via R3's network list). Trade-off against this pillar's purpose stated; no answer picked. `l1-devnet` verified as the only L1 network value; R3 makes networks configurable. |
| Opus P3 | Disputed in part: Pillar 3 makes admission permissioned and ratified, so the validator path is not cheap while that holds. Economics notes the dependency. |
| Opus P5 | R13 adds per-section caps with *see all*, empty, stale and non-trading-day states, asset-class fields and phone width. Acceptance 6 adds founder sign-off on laptop and phone. No targets invented. |
| Opus P6 | Open question 8 adds validator Deep Research credit as an option; R26 Library with per-item bases. Verified: research API keys are charged at cost. Contents stay a founder decision. |
| Opus P7 | Economics adds supply cost (unpriced), path comparison and overlap. No figures invented. |
| Opus P8 | R5: one grace mechanism for both bases; validator grace is Pillar 3's *lapsed* window; story step 6; open question 4 asks per-basis lengths. |
| Opus P9 | R10 uses the existing `server/rate-limit.js`, operator-set limits tuned from logs, and an anomaly alert. Watermarks and screenshots stay in open question 6, which also asks who may suspend and how appeals work. |
| Opus P10 | R20 adds a documented manual publish path and an optional offline backup key. Recovery-time target not set (no durations). |
| Opus P11 | New *Delivery order* subsection, Step 1–4. |
| Opus P12 | R4 explains which pillars read the snapshot (1, 4, 6, 8: governance callers, badges, identity gating, credentials). Pillar 6 added to Dependencies and open question 7. |
| Opus P13 | Verified and documented: alignment is the daily airdrop model's 0–100 `retention_value_score` averaged over 30 days; rewards are PFT divided by 25,000. Open question 2 option (c) mentions capping alignment. No default picked. |
| Opus P14 | R4: configured UTC snapshot time, last good snapshot kept on failure with an alert, snapshot time shown (R1, R6, story step 1). |
| Opus P15 | Linked PR #384 and the plan that records the 221,307-byte replay. Revision notes were already outside the spec. |
| Cross-pillar (Pillar 10) | R19: the coverage manifest entry notes that Pillar 10 R1 adds `interview_mode`, `interview_enabled` and `budget_priority`. |
| Cross-pillar (Pillar 10) | R19: **Ask the agent** opens Pillar 10's screened interview mode (R6, R9), with the same eligibility, attestation and screening. |
| Cross-pillar (Pillar 3) | Verified in `l1-validator-uptime.js` and migration 156: the binding row now states the participation rule as a mean of 95% or more over at least 25 observed days of the last 30, and that an account may hold several bindings (unique per validator id and per key), any of which grants access. Neither old wording remained in this revision's spec. |
