# ZK Prover Key Cache Optimization

Status: optimization landed locally, benchmarked
Date: 2026-06-20
Repo: `postfiatl1v2`

## Change

The AssetOrchard full swap proving and verifying keys are now cached in process
with fallible `OnceLock`s:

- `AssetOrchardSwapProvingKey::cached()`
- `AssetOrchardSwapVerifyingKey::cached()`

The existing `build()` methods remain available for release-pin and metadata
tests. The hot transaction paths now use the cached handles:

- `build_asset_orchard_swap_action(...)` uses the cached proving key.
- `verify_serialized_asset_orchard_swap_action(...)` uses the cached verifying
  key.

This does not change the circuit, public instance, proof bytes, VK pin
constants, or consensus semantics. It removes repeated full key generation in
long-lived processes.

## Benchmark

Command:

```bash
cargo test -p postfiat-privacy-orchard \
  zk_prover_cached_key_benchmark \
  --release -- --ignored --nocapture
```

Result:

```text
cold_pk_lookup_ms     341,142
first_prove_ms         10,054
cold_vk_lookup_ms      20,095
first_verify_ms            94
hot_pk_lookup_ms            0
second_prove_ms         9,909
hot_vk_lookup_ms            0
second_verify_ms           91
proof_bytes             6,880
K                          16
available_parallelism      32
```

## Measured Delta

Baseline operator-visible local key/proof/verify path:

```text
pk build + prove + vk build + verify ~= 370 seconds
```

Hot in-process path after cache warmup:

```text
prove + verify ~= 10.0 seconds
```

The optimization removes repeated key construction from long-lived prover and
validator processes. It does not by itself make a one-shot CLI invocation fast,
because a one-shot process still has to build the proving key the first time it
starts.

## Validator Impact

For long-lived validators, the first `shielded_swap_v1` verification still pays
the VK build. Later swaps reuse the same pinned key and only pay proof
verification. This should materially reduce repeated certification/apply
latency for multiple swaps after warmup.

## Remaining Bottleneck

The fastest measured CPU proof generation on this host is still:

```text
prove only ~= 9.9-10.5 seconds
```

The next optimization target is therefore the circuit/prover work itself:

- test whether the full circuit fits at `K=15`;
- inspect row/lookup utilization;
- evaluate whether the Halo2 backend has a compatible faster fork or whether
  GPU proving is the next realistic path.
