#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
phase_dir=
workflow_id=
release_id=${A666_PFTL_RELEASE_ID:-a666-variable-nav-9ffdfb6}
hosts_file=${A666_PROPOSER_HOSTS_FILE:-docs/evidence/a666-joe-mainnet-e2e-20260728/proposer-hosts.json}
holder_key=${A666_JOE_HOLDER_KEY:-/home/postfiat/tmp/pfusdc-closed-roundtrip-20260720/keys/holder.json}
resume=false
nav_amount_atoms=
settlement_output_atoms=

while (($#)); do
  case "$1" in
    --phase-dir) phase_dir=$2; shift 2 ;;
    --workflow-id) workflow_id=$2; shift 2 ;;
    --nav-amount-atoms) nav_amount_atoms=$2; shift 2 ;;
    --settlement-output-atoms) settlement_output_atoms=$2; shift 2 ;;
    --resume) resume=true; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

for value in "$phase_dir" "$workflow_id" "$nav_amount_atoms" "$settlement_output_atoms"; do
  test -n "$value"
done
[[ "$workflow_id" =~ ^[a-z0-9][a-z0-9-]{0,39}$ ]]
[[ "$nav_amount_atoms" =~ ^[1-9][0-9]*$ ]]
[[ "$settlement_output_atoms" =~ ^[1-9][0-9]*$ ]]
test "$settlement_output_atoms" -le "$nav_amount_atoms"

cd "$repo"
phase_dir=$(realpath "$phase_dir")
hosts_file=$(realpath "$hosts_file")
holder_key=$(realpath "$holder_key")
orchard_dir="$phase_dir/orchard"
remote_node="/opt/postfiat/releases/$release_id/postfiat-node"
remote_topology="/etc/postfiat/releases/$release_id/topology.json"
remote_root="/var/lib/postfiat/validator-2/$workflow_id-orchard"
remote_public="$remote_root/public"
remote_private="$remote_root/private"
remote_orchard_service=http://127.0.0.1:8789
validator2_host=$(jq -er '."validator-2"' "$hosts_file")
joe=pfab9b9228942e5c529633a13aa271d5297bec6353
a666=521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c
pfusdc=02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b
route_id=pftl-a666-ethereum-wA666-usdc-v1
private_egress_policy=77e8d7b093335a9391c7aa7ed940308c055e432fd804b4b8b15eff983a0edbdb

test -s "$hosts_file"
test -s "$holder_key"
if ! "$resume"; then
  test ! -e "$orchard_dir"
  ssh -o BatchMode=yes "root@$validator2_host" "test ! -e '$remote_root'"
fi
mkdir -p \
  "$orchard_dir/ingress" \
  "$orchard_dir/private-primary-redeem" \
  "$orchard_dir/pfusdc-private-egress"
ssh -o BatchMode=yes "root@$validator2_host" \
  "install -d -m 700 '$remote_private'; install -d -m 755 '$remote_public'"
ssh -o BatchMode=yes "root@$validator2_host" \
  "curl --silent --show-error --fail-with-body '$remote_orchard_service/asset-orchard/readiness' \
     > '$remote_public/resident-prover-readiness.json'; \
   jq -e '.ready==true and .prover_warm.ready==true' \
     '$remote_public/resident-prover-readiness.json' >/dev/null"
scp -q "root@$validator2_host:$remote_public/resident-prover-readiness.json" \
  "$orchard_dir/resident-prover-readiness.json"

round_args=(
  --remote-runner scripts/a666-remote-sync-batch-round.py
  --proposer-hosts-file "$hosts_file"
  --remote-binary "$remote_node"
  --remote-topology "$remote_topology"
)

