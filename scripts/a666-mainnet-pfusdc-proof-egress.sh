#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
phase_dir=
workflow_id=
release_id=${A666_PFTL_RELEASE_ID:-a666-variable-nav-9ffdfb6}
hosts_file=${A666_PROPOSER_HOSTS_FILE:-docs/evidence/a666-joe-mainnet-e2e-20260728/proposer-hosts.json}
holder_key=${A666_JOE_HOLDER_KEY:-/home/postfiat/tmp/pfusdc-closed-roundtrip-20260720/keys/holder.json}
issuer_key=${A666_PFUSDC_ISSUER_KEY:-/home/postfiat/tmp/navswap-ce22-venue-rebuild-20260719/private/pfusdc-issuer-key.json}
a100_host=${A666_A100_HOST:-}
a100_port=${A666_A100_PORT:-30886}
resume=false
amount_atoms=
bucket_id=
existing_burn_tx_id=
owner=pfab9b9228942e5c529633a13aa271d5297bec6353
recipient=0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0
prover_backend=${A666_PFUSDC_EGRESS_PROVER_BACKEND:-cuda}
pftl_rpc_ports=${A666_PFTL_RPC_PORTS:-28650,28651,28652,28653,28654,28655}

while (($#)); do
  case "$1" in
    --phase-dir) phase_dir=$2; shift 2 ;;
    --workflow-id) workflow_id=$2; shift 2 ;;
    --amount-atoms) amount_atoms=$2; shift 2 ;;
    --bucket-id) bucket_id=${2,,}; shift 2 ;;
    --existing-burn-tx-id) existing_burn_tx_id=${2,,}; shift 2 ;;
    --owner) owner=${2,,}; shift 2 ;;
    --recipient) recipient=$2; shift 2 ;;
    --prover-backend) prover_backend=${2,,}; shift 2 ;;
    --resume) resume=true; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

for value in "$phase_dir" "$workflow_id" "$amount_atoms"; do
  test -n "$value"
done
[[ "$workflow_id" =~ ^[a-z0-9][a-z0-9-]{0,39}$ ]]
[[ "$amount_atoms" =~ ^[1-9][0-9]*$ ]]
[[ "$owner" =~ ^pf[0-9a-f]{40}$ ]]
[[ "$recipient" =~ ^0x[0-9a-fA-F]{40}$ ]]
[[ "$prover_backend" =~ ^(cpu|cuda)$ ]]
[[ "$pftl_rpc_ports" =~ ^[1-9][0-9]{0,4}(,[1-9][0-9]{0,4}){5}$ ]]
IFS=, read -r -a pftl_rpc_port_values <<< "$pftl_rpc_ports"
declare -A seen_pftl_rpc_ports=()
for rpc_port in "${pftl_rpc_port_values[@]}"; do
  ((rpc_port >= 1024 && rpc_port <= 65535))
  test -z "${seen_pftl_rpc_ports[$rpc_port]+present}"
  seen_pftl_rpc_ports[$rpc_port]=1
done
if test -n "$existing_burn_tx_id"; then
  [[ "$existing_burn_tx_id" =~ ^[0-9a-f]{96}$ ]]
fi
if test -n "$bucket_id"; then
  [[ "$bucket_id" =~ ^[0-9a-f]{96}$ ]]
fi

