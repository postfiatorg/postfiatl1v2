# Verification commands and scope

Run commands from the candidate checkout unless a working directory is given. Logs retain the original command output; compiler warnings and terminal whitespace are preserved. `build-provenance.md` records the two independent release builds separately.

## Regression suites

```sh
cargo test --workspace --locked -- --test-threads=2
cargo test --locked -p postfiat-node --lib pfeth_reserve_replay_tests
cargo test --locked -p postfiat-node --lib source_settlement_commitment_tests
cargo test --locked -p postfiat-node --bin postfiat-node rpc_serve
cargo test --locked -p arc-conformance
cargo test --locked -p postfiat-node --bin postfiat-node \
  fastswap_service::tests::persistent_wallet_driver_meets_isolated_warm_latency_gate \
  -- --ignored --exact --nocapture
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

The original workspace invocation uses `CARGO_BUILD_JOBS=2` and the existing September 9 test target cache. It began at merge `7a41d5a4`, before the rustls update. Focused Arc conformance, final check and Clippy use the updated dependency lock.

Because unoptimized proof construction is expensive, a second full workspace invocation uses a separate, initially empty target directory and the final lockfile:

```sh
CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 \
CARGO_PROFILE_TEST_OPT_LEVEL=2 \
CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true \
CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true \
CARGO_TARGET_DIR=/home/postfiatchad/.cache/combined-release-20260915/optimized-test-target \
cargo test --workspace --locked -- --test-threads=2
```

This preserves debug assertions, overflow checks and test selection. The original full run completed first: 1,429 passed, 39 ignored, zero failures, exit 0. The optimized follow-up was then stopped during compilation (exit 143); its partial log supplies no passing-test claim. The final rustls delta is separately covered by four passing Arc conformance tests and final all-target check/Clippy.

The Python SDK uses the private `test-venv` under the qualification cache, with `requirements-test.txt` installed using `--require-hashes`:

```sh
PYTHONPATH=python python -m pytest python/tests
npm test --prefix wallet-web
npm run build --prefix wallet-web
npm audit --prefix wallet-web --audit-level=moderate
npm test --prefix wallet-proxy
npm audit --prefix wallet-proxy --audit-level=moderate
```

From `crates/ethereum-contracts`:

```sh
forge test --no-match-path 'test/*Fork.t.sol'
forge test --match-path 'test/*Fork.t.sol'
forge test --match-path test/ExitExecutorV1.t.sol
```

The fork invocation had Ethereum mainnet, Ethereum Sepolia and Arbitrum Sepolia RPC endpoints configured. Both official mainnet tests pass; the two pinned testnet tests fail because the providers lack historical state. The 178-test offline result excludes all fork files and establishes only offline coverage.

## Current-chain and rollback checks

All invocations below target disposable local copies. `$candidate` is the pinned candidate executable, `$rollback` is the exact downloaded deployed executable, and `$qualification_root` is the private local cache. These variables describe retained paths; they are not fleet deployment commands.

```sh
"$candidate" verify-state --data-dir "$qualification_root/working/validator-0"
"$candidate" verify-blocks --data-dir "$qualification_root/working/validator-0"
"$rollback" verify-state --data-dir "$qualification_root/rollback-drill/validator-0"
```

Candidate verification rejects the FastPay committee. The old binary reaches block 1011 and rejects the recorded supply relation. Its full-history snapshot export fails on the same relation. Its finalized-checkpoint export succeeds, but candidate import rejects the historical FastPay committee. Original stderr is retained for each failure.

The local service exercise reuses `deployments/signing-fix-qualification-20260909/run_local_service_gate.py`: six disposable copies, local isolated signing keys already present on the workstation, loopback peers, alternating transport-first/RPC-first startup, status checks, stop, restart, status checks, stop. The receipt explicitly records `finality_exercised: false`.

Read-only fleet probes and local restore status checks are summarized in `fleet-and-restore-observations.json`. Original full RPC captures and node data remain private.

## Repository and publication checks

Raw logs are retained for tracked-tree secrets, reachable-history secrets, source portability, public defaults, artifact policy, proof-input identities, archived proof qualification, cryptographic call sites, RPC authorization inventory, documentation, provider boundaries, dependency advisories, vendored Halo2 and the SBOM generator. The reachable-history scan was run independently on single-branch candidate and main clones; their seven findings match byte-for-byte. That comparison identifies baseline findings and leaves the publication gate blocked.

The draft PR carries independent CI results. PR CI skips the official mainnet fork job; the configured local run above supplies narrower evidence for this candidate. Any still-running CI check remains pending rather than passing by inference.
