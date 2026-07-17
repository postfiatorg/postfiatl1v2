# ZK Prover Baseline Benchmark

Status: Phase 1 measured baseline
Date: 2026-06-20
Repo: `postfiatl1v2`
Commit measured: `302dc058`

## Scope

This report measures the current AssetOrchard shielded-swap proof path before
any prover optimization. The measured circuit is the real full swap circuit:

- crate: `postfiat-privacy-orchard`
- circuit: `AssetOrchardSwapConservationCircuit`
- proof wrapper: `AssetOrchardSwapProvingKey::create_proof`
- verifier wrapper: `AssetOrchardSwapVerifyingKey::verify_proof`
- K: `16`
- proof bytes: `6,880`

The benchmark constructs two valid asset-typed input notes, two valid output
notes, a Merkle anchor/witness, nullifiers, randomized verification keys, public
instance, and a real Halo2 proof. The proof is then verified against the swap
verifying key.

## Hardware

```text
CPU(s)             32
Model              AMD EPYC Processor
Threads/core       2
Cores/socket       16
Sockets            1
NUMA nodes         1
Kernel             Linux 6.8.0-55-generic x86_64
available_parallelism reported by Rust: 32
```

## Command

```bash
cargo test -p postfiat-privacy-orchard \
  zk_prover_baseline_benchmark \
  --release -- --ignored --nocapture
```

CPU utilization was sampled once per second from the Rust test process and its
threads using `ps -L`.

## Results

Second clean run, after compilation was already warm:

```text
pk_build_ms          341,879
baseline_prove_ms     10,515
vk_build_ms           18,081
baseline_verify_ms        88
proof_bytes            6,880
K                         16
available_parallelism     32
test_wall_time_ms    370,690
```

First run was consistent:

```text
pk_build_ms          341,753
baseline_prove_ms     10,431
vk_build_ms           17,417
baseline_verify_ms        95
proof_bytes            6,880
K                         16
available_parallelism     32
test_wall_time_ms    369,860
```

## CPU Utilization

The feature tree already has `halo2_proofs 0.3.2` with the `multicore` feature
enabled through Orchard/Halo2 defaults. The process does create Rayon worker
threads.

Observed thread/process utilization from the second run:

```text
sample_ticks                         241
asset_orchard_process_ticks          240
peak_total_pcpu                    3,084.6   # about 30.8 cores
average_total_pcpu                   245.2   # about 2.45 cores
ticks >= 24 cores                        1
ticks >= 4 cores                        21
low-utilization ticks                  218
```

Interpretation:

- Rayon/multicore is active, but it is not the main missing switch.
- The prover briefly fans out across most cores, especially early in key
  generation, but most of the long baseline run uses far fewer cores.
- The current end-to-end CLI path is dominated by proving-key generation, not by
  proof verification.

## Baseline Determination

The current stock CPU path is not near the `<5s` target when measured as the
operator-visible `asset-orchard-swap-create` style path:

```text
pk build + prove + vk build + verify ~= 370 seconds
```

The actual proof generation step alone is closer but still above target:

```text
prove only ~= 10.5 seconds
verify only ~= 0.09 seconds
```

The highest-impact optimization is therefore not a circuit rewrite first. It is
removing repeated proving/verifying key generation from the hot transaction path
and then reducing the remaining ~10.5s proof time.

## Evidence Files

Local benchmark artifacts from this run:

```text
/tmp/zk-prover-baseline-v2.out
/tmp/zk-prover-baseline-cpu-samples-v2.tsv
```

These are local scratch files and are not committed.
