#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
phase_dir=
workflow_id=
expected_pftl_height=
prior_checkpoint_block_id=
expected_verifier_height=
expected_wrapped_balance_before=
expected_wrapped_supply_before=
resume_after_ingress_proof=false
resume_after_ingress_deployment=false
resume_after_pfusdc_claim=false
resume_after_export_proof=false
resume_after_private_middle=false
allow_recovery_timing_exception=false
private_middle=false
a100_host=${A666_A100_HOST:?A666_A100_HOST is required}
a100_port=${A666_A100_PORT:-30886}
validator2_host=${A666_VALIDATOR2_HOST:?A666_VALIDATOR2_HOST is required}
release_id=${A666_PFTL_RELEASE_ID:-a666-variable-nav-9ffdfb6}
pfusdc_deployment_manifest=${A666_PFUSDC_DEPLOYMENT_MANIFEST:?A666_PFUSDC_DEPLOYMENT_MANIFEST is required}
pfusdc_deployment_manifest_sha256=${A666_PFUSDC_DEPLOYMENT_MANIFEST_SHA256:?A666_PFUSDC_DEPLOYMENT_MANIFEST_SHA256 is required}

while (($#)); do
  case "$1" in
    --phase-dir) phase_dir=$2; shift 2 ;;
    --workflow-id) workflow_id=$2; shift 2 ;;
    --expected-pftl-height) expected_pftl_height=$2; shift 2 ;;
    --prior-checkpoint-block-id) prior_checkpoint_block_id=$2; shift 2 ;;
    --expected-verifier-height) expected_verifier_height=$2; shift 2 ;;
    --expected-wrapped-balance-before) expected_wrapped_balance_before=$2; shift 2 ;;
    --expected-wrapped-supply-before) expected_wrapped_supply_before=$2; shift 2 ;;
    --resume-after-ingress-proof) resume_after_ingress_proof=true; shift ;;
    --resume-after-ingress-deployment) resume_after_ingress_deployment=true; shift ;;
    --resume-after-pfusdc-claim) resume_after_pfusdc_claim=true; shift ;;
    --resume-after-export-proof) resume_after_export_proof=true; shift ;;
    --resume-after-private-middle) resume_after_private_middle=true; shift ;;
    --allow-recovery-timing-exception) allow_recovery_timing_exception=true; shift ;;
    --private-middle) private_middle=true; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

for value in \
  "$phase_dir" \
  "$workflow_id" \
  "$expected_pftl_height" \
  "$prior_checkpoint_block_id" \
  "$expected_verifier_height" \
  "$expected_wrapped_balance_before" \
  "$expected_wrapped_supply_before"
do
  test -n "$value"
done
[[ "$workflow_id" =~ ^[a-z0-9][a-z0-9-]{0,63}$ ]]
[[ "$expected_pftl_height" =~ ^[0-9]+$ ]]
[[ "$expected_verifier_height" =~ ^[0-9]+$ ]]
[[ "$expected_wrapped_balance_before" =~ ^[0-9]+$ ]]
[[ "$expected_wrapped_supply_before" =~ ^[0-9]+$ ]]
[[ "$prior_checkpoint_block_id" =~ ^[0-9a-f]{96}$ ]]
resume_point_count=0
for resume_point in \
  "$resume_after_ingress_proof" \
  "$resume_after_ingress_deployment" \
  "$resume_after_pfusdc_claim" \
  "$resume_after_export_proof" \
  "$resume_after_private_middle"
do
  if "$resume_point"; then
    resume_point_count=$((resume_point_count + 1))
  fi
done
if test "$resume_point_count" -gt 1; then
  echo "choose exactly one recovery resume point" >&2
  exit 2
fi
if "$allow_recovery_timing_exception" \
  && ! "$resume_after_ingress_proof" \
  && ! "$resume_after_ingress_deployment" \
  && ! "$resume_after_pfusdc_claim" \
  && ! "$resume_after_export_proof" \
  && ! "$resume_after_private_middle"
