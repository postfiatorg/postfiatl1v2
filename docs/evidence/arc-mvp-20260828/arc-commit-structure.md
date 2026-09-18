# Arc testnet commit structure

Pinned source: `circlefin/arc-node` commit `66ad2d5aa6d9b41e8f689812004be4c7233a9e16` (also recorded in `arc-node-pin.txt`). All source paths and line numbers below refer to that commit.

## Signed value and algorithm

Arc commit certificates contain Ed25519 signatures over `Vote::to_sign_bytes()`. `Vote` is SSZ-encoded after its extension is removed (`crates/types/src/vote.rs:32-45,82-99`). A commit signature is a `Precommit` (`crates/types/src/vote.rs:65-79`). The vote type is an SSZ tagged enum where `Prevote = 0` and `Precommit = 1` (`crates/types/src/ssz/v1/vote.rs:23-46`).

For a non-nil commit value, the signed preimage is exactly 75 bytes:

| Offset | Length | Encoding | Meaning |
|---:|---:|---|---|
| 0 | 1 | `0x01` | `SszVoteType::Precommit` tag |
| 1 | 8 | unsigned little-endian | consensus height |
| 9 | 4 | unsigned little-endian, value `37` | SSZ offset to dynamic `round` |
| 13 | 4 | unsigned little-endian, value `42` | SSZ offset to dynamic `value` |
| 17 | 20 | raw bytes | validator address |
| 37 | 1 | `0x01` | `Some(round)` tag |
| 38 | 4 | unsigned little-endian | round |
| 42 | 1 | `0x01` | `Some(value_id)` tag |
| 43 | 32 | raw bytes | EVM block hash |

Reconstruction pseudocode:

```text
preimage =
    0x01 ||
    LE64(height) ||
    LE32(37) ||
    LE32(42) ||
    validator_address[20] ||
    0x01 || LE32(round) ||
    0x01 || evm_block_hash[32]
```

The implementation used by the golden-vector tests is `crates/arc-conformance/src/lib.rs::commit_preimage`.

There is no chain ID or network ID in this signing preimage. The precommit tag separates the vote phase, and height, round, signer address, and value bind the vote within a chain, but chain-level replay separation is external to this encoding. The Arc ingress route must therefore pin chain ID `5042002`, the validator-set commitment, and the source-domain configuration independently.

## Validator set and quorum

The validator set for a certificate at height `H` is `ValidatorRegistry.getActiveValidatorSet()` read at execution block `H-1`; Arc documents and implements this timing in `crates/eth-engine/src/ipc/ethereum_ipc.rs:147-174`.

Each validator contains an Ed25519 public key, an address, and `u64` voting power (`crates/types/src/validator_set.rs:27-42`). The address is the first 20 bytes of `Keccak256(public_key)` (`crates/types/src/address.rs:44-77`). Arc sorts validators by descending voting power and then ascending address (`crates/types/src/validator_set.rs:126-138`).

The verifier rejects duplicate validators, duplicate signers, key/address mismatches, unknown or zero-power signers, malformed Ed25519 keys/signatures, and checked-arithmetic overflow. Quorum is strict Tendermint-style greater-than two thirds:

```text
signed_voting_power * 3 > total_voting_power * 2
```

All multiplication and accumulation in the conformance implementation uses checked `u128` arithmetic and is bounded to 256 validators and 256 signatures.

## Commit-to-execution binding

The binding chain is:

1. `ValueId` is a transparent 32-byte block hash (`crates/types/src/value.rs:27-40`).
2. Consensus proposals and votes use the execution payload's wire block hash as the value (`crates/types/src/block.rs:91-123`).
3. A decided block asserts that the certificate value equals the execution payload block hash (`crates/types/src/block.rs:160-182`).
4. Before voting a network block valid, Arc checks that the payload block number equals the consensus height and, when the immediate predecessor is available, that the payload parent hash equals the block finalized at `H-1` (`crates/malachite-app/src/payload.rs:365-432,458-499`).
5. Arc sends the complete payload through Engine API `newPayload`; only `VALID` becomes a valid consensus payload (`crates/malachite-app/src/payload.rs:235-305`). Engine validation checks the supplied block hash against the canonical execution header. Arc also exposes a canonical reconstruction through the execution-layer block path (`crates/types/src/block.rs:126-145`).
6. The canonical EVM header commits to `receiptsRoot`. Once consensus decides, Arc makes that same payload hash canonical using `forkchoice_updated` (`crates/malachite-app/src/finalize.rs:89-141`).

Therefore, an accepted certificate over `ValueId = block_hash`, plus a receipt trie proof under that block header's `receiptsRoot`, binds the proven log to an execution result finalized by the Arc validator quorum. The proof must open every link; treating the RPC-returned `receiptsRoot` as an unauthenticated standalone input would not establish this binding.

## Live golden vectors

Both fixtures were captured from `https://rpc.testnet.arc.network` and include the block header, `arc_getCertificate`, and the `H-1` validator registry result.

| Fixture | Height | Block hash | Receipts root | Signatures | Validators | Total power |
|---|---:|---|---|---:|---:|---:|
| `arc-block-a.json` | 59,326,947 | `0x8f15ff19e2e49eefb75f557e12ddb5830aa1a560909de6fa7f69ba3f4f758354` | `0x1a3ad7378aeff5fcc7200810c4ffbefa13869fa7b24859f005f8ab27138d3b07` | 20 | 20 | 29,002 |
| `arc-block-b.json` | 59,326,923 | `0x1fa88a5642fc483660dda84cf02696d2e3b8d3797e52663d5ac746c23421d1f5` | `0x4a7cdaa1853921f894317264b8eeeac68166d1b4bc352fa301d47f4bcf406c9f` | 20 | 20 | 29,002 |

Re-capture one fixture with:

```bash
cargo run -p arc-conformance --bin capture -- \
  https://rpc.testnet.arc.network <height> <output.json>
```
