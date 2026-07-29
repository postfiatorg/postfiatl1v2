#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
phase_dir=
workflow_id=
mint_amount_atoms=1000000

while (($#)); do
  case "$1" in
    --phase-dir) phase_dir=$2; shift 2 ;;
    --workflow-id) workflow_id=$2; shift 2 ;;
    --mint-amount-atoms) mint_amount_atoms=$2; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

for value in "$phase_dir" "$workflow_id" "$mint_amount_atoms"; do
  test -n "$value"
done
[[ "$workflow_id" =~ ^[a-z0-9][a-z0-9-]{0,39}$ ]]
[[ "$mint_amount_atoms" =~ ^[1-9][0-9]*$ ]]

cd "$repo"
test ! -e "$phase_dir"
git diff --quiet
git diff --cached --quiet

hosts_file=docs/evidence/a666-joe-mainnet-e2e-20260728/proposer-hosts.json
validator2_host=$(jq -er '."validator-2"' "$hosts_file")
release_id=a666-variable-nav-9ffdfb6
remote_node="/opt/postfiat/releases/$release_id/postfiat-node"
route_id=pftl-a666-ethereum-wA666-usdc-v1
a666=521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c
holder_key=/home/postfiat/tmp/pfusdc-closed-roundtrip-20260720/keys/holder.json
ethereum_rpc=https://ethereum-rpc.publicnode.com
wallet=0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0
usdc=0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48
wa666=0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5
controller=0x9A0262C0572fb4DB08765408eB225E207F40c3d9
verifier=0xb79FF97EcC11574a8A78d0b5a9D7C8c2A94bF96A
vault=0xaaa78FdA7062eFce769e95cd72Fc55e507BC8183
vault_verifier=0x9a45D6F1DC9DA443A88B1C336B3188FA7924D1AE
lane_manifest=docs/evidence/a666-acceptance-20260728/phase-5-transparent-redeem-verify/pfusdc-egress/recovery-epoch5/deploy/manifest.postdeploy-enriched.json
lane_manifest_sha256=b69417647e6a4bed5a3e7fa5069a0844b80a63f78020ba34f4796e373e92e904
nav_source=docs/evidence/a666-optimization-run-20260729/private-1-a666-roundtrip-nav-aware/baseline/nav-manifest.json
a100_host=${A666_A100_HOST:-194.228.55.129}
a100_port=${A666_A100_PORT:-30886}
a100_root="/workspace/a666-acceptance/live/$workflow_id"

test -s "$hosts_file"
test -s "$holder_key"
test -x target/release/postfiat-node
test "$(sha256sum "$lane_manifest" | awk '{print $1}')" = "$lane_manifest_sha256"
mkdir -p "$phase_dir"/{a666,baseline,preflight}

python3 - <<'PY' > "$phase_dir/preflight/fleet-status.json"
import importlib.util
import json
from pathlib import Path

path = Path("scripts/a666-ce22-finality-op.py")
spec = importlib.util.spec_from_file_location("a666_fleet", path)
if spec is None or spec.loader is None:
    raise RuntimeError("cannot load fleet status helper")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
rows = module.wait_for_fleet_status(
    [28650, 28651, 28652, 28653, 28654, 28655],
    45.0,
    45.0,
)
parent = rows[0]
print(json.dumps({
    "schema": "postfiat.a666.optimization_fleet_preflight.v2",
    "validator_count": len(rows),
    "height": parent["block_height"],
    "block_tip_hash": parent["block_tip_hash"],
    "state_root": parent["state_root"],
    "mempool_pending": 0,
    "build_git_revision": parent["build_git_revision"],
    "binary_sha256": parent["deployment_runtime_artifacts"]["binary_sha256"],
    "nodes": [{
        "node_id": row["node_id"],
        "height": row["block_height"],
        "block_tip_hash": row["block_tip_hash"],
        "state_root": row["state_root"],
        "mempool_pending": row["mempool_pending"],
        "build_git_revision": row["build_git_revision"],
        "binary_sha256": row["deployment_runtime_artifacts"]["binary_sha256"],
    } for row in rows],
}, indent=2, sort_keys=True))
PY

expected_pftl_height=$(jq -er '.height' "$phase_dir/preflight/fleet-status.json")
ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node status --data-dir /var/lib/postfiat/validator-2 \
    --expect-height '$expected_pftl_height'" \
  > "$phase_dir/baseline/validator-2-status.json"
ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node navcoin-bridge-supply-status \
    --data-dir /var/lib/postfiat/validator-2 \
    --route-id '$route_id'" \
  > "$phase_dir/baseline/supply-status.json"
