# ML-DSA-65 verification acceleration for the SP1 egress guest

Status: validated for code review; not deployed  
Date: 2026-07-18  
Branch baseline: `0949640af29a8750ab5fb37da9b1da2c201d1afd`  
SP1: `6.3.1`; ML-DSA oracle: crates.io `fips204 = 0.4.6`

## 1. Validated recommendation

Use a guest-only combination of:

1. a bounded cache of fully prepared ML-DSA-65 public keys, including the
   expanded `A_hat` matrix and the existing `t1 * 2^d` NTT term; and
2. a vendored, pinned `fips204 0.4.6` with two algebraically equivalent
   RISC-V arithmetic changes: one Montgomery reduction per matrix/vector dot
   product and direct `R^2 mod q` conversion to Montgomery form.

Keep an explicit, feature-disabled reference verification path and compare the
guest's public values byte-for-byte with native reference execution. Do not add
an unconstrained host hint and call it a precompile. A true SP1 NTT syscall is a
promising follow-up, but it requires an SP1 executor/AIR change so that every
returned coefficient is constrained; that change is outside this repository's
consensus boundary.

The measurement changed the initial hypothesis. NTT and pointwise arithmetic
are hot, but repeated `ExpandA(rho)` is the largest measured ML-DSA component.
The best repository-local design therefore prepares and reuses the complete
public polynomial state, then optimizes the remaining pointwise loop. An
NTT-only rewrite would leave most of the avoidable cost in place.

## 2. Scope and invariants

The acceleration applies only to ML-DSA-65 verification inside the
`pfusdc-egress-program` SP1 guest. It does not change:

- FIPS 204 encodings, `q = 8_380_417`, dimensions `K = 6`, `L = 5`, or any
  acceptance predicate;
- signed bytes, contexts, committee/quorum rules, public-values encoding, or
  the proof statement;
- host/node signing or verification by default; or
- the SP1 machine, proof system, or syscall table.

Consensus invariant: for every byte tuple `(pk, message, signature, context)`,
the accelerated path must return exactly the same boolean as unmodified
crates.io `fips204 0.4.6`. Length errors and malformed encodings must reject,
never panic. Any unresolved mismatch disables the accelerated feature rather
than weakening the oracle.

## 3. Measurement method

### 3.1 Reproducible input and controls

The benchmark input is the archived 26-block egress witness copied read-only
from the frozen gate evidence to the dedicated benchmark box:

- witness SHA-256:
  `b5645fde0d8c13438be5218fc91b0cb5d275405ab1aba068374ca3e3874a7b46`;
- witness size: 5,505,597 bytes;
- ML-DSA-65 verify calls observed by the tracker: 468;
- distinct `public_key_hex` values: 6; and
- canonical public values: 1,486 bytes, SHA-256
  `dfe21e43169ba1d262abbc941dd66dab8747503414b3c79db7968c813ca8d0e8`.

The branch's checked-in V1 ELF has SHA-256
`8d2d5ce451bbd91c28f8fafcbd12f7bc961c6a4be59de12e246b8cb6734f81e8`.
On the dedicated 128-core box it reproduced the archived result exactly:
2,030,290,233 execute cycles, 15,272 ms inside the execute harness and 22.03 s
end-to-end wall time. The frozen hash-precompile-only control recorded by the
gate worktree is 1,425,953,847 cycles, a 29.77% reduction from V1.

All final ML-DSA measurements stack the same official SP1 6.0.0 SHA2/SHA3
patches beneath both variants. The locked, feature-disabled reference retains
the crates.io 0.4.6 arithmetic and has ELF SHA-256
`657cf348b572f497f226812e24ed053b5ebcba3aa004f78bb2216fe56000e5e0`.
It executes in 1,424,788,289 cycles, only 1,165,558 cycles (0.082%) below the
frozen gate result. This close reproduction is the controlled before value;
the accelerated comparison changes only the scoped ML-DSA guest feature.

### 3.2 Instrumentation

The vendored reference was instrumented with static SP1
`cycle-tracker-report-start/end` commands. The host tool was built with
`sp1-sdk/profiling`; production builds compile the guest hooks to no-ops. Labels
were placed around public-key and signature decoding, every forward and inverse
NTT, `ExpandA`, all other SHAKE work, matrix/vector and challenge/t1 pointwise
work, hint reconstruction, and commitment encoding. The report accumulates
cycles and invocation counts across the complete witness.

