# Verification commands and evidence scope

Candidate source: `1c435f4fb482ea7830bee5c7018d370a10a34bfd`. Run from the release checkout. Raw terminal logs preserve original ANSI escapes, whitespace and blank lines; whitespace checks apply to authored files. Temporary files live on disk. The resumed runners execute one heavy stage at a time, use two build/test workers, cap each process at 20 GiB address space and stop if available memory falls below 4 GiB.

## Final software gates

The full workspace gate is required once because these repairs cross historical state commitments and Orchard supply accounting. This invocation uses the existing optimized test cache while retaining debug assertions and overflow checks:

```sh
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0
export TMPDIR="$disk_cache/tmp" RAYON_NUM_THREADS=2 OMP_NUM_THREADS=2
mkdir -p "$TMPDIR"
cargo fmt --all -- --check
scripts/test-proof-public-input-inventory

CARGO_PROFILE_TEST_OPT_LEVEL=2 \
CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true \
CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true \
CARGO_TARGET_DIR="$optimized_test_target" \
cargo test --workspace --locked --no-fail-fast -- --test-threads=2

# Use the node binary test executable printed by the completed workspace run.
"$node_test_executable" \
  fastswap_service::tests::persistent_wallet_driver_meets_isolated_warm_latency_gate \
  --ignored --exact --nocapture

CARGO_TARGET_DIR="$check_target" cargo check --workspace --all-targets --locked
CARGO_TARGET_DIR="$check_target" cargo clippy --workspace --all-targets --locked -- -D warnings
```

The runner's status report records the exact commands, environment overrides, start/end times and exits. Interrupted September 16 runs are excluded. The first separate latency invocation selected a different Cargo feature graph and began recompiling; it was stopped before tests ran. The successful latency check executes the exact node test binary from the passing workspace suite, with unchanged assertions. Its executable hash and the interrupted compilation log are retained. Earlier focused regression logs cover FastPay types/governance, retained votes, live/replay supply equality and rejection of a substituted withdrawal proof identity. The standalone prover is excluded from the Rust workspace; its focused result is retained separately.

## Captured history, governed continuation and rollback

`candidate` must hash to `86f62a5a3e0105f839c8491047a648696e6295cd4f80956a6fb774b7ddbefbe2`.
`rollback` must hash to `57b0f4d1d42d66878d7dbb8c33919c7fa0f87c6cc1a4b9cc1a85d75b634eec83`.
All directories below are disposable local copies, and all services use loopback peers.

```sh
"$candidate" verify-state --data-dir "$post_v2_clones/validator-0"
# Repeat sequentially for validator-1 through validator-5.
"$rollback" verify-finalized-checkpoint --data-dir "$rollback_clones/validator-0"
# Repeat for all six restored pre-upgrade copies.
"$candidate" verify-state --data-dir "$rollback_clones/validator-0"
```

The six final-source post-V2 reports verify the full 1,021-block history, including block 1011 and the signed V2 installation. Six earlier-repair reports independently verified the original 1,020-block capture before the final retained-vote normalization commit. The additional final-source rollback-history check tests the unchanged pre-upgrade history directly on restored validator-0; its scope is one copy.

The [governance runner](run_local_governance_gate.py) exercises both startup orders, acceptance of the signed rotation on all six nodes, the certified block at 1021 and restart at the identical root. Both service receipts omit private key-staging paths; no key, signed batch or full database is published.

Rollback serves the authenticated pre-upgrade tip using the exact old executable and compatible raw state. That executable's full-replay bug remains documented in the original failure packet. Its checkpoint checks and service restarts are paired with the repaired verifier's full-history check. This is **pre-activation rollback only**: after a live V2 installation, recovery requires a compatible binary rather than discarding finalized commitments.

The qualification-only [measurement probe](measure_retained_commitments.rs) calls the actual types crate's V1/V2 encoders on the retained checkpoint's reveal/fence records. It reports byte counts and encoding time only; full-history verification is the separate check above. Its libraries come from final build-source-1's release output. Private input and executable remain in the local cache.

## Node and proof builds

[node-builds.json](node-builds.json) records two clean detached source trees at the candidate commit, separate existing target caches, exact lockfile hashes and identical unmodified executable hashes. Compiler output is retained. These final repair builds reused the independently populated caches from the original release exercise; they are not represented as builds from empty dependency caches.

The release build command was `cargo build --release --locked -p postfiat-node --bin postfiat-node`, with two workers and incremental compilation disabled. Recorded Cargo fingerprints retain source-directory remapping to `/src/postfiatl1v2` and target-directory remapping to `/target`.

The [proof identity report](proof-identities.json) identifies successful CI run 35044347138 at `5b1093fa`. Both historical releases were each built twice from clean source trees, matched byte-for-byte against their retained executable, and produced the pinned verification key. Downloaded report checksums, executable hashes and retained-file equality were checked independently here. The only candidate delta after that CI source is FastPay vote normalization; proof workflows and historical release inputs are unchanged. This is not a claim that CI ran at the later commit.

## Separate release obligations

The original packet remains authoritative for unrelated baseline findings: seven reachable-history secret-scan findings and unavailable pinned testnet archive state. Existing Python, wallet and contract results are earlier evidence, not new runs attributed to this repair. Review, repository publication decisions, live deployment and governance activation remain separate.
