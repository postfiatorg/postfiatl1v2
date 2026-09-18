# Verification commands and scope

Source: `d224c0be402dbd7bda4fa350ad3f349f009d0667`. Initial fetch and source pin were performed at 2026-09-18T10:40:05Z.
The qualification checkout was created with:

```sh
git fetch origin
git rev-parse origin/release/combined-devnet-20260915
git worktree add --detach /tmp/qualify-20260918 origin/release/combined-devnet-20260915
```

[Build records](node-builds.json) contain the two source directories, tree/lockfile identities, commands,
path remapping, limits, start/end times, exits and executable hashes.
Each command was `cargo build --release --locked -p postfiat-node --bin postfiat-node`.
Separate existing target caches were copied into the new disposable cache. Only one Cargo build ran at a time.

All stages inherit:

```sh
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_TERM_COLOR=never
export TMPDIR=/home/postfiatchad/.cache/release-repair-20260918/tmp
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

Software commands and limits are in `software-gates.json` and per-stage receipts.
Focused tests have a five-minute local budget; timeout is reported without a passing claim.
The complete workspace test suite is reserved for CI and was not run here.
Documentation uses the existing docs environment linked into the fresh worktree.

Publication runs `.venv-docs/bin/mkdocs build --strict`, `scripts/public-doc-links`, and
`scripts/public-secret-scan` in the fresh worktree. The secret scan includes staged packet files.
Raw logs preserve compiler/tool output. No source changes are permitted to resolve a failure.
The packet alone is committed and pushed to `release/combined-devnet-20260915`.
