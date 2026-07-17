# ZK Prover Backend/Fork Decision

Status: Phase 3 decision
Date: 2026-06-20
Repo: `postfiatl1v2`

## Decision

Do not migrate away from stock `halo2_proofs 0.3.2` in this sprint.

The current dependency is already the Zcash/ECC Halo2 line used by Orchard:

```text
halo2_proofs = "0.3.2"
repository   https://github.com/zcash/halo2
default      ["batch", "multicore"]
multicore    ["maybe-rayon/threads"]
```

Workspace dependency tree:

```text
postfiat-privacy-orchard
├── halo2_gadgets v0.5.0
│   ├── halo2_poseidon v0.1.0
│   └── halo2_proofs v0.3.2
├── halo2_poseidon v0.1.0
├── halo2_proofs v0.3.2
└── orchard v0.14.0
    ├── halo2_gadgets v0.5.0
    ├── halo2_poseidon v0.1.0
    └── halo2_proofs v0.3.2
```

The Phase 2 benchmark already proved that multicore is active:

```text
default Rayon prove_ms        10,515 at K=16
RAYON_NUM_THREADS=1 prove_ms  69,389 at K=16
```

## Alternative Checked

`cargo search halo2_proofs` shows `halo2_proofs = "0.3.2"` as the current crate
version. The visible alternative `halo2-axiom = "0.5.1"` is not a safe drop-in
for this sprint:

```text
repository    https://github.com/axiom-crypto/halo2
lineage       fork of PSE halo2, itself forked from Zcash halo2
backend       KZG polynomial commitment scheme
setup         one-time universal trusted setup required
toolchain     nightly Rust
```

The AssetOrchard design is Orchard/ECC/Halo2-IPA aligned. Moving to an
Axiom/PSE KZG backend would change the proof-system assumptions and setup model,
not merely speed up the existing circuit. That is a separate cryptographic
review and migration project, not an overnight optimization.

## Result

Keep `halo2_proofs 0.3.2` and optimize inside the current backend:

- multicore remains enabled;
- K was reduced from 16 to 15;
- in-process key caching removes repeated keygen in long-lived processes;
- GPU proving is the next backend path to scope.
