# ZK Prover K=15 Circuit Optimization

Status: circuit-parameter optimization benchmarked
Date: 2026-06-20
Repo: `postfiatl1v2`

## Change

The AssetOrchard full swap circuit was reduced from `K=16` to `K=15`.

Updated pinned metadata:

- `ASSET_ORCHARD_SWAP_V1_K = 15`
- `ASSET_ORCHARD_SWAP_V1_PARAMS_HASH =
  9be0057af858459fe2b4545dec144e83f4951be0bef2bc90e30e5f26e75f88ba69f1be10ac376a6af5ce973c6b7ad0d8`
- `ASSET_ORCHARD_SWAP_V1_VK_HASH =
  685e00bc2adbe3af9b8c524c7a7ba5aa452760d323390a1184159553757c655daf05e4e8b23c9a371b1516cd8319d24b`

The public instance layout, Poseidon parameter hash, note-message layout hash,
Merkle depth, and Merkle parameter hash are unchanged.

## Fit Checks

K=15 full note-commitment/Merkle/spend-auth MockProver:

```bash
cargo test -p postfiat-privacy-orchard \
  swap_circuit_recomputes_input_and_output_note_commitments \
  --release -- --ignored --nocapture
```

Result:

```text
K=15: passed
finished in 319.79s
```

K=14 scratch fit check:

```text
K=14: failed
NotEnoughRowsAvailable { current_k: 14 }
finished in 317.43s
```

Conclusion: `K=15` is the smallest viable parameter set without reducing
constraints.

## Metadata Pin Check

```bash
cargo test -p postfiat-privacy-orchard \
  swap_full_shape_key_metadata_is_pinned_and_consistent \
  --release -- --ignored --nocapture
```

Result:

```text
passed
finished in 339.59s
```

## Benchmark

Default Rayon on the 32-vCPU host:

```bash
cargo test -p postfiat-privacy-orchard \
  zk_prover_baseline_benchmark \
  --release -- --ignored --nocapture
```

```text
pk_build_ms          330,005
prove_ms               5,841
vk_build_ms           10,233
verify_ms                 63
proof_bytes            6,816
K                         15
wall_time_ms         346,270
```

Cached-key hot path:

```bash
cargo test -p postfiat-privacy-orchard \
  zk_prover_cached_key_benchmark \
  --release -- --ignored --nocapture
```

```text
cold_pk_lookup_ms     329,543
first_prove_ms          5,729
cold_vk_lookup_ms       9,478
first_verify_ms            62
hot_pk_lookup_ms            0
second_prove_ms         5,780
hot_vk_lookup_ms            0
second_verify_ms           66
proof_bytes             6,816
K                          15
```

Thread-count check:

```bash
RAYON_NUM_THREADS=16 cargo test -p postfiat-privacy-orchard \
  zk_prover_baseline_benchmark \
  --release -- --ignored --nocapture
```

```text
prove_ms    5,951
verify_ms      43
```

`RAYON_NUM_THREADS=16` did not improve proof generation versus default Rayon on
this host.

## Delta Versus K=16

```text
K=16 prove_ms       10,515
K=15 prove_ms        5,841
speedup              1.80x

K=16 hot prove_ms    9,909
K=15 hot prove_ms    5,780
speedup              1.71x

K=16 proof_bytes     6,880
K=15 proof_bytes     6,816
```

The K reduction does not reach the `<5s` CPU target, but it closes most of the
gap without changing the circuit statement or weakening soundness. Further CPU
improvement likely requires reducing constraints inside the Sinsemilla/Merkle
gadget stack or moving proof generation to a GPU backend.
