# Verification commands and scope

## September 22 repair follow-up

Started at 2026-09-22T10:34:31Z with a 40-minute limit. Fetched
`origin/release/combined-devnet-20260915` at `f19c7344` into
`/tmp/qualify-fix-20260922`. No Task Node, fleet or protected-checkout action.

The inventory checker has no update mode. The repository's
[reviewed repinning procedure](../../docs/review/proof-input-review-20260907.md)
was followed: compare exact prior bytes, review the owning repair, recompute SHA-256,
and change only the reviewed pin. These commands establish the old and new values:

```sh
git show 043b9d69^:crates/execution/src/nav_vault_asset_execution.rs | sha256sum
sha256sum crates/execution/src/nav_vault_asset_execution.rs
scripts/test-proof-public-input-inventory
```

Only that source hash differs; proof metadata and the remaining 93 pins are identical.
The inventory gate passed before commit `85eee166`. The one-character borrow repair
then passed these commands before commit `1a0989ad`:

```sh
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test -p postfiat-node --lib --locked navcoin_bridge
cargo fmt --all -- --check
```

The checks ran on the inventory commit with the uncommitted borrow fix; their receipts
record the subsequent unchanged validated commit and tested file hash.

Two separate clean worktrees at `1a0989ad6c35b7ea3eb6418f864958563233945a`, with separately
copied dependency caches, each ran:

```sh
cargo build --release --locked -p postfiat-node --bin postfiat-node
```

[Build records](node-builds.json) retain exact directories, remaps, environment, source
tree and lockfile identities, exit codes, hashes and the byte comparison.
Both use `SOURCE_DATE_EPOCH=1789514690`, the explicit `/usr/bin/gcc` Rust linker,
and the identical complete source/target remap string. ELF dynamic-section logs
confirm no RPATH/RUNPATH. No executable is patched or stripped.

All commands use the packet limits:

```sh
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_TERM_COLOR=never
export TMPDIR=/home/postfiatchad/.cache/qualify-fix-20260922/tmp
export RUST_TEST_THREADS=2 RAYON_NUM_THREADS=2 OMP_NUM_THREADS=2 MALLOC_ARENA_MAX=2
ulimit -v 20971520
ulimit -c 0
```

Only one Cargo process runs at a time. Commands start with at least 8 GiB available
and stop below 4 GiB. The existing disposable September 18 test cache is reused.

Publication stages the packet before the complete tracked-tree secret scan and runs
`.venv-docs/bin/mkdocs build --strict`, `scripts/public-doc-links`, and
`scripts/public-secret-scan`. The docs environment is the existing repository venv.
The refreshed receipts supersede the original timeout without altering the old logs.
Checksums are regenerated after evidence collection and verified before commit.

No history, rotation, rollback, Orchard suite or full workspace test suite is rerun.
Those prior operational results retain their original source identity; full workspace
tests and fresh required verdicts remain with CI. The final evidence commit changes
only this packet; the executable embeds the build source revision `1a0989ad`.

## Original qualification (superseded failures retained)

Source: `048d23df95929b1d544c8c13825e59844eb70e26`. Initial fetch and source pin were performed at 2026-09-22T07:47:09Z.
The qualification checkout was created with:

```sh
git fetch origin
git rev-parse origin/release/combined-devnet-20260915
git worktree add --detach /tmp/qualify-20260922 origin/release/combined-devnet-20260915
```

[Build records](node-builds.json) contain the two source directories, tree/lockfile identities, commands,
path remapping, limits, start/end times, exits and executable hashes.
Each command was `cargo build --release --locked -p postfiat-node --bin postfiat-node`.
Separate existing target caches were copied into the new disposable cache. Only one Cargo build ran at a time.
Both release commands use `CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=/usr/bin/gcc`,
`SOURCE_DATE_EPOCH=1789514690`, and the same complete remap string recorded in `node-builds.json`.

All stages inherit:

```sh
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_TERM_COLOR=never
export TMPDIR=/home/postfiatchad/.cache/release-repair-20260922/tmp
export RUST_TEST_THREADS=2 RAYON_NUM_THREADS=2 OMP_NUM_THREADS=2 MALLOC_ARENA_MAX=2
ulimit -v 20971520
ulimit -c 0
```

Per-command execution reports under `receipts/` are authoritative for complete argument vectors.
Signing-preparation stdout/stderr remain at their private cache paths; their commands and exit receipts are published.
Verifier stdout is written directly to disk, never accumulated in an in-memory pipe.
The prior raw originals were copied to `working/` and `rollback/`; the prior V2 copies were copied to
`post-v2/`. Authenticated pointer relocation verifies the original MAC and changes only
`database_directory`, recorded in the relocation receipts.

```sh
"$candidate" verify-state --data-dir "$cache/working/validator-0"
"$candidate" verify-state --data-dir "$cache/post-v2/validator-0"
# Each sequence repeats for validator-1 through validator-5, one process at a time.
"$rollback" verify-finalized-checkpoint --data-dir "$cache/rollback/validator-0"
# Repeat for all six restored originals.
"$candidate" verify-state --data-dir "$cache/rollback/validator-0"
```

The unchanged `deployments/release-repair-20260916/run_local_governance_gate.py` and its existing
service runner exercise both startup orders, certified rotation, six accepted receipts, convergence and restart.
The candidate governance CLI regenerates the six-authorized V2 batch from the retained public payload.
Only copied isolated signers and the retained six-peer 127.0.0.1 topology are used.
The rollback wrapper uses the same service runner with the exact old binary and performs reads only.
Service and preparation wrappers are retained with other orchestration tools under the disposable cache.
Software gates reuse the existing September 18 disposable test cache, without copying its 20 GiB contents.
Release builds use separate copied caches. The preserved release worktree is not accessed.

Software commands and limits are in `software-gates.json` and per-stage receipts.
Focused tests have a five-minute local budget; timeout is reported without a passing claim.
The complete workspace test suite is reserved for CI and was not run here.
Documentation uses the existing docs environment linked into the fresh worktree.

Publication runs `.venv-docs/bin/mkdocs build --strict`, `scripts/public-doc-links`, and
`scripts/public-secret-scan` in the fresh worktree. The secret scan includes staged packet files.
Raw logs preserve compiler/tool output. No source changes are permitted to resolve a failure.
The packet alone is committed and pushed to `release/combined-devnet-20260915`.