The final reference-instrumented ELF SHA-256 is
`df0981a91c63cbb444d2e51d7ad511bf4d8a3be464c2cd47bb6b6b843c37c144`.
It executed in 1,425,315,279 cycles. That total is not used as the optimization
baseline because instrumentation changes ELF layout; it is 526,990 cycles
(0.037%) above the clean reference. Component measurements below are inclusive
of their tracker region. Nested component rows are intentionally not added to
the whole-guest total.

### 3.3 Measured ML-DSA-65 breakdown

| Component | Calls | Total cycles | Cycles/verify | Share of tracked verify |
|---|---:|---:|---:|---:|
| SHAKE total | varies | 205,537,498 | 439,183 | 29.55% |
| Forward + inverse NTT | 1,872 | 167,882,832 | 358,724 | 24.13% |
| Pointwise polynomial arithmetic | 936 | 82,900,584 | 177,138 | 11.92% |
| Public-key + signature decode | 936 | 75,444,298 | 161,206 | 10.85% |
| Hint reconstruction + commitment encode | 936 | 42,199,539 | 90,170 | 6.07% |
| Other verify work/residual | — | 121,659,612 | 259,956 | 17.49% |
| **Tracked verify total** | **468** | **695,624,363** | **1,486,377** | **100.00%** |

Tracked verification is 48.80% of the instrumented whole guest. The detailed
labels are:

| Tracker label | Calls | Total cycles | Cycles/call |
|---|---:|---:|---:|
| `mldsa.shake.expand_a` | 468 | 196,832,292 | 420,582 |
| `mldsa.ntt.forward` | 1,404 | 84,959,784 | 60,513 |
| `mldsa.ntt.inverse` | 468 | 82,923,048 | 177,186 |
| `mldsa.pointwise.matvec` | 468 | 65,553,696 | 140,072 |
| `mldsa.decode.signature` | 468 | 39,911,866 | 85,282 |
| `mldsa.decode.public_key` | 468 | 35,532,432 | 75,924 |
| `mldsa.poly.use_hint` | 468 | 22,630,119 | 48,355 |
| `mldsa.encode.commitment` | 468 | 19,569,420 | 41,815 |
| `mldsa.pointwise.challenge_t1` | 468 | 17,346,888 | 37,066 |
| `mldsa.shake.public_key` | 468 | 3,385,512 | 7,234 |
| `mldsa.shake.message` | 468 | 2,178,578 | 4,655 |
| `mldsa.shake.commitment` | 468 | 1,578,564 | 3,373 |
| `mldsa.shake.challenge` | 468 | 1,562,552 | 3,339 |

One public-key parse performs one of the three forward NTT calls per verify;
the other two transform `z` and `c`. Preparing the six distinct keys once can
therefore remove 462 redundant public-key decodes, public-key hashes, `A_hat`
expansions, and t1 NTT preparations without changing per-signature inputs.

## 4. Design options

### 4.1 Option A: SP1 constrained NTT/polynomial syscall

Define fixed-shape ML-DSA operations for forward NTT, inverse NTT, and
matrix/vector or coefficientwise multiplication over `Z_q`. A guest call would
write a versioned operation header and fixed-length little-endian coefficients,
then receive fixed-length canonical coefficients.

Potential benefits:

- replaces thousands of RISC-V integer instructions with specialized trace
  rows;
- amortizes syscall overhead across a whole polynomial; and
- creates reusable lattice-cryptography acceleration in SP1.

Required soundness work:

- add the syscall to the SP1 executor and prove every input/output relation in
  an AIR chip or sound lookup argument;
- range-constrain coefficients and define canonical or centered output form;
- bind operation version, dimensions, `q`, zeta table, transform direction, and
  Montgomery-domain convention;
- reject misaligned pointers, overlapping buffers, wrong lengths, unsupported
  dimensions, and out-of-range inputs deterministically; and
- differential-test CPU executor, CUDA prover, and circuit behavior.

A host hook that returns an NTT result without an AIR constraint is unsound: a
malicious prover could choose coefficients that make an invalid signature pass.
This option also requires maintaining an SP1 fork and regenerating all program
identities. It remains the long-term highest-ceiling option, but is not the
recommended repository-local implementation.

### 4.2 Option B: vendored `fips204` tuned for 32-bit RISC-V

Pin and vendor exactly version 0.4.6, retain its API and algorithms, and make
small equivalence-auditable arithmetic changes:

