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
The exact build was deployed immutably to Solana mainnet-beta on 2026-08-02 as
program `Gp2oTn6VjFF22n98H6YSH4uVvQxWFHNCL7pp1tcAPF36`, with ProgramData
account `9xVv6Q8Z1AJsK4aWKydhYyEGeA7Ai8k6t3gpreR7QBh8`. The on-chain program bytes
match the pinned raw ELF SHA-256 and the upgrade authority is absent. This
closes deployment only; the reader remains unqualified until its governed
A666 policy, fresh certified epochs, reconciliation, fuzzing, and independent
reproduction gates pass.
