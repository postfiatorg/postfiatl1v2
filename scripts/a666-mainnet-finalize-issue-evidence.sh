#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
phase_dir=
allow_recovery_timing_exception=false
ethereum_rpc=${A666_ETHEREUM_RPC:-https://ethereum-rpc.publicnode.com}

while (($#)); do
  case "$1" in
    --phase-dir) phase_dir=$2; shift 2 ;;
    --allow-recovery-timing-exception) allow_recovery_timing_exception=true; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

test -n "$phase_dir"
cd "$repo"
phase_dir=$(realpath "$phase_dir")

deposit_file="$phase_dir/deposit/deposit-result.json"
mint_state="$phase_dir/ethereum/mint-state.json"
manifest="$phase_dir/a666/ops/manifest.json"
destination_summary="$phase_dir/destination-consume/summary.json"
run_manifest="$phase_dir/run-manifest.json"
for file in \
  "$deposit_file" \
  "$mint_state" \
  "$manifest" \
  "$destination_summary" \
  "$run_manifest"
do
  test -s "$file"
done

wa666=0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5
joe_evm=0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0
uniswap_state_view=0x7fFE42C4a5DEeA5b0feC41C94C136Cf115597227
uniswap_pool_id=0xc5f1e4b5bb07c0718eddcc3d102dc751b8953ec25bb05cdc14d95419d4d16e98

deposit_tx=$(jq -er '.deposit.tx_hash' "$deposit_file")
packet_hash=$(jq -er '.packet_hash' "$manifest")
packet_digest=$(jq -er '.ethereum_packet_digest' "$manifest")
deposit_block_number=$(jq -er '.deposit.block_number' "$deposit_file")
mint_block_number=$(jq -er \
  '.transactions[] | select(.label=="consume finalized A666 mint packet") | .block_number' \
  "$mint_state")
start_height=$(jq -er '.start_height' "$run_manifest")
export_height=$(jq -er '.post_state.latest_finalized_height' "$mint_state")
end_height=$(jq -er '.pftl_height' "$destination_summary")

cast block "$deposit_block_number" --json --rpc-url "$ethereum_rpc" \
  > "$phase_dir/ethereum/deposit-block.json"
cast block "$mint_block_number" --json --rpc-url "$ethereum_rpc" \
  > "$phase_dir/ethereum/mint-block.json"
deposit_timestamp_hex=$(jq -er '.timestamp' "$phase_dir/ethereum/deposit-block.json")
mint_timestamp_hex=$(jq -er '.timestamp' "$phase_dir/ethereum/mint-block.json")
[[ "$deposit_timestamp_hex" =~ ^0x[0-9a-fA-F]+$ ]]
[[ "$mint_timestamp_hex" =~ ^0x[0-9a-fA-F]+$ ]]
deposit_block_timestamp=$((16#${deposit_timestamp_hex#0x}))
mint_block_timestamp=$((16#${mint_timestamp_hex#0x}))
deposit_to_mint_seconds=$((mint_block_timestamp - deposit_block_timestamp))

jq -n \
  --argjson deposit_block_number "$deposit_block_number" \
  --argjson deposit_block_timestamp "$deposit_block_timestamp" \
  --argjson mint_block_number "$mint_block_number" \
  --argjson mint_block_timestamp "$mint_block_timestamp" \
  --argjson elapsed "$deposit_to_mint_seconds" \
  --argjson recovery_exception "$allow_recovery_timing_exception" \
  '{
    schema:"postfiat.a666.issue_timing.v2",
    deposit_block_number:$deposit_block_number,
    deposit_block_timestamp:$deposit_block_timestamp,
    mint_block_number:$mint_block_number,
    mint_block_timestamp:$mint_block_timestamp,
    deposit_to_mint_seconds:$elapsed,
    slo_seconds:1500,
    slo_pass:($elapsed<=1500),
    recovery_timing_exception:$recovery_exception,
    timing_gate_pass:(($elapsed<=1500) or $recovery_exception)
  }' > "$phase_dir/timing.json"
jq -e '.timing_gate_pass==true' "$phase_dir/timing.json" >/dev/null

liquidity_after=$(cast call "$uniswap_state_view" \
  'getLiquidity(bytes32)(uint128)' "$uniswap_pool_id" \
  --rpc-url "$ethereum_rpc" | awk '{print $1}')
wa666_transfer_simulation=$(cast call "$wa666" \
  'transfer(address,uint256)(bool)' "$joe_evm" 1 \
  --from "$joe_evm" --rpc-url "$ethereum_rpc")
[[ "$liquidity_after" =~ ^[0-9]+$ ]]
test "$liquidity_after" -gt 0
test "$wa666_transfer_simulation" = true

liquidity_before_file="$phase_dir/ethereum/uniswap-liquidity-before.json"
comparison_available=false
liquidity_before="$liquidity_after"
if test -s "$liquidity_before_file"; then
  comparison_available=true
  liquidity_before=$(jq -er '.liquidity_atoms' "$liquidity_before_file")
  test "$liquidity_after" = "$liquidity_before"
elif ! "$allow_recovery_timing_exception"; then
  echo "missing frozen pre-issue Uniswap liquidity observation" >&2
  exit 1
fi

jq -n \
  --arg pool_id "$uniswap_pool_id" \
  --argjson comparison_available "$comparison_available" \
  --argjson liquidity_before "$liquidity_before" \
  --argjson liquidity_after "$liquidity_after" \
  --arg transfer_simulation "$wa666_transfer_simulation" \
  '{
    schema:"postfiat.a666.uniswap_eligibility.v2",
    pool_id:$pool_id,
    comparison_available:$comparison_available,
    liquidity_before:$liquidity_before,
    liquidity_after:$liquidity_after,
    liquidity_consumed:(if $comparison_available
      then $liquidity_before-$liquidity_after else null end),
    wa666_transfer_eth_call:$transfer_simulation,
    verdict:(if $liquidity_after>0
      and ((($comparison_available|not)) or $liquidity_before==$liquidity_after)
      and $transfer_simulation=="true" then "PASS" else "FAIL" end)
  }' > "$phase_dir/ethereum/uniswap-eligibility.json"
jq -e '.verdict=="PASS"' "$phase_dir/ethereum/uniswap-eligibility.json" >/dev/null

jq -n \
  --arg deposit_tx "$deposit_tx" \
  --arg packet_hash "$packet_hash" \
  --arg packet_digest "$packet_digest" \
  --argjson start_height "$start_height" \
  --argjson export_height "$export_height" \
  --argjson end_height "$end_height" \
  --slurpfile timing "$phase_dir/timing.json" \
  --slurpfile uniswap "$phase_dir/ethereum/uniswap-eligibility.json" \
  '{
    schema:"postfiat.a666.issue_acceptance.v2",
    verdict:"PASS",
    deposit_tx:$deposit_tx,
    packet_hash:$packet_hash,
    packet_digest:$packet_digest,
    start_height:$start_height,
    export_height:$export_height,
    end_height:$end_height,
    destination_consume_recorded:true,
    timing:$timing[0],
    uniswap_eligibility:$uniswap[0]
  }' > "$phase_dir/summary.json"
cat "$phase_dir/summary.json"
