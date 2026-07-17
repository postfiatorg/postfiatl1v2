# Shielded Asset Predicate Registry

PostFiat can add programmable shielded assets without becoming a general
private smart-contract chain. The v0 design is deliberately narrow:
transparent per-asset supply, private transfers, and a closed library of
owner-scoped Halo2 predicates admitted by Cobalt governance.

This is the useful institutional lane. Stablecoins, tokenized cash balances,
fund shares, vesting schedules, scoped assurance, and issuer-controlled mint or
burn rules mostly need typed asset behavior, not an AMM hidden inside the
privacy pool.

## Boundary

```text
Shielded asset note
  -> Cobalt-governed predicate registry root
  -> owner-scoped predicate proof
  -> public {root, nullifier, outputCommitments, fee, assetCommitment, roots}
  -> private flow, transparent supply map
```

The v0 registry does not admit arbitrary private programs. It also does not
admit bilateral DvP, private supply, AMM pools, order books, global counters,
or recipient-identity transfer policies. Those are different hardness classes,
not missing flags.

```mermaid
flowchart TD
  Proposal[Predicate proposal<br/>circuit content and declared scope]
  Hash[predicate_id = sha3_384(circuit content)]
  Registry[Cobalt-governed predicate registry root]
  Boundary[Boundary checks<br/>owner-scoped<br/>transparent supply<br/>no recipient identity leak<br/>no shared mutable state]
  Privacy[Privacy fixture<br/>same asset and policy<br/>same public artifact shape]
  Accept[Accept predicate<br/>registry transition]
  Reject[Reject predicate<br/>fail closed]

  Proposal --> Hash --> Registry --> Boundary
  Boundary -->|passes| Privacy --> Accept
  Boundary -->|violates v0 boundary| Reject
  Privacy -->|leaks metadata| Reject
```

## Why DvP Is Deferred

Bilateral DvP is the first serious v1/v2 target, but it is not owner-scoped.
It binds two owners and two notes into one atomic release condition. That is
much easier than shared AMM state, but it still needs a session protocol,
state-root freshness rules, abort handling, and proof construction that cannot
strand either party.

The v0 registry keeps DvP out of the predicate vocabulary until that atomicity
story has its own fixture.

## Transfer Policy Rule

Transfer policy is the privacy danger zone. A max-supply rule is a static
asset fact. A counterparty rule is live conditional logic over recipient
identity.

For v0, transfer policies must be `owner_scoped_static`:

- no recipient identity requirement;
- no recipient attributes as public inputs;
- no jurisdiction, allowlist, or credential field in the public artifact;
- no policy branch that narrows the public anonymity set.

Compliance-gated transfer logic can exist later only as a named weaker privacy
tier with explicit anonymity-set tests. It is not part of this MVP.

## Supply Rule

V0 chooses transparent supply and private flow. Issuance and burn affect a
public per-asset supply map. Transfers stay shielded.

Private supply is deferred. It requires a stronger conservation proof because
there is no public counter for observers to sanity-check. That is a hard mode,
not a launch toggle.

## Predicate IDs

Predicate IDs are content-addressed:

```text
predicate_id = pred-<sha3_384(circuit_content)[0:32]>
```

The registry root commits to the sorted predicate list and registry version.
Reusing a predicate ID for different circuit contents fails verification.
Adding or replacing a predicate is a Cobalt-governed registry transition.

## Evidence Packet

The current packet is:

```text
postfiat-shielded-asset-predicate-registry-v1
```

Core files:

```text
docs/governance/agent/shielded_asset_predicate_registry_schema.json
docs/governance/agent/fixtures/shielded_asset_predicate_registry/valid_predicate_registry.json
scripts/shielded-asset-predicate-registry-verify
reports/shielded-asset-predicate-registry/20260529T000926Z/shielded-asset-predicate-registry-report.json
```

The valid fixture admits five owner-scoped predicate classes:

| Predicate | Scope | V0 role |
| --- | --- | --- |
| `asset_issuance` | owner-scoped | Transparent supply-map issuance. |
| `asset_transfer` | owner-scoped | Private flow between shielded notes. |
| `asset_burn` | owner-scoped | Transparent supply-map decrement. |
| `assurance_non_inclusion` | owner-scoped | Scoped policy/list assurance. |
| `vesting_lock` | owner-scoped | Single-owner release-height condition. |

## Rejection Fixtures

The verifier rejects the boundary violations that would turn this into a
general private-programming layer:

```text
invalid_arbitrary_zkvm.json
invalid_bilateral_dvp_v0.json
invalid_full_viewing_key_disclosure.json
invalid_private_supply.json
invalid_recipient_identity_transfer_policy.json
invalid_registry_root_mismatch.json
invalid_shared_mutable_amm_predicate.json
invalid_ungoverned_issuer_rule.json
```

These cases are not cosmetic. They are the actual boundaries that keep the MVP
inside the privacy model.

## Privacy Fixture

The packet includes a privacy-preservation fixture:

```text
same_asset_same_policy_transfers_have_no_distinguishing_public_artifact
```

It checks that two transfers of the same shielded asset under the same policy
publish the same public artifact shape and do not expose `asset_id`,
`predicate_id`, recipient attributes, jurisdiction, allowlist membership,
policy branch, owner, value, witness path, or viewing-key material.

This is not a full traffic-analysis proof. It is a first gate that prevents the
feature from quietly leaking the exact metadata most likely to partition the
anonymity set.

## Verification

```bash
scripts/shielded-asset-predicate-registry-verify --fixtures
scripts/shielded-asset-predicate-registry-verify --write-report
scripts/shielded-asset-predicate-registry-verify --verify-report
```

Latest report:

```text
reports/shielded-asset-predicate-registry/20260529T000926Z/shielded-asset-predicate-registry-report.json
sha3_384=624f91a0c6ff4ecfd8f4ab34872c2074f2944c69fa51ec6ba6e10d19f342c0b005d0cd9d04457dd98950c98833bb6dda
```

Root values:

```text
valid_packet_hash=ebc81d9149259a54f29ddaf7b733b7261bb52947edb09eb528c5908980f10babb3207d18551cde1bdae766cb8d1da6c1
valid_statement_hash=58fc3d2729ed6868a404c37bc3d5b4285a94f46012863e814c9cc8217c6daee452b33d562c0248c0996965573d97dcbe
valid_registry_root_hash=49abd466e5b01e1b461b12f214135465273fabd289c8700399c669f5e04b6e8d013875249aec9e826bfa7421b5c9ae80
```

## Roadmap

| Phase | Goal | Gate |
| --- | --- | --- |
| 0. Packet | Schema, valid fixture, invalid fixtures, privacy fixture, report. | `shielded-asset-predicate-registry-verify --fixtures` passes. |
| 1. Envelope binding | Bind asset commitments and predicate registry root into Orchard action envelopes. | Tamper tests reject root or asset commitment mismatch. |
| 2. Wallet proof | Produce a real owner-scoped asset-transfer predicate proof. | Verifier accepts valid transfer and rejects recipient-attribute leakage. |
| 3. Issuer supply map | Add transparent issuance and burn accounting. | Conservation tests reconcile public supply with mint/burn deltas. |
| 4. Assurance integration | Reuse assurance receipts for policy/list proofs over shielded assets. | Scoped receipt passes without full viewing-key export. |
| 5. DvP research | Design two-party atomicity and abort handling. | Separate bilateral fixture proves no partial release. |

The product claim is intentionally smaller than private smart contracts:
PostFiat learns from Aztec without becoming Aztec.
