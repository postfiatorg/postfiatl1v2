# Validator Identity Packet

## Packet Status

**SHADOW_ONLY** — Researched at **2026-09-01T20:12:20Z**. This packet contains external public-identity evidence, not XRP Ledger consensus data.

## Validator Coordinates

- **Network:** XRP Ledger mainnet
- **Validator master public key:** `nHUrUNXCy4DgPPNABX9C6mUctpoq7CwgLKAUxjw6zYtTfiqsj1ew`
- **Claimed domain:** `xrp-validator.interledger.org`
- **Frozen domain-verification status:** `null` — not independently established in the frozen input
- **Validator-list publishers:** `ripple`, `xrpl_foundation` — supplied frozen metadata
- **Metadata source:** [XRPSCAN validator API](https://api.xrpscan.com/api/v1/validator). The source endpoint was supplied, but its exact per-key response was not retrievable through the research client.

## Claimed Domain and Official URLs

**Conclusion:** `xrp-validator.interledger.org` is a claimed validator hostname under `interledger.org`, the official website domain used by the Interledger Foundation. A current public validator registry maps the exact master key to this hostname and labels its TOML domain association verified; however, this research did not retrieve and independently validate the underlying network-specific attestation. The packet therefore preserves the frozen status as `null` and labels the hostname **claimed, with third-party verification evidence but not independently verified here**. [Bithomp validator registry](https://bithomp.com/validators?amendment=PermissionDelegation), [Interledger Foundation official site](https://interledger.org/about-us)

Supported official URLs:

- [https://interledger.org/](https://interledger.org/)
- [https://github.com/interledger](https://github.com/interledger)
- [https://x.com/interledger](https://x.com/interledger)

## Public Identity

- **Canonical public entity:** **Interledger Foundation Inc.**
- **Entity type:** California-incorporated nonprofit corporation and U.S. 501(c)(3) charitable foundation. The Florida Division of Corporations records it as an active foreign not-for-profit corporation whose home state is California, while public IRS-derived filings identify EIN 84-2364885 and federal tax-exempt status. [Florida Division of Corporations](https://search.sunbiz.org/Inquiry/CorporationSearch/SearchResultDetail?aggregateId=fornp-f20000003146-33c5dcbc-6240-4589-b760-371354f5e4a4&directionType=CurrentList&inquirytype=OfficerRegisteredAgentName&listNameOrder=SHINAKATLLC+L200003840321&searchNameOrder=SHINALEX+F200000031469&searchTerm=Shina+Investment+Llc), [Nonprofit Explorer](https://projects.propublica.org/nonprofits/organizations/842364885)
- **Supported aliases:** **Interledger Foundation** and **ILF**, both used by the organization. [Official About Us page](https://interledger.org/about-us)
- **Identity connection:** The exact validator key is publicly paired with `xrp-validator.interledger.org`; the parent domain is the Foundation’s official domain. This supports Interledger Foundation Inc. as the most likely public identity, but the connection remains indirect because no first-party Foundation page found in this research names the validator key or expressly claims current operation of it. [Bithomp validator registry](https://bithomp.com/validators?amendment=PermissionDelegation), [official Interledger site](https://interledger.org/)

## Business Summary

Interledger Foundation Inc. is a United States charitable nonprofit and grantmaking foundation, incorporated in California and publicly associated with operating addresses in California and Florida. The organization stewards the Interledger Protocol, Open Payments, Rafiki, and related open standards and software, while funding research, education, community-finance, and digital-inclusion initiatives. Its stakeholders include regulated financial-service providers, community banks and credit unions, fintech developers, universities, grantees, governments, and underserved communities. Programs and partnerships are international, with activity documented across the United States and multiple global regions. Public filing-derived data places the organization in a small workforce tier, although its financial resources and sector reach are substantial relative to staff size. The validator key is publicly associated with the claimed xrp-validator.interledger.org hostname, but this packet does not independently establish that the Foundation currently controls or operates the validator.

## Public X Handle

**@interledger** — established through the Foundation’s official launch announcement, which links directly to `twitter.com/interledger`; the corresponding current URL is `https://x.com/interledger`. [Official Interledger Foundation announcement](https://interledger.org/news/interledger-foundation-launches-build-more-equitable-and-creative-opportunities-web), [linked profile](https://twitter.com/interledger)

## Region of Incorporation and Operations

- **Incorporation jurisdiction:** **California, United States — high confidence.** Florida’s official registry identifies Interledger Foundation Inc. as a foreign not-for-profit corporation with home state `CA`. [Florida Division of Corporations](https://search.sunbiz.org/Inquiry/CorporationSearch/SearchResultDetail?aggregateId=fornp-f20000003146-33c5dcbc-6240-4589-b760-371354f5e4a4&directionType=CurrentList&inquirytype=OfficerRegisteredAgentName&listNameOrder=SHINAKATLLC+L200003840321&searchNameOrder=SHINALEX+F200000031469&searchTerm=Shina+Investment+Llc)
- **Principal operating regions:** **United States and globally distributed programs — medium confidence.** A June 2026 first-party regulatory submission describes the Foundation as based in California, while its current Florida filing gives Tampa as its principal and mailing location. The Foundation describes a global workforce and worldwide partners and programs. These records may represent different legal, administrative, and distributed operating functions. [FDIC-filed Foundation submission](https://www.fdic.gov/federal-register-publications/interledger-foundation-briana-marbury-rin-3064-ag19.pdf), [Florida Division of Corporations](https://search.sunbiz.org/Inquiry/CorporationSearch/SearchResultDetail?aggregateId=fornp-f20000003146-33c5dcbc-6240-4589-b760-371354f5e4a4&directionType=CurrentList&inquirytype=OfficerRegisteredAgentName&listNameOrder=SHINAKATLLC+L200003840321&searchNameOrder=SHINALEX+F200000031469&searchTerm=Shina+Investment+Llc), [official team page](https://interledger.org/team)

## Activities

The Foundation stewards open-payment infrastructure, including the Interledger Protocol, Open Payments, and Rafiki; supports their adoption; funds research, education, technical development, and community-finance projects; and works with financial-service providers, governments, developers, universities, and underserved communities. In August 2026 it announced a shift away from directly operating consumer financial products toward open infrastructure and grantmaking. [Official About Us page](https://interledger.org/about-us), [official strategic-shift announcement](https://interledger.org/news/strategic-shift-interledger-foundation), [official GitHub organization](https://github.com/interledger)

The exact validator key is publicly listed with the claimed hostname and appears on recommended validator lists. That evidence supports a public validator/domain association, but list membership alone does not prove that the Foundation currently operates the server, possesses the master key, or directs its votes. [Bithomp validator registry](https://bithomp.com/validators?amendment=PermissionDelegation), [XRPL validator guidance](https://xrpl.org/docs/infrastructure/configuration/server-modes/run-xrpld-as-a-validator)

## Estimated Public-Profile Size

**Small** — filing-derived public data reports **24 employees** for the 2024 snapshot, within the rubric’s approximate 11–50 range; the official site also presents an established leadership team and globally distributed activity. **Confidence: medium.** Headcount is established for that historical filing-derived snapshot, but current September 2026 headcount is not established, particularly following the Foundation’s August 2026 strategic shift and associated personnel changes. [Cause IQ filing-derived profile](https://www.causeiq.com/organizations/interledger-foundation%2C842364885/), [official team page](https://interledger.org/team), [official strategic-shift announcement](https://interledger.org/news/strategic-shift-interledger-foundation)

## Evidence

1. [Bithomp — Validators | XRP Ledger](https://bithomp.com/validators?amendment=PermissionDelegation) — **Public validator registry/explorer**; accessed **2026-09-01**. Maps the exact master key to `xrp-validator.interledger.org`, reports recommended-list membership, and labels the domain association “Verified domain (TOML file).”
2. [Interledger Foundation — About Us](https://interledger.org/about-us) — **First-party institutional page**; accessed **2026-09-01**. Supports the names “Interledger Foundation” and “ILF,” nonprofit status, global mission, stakeholders, grantmaking, and stewardship activities.
3. [Florida Division of Corporations — Interledger Foundation Inc.](https://search.sunbiz.org/Inquiry/CorporationSearch/SearchResultDetail?aggregateId=fornp-f20000003146-33c5dcbc-6240-4589-b760-371354f5e4a4&directionType=CurrentList&inquirytype=OfficerRegisteredAgentName&listNameOrder=SHINAKATLLC+L200003840321&searchNameOrder=SHINALEX+F200000031469&searchTerm=Shina+Investment+Llc) — **Official company registry**; accessed **2026-09-01**. Supports the legal name, active foreign not-for-profit status, California home state, Florida administrative location, and EIN.
4. [ProPublica Nonprofit Explorer — Interledger Foundation](https://projects.propublica.org/nonprofits/organizations/842364885) — **IRS-filing repository**; accessed **2026-09-01**. Supports EIN 84-2364885, federal 501(c)(3) status, tax filings, organizational scale, and financial footprint.
5. [Interledger Foundation submission to the FDIC](https://www.fdic.gov/federal-register-publications/interledger-foundation-briana-marbury-rin-3064-ag19.pdf) — **First-party regulatory submission hosted by a U.S. regulator**; accessed **2026-09-01**. Describes the Foundation as a California-based nonprofit grantmaking organization and steward of the Interledger Protocol, Open Payments API, and Web Monetization standard.
6. [Interledger Foundation — Team](https://interledger.org/team) — **First-party institutional page**; accessed **2026-09-01**. Supports current public leadership, the ILF abbreviation, and the description of a global workforce.
7. [Interledger Foundation — A Strategic Shift](https://interledger.org/news/strategic-shift-interledger-foundation) — **First-party announcement dated 2026-08-19**; accessed **2026-09-01**. Supports the current focus on open infrastructure and grantmaking, stewardship of Interledger Protocol/Open Payments/Rafiki, discontinuation of consumer initiatives, and personnel-change caveat.
8. [Interledger Foundation launch announcement](https://interledger.org/news/interledger-foundation-launches-build-more-equitable-and-creative-opportunities-web) — **First-party announcement**; accessed **2026-09-01**. Supports charitable 501(c)(3) status, official `interledger.org` URL, institutional activities, and the official link to `twitter.com/interledger`.
9. [Interledger on GitHub](https://github.com/interledger) — **Official public code organization**; accessed **2026-09-01**. Supports the Foundation-domain association and its public portfolio of Interledger specifications, Open Payments, Rafiki, and related software.
10. [Cause IQ — Interledger Foundation](https://www.causeiq.com/organizations/interledger-foundation%2C842364885/) — **Secondary profile derived from public tax filings**; accessed **2026-09-01**. Reports a 2024 employee count of 24 and corroborates EIN, formation year, activities, and filing history.

## Uncertainty and Conflicts

- The frozen upstream `domain_verification_status` is `null`. Bithomp currently labels the exact key/domain pairing verified through a TOML file, but this packet did not independently retrieve and cryptographically validate the required attestation.
- No first-party Interledger Foundation page found in this research names the validator master key or explicitly states that the Foundation currently operates the validator. Operator identity, key custody, infrastructure control, and voting control therefore remain unresolved.
- The exact per-key XRPSCAN API response was not accessible through the research client; no claim is made that it was browsed.
- California is supported as the incorporation state, but operational-location evidence is mixed: a June 2026 first-party submission says the organization is based in California, while the current Florida registry records a Tampa principal location. Neither record establishes where most personnel work.
- Server-geolocation reports were excluded from incorporation and operating-region conclusions.
- `Interledger` alone was not treated as an entity alias because first-party technical material distinguishes the protocol and ecosystem from the Foundation. Similar “ledger” or “foundation” names were not treated as aliases.
- The supplied publisher memberships were retained as frozen input; this research did not independently decode and verify both signed live publisher lists.
- Current headcount is unresolved. The 24-employee figure is a filing-derived historical snapshot preceding the August 2026 strategic change.

## Machine-Readable Summary

```json
{
  "validator_id": "nHUrUNXCy4DgPPNABX9C6mUctpoq7CwgLKAUxjw6zYtTfiqsj1ew",
  "network": "XRP Ledger mainnet",
  "claimed_domain": "xrp-validator.interledger.org",
  "domain_verification_status": null,
  "canonical_entity": "Interledger Foundation Inc.",
  "entity_type": "California-incorporated nonprofit corporation; U.S. 501(c)(3) charitable/private foundation",
  "aliases": [
    "Interledger Foundation",
    "ILF"
  ],
  "official_urls": [
    "https://interledger.org/",
    "https://github.com/interledger",
    "https://x.com/interledger"
  ],
  "business_summary": "Interledger Foundation Inc. is a United States charitable nonprofit and grantmaking foundation, incorporated in California and publicly associated with operating addresses in California and Florida. The organization stewards the Interledger Protocol, Open Payments, Rafiki, and related open standards and software, while funding research, education, community-finance, and digital-inclusion initiatives. Its stakeholders include regulated financial-service providers, community banks and credit unions, fintech developers, universities, grantees, governments, and underserved communities. Programs and partnerships are international, with activity documented across the United States and multiple global regions. Public filing-derived data places the organization in a small workforce tier, although its financial resources and sector reach are substantial relative to staff size. The validator key is publicly associated with the claimed xrp-validator.interledger.org hostname, but this packet does not independently establish that the Foundation currently controls or operates the validator.",
  "x_handle": "@interledger",
  "incorporation_region": "California, United States",
  "operating_regions": [
    "United States",
    "Global programs and partnerships"
  ],
  "profile_size_tier": "Small",
  "profile_size_confidence": "medium",
  "identity_confidence": "medium-high",
  "unresolved_fields": [
    "Independent cryptographic verification of the validator-domain attestation",
    "Direct first-party confirmation of the current validator operator",
    "Current validator master-key custody and infrastructure control",
    "Current principal operating base",
    "Current exact headcount",
    "Independent verification of both supplied live publisher-list memberships"
  ],
  "evidence_urls": [
    "https://api.xrpscan.com/api/v1/validator",
    "https://bithomp.com/validators?amendment=PermissionDelegation",
    "https://interledger.org/",
    "https://interledger.org/about-us",
    "https://interledger.org/team",
    "https://interledger.org/news/strategic-shift-interledger-foundation",
    "https://interledger.org/news/interledger-foundation-launches-build-more-equitable-and-creative-opportunities-web",
    "https://github.com/interledger",
    "https://search.sunbiz.org/Inquiry/CorporationSearch/SearchResultDetail?aggregateId=fornp-f20000003146-33c5dcbc-6240-4589-b760-371354f5e4a4&directionType=CurrentList&inquirytype=OfficerRegisteredAgentName&listNameOrder=SHINAKATLLC+L200003840321&searchNameOrder=SHINALEX+F200000031469&searchTerm=Shina+Investment+Llc",
    "https://projects.propublica.org/nonprofits/organizations/842364885",
    "https://www.fdic.gov/federal-register-publications/interledger-foundation-briana-marbury-rin-3064-ag19.pdf",
    "https://www.causeiq.com/organizations/interledger-foundation%2C842364885/"
  ]
}
```