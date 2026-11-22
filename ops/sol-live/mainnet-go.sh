#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
export SOLANA_BRIDGE_IMAGES="$PWD/target/task-l/repaired-images"
exec python3 -u ops/sol-live/mainnet-go.py "$@"
