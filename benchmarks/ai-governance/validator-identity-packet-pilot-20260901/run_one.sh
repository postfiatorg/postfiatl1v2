#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VALIDATOR_ID="nHU4bLE3EmSqNwfL4AP1UZeTNPrSPPP6FXLKXo2uqfHuvBQxDVKd"
PROMPT="$ROOT/prompts/$VALIDATOR_ID.txt"
REPO_ROOT="$(cd "$ROOT/../../.." && pwd)"
OUTPUT_ROOT="${1:?usage: run_one.sh <new-output-directory>}"
PACKET="$OUTPUT_ROOT/packets/xrpl/$VALIDATOR_ID.md"
LOG="$OUTPUT_ROOT/logs/xrpl/$VALIDATOR_ID.jsonl"
STDERR_LOG="$OUTPUT_ROOT/logs/xrpl/$VALIDATOR_ID.stderr.log"

if [[ -e "$PACKET" || -e "$LOG" || -e "$STDERR_LOG" ]]; then
  echo "refusing to overwrite an existing identity-packet run" >&2
  exit 1
fi
mkdir -p "$(dirname "$PACKET")" "$(dirname "$LOG")"

cat "$PROMPT" |
  corbanu --search -a never exec \
    -m gpt-5.6-sol \
    --json \
    --color never \
    -s read-only \
    -C "$REPO_ROOT" \
    -o "$PACKET" \
    - >"$LOG" 2>"$STDERR_LOG"

printf 'packet=%s\nlog=%s\nstderr=%s\n' "$PACKET" "$LOG" "$STDERR_LOG"
