#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
temporary=$(mktemp -d)
trap 'find "$temporary" -depth -delete' EXIT

valid_specs='validator-0,39650,v0.example,27650;validator-1,39651,v1.example,27651;validator-2,39652,v2.example,27652;validator-3,39653,v3.example,27653;validator-4,39654,v4.example,27654;validator-5,39655,v5.example,27655;private-swap,39798,v2.example,8798'

set +e
A666_WALLET_SSH_BIN=/bin/true \
A666_WALLET_SSH_IDENTITY_FILE="$repo/README.md" \
A666_WALLET_TUNNEL_SPECS="$valid_specs" \
  "$repo/scripts/start-a666-wallet-tunnels.sh" >"$temporary/valid.out" 2>&1
valid_status=$?

A666_WALLET_SSH_BIN=/bin/true \
A666_WALLET_SSH_IDENTITY_FILE="$repo/README.md" \
A666_WALLET_TUNNEL_SPECS='validator-0,39650,v0.example,27650' \
  "$repo/scripts/start-a666-wallet-tunnels.sh" >"$temporary/incomplete.out" 2>&1
incomplete_status=$?

A666_WALLET_SSH_BIN=/bin/true \
A666_WALLET_SSH_IDENTITY_FILE="$repo/README.md" \
A666_WALLET_TUNNEL_SPECS='validator-0,039650,v0.example,27650' \
  "$repo/scripts/start-a666-wallet-tunnels.sh" >"$temporary/malformed.out" 2>&1
malformed_status=$?

A666_WALLET_SSH_BIN=/bin/true \
A666_WALLET_SSH_IDENTITY_FILE="$repo/README.md" \
A666_WALLET_TUNNEL_SPECS="${valid_specs/validator-1,39651/validator-0,39651}" \
  "$repo/scripts/start-a666-wallet-tunnels.sh" >"$temporary/duplicate-name.out" 2>&1
duplicate_name_status=$?

A666_WALLET_SSH_BIN=/bin/true \
A666_WALLET_SSH_IDENTITY_FILE="$repo/README.md" \
A666_WALLET_TUNNEL_SPECS="${valid_specs/validator-1,39651/validator-1,39650}" \
  "$repo/scripts/start-a666-wallet-tunnels.sh" >"$temporary/duplicate-port.out" 2>&1
duplicate_port_status=$?
set -e

[[ $valid_status -eq 1 ]]
[[ $incomplete_status -eq 2 ]]
[[ $malformed_status -eq 2 ]]
[[ $duplicate_name_status -eq 2 ]]
[[ $duplicate_port_status -eq 2 ]]
grep -Fq 'restarting the complete bounded tunnel set' "$temporary/valid.out"
grep -Fq 'exactly six validator RPC tunnels and one private-swap tunnel are required' "$temporary/incomplete.out"
grep -Fq 'invalid bounded A666 wallet tunnel entry' "$temporary/malformed.out"
grep -Fq 'duplicate or unexpected A666 wallet tunnel entry' "$temporary/duplicate-name.out"
grep -Fq 'duplicate or unexpected A666 wallet tunnel entry' "$temporary/duplicate-port.out"

echo 'A666 restart-managed wallet tunnel regression passed'
