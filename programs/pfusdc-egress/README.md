# Withdrawal program releases

Destination verifiers pin an SP1 program identity. Arc v2 and pfETH v1 use different frozen egress programs. `releases.json` records their source, lockfile, build layout and retained identity manifests. Keep these pins unchanged when updating the node or host prover.

## Select the destination's release

Build the host with Rust 1.95.0 and `protoc` available:

```sh
cargo build --locked --release --no-default-features \
  --manifest-path tools/pfusdc-tier4-prover/Cargo.toml
```

The `egress` and `checkpoint` commands accept `--egress-release arc-v2|pfeth-v1` and `--elf PATH`. The default is `pfeth-v1` with the embedded retained pfETH ELF. For Arc, extract its retained ELF explicitly:

```sh
git show 65df4263746566c90d0e46db8c90a46c23576265:programs/pfusdc-egress/elf/pfusdc-egress-program > /tmp/arc-v2-egress.elf
tools/pfusdc-tier4-prover/target/release/pfusdc-tier4-prover egress \
  --egress-release arc-v2 --elf /tmp/arc-v2-egress.elf \
  --witness /path/to/witness.json --output-dir /path/to/output
```

Add `--prove` to generate a Groth16 proof. The host checks the ELF digest before execution and the derived verifying key before proving. A mismatched release or modified ELF fails closed. The existing witness and public-input checks also apply.

## Reproduce a release

Run the `arc-proof-identities` GitHub Actions workflow on the release branch. Each matrix job uses two clean trees at the manifest's source revision, reproduces the original build layout, compares both outputs with the retained ELF and expected digest, then derives the SP1 verifying key. The workflow uploads build artifacts even when identity comparison fails.

A retained ELF's digest identifies that artifact; successful source reproduction requires the entire job to pass. Build failures leave the corresponding release unqualified. Current-source guest builds require a new program identity and destination deployment process.
