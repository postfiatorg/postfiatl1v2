#!/usr/bin/env bash
# Build the B24 rehearsal tool from the commit the validators run, install it for the
# daily job and print its known-releases.txt line. Changes nothing on any validator.
# Usage: ops/b24-live/rehearsal_tool.sh <release-name> <commit> <node-sha256>
set -euo pipefail
[ $# -eq 3 ] || { sed -n 4p "$0" >&2; exit 2; }
name=$1 commit=$2 node=$3
repo=$(cd "$(dirname "$0")/../.." && pwd)
coord=${B23_COORD:-/home/postfiat/repos/b23-coord}
dest=$coord/artifacts/$name
src=$(mktemp -d)/src
git -C "$repo" worktree add --detach "$src" "$commit" >/dev/null
trap 'git -C "$repo" worktree remove --force "$src"' EXIT
full=$(git -C "$src" rev-parse HEAD)
manifest=$src/tools/b23-nav-canary/Cargo.toml
if ! cargo metadata --locked --offline --format-version 1 --manifest-path "$manifest" >/dev/null 2>&1; then
  # Older release commits carry a tool lock file that lags the workspace; use this checkout's.
  cp "$repo/tools/b23-nav-canary/Cargo.lock" "$src/tools/b23-nav-canary/Cargo.lock"
  echo "Note: built with this checkout's tools/b23-nav-canary/Cargo.lock ($(sha256sum "$repo/tools/b23-nav-canary/Cargo.lock" | cut -c1-16)); $commit's own lock lags its workspace."
fi
CARGO_TARGET_DIR=$src/target cargo build --release --locked --manifest-path "$manifest"
install -D -m 755 "$src/target/release/postfiat-b23-nav-canary" "$dest/postfiat-b23-nav-canary"
(cd "$dest" && sha256sum postfiat-b23-nav-canary > SHA256SUMS)
tool=$(cut -d' ' -f1 "$dest/SHA256SUMS")
echo "Installed $dest/postfiat-b23-nav-canary ($tool)."
echo "Add this line to ops/b24-live/known-releases.txt:"
echo "$name ${full:0:9} $node $tool"
echo "Then copy it to validator-0 as /tmp/b23-tee-nav-canary-${tool:0:16} and run: python3 ops/b24-live/daily.py --check"
