#!/usr/bin/env bash
# Test the supplied local directory with the absolute generation paths used on the fleet.
set -eu
bin=$(realpath "${1:?release binary}")
src=$(realpath "${2:?local validator store}")
node=${src##*/}
[[ $node =~ ^validator-[0-5]$ ]]
cmd=${3:-status}
case $cmd in status|verify-finalized-checkpoint|verify-state|verify-blocks) ;; *) exit 2;; esac
# A disposable container layer permits the binary's own recovery and key creation.
# The source is mounted read-only. Only the canonical data directory is projected.
docker run --rm --network none --user "$(id -u):$(id -g)" --tmpfs "/var/lib/postfiat:uid=$(id -u),gid=$(id -g),mode=0700" --cap-drop ALL --security-opt no-new-privileges \
  --mount "type=bind,src=$bin,dst=/opt/node,readonly" \
  --mount "type=bind,src=$src,dst=/source,readonly" ubuntu:24.04 \
  bash -euc 'dir=/var/lib/postfiat/$1; mkdir -p "$dir"; cp -as /source/. "$dir/"
    exec /opt/node "$2" --data-dir "$dir"' bash "$node" "$cmd"
