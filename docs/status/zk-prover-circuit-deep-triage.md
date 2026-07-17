# ZK Prover Circuit Deep Triage

Status: Phase 4 safe-follow-up triage
Date: 2026-06-20
Repo: `postfiatl1v2`

## Purpose

After the K=15 reduction, the sprint checked whether there was an obvious safe
follow-on circuit optimization that could be landed without redesigning the
AssetOrchard circuit or changing cryptographic assumptions.

Result: no further low-risk code change was identified in this pass.

## K Boundary

```text
K=15 full circuit MockProver: passed
K=14 full circuit MockProver: failed with NotEnoughRowsAvailable { current_k: 14 }
```

`K=15` is therefore the smallest safe parameter set without reducing
constraints.

## Sinsemilla Note Commitments

Files checked:

```text
crates/privacy_orchard/src/asset_orchard_sinsemilla.rs
crates/privacy_orchard/src/asset_orchard_circuit.rs
```

Current shape:

```text
ASSET_ORCHARD_SINSEMILLA_PIECE_WORDS = 25
ASSET_ORCHARD_NOTE_COMMIT_MAX_WORDS = 200
message layout hash pinned in ASSET_ORCHARD_SWAP_V1_NOTE_MESSAGE_LAYOUT_HASH
```

The asset note commitment path uses the stock `halo2_gadgets::sinsemilla`
commitment machinery plus a local assigned-subpiece wrapper to bind the
asset-typed message:

```text
pool_domain || asset_tag_lo || asset_tag_hi || g_d || pk_d || value || rho || psi
```

No safe one-line row reduction was identified. Changing piece packing, word
count, or fixed-base/table layout would change the pinned note-message layout
and must be treated as a circuit redesign with new host/gadget equivalence
tests, VK pin refresh, soundness regression, and review.

## Lookup / Range Configuration

The full swap circuit configures a shared `PallasLookupRangeCheckConfig` and
Sinsemilla lookup columns used by:

```text
AssetOrchardSinsemillaChip
AssetOrchardEccChip
Merkle Sinsemilla chips
asset-tag/value bit range checks
```

The current API does not expose a local "wider table" knob that can be changed
without rewiring the halo2_gadgets configuration. A lookup-width experiment is
therefore a real gadget refactor, not a sprint-safe parameter flip.

## Poseidon

The circuit uses:

```text
halo2_poseidon::P128Pow5T3
width = 3
rate = 2
```

Round constants and MDS are pinned by:

```text
ASSET_ORCHARD_SWAP_V1_POSEIDON_PARAMETER_HASH
```

Reducing full or partial rounds would change the hash security margin and the
pinned parameter hash. That is not an acceptable latency optimization without
cryptographic review.

## Conclusion

The sprint landed the safe CPU changes:

```text
in-process key cache
K=16 -> K=15
```

The remaining CPU proof time is:

```text
K=15 hot proof ~= 5.78s
```

Further CPU optimization should start with profiling, not blind edits:

```bash
cargo flamegraph -p postfiat-privacy-orchard \
  --test postfiat_privacy_orchard \
  -- zk_prover_cached_key_benchmark --ignored --nocapture
```

If `cargo flamegraph` is unavailable, use:

```bash
perf record -F 997 -g --call-graph dwarf -- \
  cargo test -p postfiat-privacy-orchard \
  zk_prover_cached_key_benchmark \
  --release -- --ignored --nocapture
```

Expected next code-level targets after profiling:

- reduce Sinsemilla note-commitment rows;
- reduce Merkle path rows;
- tune lookup/range table layout;
- add a long-lived local prover service so one-shot CLI flows do not pay
  proving-key build.