then
  echo "a recovery timing exception requires an explicit resume point" >&2
  exit 2
fi
if "$resume_after_private_middle" && ! "$private_middle"; then
  echo "private-middle recovery requires --private-middle" >&2
  exit 2
fi
if "$resume_after_private_middle" && ! "$allow_recovery_timing_exception"; then
  echo "private-middle recovery requires an explicit timing exception" >&2
  exit 2
fi

cd "$repo"
phase_dir=$(realpath "$phase_dir")
pfusdc_deployment_manifest=$(realpath "$pfusdc_deployment_manifest")
deposit_file="$phase_dir/deposit/deposit-result.json"
ops_dir="$phase_dir/a666/ops"
test -s "$deposit_file"
test -s "$pfusdc_deployment_manifest"
test -s "$ops_dir/manifest.json"
for directory in ingress pftl export-proof ethereum; do
  mkdir -p "$phase_dir/$directory"
done

deposit_tx=$(jq -er '.deposit.tx_hash' "$deposit_file")
deposit_id=$(jq -er '.event.deposit_id | ltrimstr("0x")' "$deposit_file")
deposit_vault=$(jq -er '.vault | ascii_downcase' "$deposit_file")
deposit_manifest_sha256=$(jq -er '.manifest_sha256' "$deposit_file")
actual_manifest_sha256=$(sha256sum "$pfusdc_deployment_manifest" | awk '{print $1}')
test "$actual_manifest_sha256" = "$pfusdc_deployment_manifest_sha256"
test "$deposit_manifest_sha256" = "$pfusdc_deployment_manifest_sha256"
manifest_vault=$(jq -er '.route.vault_address | ascii_downcase' "$pfusdc_deployment_manifest")
claim_policy_hash=$(jq -er '.route.route_profile_hash' "$pfusdc_deployment_manifest")
[[ "$manifest_vault" =~ ^0x[0-9a-f]{40}$ ]]
[[ "$claim_policy_hash" =~ ^[0-9a-f]{96}$ ]]
test "$deposit_vault" = "$manifest_vault"
packet_hash=$(jq -er '.packet_hash' "$ops_dir/manifest.json")
packet_digest=$(jq -er '.ethereum_packet_digest' "$ops_dir/manifest.json")
mint_amount=$(jq -er '.mint_amount_atoms' "$ops_dir/manifest.json")
base_value_amount=$(jq -er '.base_value_atoms' "$ops_dir/manifest.json")
settlement_amount=$(jq -er '.settlement_value_atoms' "$ops_dir/manifest.json")
spread_amount=$(jq -er '.issue_spread_atoms' "$ops_dir/manifest.json")
[[ "$mint_amount" =~ ^[1-9][0-9]*$ ]]
[[ "$base_value_amount" =~ ^[1-9][0-9]*$ ]]
[[ "$settlement_amount" =~ ^[1-9][0-9]*$ ]]
[[ "$spread_amount" =~ ^[0-9]+$ ]]
test "$settlement_amount" -eq "$((base_value_amount + spread_amount))"
jq -e --argjson settlement "$settlement_amount" \
  '.verdict=="PASS" and .amount_atoms==$settlement' "$deposit_file" >/dev/null

run_manifest="$phase_dir/run-manifest.json"
if test -e "$run_manifest"; then
  jq -e \
    --arg workflow_id "$workflow_id" \
    --arg prior_checkpoint_block_id "$prior_checkpoint_block_id" \
    --argjson start_height "$expected_pftl_height" \
    --argjson expected_verifier_height "$expected_verifier_height" \
    --argjson expected_wrapped_balance_before "$expected_wrapped_balance_before" \
    --argjson expected_wrapped_supply_before "$expected_wrapped_supply_before" \
    '.workflow_id==$workflow_id
     and .start_height==$start_height
     and .expected_verifier_height==$expected_verifier_height
     and .prior_checkpoint_block_id==$prior_checkpoint_block_id
     and .expected_wrapped_balance_before==$expected_wrapped_balance_before
     and .expected_wrapped_supply_before==$expected_wrapped_supply_before' \
    "$run_manifest" >/dev/null
