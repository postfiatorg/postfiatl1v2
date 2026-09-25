# NAVCoin machine handoff validation — September 7, 2026

These checks ran against the consolidated `handoff/navcoin-cobalt-local-20260907` and companion StakeHub branch. [L1 output logs](../../deployments/a666-source-route-20260907/handoff-validation/) and the private StakeHub recovery archive retain the results. Existing historical deployment/test reports remain separately labeled.

| Check | Fresh result |
| --- | --- |
| `cargo build --locked -p postfiat-node` | PASS |
| `cargo test -p postfiat-execution pftl_source_settlement --lib` | 1 passed: signed source allowlist, issue/reserve/redeem, custody conservation and rejection paths |
| `cargo test -p postfiat-types pftl_source --lib` | 2 passed: source choices and unchanged/disabled/selected governance states are signed |
| `cargo test -p postfiat-node source_settlement_commitment --lib` | 1 passed: source custody committed; empty state preserves legacy bytes |
| `cargo test -p postfiat-node rpc_serve_status_cache_expires_after_transactional_commit --bin postfiat-node` | 1 passed |
| `cargo test -p postfiat-node pfeth_reserve --lib` | 2 passed: strict new supply validation and exactly pinned historical replay |
| `cargo test -p postfiat-execution ingress --lib` | 3 passed, including real Arc proof verification/mutations and pfETH source selection |
| `forge test --root crates/ethereum-contracts --match-contract 'WETHBridgeVaultL1Test\|ExitExecutorV1Test'` | 19 passed |
| `scripts/test-proof-public-input-inventory` | PASS: 7 systems, 150 public fields, 93 source hashes |
| `scripts/test-pfeth-eth-mainnet-package` | PASS after building the node and Solidity artifacts |
| Focused Python `test_tasknode_unl*.py` and `test_cobalt*.py` | 130 passed, 34 subtests passed |
| StakeHub native round-trip, governed reserves, dashboard, reserve proof and `test_shielded_exit*.py` | 469 passed, 4 skipped; two dependency deprecation warnings |
| StakeHub CLI `--help` | PASS |
| StakeHub `mkdocs build --strict` into a temporary site directory | PASS; generated site not committed |
| Secret scans | Both tracked trees passed; newly included tar members and SQLite dump separately scanned, screenshot inspected |
| Source whitespace | `git diff --check --cached -- crates scripts tools programs` passed. Archived patches/logs and locked Markdown evidence retain original bytes. |

The initial combined-source inventory check caught two changed source digests: the Arc constants retained in `pfusdc_tier4_types.rs` and the merged reserve replay/source-custody execution file. Their source hashes were refreshed after reviewing the merged changes. No public-value layout or deployed program vkey was changed to make this inventory pass; this refresh is not a new cryptographic audit or guest rebuild.

Known failing check: `PYTHONPATH=python python3 benchmarks/cobalt-adversarial-verification/packet/verify_packet.py` still exits with `adversarial packet semantic verifier is missing, failed, or inconsistent`. The September 6 review traced the historical publication-binding failure. This handoff does not mark it repaired.

Not rerun during the initial packaging pass: full Rust workspace/Orchard suites, full live archived-chain replay of the merged branch, sustained storage-contention qualification, SP1 guest reproduction, and the external Ethereum/Uniswap round trip. That initial packaging pass performed no live chain or wallet operation; the subsequent authorized external-route execution is recorded below. The deployed September 7 source's historical replay through block 1001 is retained as historical evidence, not a result for this merged branch.

## Completed external-route resumption

The subsequent authorized run completed the real Ethereum mainnet → PFTL devnet → Uniswap → PFTL → Ethereum traversal. The [execution record](NAVCOIN-EXTERNAL-ROUTE-20260907.md) gives accepted transactions and the diagram. The existing deployed node release remained unchanged.

- Actual 10 USDC ingress, source-series claim, NAV issuance/export, all three historical A666 checkpoint advancements including committee rotations, Ethereum mint, both Uniswap trades, burn/import, same-source NAV redemption, 9.932860 USDC Ethereum payout, and native settlement passed through PFTL 1020.
- Independent read-only reconciliation passed on all six validators: equal state roots, empty mempools, protected balances, source claims including dedicated custody, vault obligations, and wrapped supply. Duplicate withdrawal was rejected.
- Focused changed-path checks passed: 8 issue/NAV arithmetic tests, 4 withdrawal recovery tests, 3 allowance tests, 3 swap-journal tests, 1 partial-fanout evidence test, and 5 checkpoint vote-file tests. StakeHub passed 15 checkpoint tests, including the existing-contract signer path and refusal to resend a durable journal; route and deposit regression tests passed separately.
- Final `mkdocs build --strict` and both repository whitespace checks passed. All 190 resumed-run archive hashes matched; structured nonempty secret-field scans passed. Private billing responses, wallet state, and keys are excluded.
- Temporary GPU instance 50127946 was deleted and independently confirmed absent from the provider instance list at 06:37:54 UTC. All completed proofs were retrieved before deletion.

The initial broad-suite counts above are separate from these later focused checks. No full merged-branch replay, independent-operator qualification, or repair of the separate historical Cobalt publication-binding defect is claimed.
