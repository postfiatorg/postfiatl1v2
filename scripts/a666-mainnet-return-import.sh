#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
phase_dir=
workflow_id=
expected_pftl_height=
release_id=${A666_PFTL_RELEASE_ID:-a666-variable-nav-9ffdfb6}
hosts_file=${A666_PROPOSER_HOSTS_FILE:-docs/evidence/a666-joe-mainnet-e2e-20260728/proposer-hosts.json}
operator_key=${A666_BRIDGE_OPERATOR_KEY:-/home/postfiat/tmp/navswap-ce22-venue-rebuild-20260719/private/reserve-key.json}
operator=${A666_BRIDGE_OPERATOR:-pfd0c86d9084915e1fefd22eab891806397d5a5937}
route_id=pftl-a666-ethereum-wA666-usdc-v1
controller=0x9A0262C0572fb4DB08765408eB225E207F40c3d9
wrapped_token=0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5
a666=521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c
return_burned_topic=0x4d6105cbfd6dce49c1a94770a1492db4e1f2b0670d8bb14fe8da318d880f2c01
ethereum_rpc=${A666_ETHEREUM_RPC:-https://ethereum-rpc.publicnode.com}
cast_bin=${A666_CAST_BIN:-cast}
finality_timeout_seconds=1800

while (($#)); do
  case "$1" in
    --phase-dir) phase_dir=$2; shift 2 ;;
    --workflow-id) workflow_id=$2; shift 2 ;;
    --expected-pftl-height) expected_pftl_height=$2; shift 2 ;;
    --finality-timeout-seconds) finality_timeout_seconds=$2; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

for value in "$phase_dir" "$workflow_id" "$expected_pftl_height"; do
  test -n "$value"
done
[[ "$workflow_id" =~ ^[a-z0-9][a-z0-9-]{0,39}$ ]]
[[ "$expected_pftl_height" =~ ^[0-9]+$ ]]
[[ "$finality_timeout_seconds" =~ ^[0-9]+$ ]]

cd "$repo"
phase_dir=$(realpath "$phase_dir")
hosts_file=$(realpath "$hosts_file")
burn_report="$phase_dir/return/ethereum-burn/burn.json"
proof_dir="$phase_dir/return/proof"
artifact_dir="$phase_dir/return/import-finality-h$expected_pftl_height"
remote_node="/opt/postfiat/releases/$release_id/postfiat-node"
remote_topology="/etc/postfiat/releases/$release_id/topology.json"
remote_root="/var/lib/postfiat/validator-2/$workflow_id-return-import"
rpc=http://127.0.0.1:28701

test -s "$hosts_file"
test -s "$operator_key"
test -s "$burn_report"
test ! -e "$proof_dir"
test ! -e "$artifact_dir"
mkdir -p "$proof_dir"

burn_tx=$(jq -er '.transaction.tx' "$burn_report")
burn_block=$(jq -er '.transaction.block_number' "$burn_report")
burn_event_hash=$(jq -er '.return_burn_id' "$burn_report")
ethereum_sender=$(jq -er '.ethereum_sender | ascii_downcase' "$burn_report")
pftl_recipient=$(jq -er '.pftl_recipient' "$burn_report")
amount_atoms=$(jq -er '.amount_atoms' "$burn_report")
return_nonce=$(jq -er '.return_nonce' "$burn_report")
log_index=$(jq -er '.event_log_index' "$burn_report")
[[ "$amount_atoms" =~ ^[1-9][0-9]*$ ]]
test "$log_index" = 1
[[ "$burn_tx" =~ ^0x[0-9a-fA-F]{64}$ ]]
[[ "$burn_event_hash" =~ ^[0-9a-f]{64}$ ]]
[[ "$return_nonce" =~ ^[0-9a-f]{64}$ ]]

"$cast_bin" receipt "$burn_tx" --json --rpc-url "$ethereum_rpc" \
  > "$proof_dir/burn-receipt.json"
jq -e \
  --arg controller "${controller,,}" \
  --arg topic "$return_burned_topic" \
  --argjson log_index "$log_index" \
  '.status=="0x1"
   and .logs[$log_index].address==$controller
   and .logs[$log_index].topics[0]==$topic' \
  "$proof_dir/burn-receipt.json" >/dev/null

validator2_host=$(jq -er '."validator-2"' "$hosts_file")
ssh -o BatchMode=yes "root@$validator2_host" \
  "test ! -e '$remote_root'; install -d -m 700 '$remote_root'"

deadline=$((SECONDS + finality_timeout_seconds))
while ! ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node ethereum-checkpoint-observe \
    --data-dir /var/lib/postfiat/validator-2 \
    --route-id '$route_id' \
    --ethereum-rpc '$rpc' \
    --block-number '$burn_block' \
    --checkpoint-file '$remote_root/checkpoint.json'" \
  > "$proof_dir/checkpoint-report.json" 2> "$proof_dir/checkpoint-observe.stderr"
do
  if ((SECONDS >= deadline)); then
    echo "timed out waiting for Ethereum checkpoint finality" >&2
    exit 1
  fi
  sleep 12
done
if ! test -s "$proof_dir/checkpoint-observe.stderr"; then
  printf '%s\n' "checkpoint observation completed without stderr" \
    > "$proof_dir/checkpoint-observe.stderr"
