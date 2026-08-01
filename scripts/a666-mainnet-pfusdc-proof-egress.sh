#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
phase_dir=
workflow_id=
release_id=${A666_PFTL_RELEASE_ID:-a666-variable-nav-9ffdfb6}
hosts_file=${A666_PROPOSER_HOSTS_FILE:-docs/evidence/a666-joe-mainnet-e2e-20260728/proposer-hosts.json}
holder_key=${A666_JOE_HOLDER_KEY:-/home/postfiat/tmp/pfusdc-closed-roundtrip-20260720/keys/holder.json}
issuer_key=${A666_PFUSDC_ISSUER_KEY:-/home/postfiat/tmp/navswap-ce22-venue-rebuild-20260719/private/pfusdc-issuer-key.json}
a100_host=${A666_A100_HOST:?A666_A100_HOST is required}
a100_port=${A666_A100_PORT:-30886}
resume=false
amount_atoms=
bucket_id=

while (($#)); do
  case "$1" in
    --phase-dir) phase_dir=$2; shift 2 ;;
    --workflow-id) workflow_id=$2; shift 2 ;;
    --amount-atoms) amount_atoms=$2; shift 2 ;;
    --bucket-id) bucket_id=${2,,}; shift 2 ;;
    --resume) resume=true; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

for value in "$phase_dir" "$workflow_id" "$amount_atoms"; do
  test -n "$value"
done
[[ "$workflow_id" =~ ^[a-z0-9][a-z0-9-]{0,39}$ ]]
[[ "$amount_atoms" =~ ^[1-9][0-9]*$ ]]
if test -n "$bucket_id"; then
  [[ "$bucket_id" =~ ^[0-9a-f]{96}$ ]]
fi

