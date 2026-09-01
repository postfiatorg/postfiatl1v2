# Validator Identity Packet

## Packet Status

**SHADOW_ONLY**

Research timestamp: **2026-09-01T20:21:50Z**

This packet contains external public-identity evidence and research conclusions. It is not consensus data and does not modify or authenticate the published validator list.

## Validator Coordinates

- **Network:** PostFiat testnet current published UNL (completed round 20)
- **Validator master public key:** `nHUatddiVB2GN6zHHCk1gtepjANF8BdjPKuVcN6jwG2JwBYPea3k`
- **Claimed domain:** `local-maxi.github.io`
- **Frozen domain-verification status:** `true` in the frozen upstream input; not independently re-verified here
- **Validator-list publishers containing the key:** `postfiat-round-20`
- **Metadata source:** [Round 20 model request](https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json)

## Claimed Domain and Official URLs

**Domain conclusion:** `local-maxi.github.io` is the claimed validator domain recorded with a `true` verification value in the frozen round-20 input. That value is frozen upstream evidence, not an independent verification performed for this packet.

The most likely validator-domain URL is [https://local-maxi.github.io/](https://local-maxi.github.io/). Neither the site root nor the expected [PostFiat attestation file](https://local-maxi.github.io/.well-known/pft-ledger.toml) yielded retrievable content through the research interface, so current control and content were not independently checked. PostFiat’s [validator setup documentation](https://postfiat.org/validator-setup/#publish-domain-attestation) specifies `/.well-known/pft-ledger.toml` as the network-specific proof location.

**Official entity URLs:** Not established.

## Public Identity

- **Canonical public entity name:** Not established.
- **Entity type:** Not established.
- **Supported aliases:** None.
- **Domain/key connection:** Limited to the association recorded in the [frozen round-20 metadata source](https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json). That source was not retrievable through the research interface, so its contents were not independently extracted or revalidated.
- **Identity evidence:** The hostname follows GitHub Pages’ `<owner>.github.io` convention, but GitHub documents that such an owner may be either a person or an organization; the hostname therefore does not establish an institution or legal identity. [GitHub Pages documentation](https://docs.github.com/en/pages/getting-started-with-github-pages/what-is-github-pages)

Exact-key and exact-domain searches did not produce a primary source identifying a person, company, foundation, or other institution controlling the validator.

## Business Summary

The public identity behind the claimed validator domain local-maxi.github.io is not established. No supported legal or canonical organization name, entity type, jurisdiction of incorporation, principal operating base, products or services, customer or stakeholder groups, geographic reach, ownership, personnel, or headcount was identified in the public sources checked. The observable footprint is limited to a GitHub Pages-style hostname supplied in the frozen round-20 metadata and its association there with a PostFiat testnet validator entry. That evidence does not establish that a registered business exists, identify the controller of the hosting account, or independently prove current control of the validator master key. Accordingly, the public institutional profile is classified as Unknown rather than as an individual or organization size tier, and no entity-level operating history or commercial activity is attributed.

## Public X Handle

**Not established.** No official website content, X profile, or other strong primary source was found connecting an X handle to the claimed domain or exact validator key.

## Region of Incorporation and Operations

- **Incorporation jurisdiction:** Not established. **Confidence:** Low; no canonical entity, company filing, public registry entry, or official legal page was identified.
- **Principal operating regions:** Not established. **Confidence:** Low; no supported operational address or geographic statement was found.

No jurisdiction was inferred from GitHub hosting, infrastructure, language, domain registration, or network participation.

## Activities

No entity-level principal activities are established. The frozen input associates the claimed domain and validator key with a PostFiat testnet validator-list publication, but list membership alone does not prove that an identified person or organization operates the validator or provide evidence of commercial activity.

PostFiat documentation describes the network-specific process by which an operator can bind a validator key to a domain and publish an attestation, but the relevant live attestation was not independently retrieved in this research session. [PostFiat Validator Setup](https://postfiat.org/validator-setup/#publish-domain-attestation)

## Estimated Public-Profile Size

- **Rubric tier:** Unknown
- **Evidence:** The public footprint located consists only of the supplied GitHub Pages-style domain and its frozen upstream validator association; no attributable organization, personnel roster, filings, operating history, or institutional publications were found.
- **Confidence:** High that **Unknown** is the appropriate evidence-based tier.
- **Headcount established:** No.

## Evidence

1. [Round 20 model request](https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json) — **Source type:** PostFiat upstream metadata endpoint. **Access date:** 2026-09-01. **Facts supported:** Supplied location of the frozen round-20 metadata. Access was attempted, but the research interface did not retrieve its contents; the supplied coordinates were therefore not independently extracted from it.

2. [local-maxi.github.io](https://local-maxi.github.io/) — **Source type:** Claimed validator-domain root. **Access date:** 2026-09-01. **Facts supported:** Identifies the URL corresponding to the supplied claimed domain. No page content was retrievable through the research interface, so it supports no entity name, activity, location, or social account.

3. [local-maxi.github.io PostFiat attestation path](https://local-maxi.github.io/.well-known/pft-ledger.toml) — **Source type:** Expected network-specific domain-attestation endpoint. **Access date:** 2026-09-01. **Facts supported:** Identifies the expected proof URL derived from PostFiat’s published setup convention. Its contents were not retrievable, so the key/domain attestation was not independently checked.

4. [PostFiat Validator Setup](https://postfiat.org/validator-setup/#publish-domain-attestation) — **Source type:** Official network documentation. **Access date:** 2026-09-01. **Facts supported:** Specifies that PostFiat operators publish the validator public key and attestation at `https://<domain>/.well-known/pft-ledger.toml`; also explains the GitHub Pages publication path.

5. [What is GitHub Pages?](https://docs.github.com/en/pages/getting-started-with-github-pages/what-is-github-pages) — **Source type:** Official GitHub documentation. **Access date:** 2026-09-01. **Facts supported:** Explains that `<owner>.github.io` is the default location for a user or organization site. It does not identify whether `local-maxi` is a person, organization, or legal entity.

6. [GitHub profile URL for `local-maxi`](https://github.com/local-maxi) — **Source type:** Potential primary account profile. **Access date:** 2026-09-01. **Facts supported:** Checked as the account URL implied by the GitHub Pages hostname. No attributable profile content was retrievable through the research interface.

7. [Post Fiat whitepaper](https://postfiat.org/whitepaper/) — **Source type:** Official network publication. **Access date:** 2026-09-01. **Facts supported:** Describes PostFiat’s public validator-scoring evidence pipeline and signed testnet validator-list publication model; it does not identify the controller of this validator.

## Uncertainty and Conflicts

- The frozen upstream `true` value was not independently re-verified against a live network-specific attestation.
- The claimed site, expected attestation file, upstream JSON, and implied GitHub profile did not provide retrievable identity content through the research interface.
- Current control of the claimed domain, hosting account, or validator master key is unresolved.
- No canonical or legal name, entity type, ownership, personnel, incorporation jurisdiction, operating region, headcount, official X handle, or commercial activity is established.
- A GitHub Pages hostname can belong to either a user or an organization and is not, by itself, a legal identity.
- Search results for similarly named “LocalMax” projects were excluded because no exact validator-key or claimed-domain connection supported treating them as aliases or related entities.
- No conflicting corporate names or registry records were found; the issue is absence of attributable identity evidence rather than competing identities.

## Machine-Readable Summary

```json
{
  "validator_id": "nHUatddiVB2GN6zHHCk1gtepjANF8BdjPKuVcN6jwG2JwBYPea3k",
  "network": "PostFiat testnet current published UNL (completed round 20)",
  "claimed_domain": "local-maxi.github.io",
  "domain_verification_status": true,
  "canonical_entity": null,
  "entity_type": null,
  "aliases": [],
  "official_urls": [],
  "business_summary": "The public identity behind the claimed validator domain local-maxi.github.io is not established. No supported legal or canonical organization name, entity type, jurisdiction of incorporation, principal operating base, products or services, customer or stakeholder groups, geographic reach, ownership, personnel, or headcount was identified in the public sources checked. The observable footprint is limited to a GitHub Pages-style hostname supplied in the frozen round-20 metadata and its association there with a PostFiat testnet validator entry. That evidence does not establish that a registered business exists, identify the controller of the hosting account, or independently prove current control of the validator master key. Accordingly, the public institutional profile is classified as Unknown rather than as an individual or organization size tier, and no entity-level operating history or commercial activity is attributed.",
  "x_handle": null,
  "incorporation_region": null,
  "operating_regions": [],
  "profile_size_tier": "Unknown",
  "profile_size_confidence": "High",
  "identity_confidence": "Low",
  "unresolved_fields": [
    "canonical entity name",
    "entity type",
    "current domain control",
    "current validator-key control",
    "live domain attestation",
    "official entity URLs",
    "official X handle",
    "incorporation jurisdiction",
    "principal operating regions",
    "principal activities",
    "ownership",
    "personnel",
    "headcount"
  ],
  "evidence_urls": [
    "https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json",
    "https://local-maxi.github.io/",
    "https://local-maxi.github.io/.well-known/pft-ledger.toml",
    "https://postfiat.org/validator-setup/#publish-domain-attestation",
    "https://docs.github.com/en/pages/getting-started-with-github-pages/what-is-github-pages",
    "https://github.com/local-maxi",
    "https://postfiat.org/whitepaper/"
  ]
}
```