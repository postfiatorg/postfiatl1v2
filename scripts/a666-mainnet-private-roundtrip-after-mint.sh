#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
phase_dir=
workflow_id=
resume=false

while (($#)); do
  case "$1" in
    --phase-dir) phase_dir=$2; shift 2 ;;
    --workflow-id) workflow_id=$2; shift 2 ;;
    --resume) resume=true; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

for value in "$phase_dir" "$workflow_id"; do
  test -n "$value"
done
[[ "$workflow_id" =~ ^[a-z0-9][a-z0-9-]{0,39}$ ]]

cd "$repo"
phase_dir=$(realpath "$phase_dir")
return_dir="$phase_dir/return"
burn_dir="$return_dir/ethereum-burn"
hosts_file=${A666_PROPOSER_HOSTS_FILE:-docs/evidence/a666-joe-mainnet-e2e-20260728/proposer-hosts.json}
hosts_file=$(realpath "$hosts_file")
validator2_host=$(jq -er '."validator-2"' "$hosts_file")
release_id=${A666_PFTL_RELEASE_ID:-a666-private-redeem-9061829}
remote_node="/opt/postfiat/releases/$release_id/postfiat-node"
route_id=pftl-a666-ethereum-wA666-usdc-v1

test -s "$phase_dir/destination-consume/summary.json"
jq -e '.verdict=="PASS"' "$phase_dir/destination-consume/summary.json" >/dev/null

if ! test -s "$burn_dir/burn.json"; then
  test ! -e "$burn_dir"
  mkdir -p "$burn_dir"
  return_nonce=$(openssl rand -hex 32)
  printf '%s\n' "$return_nonce" > "$burn_dir/return-nonce.txt"
  python3 scripts/a666-mainnet-burn-for-return.py \
    --execute \
    --amount-atoms 1000000 \
    --return-nonce "$return_nonce" \
    > "$burn_dir/burn.json"
fi
jq -e \
  '.phase=="burned"
   and .amount_atoms==1000000
   and .event_log_index==1
   and (.pre_state.recipient_balance_atoms-.post_state.recipient_balance_atoms)==1000000
   and (.pre_state.token_total_supply-.post_state.token_total_supply)==1000000
   and .post_state.nonce_consumed==true' \
  "$burn_dir/burn.json" >/dev/null

if ! test -s "$return_dir/summary.json"; then
  current_height=$(ssh -o BatchMode=yes "root@$validator2_host" \
    "$remote_node status --data-dir /var/lib/postfiat/validator-2" \
    | jq -er '.block_height')
  return_height=$((current_height + 1))
  bash scripts/a666-mainnet-return-import.sh \
    --phase-dir "$phase_dir" \
    --workflow-id "$workflow_id" \
    --expected-pftl-height "$return_height"
fi

private_args=(
  --phase-dir "$phase_dir"
  --workflow-id "$workflow_id"
)
if "$resume"; then
  private_args+=(--resume)
fi
if ! test -s "$phase_dir/orchard/summary.json"; then
  bash scripts/a666-mainnet-private-primary-redeem.sh "${private_args[@]}"
fi
jq -e '.verdict=="PASS"' "$phase_dir/orchard/summary.json" >/dev/null

egress_args=(
  --phase-dir "$phase_dir"
  --workflow-id "$workflow_id"
)
if "$resume"; then
  egress_args+=(--resume)
fi
if ! test -s "$phase_dir/pfusdc-egress/summary.json"; then
  bash scripts/a666-mainnet-pfusdc-proof-egress.sh "${egress_args[@]}"
fi
jq -e '.verdict=="PASS"' "$phase_dir/pfusdc-egress/summary.json" >/dev/null

ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node navcoin-bridge-supply-status \
    --data-dir /var/lib/postfiat/validator-2 \
    --route-id '$route_id'" \
  > "$phase_dir/final-pftl-supply-status.json"
jq -e \
  '.invariant_holds==true
   and .active_reservation_atoms==0
   and .export_entitlement_atoms==0
   and .outstanding_bridge_claims_atoms==0
   and .ethereum_spendable_supply_atoms==0' \
  "$phase_dir/final-pftl-supply-status.json" >/dev/null

jq -n \
  --slurpfile destination "$phase_dir/destination-consume/summary.json" \
  --slurpfile returned "$phase_dir/return/summary.json" \
  --slurpfile private_redeem "$phase_dir/orchard/summary.json" \
  --slurpfile egress "$phase_dir/pfusdc-egress/summary.json" \
  --slurpfile supply "$phase_dir/final-pftl-supply-status.json" \
  '{
    schema:"postfiat.a666.private_roundtrip_after_mint_acceptance.v1",
    verdict:"PASS",
    stages:{
      destination_consume:$destination[0].verdict,
      return_import:$returned[0].verdict,
      private_primary_redeem:$private_redeem[0].verdict,
      pfusdc_proof_egress:$egress[0].verdict
    },
    final_supply_invariant:$supply[0].invariant_holds,
    final_ethereum_spendable_supply_atoms:$supply[0].ethereum_spendable_supply_atoms,
    private_material:"validator-2 only; never copied into evidence"
  }' > "$phase_dir/private-roundtrip-summary.json"