cd "$repo"
phase_dir=$(realpath "$phase_dir")
hosts_file=$(realpath "$hosts_file")
if test -z "$existing_burn_tx_id"; then holder_key=$(realpath "$holder_key"); fi
issuer_key=$(realpath "$issuer_key")
egress_dir="$phase_dir/pfusdc-egress"
remote_node="/opt/postfiat/releases/$release_id/postfiat-node"
remote_topology=${A666_PFTL_TOPOLOGY_PATH:-/etc/postfiat/releases/$release_id/topology.json}
local_node=${A666_LOCAL_NODE_BIN:-target/release/postfiat-node}
remote_root="/var/lib/postfiat/validator-2/$workflow_id-pfusdc-egress"
a100_root="/workspace/a666-acceptance/live/$workflow_id-pfusdc-egress"
a100_prover=${A666_PFUSDC_EGRESS_PROVER_BIN:-/workspace/a666-acceptance/live/a666-epoch5-transparent-20260728t1505z/pfusdc-egress/pfusdc-tier4-prover-cuda}
local_prover=${A666_PFUSDC_EGRESS_LOCAL_PROVER_BIN:-$repo/tools/pfusdc-tier4-prover/target/release/pfusdc-tier4-prover}
egress_elf=${A666_PFUSDC_EGRESS_ELF:-$repo/programs/pfusdc-egress/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/pfusdc-egress-program}
expected_a100_prover_sha256=${A666_PFUSDC_EGRESS_PROVER_SHA256:-}
validator2_host=$(jq -er '."validator-2"' "$hosts_file")
pfusdc_issuer=pf23d8831301aa1cce6fdd7bf4a2db2aead1619ba8
pfusdc=02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b
verifier=${A666_PFUSDC_VERIFIER_ADDRESS:-0x9a45D6F1DC9da443a88b1c336B3188fa7924d1ae}
program_vkey=${A666_PFUSDC_EGRESS_PROGRAM_VKEY:-0x0026a156bfd82ce1d1bf3f966c77daba8d5c266b8cc29928474747c4a02ca89b}
manifest=${A666_PFUSDC_DEPLOYMENT_MANIFEST:-docs/evidence/a666-acceptance-20260728/phase-5-transparent-redeem-verify/pfusdc-egress/recovery-epoch5/deploy/manifest.postdeploy-enriched.json}
manifest_sha256=${A666_PFUSDC_DEPLOYMENT_MANIFEST_SHA256:-b69417647e6a4bed5a3e7fa5069a0844b80a63f78020ba34f4796e373e92e904}
stakehub_repo=${A666_STAKEHUB_REPO:-/home/postfiat/repos/StakeHub-master-e6}
contract_artifact_root=${A666_CONTRACT_ARTIFACT_ROOT:-$repo}
ethereum_rpc=${A666_ETHEREUM_RPC:-https://ethereum-rpc.publicnode.com}
cast_bin=${A666_CAST_BIN:-/home/postfiat/.foundry/bin/cast}

test -s "$hosts_file"
test -x "$cast_bin"
if test -z "$existing_burn_tx_id"; then test -s "$holder_key"; fi
test -s "$issuer_key"
test -s "$manifest"
if ! "$resume"; then
  test ! -e "$egress_dir"
  ssh -o BatchMode=yes "root@$validator2_host" "test ! -e '$remote_root'"
  if test "$prover_backend" = cuda; then
    test -n "$a100_host"
    ssh -o BatchMode=yes -p "$a100_port" "root@$a100_host" "test ! -e '$a100_root'"
  fi
fi
mkdir -p "$egress_dir"

round_args=(
  --node-bin "$local_node"
  --remote-runner scripts/a666-remote-sync-round.py
  --proposer-hosts-file "$hosts_file"
  --remote-binary "$remote_node"
  --remote-topology "$remote_topology"
  --ports "$pftl_rpc_ports"
)

if test -n "$existing_burn_tx_id"; then
  ssh -o BatchMode=yes "root@$validator2_host" \
    "$remote_node vault-bridge-status --data-dir /var/lib/postfiat/validator-2 --asset-id '$pfusdc'" \
    > "$egress_dir/vault-bridge-status-after-burn.json"
  redemption_row=$(jq -ce \
    --arg tx "$existing_burn_tx_id" --arg owner "$owner" --arg recipient "${recipient,,}" --argjson amount "$amount_atoms" \
    '[.redemptions[] | select(.burn_tx_id==$tx and .owner==$owner and .amount_atoms==$amount and (.withdrawal_recipient|ascii_downcase)==$recipient)]
     | if length==1 and (.[0].state=="pending" or .[0].state=="settled") then .[0] else error("signed burn is missing, ambiguous, or bound to different withdrawal terms") end' \
    "$egress_dir/vault-bridge-status-after-burn.json")
  redemption_id=$(jq -er '.redemption_id' <<<"$redemption_row")
  burn_height=$(jq -er '.created_at_height' <<<"$redemption_row")
elif ! test -s "$egress_dir/burn-finality/summary.json"; then
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
         --owner '$owner' \
         --asset-id '$pfusdc' \
         --amount-atoms '$amount_atoms' \
         --bucket-id '$bucket_id' \
         --destination-ref 'evm-erc20:1:${recipient,,}' \
         --bundle '$remote_root/burn-bundle' \
         > '$remote_root/burn-bundle-report.json'"
    scp -q "root@$validator2_host:$remote_root/burn-bundle-report.json" \
      "$egress_dir/burn-bundle-report.json"
    scp -q \
      "root@$validator2_host:$remote_root/burn-bundle/burn-to-redeem.operation.json" \
      "$egress_dir/burn-to-redeem.operation.json"
    jq -n \
      --arg label "$workflow_id-pfusdc-burn" \
      --arg source "$owner" \
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
if test -z "$existing_burn_tx_id"; then
  jq -e '.confirmed==true and .accepted==true' "$egress_dir/burn-finality/summary.json" >/dev/null
  burn_height=$(jq -er '.end_height' "$egress_dir/burn-finality/summary.json")
fi

verifier_height=$("$cast_bin" call "$verifier" \
  'latestFinalizedHeight()(uint64)' --rpc-url "$ethereum_rpc")
[[ "$verifier_height" =~ ^[0-9]+$ ]]
current_prior_file="$egress_dir/current-prior-checkpoint-block.json"
ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node blocks \
    --data-dir /var/lib/postfiat/validator-2 \
    --from-height '$verifier_height' \
    --limit 1" \
  > "$current_prior_file"
current_prior_checkpoint=$(jq -er \
  --argjson height "$verifier_height" \
  'if length==1 and .[0].header.height==$height
   then .[0].header.block_hash
   else error("missing current verifier checkpoint block")
   end' "$current_prior_file")
if test -s "$egress_dir/witness.json" && ! test -s "$egress_dir/withdrawal-result.json"; then
  witness_prior_checkpoint=$(jq -er '.prior_checkpoint_block_id' "$egress_dir/witness.json")
  witness_target_height=$(jq -er '.block.header.height' "$egress_dir/witness.json")
  test "$witness_target_height" -gt "$verifier_height"
  if test "$witness_prior_checkpoint" != "$current_prior_checkpoint"; then
    stale_dir="$egress_dir/stale-prior-checkpoint-$verifier_height-$(date +%s)"
    mkdir -m 700 "$stale_dir"
    for stale in witness.json prior-checkpoint-block.json proof-cpu proof-cuda; do
      if test -e "$egress_dir/$stale"; then mv "$egress_dir/$stale" "$stale_dir/"; fi
    done
  fi
fi

if ! test -s "$egress_dir/witness.json"; then
  if ! test -s "$egress_dir/vault-bridge-status-after-burn.json"; then
    ssh -o BatchMode=yes "root@$validator2_host" \
      "$remote_node vault-bridge-status --data-dir /var/lib/postfiat/validator-2 --asset-id '$pfusdc'" \
      > "$egress_dir/vault-bridge-status-after-burn.json"
  fi
  if test -z "${redemption_id:-}"; then
    redemption_id=$(jq -er --arg owner "$owner" --argjson height "$burn_height" --argjson amount "$amount_atoms" \
      '[.redemptions[] | select(.owner==$owner and .amount_atoms==$amount and .created_at_height==$height and .state=="pending")]
       | if length==1 then .[0].redemption_id else error("ambiguous redemption") end' \
      "$egress_dir/vault-bridge-status-after-burn.json")
  fi

  test "$verifier_height" -lt "$burn_height"
  cp "$current_prior_file" "$egress_dir/prior-checkpoint-block.json"
  prior_checkpoint=$current_prior_checkpoint

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

proof_dir="$egress_dir/proof-$prover_backend"
if ! test -s "$proof_dir/proof-report.json"; then
  if test "$prover_backend" = cpu; then
    test -x "$local_prover"
    test -s "$egress_elf"
    mkdir -p "$proof_dir"
    SP1_PROVER=cpu "$local_prover" egress --elf "$egress_elf" --witness "$egress_dir/witness.json" --output-dir "$proof_dir" --prove
  else
  ssh -o BatchMode=yes -p "$a100_port" "root@$a100_host" \
    "test ! -e '$a100_root'; install -d -m 700 '$a100_root'"
  scp -q -P "$a100_port" "$egress_dir/witness.json" \
    "root@$a100_host:$a100_root/witness.json"
  ssh -o BatchMode=yes -p "$a100_port" "root@$a100_host" \
    "SP1_PROVER=cuda '$a100_prover' egress \
      --witness '$a100_root/witness.json' \
      --output-dir '$a100_root/proof' \
      --prove"
  mkdir -p "$proof_dir"
  rsync -a -e "ssh -p $a100_port" \
    "root@$a100_host:$a100_root/proof/" \
    "$proof_dir/"
  fi
fi
jq -e \
  --arg vkey "$program_vkey" \
  '.proof_mode=="groth16"
   and .program_vkey==$vkey
   and .proof_bytes==356
   and .public_values_bytes<=4096' \
  "$proof_dir/proof-report.json" >/dev/null

cuda_provenance="$proof_dir/proof-provenance.json"
if ! test -s "$cuda_provenance"; then
  if test "$prover_backend" = cpu; then
    remote_prover_sha256=$(sha256sum "$local_prover" | awk '{print $1}')
    gpu_identity="local 32-core CPU fallback"
    prover_binary="$local_prover"
  else
    remote_prover_sha256=$(ssh -o BatchMode=yes -p "$a100_port" "root@$a100_host" "sha256sum '$a100_prover' | cut -d' ' -f1")
    gpu_identity=$(ssh -o BatchMode=yes -p "$a100_port" "root@$a100_host" "nvidia-smi --query-gpu=name,driver_version --format=csv,noheader | head -n1")
    prover_binary="$a100_prover"
  fi
  [[ "$remote_prover_sha256" =~ ^[0-9a-f]{64}$ ]]
  if test "$prover_backend" = cuda && test -n "$expected_a100_prover_sha256"; then
    test "$remote_prover_sha256" = "$expected_a100_prover_sha256"
  fi
  test -n "$gpu_identity"
  proof_report_sha256=$(sha256sum "$proof_dir/proof-report.json" | awk '{print $1}')
  proof_calldata_sha256=$(sha256sum "$proof_dir/proof-calldata.bin" | awk '{print $1}')
  jq -n \
    --arg requested_backend "$prover_backend" \
    --arg prover_binary "$prover_binary" \
    --arg prover_binary_sha256 "$remote_prover_sha256" \
    --arg expected_prover_binary_sha256 "$expected_a100_prover_sha256" \
    --arg gpu_identity "$gpu_identity" \
    --arg proof_report_sha256 "$proof_report_sha256" \
    --arg proof_calldata_sha256 "$proof_calldata_sha256" \
    --arg program_vkey "$program_vkey" \
    '{
      schema:"postfiat.a666.pfusdc_cuda_proof_provenance.v1",
      verdict:"PASS",
      requested_backend:$requested_backend,
      prover_binary:$prover_binary,
      prover_binary_sha256:$prover_binary_sha256,
      expected_prover_binary_sha256:$expected_prover_binary_sha256,
      gpu_identity:$gpu_identity,
      proof_report_sha256:$proof_report_sha256,
      proof_calldata_sha256:$proof_calldata_sha256,
      program_vkey:$program_vkey
    }' > "$cuda_provenance"
fi
jq -e \
  --arg vkey "$program_vkey" \
  --arg expected_sha "$expected_a100_prover_sha256" \
  --arg backend "$prover_backend" \
  '.verdict=="PASS"
   and .requested_backend==$backend
   and .program_vkey==$vkey
   and (.prover_binary_sha256|test("^[0-9a-f]{64}$"))
   and ($backend=="cpu" or $expected_sha=="" or .prover_binary_sha256==$expected_sha)' \
  "$cuda_provenance" >/dev/null

if ! test -s "$egress_dir/withdrawal-result.json"; then
  python3 scripts/a666-mainnet-pfusdc-withdraw.py \
    --proof-dir "$proof_dir" \
    --output "$egress_dir/withdrawal-result.json" \
    --deployment-manifest "$manifest" \
    --expected-manifest-sha256 "$manifest_sha256" \
    --amount-atoms "$amount_atoms" \
    --recipient "$recipient" \
    --ethereum-rpc "$ethereum_rpc" \
    --stakehub-repo "$stakehub_repo" \
    --contract-artifact-root "$contract_artifact_root"
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
  ssh -o BatchMode=yes "root@$validator2_host" \
    "$remote_node vault-bridge-status --data-dir /var/lib/postfiat/validator-2 --asset-id '$pfusdc'" \
    > "$egress_dir/vault-bridge-status-before-settle.json"
  current_redemption_state=$(jq -er \
    --arg redemption_id "$redemption_id" --arg receipt "$settlement_receipt_hash" --argjson amount "$amount_atoms" \
    '[.redemptions[] | select(.redemption_id==$redemption_id)]
     | if length==1
       and .[0].amount_atoms==$amount
       and (.[0].state=="pending" or (.[0].state=="settled" and .[0].settled_atoms==$amount and .[0].settlement_receipt_hash==$receipt))
       then .[0].state
       else error("redemption state does not match this Ethereum payout")
       end' "$egress_dir/vault-bridge-status-before-settle.json")
  if test "$current_redemption_state" = settled; then
    recovery_dir="$egress_dir/settle-recovery"
    mkdir -p "$recovery_dir" "$egress_dir/settle-finality"
    for index in 0 1 2 3 4 5; do
      host=$(jq -er --arg validator "validator-$index" '.[$validator]' "$hosts_file")
      ssh -o BatchMode=yes "root@$host" \
        "$remote_node status --data-dir /var/lib/postfiat/validator-$index" \
        > "$recovery_dir/status-$index.json"
      ssh -o BatchMode=yes "root@$host" \
        "$remote_node vault-bridge-status --data-dir /var/lib/postfiat/validator-$index --asset-id '$pfusdc'" \
        > "$recovery_dir/vault-bridge-status-$index.json"
    done
    jq -s -e \
      'length==6
       and (map(.block_height)|unique|length)==1
       and (map(.block_tip_hash)|unique|length)==1
       and (map(.state_root)|unique|length)==1
       and all(.mempool_pending==0)' \
      "$recovery_dir"/status-*.json >/dev/null
    jq -s -e \
      --arg redemption_id "$redemption_id" --arg receipt "$settlement_receipt_hash" --argjson amount "$amount_atoms" \
      'length==6 and all(
        [.redemptions[] | select(.redemption_id==$redemption_id)]
        | length==1 and .[0].state=="settled" and .[0].settled_atoms==$amount
          and .[0].settlement_receipt_hash==$receipt)' \
      "$recovery_dir"/vault-bridge-status-*.json >/dev/null
    settled_height=$(jq -er '.block_height' "$recovery_dir/status-0.json")
    settled_state_root=$(jq -er '.state_root' "$recovery_dir/status-0.json")
    jq -n \
      --argjson height "$settled_height" --arg state_root "$settled_state_root" \
      '{
        schema:"postfiat-a666-ce22-remote-finality-operation-v1",
        accepted:true,
        confirmed:true,
        round_ok:true,
        transaction_kind:"vault_bridge_redeem_settle",
        validator_count:6,
        vote_count:6,
        end_height:$height,
        end_state_root:$state_root,
        end_mempool_pending:0,
        recovered_from_converged_state:true,
        trust_class:"CONTROLLED"
      }' > "$egress_dir/settle-finality/summary.json"
  else
  if test -d "$egress_dir/settle-finality"; then
    stale_settle_dir="$egress_dir/stale-settle-attempt-$(date +%s)"
    mv "$egress_dir/settle-finality" "$stale_settle_dir"
  fi
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
fi
jq -e '.confirmed==true and .accepted==true' \
  "$egress_dir/settle-finality/summary.json" >/dev/null

jq -n \
  --argjson burn_height "$burn_height" \
  --slurpfile proof "$proof_dir/proof-report.json" \
  --slurpfile provenance "$cuda_provenance" \
  --slurpfile withdrawal "$egress_dir/withdrawal-result.json" \
  --slurpfile settle "$egress_dir/settle-finality/summary.json" \
  '{
    schema:"postfiat.a666.pfusdc_proof_egress_acceptance.v1",
    verdict:"PASS",
    pftl_burn_height:$burn_height,
    proof:$proof[0],
    proof_provenance:$provenance[0],
    ethereum_withdrawal:{
      tx:$withdrawal[0].withdrawal_tx,
      amount_atoms:$withdrawal[0].amount_atoms,
      replay_rejected:$withdrawal[0].replay_rejected
    },
    pftl_accounting_settle_height:$settle[0].end_height,
    trust_boundary:"Ethereum payout is proof-gated and precedes PFTL issuer-signed accounting closure; settlement signature does not authorize or gate user funds"
  }' > "$egress_dir/summary.json"
