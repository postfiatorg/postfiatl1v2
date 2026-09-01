# Validator Identity Packet

## Packet Status

**SHADOW_ONLY.** Researched **2026-09-01 19:56:25 UTC**. This packet contains external public-identity evidence and research conclusions; it is not XRP Ledger consensus data.

## Validator Coordinates

- **Network:** XRP Ledger mainnet
- **Validator master public key:** `nHB8QMKGt9VB4Vg71VszjBVQnDW3v3QudM4DwFaJfy96bj4Pv9fA`
- **Claimed domain:** `bithomp.com` (claimed)
- **Frozen domain-verification status:** `null`; verification was not independently established in the frozen input.
- **Validator-list publishers containing the key:** `ripple`, `xrpl_foundation` (supplied frozen coordinates; Ripple inclusion was not independently decoded during this research).
- **Upstream metadata source:** [XRPSCAN validator API](https://api.xrpscan.com/api/v1/validator). The endpoint was supplied as input but did not return inspectable content through the research interface.

## Claimed Domain and Official URLs

**Domain conclusion:** `bithomp.com` is the most likely official domain, with high confidence. Its first-party [XRP Ledger TOML file](https://bithomp.com/.well-known/xrp-ledger.toml) names “Bithomp” as principal and contains the exact validator key, an attestation value, mainnet designation, and Swedish owner-country claim. The [verified Bithomp GitHub organization](https://github.com/bithomp) independently states that the organization controls `bithomp.com`. Nevertheless, the frozen `domain_verification_status` remains `null`: this research inspected but did not cryptographically verify the TOML attestation.

Official or first-party-associated URLs:

- Primary site: [https://bithomp.com/](https://bithomp.com/)
- Validator declaration: [https://bithomp.com/.well-known/xrp-ledger.toml](https://bithomp.com/.well-known/xrp-ledger.toml)
- API documentation: [https://docs.bithomp.com/](https://docs.bithomp.com/)
- Official GitHub: [https://github.com/bithomp](https://github.com/bithomp)
- XRP Explorer product URL: [https://xrplexplorer.com/](https://xrplexplorer.com/) (redirects to `bithomp.com`)
- Xahau Explorer product URL: [https://xahauexplorer.com/](https://xahauexplorer.com/)

## Public Identity

- **Canonical public entity:** **Bithomp AB**, the most likely legal entity behind the Bithomp brand; confidence is moderate-high rather than conclusive.
- **Entity type:** Swedish private limited company (`aktiebolag`).
- **Supported alias/trade name:** **Bithomp**.
- **Former legal names:** Not established.
- **Associated but separate entity:** **Ledger Explorer Ltd** is identified by Bithomp’s [privacy policy](https://bithomp.com/privacy-policy) as the service “Company” and is also listed in the site footer with a Seychelles address. It is not treated as an alias of Bithomp AB.
- **Identity connection:** The exact validator key is published under principal “Bithomp” in the domain’s [TOML file](https://bithomp.com/.well-known/xrp-ledger.toml). The official site identifies Bithomp AB and organization number `559342-2867`, while a [Swedish registry-sourced company profile](https://www.allabolag.se/foretag/bithomp-ab/stockholm/internet-konsulter-operat%C3%B6rer/2KI6GPFI5YFHL) corroborates that legal name, number, corporate form, and Stockholm seat. This supports the entity match but does not independently establish which legal entity controls the validator signing keys.

“XRP Explorer” and “Xahau Explorer” are treated as product descriptions, not entity aliases.

## Business Summary

Bithomp AB is a Swedish private limited software company and the most likely canonical legal entity behind the Bithomp public brand. Based in Stockholm, it provides web-based XRP Ledger and Xahau explorers, data APIs, network statistics, NFT discovery and transaction tools, and test-network utilities for ledger users, developers, token issuers, and other ecosystem participants. Its services are delivered online to an international blockchain audience. The official site also identifies Seychelles-addressed Ledger Explorer Ltd as the service company, so the precise allocation of activities between the two entities is not established. Registry-sourced records report one employee for Bithomp AB, placing its disclosed institutional footprint in the micro tier. The validator key appears in bithomp.com's published XRP Ledger TOML under the Bithomp principal, but this research did not independently verify the attestation or establish which legal entity controls the validator signing keys.

## Public X Handle

**@bithomp** — high confidence. The handle is declared as `x = "bithomp"` in the domain’s [XRP Ledger TOML](https://bithomp.com/.well-known/xrp-ledger.toml) and linked by the domain-verified [Bithomp GitHub organization](https://github.com/bithomp). The direct [X profile](https://x.com/bithomp) could not be inspected because it returned HTTP 403 through the research interface.

## Region of Incorporation and Operations

- **Incorporation jurisdiction:** **Sweden — high confidence for Bithomp AB.** The [registry-sourced profile](https://www.allabolag.se/foretag/bithomp-ab/stockholm/internet-konsulter-operat%C3%B6rer/2KI6GPFI5YFHL), citing Bolagsverket, SCB, and Skatteverket, identifies Bithomp AB as a Swedish `aktiebolag`, registered on 22 October 2021 with a Stockholm seat.
- **Principal operating region:** **Stockholm, Sweden — high confidence for Bithomp AB.** Both the [official Bithomp site](https://bithomp.com/about-us) and registry-sourced profile place it in Stockholm.
- **Geographic service reach:** **International online reach — medium confidence.** Bithomp provides browser-based explorers and APIs for XRPL and Xahau networks through its [official product pages](https://bithomp.com/about-us); this indicates international accessibility, not additional physical offices.
- **Ledger Explorer Ltd:** The official [privacy policy](https://bithomp.com/privacy-policy) gives it a Seychelles address, but its incorporation jurisdiction and operational base were not independently confirmed through a company registry.
- Server-country information was not used to infer incorporation or principal operations.

## Activities

Bithomp describes its activities as operating XRPL and Xahau ledger explorers, providing APIs and network statistics, indexing accounts and transactions, supporting NFT discovery and transactions, and offering test-network explorers and faucets for developers ([Bithomp About Us](https://bithomp.com/about-us), [Bithomp API documentation](https://docs.bithomp.com/)). Its Swedish registered activity is software-development consulting and computer programming ([Allabolag company profile](https://www.allabolag.se/foretag/bithomp-ab/stockholm/internet-konsulter-operat%C3%B6rer/2KI6GPFI5YFHL)).

The relationship to the validator is supported by the first-party [TOML declaration](https://bithomp.com/.well-known/xrp-ledger.toml), which places the exact key under the Bithomp principal, and by the current [Bithomp validator registry](https://bithomp.com/validators), which associates the key with Bithomp. This is stronger than list membership alone, but technical control of the signing keys was not independently demonstrated.

## Estimated Public-Profile Size

**Micro.** The latest [registry-sourced company listing](https://www.allabolag.se/foretag/bithomp-ab/stockholm/internet-konsulter-operat%C3%B6rer/2KI6GPFI5YFHL) reports one employee for Bithomp AB, while its [LinkedIn company page](https://www.linkedin.com/company/bithomp) describes a 2–10-person organization and displays three employee profiles. **Confidence: high** for the rubric tier. Headcount is established as one employee for Bithomp AB in the latest registry-sourced record; current platform-wide or combined headcount across Bithomp AB and Ledger Explorer Ltd is not established.

## Evidence

1. [bithomp.com XRP Ledger TOML](https://bithomp.com/.well-known/xrp-ledger.toml) — **“bithomp.com xrp-ledger.toml file”; first-party network identity declaration; accessed 2026-09-01.** Names Bithomp as principal, declares `@bithomp`, contains the exact validator key and attestation, identifies mainnet, and self-reports Sweden as owner country.
2. [Bithomp About Us](https://bithomp.com/about-us) — **“Welcome to Bithomp”; official website; accessed 2026-09-01.** Supports the Bithomp brand, service history since 2015, XRPL/Xahau explorers, APIs, NFT services, test-network utilities, Bithomp AB footer identity, organization number, and Stockholm presence.
3. [Bithomp Privacy Policy](https://bithomp.com/privacy-policy) — **“Privacy Policy”; official legal page; accessed 2026-09-01.** Identifies Ledger Explorer Ltd as the service company and provides its Seychelles address; it also contains the unexplained statement “Country refers to: Malta.”
4. [Bithomp GitHub organization](https://github.com/bithomp) — **“Bithomp: XRP Explorer / Xahau Explorer”; domain-verified organization profile; accessed 2026-09-01.** GitHub states that the organization controls `bithomp.com`; the profile links `@bithomp`, related explorer handles, the official site, and public software repositories.
5. [Bithomp AB company profile](https://www.allabolag.se/foretag/bithomp-ab/stockholm/internet-konsulter-operat%C3%B6rer/2KI6GPFI5YFHL) — **Allabolag registry-sourced profile; company-data aggregator citing SCB, Bolagsverket, and Skatteverket; accessed 2026-09-01.** Supports legal name, organization number, Swedish private-company form, registration date, Stockholm seat, computer-programming activity, and latest reported headcount of one.
6. [Bithomp validator registry](https://bithomp.com/validators) — **“Validators”; first-party live network registry/aggregator; accessed 2026-09-01.** Associates the exact key with Bithomp, `bithomp.com`, Sweden as owner country, and the XRPL Foundation list; it labels the domain verified, a status not independently reproduced here.
7. [Bithomp AB LinkedIn](https://www.linkedin.com/company/bithomp) — **Company social profile; accessed 2026-09-01.** Supports Stockholm headquarters, privately held status, a stated 2–10-person size range, three displayed employee profiles, and association with XRP and Xahau explorer products.
8. [XRPL FAQ](https://xrpl.org/about/faq) — **Official XRP Ledger documentation; accessed 2026-09-01.** Explains validator lists and identifies Ripple and the XRP Ledger Foundation as known recommended-list publishers; it does not by itself prove this key’s inclusion.
9. [XRPSCAN validator API](https://api.xrpscan.com/api/v1/validator) — **Supplied upstream metadata endpoint; access attempted 2026-09-01.** The research interface did not return inspectable content, so no additional fact was independently derived from it.

## Uncertainty and Conflicts

- The frozen `domain_verification_status` is `null`. Bithomp’s current validator page labels the domain “Verified domain (TOML file),” and the TOML contains an attestation, but this research did not cryptographically verify that attestation; the machine-readable value therefore remains `null`.
- The official website names both **Bithomp AB** and **Ledger Explorer Ltd**. Its privacy policy defines Ledger Explorer Ltd as the service company, while the footer asserts Bithomp AB copyright. The allocation of website, validator, intellectual-property, and operating responsibilities between them is not established.
- Ledger Explorer Ltd’s Seychelles address is first-party evidence of an address, not independently verified evidence of Seychelles incorporation. The privacy policy’s separate reference to Malta is unexplained and was not treated as an incorporation or operating-region fact.
- Control of the validator signing keys by Bithomp AB, Ledger Explorer Ltd, or another party is not independently established.
- Ripple and XRP Ledger Foundation list inclusion was supplied in the frozen coordinates. Current XRPL Foundation association is visible in the checked Bithomp registry, but Ripple’s current list blob was not independently decoded.
- The latest registry-sourced listing reports one Bithomp AB employee, while LinkedIn states 2–10 people and shows three profiles. This does not change the Micro classification but leaves combined current headcount unresolved.
- Finland server location, Hetzner hosting, domain-registration data, language, and country-code indicators were excluded from incorporation and operating-region conclusions.
- “XRP Explorer” and “Xahau Explorer” were excluded as entity aliases because the evidence supports them as products or descriptive labels. No former legal name was established.
- The similarly named **Bithumb** was excluded as unrelated.

## Machine-Readable Summary

```json
{
  "validator_id": "nHB8QMKGt9VB4Vg71VszjBVQnDW3v3QudM4DwFaJfy96bj4Pv9fA",
  "network": "XRP Ledger mainnet",
  "claimed_domain": "bithomp.com",
  "domain_verification_status": null,
  "canonical_entity": "Bithomp AB",
  "entity_type": "Swedish private limited company (aktiebolag)",
  "aliases": [
    "Bithomp"
  ],
  "official_urls": [
    "https://bithomp.com/",
    "https://bithomp.com/.well-known/xrp-ledger.toml",
    "https://docs.bithomp.com/",
    "https://github.com/bithomp",
    "https://xrplexplorer.com/",
    "https://xahauexplorer.com/"
  ],
  "business_summary": "Bithomp AB is a Swedish private limited software company and the most likely canonical legal entity behind the Bithomp public brand. Based in Stockholm, it provides web-based XRP Ledger and Xahau explorers, data APIs, network statistics, NFT discovery and transaction tools, and test-network utilities for ledger users, developers, token issuers, and other ecosystem participants. Its services are delivered online to an international blockchain audience. The official site also identifies Seychelles-addressed Ledger Explorer Ltd as the service company, so the precise allocation of activities between the two entities is not established. Registry-sourced records report one employee for Bithomp AB, placing its disclosed institutional footprint in the micro tier. The validator key appears in bithomp.com's published XRP Ledger TOML under the Bithomp principal, but this research did not independently verify the attestation or establish which legal entity controls the validator signing keys.",
  "x_handle": "@bithomp",
  "incorporation_region": "Sweden",
  "operating_regions": [
    "Stockholm, Sweden",
    "International online service reach"
  ],
  "profile_size_tier": "Micro",
  "profile_size_confidence": "high",
  "identity_confidence": "moderate-high",
  "unresolved_fields": [
    "Independent cryptographic verification of the validator/domain attestation",
    "Legal entity controlling the validator signing keys",
    "Allocation of services between Bithomp AB and Ledger Explorer Ltd",
    "Ledger Explorer Ltd incorporation jurisdiction",
    "Combined current headcount across associated entities",
    "Independent current confirmation of Ripple list inclusion"
  ],
  "evidence_urls": [
    "https://bithomp.com/.well-known/xrp-ledger.toml",
    "https://bithomp.com/about-us",
    "https://bithomp.com/privacy-policy",
    "https://github.com/bithomp",
    "https://www.allabolag.se/foretag/bithomp-ab/stockholm/internet-konsulter-operat%C3%B6rer/2KI6GPFI5YFHL",
    "https://bithomp.com/validators",
    "https://www.linkedin.com/company/bithomp",
    "https://xrpl.org/about/faq",
    "https://api.xrpscan.com/api/v1/validator"
  ]
}
```