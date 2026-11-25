#!/usr/bin/env bash
# Build the B24 rehearsal tool from the commit the validators run, install it for the
# daily job and print its known-releases.txt line. Changes nothing on any validator.
# Usage: ops/b24-live/rehearsal_tool.sh <release-name> <commit> <node-sha256>
#
# The build is reproducible: the same commit gives the same sha256 on every run on this machine.
# It runs at a fixed path that depends only on the commit, with that path, the Cargo home and the
# toolchain remapped out of the binary, and it links with the system compiler, which adds no
# RUNPATH (a linker that adds one records rustc's random temporary directory in the binary).
set -euo pipefail
[ $# -eq 3 ] || { sed -n 4p "$0" >&2; exit 2; }
name=$1 commit=$2 node=$3
repo=$(cd "$(dirname "$0")/../.." && pwd)
coord=${B23_COORD:-/home/postfiat/repos/b23-coord}
dest=$coord/artifacts/$name
full=$(git -C "$repo" rev-parse --verify "$commit^{commit}")
root=/tmp/b23-nav-canary-build
src=$root/$full
target=$root/$full.target
mkdir -p "$root"
# A tree or target left by an earlier run at this path is replaced, never reused.
git -C "$repo" worktree remove --force "$src" >/dev/null 2>&1 || true
rm -rf -- "$src" "$target"
git -C "$repo" worktree add --detach "$src" "$full" >/dev/null
trap 'git -C "$repo" worktree remove --force "$src"; rm -rf -- "$target"' EXIT
tool=tools/b23-nav-canary
placed=()
if [ ! -e "$src/$tool/build.rs" ]; then
  # A commit from before the tool recorded its source: put in place this checkout's build.rs, the
  # main.rs that answers build-info and the completed lock. The recorded source names them.
  for file in build.rs src/main.rs Cargo.lock; do
    cp "$repo/$tool/$file" "$src/$tool/$file"
    placed+=("$tool/$file")
  done
elif ! cargo metadata --locked --offline --format-version 1 --manifest-path "$src/$tool/Cargo.toml" >/dev/null 2>&1; then
  cp "$repo/$tool/Cargo.lock" "$src/$tool/Cargo.lock"
  placed+=("$tool/Cargo.lock")
fi
source_text=$(python3 "$repo/ops/b24-live/tool_source.py" "$src" "${placed[@]}")
case $source_text in *+modified) echo "The tree at $full differs from the commit beyond the files put in place; not built." >&2; exit 1;; esac
cargo_home=${CARGO_HOME:-$HOME/.cargo}
rustup_home=${RUSTUP_HOME:-$HOME/.rustup}
export CC=/usr/bin/cc
export RUSTFLAGS="-C linker=/usr/bin/cc --remap-path-prefix=$src=/b23-nav-canary --remap-path-prefix=$cargo_home=/cargo --remap-path-prefix=$rustup_home=/rustup"
export CARGO_INCREMENTAL=0 POSTFIAT_BUILD_SOURCE=$source_text
(cd "$src" && CARGO_TARGET_DIR=$target cargo build --release --locked --manifest-path "$src/$tool/Cargo.toml")
install -D -m 755 "$target/release/postfiat-b23-nav-canary" "$dest/postfiat-b23-nav-canary"
(cd "$dest" && sha256sum postfiat-b23-nav-canary > SHA256SUMS)
sha=$(cut -d' ' -f1 "$dest/SHA256SUMS")
echo "Installed $dest/postfiat-b23-nav-canary ($sha), source $source_text."
echo "Add this line to ops/b24-live/known-releases.txt (replace an older line for $name):"
echo "$name ${full:0:9} $node $sha"
echo "Then copy it to validator-0 as /tmp/b23-tee-nav-canary-${sha:0:16} and run: python3 ops/b24-live/daily.py --check"