if ! test -s "$orchard_dir/ingress/finality/summary.json"; then
  if ! test -s "$orchard_dir/ingress/ingress-batch.json"; then
    ssh -o BatchMode=yes "root@$validator2_host" \
      "install -d -m 700 '$remote_private'; install -d -m 755 '$remote_public'"
    scp -q "$holder_key" "root@$validator2_host:$remote_private/holder-key.json"
    ssh -o BatchMode=yes "root@$validator2_host" \
      "chmod 600 '$remote_private/holder-key.json'; \
       seed=\$(openssl rand -hex 32); \
       '$remote_node' asset-orchard-ingress-create \
         --data-dir /var/lib/postfiat/validator-2 \
         --key-file '$remote_private/holder-key.json' \
         --asset-id '$a666' \
         --amount '$nav_amount_atoms' \
         --note-seed-hex \"\$seed\" \
         --ingress-file '$remote_public/a666-ingress.json' \
         --note-file '$remote_private/a666-note.json' \
         > '$remote_public/ingress-report.json'; \
       '$remote_node' shield-batch-asset-orchard-ingress \
         --data-dir /var/lib/postfiat/validator-2 \
         --ingress-file '$remote_public/a666-ingress.json' \
         --batch-file '$remote_public/ingress-batch.json' \
         > '$remote_public/ingress-batch-report.json'; \
       chmod 600 '$remote_private/a666-note.json'"
    for name in ingress-report ingress-batch-report ingress-batch; do
      scp -q "root@$validator2_host:$remote_public/$name.json" \
        "$orchard_dir/ingress/$name.json"
    done
  fi
  python3 scripts/a666-ce22-remote-finality-batch.py \
    --batch-file "$orchard_dir/ingress/ingress-batch.json" \
    --batch-kind shielded \
    --label "$workflow_id-orchard-ingress" \
    --artifact-dir "$orchard_dir/ingress/finality" \
    "${round_args[@]}"
fi
jq -e '.confirmed==true and .accepted==true' \
  "$orchard_dir/ingress/finality/summary.json" >/dev/null

if ! test -s "$orchard_dir/private-primary-redeem/finality/summary.json"; then
  if ! test -s "$orchard_dir/private-primary-redeem/private-primary-redeem-batch.json"; then
    current_height=$(jq -er '.end_height' "$orchard_dir/ingress/finality/summary.json")
    expires_at_height=$((current_height + 128))
    ssh -o BatchMode=yes "root@$validator2_host" \
      "set -euo pipefail
       jq -n \
         --arg request_id '$workflow_id-primary-redeem' \
         --arg input_note_path '$remote_private/a666-note.json' \
         --arg route_id '$route_id' \
         --arg owner '$joe' \
         --arg settlement_recipient '$joe' \
         --arg nav_amount_atoms '$nav_amount_atoms' \
         --arg settlement_output_atoms '$settlement_output_atoms' \
         --arg expires_at_height '$expires_at_height' \
         '{request_id:\$request_id,input_note_path:\$input_note_path,
           route_id:\$route_id,owner:\$owner,
           settlement_recipient:\$settlement_recipient,
           nav_amount_atoms:\$nav_amount_atoms,
           settlement_output_atoms:\$settlement_output_atoms,
           expires_at_height:\$expires_at_height}' \
         > '$remote_public/private-primary-redeem-request.json'
       curl --silent --show-error --fail-with-body --max-time 300 \
         -H 'Content-Type: application/json' \
         --data-binary '@$remote_public/private-primary-redeem-request.json' \
         '$remote_orchard_service/asset-orchard/private-primary-redeem-actions' \
         > '$remote_public/private-primary-redeem-response.json'
       jq -e '.ok==true and .verification.verified==true' \
         '$remote_public/private-primary-redeem-response.json' >/dev/null
       jq '.action' '$remote_public/private-primary-redeem-response.json' \
         > '$remote_public/private-primary-redeem.json'
       jq '.batch' '$remote_public/private-primary-redeem-response.json' \
         > '$remote_public/private-primary-redeem-batch.json'
       jq '.verification' '$remote_public/private-primary-redeem-response.json' \
         > '$remote_public/private-primary-redeem-report.json'
       jq '{schema,request_id,timing,prover_warm:.readiness.prover_warm}' \
         '$remote_public/private-primary-redeem-response.json' \
         > '$remote_public/private-primary-redeem-timing.json'
       output_note_path=\$(jq -er '.output_note_path' \
         '$remote_public/private-primary-redeem-response.json')
       install -m 600 \"\$output_note_path\" '$remote_private/pfusdc-note.json'
       '$remote_node' shield-batch-asset-orchard-private-primary-redeem \
         --data-dir /var/lib/postfiat/validator-2 \
         --action-file '$remote_public/private-primary-redeem.json' \
         --batch-file '$remote_public/private-primary-redeem-batch-check.json' \
         > '$remote_public/private-primary-redeem-batch-report.json'
       jq -e --slurp '.[0]==.[1]' \
         '$remote_public/private-primary-redeem-batch.json' \
         '$remote_public/private-primary-redeem-batch-check.json' >/dev/null"
    for name in \
      private-primary-redeem-report \
      private-primary-redeem-timing \
      private-primary-redeem-batch-report \
      private-primary-redeem-batch
    do
      scp -q "root@$validator2_host:$remote_public/$name.json" \
        "$orchard_dir/private-primary-redeem/$name.json"
    done
  fi
  python3 scripts/a666-ce22-remote-finality-batch.py \
    --batch-file \
      "$orchard_dir/private-primary-redeem/private-primary-redeem-batch.json" \
    --batch-kind shielded \
    --label "$workflow_id-private-redeem" \
    --artifact-dir "$orchard_dir/private-primary-redeem/finality" \
    "${round_args[@]}"
