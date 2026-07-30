#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
fleet=${PFTL_FLEET_FILE:-/home/postfiat/repos/wan-vultr-all-fleet.txt}
ssh_key=${PFTL_SSH_KEY:-/home/postfiat/.ssh/id_ed25519}
node=${PFTL_NODE_BIN:-/opt/postfiat/releases/a666-variable-nav-9ffdfb6/postfiat-node}
topology=${PFTL_TOPOLOGY:-/etc/postfiat/releases/a666-variable-nav-9ffdfb6/topology.json}
run=${PFTL_RUN_DIR:?PFTL_RUN_DIR is required}
local_evidence=${PFTL_LOCAL_EVIDENCE:?PFTL_LOCAL_EVIDENCE is required}
issuer_key=${PFTL_ISSUER_KEY:-/var/lib/postfiat/validator-2/a666-joe-e2e-20260728/pfusdc-issuer-key.json}
cast=${PFTL_CAST_BIN:-/var/lib/postfiat/validator-2/pfusdc-latency-20260727-run2/cast}
proof_dir=${PFTL_PROOF_DIR:?PFTL_PROOF_DIR is required}
resident_manifest=${A666_RESIDENT_ROUNDS_MANIFEST:-}
relay_phase=${PFTL_RELAY_PHASE:-all}
asset=02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b
policy=${PFTL_POLICY_HASH:-5025bdfe92669e3d8f81ce7e739fd132063261b92ef7e7ee7db19b2762e88b736bd40cd4826375e041584533f4137158}
vault=${PFTL_VAULT_ADDRESS:-0xaaa78fda7062efce769e95cd72fc55e507bc8183}
issuer=pf23d8831301aa1cce6fdd7bf4a2db2aead1619ba8
holder=${PFTL_HOLDER:-pfab9b9228942e5c529633a13aa271d5297bec6353}
deposit_tx=${DEPOSIT_TX:?DEPOSIT_TX is required}
deposit_atoms=${DEPOSIT_ATOMS:?DEPOSIT_ATOMS is required}
expected_holder_atoms=${EXPECTED_HOLDER_ATOMS:?EXPECTED_HOLDER_ATOMS is required}
label_suffix=${PFTL_LABEL_SUFFIX:-}

case "$relay_phase" in
  all|propose|claim) ;;
  *) echo "PFTL_RELAY_PHASE must be all, propose, or claim" >&2; exit 2 ;;
esac

v0=$(awk '$1=="validator-0"{print $2}' "$fleet")
v2=$(awk '$1=="validator-2"{print $2}' "$fleet")

ssh_v2() {
  ssh -i "$ssh_key" "root@$v2" "$@"
}

host_for_validator() {
  awk -v validator="$1" '$1==validator{print $2}' "$fleet"
}

