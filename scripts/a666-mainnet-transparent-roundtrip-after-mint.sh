#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
phase_dir=
workflow_id=
protected_wa666_baseline=
resume_after_return_import=false

while (($#)); do
  case "$1" in
    --phase-dir) phase_dir=$2; shift 2 ;;
    --workflow-id) workflow_id=$2; shift 2 ;;
    --protected-wa666-baseline) protected_wa666_baseline=$2; shift 2 ;;
    --resume-after-return-import) resume_after_return_import=true; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
for value in "$phase_dir" "$workflow_id" "$protected_wa666_baseline"; do
  test -n "$value"
done
[[ "$workflow_id" =~ ^[a-z0-9][a-z0-9-]{0,39}$ ]]
[[ "$protected_wa666_baseline" =~ ^[0-9]+$ ]]

cd "$repo"
phase_dir=$(realpath "$phase_dir")
release_id=${A666_PFTL_RELEASE_ID:?A666_PFTL_RELEASE_ID is required}
remote_node="/opt/postfiat/releases/$release_id/postfiat-node"
remote_topology=${A666_PFTL_TOPOLOGY_PATH:?A666_PFTL_TOPOLOGY_PATH is required}
local_node=${A666_LOCAL_NODE_BIN:?A666_LOCAL_NODE_BIN is required}
hosts_file=${A666_PROPOSER_HOSTS_FILE:-docs/evidence/a666-joe-mainnet-e2e-20260728/proposer-hosts.json}
validator2_host=$(jq -er '."validator-2"' "$hosts_file")
rpc=${A666_ETHEREUM_RPC:-https://ethereum-rpc.publicnode.com}
wallet=0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0
usdc=0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48
wa666=0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5
joe=pfab9b9228942e5c529633a13aa271d5297bec6353
pfusdc=02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b
a666=521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c
nav_manifest=${A666_NAV_MANIFEST:-docs/evidence/a666-public-reserve-product-20260803/nav-e6-fresh/20260808T005948Z-e5compat/e6-ops/live-nav-mark-manifest.json}

test -s "$nav_manifest"

test -s "$phase_dir/destination-consume/summary.json"
jq -e '.verdict=="PASS"' "$phase_dir/destination-consume/summary.json" >/dev/null
mint_amount=$(jq -er '.mint_amount_atoms' "$phase_dir/a666/ops/manifest.json")
packet_binding=$(sha256sum "$phase_dir/a666/ops/manifest.json" | awk '{print $1}')
mkdir -p "$phase_dir/uniswap"

if ! "$resume_after_return_import"; then
allowance_event_id=$(date -u +%Y%m%dT%H%M%SZ)-$$
allowance_prepare_file="$phase_dir/uniswap/allowances-prepare-$allowance_event_id.json"
allowance_revoke_file="$phase_dir/uniswap/allowances-revoke-$allowance_event_id.json"
revoke_uniswap_allowances() {
  if ! test -e "$allowance_revoke_file"; then
    python3 scripts/a666-mainnet-uniswap-allowances.py revoke \
      --output "$allowance_revoke_file"
  fi
}
trap revoke_uniswap_allowances EXIT
python3 scripts/a666-mainnet-uniswap-allowances.py prepare \
  --output "$allowance_prepare_file" \
  --ttl-seconds 86400
jq -e '.verdict=="PASS" and .mode=="prepare"' "$allowance_prepare_file" >/dev/null

deadline=$(( $(date +%s) + 1800 ))
python3 scripts/pftl-uniswap-mainnet-swap.py \
  --direction wa666-to-usdc \
  --amount-in-atoms "$mint_amount" \
  --min-out-atoms 1 \
  --deadline-epoch "$deadline" \
  --rpc-url "$rpc" \
  --packet-sha256 "$packet_binding" \
  --quote-from-stateview > "$phase_dir/uniswap/forward-quote.json"
forward_fair=$(jq -er '.fair_value_estimate' "$phase_dir/uniswap/forward-quote.json")
forward_min=$((forward_fair * 98 / 100))
test "$forward_min" -gt 0
python3 scripts/pftl-uniswap-mainnet-swap.py \
  --direction wa666-to-usdc \
  --amount-in-atoms "$mint_amount" \
  --min-out-atoms "$forward_min" \
  --deadline-epoch "$deadline" \
  --rpc-url "$rpc" \
  --packet-sha256 "$packet_binding" \
  --quote-from-stateview > "$phase_dir/uniswap/forward-executable-probe.json" || true
if test "$(jq -r '.simulation.status' "$phase_dir/uniswap/forward-executable-probe.json")" != success; then
  probe_error=$(jq -r '.simulation.error' "$phase_dir/uniswap/forward-executable-probe.json")
  if [[ "$probe_error" =~ 0x8b063d73([0-9a-fA-F]{64})([0-9a-fA-F]{64}) ]]; then
    forward_executable=$(cast --to-dec "0x${BASH_REMATCH[2]}")
    forward_min=$((forward_executable * 98 / 100))
  else
    echo "forward executable quote failed with an unknown error: $probe_error" >&2
    exit 1
  fi
fi
test "$forward_min" -gt 0
python3 scripts/pftl-uniswap-mainnet-swap.py \
  --direction wa666-to-usdc \
  --amount-in-atoms "$mint_amount" \
  --min-out-atoms "$forward_min" \
  --deadline-epoch "$deadline" \
  --rpc-url "$rpc" \
  --packet-sha256 "$packet_binding" \
  --quote-from-stateview \
  --execute > "$phase_dir/uniswap/forward-execution.json"
jq -e --argjson amount "$mint_amount" --argjson minimum "$forward_min" \
  '.tx_status==1 and .input_spent_atoms==$amount and .output_received_atoms >= $minimum' \
  "$phase_dir/uniswap/forward-execution.json" >/dev/null
forward_output=$(jq -er '.output_received_atoms' "$phase_dir/uniswap/forward-execution.json")

deadline=$(( $(date +%s) + 1800 ))
reverse_binding=$(sha256sum "$phase_dir/uniswap/forward-execution.json" | awk '{print $1}')
python3 scripts/pftl-uniswap-mainnet-swap.py \
  --direction usdc-to-wa666 \
  --amount-in-atoms "$forward_output" \
  --min-out-atoms 1 \
  --deadline-epoch "$deadline" \
  --rpc-url "$rpc" \
  --packet-sha256 "$reverse_binding" \
  --quote-from-stateview > "$phase_dir/uniswap/reverse-quote.json"
reverse_fair=$(jq -er '.fair_value_estimate' "$phase_dir/uniswap/reverse-quote.json")
reverse_min=$((reverse_fair * 98 / 100))
test "$reverse_min" -gt 0
python3 scripts/pftl-uniswap-mainnet-swap.py \
  --direction usdc-to-wa666 \
  --amount-in-atoms "$forward_output" \
  --min-out-atoms "$reverse_min" \
  --deadline-epoch "$deadline" \
  --rpc-url "$rpc" \
  --packet-sha256 "$reverse_binding" \
  --quote-from-stateview > "$phase_dir/uniswap/reverse-executable-probe.json" || true
if test "$(jq -r '.simulation.status' "$phase_dir/uniswap/reverse-executable-probe.json")" != success; then
  probe_error=$(jq -r '.simulation.error' "$phase_dir/uniswap/reverse-executable-probe.json")
  if [[ "$probe_error" =~ 0x8b063d73([0-9a-fA-F]{64})([0-9a-fA-F]{64}) ]]; then
    reverse_executable=$(cast --to-dec "0x${BASH_REMATCH[2]}")
    reverse_min=$((reverse_executable * 98 / 100))
  else
    echo "reverse executable quote failed with an unknown error: $probe_error" >&2
    exit 1
  fi
fi
test "$reverse_min" -gt 0
python3 scripts/pftl-uniswap-mainnet-swap.py \
  --direction usdc-to-wa666 \
  --amount-in-atoms "$forward_output" \
  --min-out-atoms "$reverse_min" \
  --deadline-epoch "$deadline" \
  --rpc-url "$rpc" \
  --packet-sha256 "$reverse_binding" \
  --quote-from-stateview \
  --execute > "$phase_dir/uniswap/reverse-execution.json"
jq -e --argjson amount "$forward_output" --argjson minimum "$reverse_min" \
  '.tx_status==1 and .input_spent_atoms==$amount and .output_received_atoms >= $minimum' \
  "$phase_dir/uniswap/reverse-execution.json" >/dev/null
return_amount=$(jq -er '.output_received_atoms' "$phase_dir/uniswap/reverse-execution.json")
revoke_uniswap_allowances
jq -e '.verdict=="PASS" and .mode=="revoke"' "$allowance_revoke_file" >/dev/null
trap - EXIT

ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node navcoin-bridge-supply-status \
    --data-dir /var/lib/postfiat/validator-2 \
    --route-id pftl-a666-ethereum-wA666-usdc-v1" \
  > "$phase_dir/route-status-before-return.json"
jq -e --argjson amount "$return_amount" \
  '.invariant_holds==true
   and .paused==false
   and .live_value_enabled==true
   and (.available_return_import_atoms // .ethereum_spendable_supply_atoms) >= $amount' \
  "$phase_dir/route-status-before-return.json" >/dev/null

mkdir -p "$phase_dir/return/ethereum-burn"
return_nonce=$(openssl rand -hex 32)
printf '%s\n' "$return_nonce" > "$phase_dir/return/ethereum-burn/return-nonce.txt"
python3 scripts/a666-mainnet-burn-for-return.py \
  --execute \
  --amount-atoms "$return_amount" \
  --return-nonce "$return_nonce" \
  --pftl-supply-status "$phase_dir/route-status-before-return.json" \
  > "$phase_dir/return/ethereum-burn/burn.json"
jq -e --argjson amount "$return_amount" \
  '.phase=="burned" and .amount_atoms==$amount and .post_state.nonce_consumed==true' \
  "$phase_dir/return/ethereum-burn/burn.json" >/dev/null

current_height=$(ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node status --data-dir /var/lib/postfiat/validator-2" | jq -er '.block_height')
return_height=$((current_height + 1))
bash scripts/a666-mainnet-return-import.sh \
  --phase-dir "$phase_dir" \
  --workflow-id "$workflow_id" \
  --expected-pftl-height "$return_height"
jq -e '.verdict=="PASS"' "$phase_dir/return/summary.json" >/dev/null
jq -e --argjson amount "$return_amount" '.amount_atoms==$amount' \
  "$phase_dir/return/ethereum-burn/burn.json" >/dev/null
fi

forward_output=$(jq -er '.output_received_atoms' "$phase_dir/uniswap/forward-execution.json")
return_amount=$(jq -er '.output_received_atoms' "$phase_dir/uniswap/reverse-execution.json")
jq -e '.verdict=="PASS"' "$phase_dir/return/summary.json" >/dev/null
jq -e --argjson amount "$return_amount" '.amount_atoms==$amount' \
  "$phase_dir/return/ethereum-burn/burn.json" >/dev/null

ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node navcoin-bridge-supply-status --data-dir /var/lib/postfiat/validator-2 --route-id pftl-a666-ethereum-wA666-usdc-v1" \
  > "$phase_dir/route-status-before-redeem.json"
current_height=$(ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node status --data-dir /var/lib/postfiat/validator-2" | jq -er '.block_height')
python3 scripts/a666-build-transparent-redeem-op.py \
  --route-status "$phase_dir/route-status-before-redeem.json" \
  --nav-manifest "$nav_manifest" \
  --nav-amount-atoms "$return_amount" \
  --expires-at-height "$((current_height + 1000))" \
  --output-dir "$phase_dir/primary-redeem"
redeem_height=$((current_height + 1))
python3 scripts/a666-ce22-remote-finality-op.py \
  --node-bin "$local_node" \
  --remote-runner scripts/a666-remote-sync-round.py \
  --proposer-hosts-file "$hosts_file" \
  --remote-binary "$remote_node" \
  --remote-topology "$remote_topology" \
  --ops-file "$phase_dir/primary-redeem/primary-redeem.ops.json" \
  --artifact-dir "$phase_dir/primary-redeem/finality-h$redeem_height"
jq -e --argjson height "$redeem_height" \
  '.confirmed==true and .accepted==true and .end_height==$height' \
  "$phase_dir/primary-redeem/finality-h$redeem_height/summary.json" >/dev/null
settlement_output=$(jq -er '.settlement_output_atoms' "$phase_dir/primary-redeem/primary-redeem-manifest.json")

bash scripts/a666-mainnet-pfusdc-proof-egress.sh \
  --phase-dir "$phase_dir" \
  --workflow-id "$workflow_id" \
  --amount-atoms "$settlement_output"
jq -e --argjson amount "$settlement_output" \
  '.verdict=="PASS" and .ethereum_withdrawal.amount_atoms==$amount' \
  "$phase_dir/pfusdc-egress/summary.json" >/dev/null

ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node navcoin-bridge-supply-status --data-dir /var/lib/postfiat/validator-2 --route-id pftl-a666-ethereum-wA666-usdc-v1" \
  > "$phase_dir/final-pftl-supply-status.json"
ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node account-assets --data-dir /var/lib/postfiat/validator-2 --account $joe --asset-id $pfusdc" \
  > "$phase_dir/final-holder-pfusdc.json"
ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node account-assets --data-dir /var/lib/postfiat/validator-2 --account $joe --asset-id $a666" \
  > "$phase_dir/final-holder-a666.json"
final_wa666=$(cast call "$wa666" 'balanceOf(address)(uint256)' "$wallet" --rpc-url "$rpc" | awk '{print $1}')
final_usdc=$(cast call "$usdc" 'balanceOf(address)(uint256)' "$wallet" --rpc-url "$rpc" | awk '{print $1}')
test "$final_wa666" -eq "$protected_wa666_baseline"
jq -e '.invariant_holds==true and .active_reservation_atoms==0 and .export_entitlement_atoms==0' \
  "$phase_dir/final-pftl-supply-status.json" >/dev/null

jq -n \
  --arg workflow_id "$workflow_id" \
  --argjson mint_amount "$mint_amount" \
  --argjson forward_usdc "$forward_output" \
  --argjson return_amount "$return_amount" \
  --argjson settlement_output "$settlement_output" \
  --argjson protected_wa666 "$protected_wa666_baseline" \
  --argjson final_wa666 "$final_wa666" \
  --argjson final_usdc "$final_usdc" \
  --slurpfile deposit "$phase_dir/deposit/deposit-result.json" \
  --slurpfile forward "$phase_dir/uniswap/forward-execution.json" \
  --slurpfile reverse "$phase_dir/uniswap/reverse-execution.json" \
  --slurpfile returned "$phase_dir/return/summary.json" \
  --slurpfile redeem "$phase_dir/primary-redeem/primary-redeem-manifest.json" \
  --slurpfile egress "$phase_dir/pfusdc-egress/summary.json" \
  --slurpfile supply "$phase_dir/final-pftl-supply-status.json" \
  '{
    schema:"postfiat.a666.full_roundtrip_acceptance.v1",
    verdict:"PASS",
    workflow_id:$workflow_id,
    deposit:{tx:$deposit[0].deposit.tx_hash,deposit_id:$deposit[0].event.deposit_id,amount_atoms:$deposit[0].amount_atoms},
    verified_nav_issue:{mint_amount_atoms:$mint_amount,pricing_source:"governed NAV epoch 6"},
    uniswap:{forward_tx:$forward[0].tx_hash,forward_usdc_atoms:$forward_usdc,reverse_tx:$reverse[0].tx_hash,returned_wa666_atoms:$return_amount},
    return_import:{pftl_height:$returned[0].pftl_height,amount_atoms:$return_amount},
    verified_nav_redeem:{redemption_nonce:$redeem[0].redemption_nonce,pfusdc_output_atoms:$settlement_output},
    successor_egress:{withdrawal_tx:$egress[0].ethereum_withdrawal.tx,amount_atoms:$settlement_output},
    protected_wa666:{baseline_atoms:$protected_wa666,final_atoms:$final_wa666},
    final_wallet_usdc_atoms:$final_usdc,
    final_supply_invariant:$supply[0].invariant_holds
  }' > "$phase_dir/roundtrip-PASS.json"
