#!/usr/bin/env bash
set -euo pipefail

fleet=${PFTL_FLEET_FILE:-/home/postfiat/repos/wan-vultr-all-fleet.txt}
ssh_key=${PFTL_SSH_KEY:-/home/postfiat/.ssh/id_ed25519}
node=${PFTL_NODE_BIN:-/opt/postfiat/releases/a666-acceptance-98c81a9/postfiat-node}
topology=${PFTL_TOPOLOGY:-/etc/postfiat/releases/a666-acceptance-98c81a9/topology.json}
run=${PFTL_RUN_DIR:-/var/lib/postfiat/validator-2/a666-acceptance-20260728}
local_evidence=${PFTL_LOCAL_EVIDENCE:-$(cd "$(dirname "$0")" && pwd)}
issuer_key=${PFTL_ISSUER_KEY:-/var/lib/postfiat/validator-2/a666-joe-e2e-20260728/pfusdc-issuer-key.json}
cast=${PFTL_CAST_BIN:-/var/lib/postfiat/validator-2/pfusdc-latency-20260727-run2/cast}
proof_dir=${PFTL_PROOF_DIR:-"$run/phase1-ingress-proof-cuda"}
asset=02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b
policy=${PFTL_POLICY_HASH:-928eaf6cef31bd832f67a89e02b5c9195763c59505dadd46c7439679643b26a06e5a6269ae41de2bb2ef2960716a7c81}
vault=${PFTL_VAULT_ADDRESS:-0x8583409ddbac984ec195dfa06a21103d92403c1e}
issuer=pf23d8831301aa1cce6fdd7bf4a2db2aead1619ba8
holder=pfab9b9228942e5c529633a13aa271d5297bec6353
deposit_tx=${DEPOSIT_TX:-0xadecf2fe0b96b7aef2eaaa62ebeac33f16201f8014c784603486a46fe1a0cbb1}
deposit_atoms=${DEPOSIT_ATOMS:-1005000}
expected_holder_atoms=${EXPECTED_HOLDER_ATOMS:-1805000}
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
    "$node block-proposer --unsafe-devnet-json-storage --data-dir /var/lib/postfiat/validator-0 --height $height --view 0" |
    jq -er .proposer)
  local host
  host=$(host_for_validator "$proposer")
  local index=${proposer#validator-}
  local remote_ops=/var/lib/postfiat/validator-"$index"/"$label".ops.json
  scp -q -3 -i "$ssh_key" "root@$v2:$ops_file" "root@$host:$remote_ops"
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
      --send-retries 8 \
      --retry-backoff-ms 250 \
      --quorum-early-full-propagation \
      --local-apply-before-certified-send" > "$local_evidence/$label.report.json"
  jq -e --argjson height "$height" \
    '.end_height==$height and .end_mempool_pending==0 and .round_ok==true' \
    "$local_evidence/$label.report.json" >/dev/null
}

mkdir -p "$local_evidence"
start_height=$(ssh_v2 \
  "'$node' status --data-dir /var/lib/postfiat/validator-2" | jq -er .block_height)
propose_height=$((start_height + 1))
claim_height=$((start_height + 2))

ssh_v2 "set -euo pipefail
  test -x '$cast'
  test -s '$proof_dir/proof-calldata.bin'
  test -s '$proof_dir/public-values.bin'
  '$node' status --data-dir /var/lib/postfiat/validator-2 --expect-height '$start_height' >/dev/null
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
  jq '{schema,operations:[.operations[]|select(.label==\"propose\")]}' \
    '$run/full.ops.json' > '$run/propose.ops.json'
  jq --arg holder '$holder' \
    '{schema,operations:[.operations[]|select(.label==\"finalize\" or .label==\"claim\")|if .label==\"claim\" then .operation.recipient=\$holder else . end]}' \
    '$run/full.ops.json' > '$run/finalize-claim.ops.json'
  test \"\$(jq '.operations|length' '$run/propose.ops.json')\" = 1
  test \"\$(jq '.operations|length' '$run/finalize-claim.ops.json')\" = 2"

submit_round "$run/propose.ops.json" "$propose_height" 1 "joe-pfusdc-propose-h$propose_height"
submit_round "$run/finalize-claim.ops.json" "$claim_height" 2 "joe-pfusdc-claim-h$claim_height"

ssh_v2 "set -euo pipefail
  '$node' status --data-dir /var/lib/postfiat/validator-2 --expect-height '$claim_height' > '$run/post-status.json'
  '$node' account-assets \
    --data-dir /var/lib/postfiat/validator-2 \
    --account '$holder' \
    --asset-id '$asset' > '$run/holder-after-claim.json'
  jq -e --argjson expected '$expected_holder_atoms' \
    '.assets|any(.balance == \$expected)' '$run/holder-after-claim.json' >/dev/null"

scp -q -i "$ssh_key" "root@$v2:$run/relay-bundle.report.json" \
  "$local_evidence/relay-bundle.report.json"
scp -q -i "$ssh_key" "root@$v2:$run/holder-after-claim.json" \
  "$local_evidence/holder-after-claim.json"
scp -q -i "$ssh_key" "root@$v2:$run/post-status.json" \
  "$local_evidence/post-status.json"

jq -n \
  --argjson start_height "$start_height" \
  --argjson finalized_height "$claim_height" \
  --argjson amount_atoms "$deposit_atoms" \
  --arg deposit_tx "$deposit_tx" \
  '{schema:"postfiat.a666.pfusdc_relay.v1",verdict:"PASS",deposit_tx:$deposit_tx,start_height:$start_height,finalized_height:$finalized_height,amount_atoms:$amount_atoms}' \
  > "$local_evidence/summary.json"
cat "$local_evidence/summary.json"
