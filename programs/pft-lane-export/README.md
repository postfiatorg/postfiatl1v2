# Lane export program

The SP1 guest for PFT exports from an external lane to Base. It reads the versioned CBOR `ExternalLaneExportProgramInputV1`, a `withdrawal` (one export) or a `checkpoint`. It runs `verify_external_lane_export_witness_v1` or `verify_external_lane_checkpoint_witness_v1` (`crates/pft_lane_proofs`) and commits `canonical_bytes_without_commitment()` of the result. The statement is in the design note, section "Base half, step 3" (`docs/specs/pft-erc20-base-bridge-phase3-export-design-20261105.md`).

It has its own key. The pfUSDC egress program (`programs/pfusdc-egress`) binds a burn receipt and the bridge-exit root, which a lane export does not have. That program and its key are unchanged.

## Release status

No ELF and no `program-info` are committed yet. A release build must be the Docker-reproducible one that the proof-identity CI jobs check:

```sh
cargo prove build --docker --tag v6.3.1 --locked --output-directory elf
```

Run it on a host with Docker and SP1 6.3.1. Then record the ELF digest and verifying key in `program-info.json` in the neighbours' format, and add the program to the proof public-input inventory and the CI identity job.

## Execute locally

Build the input with `postfiat-node external-lane-export-proof-input`, then build the guest (`cargo prove build`) and run it in SP1's CPU executor. No proof is generated:

```sh
SP1_PROVER=cpu tools/pft-lane-export-prover/target/release/pft-lane-export-prover execute \
  --elf PATH/pft-lane-export-program --input OUT/input.cbor --output-dir OUT/exec
```
