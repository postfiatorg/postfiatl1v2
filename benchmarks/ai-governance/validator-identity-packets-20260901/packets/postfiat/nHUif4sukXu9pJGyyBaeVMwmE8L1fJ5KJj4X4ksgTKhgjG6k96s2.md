# Validator Identity Packet

## Packet Status

**SHADOW_ONLY.** Researched **2026-09-01T20:26:56Z**. This packet contains external identity evidence and is not consensus data.

## Validator Coordinates

- **Network:** PostFiat testnet current published UNL (completed round 20)
- **Validator master public key:** `nHUif4sukXu9pJGyyBaeVMwmE8L1fJ5KJj4X4ksgTKhgjG6k96s2`
- **Claimed domain:** `pfthaploid.com`
- **Frozen domain-verification status:** `true` in the frozen upstream input; not independently re-verified here
- **Validator-list publishers containing the key:** `postfiat-round-20`
- **Metadata source:** [Round 20 model request](https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json)

## Claimed Domain and Official URLs

**Domain conclusion:** `pfthaploid.com` is a claimed network identity coordinate. The frozen upstream input records domain verification as `true`, but the network-specific attestation was not independently retrieved or checked during this research.

A [third-party validator board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php) displays the exact domain and master-key pair. However, that association does not identify a legal entity or establish that the domain is an official public business website.