cd "$repo"
phase_dir=$(realpath "$phase_dir")
hosts_file=$(realpath "$hosts_file")
holder_key=$(realpath "$holder_key")
issuer_key=$(realpath "$issuer_key")
egress_dir="$phase_dir/pfusdc-egress"
remote_node="/opt/postfiat/releases/$release_id/postfiat-node"
remote_topology="/etc/postfiat/releases/$release_id/topology.json"
remote_root="/var/lib/postfiat/validator-2/$workflow_id-pfusdc-egress"
a100_root="/workspace/a666-acceptance/live/$workflow_id-pfusdc-egress"
a100_prover=/workspace/a666-acceptance/live/a666-epoch5-transparent-20260728t1505z/pfusdc-egress/pfusdc-tier4-prover-cuda
validator2_host=$(jq -er '."validator-2"' "$hosts_file")
joe=pfab9b9228942e5c529633a13aa271d5297bec6353
joe_evm=0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0
pfusdc_issuer=pf23d8831301aa1cce6fdd7bf4a2db2aead1619ba8
pfusdc=02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b
verifier=0x9a45D6F1DC9da443a88b1c336B3188fa7924d1ae
program_vkey=0x0026a156bfd82ce1d1bf3f966c77daba8d5c266b8cc29928474747c4a02ca89b
manifest=docs/evidence/a666-acceptance-20260728/phase-5-transparent-redeem-verify/pfusdc-egress/recovery-epoch5/deploy/manifest.postdeploy-enriched.json
manifest_sha256=b69417647e6a4bed5a3e7fa5069a0844b80a63f78020ba34f4796e373e92e904
ethereum_rpc=${A666_ETHEREUM_RPC:-https://ethereum-rpc.publicnode.com}

test -s "$hosts_file"
test -s "$holder_key"
test -s "$issuer_key"
test -s "$manifest"
if ! "$resume"; then
  test ! -e "$egress_dir"
  ssh -o BatchMode=yes "root@$validator2_host" "test ! -e '$remote_root'"
  ssh -o BatchMode=yes -p "$a100_port" "root@$a100_host" \
    "test ! -e '$a100_root'"
fi
mkdir -p "$egress_dir"

round_args=(
  --node-bin target/release/postfiat-node
  --remote-runner scripts/a666-remote-sync-round.py
  --proposer-hosts-file "$hosts_file"
  --remote-binary "$remote_node"
  --remote-topology "$remote_topology"
)

if ! test -s "$egress_dir/burn-finality/summary.json"; then
  if ! test -s "$egress_dir/burn.ops.json"; then
    ssh -o BatchMode=yes "root@$validator2_host" \
      "$remote_node vault-bridge-status \
        --data-dir /var/lib/postfiat/validator-2 \
        --asset-id '$pfusdc'" \
      > "$egress_dir/vault-bridge-status-before-burn.json"
    source_chain_id=$(jq -er '.route.route_profile.source_chain_id' "$manifest")
    vault_address=$(jq -er '.route.route_profile.vault_address | ascii_downcase' "$manifest")
    token_address=$(jq -er '.route.route_profile.token_address | ascii_downcase' "$manifest")
    route_profile_hash=$(jq -er '.route.route_profile_hash' "$manifest")
    source_domain="erc20_bridge_vault:$source_chain_id:$vault_address:$token_address"
    selected_bucket=$(jq -c \
      --arg source_domain "$source_domain" \
      --arg policy_hash "$route_profile_hash" \
      --arg requested_bucket "$bucket_id" \
      --argjson amount "$amount_atoms" \
      '[.buckets[]
        | select(.source_domain==$source_domain
          and .policy_hash==$policy_hash
          and .status=="active"
          and .outstanding_vault_bridge_atoms >= $amount)
        | select($requested_bucket=="" or .bucket_id==$requested_bucket)]
       | if length==1 then .[0]
         elif length==0 then error("no active deployment-matched bucket has sufficient outstanding backing")
         else error("multiple deployment-matched buckets remain ambiguous")
         end' "$egress_dir/vault-bridge-status-before-burn.json")
    bucket_id=$(jq -er '.bucket_id' <<<"$selected_bucket")
    jq -n \
      --arg selection_rule \
        "unique active bucket matching deployed route source_domain and route_profile_hash with sufficient outstanding backing" \
      --arg source_domain "$source_domain" \
      --arg route_profile_hash "$route_profile_hash" \
      --argjson amount_atoms "$amount_atoms" \
      --argjson bucket "$selected_bucket" \
      '{
        schema:"postfiat.a666.pfusdc_egress_bucket_selection.v1",
        selection_rule:$selection_rule,
        source_domain:$source_domain,
        route_profile_hash:$route_profile_hash,
        requested_amount_atoms:$amount_atoms,
        selected_bucket:$bucket
      }' > "$egress_dir/selected-bucket.json"
    ssh -o BatchMode=yes "root@$validator2_host" \
      "install -d -m 700 '$remote_root'; \
       '$remote_node' vault-bridge-burn-to-redeem-bundle \
         --data-dir /var/lib/postfiat/validator-2 \
         --owner '$joe' \
         --asset-id '$pfusdc' \
         --amount-atoms '$amount_atoms' \
         --bucket-id '$bucket_id' \
         --destination-ref 'evm-erc20:1:${joe_evm,,}' \
         --bundle '$remote_root/burn-bundle' \
         > '$remote_root/burn-bundle-report.json'"
    scp -q "root@$validator2_host:$remote_root/burn-bundle-report.json" \
      "$egress_dir/burn-bundle-report.json"
    scp -q \
      "root@$validator2_host:$remote_root/burn-bundle/burn-to-redeem.operation.json" \
      "$egress_dir/burn-to-redeem.operation.json"
    jq -n \
      --arg label "$workflow_id-pfusdc-burn" \
      --arg source "$joe" \
      --arg key_file "$holder_key" \
      --slurpfile operation "$egress_dir/burn-to-redeem.operation.json" \
      '{
        schema:"postfiat-certified-asset-ops-request-v1",
        operations:[{
          label:$label,
          source:$source,
          key_file:$key_file,
          operation:$operation[0]
        }]
      }' > "$egress_dir/burn.ops.json"
  fi
  python3 scripts/a666-ce22-remote-finality-op.py \
    --ops-file "$egress_dir/burn.ops.json" \
    --artifact-dir "$egress_dir/burn-finality" \
    "${round_args[@]}"
