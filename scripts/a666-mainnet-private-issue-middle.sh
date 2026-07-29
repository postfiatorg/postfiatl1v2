#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
phase_dir=
workflow_id=
expected_pftl_height=
resume=false
request_attempt=0
release_id=${A666_PFTL_RELEASE_ID:-a666-variable-nav-9ffdfb6}
hosts_file=${A666_PROPOSER_HOSTS_FILE:-docs/evidence/a666-joe-mainnet-e2e-20260728/proposer-hosts.json}
holder_key=${A666_JOE_HOLDER_KEY:-/home/postfiat/tmp/pfusdc-closed-roundtrip-20260720/keys/holder.json}

while (($#)); do
  case "$1" in
    --phase-dir) phase_dir=$2; shift 2 ;;
    --workflow-id) workflow_id=$2; shift 2 ;;
    --expected-pftl-height) expected_pftl_height=$2; shift 2 ;;
    --resume) resume=true; shift ;;
    --request-attempt) request_attempt=$2; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

for value in "$phase_dir" "$workflow_id" "$expected_pftl_height"; do
  test -n "$value"
done
[[ "$workflow_id" =~ ^[a-z0-9][a-z0-9-]{0,39}$ ]]
[[ "$expected_pftl_height" =~ ^[0-9]+$ ]]
[[ "$request_attempt" =~ ^[0-9]+$ ]]
test "$request_attempt" -le 9
if test "$request_attempt" -gt 0 && ! "$resume"; then
  echo "a nonzero request attempt is valid only with --resume" >&2
  exit 2
fi

cd "$repo"
phase_dir=$(realpath "$phase_dir")
hosts_file=$(realpath "$hosts_file")
holder_key=$(realpath "$holder_key")
ops_dir="$phase_dir/a666/ops"
manifest="$ops_dir/manifest.json"
orchard_dir="$phase_dir/orchard-private-issue"
remote_node="/opt/postfiat/releases/$release_id/postfiat-node"
remote_topology="/etc/postfiat/releases/$release_id/topology.json"
remote_root="/var/lib/postfiat/validator-2/$workflow_id-private-issue"
remote_public="$remote_root/public"
remote_private="$remote_root/private"
remote_keys="$remote_root/keys"
remote_orchard_service=http://127.0.0.1:8789
validator2_host=$(jq -er '."validator-2"' "$hosts_file")

joe=pfab9b9228942e5c529633a13aa271d5297bec6353
pfusdc=02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b
a666=521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c
route_id=pftl-a666-ethereum-wA666-usdc-v1
private_egress_policy=77e8d7b093335a9391c7aa7ed940308c055e432fd804b4b8b15eff983a0edbdb
joe_evm=0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0

test -s "$hosts_file"
test -s "$holder_key"
test -s "$manifest"
test -s "$ops_dir/03-export.ops.json"
if "$resume"; then
  test -d "$orchard_dir"
  ssh -o BatchMode=yes "root@$validator2_host" "test -d '$remote_root'"
else
  test ! -e "$orchard_dir"
  ssh -o BatchMode=yes "root@$validator2_host" "test ! -e '$remote_root'"
fi

mint_amount=$(jq -er '.mint_amount_atoms' "$manifest")
settlement_amount=$(jq -er '.settlement_value_atoms' "$manifest")
reservation_id=$(jq -er '.reservation_id' "$manifest")
subscription_nonce=$(jq -er '.subscription_nonce' "$manifest")
[[ "$mint_amount" =~ ^[1-9][0-9]*$ ]]
[[ "$settlement_amount" =~ ^[1-9][0-9]*$ ]]
[[ "$reservation_id" =~ ^[0-9a-f]{96}$ ]]
[[ "$subscription_nonce" =~ ^[0-9a-f]{64}$ ]]

mkdir -p \
  "$orchard_dir/01-pfusdc-ingress" \
  "$orchard_dir/03-private-primary-issue" \
  "$orchard_dir/04-a666-private-egress" \
  "$orchard_dir/05-export"

