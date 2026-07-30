#!/usr/bin/env bash
set -euo pipefail

state_dir=${A666_WALLET_STATE_DIR:-/home/postfiat/.local/state/postfiat-a666-wallet}
token_file=$state_dir/proxy-tokens.json
test -r "$token_file"
test "$(stat -c %a "$token_file")" = 600
jq -er '."local-demo"' "$token_file"