- In the `K x L` NTT-domain matrix/vector product, accumulate the at-most-seven
  signed products in `i64` and Montgomery-reduce once per output coefficient.
  Montgomery reduction is linear modulo `q`; this removes `L - 1` reductions
  and intermediate output loads/stores. With `|a|, |b| < q` and `L <= 7`, the
  magnitude is below `7q^2 < 2^31q`, the FIPS Algorithm 49 input bound.
- Convert an NTT vector to Montgomery form with
  `MontgomeryReduce(x * (2^64 mod q))`, where
  `2^64 mod 8_380_417 = 2_365_951`. The result is congruent to `x * 2^32`
  modulo `q` and replaces a multi-step 64-bit Barrett-style reduction.

Benefits are no machine fork, ordinary SP1 proof coverage, and a small review
surface. The ceiling is lower than a constrained syscall, and vendoring creates
an explicit upstream-update obligation. This option is recommended as one part
of the implementation.

### 4.3 Option C: guest-only prepared ML-DSA-65 verifier

On first use of an exact 1,952-byte public key, perform the reference decode and
prepare:

- `tr = SHAKE256(pk)`;
- `NTT(t1 * 2^d)` in the expected Montgomery representation; and
- `A_hat = ExpandA(rho)`.

Cache that prepared reference `PublicKey`, keyed by all public-key bytes. The
egress witness contains 468 verifications but only six keys, so this targets the
largest measured repeated work. The verifier still executes the reference
signature decode, message/challenge/commitment hashes, transforms, norm checks,
hint reconstruction, and final comparison for every signature.

The cache is deterministic FIFO with a hard capacity of 64. It does not cache
invalid encodings. Exact byte keys avoid collision-dependent correctness, and
FIFO avoids platform-dependent hash iteration. The prepared payload for 64
ML-DSA-65 keys is about 2.4 MiB before container overhead and is strictly
bounded. A recovered poisoned mutex remains deterministic; the SP1 guest is
single-threaded, while native feature users cannot grow the cache without
bound.

This option has the best measured leverage and remains inside the proven guest
execution. Its risk is a larger divergence from upstream internals, addressed
by retaining the reference function and using an independent crates.io oracle.
It is recommended only under the egress guest feature.

## 5. Selected interface and integration

The workspace dependency `fips204` resolves to the checked-in, pinned vendor.
The vendor exposes three opt-in features:

- `precompute-verify-matrix`, which stores `A_hat` in a parsed public key and
  reuses it during verification;
- `riscv32-verify-arithmetic`, which enables the accumulated Montgomery
  dot-product and direct `R^2 mod q` conversion while leaving the
  feature-disabled control byte-for-byte aligned with upstream formulas; and
- `sp1-cycle-tracking`, which adds measurement-only static tracker commands.

`postfiat-crypto-provider` exposes:

```rust
pub fn ml_dsa_65_verify_with_context(
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
    context: &[u8],
) -> bool;

pub fn ml_dsa_65_verify_with_context_reference(
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
    context: &[u8],
) -> bool;
```

The first function uses the cache and RISC-V arithmetic only with feature
`mldsa-guest-acceleration`; otherwise it is the second function. The feature is
propagated through `postfiat-pfusdc-proofs` and enabled by default only in
`programs/pfusdc-egress`. The native prover tool does not enable it, so its
expected public values remain an independent reference-path check.

The egress input, output, and proof interfaces do not change. Enabling the
feature changes the ELF and therefore its SHA-256 and SP1 verifying key; those
identities must be treated as new artifacts and must not be substituted into a
deployment without the normal gate process.

## 6. Correctness and soundness argument

### 6.1 Prepared-key equivalence

For a fixed valid public-key byte array, `rho`, `tr`, `t1_d2_hat_mont`, and
`A_hat` are deterministic pure functions of those bytes. Reusing those values
does not reuse message- or signature-dependent state. The cache key covers
every input byte, so two different encodings cannot share a prepared entry.
Eviction changes performance only; a miss recomputes the same values.

Malformed public keys are rejected by `PublicKey::try_from_bytes` and are not
inserted. Malformed signatures remain decoded on every invocation and follow
the reference rejection path. Contexts longer than 255 and wrong public-key or
signature lengths reject before cache lookup.

### 6.2 Arithmetic equivalence

For dot products, replacing
`sum_j MontgomeryReduce(a_j * b_j)` with
`MontgomeryReduce(sum_j a_j * b_j)` preserves the residue modulo `q`. The
bounded `i64` sum satisfies the reduction precondition for every standardized
ML-DSA parameter set. Later operations accept any bounded congruent
representative; final encoding and comparison are unchanged.