fi
jq -e '.confirmed==true and .accepted==true' \
  "$orchard_dir/private-primary-redeem/finality/summary.json" >/dev/null

if ! test -s "$orchard_dir/pfusdc-private-egress/finality/summary.json"; then
  if ! test -s "$orchard_dir/pfusdc-private-egress/pfusdc-private-egress-batch.json"; then
    ssh -o BatchMode=yes "root@$validator2_host" \
      "set -euo pipefail
       disclosure_hash=\$(openssl rand -hex 32)
       jq -n \
         --arg wallet_address '$joe' \
         --arg to '$joe' \
         --arg asset_id '$pfusdc' \
         --arg amount_atoms '$settlement_output_atoms' \
         --arg input_note_path '$remote_private/pfusdc-note.json' \
         --arg policy_id '$private_egress_policy' \
         --arg disclosure_hash \"\$disclosure_hash\" \
         '{wallet_address:\$wallet_address,to:\$to,asset_id:\$asset_id,
           amount_atoms:\$amount_atoms,input_note_path:\$input_note_path,
           policy_id:\$policy_id,disclosure_hash:\$disclosure_hash,
           disclosure_ack:true}' \
         > '$remote_public/pfusdc-private-egress-request.json'
       curl --silent --show-error --fail-with-body --max-time 300 \
         -H 'Content-Type: application/json' \
         --data-binary '@$remote_public/pfusdc-private-egress-request.json' \
         '$remote_orchard_service/asset-orchard/private-egress-actions' \
         > '$remote_public/pfusdc-private-egress-response.json'
       jq -e '.ok==true and .private_egress_report.verified==true' \
         '$remote_public/pfusdc-private-egress-response.json' >/dev/null
       jq '.egress' '$remote_public/pfusdc-private-egress-response.json' \
         > '$remote_public/pfusdc-private-egress.json'
       jq '.private_egress_report' \
         '$remote_public/pfusdc-private-egress-response.json' \
         > '$remote_public/pfusdc-private-egress-report.json'
       jq '{schema,egress_id,timing,prover_warm:.readiness.prover_warm}' \
         '$remote_public/pfusdc-private-egress-response.json' \
         > '$remote_public/pfusdc-private-egress-timing.json'
       '$remote_node' shield-batch-asset-orchard-private-egress \
         --data-dir /var/lib/postfiat/validator-2 \
         --egress-file '$remote_public/pfusdc-private-egress.json' \
         --batch-file '$remote_public/pfusdc-private-egress-batch.json' \
         > '$remote_public/pfusdc-private-egress-batch-report.json'"
    for name in \
      pfusdc-private-egress-report \
      pfusdc-private-egress-timing \
      pfusdc-private-egress-batch-report \
      pfusdc-private-egress-batch
    do
      scp -q "root@$validator2_host:$remote_public/$name.json" \
        "$orchard_dir/pfusdc-private-egress/$name.json"
    done
  fi
  python3 scripts/a666-ce22-remote-finality-batch.py \
    --batch-file \
      "$orchard_dir/pfusdc-private-egress/pfusdc-private-egress-batch.json" \
    --batch-kind shielded \
    --label "$workflow_id-private-egress" \
    --artifact-dir "$orchard_dir/pfusdc-private-egress/finality" \
    "${round_args[@]}"
