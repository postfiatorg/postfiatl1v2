# Organization Profile

## Identity

**Canonical name:** Not established.  
**Entity type:** Not established.  
**Supported aliases or former names:** None.

A public validator board pairs `lc66validator.postfiatcn.org` with validator key `nHUCEXpC5LhFAm1Mmf8TqrzVGt3QCuwWoW2V8PYynDpjZe8m8mHj`, establishing an observable association between the two coordinates but not identifying the operator behind them ([validator board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php)). Post Fiat’s official setup documentation says a validator domain should be controlled by the operator and should publish a key-bound attestation, but the claimed domain’s attestation could not be retrieved during this research; domain control and organizational identity therefore remain unconfirmed ([Post Fiat validator setup](https://postfiat.org/validator-setup/)).

## Official Web Presence

**Official website:** Not established. `lc66validator.postfiatcn.org` is a claimed validator domain, but no accessible organizational website or primary-source operator identity was found.

**Official X handle:** Not established. No primary source connected an X account to the validator key or claimed domain.

## Incorporation and Operations

**Incorporation jurisdiction:** Not established — high confidence that the available evidence does not support an attribution.

**Principal operating regions:** Not established — high confidence. The hostname string and infrastructure geolocation are not evidence of an operator’s legal or principal operating location. A validator board’s hosting-location field was therefore excluded from the organizational profile ([validator board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php)).

## Activities

The only supported activity is association with a validator identity on the Post Fiat network. The key-domain pairing appears on a public validator board, while Post Fiat’s official documentation describes validators as network participants that bind a public key to an operator-controlled domain through a published attestation ([validator board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php); [official setup documentation](https://postfiat.org/validator-setup/)). No attributable products, services, customers, commercial activities, or other institutional functions were established.

## Public Footprint

**Unknown.** The attributable footprint consists only of the key-domain listing; no supported team information, headcount, legal filing, official organizational page, or social account was found ([validator board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php)). Headcount is not established. Confidence in assigning a more specific size classification is low.

## Business Summary

The organization behind validator nHUCEXpC5LhFAm1Mmf8TqrzVGt3QCuwWoW2V8PYynDpjZe8m8mHj is not established from accessible public evidence. The public record reviewed ties the validator key to the claimed domain lc66validator.postfiatcn.org, but does not identify a canonical operator, legal entity, individual, alias, incorporation jurisdiction, principal operating base, official website, or X account. The only supported activity is participation in the Post Fiat network through a validator identity and associated domain claim; no separate products, services, customers, or institutional stakeholders are documented for the operator. The hostname’s “cn” string and infrastructure geolocation are not evidence of incorporation or operations and therefore are excluded. Public footprint is classified as Unknown because no attributable team, filings, official organizational pages, or supported headcount were found.

## Evidence

1. [Network Health and Validator Board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php) — third-party validator board; accessed 2026-09-03. Pairs `lc66validator.postfiatcn.org` with the exact supplied validator public key. It does not identify a legal or public operator.
2. [Post Fiat Validator Setup](https://postfiat.org/validator-setup/) — official network documentation; accessed 2026-09-03. Explains that operators bind validators to domains and publish `/.well-known/pft-ledger.toml` containing the public key and attestation.
3. [Post Fiat](https://postfiat.org/) — official network website; accessed 2026-09-03. Supports that validator participation is part of the Post Fiat network, but contains no identified connection between this validator and an organization.
4. [Claimed validator domain](https://lc66validator.postfiatcn.org/) — supplied public endpoint; retrieval attempted 2026-09-03. The page was not accessible through the research interface, so no organizational facts were derived from it.
5. [Public Interest Registry RDAP query for postfiatcn.org](https://rdap.publicinterestregistry.org/rdap/domain/postfiatcn.org) — registry endpoint; retrieval attempted 2026-09-03. The domain-specific response was unavailable through the research interface, so no registrant or incorporation facts were used.

## Uncertainty

The validator operator’s name, entity type, legal existence, incorporation, operating base, personnel, ownership, products, customers, official website, and social accounts remain unresolved. The strings `lc66`, `postfiatcn`, and `cn` were not treated as an operator name, alias, organization, or geographic designation because no primary source supports those interpretations. The key-domain association is observable, but the underlying domain-attestation file was not independently retrieved. Registration details were also unavailable, and domain registration would not by itself establish validator control or incorporation.

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
    "Association with a Post Fiat validator identity and claimed validator domain"
  ],
  "profile_size_tier": "Unknown",
  "profile_size_confidence": "Low",
  "identity_confidence": "Low",
  "business_summary": "The organization behind validator nHUCEXpC5LhFAm1Mmf8TqrzVGt3QCuwWoW2V8PYynDpjZe8m8mHj is not established from accessible public evidence. The public record reviewed ties the validator key to the claimed domain lc66validator.postfiatcn.org, but does not identify a canonical operator, legal entity, individual, alias, incorporation jurisdiction, principal operating base, official website, or X account. The only supported activity is participation in the Post Fiat network through a validator identity and associated domain claim; no separate products, services, customers, or institutional stakeholders are documented for the operator. The hostname’s “cn” string and infrastructure geolocation are not evidence of incorporation or operations and therefore are excluded. Public footprint is classified as Unknown because no attributable team, filings, official organizational pages, or supported headcount were found.",
  "evidence_urls": [
    "https://mikeyexplains.com/webpage_network_health_and_validator_board.php",
    "https://postfiat.org/validator-setup/",
    "https://postfiat.org/",
    "https://lc66validator.postfiatcn.org/",
    "https://rdap.publicinterestregistry.org/rdap/domain/postfiatcn.org"
  ],
  "unresolved": [
    "Canonical operator identity",
    "Entity type",
    "Domain control and attestation",
    "Legal incorporation",
    "Principal operating base",
    "Official organizational website",
    "Official X handle",
    "Products, services, and stakeholder types",
    "Headcount and institutional scale"
  ]
}
```