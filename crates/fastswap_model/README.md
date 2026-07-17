# FastSwapV1 executable model

This crate is Packet P0 from
`orc_directives/FASTPAY-ATOMIC-SWAP-PROTOCOL-GROUND-SPEC-20260714.md`.
It is an isolated, explicit-state safety model, not validator production code.

The model explores arbitrary interleavings of two conflicting swaps, delayed
lock certificates, objective-expiry cancellation, crash/restart, stop-prepare
fencing, effects finality, cancel tombstones, and drained reconfiguration. One
validator is Byzantine and may vote in conflicting streams. Honest validators
persist reservations, round high-water marks, decision locks, terminals, and
tombstones before their corresponding vote becomes visible.

Run the required committee instances with:

```sh
cargo run -p postfiat-fastswap-model --release -- --n 4 --depth 28
cargo run -p postfiat-fastswap-model --release -- --n 6 --depth 28
```

The executable exits non-zero and prints a shortest counterexample trace when
an invariant fails. A negative control disables the stale-lock-certificate
guard and must find a confirm/cancel contradiction.
