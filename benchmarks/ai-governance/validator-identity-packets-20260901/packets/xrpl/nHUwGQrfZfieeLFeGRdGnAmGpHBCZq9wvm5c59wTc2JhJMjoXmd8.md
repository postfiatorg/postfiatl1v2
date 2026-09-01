# Validator Identity Packet

## Packet Status

**SHADOW_ONLY** — Researched at **2026-09-01T20:15:44Z**. This packet contains external public-identity evidence and is not XRP Ledger consensus data.

## Validator Coordinates

- **Network:** XRP Ledger mainnet
- **Validator master public key:** `nHUwGQrfZfieeLFeGRdGnAmGpHBCZq9wvm5c59wTc2JhJMjoXmd8`
- **Claimed domain:** `xrpgoat.com`
- **Frozen domain-verification status:** `null` — not independently established in the frozen input
- **Validator-list publishers containing the key:** `ripple`, `xrpl_foundation` (frozen upstream metadata)
- **Upstream metadata source:** [XRPSCAN Validator API](https://api.xrpscan.com/api/v1/validator)

## Claimed Domain and Official URLs

**Domain conclusion:** `xrpgoat.com` is the most likely official validator-operator domain. Its public home page describes itself as “XRPGoat’s Validator Node” and displays the exact supplied master public key alongside node telemetry, directly corroborating a domain-to-key association at the content level. [Official validator page](https://xrpgoat.com/)

**Official URL:** [https://xrpgoat.com/](https://xrpgoat.com/)

**Verification label:** **Claimed and independently corroborated, but not independently network-attestation verified.** XRPSCAN associates the same key with `xrpgoat.com`, and Bithomp labels the domain TOML-verified; however, the required two-way XRPL attestation was not independently inspected during this research, so the frozen machine-readable status remains `null`. [XRPSCAN validator record](https://xrpscan.com/validator/nHUwGQrfZfieeLFeGRdGnAmGpHBCZq9wvm5c59wTc2JhJMjoXmd8) [Bithomp validator registry](https://bithomp.com/validators)

## Public Identity

- **Canonical public identity:** **XRPGoat**, an operator moniker rather than an established legal name. The official domain calls the service “XRPGoat’s Validator Node” and publishes the exact validator key. [XRPGoat validator page](https://xrpgoat.com/)
- **Entity type:** Pseudonymous XRP Ledger validator operator; whether the operator is an individual, informal group, or incorporated organization is **not established**.
- **Supported aliases:** None established. `XrpGoat` is treated only as a capitalization variant used by the [Bithomp validator registry](https://bithomp.com/validators), not a separate alias.
- **Connection evidence:** The official domain displays the supplied key, while [XRPSCAN](https://xrpscan.com/validator/nHUwGQrfZfieeLFeGRdGnAmGpHBCZq9wvm5c59wTc2JhJMjoXmd8) independently records that key with the domain `xrpgoat.com`. No named person or legal entity was found in the reviewed primary public footprint.

## Business Summary

XRPGoat is the public moniker of a pseudonymous XRP Ledger validator operator whose legal or incorporated identity is not established. Its observable public footprint consists primarily of xrpgoat.com, a validator-status page that identifies the supplied master public key and displays node and ledger telemetry, plus listings in public XRPL validator registries. No supported incorporation jurisdiction, principal office, ownership, personnel, or broader commercial product portfolio was found. The operator’s principal observable service is participation in XRP Ledger mainnet validation and publication of basic operational status information for XRPL stakeholders, including node operators, list publishers, and network users. Public evidence suggests a micro-scale, narrowly focused technical operation rather than an established institution, but headcount is not established and the operator’s geographic reach cannot be determined. Control of the validator key is not independently attributed to any named person or legal entity.

## Public X Handle

**Not established.** A candidate account, `@XrpGoat`, appears in third-party captures discussing XRPL node upgrades, but the official validator website does not link it and the primary X profile could not be accessed for verification. The similarly named `@Xrp_Goat` is a distinct account and was excluded. [Third-party capture mentioning @XrpGoat](https://www.twstalker.com/xrplvision)

## Region of Incorporation and Operations

- **Jurisdiction of incorporation:** **Not established** — confidence: **none**. No legal page, company filing, or authoritative public registry was found connecting XRPGoat or `xrpgoat.com` to an incorporated entity.
- **Principal operating regions:** **Not established** — confidence: **low**. Bithomp labels the owner country and server country as the United States, but these third-party fields do not establish incorporation, residence, or a principal operating base and are therefore not adopted as identity facts. [Bithomp validator registry](https://bithomp.com/validators)

## Activities

The supported public activity is operation of an XRP Ledger mainnet validator and publication of a basic status page. The official page displays the exact validator master key and reports validator-node telemetry, including a proposing server state and validated-ledger data. [XRPGoat validator page](https://xrpgoat.com/) XRPL documentation explains that validators connect to peers, relay signed transactions, maintain ledger state, and issue validation messages; this describes the network role without establishing any broader business activity by XRPGoat. [XRPL validator documentation](https://xrpl.org/docs/infrastructure/configuration/server-modes/run-xrpld-as-a-validator)

## Estimated Public-Profile Size

**Micro** — evidence: a narrowly scoped validator-status website, a pseudonymous operator moniker, and validator-registry entries, with no established legal organization, personnel page, broader product portfolio, or substantial institutional footprint. [Official site](https://xrpgoat.com/) [XRPSCAN record](https://xrpscan.com/validator/nHUwGQrfZfieeLFeGRdGnAmGpHBCZq9wvm5c59wTc2JhJMjoXmd8) **Confidence:** low. **Headcount:** not established.

## Evidence

1. [XrpGoat: Validate This](https://xrpgoat.com/) — official operator website; accessed 2026-09-01. Displays “XRPGoat’s Validator Node,” the exact supplied master public key, and validator/ledger telemetry.
2. [XRPSCAN validator record](https://xrpscan.com/validator/nHUwGQrfZfieeLFeGRdGnAmGpHBCZq9wvm5c59wTc2JhJMjoXmd8) — public XRPL validator registry; accessed 2026-09-01. Associates the exact key with `xrpgoat.com`, XRP Ledger mainnet, and validator history.
3. [Bithomp Validators](https://bithomp.com/validators) — independent XRPL explorer/registry; accessed 2026-09-01. Associates the exact key with the name `XrpGoat` and `xrpgoat.com`, and reports TOML verification and UNL inclusion. Its geographic fields and verification result were not treated as independently proven primary facts.
4. [Run xrpld as a Validator](https://xrpl.org/docs/infrastructure/configuration/server-modes/run-xrpld-as-a-validator) — official XRP Ledger technical documentation; accessed 2026-09-01. Defines validator activity and explains that strong domain verification requires a two-way domain/key link.
5. [Ripple validator-list endpoint](https://vl.ripple.com/) — official signed-list endpoint; accessed 2026-09-01. Confirms the public Ripple publisher endpoint exists; the statement that it contains the supplied key comes from the frozen upstream coordinates because the signed payload was not independently decoded here.
6. [XRPL Foundation validator-list endpoint](https://unl.xrplf.org/) — official signed-list endpoint; accessed 2026-09-01. Confirms the public Foundation publisher endpoint exists; the statement that it contains the supplied key comes from the frozen upstream coordinates because the signed payload was not independently decoded here.
7. [XRPSCAN Validator API](https://api.xrpscan.com/api/v1/validator) — upstream metadata source named in the frozen input; checked 2026-09-01. A usable per-key API response was not retrieved during this research, so no additional identity fact relies solely on it.
8. [xrpl.vision validator list](https://xrpl.vision/) — independent community validator-list publication; accessed 2026-09-01. Lists `xrpgoat.com` among its public validator domains; its country flag was not used to infer incorporation or operations.

## Uncertainty and Conflicts

- The underlying person, organization, ownership, and validator-key controller are not publicly established.
- No incorporation jurisdiction, principal office, personnel, headcount, or broader commercial activities were established.
- The frozen upstream domain-verification value is `null`, while Bithomp currently describes `xrpgoat.com` as TOML-verified. Because the actual network-specific attestation was not independently inspected, the frozen `null` value is preserved.
- United States owner/server labels appear in third-party validator registries, but neither server geolocation nor an uncorroborated owner-country field establishes incorporation or principal operations.
- `@XrpGoat` is a plausible social-account candidate but lacks a verified primary link to the official domain or supplied key; it is therefore excluded.
- `@Xrp_Goat`, XRP-themed tokens, and other similarly named “Goat” projects were excluded as unrelated.
- `XRPGoat` and `XrpGoat` are capitalization variants, not evidence of separate historical aliases or entities.
- Membership in the Ripple and XRPL Foundation publisher lists is recorded from frozen upstream metadata and corroborating public registries; the signed publisher payloads were not independently decoded during this research.

## Machine-Readable Summary

```json
{
  "validator_id": "nHUwGQrfZfieeLFeGRdGnAmGpHBCZq9wvm5c59wTc2JhJMjoXmd8",
  "network": "XRP Ledger mainnet",
  "claimed_domain": "xrpgoat.com",
  "domain_verification_status": null,
  "canonical_entity": "XRPGoat",
  "entity_type": "Pseudonymous XRP Ledger validator operator",
  "aliases": [],
  "official_urls": [
    "https://xrpgoat.com/"
  ],
  "business_summary": "XRPGoat is the public moniker of a pseudonymous XRP Ledger validator operator whose legal or incorporated identity is not established. Its observable public footprint consists primarily of xrpgoat.com, a validator-status page that identifies the supplied master public key and displays node and ledger telemetry, plus listings in public XRPL validator registries. No supported incorporation jurisdiction, principal office, ownership, personnel, or broader commercial product portfolio was found. The operator’s principal observable service is participation in XRP Ledger mainnet validation and publication of basic operational status information for XRPL stakeholders, including node operators, list publishers, and network users. Public evidence suggests a micro-scale, narrowly focused technical operation rather than an established institution, but headcount is not established and the operator’s geographic reach cannot be determined. Control of the validator key is not independently attributed to any named person or legal entity.",
  "x_handle": null,
  "incorporation_region": null,
  "operating_regions": [],
  "profile_size_tier": "Micro",
  "profile_size_confidence": "low",
  "identity_confidence": "medium",
  "unresolved_fields": [
    "Underlying legal or personal identity",
    "Ownership and validator-key control",
    "Jurisdiction of incorporation",
    "Principal operating regions",
    "Personnel and headcount",
    "Official X handle",
    "Independent network-specific domain attestation"
  ],
  "evidence_urls": [
    "https://xrpgoat.com/",
    "https://xrpscan.com/validator/nHUwGQrfZfieeLFeGRdGnAmGpHBCZq9wvm5c59wTc2JhJMjoXmd8",
    "https://bithomp.com/validators",
    "https://xrpl.org/docs/infrastructure/configuration/server-modes/run-xrpld-as-a-validator",
    "https://vl.ripple.com/",
    "https://unl.xrplf.org/",
    "https://api.xrpscan.com/api/v1/validator",
    "https://xrpl.vision/"
  ]
}
```