# Organization Profile

## Identity

Canonical name: **Not established.**

Entity type: **Not established.**

Supported aliases or former names: **None established.**

The supplied coordinates associate validator key `nHUatddiVB2GN6zHHCk1gtepjANF8BdjPKuVcN6jwG2JwBYPea3k` with the claimed domain [`local-maxi.github.io`](https://local-maxi.github.io/), but exact-key and exact-domain searches did not locate an indexed primary source naming its operator. Post Fiat’s official instructions require a validator’s domain to publish a matching key and attestation at `/.well-known/pft-ledger.toml`; the claimed domain’s proof file could not be retrieved during this research, so the domain-to-key relationship was not independently verified. [Post Fiat validator-domain documentation](https://postfiat.org/validator-setup/)

## Official Web Presence

- Official website: **Not established.** [`local-maxi.github.io`](https://local-maxi.github.io/) is the validator’s claimed domain, but accessible content identifying an organization was not found.
- Official X handle: **Not established.**
- Official GitHub account: **Not established.** The apparent account and repository URLs—[`github.com/local-maxi`](https://github.com/local-maxi) and [`github.com/local-maxi/local-maxi.github.io`](https://github.com/local-maxi/local-maxi.github.io)—did not yield accessible, indexed profiles during this research.

## Incorporation and Operations

- Incorporation jurisdiction: **Not established**; confidence: **none**. No filing, registry record, or legal page was found linking an incorporated entity to the key or claimed domain.
- Principal operating regions: **Not established**; confidence: **none**. No location was inferred from GitHub hosting, infrastructure, or the domain name.

## Activities

No organizational products, services, customers, or stakeholders were established. The supplied coordinates assert only a relationship to a Post Fiat validator. Post Fiat documentation confirms that validator operators can use a GitHub Pages domain to publish cryptographic domain proof, but it does not identify this operator or demonstrate ownership or control of the claimed domain. [Post Fiat Validator Setup](https://postfiat.org/validator-setup/)

## Public Footprint

**Unknown.** Headcount is not established. No accessible organization profile, personnel listing, legal record, official social account, or substantive website was found for the supplied coordinates. Confidence in this tier: **high**, as a classification of the currently observable footprint—not as evidence that the operator lacks a larger private or unlinked presence.

## Business Summary

The identity of the organization or person behind the supplied Post Fiat validator key is not established. The only observable public footprint supplied for attribution is the claimed GitHub Pages domain local-maxi.github.io and its asserted association with a Post Fiat validator. No accessible primary source was found that names a legal or public entity, characterizes its entity type, identifies an incorporation jurisdiction or principal operating base, describes products or services beyond the asserted validator role, defines customer or stakeholder groups, or establishes geographic reach. An official website separate from the claimed domain and an official X account are likewise not established. Because no reliable headcount or broader institutional evidence was located, the public footprint size tier is Unknown. The validator-domain connection remains unverified until the domain proof or another primary source can be accessed and matched to the supplied key.

## Evidence

1. [`https://local-maxi.github.io/`](https://local-maxi.github.io/) — “local-maxi.github.io,” claimed validator domain; operator-supplied public coordinate; accessed 2026-09-03. The research interface could not retrieve page content, so it supports no affirmative organizational identity claim.
2. [`https://local-maxi.github.io/.well-known/pft-ledger.toml`](https://local-maxi.github.io/.well-known/pft-ledger.toml) — expected validator-domain proof; primary operator-controlled endpoint; accessed 2026-09-03. Content could not be retrieved, so the supplied key could not be matched to a published attestation.
3. [`https://vhs.testnet.postfiat.org/v1/network/validators/test`](https://vhs.testnet.postfiat.org/v1/network/validators/test) — Post Fiat testnet validator API; primary network source; accessed 2026-09-03. Checked as the official network validator surface, but no retrievable record established the operator’s organizational identity.
4. [“Post Fiat Validator Setup”](https://postfiat.org/validator-setup/) — official network documentation; primary source; accessed 2026-09-03. Supports that validator operators may use `<owner>.github.io` and should publish the validator public key plus attestation at `/.well-known/pft-ledger.toml`.
5. [`https://github.com/local-maxi`](https://github.com/local-maxi) and [`https://github.com/local-maxi/local-maxi.github.io`](https://github.com/local-maxi/local-maxi.github.io) — apparent GitHub account and Pages-repository locations; potential primary sources; accessed 2026-09-03. Neither produced accessible, indexed identifying information in the research interface.
6. [Post Fiat Validator History Service route definition](https://github.com/ripple/validator-history-service/blob/main/src/api/routes/v1/index.ts) — upstream API implementation; primary technical source; accessed 2026-09-03. Supports the availability of public-key-specific and validator-list API routes, but does not identify this validator’s operator.

## Uncertainty

- The canonical operator name, entity type, legal existence, ownership, personnel, incorporation, operating location, and headcount remain unknown.
- Control of [`local-maxi.github.io`](https://local-maxi.github.io/) by the validator operator was not independently verified.
- No official X account, separate official website, products, services, customers, or stakeholder groups were established.
- “Local Maxi” was not adopted as an entity name or alias because it appears only in the claimed hostname and no primary source identifies it as such.
- Unrelated organizations and individuals with similar “LocalMax,” “Maxi,” or “local maxi” names were excluded.
- GitHub hosting, a server location, and domain syntax were not used to infer incorporation or operating geography.

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
  "activities": [],
  "profile_size_tier": "Unknown",
  "profile_size_confidence": "High",
  "identity_confidence": "Low",
  "business_summary": "The identity of the organization or person behind the supplied Post Fiat validator key is not established. The only observable public footprint supplied for attribution is the claimed GitHub Pages domain local-maxi.github.io and its asserted association with a Post Fiat validator. No accessible primary source was found that names a legal or public entity, characterizes its entity type, identifies an incorporation jurisdiction or principal operating base, describes products or services beyond the asserted validator role, defines customer or stakeholder groups, or establishes geographic reach. An official website separate from the claimed domain and an official X account are likewise not established. Because no reliable headcount or broader institutional evidence was located, the public footprint size tier is Unknown. The validator-domain connection remains unverified until the domain proof or another primary source can be accessed and matched to the supplied key.",
  "evidence_urls": [
    "https://local-maxi.github.io/",
    "https://local-maxi.github.io/.well-known/pft-ledger.toml",
    "https://vhs.testnet.postfiat.org/v1/network/validators/test",
    "https://postfiat.org/validator-setup/",
    "https://github.com/local-maxi",
    "https://github.com/local-maxi/local-maxi.github.io",
    "https://github.com/ripple/validator-history-service/blob/main/src/api/routes/v1/index.ts"
  ],
  "unresolved": [
    "Validator operator identity",
    "Control of the claimed domain",
    "Canonical or legal entity name",
    "Entity type",
    "Official website and X handle",
    "Incorporation jurisdiction",
    "Principal operating regions",
    "Activities and stakeholder groups",
    "Headcount and institutional scale"
  ]
}
```