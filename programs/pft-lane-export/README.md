# Lane export program

The SP1 guest for PFT exports from an external lane to Base. It reads the versioned CBOR `ExternalLaneExportProgramInputV1`, a `withdrawal` (one export) or a `checkpoint`. It runs `verify_external_lane_export_witness_v1` or `verify_external_lane_checkpoint_witness_v1` (`crates/pft_lane_proofs`) and commits `canonical_bytes_without_commitment()` of the result. The statement is in the design note, section "Base half, step 3" (`docs/specs/pft-erc20-base-bridge-phase3-export-design-20261105.md`).

It has its own key. The pfUSDC egress program (`programs/pfusdc-egress`) binds a burn receipt and the bridge-exit root, which a lane export does not have. That program and its key are unchanged.

## Release identity

`elf/pft-lane-export-program` and `program-info.json` are the program's identity:

- ELF sha256: `8ac7696a5912eea37715b80798124baae0b81f8762d9351d1236737a980a1188`
- Verifying key: `0x00d5ebebe8e4551cf99ecfcea71f48957e61ad2fde1eab96357ddee66148ab65`

`releases.json` records how they were produced, the latest release last:

- **Workflow:** `pft-lane-export-identity`, runs 43226356342, 43227408081 and 43227528337 at `c1a8a7f7a`, `32c7db71b` and `2181cc42c`. The guest source did not change; the fence status merge `c1a8a7f7a` grew `crates/types`, which the guest links, and moved the ELF and the key. The first release (`e92f2004…`, key `0x000cef5b…0a81414`, run 42818337185 at `439803aaa`) is pinned by no verifier on any network; only the local fork rehearsal of 2026-11-10 used it.
- **Builds:** each run built the guest twice from two clean trees in the pinned SP1 image `ghcr.io/succinctlabs/sp1@sha256:0942a27d…3400`, with `cargo prove build --docker --tag v6.3.1 --locked`. All six ELFs were byte-identical, and all three runs derived the same key.
- **Rebuild check:** the workflow rebuilds the guest on every push and pull request and fails when the ELF or the key differs from `program-info.json`.

**No proof has been generated with this key yet.**

Before any verifier is constructed with this key, all of these must hold:

1. This committed identity, unchanged.
2. The program's entry in the proof public-input inventory (`docs/status/OPEN-SOURCE-PROOF-PUBLIC-INPUT-INVENTORY-20260716.json`).
3. The identity check green on the commit being deployed.
4. The verifier's `programVKey` taken from `program-info.json` and cross-checked in the deployment record.
5. Before production: at least one real SP1 proof generated with this program and verified against the versioned SP1 verifier. None exists yet.
6. Before any verifier meant to last (anything beyond a rehearsal pair) is constructed with Succinct's `V6_1_0_SP1_VERIFIER_GROTH16` (`0xb69f2584CBcFf99a58C4e7002E8b89Af54a6f4e2` on Base Sepolia): one real proof of this program verified against that pinned verifier. It is the acceptance gate of the Base Sepolia dry run's `prove-and-release` stage. The repository's evidence that an SP1 6.3.1 Groth16 proof verifies against the v6.1.0 verifier (`docs/evidence/arc-mvp-20260828/deployments.md:107` and `conformance-test.log`) is for the Arc egress program only.

## Execute locally

Build the input with `postfiat-node external-lane-export-proof-input`, then build the guest (`cargo prove build`) and run it in SP1's CPU executor. No proof is generated:

```sh
SP1_PROVER=cpu tools/pft-lane-export-prover/target/release/pft-lane-export-prover execute \
  --elf PATH/pft-lane-export-program --input OUT/input.cbor --output-dir OUT/exec
```
