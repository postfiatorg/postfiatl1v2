# Combined devnet and FastPay committee merge (2026-09-25)

Branch `release/combined-fastpay-20260925` = our qualified line
`release/combined-devnet-20260915` (`f59dc07a`) merged with the deployed line
`release/fastpay-committee-20260925-r4` (`c1c81119`). Merge base `a699d560`.

## Why the lines diverged

The FastPay line was built on the deployed `a666-source-route-20260907` source to
restore FastPay after validator-5's key rotation, while the history, proof and
burn-5/6/P3 repairs were qualified on the combined line. Lacking the history repair,
its full-history replay fails at block 1011, so r4 signs consensus-v2 timeout votes
against the finalized checkpoint instead.

## Resolution rule

Keep our repaired behaviour, then re-apply his intent on top. His timeout-vote
change (`verify_block_log: false` in `crates/node/src/finality_view_recovery.rs`)
and his FastPay committee, effect-restore and checkpoint-trust changes come in whole.
No repair of ours was dropped. Nothing needed to stop the merge.

## Conflicts

Most of his conflicting hunks were unformatted cherry-picks of repairs already on
our line; those resolved to our formatted text with identical code.

| File | Ours | His | Result |
|---|---|---|---|
| `crates/execution/src/nav_vault_asset_execution.rs` | family supply, legacy base-only only for pinned archive packets | same code, longer comment | ours; code identical |
| `crates/node/src/batch_snapshot.rs` (1) | bounded snapshot reads, private staging dir, no-replace publish | sealed `FreshCheckpointImport` capability | both kept |
| `crates/node/src/batch_snapshot.rs` (2) | import rebuilds storage inside the private staging dir (`&Path`) | `FinalizedCheckpoint` basis calls `restore_transactional_checkpoint` | his basis match on our staging import; capability bound to the staging dir (`to_path_buf`) |
| `crates/node/src/block_replay_wallet.rs` (3) | YOLO fields in destructure, native bond custody, burn-6 wallet tests | cherry-picked bond custody and its test | ours |
| `crates/node/src/execution_actions.rs` (4) | archive replay with Orchard balances and pinned pfETH packets, tests | same pinning without Orchard balances | ours; keeps shielded supply in replay |
| `crates/node/src/market_bridge.rs` | source custody rows in NAV bridge view | same, one line | ours |
| `crates/node/src/rpc_cli.rs` | `rpc_serve` moved to `rpc_serve_runtime.rs`, accept budget | age-bounded status cache inside old `rpc_serve` | ours; the moved body already has his cache change, and `rpc_serve_cached_status` is identical |
| `crates/node/src/state_commitment.rs` (3) | source custody validation, commitment and test | same, unformatted | ours |
| `crates/node/tests/atomic_swap_local_six.rs` (3) | `settlement_source_asset_ids: None` | same, misindented | ours; his other two fields merged cleanly |
| `docs/status/OPEN-SOURCE-PROOF-PUBLIC-INPUT-INVENTORY-20260716.json` | 94 pins plus handoff refresh | subset with older pins | regenerated (below) |

Follow-up edits inside the merge commit:

- `crates/node/src/fastpay_recovery_node.rs`: our burn-5 rollback test gains
  `retained_tip: None` for his new journal field (legacy record, same meaning).
- `cargo fmt` on `fastpay_recovery_node.rs` and `crates/rpc_sdk/src/main.rs`,
  where clean hunks from both sides combined into unformatted lines.
- `docs/specs/fastpay-committee-local-authority-repair-20260925.md`: his link to the
  stale-committee handoff, which exists only on `main`, becomes a path reference so
  `scripts/public-doc-links` passes.

All 61 files his line changed were checked: every block he added is present in the
merge, except the three adapted spots above.

## Regenerated

The inventory checker `scripts/test-proof-public-input-inventory` has no update mode.
All 94 pinned sources were rehashed from merged bytes. None differ from our pins,
because every pinned source merged to our bytes. The file equals our side's, and
the gate passes: `systems=7 public_fields=150 source_hashes=94`.

## Test evidence

Build and test commands ran one at a time with `-j 2`, `RUST_TEST_THREADS=2` and
`RAYON_NUM_THREADS=2`. No Orchard suite and no fleet access.

| Gate | Result |
|---|---|
| `cargo check --workspace` | pass |
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --all-targets` on node, execution, rpc-sdk, types, bridge | pass, 0 warnings |
| node lib: batch snapshot, block replay wallet, execution actions, FastPay recovery node, market bridge, state commitment, consensus history (timeout votes), FastPay payment safety (signer retention, effect restore, rotated sixth validator), snapshot deployment (checkpoint restore), vault bridge, vote locks, checkpoint, pfUSDC tier 4 | 156 passed, 3 ignored (Anvil) |
| node bin: finality view recovery, RPC serve (timeout-vote RPC, status cache), transport batch payload (failed view-zero proposer n4/n6, peer round), FastPay RPC | 48 passed, first run |
| execution lib: NAV vault, market NAV, vault bridge, owned transfer, FastPay, source settlement | 57 passed |
| `atomic_swap_local_six` | 1 passed, 8 ignored (live six-node) |
| `scripts/test-proof-public-input-inventory` | pass |
| `mkdocs build --strict`, `scripts/public-doc-links`, `scripts/public-secret-scan` | pass (mkdocs strict; links ok, 476 files; secret scan passed, tracked tree) |

## What qualification must prove

- Full-history replay of the archived `postfiat-wan-devnet-2` chain through block
  1011 and the current tip on this merged build (r4 itself fails there).
- The timeout-vote and view-recovery path: a stalled view-0 proposer is passed by
  a timeout certificate, and `verify_block_log: false` remains the only change of
  trust basis.
- FastPay effect anchoring: a FastPay payment followed by validator-5's turn,
  pending effects restored over finalized state after restart, and the effect
  anchored in the next certified block with identical roots on all six nodes.
