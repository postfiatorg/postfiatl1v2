# Validator Identity Packet

## Packet Status

**SHADOW_ONLY.** Researched at **2026-09-01T20:20:29Z**. This packet contains external identity evidence and is not consensus data or a validator-authorization determination.

## Validator Coordinates

- **Network:** PostFiat testnet current published UNL (completed round 20)
- **Master public key:** `nHDfSHLutR7QqttAkx3AiYG2bukCdfAa3P9hrYL4yNJZQpvEpW1A`
- **Claimed domain:** `postfiat.live`
- **Frozen domain-verification status:** `true` in the supplied upstream input; not independently re-verified in this research
- **Validator-list publishers containing the key:** `postfiat-round-20`
- **Metadata source:** [Round 20 model request](https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json)

## Claimed Domain and Official URLs

**Domain conclusion:** `postfiat.live` is the claimed validator domain recorded in the supplied frozen round-20 input. The input reports verification as true, but the network-specific attestation was not independently retrieved or checked during this research.

**Official entity URL:** Not established. The claimed URL, [https://postfiat.live/](https://postfiat.live/), was not accessible through the research interface, so its content and operator identity could not be inspected. A secondary DNS/hosting index associates the domain with `194.164.64.20`, an address carrying numerous unrelated domains in a Hostinger allocation; this is hosting evidence only and does not identify the operator. [Hurricane Electric BGP Toolkit](https://bgp.he.net/net/194.164.64.0/21)

## Public Identity

- **Canonical public entity name:** Not established.
- **Entity type:** Not established.
- **Supported aliases:** None established.
- **Identity connection:** The only supported connection between the exact validator key and `postfiat.live` is the supplied frozen round-20 collector record. No accessible official site, filing, registry record, GitHub account, social profile, or other primary source was found that identifies the person or organization controlling both coordinates.
- **Excluded association:** The domain resembles the “Post Fiat” network name, whose official public site is [postfiat.org](https://postfiat.org/), but resemblance does not establish that the validator is operated by the Post Fiat organization or foundation.

## Business Summary

An identifiable legal or operating entity behind postfiat.live has not been established from accessible public institutional sources. The observable footprint is limited to the supplied frozen PostFiat round-20 metadata, which records the domain as associated with the specified testnet validator key and marks domain verification true, plus a public DNS-index record associating the domain with shared Hostinger infrastructure. Neither item establishes a canonical legal name, entity type, incorporation jurisdiction, principal operating base, products or services, customers, geographic reach, staffing, ownership, or control. The domain name resembles the Post Fiat network brand, but no accessible primary source was found that authorizes treating the validator operator as the Post Fiat organization. Accordingly, this packet classifies the operator's public profile as Unknown and treats validator-key control and organizational identity as unresolved.

## Public X Handle

**Not established.** The Post Fiat network maintains the public account [@PostFiatOrg](https://x.com/PostFiatOrg), also linked by the network’s [official website](https://postfiat.org/), but no primary source connects that account to the operator of this particular validator or `postfiat.live`.

## Region of Incorporation and Operations

- **Jurisdiction of incorporation:** Not established. **Confidence:** Low.
- **Principal operating regions:** Not established. **Confidence:** Low.

No incorporation filing or official legal page was located. Hosting-provider, IP-allocation, registrar, or domain-suffix information was not used to infer incorporation or operations.

## Activities

The frozen input associates the exact validator key and claimed domain with the completed PostFiat testnet round-20 list. That list membership is evidence of collection and publication, not by itself proof that an identified entity actively operates the node.

Post Fiat’s official setup documentation explains that validator operators can bind a domain to a validator and publish a network-specific attestation at `/.well-known/pft-ledger.toml`; however, the relevant attestation for `postfiat.live` was not independently accessed here. [Post Fiat Validator Setup](https://postfiat.org/validator-setup/) No other business or institutional activities are established.

## Estimated Public-Profile Size

**Tier: Unknown.** The accessible footprint consists of a claimed domain, a frozen validator/domain association, and secondary hosting metadata, without a supported entity identity, organizational presence, personnel listing, or institutional record. **Confidence:** Low. **Headcount:** Not established.

## Evidence

1. [Round 20 Model Request](https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json) — **Public scoring API input; accessed 2026-09-01.** This is the supplied upstream metadata source for the exact key, claimed domain, frozen verification value, and round publisher. The endpoint could not be rendered through the research interface, so those values remain frozen upstream evidence rather than an independent attestation check.
2. [postfiat.live](https://postfiat.live/) — **Claimed validator-domain website; access attempted 2026-09-01.** The site was not accessible through the research interface; no entity name, legal page, activity description, or social link could be inspected.
3. [Post Fiat Validator Setup](https://postfiat.org/validator-setup/) — **Primary network documentation; accessed 2026-09-01.** Describes validator-domain binding and publication of `/.well-known/pft-ledger.toml`, supporting the distinction between a frozen verification value and independently checked network-specific attestation.
4. [Post Fiat Whitepaper](https://postfiat.org/whitepaper/) — **Primary network publication; accessed 2026-09-01.** Describes the PostFiat testnet’s evidence-driven validator-list process and identifies domain/operator resolution as an imperfect research problem.
5. [Post Fiat](https://postfiat.org/) — **Official network website; accessed 2026-09-01.** Supports the canonical Post Fiat network identity and its public activities, but does not establish that `postfiat.live` or the specified key is controlled by that organization.
6. [Post Fiat on X](https://x.com/PostFiatOrg) — **Official network social profile; accessed 2026-09-01.** Establishes the network-level handle `@PostFiatOrg`, but provides no supported connection to this validator operator.
7. [194.164.64.0/21 — Hurricane Electric BGP Toolkit](https://bgp.he.net/net/194.164.64.0/21) — **Secondary DNS and network-infrastructure index; accessed 2026-09-01.** Associates `postfiat.live` with `194.164.64.20` in a Hostinger allocation alongside many unrelated domains. It does not establish ownership, jurisdiction, or operator identity.

## Uncertainty and Conflicts

- The frozen upstream value `domain_verification_status: true` was not independently reproduced by retrieving the validator manifest and `postfiat.live` attestation.
- Control of the domain, validator master key, and any underlying validator infrastructure remains unattributed to a public person or organization.
- No canonical legal name, entity type, incorporation record, operating base, personnel, ownership, or headcount was established.
- “PostFiat,” “Post Fiat,” and any foundation or project entity were excluded as aliases because the similar domain name alone does not prove common identity or control.
- `@PostFiatOrg` was excluded as the validator operator’s handle because only a network-level association is supported.
- Shared hosting, IP-allocation geography, registrar information, and the `.live` suffix were excluded as evidence of incorporation or operational location.
- No conflicting corporate names or filings were found; the principal conflict is between brand resemblance and the absence of an attributable primary-source connection.

## Machine-Readable Summary

```json
{
  "validator_id": "nHDfSHLutR7QqttAkx3AiYG2bukCdfAa3P9hrYL4yNJZQpvEpW1A",
  "network": "PostFiat testnet current published UNL (completed round 20)",
  "claimed_domain": "postfiat.live",
  "domain_verification_status": true,
  "canonical_entity": null,
  "entity_type": null,
  "aliases": [],
  "official_urls": [],
  "business_summary": "An identifiable legal or operating entity behind postfiat.live has not been established from accessible public institutional sources. The observable footprint is limited to the supplied frozen PostFiat round-20 metadata, which records the domain as associated with the specified testnet validator key and marks domain verification true, plus a public DNS-index record associating the domain with shared Hostinger infrastructure. Neither item establishes a canonical legal name, entity type, incorporation jurisdiction, principal operating base, products or services, customers, geographic reach, staffing, ownership, or control. The domain name resembles the Post Fiat network brand, but no accessible primary source was found that authorizes treating the validator operator as the Post Fiat organization. Accordingly, this packet classifies the operator's public profile as Unknown and treats validator-key control and organizational identity as unresolved.",
  "x_handle": null,
  "incorporation_region": null,
  "operating_regions": [],
  "profile_size_tier": "Unknown",
  "profile_size_confidence": "Low",
  "identity_confidence": "Low",
  "unresolved_fields": [
    "canonical_entity",
    "entity_type",
    "aliases",
    "official_entity_urls",
    "independent_domain_attestation",
    "x_handle",
    "incorporation_region",
    "operating_regions",
    "activities",
    "ownership_or_control",
    "headcount"
  ],
  "evidence_urls": [
    "https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json",
    "https://postfiat.live/",
    "https://postfiat.org/validator-setup/",
    "https://postfiat.org/whitepaper/",
    "https://postfiat.org/",
    "https://x.com/PostFiatOrg",
    "https://bgp.he.net/net/194.164.64.0/21"
  ]
}
```