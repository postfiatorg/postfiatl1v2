# Arc receipt inclusion conformance

`crates/arc-conformance/fixtures/arc-receipts.json` was captured from live Arc testnet block 59,327,040 (`0xb3dc7a1a3a6ab2e4f24dfc688e0f0e5666c8068003780fcf63bc9b9d5fb3c776`). Its header receipts root is `0xfd11957fa6a786ff69dcf1e2ef5bbe2471b05cf960dd1f61775f67bad8e57800`.

| Transaction | Index | Receipt encoding | Proof nodes |
|---|---:|---|---:|
| `0xd148a8e253fe119ec8264011187c634446aed18df609229c5eef97c5145cfdc6` | 0 | EIP-2718 type `0x02` | 2 |
| `0x8c3aff58cbf16259bb21e03fd29b55f05993e5d73bbe4b256101b684f4546928` | 1 | legacy RLP | 3 |

The capture path fetches every receipt in the block, reproduces canonical receipt RLP including the EIP-2718 envelope, reconstructs the receipt trie, requires its root to equal the block header, and retains inclusion paths for both target transactions. The verifier uses the same pinned `alloy-trie = 0.9.5` dependency as `programs/pfusdc-ingress`, with limits of 16,384 receipts, 64 proof nodes, and 16,384 bytes per node.

Re-capture with:

```bash
cargo run -p arc-conformance --bin capture_receipts -- \
  https://rpc.testnet.arc.network <height> <output.json>
```

The `receipt_inclusion_conformance` test verifies both paths and then alters each encoded receipt in turn; both mutations fail with `InvalidProof`.
