# Cobalt adversarial verification E1

This packet records the independent-oracle and generated-trust-graph experiment from the locked Cobalt adversarial verification specification.

The second oracle lives in `crates/cobalt_adversarial_oracle`. It imports neither production Cobalt nor the first activation oracle. The adapter in `crates/cobalt_e1_harness` regenerates the frozen corpus, verifies its SHA-256, and compares every case with:

- the second oracle;
- `postfiat-cobalt-decision-oracle`;
- production `analyze_trust_graph`;
- production `has_strong_support`; and
- production non-uniform certificate construction and verification.

The corpus is frozen before comparison by `corpus-manifest.json`. It contains 10,240 deterministic cases spanning 6-20 validators, randomized parameter/view shapes, and named cases at every subset/linkage boundary. Equality at each strict subset inequality is retained as a deliberately invalid graph, so all three implementations must reject it consistently.

Run the focused experiment with the repository's pinned Zig wrappers:

```bash
export POSTFIAT_ZIG=/path/to/pinned/zig
export CC=$PWD/scripts/zig-cc
export CXX=$PWD/scripts/zig-cxx
export AR=$PWD/scripts/zig-ar
export RANLIB=$PWD/scripts/zig-ranlib
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=$PWD/scripts/zig-cc

cargo test -p postfiat-cobalt-adversarial-oracle
cargo run -p postfiat-cobalt-adversarial-oracle -- verify \
  benchmarks/cobalt-adversarial-verification/e1/corpus-manifest.json
cargo run -p postfiat-cobalt-e1-harness -- compare \
  benchmarks/cobalt-adversarial-verification/e1/corpus-manifest.json \
  benchmarks/cobalt-adversarial-verification/e1/initial
```

The initial comparison preserves every disagreement before remediation. Large generated artifacts are committed losslessly as `initial/classifications.jsonl.gz` and `initial/disagreements.json.gz`; their uncompressed hashes are recorded in `initial/summary.json`. The remediated full pass is under `reconciled`, with lossless per-graph classifications in `classifications.jsonl.gz`. A clean-state rerun uses the same manifest and writes a summary-only result under `clean-rerun`; its corpus and classification hashes must match the reconciled comparison.

Verify the complete packet from the repository root:

```bash
python3 benchmarks/cobalt-adversarial-verification/e1/verify_packet.py
```

The 2026-08-26 evidence review added the standalone verifier and re-bound every
committed packet file in `SHA256SUMS.txt`; it did not change the frozen corpus,
classification streams, disagreement record, or experiment result.
