# Public NEAR Stake Reader

This stateless NEAR contract is the public source reader for the
`near-stake-receipt-bft-checkpoint-v1` reserve-proof adapter. It reads the
standard staking-pool interface, emits a canonical `postfiat-nav` snapshot
event, and returns exactly the payload committed by that event.

The contract is not an oracle and does not attest to NAV. The public verifier
in `reserve-proof-types` independently verifies the receipt proof, event and
callback equality, payload fields, reserve-owner signature, governed source
checkpoint, and the policy-pinned deployed reader and pool code hashes.

Build the reproducible WebAssembly artifact with the pinned repository
toolchain:

```text
rustup target add wasm32-unknown-unknown
cargo build --manifest-path contracts/near-stake-reader/Cargo.toml \
  --target wasm32-unknown-unknown --release --locked
```

Before use, governance must pin the SHA-256-derived NEAR code hash of the
deployed reader and the staking-pool code hash in the source policy. A source
checkpoint attests to those code identities at the exact finalized source
block; an RPC response by itself is not accepted as cryptographic finality.
