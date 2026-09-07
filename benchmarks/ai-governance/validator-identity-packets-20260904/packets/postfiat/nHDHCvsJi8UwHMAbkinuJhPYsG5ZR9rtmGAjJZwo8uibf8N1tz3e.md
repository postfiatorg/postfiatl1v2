# Organization Profile

## Identity

Canonical name: **Not established**.

Entity type: **Not established**.

Supported aliases or former names: **None established**. “XBTSeal” appears only as part of the domain and is not treated as an organizational name or alias.

A public validator board associates `pft.xbtseal.com` with the exact supplied key, `nHDHCvsJi8UwHMAbkinuJhPYsG5ZR9rtmGAjJZwo8uibf8N1tz3e`. This establishes a public domain-key claim but does not identify the legal or public operator ([Network Health and Validator Board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php)). Post Fiat documentation requires operators to publish domain attestation at `https://<VALIDATOR_DOMAIN>/.well-known/pft-ledger.toml`, but the claimed domain and expected attestation file could not be retrieved during this research ([Post Fiat Validator Setup](https://postfiat.org/validator-setup/)).

Identity confidence: **Low**.

## Official Web Presence

Official website: **Not established**. The claimed validator hostname is [pft.xbtseal.com](https://pft.xbtseal.com/), but no accessible organizational website content was obtained.

Official X handle: **Not established**.

## Incorporation and Operations

Incorporation jurisdiction: **Not established** — high confidence that the accessible sources do not establish one.

Principal operating regions: **Not established** — high confidence. The validator board’s infrastructure-location field was excluded because server location does not establish an operator’s principal place of business ([Network Health and Validator Board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php)).

## Activities

The only supported activity is an apparent connection to a Post Fiat validator: the public validator board maps the exact supplied key to `pft.xbtseal.com` ([Network Health and Validator Board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php)). Official Post Fiat documentation describes a domain-attestation mechanism by which an operator publishes its validator key and attestation on its controlled domain ([Post Fiat Validator Setup](https://postfiat.org/validator-setup/)). Because the expected domain proof was not retrievable, domain control and the operator’s broader activities, products, services, customers, and stakeholders remain unestablished.

## Public Footprint

**Unknown** — low confidence. The accessible footprint consists only of one claimed validator hostname and its association with the supplied public key. No substantiated organization page, registry identity, personnel information, official social account, products, or customers were found. Headcount is **not established**.

## Business Summary

The organization behind the validator is not established from the accessible public record. The only observable institutional footprint is the claimed domain pft.xbtseal.com and its public association with the supplied Post Fiat validator key. No canonical legal or public name, entity type, aliases, incorporation jurisdiction, principal operating base, official website content, official X account, personnel, products, services, customer groups, or broader geographic reach could be substantiated. The hostname indicates participation in the Post Fiat network, but it does not by itself identify whether the operator is a company, nonprofit, individual, or pseudonymous group. Accordingly, the profile size tier is Unknown, headcount is not established, and no operating region is assigned. Any apparent server location or resemblance between “xbtseal” and another organization has been excluded as insufficient identity evidence.

## Evidence

1. [Network Health and Validator Board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php) — third-party public validator registry; accessed 2026-09-03. Maps `pft.xbtseal.com` to the exact validator key `nHDHCvsJi8UwHMAbkinuJhPYsG5ZR9rtmGAjJZwo8uibf8N1tz3e`.
2. [Post Fiat Validator Setup](https://postfiat.org/validator-setup/) — official network documentation; accessed 2026-09-03. Explains that an operator binds a validator to a domain and publishes the public key and attestation at `/.well-known/pft-ledger.toml`.
3. [Claimed validator domain](https://pft.xbtseal.com/) — domain-controlled source checked; access attempted 2026-09-03. No page content was retrievable, so it supplied no organizational identity facts.
4. [Expected Post Fiat domain-attestation path](https://pft.xbtseal.com/.well-known/pft-ledger.toml) — domain-controlled proof endpoint checked; access attempted 2026-09-03. No attestation content was retrievable.
5. [Verisign RDAP record for xbtseal.com](https://rdap.verisign.com/com/v1/domain/xbtseal.com) — authoritative registry endpoint checked; access attempted 2026-09-03. The record was not retrievable through the research interface, so no registrant identity or incorporation fact was obtained.

## Uncertainty

The operator’s name, entity type, ownership, personnel, incorporation, operating base, activities beyond the validator association, headcount, official website, and social accounts remain unresolved. The inaccessible domain proof prevents confirmation of the expected two-way validator-domain attestation. “XBTSeal” was not promoted from a domain label to an organization name. Similar names—including XBTO and XBT Provider—were excluded because no source links them to this domain or validator key. Infrastructure-location data was excluded from organizational-location conclusions.

## Machine-Readable Summary

```json
{
  "canonical_entity": null,
  "entity_type": null,
  "aliases": [],
  "official_website": null,
  "x_handle": null,
  "incorporation_region": null,
  "operating_regions": [],
  "activities": [
    "Publicly associated with the supplied Post Fiat validator key through the claimed domain pft.xbtseal.com"
  ],
  "profile_size_tier": "Unknown",
  "profile_size_confidence": "Low",
  "identity_confidence": "Low",
  "business_summary": "The organization behind the validator is not established from the accessible public record. The only observable institutional footprint is the claimed domain pft.xbtseal.com and its public association with the supplied Post Fiat validator key. No canonical legal or public name, entity type, aliases, incorporation jurisdiction, principal operating base, official website content, official X account, personnel, products, services, customer groups, or broader geographic reach could be substantiated. The hostname indicates participation in the Post Fiat network, but it does not by itself identify whether the operator is a company, nonprofit, individual, or pseudonymous group. Accordingly, the profile size tier is Unknown, headcount is not established, and no operating region is assigned. Any apparent server location or resemblance between “xbtseal” and another organization has been excluded as insufficient identity evidence.",
  "evidence_urls": [
    "https://mikeyexplains.com/webpage_network_health_and_validator_board.php",
    "https://postfiat.org/validator-setup/",
    "https://pft.xbtseal.com/",
    "https://pft.xbtseal.com/.well-known/pft-ledger.toml",
    "https://rdap.verisign.com/com/v1/domain/xbtseal.com"
  ],
  "unresolved": [
    "Canonical operator identity",
    "Entity type",
    "Domain control and two-way validator attestation",
    "Official website content",
    "Official X handle",
    "Incorporation jurisdiction",
    "Principal operating regions",
    "Products, services, customers, and stakeholders",
    "Personnel and headcount"
  ]
}
```