fi
scp -q "root@$validator2_host:$remote_root/checkpoint.json" "$proof_dir/checkpoint.json"

ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node ethereum-receipt-proof-build \
    --data-dir /var/lib/postfiat/validator-2 \
    --route-id '$route_id' \
    --ethereum-rpc '$rpc' \
    --transaction-hash '$burn_tx' \
    --proof-file '$remote_root/receipt-proof.json'" \
  > "$proof_dir/receipt-proof-report.json"
scp -q "root@$validator2_host:$remote_root/receipt-proof.json" "$proof_dir/receipt-proof.json"

python3 scripts/a666-parallel-checkpoint-votes.py \
  --hosts-file "$hosts_file" \
  --checkpoint-file "$proof_dir/checkpoint.json" \
  --proof-dir "$proof_dir" \
  --workflow-id "$workflow_id" \
  --remote-suffix return-import \
  --remote-node "$remote_node" \
  --ethereum-rpc "$rpc" \
  --validator2-remote-root "$remote_root" \
  > "$proof_dir/checkpoint-vote-fanout.json"
vote_files=$(python3 scripts/a666-checkpoint-vote-files.py \
  --fanout-file "$proof_dir/checkpoint-vote-fanout.json")
ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node ethereum-checkpoint-certificate-assemble \
    --data-dir /var/lib/postfiat/validator-2 \
    --checkpoint-file '$remote_root/checkpoint.json' \
    --vote-files '$vote_files' \
    --certificate-file '$remote_root/checkpoint-certificate.json'" \
  > "$proof_dir/checkpoint-certificate-report.json"
scp -q "root@$validator2_host:$remote_root/checkpoint-certificate.json" \
  "$proof_dir/checkpoint-certificate.json"

finalized_height=$(jq -er '.observed_head_number' "$proof_dir/checkpoint.json")
jq -n \
  --arg label "$workflow_id-return-import" \
  --arg source "$operator" \
  --arg key_file "$operator_key" \
  --arg route_id "$route_id" \
  --arg burn_event_hash "$burn_event_hash" \
  --arg controller "${controller,,}" \
  --arg wrapped_token "${wrapped_token,,}" \
  --arg a666 "$a666" \
  --arg ethereum_sender "$ethereum_sender" \
  --arg pftl_recipient "$pftl_recipient" \
  --arg return_nonce "$return_nonce" \
  --argjson amount_atoms "$amount_atoms" \
  --argjson burn_height "$burn_block" \
  --argjson finalized_height "$finalized_height" \
  --argjson log_index "$log_index" \
  --slurpfile certificate "$proof_dir/checkpoint-certificate.json" \
  --slurpfile receipt "$proof_dir/receipt-proof.json" \
  '{
    schema:"postfiat-certified-asset-ops-request-v1",
    operations:[{
      label:$label,
      source:$source,
      key_file:$key_file,
      operation:{
        operation:"pftl_uniswap_return_import",
        operator:$source,
        route_id:$route_id,
        burn_event_hash:$burn_event_hash,
        ethereum_chain_id:1,
        bridge_controller:$controller,
        wrapped_navcoin_token:$wrapped_token,
        native_nav_asset_id:$a666,
        ethereum_sender:$ethereum_sender,
        pftl_recipient:$pftl_recipient,
        amount_atoms:$amount_atoms,
        return_nonce:$return_nonce,
        burn_height:$burn_height,
        finalized_height:$finalized_height,
        external_event_proof:{
          checkpoint_certificate:$certificate[0],
          receipt_proof:$receipt[0].proof,
          log_index:$log_index
        }
      }
    }]
  }' > "$proof_dir/return-import.ops.json"

python3 scripts/a666-ce22-remote-finality-op.py \
  --node-bin target/release/postfiat-node \
  --remote-runner scripts/a666-remote-sync-round.py \
  --proposer-hosts-file "$hosts_file" \
  --remote-binary "$remote_node" \
  --remote-topology "$remote_topology" \
  --ops-file "$proof_dir/return-import.ops.json" \
  --artifact-dir "$artifact_dir"
jq -e --argjson height "$expected_pftl_height" \
  '.confirmed==true and .accepted==true and .end_height==$height' \
  "$artifact_dir/summary.json" >/dev/null

ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node account-assets \
    --data-dir /var/lib/postfiat/validator-2 \
    --account '$pftl_recipient' \
    --asset-id '$a666'" \
  > "$phase_dir/return/pftl-recipient-a666-after.json"
jq -e \
  --argjson amount "$amount_atoms" \
  '.assets|length==1 and .[0].balance >= $amount' \
  "$phase_dir/return/pftl-recipient-a666-after.json" >/dev/null

jq -n \
  --arg tx "$burn_tx" \
  --arg burn_event_hash "$burn_event_hash" \
  --argjson burn_block "$burn_block" \
  --argjson finalized_height "$finalized_height" \
  --argjson pftl_height "$expected_pftl_height" \
  '{
    schema:"postfiat.a666.return_import_acceptance.v1",
    verdict:"PASS",
    ethereum_burn_tx:$tx,
    burn_event_hash:$burn_event_hash,
    ethereum_burn_block:$burn_block,
    checkpoint_finalized_height:$finalized_height,
    pftl_height:$pftl_height
  }' > "$phase_dir/return/summary.json"
