# Validator Identity Packet

## Packet Status

**SHADOW_ONLY.** Researched at **2026-09-01T20:11:32Z**. This packet contains external identity evidence and is not XRP Ledger consensus data.

## Validator Coordinates

- **Network:** XRP Ledger mainnet
- **Validator master public key:** `nHUge3GFusbqmfYAJjxfKgm2j4JXGxrRsfYMcEViHrFSzQDdk5Hq`
- **Claimed domain:** `katczynski.net`
- **Frozen domain-verification status:** `null` — not independently established in the frozen input
- **Validator-list publishers containing the key:** ripple, xrpl_foundation
- **Upstream metadata source:** [XRPSCAN validator API](https://api.xrpscan.com/api/v1/validator)

## Claimed Domain and Official URLs

**Conclusion:** `katczynski.net` is the most likely official validator domain. Its landing page identifies itself as an “XRPL Validator” and publishes the exact supplied master public key, providing direct public evidence connecting the domain and key ([katczynski.net](https://katczynski.net/)).

**Most likely official URL:** [https://katczynski.net/](https://katczynski.net/)

The domain remains **claimed and not independently verified for this packet** because the required network-specific attestation was not successfully retrieved and checked. Bithomp currently labels the domain “Verified domain (TOML file),” but that is third-party evidence and does not change the required frozen status of `null` ([Bithomp validator record](https://bithomp.com/validator/nHUge3GFusbqmfYAJjxfKgm2j4JXGxrRsfYMcEViHrFSzQDdk5Hq)).

## Public Identity

- **Canonical public identity:** Christian Katczynski
- **Entity type:** Individual validator operator
- **Supported aliases:** None established. `katczynski.net` is a validator/domain label, not a separate entity alias.
- **Identity basis:** The official domain publishes the exact validator key ([katczynski.net](https://katczynski.net/)); Bithomp’s exact-key record identifies the operator as Christian Katczynski and associates that person with the same domain ([Bithomp](https://bithomp.com/validator/nHUge3GFusbqmfYAJjxfKgm2j4JXGxrRsfYMcEViHrFSzQDdk5Hq)). An XRPL Foundation repository announcement also includes the exact key among trusted validator keys ([XRPLF rippled discussion](https://github.com/XRPLF/rippled/discussions/5463)).
- **Confidence:** Medium. The key-to-domain connection is direct, while the personal name appears in public validator-registry evidence rather than on the domain landing page.

## Business Summary

Christian Katczynski is the most likely canonical public identity associated with katczynski.net, which presents itself as an XRP Ledger validator. The public footprint is consistent with an individual validator operator rather than a registered company or other institution; no incorporation record, separate legal entity, ownership structure, headcount, or commercial offering was established. Public validator records place the operator in Germany and show participation in XRP Ledger mainnet consensus, serving network participants and node operators through validation infrastructure. The activity has global network reach because XRPL is publicly accessible, while the observable organizational footprint remains individual in scale. No products or services beyond validator operation were established. The domain displays the supplied validator master key, but this research did not independently verify the network-specific domain attestation or prove current cryptographic control of that key.

## Public X Handle

**Not established.** Bithomp associates the operator record with [x.com/Katczynski](https://x.com/Katczynski), but the official validator landing page does not link that account, and the X page yielded no readable profile content during this research. Accordingly, `@Katczynski` is retained only as an unresolved candidate, not a confirmed official handle ([Bithomp](https://bithomp.com/validator/nHUge3GFusbqmfYAJjxfKgm2j4JXGxrRsfYMcEViHrFSzQDdk5Hq)).

## Region of Incorporation and Operations

- **Jurisdiction of incorporation:** Not established; effectively not applicable unless a separate legal entity is identified. **Confidence: High** that no incorporation jurisdiction can be assigned from the available evidence.
- **Principal operational region:** Germany. **Confidence: Medium.** Bithomp explicitly reports Germany as the operator country; this conclusion does not rely on its separate server-country field or on hosting location ([Bithomp](https://bithomp.com/validator/nHUge3GFusbqmfYAJjxfKgm2j4JXGxrRsfYMcEViHrFSzQDdk5Hq)).
- **Network reach:** Global, limited to the public reach of XRP Ledger validator participation rather than a demonstrated commercial operating footprint.

## Activities

The observable activity is operation of XRP Ledger mainnet validator infrastructure. The domain expressly describes itself as an XRPL validator and displays the exact key ([katczynski.net](https://katczynski.net/)). Bithomp reports current validation activity, manifest information, software version, amendment votes, and inclusion through `vl.ripple.com` for the same key ([Bithomp](https://bithomp.com/validator/nHUge3GFusbqmfYAJjxfKgm2j4JXGxrRsfYMcEViHrFSzQDdk5Hq)). XRPL Foundation technical guidance also lists the key among trusted validator keys ([XRPLF](https://github.com/XRPLF/rippled/discussions/5463)). No separate products, customers, commercial services, or institutional activities were established.

## Estimated Public-Profile Size

**Individual.** The public footprint identifies one person operating a validator-specific domain and does not establish a company, team, or broader institution ([Bithomp](https://bithomp.com/validator/nHUge3GFusbqmfYAJjxfKgm2j4JXGxrRsfYMcEViHrFSzQDdk5Hq); [katczynski.net](https://katczynski.net/)). **Confidence: Medium. Headcount is not established.**

## Evidence

1. [XRPL Validator — katczynski.net](https://katczynski.net/) — **Primary, operator-controlled website**; accessed 2026-09-01. Displays “XRPL Validator,” the claimed domain, and the exact supplied validator master public key.
2. [Christian Katczynski Validator — Bithomp](https://bithomp.com/validator/nHUge3GFusbqmfYAJjxfKgm2j4JXGxrRsfYMcEViHrFSzQDdk5Hq) — **Public XRP Ledger explorer and validator registry**; accessed 2026-09-01. Associates the exact key and domain with Christian Katczynski, reports operator country DE, validator telemetry, Ripple-list inclusion, a TOML-verification label, and an X-account link.
3. [Configuration Guidance for Using the New UNL — XRPLF rippled Discussion #5463](https://github.com/XRPLF/rippled/discussions/5463) — **Primary XRPL Foundation technical record**; accessed 2026-09-01. Includes the exact master key in a published trusted-validator-key example and documents Ripple and XRPL Foundation validator-list endpoints.
4. [@Katczynski on X](https://x.com/Katczynski) — **Candidate primary social-profile URL**; checked 2026-09-01. The URL resolved, but no readable profile content was available, so it does not independently establish the official handle.
5. [XRPSCAN Validator Info API documentation](https://docs.xrpscan.com/api-documentation/validator/validator-info) — **Primary API documentation**; accessed 2026-09-01. Documents the validator-key lookup endpoint used by the stated upstream metadata source. The key-specific endpoint was not successfully retrieved in this research session, so no live identity facts were taken from it.

## Uncertainty and Conflicts

- The frozen input requires `domain_verification_status: null`. Bithomp currently reports a verified TOML file, but the attestation was not independently retrieved and checked; therefore the frozen status remains unchanged.
- The domain directly connects itself to the exact key but does not name Christian Katczynski. The personal identity depends principally on Bithomp’s exact-key registry record.
- Current cryptographic control of the validator master key was not independently proven.
- No registered company, incorporation jurisdiction, ownership structure, personnel roster, headcount, or commercial business was established.
- `@Katczynski` is a plausible X account linked by Bithomp, but it was excluded as an official handle because adequate primary confirmation was unavailable.
- Germany is supported as declared operator country. Server location, IP geolocation, hosting provider, registrar information, and surname origin were excluded as bases for jurisdiction or operational-region conclusions.
- No separate current or historical aliases were supported.

## Machine-Readable Summary

```json
{
  "validator_id": "nHUge3GFusbqmfYAJjxfKgm2j4JXGxrRsfYMcEViHrFSzQDdk5Hq",
  "network": "XRP Ledger mainnet",
  "claimed_domain": "katczynski.net",
  "domain_verification_status": null,
  "canonical_entity": "Christian Katczynski",
  "entity_type": "Individual validator operator",
  "aliases": [],
  "official_urls": [
    "https://katczynski.net/"
  ],
  "business_summary": "Christian Katczynski is the most likely canonical public identity associated with katczynski.net, which presents itself as an XRP Ledger validator. The public footprint is consistent with an individual validator operator rather than a registered company or other institution; no incorporation record, separate legal entity, ownership structure, headcount, or commercial offering was established. Public validator records place the operator in Germany and show participation in XRP Ledger mainnet consensus, serving network participants and node operators through validation infrastructure. The activity has global network reach because XRPL is publicly accessible, while the observable organizational footprint remains individual in scale. No products or services beyond validator operation were established. The domain displays the supplied validator master key, but this research did not independently verify the network-specific domain attestation or prove current cryptographic control of that key.",
  "x_handle": null,
  "incorporation_region": null,
  "operating_regions": [
    "Germany"
  ],
  "profile_size_tier": "Individual",
  "profile_size_confidence": "Medium",
  "identity_confidence": "Medium",
  "unresolved_fields": [
    "Independent XRP Ledger domain attestation",
    "Current cryptographic control of the validator key",
    "Official X handle",
    "Separate legal entity and incorporation jurisdiction",
    "Headcount",
    "Commercial products or services"
  ],
  "evidence_urls": [
    "https://katczynski.net/",
    "https://bithomp.com/validator/nHUge3GFusbqmfYAJjxfKgm2j4JXGxrRsfYMcEViHrFSzQDdk5Hq",
    "https://github.com/XRPLF/rippled/discussions/5463",
    "https://x.com/Katczynski",
    "https://docs.xrpscan.com/api-documentation/validator/validator-info"
  ]
}
```