else
  jq -n \
    --arg workflow_id "$workflow_id" \
    --arg validator_release "$release_id" \
    --arg prior_checkpoint_block_id "$prior_checkpoint_block_id" \
    --arg packet_hash "$packet_hash" \
    --argjson start_height "$expected_pftl_height" \
    --argjson expected_verifier_height "$expected_verifier_height" \
    --argjson expected_wrapped_balance_before "$expected_wrapped_balance_before" \
    --argjson expected_wrapped_supply_before "$expected_wrapped_supply_before" \
    --argjson settlement_atoms "$settlement_amount" \
    --argjson mint_atoms "$mint_amount" \
    '{
      schema:"postfiat.a666.transparent_issue_run_manifest.v1",
      workflow_id:$workflow_id,
      validator_release:$validator_release,
      start_height:$start_height,
      expected_verifier_height:$expected_verifier_height,
      prior_checkpoint_block_id:$prior_checkpoint_block_id,
      expected_wrapped_balance_before:$expected_wrapped_balance_before,
      expected_wrapped_supply_before:$expected_wrapped_supply_before,
      amounts:{settlement_atoms:$settlement_atoms,mint_atoms:$mint_atoms},
      packet_hash:$packet_hash
    }' > "$run_manifest"
fi

remote_node=/opt/postfiat/releases/$release_id/postfiat-node
remote_topology=${A666_PFTL_TOPOLOGY_PATH:-/etc/postfiat/releases/$release_id/topology.json}
local_node=${A666_LOCAL_NODE_BIN:-target/release/postfiat-node}
remote_run="/var/lib/postfiat/validator-2/$workflow_id"
a100_root="/workspace/a666-acceptance/live/$workflow_id"
ingress_prover=/workspace/a666-acceptance/bin/eth-l1-mainnet-fast-lane-p0-cuda-optimized
export_prover=/workspace/a666-acceptance/bin/pftl-uniswap-prover-cuda-optimized-20260729
export_elf=/workspace/a666-acceptance/witness/deployed-program-004e44.elf
joe=pfab9b9228942e5c529633a13aa271d5297bec6353
pfusdc=02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b
a666=521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c
ethereum_rpc=https://ethereum-rpc.publicnode.com
wa666=0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5
joe_evm=0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0
uniswap_state_view=0x7fFE42C4a5DEeA5b0feC41C94C136Cf115597227
uniswap_pool_id=0xc5f1e4b5bb07c0718eddcc3d102dc751b8953ec25bb05cdc14d95419d4d16e98

if "$resume_after_export_proof"; then
  test -s "$phase_dir/a666/03-export-round/summary.json"
  test -s "$phase_dir/a666/joe-pfusdc-before.json"
  test -s "$phase_dir/a666/joe-a666-before.json"
  test -s "$phase_dir/pftl-supply-status-after.json"
  test -s "$phase_dir/export-proof/receipt-witness.json"
  test -s "$phase_dir/export-proof/proof-cuda/proof-report.json"
  jq -e \
    --argjson start "$((expected_pftl_height + 5))" \
    --argjson end "$((expected_pftl_height + 6))" \
    '.accepted==true and .confirmed==true
     and .start_height==$start and .end_height==$end
     and .transaction_kind=="pftl_uniswap_export_debit"' \
    "$phase_dir/a666/03-export-round/summary.json" >/dev/null
  ssh -o BatchMode=yes "root@$validator2_host" \
    "$remote_node status --data-dir /var/lib/postfiat/validator-2 --expect-height $((expected_pftl_height + 6))" \
    > "$phase_dir/pftl/resume-after-export-proof-status.json"
  pfusdc_balance_before=$(jq -er '[.assets[]?.balance] | add // 0' \
    "$phase_dir/a666/joe-pfusdc-before.json")
  a666_balance_before=$(jq -er '[.assets[]?.balance] | add // 0' \
    "$phase_dir/a666/joe-a666-before.json")
  export_height=$((expected_pftl_height + 6))