ssh -o BatchMode=yes "root@$validator2_host" \
  "install -d -o root -g root -m 755 '$remote_root'; \
   install -d -o postfiat -g postfiat -m 700 '$remote_private'; \
   chown postfiat:postfiat '$remote_private'; \
   install -d -o root -g root -m 700 '$remote_keys'; \
   install -d -m 755 '$remote_public'; \
   if test -e '$remote_private/pfusdc-note.json'; then \
     chown postfiat:postfiat '$remote_private/pfusdc-note.json'; \
     chmod 600 '$remote_private/pfusdc-note.json'; \
   fi; \
   if test -e '$remote_private/a666-note.json'; then \
     chown postfiat:postfiat '$remote_private/a666-note.json'; \
     chmod 600 '$remote_private/a666-note.json'; \
   fi"
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
if ! test -s "$orchard_dir/01-pfusdc-ingress/finality/summary.json"; then
  if ! test -s "$orchard_dir/01-pfusdc-ingress/pfusdc-ingress-batch.json"; then
    scp -q "$holder_key" "root@$validator2_host:$remote_keys/holder-key.json"
    ssh -o BatchMode=yes "root@$validator2_host" \
      "set -euo pipefail
       chown root:root '$remote_keys/holder-key.json'
       chmod 600 '$remote_keys/holder-key.json'
       seed=\$(openssl rand -hex 32)
       '$remote_node' asset-orchard-ingress-create \
         --data-dir /var/lib/postfiat/validator-2 \
         --key-file '$remote_keys/holder-key.json' \
         --asset-id '$pfusdc' \
         --amount '$settlement_amount' \
         --note-seed-hex \"\$seed\" \
         --ingress-file '$remote_public/pfusdc-ingress.json' \
         --note-file '$remote_private/pfusdc-note.json' \
         > '$remote_public/pfusdc-ingress-report.json'
       '$remote_node' shield-batch-asset-orchard-ingress \
         --data-dir /var/lib/postfiat/validator-2 \
         --ingress-file '$remote_public/pfusdc-ingress.json' \
         --batch-file '$remote_public/pfusdc-ingress-batch.json' \
         > '$remote_public/pfusdc-ingress-batch-report.json'
       chown postfiat:postfiat '$remote_private/pfusdc-note.json'
       chmod 600 '$remote_private/pfusdc-note.json'"
    for name in \
      pfusdc-ingress-report \
      pfusdc-ingress-batch-report \
      pfusdc-ingress-batch
    do
      scp -q "root@$validator2_host:$remote_public/$name.json" \
        "$orchard_dir/01-pfusdc-ingress/$name.json"
    done
  fi
  jq -e \
    --arg asset "$pfusdc" \
    --argjson amount "$settlement_amount" \
    '.verified==true and .asset_id==$asset and .amount==$amount' \
    "$orchard_dir/01-pfusdc-ingress/pfusdc-ingress-report.json" >/dev/null
  python3 scripts/a666-ce22-remote-finality-batch.py \
    --batch-file "$orchard_dir/01-pfusdc-ingress/pfusdc-ingress-batch.json" \
    --batch-kind shielded \
    --label "$workflow_id-pfusdc-ingress" \
    --artifact-dir "$orchard_dir/01-pfusdc-ingress/finality" \
    "${round_args[@]}"
fi
jq -e \
  --argjson height "$((expected_pftl_height + 1))" \
  '.confirmed==true and .accepted==true and .end_height==$height' \
  "$orchard_dir/01-pfusdc-ingress/finality/summary.json" >/dev/null

expires_at_height=$((expected_pftl_height + 130))
primary_issue_request_id="$workflow_id-primary-issue"
if test "$request_attempt" -gt 0; then
  primary_issue_request_id="$primary_issue_request_id-retry$request_attempt"