submit_round() {
  local ops_file=$1
  local height=$2
  local max_transactions=$3
  local label=$4
  local proposer
  proposer=$(ssh -i "$ssh_key" "root@$v0" \
    "$node block-proposer --unsafe-devnet-json-storage \
      --data-dir /var/lib/postfiat/validator-0 --height $height --view 0" |
    jq -er .proposer)
  local host
  host=$(host_for_validator "$proposer")
  local index=${proposer#validator-}
  local remote_ops=/var/lib/postfiat/validator-"$index"/"$label".ops.json
  scp -q -3 -i "$ssh_key" "root@$v2:$ops_file" "root@$host:$remote_ops"

  if test -n "$resident_manifest"; then
    local build_dir=/var/lib/postfiat/validator-"$index"/"$label"-batch-build
    ssh -i "$ssh_key" "root@$host" "set -euo pipefail
      test ! -e '$build_dir'
      '$node' pftl-submit-certified-asset-ops \
        --data-dir /var/lib/postfiat/validator-$index \
        --topology '$topology' \
        --key-file /var/lib/postfiat/validator-$index/validator_keys.json \
        --proposal-key-file /var/lib/postfiat/validator-$index/validator_keys.json \
        --ops-file '$remote_ops' \
        --artifact-dir '$build_dir' \
        --max-transactions '$max_transactions' \
        --require-local-proposer \
        --height '$height' \
        --timeout-ms 30000 \
        --send-retries 16 \
        --retry-backoff-ms 250 \
        --quorum-early-full-propagation \
        --local-apply-before-certified-send \
        --batch-only" > "$local_evidence/$label.batch-build-report.json"
    jq -e --argjson count "$max_transactions" \
      '.batch_only==true and .round_ok==null and .operation_count==$count' \
      "$local_evidence/$label.batch-build-report.json" >/dev/null
    scp -q -i "$ssh_key" \
      "root@$host:$build_dir/mempool-batch.json" \
      "$local_evidence/$label.batch.json"
    python3 "$repo/scripts/a666-resident-rounds.py" submit \
      --manifest "$resident_manifest" \
      --batch-file "$local_evidence/$label.batch.json" \
      --batch-kind transparent \
      --label "$label" \
      --artifact-dir "$local_evidence/$label.finality"
    cp "$local_evidence/$label.finality/summary.json" \
      "$local_evidence/$label.report.json"
  else
    ssh -i "$ssh_key" "root@$host" "set -euo pipefail
      '$node' pftl-submit-certified-asset-ops \
        --data-dir /var/lib/postfiat/validator-$index \
        --topology '$topology' \
        --key-file /var/lib/postfiat/validator-$index/validator_keys.json \
        --proposal-key-file /var/lib/postfiat/validator-$index/validator_keys.json \
        --ops-file '$remote_ops' \
        --artifact-dir /var/lib/postfiat/validator-$index/$label \
        --max-transactions '$max_transactions' \
        --require-local-proposer \
        --height '$height' \
        --timeout-ms 180000 \
        --send-retries 16 \
        --retry-backoff-ms 250 \
        --quorum-early-full-propagation \
        --local-apply-before-certified-send" > "$local_evidence/$label.report.json"
  fi
  jq -e --argjson height "$height" \
    '(.end_height==$height)
     and ((.end_mempool_pending // 0)==0)
     and .round_ok==true' \
    "$local_evidence/$label.report.json" >/dev/null
}

mkdir -p "$local_evidence"
start_height=$(ssh_v2 \
  "'$node' status --data-dir /var/lib/postfiat/validator-2" | jq -er .block_height)
relay_height=$((start_height + 1))
if test "$relay_phase" = all; then
  claim_height=$((start_height + 2))
else
  claim_height=$((start_height + 1))
fi

if test "$relay_phase" != claim; then
  ssh_v2 "set -euo pipefail
  test -x '$cast'
  test -s '$proof_dir/proof-calldata.bin'
  test -s '$proof_dir/public-values.bin'
  '$node' status --data-dir /var/lib/postfiat/validator-2 \
    --expect-height '$start_height' >/dev/null
  '$node' vault-bridge-deposit-relay-rpc-bundle \
    --cast-bin '$cast' \
    --source-rpc-url https://ethereum-rpc.publicnode.com \
    --tx-hash '$deposit_tx' \
    --vault-address '$vault' \
    --token-address 0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48 \
    --asset-id '$asset' \
    --policy-hash '$policy' \
    --proposer '$issuer' \
    --finalizer '$issuer' \
    --claimer '$issuer' \
    --expires-at-height 2000 \
    --bundle '$run/relay-bundle' \
    --overwrite \
    --source-proof-kind sp1-ethereum-finality-v1 \
    --source-proof-file '$proof_dir/proof-calldata.bin' \
    --source-public-values-file '$proof_dir/public-values.bin' \
    > '$run/relay-bundle.report.json'
  '$node' pftl-certified-asset-ops-from-bundle \
    --bundle '$run/relay-bundle' \
    --output '$run/full.ops.json' \
    --proposer-key-file '$issuer_key' \
    --finalizer-key-file '$issuer_key' \
    --claimer-key-file '$issuer_key' \
    --overwrite
  test \"\$(jq '.operations|length' '$run/full.ops.json')\" = 3
  test \"\$(jq -r '[.operations[].label]|join(\",\")' \
    '$run/full.ops.json')\" = 'propose,finalize,claim'
  jq '{schema,operations:[.operations[]|select(.label==\"propose\")]}' \
    '$run/full.ops.json' > '$run/propose.ops.json'
  jq --arg holder '$holder' \
    '{schema,operations:[.operations[]
      |select(.label==\"finalize\" or .label==\"claim\")
      |if .label==\"claim\" then .operation.recipient=\$holder else . end]}' \
    '$run/full.ops.json' > '$run/finalize-claim.ops.json'
  test \"\$(jq '.operations|length' '$run/propose.ops.json')\" = 1
  test \"\$(jq '.operations|length' '$run/finalize-claim.ops.json')\" = 2
  test \"\$(jq -r '.operations[1].operation.recipient' \
    '$run/finalize-claim.ops.json')\" = '$holder'"
else
  ssh_v2 "set -euo pipefail
    test -s '$run/full.ops.json'
    test -s '$run/propose.ops.json'
    test -s '$run/finalize-claim.ops.json'
    test \"\$(jq -r '.operations[1].operation.recipient' \
      '$run/finalize-claim.ops.json')\" = '$holder'"
fi

if test "$relay_phase" != claim; then
  submit_round "$run/propose.ops.json" "$relay_height" 1 \
    "joe-pfusdc-propose-h$relay_height$label_suffix"
fi
if test "$relay_phase" != propose; then
  submit_round "$run/finalize-claim.ops.json" "$claim_height" 2 \
    "joe-pfusdc-claim-h$claim_height$label_suffix"
fi

if test "$relay_phase" != propose; then
  ssh_v2 "set -euo pipefail
  '$node' status --data-dir /var/lib/postfiat/validator-2 \
    --expect-height '$claim_height' > '$run/post-status.json'
  '$node' account-assets \
    --data-dir /var/lib/postfiat/validator-2 \
    --account '$holder' \
    --asset-id '$asset' > '$run/holder-after-claim.json'
  jq -e --argjson expected '$expected_holder_atoms' \
    '.assets|any(.balance == \$expected)' '$run/holder-after-claim.json' >/dev/null"
fi

if test "$relay_phase" != claim; then
  scp -q -i "$ssh_key" "root@$v2:$run/relay-bundle.report.json" \
    "$local_evidence/relay-bundle.report.json"
fi
if test "$relay_phase" != propose; then
  scp -q -i "$ssh_key" "root@$v2:$run/holder-after-claim.json" \
    "$local_evidence/holder-after-claim.json"
  scp -q -i "$ssh_key" "root@$v2:$run/post-status.json" \
    "$local_evidence/post-status.json"
fi

completed_height=$relay_height
if test "$relay_phase" != propose; then
  completed_height=$claim_height
fi

jq -n \
  --argjson start_height "$start_height" \
  --argjson finalized_height "$completed_height" \
  --argjson amount_atoms "$deposit_atoms" \
  --arg deposit_tx "$deposit_tx" \
  --arg relay_phase "$relay_phase" \
  '{
    schema:"postfiat.a666.pfusdc_relay.v2",
    verdict:"PASS",
    execution_mode:$relay_phase,
    deposit_tx:$deposit_tx,
    start_height:$start_height,
    finalized_height:$finalized_height,
    amount_atoms:$amount_atoms
  }' > "$local_evidence/summary.json"
cat "$local_evidence/summary.json"