else
if "$resume_after_private_middle"; then
  test -s "$phase_dir/orchard-private-issue/summary.json"
  jq -e \
    --argjson start "$((expected_pftl_height + 2))" \
    --argjson end "$((expected_pftl_height + 6))" \
    '.verdict=="PASS" and .start_height==$start and .end_height==$end' \
    "$phase_dir/orchard-private-issue/summary.json" >/dev/null
  test -s "$phase_dir/a666/joe-pfusdc-before.json"
  test -s "$phase_dir/a666/joe-a666-before.json"
  pfusdc_balance_before=$(jq -er '[.assets[]?.balance] | add // 0' \
    "$phase_dir/a666/joe-pfusdc-before.json")
  a666_balance_before=$(jq -er '[.assets[]?.balance] | add // 0' \
    "$phase_dir/a666/joe-a666-before.json")
  export_height=$((expected_pftl_height + 6))
else
if "$resume_after_pfusdc_claim"; then
  test -s "$phase_dir/pftl/summary.json"
  test -s "$phase_dir/pftl/status-before.json"
  test -s "$phase_dir/pftl-supply-status-before.json"
  test -s "$phase_dir/a666/joe-pfusdc-before.json"
  test -s "$phase_dir/a666/joe-a666-before.json"
  jq -e \
    --argjson start "$expected_pftl_height" \
    --argjson end "$((expected_pftl_height + 3))" \
    --argjson amount "$settlement_amount" \
    '.verdict=="PASS"
     and .start_height==$start
     and .finalized_height==$end
     and .amount_atoms==$amount' \
    "$phase_dir/pftl/summary.json" >/dev/null
  ssh -o BatchMode=yes "root@$validator2_host" \
    "$remote_node status --data-dir /var/lib/postfiat/validator-2 --expect-height $((expected_pftl_height + 3))" \
    > "$phase_dir/pftl/resume-after-claim-status.json"
  pfusdc_balance_before=$(jq -er '[.assets[]?.balance] | add // 0' \
    "$phase_dir/a666/joe-pfusdc-before.json")
  a666_balance_before=$(jq -er '[.assets[]?.balance] | add // 0' \
    "$phase_dir/a666/joe-a666-before.json")
  ssh -o BatchMode=yes "root@$validator2_host" \
    "$remote_node account-assets --data-dir /var/lib/postfiat/validator-2 --account $joe --asset-id $pfusdc" \
    > "$phase_dir/pftl/resume-holder-after-claim.json"
  jq -e \
    --argjson expected "$((pfusdc_balance_before + settlement_amount))" \
    '([.assets[]?.balance] | add // 0)==$expected' \
    "$phase_dir/pftl/resume-holder-after-claim.json" >/dev/null
else
ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node status --data-dir /var/lib/postfiat/validator-2 --expect-height $expected_pftl_height" \
  > "$phase_dir/pftl/status-before.json"
ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node navcoin-bridge-supply-status \
    --data-dir /var/lib/postfiat/validator-2 \
    --route-id pftl-a666-ethereum-wA666-usdc-v1" \
  > "$phase_dir/pftl-supply-status-before.json"
ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node account-assets --data-dir /var/lib/postfiat/validator-2 --account $joe --asset-id $pfusdc" \
  > "$phase_dir/a666/joe-pfusdc-before.json"
ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node account-assets --data-dir /var/lib/postfiat/validator-2 --account $joe --asset-id $a666" \
  > "$phase_dir/a666/joe-a666-before.json"
pfusdc_balance_before=$(jq -er '[.assets[]?.balance] | add // 0' \
  "$phase_dir/a666/joe-pfusdc-before.json")
