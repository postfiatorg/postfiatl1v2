## 5. Reputation rubric — 0–100 in 5-point bands

Scores represent **value to the network**. A band is earned only when packet evidence and explicitly flagged weights-prior support the applicable description. When evidence conflicts within one dimension, the lower supported band applies.

The displayed range labels below are retained exactly as pre-registered. Machine evaluation resolves shared printed endpoints using the §4 band formula: B00 covers integer scores 0–4, B05 covers 5–9, and so on, while B95 covers 95–100.

`sanctions_safety` is inverted risk: 100 means no exposure.

| Band | Organization prestige | Censorship resistance | Sanctions risk (safety) |
| --- | --- | --- | --- |
| 0–5 | Fabricated or deceptive identity; name-squat of a real brand | Entity exists to enforce content/transaction blocking | On SDN/comprehensive sanctions list, or evidence of evasion services |
| 5–10 | Unverifiable shell; no independent footprint predating this network | Operates under direct state direction with takedown history | Majority-owned by a listed entity |
| 10–15 | Anonymous operator, no organization, no track record | Contractually bound to censor (licensing regime with enforced blocking) | Registered in comprehensively sanctioned jurisdiction |
| 15–20 | Pseudonymous but consistent identity across this network only | Single jurisdiction with routine compelled takedowns, no resistance record | Material business with listed counterparties |
| 20–25 | Named individual, verifiable person, no institution | High-censorship jurisdiction, compliance posture unknown | Operates in secondary-sanctions exposure sectors |
| 25–30 | Small informal collective with public repos/output | Single high-pressure jurisdiction but no compliance history either way | Unresolved sanctions-adjacent ownership questions |
| 30–35 | Registered small company, thin public record | Discloses compliance with local blocking orders transparently | Minor indirect exposure via investors/customers |
| 35–40 | Established small company, verifiable customers or products | Single low-pressure jurisdiction, no redundancy | Fully disclosed structure, one flagged historical association |
| 40–45 | Recognized niche operator known inside the industry | Some infrastructure redundancy, one legal jurisdiction | Clean structure, jurisdiction with weak enforcement transparency |
| 45–50 | Mid-size firm with multi-year public operating history | Two-jurisdiction presence, untested under pressure | Clean structure, standard KYC-regulated jurisdiction |
| 50–55 | Nationally known company or institution | Public commitment to neutrality, no test cases | Clean, with routine regulatory interactions on record |
| 55–60 | National institution with independent press coverage | Declined at least one informal pressure request (documented) | Clean, periodic third-party attestation |
| 60–65 | Multi-national operating footprint, audited financials | Multi-jurisdiction infra able to survive one country's exit | Clean, publicly audited ownership chain |
| 65–70 | Sector leader in one region; regulators/press cite it | Track record of contesting overbroad orders in court or public | Clean and demonstrably screens its own counterparties |
| 70–75 | Household name in its sector; decade-plus history | Operates lawfully while resisting extra-legal pressure; transparency reports | Clean, long history under strict regulators with zero findings |
| 75–80 | Globally recognized institution (top exchange, major university, global media) | Transparency reports plus warrant-canary-class practices, multi-year | Clean, gold-standard compliance program, public attestations |
| 80–85 | Global top tier; systemically relied upon in its sector | Survived documented state-level pressure without capitulating | Clean at scale across many strict jurisdictions, years of audits |
| 85–90 | Century-class or sovereign-grade reputation (major university, central-bank-adjacent, global standards body) | Structurally censorship-proof: distributed governance, no single compellable point | Effectively un-sanctionable structure; sovereign-neutral standing |
| 90–95 | Reputation itself is global infrastructure; impersonation instantly detectable worldwide | Proven multi-decade resistance across regimes | Multi-decade spotless record, universally recognized neutrality |
| 95–100 | Reserved: institutions whose failure would be a world-historical event | Reserved: censorship-resistance is the institution's founding function with a proven record | Reserved: no plausible sanctions pathway exists |

Calibration rules embedded in the prompt:

- **Obscurity ≠ fabrication.** A real-but-obscure operator with a thin footprint floors at 25–30 prestige, never in the 0–15 fabrication bands. Mere lack of press coverage is not positive evidence of fabrication. The 10–15 anonymous-operator description applies only when the packet affirmatively establishes that there is no organization and no track record; it is not inferred from missing coverage.
- **Absence of sanctions evidence ≠ safety ceiling.** Unknown structure caps `sanctions_safety` at 45–50; high bands require affirmative evidence.
- **Prestige never rescues sanctions.** The deterministic gate enforces this independently of the prompt.

