#!/usr/bin/env bash
# Start from an official genesis archive and a matching full/incremental snapshot pair.
set -euo pipefail
cluster=${1:?usage: run-node.sh testnet|mainnet-beta ledger-directory identity-file rpc-port}
ledger=$(realpath "${2:?ledger directory}")
identity=${3:?path to observer identity}
port=${4:?private RPC port}
case "$cluster" in
    testnet) genesis=4uhcVJyU9pJkvQyS88uRDiswHXSCkY3zQawwpjk2NsNY ;;
    mainnet-beta) genesis=5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d ;;
    *) echo "unsupported cluster" >&2; exit 2 ;;
esac
export SOLBRIDGE_CAPTURE_DIR="$ledger/public-capture"
mkdir -p "$SOLBRIDGE_CAPTURE_DIR"
extra=(--no-xdp)
if [[ ${MAPPED_PORTS:-0} == 1 ]]; then
    extra+=(--no-port-check)
fi
if [[ -n ${PUBLIC_TVU:-} ]]; then
    extra+=(--public-tvu-address "$PUBLIC_TVU")
fi
exec "${AGAVE_VALIDATOR:-agave-validator}" \
    --identity "$identity" --ledger "$ledger" --no-voting \
    --entrypoint "entrypoint.$cluster.solana.com:8001" \
    --entrypoint "entrypoint2.$cluster.solana.com:8001" \
    --expected-genesis-hash "$genesis" \
    --no-snapshot-fetch --no-genesis-fetch --no-snapshots \
    --no-poh-speed-test --no-os-network-limits-test \
    --gossip-port "${GOSSIP_PORT:-8801}" --dynamic-port-range "${DYNAMIC_PORT_RANGE:-8802-8840}" \
    --limit-blockstore-size 100000000 --no-accounts-db-snapshots-direct-io \
    --rpc-port "$port" --rpc-bind-address 127.0.0.1 --private-rpc --full-rpc-api \
    --log "$ledger/observer.log" "${extra[@]}"