a666_balance_before=$(jq -er '[.assets[]?.balance] | add // 0' \
  "$phase_dir/a666/joe-a666-before.json")
expected_holder_after_claim=$((pfusdc_balance_before + settlement_amount))
uniswap_liquidity_before=$(cast call "$uniswap_state_view" \
  'getLiquidity(bytes32)(uint128)' "$uniswap_pool_id" \
  --rpc-url "$ethereum_rpc" | awk '{print $1}')
[[ "$uniswap_liquidity_before" =~ ^[0-9]+$ ]]
test "$uniswap_liquidity_before" -gt 0
jq -n \
  --arg pool_id "$uniswap_pool_id" \
  --argjson liquidity "$uniswap_liquidity_before" \
  '{schema:"postfiat.a666.uniswap_liquidity_observation.v1",
    pool_id:$pool_id,liquidity_atoms:$liquidity}' \
  > "$phase_dir/ethereum/uniswap-liquidity-before.json"

jq '{
  vault,
  deposit_tx:.deposit.tx_hash,
  amount_atoms,
  recipient:.pftl_recipient,
  route_binding:(.route_binding|ltrimstr("0x")),
  nonce:(.nonce|ltrimstr("0x")),
  creation_bytecode_hash:"0xc02403a4d05a2b4400d21b360e5787ad560c1fccd293c1ad937840f986fdcd38"
}' "$deposit_file" > "$phase_dir/ingress/capture-deployment.json"

if "$resume_after_ingress_proof"; then
  test -s "$phase_dir/ingress/witness.public-values.json"
  test -s "$phase_dir/ingress/proof-cuda/proof-report.json"
