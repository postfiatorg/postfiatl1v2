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
a100_host=${A666_A100_HOST:-194.228.55.129}
a100_port=${A666_A100_PORT:-30886}
validator2_host=${A666_VALIDATOR2_HOST:-66.42.48.39}
release_id=${A666_PFTL_RELEASE_ID:-a666-private-redeem-9061829}

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

cd "$repo"
phase_dir=$(realpath "$phase_dir")
deposit_file="$phase_dir/deposit/deposit-result.json"
ops_dir="$phase_dir/a666/ops"
test -s "$deposit_file"
test -s "$ops_dir/manifest.json"
for directory in ingress pftl export-proof ethereum; do
  mkdir -p "$phase_dir/$directory"
done

deposit_tx=$(jq -er '.deposit.tx_hash' "$deposit_file")
deposit_id=$(jq -er '.event.deposit_id | ltrimstr("0x")' "$deposit_file")
packet_hash=$(jq -er '.packet_hash' "$ops_dir/manifest.json")
packet_digest=$(jq -er '.ethereum_packet_digest' "$ops_dir/manifest.json")
mint_amount=$(jq -er '.mint_amount_atoms' "$ops_dir/manifest.json")
settlement_amount=$(jq -er '.settlement_value_atoms' "$ops_dir/manifest.json")
test "$mint_amount" = 1000000
test "$settlement_amount" = 1005000
jq -e '.verdict=="PASS" and .amount_atoms==1005000' "$deposit_file" >/dev/null

remote_node=/opt/postfiat/releases/$release_id/postfiat-node
remote_topology=/etc/postfiat/releases/$release_id/topology.json
remote_run="/var/lib/postfiat/validator-2/$workflow_id"
a100_root="/workspace/a666-acceptance/live/$workflow_id"
ingress_prover=/workspace/a666-acceptance/bin/eth-l1-mainnet-fast-lane-p0-cuda-optimized
export_prover=/workspace/a666-acceptance/bin/pftl-uniswap-prover-cuda
export_elf=/workspace/a666-acceptance/witness/deployed-program-004e44.elf
joe=pfab9b9228942e5c529633a13aa271d5297bec6353
pfusdc=02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b
a666=521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c
ethereum_rpc=https://ethereum-rpc.publicnode.com
wa666=0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5
joe_evm=0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0
uniswap_state_view=0x7fFE42C4a5DEeA5b0feC41C94C136Cf115597227
uniswap_pool_id=0xc5f1e4b5bb07c0718eddcc3d102dc751b8953ec25bb05cdc14d95419d4d16e98

ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node status --data-dir /var/lib/postfiat/validator-2 --expect-height $expected_pftl_height" \
  > "$phase_dir/pftl/status-before.json"
uniswap_liquidity_before=$(cast call "$uniswap_state_view" \
  'getLiquidity(bytes32)(uint128)' "$uniswap_pool_id" \
  --rpc-url "$ethereum_rpc" | awk '{print $1}')
[[ "$uniswap_liquidity_before" =~ ^[0-9]+$ ]]
test "$uniswap_liquidity_before" -gt 0

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
  ssh -o BatchMode=yes -p "$a100_port" "root@$a100_host" \
    "test -s '$a100_root/ingress-proof/proof-calldata.bin'; \
     test -s '$a100_root/ingress-proof/public-values.bin'"
else
  ssh -o BatchMode=yes -p "$a100_port" "root@$a100_host" \
    "test ! -e '$a100_root'; install -d -m 700 '$a100_root/ingress'"
  scp -q -P "$a100_port" "$phase_dir/ingress/capture-deployment.json" \
    "root@$a100_host:$a100_root/ingress/deployment.json"
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
  '.deposit_id==$deposit_id and .amount_atoms==1005000' \
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
    "test -d '$remote_run'; test -s '$remote_run/ingress-proof/proof-calldata.bin'"
else
  ssh -o BatchMode=yes "root@$validator2_host" \
    "test ! -e '$remote_run'; install -d -o root -g root -m 700 '$remote_run/ingress-proof'"
