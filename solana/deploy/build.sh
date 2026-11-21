#!/usr/bin/env bash
# Compare the deployable release images from two clean exports of a pinned commit.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
platform="${SBF_PLATFORM_TOOLS:?point SBF_PLATFORM_TOOLS at Anza platform-tools v1.58}"
ref="$(git rev-parse "${1:-HEAD}^{commit}")"
out="${SOLANA_BRIDGE_IMAGES:-$PWD/target/task-k/images}"
mkdir -p "$out"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
for pass in 1 2; do
    tree="$work/build-$pass"
    mkdir "$tree"
    git archive "$ref" solana/pfusdc-vault solana/pfusdc-recorder solana/pfusdc-egress-verifier | tar -x -C "$tree"
    for name in vault recorder egress-verifier; do
        start=$SECONDS
        RUSTFLAGS="--remap-path-prefix=$tree=/home/postfiat/repos/solbridge-live" \
            PATH="$platform/rust/bin:$PATH" RUSTC="$platform/rust/bin/rustc" CARGO_TARGET_DIR="$tree/target" \
            "$platform/rust/bin/cargo" build --locked --release --target sbpfv3-solana-solana \
            --manifest-path "$tree/solana/pfusdc-$name/Cargo.toml"
        file="pfusdc_${name//-/_}.so"
        # Strip debug/symbol sections using the pinned SBF toolchain before upload.
        "$platform/llvm/bin/llvm-objcopy" --strip-all "$tree/target/sbpfv3-solana-solana/release/$file" "$tree/$file"
        (cd "$tree" && sha256sum "$file")
        echo "build=$pass program=$name elapsed_seconds=$((SECONDS-start)) source=$ref"
    done
done
for file in pfusdc_vault.so pfusdc_recorder.so pfusdc_egress_verifier.so; do
    cmp "$work/build-1/$file" "$work/build-2/$file"
    install -m 644 "$work/build-1/$file" "$out/$file"
done
(cd "$out" && sha256sum pfusdc_{vault,recorder,egress_verifier}.so > SHA256SUMS && sha256sum -c SHA256SUMS)
