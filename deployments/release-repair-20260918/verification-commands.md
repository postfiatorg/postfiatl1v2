# Verification commands and scope

Initial qualification source: `d224c0be402dbd7bda4fa350ad3f349f009d0667`.
Repair/build source: `03e422a722eba5bc37e9b3ea71ecae81c02d6f45`.
Initial fetch and source pin were performed at 2026-09-18T10:40:05Z.
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
Raw logs preserve compiler/tool output. The initial qualification made no source changes.
The follow-up authorized only the minimal Clippy test repair and build/packet correction below.

## Local failure repair

The follow-up fetched origin and created `/tmp/qualify-fix-20260918` detached at `e49a2390`.
Only `.err().expect(...)` in the Cobalt handoff test changes, to `.expect_err(...)`.
The exact Clippy command is `cargo clippy --workspace --all-targets --locked -- -D warnings`;
the focused test command is `cargo test -p postfiat-node --lib --locked cobalt_handoff::tests`.
Focused tests reuse the existing optimized test cache with `CARGO_PROFILE_TEST_OPT_LEVEL=2`,
`CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true`, and `CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true`.
No full workspace or Orchard test suite is rerun for this test-only change.

The corrected builds reuse the September 16 procedure and its inherited
[explicit linker/environment settings](../combined-release-20260915/build-provenance.md).
Two new clean detached source worktrees are created at the Clippy fix commit;
their target directories are separate copies of the September 16 existing caches.
Both receive the identical complete `RUSTFLAGS` string below. The GCC linker selection
avoids the ambient Zig wrapper's automatically generated RUNPATH; no executable is patched,
stripped, or otherwise changed after compilation. GCC supplies its normal deterministic
SHA-1 build ID. Exact commands, environments, commit/tree identities, times and hashes
are in [node-builds.json](node-builds.json).

```sh
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_TERM_COLOR=never
export TMPDIR=/home/postfiatchad/.cache/qualify-fix-20260918/tmp
export RUST_TEST_THREADS=2 RAYON_NUM_THREADS=2 OMP_NUM_THREADS=2 MALLOC_ARENA_MAX=2
export PYTHONDONTWRITEBYTECODE=1
export SOURCE_DATE_EPOCH=1789514690
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=/usr/bin/gcc
export RUSTFLAGS="--remap-path-prefix=/home/postfiatchad/.cache/qualify-fix-20260918/source-1=/src/postfiatl1v2 --remap-path-prefix=/home/postfiatchad/.cache/qualify-fix-20260918/source-2=/src/postfiatl1v2 --remap-path-prefix=/home/postfiatchad/.cache/qualify-fix-20260918/target-1=/target --remap-path-prefix=/home/postfiatchad/.cache/qualify-fix-20260918/target-2=/target"
ulimit -v 20971520
ulimit -c 0
for i in 1 2; do
  (cd "/home/postfiatchad/.cache/qualify-fix-20260918/source-$i" && \
    CARGO_TARGET_DIR="/home/postfiatchad/.cache/qualify-fix-20260918/target-$i" \
    cargo build --release --locked -p postfiat-node --bin postfiat-node) || exit
done
cmp /home/postfiatchad/.cache/qualify-fix-20260918/binaries/candidate-1 \
    /home/postfiatchad/.cache/qualify-fix-20260918/binaries/candidate-2
```

The build runner copies each unmodified executable into `binaries/` for comparison.
All follow-up processes inherit the same 20 GiB address-space limit and disk-backed TMPDIR;
Cargo commands run sequentially. The 60-minute repair box began at 12:27:39 UTC.
The three publication gates are rerun on the updated packet; MkDocs output goes to the disk cache.
History, governed continuation, restart and rollback evidence remains explicitly bound to
`d224c0be402dbd7bda4fa350ad3f349f009d0667` and its original build-1 hash.
No Task Node or fleet operation is part of this follow-up.
