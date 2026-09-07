# Arc ingress proving benchmark

Status: **To-do 3.5 accepted** on 2026-08-28 UTC. The measured Groth16 proof
generation time was **54.420955902 GPU-seconds**, below the 60-second target
and the three-minute hard ceiling.

## Reproduction

The run used a freshly rented Vast.ai NVIDIA A100 SXM4 80 GB instance and the
repository's corrected Arc ingress ELF. The prover was built with
`--features sp1-cuda`, SP1 circuit version `v6.1.0`, and proof mode
`groth16`. The input was the real corrected-route Arc testnet deposit witness
at block 59,335,780.

The full client interval reported by `setup_and_prove_ms` was 122,756 ms. That
interval includes client/server setup and circuit loading. The GPU prover's
own timed proof-generation phase was 54.420955902 s. The guest executed in
1,304 ms and 2,376,633 cycles. Local `client.verify` accepted the resulting
proof before it was copied into the evidence tree.

| Measurement | Value |
|---|---:|
| GPU | NVIDIA A100 SXM4 80 GB |
| SP1 circuit | v6.1.0 Groth16 |
| Guest execution | 1.304 s |
| Guest cycles | 2,376,633 |
| Groth16 generation | **54.420955902 s** |
| Setup plus prove client interval | 122.756 s |
| Proof calldata | 356 bytes |
| Public values | 314 bytes |
| Program vkey | `0x00b218e0ab7d2582baacca0dfaa8a5b211f258880ee44898797e109ae6b55ee0` |

The rental instance was destroyed immediately after the proof artifacts were
copied and checked. Its measured lifetime was approximately 33 minutes at
about USD 0.992 per hour, or approximately USD 0.55.

## Artifact integrity

All paths below are relative to `arc-ingress-proof/`.

| Artifact | SHA-256 |
|---|---|
| `cuda-prover.log` | `a23ec745a9b1eb7fa34929d3d58ac48a73a8cb356b84405a7771819e7f206bad` |
| `execute-report.json` | `af9550a9d2d6f644361ea843c1e21d5a3efbb1f10c69f4c481919660173ec0e8` |
| `proof-calldata.bin` | `f828055070885adb2b2e0f4c02733a3f6174a4493c94894331ba76d533513fe1` |
| `proof-report.json` | `8ad26c459ca7a3bc0e25780abf3116bf08fffd3153465e583159ad6b10a570c3` |
| `proof.bin` | `0a01aedb26b7df6de94c40d930f6de3b450ba2e7d2b2dd446bcb81fa268f5fa1` |
| `public-values.bin` | `cd985257ccb45f8dd3c54c0dbdec8f63e3ab6ee14e8846485e3b5749032f074b` |
| `public-values.sha3-384` | `30f6a0ea59902730126da341fda63051906a9a5d31282e432e07b25bf6542da4` |

The proof's public values are byte-identical to the prior CPU execution output
in `arc-ingress-execute/public-values.bin`.