install -m 644 "$nav_source" "$phase_dir/baseline/nav-manifest.json"

jq -e \
  --arg asset "$a666" \
  --argjson mint "$mint_amount_atoms" \
  --slurpfile nav "$phase_dir/baseline/nav-manifest.json" \
  '.schema=="postfiat-pftl-uniswap-supply-status-v2"
   and .route_id=="pftl-a666-ethereum-wA666-usdc-v1"
   and .native_nav_asset_id==$asset
   and .paused==false
   and .live_value_enabled==true
   and .invariant_holds==true
   and .pricing_nav_epoch==$nav[0].epoch
   and .pricing_reserve_packet_hash==$nav[0].reserve_packet_hash
   and .available_issue_atoms >= $mint
   and .route_supply_cap_atoms > .authorized_valid_supply_atoms' \
  "$phase_dir/baseline/supply-status.json" >/dev/null

for index in 0 1 2 3 4 5; do
  host=$(jq -er --arg validator "validator-$index" '.[$validator]' "$hosts_file")
  if ssh -o BatchMode=yes "root@$host" \
    "test ! -e '/var/lib/postfiat/validator-$index/$workflow_id'"
  then
    printf '%s\n' fresh > "$phase_dir/preflight/validator-$index-workspace.txt"
  else
    echo "validator-$index workflow workspace already exists" >&2
    exit 1
  fi
done
ssh -o BatchMode=yes "root@$validator2_host" \
  "set -euo pipefail
   for suffix in private-issue destination-consume return orchard pfusdc-egress; do
     test ! -e '/var/lib/postfiat/validator-2/$workflow_id-'\$suffix
   done"

ssh -o BatchMode=yes "root@$validator2_host" \
  "curl --silent --show-error --fail-with-body \
    http://127.0.0.1:8789/asset-orchard/readiness" \
  > "$phase_dir/preflight/resident-prover-readiness.json"
jq -e \
  --argjson height "$expected_pftl_height" \
  '.ok==true
   and .ready==true
   and .local_only==true
   and .prover_warm.ready==true
   and .prover_warm.circuits.private_egress.ready==true
   and .mirror.height==$height' \
  "$phase_dir/preflight/resident-prover-readiness.json" >/dev/null
ssh -o BatchMode=yes "root@$validator2_host" \
  "systemctl show postfiat-asset-orchard-local.service \
    -p MainPID -p NRestarts -p ExecMainStartTimestamp \
    -p MemoryCurrent -p MemoryPeak -p CPUUsageNSec -p ActiveState -p SubState" \
  > "$phase_dir/preflight/resident-prover-systemd.txt"
grep -qx 'NRestarts=0' "$phase_dir/preflight/resident-prover-systemd.txt"
grep -qx 'ActiveState=active' "$phase_dir/preflight/resident-prover-systemd.txt"
grep -qx 'SubState=running' "$phase_dir/preflight/resident-prover-systemd.txt"
ssh -o BatchMode=yes "root@$validator2_host" \
  "sha256sum /opt/postfiat/services/asset-orchard-local-service; \
   readlink -f /opt/postfiat/services/asset-orchard-local-service" \
  > "$phase_dir/preflight/resident-prover-binary.txt"

ssh -o BatchMode=yes -p "$a100_port" "root@$a100_host" \
  "set -euo pipefail
   test ! -e '$a100_root'
   test ! -e '$a100_root-pfusdc-egress'
   echo workspace=fresh
   sha256sum \
     /workspace/a666-acceptance/bin/eth-l1-mainnet-fast-lane-p0-cuda-optimized \
     /workspace/a666-acceptance/bin/pftl-uniswap-prover-cuda-optimized-20260729 \
     /workspace/a666-acceptance/live/a666-epoch5-transparent-20260728t1505z/pfusdc-egress/pfusdc-tier4-prover-cuda \
     /workspace/a666-acceptance/witness/deployed-program-004e44.elf
   nvidia-smi --query-gpu=name,memory.total,memory.used,utilization.gpu \
     --format=csv,noheader
   free -b" \
  > "$phase_dir/preflight/a100-readiness.txt"
export_prover_sha256=$(awk \
  '$2=="/workspace/a666-acceptance/bin/pftl-uniswap-prover-cuda-optimized-20260729"{print $1}' \
  "$phase_dir/preflight/a100-readiness.txt")
[[ "$export_prover_sha256" =~ ^[0-9a-f]{64}$ ]]

