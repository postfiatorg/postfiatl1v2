#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  printf 'usage: %s SIGNED_ESCROW_TRANSACTION_JSON_FILE\n' "$0" >&2
  exit 2
fi

BIN=/home/postfiat/tmp/pftl-escrow-ae3c53c-00616722/postfiat-node
BASE=/home/postfiat/tmp/pftl-proven-nav-v2-20260724
TOPO=$BASE/public/topology.json
SIGNED_FILE=$1

jq empty "$SIGNED_FILE"
height=
root=
for i in 0 1 2 3 4 5; do
  status=$("$BIN" status --data-dir "$BASE/nodes/validator-$i")
  node_height=$(jq -r '.block_height' <<<"$status")
  node_root=$(jq -r '.state_root' <<<"$status")
  if [ -z "$height" ]; then
    height=$node_height
    root=$node_root
  elif [ "$node_height" != "$height" ] || [ "$node_root" != "$root" ]; then
    printf 'validator-%s is not converged\n' "$i" >&2
    exit 1
  fi
done

next_height=$((height + 1))
proposer=$("$BIN" block-proposer \
  --data-dir "$BASE/nodes/validator-0" \
  --height "$next_height" \
  --view 0 | jq -r '.proposer')
proposer_index=${proposer#validator-}
proposer_dir=$BASE/nodes/validator-$proposer_index
source=$(jq -r '.unsigned.source' "$SIGNED_FILE")
sequence=$(jq -r '.unsigned.sequence' "$SIGNED_FILE")
escrow_id=$(jq -r '.unsigned.escrow_id' "$SIGNED_FILE")
pending=$("$BIN" mempool-status --data-dir "$proposer_dir")
matches=$(jq \
  --arg source "$source" \
  --arg sequence "$sequence" \
  --arg escrow_id "$escrow_id" \
  '[.pending_escrow_transactions[].transaction.unsigned
    | select(.source == $source)
    | select((.sequence | tostring) == $sequence)
    | select(.escrow_id == $escrow_id)] | length' <<<"$pending")
if [ "$matches" -ne 1 ]; then
  printf 'expected one bound pending escrow transaction, found %s\n' "$matches" >&2
  exit 1
fi

stamp=$(date -u +%Y%m%dT%H%M%SZ)
run_dir=$BASE/runtime/escrow-pending-height-$next_height-$stamp
artifact_dir=$BASE/private/finality/height-$next_height-escrow-pending-$stamp
mkdir -p "$run_dir" "$artifact_dir"
export POSTFIAT_PREWARM_SHIELDED_VERIFIER=1
export POSTFIAT_PREWARM_ASSET_ORCHARD_SWAP_VERIFIER=1
export POSTFIAT_PREWARM_ASSET_ORCHARD_PRIVATE_EGRESS_VERIFIER=1

pids=()
cleanup() {
  for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
  for pid in "${pids[@]}"; do wait "$pid" 2>/dev/null || true; done
}
trap cleanup EXIT

for i in 0 1 2 3 4 5; do
  mkdir -p "$run_dir/validator-$i-votes"
  POSTFIAT_TRANSPORT_VALIDATOR_READY_FILE="$run_dir/validator-$i.ready.json" \
  "$BIN" transport-validator-serve \
    --unsafe-devnet-file-signer \
    --unsafe-devnet-json-storage \
    --data-dir "$BASE/nodes/validator-$i" \
    --topology "$TOPO" \
    --key-file "$BASE/nodes/validator-$i/validator_keys.json" \
    --vote-dir "$run_dir/validator-$i-votes" \
    --bind-host 127.0.0.1 \
    --max-connections 16 \
    --timeout-ms 30000 \
    --event-log "$run_dir/validator-$i.events.jsonl" \
    >"$run_dir/validator-$i.stdout.log" 2>&1 &
  pids+=("$!")
done

ready=0
for _ in $(seq 1 160); do
  ready=0
  for i in 0 1 2 3 4 5; do
    if [ -s "$run_dir/validator-$i.ready.json" ]; then
      ready=$((ready + 1))
    fi
  done
  if [ "$ready" -eq 6 ]; then break; fi
  sleep 0.25
done
if [ "$ready" -ne 6 ]; then
  printf 'validator services did not become ready: %s/6\n' "$ready" >&2
  exit 1
fi

"$BIN" transport-peer-certified-mempool-round \
  --data-dir "$proposer_dir" \
  --topology "$TOPO" \
  --key-file "$proposer_dir/validator_keys.json" \
  --proposal-key-file "$proposer_dir/validator_keys.json" \
  --require-local-proposer \
  --quorum-early-full-propagation \
  --local-apply-before-certified-send \
  --artifact-dir "$artifact_dir" \
  --height "$next_height" \
  --view 0 \
  --timeout-ms 30000 \
  --send-retries 2 \
  --retry-backoff-ms 200 \
  --max-transactions 1 \
  | tee "$artifact_dir/report.json"

for i in 0 1 2 3 4 5; do
  "$BIN" verify-state --data-dir "$BASE/nodes/validator-$i" >/dev/null
  "$BIN" verify-blocks --data-dir "$BASE/nodes/validator-$i" >/dev/null
done
printf 'finalized pending escrow at height %s via %s; report: %s\n' \
  "$next_height" "$proposer" "$artifact_dir/report.json"
