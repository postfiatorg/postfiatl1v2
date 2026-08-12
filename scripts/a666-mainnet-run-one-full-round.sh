#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
round=
campaign_dir=
amount_atoms=10000000
run_label=
workflow_id=

while (($#)); do
  case "$1" in
    --round) round=$2; shift 2 ;;
    --campaign-dir) campaign_dir=$2; shift 2 ;;
    --amount-atoms) amount_atoms=$2; shift 2 ;;
    --run-label) run_label=$2; shift 2 ;;
    --workflow-id) workflow_id=$2; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
[[ "$amount_atoms" =~ ^[1-9][0-9]*$ ]]
test -n "$campaign_dir"
test "$amount_atoms" -eq 10000000

# The original six-round acceptance campaign used --round 01..06.  Wallet UX
# invocations need fresh, replay-safe identifiers after that campaign, so they
# supply an explicit run label and workflow id instead.  Keep the old interface
# for reproducibility while refusing mixed or partially specified identities.
if test -n "$round"; then
  [[ "$round" =~ ^0[1-6]$ ]]
  test -z "$run_label"
  test -z "$workflow_id"
  run_label="round-$round"
  workflow_id="a666-rt${round}-20260810"
else
  [[ "$run_label" =~ ^[a-z0-9][a-z0-9-]{0,63}$ ]]
  [[ "$workflow_id" =~ ^[a-z0-9][a-z0-9-]{0,39}$ ]]
fi

cd "$repo"
campaign_dir=$(realpath -m "$campaign_dir")
phase_dir="$campaign_dir/$run_label"
test ! -e "$phase_dir"
release_id=${A666_PFTL_RELEASE_ID:?A666_PFTL_RELEASE_ID is required}
remote_node="/opt/postfiat/releases/$release_id/postfiat-node"
local_node=${A666_LOCAL_NODE_BIN:?A666_LOCAL_NODE_BIN is required}
hosts_file=${A666_PROPOSER_HOSTS_FILE:-docs/evidence/a666-joe-mainnet-e2e-20260728/proposer-hosts.json}
validator2_host=$(jq -er '."validator-2"' "$hosts_file")
rpc=${A666_ETHEREUM_RPC:-https://ethereum-rpc.publicnode.com}
wallet=0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0
wa666=0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5
nav_manifest=${A666_NAV_MANIFEST:-docs/evidence/a666-public-reserve-product-20260803/nav-e6-fresh/20260808T005948Z-e5compat/e6-ops/live-nav-mark-manifest.json}
holder_key=/home/postfiat/tmp/pfusdc-closed-roundtrip-20260720/keys/holder.json

test -s "$nav_manifest"
test -s "$holder_key"

status=$(ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node status --data-dir /var/lib/postfiat/validator-2")
start_height=$(jq -er '.block_height' <<<"$status")
current_tip=$(jq -er '.block_tip_hash' <<<"$status")
test "$(jq -er '.mempool_pending' <<<"$status")" -eq 0
[[ "$current_tip" =~ ^[0-9a-f]{96}$ ]]
expected_verifier_height=$(cast call 0xb79FF97EcC11574a8A78d0b5a9D7C8c2A94bF96A \
  'latestFinalizedHeight()(uint64)' --rpc-url "$rpc" | awk '{print $1}')
checkpoint_block=$(ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node blocks --data-dir /var/lib/postfiat/validator-2 --from-height $expected_verifier_height --limit 1")
test "$(jq -er '.[0].header.height' <<<"$checkpoint_block")" -eq "$expected_verifier_height"
prior_checkpoint=$(jq -er '.[0].header.block_hash' <<<"$checkpoint_block")
[[ "$prior_checkpoint" =~ ^[0-9a-f]{96}$ ]]
protected_wa666=$(cast call "$wa666" 'balanceOf(address)(uint256)' "$wallet" \
  --rpc-url "$rpc" | awk '{print $1}')
wrapped_supply=$(cast call "$wa666" 'totalSupply()(uint256)' --rpc-url "$rpc" | awk '{print $1}')
for value in "$start_height" "$expected_verifier_height" "$protected_wa666" "$wrapped_supply"; do
  [[ "$value" =~ ^[0-9]+$ ]]
done

install -d -m 700 "$phase_dir/a666"
ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node navcoin-bridge-supply-status --data-dir /var/lib/postfiat/validator-2 --route-id pftl-a666-ethereum-wA666-usdc-v1" \
  > "$phase_dir/route-status-before.json"
jq -e '.invariant_holds==true and .paused==false and .pricing_nav_epoch==6' \
  "$phase_dir/route-status-before.json" >/dev/null
python3 scripts/a666-mainnet-primary-issue-ops.py \
  --supply-status "$phase_dir/route-status-before.json" \
  --nav-manifest "$nav_manifest" \
  --holder-key-file "$holder_key" \
  --node-bin "$local_node" \
  --output-dir "$phase_dir/a666/ops" \
  --mint-amount-atoms 11012575 \
  --reservation-expires-at-height 2000
jq -e --argjson amount "$amount_atoms" \
  '.settlement_value_atoms==$amount and .mint_amount_atoms==11012575' \
  "$phase_dir/a666/ops/manifest.json" >/dev/null

python3 scripts/a666-epoch6-successor-deposit-generic.py \
  --amount-atoms "$amount_atoms" \
  --output-dir "$phase_dir/deposit" \
  --workflow-id "$workflow_id"
jq -e --argjson amount "$amount_atoms" \
  '.verdict=="PASS" and .amount_atoms==$amount' "$phase_dir/deposit/deposit-result.json" >/dev/null

bash scripts/a666-mainnet-transparent-issue-after-deposit.sh \
  --phase-dir "$phase_dir" \
  --workflow-id "$workflow_id" \
  --expected-pftl-height "$start_height" \
  --prior-checkpoint-block-id "$prior_checkpoint" \
  --expected-verifier-height "$expected_verifier_height" \
  --expected-wrapped-balance-before "$protected_wa666" \
  --expected-wrapped-supply-before "$wrapped_supply"

bash scripts/a666-mainnet-transparent-roundtrip-after-mint.sh \
  --phase-dir "$phase_dir" \
  --workflow-id "$workflow_id" \
  --protected-wa666-baseline "$protected_wa666"
jq -e '.verdict=="PASS"' "$phase_dir/roundtrip-PASS.json" >/dev/null