wallet_usdc=$(cast call "$usdc" 'balanceOf(address)(uint256)' "$wallet" \
  --rpc-url "$ethereum_rpc" | awk '{print $1}')
wallet_wa666=$(cast call "$wa666" 'balanceOf(address)(uint256)' "$wallet" \
  --rpc-url "$ethereum_rpc" | awk '{print $1}')
wa666_supply=$(cast call "$wa666" 'totalSupply()(uint256)' \
  --rpc-url "$ethereum_rpc" | awk '{print $1}')
wallet_eth=$(cast balance "$wallet" --rpc-url "$ethereum_rpc")
confirmed_nonce=$(cast nonce "$wallet" --block latest --rpc-url "$ethereum_rpc")
pending_nonce=$(cast nonce "$wallet" --block pending --rpc-url "$ethereum_rpc")
verifier_height=$(cast call "$verifier" 'latestFinalizedHeight()(uint64)' \
  --rpc-url "$ethereum_rpc" | awk '{print $1}')
vault_usdc=$(cast call "$usdc" 'balanceOf(address)(uint256)' "$vault" \
  --rpc-url "$ethereum_rpc" | awk '{print $1}')
vault_obligations=$(cast call "$vault" 'totalObligations()(uint256)' \
  --rpc-url "$ethereum_rpc" | awk '{print $1}')
vault_actual_verifier=$(cast call "$vault" 'finalityVerifier()(address)' \
  --rpc-url "$ethereum_rpc")
vault_paused=$(cast call "$vault" 'paused()(bool)' --rpc-url "$ethereum_rpc")
controller_paused=$(cast call "$controller" 'mintPaused()(bool)' \
  --rpc-url "$ethereum_rpc")
allowance=$(cast call "$usdc" 'allowance(address,address)(uint256)' \
  "$wallet" "$vault" --rpc-url "$ethereum_rpc" | awk '{print $1}')

test "$confirmed_nonce" = "$pending_nonce"
test "${vault_actual_verifier,,}" = "${vault_verifier,,}"
test "$vault_paused" = false
test "$controller_paused" = false
test "$wallet_usdc" -gt 1000000
test "$wallet_eth" -gt 10000000000000000
test "$wa666_supply" = "$(jq -er '.authorized_valid_supply_atoms' \
  "$phase_dir/baseline/supply-status.json")"

observation_block=$(cast block-number --rpc-url "$ethereum_rpc")
observation_timestamp=$(date +%s)
jq -n \
  --arg rpc "$ethereum_rpc" \
  --arg wallet "$wallet" \
  --arg vault "$vault" \
  --arg vault_verifier "$vault_actual_verifier" \
  --arg controller "$controller" \
  --arg verifier "$verifier" \
  --argjson observation_block "$observation_block" \
  --argjson observation_timestamp "$observation_timestamp" \
  --argjson wallet_usdc_atoms "$wallet_usdc" \
  --argjson wallet_wa666_atoms "$wallet_wa666" \
  --argjson wa666_total_supply_atoms "$wa666_supply" \
  --argjson wallet_eth_wei "$wallet_eth" \
  --argjson confirmed_nonce "$confirmed_nonce" \
  --argjson pending_nonce "$pending_nonce" \
  --argjson verifier_latest_finalized_height "$verifier_height" \
  --argjson vault_usdc_atoms "$vault_usdc" \
  --argjson vault_obligations_atoms "$vault_obligations" \
  --argjson usdc_allowance_to_vault_atoms "$allowance" \
  '{
    schema:"postfiat.a666.optimization_ethereum_preflight.v2",
    rpc:$rpc,
    observation_block:$observation_block,
    observation_timestamp:$observation_timestamp,
    wallet:$wallet,
    wallet_usdc_atoms:$wallet_usdc_atoms,
    wallet_wa666_atoms:$wallet_wa666_atoms,
    wa666_total_supply_atoms:$wa666_total_supply_atoms,
    wallet_eth_wei:$wallet_eth_wei,
    confirmed_nonce:$confirmed_nonce,
    pending_nonce:$pending_nonce,
    vault:$vault,
    vault_finality_verifier:$vault_verifier,
    vault_paused:false,
    vault_usdc_atoms:$vault_usdc_atoms,
    vault_obligations_atoms:$vault_obligations_atoms,
    usdc_allowance_to_vault_atoms:$usdc_allowance_to_vault_atoms,
    controller:$controller,
    controller_mint_paused:false,
    verifier:$verifier,
    verifier_latest_finalized_height:$verifier_latest_finalized_height
  }' > "$phase_dir/preflight/ethereum-state.json"

ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node blocks --data-dir /var/lib/postfiat/validator-2 \
    --from-height '$verifier_height' --limit 1" \
  > "$phase_dir/preflight/prior-checkpoint-block.json"