fi
if ! test -s "$orchard_dir/03-private-primary-issue/finality/summary.json"; then
  if ! test -s \
    "$orchard_dir/03-private-primary-issue/private-primary-issue-batch.json"
  then
    ssh -o BatchMode=yes "root@$validator2_host" \
      "set -euo pipefail
       chown postfiat:postfiat '$remote_private/pfusdc-note.json'
       chmod 600 '$remote_private/pfusdc-note.json'
       jq -n \
         --arg request_id '$primary_issue_request_id' \
         --arg input_note_path '$remote_private/pfusdc-note.json' \
         --arg route_id '$route_id' \
         --arg subscriber '$joe' \
         --arg ethereum_recipient '$joe_evm' \
         --arg reservation_id '$reservation_id' \
         --arg subscription_nonce '$subscription_nonce' \
         --arg mint_amount_atoms '$mint_amount' \
         --arg settlement_value_atoms '$settlement_amount' \
         --arg expires_at_height '$expires_at_height' \
         '{request_id:\$request_id,input_note_path:\$input_note_path,route_id:\$route_id,
           subscriber:\$subscriber,ethereum_recipient:\$ethereum_recipient,
           reservation_id:\$reservation_id,subscription_nonce:\$subscription_nonce,
           mint_amount_atoms:\$mint_amount_atoms,
           settlement_value_atoms:\$settlement_value_atoms,
           expires_at_height:\$expires_at_height}' \
         > '$remote_public/private-primary-issue-request.json'
       curl --silent --show-error --fail-with-body --max-time 300 \
         -H 'Content-Type: application/json' \
         --data-binary '@$remote_public/private-primary-issue-request.json' \
         '$remote_orchard_service/asset-orchard/private-primary-issue-actions' \
         > '$remote_public/private-primary-issue-response.json'
       jq -e '.ok==true and .verification.verified==true' \
         '$remote_public/private-primary-issue-response.json' >/dev/null
       jq '.action' '$remote_public/private-primary-issue-response.json' \
         > '$remote_public/private-primary-issue.json'
       jq '.batch' '$remote_public/private-primary-issue-response.json' \
         > '$remote_public/private-primary-issue-batch.json'
       jq '.verification' '$remote_public/private-primary-issue-response.json' \
         > '$remote_public/private-primary-issue-report.json'
       jq '{schema,request_id,timing,prover_warm:.readiness.prover_warm}' \
         '$remote_public/private-primary-issue-response.json' \
         > '$remote_public/private-primary-issue-timing.json'
       output_note_path=\$(jq -er '.output_note_path' \
         '$remote_public/private-primary-issue-response.json')
       install -o postfiat -g postfiat -m 600 \"\$output_note_path\" \
         '$remote_private/a666-note.json'
       '$remote_node' shield-batch-asset-orchard-private-primary-issue \
         --data-dir /var/lib/postfiat/validator-2 \
         --action-file '$remote_public/private-primary-issue.json' \
         --batch-file '$remote_public/private-primary-issue-batch-check.json' \
         > '$remote_public/private-primary-issue-batch-report.json'
       jq -e --slurp '.[0]==.[1]' \
         '$remote_public/private-primary-issue-batch.json' \
         '$remote_public/private-primary-issue-batch-check.json' >/dev/null"
    for name in \
      private-primary-issue-report \
      private-primary-issue-timing \
      private-primary-issue-batch-report \
      private-primary-issue-batch
    do
      scp -q "root@$validator2_host:$remote_public/$name.json" \
        "$orchard_dir/03-private-primary-issue/$name.json"
    done
  fi
  jq -e \
    --argjson mint "$mint_amount" \
    --argjson settlement "$settlement_amount" \
    '.verified==true
     and .mint_amount_atoms==$mint
     and .settlement_value_atoms==$settlement' \
    "$orchard_dir/03-private-primary-issue/private-primary-issue-report.json" >/dev/null
  python3 scripts/a666-ce22-remote-finality-batch.py \
    --batch-file \
      "$orchard_dir/03-private-primary-issue/private-primary-issue-batch.json" \
    --batch-kind shielded \
    --label "$workflow_id-private-primary-issue" \
    --artifact-dir "$orchard_dir/03-private-primary-issue/finality" \
    "${round_args[@]}"
fi
jq -e \
  --argjson height "$((expected_pftl_height + 2))" \
  '.confirmed==true and .accepted==true and .end_height==$height' \
  "$orchard_dir/03-private-primary-issue/finality/summary.json" >/dev/null

