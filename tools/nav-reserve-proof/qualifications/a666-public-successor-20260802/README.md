# A666 public-successor reserve qualification

This directory contains two fresh, complete A666 reserve observations produced
by the public provider-neutral proof kit on 2026-08-02. It is the publication
fixture for the proposed migration of the existing A666 NAVCoin from its
legacy proof profile to the immutable public successor profile. It does not
create a new NAVCoin and does not by itself authorize live activation.

Neither observation depends on an internal operator application or API, a
private filesystem path, or an operator attestation. Each epoch contains all
six bounded public inputs needed to verify quantity and valuation for:

- Aave v3 positions on Arbitrum;
- multi-chain EVM spot assets;
- Hyperliquid assets;
- staked NEAR;
- staked Solana; and
- Monero reserves.

Both quantity and valuation classify all six sources as cryptographic. The
successor manifest rejects controlled sources and the public values report
zero attested and zero controlled value.

## Pinned identity

The governing identity is
`../../a666-successor-program-identity.json`:

- guest source commit:
  `5b8f0317375af6fb46d586d9d9152b511457b802`;
- guest ELF SHA-256:
  `2b41e4e8095b1dacdc519b2f0a2b4831ebc57cc8003a4d3686f6d9e4687e81df`;
- SP1 program vkey:
  `0x00f3857f96ef97e00bd15b4030acd8d6b0a72740b28c6160d154bc2c9bb141bf`;
- proof profile:
  `f8784629ff7338002d836c1988b8e2c0f19caf448429e0eb7fdc39fa2b08f7d9a44171fc1e7239bc25e06ad833c14e91`;
- source manifest hash:
  `8abe3e59198b72945d4778a7fa91e5af157a6c65032d8940cca486850ffe59fcb567268ca5942669ff6977ef32dd3a41`;
- valuation policy hash:
  `350eaee0a1ca12ba51637781ba52661b8685f868657a7c5e7d07c31b2899869c`.

The profile registration is published at
`../../manifests/a666/profile-registration-public-successor.json`. The legacy
profile is immutable and is not changed in place.

## Results

All amounts below are USD e8 atoms.

| Result | Epoch 7 | Epoch 8 |
|---|---:|---:|
| PFTL observation window | 776–784 | 776–784 |
| Gross assets | 2,855,886,091,629 | 2,859,789,254,961 |
| Liabilities | 20,094,872,960 | 20,094,965,843 |
| Verified net assets | 2,835,791,218,669 | 2,839,694,289,118 |
| Cryptographically verified value | 2,835,791,218,669 | 2,839,694,289,118 |
| Attested value | 0 | 0 |
| Controlled value | 0 | 0 |
| Witness CBOR SHA-256 | `8f4538b111a6cebf97a71406895af40d4bc4c6ed369c3ecd540b6050765b6dd6` | `4bb36f68ebee3537afe62f6d9ed61d68513a96c747fffc3622024b0a2f9efe89` |
| Public values SHA-256 | `a215726624267dc5c5a60ac2829b24a149855a3edcfc798c965826e17bca7e68` | `1bc443108e0f2b78d92037d986378cd6df51bd3fc069e64594a521f83a36b9dd` |

`expected-results.json` records the complete machine-readable result pins.

## Reproduce the observations and witness

Install `protoc` (the Debian/Ubuntu package is `protobuf-compiler`) and build
the public host CLI with SP1 support. The default feature set can reproduce
observations and witnesses, but it intentionally cannot execute or verify an
SP1 proof:

```bash
cd tools/nav-reserve-proof
cargo build --locked --release -p postfiat-reserve-proof --features sp1
cd ../..
```

Then, from the repository root:

```bash
KIT=tools/nav-reserve-proof
QUAL=$KIT/qualifications/a666-public-successor-20260802
RUN=$(mktemp -d)

$KIT/target/release/postfiat-reserve-proof manifest validate \
  $QUAL/manifest.json

for EPOCH in 7 8; do
  mkdir -p $RUN/epoch-$EPOCH
  $KIT/target/release/postfiat-reserve-proof observe \
    --manifest $QUAL/manifest.json \
    --context $QUAL/epoch-$EPOCH/context.json \
    --input-dir $QUAL/epoch-$EPOCH/inputs \
    --output $RUN/epoch-$EPOCH/observations.json
  $KIT/target/release/postfiat-reserve-proof witness build \
    --input $RUN/epoch-$EPOCH/observations.json \
    --output $RUN/epoch-$EPOCH/witness.cbor
  sha256sum $RUN/epoch-$EPOCH/witness.cbor
done
```

The resulting witness hashes must match the table above. Every observation is
self-contained and includes the public state, ownership, receipt, checkpoint,
and valuation evidence required by its adapter.

## Rebuild and execute the immutable successor

Production identity is defined by a Docker build of the pinned source commit,
not by a mutable ELF copied from an operator machine:

```bash
ROOT=$(git rev-parse --show-toplevel)
SOURCE=$(jq -r .source_commit \
  $ROOT/tools/nav-reserve-proof/a666-successor-program-identity.json)
WORKSPACE=$(mktemp -d)
BUILD=$(mktemp -d)

git -C $ROOT archive $SOURCE | tar -x -C $WORKSPACE
cd $WORKSPACE/tools/nav-reserve-proof/programs/reserve-proof-guest
cargo prove build --docker --tag v6.3.1 --locked \
  --features a666-public-adapters-v2 \
  --workspace-directory $WORKSPACE \
  --output-directory $BUILD
sha256sum $BUILD/postfiat-reserve-proof-guest
```

The ELF hash must match the pinned identity. Use it with the witnesses above:

```bash
cd $ROOT
for EPOCH in 7 8; do
  $KIT/target/release/postfiat-reserve-proof execute \
    --witness $RUN/epoch-$EPOCH/witness.cbor \
    --elf $BUILD/postfiat-reserve-proof-guest \
    --output $RUN/epoch-$EPOCH/public-values.bin
  $KIT/target/release/postfiat-reserve-proof verify \
    --public-values $RUN/epoch-$EPOCH/public-values.bin
  sha256sum $RUN/epoch-$EPOCH/public-values.bin
done
```

CPU Groth16 proof reports and calldata may be published after independent host
verification. Their publication does not authorize activation. Live profile
registration and A666 rebinding remain prohibited until the exact
six-validator migration and release-recovery gates in the canonical
deprecation plan pass.
