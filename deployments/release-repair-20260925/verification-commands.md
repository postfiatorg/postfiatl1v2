# Verification commands and scope

Started 2026-09-25T10:17Z with a 100-minute limit. The start commands were:

```sh
git fetch origin
git rev-parse origin/release/combined-fastpay-20260925   # a2dfa94735a1182551931b0de92da701898312b4
git worktree add --detach ~/.cache/qualify-20260925 origin/release/combined-fastpay-20260925
```

## Pre-step (tests only)

On the branch, `test_unwrap_fastpay_prefers_recovery_safe_v3_and_authenticates_apply_quorum`
and `test_send_fastpay_prefers_recovery_safe_v3_and_authenticates_apply_quorum`
had their `verified_effects` and `created_objects` fixtures swapped.
Both functions were taken from main at `d8f65885`. Main's version of the file
differs from the branch only in those two functions.

```sh
git checkout d8f65885 -- python/tests/test_wallet.py
(cd python && python3 -m pytest -q tests/test_wallet.py)   # 67 passed
git commit -m "Restore the two FastPay wallet tests from main"
git push origin HEAD:release/combined-fastpay-20260925      # f60e9639
```

The qualified source is `f60e9639f83649769f29276a9de14f5f16271877`.
[Pre-step test log](logs/pre-step-wallet-tests.log).

## Limits

```sh
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_TERM_COLOR=never
export TMPDIR=/home/postfiatchad/.cache/release-repair-20260925/tmp
export RUST_TEST_THREADS=2 RAYON_NUM_THREADS=2 OMP_NUM_THREADS=2 MALLOC_ARENA_MAX=2
ulimit -v 20971520
ulimit -c 0
```

Only one Cargo process runs at a time; the runner waits while any `cargo` process
exists. A command stops if available memory drops below 4 GiB, and replays need
8 GiB free before they start. Verifier stdout goes straight to disk.

## Builds

Two clean detached worktrees at the source commit were built, one at a time, from
separate copies of the September 22 release caches:

```sh
cargo build --release --locked -p postfiat-node --bin postfiat-node
```

Both builds used `SOURCE_DATE_EPOCH=1789514690`, the `/usr/bin/gcc` Rust linker, and
this remap string. It follows the September 22 scheme with this run's directories:

```text
--remap-path-prefix=<root>/source-1=/src/postfiatl1v2 --remap-path-prefix=<root>/source-2=/src/postfiatl1v2
--remap-path-prefix=<root>/target-1=/target --remap-path-prefix=<root>/target-2=/target
```

`<root>` is `/home/postfiatchad/.cache/release-repair-20260925`. `readelf -d` and
`readelf -p .comment` are logged for each executable, and `cmp` compares the two.
[node-builds.json](node-builds.json) records the directories, environment, tree,
lockfile, exit status and hashes.

## History, rotation and rollback

Disposable copies were made with `cp -a`. Each database pointer was then moved with
MAC-authenticated relocation, which changes only `database_directory`:

- `working/`: originals from `~/.cache/combined-release-20260915/raw`, height 1020.
- `post-v2/`: saved September 16 V2 copies from `~/.cache/release-repair-20260916/working`, height 1021.
- `rollback/`: originals again.

Each replay ran alone:

```sh
candidate-1 verify-state --data-dir working/validator-N        # original, 1020
rollback    verify-state --data-dir r4-replay-probe/validator-0 # r4 contrast probe
candidate-1 verify-state --data-dir post-v2/validator-N        # saved V2, 1021
# governance-prepare + local governance gate rotate working/ to a fresh 1021
candidate-1 verify-state --data-dir working/validator-N        # fresh V2, 1021
rollback    verify-finalized-checkpoint --data-dir rollback/validator-N
# rollback services: r4 executable, six services, startup and restart
candidate-1 verify-state --data-dir rollback/validator-0
```

`rollback` is the deployed r4 executable, copied from
`~/.local/lib/postfiat/releases/fastpay-committee-20260925-r4/postfiat-node` after its
SHA-256 was checked. The governance gate is the unchanged
`deployments/release-repair-20260916/run_local_governance_gate.py`. It runs with copied
isolated signers and the retained six-peer 127.0.0.1 topology. The rollback wrapper uses
`deployments/signing-fix-qualification-20260909/run_local_service_gate.py`.

## Software

[software-gates.json](software-gates.json) records every command. Each receipt
under `receipts/` has the full argument vector. The tests use the existing
September 18 test cache at opt-level 2, with debug assertions and overflow checks on.
The full workspace suite and the long Orchard suite were not run locally.

## Publication

```sh
.venv-docs/bin/mkdocs build --strict
scripts/public-doc-links
scripts/public-secret-scan
```