For Montgomery conversion, with `R = 2^32` and `R2 = R^2 mod q`,
`MontgomeryReduce(x * R2) = x * R mod q`, exactly the representation required
by the existing multiplication path. Tests compare every tested coefficient
modulo `q` against the original implementation.

### 6.3 Proof-system boundary

All accelerated computation is ordinary guest Rust compiled to the SP1 RISC-V
ELF. SP1 therefore proves the actual cache preparation, lookup, NTT, modular
arithmetic, decoding, and boolean decision. The cache is an optimization of
deterministic guest state, not trusted host advice.

## 7. Verification plan and release gates

The accelerated feature is acceptable only if all gates pass:

1. Run the vendored upstream NIST ACVP `internalProjection` JSON suites for
   ML-DSA-65 key generation, signature generation, and signature verification.
2. Pin a separate dev-dependency named `fips204-reference` to crates.io
   `=0.4.6`; do not route it through the vendored path.
3. For deterministic seeds, messages, and contexts, require all valid
   signatures to pass both paths and compare every decision.
4. Test signature bit flips across the encoding, wrong messages, wrong
   contexts, truncated public keys/signatures, malformed encodings, and at
   least 512 deterministic multi-byte signature mutations against the
   independent oracle.
5. Unit-test the two arithmetic substitutions against the original formulas,
   including boundary representatives and all polynomial coefficients in a
   deterministic corpus.
6. Exercise more than 64 valid keys to prove bounded FIFO eviction cannot
   change decisions or exceed the entry cap.
7. Build reference and accelerated SP1 ELFs with locked dependencies, execute
   both against the exact archived witness, and require identical 1,486 public
   bytes and digest.
8. Prove the accelerated witness with `SP1_PROVER=cuda`, verify the Groth16
   proof locally, and require proof public values to equal the native reference
   bytes.

Any decision mismatch, panic on a negative input, non-deterministic public
output, unbounded allocation, or failed GPU proof is a release blocker.

## 8. Measured hypothesis and acceptance target

With six distinct keys, 462 of 468 preparations are redundant. Under the
official SP1 hash patches, the measured preparation labels imply about 0.275
billion removable cycles: repeated `ExpandA`, public-key decode, public-key
SHAKE, and the public-key forward-NTT preparation. The measured matrix/vector
arithmetic offers another 0.031 billion cycles, while other `to_mont` uses are
accounted for in residual verify work. ELF layout, cache lookup, and shifted
preparation order make these estimates rather than acceptance claims.

The pre-implementation target for the exact branch was at most 1.25 billion
cycles with byte-identical public values. That is at least 12.27% below the
matched 1,424,788,289-cycle hash control and at least 38% below V1. Correctness
gates take precedence over this target. Report both execute cycles and wall
time; wall time is secondary because executor parallelism and profiling change
it.

## 9. Operational and maintenance risks

- **Consensus drift:** pin the oracle and vendor at 0.4.6; review upstream
  updates as new cryptographic changes rather than automatic dependency bumps.
- **Cache denial of service:** capacity is 64, keys are fixed-size, invalid keys
  are not retained, and every miss does at most one bounded preparation.
- **Feature skew:** production/non-guest crates default to the reference path;
  tests explicitly build both feature states.
- **Stack and memory pressure:** prepared matrices live in bounded heap-backed
  cache entries; execute benchmarks record peak RSS and the GPU proof is the
  definitive whole-program memory validation.
- **Instrumentation bias:** component trackers are feature-gated and never used
  for the before/after production delta.
- **Future syscall risk:** no NTT syscall may ship until its output relation is
  constrained by SP1 and differentially tested across executor and prover.

## 10. Text Improvement Harness record

The founder-specified `criticize --mode gpt` command is run exactly once after
the measured draft. Its critique, the accepted gaps, and the resulting edits
are recorded in
`docs/evidence/mldsa-ntt-precompile-20260718/tih-critique-and-response.md`.
The harness score is not an acceptance criterion.

## 11. Implementation validation

All section 7 release gates passed on the dedicated box. The implementation is
validated for review and later gate integration; this work did not deploy it.
Full copied reports and proof artifacts are under
`docs/evidence/mldsa-ntt-precompile-20260718/`.

### 11.1 Cryptographic tests

Commands:

