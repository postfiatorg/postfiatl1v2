# Validator Identity Packet

## Packet Status

**SHADOW_ONLY.** Researched at **2026-09-01T20:25:44Z**. This packet contains external public-identity evidence and research conclusions; it is not consensus data.

## Validator Coordinates

- **Network:** PostFiat testnet current published UNL (completed round 20)
- **Validator master public key:** `nHU74qX4tCQDSpE6zBS5PB3jybuGZJ7QMbeyLWDQRy3Lhb4DYDSR`
- **Claimed domain:** `validator.pftperry.com`
- **Frozen domain-verification status:** `true` in the supplied upstream input; not independently re-verified during this research
- **Validator-list publishers containing the key:** `postfiat-round-20`
- **Metadata source:** [PostFiat round-20 model request](https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json)

## Claimed Domain and Official URLs

**Domain conclusion:** `validator.pftperry.com` is the best-supported validator service domain. The supplied frozen PostFiat input reports verification as `true`, and a separate public validator dashboard pairs the exact domain with the exact master key. This is evidence of a domain-to-key association, not proof of a named entity’s ownership or control. [Network Health & Validator Intelligence](https://mikeyexplains.com/webpage_network_health_and_validator_board.php)

- **Most likely official validator URL:** [https://validator.pftperry.com/](https://validator.pftperry.com/) — claimed; frozen upstream verification `true`; not independently re-verified; page content was not accessible through the research client
- **Official institutional or corporate website:** Not established

## Public Identity

- **Canonical public entity name:** Not established
- **Entity type:** Not established
- **Supported aliases:** None established
- **Identity conclusion:** The exact key-domain pairing appears in a third-party XRP Ledger validator dashboard, corroborating the supplied coordinate pair, but neither that dashboard nor accessible primary sources identify a legal entity, organization, or person behind it. [Network Health & Validator Intelligence](https://mikeyexplains.com/webpage_network_health_and_validator_board.php)
- **Name caveat:** “pftperry” is only a string embedded in the domain. It is not treated as a legal name, canonical identity, or supported alias.

## Business Summary

An identifiable legal or canonical business entity behind validator.pftperry.com is not established from accessible public sources. The observable footprint consists of the claimed validator domain, the specified validator master public key, a frozen upstream PostFiat round-20 record reporting domain verification, and a third-party validator dashboard that pairs the same domain with the same key. These records indicate a validator-related technical presence, but they do not establish an incorporated entity, ownership, personnel, products, commercial services, customer base, principal office, or geographic operating reach. The string “pftperry” may be an operator label or pseudonymous identifier, but it is not treated as a supported legal name or alias. Accordingly, entity type, incorporation jurisdiction, operating base, headcount, and institutional scale remain unknown. Control of the validator key is not attributed to any named person or organization.

## Public X Handle

**Not established.** No official website, X profile, or other strong primary source located in exact-key, exact-domain, and exact-name searches supports an X handle for this validator operator.

## Region of Incorporation and Operations

- **Jurisdiction of incorporation:** Not established (**confidence: high that public evidence is insufficient**). No supported legal entity or company filing was identified.
- **Principal operating region(s):** Not established (**confidence: high that public evidence is insufficient**).
- **Excluded inference:** A third-party dashboard displays “United States” beside the validator, but this appears in a validator-location field and is not evidence of incorporation, principal office, or operator residence. It is therefore excluded from the identity conclusion. [Network Health & Validator Intelligence](https://mikeyexplains.com/webpage_network_health_and_validator_board.php)

## Activities

The observable footprint is limited to validator-related infrastructure: the exact domain and master key are paired on a third-party XRP Ledger validator dashboard, while the supplied frozen PostFiat round-20 input places the key in the current published PostFiat testnet UNL. [Network Health & Validator Intelligence](https://mikeyexplains.com/webpage_network_health_and_validator_board.php) PostFiat documentation describes validators as network servers that may publish a network-specific domain proof, but list membership alone does not establish who operates this validator, whether it remains independently operational, or whether the operator conducts any broader commercial activity. [PostFiat Validator Setup](https://postfiat.org/validator-setup/)

## Estimated Public-Profile Size

**Unknown** — **confidence: low**. Headcount is **not established**. No supported entity identity, personnel roster, institutional website, corporate filing, product portfolio, or other evidence permits assignment to the Individual, Micro, Small, Medium, Large, or Very large tiers. The public footprint found is limited to a validator domain-key pairing and frozen list metadata.

## Evidence

1. [PostFiat round-20 model request](https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json) — **Official PostFiat scoring API / frozen metadata source**; accessed 2026-09-01. This is the designated upstream source for the supplied round-20 coordinates. Direct content retrieval was unsuccessful, so its `true` verification value and list membership are treated strictly as supplied frozen input, not independently inspected facts.
2. [validator.pftperry.com](https://validator.pftperry.com/) — **Claimed validator endpoint**; checked 2026-09-01. The research client could not retrieve page content; the URL therefore supports no entity-name, ownership, activity, location, or social-account claim.
3. [Network Health & Validator Intelligence](https://mikeyexplains.com/webpage_network_health_and_validator_board.php) — **Third-party XRP Ledger validator dashboard**; accessed 2026-09-01. It pairs `validator.pftperry.com` with the exact master key `nHU74qX4tCQDSpE6zBS5PB3jybuGZJ7QMbeyLWDQRy3Lhb4DYDSR`. It does not identify the operator.
4. [PostFiat Validator Setup](https://postfiat.org/validator-setup/) — **Official network operator documentation**; accessed 2026-09-01. It explains the PostFiat testnet validator role and the network-specific public domain-proof mechanism using `.well-known/pft-ledger.toml`.
5. [Post Fiat Whitepaper](https://postfiat.org/whitepaper/) — **Official network publication**; accessed 2026-09-01. It documents PostFiat’s public validator-scoring and signed validator-list publication framework. It does not identify this validator’s operator.

## Uncertainty and Conflicts

- The legal or canonical operator identity, entity type, ownership, validator-key controller, personnel, incorporation jurisdiction, operating regions, headcount, and official institutional website remain unresolved.
- The frozen upstream `domain_verification_status: true` was not independently re-verified against a currently accessible network-specific attestation.
- The claimed validator endpoint did not yield accessible page content during this research.
- “PFT Perry,” “PFTPerry,” and “pftperry” were not adopted as aliases because no primary source establishes them as names of a person or organization.
- The third-party dashboard labels the key as outside its XRP Ledger UNL, while the supplied input places it in PostFiat’s round-20 published list. These refer to different networks or publishers and are not treated as a substantive conflict.
- The dashboard’s “United States” location was excluded because validator or server geolocation cannot establish incorporation or principal operations.
- No X account found through name similarity alone was attributed to the validator.

## Machine-Readable Summary

```json
{
  "validator_id": "nHU74qX4tCQDSpE6zBS5PB3jybuGZJ7QMbeyLWDQRy3Lhb4DYDSR",
  "network": "PostFiat testnet current published UNL (completed round 20)",
  "claimed_domain": "validator.pftperry.com",
  "domain_verification_status": true,
  "canonical_entity": null,
  "entity_type": null,
  "aliases": [],
  "official_urls": [
    "https://validator.pftperry.com/"
  ],
  "business_summary": "An identifiable legal or canonical business entity behind validator.pftperry.com is not established from accessible public sources. The observable footprint consists of the claimed validator domain, the specified validator master public key, a frozen upstream PostFiat round-20 record reporting domain verification, and a third-party validator dashboard that pairs the same domain with the same key. These records indicate a validator-related technical presence, but they do not establish an incorporated entity, ownership, personnel, products, commercial services, customer base, principal office, or geographic operating reach. The string “pftperry” may be an operator label or pseudonymous identifier, but it is not treated as a supported legal name or alias. Accordingly, entity type, incorporation jurisdiction, operating base, headcount, and institutional scale remain unknown. Control of the validator key is not attributed to any named person or organization.",
  "x_handle": null,
  "incorporation_region": null,
  "operating_regions": [],
  "profile_size_tier": "Unknown",
  "profile_size_confidence": "low",
  "identity_confidence": "low",
  "unresolved_fields": [
    "canonical_entity",
    "entity_type",
    "supported_aliases",
    "official_institutional_website",
    "public_x_handle",
    "incorporation_region",
    "operating_regions",
    "headcount",
    "validator_key_controller"
  ],
  "evidence_urls": [
    "https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json",
    "https://validator.pftperry.com/",
    "https://mikeyexplains.com/webpage_network_health_and_validator_board.php",
    "https://postfiat.org/validator-setup/",
    "https://postfiat.org/whitepaper/"
  ]
}
```