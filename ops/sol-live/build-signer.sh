#!/usr/bin/env bash
# Link the helper against the exact validator release's wallet implementation.
set -eu
cd "$(dirname "$0")/../.."
release=${1:-/mnt/HC_Volume_106212907/stakehub-hyperliquid/sol-release6/r6-1-target-primary/release}
args=(-L "dependency=$release/deps")
for name in postfiat_node serde_json; do
    libs=("$release/deps/lib$name"-*.rlib)
    test "${#libs[@]}" -eq 1
    args+=(--extern "$name=${libs[0]}")
done
while IFS= read -r directory; do args+=(-L "native=$directory"); done < <(find "$release/build" -name '*.a' -printf '%h\n' | sort -u)
mkdir -p target/task-l
rustc +1.95.0 --edition=2021 -C opt-level=2 ops/sol-live/warm-signer.rs "${args[@]}" -o target/task-l/warm-signer.new
mv target/task-l/warm-signer{.new,}