fi
jq -e '.confirmed==true and .accepted==true' \
  "$orchard_dir/pfusdc-private-egress/finality/summary.json" >/dev/null
if ! test -s "$orchard_dir/pfusdc-private-egress/pfusdc-private-egress-finalize.json"; then
  ssh -o BatchMode=yes "root@$validator2_host" \
    "set -euo pipefail
     egress_id=\$(jq -er '.egress_id' \
       '$remote_public/pfusdc-private-egress-response.json')
     jq -n --arg egress_id \"\$egress_id\" \
       '{egress_id:\$egress_id,accepted:true}' \
       > '$remote_public/pfusdc-private-egress-finalize-request.json'
     curl --silent --show-error --fail-with-body \
       -H 'Content-Type: application/json' \
       --data-binary '@$remote_public/pfusdc-private-egress-finalize-request.json' \
       '$remote_orchard_service/asset-orchard/private-egress-finalize' \
       > '$remote_public/pfusdc-private-egress-finalize.json'
     jq -e '.ok==true and .accepted==true' \
       '$remote_public/pfusdc-private-egress-finalize.json' >/dev/null"
  scp -q "root@$validator2_host:$remote_public/pfusdc-private-egress-finalize.json" \
    "$orchard_dir/pfusdc-private-egress/pfusdc-private-egress-finalize.json"
fi

ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node account-assets \
    --data-dir /var/lib/postfiat/validator-2 \
    --account '$joe' \
    --asset-id '$pfusdc'" \
  > "$orchard_dir/joe-pfusdc-after-private-egress.json"
jq -e --argjson minimum "$settlement_output_atoms" \
  '.assets|length==1 and .[0].balance >= $minimum' \
  "$orchard_dir/joe-pfusdc-after-private-egress.json" >/dev/null

jq -n \
  --slurpfile ingress "$orchard_dir/ingress/ingress-report.json" \
  --slurpfile redeem \
    "$orchard_dir/private-primary-redeem/private-primary-redeem-report.json" \
  --slurpfile egress \
    "$orchard_dir/pfusdc-private-egress/pfusdc-private-egress-report.json" \
  --argjson expected_nav "$nav_amount_atoms" \
  --argjson expected_settlement "$settlement_output_atoms" \
  '{
    schema:"postfiat.a666.private_primary_redeem_acceptance.v1",
    verdict:"PASS",
    privacy:"private A666 note to private pfUSDC note; only governed route economics and exit are public",
    a666_ingress:{
      amount_atoms:$ingress[0].amount,
      verified:$ingress[0].verified
    },
    private_redeem:{
      nav_amount_atoms:$redeem[0].nav_amount_atoms,
      settlement_output_atoms:$redeem[0].settlement_output_atoms,
      verified:$redeem[0].verified
    },
    private_egress:{
      amount_atoms:$egress[0].amount,
      verified:$egress[0].verified
    },
    private_material_location:"validator-2 only; mode 0600; excluded from evidence"
  }
  | if .a666_ingress.amount_atoms!=$expected_nav
      or .private_redeem.nav_amount_atoms!=$expected_nav
      or .private_redeem.settlement_output_atoms!=$expected_settlement
      or .private_egress.amount_atoms!=$expected_settlement
    then error("private redeem amount mismatch") else . end' \
  > "$orchard_dir/summary.json"
