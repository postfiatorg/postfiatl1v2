# A666 provider-neutral reserve-proof shadow qualification

This fixture ports the exact reserve amounts used by the finalized A666
epoch-3 proof on 2026-07-30 into the open, provider-neutral reserve-proof
schema. It is a historical-observation shadow comparison, not a fresh reserve
claim and not a live profile activation.

The successor guest reproduces the old economic totals exactly:

| Measure (USD e8 atoms) | Legacy proof | Successor shadow | Delta |
|---|---:|---:|---:|
| Gross assets | 2,846,461,376,975 | 2,846,461,376,975 | 0 |
| Liabilities | 20,088,300,169 | 20,088,300,169 | 0 |
| Verified net assets | 2,826,373,076,806 | 2,826,373,076,806 | 0 |

Trust classifications do **not** match, by design. The old program contained
provider-specific verifiers and labeled Aave and Hyperliquid quantity and
valuation, plus EVM and NEAR quantity, as cryptographic. The open successor
fixture uses a bounded Ed25519 attestation adapter for all six ported
observations, so all value is disclosed as attested. The successor does not
claim that a signature over a historical value proves underlying venue state.

The shadow attestor private key is not in the repository. The public key and
the complete signed observations are committed, so anyone can reproduce and
verify the witness and public values without the key.

## Reproduce

From the repository root:

```bash
KIT=tools/nav-reserve-proof
QUAL=$KIT/qualifications/a666-shadow-20260730
RUN=$(mktemp -d)

$KIT/target/release/postfiat-reserve-proof manifest validate \
  $QUAL/manifest.json

$KIT/target/release/postfiat-reserve-proof profile derive \
  --registration $QUAL/profile-registration.json \
  --output $RUN/profile.json

$KIT/target/release/postfiat-reserve-proof observe \
  --manifest $QUAL/manifest.json \
  --context $QUAL/context.json \
  --input-dir $QUAL/observations \
  --output $RUN/observations.json

$KIT/target/release/postfiat-reserve-proof witness build \
  --input $RUN/observations.json \
  --output $RUN/witness.cbor

$KIT/target/release/postfiat-reserve-proof execute \
  --witness $RUN/witness.cbor \
  --elf $KIT/elf/postfiat-reserve-proof-guest \
  --output $RUN/public-values.bin

$KIT/target/release/postfiat-reserve-proof verify \
  --public-values $RUN/public-values.bin
```

CPU Groth16 proof generation and host verification also completed for this
exact witness. The proof is pinned by `proof-report.json`; the 356-byte
consensus proof and 584-byte public values are committed as hex fixtures under
`crates/execution/testdata/`. The execution-layer test
`a666_successor_shadow_real_proof_verifies_in_consensus` verifies the real
proof under the exact successor profile and rejects a one-byte proof mutation.

The proof can be reproduced and verified with:

```bash
SP1_PROVER=cpu $KIT/target/release/postfiat-reserve-proof prove \
  --witness $RUN/witness.cbor \
  --elf $KIT/elf/postfiat-reserve-proof-guest \
  --output-dir $RUN/proof

$KIT/target/release/postfiat-reserve-proof verify \
  --proof $RUN/proof/proof.bin \
  --elf $KIT/elf/postfiat-reserve-proof-guest
```

The expected successor profile ID is
`a18c1bbee443f5f9958592cf65f4c16ee837c5c717cf9144f5b54f90bea267c80b4ae161001c2f7e5267fdc424c366c3`.
The expected public-values SHA-256 is
`699cb8acb39a3d351549e9a08c13a7712126841b3c3d480606c684a6db4184c8`.
The complete machine-readable comparison is in `reconciliation.json`.

## Remaining qualification work

This fixture proves amount-preserving translation, SP1 guest execution, CPU
Groth16 proving, host verification, exact consensus verification, and tamper
rejection. It does not satisfy the live migration gate by itself. Remaining
A666 gates are:

- collect and compare multiple genuinely fresh reserve epochs;
- replace the conservative attestation adapters with open state/receipt
  adapters wherever a cryptographic claim is required;
- register and finalize the immutable successor profile and packet;
- govern the A666 route transition; and
- execute transparent/private issue and redeem plus Ethereum export/return.
