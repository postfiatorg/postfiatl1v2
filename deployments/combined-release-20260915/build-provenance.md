# Reproducible build provenance

Both final builds use clean detached worktrees at `87cf9dd136e79e041b395229b9b734ac05e0d8e2` (tree `b9def5abd981bfbddc2dac8ba6e0c2c5ffc04e25`). They use separate, initially empty target directories. Subsequent commit `dd276d70e0974fa17e7dfa3ad0b4e6cbd00349f3` changes a Solidity test comment; subsequent qualification records leave the compiled node source unchanged.

| Identity | Value |
| --- | --- |
| Both node executables, SHA-256 | `203995122290895a023dde4bf1cfb8cc4f05b22628822d7a32829ffd185fb314` |
| Both executable sizes | 62,508,720 bytes |
| Cargo.lock, SHA-256 | `8a4b5e43056f021d07c6bf150c8cfd8c470d20ca9a076a31418fa34c14697d55` |
| rustc | `1.95.0 (59807616e 2026-04-14)` |
| cargo | `1.95.0 (f2d3ce0bd 2026-03-21)` |
| Rust linker | `/usr/bin/gcc`, GCC 15.2.0 |
| Native C compiler | Existing `cc` wrapper, Zig `0.17.0-dev.1857` |
| Target | `x86_64-unknown-linux-gnu` |
| Required glibc | 2.39; all six fleet machines report x86_64 / glibc 2.39 |
| ELF search paths | No RPATH or RUNPATH |
| SBOM, SHA-256 | `1b76954fa40c25553452f31d909a8e9671b5bdf3afcbeb29c97491a07154125e` |

For each build, `release_root` was `/home/postfiatchad/.cache/combined-release-20260915` and `i` was 1 or 2. The effective invocation from the corresponding `build-source-$i` checkout was:

```sh
CARGO_BUILD_JOBS=2 \
CARGO_INCREMENTAL=0 \
SOURCE_DATE_EPOCH=1789514690 \
CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=/usr/bin/gcc \
CARGO_TARGET_DIR="$release_root/final-target-$i" \
RUSTFLAGS="--remap-path-prefix=$release_root/build-source-$i=/src/postfiatl1v2 --remap-path-prefix=$release_root/final-target-$i=/target" \
cargo build --release --locked -p postfiat-node --bin postfiat-node
```

Both commands exit 0. `cmp` on the two executables exits 0; no executable postprocessing was performed. Complete compiler output is retained in `logs/final-release-build-1.log` and `logs/final-release-build-2.log`. The six-copy service receipt identifies the final binary and its embedded build revision `87cf9dd1`.

The exact deployed A666 rollback binary has SHA-256 `57b0f4d1d42d66878d7dbb8c33919c7fa0f87c6cc1a4b9cc1a85d75b634eec83`. The old binary's successful **finalized-checkpoint** export produces snapshot-manifest SHA-256 `fa167b7979cd4d8a8ad7b16b2597eda7148615fb6d5b9967bbaa667904253b3e`. Full-history export fails at block 1011; the checkpoint export establishes a narrower result.

Executables, snapshot data and private node material remain in the local qualification cache. This packet records their identities and exposes the reviewable test/build output.
