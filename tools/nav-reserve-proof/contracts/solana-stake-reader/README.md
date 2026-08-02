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
