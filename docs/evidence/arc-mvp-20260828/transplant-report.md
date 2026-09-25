# Arc semantic transplant report

Captured `2026-08-29T01:20:45Z`.

The Arc implementation is composed onto the exact controlled-devnet parent
`8cc7d15edc58b5f5a0b745143fef2d45203465ff`. The initial composed checkpoint
is `3e2c9caa9159cd899664434f0377f05b27f31deb`.

The conflict resolution retained the newer live-lineage Ethereum finality,
bonded fast-ingress, storage-integrity, governance-history, and node proof
routes. Arc was added as a distinct proof-native ingress variant instead of
replacing any existing route.

Replicated state now carries both bonded fast-ingress campaigns and Arc
finality states. Arc state participates in canonical validation, state
commitment, snapshot import/export, rollback, and history replay. Deposit
execution clones and validates the Arc finality transition before committing
either the deposit record or advanced state, so rejection cannot leave a
partial transition.

The prover CLI retains the live-lineage bonded commands and adds the bounded
Arc capture, audit, prove, and program-info paths. The independent guest keeps
its SP1 dependency graph isolated. The Alloy `sol` macro family is pinned
consistently at `1.5.7` to prevent an incompatible macro/runtime lock mix.

The pinned source dependencies are:

- `circlefin/arc-node` at
  `66ad2d5aa6d9b41e8f689812004be4c7233a9e16`;
- `succinctlabs/sp1-contracts` at
  `2ac5ecbbe473421a963d67e55f182e9a36576f7c`.

The public Arc RPC does not implement `eth_getProof` for the validator
registry, and the native Arc commit certificate does not authenticate an
arbitrary `eth_call` result for the next set. The guest therefore continues to
reject every asserted validator-set rotation. This is fail-closed, but it does
not satisfy the final authenticated-rotation gate.

Focused qualification results are recorded in `test-report.json`. Historical
proof artifacts remain benchmark evidence only: the live-lineage candidate
must regenerate and freeze its final ELF, vkey, proof, vault, and route
bindings together.
