#!/bin/bash
set -euo pipefail
: "${CARGO_TARGET_DIR:?set an absolute build directory}"
ops=$(cd -- "$(dirname -- "$0")" && pwd)
build=$(mktemp -d)
trap 'rm -rf -- "$build"' EXIT
crate=$ops/../../solana/pfusdc-egress-verifier
cp -r "$crate/Cargo.toml" "$crate/Cargo.lock" "$crate/src" "$build/"
mkdir "$build/src/bin"
cp "$ops/exit-verify-proof.rs" "$build/src/bin/exit-verify-proof.rs"
cargo build --locked --release --manifest-path "$build/Cargo.toml" --features no-entrypoint --bin exit-verify-proof