prior_checkpoint_block_id=$(jq -er \
  --argjson height "$verifier_height" \
  'if length==1 and .[0].header.height==$height
   then .[0].header.block_hash
   else error("missing verifier checkpoint block")
   end' "$phase_dir/preflight/prior-checkpoint-block.json")

python3 scripts/a666-mainnet-primary-issue-ops.py \
  --supply-status "$phase_dir/baseline/supply-status.json" \
  --nav-manifest "$phase_dir/baseline/nav-manifest.json" \
  --holder-key-file "$holder_key" \
  --node-bin target/release/postfiat-node \
  --output-dir "$phase_dir/a666/ops" \
  --mint-amount-atoms "$mint_amount_atoms" \
  --reservation-expires-at-height "$((expected_pftl_height + 256))" \
  > "$phase_dir/a666/ops-builder.stdout.json"

base_value_atoms=$(jq -er '.base_value_atoms' "$phase_dir/a666/ops/manifest.json")
deposit_atoms=$(jq -er '.settlement_value_atoms' "$phase_dir/a666/ops/manifest.json")
issue_spread_atoms=$(jq -er '.issue_spread_atoms' "$phase_dir/a666/ops/manifest.json")
redeem_multiplier=$(jq -er '.redeem_multiplier_bps' \
  "$phase_dir/baseline/supply-status.json")
redemption_output_atoms=$((base_value_atoms * redeem_multiplier / 10000))
orchestration_commit=$(git rev-parse HEAD)
validator_revision=$(jq -er '.build_git_revision' \
  "$phase_dir/preflight/fleet-status.json")
validator_binary=$(jq -er '.binary_sha256' \
  "$phase_dir/preflight/fleet-status.json")
resident_binary=$(awk 'NR==1{print $1}' \
  "$phase_dir/preflight/resident-prover-binary.txt")

jq -n \
  --arg created_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg workflow_id "$workflow_id" \
  --arg orchestration_commit "$orchestration_commit" \
  --arg validator_release "$release_id" \
  --arg validator_revision "$validator_revision" \
  --arg validator_binary_sha256 "$validator_binary" \
  --arg resident_binary_sha256 "$resident_binary" \
  --arg export_prover_binary_sha256 "$export_prover_sha256" \
  --arg lane_manifest "$lane_manifest" \
  --arg lane_manifest_sha256 "$lane_manifest_sha256" \
  --arg prior_checkpoint_block_id "$prior_checkpoint_block_id" \
  --argjson expected_pftl_height "$expected_pftl_height" \
  --argjson expected_verifier_height "$verifier_height" \
  --argjson mint_atoms "$mint_amount_atoms" \
  --argjson issue_base_value_atoms "$base_value_atoms" \
  --argjson deposit_atoms "$deposit_atoms" \
  --argjson issue_spread_atoms "$issue_spread_atoms" \
  --argjson redemption_output_atoms "$redemption_output_atoms" \
  --argjson expected_wrapped_balance_before "$wallet_wa666" \
  --argjson expected_wrapped_supply_before "$wa666_supply" \
  '{
    schema:"postfiat.a666.optimization_run_manifest.v4",
    created_at:$created_at,
    objective:"fresh intervention-free private issue and private redemption each within 1500 seconds",
    orchestration_commit:$orchestration_commit,
    validator_release:$validator_release,
    validator_revision:$validator_revision,
    validator_binary_sha256:$validator_binary_sha256,
    resident_prover:{binary_sha256:$resident_binary_sha256},
    export_prover:{
      binary_sha256:$export_prover_binary_sha256,
      redundant_host_execute_skipped:true
    },
    workflow_id:$workflow_id,
    pfusdc_deposit_lane:{
      route_epoch:5,
      vault:"0xaaa78fda7062efce769e95cd72fc55e507bc8183",
      verifier:"0x9a45d6f1dc9da443a88b1c336b3188fa7924d1ae",
      deployment_manifest:$lane_manifest,
      deployment_manifest_sha256:$lane_manifest_sha256
    },
    expected_pftl_height:$expected_pftl_height,
    start_height:$expected_pftl_height,
    expected_verifier_height:$expected_verifier_height,
    prior_checkpoint_block_id:$prior_checkpoint_block_id,
    amounts:{
      mint_atoms:$mint_atoms,
      issue_base_value_atoms:$issue_base_value_atoms,
      deposit_atoms:$deposit_atoms,
      issue_spread_atoms:$issue_spread_atoms,
      redemption_output_atoms:$redemption_output_atoms
    },
    expected_wrapped_balance_before:$expected_wrapped_balance_before,
    expected_wrapped_supply_before:$expected_wrapped_supply_before,
    post_funding_code_changes_permitted:false,
    post_funding_manual_state_repair_permitted:false,
    latency_optimizations:{
      exact_preapproval_before_slo_clock:true,
      ethereum_deposit_head_slot_in_epoch:28,
      full_finality_policy_unchanged:true,
      same_block_pfusdc_propose_finalize_claim:true,
      prewarmed_resident_issue_rounds:5,
      expected_issue_export_height:($expected_pftl_height+5)
    },
    issue_slo_seconds:1500,
    redemption_slo_seconds:1500
  }' > "$phase_dir/run-manifest.json"