fi
rsync -a "$phase_dir/ingress/proof-cuda/" \
  "root@$validator2_host:$remote_run/ingress-proof/"
DEPOSIT_TX="$deposit_tx" \
EXPECTED_HOLDER_ATOMS=1805000 \
PFTL_NODE_BIN="$remote_node" \
PFTL_TOPOLOGY="$remote_topology" \
PFTL_POLICY_HASH=5025bdfe92669e3d8f81ce7e739fd132063261b92ef7e7ee7db19b2762e88b736bd40cd4826375e041584533f4137158 \
PFTL_LABEL_SUFFIX=$(if "$resume_after_ingress_proof"; then printf '%s' -retry1; fi) \
PFTL_VAULT_ADDRESS=0xaaa78FdA7062eFce769e95cd72Fc55e507BC8183 \
PFTL_RUN_DIR="$remote_run" \
PFTL_PROOF_DIR="$remote_run/ingress-proof" \
PFTL_LOCAL_EVIDENCE="$phase_dir/pftl" \
bash "$repo/docs/evidence/a666-acceptance-20260728/phase-1-transparent-issue-debug/pftl/relay.sh"

round_args=(
  --node-bin target/release/postfiat-node
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
jq -e '.assets|length==1 and .[0].balance==800000' \
  "$phase_dir/a666/joe-pfusdc-after-subscribe.json" >/dev/null
jq -e '.assets|length==1 and .[0].balance==1000000' \
  "$phase_dir/a666/joe-a666-after-subscribe.json" >/dev/null

python3 scripts/a666-ce22-remote-finality-op.py \
  --ops-file "$ops_dir/03-export.ops.json" \
  --artifact-dir "$phase_dir/a666/03-export-round" \
  "${round_args[@]}"
export_height=$((expected_pftl_height + 5))
ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node account-assets --data-dir /var/lib/postfiat/validator-2 --account $joe --asset-id $a666" \
  > "$phase_dir/a666/joe-a666-after-export.json"
ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node navcoin-bridge-supply-status \
    --data-dir /var/lib/postfiat/validator-2 \
    --route-id pftl-a666-ethereum-wA666-usdc-v1" \
  > "$phase_dir/pftl-supply-status-after.json"
jq -e '.assets|length==0' "$phase_dir/a666/joe-a666-after-export.json" >/dev/null
jq -e \
  --slurpfile before "$phase_dir/pftl-supply-status-before.json" \
  '.invariant_holds==true
   and .paused==false
   and .authorized_valid_supply_atoms==($before[0].authorized_valid_supply_atoms+1000000)
   and .outstanding_bridge_claims_atoms==($before[0].outstanding_bridge_claims_atoms+1000000)
   and .settlement_reserve_atoms==($before[0].settlement_reserve_atoms+1000000)
   and .non_nav_spread_atoms==($before[0].non_nav_spread_atoms+5000)
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
  '.schema=="postfiat-pftl-uniswap-receipt-proof-witness-v1"
   and .prior_checkpoint_block_id==$prior
   and .receipt.packet_hash==$packet
   and .receipt.block_height==$height
   and .receipt.amount_atoms==1000000
   and .mint_packet.source_packet_hash==$packet
   and .mint_packet.mint_amount_atoms==1000000
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
  '.phase=="minted-to-recipient"
   and .packet_digest==$digest
   and .post_state.latest_finalized_height==$height
   and .post_state.packet_consumed==true
   and .post_state.mint_paused==false
   and .post_state.recipient_balance_atoms==($balance+1000000)
   and .post_state.token_total_supply==($supply+1000000)
   and .post_state.migration_reserve_atoms==.pre_state.migration_reserve_atoms' \
  "$phase_dir/ethereum/mint-state.json" >/dev/null

destination_height=$((export_height + 1))
bash scripts/a666-mainnet-record-destination-consume.sh \
  --phase-dir "$phase_dir" \
  --workflow-id "$workflow_id" \
  --expected-pftl-height "$destination_height"

deposit_block_number=$(jq -er '.deposit.block_number' "$deposit_file")
mint_block_number=$(jq -er '.transactions[-1].block_number' "$phase_dir/ethereum/mint-state.json")
cast block "$deposit_block_number" --json --rpc-url "$ethereum_rpc" \
  > "$phase_dir/ethereum/deposit-block.json"
cast block "$mint_block_number" --json --rpc-url "$ethereum_rpc" \
  > "$phase_dir/ethereum/mint-block.json"
deposit_block_timestamp=$((16#$(jq -er '.timestamp|ltrimstr("0x")' "$phase_dir/ethereum/deposit-block.json")))
mint_block_timestamp=$((16#$(jq -er '.timestamp|ltrimstr("0x")' "$phase_dir/ethereum/mint-block.json")))
deposit_to_mint_seconds=$((mint_block_timestamp - deposit_block_timestamp))
jq -n \
  --argjson deposit_block_number "$deposit_block_number" \
  --argjson deposit_block_timestamp "$deposit_block_timestamp" \
  --argjson mint_block_number "$mint_block_number" \
  --argjson mint_block_timestamp "$mint_block_timestamp" \
  --argjson elapsed "$deposit_to_mint_seconds" \
  '{
    schema:"postfiat.a666.transparent_issue_timing.v1",
    deposit_block_number:$deposit_block_number,
    deposit_block_timestamp:$deposit_block_timestamp,
    mint_block_number:$mint_block_number,
    mint_block_timestamp:$mint_block_timestamp,
    deposit_to_mint_seconds:$elapsed,
    slo_seconds:1500,
    slo_pass:($elapsed<=1500)
  }' > "$phase_dir/timing.json"
jq -e '.slo_pass==true' "$phase_dir/timing.json" >/dev/null

uniswap_liquidity_after=$(cast call "$uniswap_state_view" \
  'getLiquidity(bytes32)(uint128)' "$uniswap_pool_id" \
  --rpc-url "$ethereum_rpc" | awk '{print $1}')
wa666_transfer_simulation=$(cast call "$wa666" \
  'transfer(address,uint256)(bool)' "$joe_evm" 1 \
  --from "$joe_evm" --rpc-url "$ethereum_rpc")
test "$uniswap_liquidity_after" = "$uniswap_liquidity_before"
test "$wa666_transfer_simulation" = true
jq -n \
  --arg pool_id "$uniswap_pool_id" \
  --argjson liquidity_before "$uniswap_liquidity_before" \
  --argjson liquidity_after "$uniswap_liquidity_after" \
  --arg transfer_simulation "$wa666_transfer_simulation" \
  '{
    schema:"postfiat.a666.uniswap_eligibility.v1",
    pool_id:$pool_id,
    liquidity_before:$liquidity_before,
    liquidity_after:$liquidity_after,
    liquidity_consumed:($liquidity_before-$liquidity_after),
    wa666_transfer_eth_call:$transfer_simulation,
    verdict:(if $liquidity_before>0
      and $liquidity_before==$liquidity_after
      and $transfer_simulation=="true" then "PASS" else "FAIL" end)
  }' > "$phase_dir/ethereum/uniswap-eligibility.json"
jq -e '.verdict=="PASS"' "$phase_dir/ethereum/uniswap-eligibility.json" >/dev/null
jq -n \
  --arg deposit_tx "$deposit_tx" \
  --arg packet_hash "$packet_hash" \
  --arg packet_digest "$packet_digest" \
  --argjson start_height "$expected_pftl_height" \
  --argjson export_height "$export_height" \
  --argjson end_height "$destination_height" \
  --slurpfile timing "$phase_dir/timing.json" \
  '{schema:"postfiat.a666.transparent_issue_acceptance.v1",verdict:"PASS",deposit_tx:$deposit_tx,packet_hash:$packet_hash,packet_digest:$packet_digest,start_height:$start_height,export_height:$export_height,end_height:$end_height,destination_consume_recorded:true,timing:$timing[0]}' \
  > "$phase_dir/summary.json"
cat "$phase_dir/summary.json"
