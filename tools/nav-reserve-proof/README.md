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
`--locked` and the pinned SP1 6.3.1 Docker image. Native `cargo prove build`
is useful for development, but its ELF can embed the host checkout path and
must never define a registered production identity.

The committed `program-identity.json` pins the immutable legacy reference's
source commit, ELF SHA-256, and SP1 program vkey. CI rebuilds that exact public
source commit, not the evolving successor checkout. Any guest rebuild used for
the legacy registration must match both values. The complete public A666
adapter guest will receive a new identity and proof profile only after all
source collectors and qualification gates pass; the legacy identity is never
rewritten in place.

## Build and test

    cd tools/nav-reserve-proof
    cargo test --locked
    cargo check --locked -p postfiat-reserve-proof --features sp1

    repo_root="$(git rev-parse --show-toplevel)"
    cd programs/reserve-proof-guest
    cargo prove build --docker --tag v6.3.1 --locked \
      --workspace-directory "$repo_root" \
      --output-directory "$repo_root/tools/nav-reserve-proof/elf"

After every legacy build, compare both the ELF SHA-256 and `program-info` vkey
with `program-identity.json`. CI archives and rebuilds the pinned source commit
and performs both comparisons; merely hashing a previously committed ELF is
not a reproducible build qualification. A successor build must instead be
recorded in a distinct identity artifact and governed as a new proof profile.

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
SP1 output byte-for-byte with native execution. `prove` emits a locally
verified Groth16 proof, calldata bytes, public values, and the program vkey.

`observe` reads exactly one bounded `<source_id>.json` adapter artifact for
every manifest entry. It orders those artifacts by the manifest rather than
filesystem order and executes all evidence checks before writing a witness.
Every source-specific collector required to reproduce a live NAVCoin must be
public, versioned, and auditable. A deployment may supply RPC credentials from
its secret manager, but neither a private collector nor an undocumented
internal API may define the artifact semantics. No credential or signing
authority is included in the witness.

All adapters that use a governed external-source checkpoint share one public
certificate workflow. Each validator obtains the exact statement bytes and
signs them in its isolated signer; the proof kit never receives its private
key. After collecting the bounded vote JSON files, assemble and validate the
certificate:

    postfiat-reserve-proof source-checkpoint vote-statement \
      --checkpoint checkpoint.json \
      --validator-id validator-0 \
      --output checkpoint-validator-0.statement

    postfiat-reserve-proof source-checkpoint assemble \
      --committee committee.json \
      --checkpoint checkpoint.json \
      --vote checkpoint-validator-0.vote.json \
      --vote checkpoint-validator-1.vote.json \
      --vote checkpoint-validator-2.vote.json \
      --output checkpoint.certificate.json

    postfiat-reserve-proof source-checkpoint validate \
      --certificate checkpoint.certificate.json

Assembly sorts votes canonically and rejects an invalid committee binding,
sub-quorum set, duplicate or unknown validator, malformed signature, or bad
signature. Adapter-specific collectors must still validate that the certified
source block and commitment match their independently queried source state.

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

The successor complete-EVM-spot adapter applies the same boundary to every
native and ERC-20 position in one governed multichain policy. For each chain,
validators independently construct the exact checkpoint candidate from the
same pinned source height before producing their isolated ML-DSA vote:

    postfiat-reserve-proof adapter evm-spot checkpoint-candidate \
      --pftl-genesis-hash 0123... \
      --policy evm-spot-policy.json \
      --source-domain eip155:1 \
      --source-height 12345678 \
      --minimum-depth 64 \
      --pftl-observation-height 500 \
      --committee ethereum-checkpoint-committee.json \
      --rpc-url https://ethereum.example \
      --output ethereum-checkpoint.json

The command verifies chain ID, required depth, block number, hash, timestamp,
state root, committee epoch, and policy-pinned committee root. After generic
checkpoint certificate assembly, `adapter evm-spot owner-authorization`
emits the EIP-191 bytes binding the owner to the complete policy and every
certificate. `adapter evm-spot collect` then queries each governed RPC at its
certified height, verifies every native-account and ERC-20 storage proof, and
emits one complete quantity observation. Its reviewed RPC map has this exact
schema and must contain precisely the policy's source domains:

    {
      "schema": "postfiat.reserve_evm_spot_rpc_map.v1",
      "sources": {
        "eip155:1": "https://ethereum.example",
        "eip155:42161": "https://arbitrum.example"
      }
    }

No checkpoint, owner, or validator private key enters any proof-kit command.
RPC credentials may be supplied operationally, but all artifact construction
and validation semantics are public here. EVM-spot valuation remains a
separately disclosed evidence dimension.

The Aave V3 adapter exposes the same three public steps under `adapter
aave-v3`: `checkpoint-candidate`, `owner-authorization`, and `collect`. Its
governed policy pins the chain, pool and oracle contracts and code hashes,
complete collateral/debt position set, each aToken or variable-debt-token
contract, each per-user storage mapping slot, reserve mapping layout,
Chainlink proxy/aggregator code hashes and storage layout, capped-stable
adapter and cap slot where applicable, decimals, accrual constants, and price
freshness. Collection queries only the certified block, proves every account
and storage value with `eth_getProof`, reconstructs accrued collateral and
debt conservatively, and verifies the resulting USD values before writing the
observation. Quantity and valuation both remain cryptographic because the one
proof includes the reserve indexes and pinned Chainlink/capped-stable oracle
state. The CLI accepts the expected collateral and liability values as an
explicit reviewed assertion, then independently recomputes and rejects either
on any mismatch; it does not trust those values as evidence.

Hyperliquid receipt production no longer relies on an unpublished reader
contract. `crates/ethereum-contracts/src/HyperCoreReserveReader.sol` is the
public source for the HyperEVM contract whose address is policy-pinned by the
receipt verifier. It reads only official HyperCore precompiles, requires
canonical ordered position sets, derives the supported XMR1 and HYPE spot
prices from pinned HyperCore mark-price assets, and emits the exact
`HyperCoreSnapshot` commitment consumed by the public verifier. The verifier
treats `allowed_spot_tokens` as the complete required spot set: omission,
addition, reordering, duplicate token, or decimal substitution fails closed.
Public deployment and receipt-proof collection tooling remain required before
this adapter is production-qualified.

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

The packet template accepts two optional, backward-compatible fields for a
consensus-accounted NAV subscription reserve overlay:

    "subscription_overlay_source_root": "<48-byte lowercase hex>",
    "subscription_overlay_value": <nonzero u64>

When both are present, `packet build` derives the versioned composite source
root over the proven public values and the overlay using the shared consensus
helper (`postfiat.nav_reserve_subscription_composite_source_root.v1` in
`postfiat-types`), requires the template's `source_root` to equal that exact
composite root, and sets the packet's `verified_net_assets` to the base proof
assets plus the overlay value with checked arithmetic. When the fields are
absent or defaulted, the original proof-only behavior is unchanged. See
`fixtures/controlled-two-source/packet-template-subscription-overlay.json`
for a worked overlay template against the qualified controlled-two-source
public values; the CLI test suite pins that fixture's composite root.
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