- **Claimed root URL:** [https://pfthaploid.com](https://pfthaploid.com) — access was attempted, but no page could be retrieved through the research interface; it is not represented as browsed.
- **Official entity URL:** Not established.

## Public Identity

- **Canonical public entity name:** Not established.
- **Entity type:** Not established.
- **Supported aliases:** None.
- **Connection evidence:** The supplied [frozen round-20 metadata source](https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json) and a [third-party validator-board record](https://mikeyexplains.com/webpage_network_health_and_validator_board.php) associate `pfthaploid.com` with the exact validator key. No accessible domain content, legal page, company filing, official repository, or institutional profile was found that identifies the operator.
- **Identity conclusion:** The domain-to-key association is supported as a limited validator identity coordinate, but the public entity behind it is not established.

## Business Summary

The public identity behind pfthaploid.com is not established. Its observable public footprint consists of a claimed domain associated with a PostFiat testnet validator master key in frozen round-20 input and a third-party validator-board entry that displays the same domain-key pair. No accessible official website content, legal notice, registry filing, organization profile, supported alias, social account, incorporation jurisdiction, principal operating base, product or service description, customer or stakeholder group, geographic reach, or workforce information was located. The domain therefore cannot be assigned a canonical legal or trading name, organizational form, operating geography, or business scale beyond an Unknown public-profile tier. The available evidence supports only a limited network-identity coordinate; it does not independently establish who controls the validator key or that any identified company, nonprofit, or individual operates it.

## Public X Handle

**Not established.** Searches for the exact domain, validator key, and `pfthaploid` label did not locate an X account supported by an official website, the profile itself, or another strong primary source.

## Region of Incorporation and Operations

- **Jurisdiction of incorporation:** Not established. **Confidence:** Low identity confidence; no identified entity or relevant filing was located.
- **Principal operating regions:** Not established. **Confidence:** Low.
- No jurisdiction or operating region was inferred from hosting, DNS, registrar information, language, or third-party geolocation.

## Activities

No entity-specific principal activities are established. The supported public footprint is limited to the reported association between `pfthaploid.com` and the validator key in the [frozen round-20 input](https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json), corroborated by a [third-party validator board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php).

PostFiat’s [validator setup documentation](https://postfiat.org/validator-setup/) explains that operators publish network-specific domain proof for validators. That documentation provides technical context only: round/list membership and a domain association do not, by themselves, establish the identity, business activities, or continuing technical control of this particular operator.

## Estimated Public-Profile Size

- **Tier:** Unknown
- **Evidence:** No established entity, personnel page, organizational profile, filing, product footprint, or supported institutional identity was located.
- **Confidence:** High that the available evidence is insufficient for a more specific tier; low confidence regarding the operator’s actual size.
- **Headcount established:** No.

## Evidence

1. [PostFiat round 20 model request](https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json) — **Upstream network metadata API; accessed 2026-09-01.** The research interface returned no page body. The validator coordinates and frozen `true` verification value are therefore recorded as supplied upstream evidence, not as independently re-verified page contents.
2. [Network Health and Validator Board](https://mikeyexplains.com/webpage_network_health_and_validator_board.php) — **Third-party validator registry/monitoring page; accessed 2026-09-01.** Displays `pfthaploid.com` beside the exact master key `nHUif4sukXu9pJGyyBaeVMwmE8L1fJ5KJj4X4ksgTKhgjG6k96s2`; it does not identify the operator.
3. [PostFiat Validator Setup](https://postfiat.org/validator-setup/) — **Official network documentation; accessed 2026-09-01.** Describes the PostFiat validator-domain proof mechanism and publication of a network-specific well-known attestation file.
4. [Post Fiat Whitepaper](https://postfiat.org/whitepaper/) — **Official network research and protocol publication; accessed 2026-09-01.** Describes PostFiat’s public validator-scoring rounds, evidence pipeline, and signed testnet validator-list publication model; it does not identify this operator.
5. [Post Fiat public website](https://postfiat.org/) — **Official network website; accessed 2026-09-01.** Identifies PostFiat as a public testnet and links its validator evidence, setup, benchmark, explorer, and source API; it supplies network context rather than entity identity for `pfthaploid.com`.
6. [PostFiat live validator API](https://vhs.testnet.postfiat.org/v1/network/validators/test) — **Official network API; checked 2026-09-01.** The endpoint was reachable as JSON, but its response body was not exposed by the research interface and therefore was not used to assert facts about this validator.
7. [pfthaploid.com](https://pfthaploid.com) — **Claimed domain; checked 2026-09-01.** No page could be retrieved through the research interface. No website content was used as evidence.

## Uncertainty and Conflicts

- The frozen upstream `true` value was not independently validated against a retrieved network-specific attestation.
- Control of the domain and control of the validator master key were not independently established.
- The domain’s website content could not be accessed, and public searches yielded no supported legal name, organization, individual operator, incorporation record, official repository, or social account.
- `pfthaploid` was excluded as an alias because it is only a string derived from the domain, not a supported public entity name.
- No similar institutional or personal names were treated as aliases.
- The third-party validator board labels the key as outside that board’s UNL, whereas the supplied coordinates place it in PostFiat round 20. These appear to concern different publisher or network contexts and do not establish a substantive contradiction.
- Incorporation, ownership, personnel, operating location, activities, headcount, and X handle remain unresolved.

## Machine-Readable Summary

```json
{
  "validator_id": "nHUif4sukXu9pJGyyBaeVMwmE8L1fJ5KJj4X4ksgTKhgjG6k96s2",
  "network": "PostFiat testnet current published UNL (completed round 20)",
  "claimed_domain": "pfthaploid.com",
  "domain_verification_status": true,
  "canonical_entity": null,
  "entity_type": null,
  "aliases": [],
  "official_urls": [],
  "business_summary": "The public identity behind pfthaploid.com is not established. Its observable public footprint consists of a claimed domain associated with a PostFiat testnet validator master key in frozen round-20 input and a third-party validator-board entry that displays the same domain-key pair. No accessible official website content, legal notice, registry filing, organization profile, supported alias, social account, incorporation jurisdiction, principal operating base, product or service description, customer or stakeholder group, geographic reach, or workforce information was located. The domain therefore cannot be assigned a canonical legal or trading name, organizational form, operating geography, or business scale beyond an Unknown public-profile tier. The available evidence supports only a limited network-identity coordinate; it does not independently establish who controls the validator key or that any identified company, nonprofit, or individual operates it.",
  "x_handle": null,
  "incorporation_region": null,
  "operating_regions": [],
  "profile_size_tier": "Unknown",
  "profile_size_confidence": "High that evidence is insufficient; actual operator size remains unknown",
  "identity_confidence": "Low",
  "unresolved_fields": [
    "canonical entity",
    "entity type",
    "domain and validator-key control",
    "independent network-specific domain attestation",
    "official entity URL",
    "aliases",
    "X handle",
    "incorporation jurisdiction",
    "principal operating regions",
    "principal activities",
    "personnel and headcount"
  ],
  "evidence_urls": [
    "https://scoring-testnet.postfiat.org/api/scoring/rounds/20/inputs/model_request.json",
    "https://mikeyexplains.com/webpage_network_health_and_validator_board.php",
    "https://postfiat.org/validator-setup/",
    "https://postfiat.org/whitepaper/",
    "https://postfiat.org/",
    "https://vhs.testnet.postfiat.org/v1/network/validators/test",
    "https://pfthaploid.com"
  ]
}
```