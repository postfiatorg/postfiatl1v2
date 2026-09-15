# Egress ELF and Groth16 vkey pin

Generated fresh from this branch on 2026-08-28 UTC:

```bash
cargo run --release --manifest-path tools/pfusdc-tier4-prover/Cargo.toml -- \
  program-info --output docs/evidence/arc-mvp-20260828/program-info.json
```

| Field | Value |
|---|---|
| SP1 SDK crate | `6.3.1` |
| SP1 circuit version | `v6.1.0` |
| Egress ELF SHA-256 | `8e2464227d7428d9928871c4a655fd73f6a87879c2e8eae6c0228a5db367f7bd` |
| Egress program vkey | `0x00c8d744e19bc828d1b3fb19709d36863d8c5aba14af0ca939eb85fc806f868f` |
| Groth16 verifier hash | `0x4388a21c687fdd5f218d7e3d13190cac4c5355818d3605fd5fb811df468ee696` |
| Gateway selector | `0x4388a21c` |

`program-info.json` is the machine-readable output. The program vkey is a
commitment derived by SP1 setup from the exact embedded ELF; it is not the ELF's
ordinary SHA-256 digest. `PFTLFinalityVerifierV1.programVKey()` must read back as
the value above. A historical document's `0x008b...` program vkey belongs to an
older ELF and must not be deployed.

The `ingress` object in that inherited Tier-4 report describes the legacy Nitro
guest. It is retained only so the existing command remains backwards
compatible; it is not an Arc ingress vkey and must not be registered for chain
5,042,002. The Arc ingress ELF/vkey will be generated only after G0 closes.

The host wrapper now requests `.groth16()` explicitly. Historical performance
evidence for this exact ELF/vkey used Plonk and is only a runtime benchmark; it
does not satisfy Gate 2.4. Gate 2.4 still requires a fresh Groth16 proof, local
host verification, successful Arc verification, and a recorded Arc receipt.
