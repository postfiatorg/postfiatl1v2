# FastSwapV1 Packet P0 — executable safety model

Protocol authority:
`orc_directives/FASTPAY-ATOMIC-SWAP-PROTOCOL-GROUND-SPEC-20260714.md`.
Baseline: `87500aed04017d1870978ca99e8dd9dd6f773291`.

The founder explicitly waived the external-review pause on 2026-07-14 and
directed continuous implementation. The formal safety, durability, quorum, and
activation requirements were not waived.

`postfiat-fastswap-model` is a deterministic explicit-state model of two
conflicting swaps with one Byzantine validator. It covers atomic all-input
reservation, delayed round-zero LockQC, objective-expiry cancellation,
round-high-water stale-QC rejection, decision locks, effects finality, cancel
tombstones, crash/restart persistence, stop-prepare fencing, and drained
committee rotation. Arbitrary transition scheduling represents message delay,
reordering, withholding, and every validator partition subset. Honest
validator symmetry is canonicalized; validator 0 is the Byzantine identity.

Required bounded runs, release mode, depth 18:

```text
n=4 f=1 q=3 states=2,416,676 transitions=8,897,518 result=PASS
n=6 f=1 q=5 states=1,844,292 transitions=8,600,633 result=PASS
```

The negative control disables stale-QC rejection and permits an unjustified
cross-value lock change. It constructs both Confirm and Cancel DecisionQCs for
one swap at depth 17 and exits nonzero. This proves the checker reaches the
specification's crux rather than merely exercising happy paths.

Reproduce:

```sh
cargo test -p postfiat-fastswap-model --release -- --test-threads=1
cargo run -p postfiat-fastswap-model --release -- --n 4 --depth 18
cargo run -p postfiat-fastswap-model --release -- --n 6 --depth 18
cargo run -p postfiat-fastswap-model --release -- \
  --n 4 --depth 18 --unsafe-no-stale-qc-guard
```

This bounded model is a gate, not a mathematical proof of unbounded liveness.
Production packets must retain deterministic simulation, property, crash,
fuzz, and multi-process gates from the grounded specification.
