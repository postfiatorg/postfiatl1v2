# Verification record

Date: 2026-07-23

All commands ran from
`/home/postfiat/repos/a666-uniswap-bridge-build-20260723`.

```text
cargo test -p postfiat-execution \
  pftl_uniswap_consensus_subscribe_export_and_refund_moves_real_balances \
  --lib -- --nocapture
result: 1 passed, 0 failed
```

This single test exercises strict controlled route initialization, primary
subscription, packet-schema-bound export, controlled operator-attested
consume/refund/return, replay-safe terminal states, and the existing
checkpoint/receipt-proof BFT path.

```text
cargo test -p postfiat-node \
  navcoin_bridge_status_reads_persisted_pftl_uniswap_ledgers \
  --lib -- --nocapture
result: 1 passed, 0 failed
```

```text
cargo test -p postfiat-rpc-sdk \
  read_response_validation_accepts_supported_results \
  --lib -- --nocapture
result: 1 passed, 0 failed
```

```text
cargo check -p postfiat-fuzz
result: passed
```

```text
cd crates/ethereum-contracts
forge test --match-path test/PFTLUniswapHandoffController.t.sol -vv
result: 36 passed, 0 failed
```

```text
cd wallet-proxy
npm test
result: 24 passed, 0 failed
```

```text
cargo build -p postfiat-node --release
result: passed
binary sha256:
76072718505b275f80c6550667f75141e78fe0a42a0b4bfbbf61a7884a48c978
```

The repository-wide stable-rustfmt check is not a clean signal on this branch:
pre-existing files use nonstandard formatting that stable rustfmt attempts to
rewrite. The mechanical rewrite was removed; the final diff is scoped.
