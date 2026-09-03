# Organization Profile

## Identity

**Canonical name:** Not established.  
**Entity type:** Not established.  
**Aliases or former names:** None supported.

The exact validator key appears in an independent public validator dataset, but its domain field is recorded as “N/A,” so that source does not connect the key to `pft.whiteguy.eu` or identify an operator ([Network Health & Validator Intelligence](https://mikeyexplains.com/webpage_network_health_and_validator_board.php)). Post Fiat’s official setup documentation explains that operators ordinarily prove control by publishing the validator key and an attestation at `/.well-known/pft-ledger.toml` ([Post Fiat Validator Setup](https://postfiat.org/validator-setup/)). The corresponding file at the claimed domain was not accessible during this research, so the domain-key relationship and operator identity remain unverified.

## Official Web Presence

**Official website:** Not established. The supplied URL, [pft.whiteguy.eu](https://pft.whiteguy.eu/), could not be accessed, and no indexed primary source identified its owner.

**Official X handle:** Not established. Search results surfaced a pseudonymous account, `@White_Guy_101`, whose third-party-indexed biography mentions Post Fiat, but neither an accessible official profile nor another primary source connected that account to the domain or validator key ([indexed profile result](https://www.idcrawl.com/white-guy)). It is therefore excluded as an official handle.

## Incorporation and Operations

**Incorporation jurisdiction:** Not established — high confidence that the public evidence reviewed is insufficient. No legal page, filing, registry record, or named entity was found. The `.eu` suffix was not treated as evidence of incorporation; EURid provides the authoritative domain-details lookup for `.eu` names ([EURid](https://eurid.eu/en/)), but no registrant identity supporting an organization was established.

**Principal operating regions:** Not established — high confidence that the public evidence reviewed is insufficient. No location was inferred from the domain suffix, infrastructure, language, or social-profile candidates.

## Activities

No organization-level activities, products, services, customers, or stakeholders were established. The only supported public activity is that the supplied key is observable in a validator dataset ([Network Health & Validator Intelligence](https://mikeyexplains.com/webpage_network_health_and_validator_board.php)). Although Post Fiat documents a cryptographic domain-attestation process for connecting a validator identity to an operator-controlled domain ([Post Fiat Validator Setup](https://postfiat.org/validator-setup/)), the claimed domain’s proof file could not be inspected. Consequently, the validator’s relationship to any named organization or pseudonymous operator remains unconfirmed.

## Public Footprint

**Unknown.** No verified organizational website, legal entity, personnel roster, official repository, product surface, or operator-controlled social account was found. Headcount is not established. Confidence in assigning any more specific size tier is low.

## Business Summary

The identity of the organization or person behind this validator is not established. Publicly observable information is limited to the supplied Post Fiat validator key and the claimed domain pft.whiteguy.eu; an independent validator page located the key but did not associate it with a domain, and the claimed site and standard domain-proof file could not be accessed during this research. No canonical name, legal entity type, incorporation jurisdiction, principal operating base, products, services, customers, stakeholders, or geographic reach could be verified from primary sources. A possibly related pseudonymous social account was excluded because no primary source connected it to the domain or validator key. The public footprint size tier is Unknown, and headcount is not established.

## Evidence

1. [Network Health & Validator Intelligence](https://mikeyexplains.com/webpage_network_health_and_validator_board.php) — independent validator dataset; accessed 2026-09-03. It contains the exact validator key but displays “N/A” for its domain and location. It does not identify an operator.
2. [Post Fiat Validator Setup](https://postfiat.org/validator-setup/) — official network documentation; accessed 2026-09-03. It explains that validator-domain proof should be published as a key and attestation at `https://<domain>/.well-known/pft-ledger.toml`.
3. [Post Fiat live validator API](https://vhs.testnet.postfiat.org/v1/network/validators/test) — official network API; checked 2026-09-03. Its response body was not inspectable through the available research interface, so it was not used to attribute the validator.
4. [Claimed validator domain](https://pft.whiteguy.eu/) — operator-claimed web endpoint; checked 2026-09-03. The page was not accessible and was not treated as browsed evidence.
5. [Claimed domain-attestation file](https://pft.whiteguy.eu/.well-known/pft-ledger.toml) — expected primary proof endpoint; checked 2026-09-03. It was not accessible, preventing independent verification of the domain-key association.
6. [EURid](https://eurid.eu/en/) — official `.eu` registry and domain-details service; accessed 2026-09-03. No registrant evidence establishing a legal or public operator was obtained.
7. [Indexed “WhiteGuy” social-profile results](https://www.idcrawl.com/white-guy) — third-party social index; accessed 2026-09-03. It shows a possible Post Fiat-related pseudonymous account, but supplies no domain or validator-key connection and therefore does not establish identity.

## Uncertainty

- Control of `pft.whiteguy.eu` by the validator-key holder was not independently verified.
- The domain’s registrant, operator, legal status, and organizational affiliation remain unknown.
- No incorporation jurisdiction or principal operating region was established.
- The possible `@White_Guy_101` association is only a name-and-interest similarity and was excluded as an unsupported inference.
- The claimed domain could represent an individual, pseudonymous operator, or organization; the available evidence does not distinguish among them.
- No aliases, former names, personnel, products, services, or headcount were verified.

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
    "A validator key is publicly observable, but the operator's identity and wider activities are not established."
  ],
  "profile_size_tier": "Unknown",
  "profile_size_confidence": "Low",
  "identity_confidence": "Low",
  "business_summary": "The identity of the organization or person behind this validator is not established. Publicly observable information is limited to the supplied Post Fiat validator key and the claimed domain pft.whiteguy.eu; an independent validator page located the key but did not associate it with a domain, and the claimed site and standard domain-proof file could not be accessed during this research. No canonical name, legal entity type, incorporation jurisdiction, principal operating base, products, services, customers, stakeholders, or geographic reach could be verified from primary sources. A possibly related pseudonymous social account was excluded because no primary source connected it to the domain or validator key. The public footprint size tier is Unknown, and headcount is not established.",
  "evidence_urls": [
    "https://mikeyexplains.com/webpage_network_health_and_validator_board.php",
    "https://postfiat.org/validator-setup/",
    "https://vhs.testnet.postfiat.org/v1/network/validators/test",
    "https://pft.whiteguy.eu/",
    "https://pft.whiteguy.eu/.well-known/pft-ledger.toml",
    "https://eurid.eu/en/",
    "https://www.idcrawl.com/white-guy"
  ],
  "unresolved": [
    "Validator-domain control",
    "Canonical operator identity",
    "Entity type",
    "Official website",
    "Official X handle",
    "Incorporation jurisdiction",
    "Principal operating regions",
    "Organization-level activities",
    "Headcount"
  ]
}
```