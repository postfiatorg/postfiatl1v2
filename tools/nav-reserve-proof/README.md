# PostFiat Open Reserve Proof Kit

This workspace is the provider-neutral reference implementation for
'postfiat.nav_reserve_public_values.v1'. It is part of the public PostFiat
protocol repository and has no dependency on an operator portfolio product,
private filesystem layout, or operator API.

The current reference guest enforces:

- a content-addressed manifest with 1–64 strictly sorted unique sources;
- explicit quantity and valuation trust classes for every source;
- independent commitments to reserve ownership, quantity verification, and
  valuation verification;
- bounded observation intervals and per-source freshness;
- checked gross-assets, liability, net-assets, and trust-bucket arithmetic;
- chain, NAVCoin, proof-profile, policy, manifest, unit, epoch, and interval
  bindings;
- canonical observation, trust, and disclosure roots; and
- the exact 584-byte public-values ABI decoded by PostFiat L1.

`controlled` evidence exists only for controlled test fixtures. An
`adapter_proof` cannot claim `cryptographic` status until its adapter verifier
is registered in the guest; unsupported adapters fail closed. Signed
Ed25519 attestations are checked in the guest and remain labeled `attested`.
The `ed25519-protocol-receipt-v1` adapter is the first registered
cryptographic adapter: it verifies a manifest-pinned protocol receipt key.
Governance must only use that adapter for a key that genuinely belongs to the
named protocol; an operator signature is an attestation, not a protocol
receipt.

`evm-erc20-bft-checkpoint-mpt-v1` is the open EVM quantity adapter. It verifies
an ERC-20 account and balance-slot Merkle-Patricia proof against a state root,
then verifies that root under a manifest-pinned, canonically ordered quorum of
ML-DSA-65 checkpoint validators. The checkpoint binds the EVM chain, block,
state root, confirmation depth, PFTL genesis, PFTL observation height,
committee epoch, and committee root. The manifest separately binds the EVM
owner, token, committee, and valuation authority. This adapter's quantity is
cryptographic relative to the disclosed BFT checkpoint; it is not represented
as trustless Ethereum consensus finality. Its valuation must use a separate
evidence object, normally an attestation or a protocol price receipt.

Reserve-proof v1 supports liabilities attributable to an asset source through
that observation's `total_liabilities`. It deliberately rejects a standalone
`liability` manifest entry: the fixed v1 public-values ABI has unsigned trust
buckets and cannot faithfully classify a negative source. A future schema may
add signed per-class accounting; v1 fails closed instead of silently treating
a standalone liability as an asset.

Every signed evidence statement cross-binds the chain, NAVCoin, profile,
policy, manifest, valuation unit and scale, source identity, reserve-owner and
verifier commitments, observation interval, amounts, both quantity and
valuation evidence commitments, and disclosure commitment. Evidence from two
different observations cannot be mixed into one valid witness.

## Pinned toolchain

- Rust 1.95.0 (rust-toolchain.toml)
- SP1 crates 6.3.1
- cargo-prove installed explicitly with `sp1up --version 6.3.1`
- protoc is required by the SP1 host SDK

The workspace lockfiles are committed. Production guest builds must use
'--locked'. A Docker build must explicitly use an SP1 6.3.1 image rather
than relying on cargo-prove's default image tag.

The committed `program-identity.json` pins the expected ELF SHA-256 and SP1
program vkey. Any guest rebuild used for registration or proof production
must match both values; a change requires a new immutable proof profile, not
an in-place reinterpretation of an existing profile.

## Build and test

    cd tools/nav-reserve-proof
    cargo test --locked
    cargo check --locked -p postfiat-reserve-proof --features sp1

    cd programs/reserve-proof-guest
    cargo prove build --locked --output-directory ../../elf

After every build, compare both the ELF SHA-256 and `program-info` vkey with
`program-identity.json`. CI rebuilds the guest from source and performs both
comparisons; merely hashing a previously committed ELF is not a reproducible
build qualification.

## Reference workflow

    cd tools/nav-reserve-proof

    cargo run --locked -p postfiat-reserve-proof -- \
      manifest validate fixtures/controlled-two-source/manifest.json

    cargo run --locked -p postfiat-reserve-proof -- \
      profile derive \
      --registration fixtures/controlled-two-source/profile-registration.json \
      --output target/reference/derived-profile.json

    cargo run --locked -p postfiat-reserve-proof -- \
      observe \
      --manifest fixtures/controlled-two-source/manifest.json \
      --context fixtures/controlled-two-source/context.json \
      --input-dir fixtures/controlled-two-source/observations \
      --output target/reference/witness.json

    cargo run --locked -p postfiat-reserve-proof -- \
      witness build \
      --input target/reference/witness.json \
      --output target/reference/witness.cbor

    cargo run --locked -p postfiat-reserve-proof -- \
      execute \
      --witness target/reference/witness.cbor \
      --output target/reference/public-values.bin

    cargo run --locked -p postfiat-reserve-proof -- \
      verify --public-values target/reference/public-values.bin

