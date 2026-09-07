# YOLO Options Reserve Profile

**Status:** pre-production foundation; no asset registration, issuance, capital,
or trading authorization

The YOLO profile connects a privately held brokerage reserve observation to the
provider-neutral PostFiat reserve guest. It does not claim that a broker API
response is a cryptographic proof of custody. Both the quantity and valuation
dimensions use `TrustClassV1::Attested`; consequently all net value from this
source appears in `attested_value`, never `cryptographically_verified_value`.

## Adapter contract

The source-manifest entry uses:

- adapter kind `yolo-broker-attested-v1`;
- an account-owner commitment distinct from the TEE verifier commitment;
- independent quantity and valuation verifier commitments, even when the same
  enclave statement key is approved for both;
- `attested` quantity and valuation classes;
- explicit, governed freshness bounds and valuation policy; and
- adapter schema version `1`.

The private observation remains outside the proof witness. Its public disclosure
record commits to:

- broker source and account/application identity;
- observation time and the collection epoch;
- basket decision and execution-receipt roots;
- the complete broker response;
- positions, cash, liabilities, open orders, fills, and pending events; and
- the private valuation inputs.

Every root is a lowercase 32-byte SHA-256 commitment. Empty collections still
require a domain-separated root; an omitted category is not interpreted as zero.
The disclosure derives separate SHA3-384 commitments for quantity evidence,
valuation evidence, and aggregate disclosure using the reserve kit's existing
bounded `postfiat.reserve_opaque_commitment.v1` construction.

The generic Ed25519 attestation statement then cross-binds those commitments to
the PFTL genesis, NAV asset, proof profile, valuation policy, source manifest,
valuation unit and scale, observation epoch and interval, asset and liability
amounts, disclosure root, source identity, reserve owner, and verifier keys.

## Implemented code

- `tools/nav-reserve-proof/crates/reserve-proof-types/src/yolo_broker.rs`
  defines and validates the typed disclosure and derives all three commitments.
- `postfiat-reserve-proof adapter yolo-broker-commitments` provides the
  create-once CLI path from a typed private disclosure to those commitments.
- The existing reserve guest verifies the two Ed25519 signatures and retains the
  `attested` classification in its fixed public-values ABI.
- `navstrategies/navstrategies/yolo_indices/reserve.py` independently implements
  the same canonical encoding. Both implementations assert the same golden
  commitment vector.

The golden vector for the fixture in both repositories is:

| Commitment | SHA3-384 |
|---|---|
| Quantity | `005cac6ea12c95763e137429fa22d19058c78472b819c4ccb670013b72e817427a327336e85e1f5dc02e536898e8f5c7` |
| Valuation | `b2b7f36933790762618cfb528b77d5733091290f702f7c4c679d290122839c19868bf8e3204cfab891b47d7d8385dc27` |
| Disclosure | `782076ab3d4ae70654b6f3de5f0d555e71a78c78fee734e4dcc397322be8d5ecf6f3857fb66b3521193bbdfade90ff1b` |

Reproduce it from the reserve-kit directory:

```bash
cargo run --locked -p postfiat-reserve-proof -- \
  adapter yolo-broker-commitments \
  --disclosure fixtures/yolo-broker-attested/disclosure.json \
  --gross-assets 1000 \
  --total-liabilities 100 \
  --valuation-scale 1000000 \
  --output target/yolo-broker-attested-commitments.json
```

## Remaining gates

This adapter is not a complete YOLO proof profile. Production still requires:

1. an owner-locked options methodology and valuation policy;
2. a complete broker observation collector and private replay packet;
3. a hardware-qualified TEE verifier and governed statement keys;
4. fixed asset, profile, source-manifest, program/vkey, valuation-unit, freshness,
   fee, supply, and challenge parameters;
5. native/SP1 byte-identical witness execution and a pinned successor program
   identity;
6. devnet registration, reserve submission, challenge, finalization, mint,
   redemption, and conservation tests; and
7. legal, broker, data-rights, custody, and operational approval.

No item above is inferred from this foundation implementation.
