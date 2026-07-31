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
mint_consumed_topic=0xe6744ee565256772cd16e05e0dcf10583d6039e18553fcafabd22a4c994137f7
ethereum_rpc=${A666_ETHEREUM_RPC:-https://ethereum-rpc.publicnode.com}
log_index=1
finality_timeout_seconds=1800
resume_after_finality=false
resume_after_vote_fanout=false

while (($#)); do
  case "$1" in
    --phase-dir) phase_dir=$2; shift 2 ;;
    --workflow-id) workflow_id=$2; shift 2 ;;
    --expected-pftl-height) expected_pftl_height=$2; shift 2 ;;
    --log-index) log_index=$2; shift 2 ;;
    --finality-timeout-seconds) finality_timeout_seconds=$2; shift 2 ;;
    --resume-after-finality) resume_after_finality=true; shift ;;
    --resume-after-vote-fanout) resume_after_vote_fanout=true; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

for value in "$phase_dir" "$workflow_id" "$expected_pftl_height"; do
  test -n "$value"
done
[[ "$workflow_id" =~ ^[a-z0-9][a-z0-9-]{0,63}$ ]]
[[ "$expected_pftl_height" =~ ^[0-9]+$ ]]
[[ "$log_index" =~ ^[0-9]+$ ]]
[[ "$finality_timeout_seconds" =~ ^[0-9]+$ ]]
if "$resume_after_finality" && "$resume_after_vote_fanout"; then
  echo "resume modes are mutually exclusive" >&2
  exit 2
fi

cd "$repo"
phase_dir=$(realpath "$phase_dir")
hosts_file=$(realpath "$hosts_file")
proof_dir="$phase_dir/destination-consume/proof"
artifact_dir="$phase_dir/destination-consume/finality-h$expected_pftl_height"
mint_state="$phase_dir/ethereum/mint-state.json"
supply_before="$phase_dir/pftl-supply-status-after.json"
manifest="$phase_dir/a666/ops/manifest.json"
remote_node="/opt/postfiat/releases/$release_id/postfiat-node"
remote_topology="/etc/postfiat/releases/$release_id/topology.json"
remote_root="/var/lib/postfiat/validator-2/$workflow_id-destination-consume"
rpc=http://127.0.0.1:28701

test -s "$hosts_file"
test -s "$operator_key"
test -s "$mint_state"
test -s "$supply_before"
test -s "$manifest"

mint_tx=$(jq -er '.transactions[] | select(.label=="consume finalized A666 mint packet") | .tx' "$mint_state")
mint_block=$(jq -er '.transactions[] | select(.label=="consume finalized A666 mint packet") | .block_number' "$mint_state")
mint_block_hex=$(printf '0x%x' "$mint_block")
packet_hash=$(jq -er '.packet_hash' "$manifest")
mint_amount=$(jq -er '.mint_amount_atoms' "$manifest")
[[ "$mint_amount" =~ ^[1-9][0-9]*$ ]]
[[ "$mint_tx" =~ ^0x[0-9a-fA-F]{64}$ ]]
[[ "$packet_hash" =~ ^[0-9a-f]{96}$ ]]

validator2_host=$(jq -er '."validator-2"' "$hosts_file")
if "$resume_after_finality"; then
  test -s "$proof_dir/checkpoint.json"
  test -s "$proof_dir/mint-receipt.json"
  test -s "$artifact_dir/summary.json"
  test -s "$phase_dir/destination-consume/pftl-supply-status-after.json"
  jq -e \
    --argjson expected_height "$expected_pftl_height" \
    '.accepted==true and .confirmed==true and .end_height==$expected_height' \
    "$artifact_dir/summary.json" >/dev/null