After building the guest, add '--features sp1' and
'--elf elf/postfiat-reserve-proof-guest' to 'execute'. The command compares
SP1 output byte-for-byte with native execution. 'prove' emits a locally
verified Groth16 proof, calldata bytes, public values, and the program vkey.

`observe` reads exactly one bounded `<source_id>.json` adapter artifact for
every manifest entry. It orders those artifacts by the manifest rather than
filesystem order and executes all evidence checks before writing a witness.
Source-specific collectors can remain private processes with private API
credentials; their artifact schema and the verifier that consumes it are
public. No credential or signing authority is included in the witness.

The open EVM adapter also provides the complete public construction boundary:

    postfiat-reserve-proof adapter evm-erc20 checkpoint-vote-statement \
      --checkpoint checkpoint.json \
      --validator-id validator-0 \
      --output checkpoint-validator-0.statement

    postfiat-reserve-proof adapter evm-erc20 owner-authorization \
      --manifest manifest.json \
      --context context.json \
      --source-id ethereum-usdc \
      --owner 0x... \
      --token 0x... \
      --committee-root 0123... \
      --output owner-authorization.statement

After the committee certificate and EIP-191 owner signature exist, `adapter
evm-erc20 collect` queries `eth_getBlockByNumber`, `eth_blockNumber`, and
`eth_getProof`. It accepts HTTPS RPC URLs, plus loopback HTTP for development;
redirects, embedded URL credentials, oversized responses, and mismatched
block/state roots fail closed. The collector verifies the checkpoint quorum,
manifest owner/token/committee pins, owner signature, and account/storage MPT
proof before emitting a `SourceObservationV1`.

Quantity and valuation remain separate. Derive the exact manifest commitment
for an Ed25519 attestation or protocol-receipt key first:

    postfiat-reserve-proof adapter ed25519-verifier-commitment \
      --public-key 0123... \
      --output verifier-commitment.txt

Place that commitment in the applicable quantity or valuation verifier field
of the manifest. Then use a placeholder 64-byte signature in the observation
and emit its exact canonical statement:

    postfiat-reserve-proof adapter ed25519-evidence-statement \
      --manifest manifest.json \
      --context context.json \
      --source-id ethereum-usdc \
      --observation ethereum-usdc.unsigned.json \
      --dimension valuation \
      --output ethereum-usdc-valuation.statement

Sign that byte file outside the proof kit, then attach and verify the canonical
lowercase-hex Ed25519 signature:

    postfiat-reserve-proof adapter ed25519-evidence-attach \
      --manifest manifest.json \
      --context context.json \
      --source-id ethereum-usdc \
      --observation ethereum-usdc.unsigned.json \
      --dimension valuation \
      --signature 0123... \
      --output ethereum-usdc.json

The final `observe` command verifies every dimension again in complete
manifest order. No adapter command receives a private key.

Opaque reserve-owner, haircut-policy, evidence, and disclosure artifacts can
be committed without inventing a local hash convention:

    postfiat-reserve-proof commitment derive \
      --label reserve-owner \
      --input reserve-owner.txt \
      --output reserve-owner.commitment

The input is capped at 16 KiB and the label is part of the domain-separated
preimage. This command creates a commitment only; it does not upgrade an
attestation into cryptographic evidence.

For local CPU proving use `SP1_PROVER=cpu`. For the authenticated SP1 network
prover use `SP1_PROVER=network` and provide `NETWORK_PRIVATE_KEY` through the
operator's secret manager. The remote prover receives only the bounded witness
and ELF; the CLI locally verifies every returned proof before writing it and
never gives the prover a PFTL or Ethereum signing key.

`packet build` combines reviewed packet metadata, proof calldata, and decoded
public values into a validated `NavReserveSubmitOperation`. Obtain an ordinary
PFTL asset fee quote and sign it locally with `postfiat-node
wallet-sign-asset-transaction`; the proof kit never receives an issuer key.
Then submit the signed transaction to one or more validator endpoints:

    cargo run --locked -p postfiat-reserve-proof -- \
      packet submit \
      --signed-transaction target/reference/reserve-packet.signed.json \
      --rpc-address 127.0.0.1:28650 \
      --rpc-address 127.0.0.1:28651 \
      --output target/reference/reserve-packet.finality.json

The command uses the bounded PFTL newline-JSON RPC, requires a confirmed and
accepted finality receipt, and only retries another supplied endpoint after a
typed wrong-proposer or connection failure. It cannot sign, modify, or silently
replace the reviewed operation.

For wrapped-NAVCoin export/return deployment, continue with the
[generic relay runbook](../../docs/runbooks/NAVCOIN-GENERIC-EXPORT-RETURN-RELAY-20260801.md).
The relay is route-configured and uses the standalone constrained signer; it
does not import an operator portfolio product.

## License

This directory is licensed under the repository's dual
MIT OR Apache-2.0 license.