jq -n \
  --arg prepare "$(sha256sum scripts/a666-mainnet-prepare-private-optimization-run.sh | awk '{print $1}')" \
  --arg run "$(sha256sum scripts/a666-mainnet-run-private-optimization.sh | awk '{print $1}')" \
  --arg deposit "$(sha256sum scripts/a666-mainnet-pfusdc-deposit.py | awk '{print $1}')" \
  --arg deposit_window "$(sha256sum scripts/a666-wait-ethereum-deposit-window.py | awk '{print $1}')" \
  --arg resident_rounds "$(sha256sum scripts/a666-resident-rounds.py | awk '{print $1}')" \
  --arg relay "$(sha256sum scripts/a666-mainnet-pfusdc-relay.sh | awk '{print $1}')" \
  --arg finality_batch "$(sha256sum scripts/a666-ce22-remote-finality-batch.py | awk '{print $1}')" \
  --arg finality_op "$(sha256sum scripts/a666-ce22-remote-finality-op.py | awk '{print $1}')" \
  --arg issue "$(sha256sum scripts/a666-mainnet-transparent-issue-after-deposit.sh | awk '{print $1}')" \
  --arg private_issue "$(sha256sum scripts/a666-mainnet-private-issue-middle.sh | awk '{print $1}')" \
  --arg consume "$(sha256sum scripts/a666-mainnet-record-destination-consume.sh | awk '{print $1}')" \
  --arg redeem "$(sha256sum scripts/a666-mainnet-private-roundtrip-after-mint.sh | awk '{print $1}')" \
  --arg private_redeem "$(sha256sum scripts/a666-mainnet-private-primary-redeem.sh | awk '{print $1}')" \
  --arg egress "$(sha256sum scripts/a666-mainnet-pfusdc-proof-egress.sh | awk '{print $1}')" \
  --arg score "$(sha256sum scripts/a666-score-private-optimization-run.py | awk '{print $1}')" \
  '{
    schema:"postfiat.a666.optimization_script_hashes.v2",
    scripts:{
      "scripts/a666-mainnet-prepare-private-optimization-run.sh":$prepare,
      "scripts/a666-mainnet-run-private-optimization.sh":$run,
      "scripts/a666-mainnet-pfusdc-deposit.py":$deposit,
      "scripts/a666-wait-ethereum-deposit-window.py":$deposit_window,
      "scripts/a666-resident-rounds.py":$resident_rounds,
      "scripts/a666-mainnet-pfusdc-relay.sh":$relay,
      "scripts/a666-ce22-remote-finality-batch.py":$finality_batch,
      "scripts/a666-ce22-remote-finality-op.py":$finality_op,
      "scripts/a666-mainnet-transparent-issue-after-deposit.sh":$issue,
      "scripts/a666-mainnet-private-issue-middle.sh":$private_issue,
      "scripts/a666-mainnet-record-destination-consume.sh":$consume,
      "scripts/a666-mainnet-private-roundtrip-after-mint.sh":$redeem,
      "scripts/a666-mainnet-private-primary-redeem.sh":$private_redeem,
      "scripts/a666-mainnet-pfusdc-proof-egress.sh":$egress,
      "scripts/a666-score-private-optimization-run.py":$score
    }
  }' > "$phase_dir/script-sha256.json"

find "$phase_dir" -type f ! -name pre-funding-sha256.txt -print0 \
  | sort -z | xargs -0 sha256sum > "$phase_dir/pre-funding-sha256.txt"
echo "A666_OPTIMIZATION_PREPARE: PASS phase=$phase_dir workflow=$workflow_id"
