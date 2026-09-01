# Validator Identity Packet

## Packet Status

**SHADOW_ONLY.** Researched at **2026-09-01T20:21:38Z**. This packet contains external identity evidence and research conclusions; it is not consensus data or a validator-list determination.

## Validator Coordinates

- **Network:** PostFiat testnet current published UNL (completed round 20)
- **Master public key:** `nHUCEXpC5LhFAm1Mmf8TqrzVGt3QCuwWoW2V8PYynDpjZe8m8mHj`
- **Claimed domain:** `lc66validator.postfiatcn.org`
- **Frozen domain-verification status:** `true` in the supplied round-20 input; not independently re-verified
- **Validator-list publishers containing the key:** `postfiat-round-20`
- **Metadata source:** [Round 20 model request](https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json)

## Claimed Domain and Official URLs

**Domain conclusion:** `lc66validator.postfiatcn.org` is the claimed validator domain. The frozen upstream input records domain verification as true, while an independent public validator board also pairs the domain with the exact supplied master key. The network-specific attestation at the expected well-known path could not be retrieved and checked in this research session, so the frozen result is not independently verified here. [PostFiat’s validator documentation](https://postfiat.org/validator-setup/) specifies that verification should use a `/.well-known/pft-ledger.toml` document containing the validator public key and attestation. The [public validator board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php) corroborates the key-domain pairing.

**Official organizational URLs:** Not established. Neither the claimed subdomain nor `postfiatcn.org` was established as an official website for an identified public entity.

## Public Identity

- **Canonical public entity name:** Not established.
- **Entity type:** Not established.
- **Supported aliases:** None.
- **Identity connection:** The [public validator board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php) associates the exact key with `lc66validator.postfiatcn.org`, corroborating the supplied frozen coordinates. This supports a domain-to-validator-identity relationship but does not identify the operator, prove a legal entity, or establish ownership of the parent domain.
- **Excluded inference:** “PostFiatCN,” “LC66,” and similar constructions were not adopted as entity names or aliases because no primary source identifies them as such.

## Business Summary

The public entity behind lc66validator.postfiatcn.org is not established. The observable footprint is limited to a claimed PostFiat testnet validator subdomain and public validator records associating it with the supplied master public key. Frozen round-20 input reports the domain-verification field as true, but this research did not independently retrieve and validate the network-specific attestation, so control of the key and domain is not independently established here. No supported legal or canonical organization name, entity type, incorporation jurisdiction, principal operating base, products or services beyond the validator-facing footprint, customer or stakeholder group, geographic reach, headcount, or official social account was found. Accordingly, the operation cannot be assigned a business size tier beyond Unknown, and no inference is made from the domain label or reported server location.

## Public X Handle

**Not established.** No official website, X profile, or other strong primary source was found connecting an X handle to the claimed domain or validator key.

## Region of Incorporation and Operations

- **Jurisdiction of incorporation:** Not established. **Confidence:** No supported determination.
- **Principal operating regions:** Not established. **Confidence:** No supported determination.
- A [third-party validator board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php) displays Helsinki, Finland for the validator endpoint. This appears to be infrastructure-location data and is not evidence of incorporation, operator domicile, or principal business operations.

## Activities

No entity-level principal activity is established. The supported public footprint is validator-facing: the frozen input and a [third-party validator record](https://mikeyexplains.com/webpage_network_health_and_validator_board.php) associate the claimed domain with the supplied validator key. [PostFiat documentation](https://postfiat.org/validator-setup/) describes how operators bind domains to validator identities through a key-specific attestation. These facts support a relationship to a validator identity, but list membership and domain association alone do not prove who operates the node or establish broader commercial activities.

## Estimated Public-Profile Size

**Unknown.** The public footprint consists of a claimed validator subdomain and validator-directory entries, without an identified organization, personnel, legal record, or supported institutional operating profile. **Confidence:** High that Unknown is the appropriate rubric tier. **Headcount:** Not established.

## Evidence

1. [Round 20 model request](https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json) — **Source type:** supplied PostFiat scoring metadata endpoint. **Access date:** 2026-09-01. **Facts supported:** supplied source location for the round-20 key, claimed domain, publisher, and frozen `domain_verification_status=true`. The research browser returned an internal error, so its contents were not independently retrieved; these facts remain input-supplied coordinates.

2. [Network Health and Validator Board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php) — **Source type:** independent public validator directory. **Access date:** 2026-09-01. **Facts supported:** pairs `lc66validator.postfiatcn.org` with the exact supplied master key. It currently labels the entry non-UNL and displays Helsinki as its location, creating a scope or timing difference from the supplied round-20 PostFiat coordinate.

3. [Post Fiat Validator Setup](https://postfiat.org/validator-setup/) — **Source type:** official network documentation. **Access date:** 2026-09-01. **Facts supported:** explains domain binding and requires `https://<domain>/.well-known/pft-ledger.toml` to contain the validator public key and attestation; also distinguishes validator identity checks from infrastructure-location evidence.

4. [Post Fiat Whitepaper](https://postfiat.org/whitepaper/) — **Source type:** official network specification and publication description. **Access date:** 2026-09-01. **Facts supported:** describes PostFiat as an XRPL-derived testnet with published scoring rounds, evidence artifacts, and signed validator-list publication. Its displayed historical examples cover earlier rounds and do not independently establish this validator’s round-20 identity.

5. [Claimed validator URL](https://lc66validator.postfiatcn.org/) and [expected attestation URL](https://lc66validator.postfiatcn.org/.well-known/pft-ledger.toml) — **Source type:** claimed first-party endpoints. **Access date:** 2026-09-01. **Facts supported:** none independently; content could not be retrieved by the research browser. They are listed as the strongest direct URLs checked.

## Uncertainty and Conflicts

- The operator’s canonical name, legal form, ownership, personnel, incorporation jurisdiction, operating base, activities beyond the validator-facing footprint, and headcount remain unresolved.
- The frozen `domain_verification_status=true` is upstream evidence, not an attestation independently checked during this research.
- The third-party validator board corroborates the key-domain pairing but currently labels the entry non-UNL, whereas the supplied coordinate places it in `postfiat-round-20`. This may reflect different networks, publishers, or observation times and is not resolved.
- The board’s Helsinki label was excluded from incorporation and operating-region conclusions because infrastructure geolocation does not establish either.
- The parent-domain string suggests possible geographic or community branding, but no jurisdiction, organization name, or alias was inferred from it.
- No official X account was established.
- No claim is made that list membership alone proves active technical operation or control by any identified entity.

## Machine-Readable Summary

```json
{
  "validator_id": "nHUCEXpC5LhFAm1Mmf8TqrzVGt3QCuwWoW2V8PYynDpjZe8m8mHj",
  "network": "PostFiat testnet current published UNL (completed round 20)",
  "claimed_domain": "lc66validator.postfiatcn.org",
  "domain_verification_status": true,
  "canonical_entity": null,
  "entity_type": null,
  "aliases": [],
  "official_urls": [],
  "business_summary": "The public entity behind lc66validator.postfiatcn.org is not established. The observable footprint is limited to a claimed PostFiat testnet validator subdomain and public validator records associating it with the supplied master public key. Frozen round-20 input reports the domain-verification field as true, but this research did not independently retrieve and validate the network-specific attestation, so control of the key and domain is not independently established here. No supported legal or canonical organization name, entity type, incorporation jurisdiction, principal operating base, products or services beyond the validator-facing footprint, customer or stakeholder group, geographic reach, headcount, or official social account was found. Accordingly, the operation cannot be assigned a business size tier beyond Unknown, and no inference is made from the domain label or reported server location.",
  "x_handle": null,
  "incorporation_region": null,
  "operating_regions": [],
  "profile_size_tier": "Unknown",
  "profile_size_confidence": "high",
  "identity_confidence": "low",
  "unresolved_fields": [
    "canonical entity name",
    "entity type",
    "operator identity",
    "independent domain-attestation verification",
    "official organizational URLs",
    "official X handle",
    "incorporation jurisdiction",
    "principal operating regions",
    "principal business activities",
    "ownership",
    "personnel",
    "headcount"
  ],
  "evidence_urls": [
    "https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json",
    "https://mikeyexplains.com/webpage_network_health_and_validator_board.php",
    "https://postfiat.org/validator-setup/",
    "https://postfiat.org/whitepaper/",
    "https://lc66validator.postfiatcn.org/",
    "https://lc66validator.postfiatcn.org/.well-known/pft-ledger.toml"
  ]
}
```