```text
cargo test --manifest-path third_party/fips204/Cargo.toml \
  --features precompute-verify-matrix,riscv32-verify-arithmetic
cargo test --locked -p postfiat-crypto-provider
cargo test -p postfiat-crypto-provider \
  --features mldsa-guest-acceleration
cargo test --locked -p postfiat-pfusdc-proofs \
  --features mldsa-guest-acceleration
```

Results:

- vendored suite: 59 passed, 0 failed, one intentionally ignored infinite
  stress test; this includes NIST ACVP `keyGen`, `sigGen`, and `sigVer`;
- crypto provider, reference feature state: 5 passed, 0 failed;
- crypto provider, accelerated feature state: 6 passed, 0 failed;
- accelerated pfUSDC proof integration: 4 passed, 0 failed;
- independent crates.io 0.4.6 comparisons cover 16 deterministic valid cases,
  96 targeted signature-bit negatives, wrong message/context/length negatives,
  and 512 deterministic two-byte mutations; and
- the cache test inserted 65 valid keys and confirmed deterministic FIFO
  eviction at exactly 64 entries.

The vendored arithmetic unit tests compare the optimized Montgomery conversion
and accumulated pointwise reduction with the original formulas modulo `q`.

### 11.2 Execute equivalence and delta

The controlled, feature-disabled reference ELF SHA-256 is
`657cf348b572f497f226812e24ed053b5ebcba3aa004f78bb2216fe56000e5e0`.
The production accelerated ELF SHA-256 is
`3c7c6ef8d0395ce1ad00dc0b8d84613c44600c433fe4d559e5481649f93078ea`.

| Exact 26-block witness | Reference | Accelerated | Delta |
|---|---:|---:|---:|
| Execute cycles | 1,424,788,289 | 1,050,064,162 | -374,724,127 (-26.30%) |
| Harness elapsed | 20,914 ms | 16,754 ms | -4,160 ms (-19.89%) |
| End-to-end execute wall | 27.72 s | 23.50 s | -4.22 s (-15.22%) |
| Maximum host RSS | 11,160,140 KiB | 10,647,548 KiB | -512,592 KiB (-4.59%) |

Both executions emitted exactly 1,486 bytes. `cmp` returned success and both
files have SHA-256
`dfe21e43169ba1d262abbc941dd66dab8747503414b3c79db7968c813ca8d0e8`.
Relative to the branch's checked-in V1 result, the accelerated ELF saves
980,226,071 cycles (48.28%). The clean reference is only 0.082% below the
frozen gate's 1,425,953,847-cycle hash-precompile control, confirming that the
before value includes the same official SP1 hash acceleration.

Post-change tracking confirmed the mechanism rather than an ELF-layout
accident:

- public-key decode, public-key SHAKE, and `ExpandA` calls: 468 to 6 each;
- forward NTT calls: 1,404 to 942;
- matrix/vector pointwise cycles: 65,553,696 to 34,846,344 (-46.84%); and
- tracked ML-DSA verify cycles: 695,624,363 to 320,792,959 (-53.88%).

### 11.3 CUDA proof validation

Command shape:

```text
SP1_PROVER=cuda CUDA_VISIBLE_DEVICES=0 \
  pfusdc-tier4-prover egress \
  --witness benchmark-inputs/archived-26-block-witness.json \
  --output-dir benchmark-results/mldsa-ntt-20260718/formatted-final-cuda-proof \
  --prove
```

SP1 6.3.1 ran its GPU server on the RTX 5090, with an observed 65% utilization
and 22,006 MiB VRAM during a sampled core-proving interval. The final Groth16
wrapper and local SDK verification succeeded. Results:

- program vkey:
  `0x008b69a0676137455435bbe635752c1a60c29710a4f9160666ea52a0240d1f10`;
- proof report `setup_and_prove_ms`: 122,161 ms;
- total wall time: 139.58 s with the versioned Groth16 circuit already cached;
- Groth16 calldata: 356 bytes, SHA-256
  `9f20c804609f9b002c867da722b14daa3cb409996a393ce4dafa8f9541e16d02`;
- serialized proof: 3,181 bytes, SHA-256
  `71c1595f940ddc172878a13b9e237b32306ac292692f97ffa8c239ddef960fc3`;
  and
- verified proof public values: 1,486 bytes, identical to the native reference.

The accelerated ELF and vkey are new consensus artifacts. Validation here does
not authorize deployment; normal Tier-4 review and artifact gates still apply.
