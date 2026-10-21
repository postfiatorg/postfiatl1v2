#!/usr/bin/env bash
# The FastSwap service and committee migration tests live in the postfiat-node bin target (main.rs), not the lib.
set -u
R=$HOME/.cache/release-store-handle-20261021; L=$R/logs
cd $HOME/repos/postfiatl1v2
while pgrep -f tools/focused.sh >/dev/null; do sleep 2; done
run() { name=$1; shift; echo "$(date -u +%FT%TZ) START $name: $*"; "$@" > $L/focused-$name.log 2>&1; echo "$(date -u +%FT%TZ) EXIT $? $name"; }
run node-bin-fastswap-service-and-committee-migration cargo test -p postfiat-node --bin postfiat-node fastswap_service::
echo "$(date -u +%FT%TZ) DONE"
