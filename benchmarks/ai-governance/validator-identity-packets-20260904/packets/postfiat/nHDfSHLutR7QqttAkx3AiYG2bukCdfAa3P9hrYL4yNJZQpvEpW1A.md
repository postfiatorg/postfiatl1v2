# Organization Profile

## Identity

**Canonical name:** Not established.  
**Entity type:** Not established.  
**Aliases or former names:** None supported.

The supplied coordinates associate validator public key `nHDfSHLutR7QqttAkx3AiYG2bukCdfAa3P9hrYL4yNJZQpvEpW1A` with `postfiat.live`, but no accessible primary source identified the operator behind that domain or key. Post Fiat’s official documentation explains that an operator normally binds a validator to a controlled domain and publishes the public key and attestation at `/.well-known/pft-ledger.toml` ([Post Fiat Validator Setup](https://postfiat.org/validator-setup/)). The [claimed domain](https://postfiat.live/) and its [expected attestation file](https://postfiat.live/.well-known/pft-ledger.toml) could not be accessed through the research interface, so domain control and operator identity were not independently verified.

The similar wording of `postfiat.live` and “Post Fiat” is not sufficient to identify the operator as the Post Fiat project or any related legal entity.

## Official Web Presence

**Official website:** Not established. The domain [postfiat.live](https://postfiat.live/) is treated only as the validator-claimed domain; no accessible identity-bearing page established it as an organization’s official website.

**Official X handle:** Not established. Although the Post Fiat network’s official site links to its own social channels, no primary source connects those accounts—or any other X account—to this validator’s operator ([Post Fiat](https://postfiat.org/)).

## Incorporation and Operations

**Incorporation jurisdiction:** Not established. **Confidence:** High that the reviewed public evidence does not establish a jurisdiction; no conclusion is offered about the operator’s actual incorporation status.

**Principal operating regions:** Not established. **Confidence:** High that the reviewed sources do not establish an operating base. Hosting, IP, registrar, and domain-suffix information were excluded as evidence of incorporation or principal operations.

## Activities

The only supportable activity is a claimed association with a Post Fiat network validator. Post Fiat describes itself as an XRP-derived public-testnet network for capital markets and collective intelligence ([About Post Fiat](https://postfiat.org/about/)), and its validator guide describes the domain-binding and public-attestation process used by operators ([Post Fiat Validator Setup](https://postfiat.org/validator-setup/)). No accessible source established the operator’s other products, services, customers, stakeholders, personnel, or organizational activities. The relationship to the supplied validator remains claimed rather than independently confirmed because neither a readable validator record nor the expected domain proof was available.

## Public Footprint

**Unknown.** No identifiable company, institution, individual, or pseudonymous operator could be connected to the supplied key and domain. The claimed domain did not yield an accessible public identity surface, while the official network materials reviewed did not name its operator ([validator data endpoint](https://vhs.testnet.postfiat.org/v1/network/validators/test), [claimed domain](https://postfiat.live/)). **Confidence:** High for the Unknown classification. **Headcount:** Not established.

## Business Summary

The organization behind validator nHDfSHLutR7QqttAkx3AiYG2bukCdfAa3P9hrYL4yNJZQpvEpW1A is not established from the accessible public record. The supplied coordinates associate the validator with postfiat.live, but the domain’s website and expected domain-attestation file were not accessible during this research, and no primary source identified an operator, legal entity, individual, or pseudonymous handle. Accordingly, entity type, incorporation jurisdiction, principal operating base, products, services, customers, and geographic reach remain unknown. The only observable activity is a claimed relationship to a Post Fiat network validator; even that relationship could not be independently confirmed from a readable registry response or domain proof. The appropriate public-footprint tier is Unknown, and no headcount, personnel roster, social account, or institutional footprint is established.

## Evidence

1. [Post Fiat Validator Setup](https://postfiat.org/validator-setup/) — Official network documentation; accessed 2026-09-03. Explains that operators bind validators to domains they control and publish the validator public key and attestation at `/.well-known/pft-ledger.toml`.
2. [postfiat.live](https://postfiat.live/) — Validator-claimed domain; checked 2026-09-03. It was not accessible through the research interface and therefore supplied no organization name, legal notice, operator identity, or social account.
3. [postfiat.live domain-attestation path](https://postfiat.live/.well-known/pft-ledger.toml) — Expected primary domain proof; checked 2026-09-03. It was not accessible through the research interface, so the supplied key-to-domain relationship could not be independently confirmed.
4. [Post Fiat validator data endpoint](https://vhs.testnet.postfiat.org/v1/network/validators/test) — Official network API; accessed 2026-09-03. The endpoint was reachable, but its response was not rendered in readable form by the research interface and supplied no usable operator identity.
5. [About Post Fiat](https://postfiat.org/about/) — Official project page; accessed 2026-09-03. Establishes the network context and activities of Post Fiat, but does not identify the operator of the supplied validator.
6. [Identity Digital RDAP query for postfiat.live](https://rdap.identitydigital.services/rdap/domain/postfiat.live) — Authoritative registry-query URL; checked 2026-09-03. The response was not accessible through the research interface and therefore supports no registrant or incorporation claim.

## Uncertainty

The validator operator’s name, entity type, legal status, aliases, incorporation, operating base, personnel, activities beyond validator participation, official website, and X handle remain unresolved. The validator-key/domain pair was supplied as the starting coordinate but could not be independently validated through a readable domain attestation or registry record. The resemblance between `postfiat.live` and the Post Fiat project name was excluded as insufficient evidence of ownership or affiliation. Registrar, hosting, DNS, IP-geolocation, and shared-server observations were also excluded because they do not establish organizational identity, incorporation, or principal operations.

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
    "Claimed association with a Post Fiat network validator; operator identity and domain proof were not independently established."
  ],
  "profile_size_tier": "Unknown",
  "profile_size_confidence": "High",
  "identity_confidence": "Low",
  "business_summary": "The organization behind validator nHDfSHLutR7QqttAkx3AiYG2bukCdfAa3P9hrYL4yNJZQpvEpW1A is not established from the accessible public record. The supplied coordinates associate the validator with postfiat.live, but the domain’s website and expected domain-attestation file were not accessible during this research, and no primary source identified an operator, legal entity, individual, or pseudonymous handle. Accordingly, entity type, incorporation jurisdiction, principal operating base, products, services, customers, and geographic reach remain unknown. The only observable activity is a claimed relationship to a Post Fiat network validator; even that relationship could not be independently confirmed from a readable registry response or domain proof. The appropriate public-footprint tier is Unknown, and no headcount, personnel roster, social account, or institutional footprint is established.",
  "evidence_urls": [
    "https://postfiat.org/validator-setup/",
    "https://postfiat.live/",
    "https://postfiat.live/.well-known/pft-ledger.toml",
    "https://vhs.testnet.postfiat.org/v1/network/validators/test",
    "https://postfiat.org/about/",
    "https://rdap.identitydigital.services/rdap/domain/postfiat.live"
  ],
  "unresolved": [
    "Validator operator identity",
    "Independent verification of the validator-key-to-domain relationship",
    "Entity type and legal name",
    "Aliases or former names",
    "Official website and X handle",
    "Incorporation jurisdiction",
    "Principal operating regions",
    "Activities beyond claimed validator participation",
    "Headcount and institutional footprint"
  ]
}
```