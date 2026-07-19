# ML-DSA-65 SP1 acceleration evidence

Date: 2026-07-18  
Dedicated box: Vast contract 45269977, RTX 5090, SP1 6.3.1  
Result: all correctness, execute-equivalence, and CUDA prove-and-verify gates passed

## Reproducibility anchors

- Git baseline: `0949640af29a8750ab5fb37da9b1da2c201d1afd`
- Witness SHA-256:
  `b5645fde0d8c13438be5218fc91b0cb5d275405ab1aba068374ca3e3874a7b46`
- Witness shape: 26 blocks, 468 ML-DSA-65 verification calls, 6 distinct keys
- Reference ELF SHA-256:
  `657cf348b572f497f226812e24ed053b5ebcba3aa004f78bb2216fe56000e5e0`
- Accelerated ELF SHA-256:
  `3c7c6ef8d0395ce1ad00dc0b8d84613c44600c433fe4d559e5481649f93078ea`
- Accelerated program vkey:
  `0x008b69a0676137455435bbe635752c1a60c29710a4f9160666ea52a0240d1f10`
- Both variants include the official SP1 6.0.0 SHA2/SHA3 patches. The reference
  is feature-disabled and retains the upstream `fips204 0.4.6` formulas.

## Outcome

| Metric | Reference | Accelerated | Change |
|---|---:|---:|---:|
| Execute cycles | 1,424,788,289 | 1,050,064,162 | -26.30% |
| Harness elapsed | 20,914 ms | 16,754 ms | -19.89% |
| End-to-end wall | 27.72 s | 23.50 s | -15.22% |
| Maximum RSS | 11,160,140 KiB | 10,647,548 KiB | -4.59% |

The two public-values files compare byte-for-byte equal: 1,486 bytes, SHA-256
`dfe21e43169ba1d262abbc941dd66dab8747503414b3c79db7968c813ca8d0e8`.

The tracked verify region fell from 695,624,363 to 320,792,959 cycles
(-53.88%). Public-key preparation labels ran six times instead of 468, and the
optimized matrix/vector loop fell from 65,553,696 to 34,846,344 cycles.
The clean reference is within 0.082% of the frozen gate's
1,425,953,847-cycle hash-precompile control.

## Correctness runs

```text
cargo test --manifest-path third_party/fips204/Cargo.toml \
  --features precompute-verify-matrix,riscv32-verify-arithmetic
cargo test --locked -p postfiat-crypto-provider
cargo test -p postfiat-crypto-provider \
  --features mldsa-guest-acceleration
cargo test --locked -p postfiat-pfusdc-proofs \
  --features mldsa-guest-acceleration
```

- Vendored FIPS suite: 59 passed, 0 failed, 1 ignored infinite stress test.
- NIST ACVP internal-projection `keyGen`, `sigGen`, and `sigVer`: passed.
- Reference feature state: 5 passed, 0 failed.
- Accelerated feature state: 6 passed, 0 failed.
- Accelerated pfUSDC proof integration: 4 passed, 0 failed.
- Differential inputs: 16 deterministic valid cases, 96 targeted bit flips,
  wrong message/context/length negatives, 512 deterministic mutations, and a
  65-key cache-eviction sequence.

## CUDA proof

The proof command used `SP1_PROVER=cuda` and verified the resulting Groth16
proof locally. A sampled core-proving interval showed 65% utilization and
22,006 MiB VRAM on the RTX 5090. With the versioned circuit already cached, the
run took 139.58 s total and the proof report records 122,161 ms for setup and
proving.

- Proof calldata: 356 bytes, SHA-256
  `9f20c804609f9b002c867da722b14daa3cb409996a393ce4dafa8f9541e16d02`
- Serialized proof: 3,181 bytes, SHA-256
  `71c1595f940ddc172878a13b9e237b32306ac292692f97ffa8c239ddef960fc3`
- Proof public values: identical to native reference

## Files

- `reference-cycle-breakdown.json`: pre-change component cycles and calls
- `accelerated-cycle-breakdown.json`: post-change component cycles and calls
- `reference-execute-report.json` / `accelerated-execute-report.json`: controlled delta
- `reference-time.log` / `accelerated-time.log`: wall and peak-RSS evidence
- `cuda-execute-report.json` / `cuda-proof-report.json`: proof-run reports
- `cuda-time.log`: CUDA prove-and-verify wall/RSS log
- `cuda-proof.bin` / `cuda-proof-calldata.bin`: verified proof artifacts
- `program-info.json`: final ELF hash and SP1 program vkey
- `public-values.bin`: verified canonical guest output
- `tih-critique-and-response.md`: the one-shot TIH critique and applied changes

No deployment, devnet, CI, canonical-worktree write, gate-worktree write, or
gate-box action was performed.
