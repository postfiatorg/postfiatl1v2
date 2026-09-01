# Validator Identity Packet

## Packet Status

**SHADOW_ONLY**  
Research timestamp: **2026-09-01T20:08:29Z**  
This packet contains external public-identity evidence, not XRP Ledger consensus data.

## Validator Coordinates

- Network: **XRP Ledger mainnet**
- Validator master public key: `nHUbgDd63HiuP68VRWazKwZRzS61N37K3NbfQaZLhSQ24LGGmjtn`
- Claimed domain: **null**
- Frozen domain-verification status: **null**
- Frozen upstream list publishers containing the key: **ripple**, **xrpl_foundation**
- Frozen upstream metadata source: [XRPSCAN validator API](https://api.xrpscan.com/api/v1/validator)

## Claimed Domain and Official URLs

- Claimed domain: **None supplied (`null`)**.
- Registry-associated candidate validator hostname: **`xrp-col.anu.edu.au` — not independently verified**. Bithomp’s public validator registry associates the exact master key with this hostname, but no network-specific domain attestation was successfully retrieved during this research. [Bithomp validator registry](https://bithomp.com/validators?amendment=fixAMMv1_3)
- Most likely institutional domain: **`anu.edu.au`**. The candidate hostname is beneath the domain used by The Australian National University’s official website. [Official ANU website](https://www.anu.edu.au/)
- Official institutional URL: [https://www.anu.edu.au/](https://www.anu.edu.au/)
- Candidate validator URL: `https://xrp-col.anu.edu.au/` — inaccessible during this research and therefore not confirmed as an official validator page.

## Public Identity

- Most likely canonical public entity: **The Australian National University**
- Entity type: **Australian statutory body corporate and research university**. The Australian National University Act continues the University as a body corporate. [Federal Register of Legislation](https://www.legislation.gov.au/C2004A04206/2014-07-01/2014-07-01/text/1/epub/OEBPS/document_1/document_1.html)
- Supported current aliases: **ANU** and **Australian National University**, both used by the University itself. [ANU facts](https://www.anu.edu.au/about/facts-about-anu)
- Historical aliases: **Not established**.
- Identity connection: the exact validator key is associated by a public validator registry with `xrp-col.anu.edu.au`; that hostname sits beneath ANU’s official institutional domain. [Bithomp validator registry](https://bithomp.com/validators?amendment=fixAMMv1_3) The key also appears in XRPL Foundation configuration guidance, but that list does not name its operator. [XRPL Foundation guidance](https://github.com/XRPLF/rippled/discussions/5463)
- Conclusion: **ANU is the most likely entity identity, with medium confidence; current control of the validator key is not independently established.**

## Business Summary

The most likely associated entity is The Australian National University (ANU), an Australian statutory body corporate and research university established under Commonwealth legislation. Its principal base is Canberra, Australian Capital Territory, with additional Australian campuses in the ACT, New South Wales and Northern Territory. ANU provides undergraduate and postgraduate education, conducts multidisciplinary research, and undertakes policy, industry and community engagement for students, researchers, governments, institutional partners and the wider public. Its educational and research reach is international, including students from more than 100 countries, while its physical operating footprint is principally Australian. With 8,836 staff and 23,659 students reported in 2025, it is a very large institution under this packet’s rubric. A public validator registry associates the subject key with an ANU subdomain, but no accessible ANU statement or independently checked XRPL domain attestation establishes that the University currently controls the key.

## Public X Handle

**[@ourANU](https://x.com/ourANU)**

Verification basis: ANU’s official Media and Publications page identifies `@ourANU` as the University’s X account. [ANU Media and Publications](https://services.anu.edu.au/business-units/anu-regulatory-affairs-and-engagement/media-and-publications)

No validator-specific X account was established.

## Region of Incorporation and Operations

- Incorporation jurisdiction: **Commonwealth of Australia — high confidence.** The University is continued as a body corporate by Commonwealth legislation. [Australian National University Act 1991](https://www.legislation.gov.au/C2004A04206/2014-07-01/2014-07-01/text/1/epub/OEBPS/document_1/document_1.html)
- Principal operating base: **Canberra, Australian Capital Territory, Australia — high confidence.** ANU identifies its primary location as Acton, Canberra. [ANU About page](https://www.anu.edu.au/about)
- Other physical operating regions: **Australian Capital Territory, New South Wales and Northern Territory — high confidence.** ANU reports campuses in these jurisdictions. [ANU About page](https://www.anu.edu.au/about)
- Broader reach: **International educational and research reach — high confidence.** ANU reports students from more than 100 countries and international research and policy engagement. [ANU Study](https://study.anu.edu.au/) [ANU Annual Report 2025](https://www.anu.edu.au/about/strategic-planning/annual-report-2025)

## Activities

ANU’s principal activities are undergraduate and postgraduate education, multidisciplinary research, policy engagement, and collaboration with government, industry, research institutions and communities. [ANU About page](https://www.anu.edu.au/about) [ANU strategic initiatives](https://www.anu.edu.au/about/strategic-initiatives)

ANU also has a documented XRP Ledger research footprint: its official research portal describes the Evernode project as bringing layer-two smart contracts to the XRP Ledger, and its Law School has reported XRPL grant-funded legal-technology work. [ANU Evernode project](https://researchportalplus.anu.edu.au/en/projects/evernode/) [ANU Law School grant report](https://law.anu.edu.au/news-and-events/news/anu-scholar-and-students-awarded-funding-legal-tech-projects)

The observable validator relationship is limited to a public registry associating the exact key with an ANU subdomain. [Bithomp validator registry](https://bithomp.com/validators?amendment=fixAMMv1_3) ANU’s XRPL research activities do not, by themselves, prove that the University operates or controls this validator, and technical operation is not inferred solely from validator-list membership.

## Estimated Public-Profile Size

- Tier: **Very large**
- Evidence: ANU’s official 2025 diversity report gives a total staff headcount of **8,836**, including casual staff, and a total student headcount of **23,659**. [ANU 2025 Diversity, Equity and Inclusion Strategies Progress Report](https://services.anu.edu.au/files/2026-02/FINAL%202025%20Diversity%20Equity%20and%20Inclusion%20Strategies%20Progress%20Report_18Feb26.pdf)
- Confidence: **High**
- Headcount established: **Yes**, as an institutional headcount reported by ANU; it is not presented as full-time-equivalent employment.

## Evidence

1. [Validators — Bithomp](https://bithomp.com/validators?amendment=fixAMMv1_3) — Public validator registry; accessed **2026-09-01 UTC**. Associates the exact validator key with `xrp-col.anu.edu.au` and reports the entry on a current UNL. It does not display a successfully checked domain-attestation basis for this entry.
2. [Configuration Guidance for Using the New UNL — XRPLF/rippled](https://github.com/XRPLF/rippled/discussions/5463) — Primary XRPL Foundation technical guidance; accessed **2026-09-01 UTC**. Contains the exact validator key in the published configuration material but does not identify its operator.
3. [Unique Node List — XRP Ledger](https://xrpl.org/docs/concepts/consensus-protocol/unl) — Primary network documentation; accessed **2026-09-01 UTC**. Explains recommended validator lists and identifies Ripple and the XRP Ledger Foundation as default list publishers; list inclusion is a trust-list fact rather than proof of operator identity.
4. [About ANU — The Australian National University](https://www.anu.edu.au/about) — Official institutional source; accessed **2026-09-01 UTC**. Supports the University’s identity, research and education functions, Canberra base, governance, and additional campuses in the ACT, NSW and NT.
5. [Australian National University Act 1991](https://www.legislation.gov.au/C2004A04206/2014-07-01/2014-07-01/text/1/epub/OEBPS/document_1/document_1.html) — Commonwealth legislative register; accessed **2026-09-01 UTC**. Supports the legal name, Commonwealth statutory basis and body-corporate status.
6. [Media and Publications — ANU](https://services.anu.edu.au/business-units/anu-regulatory-affairs-and-engagement/media-and-publications) — Official institutional source; accessed **2026-09-01 UTC**. Identifies `@ourANU` as the University’s X account.
7. [2025 Diversity, Equity and Inclusion Strategies Progress Report — ANU](https://services.anu.edu.au/files/2026-02/FINAL%202025%20Diversity%20Equity%20and%20Inclusion%20Strategies%20Progress%20Report_18Feb26.pdf) — Official institutional report; accessed **2026-09-01 UTC**. Reports 8,836 staff and 23,659 students for 2025.
8. [Facts about ANU](https://www.anu.edu.au/about/facts-about-anu) — Official institutional source; accessed **2026-09-01 UTC**. Supports the ANU abbreviation, research and education mission, Canberra location, and international student reach.
9. [Evernode — ANU Research Portal](https://researchportalplus.anu.edu.au/en/projects/evernode/) — Official institutional research record; accessed **2026-09-01 UTC**. Documents an ANU research project involving layer-two smart contracts for the XRP Ledger.
10. [xrp-ledger.toml File — XRP Ledger](https://xrpl.org/docs/references/xrp-ledger-toml) — Primary network documentation; accessed **2026-09-01 UTC**. Defines the HTTPS-hosted attestation mechanism used for validator domain verification.
11. [`xrp-col.anu.edu.au` candidate attestation endpoint](https://xrp-col.anu.edu.au/.well-known/xrp-ledger.toml) — Candidate network-attestation endpoint; access attempted **2026-09-01 UTC**. No retrievable content was obtained, so it supports no affirmative verification claim.
12. [Validator Info — XRPSCAN API documentation](https://docs.xrpscan.com/api-documentation/validator/validator-info) — Upstream API documentation; accessed **2026-09-01 UTC**. Documents the validator-key lookup endpoint used by the supplied metadata source; the subject-specific API response was not accessible during this research.

## Uncertainty and Conflicts

- The frozen input supplies no claimed domain and no domain-verification result; both remain `null`.
- `xrp-col.anu.edu.au` was discovered through a public validator registry, not supplied by the frozen collector.
- No accessible ANU page explicitly names the validator key or states that ANU currently operates it.
- The candidate `xrp-ledger.toml` attestation could not be retrieved, so no network-specific domain verification was independently completed.
- The exact key’s appearance in Ripple and XRPL Foundation lists establishes publisher inclusion, not ownership, operation or control.
- The `xrp-col` label was not expanded to “College of Law” or treated as an alias because no checked source establishes that interpretation.
- ANU’s documented XRPL grants and research provide contextual consistency but do not prove validator-key control.
- Official ANU sources present differing student totals: the 2025 diversity report reports 23,659 students, while the current facts page lists 10,252 undergraduate and 7,128 postgraduate students. [ANU diversity report](https://services.anu.edu.au/files/2026-02/FINAL%202025%20Diversity%20Equity%20and%20Inclusion%20Strategies%20Progress%20Report_18Feb26.pdf) [ANU facts](https://www.anu.edu.au/about/facts-about-anu) The difference may reflect dates or population definitions, but it was not resolved; the size tier is unaffected.
- Validator personnel, infrastructure ownership, hosting arrangements and present operational control are **not established**.
- No validator-specific social account is established.

## Machine-Readable Summary

```json
{
  "validator_id": "nHUbgDd63HiuP68VRWazKwZRzS61N37K3NbfQaZLhSQ24LGGmjtn",
  "network": "XRP Ledger mainnet",
  "claimed_domain": null,
  "domain_verification_status": null,
  "canonical_entity": "The Australian National University",
  "entity_type": "Australian statutory body corporate and research university",
  "aliases": [
    "ANU",
    "Australian National University"
  ],
  "official_urls": [
    "https://www.anu.edu.au/"
  ],
  "business_summary": "The most likely associated entity is The Australian National University (ANU), an Australian statutory body corporate and research university established under Commonwealth legislation. Its principal base is Canberra, Australian Capital Territory, with additional Australian campuses in the ACT, New South Wales and Northern Territory. ANU provides undergraduate and postgraduate education, conducts multidisciplinary research, and undertakes policy, industry and community engagement for students, researchers, governments, institutional partners and the wider public. Its educational and research reach is international, including students from more than 100 countries, while its physical operating footprint is principally Australian. With 8,836 staff and 23,659 students reported in 2025, it is a very large institution under this packet’s rubric. A public validator registry associates the subject key with an ANU subdomain, but no accessible ANU statement or independently checked XRPL domain attestation establishes that the University currently controls the key.",
  "x_handle": "@ourANU",
  "incorporation_region": "Commonwealth of Australia",
  "operating_regions": [
    "Canberra, Australian Capital Territory, Australia",
    "Australian Capital Territory, Australia",
    "New South Wales, Australia",
    "Northern Territory, Australia",
    "International educational and research reach"
  ],
  "profile_size_tier": "Very large",
  "profile_size_confidence": "high",
  "identity_confidence": "medium",
  "unresolved_fields": [
    "Current control of the validator master public key",
    "Network-specific domain verification for xrp-col.anu.edu.au",
    "Identity of the validator's technical operator",
    "Validator infrastructure ownership and hosting arrangements",
    "Validator-specific public social account",
    "Reason for differing official ANU student totals"
  ],
  "evidence_urls": [
    "https://bithomp.com/validators?amendment=fixAMMv1_3",
    "https://github.com/XRPLF/rippled/discussions/5463",
    "https://xrpl.org/docs/concepts/consensus-protocol/unl",
    "https://www.anu.edu.au/",
    "https://www.anu.edu.au/about",
    "https://www.legislation.gov.au/C2004A04206/2014-07-01/2014-07-01/text/1/epub/OEBPS/document_1/document_1.html",
    "https://services.anu.edu.au/business-units/anu-regulatory-affairs-and-engagement/media-and-publications",
    "https://services.anu.edu.au/files/2026-02/FINAL%202025%20Diversity%20Equity%20and%20Inclusion%20Strategies%20Progress%20Report_18Feb26.pdf",
    "https://www.anu.edu.au/about/facts-about-anu",
    "https://researchportalplus.anu.edu.au/en/projects/evernode/",
    "https://law.anu.edu.au/news-and-events/news/anu-scholar-and-students-awarded-funding-legal-tech-projects",
    "https://xrpl.org/docs/references/xrp-ledger-toml",
    "https://xrp-col.anu.edu.au/.well-known/xrp-ledger.toml",
    "https://docs.xrpscan.com/api-documentation/validator/validator-info"
  ]
}
```