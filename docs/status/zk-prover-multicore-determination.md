# ZK Prover Multicore Determination

Status: Phase 2 complete
Date: 2026-06-20
Repo: `postfiatl1v2`

## Finding

Stock `halo2_proofs 0.3.2` already exposes and enables multicore proving in
this workspace. No dependency fork or feature-flag patch was required for
Phase 2.

Feature evidence:

```text
halo2_proofs feature "multicore"
  maybe-rayon feature "threads"
    maybe-rayon feature "rayon"
orchard feature "multicore"
  halo2_proofs feature "multicore"
```

The upstream crate confirms the feature:

```text
halo2_proofs 0.3.2 default = ["batch", "multicore"]
multicore = ["maybe-rayon/threads"]
maybe-rayon threads = ["rayon"]
```

## Empirical Control

Same benchmark, same host, release mode:

```bash
cargo test -p postfiat-privacy-orchard \
  zk_prover_baseline_benchmark \
  --release -- --ignored --nocapture
```

Default Rayon thread count on the 32-vCPU box:

```text
pk_build_ms          341,879
prove_ms              10,515
vk_build_ms           18,081
verify_ms                 88
wall_time_ms         370,690
proof_bytes            6,880
K                         16
```

Single-thread control:

```bash
RAYON_NUM_THREADS=1 cargo test -p postfiat-privacy-orchard \
  zk_prover_baseline_benchmark \
  --release -- --ignored --nocapture
```

```text
pk_build_ms          436,047
prove_ms              69,389
vk_build_ms          105,968
verify_ms                637
wall_time_ms         612,200
proof_bytes            6,880
K                         16
```

## Speedup

```text
pk_build speedup      1.28x
prove speedup         6.60x
vk_build speedup      5.86x
verify speedup        7.24x
wall-time speedup     1.65x
```

## Interpretation

Multicore is active and materially improves the proof and verifier steps. It
does not solve the operator-visible path because the current swap builder still
builds the proving key and then verifies with a newly built verifying key inside
the same CLI invocation.

Current best CPU proof time with stock multicore Halo2 on this host is:

```text
prove only ~= 10.5 seconds
```

Current operator-visible local build path is:

```text
pk build + prove + vk build + verify ~= 370 seconds
```

Therefore the next optimization should remove hot-path key generation before
attempting riskier circuit rewrites or Halo2 fork migration.