else
  if "$resume_after_ingress_deployment"; then
    local_deployment_sha=$(sha256sum "$phase_dir/ingress/capture-deployment.json" | awk '{print $1}')
    remote_deployment_sha=$(ssh -o BatchMode=yes -p "$a100_port" "root@$a100_host" \
      "set -euo pipefail
       test \"\$(find '$a100_root' -type f | wc -l)\" = 1
       test -s '$a100_root/ingress/deployment.json'
       sha256sum '$a100_root/ingress/deployment.json'" | awk '{print $1}')
    test "$remote_deployment_sha" = "$local_deployment_sha"
  else
    ssh -o BatchMode=yes -p "$a100_port" "root@$a100_host" \
      "test ! -e '$a100_root'; install -d -m 700 '$a100_root/ingress'"
    scp -q -P "$a100_port" "$phase_dir/ingress/capture-deployment.json" \
      "root@$a100_host:$a100_root/ingress/deployment.json"
  fi
  ssh -o BatchMode=yes -p "$a100_port" "root@$a100_host" \
    "/workspace/a666-acceptance/bin/eth-l1-mainnet-fast-lane-p0-cuda capture \
      --deployment '$a100_root/ingress/deployment.json' \
      --output '$a100_root/ingress/witness.json' \
      --wait-seconds 1800"

  scp -q -P "$a100_port" "root@$a100_host:$a100_root/ingress/witness.json" \
    "$phase_dir/ingress/witness.json"
  scp -q -P "$a100_port" \
    "root@$a100_host:$a100_root/ingress/witness.public-values.json" \
    "$phase_dir/ingress/witness.public-values.json"

  ssh -o BatchMode=yes -p "$a100_port" "root@$a100_host" \
    "SP1_PROVER=cuda '$ingress_prover' prove \
      --witness '$a100_root/ingress/witness.json' \
      --output-dir '$a100_root/ingress-proof' \
      --require-prover cuda \
      --skip-redundant-execute"
  mkdir -p "$phase_dir/ingress/proof-cuda"
  rsync -a -e "ssh -p $a100_port" \
    "root@$a100_host:$a100_root/ingress-proof/" \
    "$phase_dir/ingress/proof-cuda/"
fi
jq -e \
  --arg deposit_id "$deposit_id" \
  --argjson settlement "$settlement_amount" \
  '.deposit_id==$deposit_id and .amount_atoms==$settlement' \
  "$phase_dir/ingress/witness.public-values.json" >/dev/null
jq -e '
  .program_vkey=="0x00a9f8f037da18dd1aa5a7b0f478df0c7c9fae411ee62b339baf48dc2505076e"
  and .prover_backend=="cuda"
  and .host_execute_skipped==true
  and .execute_ms==0
  and .proof_bytes==356
' "$phase_dir/ingress/proof-cuda/proof-report.json" >/dev/null

if "$resume_after_ingress_proof"; then
  ssh -o BatchMode=yes "root@$validator2_host" \
    "if test -e '$remote_run'; then
       test -d '$remote_run'
     else
       install -d -o root -g root -m 700 '$remote_run'
     fi
     install -d -o root -g root -m 700 '$remote_run/ingress-proof'"
else
  ssh -o BatchMode=yes "root@$validator2_host" \
    "test ! -e '$remote_run'; install -d -o root -g root -m 700 '$remote_run/ingress-proof'"
fi
rsync -a "$phase_dir/ingress/proof-cuda/" \
  "root@$validator2_host:$remote_run/ingress-proof/"
DEPOSIT_TX="$deposit_tx" \
DEPOSIT_ATOMS="$settlement_amount" \
EXPECTED_HOLDER_ATOMS="$expected_holder_after_claim" \
PFTL_NODE_BIN="$remote_node" \
PFTL_TOPOLOGY="$remote_topology" \
PFTL_POLICY_HASH="$claim_policy_hash" \
PFTL_LABEL_SUFFIX=$(if "$resume_after_ingress_proof" || "$resume_after_ingress_deployment"; then printf '%s' -retry1; fi) \
PFTL_VAULT_ADDRESS="$manifest_vault" \
PFTL_RUN_DIR="$remote_run" \
PFTL_PROOF_DIR="$remote_run/ingress-proof" \
PFTL_LOCAL_EVIDENCE="$phase_dir/pftl" \
bash "$repo/scripts/a666-mainnet-pfusdc-relay.sh"
fi

if "$private_middle"; then
  bash scripts/a666-mainnet-private-issue-middle.sh \
    --phase-dir "$phase_dir" \
    --workflow-id "$workflow_id" \
    --expected-pftl-height "$((expected_pftl_height + 2))"
  export_height=$((expected_pftl_height + 6))
else
  round_args=(
    --node-bin "$local_node"
    --remote-runner scripts/a666-remote-sync-round.py
    --proposer-hosts-file docs/evidence/a666-joe-mainnet-e2e-20260728/proposer-hosts.json
    --remote-binary "$remote_node"
    --remote-topology "$remote_topology"
  )
  python3 scripts/a666-ce22-remote-finality-op.py \
    --ops-file "$ops_dir/01-reserve.ops.json" \
    --artifact-dir "$phase_dir/a666/01-reserve-round" \
    "${round_args[@]}"
  python3 scripts/a666-ce22-remote-finality-op.py \
    --ops-file "$ops_dir/02-subscribe.ops.json" \
    --artifact-dir "$phase_dir/a666/02-subscribe-round" \
    "${round_args[@]}"

  ssh -o BatchMode=yes "root@$validator2_host" \
    "$remote_node account-assets --data-dir /var/lib/postfiat/validator-2 --account $joe --asset-id $pfusdc" \
    > "$phase_dir/a666/joe-pfusdc-after-subscribe.json"
  ssh -o BatchMode=yes "root@$validator2_host" \
    "$remote_node account-assets --data-dir /var/lib/postfiat/validator-2 --account $joe --asset-id $a666" \
    > "$phase_dir/a666/joe-a666-after-subscribe.json"
  jq -e --argjson expected "$pfusdc_balance_before" \
    '([.assets[]?.balance] | add // 0)==$expected' \
    "$phase_dir/a666/joe-pfusdc-after-subscribe.json" >/dev/null
  jq -e \
    --argjson before "$a666_balance_before" \
    --argjson minted "$mint_amount" \
    '([.assets[]?.balance] | add // 0)==($before+$minted)' \
    "$phase_dir/a666/joe-a666-after-subscribe.json" >/dev/null

  python3 scripts/a666-ce22-remote-finality-op.py \
    --ops-file "$ops_dir/03-export.ops.json" \
    --artifact-dir "$phase_dir/a666/03-export-round" \
    "${round_args[@]}"
  # The ingress relay finalizes propose, finalize, and claim in three distinct
  # blocks before reserve, subscribe, and export consume the next three.
  export_height=$((expected_pftl_height + 6))
fi
fi
ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node account-assets --data-dir /var/lib/postfiat/validator-2 --account $joe --asset-id $a666" \
  > "$phase_dir/a666/joe-a666-after-export.json"
ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node account-assets --data-dir /var/lib/postfiat/validator-2 --account $joe --asset-id $pfusdc" \
  > "$phase_dir/a666/joe-pfusdc-after-export.json"
ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node navcoin-bridge-supply-status \
    --data-dir /var/lib/postfiat/validator-2 \
    --route-id pftl-a666-ethereum-wA666-usdc-v1" \
  > "$phase_dir/pftl-supply-status-after.json"
jq -e --argjson expected "$a666_balance_before" \
  '([.assets[]?.balance] | add // 0)==$expected' \
  "$phase_dir/a666/joe-a666-after-export.json" >/dev/null
jq -e --argjson expected "$pfusdc_balance_before" \
  '([.assets[]?.balance] | add // 0)==$expected' \
  "$phase_dir/a666/joe-pfusdc-after-export.json" >/dev/null
jq -e \
  --slurpfile before "$phase_dir/pftl-supply-status-before.json" \
  --argjson minted "$mint_amount" \
  --argjson base "$base_value_amount" \
  --argjson spread "$spread_amount" \
  '.invariant_holds==true
   and .paused==false
   and .authorized_valid_supply_atoms==($before[0].authorized_valid_supply_atoms+$minted)
   and .outstanding_bridge_claims_atoms==($before[0].outstanding_bridge_claims_atoms+$minted)
   and .settlement_reserve_atoms==($before[0].settlement_reserve_atoms+$base)
   and .non_nav_spread_atoms==($before[0].non_nav_spread_atoms+$spread)
   and .active_reservation_atoms==0
   and .export_entitlement_atoms==0' \
  "$phase_dir/pftl-supply-status-after.json" >/dev/null
verifier_height=$(ssh -o BatchMode=yes "root@$validator2_host" \
  '/var/lib/postfiat/validator-2/pfusdc-latency-20260727-run2/cast call 0xb79FF97EcC11574a8A78d0b5a9D7C8c2A94bF96A "latestFinalizedHeight()(uint64)" --rpc-url https://ethereum-rpc.publicnode.com')
test "$verifier_height" = "$expected_verifier_height"

ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node pftl-uniswap-receipt-witness \
    --data-dir /var/lib/postfiat/validator-2 \
    --packet-hash $packet_hash \
    --prior-checkpoint $prior_checkpoint_block_id" \
  > "$phase_dir/export-proof/receipt-witness.json"
jq -e \
  --arg packet "$packet_hash" \
  --arg prior "$prior_checkpoint_block_id" \
  --argjson height "$export_height" \
  --argjson minted "$mint_amount" \
  '.schema=="postfiat-pftl-uniswap-receipt-proof-witness-v1"
   and .prior_checkpoint_block_id==$prior
   and .receipt.packet_hash==$packet
   and .receipt.block_height==$height
   and .receipt.amount_atoms==$minted
   and .mint_packet.source_packet_hash==$packet
   and .mint_packet.mint_amount_atoms==$minted
   and .block.header.height==$height' \
  "$phase_dir/export-proof/receipt-witness.json" >/dev/null

ssh -o BatchMode=yes -p "$a100_port" "root@$a100_host" \
  "install -d -m 700 '$a100_root/export'"
scp -q -P "$a100_port" "$phase_dir/export-proof/receipt-witness.json" \
  "root@$a100_host:$a100_root/export/receipt-witness.json"
ssh -o BatchMode=yes -p "$a100_port" "root@$a100_host" \
  "SP1_PROVER=cuda '$export_prover' receipt \
    --witness '$a100_root/export/receipt-witness.json' \
    --output-dir '$a100_root/export-proof' \
    --elf '$export_elf' \
    --prove"
mkdir -p "$phase_dir/export-proof/proof-cuda"
rsync -a -e "ssh -p $a100_port" \
  "root@$a100_host:$a100_root/export-proof/" \
  "$phase_dir/export-proof/proof-cuda/"
fi
jq -e '
  .program_vkey=="0x004e44aca326861252ee5ff7863b1174635b727759b75d46b28bb28d4a7b34f9"
  and .proof_mode=="groth16"
  and .proof_bytes==356
  and .public_values_bytes==1120
' "$phase_dir/export-proof/proof-cuda/proof-report.json" >/dev/null

python3 scripts/a666-mainnet-accept-and-mint.py \
  --receipt-witness "$phase_dir/export-proof/receipt-witness.json" \
  --proof-dir "$phase_dir/export-proof/proof-cuda" \
  --state-file "$phase_dir/ethereum/preflight-state.json" \
  --expected-finalized-height "$export_height" \
  > "$phase_dir/ethereum/preflight.stdout.json"
jq -e \
  --argjson height "$expected_verifier_height" \
  --argjson balance "$expected_wrapped_balance_before" \
  --argjson supply "$expected_wrapped_supply_before" \
  '.pre_state.latest_finalized_height==$height
   and .pre_state.receipt_accepted==false
   and .pre_state.mint_paused==false
   and .pre_state.packet_consumed==false
   and .pre_state.recipient_balance_atoms==$balance
   and .pre_state.token_total_supply==$supply' \
  "$phase_dir/ethereum/preflight-state.json" >/dev/null
python3 scripts/a666-mainnet-accept-and-mint.py \
  --execute \
  --receipt-witness "$phase_dir/export-proof/receipt-witness.json" \
  --proof-dir "$phase_dir/export-proof/proof-cuda" \
  --state-file "$phase_dir/ethereum/mint-state.json" \
  --expected-finalized-height "$export_height" \
  > "$phase_dir/ethereum/execute.stdout.json"
jq -e \
  --arg digest "0x$packet_digest" \
  --argjson height "$export_height" \
  --argjson balance "$expected_wrapped_balance_before" \
  --argjson supply "$expected_wrapped_supply_before" \
  --argjson minted "$mint_amount" \
  '.phase=="minted-to-recipient"
   and .packet_digest==$digest
   and .post_state.latest_finalized_height==$height
   and .post_state.packet_consumed==true
   and .post_state.mint_paused==false
   and .post_state.recipient_balance_atoms==($balance+$minted)
   and .post_state.token_total_supply==($supply+$minted)
   and .post_state.migration_reserve_atoms==.pre_state.migration_reserve_atoms' \
  "$phase_dir/ethereum/mint-state.json" >/dev/null

destination_height=$((export_height + 1))
bash scripts/a666-mainnet-record-destination-consume.sh \
  --phase-dir "$phase_dir" \
  --workflow-id "$workflow_id" \
  --expected-pftl-height "$destination_height"

finalize_args=(--phase-dir "$phase_dir")
if "$allow_recovery_timing_exception"; then
  finalize_args+=(--allow-recovery-timing-exception)
fi
bash scripts/a666-mainnet-finalize-issue-evidence.sh "${finalize_args[@]}"