if ! test -s "$orchard_dir/04-a666-private-egress/finality/summary.json"; then
  if ! test -s "$orchard_dir/04-a666-private-egress/a666-private-egress-batch.json"
  then
    ssh -o BatchMode=yes "root@$validator2_host" \
      "set -euo pipefail
       chown postfiat:postfiat '$remote_private/a666-note.json'
       chmod 600 '$remote_private/a666-note.json'
       disclosure_hash=\$(openssl rand -hex 32)
       jq -n \
         --arg wallet_address '$joe' \
         --arg to '$joe' \
         --arg asset_id '$a666' \
         --arg amount_atoms '$mint_amount' \
         --arg input_note_path '$remote_private/a666-note.json' \
         --arg policy_id '$private_egress_policy' \
         --arg disclosure_hash \"\$disclosure_hash\" \
         '{wallet_address:\$wallet_address,to:\$to,asset_id:\$asset_id,
           amount_atoms:\$amount_atoms,input_note_path:\$input_note_path,
           policy_id:\$policy_id,disclosure_hash:\$disclosure_hash,
           disclosure_ack:true}' \
         > '$remote_public/a666-private-egress-request.json'
       curl --silent --show-error --fail-with-body --max-time 300 \
         -H 'Content-Type: application/json' \
         --data-binary '@$remote_public/a666-private-egress-request.json' \
         '$remote_orchard_service/asset-orchard/private-egress-actions' \
         > '$remote_public/a666-private-egress-response.json'
       jq -e '.ok==true and .private_egress_report.verified==true' \
         '$remote_public/a666-private-egress-response.json' >/dev/null
       jq '.egress' '$remote_public/a666-private-egress-response.json' \
         > '$remote_public/a666-private-egress.json'
       jq '.private_egress_report' \
         '$remote_public/a666-private-egress-response.json' \
         > '$remote_public/a666-private-egress-report.json'
       jq '{schema,egress_id,timing,prover_warm:.readiness.prover_warm}' \
         '$remote_public/a666-private-egress-response.json' \
         > '$remote_public/a666-private-egress-timing.json'
       '$remote_node' shield-batch-asset-orchard-private-egress \
         --data-dir /var/lib/postfiat/validator-2 \
         --egress-file '$remote_public/a666-private-egress.json' \
         --batch-file '$remote_public/a666-private-egress-batch.json' \
         > '$remote_public/a666-private-egress-batch-report.json'"
    for name in \
      a666-private-egress-report \
      a666-private-egress-timing \
      a666-private-egress-batch-report \
      a666-private-egress-batch
    do
      scp -q "root@$validator2_host:$remote_public/$name.json" \
        "$orchard_dir/04-a666-private-egress/$name.json"
    done
  fi
  jq -e \
    --arg asset "$a666" \
    --argjson amount "$mint_amount" \
    '.verified==true and .asset_id==$asset and .amount==$amount' \
    "$orchard_dir/04-a666-private-egress/a666-private-egress-report.json" >/dev/null
  python3 scripts/a666-ce22-remote-finality-batch.py \
    --batch-file \
      "$orchard_dir/04-a666-private-egress/a666-private-egress-batch.json" \
    --batch-kind shielded \
    --label "$workflow_id-a666-private-egress" \
    --artifact-dir "$orchard_dir/04-a666-private-egress/finality" \
    "${round_args[@]}"
fi
jq -e \
  --argjson height "$((expected_pftl_height + 3))" \
  '.confirmed==true and .accepted==true and .end_height==$height' \
  "$orchard_dir/04-a666-private-egress/finality/summary.json" >/dev/null
if ! test -s \
  "$orchard_dir/04-a666-private-egress/a666-private-egress-finalize.json"
then
  ssh -o BatchMode=yes "root@$validator2_host" \
    "set -euo pipefail
     egress_id=\$(jq -er '.egress_id' '$remote_public/a666-private-egress-response.json')
     jq -n --arg egress_id \"\$egress_id\" \
       '{egress_id:\$egress_id,accepted:true}' \
       > '$remote_public/a666-private-egress-finalize-request.json'
     curl --silent --show-error --fail-with-body \
       -H 'Content-Type: application/json' \
       --data-binary '@$remote_public/a666-private-egress-finalize-request.json' \
       '$remote_orchard_service/asset-orchard/private-egress-finalize' \
       > '$remote_public/a666-private-egress-finalize.json'
     jq -e '.ok==true and .accepted==true' \
       '$remote_public/a666-private-egress-finalize.json' >/dev/null"
  scp -q "root@$validator2_host:$remote_public/a666-private-egress-finalize.json" \
    "$orchard_dir/04-a666-private-egress/a666-private-egress-finalize.json"
fi

if ! test -s "$orchard_dir/05-export/finality/summary.json"; then
  python3 scripts/a666-ce22-remote-finality-op.py \
    --node-bin target/release/postfiat-node \
    --remote-runner scripts/a666-remote-sync-round.py \
    --proposer-hosts-file "$hosts_file" \
    --remote-binary "$remote_node" \
    --remote-topology "$remote_topology" \
    --ops-file "$ops_dir/03-export.ops.json" \
    --artifact-dir "$orchard_dir/05-export/finality"
fi
jq -e \
  --argjson height "$((expected_pftl_height + 4))" \
  '.confirmed==true and .accepted==true and .end_height==$height' \
  "$orchard_dir/05-export/finality/summary.json" >/dev/null

jq -n \
  --argjson start_height "$expected_pftl_height" \
  --argjson end_height "$((expected_pftl_height + 4))" \
  --argjson settlement "$settlement_amount" \
  --argjson mint "$mint_amount" \
  '{
    schema:"postfiat.a666.private_issue_middle.v1",
    verdict:"PASS",
    start_height:$start_height,
    end_height:$end_height,
    private_settlement_atoms:$settlement,
    private_nav_atoms:$mint,
    privacy:"transparent PFUSDC ingress; private PFUSDC-to-A666 primary issue; transparent A666 egress for trustless Ethereum export",
    private_material_location:"validator-2 only; mode 0600; excluded from evidence"
  }' > "$orchard_dir/summary.json"
cat "$orchard_dir/summary.json"
