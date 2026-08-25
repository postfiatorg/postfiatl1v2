# Cobalt adversarial verification E3

This packet records the frozen E3 adversarial-recovery campaign from the locked
Cobalt adversarial-verification specification. The source, live-state pins,
case matrix, and four-entry history shape were frozen before the evidence runs
in commit `5c9e543ea0f56e7e6dda85d3a27093e810fdc111`.

## Result

The initial run and a clean disposable-state rerun both passed with identical
classification SHA-256
`ab53b5ddd5134e8fbbbb359b65c249ccbb1eb85a7ad034e496efa10bd85b90d3`:

- 24 durable restart cases: truncated, padded, reordered, and one-entry-modified
  histories against each of six validator clones;
- 18 peer-signed forged catch-up cases: fabricated transitions, wrong-root
  certificates, and histories omitting a known latest update against each
  clone;
- all 42 attacks rejected with named reasons before rejoin;
- zero rejected-state or journal mutations;
- all 18 ML-DSA peer-attribution envelopes verified;
- six interrupted recoveries resumed from a second honest peer; and
- all six restored journals were byte-identical to honest history, with zero
  manual repair actions.

## Production remediation included in the freeze

The pre-freeze dry run exposed two missing recovery boundaries, which were fixed
before the corpus and source were frozen:

1. history ranges now have ML-DSA peer-signed envelopes bound to the chain,
   genesis, protocol version, sender, and range hash; and
2. a range that omits a known required latest update fails closed instead of
   clearing the missing-range obligation.

The node tests cover signed catch-up and the omitted-update rejection. The E3
campaign then exercises these paths across all six validator identities.

## Live binding and evidence boundary

Every disposable clone, trust graph, signed transcript, and accepted history is
bound to the exact recorded live registry root
`945768d593497541f59961d1ba3920560cfde7bf5037e40eb89dd5466637f221709bff05b69d2d40a36d5cff8505c37e`.
The packet separately pins the recorded live trust-transition root
`9221316a…6b8b13`.

The post-rotation live trust-graph object is not committed in the activation
packet, so E3 derives a canonical clone graph from the live registry root and
records its distinct root `7f3e1562…637879`. The packet does not claim to
reconstruct or clone the unavailable live sidecar graph object or signer keys.

All work was isolated under disposable local directories. The campaign did not
query, restart, or mutate the devnet. Temporary ML-DSA private material was
deleted at the end of each run and is absent from this packet. This experiment
proves production recovery behavior under the recorded live registry domain;
it is not deployment evidence or an operator-decentralization result.

## Evidence layout

- `campaign-manifest.json`: frozen live binding, source pins, validators, and
  attack matrix.
- `initial/campaign.json`: full per-validator results, reasons, timing, hashes,
  signed peer evidence, and recovery receipts.
- `clean-rerun/summary.json`: independent summary-only rerun.
- `verify_packet.py`: static checksum, source, matrix, reason, recovery, and
  redaction verifier.
- `SHA256SUMS.txt`: hashes for every packet file other than itself.

## Reproduce and verify

Use the repository's pinned Zig wrappers:

```bash
export POSTFIAT_ZIG=/path/to/pinned/zig
export CC=$PWD/scripts/zig-cc
export AR=$PWD/scripts/zig-ar
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=$PWD/scripts/zig-cc

cargo test -p postfiat-node cobalt_shadow::tests:: --lib --locked
cargo test -p postfiat-cobalt-e3-harness --locked
cargo clippy -p postfiat-cobalt-e3-harness --locked -- -D warnings

cargo run -p postfiat-cobalt-e3-harness --locked -- verify \
  benchmarks/cobalt-adversarial-verification/e3/campaign-manifest.json \
  benchmarks/cobalt-adversarial-verification/e3/initial/campaign.json
python3 benchmarks/cobalt-adversarial-verification/e3/verify_packet.py
```

The Rust verifier revalidates the ML-DSA evidence using the production crypto
provider. The Python verifier checks packet hashes, pinned source files, the
exact 42-case matrix, per-case reasons and non-mutation, summary equality, all
six byte-identical recoveries, and redaction invariants.

No Task Node interaction was used to execute or verify E3. E4-E6 and the
milestone-wide `KEEP_ACTIVE` decision remain open.
