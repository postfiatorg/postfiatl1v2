# Validator Identity Packet

## Packet Status

**SHADOW_ONLY** — Researched at **2026-09-01T20:20:13Z**. This packet contains external public-identity evidence and is not consensus data, a validator-list decision, or a legitimacy assessment.

## Validator Coordinates

- **Network:** PostFiat testnet current published UNL (completed round 20)
- **Validator master public key:** `nHDHCvsJi8UwHMAbkinuJhPYsG5ZR9rtmGAjJZwo8uibf8N1tz3e`
- **Claimed domain:** `pft.xbtseal.com`
- **Frozen domain-verification status:** `true` in the supplied upstream input; not independently re-verified here
- **Validator-list publishers containing the key:** `postfiat-round-20`
- **Metadata source:** [Round 20 model-request input](https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json). The endpoint was supplied with the coordinates but could not be directly retrieved during this research.

## Claimed Domain and Official URLs

The strongest supported domain conclusion is **`pft.xbtseal.com` as the claimed validator domain**. The exact domain and validator key also appear together on a public validator board, corroborating the pairing but not identifying an operator or legal entity ([validator board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php)).

- **Claimed validator-domain URL:** [https://pft.xbtseal.com](https://pft.xbtseal.com)
- **Domain status:** Claimed; frozen upstream verification status `true`; network-specific attestation not independently re-checked
- **Official entity website:** Not established
- **Parent domain:** [https://xbtseal.com](https://xbtseal.com) was checked, but no accessible institutional identity content was established
- **Expected PostFiat attestation path:** [https://pft.xbtseal.com/.well-known/pft-ledger.toml](https://pft.xbtseal.com/.well-known/pft-ledger.toml), following the network’s documented verification convention ([PostFiat validator setup guide](https://postfiat.org/validator-setup/)); its contents were not independently retrieved

## Public Identity

- **Canonical public entity name:** Not established
- **Entity type:** Not established
- **Supported aliases:** None
- **Identity connection:** The public evidence connects `pft.xbtseal.com` with the exact validator key, but it does not connect either coordinate to a named person, company, foundation, or other institution ([validator board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php)).
- **Excluded candidate label:** “XBTSeal” is only a string derived from the parent domain and is not treated as an entity name or alias without primary-source support.

## Business Summary

An identifiable legal or canonical business entity behind pft.xbtseal.com is not established from the public sources reviewed. The observable footprint consists of a claimed PostFiat testnet validator-domain pairing for master public key nHDHCvsJi8UwHMAbkinuJhPYsG5ZR9rtmGAjJZwo8uibf8N1tz3e, corroborated by a public validator board, and the supplied frozen round-20 metadata. The supplied input records domain verification as true, but that network-specific attestation was not independently re-checked in this research. No organization name, entity type, incorporation jurisdiction, principal operating base, products or services, customer or stakeholder classes, geographic reach, ownership, personnel, headcount, or official social-media account was established. Accordingly, the available footprint supports description only as an unidentified validator-domain presence, not as a documented operating enterprise of any particular size.

## Public X Handle

**Not established.** Searches for the exact domain label and validator key did not produce an official website, domain-controlled page, X profile, or other strong primary source supporting a particular handle. The unsubstantiated candidate URL [x.com/xbtseal](https://x.com/xbtseal) did not provide accessible identity content and is not assigned as the validator’s account.

## Region of Incorporation and Operations

- **Incorporation jurisdiction:** Not established — **low confidence** because no canonical entity or filing was identified.
- **Principal operating regions:** Not established — **low confidence**.
- A public validator board labels the node location as Helsinki, Finland, but this is treated only as possible infrastructure geolocation and is not evidence of incorporation or an operator’s principal region ([validator board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php)).

## Activities

No entity-level commercial or institutional activities are established. The observable footprint is limited to:

- Association of the exact validator key with the claimed domain `pft.xbtseal.com` on a public validator board ([validator board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php)).
- Inclusion in the supplied `postfiat-round-20` coordinates, whose cited metadata endpoint was not directly accessible during this research ([round 20 input](https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json)).
- A frozen upstream domain-verification value of `true`, which is not a substitute for independently checking the network-specific attestation. PostFiat documents that verification material should be published at `/.well-known/pft-ledger.toml` ([validator setup guide](https://postfiat.org/validator-setup/)).

List membership and domain pairing alone do not establish who operates the validator, whether the node is currently performing validation, or whether the operator conducts any other business.

## Estimated Public-Profile Size

- **Tier:** Unknown
- **Evidence:** No named organization, personnel, corporate filing, official institutional site, documented products, or established social account was found. The public footprint consists principally of the validator-key/domain pairing ([validator board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php)).
- **Confidence:** High confidence that **Unknown** is the appropriate rubric tier given the available evidence.
- **Headcount:** Not established.

## Evidence

1. [Round 20 model-request input](https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json) — **PostFiat scoring-service metadata endpoint**; access attempted **2026-09-01**. Supplied as the upstream source for the validator key, claimed domain, frozen verification value, and round publisher. Direct content retrieval was unsuccessful, so those facts remain supplied frozen coordinates rather than independently inspected findings.
2. [Network Health and Validator Board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php) — **Independent public validator registry/monitoring page**; accessed **2026-09-01**. Displays the exact key beside `pft.xbtseal.com`, reported agreement and uptime values, and a Helsinki location label. It does not identify an operator or legal entity.
3. [Claimed validator domain](https://pft.xbtseal.com) — **Domain-controlled candidate primary source**; access attempted **2026-09-01**. No accessible institutional identity content was retrieved.
4. [Parent domain](https://xbtseal.com) — **Candidate domain-controlled primary source**; access attempted **2026-09-01**. No accessible entity name, legal page, personnel, activity description, or social-account link was established.
5. [Claimed PostFiat attestation path](https://pft.xbtseal.com/.well-known/pft-ledger.toml) — **Candidate network-specific domain attestation**; searched **2026-09-01**. Its contents were not retrieved or independently validated.
6. [PostFiat Validator Setup](https://postfiat.org/validator-setup/) — **Official network documentation**; accessed **2026-09-01**. Establishes that PostFiat validators publish a public key and attestation at `https://<domain>/.well-known/pft-ledger.toml`.
7. [PostFiat project site](https://postfiat.org/) — **Official network website**; accessed **2026-09-01**. Supports that PostFiat has public validator evidence and validator-list publication infrastructure, but does not identify this validator’s operator.
8. [PostFiat Whitepaper](https://postfiat.org/whitepaper/) — **Official network publication**; accessed **2026-09-01**. Describes the public-testnet validator-list and scoring-artifact model; it does not connect the subject key to a named entity.
9. [Unsubstantiated X candidate](https://x.com/xbtseal) — **Social-profile URL checked**; accessed **2026-09-01**. No accessible content established a connection to the validator or domain.

## Uncertainty and Conflicts

- The operator’s legal or canonical identity, entity type, ownership, personnel, and control structure remain unresolved.
- The frozen upstream verification value is `true`, but the underlying network-specific attestation was not independently retrieved or checked.
- Neither `pft.xbtseal.com` nor `xbtseal.com` yielded accessible institutional identity content during this research.
- No aliases are supported. In particular, “XBTSeal” was not promoted from a domain label into an entity name.
- No official X handle was established; similarly named accounts or URL guesses were excluded.
- The Helsinki, Finland label on the validator board was excluded from incorporation and operating-region conclusions because infrastructure geolocation does not establish either.
- The validator board’s “UNL: No” field appears within that board’s own monitoring context and does not independently resolve or contradict the supplied `postfiat-round-20` membership coordinate.
- Current technical operation was not inferred from list membership, reported monitoring values, or domain association alone.
- Domain registration data, server location, registrar information, and naming resemblance were not used to infer jurisdiction or identity.

## Machine-Readable Summary

```json
{
  "validator_id": "nHDHCvsJi8UwHMAbkinuJhPYsG5ZR9rtmGAjJZwo8uibf8N1tz3e",
  "network": "PostFiat testnet current published UNL (completed round 20)",
  "claimed_domain": "pft.xbtseal.com",
  "domain_verification_status": true,
  "canonical_entity": null,
  "entity_type": null,
  "aliases": [],
  "official_urls": [],
  "business_summary": "An identifiable legal or canonical business entity behind pft.xbtseal.com is not established from the public sources reviewed. The observable footprint consists of a claimed PostFiat testnet validator-domain pairing for master public key nHDHCvsJi8UwHMAbkinuJhPYsG5ZR9rtmGAjJZwo8uibf8N1tz3e, corroborated by a public validator board, and the supplied frozen round-20 metadata. The supplied input records domain verification as true, but that network-specific attestation was not independently re-checked in this research. No organization name, entity type, incorporation jurisdiction, principal operating base, products or services, customer or stakeholder classes, geographic reach, ownership, personnel, headcount, or official social-media account was established. Accordingly, the available footprint supports description only as an unidentified validator-domain presence, not as a documented operating enterprise of any particular size.",
  "x_handle": null,
  "incorporation_region": null,
  "operating_regions": [],
  "profile_size_tier": "Unknown",
  "profile_size_confidence": "high",
  "identity_confidence": "low",
  "unresolved_fields": [
    "canonical entity",
    "entity type",
    "operator identity",
    "ownership and control",
    "aliases",
    "official entity URLs",
    "official X handle",
    "incorporation jurisdiction",
    "principal operating regions",
    "principal business activities",
    "personnel and headcount",
    "independent verification of the network-specific domain attestation",
    "current technical validator operation"
  ],
  "evidence_urls": [
    "https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json",
    "https://mikeyexplains.com/webpage_network_health_and_validator_board.php",
    "https://pft.xbtseal.com",
    "https://xbtseal.com",
    "https://pft.xbtseal.com/.well-known/pft-ledger.toml",
    "https://postfiat.org/validator-setup/",
    "https://postfiat.org/",
    "https://postfiat.org/whitepaper/",
    "https://x.com/xbtseal"
  ]
}
```