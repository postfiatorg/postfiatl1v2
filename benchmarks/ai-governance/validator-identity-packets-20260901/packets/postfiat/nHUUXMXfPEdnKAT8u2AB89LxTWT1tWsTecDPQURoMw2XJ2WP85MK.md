# Validator Identity Packet

## Packet Status

**SHADOW_ONLY.** Researched at **2026-09-01T20:17:59Z**. This packet contains external identity evidence and research conclusions; it is not consensus data.

## Validator Coordinates

- **Network:** PostFiat testnet current published UNL (completed round 20)
- **Master public key:** `nHUUXMXfPEdnKAT8u2AB89LxTWT1tWsTecDPQURoMw2XJ2WP85MK`
- **Claimed domain:** `pft.akirax.xyz`
- **Frozen domain-verification status:** `true` in the frozen upstream input; not independently re-verified here
- **Validator-list publishers containing the key:** `postfiat-round-20`
- **Metadata source:** [Round 20 model-request input](https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json)

## Claimed Domain and Official URLs

- **Domain conclusion:** `pft.akirax.xyz` is the claimed validator domain. The supplied frozen input records verification as `true`, and a public validator-health page independently displays the exact domain beside the exact master key; however, this review did not retrieve and check the network-specific attestation itself. [Public validator-health board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php)
- **Claimed validator URL:** [https://pft.akirax.xyz/](https://pft.akirax.xyz/) — the research client could not access readable content from this endpoint.
- **Official entity website or other official URLs:** **Not established.** The claimed validator endpoint is not, by itself, evidence of an entity-level official website.

## Public Identity

- **Canonical public entity name:** Not established.
- **Entity type:** Not established.
- **Supported aliases:** None established.
- **Connection evidence:** The frozen round-20 coordinates associate the key with `pft.akirax.xyz`, while a public validator-health board displays the same exact key-domain pairing. Neither source identifies the person or organization controlling the domain or key. [Frozen metadata source](https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json), [public validator-health board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php)

## Business Summary

The public identity behind pft.akirax.xyz is not established. The observable footprint consists of a claimed validator-domain association for a PostFiat testnet master public key in frozen round-20 input and a matching key-domain pairing on a public validator-health page. No accessible official website, legal page, company filing, public registry record, or authoritative social profile was found that names an operator or organization. Accordingly, entity type, incorporation jurisdiction, principal operating base, products, services, customer or stakeholder groups, geographic reach, ownership, and headcount remain unknown. The footprint appears limited to a technical domain label associated with validator infrastructure, but list inclusion and third-party telemetry do not by themselves establish who controls the key, who operates the server, or whether any incorporated business exists.

## Public X Handle

**Not established.** No official website, authoritative profile, or other strong primary source was found connecting an X account to the claimed domain or validator key.

## Region of Incorporation and Operations

- **Jurisdiction of incorporation:** Not established. **Confidence:** Low.
- **Principal operating region(s):** Not established. **Confidence:** Low.
- A third-party validator-health page reports Nuremberg, Germany, as a location associated with the endpoint, but this is treated only as infrastructure telemetry and not as evidence of incorporation or principal operations. [Public validator-health board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php)

## Activities

Entity-level activities are **not established**. The observable footprint is limited to an asserted relationship between the claimed domain and a validator public key. PostFiat’s official setup documentation describes domain binding and publication of a network-specific proof as validator-identity mechanisms, but the specific attestation for this key was not independently retrieved during this review. [PostFiat Validator Setup](https://postfiat.org/validator-setup/) List membership or telemetry alone does not establish who technically operates or controls the validator.

## Estimated Public-Profile Size

**Tier: Unknown.** The public record reviewed exposes a validator key-domain association but no identified organization, personnel, legal record, products, or established institutional presence. **Confidence:** High that the available evidence is insufficient for a larger-profile classification. **Headcount:** Not established.

## Evidence

1. [Round 20 model-request input](https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json) — **PostFiat scoring API / frozen primary metadata; accessed 2026-09-01.** Supplied as the source for the round-20 key, claimed domain, verification flag, and publisher coordinates. Direct rendering did not return readable content during this review, so its additional contents were not independently confirmed.
2. [Validator Health and Validator Board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php) — **Third-party validator telemetry; accessed 2026-09-01.** Displays `pft.akirax.xyz` beside the exact master key and reports Nuremberg as endpoint-location telemetry. It does not identify an operator or establish PostFiat round-20 list status.
3. [PostFiat Validator Setup](https://postfiat.org/validator-setup/) — **Official network documentation; accessed 2026-09-01.** Explains that validators set a domain and publish a network-specific domain proof, supporting the distinction between a checked attestation and an unverified input flag.
4. [Post Fiat Whitepaper](https://postfiat.org/whitepaper/) — **Official technical publication; accessed 2026-09-01.** Describes PostFiat’s public validator-list scoring, evidence collection, frozen snapshots, and publication-artifact architecture.
5. [Claimed validator endpoint](https://pft.akirax.xyz/) — **Claimed primary endpoint; access attempted 2026-09-01.** No readable page content was retrievable through the research client, so it supplied no operator name, legal identity, activity description, or social link.
6. [RDAP lookup for akirax.xyz](https://rdap.org/domain/akirax.xyz) — **Public registration-data lookup endpoint; access attempted 2026-09-01.** No readable result was retrievable through the research client; no registrant identity or jurisdiction was therefore attributed from RDAP.

## Uncertainty and Conflicts

- The upstream `true` verification value is frozen evidence, not an attestation independently checked in this research session.
- The claimed domain could not be connected to a named individual, company, foundation, or other legal entity.
- Control of the validator master key, domain, and server was not independently established.
- The third-party board’s Nuremberg location was excluded from incorporation and operating-region conclusions because infrastructure location does not establish either.
- The third-party board labels the key as non-UNL in its own surveyed context; this does not resolve or contradict the supplied PostFiat round-20 coordinate because the board does not establish that it is reporting the same network or publisher list. [Public validator-health board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php)
- Search results for similarly named subjects—including [AKIRAX Co., Ltd.](https://www.akirax.co.jp/), [AKIRAX Private Limited](https://www.zaubacorp.com/AKIRAX-PRIVATE-LIMITED-U62013HR2026PTC144386), and the [Akirax travel profile](https://akirax.hatenablog.jp/about)—contained no evidence linking them to `akirax.xyz`, `pft.akirax.xyz`, or the validator key and were excluded as name collisions.
- Incorporation, ownership, personnel, headcount, aliases, official X handle, business activities, and principal operating regions remain unresolved.

## Machine-Readable Summary

```json
{
  "validator_id": "nHUUXMXfPEdnKAT8u2AB89LxTWT1tWsTecDPQURoMw2XJ2WP85MK",
  "network": "PostFiat testnet current published UNL (completed round 20)",
  "claimed_domain": "pft.akirax.xyz",
  "domain_verification_status": true,
  "canonical_entity": null,
  "entity_type": null,
  "aliases": [],
  "official_urls": [],
  "business_summary": "The public identity behind pft.akirax.xyz is not established. The observable footprint consists of a claimed validator-domain association for a PostFiat testnet master public key in frozen round-20 input and a matching key-domain pairing on a public validator-health page. No accessible official website, legal page, company filing, public registry record, or authoritative social profile was found that names an operator or organization. Accordingly, entity type, incorporation jurisdiction, principal operating base, products, services, customer or stakeholder groups, geographic reach, ownership, and headcount remain unknown. The footprint appears limited to a technical domain label associated with validator infrastructure, but list inclusion and third-party telemetry do not by themselves establish who controls the key, who operates the server, or whether any incorporated business exists.",
  "x_handle": null,
  "incorporation_region": null,
  "operating_regions": [],
  "profile_size_tier": "Unknown",
  "profile_size_confidence": "high",
  "identity_confidence": "low",
  "unresolved_fields": [
    "canonical entity",
    "entity type",
    "validator key control",
    "domain control",
    "incorporation jurisdiction",
    "principal operating regions",
    "activities",
    "aliases",
    "official URLs",
    "official X handle",
    "ownership",
    "personnel",
    "headcount",
    "independent domain-attestation verification"
  ],
  "evidence_urls": [
    "https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json",
    "https://mikeyexplains.com/webpage_network_health_and_validator_board.php",
    "https://postfiat.org/validator-setup/",
    "https://postfiat.org/whitepaper/",
    "https://pft.akirax.xyz/",
    "https://rdap.org/domain/akirax.xyz"
  ]
}
```