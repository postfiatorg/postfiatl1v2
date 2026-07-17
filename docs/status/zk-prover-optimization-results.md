# ZK Prover Optimization Results

Status: final sprint report
Date: 2026-06-20
Repo: `postfiatl1v2`
Branch: `navcoin-market-ops-envelope`

## Executive Result

The AssetOrchard shielded-swap proof path was optimized and measured on the
32-vCPU CPU box.

Best measured CPU hot path:

```text
K=15 cached proving key
prove_ms      5,780
verify_ms        66
proof_bytes   6,816
```

Baseline before optimization:

```text
K=16 cold path
pk_build_ms    341,879
prove_ms        10,515
vk_build_ms     18,081
verify_ms           88
proof_bytes      6,880
```

The sprint reduced repeated long-lived-process swap proving from roughly:

```text
K=16 hot prove + verify ~= 10.0 seconds
```

to:

```text
K=15 hot prove + verify ~= 5.85 seconds
```

The `<5s` CPU target was not reached. The remaining gap is about `0.8-0.9s`
on this host and likely requires deeper constraint reduction or GPU proving.

## Hardware

```text
CPU(s)             32
Model              AMD EPYC Processor
Threads/core       2
Cores/socket       16
Sockets            1
NUMA nodes         1
Kernel             Linux 6.8.0-55-generic x86_64
```

## Results Table

| Tier | Command / condition | K | pk build ms | prove ms | vk build ms | verify ms | proof bytes | Notes |
|---|---|---:|---:|---:|---:|---:|---:|---|
| Baseline | default Rayon, cold build | 16 | 341,879 | 10,515 | 18,081 | 88 | 6,880 | Original measured CPU baseline |
| Multicore control | `RAYON_NUM_THREADS=1` | 16 | 436,047 | 69,389 | 105,968 | 637 | 6,880 | Proves multicore is active |
| Key cache | default Rayon, hot second proof | 16 | 0 hot lookup | 9,909 | 0 hot lookup | 91 | 6,880 | Removes repeated keygen in long-lived processes |
| K reduction | default Rayon, cold build | 15 | 330,005 | 5,841 | 10,233 | 63 | 6,816 | Circuit fits at K=15 |
| K reduction + cache | default Rayon, hot second proof | 15 | 0 hot lookup | 5,780 | 0 hot lookup | 66 | 6,816 | Best measured CPU hot path |
| Thread tuning | `RAYON_NUM_THREADS=16` | 15 | 331,835 | 5,951 | 10,252 | 43 | 6,816 | Did not improve proving |

## Measured Speedups

Multicore versus single-thread at K=16:

```text
prove speedup   6.60x
verify speedup  7.24x
wall speedup    1.65x
```

K=15 versus K=16:

```text
cold prove speedup   10,515 / 5,841 = 1.80x
hot prove speedup     9,909 / 5,780 = 1.71x
proof size reduction  6,880 -> 6,816 bytes
```

Operator-visible cold one-shot CLI path:

```text
K=16 cold path  ~= 370.7s
K=15 cold path  ~= 346.3s
```

The one-shot CLI path remains keygen dominated because the process exits after
one proof. The cache optimization is aimed at long-lived prover/validator
processes and repeated swaps.

## Landed Code Changes

Commits pushed to `origin/navcoin-market-ops-envelope`:

```text
3d6b831b  Benchmark AssetOrchard swap prover baseline
12cbaaa4  Document AssetOrchard multicore prover behavior
6c891a67  Cache AssetOrchard swap proving keys
e9b8da1a  Reduce AssetOrchard swap circuit to K15
15de6fe3  Document Halo2 backend migration decision
e493db74  Scope ICICLE GPU prover deployment
this commit  Document circuit deep-triage outcome
```

Code changes:

- added ignored release benchmark `zk_prover_baseline_benchmark`;
- added ignored release benchmark `zk_prover_cached_key_benchmark`;
- added fallible in-process caches:
  - `AssetOrchardSwapProvingKey::cached()`;
  - `AssetOrchardSwapVerifyingKey::cached()`;
- moved swap build and consensus verification to cached keys;
- reduced `ASSET_ORCHARD_SWAP_V1_K` from `16` to `15`;
- updated K=15 params/VK attestation pins;
- updated stale legacy `ShieldedSwapV1` test expectation to preserve
  fail-closed-before-transcript behavior.

## Verification

Green checks run after the key-cache optimization:

```bash
cargo test -p postfiat-privacy-orchard
cargo test -p postfiat-node shielded_swap
cargo test -p postfiat-privacy-orchard \
  swap_consensus_verifier_accepts_real_proof_and_rejects_forged_nonconservation \
  --release -- --ignored --nocapture
```

Green checks run after the K=15 consensus parameter change:

```bash
cargo test -p postfiat-privacy-orchard
cargo test -p postfiat-node shielded_swap
cargo test -p postfiat-privacy-orchard \
  swap_full_shape_key_metadata_is_pinned_and_consistent \
  --release -- --ignored --nocapture
cargo test -p postfiat-privacy-orchard \
  swap_consensus_verifier_accepts_real_proof_and_rejects_forged_nonconservation \
  --release -- --ignored --nocapture
```

Release evidence:

```text
K=15 metadata pin test                       passed, 341.12s
K=15 forged-nonconservation soundness test   passed, 351.93s
```

## Negative Findings

- `K=14` does not fit. MockProver failed with
  `NotEnoughRowsAvailable { current_k: 14 }`.
- `RAYON_NUM_THREADS=16` did not improve proof generation versus default Rayon.
- The visible alternative crate `halo2-axiom 0.5.1` is not a safe drop-in; it
  is a KZG/nightly/trusted-setup backend from the Axiom/PSE line.
- The Phase 4 deep-triage pass found no safe one-line Sinsemilla, lookup, or
  Poseidon reduction after K=15; further CPU work should be profile-driven.
- No GPU benchmark was run because the sprint scoped GPU integration only and
  the AKT path is awaiting funding.

## Remaining Work

To get below 5 seconds on CPU:

- profile the K=15 proof with flamegraph/perf;
- inspect Sinsemilla note-commitment row use and lookup density;
- reduce gadget rows enough to fit more comfortably or cut proof work inside
  K=15;
- consider a long-lived local prover daemon so one-shot CLI usage does not pay
  proving-key build every time.

To get sub-second proving:

- implement the ICICLE GPU branch scoped in
  `docs/status/icicle-gpu-prover-scope.md`;
- measure RTX 4090/A5000/A6000/H100 class hardware;
- require CPU verifier acceptance or protocol-versioned verifier changes;
- keep forged-nonconservation rejection green.
