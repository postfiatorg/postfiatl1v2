# pfUSDC Nitro sendRoot conformance

Status: conformance validated; guest freeze follows the bounded witness-mutation
gate and precedes the single SP1 build.

## Reference implementation

The byte encoding and Merkle rules are pinned to Offchain Labs Nitro commit
`a618155919315241665356fe60f3cd00d66d5e46`:

- `precompiles/ArbSys.go`, `SendTxToL1`: ArbOS hashes the concatenation of the
  immediate EVM caller's 20 address bytes, destination's 20 address bytes,
  32-byte big-endian L2 block number, L1 block number, timestamp, call value,
  and unprefixed calldata. The immediate caller is supplied by ArbOS as
  `c.caller`; it is not a calldata field.
- `arbos/merkleAccumulator/merkleAccumulator.go`, `Append`: the 32-byte item
  hash is hashed once more before it becomes a Merkle leaf.
- `util/merkletree/merkleTree.go`, `MerkleProof.IsCorrect`: each sibling is
  folded left or right according to the low index bit, the index is divided by
  two at every level, and verification also requires the remaining index to be
  zero.
- `system_tests/outbox_test.go`, `TestOutboxProofs`: Nitro constructs the same
  proof through `NodeInterface.ConstructOutboxProof` and checks it against the
  exported send-tree root.

Source: <https://github.com/OffchainLabs/nitro/tree/a618155919315241665356fe60f3cd00d66d5e46>

The Rollup assertion formula and storage layout are pinned through that Nitro
commit's `contracts` gitlink to Offchain Labs `nitro-contracts` commit
`4341b132cfbdcc980ead03765ca5224ff6cb5d97`:

- `src/rollup/AssertionState.sol`: `stateHash = keccak256(abi.encode(state))`;
- `src/rollup/RollupLib.sol`: `assertionHash = keccak256(abi.encodePacked(parentAssertionHash, stateHash, inboxAcc))`;
- `src/rollup/RollupCore.sol`: confirmation authenticates that exact preimage,
  publishes its block hash and `sendRoot` to the Outbox, then stores the hash in
  `_latestConfirmed`;
- `test/storage/RollupCore`: `_latestConfirmed` occupies proxy storage slot 116
  (`0x74`).

The V3 guest has an explicit two-network allowlist:

- production: Ethereum mainnet `1`, Arbitrum One `42161`, Rollup proxy
  `0x4DCeB440657f21083db8aDd07665f8ddBe1DCfc0`;
- controlled testnet: Ethereum Sepolia `11155111`, Arbitrum Sepolia `421614`,
  Rollup proxy `0x042B2E6C5E99d4c521bd49beeD5E99651D9B0Cf4`.

Each pair requires its canonical Ethereum genesis validators root and fork
schedule plus Nitro slot `0x74`. Cross-network mixtures reject. These are guest
constants, not witness-selectable route data. The selected Rollup proxy runtime
code hash remains route-pinned and is proved at the finalized Ethereum state
root. Sepolia is the clock-critical controlled target because it exercises real
Ethereum consensus and Nitro assertion finality without Arbitrum One's normal
multi-day assertion delay; a local fork cannot substitute for that proof.

Source: <https://github.com/OffchainLabs/nitro-contracts/tree/4341b132cfbdcc980ead03765ca5224ff6cb5d97>

## Fixed Rust conformance vector

Inputs:

```text
sender:       0x1111111111111111111111111111111111111111
destination:  0x2222222222222222222222222222222222222222
l2 block:     7
l1 block:     8
timestamp:    9
value:        0
calldata:     0x7469657234 ("tier4")
output index: 1
siblings[0]:  0x9999999999999999999999999999999999999999999999999999999999999999
siblings[1]:  0x9898989898989898989898989898989898989898989898989898989898989898
```

Expected values:

```text
item hash:  f1f98fe000af938f0626c1aa9590fb6344252302d1b4acb388a10b043e756f81
sendRoot:   4f48db66d9a031e08369f9e98df246d2e80b652711b532d3236c81d6ff187d66
```

`programs/pfusdc-ingress/src/lib.rs` fixes these values in a unit test. The
same test proves that a zero-length path is valid for a single-leaf tree and
that an index with bits remaining above the supplied path is rejected.

## Fixed BoLD assertion vector

Inputs:

```text
parent assertion hash: 0x11 repeated 32 bytes
L2 block hash:         0x22 repeated 32 bytes
sendRoot:              0x33 repeated 32 bytes
inbox position:        1
position in message:   0
machine status:        1 (FINISHED)
end history root:      0x44 repeated 32 bytes
inbox accumulator:     0x55 repeated 32 bytes
```

Expected values from the Solidity ABI/formula above:

```text
AssertionState hash: e5460f927b4570a316bd9d6455ca47aa2dcc52bbda1c530579124d8ba1ad210a
assertion hash:      cd5427de6f33a41b79699cd41cb5d4adad5a88b1d6a253913acd95f27010c434
```

The Rust test fixes both values so a tuple-layout, enum-width, packed-encoding,
or field-order drift fails before the guest is built.

## State-proof boundary

The confirmed Nitro assertion authenticates its endpoint L2 block hash and
`sendRoot`. The ingress guest therefore:

1. hashes and canonically RLP-decodes the asserted L2 header;
2. uses that header's state root to verify the exact Arbitrum vault and token
   account/code proofs;
3. uses the Ethereum-finalized execution state root to verify both the Rollup
   `latestConfirmed` storage proof and the production ingress-anchor account
   code proof.

The ingress anchor is an Ethereum parent-chain destination of
`ArbSys.sendTxToL1`, not an Arbitrum L2 account. Proving it against the asserted
L2 state would be the wrong trie and is forbidden by the V3 witness layout.

The anchor's governed route binding is constructor-set storage with no setter,
not a Solidity immutable. This is intentional: compiling the route binding
into runtime bytecode would create an undeployable circular commitment because
the route profile commits the verifier policy, the policy commits the anchor
runtime code hash, and the route binding commits the route-profile hash. The
bridge, L2 vault, L2 token, and L2 chain remain bytecode-level immutables.
