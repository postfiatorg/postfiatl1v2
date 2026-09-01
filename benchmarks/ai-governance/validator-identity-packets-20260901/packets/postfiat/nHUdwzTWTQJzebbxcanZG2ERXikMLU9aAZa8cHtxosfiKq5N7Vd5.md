# Validator Identity Packet

## Packet Status

**SHADOW_ONLY** — researched 2026-09-01T20:18:17Z. This packet contains external identity evidence and is not consensus data.

## Validator Coordinates

- **Network:** PostFiat testnet current published UNL (completed round 20)
- **Validator master public key:** `nHUdwzTWTQJzebbxcanZG2ERXikMLU9aAZa8cHtxosfiKq5N7Vd5`
- **Claimed domain:** `pft.whiteguy.eu`
- **Frozen domain-verification status:** `true` in the upstream input; not independently re-verified here
- **Validator-list publishers containing the key:** `postfiat-round-20`
- **Metadata source:** [Round 20 model request](https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json)

## Claimed Domain and Official URLs

The domain conclusion is **pft.whiteguy.eu (claimed)**. The supplied frozen input records a verification value of `true`, but this research did not independently retrieve and validate the network-specific attestation. Attempts to access the [claimed host](https://pft.whiteguy.eu/) and its expected [PostFiat domain-attestation path](https://pft.whiteguy.eu/.well-known/pft-ledger.toml) did not render accessible content in the research client. No official entity website was established.

## Public Identity

- **Canonical public entity name:** Not established.
- **Entity type:** Not established.
- **Supported current or historical aliases:** None established.
- **Identity connection:** The frozen upstream coordinates connect the validator key to the claimed domain, but they do not establish a named company, institution, or individual. An independent secondary validator board displays the [exact master key with no domain or entity identity](https://mikeyexplains.com/webpage_network_health_and_validator_board.php), which does not support attribution to a public entity.

## Business Summary

No legal or canonical public entity could be established for the supplied validator identity coordinates. The observable public footprint consists of a PostFiat testnet validator master public key, a claimed host at pft.whiteguy.eu, and inclusion by the postfiat-round-20 validator-list publisher. The frozen upstream input marks the domain-verification field true, but that network-specific attestation was not independently re-verified in this research. No supported entity type, incorporation jurisdiction, principal operating base, products or services, customer or stakeholder group, geographic reach, ownership, personnel, or headcount was found in accessible primary sources. Accordingly, this packet treats the operator as an unidentified validator participant rather than attaching the domain to a company, institution, or named individual. Its public institutional profile is Unknown, and no size tier based on personnel can be substantiated.

## Public X Handle

**Not established.** Neither accessible official-site material nor another strong primary source endorsed an X account for the claimed domain or validator key.

## Region of Incorporation and Operations

- **Incorporation jurisdiction:** Not established. **Confidence:** Low.
- **Principal operating region(s):** Not established. **Confidence:** Low.

The `.eu` suffix is not sufficient to identify a particular jurisdiction of incorporation or operating base. EURid permits `.eu` registrations by several categories of eligible organizations and individuals across the EU, Iceland, Liechtenstein, and Norway, including qualifying citizens regardless of residence; it therefore cannot support a narrower geographic attribution by itself ([EURid eligibility rules](https://eurid.eu/en/knowledge-centre/rules-for-eu-domains/)).

## Activities

**Not established at the entity level.** The only supported relationship is the frozen upstream association among the key, claimed domain, and round-20 publisher. PostFiat’s primary documentation describes validator-list publication as an evidence and scoring pipeline ([PostFiat whitepaper](https://postfiat.org/whitepaper/)), but list membership alone does not prove who controls the key, who operates the associated infrastructure, or what other activities that operator conducts.

## Estimated Public-Profile Size

**Unknown.** Evidence is limited to a validator key, a claimed subdomain, and frozen list metadata; no attributable organization, personnel page, filing, institutional publication history, or supported headcount was found. **Confidence:** Low. **Headcount established:** No.

## Evidence

1. [Round 20 model request](https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json) — **Source type:** supplied frozen upstream JSON endpoint. **Access date:** 2026-09-01. **Facts:** supplied as the source for the key/domain association, `true` verification field, and round-20 publisher membership. The endpoint did not render in the research client, so its contents were not independently inspected.
2. [pft.whiteguy.eu](https://pft.whiteguy.eu/) — **Source type:** claimed-domain HTTPS endpoint. **Access date:** 2026-09-01. **Facts:** checked as the claimed validator URL; it did not render accessible website content, so it supplied no entity name, legal information, activities, location, aliases, or social account.
3. [PostFiat domain-attestation path](https://pft.whiteguy.eu/.well-known/pft-ledger.toml) — **Source type:** expected network-specific domain-attestation endpoint. **Access date:** 2026-09-01. **Facts:** checked for independent domain/key attestation; it did not render in the research client, so the frozen `true` value remains unverified here.
4. [Post Fiat Whitepaper](https://postfiat.org/whitepaper/) — **Source type:** primary network documentation. **Access date:** 2026-09-01. **Facts:** supports the description of PostFiat’s public evidence, validator scoring, and signed validator-list publication model; it does not identify this validator’s operator.
5. [Post Fiat Validator Setup](https://postfiat.org/validator-setup/) — **Source type:** primary network operator documentation. **Access date:** 2026-09-01. **Facts:** documents the network’s validator-domain proof mechanism and expected `pft-ledger.toml` publication process; it does not establish that the specific attestation was successfully checked during this research.
6. [Network Health & Validator Intelligence](https://mikeyexplains.com/webpage_network_health_and_validator_board.php) — **Source type:** independent secondary validator board. **Access date:** 2026-09-01. **Facts:** displays the exact master public key with domain and location shown as unavailable; it provides no named operator and is not a PostFiat round-20 authority.
7. [EURid Rules for Domain Names](https://eurid.eu/en/knowledge-centre/rules-for-eu-domains/) — **Source type:** primary registry policy. **Access date:** 2026-09-01. **Facts:** establishes that `.eu` eligibility spans multiple organizational and individual categories and therefore does not prove a particular incorporation or operating jurisdiction.

## Uncertainty and Conflicts

- The frozen upstream input reports domain verification as `true`, but the attestation was not independently retrieved or validated in this research.
- The claimed host and expected `pft-ledger.toml` path did not render accessible content; current ownership, control, and content therefore remain unresolved.
- No legal name, canonical public entity, entity type, ownership, personnel, incorporation record, operating base, activities, official X account, or headcount was established.
- The exact-key secondary result reports no domain, whereas the frozen PostFiat input supplies `pft.whiteguy.eu`. This is a metadata difference across sources, not proof that either identity coordinate is false.
- The `.eu` suffix was excluded as evidence of a particular country of incorporation or operation.
- Similar names, the wording of the domain, hosting geography, registrar data, and unrelated organizations were excluded from alias and identity conclusions.
- Validator-list membership was not treated as proof of technical operation, key control, organizational status, or broader business activity.

## Machine-Readable Summary

```json
{
  "validator_id": "nHUdwzTWTQJzebbxcanZG2ERXikMLU9aAZa8cHtxosfiKq5N7Vd5",
  "network": "PostFiat testnet current published UNL (completed round 20)",
  "claimed_domain": "pft.whiteguy.eu",
  "domain_verification_status": true,
  "canonical_entity": null,
  "entity_type": null,
  "aliases": [],
  "official_urls": [],
  "business_summary": "No legal or canonical public entity could be established for the supplied validator identity coordinates. The observable public footprint consists of a PostFiat testnet validator master public key, a claimed host at pft.whiteguy.eu, and inclusion by the postfiat-round-20 validator-list publisher. The frozen upstream input marks the domain-verification field true, but that network-specific attestation was not independently re-verified in this research. No supported entity type, incorporation jurisdiction, principal operating base, products or services, customer or stakeholder group, geographic reach, ownership, personnel, or headcount was found in accessible primary sources. Accordingly, this packet treats the operator as an unidentified validator participant rather than attaching the domain to a company, institution, or named individual. Its public institutional profile is Unknown, and no size tier based on personnel can be substantiated.",
  "x_handle": null,
  "incorporation_region": null,
  "operating_regions": [],
  "profile_size_tier": "Unknown",
  "profile_size_confidence": "Low",
  "identity_confidence": "Low",
  "unresolved_fields": [
    "canonical entity",
    "entity type",
    "independent domain attestation",
    "domain ownership and control",
    "validator-key control",
    "official entity URL",
    "aliases",
    "official X handle",
    "incorporation jurisdiction",
    "principal operating regions",
    "entity-level activities",
    "ownership",
    "personnel",
    "headcount"
  ],
  "evidence_urls": [
    "https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json",
    "https://pft.whiteguy.eu/",
    "https://pft.whiteguy.eu/.well-known/pft-ledger.toml",
    "https://postfiat.org/whitepaper/",
    "https://postfiat.org/validator-setup/",
    "https://mikeyexplains.com/webpage_network_health_and_validator_board.php",
    "https://eurid.eu/en/knowledge-centre/rules-for-eu-domains/"
  ]
}
```