fi
jq -e '.confirmed==true and .accepted==true' \
  "$egress_dir/burn-finality/summary.json" >/dev/null
burn_height=$(jq -er '.end_height' "$egress_dir/burn-finality/summary.json")

if ! test -s "$egress_dir/witness.json"; then
  ssh -o BatchMode=yes "root@$validator2_host" \
    "$remote_node vault-bridge-status \
      --data-dir /var/lib/postfiat/validator-2 \
      --asset-id '$pfusdc'" \
    > "$egress_dir/vault-bridge-status-after-burn.json"
  redemption_id=$(jq -er \
    --arg owner "$joe" \
    --argjson height "$burn_height" \
    --argjson amount "$amount_atoms" \
    '[.redemptions[]
      | select(.owner==$owner
        and .amount_atoms==$amount
        and .created_at_height==$height
        and .state=="pending")]
     | if length==1 then .[0].redemption_id else error("ambiguous redemption") end' \
    "$egress_dir/vault-bridge-status-after-burn.json")

  verifier_height=$(cast call "$verifier" \
    'latestFinalizedHeight()(uint64)' --rpc-url "$ethereum_rpc")
  [[ "$verifier_height" =~ ^[0-9]+$ ]]
  test "$verifier_height" -lt "$burn_height"
  ssh -o BatchMode=yes "root@$validator2_host" \
    "$remote_node blocks \
      --data-dir /var/lib/postfiat/validator-2 \
      --from-height '$verifier_height' \
      --limit 1" \
    > "$egress_dir/prior-checkpoint-block.json"
  prior_checkpoint=$(jq -er \
    --argjson height "$verifier_height" \
    'if length==1 and .[0].header.height==$height
     then .[0].header.block_hash
     else error("missing verifier checkpoint block")
     end' "$egress_dir/prior-checkpoint-block.json")

  ssh -o BatchMode=yes "root@$validator2_host" \
    "$remote_node pfusdc-egress-witness \
      --data-dir /var/lib/postfiat/validator-2 \
      --withdrawal-id '$redemption_id' \
      --prior-checkpoint '$prior_checkpoint'" \
    > "$egress_dir/witness.json"
  jq -e \
    --arg redemption_id "$redemption_id" \
    --arg prior "$prior_checkpoint" \
    --argjson height "$burn_height" \
    --argjson amount "$amount_atoms" \
    '.schema=="postfiat.pfusdc.egress_proof_witness.v1"
     and .prior_checkpoint_block_id==$prior
     and .withdrawal_packet.withdrawal_id==$redemption_id
     and .block.header.height==$height
     and .withdrawal_packet.amount_atoms==$amount' \
    "$egress_dir/witness.json" >/dev/null
fi

if ! test -s "$egress_dir/proof-cuda/proof-report.json"; then
  ssh -o BatchMode=yes -p "$a100_port" "root@$a100_host" \
    "test ! -e '$a100_root'; install -d -m 700 '$a100_root'"
  scp -q -P "$a100_port" "$egress_dir/witness.json" \
    "root@$a100_host:$a100_root/witness.json"
  ssh -o BatchMode=yes -p "$a100_port" "root@$a100_host" \
    "SP1_PROVER=cuda '$a100_prover' egress \
      --witness '$a100_root/witness.json' \
      --output-dir '$a100_root/proof' \
      --prove"
  mkdir -p "$egress_dir/proof-cuda"
  rsync -a -e "ssh -p $a100_port" \
    "root@$a100_host:$a100_root/proof/" \
    "$egress_dir/proof-cuda/"
fi
jq -e \
  --arg vkey "$program_vkey" \
  '.proof_mode=="groth16"
   and .program_vkey==$vkey
   and .proof_bytes==356
   and .public_values_bytes<=4096' \
  "$egress_dir/proof-cuda/proof-report.json" >/dev/null

if ! test -s "$egress_dir/withdrawal-result.json"; then
  python3 scripts/a666-mainnet-pfusdc-withdraw.py \
    --proof-dir "$egress_dir/proof-cuda" \
    --output "$egress_dir/withdrawal-result.json" \
    --deployment-manifest "$manifest" \
    --expected-manifest-sha256 "$manifest_sha256" \
    --amount-atoms "$amount_atoms" \
    --recipient "$joe_evm"
fi
jq -e --argjson amount "$amount_atoms" \
  '.amount_atoms==$amount
   and .withdrawal_consumed==true
   and .burn_consumed==true
   and .proof_nullifier_consumed==true
   and .replay_rejected==true
   and (.recipient_balance_after-.recipient_balance_before)==$amount
   and (.vault_balance_before-.vault_balance_after)==$amount' \
  "$egress_dir/withdrawal-result.json" >/dev/null

if ! test -s "$egress_dir/settle-finality/summary.json"; then
  redemption_id=$(jq -er '.withdrawal_packet.withdrawal_id' "$egress_dir/witness.json")
  withdrawal_tx=$(jq -er '.withdrawal_tx | ltrimstr("0x")' \
    "$egress_dir/withdrawal-result.json")
  receipt_block_hash=$(jq -er '.receipt_block_hash | ltrimstr("0x")' \
    "$egress_dir/withdrawal-result.json")
  settlement_receipt_hash=$(printf '%s' \
    "postfiat.pfusdc.ethereum_settlement.v1:$withdrawal_tx:$receipt_block_hash:$amount_atoms" \
    | openssl dgst -sha3-384 -r | awk '{print $1}')
  jq -n \
    --arg label "$workflow_id-pfusdc-settle" \
    --arg source "$pfusdc_issuer" \
    --arg key_file "$issuer_key" \
    --arg asset_id "$pfusdc" \
    --arg redemption_id "$redemption_id" \
    --arg receipt_hash "$settlement_receipt_hash" \
    --argjson amount "$amount_atoms" \
    '{
      schema:"postfiat-certified-asset-ops-request-v1",
      operations:[{
        label:$label,
        source:$source,
        key_file:$key_file,
        operation:{
          operation:"vault_bridge_redeem_settle",
          issuer_or_redemption_account:$source,
          asset_id:$asset_id,
          redemption_id:$redemption_id,
          settlement_receipt_hash:$receipt_hash,
          settled_atoms:$amount
        }
      }]
    }' > "$egress_dir/settle.ops.json"
  python3 scripts/a666-ce22-remote-finality-op.py \
    --ops-file "$egress_dir/settle.ops.json" \
    --artifact-dir "$egress_dir/settle-finality" \
    "${round_args[@]}"
fi
jq -e '.confirmed==true and .accepted==true' \
  "$egress_dir/settle-finality/summary.json" >/dev/null

jq -n \
  --slurpfile burn "$egress_dir/burn-finality/summary.json" \
  --slurpfile proof "$egress_dir/proof-cuda/proof-report.json" \
  --slurpfile withdrawal "$egress_dir/withdrawal-result.json" \
  --slurpfile settle "$egress_dir/settle-finality/summary.json" \
  '{
    schema:"postfiat.a666.pfusdc_proof_egress_acceptance.v1",
    verdict:"PASS",
    pftl_burn_height:$burn[0].end_height,
    proof:$proof[0],
    ethereum_withdrawal:{
      tx:$withdrawal[0].withdrawal_tx,
      amount_atoms:$withdrawal[0].amount_atoms,
      replay_rejected:$withdrawal[0].replay_rejected
    },
    pftl_accounting_settle_height:$settle[0].end_height,
    trust_boundary:"Ethereum payout is proof-gated and precedes PFTL issuer-signed accounting closure; settlement signature does not authorize or gate user funds"
  }' > "$egress_dir/summary.json"
