# Cobalt adversarial verification E2

This packet records the frozen E2 Byzantine-validator campaign from the locked
Cobalt adversarial-verification specification.

The campaign is bound to the last recorded live six-validator topology:
`n=6`, quorum `q=5`, and derived fault bound `f=1`. The source receipts,
their SHA-256 hashes, the registry and trust roots, the case list, the schedule
seed, and the 4,096 schedules per case were frozen before the first full run in
commit `15ef2307732cf46ff3b921bf02f3ad096dda15f3`.

## Result

The first frozen run passed without remediation:

- 108 cases: every one of 18 strategies against each of six possible Byzantine
  validators;
- 442,368 executed event schedules;
- 104,509,400 delivered events, including 62,730,200 duplicate deliveries and
  3,433,631 deliveries delayed by pre-heal partitions;
- 120 signed ML-DSA misbehavior-evidence pairs;
- zero conflicting-root schedules;
- zero false accepts, false halts, synchrony violations, or rejected-state
  mutations; and
- deterministic classification SHA-256
  `60ab419fc6cb165088c31e221a4d1a3247ad7e8d9fff9d9877bdf807b6590e93`.

The clean summary rerun produced the same classification and counters. The full
evidence verifier also passed.

## What the campaign executes

The harness in `crates/cobalt_e2_harness` audits the pinned live receipts,
builds a six-member ML-DSA simulation committee, and uses production Cobalt
signed-message validators and
`postfiat_node::cobalt_shadow::assemble_protocol_transcript`.

Every schedule executes an event-delivery model over five distinct correct
identities across the RBC, ABBA, MVBA, and DABC gates. Delay, Byzantine-message
drop, equal-time reorder, duplicate copies, and a healing two-way partition all
vary under the frozen seed. For each validator/strategy pair, the worst schedule
then supplies the honest delivery order to a fully signed production transcript.
A one-validator conflicting transcript must fail the production quorum checks,
and duplicate contributors must fail closed.

The incompatible-trust-boundary cases use disjoint correct trust views. They
must halt at trust-graph analysis before transport and must not mutate the
registry.

## Evidence layout

- `campaign-manifest.json`: immutable live binding, fault model, strategy
  corpus, seed, and search envelope.
- `initial/campaign.json`: first full run, including per-case schedules,
  correct-validator outcomes, and signed evidence.
- `clean-rerun/summary.json`: independent summary-only rerun.
- `verify_packet.py`: static packet, checksum, matrix, and invariant verifier.
- `SHA256SUMS.txt`: packet-file hashes.

The signatures are created with deterministic simulation identities, not live
validator keys. They prove that the production signature paths and attribution
checks reject modelled Byzantine conflicts; they are not evidence that a live
operator misbehaved. The campaign did not connect to or mutate the devnet. It
proves bounded protocol capability, not operator decentralization. E3-E6 and
the milestone-wide `KEEP_ACTIVE` decision remain open.

No Task Node interaction was used to execute or verify this packet.

## Reproduce and verify

Use the repository's pinned Zig wrappers:

```bash
export POSTFIAT_ZIG=/path/to/pinned/zig
export CC=$PWD/scripts/zig-cc
export CXX=$PWD/scripts/zig-cxx
export AR=$PWD/scripts/zig-ar
export RANLIB=$PWD/scripts/zig-ranlib
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=$PWD/scripts/zig-cc

cargo test -p postfiat-cobalt-e2-harness --locked
cargo run -p postfiat-cobalt-e2-harness --locked -- \
  audit-sources . \
  benchmarks/cobalt-adversarial-verification/e2/campaign-manifest.json
cargo run -p postfiat-cobalt-e2-harness --locked -- \
  verify-evidence . \
  benchmarks/cobalt-adversarial-verification/e2/campaign-manifest.json \
  benchmarks/cobalt-adversarial-verification/e2/initial/campaign.json
python3 benchmarks/cobalt-adversarial-verification/e2/verify_packet.py
```

The Rust evidence verifier performs cryptographic signature validation and a
clean deterministic replay. The Python verifier checks file hashes, the exact
validator/strategy matrix, source bindings, summary equality, schedule coverage,
zero-failure invariants, and evidence structure.
