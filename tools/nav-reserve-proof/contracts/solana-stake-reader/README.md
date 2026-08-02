# Public Solana Stake Reader

This stateless Solana program reads an exact, ordered set of standard stake
accounts plus the Clock sysvar and emits a canonical binary snapshot. It
accepts no writable accounts and moves no funds. The transaction message binds
the reader program, instruction salt, Clock sysvar, and complete ordered stake
account list.

The program is one component of the public
`solana-stake-reader-bft-checkpoint-v1` adapter. The public host verifier must
also verify the finalized transaction response, deployed program-data hash,
exact transaction message and reader output, reserve-owner authorization,
complete position policy, and a quorum-certified source checkpoint. The
immutable reader program performs the raw account parsing on chain. A public
RPC response or operator signature alone is not accepted as quantity evidence.

Build and test natively with the pinned Rust toolchain:

```text
cargo test --manifest-path contracts/solana-stake-reader/Cargo.toml --locked
```

A deployable SBF artifact additionally requires the audited Solana/Agave SBF
toolchain. The deployed program-data account hash and immutable/upgrade
authority state must be pinned in governance before any real-value use.

The reproducible build identity is in `program-identity.json`. It pins the
public source commit, Solana SDK 3.0.0, `solana-verify` 0.5.1, exact Docker
image digest, SBPF architecture, executable program hash, raw ELF SHA-256, and
byte length. Rebuild and compare every field with:

```text
scripts/check-solana-stake-reader-identity --build
```

The independently repeated executable program hash is
`1e0290cc9faa3b440b41e15e15f33ef34afcef4cc0cf65a719ab64fab4abad62`.
The program has not been deployed. A deployment is not qualified until the
on-chain program is made immutable and its exact ProgramData account and hash
are published in the governed policy.
