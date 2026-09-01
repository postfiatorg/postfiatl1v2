# Validator Identity Packet

## Packet Status

**SHADOW_ONLY**

Research timestamp: **2026-09-01T19:08:18Z**

This packet contains external public-identity evidence. It is not XRP Ledger consensus data and does not establish validator ownership through consensus.

## Validator Coordinates

- **Network:** XRP Ledger mainnet
- **Validator master public key:** `nHU4bLE3EmSqNwfL4AP1UZeTNPrSPPP6FXLKXo2uqfHuvBQxDVKd`
- **Claimed domain:** `ripple.com`
- **Frozen domain-verification status:** `null` — not independently established; this is not equivalent to a failed verification.
- **Validator-list publishers containing the key:** Ripple and XRP Ledger Foundation, as stated in the frozen input. Ripple maintains a [signed validator-list archive](https://github.com/ripple/vl), while an XRPL Foundation repository example maps this exact key to `ripple.com` and documents signed UNL generation ([XRPLF/xrpl-cli](https://github.com/XRPLF/xrpl-cli)).
- **Upstream metadata source:** [XRPSCAN validator API](https://api.xrpscan.com/api/v1/validator). XRPSCAN documents validator-specific lookup as `/validator/{VALIDATOR_PUBLIC_KEY}` ([API documentation](https://docs.xrpscan.com/api-documentation/validator/validator-info)); the supplied record itself was not successfully retrieved during this research.

## Claimed Domain and Official URLs

**Domain conclusion:** `ripple.com` is the official corporate domain of Ripple Labs Inc., with **high confidence**. Ripple’s legal terms expressly identify `www.ripple.com` as a website of Ripple Labs Inc. and its subsidiaries ([Ripple Terms of Use](https://ripple.com/legal/terms-of-use/)). Ripple’s verified GitHub organization also states that the organization controls `ripple.com` ([GitHub: Ripple](https://github.com/ripple)).

That conclusion concerns the domain-to-entity relationship. The separate mapping of this validator key to the domain remains **claimed / not independently verified**, because no validator-domain attestation was independently established in this packet.

Official URLs:

- [https://ripple.com/](https://ripple.com/) — official corporate website.
- [https://docs.ripple.com/](https://docs.ripple.com/) — official product documentation.
- [https://github.com/ripple](https://github.com/ripple) — verified corporate GitHub organization controlling `ripple.com`.

## Public Identity

- **Canonical public entity:** **Ripple Labs Inc.**
- **Entity type:** Delaware corporation and financial-technology company. A current SEC filing identifies Ripple Labs Inc. as a corporation organized in Delaware ([SEC Form D](https://www.sec.gov/Archives/edgar/data/1685012/000168501225000002/xslFormDX01/primary_doc.xml)); Ripple describes itself as a financial-technology company providing crypto solutions to institutions and other organizations ([About Ripple](https://ripple.com/company/)).
- **Identity confidence:** **Medium-high** for the validator-to-entity conclusion; **high** for the `ripple.com`-to-Ripple Labs Inc. conclusion.

Supported aliases and historical names:

- **Ripple** — current official brand and collective name for Ripple Labs Inc. and its subsidiaries ([Ripple Compliance & Disclosures](https://ripple.com/legal/compliance/)). Because Ripple uses the name collectively for a corporate group, it is not necessarily an exact legal synonym for the parent in every context.
- **Ripple Labs** — supported short-form mark used in Ripple’s official legal terms ([Ripple Terms of Use](https://ripple.com/legal/terms-of-use/)).
- **Open Coin, Inc.** — supported historical corporate name. An SEC complaint describes Ripple as “f/k/a Open Coin, Inc.” ([SEC complaint](https://www.sec.gov/files/litigation/complaints/2020/comp-pr2020-338.pdf)).

The evidence chain connecting the claimed domain to the entity is:

1. The frozen upstream metadata identifies `ripple.com` for the supplied key.
2. An XRP Ledger Foundation repository example independently pairs the exact key with the name `ripple.com` ([XRPLF/xrpl-cli](https://github.com/XRPLF/xrpl-cli)).
3. Ripple’s legal terms identify `ripple.com` as a Ripple Labs Inc. website ([Ripple Terms of Use](https://ripple.com/legal/terms-of-use/)).
4. Ripple’s verified GitHub organization reports control of `ripple.com` ([GitHub: Ripple](https://github.com/ripple)).

This supports Ripple Labs Inc. as the most likely public identity, but it does not independently prove that Ripple presently possesses or controls the validator’s private keys.

## Public X Handle

**@Ripple** — **established**.

Verification basis: the [X profile](https://x.com/Ripple) identifies itself as Ripple and links to `ripple.com`; Ripple’s official website also directs readers to its X presence in company publications ([Ripple: XRP Community Day 2025](https://ripple.com/insights/xrp-community-day-2025-a-record-breaking-event-uniting-the-global-community/)).

## Region of Incorporation and Operations

- **Incorporation jurisdiction:** **Delaware, United States — high confidence.** Ripple Labs Inc.’s SEC Form D records Delaware as its jurisdiction of incorporation ([SEC Form D](https://www.sec.gov/Archives/edgar/data/1685012/000168501225000002/xslFormDX01/primary_doc.xml)).
- **Principal operational base:** **San Francisco, California, United States — high confidence.** The same SEC filing identifies San Francisco as the principal place of business, and Ripple’s privacy policy describes the United States as the location of its corporate headquarters ([Ripple Privacy Policy](https://ripple.com/legal/privacy-policy/)).
- **Operational regions:** **North America, Europe, Asia-Pacific, the Middle East, and Latin America — high confidence.** Ripple’s official locations page identifies offices including San Francisco, New York, Washington, D.C., London, Dublin, Reykjavik, Singapore, Mumbai, Sydney, Dubai, and São Paulo ([Ripple Locations](https://ripple.com/careers/locations/)). Ripple separately reports delivering solutions in more than 90 countries ([About Ripple](https://ripple.com/company/)).

These operational regions are based on disclosed offices and service footprint, not validator-server geolocation.

## Activities

Ripple provides institutional financial infrastructure involving payments, digital-asset custody, stablecoins, prime brokerage, treasury management, and tokenization ([Ripple corporate website](https://ripple.com/)). It describes its customers and users as global financial institutions, businesses, governments, and developers ([About Ripple](https://ripple.com/company/)).

Ripple states that its solutions use XRP, RLUSD, and other digital assets, and that Ripple is an XRP holder and one of multiple developers building on and contributing to the XRP Ledger ([Ripple XRP page](https://ripple.com/xrp/)). Its verified GitHub organization publishes XRPL-related open-source software, including validator tooling and validation-history infrastructure ([GitHub: Ripple](https://github.com/ripple)).

For this particular validator, the exact public key is associated with the name `ripple.com` in an XRPL Foundation repository example ([XRPLF/xrpl-cli](https://github.com/XRPLF/xrpl-cli)). Therefore, a Ripple-operated or Ripple-associated validator is the most likely interpretation, but current operation and cryptographic control are not independently established.

## Estimated Public-Profile Size

**Very large**

- **Confidence:** High
- **Headcount status:** Established only at the published approximate threshold; exact current headcount is not established.

Ripple’s official company page reports **1,000+ employees**, **15+ offices**, and activity in **90+ countries** ([About Ripple](https://ripple.com/company/)). The disclosed headcount threshold and major international institutional footprint support the **Very large** rubric tier. The public figure is rounded and does not provide an exact current employee count.

## Evidence

1. [XRPLF/xrpl-cli](https://github.com/XRPLF/xrpl-cli) — **“XRPLF/xrpl-cli”**; XRP Ledger Foundation GitHub repository; accessed **2026-09-01**. Its signed-UNL example pairs the exact validator master key with the name `ripple.com`.

2. [Ripple Historical Validator Lists](https://github.com/ripple/vl) — **“Historical Validator Lists”**; official Ripple GitHub repository; accessed **2026-09-01**. Supports Ripple’s role as a signed XRP Ledger validator-list publisher.

3. [Ripple Terms of Use](https://ripple.com/legal/terms-of-use/) — **“Terms of Use”**; primary corporate legal page; accessed **2026-09-01**. Identifies `www.ripple.com` as a Ripple Labs Inc. website and supports the names Ripple and Ripple Labs.

4. [GitHub: Ripple](https://github.com/ripple) — **“Ripple”**; verified corporate GitHub organization; accessed **2026-09-01**. Reports verified control of `ripple.com` and supports Ripple’s open-source and XRPL-development activities.

5. [SEC Form D for Ripple Labs Inc.](https://www.sec.gov/Archives/edgar/data/1685012/000168501225000002/xslFormDX01/primary_doc.xml) — **“Form D — Ripple Labs Inc.”**; U.S. regulator filing; accessed **2026-09-01**. Supports the canonical legal name, corporation type, Delaware incorporation, and San Francisco principal place of business.

6. [About Ripple](https://ripple.com/company/) — **“About Ripple”**; primary corporate profile; accessed **2026-09-01**. Supports the financial-technology identity, institutional activities, 1,000+ employees, 15+ offices, and 90+ countries served.

7. [Ripple Locations](https://ripple.com/careers/locations/) — **“Locations Around the World”**; primary corporate careers page; accessed **2026-09-01**. Supports offices across North America, Europe, Asia-Pacific, the Middle East, and Latin America.

8. [Ripple on X](https://x.com/Ripple) — **“Ripple (@Ripple)”**; official social profile; accessed **2026-09-01**. Supports the official X handle through the profile’s Ripple identity and `ripple.com` link.

9. [Ripple XRP page](https://ripple.com/xrp/) — **“XRP Digital Asset for Global Crypto Utility”**; primary corporate product/ecosystem page; accessed **2026-09-01**. Supports Ripple’s use of XRP and RLUSD and its stated role as one of multiple XRP Ledger contributors.

10. [SEC complaint concerning Ripple](https://www.sec.gov/files/litigation/complaints/2020/comp-pr2020-338.pdf) — **“SEC v. Ripple Labs, Inc. — Complaint”**; regulator-filed court document; accessed **2026-09-01**. Supports the historical name Open Coin, Inc. and historical Delaware/San Francisco corporate description.

11. [XRPSCAN Validator Info documentation](https://docs.xrpscan.com/api-documentation/validator/validator-info) — **“Validator info”**; upstream API documentation; accessed **2026-09-01** through the public search index. Documents the validator-specific API response fields, including master key and domain. The [input-provided API endpoint](https://api.xrpscan.com/api/v1/validator) was not independently retrieved successfully on **2026-09-01**.

## Uncertainty and Conflicts

- No signed `xrp-ledger.toml` validator-domain attestation for this exact key was independently established. The frozen verification value remains `null`.
- The supplied XRPSCAN record could not be independently retrieved during this research. Its claimed domain and publisher memberships are therefore preserved as frozen input rather than represented as freshly verified API results.
- The XRPL Foundation repository’s exact key-to-`ripple.com` mapping is corroborative metadata, but a repository example is not proof of present private-key possession, validator operation, or domain attestation.
- Ripple Labs Inc. is the supported legal parent name, while **Ripple** can refer collectively to the parent and its subsidiaries. Corporate-group actions should not automatically be attributed to the parent legal entity.
- Ripple’s verified GitHub description uses “Ripple, Inc.” while current Ripple legal pages and SEC filings identify **Ripple Labs Inc.** “Ripple, Inc.” is therefore not treated as a separately established legal alias.
- Only the exact historical name **Open Coin, Inc.** is included. Similar spellings such as “OpenCoin” are excluded absent equally strong evidence for that precise form.
- The validator’s current operator, hosting provider, physical server location, and operational personnel are not established.
- Ripple publishes a rounded “1,000+” employee figure; the exact current headcount is not established.
- Similar-name organizations and social accounts, including “Rippling,” are not aliases and were excluded.

## Machine-Readable Summary

```json
{
  "validator_id": "nHU4bLE3EmSqNwfL4AP1UZeTNPrSPPP6FXLKXo2uqfHuvBQxDVKd",
  "network": "XRP Ledger mainnet",
  "claimed_domain": "ripple.com",
  "domain_verification_status": null,
  "canonical_entity": "Ripple Labs Inc.",
  "entity_type": "Delaware corporation and financial-technology company",
  "aliases": [
    "Ripple",
    "Ripple Labs",
    "Open Coin, Inc. (former legal name)"
  ],
  "official_urls": [
    "https://ripple.com/",
    "https://docs.ripple.com/",
    "https://github.com/ripple"
  ],
  "x_handle": "@Ripple",
  "incorporation_region": "Delaware, United States",
  "operating_regions": [
    "North America",
    "Europe",
    "Asia-Pacific",
    "Middle East",
    "Latin America"
  ],
  "profile_size_tier": "Very large",
  "profile_size_confidence": "high",
  "identity_confidence": "medium-high",
  "unresolved_fields": [
    "Independent validator-domain attestation for the supplied master key",
    "Current validator operator and cryptographic control",
    "Current validator hosting provider and physical server location",
    "Exact current employee headcount",
    "Independent retrieval of the frozen XRPSCAN metadata record"
  ],
  "evidence_urls": [
    "https://github.com/XRPLF/xrpl-cli",
    "https://github.com/ripple/vl",
    "https://ripple.com/legal/terms-of-use/",
    "https://github.com/ripple",
    "https://www.sec.gov/Archives/edgar/data/1685012/000168501225000002/xslFormDX01/primary_doc.xml",
    "https://ripple.com/company/",
    "https://ripple.com/careers/locations/",
    "https://ripple.com/legal/privacy-policy/",
    "https://x.com/Ripple",
    "https://ripple.com/xrp/",
    "https://www.sec.gov/files/litigation/complaints/2020/comp-pr2020-338.pdf",
    "https://docs.xrpscan.com/api-documentation/validator/validator-info",
    "https://api.xrpscan.com/api/v1/validator"
  ]
}
```