else
  if "$resume_after_vote_fanout"; then
    test -d "$proof_dir"
    test -s "$proof_dir/checkpoint.json"
    test -s "$proof_dir/mint-receipt.json"
    test -s "$proof_dir/receipt-proof.json"
    test -s "$proof_dir/checkpoint-vote-fanout.json"
    test ! -e "$proof_dir/checkpoint-certificate.json"
    test ! -e "$artifact_dir"
    test ! -e "$phase_dir/destination-consume/pftl-supply-status-after.json"
    ssh -o BatchMode=yes "root@$validator2_host" "test -d '$remote_root'"
  else
    test ! -e "$phase_dir/destination-consume"
    mkdir -p "$proof_dir"
    cast receipt "$mint_tx" --json --rpc-url "$ethereum_rpc" \
      > "$proof_dir/mint-receipt.json"
  fi
  jq -e \
    --arg controller "${controller,,}" \
    --arg topic "$mint_consumed_topic" \
    --argjson log_index "$log_index" \
    --arg tx "${mint_tx,,}" \
    --arg mint_block_hex "$mint_block_hex" \
    '(.transactionHash|ascii_downcase)==$tx
     and (.blockNumber|ascii_downcase)==$mint_block_hex
     and .status=="0x1"
     and .logs[$log_index].address==$controller
     and .logs[$log_index].topics[0]==$topic' \
    "$proof_dir/mint-receipt.json" >/dev/null

  if ! "$resume_after_vote_fanout"; then
    ssh -o BatchMode=yes "root@$validator2_host" \
      "test ! -e '$remote_root'; install -d -m 700 '$remote_root'"

    deadline=$((SECONDS + finality_timeout_seconds))
    while ! ssh -o BatchMode=yes "root@$validator2_host" \
      "$remote_node ethereum-checkpoint-observe \
        --data-dir /var/lib/postfiat/validator-2 \
        --route-id '$route_id' \
        --ethereum-rpc '$rpc' \
        --block-number '$mint_block' \
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
        --transaction-hash '$mint_tx' \
        --proof-file '$remote_root/receipt-proof.json'" \
      > "$proof_dir/receipt-proof-report.json"
    scp -q "root@$validator2_host:$remote_root/receipt-proof.json" "$proof_dir/receipt-proof.json"

    python3 scripts/a666-parallel-checkpoint-votes.py \
      --hosts-file "$hosts_file" \
      --checkpoint-file "$proof_dir/checkpoint.json" \
      --proof-dir "$proof_dir" \
      --workflow-id "$workflow_id" \
      --remote-suffix destination-consume \
      --remote-node "$remote_node" \
      --ethereum-rpc "$rpc" \
      --validator2-remote-root "$remote_root" \
      > "$proof_dir/checkpoint-vote-fanout.json"
  fi
  jq -e \
    --arg route_id "$route_id" \
    --arg tx "${mint_tx,,}" \
    --argjson mint_block "$mint_block" \
    '.route_id==$route_id
     and (.transaction_hash|ascii_downcase)==$tx
     and .block_number==$mint_block' \
    "$proof_dir/receipt-proof.json" >/dev/null
  jq -e \
    --arg route_id "$route_id" \
    --argjson mint_block "$mint_block" \
    '.route_id==$route_id
     and .ethereum_chain_id==1
     and .block_number==$mint_block
     and .observed_head_number>=.block_number' \
    "$proof_dir/checkpoint.json" >/dev/null
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
  --arg label "$workflow_id-destination-consume" \
  --arg source "$operator" \
  --arg key_file "$operator_key" \
  --arg route_id "$route_id" \
  --arg packet_hash "$packet_hash" \
  --arg mint_tx "${mint_tx#0x}" \
  --argjson mint_block "$mint_block" \
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
        operation:"pftl_uniswap_destination_consume",
        operator:$source,
        route_id:$route_id,
        packet_hash:$packet_hash,
        ethereum_consume_tx_hash:$mint_tx,
        consumed_height:$mint_block,
        finalized_height:$finalized_height,
        external_event_proof:{
          checkpoint_certificate:$certificate[0],
          receipt_proof:$receipt[0].proof,
          log_index:$log_index
        }
      }
    }]
  }' > "$proof_dir/destination-consume.ops.json"

python3 scripts/a666-ce22-remote-finality-op.py \
  --node-bin target/release/postfiat-node \
  --remote-runner scripts/a666-remote-sync-round.py \
  --proposer-hosts-file "$hosts_file" \
  --remote-binary "$remote_node" \
  --remote-topology "$remote_topology" \
  --ops-file "$proof_dir/destination-consume.ops.json" \
  --artifact-dir "$artifact_dir" \
  --postflight-seconds 120

ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node navcoin-bridge-supply-status \
    --data-dir /var/lib/postfiat/validator-2 \
    --route-id '$route_id'" \
  > "$phase_dir/destination-consume/pftl-supply-status-after.json"
fi
finalized_height=$(jq -er '.observed_head_number' "$proof_dir/checkpoint.json")
jq -e \
  --argjson mint_amount "$mint_amount" \
  --slurpfile before "$supply_before" \
  '.invariant_holds==true
   and .authorized_valid_supply_atoms==$before[0].authorized_valid_supply_atoms
   and .outstanding_bridge_claims_atoms==($before[0].outstanding_bridge_claims_atoms-$mint_amount)
   and .ethereum_spendable_supply_atoms==($before[0].ethereum_spendable_supply_atoms+$mint_amount)' \
  "$phase_dir/destination-consume/pftl-supply-status-after.json" >/dev/null

jq -n \
  --arg tx "$mint_tx" \
  --arg packet_hash "$packet_hash" \
  --argjson mint_block "$mint_block" \
  --argjson finalized_height "$finalized_height" \
  --argjson pftl_height "$expected_pftl_height" \
  '{
    schema:"postfiat.a666.destination_consume_acceptance.v1",
    verdict:"PASS",
    ethereum_mint_tx:$tx,
    packet_hash:$packet_hash,
    ethereum_mint_block:$mint_block,
    checkpoint_finalized_height:$finalized_height,
    pftl_height:$pftl_height
  }' > "$phase_dir/destination-consume/summary.json"
